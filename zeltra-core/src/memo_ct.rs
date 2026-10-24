//! Constant-Time Flag Oblivious Evaluation (CT-FOE) Memo Scanner (DEC-049)
//!
//! Provides:
//! - Software constant-time branchless trial decryption for 96-byte committing note memos
//! - Zero secret-dependent branching, memory lookups, or early-aborts
//! - Side-channel defense against Flag DPA and mid-stream leakage (IACR Communications in Cryptology, 2026)
//! - Constant-time canonical BLS12-381 Fr scalar boundary evaluation against FrConfig::MODULUS
//! - Explicit memory sanitization of all secret buffers (epk, nonce, shared secret, plaintext)

use ark_bls12_381::{Fr, FrConfig};
use ark_ff::{MontConfig, PrimeField};
use chacha20::cipher::{KeyIvInit, StreamCipher};
use chacha20::ChaCha20;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use subtle::{Choice, ConditionallySelectable, ConstantTimeEq};
use zeroize::Zeroize;

use crate::keys::IncomingViewingKey;
use crate::memo::{
    NotePlaintext, ENCRYPTED_MEMO_SIZE, INFO_COMMITTING_MEMO_KEYS_V2, INFO_MEMO_AD_BINDING_V1,
    INFO_VIEW_TAG, SALT_COMMITTING_MEMO_V2,
};
use crate::note::PrivateNoteV1;

type HmacSha256 = Hmac<Sha256>;

/// Note plaintext representation supporting constant-time branchless selection without dynamic allocation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CtNotePlaintext {
    pub value: u64,
    pub asset_id: u64,
    pub rho_limbs: [u64; 4],
}

impl Zeroize for CtNotePlaintext {
    fn zeroize(&mut self) {
        self.value.zeroize();
        self.asset_id.zeroize();
        self.rho_limbs.zeroize();
    }
}

impl CtNotePlaintext {
    /// Produces a safe zeroed dummy note plaintext.
    pub const fn dummy() -> Self {
        Self {
            value: 0,
            asset_id: 0,
            rho_limbs: [0u64; 4],
        }
    }

    /// Reconstructs into canonical Arkworks NotePlaintext.
    /// Returns None if asset_id > 1 or rho is non-canonical (>= r).
    pub fn to_canonical_note(&self) -> Option<NotePlaintext> {
        if self.asset_id > 1 {
            return None;
        }
        let bigint = ark_ff::BigInt(self.rho_limbs);
        let rho = <Fr as PrimeField>::from_bigint(bigint)?;
        Some(NotePlaintext {
            value: self.value,
            rho,
            asset_id: self.asset_id,
        })
    }

    /// Reconstructs full PrivateNoteV1 with deterministic hiding randomness.
    pub fn to_note(&self, owner_key: Fr) -> Option<PrivateNoteV1> {
        let canonical = self.to_canonical_note()?;
        Some(canonical.to_note(owner_key))
    }
}

impl From<NotePlaintext> for CtNotePlaintext {
    fn from(note: NotePlaintext) -> Self {
        let rho_bigint = note.rho.into_bigint();
        Self {
            value: note.value,
            asset_id: note.asset_id,
            rho_limbs: rho_bigint.0,
        }
    }
}

impl From<&NotePlaintext> for CtNotePlaintext {
    fn from(note: &NotePlaintext) -> Self {
        Self::from(*note)
    }
}

impl ConditionallySelectable for CtNotePlaintext {
    fn conditional_select(a: &Self, b: &Self, choice: Choice) -> Self {
        Self {
            value: u64::conditional_select(&a.value, &b.value, choice),
            asset_id: u64::conditional_select(&a.asset_id, &b.asset_id, choice),
            rho_limbs: [
                u64::conditional_select(&a.rho_limbs[0], &b.rho_limbs[0], choice),
                u64::conditional_select(&a.rho_limbs[1], &b.rho_limbs[1], choice),
                u64::conditional_select(&a.rho_limbs[2], &b.rho_limbs[2], choice),
                u64::conditional_select(&a.rho_limbs[3], &b.rho_limbs[3], choice),
            ],
        }
    }
}

/// Constant-Time Memo Scanner (DEC-049).
pub struct ConstantTimeMemoScanner;

