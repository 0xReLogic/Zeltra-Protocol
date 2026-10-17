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

/// Info tag for AEAD note memo cipher derivation (DEC-037).
pub const INFO_MEMO_CIPHER_V1: &[u8] = b"ZELTRA_MEMO_CIPHER_V1";
/// Total size of encrypted note memo in bytes: 32 (epk) + 48 (ciphertext) + 16 (tag) = 96 bytes.
pub const ENCRYPTED_MEMO_SIZE: usize = 96;

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

/// Plaintext note payload contained in the encrypted memo (48 bytes) (DEC-037).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NotePlaintext {
    pub value: u64,
    pub rho: Fr,
    pub asset_id: u64,
}

impl NotePlaintext {
    pub const SERIALIZED_SIZE: usize = 48;

    /// Serializes plaintext note to 48 bytes (8B value || 32B rho scalar || 8B asset_id).
    pub fn to_bytes(&self) -> [u8; Self::SERIALIZED_SIZE] {
        let mut bytes = [0u8; Self::SERIALIZED_SIZE];
        bytes[0..8].copy_from_slice(&self.value.to_be_bytes());
        let rho_bytes = crate::to_evm_scalar(&self.rho);
        bytes[8..40].copy_from_slice(&rho_bytes);
        bytes[40..48].copy_from_slice(&self.asset_id.to_be_bytes());
        bytes
    }

    /// Deserializes 48 bytes into a NotePlaintext.
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

/// Encrypts note plaintext into a 96-byte AEAD memo using Ephemeral ECDH (X25519) and ChaCha20Poly1305.
pub fn encrypt_note_memo<R: RecipientViewingKey>(
    value: u64,
    rho: Fr,
    asset_id: u64,
    recipient: &R,
) -> Vec<u8> {
    let recipient_pk_bytes = recipient.recipient_pubkey();
    let plaintext = NotePlaintext {
        value,
        rho,
        asset_id,
    };
    let pt_bytes = plaintext.to_bytes();

    let mut eph_bytes = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut eph_bytes);
    let ephemeral_secret = x25519_dalek::StaticSecret::from(eph_bytes);
    let ephemeral_public = x25519_dalek::PublicKey::from(&ephemeral_secret);
    let recipient_public = x25519_dalek::PublicKey::from(recipient_pk_bytes);

    let shared_secret = ephemeral_secret.diffie_hellman(&recipient_public);

    let hk =
        hkdf::Hkdf::<sha2::Sha256>::new(Some(b"ZELTRA_NOTE_MEMO_KDF_V1"), shared_secret.as_bytes());
    let mut okm = [0u8; 44];
    hk.expand(INFO_MEMO_CIPHER_V1, &mut okm)
        .expect("44 bytes is valid HKDF-Expand length");
    let mut key_bytes = [0u8; 32];
    key_bytes.copy_from_slice(&okm[0..32]);
    let cipher_key = Key::from(key_bytes);
    let mut nonce_bytes = [0u8; 12];
    nonce_bytes.copy_from_slice(&okm[32..44]);
    let nonce = Nonce::from(nonce_bytes);

    let cipher = ChaCha20Poly1305::new(&cipher_key);
    let ciphertext_and_tag = cipher
        .encrypt(&nonce, pt_bytes.as_ref())
        .expect("ChaCha20Poly1305 encryption on 48 bytes must not fail");

    let mut result = Vec::with_capacity(ENCRYPTED_MEMO_SIZE);
    result.extend_from_slice(ephemeral_public.as_bytes());
    result.extend_from_slice(&ciphertext_and_tag);
    result
}

pub fn encrypt_note_memo_optional<R: RecipientViewingKey>(
    value: u64,
    rho: Fr,
    asset_id: u64,
    recipient: &R,
) -> Option<Vec<u8>> {
    let recipient_pk_bytes = recipient.recipient_pubkey();
    let plaintext = NotePlaintext {
        value,
        rho,
        asset_id,
    };
    let pt_bytes = plaintext.to_bytes();

    let mut eph_bytes = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut eph_bytes);
    let ephemeral_secret = x25519_dalek::StaticSecret::from(eph_bytes);
    let ephemeral_public = x25519_dalek::PublicKey::from(&ephemeral_secret);
    let recipient_public = x25519_dalek::PublicKey::from(recipient_pk_bytes);

    let shared_secret = ephemeral_secret.diffie_hellman(&recipient_public);

    let hk =
        hkdf::Hkdf::<sha2::Sha256>::new(Some(b"ZELTRA_NOTE_MEMO_KDF_V1"), shared_secret.as_bytes());
    let mut okm = [0u8; 44];
    if hk.expand(INFO_MEMO_CIPHER_V1, &mut okm).is_err() {
        return None;
    }
    let mut key_bytes = [0u8; 32];
    key_bytes.copy_from_slice(&okm[0..32]);
    let cipher_key = Key::from(key_bytes);
    let mut nonce_bytes = [0u8; 12];
    nonce_bytes.copy_from_slice(&okm[32..44]);
    let nonce = Nonce::from(nonce_bytes);

    let cipher = ChaCha20Poly1305::new(&cipher_key);
    let ciphertext_and_tag = cipher.encrypt(&nonce, pt_bytes.as_ref()).ok()?;

    let mut result = Vec::with_capacity(ENCRYPTED_MEMO_SIZE);
    result.extend_from_slice(ephemeral_public.as_bytes());
    result.extend_from_slice(&ciphertext_and_tag);
    Some(result)
}

