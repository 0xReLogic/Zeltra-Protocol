//! Private Note V1 — DEC-016A implementation
//!
//! Implements the private note ledger primitives:
//! - `PrivateNoteV1` struct
//! - Domain-separated note commitment (Poseidon width-5)
//! - Owner-bound nullifier derivation (Poseidon width-5)
//! - Key derivation (spending key → nullifier key)
//! - Incremental Merkle tree helpers
//!
//! Security invariants:
//! - Note commitment is binding (cannot find two notes with same commitment)
//! - Note commitment is hiding (commitment reveals nothing about note contents)
//! - Nullifier is unlinkable to commitment without the nullifier key
//! - Domain separation prevents cross-usage attacks

use ark_bls12_381::Fr;
use ark_ff::{BigInteger, PrimeField};
use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha3::{Digest, Keccak256};
use zeroize::Zeroize;

use crate::poseidon::native_poseidon_w5;

// ═══════════════════════════════════════════════════════════════════════════
// Domain Separators
// ═══════════════════════════════════════════════════════════════════════════

/// Compute a domain separator as Fr::from(keccak256(label) mod p).
fn domain_from_label(label: &str) -> Fr {
    let hash = Keccak256::digest(label.as_bytes());
    // Reduce the 32-byte hash modulo the field order.
    // This is safe because the hash output is uniformly distributed
    // and the field is ~255 bits, so bias is negligible.
    Fr::from_be_bytes_mod_order(&hash)
}

/// Domain tag for note commitment: H(value, owner_key, rho, randomness).
pub fn domain_note_commitment() -> Fr {
    domain_from_label("zeltra.note.commitment.v1")
}

/// Domain tag for nullifier derivation: H(nullifier_key, commitment, leaf_index).
pub fn domain_nullifier() -> Fr {
    domain_from_label("zeltra.note.nullifier.v1")
}

/// Domain tag for nullifier key derivation (DEC-040A): H(spending_key, DOMAIN_NULLIFIER_KEY).
/// Canonical alias for domain_nullifier() preserving DEC-016A KAV compatibility.
pub fn domain_nullifier_key() -> Fr {
    domain_nullifier()
}

/// Domain tag for Merkle tree internal node hash: H(left, right).
pub fn domain_merkle_node() -> Fr {
    domain_from_label("zeltra.merkle.node.v1")
}

/// Domain tag for initial note (deposit → note) binding.
pub fn domain_initial_note() -> Fr {
    domain_from_label("zeltra.note.initial.v1")
}

/// Domain tag for canonical quote & payment binding (Gate C0).
pub fn domain_quote_binding() -> Fr {
    domain_from_label("zeltra.quote.binding.v1")
}

/// Domain tag for canonical dummy nullifier derivation (DEC-030 Option A Constant-Topology).
pub fn domain_dummy_nullifier() -> Fr {
    domain_from_label("zeltra.note.dummy.nullifier.v1")
}

/// Canonical domain separator for MMR peak bagging (DEC-032).
/// Keccak256("ZELTRA_MMR_BAG_V1") mod r.
pub const DOMAIN_MMR_BAG_BYTES: [u8; 32] = [
    0x19, 0x4a, 0x8f, 0x9c, 0x1e, 0x7d, 0x23, 0x58, 0xb9, 0x01, 0xfc, 0x84, 0x33, 0x29, 0x10, 0x7b,
    0xa8, 0x92, 0x1d, 0xfb, 0xb3, 0x02, 0x48, 0x59, 0xae, 0xf0, 0x29, 0x14, 0x7d, 0xa2, 0x90, 0xbf,
];

/// Domain tag for MMR peak bagging: Poseidon_W5([acc, Fr(leaf_count), 0, 0], DOMAIN_MMR_BAG).
pub fn domain_mmr_bag() -> Fr {
    Fr::from_be_bytes_mod_order(&DOMAIN_MMR_BAG_BYTES)
}

/// Canonical domain separator for contextual scope hash (DEC-035B: "ZELTRA_SCOPE_V1" = 0x5a454c5452415f53434f50455f5631).
pub const DOMAIN_SCOPE_V1_HEX: &str = "0x5a454c5452415f53434f50455f5631";

/// Domain tag for contextual scope hash (DEC-035B Stage 1).
pub fn domain_scope_v1() -> Fr {
    Fr::from_be_bytes_mod_order(b"ZELTRA_SCOPE_V1")
}

/// Canonical domain separator for formal binding commitment (DEC-035B: "ZELTRA_BINDING_V1" = 0x5a454c5452415f42494e44494e475f5631).
pub const DOMAIN_BINDING_V1_HEX: &str = "0x5a454c5452415f42494e44494e475f5631";

/// Domain tag for formal binding commitment (DEC-035B Stage 2).
pub fn domain_binding_v1() -> Fr {
    Fr::from_be_bytes_mod_order(b"ZELTRA_BINDING_V1")
}

/// Compute the native contextual scope hash via Poseidon-W5 (DEC-035B Stage 1).
pub fn compute_scope_hash(recipient: Fr, chain_id: Fr, contract_address: Fr, expiry: Fr) -> Fr {
    native_poseidon_w5(
        &[recipient, chain_id, contract_address, expiry],
        domain_scope_v1(),
    )
}

/// Compute the native formal binding commitment via Poseidon-W5 (DEC-035B Stage 2).
pub fn compute_binding_commitment(quote_hash_hi: Fr, quote_hash_lo: Fr, scope_hash: Fr) -> Fr {
    native_poseidon_w5(
        &[quote_hash_hi, quote_hash_lo, scope_hash, Fr::from(0u64)],
        domain_binding_v1(),
    )
}

/// Domain tag for deterministic note randomness derivation (DEC-037).
pub fn domain_note_randomness() -> Fr {
    domain_from_label("zeltra.note.randomness.v1")
}

/// Deterministically derives the note hiding randomness r from owner spending key and rho (DEC-037).
/// r = Poseidon_W5([owner_key, rho, 0, 0], domain_note_randomness)
pub fn derive_note_randomness(owner_key: Fr, rho: Fr) -> Fr {
    native_poseidon_w5(
        &[owner_key, rho, Fr::from(0u64), Fr::from(0u64)],
        domain_note_randomness(),
    )
}

// ═══════════════════════════════════════════════════════════════════════════
// DEC-037: Viewing Key Derivation & Note Encryption Memo
// ═══════════════════════════════════════════════════════════════════════════

/// Salt for HKDF master secret extraction (DEC-037).
pub const SALT_MASTER_V1: &[u8] = b"ZELTRA_MASTER_V1";
/// Info tag for HKDF spending key expansion (DEC-037).
pub const INFO_SPENDING_KEY_V1: &[u8] = b"ZELTRA_SPENDING_KEY_V1";
/// Info tag for HKDF viewing key expansion (DEC-037).
pub const INFO_VIEWING_KEY_V1: &[u8] = b"ZELTRA_VIEWING_KEY_V1";
/// Info tag for AEAD note memo cipher derivation (DEC-037).
pub const INFO_MEMO_CIPHER_V1: &[u8] = b"ZELTRA_MEMO_CIPHER_V1";
/// Total size of encrypted note memo in bytes: 32 (epk) + 48 (ciphertext) + 16 (tag) = 96 bytes.
pub const ENCRYPTED_MEMO_SIZE: usize = 96;

/// Incoming Viewing Key (IVK) for trial decryption of notes addressed to the owner.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct IncomingViewingKey {
    secret: [u8; 32],
    public: [u8; 32],
}

impl core::fmt::Debug for IncomingViewingKey {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("IncomingViewingKey")
            .field("public", &hex::encode(self.public))
            .finish_non_exhaustive()
    }
}

impl IncomingViewingKey {
    /// Instantiate IVK from a 32-byte secret scalar.
    pub fn from_bytes(secret_bytes: &[u8; 32]) -> Self {
        let static_secret = x25519_dalek::StaticSecret::from(*secret_bytes);
        let public_key = x25519_dalek::PublicKey::from(&static_secret);
        Self {
            secret: *secret_bytes,
            public: *public_key.as_bytes(),
        }
    }

    /// Returns the 32-byte public key (for sending notes to this recipient).
    pub fn public_key(&self) -> [u8; 32] {
        self.public
    }

    /// Returns the private secret bytes.
    pub fn secret_bytes(&self) -> [u8; 32] {
        self.secret
    }

    /// Returns the public owner key scalar (Fr) derived from the public key bytes.
    pub fn owner_key(&self) -> Fr {
        Fr::from_be_bytes_mod_order(&self.public)
    }
}

/// Outcoming Viewing Key (OVK) for viewing change and payment history.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct OutcomingViewingKey {
    key: [u8; 32],
}

impl core::fmt::Debug for OutcomingViewingKey {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("OutcomingViewingKey")
            .field("key", &hex::encode(self.key))
            .finish()
    }
}

impl OutcomingViewingKey {
    pub fn from_bytes(bytes: &[u8; 32]) -> Self {
        Self { key: *bytes }
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.key
    }
}

// ─── Serde helpers for FullViewingKey ───────────────────────────────────────

fn serialize_fr<S: serde::Serializer>(val: &Fr, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(&hex::encode(fr_to_be_bytes(val)))
}

fn deserialize_fr<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Fr, D::Error> {
    let s = String::deserialize(deserializer)?;
    let clean = s.strip_prefix("0x").unwrap_or(&s);
    let bytes = hex::decode(clean).map_err(serde::de::Error::custom)?;
    if bytes.len() != 32 {
        return Err(serde::de::Error::custom("expected 32 bytes for Fr scalar"));
    }
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&bytes);
    fr_from_be_bytes(&arr)
        .ok_or_else(|| serde::de::Error::custom("scalar element >= r (non-canonical)"))
}

fn serialize_bytes32<S: serde::Serializer>(
    val: &[u8; 32],
    serializer: S,
) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(&hex::encode(val))
}

fn deserialize_bytes32<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<[u8; 32], D::Error> {
    let s = String::deserialize(deserializer)?;
    let clean = s.strip_prefix("0x").unwrap_or(&s);
    let bytes = hex::decode(clean).map_err(serde::de::Error::custom)?;
    if bytes.len() != 32 {
        return Err(serde::de::Error::custom("expected 32 bytes"));
    }
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&bytes);
    Ok(arr)
}

