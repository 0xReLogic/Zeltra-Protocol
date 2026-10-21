//! Encrypted Note Memo Protocol — DEC-037 & DEC-039 & DEC-043A & DEC-048
//!
//! Provides:
//! - AEAD ChaCha20Poly1305 memo encryption and trial decryption (DEC-037 & DEC-045C legacy)
//! - EtH-KS Committing AEAD memo encryption, trial decryption, and partitioning oracle defense (DEC-048)
//! - 40-byte full u64 payload with Field-Bit Embedding of asset_id into Fr scalar rho (AUD-25)
//! - X25519 non-contributory point defense against low-order / all-zero public keys (AUD-26)
//! - Deterministic ECDH note hiding randomness matching on-chain Poseidon commitments

use ark_bls12_381::Fr;
use ark_ff::PrimeField;
use ark_serialize::{CanonicalDeserialize, CanonicalSerialize};
use chacha20::cipher::{KeyIvInit, StreamCipher};
use chacha20::ChaCha20;
use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use hmac::{Hmac, Mac};
use rand::RngCore;
use sha2::Sha256;
use subtle::ConstantTimeEq;
use zeroize::Zeroize;

use crate::keys::{IncomingViewingKey, KeyFormatError, ZeltraShieldedAddress};
use crate::note::{derive_note_randomness, PrivateNoteV1};

/// Info tag for AEAD note memo cipher derivation (DEC-037).
pub const INFO_MEMO_CIPHER_V1: &[u8] = b"ZELTRA_MEMO_CIPHER_V1";
/// Info domain separator for 1-byte View Tag derivation (DEC-045C / AUD-19).
pub const INFO_VIEW_TAG: &[u8] = b"zeltra_view_tag";

/// Key Separation Salt for Committing Memo PRG (DEC-048).
pub const SALT_COMMITTING_MEMO_V2: &[u8] = b"ZELTRA_COMMITTING_MEMO_SALT_V2";

/// Key Separation HKDF Info domain separator (DEC-048).
pub const INFO_COMMITTING_MEMO_KEYS_V2: &[u8] = b"ZELTRA_COMMITTING_MEMO_KEYS_V2";

/// Associated Data domain separator for Context-Committing (CMT-4) binding (DEC-048).
pub const INFO_MEMO_AD_BINDING_V1: &[u8] = b"ZELTRA_MEMO_AD_BINDING_V1";

/// Total size of encrypted note memo in bytes (INV-45C-1 & INV-DEC048-1):
/// Legacy DEC-045C: 32 (epk) + 1 (view_tag) + 11 (nonce) + 36 (ciphertext) + 16 (tag) = 96 bytes.
/// Committing DEC-048: 32 (epk) + 1 (view_tag) + 7 (nonce) + 40 (ciphertext) + 16 (tag) = 96 bytes.
pub const ENCRYPTED_MEMO_SIZE: usize = 96;
const _: () = assert!(32 + 1 + 11 + 36 + 16 == ENCRYPTED_MEMO_SIZE);
const _: () = assert!(32 + 1 + 7 + 40 + 16 == ENCRYPTED_MEMO_SIZE);

/// Error states encountered during committing memo encryption and verification (DEC-048).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommittingMemoError {
    InvalidLength,
    InvalidPublicKey,
    ViewTagMismatch,
    NonceMismatch,
    TagVerificationFailed,
    NonCanonicalScalar,
    InvalidAssetId,
    DecryptionFailed,
}

impl core::fmt::Display for CommittingMemoError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidLength => write!(f, "Invalid encrypted memo length"),
            Self::InvalidPublicKey => write!(f, "Invalid or non-contributory public key"),
            Self::ViewTagMismatch => write!(f, "View tag mismatch"),
            Self::NonceMismatch => write!(f, "Nonce mismatch"),
            Self::TagVerificationFailed => {
                write!(f, "Committing authentication tag verification failed")
            }
            Self::NonCanonicalScalar => write!(f, "Non-canonical scalar field element"),
            Self::InvalidAssetId => write!(f, "Invalid asset id"),
            Self::DecryptionFailed => write!(f, "Decryption or key derivation failed"),
        }
    }
}

impl std::error::Error for CommittingMemoError {}

/// Material produced by Key Separation PRG G(S) -> (K_enc, K_cmt, N_7) (DEC-048).
pub struct KeySeparationResult {
    pub cipher_key: [u8; 32],
    pub commitment_key: [u8; 32],
    pub nonce_7: [u8; 7],
}

impl Zeroize for KeySeparationResult {
    fn zeroize(&mut self) {
        self.cipher_key.zeroize();
        self.commitment_key.zeroize();
        self.nonce_7.zeroize();
    }
}