/// Decrypts a 96-byte AEAD note memo using the recipient's Incoming Viewing Key (IVK).
/// Returns Some(NotePlaintext) if authentication succeeds and contents are canonical, or None.
pub fn decrypt_note_memo(encrypted_memo: &[u8], ivk: &IncomingViewingKey) -> Option<NotePlaintext> {
    if encrypted_memo.len() != ENCRYPTED_MEMO_SIZE {
        return None;
    }

    let mut epk_bytes = [0u8; 32];
    epk_bytes.copy_from_slice(&encrypted_memo[0..32]);
    let epk = x25519_dalek::PublicKey::from(epk_bytes);

    let static_secret = x25519_dalek::StaticSecret::from(ivk.secret_bytes());
    let shared_secret = static_secret.diffie_hellman(&epk);

    let hk =
        hkdf::Hkdf::<sha2::Sha256>::new(Some(b"ZELTRA_NOTE_MEMO_KDF_V1"), shared_secret.as_bytes());
    let mut okm = [0u8; 44];
    hk.expand(INFO_MEMO_CIPHER_V1, &mut okm).ok()?;
    let mut key_bytes = [0u8; 32];
    key_bytes.copy_from_slice(&okm[0..32]);
    let cipher_key = Key::from(key_bytes);
    let mut nonce_bytes = [0u8; 12];
    nonce_bytes.copy_from_slice(&okm[32..44]);
    let nonce = Nonce::from(nonce_bytes);

    let cipher = ChaCha20Poly1305::new(&cipher_key);
    let decrypted = cipher.decrypt(&nonce, &encrypted_memo[32..96]).ok()?;

    if decrypted.len() != NotePlaintext::SERIALIZED_SIZE {
        return None;
    }

    let mut pt_buf = [0u8; NotePlaintext::SERIALIZED_SIZE];
    pt_buf.copy_from_slice(&decrypted);
    NotePlaintext::from_bytes(&pt_buf)
}

/// Derives note randomness deterministically from a 32-byte ECDH shared secret and rho (DEC-043A).
pub fn derive_ecdh_note_randomness(shared_secret: &[u8; 32], rho: &Fr) -> Fr {
    let secret_fr = Fr::from_be_bytes_mod_order(shared_secret);
    derive_note_randomness(secret_fr, *rho)
}

/// Encrypts note plaintext into a 96-byte AEAD memo using ephemeral X25519 ECDH,
/// and returns the deterministic hiding randomness Fr matching the recipient's derivation (DEC-043A).
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
    let pt_bytes = plaintext.to_bytes();

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
    hk.expand(INFO_MEMO_CIPHER_V1, &mut okm)
        .map_err(|_| KeyFormatError::InvalidHexEncoding)?;

    let mut key_bytes = [0u8; 32];
    key_bytes.copy_from_slice(&okm[0..32]);
    let cipher_key = Key::from(key_bytes);
    let mut nonce_bytes = [0u8; 12];
    nonce_bytes.copy_from_slice(&okm[32..44]);
    let nonce = Nonce::from(nonce_bytes);

    let cipher = ChaCha20Poly1305::new(&cipher_key);
    let ciphertext_and_tag = cipher
        .encrypt(&nonce, pt_bytes.as_ref())
        .map_err(|_| KeyFormatError::InvalidHexEncoding)?;

    let mut result = Vec::with_capacity(ENCRYPTED_MEMO_SIZE);
    result.extend_from_slice(ephemeral_public.as_bytes());
    result.extend_from_slice(&ciphertext_and_tag);
    Ok((result, randomness))
}

/// Decrypts a 96-byte AEAD note memo and simultaneously reconstructs the deterministic randomness (DEC-043A).
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

    let hk =
        hkdf::Hkdf::<sha2::Sha256>::new(Some(b"ZELTRA_NOTE_MEMO_KDF_V1"), shared_secret.as_bytes());
    let mut okm = [0u8; 44];
    hk.expand(INFO_MEMO_CIPHER_V1, &mut okm).ok()?;

    let mut key_bytes = [0u8; 32];
    key_bytes.copy_from_slice(&okm[0..32]);
    let cipher_key = Key::from(key_bytes);
    let mut nonce_bytes = [0u8; 12];
    nonce_bytes.copy_from_slice(&okm[32..44]);
    let nonce = Nonce::from(nonce_bytes);

    let cipher = ChaCha20Poly1305::new(&cipher_key);
    let decrypted = cipher.decrypt(&nonce, &encrypted_memo[32..96]).ok()?;

    if decrypted.len() != NotePlaintext::SERIALIZED_SIZE {
        return None;
    }

    let mut pt_buf = [0u8; NotePlaintext::SERIALIZED_SIZE];
    pt_buf.copy_from_slice(&decrypted);
    let pt = NotePlaintext::from_bytes(&pt_buf)?;

    let randomness = derive_ecdh_note_randomness(shared_secret.as_bytes(), &pt.rho);
    Some((pt, randomness))
}