impl ConstantTimeMemoScanner {
    /// Decrypts and verifies a 96-byte committing memo with guaranteed branchless software constant-time execution (DEC-049).
    /// No secret-dependent jumps or branches are executed during decryption, tag verification, or scalar checks.
    #[inline(never)]
    pub fn trial_decrypt_ct(
        memo_bytes: &[u8; ENCRYPTED_MEMO_SIZE],
        ivk_bytes: &[u8; 32],
    ) -> (Choice, CtNotePlaintext) {
        // 1. Wire format extraction (Strictly 96 Bytes - DEC-048)
        let mut epk_bytes = [0u8; 32];
        epk_bytes.copy_from_slice(&memo_bytes[0..32]);
        let memo_view_tag = memo_bytes[32];
        let mut memo_nonce = [0u8; 7];
        memo_nonce.copy_from_slice(&memo_bytes[33..40]);
        let mut memo_ciphertext = [0u8; 40];
        memo_ciphertext.copy_from_slice(&memo_bytes[40..80]);
        let mut memo_tag = [0u8; 16];
        memo_tag.copy_from_slice(&memo_bytes[80..96]);

        // 2. Constant-Time X25519 ECDH via dalek (RFC 7748 Montgomery Ladder)
        let epk = x25519_dalek::PublicKey::from(epk_bytes);
        let static_secret = x25519_dalek::StaticSecret::from(*ivk_bytes);
        let shared_secret = static_secret.diffie_hellman(&epk);

        // Non-contributory point defense (AUD-26): low-order / all-zero points fail-closed
        let is_contributory = Choice::from(shared_secret.was_contributory() as u8);
        let mut shared_secret_bytes = *shared_secret.as_bytes();

        // 3. Derive 1-Byte View Tag via HKDF-Expand (DEC-045C & DEC-048)
        let expected_view_tag = Self::derive_view_tag_ct(&shared_secret_bytes);
        let tag_match = expected_view_tag.ct_eq(&memo_view_tag);

        // 4. Key Separation PRG: G(S) -> (K_enc, K_cmt, N_7) via HKDF-SHA256
        let (hkdf_ok, mut cipher_key, mut commitment_key, mut derived_nonce) =
            Self::derive_key_separation_ct(&shared_secret_bytes);

        let nonce_match = derived_nonce.ct_eq(&memo_nonce);

        // 5. Compute and verify Committing Tag HMAC-SHA256 branchless
        let mut expected_tag = Self::compute_committing_tag_ct(
            &commitment_key,
            &memo_nonce,
            &epk_bytes,
            memo_view_tag,
            &memo_ciphertext,
        );
        let tag_auth_valid = expected_tag.ct_eq(&memo_tag);

        // 6. ChaCha20 Stream Cipher Decryption over 40-byte Payload
        let mut nonce_12 = [0u8; 12];
        nonce_12[0..7].copy_from_slice(&memo_nonce);

        let mut raw_plaintext = memo_ciphertext;
        let mut cipher = ChaCha20::new(&cipher_key.into(), &nonce_12.into());
        cipher.apply_keystream(&mut raw_plaintext);

        // 7. Plaintext Parsing (40B) & Canonical Scalar Boundary Check
        // Bytes [0..8]: Little-endian u64 value
        let value = u64::from_le_bytes(raw_plaintext[0..8].try_into().unwrap());

        // Bytes [8..40]: Rho scalar (32B) with asset_id embedded in bit 255
        let mut rho_bytes = [0u8; 32];
        rho_bytes.copy_from_slice(&raw_plaintext[8..40]);
        let asset_id = ((rho_bytes[31] >> 7) & 1) as u64;
        rho_bytes[31] &= 0x7F; // Clear asset_id bit before Fr scalar validation

        let (rho_canonical, rho_limbs) = Self::is_canonical_fr_ct(&rho_bytes);

        // 8. Accumulate validity flags via bitwise bitmasking
        let is_valid =
            is_contributory & tag_match & hkdf_ok & nonce_match & tag_auth_valid & rho_canonical;

        // 9. Constant-Time Selection: return dummy if invalid, candidate if valid
        let mut candidate = CtNotePlaintext {
            value,
            asset_id,
            rho_limbs,
        };
        let final_note =
            CtNotePlaintext::conditional_select(&CtNotePlaintext::dummy(), &candidate, is_valid);

        // 10. Explicit Memory Sanitization
        epk_bytes.zeroize();
        memo_nonce.zeroize();
        memo_ciphertext.zeroize();
        memo_tag.zeroize();
        shared_secret_bytes.zeroize();
        cipher_key.zeroize();
        commitment_key.zeroize();
        derived_nonce.zeroize();
        expected_tag.zeroize();
        raw_plaintext.zeroize();
        rho_bytes.zeroize();
        nonce_12.zeroize();
        candidate.zeroize();

        (is_valid, final_note)
    }

    /// Convenience wrapper taking an IncomingViewingKey directly.
    #[inline(never)]
    pub fn trial_decrypt_ct_ivk(
        memo_bytes: &[u8; ENCRYPTED_MEMO_SIZE],
        ivk: &IncomingViewingKey,
    ) -> (Choice, CtNotePlaintext) {
        Self::trial_decrypt_ct(memo_bytes, &ivk.secret_bytes())
    }

    /// Derives canonical 1-byte View Tag via HKDF-Expand (DEC-045C).
    #[inline(always)]
    pub fn derive_view_tag_ct(shared_secret: &[u8; 32]) -> u8 {
        let hk = hkdf::Hkdf::<Sha256>::new(None, shared_secret);
        let mut okm = [0u8; 1];
        let _ = hk.expand(INFO_VIEW_TAG, &mut okm);
        let vt = okm[0];
        okm.zeroize();
        vt
    }