impl Drop for KeySeparationResult {
    fn drop(&mut self) {
        self.zeroize();
    }
}

/// Evaluates the RCR-PRG G(S) -> (K_enc, K_cmt, N_7) using HKDF-SHA256 (DEC-048).
pub fn derive_key_separation(
    shared_secret: &[u8; 32],
) -> Result<KeySeparationResult, CommittingMemoError> {
    let hk = hkdf::Hkdf::<Sha256>::new(Some(SALT_COMMITTING_MEMO_V2), shared_secret);
    let mut okm = [0u8; 71];
    hk.expand(INFO_COMMITTING_MEMO_KEYS_V2, &mut okm)
        .map_err(|_| CommittingMemoError::DecryptionFailed)?;

    let mut cipher_key = [0u8; 32];
    let mut commitment_key = [0u8; 32];
    let mut nonce_7 = [0u8; 7];

    cipher_key.copy_from_slice(&okm[0..32]);
    commitment_key.copy_from_slice(&okm[32..64]);
    nonce_7.copy_from_slice(&okm[64..71]);

    okm.zeroize();

    Ok(KeySeparationResult {
        cipher_key,
        commitment_key,
        nonce_7,
    })
}

/// Extension trait adding `was_non_contributory` to `x25519_dalek::SharedSecret` (AUD-26 / RFC 7748).
pub trait SharedSecretExt {
    /// Returns true if the Diffie-Hellman output was non-contributory (i.e. small order or all-zero point).
    fn was_non_contributory(&self) -> bool;
}

impl SharedSecretExt for x25519_dalek::SharedSecret {
    #[inline]
    fn was_non_contributory(&self) -> bool {
        !self.was_contributory()
    }
}

type HmacSha256 = Hmac<Sha256>;

/// Computes the 16-byte committing authentication tag T_cmt using HMAC-SHA256 (DEC-048).
pub fn compute_committing_tag(
    commitment_key: &[u8; 32],
    nonce_7: &[u8; 7],
    epk: &[u8; 32],
    view_tag: u8,
    ciphertext: &[u8; 40],
) -> [u8; 16] {
    let mut mac =
        <HmacSha256 as Mac>::new_from_slice(commitment_key).expect("HMAC can take key of any size");

    // 1. Nonce stream (7 bytes)
    mac.update(nonce_7);
    // 2. Associated Data Context (epk || view_tag || DOMAIN_TAG) (32 + 1 + 26 bytes)
    mac.update(epk);
    mac.update(&[view_tag]);
    mac.update(INFO_MEMO_AD_BINDING_V1);
    // 3. Ciphertext payload (40 bytes)
    mac.update(ciphertext);

    let result = mac.finalize().into_bytes();
    let mut tag_16 = [0u8; 16];
    tag_16.copy_from_slice(&result[0..16]);
    tag_16
}

/// Constant-time verification of committing tag preventing timing oracles (DEC-048).
#[inline]
pub fn verify_committing_tag_ct(expected: &[u8; 16], received: &[u8; 16]) -> bool {
    expected.ct_eq(received).into()
}

/// Derives a 1-byte pseudo-random view tag from an ECDH shared secret (DEC-045C / AUD-19).
///
/// Uses standard RFC-5869 HKDF-Expand with domain separator `b"zeltra_view_tag"`.
/// Output is uniformly distributed across all 256 byte bins (0x00..=0xFF),
/// achieving 99.6% non-owned memo rejection in fast-path scanning.
pub fn derive_view_tag(shared_secret: &[u8; 32]) -> u8 {
    let hk = hkdf::Hkdf::<Sha256>::new(None, shared_secret);
    let mut okm = [0u8; 1];
    hk.expand(INFO_VIEW_TAG, &mut okm)
        .expect("1 byte is valid HKDF-Expand length");
    okm[0]
}

/// Helper trait allowing encrypt_note_memo to accept either a raw public key, an IVK, or a ZSA.
pub trait RecipientViewingKey {
    fn recipient_pubkey(&self) -> [u8; 32];
}

impl RecipientViewingKey for [u8; 32] {
    fn recipient_pubkey(&self) -> [u8; 32] {
        *self
    }
}

impl RecipientViewingKey for &[u8; 32] {
    fn recipient_pubkey(&self) -> [u8; 32] {
        **self
    }
}

impl RecipientViewingKey for IncomingViewingKey {
    fn recipient_pubkey(&self) -> [u8; 32] {
        self.public_key()
    }
}

impl RecipientViewingKey for &IncomingViewingKey {
    fn recipient_pubkey(&self) -> [u8; 32] {
        self.public_key()
    }
}

