//! Encrypted Note Memo Protocol — DEC-037 & DEC-039 & DEC-043A
//!
//! Provides AEAD ChaCha20Poly1305 memo encryption, trial decryption,
//! and deterministic ECDH note hiding randomness matching on-chain commitments.

use ark_bls12_381::Fr;
use ark_ff::PrimeField;
use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use rand::RngCore;

use crate::keys::{IncomingViewingKey, KeyFormatError, ZeltraShieldedAddress};
use crate::note::{derive_note_randomness, PrivateNoteV1};

use zeroize::Zeroize;

/// Info tag for AEAD note memo cipher derivation (DEC-037).
pub const INFO_MEMO_CIPHER_V1: &[u8] = b"ZELTRA_MEMO_CIPHER_V1";
/// Info domain separator for 1-byte View Tag derivation (DEC-045C / AUD-19).
pub const INFO_VIEW_TAG: &[u8] = b"zeltra_view_tag";

/// Total size of encrypted note memo in bytes (INV-45C-1):
/// 32 (epk) + 1 (view_tag) + 11 (nonce) + 36 (ciphertext) + 16 (tag) = 96 bytes.
pub const ENCRYPTED_MEMO_SIZE: usize = 96;
const _: () = assert!(32 + 1 + 11 + 36 + 16 == ENCRYPTED_MEMO_SIZE);

/// Derives a 1-byte pseudo-random view tag from an ECDH shared secret (DEC-045C / AUD-19).
///
/// Uses standard RFC-5869 HKDF-Expand with domain separator `b"zeltra_view_tag"`.
/// Output is uniformly distributed across all 256 byte bins (0x00..=0xFF),
/// achieving 99.6% non-owned memo rejection in fast-path scanning.
pub fn derive_view_tag(shared_secret: &[u8; 32]) -> u8 {
    let hk = hkdf::Hkdf::<sha2::Sha256>::new(None, shared_secret);
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

/// Plaintext note payload contained in the encrypted memo (DEC-037 / DEC-045C).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NotePlaintext {
    pub value: u64,
    pub rho: Fr,
    pub asset_id: u64,
}

impl NotePlaintext {
    pub const SERIALIZED_SIZE_V1: usize = 48;
    pub const SERIALIZED_SIZE_V2: usize = 36;
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

/// Derives note randomness deterministically from a 32-byte ECDH shared secret and rho (DEC-043A).
pub fn derive_ecdh_note_randomness(shared_secret: &[u8; 32], rho: &Fr) -> Fr {
    let secret_fr = Fr::from_be_bytes_mod_order(shared_secret);
    derive_note_randomness(secret_fr, *rho)
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
    result.push(view_tag);                                 // 1 byte (offset 32)
    result.extend_from_slice(&nonce_11);                   // 11 bytes (33..44)
    result.extend_from_slice(&ciphertext_and_tag[0..36]);  // 36 bytes (44..80)
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

    let hk =
        hkdf::Hkdf::<sha2::Sha256>::new(Some(b"ZELTRA_NOTE_MEMO_KDF_V1"), shared_secret);
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

/// Decrypts a 96-byte AEAD note memo and simultaneously reconstructs the deterministic randomness (DEC-043A / DEC-045C).
/// Supports fast-path 1-byte view tag verification with transparent fallback to legacy DEC-037 memos.
pub fn decrypt_note_memo_deterministic(
    encrypted_memo: &[u8],
    ivk: &IncomingViewingKey,
) -> Option<(NotePlaintext, Fr)> {
    if encrypted_memo.len() != ENCRYPTED_MEMO_SIZE {
        return None;
    }

    let mut epk_bytes = [0u8; 32];
    epk_bytes.copy_from_slice(&encrypted_memo[0..32]);
    let epk = x25519_dalek::PublicKey::from(epk_bytes);

    let static_secret = x25519_dalek::StaticSecret::from(ivk.secret_bytes());
    let shared_secret = static_secret.diffie_hellman(&epk);
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

    let hk = hkdf::Hkdf::<sha2::Sha256>::new(
        Some(b"ZELTRA_NOTE_MEMO_KDF_V1"),
        shared_secret.as_bytes(),
    );
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