    /// Key Separation PRG G(S) -> (K_enc, K_cmt, N_7) using HKDF-SHA256 (DEC-048).
    #[inline(always)]
    pub fn derive_key_separation_ct(
        shared_secret: &[u8; 32],
    ) -> (Choice, [u8; 32], [u8; 32], [u8; 7]) {
        let hk = hkdf::Hkdf::<Sha256>::new(Some(SALT_COMMITTING_MEMO_V2), shared_secret);
        let mut okm = [0u8; 71];
        let res = hk.expand(INFO_COMMITTING_MEMO_KEYS_V2, &mut okm);
        let success = Choice::from(res.is_ok() as u8);

        let mut cipher_key = [0u8; 32];
        let mut commitment_key = [0u8; 32];
        let mut nonce_7 = [0u8; 7];

        cipher_key.copy_from_slice(&okm[0..32]);
        commitment_key.copy_from_slice(&okm[32..64]);
        nonce_7.copy_from_slice(&okm[64..71]);
        okm.zeroize();

        (success, cipher_key, commitment_key, nonce_7)
    }

    /// Computes 16-byte Committing Authentication Tag using HMAC-SHA256 (DEC-048).
    #[inline(always)]
    pub fn compute_committing_tag_ct(
        commitment_key: &[u8; 32],
        nonce: &[u8; 7],
        epk: &[u8; 32],
        view_tag: u8,
        ciphertext: &[u8; 40],
    ) -> [u8; 16] {
        let mut mac =
            <HmacSha256 as Mac>::new_from_slice(commitment_key).expect("HMAC supports 32-byte key");
        mac.update(nonce);
        mac.update(epk);
        mac.update(&[view_tag]);
        mac.update(INFO_MEMO_AD_BINDING_V1);
        mac.update(ciphertext);
        let result = mac.finalize().into_bytes();

        let mut tag_16 = [0u8; 16];
        tag_16.copy_from_slice(&result[0..16]);
        tag_16
    }

    /// Validates canonicality of BLS12-381 Fr scalar element branchless (scalar < MODULUS).
    #[inline(always)]
    pub fn is_canonical_fr_ct(bytes: &[u8; 32]) -> (Choice, [u64; 4]) {
        let mut limbs = [0u64; 4];
        for i in 0..4 {
            limbs[i] = u64::from_le_bytes(bytes[i * 8..(i + 1) * 8].try_into().unwrap());
        }

        let modulus = FrConfig::MODULUS.0;

        let mut is_less = Choice::from(0u8);
        let mut is_greater = Choice::from(0u8);

        for i in (0..4).rev() {
            let a = limbs[i];
            let b = modulus[i];
            let (_, borrow_lt) = a.overflowing_sub(b);
            let (_, borrow_gt) = b.overflowing_sub(a);
            let lt = Choice::from(borrow_lt as u8);
            let gt = Choice::from(borrow_gt as u8);

            let undecided = !(is_less | is_greater);
            is_less |= undecided & lt;
            is_greater |= undecided & gt;
        }

        (is_less, limbs)
    }
}

// Top-level re-exports for ergonomic access
#[inline(never)]
pub fn trial_decrypt_ct(
    memo_bytes: &[u8; ENCRYPTED_MEMO_SIZE],
    ivk_bytes: &[u8; 32],
) -> (Choice, CtNotePlaintext) {
    ConstantTimeMemoScanner::trial_decrypt_ct(memo_bytes, ivk_bytes)
}

#[inline(never)]
pub fn trial_decrypt_ct_ivk(
    memo_bytes: &[u8; ENCRYPTED_MEMO_SIZE],
    ivk: &IncomingViewingKey,
) -> (Choice, CtNotePlaintext) {
    ConstantTimeMemoScanner::trial_decrypt_ct_ivk(memo_bytes, ivk)
}

#[inline(always)]
pub fn derive_view_tag_ct(shared_secret: &[u8; 32]) -> u8 {
    ConstantTimeMemoScanner::derive_view_tag_ct(shared_secret)
}

#[inline(always)]
pub fn derive_key_separation_ct(shared_secret: &[u8; 32]) -> (Choice, [u8; 32], [u8; 32], [u8; 7]) {
    ConstantTimeMemoScanner::derive_key_separation_ct(shared_secret)
}

#[inline(always)]
pub fn compute_committing_tag_ct(
    commitment_key: &[u8; 32],
    nonce: &[u8; 7],
    epk: &[u8; 32],
    view_tag: u8,
    ciphertext: &[u8; 40],
) -> [u8; 16] {
    ConstantTimeMemoScanner::compute_committing_tag_ct(
        commitment_key,
        nonce,
        epk,
        view_tag,
        ciphertext,
    )
}

#[inline(always)]
pub fn is_canonical_fr_ct(bytes: &[u8; 32]) -> (Choice, [u64; 4]) {
    ConstantTimeMemoScanner::is_canonical_fr_ct(bytes)
}
