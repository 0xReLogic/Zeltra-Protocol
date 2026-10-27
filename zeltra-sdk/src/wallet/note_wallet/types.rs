use ark_bls12_381::Fr;
use ark_ff::{PrimeField, UniformRand};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use zeltra_core::{derive_joinsplit_dummy_nullifier, derive_joinsplit_nullifier, fr_from_be_bytes};

/// Minimum non-dust amount (1000 base units = 0.001 USDC)
pub const DUST_THRESHOLD: u64 = 1_000;

/// Default lease TTL for note reservations in seconds (2 minutes)
pub const DEFAULT_RESERVATION_TTL_SECS: u64 = 120;

/// Safely parses an arkworks Fr scalar from a 0x-prefixed or raw hex string (DEC-030).
pub fn parse_fr_from_hex(hex_str: &str) -> Result<Fr, WalletError> {
    let clean = hex_str.strip_prefix("0x").unwrap_or(hex_str);
    let bytes = hex::decode(clean).map_err(|e| WalletError::InvalidHex(e.to_string()))?;
    if bytes.len() != 32 {
        return Err(WalletError::CryptoError(format!(
            "Invalid scalar byte length: expected 32, got {}",
            bytes.len()
        )));
    }
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&bytes);
    fr_from_be_bytes(&arr)
        .ok_or_else(|| WalletError::CryptoError("Non-canonical field scalar".into()))
}

/// Safely parses a 32-byte scalar array or 20-byte EVM address (left-padded to 32 bytes) from hex (DEC-030).
pub fn parse_bytes32_from_hex(
    hex_str: &str,
    pad_20_byte_address: bool,
) -> Result<[u8; 32], WalletError> {
    let clean = hex_str.strip_prefix("0x").unwrap_or(hex_str);
    let dec = hex::decode(clean).map_err(|e| WalletError::InvalidHex(e.to_string()))?;
    let mut buf = [0u8; 32];
    if dec.len() == 20 && pad_20_byte_address {
        buf[12..].copy_from_slice(&dec);
        Ok(buf)
    } else if dec.len() == 32 {
        buf.copy_from_slice(&dec);
        Ok(buf)
    } else {
        Err(WalletError::InvalidHex(format!(
            "Invalid byte length: expected {}32, got {}",
            if pad_20_byte_address { "20 or " } else { "" },
            dec.len()
        )))
    }
}

/// Note Lifecycle Status
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum NoteStatus {
    /// Newly created note (deposit or change) awaiting on-chain inclusion
    Unconfirmed,
    /// Direct deposit prepared and staged, awaiting NoteCommitmentAppended event (DEC-038C)
    PendingDeposit,
    /// Confirmed on-chain and ready for spending
    Unspent,
    /// Temporarily locked for an active spending session
    Reserved {
        session_id: String,
        lease_expiry_secs: u64,
    },
    /// Consumed on-chain with known nullifier
    Spent {
        nullifier_hex: String,
        spent_at_secs: u64,
    },
}

/// Direct Deposit Payload returned by client SDK for on-chain deposit_direct_note (DEC-038C)
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct DirectDepositPayload {
    pub amount: u64,
    pub commitment_bytes: [u8; 32],
    pub encrypted_memo: Vec<u8>,
}

/// CCIP Inbound Programmable Deposit Payload (DEC-055)
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct CcipInboundDepositPayload {
    pub amount: u64,
    pub asset_id: u8,
    pub commitment_bytes: [u8; 32],
    pub encrypted_memo: Vec<u8>,
    pub wire_payload_128: Vec<u8>,
    pub destination_chain_selector: u64,
    pub destination_contract: [u8; 20],
    pub source_token: [u8; 20],
}

impl CcipInboundDepositPayload {
    /// Builds the 128-byte wire payload: [32B commitment || 96B memo]
    pub fn build_wire_payload(commitment: &[u8; 32], memo: &[u8]) -> Result<Vec<u8>, WalletError> {
        if memo.len() != 96 {
            return Err(WalletError::CryptoError(format!(
                "Invalid memo length for CCIP deposit: expected 96, got {}",
                memo.len()
            )));
        }
        let mut wire = vec![0u8; 128];
        wire[0..32].copy_from_slice(commitment);
        wire[32..128].copy_from_slice(memo);
        Ok(wire)
    }
}