impl RecipientViewingKey for ZeltraShieldedAddress {
    fn recipient_pubkey(&self) -> [u8; 32] {
        self.incoming_viewing_public_key
    }
}

impl RecipientViewingKey for &ZeltraShieldedAddress {
    fn recipient_pubkey(&self) -> [u8; 32] {
        self.incoming_viewing_public_key
    }
}

/// Plaintext note payload contained in the encrypted memo (DEC-037 / DEC-045C / DEC-048).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NotePlaintext {
    pub value: u64,
    pub rho: Fr,
    pub asset_id: u64,
}

impl NotePlaintext {
    pub const SERIALIZED_SIZE_V1: usize = 48;
    pub const SERIALIZED_SIZE_V2: usize = 36;
    pub const SERIALIZED_SIZE_V3: usize = 40;
    pub const SERIALIZED_SIZE: usize = 48;

    /// Serializes plaintext note to 48 bytes (8B value || 32B rho scalar || 8B asset_id) (DEC-037 legacy).
    pub fn to_bytes(&self) -> [u8; Self::SERIALIZED_SIZE] {
        let mut bytes = [0u8; Self::SERIALIZED_SIZE];
        bytes[0..8].copy_from_slice(&self.value.to_be_bytes());
        let rho_bytes = crate::to_evm_scalar(&self.rho);
        bytes[8..40].copy_from_slice(&rho_bytes);
        bytes[40..48].copy_from_slice(&self.asset_id.to_be_bytes());
        bytes
    }

    /// Deserializes 48 bytes into a NotePlaintext (DEC-037 legacy).
    /// Returns None if the rho scalar is non-canonical (>= r) or asset_id > 1.
    pub fn from_bytes(bytes: &[u8; Self::SERIALIZED_SIZE]) -> Option<Self> {
        let value = u64::from_be_bytes(bytes[0..8].try_into().unwrap());
        let mut rho_bytes = [0u8; 32];
        rho_bytes.copy_from_slice(&bytes[8..40]);
        let rho = crate::from_evm_scalar(&rho_bytes)?;
        let asset_id = u64::from_be_bytes(bytes[40..48].try_into().unwrap());
        if asset_id > 1 {
            return None;
        }
        Some(Self {
            value,
            rho,
            asset_id,
        })
    }

    /// Serializes plaintext note to 36 bytes for DEC-045C (4B header [1b asset_id || 31b value] || 32B rho scalar).
    pub fn to_bytes_v2(&self) -> [u8; Self::SERIALIZED_SIZE_V2] {
        let mut bytes = [0u8; Self::SERIALIZED_SIZE_V2];
        let asset_bit = (self.asset_id as u32 & 1) << 31;
        let val_bits = (self.value as u32) & 0x7fff_ffff;
        let header = asset_bit | val_bits;
        bytes[0..4].copy_from_slice(&header.to_be_bytes());
        let rho_bytes = crate::to_evm_scalar(&self.rho);
        bytes[4..36].copy_from_slice(&rho_bytes);
        bytes
    }

    /// Deserializes 36 bytes into a NotePlaintext (DEC-045C).
    /// Returns None if the rho scalar is non-canonical (>= r).
    pub fn from_bytes_v2(bytes: &[u8; Self::SERIALIZED_SIZE_V2]) -> Option<Self> {
        let header = u32::from_be_bytes(bytes[0..4].try_into().unwrap());
        let asset_id = ((header >> 31) & 1) as u64;
        let value = (header & 0x7fff_ffff) as u64;
        let mut rho_bytes = [0u8; 32];
        rho_bytes.copy_from_slice(&bytes[4..36]);
        let rho = crate::from_evm_scalar(&rho_bytes)?;
        Some(Self {
            value,
            rho,
            asset_id,
        })
    }

    /// Serializes plaintext note to 40 bytes using Field-Bit Embedding (DEC-048 / AUD-25).
    /// - value: full 8 bytes (u64, little-endian)
    /// - rho: 32 bytes (BLS12-381 scalar Fr in canonical little-endian format)
    /// - asset_id: 1 bit embedded into bit 255 (highest bit of byte 31) of rho.
    pub fn to_bytes_v3(&self) -> [u8; Self::SERIALIZED_SIZE_V3] {
        let mut bytes = [0u8; Self::SERIALIZED_SIZE_V3];
        // 1. value: full 8 bytes (u64, little-endian)
        bytes[0..8].copy_from_slice(&self.value.to_le_bytes());

        // 2. rho: 32 bytes (BLS12-381 scalar Fr in canonical little-endian format)
        let mut rho_bytes = [0u8; 32];
        self.rho
            .serialize_uncompressed(&mut rho_bytes[..])
            .expect("Fr serialization to 32 bytes uncompressed should never fail");

        // 3. asset_id: 1 bit embedded into bit 255 (highest bit of byte 31) of rho.
        // BLS12-381 scalar field modulus r < 2^255, so bit 255 of canonical rho is always 0.
        let asset_bit = ((self.asset_id & 1) as u8) << 7;
        rho_bytes[31] |= asset_bit;

        bytes[8..40].copy_from_slice(&rho_bytes);
        bytes
    }

