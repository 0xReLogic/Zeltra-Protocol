//! Key Derivation & Viewing Keys — DEC-037 & DEC-040A & DEC-043A
//!
//! Provides cryptographic key types and derivation functions:
//! - `IncomingViewingKey` (IVK) for trial decryption of incoming notes
//! - `OutcomingViewingKey` (OVK) for viewing outbound payments and change
//! - `FullViewingKey` (FVK) for CEX compliance and non-spending auditing
//! - `ZeltraShieldedAddress` (ZSA) BIP-350 Bech32m shielded address format (`zs1...`)
//! - Key derivation hierarchy: Seed -> Master Secret -> Spending Key -> Owner Key & Viewing Keys

use ark_bls12_381::Fr;
use ark_ff::PrimeField;
use serde::{Deserialize, Serialize};
use zeroize::Zeroize;

use crate::note::{domain_from_label, fr_from_be_bytes, fr_to_be_bytes};

/// Domain tag for one-way owner key derivation from spending key (DEC-043A).
pub fn domain_owner_key() -> Fr {
    domain_from_label("zeltra.owner.key.v1")
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

/// Incoming Viewing Key (IVK) for trial decryption of notes addressed to the owner.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct IncomingViewingKey {
    pub(crate) secret: [u8; 32],
    pub(crate) public: [u8; 32],
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

    /// Computes X25519 Diffie-Hellman shared secret with an ephemeral public key.
    pub fn diffie_hellman(&self, eph_pk_bytes: &[u8; 32]) -> [u8; 32] {
        let static_secret = x25519_dalek::StaticSecret::from(self.secret);
        let epk = x25519_dalek::PublicKey::from(*eph_pk_bytes);
        *static_secret.diffie_hellman(&epk).as_bytes()
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

/// Zeltra Shielded Address (ZSA) representing a 64-byte recipient destination (DEC-043A).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct ZeltraShieldedAddress {
    #[serde(serialize_with = "serialize_fr", deserialize_with = "deserialize_fr")]
    pub owner_key: Fr,
    #[serde(
        serialize_with = "serialize_bytes32",
        deserialize_with = "deserialize_bytes32"
    )]
    pub incoming_viewing_public_key: [u8; 32],
}

impl ZeltraShieldedAddress {
    pub const HRP: &'static str = "zs";
    pub const BYTE_LEN: usize = 64;

    pub fn new(owner_key: Fr, incoming_viewing_public_key: [u8; 32]) -> Self {
        Self {
            owner_key,
            incoming_viewing_public_key,
        }
    }

    /// Derives ZSA deterministically from a master spending key and viewing key.
    pub fn from_keys(spending_key: Fr, incoming_viewing_public_key: [u8; 32]) -> Self {
        let owner_key = derive_owner_key(spending_key);
        Self {
            owner_key,
            incoming_viewing_public_key,
        }
    }

    /// Canonical 64-byte serialization (32B owner_key || 32B pk_ivk).
    pub fn to_bytes(&self) -> [u8; Self::BYTE_LEN] {
        let mut bytes = [0u8; Self::BYTE_LEN];
        bytes[0..32].copy_from_slice(&fr_to_be_bytes(&self.owner_key));
        bytes[32..64].copy_from_slice(&self.incoming_viewing_public_key);
        bytes
    }

    /// Deserializes 64 bytes into ZSA. Rejects non-canonical scalar field elements (>= r).
    pub fn from_bytes(bytes: &[u8; Self::BYTE_LEN]) -> Option<Self> {
        let mut ok_bytes = [0u8; 32];
        ok_bytes.copy_from_slice(&bytes[0..32]);
        let owner_key = fr_from_be_bytes(&ok_bytes)?;
        let mut pk = [0u8; 32];
        pk.copy_from_slice(&bytes[32..64]);
        Some(Self {
            owner_key,
            incoming_viewing_public_key: pk,
        })
    }

    /// Encodes ZSA into canonical 128-character hex string.
    pub fn to_hex(&self) -> String {
        hex::encode(self.to_bytes())
    }

    /// Parses ZSA from a 0x-prefixed or raw 128-character hex string.
    pub fn from_hex(hex_str: &str) -> Result<Self, KeyFormatError> {
        let clean = hex_str.strip_prefix("0x").unwrap_or(hex_str);
        if clean.len() != 128 {
            return Err(KeyFormatError::InvalidLength {
                expected: 128,
                actual: clean.len(),
            });
        }
        let bytes = hex::decode(clean).map_err(|_| KeyFormatError::InvalidHexEncoding)?;
        let mut arr = [0u8; 64];
        arr.copy_from_slice(&bytes);
        Self::from_bytes(&arr).ok_or(KeyFormatError::NonCanonicalScalar { field: "owner_key" })
    }

    /// Encodes ZSA into Bech32m string format with "zs" HRP.
    pub fn to_bech32m(&self) -> Result<String, bech32::EncodeError> {
        use bech32::{Bech32m, Hrp};
        let hrp = Hrp::parse(Self::HRP).expect("valid hrp 'zs'");
        let data = self.to_bytes();
        bech32::encode::<Bech32m>(hrp, &data)
    }

    /// Parses ZSA from a Bech32m string format with "zs" HRP.
    pub fn from_bech32m(s: &str) -> Result<Self, KeyFormatError> {
        use bech32::primitives::decode::CheckedHrpstring;
        use bech32::{Bech32m, Hrp};
        let checked =
            CheckedHrpstring::new::<Bech32m>(s).map_err(|_| KeyFormatError::InvalidHexEncoding)?;
        let expected_hrp = Hrp::parse(Self::HRP).expect("valid hrp 'zs'");
        if checked.hrp() != expected_hrp {
            return Err(KeyFormatError::InvalidLength {
                expected: 64,
                actual: 0,
            });
        }
        let data: Vec<u8> = checked.byte_iter().collect();
        if data.len() != 64 {
            return Err(KeyFormatError::InvalidLength {
                expected: 64,
                actual: data.len(),
            });
        }
        let mut arr = [0u8; 64];
        arr.copy_from_slice(&data);
        Self::from_bytes(&arr).ok_or(KeyFormatError::NonCanonicalScalar { field: "owner_key" })
    }
}

impl core::fmt::Display for ZeltraShieldedAddress {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self.to_bech32m() {
            Ok(s) => write!(f, "{}", s),
            Err(_) => write!(f, "0x{}", self.to_hex()),
        }
    }
}

impl core::str::FromStr for ZeltraShieldedAddress {
    type Err = KeyFormatError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.starts_with(Self::HRP) {
            Self::from_bech32m(s)
        } else {
            Self::from_hex(s)
        }
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

/// Derives the public owner key scalar (Fr) from a master spending key (DEC-043A).
pub fn derive_owner_key(spending_key: Fr) -> Fr {
    crate::poseidon::native_poseidon_w3(spending_key, Fr::from(0u64), domain_owner_key())
}

/// Derive the nullifier key from the spending key.
///
/// nullifier_key = Poseidon_W3(spending_key, DOMAIN_NULLIFIER_V1)
///
/// Uses the existing width-3 Poseidon (rate=2) since we only have 2 inputs:
/// the spending key and the domain tag.
pub fn derive_nullifier_key(spending_key: Fr) -> Fr {
    crate::poseidon::compute_nullifier(spending_key, crate::note::domain_nullifier())
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