mod fr_serde {
    use serde::{Deserialize, Deserializer, Serializer};
    use zeltra_core::{fr_to_be_bytes, Fr};

    pub fn serialize<S>(fr: &Fr, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let hex_str = format!("0x{}", hex::encode(fr_to_be_bytes(fr)));
        serializer.serialize_str(&hex_str)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Fr, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        let clean = s.strip_prefix("0x").unwrap_or(&s);
        let bytes = hex::decode(clean).map_err(serde::de::Error::custom)?;
        if bytes.len() != 32 {
            return Err(serde::de::Error::custom("expected 32 bytes for Fr"));
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes);
        zeltra_core::from_evm_scalar(&arr)
            .ok_or_else(|| serde::de::Error::custom("non-canonical scalar"))
    }
}

/// Private Note V1 with integrated asset_id (DEC-034C / Patch Audit #5)
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrivateNoteV1 {
    pub value: u64,   // Gwei (if asset_id == 0) or Micro-USDC (if asset_id == 1)
    pub asset_id: u8, // 0: WETH, 1: USDC
    #[serde(with = "fr_serde")]
    pub owner_key: Fr, // Public owner key (not raw spending_key)
    #[serde(with = "fr_serde")]
    pub rho: Fr,
    #[serde(with = "fr_serde")]
    pub randomness: Fr,
}

impl PrivateNoteV1 {
    pub fn new(value: u64, asset_id: u8, owner_key: Fr, rho: Fr, randomness: Fr) -> Self {
        Self {
            value,
            asset_id,
            owner_key,
            rho,
            randomness,
        }
    }

    /// Computes canonical bit-packed value according to DEC-034A: value + asset_id * 2^64
    pub fn value_packed(&self) -> Fr {
        let two_pow_64 = Fr::from(1u128 << 64);
        Fr::from(self.value) + (Fr::from(self.asset_id as u64) * two_pow_64)
    }

    /// Computes leaf commitment Poseidon-W5
    pub fn commitment(&self) -> Fr {
        zeltra_core::note_commitment(
            self.value,
            self.asset_id,
            self.owner_key,
            self.rho,
            self.randomness,
        )
    }
}

/// Deterministically derives the note hiding randomness r from shared secret and rho (DEC-037 / DEC-034C).
pub fn derive_note_randomness(shared_secret: &[u8; 32], rho: &Fr) -> Fr {
    let secret_fr = Fr::from_be_bytes_mod_order(shared_secret);
    zeltra_core::derive_note_randomness(secret_fr, *rho)
}

fn default_wallet_note_asset_id() -> u8 {
    1 // USDC
}

/// Note Classification for DEC-051 Cryptographic Deniability & Decoy Dummy Notes
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum NoteClassification {
    #[default]
    ActiveSpendable,
    PendingSpend,
    Spent,
    DecoyDummy, // Nopenena decoy note (v = 0, excluded from spendable balance)
}

/// UTXO Note stored in client wallet
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WalletNote {
    pub commitment_hex: String,
    pub value: u64,
    #[serde(default = "default_wallet_note_asset_id")]
    pub asset_id: u8,
    pub owner_key_hex: String,
    pub rho_hex: String,
    pub randomness_hex: String,
    pub leaf_index: Option<u64>,
    pub leaf_count: Option<u64>,
    #[serde(default)]
    pub epoch_id: Option<u32>,
    pub merkle_path_hex: Option<Vec<String>>,
    pub status: NoteStatus,
    #[serde(default)]
    pub classification: NoteClassification, // DEC-051
    pub created_at_secs: u64,
    pub session_id: Option<String>,
}

/// Classification of a note's generation epoch relative to current on-chain epoch (DEC-033)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum NoteEpochStatus {
    /// Fresh note minted in the current active epoch E. Can be spent directly.
    Fresh,
    /// Mature note in grace window E-1 (valid up to 180 days). Direct spendable or auto in-flight rollover.
    MatureGrace,
    /// Expired note older than active window (<= E-2, > 180 days). Requires rollover before spend.
    ExpiredRequiresRollover,
}