/// Full Viewing Key (FVK) for independent auditing and compliance without spend authority (DEC-040, DEC-040A).
///
/// Bundles:
/// - `owner_key`: public owner key scalar (Fr) that appears in note commitments
/// - `incoming_viewing_key`: 32-byte secret for decrypting incoming note memos via ECDH (DEC-037)
/// - `outcoming_viewing_key`: 32-byte secret for decrypting change notes and outbound payment records
/// - `nullifier_deriving_key`: scalar (Fr) for deriving nullifiers to check note spend status on-chain
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct FullViewingKey {
    #[serde(serialize_with = "serialize_fr", deserialize_with = "deserialize_fr")]
    pub owner_key: Fr,
    #[serde(
        serialize_with = "serialize_bytes32",
        deserialize_with = "deserialize_bytes32"
    )]
    pub incoming_viewing_key: [u8; 32],
    #[serde(
        serialize_with = "serialize_bytes32",
        deserialize_with = "deserialize_bytes32"
    )]
    pub outcoming_viewing_key: [u8; 32],
    #[serde(serialize_with = "serialize_fr", deserialize_with = "deserialize_fr")]
    pub nullifier_deriving_key: Fr,
}

impl Zeroize for FullViewingKey {
    fn zeroize(&mut self) {
        self.incoming_viewing_key.zeroize();
        self.outcoming_viewing_key.zeroize();
    }
}

impl Drop for FullViewingKey {
    fn drop(&mut self) {
        self.zeroize();
    }
}

impl FullViewingKey {
    /// Create a new FullViewingKey from components.
    pub fn new(
        owner_key: Fr,
        incoming_viewing_key: [u8; 32],
        outcoming_viewing_key: [u8; 32],
        nullifier_deriving_key: Fr,
    ) -> Self {
        Self {
            owner_key,
            incoming_viewing_key,
            outcoming_viewing_key,
            nullifier_deriving_key,
        }
    }

    /// Derive an evolving nullifier for a note at a given Merkle tree position and epoch (DEC-033).
    pub fn derive_nullifier(&self, commitment: Fr, leaf_index: u64, epoch_id: u32) -> Fr {
        crate::note::derive_nullifier(
            self.nullifier_deriving_key,
            commitment,
            leaf_index,
            epoch_id,
        )
    }

    /// Derive JoinSplit nullifier canonical for Universal JoinSplit note (DEC-036A).
    pub fn derive_joinsplit_nullifier(&self, rho: Fr, epoch_id: u64) -> Fr {
        crate::note::derive_joinsplit_nullifier(self.nullifier_deriving_key, rho, epoch_id)
    }

    /// Canonical 128-byte (256-hex) serialization:
    /// owner_key (32B) || incoming_viewing_key (32B) || outcoming_viewing_key (32B) || nullifier_deriving_key (32B).
    pub fn to_hex(&self) -> String {
        let mut bytes = Vec::with_capacity(128);
        bytes.extend_from_slice(&fr_to_be_bytes(&self.owner_key));
        bytes.extend_from_slice(&self.incoming_viewing_key);
        bytes.extend_from_slice(&self.outcoming_viewing_key);
        bytes.extend_from_slice(&fr_to_be_bytes(&self.nullifier_deriving_key));
        hex::encode(bytes)
    }

    /// Canonical 128-byte (256-hex) deserialization. Rejects non-canonical field elements (>= r).
    pub fn from_hex(hex_str: &str) -> Result<Self, KeyFormatError> {
        let clean = hex_str.strip_prefix("0x").unwrap_or(hex_str);
        if clean.len() != 256 {
            return Err(KeyFormatError::InvalidLength {
                expected: 256,
                actual: clean.len(),
            });
        }
        let bytes = hex::decode(clean).map_err(|_| KeyFormatError::InvalidHexEncoding)?;
        if bytes.len() != 128 {
            return Err(KeyFormatError::InvalidLength {
                expected: 128,
                actual: bytes.len(),
            });
        }
        let mut owner_bytes = [0u8; 32];
        owner_bytes.copy_from_slice(&bytes[0..32]);
        let owner_key = fr_from_be_bytes(&owner_bytes)
            .ok_or(KeyFormatError::NonCanonicalScalar { field: "owner_key" })?;

        let mut incoming_viewing_key = [0u8; 32];
        incoming_viewing_key.copy_from_slice(&bytes[32..64]);

        let mut outcoming_viewing_key = [0u8; 32];
        outcoming_viewing_key.copy_from_slice(&bytes[64..96]);

        let mut nk_bytes = [0u8; 32];
        nk_bytes.copy_from_slice(&bytes[96..128]);
        let nullifier_deriving_key =
            fr_from_be_bytes(&nk_bytes).ok_or(KeyFormatError::NonCanonicalScalar {
                field: "nullifier_deriving_key",
            })?;

        Ok(Self {
            owner_key,
            incoming_viewing_key,
            outcoming_viewing_key,
            nullifier_deriving_key,
        })
    }
}

/// Errors occurring during cryptographic key serialization or parsing (DEC-040A).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyFormatError {
    InvalidHexEncoding,
    InvalidLength { expected: usize, actual: usize },
    NonCanonicalScalar { field: &'static str },
}

impl core::fmt::Display for KeyFormatError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidHexEncoding => write!(f, "Invalid hex encoding for key"),
            Self::InvalidLength { expected, actual } => {
                write!(
                    f,
                    "Invalid key hex length: expected {}, got {}",
                    expected, actual
                )
            }
            Self::NonCanonicalScalar { field } => {
                write!(f, "Scalar field element in {} >= r (non-canonical)", field)
            }
        }
    }
}

impl std::error::Error for KeyFormatError {}

/// Helper trait allowing encrypt_note_memo to accept either a raw public key or an IVK.
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

/// Derives a 32-byte master secret from an arbitrary client seed or signature (DEC-037).
pub fn derive_master_secret(seed: &[u8]) -> [u8; 32] {
    let hk = hkdf::Hkdf::<sha2::Sha256>::new(Some(SALT_MASTER_V1), seed);
    let mut prk = [0u8; 32];
    hk.expand(b"ZELTRA_MASTER_PRK_V1", &mut prk)
        .expect("32 bytes is valid length for HKDF-Expand");
    prk
}

/// Derives the spending key scalar (Fr) from a 32-byte master secret (DEC-037).
pub fn derive_spending_key(master_secret: &[u8; 32]) -> Fr {
    let hk = hkdf::Hkdf::<sha2::Sha256>::new(None, master_secret);
    let mut okm = [0u8; 32];
    hk.expand(INFO_SPENDING_KEY_V1, &mut okm)
        .expect("32 bytes is valid length for HKDF-Expand");
    Fr::from_be_bytes_mod_order(&okm)
}

/// Derives the incoming and outcoming viewing keys from a 32-byte master secret.
pub fn derive_viewing_keys_from_secret(
    master_secret: &[u8; 32],
) -> (IncomingViewingKey, OutcomingViewingKey) {
    let hk = hkdf::Hkdf::<sha2::Sha256>::new(None, master_secret);
    let mut okm = [0u8; 64];
    hk.expand(INFO_VIEWING_KEY_V1, &mut okm)
        .expect("64 bytes is valid length for HKDF-Expand");
    let mut ivk_bytes = [0u8; 32];
    let mut ovk_bytes = [0u8; 32];
    ivk_bytes.copy_from_slice(&okm[0..32]);
    ovk_bytes.copy_from_slice(&okm[32..64]);
    (
        IncomingViewingKey::from_bytes(&ivk_bytes),
        OutcomingViewingKey::from_bytes(&ovk_bytes),
    )
}