    /// Deserializes 40 bytes into a NotePlaintext using Field-Bit Embedding (DEC-048 / AUD-25).
    /// Extracts value (u64, LE), extracts asset_id from bit 255 of rho, clears bit 255,
    /// and strictly validates that rho is a canonical scalar (< r).
    pub fn from_bytes_v3(bytes: &[u8; Self::SERIALIZED_SIZE_V3]) -> Option<Self> {
        let value = u64::from_le_bytes(bytes[0..8].try_into().unwrap());
        let mut rho_bytes = [0u8; 32];
        rho_bytes.copy_from_slice(&bytes[8..40]);

        let asset_id = ((rho_bytes[31] >> 7) & 1) as u64;
        rho_bytes[31] &= 0x7f;

        let rho = Fr::deserialize_uncompressed(&rho_bytes[..]).ok()?;
        Some(Self {
            value,
            rho,
            asset_id,
        })
    }

    /// Reconstructs the complete PrivateNoteV1 with deterministic randomness.
    pub fn to_note(&self, owner_key: Fr) -> PrivateNoteV1 {
        let randomness = derive_note_randomness(owner_key, self.rho);
        PrivateNoteV1::new(
            self.value,
            self.asset_id as u8,
            owner_key,
            self.rho,
            randomness,
        )
    }
}

/// Derives note randomness deterministically from a 32-byte ECDH shared secret and rho (DEC-043A).
pub fn derive_ecdh_note_randomness(shared_secret: &[u8; 32], rho: &Fr) -> Fr {
    let secret_fr = Fr::from_be_bytes_mod_order(shared_secret);
    derive_note_randomness(secret_fr, *rho)
}

/// Encrypts note plaintext into a 96-byte committing AEAD memo using EtH-KS (DEC-048).
/// Enforces canonical 96-byte calldata invariant and AUD-26 non-contributory point check.
pub fn encrypt_committing_note_memo<R: RecipientViewingKey>(
    value: u64,
    rho: Fr,
    asset_id: u64,
    recipient: &R,
) -> Result<(Vec<u8>, Fr), CommittingMemoError> {
    if asset_id > 1 {
        return Err(CommittingMemoError::InvalidAssetId);
    }

    let recipient_pk_bytes = recipient.recipient_pubkey();
    let plaintext = NotePlaintext {
        value,
        rho,
        asset_id,
    };
    let pt_bytes = plaintext.to_bytes_v3();

    // 1. Ephemeral Diffie-Hellman Key Agreement
    let mut eph_bytes = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut eph_bytes);
    let ephemeral_secret = x25519_dalek::StaticSecret::from(eph_bytes);
    let ephemeral_public = x25519_dalek::PublicKey::from(&ephemeral_secret);
    let recipient_public = x25519_dalek::PublicKey::from(recipient_pk_bytes);

    let shared_secret = ephemeral_secret.diffie_hellman(&recipient_public);
    if shared_secret.was_non_contributory() {
        eph_bytes.zeroize();
        return Err(CommittingMemoError::InvalidPublicKey);
    }
    let shared_secret_bytes = *shared_secret.as_bytes();

    let randomness = derive_ecdh_note_randomness(&shared_secret_bytes, &rho);
    let view_tag = derive_view_tag(&shared_secret_bytes);

    // 2. Key Separation: G(S) -> (K_enc, K_cmt, N_7)
    let keys = derive_key_separation(&shared_secret_bytes)?;

    // 3. ChaCha20 Stream Encryption
    let mut nonce_12 = [0u8; 12];
    nonce_12[0..7].copy_from_slice(&keys.nonce_7);
    // nonce_12[7..12] is zero-padded

    let mut cipher = ChaCha20::new(&keys.cipher_key.into(), &nonce_12.into());
    let mut ciphertext = [0u8; 40];
    ciphertext.copy_from_slice(&pt_bytes);
    cipher.apply_keystream(&mut ciphertext);

    // 4. Committing Tag Evaluation via HMAC-SHA256 (EtH-KS)
    let tag = compute_committing_tag(
        &keys.commitment_key,
        &keys.nonce_7,
        ephemeral_public.as_bytes(),
        view_tag,
        &ciphertext,
    );

    // 5. Pack into Canonical 96-Byte Calldata Wire:
    // 32B (epk) + 1B (view_tag) + 7B (nonce) + 40B (ciphertext) + 16B (tag) = 96B
    let mut memo = Vec::with_capacity(ENCRYPTED_MEMO_SIZE);
    memo.extend_from_slice(ephemeral_public.as_bytes()); // 0..32
    memo.push(view_tag); // 32
    memo.extend_from_slice(&keys.nonce_7); // 33..40
    memo.extend_from_slice(&ciphertext); // 40..80
    memo.extend_from_slice(&tag); // 80..96

    assert_eq!(memo.len(), ENCRYPTED_MEMO_SIZE);

    eph_bytes.zeroize();
    Ok((memo, randomness))
}