impl WalletNote {
    pub fn epoch(&self) -> u32 {
        self.epoch_id.unwrap_or(0)
    }

    pub fn is_decoy(&self) -> bool {
        self.classification == NoteClassification::DecoyDummy || self.value == 0
    }

    pub fn check_epoch_status(&self, current_epoch: u32) -> NoteEpochStatus {
        let note_epoch = self.epoch();
        if note_epoch == current_epoch {
            NoteEpochStatus::Fresh
        } else if note_epoch == current_epoch.saturating_sub(1) {
            NoteEpochStatus::MatureGrace
        } else {
            NoteEpochStatus::ExpiredRequiresRollover
        }
    }
}

impl Drop for WalletNote {
    fn drop(&mut self) {
        crate::secure_zeroize_string(&mut self.owner_key_hex);
        crate::secure_zeroize_string(&mut self.rho_hex);
        crate::secure_zeroize_string(&mut self.randomness_hex);
    }
}

/// Result of Coin Selection (supporting 1-note and 2-note JoinSplit — DEC-030)
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelectedSpend {
    pub input_commitment_hex: String,
    pub input_value: u64,
    pub second_input_commitment_hex: Option<String>,
    pub second_input_value: u64,
    pub merchant_amount: u64,
    pub protocol_fee: u64,
    pub execution_fee: u64,
    pub total_required: u64,
    pub change_amount: u64,
    pub has_change: bool,
    pub has_decoy_dummy: bool, // DEC-051
}

impl SelectedSpend {
    /// Returns true if this spend uses 2 input notes (JoinSplit).
    pub fn is_joinsplit(&self) -> bool {
        self.second_input_commitment_hex.is_some()
    }

    /// Returns the combined total of all input notes.
    pub fn total_input_value(&self) -> u64 {
        self.input_value + self.second_input_value
    }
}

/// Note commitment appended on-chain event log representation (DEC-037).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NoteEvent {
    pub leaf_index: u64,
    pub commitment_hex: String,
    pub new_mmr_root_hex: String,
    pub leaf_count: u64,
    pub encrypted_memo: Vec<u8>,
    #[serde(default)]
    pub block_number: Option<u64>,
}

/// A note recovered via trial decryption from an on-chain memo (DEC-037).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct RecoveredNote {
    pub commitment_hex: String,
    pub nullifier_hex: String,
    pub leaf_index: u64,
    pub value: u64,
    pub asset_id: u64,
    pub rho_hex: String,
    pub is_spent: bool,
}

/// Result of trial decryption and balance recovery (DEC-037).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RecoveryResult {
    pub scanned_events: usize,
    pub recovered_notes: Vec<RecoveredNote>,
    pub total_unspent_balance: u64,
    pub total_spent_balance: u64,
}

/// Spend Proof Payload generated locally by client SDK (1-in 1-out PrivateNoteCircuit, DEC-032, DEC-033)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SpendProofPayload {
    pub session_id: String,
    pub input_commitment_hex: String,
    pub input_nullifier_hex: String,
    pub note_root_hex: String,
    pub leaf_count: u64,
    #[serde(default)]
    pub note_epoch_id: u32,
    pub output_commitment_hex: String,
    pub recipient_hex: String,
    pub merchant_amount: u64,
    pub protocol_fee: u64,
    pub execution_fee: u64,
    pub max_execution_fee: u64,
    pub quote_hash_hex: String,
    pub chain_id: u64,
    pub contract_address_hex: String,
    pub expiry: u64,
    pub has_change: bool,
    #[serde(default)]
    pub is_rollover: bool,
    pub proof_a_neg_hex: String,        // 128 bytes EVM format
    pub proof_b_hex: String,            // 256 bytes EVM format
    pub proof_c_hex: String,            // 128 bytes EVM format
    pub public_inputs_hex: Vec<String>, // 16 x 32 bytes EVM scalars (DEC-035A)
    #[serde(default)]
    pub change_encrypted_memo_hex: Option<String>, // 96-byte AEAD memo (DEC-037)
}