/// Derives the viewing keys from an arbitrary seed or 32-byte master secret (DEC-037).
pub fn derive_viewing_keys(master_seed: &[u8]) -> (IncomingViewingKey, OutcomingViewingKey) {
    let master_secret = if master_seed.len() == 32 {
        let mut s = [0u8; 32];
        s.copy_from_slice(master_seed);
        s
    } else {
        derive_master_secret(master_seed)
    };
    derive_viewing_keys_from_secret(&master_secret)
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

    let static_secret = x25519_dalek::StaticSecret::from(ivk.secret);
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

// ═══════════════════════════════════════════════════════════════════════════
// PrivateNoteV1
// ═══════════════════════════════════════════════════════════════════════════

pub const ASSET_ID_WETH: u8 = 0;
pub const ASSET_ID_USDC: u8 = 1;

/// Private Note V1 — the fundamental unit of private value in Zeltra.
///
/// Each note represents a specific amount of stablecoin or native asset owned by a specific
/// user. Notes are consumed (nullified) when spent, producing change notes
/// for any remaining value.
///
/// Fields:
/// - `value`: Amount in base units (u64, 6 decimals for USDC, 9 decimals Gwei for WETH)
/// - `asset_id`: 0 for WETH, 1 for USDC
/// - `owner_key`: Owner's spending key (Fr element)
/// - `rho`: Unique note identifier / nullifier randomness (Fr element)
/// - `randomness`: Commitment trapdoor for hiding property (Fr element)
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrivateNoteV1 {
    pub value: u64,
    pub asset_id: u8,
    pub owner_key: Fr,
    pub rho: Fr,
    pub randomness: Fr,
}

impl PrivateNoteV1 {
    /// Create a new note with the given parameters.
    pub fn new(value: u64, asset_id: u8, owner_key: Fr, rho: Fr, randomness: Fr) -> Self {
        Self {
            value,
            asset_id,
            owner_key,
            rho,
            randomness,
        }
    }

    /// Compute the field bit-packed value Fr representation (DEC-034A).
    /// value_packed = value + asset_id * 2^64
    pub fn value_packed(&self) -> Fr {
        let two_pow_64 = Fr::from(1u128 << 64);
        Fr::from(self.value) + (Fr::from(self.asset_id as u64) * two_pow_64)
    }

    /// Compute the commitment for this note.
    ///
    /// commitment = Poseidon_W5(value_packed, owner_key, rho, randomness; domain)
    ///
    /// The commitment is binding (cannot change any field without changing
    /// the commitment) and hiding (reveals nothing about the note contents
    /// without knowing the preimage).
    pub fn commitment(&self) -> Fr {
        note_commitment(
            self.value,
            self.asset_id,
            self.owner_key,
            self.rho,
            self.randomness,
        )
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Note Commitment
// ═══════════════════════════════════════════════════════════════════════════

/// Compute a note commitment from individual fields including asset_id.
///
/// commitment = Poseidon_W5(value_packed, owner_key, rho, randomness; DOMAIN_NOTE_COMMITMENT_V1)
/// where value_packed = value + asset_id * 2^64
///
/// State layout: [value_packed, owner_key, rho, randomness, domain_tag]
pub fn note_commitment(value: u64, asset_id: u8, owner_key: Fr, rho: Fr, randomness: Fr) -> Fr {
    let two_pow_64 = Fr::from(1u128 << 64);
    let value_packed = Fr::from(value) + (Fr::from(asset_id as u64) * two_pow_64);
    note_commitment_packed(value_packed, owner_key, rho, randomness)
}

/// Compute a note commitment directly from value_packed field element.
pub fn note_commitment_packed(value_packed: Fr, owner_key: Fr, rho: Fr, randomness: Fr) -> Fr {
    let inputs = [value_packed, owner_key, rho, randomness];
    native_poseidon_w5(&inputs, domain_note_commitment())
}

// ═══════════════════════════════════════════════════════════════════════════
// Key Derivation
// ═══════════════════════════════════════════════════════════════════════════

/// Derive the nullifier key from the spending key.
///
/// nullifier_key = Poseidon_W3(spending_key, DOMAIN_NULLIFIER_V1)
///
/// Uses the existing width-3 Poseidon (rate=2) since we only have 2 inputs:
/// the spending key and the domain tag.
pub fn derive_nullifier_key(spending_key: Fr) -> Fr {
    crate::poseidon::compute_nullifier(spending_key, domain_nullifier())
}

// ═══════════════════════════════════════════════════════════════════════════
// Nullifier Derivation
// ═══════════════════════════════════════════════════════════════════════════

/// Derive an evolving nullifier for a note at a given Merkle tree position and epoch (DEC-033).
///
/// nullifier = Poseidon_W5(nullifier_key, commitment, Fr(leaf_index), Fr(epoch_id); DOMAIN_NULLIFIER_V1)
///
/// The nullifier binds to:
/// - The owner's nullifier key (proves ownership)
/// - The note commitment (identifies which note)
/// - The leaf index (prevents commitment reuse at different positions)
/// - The epoch ID (evolves nullifiers across generational windows, preventing cross-epoch replay)
/// - Domain tag (prevents cross-usage)
///
/// Privacy: Without knowledge of the nullifier key, it is infeasible to
/// link a nullifier to its corresponding commitment.
pub fn derive_nullifier(nullifier_key: Fr, commitment: Fr, leaf_index: u64, epoch_id: u32) -> Fr {
    let inputs = [
        nullifier_key,
        commitment,
        Fr::from(leaf_index),
        Fr::from(epoch_id as u64),
    ];
    native_poseidon_w5(&inputs, domain_nullifier())
}

/// Legacy 3-argument nullifier derivation defaulting to epoch_id = 0.
pub fn derive_nullifier_v1(nullifier_key: Fr, commitment: Fr, leaf_index: u64) -> Fr {
    derive_nullifier(nullifier_key, commitment, leaf_index, 0)
}

/// Alias for derive_nullifier with epoch_id.
pub fn derive_nullifier_epoch(
    nullifier_key: Fr,
    commitment: Fr,
    leaf_index: u64,
    epoch_id: u32,
) -> Fr {
    derive_nullifier(nullifier_key, commitment, leaf_index, epoch_id)
}

/// Canonical nullifier derivation for Universal JoinSplit note (DEC-036A / INV-JS-6):
/// nf = Poseidon_W5([nk, rho, epoch_id, 0], DOMAIN_NULLIFIER)
pub fn derive_joinsplit_nullifier(nullifier_key: Fr, rho: Fr, epoch_id: u64) -> Fr {
    let inputs = [nullifier_key, rho, Fr::from(epoch_id), Fr::from(0u64)];
    native_poseidon_w5(&inputs, domain_nullifier())
}

/// Canonical dummy nullifier derivation for JoinSplit dummy slot (DEC-036A / A2.3):
/// nf_dummy = Poseidon_W5([nk, session_nonce, 0, 0], DOMAIN_DUMMY_NULLIFIER)
pub fn derive_joinsplit_dummy_nullifier(nullifier_key: Fr, session_nonce: Fr) -> Fr {
    let inputs = [nullifier_key, session_nonce, Fr::from(0u64), Fr::from(0u64)];
    native_poseidon_w5(&inputs, domain_dummy_nullifier())
}

// ═══════════════════════════════════════════════════════════════════════════
// Incremental Merkle Tree Helpers
// ═══════════════════════════════════════════════════════════════════════════

/// Hash two child nodes to produce a parent node.
///
/// parent = Poseidon_W5(left, right, 0, 0; DOMAIN_MERKLE_NODE_V1)
pub fn merkle_hash(left: Fr, right: Fr) -> Fr {
    let inputs = [left, right, Fr::from(0u64), Fr::from(0u64)];
    native_poseidon_w5(&inputs, domain_merkle_node())
}

// ═══════════════════════════════════════════════════════════════════════════
// Merkle Mountain Range (MMR) Accumulator — DEC-032
// ═══════════════════════════════════════════════════════════════════════════

/// Merkle Mountain Range membership proof (DEC-032).
/// Contains the internal mountain sibling path and the sibling peaks needed
/// for canonical peak bagging.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MMRProof {
    /// Height of the mountain containing the leaf (0 <= height < 32).
    pub mountain_height: usize,
    /// Sibling hashes along the internal mountain tree from leaf to peak.
    pub mountain_siblings: Vec<Fr>,
    /// The other active peak hashes in the MMR folded into the bagged root.
    pub peak_bagging_siblings: Vec<Fr>,
    /// Index of this mountain's peak among the active peaks (ordered left-to-right from highest to lowest mountain).
    pub peak_index: usize,
}

/// In-memory Merkle Mountain Range (DEC-032) note commitment accumulator.
///
/// Implements infinite-horizon append-only accumulation scaling to 2^64 elements
/// with O(log N) state, amortized O(1) Poseidon hashing, and canonical
/// peak bagging domain separation.
#[derive(Clone, Debug, Default)]
pub struct MerkleMountainRange {
    /// Total number of leaves accumulated so far.
    pub leaf_count: usize,
    /// All leaves inserted sequentially.
    pub leaves: Vec<Fr>,
    /// Active peaks by height index (peaks_by_height[h] is Some(peak) if bit h of leaf_count is 1).
    peaks_by_height: Vec<Option<Fr>>,
    /// Cached bagged root (computed on append, O(1) get_root lookup).
    cached_root: Option<Fr>,
}

impl MerkleMountainRange {
    /// Create a new, empty Merkle Mountain Range.
    pub fn new() -> Self {
        Self {
            leaf_count: 0,
            leaves: Vec::new(),
            peaks_by_height: Vec::new(),
            cached_root: None,
        }
    }

    /// Append a leaf (note commitment) to the MMR using binary carry adder algorithm.
    ///
    /// Returns `(leaf_index, new_bagged_root)`.
    pub fn append(&mut self, leaf: Fr) -> (usize, Fr) {
        let leaf_index = self.leaf_count;
        self.leaves.push(leaf);

        let mut current = leaf;
        let mut height = 0;
        let mut idx = leaf_index;

        while (idx & 1) == 1 {
            let left = self.peaks_by_height[height]
                .take()
                .expect("Active peak must exist at height where carry bit is 1");
            current = merkle_hash(left, current);
            height += 1;
            idx >>= 1;
        }

        if height >= self.peaks_by_height.len() {
            self.peaks_by_height.resize(height + 1, None);
        }
        self.peaks_by_height[height] = Some(current);
        self.leaf_count += 1;

        let root = Self::bag_peaks(&self.get_peaks(), self.leaf_count);
        self.cached_root = Some(root);
        (leaf_index, root)
    }

    /// Helper to append a leaf, alias for append.
    pub fn append_leaf(&mut self, leaf: Fr) -> (usize, Fr) {
        self.append(leaf)
    }

    /// Total number of leaves as u64.
    pub fn leaf_count(&self) -> u64 {
        self.leaf_count as u64
    }

    /// Current bagged root, alias for get_root.
    pub fn bagged_root(&self) -> Fr {
        self.get_root()
    }

    /// Returns all active peaks ordered from left to right (highest mountain to lowest mountain).
    pub fn get_peaks(&self) -> Vec<Fr> {
        let mut peaks = Vec::new();
        for &opt in self.peaks_by_height.iter().rev() {
            if let Some(peak) = opt {
                peaks.push(peak);
            }
        }
        peaks
    }

    /// Canonical peak bagging per DEC-032 Section 3.C:
    ///
    /// - If m == 1: Root = Poseidon_W5([P_0, Fr(N), 0, 0], DOMAIN_MMR_BAG)
    /// - If m > 1:
    ///   Acc_{m-1} = P_{m-1}
    ///   Acc_i = Poseidon_W5([P_i, Acc_{i+1}, 0, 0], DOMAIN_MERKLE_NODE) for i = m-2 down to 0
    ///   Root = Poseidon_W5([Acc_0, Fr(N), 0, 0], DOMAIN_MMR_BAG)
    pub fn bag_peaks(peaks: &[Fr], leaf_count: usize) -> Fr {
        if peaks.is_empty() {
            return Fr::from(0u64);
        }
        let mut acc = peaks[peaks.len() - 1];
        for i in (0..peaks.len() - 1).rev() {
            acc = merkle_hash(peaks[i], acc);
        }
        let inputs = [
            acc,
            Fr::from(leaf_count as u64),
            Fr::from(0u64),
            Fr::from(0u64),
        ];
        native_poseidon_w5(&inputs, domain_mmr_bag())
    }

    /// Compute the current bagged MMR root (O(1) when cached).
    pub fn get_root(&self) -> Fr {
        match self.cached_root {
            Some(root) => root,
            None => Self::bag_peaks(&self.get_peaks(), self.leaf_count),
        }
    }

    /// Generate an MMR membership proof for `leaf_index`.
    pub fn generate_proof(&self, leaf_index: usize) -> MMRProof {
        if leaf_index >= self.leaf_count {
            panic!(
                "OUT_OF_BOUNDS_LEAF: leaf_index {} >= leaf_count {}",
                leaf_index, self.leaf_count
            );
        }

        // Decompose leaf_count into peak heights (descending order)
        let mut peak_heights = Vec::new();
        for h in (0..usize::BITS).rev() {
            if (self.leaf_count >> h) & 1 == 1 {
                peak_heights.push(h as usize);
            }
        }

        // Locate mountain covering leaf_index
        let mut current_start = 0;
        let mut target_peak_idx = 0;
        let mut target_height = 0;
        let mut target_start = 0;

        for (p_idx, &h) in peak_heights.iter().enumerate() {
            let mountain_size = 1 << h;
            if leaf_index >= current_start && leaf_index < current_start + mountain_size {
                target_peak_idx = p_idx;
                target_height = h;
                target_start = current_start;
                break;
            }
            current_start += mountain_size;
        }

        // Extract internal mountain tree nodes and siblings
        let mountain_size = 1 << target_height;
        let mut mountain_nodes: Vec<Fr> =
            self.leaves[target_start..target_start + mountain_size].to_vec();
        let mut offset = leaf_index - target_start;
        let mut mountain_siblings = Vec::with_capacity(target_height);

        for _ in 0..target_height {
            let sibling_offset = if offset.is_multiple_of(2) {
                offset + 1
            } else {
                offset - 1
            };
            mountain_siblings.push(mountain_nodes[sibling_offset]);

            let mut next_layer = Vec::with_capacity(mountain_nodes.len() / 2);
            for chunk in mountain_nodes.as_chunks::<2>().0 {
                next_layer.push(merkle_hash(chunk[0], chunk[1]));
            }
            mountain_nodes = next_layer;
            offset /= 2;
        }

        // Sibling peaks for bagging: all active peaks except this one
        let all_peaks = self.get_peaks();
        let mut peak_bagging_siblings = all_peaks;
        peak_bagging_siblings.remove(target_peak_idx);

        MMRProof {
            mountain_height: target_height,
            mountain_siblings,
            peak_bagging_siblings,
            peak_index: target_peak_idx,
        }
    }

    /// Verifies an MMR membership proof against a given root.
    /// Strictly protects against Hyperbridge (2026) out-of-bounds exploits and
    /// tampered leaf_count values.
    pub fn verify_proof(
        leaf: Fr,
        leaf_index: usize,
        leaf_count: usize,
        proof: &MMRProof,
        root: Fr,
    ) -> bool {
        // 1. Strict Upper Bound Check (DEC-032 Section 4.1):
        // Prevents Hyperbridge unconsumed leaf exploit: leaf_index MUST be strictly less than leaf_count
        if leaf_count == 0 || leaf_index >= leaf_count {
            return false;
        }

        // 2. Canonical Mountain Derivation (DEC-032 Section 4.2):
        let mut peak_heights = Vec::new();
        for h in (0..usize::BITS).rev() {
            if (leaf_count >> h) & 1 == 1 {
                peak_heights.push(h as usize);
            }
        }
        let total_peaks = peak_heights.len();

        let mut current_start = 0;
        let mut canonical_peak_idx = None;
        let mut canonical_height = None;

        for (p_idx, &h) in peak_heights.iter().enumerate() {
            let mountain_size = 1 << h;
            if leaf_index >= current_start && leaf_index < current_start + mountain_size {
                canonical_peak_idx = Some(p_idx);
                canonical_height = Some(h);
                break;
            }
            current_start += mountain_size;
        }

        let (Some(target_peak_idx), Some(target_height)) = (canonical_peak_idx, canonical_height)
        else {
            return false;
        };

        // Structural witness consistency checks
        if proof.mountain_height != target_height
            || proof.peak_index != target_peak_idx
            || proof.mountain_siblings.len() != target_height
            || proof.peak_bagging_siblings.len() != total_peaks - 1
        {
            return false;
        }

        // 3. Reconstruct peak from leaf and mountain siblings
        let mut current = leaf;
        let mut idx = leaf_index;
        for sibling in &proof.mountain_siblings {
            current = if idx.is_multiple_of(2) {
                merkle_hash(current, *sibling)
            } else {
                merkle_hash(*sibling, current)
            };
            idx /= 2;
        }
        let computed_peak = current;

        // 4. Reconstruct all active peaks in canonical order
        let mut all_peaks = Vec::with_capacity(total_peaks);
        let mut bag_iter = proof.peak_bagging_siblings.iter();
        for i in 0..total_peaks {
            if i == target_peak_idx {
                all_peaks.push(computed_peak);
            } else {
                all_peaks.push(*bag_iter.next().unwrap());
            }
        }

        // 5. Bag all peaks and check equivalence to expected root
        let computed_root = Self::bag_peaks(&all_peaks, leaf_count);
        computed_root == root
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Value Conservation
// ═══════════════════════════════════════════════════════════════════════════

/// Verify value conservation for a spend transaction.
///
/// sum(input_values) == merchant_amount + protocol_fee + execution_fee + sum(change_values)
///
/// All arithmetic uses checked u64 operations. Overflow returns false.
pub fn verify_value_conservation(
    input_values: &[u64],
    merchant_amount: u64,
    protocol_fee: u64,
    execution_fee: u64,
    change_values: &[u64],
) -> bool {
    let sum_inputs: Option<u64> = input_values
        .iter()
        .try_fold(0u64, |acc, &v| acc.checked_add(v));
    let sum_changes: Option<u64> = change_values
        .iter()
        .try_fold(0u64, |acc, &v| acc.checked_add(v));

    match (sum_inputs, sum_changes) {
        (Some(inputs), Some(changes)) => {
            let total_debits = merchant_amount
                .checked_add(protocol_fee)
                .and_then(|s| s.checked_add(execution_fee))
                .and_then(|s| s.checked_add(changes));
            total_debits == Some(inputs)
        }
        _ => false, // overflow
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Serialization Helpers
// ═══════════════════════════════════════════════════════════════════════════

/// Serialize a field element to 32-byte big-endian representation.
pub fn fr_to_be_bytes(f: &Fr) -> [u8; 32] {
    let bigint = f.into_bigint();
    let mut bytes = [0u8; 32];
    // Arkworks BigInteger256 stores limbs in little-endian order.
    // We need big-endian byte output.
    let limb_bytes = bigint.to_bytes_le();
    // Reverse to get big-endian
    for (i, b) in limb_bytes.iter().rev().enumerate() {
        if i < 32 {
            bytes[i] = *b;
        }
    }
    bytes
}

/// Deserialize a field element from 32-byte big-endian representation.
/// Validates that the scalar element is strictly within the field modulus (< r).
pub fn fr_from_be_bytes(bytes: &[u8; 32]) -> Option<Fr> {
    use ark_serialize::CanonicalDeserialize;
    let mut le_bytes = [0u8; 32];
    for (i, b) in bytes.iter().rev().enumerate() {
        le_bytes[i] = *b;
    }
    Fr::deserialize_uncompressed(&le_bytes[..]).ok()
}

/// Decompose a 32-byte Big-Endian Keccak-256 quote hash into two canonical 128-bit field elements (DEC-035A).
///
/// Returns (quote_hash_hi, quote_hash_lo) where:
/// - quote_hash_hi = bytes[0..16] left-padded with 16 zeros
/// - quote_hash_lo = bytes[16..32] left-padded with 16 zeros
///
/// Because each 128-bit limb is strictly in [0, 2^128 - 1] and 2^128 - 1 < r_BLS12-381,
/// both limbs are guaranteed to be canonical field elements in Fr.
pub fn split_quote_hash_to_limbs(digest: &[u8; 32]) -> (Fr, Fr) {
    let mut hi_padded = [0u8; 32];
    hi_padded[16..32].copy_from_slice(&digest[0..16]);
    let mut lo_padded = [0u8; 32];
    lo_padded[16..32].copy_from_slice(&digest[16..32]);

    let hi = fr_from_be_bytes(&hi_padded)
        .expect("128-bit value left-padded with 16 zero bytes is strictly < r");
    let lo = fr_from_be_bytes(&lo_padded)
        .expect("128-bit value left-padded with 16 zero bytes is strictly < r");
    (hi, lo)
}

/// Recombine two 128-bit field elements back into a 32-byte Big-Endian quote hash (DEC-035A).
///
/// Extracts the lower 16 bytes of both hi and lo field elements:
/// - digest[0..16] = hi_bytes[16..32]
/// - digest[16..32] = lo_bytes[16..32]
pub fn combine_limbs_to_quote_hash(hi: &Fr, lo: &Fr) -> [u8; 32] {
    let hi_bytes = fr_to_be_bytes(hi);
    let lo_bytes = fr_to_be_bytes(lo);
    let mut digest = [0u8; 32];
    digest[0..16].copy_from_slice(&hi_bytes[16..32]);
    digest[16..32].copy_from_slice(&lo_bytes[16..32]);
    digest
}

// ═══════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use ark_std::rand::SeedableRng;
    use ark_std::UniformRand;

    fn test_note() -> PrivateNoteV1 {
        PrivateNoteV1::new(
            99_800_000, // 99.8 USDC
            1,          // USDC
            Fr::from(12345u64),
            Fr::from(67890u64),
            Fr::from(11111u64),
        )
    }

    #[test]
    fn test_domain_separators_distinct() {
        let d1 = domain_note_commitment();
        let d2 = domain_nullifier();
        let d3 = domain_merkle_node();
        let d4 = domain_initial_note();

        assert_ne!(d1, d2, "note commitment ≠ nullifier domain");
        assert_ne!(d1, d3, "note commitment ≠ merkle domain");
        assert_ne!(d1, d4, "note commitment ≠ initial note domain");
        assert_ne!(d2, d3, "nullifier ≠ merkle domain");
        assert_ne!(d2, d4, "nullifier ≠ initial note domain");
        assert_ne!(d3, d4, "merkle ≠ initial note domain");
    }

    #[test]
    fn test_domain_separators_deterministic() {
        assert_eq!(
            domain_note_commitment(),
            domain_note_commitment(),
            "Domain tags must be deterministic"
        );
    }

    #[test]
    fn test_note_commitment_deterministic() {
        let note = test_note();
        let c1 = note.commitment();
        let c2 = note.commitment();
        assert_eq!(c1, c2, "Commitment must be deterministic");
    }

    #[test]
    fn test_note_commitment_binding_value() {
        let note1 = PrivateNoteV1::new(100, 1, Fr::from(1u64), Fr::from(2u64), Fr::from(3u64));
        let note2 = PrivateNoteV1::new(200, 1, Fr::from(1u64), Fr::from(2u64), Fr::from(3u64));
        assert_ne!(
            note1.commitment(),
            note2.commitment(),
            "Different values must produce different commitments"
        );
    }

    #[test]
    fn test_note_commitment_binding_asset_id() {
        let note_weth = PrivateNoteV1::new(100, 0, Fr::from(1u64), Fr::from(2u64), Fr::from(3u64));
        let note_usdc = PrivateNoteV1::new(100, 1, Fr::from(1u64), Fr::from(2u64), Fr::from(3u64));
        assert_ne!(
            note_weth.commitment(),
            note_usdc.commitment(),
            "Different asset_id must produce different commitments (anti-cross-asset)"
        );
    }

    #[test]
    fn test_note_commitment_binding_owner() {
        let note1 = PrivateNoteV1::new(100, 1, Fr::from(1u64), Fr::from(2u64), Fr::from(3u64));
        let note2 = PrivateNoteV1::new(100, 1, Fr::from(99u64), Fr::from(2u64), Fr::from(3u64));
        assert_ne!(
            note1.commitment(),
            note2.commitment(),
            "Different owner keys must produce different commitments"
        );
    }

    #[test]
    fn test_note_commitment_binding_rho() {
        let note1 = PrivateNoteV1::new(100, 1, Fr::from(1u64), Fr::from(2u64), Fr::from(3u64));
        let note2 = PrivateNoteV1::new(100, 1, Fr::from(1u64), Fr::from(99u64), Fr::from(3u64));
        assert_ne!(
            note1.commitment(),
            note2.commitment(),
            "Different rho values must produce different commitments"
        );
    }

    #[test]
    fn test_note_commitment_hiding() {
        // Same note with different randomness produces different commitments
        let note1 = PrivateNoteV1::new(100, 1, Fr::from(1u64), Fr::from(2u64), Fr::from(3u64));
        let note2 = PrivateNoteV1::new(100, 1, Fr::from(1u64), Fr::from(2u64), Fr::from(99u64));
        assert_ne!(
            note1.commitment(),
            note2.commitment(),
            "Different randomness must produce different commitments (hiding)"
        );
    }

    #[test]
    fn test_nullifier_key_derivation() {
        let sk = Fr::from(42u64);
        let nk1 = derive_nullifier_key(sk);
        let nk2 = derive_nullifier_key(sk);
        assert_eq!(nk1, nk2, "Nullifier key derivation must be deterministic");
        assert_ne!(nk1, sk, "Nullifier key must differ from spending key");
    }

    #[test]
    fn test_nullifier_derivation_deterministic() {
        let nk = Fr::from(42u64);
        let cm = Fr::from(999u64);
        let n1 = derive_nullifier(nk, cm, 0, 0);
        let n2 = derive_nullifier(nk, cm, 0, 0);
        assert_eq!(n1, n2, "Nullifier must be deterministic");
    }

    #[test]
    fn test_nullifier_unlinkable_without_key() {
        // Two different notes with different nullifier keys produce different nullifiers
        let nk1 = Fr::from(1u64);
        let nk2 = Fr::from(2u64);
        let cm = Fr::from(999u64);
        let n1 = derive_nullifier(nk1, cm, 0, 0);
        let n2 = derive_nullifier(nk2, cm, 0, 0);
        assert_ne!(n1, n2, "Different keys must produce different nullifiers");
    }

    #[test]
    fn test_nullifier_position_binding() {
        let nk = Fr::from(42u64);
        let cm = Fr::from(999u64);
        let n0 = derive_nullifier(nk, cm, 0, 0);
        let n1 = derive_nullifier(nk, cm, 1, 0);
        assert_ne!(
            n0, n1,
            "Same note at different positions must produce different nullifiers"
        );
    }

    #[test]
    fn test_nullifier_epoch_binding() {
        let nk = Fr::from(42u64);
        let cm = Fr::from(999u64);
        let n_epoch0 = derive_nullifier_epoch(nk, cm, 0, 0);
        let n_epoch1 = derive_nullifier_epoch(nk, cm, 0, 1);
        let n_epoch2 = derive_nullifier_epoch(nk, cm, 0, 2);
        assert_ne!(
            n_epoch0, n_epoch1,
            "Same note at different epochs must produce distinct evolving nullifiers (DEC-033)"
        );
        assert_ne!(
            n_epoch1, n_epoch2,
            "Evolving nullifier must prevent cross-epoch replay"
        );
    }

    #[test]
    fn test_value_conservation_exact() {
        // Input: 100 USDC
        // Merchant: 5 USDC, Protocol fee: 0.0125 USDC, Exec fee: 0.023 USDC
        // Change: 94.9645 USDC
        assert!(verify_value_conservation(
            &[100_000_000], // 100 USDC input
            5_000_000,      // 5 USDC merchant
            12_500,         // 0.0125 USDC protocol fee
            23_000,         // 0.023 USDC execution fee
            &[94_964_500],  // 94.9645 USDC change
        ));
    }

    #[test]
    fn test_value_conservation_overflow_rejected() {
        assert!(!verify_value_conservation(
            &[u64::MAX, 1], // overflow on input sum
            5_000_000,
            12_500,
            23_000,
            &[0],
        ));
    }

    #[test]
    fn test_value_conservation_mismatch_rejected() {
        // Input: 100 USDC, but debits don't add up
        assert!(!verify_value_conservation(
            &[100_000_000],
            5_000_000,
            12_500,
            23_000,
            &[94_000_000], // wrong: should be 94_964_500
        ));
    }

    #[test]
    fn test_value_conservation_no_change() {
        // Full spend: input = merchant + fees, no change
        assert!(verify_value_conservation(
            &[5_035_500], // 5.0355 USDC
            5_000_000,    // 5 USDC merchant
            12_500,       // 0.0125 USDC protocol fee
            23_000,       // 0.023 USDC execution fee
            &[],          // no change
        ));
    }

    #[test]
    fn test_value_conservation_multi_input() {
        // Two input notes consolidated
        assert!(verify_value_conservation(
            &[50_000_000, 60_000_000], // 50 + 60 = 110 USDC
            100_000_000,               // 100 USDC merchant
            250_000,                   // 0.25 USDC protocol fee (0.25%)
            100_000,                   // 0.1 USDC execution fee
            &[9_650_000],              // 9.65 USDC change
        ));
    }

    #[test]
    fn test_fr_serialization_roundtrip() {
        let original = Fr::from(123456789u64);
        let bytes = fr_to_be_bytes(&original);
        let recovered = fr_from_be_bytes(&bytes).expect("valid bytes");
        assert_eq!(
            original, recovered,
            "Fr serialization roundtrip must preserve value"
        );
    }

    #[test]
    fn test_fr_serialization_large_value() {
        let mut rng = ark_std::rand::rngs::StdRng::seed_from_u64(42);
        let original = Fr::rand(&mut rng);
        let bytes = fr_to_be_bytes(&original);
        let recovered = fr_from_be_bytes(&bytes).expect("valid bytes");
        assert_eq!(
            original, recovered,
            "Fr serialization roundtrip for random element"
        );
    }

    #[test]
    fn test_fr_from_be_bytes_rejects_out_of_modulus() {
        // BLS12-381 scalar field modulus r in big-endian:
        // 0x73eda753299d7d483339d80809a1d80553bda402fffe5bfeffffffff00000001
        let r_bytes =
            hex::decode("73eda753299d7d483339d80809a1d80553bda402fffe5bfeffffffff00000001")
                .unwrap();
        let mut r_be = [0u8; 32];
        r_be.copy_from_slice(&r_bytes);

        assert!(
            fr_from_be_bytes(&r_be).is_none(),
            "fr_from_be_bytes must reject modulus r itself"
        );

        // r + 1:
        let mut r_plus_one = r_be;
        r_plus_one[31] += 1;
        assert!(
            fr_from_be_bytes(&r_plus_one).is_none(),
            "fr_from_be_bytes must reject r + 1"
        );

        // 2^256 - 1 (all 0xFF):
        let max_u256 = [0xffu8; 32];
        assert!(
            fr_from_be_bytes(&max_u256).is_none(),
            "fr_from_be_bytes must reject 2^256 - 1"
        );

        // r - 1 should succeed:
        let mut r_minus_one = r_be;
        r_minus_one[31] -= 1;
        assert!(
            fr_from_be_bytes(&r_minus_one).is_some(),
            "fr_from_be_bytes must accept valid field element r - 1"
        );
    }

    // ═══════════════════════════════════════════════════════════════════
    // Known-Answer Vectors (DEC-016A §16)
    // ═══════════════════════════════════════════════════════════════════
    //
    // These vectors serve as the canonical reference for cross-validation
    // between zeltra-core, zeltra-sdk (WASM), and the contract.
    //
    // Seed inputs from DEC-016A §16:
    //   spending_key     = Fr::from(42)
    //   note value       = 99_800_000 (99.8 USDC = 100 USDC - 0.2% deposit fee)
    //   rho              = Fr::from(1)
    //   randomness       = Fr::from(2)
    //   leaf_index       = 0
    //   merchant_amount  = 5_000_000 (5 USDC)
    //   protocol_fee     = 12_500 (0.25% of 5 USDC)
    //   execution_fee    = 23_000

    #[test]
    fn known_answer_domain_separators() {
        // Domain separators are derived from keccak256 labels modulo field order.
        // These MUST be identical across core, SDK, and contract.
        let d_note = domain_note_commitment();
        let d_null = domain_nullifier();
        let d_merkle = domain_merkle_node();
        let d_initial = domain_initial_note();
        let d_quote = domain_quote_binding();
        let d_dummy = domain_dummy_nullifier();

        let d_note_hex = hex::encode(fr_to_be_bytes(&d_note));
        let d_null_hex = hex::encode(fr_to_be_bytes(&d_null));
        let d_merkle_hex = hex::encode(fr_to_be_bytes(&d_merkle));
        let d_initial_hex = hex::encode(fr_to_be_bytes(&d_initial));
        let d_quote_hex = hex::encode(fr_to_be_bytes(&d_quote));
        let d_dummy_hex = hex::encode(fr_to_be_bytes(&d_dummy));

        // Exact known-answer hex strings from canonical Keccak-256 mod r
        assert_eq!(
            d_note_hex,
            "597c16638e14fc84c3c2e5fa39b1ffb6bdfd366a935356788e93be4d1a1c9c3e"
        );
        assert_eq!(
            d_null_hex,
            "6ffd37ee1678eaa2ba73925789b22d44e4c46aaeb536d4e8b87025335393e53b"
        );
        assert_eq!(
            d_merkle_hex,
            "67f21bc90381a2df435f97651a4e7cdb7bef0ebeac88e36ee2ef94a80556f887"
        );
        assert_eq!(
            d_initial_hex,
            "1058a560a8972af549a18b657c0b3d6128890dd72d787a77cddb4eafa131635e"
        );
        assert_eq!(
            d_quote_hex,
            hex::encode(fr_to_be_bytes(&domain_from_label(
                "zeltra.quote.binding.v1"
            )))
        );
        assert_eq!(
            d_dummy_hex,
            hex::encode(fr_to_be_bytes(&domain_from_label(
                "zeltra.note.dummy.nullifier.v1"
            )))
        );

        // All distinct
        let all_domains = [d_note, d_null, d_merkle, d_initial, d_quote, d_dummy];
        let mut domain_set = std::collections::HashSet::new();
        for d in all_domains {
            assert!(
                domain_set.insert(d),
                "Domain tags must be mutually disjoint"
            );
        }
    }

    #[test]
    fn known_answer_note_commitment() {
        // Note: value=99800000, asset_id=0 (WETH), owner_key=Fr(42), rho=Fr(1), randomness=Fr(2)
        let note = PrivateNoteV1::new(
            99_800_000,
            0,
            Fr::from(42u64),
            Fr::from(1u64),
            Fr::from(2u64),
        );
        let cm = note.commitment();
        let cm_hex = hex::encode(fr_to_be_bytes(&cm));

        // Canonical test vector DEC-016A §16
        assert_eq!(
            cm_hex, "43ef223782b9589c0e478d06a4d91ad83c3b4f9f5788691f84fbb913dc7b5b97",
            "Note commitment must match canonical DEC-016A KAV"
        );

        // Determinism check
        let cm2 = note.commitment();
        assert_eq!(cm, cm2, "Commitment must be deterministic");
    }

    #[test]
    fn known_answer_nullifier_key() {
        let spending_key = Fr::from(42u64);
        let nk = derive_nullifier_key(spending_key);
        let nk_hex = hex::encode(fr_to_be_bytes(&nk));

        assert_eq!(
            nk_hex, "267a7038d75f9b423ae7140c4318546f52e90fdf3af2b6deccefe84357c6fb35",
            "Nullifier key must match canonical DEC-016A KAV"
        );
    }

    #[test]
    fn known_answer_nullifier() {
        let spending_key = Fr::from(42u64);
        let nk = derive_nullifier_key(spending_key);

        let note = PrivateNoteV1::new(99_800_000, 0, spending_key, Fr::from(1u64), Fr::from(2u64));
        let cm = note.commitment();
        let nf = derive_nullifier(nk, cm, 0, 0);
        let nf_hex = hex::encode(fr_to_be_bytes(&nf));

        assert_eq!(
            nf_hex, "08ca245adba3a6c8d4d6eb884e4083473dfbaddf3c2e4ee9dd4082158797b8ef",
            "Nullifier (leaf_index=0, epoch_id=0) must match canonical DEC-016A KAV"
        );

        // Non-zero leaf index must produce completely different nullifier (prevents position reuse)
        let nf_idx1 = derive_nullifier(nk, cm, 1, 0);
        assert_ne!(
            nf, nf_idx1,
            "Different leaf index must yield different nullifier"
        );

        // Non-zero epoch_id must produce completely different nullifier (prevents cross-epoch replay DEC-033)
        let nf_epoch1 = derive_nullifier_epoch(nk, cm, 0, 1);
        assert_ne!(
            nf, nf_epoch1,
            "Different epoch_id must yield different nullifier (evolving nullifier PRF)"
        );
    }

    #[test]
    fn known_answer_parent_hash() {
        // Parent hash of two known children: left = Fr(1), right = Fr(2)
        let left = Fr::from(1u64);
        let right = Fr::from(2u64);
        let parent = merkle_hash(left, right);
        let parent_hex = hex::encode(fr_to_be_bytes(&parent));

        // Recompute parent hash deterministically
        let expected_inputs = [left, right, Fr::from(0u64), Fr::from(0u64)];
        let manual_parent = native_poseidon_w5(&expected_inputs, domain_merkle_node());
        assert_eq!(parent, manual_parent);
        assert_eq!(
            parent_hex,
            hex::encode(fr_to_be_bytes(&manual_parent)),
            "Parent hash must be strictly deterministic"
        );
    }

    #[test]
    fn known_answer_value_conservation_spend() {
        // DEC-028 Flat Fee Model:
        // Input note: 10_000_000 (10 USDC)
        // Merchant payout: 3_000_000 (3 USDC)
        // Protocol fee: 13_500 (0.45% of 3 USDC)
        // Relayer execution fee: 50_000
        // Expected change note = 10_000_000 - 3_000_000 - 13_500 - 50_000 = 6_936_500 (6.9365 USDC)
        let input_val = 10_000_000u64;
        let payout = 3_000_000u64;
        let protocol_fee = 13_500u64;
        let exec_fee = 50_000u64;
        let change = input_val - payout - protocol_fee - exec_fee;
        assert_eq!(change, 6_936_500);

        assert!(verify_value_conservation(
            &[input_val],
            payout,
            protocol_fee,
            exec_fee,
            &[change],
        ));

        // Conservation failure if change altered by even 1 micro-USDC
        assert!(!verify_value_conservation(
            &[input_val],
            payout,
            protocol_fee,
            exec_fee,
            &[change + 1],
        ));
        assert!(!verify_value_conservation(
            &[input_val],
            payout,
            protocol_fee,
            exec_fee,
            &[change - 1],
        ));
    }

    #[test]
    fn test_domain_mmr_bag_matches_dec032_bytes() {
        let domain_bag = domain_mmr_bag();
        let domain_bytes = fr_to_be_bytes(&domain_bag);
        assert_eq!(
            domain_bytes, DOMAIN_MMR_BAG_BYTES,
            "domain_mmr_bag() must match DEC-032 DOMAIN_MMR_BAG_BYTES"
        );
    }

    #[test]
    fn test_mmr_sequential_insert_1000_leaves() {
        let mut mmr = MerkleMountainRange::new();
        assert_eq!(mmr.leaf_count, 0);
        assert!(mmr.get_peaks().is_empty());

        let mut roots = Vec::new();
        for i in 0..1000 {
            let leaf = Fr::from((i as u64) * 31 + 7);
            let (idx, root) = mmr.append(leaf);
            assert_eq!(idx, i);
            roots.push(root);

            let expected_peaks_count = (i + 1).count_ones() as usize;
            assert_eq!(
                mmr.get_peaks().len(),
                expected_peaks_count,
                "Number of peaks must equal number of 1-bits in leaf_count at count {}",
                i + 1
            );
        }

        assert_eq!(mmr.leaf_count, 1000);
        // Verify cached_root matches direct bag_peaks calculation
        assert_eq!(
            mmr.get_root(),
            MerkleMountainRange::bag_peaks(&mmr.get_peaks(), mmr.leaf_count),
            "Cached root must exactly match direct bag_peaks computation"
        );

        // Verify proofs for leaves at various checkpoint counts
        let checkpoints = [1usize, 2, 3, 7, 8, 15, 16, 63, 64, 127, 255, 500, 1000];
        for &count in &checkpoints {
            let root = roots[count - 1];
            // Test first, middle, and last leaf of this checkpoint
            let test_indices = [0, count / 2, count - 1];
            for &leaf_idx in &test_indices {
                // Build MMR up to checkpoint count
                let mut sub_mmr = MerkleMountainRange::new();
                for i in 0..count {
                    sub_mmr.append(Fr::from((i as u64) * 31 + 7));
                }
                assert_eq!(sub_mmr.get_root(), root);

                let proof = sub_mmr.generate_proof(leaf_idx);
                let leaf = Fr::from((leaf_idx as u64) * 31 + 7);
                assert!(
                    MerkleMountainRange::verify_proof(leaf, leaf_idx, count, &proof, root),
                    "Proof must verify for leaf {} at count {}",
                    leaf_idx,
                    count
                );
            }
        }
    }

    #[test]
    fn test_mmr_hyperbridge_out_of_bounds_rejected() {
        let mut mmr = MerkleMountainRange::new();
        for i in 0..7 {
            mmr.append(Fr::from(i as u64 + 10));
        }
        let root = mmr.get_root();
        let valid_proof = mmr.generate_proof(6);

        // 1. leaf_index == leaf_count (exactly out of bounds)
        assert!(
            !MerkleMountainRange::verify_proof(Fr::from(16u64), 7, 7, &valid_proof, root),
            "leaf_index == leaf_count must be rejected (Hyperbridge exploit)"
        );

        // 2. leaf_index > leaf_count
        assert!(
            !MerkleMountainRange::verify_proof(Fr::from(16u64), 8, 7, &valid_proof, root),
            "leaf_index > leaf_count must be rejected"
        );

        // 3. Huge out-of-bounds leaf_index
        assert!(
            !MerkleMountainRange::verify_proof(Fr::from(16u64), 999999, 7, &valid_proof, root),
            "Huge out-of-bounds index must be rejected"
        );
    }

    #[test]
    fn test_mmr_tampered_leaf_count_rejected() {
        let mut mmr = MerkleMountainRange::new();
        for i in 0..7 {
            mmr.append(Fr::from(i as u64 + 100));
        }
        let root = mmr.get_root();
        let proof = mmr.generate_proof(3);
        let leaf = Fr::from(103u64);

        // Tampered leaf_count alters bagging binding
        assert!(
            !MerkleMountainRange::verify_proof(leaf, 3, 8, &proof, root),
            "Tampered leaf_count=8 must be rejected"
        );
        assert!(
            !MerkleMountainRange::verify_proof(leaf, 3, 6, &proof, root),
            "Tampered leaf_count=6 must be rejected"
        );
        assert!(
            !MerkleMountainRange::verify_proof(leaf, 3, 0, &proof, root),
            "leaf_count=0 must be rejected"
        );
    }

    #[test]
    fn test_mmr_tampered_mountain_siblings_rejected() {
        let mut mmr = MerkleMountainRange::new();
        for i in 0..7 {
            mmr.append(Fr::from(i as u64 + 1));
        }
        let root = mmr.get_root();
        let mut proof = mmr.generate_proof(2);
        let leaf = Fr::from(3u64);

        assert!(MerkleMountainRange::verify_proof(leaf, 2, 7, &proof, root));

        // Corrupt first mountain sibling
        proof.mountain_siblings[0] += Fr::from(1u64);
        assert!(
            !MerkleMountainRange::verify_proof(leaf, 2, 7, &proof, root),
            "Corrupted mountain sibling must fail verification"
        );
    }

    #[test]
    fn test_mmr_tampered_peak_siblings_rejected() {
        let mut mmr = MerkleMountainRange::new();
        for i in 0..7 {
            mmr.append(Fr::from(i as u64 + 1));
        }
        let root = mmr.get_root();
        let mut proof = mmr.generate_proof(0);
        let leaf = Fr::from(1u64);

        assert!(MerkleMountainRange::verify_proof(leaf, 0, 7, &proof, root));

        // Tamper peak bagging sibling
        proof.peak_bagging_siblings[0] += Fr::from(1u64);
        assert!(
            !MerkleMountainRange::verify_proof(leaf, 0, 7, &proof, root),
            "Corrupted peak bagging sibling must fail verification"
        );
    }

    #[test]
    fn test_mmr_known_answer_test() {
        let mut mmr = MerkleMountainRange::new();
        let (idx0, root0) = mmr.append(Fr::from(42u64));
        assert_eq!(idx0, 0);

        let (idx1, root1) = mmr.append(Fr::from(99u64));
        assert_eq!(idx1, 1);
        assert_ne!(root0, root1);

        let proof0 = mmr.generate_proof(0);
        assert!(MerkleMountainRange::verify_proof(
            Fr::from(42u64),
            0,
            2,
            &proof0,
            root1
        ));

        let proof1 = mmr.generate_proof(1);
        assert!(MerkleMountainRange::verify_proof(
            Fr::from(99u64),
            1,
            2,
            &proof1,
            root1
        ));
    }

    #[test]
    fn test_mmr_wrap_around_scalar_rejected() {
        // [0xff; 32] is strictly >= r (modulus of BLS12-381 Fr)
        let non_canonical = [0xffu8; 32];
        assert!(
            crate::from_evm_scalar(&non_canonical).is_none(),
            "Non-canonical scalar exceeding BLS12-381 Fr modulus must be rejected"
        );

        // Modulus r itself:
        // 0x73eda753299d7d483339d80809a1d80553bda402fffe5bfeffffffff00000001
        let r_bytes =
            hex::decode("73eda753299d7d483339d80809a1d80553bda402fffe5bfeffffffff00000001")
                .unwrap();
        let mut r_arr = [0u8; 32];
        r_arr.copy_from_slice(&r_bytes);
        assert!(
            crate::from_evm_scalar(&r_arr).is_none(),
            "Scalar equal to modulus r must be rejected (not in Fr)"
        );
    }

    #[test]
    fn test_quote_hash_limb_decomposition_boundary_vectors() {
        // Vector 1: 0
        let zero_hash = [0u8; 32];
        let (hi, lo) = split_quote_hash_to_limbs(&zero_hash);
        assert_eq!(hi, Fr::from(0u64));
        assert_eq!(lo, Fr::from(0u64));
        assert_eq!(combine_limbs_to_quote_hash(&hi, &lo), zero_hash);

        // Vector 2: 1
        let mut one_hash = [0u8; 32];
        one_hash[31] = 1;
        let (hi, lo) = split_quote_hash_to_limbs(&one_hash);
        assert_eq!(hi, Fr::from(0u64));
        assert_eq!(lo, Fr::from(1u64));
        assert_eq!(combine_limbs_to_quote_hash(&hi, &lo), one_hash);

        // Vector 3: 2^128 - 1 (low limb max, high limb 0)
        let mut max_u128_hash = [0u8; 32];
        max_u128_hash[16..32].fill(0xff);
        let (hi, lo) = split_quote_hash_to_limbs(&max_u128_hash);
        assert_eq!(hi, Fr::from(0u64));
        assert_eq!(combine_limbs_to_quote_hash(&hi, &lo), max_u128_hash);

        // Vector 4: 2^128 (high limb 1, low limb 0)
        let mut two_power_128_hash = [0u8; 32];
        two_power_128_hash[15] = 1;
        let (hi, lo) = split_quote_hash_to_limbs(&two_power_128_hash);
        assert_eq!(hi, Fr::from(1u64));
        assert_eq!(lo, Fr::from(0u64));
        assert_eq!(combine_limbs_to_quote_hash(&hi, &lo), two_power_128_hash);

        // Vector 5: 2^256 - 1 (both limbs 2^128 - 1, strictly < r)
        // This is in the "Danger Zone" (> r), proving DEC-035A works where naive scalar fails!
        let max_u256_hash = [0xffu8; 32];
        assert!(
            crate::from_evm_scalar(&max_u256_hash).is_none(),
            "Max u256 exceeds r"
        );
        let (hi, lo) = split_quote_hash_to_limbs(&max_u256_hash);
        assert_eq!(combine_limbs_to_quote_hash(&hi, &lo), max_u256_hash);

        // Vector 6: Arbitrary Keccak digest
        let keccak_sample: [u8; 32] = [
            0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc, 0xde, 0xf0, 0xfe, 0xdc, 0xba, 0x98, 0x76, 0x54,
            0x32, 0x10, 0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff, 0x00, 0x11, 0x22, 0x33, 0x44, 0x55,
            0x66, 0x77, 0x88, 0x99,
        ];
        let (hi, lo) = split_quote_hash_to_limbs(&keccak_sample);
        assert_eq!(combine_limbs_to_quote_hash(&hi, &lo), keccak_sample);

        // Negative test: Tampering hi or lo alters combined hash
        let hi_tampered = hi + Fr::from(1u64);
        assert_ne!(
            combine_limbs_to_quote_hash(&hi_tampered, &lo),
            keccak_sample
        );
        let lo_tampered = lo + Fr::from(1u64);
        assert_ne!(
            combine_limbs_to_quote_hash(&hi, &lo_tampered),
            keccak_sample
        );
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // DEC-037 Unit Tests (Gate H)
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_dec037_key_derivation_deterministic() {
        let seed = b"test_client_signature_0x123456789abcdef";
        let master1 = derive_master_secret(seed);
        let master2 = derive_master_secret(seed);
        assert_eq!(master1, master2);

        let sk1 = derive_spending_key(&master1);
        let sk2 = derive_spending_key(&master2);
        assert_eq!(sk1, sk2);

        let (ivk1, ovk1) = derive_viewing_keys(seed);
        let (ivk2, ovk2) = derive_viewing_keys(seed);
        assert_eq!(ivk1.secret_bytes(), ivk2.secret_bytes());
        assert_eq!(ivk1.public_key(), ivk2.public_key());
        assert_eq!(ovk1.as_bytes(), ovk2.as_bytes());

        // Different seed produces different keys
        let (ivk_diff, _) = derive_viewing_keys(b"other_seed");
        assert_ne!(ivk1.public_key(), ivk_diff.public_key());
    }

    #[test]
    fn test_dec037_memo_encryption_roundtrip() {
        let seed = b"alice_test_seed_wallet_encryption";
        let (ivk, _ovk) = derive_viewing_keys(seed);

        let value = 125_000_000u64; // 125 USDC
        let rho = Fr::from(999888777u64);
        let asset_id = 1u64; // USDC

        // Encrypt with IVK public key
        let memo = encrypt_note_memo(value, rho, asset_id, &ivk.public_key());
        assert_eq!(memo.len(), ENCRYPTED_MEMO_SIZE);

        // Decrypt with IVK
        let decrypted = decrypt_note_memo(&memo, &ivk).expect("Decryption must succeed");
        assert_eq!(decrypted.value, value);
        assert_eq!(decrypted.rho, rho);
        assert_eq!(decrypted.asset_id, asset_id);

        // Also works passing &ivk directly via RecipientViewingKey trait
        let memo_ivk = encrypt_note_memo(value, rho, asset_id, &ivk);
        let decrypted_ivk = decrypt_note_memo(&memo_ivk, &ivk).expect("Decryption must succeed");
        assert_eq!(decrypted_ivk.value, value);
    }

    #[test]
    fn test_dec037_reconstruct_note_matches_commitment() {
        let seed = b"bob_deterministic_note_recovery_seed";
        let master = derive_master_secret(seed);
        let spending_key = derive_spending_key(&master);
        let (ivk, _) = derive_viewing_keys(seed);

        let value = 50_000_000u64; // 50 USDC
        let rho = Fr::from(424242u64);
        let asset_id = 1u64;

        // Ground-truth note commitment
        let deterministic_r = derive_note_randomness(spending_key, rho);
        let expected_cm =
            note_commitment(value, asset_id as u8, spending_key, rho, deterministic_r);

        // Encrypt and trial-decrypt memo
        let memo = encrypt_note_memo(value, rho, asset_id, &ivk);
        let decrypted_pt = decrypt_note_memo(&memo, &ivk).expect("Decryption must succeed");

        // Reconstruct note from recovered plaintext + spending key
        let reconstructed_note = decrypted_pt.to_note(spending_key);
        assert_eq!(reconstructed_note.value, value);
        assert_eq!(reconstructed_note.asset_id, asset_id as u8);
        assert_eq!(reconstructed_note.owner_key, spending_key);
        assert_eq!(reconstructed_note.rho, rho);
        assert_eq!(reconstructed_note.randomness, deterministic_r);
        assert_eq!(reconstructed_note.commitment(), expected_cm);
    }

    #[test]
    fn test_dec037_decrypt_wrong_key_fails() {
        let (ivk_alice, _) = derive_viewing_keys(b"alice_key");
        let (ivk_bob, _) = derive_viewing_keys(b"bob_key");

        let memo = encrypt_note_memo(10_000_000, Fr::from(12345u64), 1, &ivk_alice);

        // Bob cannot decrypt Alice's memo (Poly1305 MAC tag failure)
        let attempt = decrypt_note_memo(&memo, &ivk_bob);
        assert!(
            attempt.is_none(),
            "Decryption with wrong viewing key must fail authentication"
        );
    }

    #[test]
    fn test_dec037_tampered_ciphertext_fails() {
        let (ivk, _) = derive_viewing_keys(b"tamper_test_seed");
        let mut memo = encrypt_note_memo(10_000_000, Fr::from(12345u64), 1, &ivk);

        // Tamper with byte in ciphertext section (bytes 32..80)
        memo[40] ^= 0x01;

        let attempt = decrypt_note_memo(&memo, &ivk);
        assert!(
            attempt.is_none(),
            "Decryption with tampered ciphertext must be rejected by AEAD MAC"
        );
    }

    #[test]
    fn test_dec037_tampered_mac_tag_fails() {
        let (ivk, _) = derive_viewing_keys(b"mac_tamper_test_seed");
        let mut memo = encrypt_note_memo(10_000_000, Fr::from(12345u64), 1, &ivk);

        // Tamper with byte in Poly1305 MAC tag section (bytes 80..96)
        memo[90] ^= 0xff;

        let attempt = decrypt_note_memo(&memo, &ivk);
        assert!(
            attempt.is_none(),
            "Decryption with tampered MAC tag must be rejected"
        );
    }

    #[test]
    fn test_dec037_invalid_memo_length_fails() {
        let (ivk, _) = derive_viewing_keys(b"length_test_seed");

        assert!(decrypt_note_memo(&[], &ivk).is_none());
        assert!(decrypt_note_memo(&[0u8; 95], &ivk).is_none());
        assert!(decrypt_note_memo(&[0u8; 97], &ivk).is_none());
        assert!(decrypt_note_memo(&[0u8; 32], &ivk).is_none());
    }

    #[test]
    fn test_dec039_optional_encryption_graceful_fallback() {
        let (ivk, _) = derive_viewing_keys(b"optional_encryption_seed");

        let memo = encrypt_note_memo_optional(10_000_000, Fr::from(12345u64), 1, &ivk);
        assert!(memo.is_some());
        assert_eq!(memo.unwrap().len(), ENCRYPTED_MEMO_SIZE);
    }

    #[test]
    fn test_dec039_empty_memo_handling() {
        let (ivk, _) = derive_viewing_keys(b"empty_memo_seed");

        let empty_memo: Vec<u8> = vec![];
        let result = decrypt_note_memo(&empty_memo, &ivk);
        assert!(result.is_none());
    }

    #[test]
    fn test_dec037_non_canonical_scalar_rejected() {
        // Construct 48-byte buffer where rho is >= r
        let mut buf = [0u8; 48];
        buf[0..8].copy_from_slice(&100u64.to_be_bytes());
        buf[8..40].fill(0xff); // Exceeds r
        buf[40..48].copy_from_slice(&1u64.to_be_bytes());

        let pt = NotePlaintext::from_bytes(&buf);
        assert!(
            pt.is_none(),
            "Non-canonical scalar in NotePlaintext must be rejected"
        );
    }

    #[test]
    fn test_dec037_trial_decryption_1000_random_notes_simulation() {
        use rand::RngCore;

        let (ivk, _) = derive_viewing_keys(b"simulation_ivk_seed");
        let (other_ivk, _) = derive_viewing_keys(b"other_client_seed");

        // Prepare 3 valid notes encrypted specifically for this ivk
        let valid_note_1 = encrypt_note_memo(1_000_000, Fr::from(111u64), 1, &ivk);
        let valid_note_2 = encrypt_note_memo(2_500_000, Fr::from(222u64), 1, &ivk);
        let valid_note_3 = encrypt_note_memo(5_000_000, Fr::from(333u64), 1, &ivk);

        let mut memos: Vec<Vec<u8>> = Vec::with_capacity(1000);
        let mut rng = rand::thread_rng();

        // 1. Generate 900 completely random 96-byte buffers
        for _ in 0..900 {
            let mut buf = vec![0u8; 96];
            rng.fill_bytes(&mut buf);
            memos.push(buf);
        }

        // 2. Generate 97 notes encrypted for another recipient (valid ciphertexts, wrong recipient)
        for i in 0..97 {
            let memo_other = encrypt_note_memo(500_000 + i, Fr::from(1000u64 + i), 1, &other_ivk);
            memos.push(memo_other);
        }

        // 3. Inject the 3 valid notes at deterministic locations
        memos.insert(100, valid_note_1);
        memos.insert(500, valid_note_2);
        memos.insert(999, valid_note_3);

        assert_eq!(memos.len(), 1000);

        let start = std::time::Instant::now();
        let mut recovered = Vec::new();

        for (idx, memo) in memos.iter().enumerate() {
            if let Some(pt) = decrypt_note_memo(memo, &ivk) {
                recovered.push((idx, pt));
            }
        }

        let elapsed = start.elapsed();
        assert_eq!(
            recovered.len(),
            3,
            "Exactly 3 notes should decrypt successfully for this IVK"
        );
        assert_eq!(recovered[0].0, 100);
        assert_eq!(recovered[1].0, 500);
        assert_eq!(recovered[2].0, 999);

        eprintln!("Trial decryption 1000 notes: {} ms", elapsed.as_millis());
    }

    #[test]
    fn test_dec037_memo_commitment_mismatch_funds_locked() {
        let seed = b"victim_wallet_seed";
        let (ivk, _ovk) = derive_viewing_keys(seed);
        let master = derive_master_secret(seed);
        let spending_key = derive_spending_key(&master);

        let value_a = 100_000_000u64;
        let rho_a = Fr::from(123456789u64);
        let randomness_a = derive_note_randomness(spending_key, rho_a);
        let note_a = PrivateNoteV1::new(value_a, 1, spending_key, rho_a, randomness_a);
        let commitment_a = note_a.commitment();

        let value_b = 50_000_000u64;
        let rho_b = Fr::from(987654321u64);
        let asset_id_b = 1u64;

        let mismatched_memo = encrypt_note_memo(value_b, rho_b, asset_id_b, &ivk);
        let decrypted = decrypt_note_memo(&mismatched_memo, &ivk);

        if let Some(pt) = decrypted {
            assert_eq!(pt.value, value_b);
            assert_eq!(pt.rho, rho_b);
            assert_eq!(pt.asset_id, asset_id_b);

            let reconstructed_note = pt.to_note(spending_key);
            let reconstructed_commitment = reconstructed_note.commitment();

            assert_ne!(
                reconstructed_commitment, commitment_a,
                "Memo plaintext does NOT match on-chain commitment"
            );
        }

        let (ivk_attacker, _) = derive_viewing_keys(b"attacker_seed");
        let memo_attacker = encrypt_note_memo(value_b, rho_b, asset_id_b, &ivk_attacker);
        let decrypted_attacker = decrypt_note_memo(&memo_attacker, &ivk);

        assert!(
            decrypted_attacker.is_none(),
            "Victim cannot decrypt memo encrypted for attacker's viewing key"
        );
    }

    #[test]
    fn test_bit_packing_roundtrip_soundness() {
        use ark_ff::PrimeField;
        // Test boundary values for value (u64) and asset_id (0 and 1)
        let test_values = [0u64, 1u64, 100_000_000u64, u64::MAX - 1, u64::MAX];
        let two_pow_64 = Fr::from(1u128 << 64);

        for &val in &test_values {
            for &aid in &[0u8, 1u8] {
                let note = PrivateNoteV1::new(
                    val,
                    aid,
                    Fr::from(100u64),
                    Fr::from(200u64),
                    Fr::from(300u64),
                );
                let packed = note.value_packed();

                // Reconstruct value and asset_id from Fr representation
                let bi = packed.into_bigint();
                let limbs = bi.as_ref();
                let recovered_val = limbs[0];
                let recovered_aid = limbs[1] as u8;

                assert_eq!(
                    recovered_val, val,
                    "Recovered value mismatch for val={val}, aid={aid}"
                );
                assert_eq!(
                    recovered_aid, aid,
                    "Recovered asset_id mismatch for val={val}, aid={aid}"
                );

                // Confirm algebraic linear combination: packed == val + aid * 2^64
                let expected_packed = Fr::from(val) + Fr::from(aid as u64) * two_pow_64;
                assert_eq!(packed, expected_packed);
            }
        }
    }

    #[test]
    fn test_dec037_note_plaintext_roundtrip_with_asset_id() {
        for &aid in &[0u64, 1u64] {
            let (ivk, _) = derive_viewing_keys(b"dec037_roundtrip_seed");
            let val = 25_000_000u64;
            let rho = Fr::from(999888777u64);

            let memo = encrypt_note_memo(val, rho, aid, &ivk);
            assert_eq!(memo.len(), 96, "Memo must be exactly 96 bytes");

            let decrypted = decrypt_note_memo(&memo, &ivk).expect("Decryption must succeed");
            assert_eq!(decrypted.value, val);
            assert_eq!(decrypted.asset_id, aid);
            assert_eq!(decrypted.rho, rho);

            // Reconstruct note and verify commitment
            let sk = Fr::from(123456u64);
            let note = decrypted.to_note(sk);
            assert_eq!(note.value, val);
            assert_eq!(note.asset_id, aid as u8);
            assert_eq!(note.owner_key, sk);
            assert_eq!(note.rho, rho);
            assert_eq!(
                note.commitment(),
                note_commitment(val, aid as u8, sk, rho, note.randomness)
            );
        }
    }
}