/// Decrypts a 96-byte committing memo using the recipient's Incoming Viewing Key (IVK).
/// Enforces Verify-then-Decrypt invariant: returns None without touching ciphertext if tag fails (DEC-048).
pub fn decrypt_committing_note_memo(
    encrypted_memo: &[u8],
    ivk: &IncomingViewingKey,
) -> Option<(NotePlaintext, Fr)> {
    if encrypted_memo.len() != ENCRYPTED_MEMO_SIZE {
        return None;
    }

    let epk_bytes: [u8; 32] = encrypted_memo[0..32].try_into().ok()?;
    let memo_view_tag = encrypted_memo[32];
    let memo_nonce: [u8; 7] = encrypted_memo[33..40].try_into().ok()?;
    let memo_ciphertext: [u8; 40] = encrypted_memo[40..80].try_into().ok()?;
    let memo_tag: [u8; 16] = encrypted_memo[80..96].try_into().ok()?;

    let epk = x25519_dalek::PublicKey::from(epk_bytes);
    let static_secret = x25519_dalek::StaticSecret::from(ivk.secret_bytes());
    let shared_secret = static_secret.diffie_hellman(&epk);

    // AUD-26: Non-Contributory Point Defense (rejection of low-order / all-zero points)
    if shared_secret.was_non_contributory() {
        return None;
    }

    let shared_secret_bytes = *shared_secret.as_bytes();

    // 1. Fast-Path Pre-Filter: 1-Byte View Tag Check (DEC-045C)
    let expected_view_tag = derive_view_tag(&shared_secret_bytes);
    if memo_view_tag != expected_view_tag {
        return None; // 99.6% non-owned memos discarded in ~50 ns
    }

    // 2. Key Separation Derivation: G(S) -> (K_enc, K_cmt, N_7)
    let keys = derive_key_separation(&shared_secret_bytes).ok()?;

    // 3. Strict Nonce Verification
    if !bool::from(keys.nonce_7.ct_eq(&memo_nonce)) {
        return None;
    }

    // 4. VERIFY-THEN-DECRYPT: Constant-Time Committing Tag Verification
    let expected_tag = compute_committing_tag(
        &keys.commitment_key,
        &memo_nonce,
        &epk_bytes,
        memo_view_tag,
        &memo_ciphertext,
    );

    if !verify_committing_tag_ct(&expected_tag, &memo_tag) {
        // Tag verification failed: FAIL CLOSED immediately.
        // Zero decryption executed; zero plaintext allocated.
        return None;
    }

    // 5. Verified Stream Cipher Decryption
    let mut nonce_12 = [0u8; 12];
    nonce_12[0..7].copy_from_slice(&memo_nonce);
    // nonce_12[7..12] zero-padded

    let mut cipher = ChaCha20::new(&keys.cipher_key.into(), &nonce_12.into());
    let mut pt_buf = memo_ciphertext;
    cipher.apply_keystream(&mut pt_buf);

    // 6. Canonical Field & Asset Bounds Deserialization
    let pt = NotePlaintext::from_bytes_v3(&pt_buf)?;
    let randomness = derive_ecdh_note_randomness(&shared_secret_bytes, &pt.rho);

    Some((pt, randomness))
}