/// Universal 2-in-2-out JoinSplit Spend Proof Payload generated locally by client SDK (DEC-030 / DEC-036B)
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct JoinSplitSpendProofPayload {
    pub session_id: String,
    pub input_commitment_1_hex: String,
    pub input_commitment_2_hex: Option<String>,
    pub input_nullifier_1_hex: String,
    pub input_nullifier_2_hex: String,
    pub note_root_hex: String,
    pub output_commitment_1_hex: String,
    pub output_commitment_2_hex: String,
    pub recipient_hex: String,
    pub merchant_amount: u64,
    pub protocol_fee: u64,
    pub execution_fee: u64,
    pub max_execution_fee: u64,
    pub quote_hash_hex: String,
    pub chain_id: u64,
    pub contract_address_hex: String,
    pub expiry: u64,
    pub has_change_1: bool,
    pub has_change_2: bool,
    pub proof_a_neg_hex: String,        // 128 bytes EVM format
    pub proof_b_hex: String,            // 256 bytes EVM format
    pub proof_c_hex: String,            // 128 bytes EVM format
    pub public_inputs_hex: Vec<String>, // 19 x 32 bytes EVM scalars (DEC-036A)
    #[serde(alias = "encrypted_memo_1_hex", default)]
    pub encrypted_memo_1: String, // 192 hex chars (96 bytes)
    #[serde(alias = "encrypted_memo_2_hex", default)]
    pub encrypted_memo_2: String, // 192 hex chars (96 bytes)
    #[serde(default)]
    pub leaf_count: u64,
    #[serde(default)]
    pub epoch_id_1: u32,
    #[serde(default)]
    pub epoch_id_2: u32,
    #[serde(default)]
    pub flags_packed: u8,
    #[serde(default)]
    pub is_rollover: u64,
}

/// Dual-Witness Assembly for JoinSplit Inputs (DEC-036B Phase 4)
#[derive(Clone, Debug)]
pub struct PrivateNoteWitness {
    pub value: u64,
    pub owner_key: Fr,
    pub rho: Fr,
    pub rand: Fr,
    pub nullifier_key: Fr,
    pub epoch_id: u32,
    pub leaf_index: u64,
    pub mountain_height: u8,
    pub mountain_siblings: [Fr; 32],
    pub is_dummy: bool,
    pub session_nonce: Fr,
}

impl PrivateNoteWitness {
    /// Synthesizes a Canonical Dummy Witness for single-input spends (B4.2).
    /// Is dummy = true, v = 0, epoch_id aligned with input 1.
    pub fn canonical_dummy(aligned_epoch: u32, owner_key: Fr, nullifier_key: Fr) -> Self {
        let mut rng = OsRng;
        let dummy_nonce = Fr::rand(&mut rng);
        Self {
            value: 0,
            owner_key,
            rho: Fr::from(0u64),
            rand: Fr::from(0u64),
            nullifier_key,
            epoch_id: aligned_epoch,
            leaf_index: 0,
            mountain_height: 0,
            mountain_siblings: [Fr::from(0u64); 32],
            is_dummy: true,
            session_nonce: dummy_nonce,
        }
    }

    /// Derives the appropriate nullifier using PRF domain separation (B4.3).
    pub fn compute_nullifier(&self) -> Fr {
        if self.is_dummy {
            derive_joinsplit_dummy_nullifier(self.nullifier_key, self.session_nonce)
        } else {
            derive_joinsplit_nullifier(self.nullifier_key, self.rho, self.epoch_id as u64)
        }
    }
}

/// Pre-flight gas and round estimate for In-Pool Progressive Consolidation (B2.4)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConsolidationEstimate {
    pub rounds_needed: usize,
    pub total_estimated_gas_fees: u64,
    pub initial_note_count: usize,
    pub estimated_final_note_count: usize,
    pub can_satisfy_target: bool,
}