/// Decrypts the 40-byte committing memo payload given pre-calculated ECDH shared secret (DEC-048).
pub fn decrypt_committing_note_memo_payload(
    encrypted_memo: &[u8],
    shared_secret: &[u8; 32],
) -> Option<(NotePlaintext, Fr)> {
    if encrypted_memo.len() != ENCRYPTED_MEMO_SIZE {
        return None;
    }

    let epk_bytes: [u8; 32] = encrypted_memo[0..32].try_into().ok()?;
    let memo_view_tag = encrypted_memo[32];
    let memo_nonce: [u8; 7] = encrypted_memo[33..40].try_into().ok()?;
    let memo_ciphertext: [u8; 40] = encrypted_memo[40..80].try_into().ok()?;
    let memo_tag: [u8; 16] = encrypted_memo[80..96].try_into().ok()?;

    let keys = derive_key_separation(shared_secret).ok()?;

    if !bool::from(keys.nonce_7.ct_eq(&memo_nonce)) {
        return None;
    }

    let expected_tag = compute_committing_tag(
        &keys.commitment_key,
        &memo_nonce,
        &epk_bytes,
        memo_view_tag,
        &memo_ciphertext,
    );

    if !verify_committing_tag_ct(&expected_tag, &memo_tag) {
        return None;
    }

    let mut nonce_12 = [0u8; 12];
    nonce_12[0..7].copy_from_slice(&memo_nonce);

    let mut cipher = ChaCha20::new(&keys.cipher_key.into(), &nonce_12.into());
    let mut pt_buf = memo_ciphertext;
    cipher.apply_keystream(&mut pt_buf);

    let pt = NotePlaintext::from_bytes_v3(&pt_buf)?;
    let randomness = derive_ecdh_note_randomness(shared_secret, &pt.rho);

    Some((pt, randomness))
}

/// Convenience helper to decrypt a committing memo returning only the NotePlaintext.
pub fn decrypt_committing_note_memo_plaintext(
    encrypted_memo: &[u8],
    ivk: &IncomingViewingKey,
) -> Option<NotePlaintext> {
    decrypt_committing_note_memo(encrypted_memo, ivk).map(|(pt, _)| pt)
}

/// Encrypts note plaintext into a 96-byte AEAD memo using Ephemeral ECDH (X25519) and ChaCha20Poly1305 (DEC-045C).
pub fn encrypt_note_memo<R: RecipientViewingKey>(
    value: u64,
    rho: Fr,
    asset_id: u64,
    recipient: &R,
) -> Vec<u8> {
    encrypt_note_memo_deterministic(value, rho, asset_id, recipient)
        .map(|(memo, _)| memo)
        .expect("ChaCha20Poly1305 encryption must not fail")
}

pub fn encrypt_note_memo_optional<R: RecipientViewingKey>(
    value: u64,
    rho: Fr,
    asset_id: u64,
    recipient: &R,
) -> Option<Vec<u8>> {
    encrypt_note_memo_deterministic(value, rho, asset_id, recipient)
        .map(|(memo, _)| memo)
        .ok()
}

/// Decrypts a 96-byte AEAD note memo using the recipient's Incoming Viewing Key (IVK).
/// Returns Some(NotePlaintext) if authentication succeeds and contents are canonical, or None.
pub fn decrypt_note_memo(encrypted_memo: &[u8], ivk: &IncomingViewingKey) -> Option<NotePlaintext> {
    decrypt_note_memo_deterministic(encrypted_memo, ivk).map(|(pt, _)| pt)
}

/// Encrypts note plaintext into a 96-byte AEAD memo using ephemeral X25519 ECDH,
/// with 1-byte view tag, 11-byte nonce, 36-byte ciphertext, and 16-byte Poly1305 tag (DEC-045C).
/// Returns the 96-byte memo and the deterministic hiding randomness Fr matching the recipient's derivation.
pub fn encrypt_note_memo_deterministic<R: RecipientViewingKey>(
    value: u64,
    rho: Fr,
    asset_id: u64,
    recipient: &R,
) -> Result<(Vec<u8>, Fr), KeyFormatError> {
    let recipient_pk_bytes = recipient.recipient_pubkey();
    let plaintext = NotePlaintext {
        value,
        rho,
        asset_id,
    };
    let pt_bytes = plaintext.to_bytes_v2();

    let mut eph_bytes = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut eph_bytes);
    let ephemeral_secret = x25519_dalek::StaticSecret::from(eph_bytes);
    let ephemeral_public = x25519_dalek::PublicKey::from(&ephemeral_secret);
    let recipient_public = x25519_dalek::PublicKey::from(recipient_pk_bytes);

    let shared_secret = ephemeral_secret.diffie_hellman(&recipient_public);
    let shared_secret_bytes = *shared_secret.as_bytes();
    let randomness = derive_ecdh_note_randomness(&shared_secret_bytes, &rho);
    let view_tag = derive_view_tag(&shared_secret_bytes);

    let hk =
        hkdf::Hkdf::<sha2::Sha256>::new(Some(b"ZELTRA_NOTE_MEMO_KDF_V1"), &shared_secret_bytes);
    let mut okm = [0u8; 43];
    hk.expand(INFO_MEMO_CIPHER_V1, &mut okm)
        .map_err(|_| KeyFormatError::InvalidHexEncoding)?;

    let mut key_bytes = [0u8; 32];
    key_bytes.copy_from_slice(&okm[0..32]);
    let cipher_key = Key::from(key_bytes);

    let mut nonce_11 = [0u8; 11];
    nonce_11.copy_from_slice(&okm[32..43]);

    let mut nonce_12 = [0u8; 12];
    nonce_12[0..11].copy_from_slice(&nonce_11);
    nonce_12[11] = 0u8;
    let nonce = Nonce::from(nonce_12);

    let cipher = ChaCha20Poly1305::new(&cipher_key);
    let ciphertext_and_tag = cipher
        .encrypt(&nonce, pt_bytes.as_ref())
        .map_err(|_| KeyFormatError::InvalidHexEncoding)?;

    let mut result = Vec::with_capacity(ENCRYPTED_MEMO_SIZE);
    result.extend_from_slice(ephemeral_public.as_bytes()); // 32 bytes (0..32)
    result.push(view_tag); // 1 byte (offset 32)
    result.extend_from_slice(&nonce_11); // 11 bytes (33..44)
    result.extend_from_slice(&ciphertext_and_tag[0..36]); // 36 bytes (44..80)
    result.extend_from_slice(&ciphertext_and_tag[36..52]); // 16 bytes (80..96)

    assert_eq!(result.len(), ENCRYPTED_MEMO_SIZE);

    eph_bytes.zeroize();
    okm.zeroize();
    key_bytes.zeroize();

    Ok((result, randomness))
}

/// Decrypts the 36-byte payload of a DEC-045C 96-byte memo given the pre-calculated ECDH shared secret.
/// Returns Some((NotePlaintext, deterministic_r)) if ChaCha20Poly1305 authentication succeeds
/// and the payload contents are canonical.
pub fn decrypt_note_memo_payload(
    encrypted_memo: &[u8],
    shared_secret: &[u8; 32],
) -> Option<(NotePlaintext, Fr)> {
    if encrypted_memo.len() != ENCRYPTED_MEMO_SIZE {
        return None;
    }

    let nonce_11 = &encrypted_memo[33..44];
    let ciphertext = &encrypted_memo[44..80];
    let tag = &encrypted_memo[80..96];

    let hk = hkdf::Hkdf::<sha2::Sha256>::new(Some(b"ZELTRA_NOTE_MEMO_KDF_V1"), shared_secret);
    let mut okm = [0u8; 43];
    hk.expand(INFO_MEMO_CIPHER_V1, &mut okm).ok()?;

    let mut key_bytes = [0u8; 32];
    key_bytes.copy_from_slice(&okm[0..32]);
    let cipher_key = Key::from(key_bytes);

    let mut nonce_12 = [0u8; 12];
    nonce_12[0..11].copy_from_slice(nonce_11);
    nonce_12[11] = 0u8;
    let nonce = Nonce::from(nonce_12);

    let mut ct_and_tag = Vec::with_capacity(52);
    ct_and_tag.extend_from_slice(ciphertext);
    ct_and_tag.extend_from_slice(tag);

    let cipher = ChaCha20Poly1305::new(&cipher_key);
    let decrypted = cipher.decrypt(&nonce, ct_and_tag.as_ref()).ok()?;

    key_bytes.zeroize();
    okm.zeroize();

    if decrypted.len() != NotePlaintext::SERIALIZED_SIZE_V2 {
        return None;
    }

    let mut pt_buf = [0u8; NotePlaintext::SERIALIZED_SIZE_V2];
    pt_buf.copy_from_slice(&decrypted);
    let pt = NotePlaintext::from_bytes_v2(&pt_buf)?;

    let randomness = derive_ecdh_note_randomness(shared_secret, &pt.rho);
    Some((pt, randomness))
}