/// Execution record of a single consolidation self-spend round (DEC-036B B2.1)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConsolidationRoundResult {
    pub round_index: usize,
    pub session_id: String,
    pub input_commitment_1: String,
    pub input_commitment_2: String,
    pub consolidated_output_commitment: String,
    pub input_1_value: u64,
    pub input_2_value: u64,
    pub output_value: u64,
    pub execution_fee_paid: u64,
    pub payload: JoinSplitSpendProofPayload,
}

/// Full summary returned after progressive in-pool consolidation completes (DEC-036B Phase 2)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConsolidationSummary {
    pub rounds_executed: usize,
    pub initial_note_count: usize,
    pub final_note_count: usize,
    pub total_relayer_fees_paid: u64,
    pub total_value_consolidated: u64,
    pub rounds: Vec<ConsolidationRoundResult>,
}

/// Errors occurring within the Private Note Wallet
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WalletError {
    InsufficientBalance {
        requested: u64,
        available: u64,
    },
    FragmentationLockout {
        requested: u64,
        available: u64,
        note_count: usize,
    },
    NoteNotFound(String),
    NoteNotSpendable(String),
    NoteAlreadyReserved(String),
    NoteMissingWitness(String),
    InvalidHex(String),
    CryptoError(String),
    SerializationError(String),
    BackupDecryptionFailed,
    NoteNotYetIndexed(u64),
    RelayerError(String),
    UnsupportedAssetId(u8),
    NonCanonicalRhoScalar,
    MixedAssetSelection,
    SpendAuthorityRequired,
    ComplianceError(String),
}

impl std::fmt::Display for WalletError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InsufficientBalance {
                requested,
                available,
            } => {
                write!(
                    f,
                    "Insufficient balance: requested {requested}, available {available}"
                )
            }
            Self::FragmentationLockout {
                requested,
                available,
                note_count,
            } => {
                write!(
                    f,
                    "Balance fragmentation lockout: requested {requested}, available {available} across {note_count} notes. Please run consolidate_notes() first"
                )
            }
            Self::NoteNotFound(id) => write!(f, "Note not found: {id}"),
            Self::NoteNotSpendable(id) => write!(f, "Note not spendable: {id}"),
            Self::NoteAlreadyReserved(id) => write!(f, "Note already reserved: {id}"),
            Self::NoteMissingWitness(id) => write!(f, "Note missing Merkle witness: {id}"),
            Self::InvalidHex(err) => write!(f, "Invalid hex data: {err}"),
            Self::CryptoError(err) => write!(f, "Cryptographic error: {err}"),
            Self::SerializationError(err) => write!(f, "Serialization error: {err}"),
            Self::BackupDecryptionFailed => write!(f, "Backup decryption failed or corrupted"),
            Self::NoteNotYetIndexed(idx) => {
                write!(f, "Note at leaf index {idx} is not yet indexed on relayer")
            }
            Self::RelayerError(err) => write!(f, "Relayer sync error: {err}"),
            Self::UnsupportedAssetId(id) => {
                write!(
                    f,
                    "Unsupported asset ID: {id}. Supported: 0 (WETH), 1 (USDC)"
                )
            }
            Self::NonCanonicalRhoScalar => write!(f, "Non-canonical rho scalar (>= r)"),
            Self::MixedAssetSelection => {
                write!(
                    f,
                    "Mixed-asset transaction rejected: notes must share the same asset_id"
                )
            }
            Self::SpendAuthorityRequired => {
                write!(
                    f,
                    "Spend authority required: Read-only auditor session cannot spend notes or generate spend proofs"
                )
            }
            Self::ComplianceError(err) => write!(f, "Compliance error: {err}"),
        }
    }
}

impl std::error::Error for WalletError {}

impl From<crate::compliance::ComplianceError> for WalletError {
    fn from(err: crate::compliance::ComplianceError) -> Self {
        WalletError::ComplianceError(err.to_string())
    }
}

/// MMR inclusion proof retrieved from Relayer REST sync endpoint (DEC-035C)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MmrProofResponse {
    pub leaf_index: u64,
    pub leaf_count: u64,
    pub commitment: String,
    pub mountain_height: usize,
    pub mountain_siblings: Vec<String>,
    pub peak_bagging_siblings: Vec<String>,
    pub bagged_root: String,
    pub block_number: u64,
}