/// Decrypts a 96-byte AEAD note memo and simultaneously reconstructs the deterministic randomness (DEC-043A / DEC-045C / DEC-048).
/// Enforces dual-path scanning: attempts DEC-048 committing AEAD first, then fast-path DEC-045C, and finally legacy DEC-037.
pub fn decrypt_note_memo_deterministic(
    encrypted_memo: &[u8],
    ivk: &IncomingViewingKey,
) -> Option<(NotePlaintext, Fr)> {
    if encrypted_memo.len() != ENCRYPTED_MEMO_SIZE {
        return None;
    }

    // 1. Primary Path: DEC-048 Committing AEAD memo (EtH-KS)
    if let Some(res) = decrypt_committing_note_memo(encrypted_memo, ivk) {
        return Some(res);
    }

    // 2. Legacy Fallback Path: DEC-045C and DEC-037
    let mut epk_bytes = [0u8; 32];
    epk_bytes.copy_from_slice(&encrypted_memo[0..32]);
    let epk = x25519_dalek::PublicKey::from(epk_bytes);

    let static_secret = x25519_dalek::StaticSecret::from(ivk.secret_bytes());
    let shared_secret = static_secret.diffie_hellman(&epk);

    // AUD-26 defense: fail-closed on non-contributory points
    if shared_secret.was_non_contributory() {
        return None;
    }

    let shared_secret_bytes = *shared_secret.as_bytes();

    // Fast-path: check view tag first (DEC-045C)
    let memo_view_tag = encrypted_memo[32];
    let expected_view_tag = derive_view_tag(&shared_secret_bytes);
    if memo_view_tag == expected_view_tag {
        if let Some(res) = decrypt_note_memo_payload(encrypted_memo, &shared_secret_bytes) {
            return Some(res);
        }
    }

    // Fallback: Legacy DEC-037 layout (48-byte ciphertext, no tag)
    let hk =
        hkdf::Hkdf::<sha2::Sha256>::new(Some(b"ZELTRA_NOTE_MEMO_KDF_V1"), &shared_secret_bytes);
    let mut okm = [0u8; 44];
    if hk.expand(INFO_MEMO_CIPHER_V1, &mut okm).is_ok() {
        let mut key_bytes = [0u8; 32];
        key_bytes.copy_from_slice(&okm[0..32]);
        let cipher_key = Key::from(key_bytes);
        let mut nonce_bytes = [0u8; 12];
        nonce_bytes.copy_from_slice(&okm[32..44]);
        let nonce = Nonce::from(nonce_bytes);

        let cipher = ChaCha20Poly1305::new(&cipher_key);
        if let Ok(decrypted) = cipher.decrypt(&nonce, &encrypted_memo[32..96]) {
            if decrypted.len() == NotePlaintext::SERIALIZED_SIZE_V1 {
                let mut pt_buf = [0u8; NotePlaintext::SERIALIZED_SIZE_V1];
                pt_buf.copy_from_slice(&decrypted);
                if let Some(pt) = NotePlaintext::from_bytes(&pt_buf) {
                    let randomness = derive_ecdh_note_randomness(&shared_secret_bytes, &pt.rho);
                    return Some((pt, randomness));
                }
            }
        }
    }

    None
}

/// Helper to construct a legacy DEC-037 96-byte encrypted memo for backward-compatibility testing (DEC-045C C5.3).
pub fn create_legacy_dec037_memo(
    value: u64,
    rho: Fr,
    asset_id: u64,
    recipient_pk_bytes: [u8; 32],
) -> (Vec<u8>, Fr) {
    let plaintext = NotePlaintext {
        value,
        rho,
        asset_id,
    };
    let pt_bytes = plaintext.to_bytes(); // 48 bytes legacy

    let mut eph_bytes = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut eph_bytes);
    let ephemeral_secret = x25519_dalek::StaticSecret::from(eph_bytes);
    let ephemeral_public = x25519_dalek::PublicKey::from(&ephemeral_secret);
    let recipient_public = x25519_dalek::PublicKey::from(recipient_pk_bytes);

    let shared_secret = ephemeral_secret.diffie_hellman(&recipient_public);
    let randomness = derive_ecdh_note_randomness(shared_secret.as_bytes(), &rho);

    let hk =
        hkdf::Hkdf::<sha2::Sha256>::new(Some(b"ZELTRA_NOTE_MEMO_KDF_V1"), shared_secret.as_bytes());
    let mut okm = [0u8; 44];
    hk.expand(INFO_MEMO_CIPHER_V1, &mut okm).unwrap();

    let mut key_bytes = [0u8; 32];
    key_bytes.copy_from_slice(&okm[0..32]);
    let cipher_key = Key::from(key_bytes);
    let mut nonce_bytes = [0u8; 12];
    nonce_bytes.copy_from_slice(&okm[32..44]);
    let nonce = Nonce::from(nonce_bytes);

    let cipher = ChaCha20Poly1305::new(&cipher_key);
    let ciphertext_and_tag = cipher.encrypt(&nonce, pt_bytes.as_ref()).unwrap();

    let mut result = Vec::with_capacity(96);
    result.extend_from_slice(ephemeral_public.as_bytes()); // 32
    result.extend_from_slice(&ciphertext_and_tag); // 48 + 16 = 64
    (result, randomness)
}
