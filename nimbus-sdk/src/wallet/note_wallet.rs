//! Private Note Wallet (Gate E - DEC-024)
//!
//! Implements client-side ZK-UTXO wallet state, crash-safe Two-Phase Commit (2PC)
//! lifecycle, privacy-preserving coin selection, local Groth16 proving,
//! and encrypted state persistence.
//!
//! Academic Foundations & References:
//! - IACR ePrint 2026/2174: Chen et al., "Blind Spots in Blind Signatures: A System-Level Security Analysis of Deployed Chaumian Ecash" (Compositional state & recovery failure defense).
//! - IACR ePrint 2026/1621: Kiraz & Kardas, "Z-SCAPE: Zero-Knowledge Self-Custodial Credential Operation under Entropy-Source Failure" (Deterministic key derivation & isolation).
//! - IACR ePrint 2026/513: Zheng & Han, "zkBSA: Auditable and Compliant Stealth Addresses for Blockchains" (Stealth recipient transfers).
//! - ACM CCS 2024/2025: "Attacking Anonymity Set in Tornado Cash via Wallet Fingerprints" (Stochastic bucket coin selection).

use argon2::{Algorithm, Argon2, Params, Version};
use ark_bls12_381::{Bls12_381, Fr};
use ark_ff::{PrimeField, UniformRand};
use chacha20poly1305::{
    aead::{Aead, KeyInit},
    ChaCha20Poly1305, Nonce,
};
use nimbus_core::{
    compute_empty_hashes, derive_nullifier, derive_nullifier_key, domain_dummy_nullifier,
    fr_from_be_bytes, fr_to_be_bytes, generate_joinsplit_proof, generate_note_proof,
    native_poseidon_w5, note_commitment, to_evm_g1, to_evm_g2, JoinSplitCircuit, MERKLE_TREE_DEPTH,
};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

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

/// UTXO Note stored in client wallet
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WalletNote {
    pub commitment_hex: String,
    pub value: u64,
    pub owner_key_hex: String,
    pub rho_hex: String,
    pub randomness_hex: String,
    pub leaf_index: Option<u64>,
    pub leaf_count: Option<u64>,
    #[serde(default)]
    pub epoch_id: Option<u32>,
    pub merkle_path_hex: Option<Vec<String>>,
    pub status: NoteStatus,
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
}

/// Universal 2-in-2-out JoinSplit Spend Proof Payload generated locally by client SDK (DEC-030)
#[derive(Clone, Debug, Serialize, Deserialize)]
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
    pub public_inputs_hex: Vec<String>, // 14 x 32 bytes EVM scalars
}

/// Errors occurring within the Private Note Wallet
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WalletError {
    InsufficientBalance { requested: u64, available: u64 },
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
        }
    }
}

impl std::error::Error for WalletError {}

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

/// Private Note Wallet State
#[derive(Clone, Serialize, Deserialize)]
pub struct PrivateNoteWallet {
    spending_key_hex: String,
    nullifier_key_hex: String,
    pub notes: HashMap<String, WalletNote>,
    pub current_merkle_root_hex: Option<String>,
    #[serde(skip)]
    pub mmr: nimbus_core::MerkleMountainRange,
    #[serde(skip)]
    pub mmr_proof_cache: HashMap<(u64, String), MmrProofResponse>,
}

impl Drop for PrivateNoteWallet {
    fn drop(&mut self) {
        crate::secure_zeroize_string(&mut self.spending_key_hex);
        crate::secure_zeroize_string(&mut self.nullifier_key_hex);
    }
}

impl PrivateNoteWallet {
    /// Initialize a new wallet deterministically from a 32-byte master seed.
    ///
    /// Derives spending key and nullifier key using domain separation (DEC-024).
    pub fn new(seed: &[u8; 32]) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(b"nimbus.wallet.seed.v1");
        hasher.update(seed);
        let sk_bytes = hasher.finalize();

        let sk_fr = Fr::from_be_bytes_mod_order(&sk_bytes);
        let nk_fr = derive_nullifier_key(sk_fr);

        let sk_hex = hex::encode(fr_to_be_bytes(&sk_fr));
        let nk_hex = hex::encode(fr_to_be_bytes(&nk_fr));

        Self {
            spending_key_hex: sk_hex,
            nullifier_key_hex: nk_hex,
            notes: HashMap::new(),
            current_merkle_root_hex: None,
            mmr: nimbus_core::MerkleMountainRange::new(),
            mmr_proof_cache: HashMap::new(),
        }
    }

    /// Spending key as Fr
    pub fn spending_key(&self) -> Result<Fr, WalletError> {
        parse_fr_from_hex(&self.spending_key_hex)
    }

    /// Nullifier key as Fr
    pub fn nullifier_key(&self) -> Result<Fr, WalletError> {
        parse_fr_from_hex(&self.nullifier_key_hex)
    }

    /// Aggregate balance of all fully confirmed, spendable notes.
    pub fn balance(&self) -> u64 {
        self.notes
            .values()
            .filter(|n| n.status == NoteStatus::Unspent)
            .map(|n| n.value)
            .sum()
    }

    /// Balance of notes currently locked in active spend reservations.
    pub fn reserved_balance(&self, current_time_secs: u64) -> u64 {
        self.notes
            .values()
            .filter(|n| match &n.status {
                NoteStatus::Reserved {
                    lease_expiry_secs, ..
                } => *lease_expiry_secs > current_time_secs,
                _ => false,
            })
            .map(|n| n.value)
            .sum()
    }

    /// Total balance (unspent + currently reserved).
    pub fn total_balance(&self, current_time_secs: u64) -> u64 {
        self.balance() + self.reserved_balance(current_time_secs)
    }

    /// Creates a new note for an on-chain deposit.
    ///
    /// Returns the created `WalletNote` and its `commitment_hex` ready to be passed
    /// to `Nimbus::deposit(amount, commitment)`.
    pub fn create_deposit_note(
        &mut self,
        amount: u64,
        current_time_secs: u64,
    ) -> Result<(WalletNote, String), WalletError> {
        let mut rng = OsRng;
        let sk = self.spending_key()?;
        let rho = Fr::rand(&mut rng);
        let rand_val = Fr::rand(&mut rng);

        let cm = note_commitment(amount, sk, rho, rand_val);
        let cm_hex = hex::encode(fr_to_be_bytes(&cm));
        let sk_hex = hex::encode(fr_to_be_bytes(&sk));
        let rho_hex = hex::encode(fr_to_be_bytes(&rho));
        let rand_hex = hex::encode(fr_to_be_bytes(&rand_val));

        let note = WalletNote {
            commitment_hex: cm_hex.clone(),
            value: amount,
            owner_key_hex: sk_hex,
            rho_hex,
            randomness_hex: rand_hex,
            leaf_index: None,
            leaf_count: None,
            epoch_id: Some(0),
            merkle_path_hex: None,
            status: NoteStatus::Unconfirmed,
            created_at_secs: current_time_secs,
            session_id: None,
        };

        self.notes.insert(cm_hex.clone(), note.clone());
        Ok((note, cm_hex))
    }

    /// Creates an unconfirmed note for a deposit transaction with a specific epoch (DEC-033).
    pub fn create_deposit_note_with_epoch(
        &mut self,
        amount: u64,
        epoch_id: u32,
        current_time_secs: u64,
    ) -> Result<(WalletNote, String), WalletError> {
        let mut rng = OsRng;
        let sk = self.spending_key()?;
        let rho = Fr::rand(&mut rng);
        let rand_val = Fr::rand(&mut rng);

        let cm = note_commitment(amount, sk, rho, rand_val);
        let cm_hex = hex::encode(fr_to_be_bytes(&cm));
        let sk_hex = hex::encode(fr_to_be_bytes(&sk));
        let rho_hex = hex::encode(fr_to_be_bytes(&rho));
        let rand_hex = hex::encode(fr_to_be_bytes(&rand_val));

        let note = WalletNote {
            commitment_hex: cm_hex.clone(),
            value: amount,
            owner_key_hex: sk_hex,
            rho_hex,
            randomness_hex: rand_hex,
            leaf_index: None,
            leaf_count: None,
            epoch_id: Some(epoch_id),
            merkle_path_hex: None,
            status: NoteStatus::Unconfirmed,
            created_at_secs: current_time_secs,
            session_id: None,
        };

        self.notes.insert(cm_hex.clone(), note.clone());
        Ok((note, cm_hex))
    }

    /// Checks the epoch status of a note relative to current on-chain epoch (DEC-033).
    pub fn check_note_epoch_status(
        &self,
        note: &WalletNote,
        current_epoch: u32,
    ) -> NoteEpochStatus {
        note.check_epoch_status(current_epoch)
    }

    /// Confirms a deposit once mined on-chain, associating its Merkle leaf index and path.
    pub fn confirm_deposit(
        &mut self,
        commitment_hex: &str,
        leaf_index: u64,
        merkle_path: Vec<String>,
        root_hex: &str,
    ) -> Result<(), WalletError> {
        if merkle_path.len() != MERKLE_TREE_DEPTH {
            return Err(WalletError::CryptoError(format!(
                "Invalid Merkle path length: expected {}, got {}",
                MERKLE_TREE_DEPTH,
                merkle_path.len()
            )));
        }

        let cm_fr = parse_fr_from_hex(commitment_hex)?;

        let note = self
            .notes
            .get_mut(commitment_hex)
            .ok_or_else(|| WalletError::NoteNotFound(commitment_hex.to_string()))?;

        if self.mmr.leaf_count <= leaf_index as usize {
            while self.mmr.leaf_count < leaf_index as usize {
                self.mmr.append(Fr::from(0u64));
            }
            self.mmr.append(cm_fr);
        }

        note.leaf_index = Some(leaf_index);
        note.leaf_count = Some(self.mmr.leaf_count as u64);
        note.merkle_path_hex = Some(merkle_path);
        note.status = NoteStatus::Unspent;
        self.current_merkle_root_hex = Some(root_hex.to_string());

        Ok(())
    }

    /// Updates the Merkle witness and promotes an Unconfirmed or stale note to Unspent (DEC-030).
    ///
    /// Essential for asynchronous LeanIMT indexing (eliminates Ghost Notes) and local witness fast-forwarding.
    pub fn update_note_witness(
        &mut self,
        commitment_hex: &str,
        leaf_index: u64,
        merkle_path: Vec<String>,
        root_hex: &str,
    ) -> Result<(), WalletError> {
        if merkle_path.len() != MERKLE_TREE_DEPTH {
            return Err(WalletError::CryptoError(format!(
                "Invalid Merkle path length: expected {}, got {}",
                MERKLE_TREE_DEPTH,
                merkle_path.len()
            )));
        }

        if !self.notes.contains_key(commitment_hex) {
            return Err(WalletError::NoteNotFound(commitment_hex.to_string()));
        }

        let cm_fr = parse_fr_from_hex(commitment_hex)?;

        if self.mmr.leaf_count <= leaf_index as usize {
            while self.mmr.leaf_count < leaf_index as usize {
                self.mmr.append(Fr::from(0u64));
            }
            self.mmr.append(cm_fr);
        }

        let note = self
            .notes
            .get_mut(commitment_hex)
            .ok_or_else(|| WalletError::NoteNotFound(commitment_hex.to_string()))?;

        note.leaf_index = Some(leaf_index);
        note.leaf_count = Some(self.mmr.leaf_count as u64);
        note.merkle_path_hex = Some(merkle_path);
        note.status = NoteStatus::Unspent;
        self.current_merkle_root_hex = Some(root_hex.to_string());

        Ok(())
    }

    /// Privacy-Preserving Tiered Stochastic Knapsack Coin Selection (DEC-024 / DEC-030)
    ///
    /// 1. Branch 1: Prioritizes Exact Match (`has_change == 0`) to eliminate change note fingerprinting.
    /// 2. Branch 2: Single-Note Best Fit (Minimal Change).
    /// 3. Branch 3: 2-Note Knapsack Optimization (JoinSplit Execution) when no single note is large enough.
    ///    Minimizes change with a stochastic tie-breaking bucket (±5% tolerance) for privacy.
    /// 4. Reclaims expired reservations if lease has expired.
    pub fn select_note_for_spend(
        &self,
        merchant_amount: u64,
        protocol_fee: u64,
        execution_fee: u64,
        current_time_secs: u64,
    ) -> Result<SelectedSpend, WalletError> {
        let total_required = merchant_amount
            .checked_add(protocol_fee)
            .and_then(|v| v.checked_add(execution_fee))
            .ok_or_else(|| {
                WalletError::CryptoError("Total required spend amount overflow".into())
            })?;

        // Gather spendable notes
        let spendable_candidates: Vec<&WalletNote> = self
            .notes
            .values()
            .filter(|n| {
                if n.merkle_path_hex.is_none() || n.leaf_index.is_none() {
                    return false;
                }
                match &n.status {
                    NoteStatus::Unspent => true,
                    NoteStatus::Reserved {
                        lease_expiry_secs, ..
                    } => *lease_expiry_secs <= current_time_secs,
                    _ => false,
                }
            })
            .collect();

        let total_available: u64 = spendable_candidates.iter().map(|n| n.value).sum();
        if total_available < total_required {
            return Err(WalletError::InsufficientBalance {
                requested: total_required,
                available: total_available,
            });
        }

        // Branch 1: Exact Match Search (Zero-Change Priority - 1 Note)
        if let Some(exact_note) = spendable_candidates
            .iter()
            .find(|n| n.value == total_required)
        {
            return Ok(SelectedSpend {
                input_commitment_hex: exact_note.commitment_hex.clone(),
                input_value: exact_note.value,
                second_input_commitment_hex: None,
                second_input_value: 0,
                merchant_amount,
                protocol_fee,
                execution_fee,
                total_required,
                change_amount: 0,
                has_change: false,
            });
        }

        // Branch 2: Single-Note Best Fit (Smallest Note > total_required)
        let mut single_notes: Vec<&&WalletNote> = spendable_candidates
            .iter()
            .filter(|n| n.value > total_required)
            .collect();

        if !single_notes.is_empty() {
            single_notes.sort_by_key(|n| n.value);
            let selected_note = single_notes[0];
            let change_amount = selected_note.value - total_required;
            return Ok(SelectedSpend {
                input_commitment_hex: selected_note.commitment_hex.clone(),
                input_value: selected_note.value,
                second_input_commitment_hex: None,
                second_input_value: 0,
                merchant_amount,
                protocol_fee,
                execution_fee,
                total_required,
                change_amount,
                has_change: change_amount > 0,
            });
        }

        // Branch 3: 2-Note Knapsack Optimization (JoinSplit Execution - DEC-030)
        // No single note suffices, search pairs (n_i, n_j) where sum >= total_required
        let n = spendable_candidates.len();
        let mut pair_candidates: Vec<(usize, usize, u64)> = Vec::new(); // (i, j, excess)

        for i in 0..n {
            for j in (i + 1)..n {
                if let Some(sum) = spendable_candidates[i]
                    .value
                    .checked_add(spendable_candidates[j].value)
                {
                    if sum >= total_required {
                        pair_candidates.push((i, j, sum - total_required));
                    }
                }
            }
        }

        if !pair_candidates.is_empty() {
            let min_excess = pair_candidates.iter().map(|(_, _, ex)| *ex).min().unwrap();
            let tolerance = (min_excess as f64 * 0.05).ceil() as u64;
            let threshold = min_excess.saturating_add(tolerance);

            let pool: Vec<&(usize, usize, u64)> = pair_candidates
                .iter()
                .filter(|(_, _, ex)| *ex <= threshold)
                .collect();

            use rand::seq::SliceRandom;
            let mut rng = rand::thread_rng();
            let chosen = pool
                .choose(&mut rng)
                .copied()
                .unwrap_or(&pair_candidates[0]);

            let note_1 = spendable_candidates[chosen.0];
            let note_2 = spendable_candidates[chosen.1];
            let sum = note_1.value + note_2.value;
            let change_amount = sum - total_required;

            return Ok(SelectedSpend {
                input_commitment_hex: note_1.commitment_hex.clone(),
                input_value: note_1.value,
                second_input_commitment_hex: Some(note_2.commitment_hex.clone()),
                second_input_value: note_2.value,
                merchant_amount,
                protocol_fee,
                execution_fee,
                total_required,
                change_amount,
                has_change: change_amount > 0,
            });
        }

        // Branch 4: Insufficient Balance for 1-note or 2-note spend
        Err(WalletError::InsufficientBalance {
            requested: total_required,
            available: total_available,
        })
    }

    /// Acquires a lease on a note for an active spend session (Crash Safety Phase 1).
    pub fn reserve_note(
        &mut self,
        commitment_hex: &str,
        session_id: &str,
        current_time_secs: u64,
        ttl_secs: u64,
    ) -> Result<(), WalletError> {
        let note = self
            .notes
            .get_mut(commitment_hex)
            .ok_or_else(|| WalletError::NoteNotFound(commitment_hex.to_string()))?;

        match &note.status {
            NoteStatus::Unspent => {}
            NoteStatus::Reserved {
                lease_expiry_secs, ..
            } => {
                if *lease_expiry_secs > current_time_secs {
                    return Err(WalletError::NoteAlreadyReserved(commitment_hex.to_string()));
                }
            }
            _ => return Err(WalletError::NoteNotSpendable(commitment_hex.to_string())),
        }

        note.status = NoteStatus::Reserved {
            session_id: session_id.to_string(),
            lease_expiry_secs: current_time_secs + ttl_secs,
        };
        note.session_id = Some(session_id.to_string());

        Ok(())
    }

    /// Fetches MMR inclusion proof from Relayer REST sync endpoint (C4.1).
    pub async fn fetch_mmr_proof(
        &mut self,
        relayer_url: &str,
        leaf_index: u64,
    ) -> Result<MmrProofResponse, WalletError> {
        let url = format!(
            "{}/api/v1/mmr/proof/{}",
            relayer_url.trim_end_matches('/'),
            leaf_index
        );
        let client = reqwest::Client::new();
        let resp = client
            .get(&url)
            .send()
            .await
            .map_err(|e| WalletError::RelayerError(e.to_string()))?;

        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(WalletError::NoteNotYetIndexed(leaf_index));
        }

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(WalletError::RelayerError(format!("HTTP {status}: {body}")));
        }

        let proof_data: MmrProofResponse = resp
            .json()
            .await
            .map_err(|e| WalletError::RelayerError(e.to_string()))?;

        self.cache_mmr_proof(proof_data.clone());
        Ok(proof_data)
    }

    /// Queries the relayer for the inclusion proof of a confirmed note and caches it.
    pub async fn sync_note_mmr_proof(
        &mut self,
        relayer_url: &str,
        commitment_hex: &str,
    ) -> Result<MmrProofResponse, WalletError> {
        let note = self
            .notes
            .get(commitment_hex)
            .ok_or_else(|| WalletError::NoteNotFound(commitment_hex.to_string()))?;
        let leaf_index = note
            .leaf_index
            .ok_or_else(|| WalletError::NoteMissingWitness("leaf_index".into()))?;
        self.fetch_mmr_proof(relayer_url, leaf_index).await
    }

    /// Deserializes sibling hex strings from MmrProofResponse into Fr scalar arrays (C4.2).
    pub fn deserialize_mmr_siblings(
        proof_data: &MmrProofResponse,
    ) -> Result<([Fr; 32], [Fr; 32]), WalletError> {
        if proof_data.mountain_siblings.len() > 32 {
            return Err(WalletError::CryptoError(format!(
                "mountain_siblings length {} exceeds max 32",
                proof_data.mountain_siblings.len()
            )));
        }
        if proof_data.peak_bagging_siblings.len() > 32 {
            return Err(WalletError::CryptoError(format!(
                "peak_bagging_siblings length {} exceeds max 32",
                proof_data.peak_bagging_siblings.len()
            )));
        }

        let mut mountain_siblings = [Fr::from(0u64); 32];
        for (i, hex_str) in proof_data.mountain_siblings.iter().enumerate() {
            mountain_siblings[i] = parse_fr_from_hex(hex_str)?;
        }

        let mut peak_bagging_siblings = [Fr::from(0u64); 32];
        for (i, hex_str) in proof_data.peak_bagging_siblings.iter().enumerate() {
            peak_bagging_siblings[i] = parse_fr_from_hex(hex_str)?;
        }

        Ok((mountain_siblings, peak_bagging_siblings))
    }

    /// Caches an MMR inclusion proof locally (C4.4).
    pub fn cache_mmr_proof(&mut self, proof: MmrProofResponse) {
        self.mmr_proof_cache
            .insert((proof.leaf_index, proof.bagged_root.clone()), proof);
    }

    /// Retrieves a cached MMR inclusion proof matching leaf_index and bagged_root (C4.4).
    pub fn get_cached_mmr_proof(
        &self,
        leaf_index: u64,
        bagged_root: &str,
    ) -> Option<&MmrProofResponse> {
        let clean = bagged_root.strip_prefix("0x").unwrap_or(bagged_root);
        self.mmr_proof_cache
            .iter()
            .find(|((idx, root), _)| {
                *idx == leaf_index
                    && (root
                        .strip_prefix("0x")
                        .unwrap_or(root)
                        .eq_ignore_ascii_case(clean))
            })
            .map(|(_, v)| v)
    }

    /// Retrieves any cached MMR inclusion proof for the specified leaf_index (most recent or available).
    pub fn get_any_cached_mmr_proof_for_leaf(&self, leaf_index: u64) -> Option<&MmrProofResponse> {
        self.mmr_proof_cache
            .iter()
            .find(|((idx, _), _)| *idx == leaf_index)
            .map(|(_, v)| v)
    }

    /// Clears the local MMR proof cache.
    pub fn clear_mmr_proof_cache(&mut self) {
        self.mmr_proof_cache.clear();
    }

    /// Prepares a Groth16 spend proof locally using `nimbus-core` PrivateNoteCircuit.
    ///
    /// Prepares a Groth16 spend proof locally using `nimbus-core` PrivateNoteCircuit.
    ///
    /// NEVER transmits the private spending key or note witnesses to any relayer.
    #[allow(clippy::too_many_arguments)]
    pub fn prepare_spend_proof(
        &mut self,
        selected: &SelectedSpend,
        session_id: &str,
        recipient_evm_address: &str,
        max_execution_fee: u64,
        quote_hash_hex: &str,
        chain_id: u64,
        contract_address_hex: &str,
        expiry: u64,
        current_time_secs: u64,
        pk: &ark_groth16::ProvingKey<Bls12_381>,
    ) -> Result<SpendProofPayload, WalletError> {
        self.prepare_spend_proof_with_epoch(
            selected,
            session_id,
            recipient_evm_address,
            max_execution_fee,
            quote_hash_hex,
            chain_id,
            contract_address_hex,
            expiry,
            None,
            false,
            current_time_secs,
            pk,
        )
    }

    /// Prepares a Groth16 spend proof locally with optional target change epoch and rollover mode (DEC-033).
    #[allow(clippy::too_many_arguments)]
    pub fn prepare_spend_proof_with_epoch(
        &mut self,
        selected: &SelectedSpend,
        session_id: &str,
        recipient_evm_address: &str,
        max_execution_fee: u64,
        quote_hash_hex: &str,
        chain_id: u64,
        contract_address_hex: &str,
        expiry: u64,
        target_change_epoch: Option<u32>,
        is_rollover: bool,
        current_time_secs: u64,
        pk: &ark_groth16::ProvingKey<Bls12_381>,
    ) -> Result<SpendProofPayload, WalletError> {
        if selected.is_joinsplit() {
            return Err(WalletError::CryptoError(
                "Selected spend requires 2-note JoinSplit; please use prepare_joinsplit_spend_proof or pay_joinsplit".into(),
            ));
        }

        // Reserve the input note
        self.reserve_note(
            &selected.input_commitment_hex,
            session_id,
            current_time_secs,
            DEFAULT_RESERVATION_TTL_SECS,
        )?;

        let input_note = self
            .notes
            .get(&selected.input_commitment_hex)
            .ok_or_else(|| WalletError::NoteNotFound(selected.input_commitment_hex.clone()))?
            .clone();

        let leaf_index = input_note
            .leaf_index
            .ok_or_else(|| WalletError::NoteMissingWitness("leaf_index".into()))?;
        let merkle_path_strings = input_note
            .merkle_path_hex
            .as_ref()
            .ok_or_else(|| WalletError::NoteMissingWitness("merkle_path".into()))?;

        if merkle_path_strings.len() != MERKLE_TREE_DEPTH {
            return Err(WalletError::CryptoError("Invalid Merkle path depth".into()));
        }

        let sk = self.spending_key()?;
        let in_rho = parse_fr_from_hex(&input_note.rho_hex)?;
        let in_rand = parse_fr_from_hex(&input_note.randomness_hex)?;

        let in_cm = note_commitment(input_note.value, sk, in_rho, in_rand);
        let nk = self.nullifier_key()?;
        let note_epoch_id = input_note.epoch_id.unwrap_or(0);
        let nullifier = derive_nullifier(nk, in_cm, leaf_index, note_epoch_id);

        // Synchronize local MMR if needed, or use cached canonical MMR proof (C4.3)
        let (
            note_root,
            total_leaves,
            mountain_height,
            mountain_siblings,
            peak_bagging_siblings,
            peak_bagging_count,
        ) = if let Some(cached) = self.get_any_cached_mmr_proof_for_leaf(leaf_index) {
            let (m_sibs, p_sibs) = Self::deserialize_mmr_siblings(cached)?;
            let root = parse_fr_from_hex(&cached.bagged_root)?;
            (
                root,
                cached.leaf_count,
                cached.mountain_height as u8,
                m_sibs,
                p_sibs,
                cached.peak_bagging_siblings.len() as u8,
            )
        } else {
            if self.mmr.leaf_count <= leaf_index as usize {
                while self.mmr.leaf_count < leaf_index as usize {
                    self.mmr.append(Fr::from(0u64));
                }
                self.mmr.append(in_cm);
            }

            let mmr_proof = self.mmr.generate_proof(leaf_index as usize);
            let note_root = self.mmr.get_root();
            let total_leaves = self.mmr.leaf_count as u64;

            let mut mountain_siblings = [Fr::from(0u64); 32];
            for (i, &s) in mmr_proof.mountain_siblings.iter().take(32).enumerate() {
                mountain_siblings[i] = s;
            }

            let mut peak_bagging_siblings = [Fr::from(0u64); 32];
            for (i, &s) in mmr_proof.peak_bagging_siblings.iter().take(32).enumerate() {
                peak_bagging_siblings[i] = s;
            }

            (
                note_root,
                total_leaves,
                mmr_proof.mountain_height as u8,
                mountain_siblings,
                peak_bagging_siblings,
                mmr_proof.peak_bagging_siblings.len() as u8,
            )
        };

        // Handle Change Note
        let (ch_val, ch_cm, ch_rho, ch_rand) = if selected.has_change {
            let mut rng = OsRng;
            let c_rho = Fr::rand(&mut rng);
            let c_rand = Fr::rand(&mut rng);
            let c_cm = note_commitment(selected.change_amount, sk, c_rho, c_rand);

            let c_cm_hex = hex::encode(fr_to_be_bytes(&c_cm));
            let c_rho_hex = hex::encode(fr_to_be_bytes(&c_rho));
            let c_rand_hex = hex::encode(fr_to_be_bytes(&c_rand));
            let sk_hex = hex::encode(fr_to_be_bytes(&sk));

            let change_epoch = target_change_epoch.or(input_note.epoch_id).or(Some(0));

            // Persist unconfirmed change note (2PC Phase 1)
            let change_note = WalletNote {
                commitment_hex: c_cm_hex,
                value: selected.change_amount,
                owner_key_hex: sk_hex,
                rho_hex: c_rho_hex,
                randomness_hex: c_rand_hex,
                leaf_index: None,
                leaf_count: None,
                epoch_id: change_epoch,
                merkle_path_hex: None,
                status: NoteStatus::Unconfirmed,
                created_at_secs: current_time_secs,
                session_id: Some(session_id.to_string()),
            };
            self.notes
                .insert(change_note.commitment_hex.clone(), change_note);

            (Fr::from(selected.change_amount), c_cm, c_rho, c_rand)
        } else {
            (
                Fr::from(0u64),
                Fr::from(0u64),
                Fr::from(0u64),
                Fr::from(0u64),
            )
        };

        // Parse EVM public input scalars
        let recipient_bytes = parse_bytes32_from_hex(recipient_evm_address, true)?;
        let recipient_fr = Fr::from_be_bytes_mod_order(&recipient_bytes);

        let quote_bytes = parse_bytes32_from_hex(quote_hash_hex, false)?;
        let (quote_hi_fr, quote_lo_fr) = nimbus_core::split_quote_hash_to_limbs(&quote_bytes);

        let contract_bytes = parse_bytes32_from_hex(contract_address_hex, true)?;
        let contract_fr = Fr::from_be_bytes_mod_order(&contract_bytes);

        let has_change_fr = if selected.has_change {
            Fr::from(1u64)
        } else {
            Fr::from(0u64)
        };

        let is_rollover_fr = if is_rollover {
            Fr::from(1u64)
        } else {
            Fr::from(0u64)
        };
        let note_epoch_id_fr = Fr::from(note_epoch_id as u64);

        // Compute Scope Binding Commitment (DEC-035B Stage 1 & Stage 2)
        let scope_hash = nimbus_core::compute_scope_hash(
            recipient_fr,
            Fr::from(chain_id),
            contract_fr,
            Fr::from(expiry),
        );
        let expected_binding =
            nimbus_core::compute_binding_commitment(quote_hi_fr, quote_lo_fr, scope_hash);

        // Construct PrivateNoteCircuit (DEC-032, DEC-033, DEC-035A, DEC-035B 16 Public Inputs)
        let circuit = nimbus_core::PrivateNoteCircuit {
            note_root: Some(note_root),
            leaf_count: Some(Fr::from(total_leaves)),
            input_nullifier: Some(nullifier),
            note_epoch_id: Some(note_epoch_id_fr),
            output_commitment: Some(ch_cm),
            recipient: Some(recipient_fr),
            merchant_amount: Some(Fr::from(selected.merchant_amount)),
            protocol_fee: Some(Fr::from(selected.protocol_fee)),
            execution_fee: Some(Fr::from(selected.execution_fee)),
            quote_hash_hi: Some(quote_hi_fr),
            quote_hash_lo: Some(quote_lo_fr),
            chain_id: Some(Fr::from(chain_id)),
            contract_address: Some(contract_fr),
            expiry: Some(Fr::from(expiry)),
            has_change: Some(has_change_fr),
            is_rollover: Some(is_rollover_fr),

            input_value: Some(Fr::from(input_note.value)),
            input_owner_key: Some(sk),
            input_rho: Some(in_rho),
            input_randomness: Some(in_rand),
            input_leaf_index: Some(Fr::from(leaf_index)),
            mountain_height: Some(mountain_height),
            mountain_siblings: Some(mountain_siblings),
            peak_bagging_siblings: Some(peak_bagging_siblings),
            peak_bagging_count: Some(peak_bagging_count),

            change_value: Some(ch_val),
            change_owner_key: Some(sk),
            change_rho: Some(ch_rho),
            change_randomness: Some(ch_rand),
            expected_binding: Some(expected_binding),
        };

        // Generate Groth16 Proof
        let proof = generate_note_proof(circuit, pk)
            .map_err(|e| WalletError::CryptoError(format!("Proof generation failed: {e:?}")))?;

        // Format EVM Proof Points
        let proof_a_neg = to_evm_g1(&-proof.a);
        let proof_b = to_evm_g2(&proof.b);
        let proof_c = to_evm_g1(&proof.c);

        // Format 16 Public Inputs (DEC-035A)
        let public_inputs = vec![
            hex::encode(fr_to_be_bytes(&note_root)),
            hex::encode(fr_to_be_bytes(&Fr::from(total_leaves))),
            hex::encode(fr_to_be_bytes(&nullifier)),
            hex::encode(fr_to_be_bytes(&note_epoch_id_fr)),
            hex::encode(fr_to_be_bytes(&ch_cm)),
            hex::encode(recipient_bytes),
            hex::encode(fr_to_be_bytes(&Fr::from(selected.merchant_amount))),
            hex::encode(fr_to_be_bytes(&Fr::from(selected.protocol_fee))),
            hex::encode(fr_to_be_bytes(&Fr::from(selected.execution_fee))),
            hex::encode(fr_to_be_bytes(&quote_hi_fr)),
            hex::encode(fr_to_be_bytes(&quote_lo_fr)),
            hex::encode(fr_to_be_bytes(&Fr::from(chain_id))),
            hex::encode(contract_bytes),
            hex::encode(fr_to_be_bytes(&Fr::from(expiry))),
            hex::encode(fr_to_be_bytes(&has_change_fr)),
            hex::encode(fr_to_be_bytes(&is_rollover_fr)),
        ];

        Ok(SpendProofPayload {
            session_id: session_id.to_string(),
            input_commitment_hex: selected.input_commitment_hex.clone(),
            input_nullifier_hex: hex::encode(fr_to_be_bytes(&nullifier)),
            note_root_hex: hex::encode(fr_to_be_bytes(&note_root)),
            leaf_count: total_leaves,
            note_epoch_id,
            output_commitment_hex: hex::encode(fr_to_be_bytes(&ch_cm)),
            recipient_hex: recipient_evm_address.to_string(),
            merchant_amount: selected.merchant_amount,
            protocol_fee: selected.protocol_fee,
            execution_fee: selected.execution_fee,
            max_execution_fee,
            quote_hash_hex: quote_hash_hex.to_string(),
            chain_id,
            contract_address_hex: contract_address_hex.to_string(),
            expiry,
            has_change: selected.has_change,
            is_rollover,
            proof_a_neg_hex: hex::encode(proof_a_neg),
            proof_b_hex: hex::encode(proof_b),
            proof_c_hex: hex::encode(proof_c),
            public_inputs_hex: public_inputs,
        })
    }

    /// Prepares spend proof using an explicit MmrProofResponse (C4.3).
    #[allow(clippy::too_many_arguments)]
    pub fn prepare_spend_proof_with_mmr(
        &mut self,
        selected: &SelectedSpend,
        session_id: &str,
        recipient_evm_address: &str,
        max_execution_fee: u64,
        quote_hash_hex: &str,
        chain_id: u64,
        contract_address_hex: &str,
        expiry: u64,
        target_change_epoch: Option<u32>,
        is_rollover: bool,
        mmr_proof: &MmrProofResponse,
        current_time_secs: u64,
        pk: &ark_groth16::ProvingKey<Bls12_381>,
    ) -> Result<SpendProofPayload, WalletError> {
        self.cache_mmr_proof(mmr_proof.clone());
        self.prepare_spend_proof_with_epoch(
            selected,
            session_id,
            recipient_evm_address,
            max_execution_fee,
            quote_hash_hex,
            chain_id,
            contract_address_hex,
            expiry,
            target_change_epoch,
            is_rollover,
            current_time_secs,
            pk,
        )
    }

    /// Builds an in-flight spend rollover proof (DEC-033).
    /// Spends a note from an older epoch (E-1 or earlier) while refreshing the change note into `target_epoch` (E).
    #[allow(clippy::too_many_arguments)]
    pub fn build_in_flight_spend_rollover(
        &mut self,
        recipient_evm_address: &str,
        merchant_amount: u64,
        protocol_fee: u64,
        execution_fee: u64,
        max_execution_fee: u64,
        quote_hash_hex: &str,
        chain_id: u64,
        contract_address_hex: &str,
        expiry: u64,
        target_epoch: u32,
        current_time_secs: u64,
        pk: &ark_groth16::ProvingKey<Bls12_381>,
    ) -> Result<SpendProofPayload, WalletError> {
        let selected = self.select_note_for_spend(
            merchant_amount,
            protocol_fee,
            execution_fee,
            current_time_secs,
        )?;

        let session_id = format!(
            "rollover_spend_{}_{}",
            current_time_secs,
            hex::encode(rand::random::<[u8; 8]>())
        );

        self.prepare_spend_proof_with_epoch(
            &selected,
            &session_id,
            recipient_evm_address,
            max_execution_fee,
            quote_hash_hex,
            chain_id,
            contract_address_hex,
            expiry,
            Some(target_epoch),
            false,
            current_time_secs,
            pk,
        )
    }

    /// Builds a standalone note refresh proof (DEC-033).
    /// Refreshes a note into `target_epoch` without paying a merchant (merchant_amount = 0, protocol_fee = 0).
    /// The note must satisfy `input_value >= MIN_STANDALONE_ROLLOVER_THRESHOLD_USDC` ($2.00 USDC).
    /// Execution fee (gas reimbursement + 15% relayer markup) is deducted from the note value.
    #[allow(clippy::too_many_arguments)]
    pub fn build_standalone_refresh(
        &mut self,
        input_commitment_hex: &str,
        execution_fee: u64,
        max_execution_fee: u64,
        quote_hash_hex: &str,
        chain_id: u64,
        contract_address_hex: &str,
        expiry: u64,
        target_epoch: u32,
        current_time_secs: u64,
        pk: &ark_groth16::ProvingKey<Bls12_381>,
    ) -> Result<SpendProofPayload, WalletError> {
        let input_note = self
            .notes
            .get(input_commitment_hex)
            .ok_or_else(|| WalletError::NoteNotFound(input_commitment_hex.to_string()))?
            .clone();

        // Check dust threshold (DEC-033: MIN_STANDALONE_ROLLOVER_THRESHOLD_USDC = 2_000_000)
        if input_note.value < nimbus_core::fees::MIN_STANDALONE_ROLLOVER_THRESHOLD_USDC {
            return Err(WalletError::CryptoError(
                "DUST_NOTE_ROLLOVER_REJECTED: Note value is below $2.00 USDC minimum threshold"
                    .into(),
            ));
        }

        // Check sufficient balance for gas
        if input_note.value < execution_fee {
            return Err(WalletError::InsufficientBalance {
                requested: execution_fee,
                available: input_note.value,
            });
        }

        let change_amount = input_note.value - execution_fee;
        let selected = SelectedSpend {
            input_commitment_hex: input_commitment_hex.to_string(),
            input_value: input_note.value,
            second_input_commitment_hex: None,
            second_input_value: 0,
            merchant_amount: 0,
            protocol_fee: 0,
            execution_fee,
            total_required: execution_fee,
            change_amount,
            has_change: true,
        };

        let session_id = format!(
            "refresh_{}_{}",
            current_time_secs,
            hex::encode(rand::random::<[u8; 8]>())
        );

        let zero_address = "0x0000000000000000000000000000000000000000";

        self.prepare_spend_proof_with_epoch(
            &selected,
            &session_id,
            zero_address,
            max_execution_fee,
            quote_hash_hex,
            chain_id,
            contract_address_hex,
            expiry,
            Some(target_epoch),
            true, // is_rollover: true
            current_time_secs,
            pk,
        )
    }

    /// High-level client API: executes coin selection and generates spend proof payload.
    #[allow(clippy::too_many_arguments)]
    pub fn pay(
        &mut self,
        recipient_evm_address: &str,
        merchant_amount: u64,
        protocol_fee: u64,
        execution_fee: u64,
        max_execution_fee: u64,
        quote_hash_hex: &str,
        chain_id: u64,
        contract_address_hex: &str,
        expiry: u64,
        current_time_secs: u64,
        pk: &ark_groth16::ProvingKey<Bls12_381>,
    ) -> Result<SpendProofPayload, WalletError> {
        let selected = self.select_note_for_spend(
            merchant_amount,
            protocol_fee,
            execution_fee,
            current_time_secs,
        )?;

        let session_id = format!(
            "spend_{}_{}",
            current_time_secs,
            hex::encode(rand::random::<[u8; 8]>())
        );

        self.prepare_spend_proof(
            &selected,
            &session_id,
            recipient_evm_address,
            max_execution_fee,
            quote_hash_hex,
            chain_id,
            contract_address_hex,
            expiry,
            current_time_secs,
            pk,
        )
    }

    /// Prepares a Universal 2-in-2-out JoinSplit Groth16 proof locally using `nimbus-core` JoinSplitCircuit (DEC-030).
    ///
    /// Supports both 1-note spend (using Canonical Dummy Zero-Note as input 2) and 2-note JoinSplit.
    /// NEVER transmits the private spending key or note witnesses to any relayer.
    #[allow(clippy::too_many_arguments)]
    pub fn prepare_joinsplit_spend_proof(
        &mut self,
        selected: &SelectedSpend,
        session_id: &str,
        recipient_evm_address: &str,
        max_execution_fee: u64,
        quote_hash_hex: &str,
        chain_id: u64,
        contract_address_hex: &str,
        expiry: u64,
        current_time_secs: u64,
        joinsplit_pk: &ark_groth16::ProvingKey<Bls12_381>,
    ) -> Result<JoinSplitSpendProofPayload, WalletError> {
        // 1. Reserve Note 1
        self.reserve_note(
            &selected.input_commitment_hex,
            session_id,
            current_time_secs,
            DEFAULT_RESERVATION_TTL_SECS,
        )?;

        // 2. Reserve Note 2 if 2-note JoinSplit
        if let Some(ref cm2_hex) = selected.second_input_commitment_hex {
            if let Err(e) = self.reserve_note(
                cm2_hex,
                session_id,
                current_time_secs,
                DEFAULT_RESERVATION_TTL_SECS,
            ) {
                // Rollback note 1 on failure
                let _ = self.rollback_spend(session_id);
                return Err(e);
            }
        }

        // 3. Load witnesses for Input Note 1
        let input_note_1 = self
            .notes
            .get(&selected.input_commitment_hex)
            .ok_or_else(|| WalletError::NoteNotFound(selected.input_commitment_hex.clone()))?
            .clone();

        let leaf_index_1 = input_note_1
            .leaf_index
            .ok_or_else(|| WalletError::NoteMissingWitness("input 1 leaf_index".into()))?;
        let merkle_path_1_strings = input_note_1
            .merkle_path_hex
            .as_ref()
            .ok_or_else(|| WalletError::NoteMissingWitness("input 1 merkle_path".into()))?;

        if merkle_path_1_strings.len() != MERKLE_TREE_DEPTH {
            return Err(WalletError::CryptoError(
                "Invalid Merkle path depth for input 1".into(),
            ));
        }

        let mut merkle_path_1 = [Fr::from(0u64); MERKLE_TREE_DEPTH];
        for (i, p_hex) in merkle_path_1_strings.iter().enumerate() {
            merkle_path_1[i] = parse_fr_from_hex(p_hex)?;
        }

        let sk = self.spending_key()?;
        let in1_rho = parse_fr_from_hex(&input_note_1.rho_hex)?;
        let in1_rand = parse_fr_from_hex(&input_note_1.randomness_hex)?;

        let in1_cm = note_commitment(input_note_1.value, sk, in1_rho, in1_rand);
        let nk = self.nullifier_key()?;
        let nullifier_1 =
            derive_nullifier(nk, in1_cm, leaf_index_1, input_note_1.epoch_id.unwrap_or(0));
        let note_root = nimbus_core::compute_merkle_root(in1_cm, leaf_index_1, &merkle_path_1);

        // 4. Load witnesses or generate dummy for Input Note 2
        let (in2_val, in2_rho, in2_rand, in2_index, in2_path, in2_is_dummy, in2_nonce, nullifier_2) =
            if let Some(ref cm2_hex) = selected.second_input_commitment_hex {
                let input_note_2 = self
                    .notes
                    .get(cm2_hex)
                    .ok_or_else(|| WalletError::NoteNotFound(cm2_hex.clone()))?
                    .clone();

                let leaf_index_2 = input_note_2
                    .leaf_index
                    .ok_or_else(|| WalletError::NoteMissingWitness("input 2 leaf_index".into()))?;
                let merkle_path_2_strings = input_note_2
                    .merkle_path_hex
                    .as_ref()
                    .ok_or_else(|| WalletError::NoteMissingWitness("input 2 merkle_path".into()))?;

                if merkle_path_2_strings.len() != MERKLE_TREE_DEPTH {
                    return Err(WalletError::CryptoError(
                        "Invalid Merkle path depth for input 2".into(),
                    ));
                }

                let mut merkle_path_2 = [Fr::from(0u64); MERKLE_TREE_DEPTH];
                for (i, p_hex) in merkle_path_2_strings.iter().enumerate() {
                    merkle_path_2[i] = parse_fr_from_hex(p_hex)?;
                }

                let rho_2 = parse_fr_from_hex(&input_note_2.rho_hex)?;
                let rand_2 = parse_fr_from_hex(&input_note_2.randomness_hex)?;
                let cm_2 = note_commitment(input_note_2.value, sk, rho_2, rand_2);
                let nf_2 =
                    derive_nullifier(nk, cm_2, leaf_index_2, input_note_2.epoch_id.unwrap_or(0));

                let root_2 = nimbus_core::compute_merkle_root(cm_2, leaf_index_2, &merkle_path_2);
                if root_2 != note_root {
                    return Err(WalletError::CryptoError(format!(
                    "Divergent Merkle roots between input 1 ({}) and input 2 ({}); please synchronize note witnesses",
                    hex::encode(fr_to_be_bytes(&note_root)),
                    hex::encode(fr_to_be_bytes(&root_2))
                )));
                }

                (
                    Fr::from(input_note_2.value),
                    rho_2,
                    rand_2,
                    Fr::from(leaf_index_2),
                    merkle_path_2,
                    Fr::from(0u64), // not dummy
                    Fr::from(0u64),
                    nf_2,
                )
            } else {
                // Canonical Dummy Zero-Note (DEC-030 Seksi 3.B)
                let empty_hashes = compute_empty_hashes();
                let mut empty_path = [Fr::from(0u64); MERKLE_TREE_DEPTH];
                empty_path.copy_from_slice(&empty_hashes[..MERKLE_TREE_DEPTH]);

                let mut rng = OsRng;
                let dummy_nonce = Fr::rand(&mut rng);
                let dummy_nf = native_poseidon_w5(
                    &[nk, dummy_nonce, Fr::from(0u64), Fr::from(0u64)],
                    domain_dummy_nullifier(),
                );

                (
                    Fr::from(0u64),
                    Fr::from(0u64),
                    Fr::from(0u64),
                    Fr::from(0u64),
                    empty_path,
                    Fr::from(1u64), // is dummy
                    dummy_nonce,
                    dummy_nf,
                )
            };

        // 5. Change Note 1 (Output 1)
        let (ch1_val, ch1_cm, ch1_rho, ch1_rand) = if selected.has_change {
            let mut rng = OsRng;
            let c_rho = Fr::rand(&mut rng);
            let c_rand = Fr::rand(&mut rng);
            let c_cm = note_commitment(selected.change_amount, sk, c_rho, c_rand);

            let c_cm_hex = hex::encode(fr_to_be_bytes(&c_cm));
            let c_rho_hex = hex::encode(fr_to_be_bytes(&c_rho));
            let c_rand_hex = hex::encode(fr_to_be_bytes(&c_rand));
            let sk_hex = hex::encode(fr_to_be_bytes(&sk));

            // Persist unconfirmed change note (2PC Phase 1)
            let change_note = WalletNote {
                commitment_hex: c_cm_hex,
                value: selected.change_amount,
                owner_key_hex: sk_hex,
                rho_hex: c_rho_hex,
                randomness_hex: c_rand_hex,
                leaf_index: None,
                leaf_count: None,
                epoch_id: input_note_1.epoch_id.or(Some(0)),
                merkle_path_hex: None,
                status: NoteStatus::Unconfirmed,
                created_at_secs: current_time_secs,
                session_id: Some(session_id.to_string()),
            };
            self.notes
                .insert(change_note.commitment_hex.clone(), change_note);

            (Fr::from(selected.change_amount), c_cm, c_rho, c_rand)
        } else {
            (
                Fr::from(0u64),
                Fr::from(0u64),
                Fr::from(0u64),
                Fr::from(0u64),
            )
        };

        // Output 2 is zero in standard payment/consolidation
        let ch2_val = Fr::from(0u64);
        let ch2_cm = Fr::from(0u64);
        let ch2_rho = Fr::from(0u64);
        let ch2_rand = Fr::from(0u64);

        // 6. EVM parameters & domain scope
        let recipient_bytes = parse_bytes32_from_hex(recipient_evm_address, true)?;
        let recipient_fr = Fr::from_be_bytes_mod_order(&recipient_bytes);

        let quote_bytes = parse_bytes32_from_hex(quote_hash_hex, false)?;
        let quote_fr = Fr::from_be_bytes_mod_order(&quote_bytes);

        let contract_bytes = parse_bytes32_from_hex(contract_address_hex, true)?;
        let contract_fr = Fr::from_be_bytes_mod_order(&contract_bytes);

        let has_change_1_fr = if selected.has_change {
            Fr::from(1u64)
        } else {
            Fr::from(0u64)
        };
        let has_change_2_fr = Fr::from(0u64);
        let flags_packed_fr = has_change_1_fr; // bit 0 = has_change_1, bit 1 = 0

        // 7. Construct JoinSplitCircuit
        let circuit = JoinSplitCircuit {
            note_root: Some(note_root),
            input_nullifier_1: Some(nullifier_1),
            input_nullifier_2: Some(nullifier_2),
            output_commitment_1: Some(ch1_cm),
            output_commitment_2: Some(ch2_cm),
            recipient: Some(recipient_fr),
            merchant_amount: Some(Fr::from(selected.merchant_amount)),
            protocol_fee: Some(Fr::from(selected.protocol_fee)),
            execution_fee: Some(Fr::from(selected.execution_fee)),
            quote_hash: Some(quote_fr),
            chain_id: Some(Fr::from(chain_id)),
            contract_address: Some(contract_fr),
            expiry: Some(Fr::from(expiry)),
            flags_packed: Some(flags_packed_fr),

            in1_value: Some(Fr::from(input_note_1.value)),
            in1_owner_key: Some(sk),
            in1_rho: Some(in1_rho),
            in1_randomness: Some(in1_rand),
            in1_leaf_index: Some(Fr::from(leaf_index_1)),
            in1_merkle_path: Some(merkle_path_1),

            in2_value: Some(in2_val),
            in2_owner_key: Some(sk),
            in2_rho: Some(in2_rho),
            in2_randomness: Some(in2_rand),
            in2_leaf_index: Some(in2_index),
            in2_merkle_path: Some(in2_path),
            in2_is_dummy: Some(in2_is_dummy),
            in2_session_nonce: Some(in2_nonce),

            out1_value: Some(ch1_val),
            out1_owner_key: Some(sk),
            out1_rho: Some(ch1_rho),
            out1_randomness: Some(ch1_rand),

            out2_value: Some(ch2_val),
            out2_owner_key: Some(sk),
            out2_rho: Some(ch2_rho),
            out2_randomness: Some(ch2_rand),

            has_change_1: Some(has_change_1_fr),
            has_change_2: Some(has_change_2_fr),
            expected_binding: None,
        };

        // 8. Generate Groth16 Proof
        let proof = generate_joinsplit_proof(circuit, joinsplit_pk).map_err(|e| {
            WalletError::CryptoError(format!("JoinSplit proof generation failed: {e:?}"))
        })?;

        // 9. Format EVM Proof Points
        let proof_a_neg = to_evm_g1(&-proof.a);
        let proof_b = to_evm_g2(&proof.b);
        let proof_c = to_evm_g1(&proof.c);

        // 10. Format 14 Public Inputs
        let public_inputs = vec![
            hex::encode(fr_to_be_bytes(&note_root)),
            hex::encode(fr_to_be_bytes(&nullifier_1)),
            hex::encode(fr_to_be_bytes(&nullifier_2)),
            hex::encode(fr_to_be_bytes(&ch1_cm)),
            hex::encode(fr_to_be_bytes(&ch2_cm)),
            hex::encode(recipient_bytes),
            hex::encode(fr_to_be_bytes(&Fr::from(selected.merchant_amount))),
            hex::encode(fr_to_be_bytes(&Fr::from(selected.protocol_fee))),
            hex::encode(fr_to_be_bytes(&Fr::from(selected.execution_fee))),
            hex::encode(quote_bytes),
            hex::encode(fr_to_be_bytes(&Fr::from(chain_id))),
            hex::encode(contract_bytes),
            hex::encode(fr_to_be_bytes(&Fr::from(expiry))),
            hex::encode(fr_to_be_bytes(&flags_packed_fr)),
        ];

        Ok(JoinSplitSpendProofPayload {
            session_id: session_id.to_string(),
            input_commitment_1_hex: selected.input_commitment_hex.clone(),
            input_commitment_2_hex: selected.second_input_commitment_hex.clone(),
            input_nullifier_1_hex: hex::encode(fr_to_be_bytes(&nullifier_1)),
            input_nullifier_2_hex: hex::encode(fr_to_be_bytes(&nullifier_2)),
            note_root_hex: hex::encode(fr_to_be_bytes(&note_root)),
            output_commitment_1_hex: hex::encode(fr_to_be_bytes(&ch1_cm)),
            output_commitment_2_hex: hex::encode(fr_to_be_bytes(&ch2_cm)),
            recipient_hex: recipient_evm_address.to_string(),
            merchant_amount: selected.merchant_amount,
            protocol_fee: selected.protocol_fee,
            execution_fee: selected.execution_fee,
            max_execution_fee,
            quote_hash_hex: quote_hash_hex.to_string(),
            chain_id,
            contract_address_hex: contract_address_hex.to_string(),
            expiry,
            has_change_1: selected.has_change,
            has_change_2: false,
            proof_a_neg_hex: hex::encode(proof_a_neg),
            proof_b_hex: hex::encode(proof_b),
            proof_c_hex: hex::encode(proof_c),
            public_inputs_hex: public_inputs,
        })
    }

    /// High-level client API: executes coin selection and generates a JoinSplit spend proof payload (DEC-030).
    #[allow(clippy::too_many_arguments)]
    pub fn pay_joinsplit(
        &mut self,
        recipient_evm_address: &str,
        merchant_amount: u64,
        protocol_fee: u64,
        execution_fee: u64,
        max_execution_fee: u64,
        quote_hash_hex: &str,
        chain_id: u64,
        contract_address_hex: &str,
        expiry: u64,
        current_time_secs: u64,
        joinsplit_pk: &ark_groth16::ProvingKey<Bls12_381>,
    ) -> Result<JoinSplitSpendProofPayload, WalletError> {
        let selected = self.select_note_for_spend(
            merchant_amount,
            protocol_fee,
            execution_fee,
            current_time_secs,
        )?;

        let session_id = format!(
            "joinsplit_{}_{}",
            current_time_secs,
            hex::encode(rand::random::<[u8; 8]>())
        );

        self.prepare_joinsplit_spend_proof(
            &selected,
            &session_id,
            recipient_evm_address,
            max_execution_fee,
            quote_hash_hex,
            chain_id,
            contract_address_hex,
            expiry,
            current_time_secs,
            joinsplit_pk,
        )
    }

    /// In-Pool Autonomous Consolidation (DEC-030 Branch 4).
    ///
    /// Combines two fragmented notes (`note_cm_1` and `note_cm_2`) into a single
    /// consolidated change note within the shielded pool without revealing balances
    /// to public accounts on-chain.
    #[allow(clippy::too_many_arguments)]
    pub fn consolidate_notes(
        &mut self,
        note_cm_1: &str,
        note_cm_2: &str,
        session_id: &str,
        execution_fee: u64,
        max_execution_fee: u64,
        quote_hash_hex: &str,
        chain_id: u64,
        contract_address_hex: &str,
        expiry: u64,
        current_time_secs: u64,
        joinsplit_pk: &ark_groth16::ProvingKey<Bls12_381>,
    ) -> Result<JoinSplitSpendProofPayload, WalletError> {
        let (val1, val2) = {
            let n1 = self
                .notes
                .get(note_cm_1)
                .ok_or_else(|| WalletError::NoteNotFound(note_cm_1.to_string()))?;
            let n2 = self
                .notes
                .get(note_cm_2)
                .ok_or_else(|| WalletError::NoteNotFound(note_cm_2.to_string()))?;

            if !matches!(n1.status, NoteStatus::Unspent) {
                return Err(WalletError::NoteNotSpendable(note_cm_1.to_string()));
            }
            if !matches!(n2.status, NoteStatus::Unspent) {
                return Err(WalletError::NoteNotSpendable(note_cm_2.to_string()));
            }
            (n1.value, n2.value)
        };

        let total_in = val1
            .checked_add(val2)
            .ok_or_else(|| WalletError::CryptoError("Combined note value overflow".into()))?;

        if total_in <= execution_fee {
            return Err(WalletError::CryptoError(
                "Combined note value cannot cover execution fee".into(),
            ));
        }

        let consolidated_value = total_in - execution_fee;

        let selected = SelectedSpend {
            input_commitment_hex: note_cm_1.to_string(),
            input_value: val1,
            second_input_commitment_hex: Some(note_cm_2.to_string()),
            second_input_value: val2,
            merchant_amount: 0,
            protocol_fee: 0,
            execution_fee,
            total_required: execution_fee,
            change_amount: consolidated_value,
            has_change: true,
        };

        // Recipient is Address::ZERO for autonomous in-pool consolidation
        let zero_address = "0x0000000000000000000000000000000000000000";

        self.prepare_joinsplit_spend_proof(
            &selected,
            session_id,
            zero_address,
            max_execution_fee,
            quote_hash_hex,
            chain_id,
            contract_address_hex,
            expiry,
            current_time_secs,
            joinsplit_pk,
        )
    }

    /// High-level client API: transfers funds to another EVM address or stealth address.
    #[allow(clippy::too_many_arguments)]
    pub fn send_to_wallet(
        &mut self,
        recipient_wallet: &str,
        amount: u64,
        protocol_fee: u64,
        execution_fee: u64,
        max_execution_fee: u64,
        quote_hash_hex: &str,
        chain_id: u64,
        contract_address_hex: &str,
        expiry: u64,
        current_time_secs: u64,
        pk: &ark_groth16::ProvingKey<Bls12_381>,
    ) -> Result<SpendProofPayload, WalletError> {
        self.pay(
            recipient_wallet,
            amount,
            protocol_fee,
            execution_fee,
            max_execution_fee,
            quote_hash_hex,
            chain_id,
            contract_address_hex,
            expiry,
            current_time_secs,
            pk,
        )
    }

    /// High-level client API: generates withdrawal spend proofs to sweep entire balance to an owner wallet.
    #[allow(clippy::too_many_arguments)]
    pub fn withdraw_all(
        &mut self,
        owner_wallet: &str,
        protocol_fee_per_note: u64,
        execution_fee_per_note: u64,
        max_execution_fee_per_note: u64,
        quote_hash_hex: &str,
        chain_id: u64,
        contract_address_hex: &str,
        expiry: u64,
        current_time_secs: u64,
        pk: &ark_groth16::ProvingKey<Bls12_381>,
    ) -> Result<Vec<SpendProofPayload>, WalletError> {
        let spendable_cms: Vec<(String, u64)> = self
            .notes
            .values()
            .filter(|n| {
                if n.merkle_path_hex.is_none() || n.leaf_index.is_none() {
                    return false;
                }
                match &n.status {
                    NoteStatus::Unspent => true,
                    NoteStatus::Reserved {
                        lease_expiry_secs, ..
                    } => *lease_expiry_secs <= current_time_secs,
                    _ => false,
                }
            })
            .map(|n| (n.commitment_hex.clone(), n.value))
            .collect();

        if spendable_cms.is_empty() {
            return Err(WalletError::InsufficientBalance {
                requested: 1,
                available: 0,
            });
        }

        let mut payloads = Vec::with_capacity(spendable_cms.len());
        for (cm, val) in spendable_cms {
            let total_fees = protocol_fee_per_note + execution_fee_per_note;
            if val <= total_fees {
                continue; // Skip dust notes smaller than fees
            }
            let net_merchant = val - total_fees;
            let selected = SelectedSpend {
                input_commitment_hex: cm,
                input_value: val,
                second_input_commitment_hex: None,
                second_input_value: 0,
                merchant_amount: net_merchant,
                protocol_fee: protocol_fee_per_note,
                execution_fee: execution_fee_per_note,
                total_required: val,
                change_amount: 0,
                has_change: false,
            };

            let session_id = format!(
                "withdraw_{}_{}",
                current_time_secs,
                hex::encode(rand::random::<[u8; 8]>())
            );
            let payload = self.prepare_spend_proof(
                &selected,
                &session_id,
                owner_wallet,
                max_execution_fee_per_note,
                quote_hash_hex,
                chain_id,
                contract_address_hex,
                expiry,
                current_time_secs,
                pk,
            )?;
            payloads.push(payload);
        }

        Ok(payloads)
    }

    /// Finalizes transaction receipt (2PC Phase 2 - Commit).
    ///
    /// Transitions input note to `Spent` and change note to `Unspent`
    /// once confirmed on-chain with its Merkle index & path.
    pub fn commit_spend(
        &mut self,
        session_id: &str,
        nullifier_hex: &str,
        current_time_secs: u64,
        change_leaf_index: Option<u64>,
        change_merkle_path: Option<Vec<String>>,
        new_merkle_root: Option<&str>,
    ) -> Result<(), WalletError> {
        // 1. Mark input note(s) as Spent
        let mut found_input = false;
        for note in self.notes.values_mut() {
            if note.session_id.as_deref() == Some(session_id) {
                if let NoteStatus::Reserved { .. } = &note.status {
                    note.status = NoteStatus::Spent {
                        nullifier_hex: nullifier_hex.to_string(),
                        spent_at_secs: current_time_secs,
                    };
                    found_input = true;
                }
            }
        }

        if !found_input {
            return Err(WalletError::NoteNotFound(format!(
                "Session {session_id} input note"
            )));
        }

        // 2. Promote Change Note to Unspent (if present and witness provided)
        if let (Some(idx), Some(path)) = (change_leaf_index, change_merkle_path) {
            for note in self.notes.values_mut() {
                if note.session_id.as_deref() == Some(session_id)
                    && note.status == NoteStatus::Unconfirmed
                {
                    note.leaf_index = Some(idx);
                    note.merkle_path_hex = Some(path);
                    note.status = NoteStatus::Unspent;
                    break;
                }
            }
        }

        if let Some(root) = new_merkle_root {
            self.current_merkle_root_hex = Some(root.to_string());
        }

        Ok(())
    }

    /// Finalizes a 2-note JoinSplit spend transaction receipt (DEC-030).
    ///
    /// Transitions input note 1 to `Spent(nullifier_1)` and input note 2 (if present)
    /// to `Spent(nullifier_2)`, then promotes the change note to `Unspent`.
    #[allow(clippy::too_many_arguments)]
    pub fn commit_joinsplit_spend(
        &mut self,
        session_id: &str,
        nullifier_1_hex: &str,
        nullifier_2_hex: Option<&str>,
        current_time_secs: u64,
        change_leaf_index: Option<u64>,
        change_merkle_path: Option<Vec<String>>,
        new_merkle_root: Option<&str>,
    ) -> Result<(), WalletError> {
        let mut reserved_cms: Vec<String> = Vec::new();
        for (cm, note) in &self.notes {
            if note.session_id.as_deref() == Some(session_id) {
                if let NoteStatus::Reserved { .. } = &note.status {
                    reserved_cms.push(cm.clone());
                }
            }
        }

        if reserved_cms.is_empty() {
            return Err(WalletError::NoteNotFound(format!(
                "Session {session_id} input notes"
            )));
        }

        // Mark note 1 as Spent with nullifier 1
        if let Some(note1) = self.notes.get_mut(&reserved_cms[0]) {
            note1.status = NoteStatus::Spent {
                nullifier_hex: nullifier_1_hex.to_string(),
                spent_at_secs: current_time_secs,
            };
        }

        // If there is a note 2, mark as Spent with nullifier 2
        if reserved_cms.len() > 1 {
            let n2_hex = nullifier_2_hex.unwrap_or(nullifier_1_hex);
            if let Some(note2) = self.notes.get_mut(&reserved_cms[1]) {
                note2.status = NoteStatus::Spent {
                    nullifier_hex: n2_hex.to_string(),
                    spent_at_secs: current_time_secs,
                };
            }
        }

        // Promote Change Note to Unspent (if present and witness provided)
        if let (Some(idx), Some(path)) = (change_leaf_index, change_merkle_path) {
            for note in self.notes.values_mut() {
                if note.session_id.as_deref() == Some(session_id)
                    && note.status == NoteStatus::Unconfirmed
                {
                    note.leaf_index = Some(idx);
                    note.merkle_path_hex = Some(path);
                    note.status = NoteStatus::Unspent;
                    break;
                }
            }
        }

        if let Some(root) = new_merkle_root {
            self.current_merkle_root_hex = Some(root.to_string());
        }

        Ok(())
    }

    /// Aborts an active session on failure or timeout (2PC Phase 2 - Rollback).
    ///
    /// Restores input note to `Unspent` and discards unconfirmed change notes.
    pub fn rollback_spend(&mut self, session_id: &str) -> Result<(), WalletError> {
        let mut found_input = false;
        let mut change_cm_to_remove = None;

        for (cm, note) in self.notes.iter_mut() {
            if note.session_id.as_deref() == Some(session_id) {
                match &note.status {
                    NoteStatus::Reserved { .. } => {
                        note.status = NoteStatus::Unspent;
                        note.session_id = None;
                        found_input = true;
                    }
                    NoteStatus::Unconfirmed => {
                        change_cm_to_remove = Some(cm.clone());
                    }
                    _ => {}
                }
            }
        }

        if let Some(cm) = change_cm_to_remove {
            self.notes.remove(&cm);
        }

        if !found_input {
            return Err(WalletError::NoteNotFound(format!("Session {session_id}")));
        }

        Ok(())
    }

    /// Automatically detects expired note reservations and restores them to `Unspent` (DEC-027).
    ///
    /// Cleans up any unconfirmed change notes created for those sessions, restoring strict self-custody.
    /// Returns the number of notes reclaimed.
    pub fn reclaim_expired_reservations(&mut self, current_time_secs: u64) -> usize {
        let mut expired_sessions = Vec::new();

        for note in self.notes.values() {
            if let NoteStatus::Reserved {
                lease_expiry_secs,
                session_id,
            } = &note.status
            {
                if *lease_expiry_secs <= current_time_secs {
                    expired_sessions.push(session_id.clone());
                }
            }
        }

        let mut reclaimed_count = 0;
        for session_id in expired_sessions {
            if self.rollback_spend(&session_id).is_ok() {
                reclaimed_count += 1;
            }
        }

        reclaimed_count
    }

    /// Explicitly syncs a mempool expiration for a session, restoring input note to `Unspent` (DEC-027).
    pub fn sync_mempool_expiry(&mut self, session_id: &str) -> Result<(), WalletError> {
        self.rollback_spend(session_id)
    }

    /// Calculates the effective spendable balance, treating expired reservations as unspent (DEC-027).
    pub fn effective_balance(&self, current_time_secs: u64) -> u64 {
        self.notes
            .values()
            .filter(|n| match &n.status {
                NoteStatus::Unspent => true,
                NoteStatus::Reserved {
                    lease_expiry_secs, ..
                } => *lease_expiry_secs <= current_time_secs,
                _ => false,
            })
            .map(|n| n.value)
            .sum()
    }

    /// Exports an encrypted backup of the wallet state using Argon2id + ChaCha20-Poly1305 AEAD (DEC-030 / RFC 9106 + RFC 8439).
    pub fn export_backup(&self, password: &str) -> Result<String, WalletError> {
        let serialized = serde_json::to_string(self)
            .map_err(|e| WalletError::SerializationError(e.to_string()))?;

        // 1. Generate fresh 32-byte salt and 12-byte nonce
        let mut salt = [0u8; 32];
        let mut nonce_bytes = [0u8; 12];
        rand::RngCore::fill_bytes(&mut OsRng, &mut salt);
        rand::RngCore::fill_bytes(&mut OsRng, &mut nonce_bytes);

        // 2. Derive 32-byte key using Argon2id (RFC 9106)
        // Memory: 16 MiB (16384 KiB) for responsive execution & WASM compatibility, 3 iterations, 1 lane
        let params = Params::new(16_384, 3, 1, Some(32))
            .map_err(|e| WalletError::CryptoError(format!("Argon2 params error: {e}")))?;
        let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

        let mut derived_key = [0u8; 32];
        argon2
            .hash_password_into(password.as_bytes(), &salt, &mut derived_key)
            .map_err(|e| WalletError::CryptoError(format!("Argon2 KDF error: {e}")))?;

        // 3. Encrypt with ChaCha20-Poly1305 AEAD (RFC 8439)
        let cipher = ChaCha20Poly1305::new_from_slice(&derived_key)
            .map_err(|e| WalletError::CryptoError(format!("Cipher init error: {e}")))?;
        let nonce = Nonce::from(nonce_bytes);

        let ciphertext = cipher
            .encrypt(&nonce, serialized.as_bytes())
            .map_err(|e| WalletError::CryptoError(format!("AEAD encryption error: {e}")))?;

        // Securely erase derived key from memory
        crate::secure_zeroize(&mut derived_key);

        let backup_payload = serde_json::json!({
            "version": 2,
            "kdf": "argon2id",
            "salt_hex": hex::encode(salt),
            "nonce_hex": hex::encode(nonce_bytes),
            "ciphertext_hex": hex::encode(ciphertext),
        });

        serde_json::to_string(&backup_payload)
            .map_err(|e| WalletError::SerializationError(e.to_string()))
    }

    /// Imports and decrypts a wallet backup (Supports V2 Argon2id-ChaCha20Poly1305 and legacy V1).
    pub fn import_backup(backup_json: &str, password: &str) -> Result<Self, WalletError> {
        let val: serde_json::Value =
            serde_json::from_str(backup_json).map_err(|_| WalletError::BackupDecryptionFailed)?;

        let version = val["version"].as_u64().unwrap_or(1);
        if version == 2 {
            let salt_hex = val["salt_hex"]
                .as_str()
                .ok_or(WalletError::BackupDecryptionFailed)?;
            let nonce_hex = val["nonce_hex"]
                .as_str()
                .ok_or(WalletError::BackupDecryptionFailed)?;
            let ciphertext_hex = val["ciphertext_hex"]
                .as_str()
                .ok_or(WalletError::BackupDecryptionFailed)?;

            let salt = hex::decode(salt_hex).map_err(|_| WalletError::BackupDecryptionFailed)?;
            let nonce_bytes =
                hex::decode(nonce_hex).map_err(|_| WalletError::BackupDecryptionFailed)?;
            let ciphertext =
                hex::decode(ciphertext_hex).map_err(|_| WalletError::BackupDecryptionFailed)?;

            if salt.len() != 32 || nonce_bytes.len() != 12 {
                return Err(WalletError::BackupDecryptionFailed);
            }

            let mut nonce_arr = [0u8; 12];
            nonce_arr.copy_from_slice(&nonce_bytes);
            let nonce = Nonce::from(nonce_arr);

            let params = Params::new(16_384, 3, 1, Some(32))
                .map_err(|_| WalletError::BackupDecryptionFailed)?;
            let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

            let mut derived_key = [0u8; 32];
            argon2
                .hash_password_into(password.as_bytes(), &salt, &mut derived_key)
                .map_err(|_| WalletError::BackupDecryptionFailed)?;

            let cipher = ChaCha20Poly1305::new_from_slice(&derived_key)
                .map_err(|_| WalletError::BackupDecryptionFailed)?;

            let plaintext_bytes = cipher.decrypt(&nonce, ciphertext.as_slice()).map_err(|_| {
                crate::secure_zeroize(&mut derived_key);
                WalletError::BackupDecryptionFailed
            })?;

            crate::secure_zeroize(&mut derived_key);

            let plaintext = String::from_utf8(plaintext_bytes)
                .map_err(|_| WalletError::BackupDecryptionFailed)?;
            serde_json::from_str(&plaintext).map_err(|_| WalletError::BackupDecryptionFailed)
        } else {
            // Legacy V1 backup fallback for backward compatibility
            let mac_hex = val["mac_hex"]
                .as_str()
                .ok_or(WalletError::BackupDecryptionFailed)?;
            let ciphertext_hex = val["ciphertext_hex"]
                .as_str()
                .ok_or(WalletError::BackupDecryptionFailed)?;

            let expected_mac =
                hex::decode(mac_hex).map_err(|_| WalletError::BackupDecryptionFailed)?;
            let mut ciphertext =
                hex::decode(ciphertext_hex).map_err(|_| WalletError::BackupDecryptionFailed)?;

            let key = crate::hmac_sha256(password.as_bytes(), b"nimbus.wallet.backup.v1");

            let mut stream_hasher = Sha256::new();
            stream_hasher.update(key);
            stream_hasher.update(&expected_mac);
            let mut block = stream_hasher.finalize();

            for (i, byte) in ciphertext.iter_mut().enumerate() {
                if i % 32 == 0 && i > 0 {
                    let mut next_hasher = Sha256::new();
                    next_hasher.update(block);
                    block = next_hasher.finalize();
                }
                *byte ^= block[i % 32];
            }

            let computed_mac = crate::hmac_sha256(&key, &ciphertext);
            if computed_mac.as_slice() != expected_mac.as_slice() {
                return Err(WalletError::BackupDecryptionFailed);
            }

            let plaintext =
                String::from_utf8(ciphertext).map_err(|_| WalletError::BackupDecryptionFailed)?;
            serde_json::from_str(&plaintext).map_err(|_| WalletError::BackupDecryptionFailed)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_merkle_path() -> Vec<String> {
        let empty = nimbus_core::compute_empty_hashes();
        (0..MERKLE_TREE_DEPTH)
            .map(|i| hex::encode(fr_to_be_bytes(&empty[i])))
            .collect()
    }

    #[test]
    fn test_wallet_deterministic_keys() {
        let seed = [7u8; 32];
        let w1 = PrivateNoteWallet::new(&seed);
        let w2 = PrivateNoteWallet::new(&seed);

        assert_eq!(w1.spending_key().unwrap(), w2.spending_key().unwrap());
        assert_eq!(w1.nullifier_key().unwrap(), w2.nullifier_key().unwrap());
        assert_eq!(w1.balance(), 0);
    }

    #[test]
    fn test_note_lifecycle_and_balance() {
        let seed = [9u8; 32];
        let mut wallet = PrivateNoteWallet::new(&seed);

        // 1. Create deposit
        let (note1, cm1) = wallet.create_deposit_note(50_000_000, 1000).unwrap();
        assert_eq!(note1.status, NoteStatus::Unconfirmed);
        assert_eq!(wallet.balance(), 0); // Unconfirmed doesn't count towards balance

        // 2. Confirm deposit
        let path = dummy_merkle_path();
        wallet
            .confirm_deposit(&cm1, 0, path.clone(), "0xroot")
            .unwrap();
        assert_eq!(wallet.balance(), 50_000_000);

        // 3. Add second note
        let (_, cm2) = wallet.create_deposit_note(30_000_000, 1005).unwrap();
        wallet.confirm_deposit(&cm2, 1, path, "0xroot").unwrap();
        assert_eq!(wallet.balance(), 80_000_000);
    }

    #[test]
    fn test_coin_selection_exact_match_zero_change() {
        let seed = [11u8; 32];
        let mut wallet = PrivateNoteWallet::new(&seed);
        let path = dummy_merkle_path();

        let (_, cm1) = wallet.create_deposit_note(10_000_000, 1000).unwrap();
        let (_, cm2) = wallet.create_deposit_note(25_000_000, 1000).unwrap();
        wallet
            .confirm_deposit(&cm1, 0, path.clone(), "0xroot")
            .unwrap();
        wallet.confirm_deposit(&cm2, 1, path, "0xroot").unwrap();

        // Exact match for 10_000_000 total (9_800_000 + 150_000 + 50_000)
        let selected = wallet
            .select_note_for_spend(9_800_000, 150_000, 50_000, 1010)
            .unwrap();
        assert_eq!(selected.input_commitment_hex, cm1);
        assert_eq!(selected.total_required, 10_000_000);
        assert_eq!(selected.change_amount, 0);
        assert!(!selected.has_change);
    }

    #[test]
    fn test_coin_selection_best_fit_with_change() {
        let seed = [12u8; 32];
        let mut wallet = PrivateNoteWallet::new(&seed);
        let path = dummy_merkle_path();

        let (_, cm1) = wallet.create_deposit_note(10_000_000, 1000).unwrap();
        let (_, cm2) = wallet.create_deposit_note(50_000_000, 1000).unwrap();
        wallet
            .confirm_deposit(&cm1, 0, path.clone(), "0xroot")
            .unwrap();
        wallet.confirm_deposit(&cm2, 1, path, "0xroot").unwrap();

        // Need 15_000_000: cm1 is too small, cm2 is selected
        let selected = wallet
            .select_note_for_spend(14_000_000, 700_000, 300_000, 1010)
            .unwrap();
        assert_eq!(selected.input_commitment_hex, cm2);
        assert_eq!(selected.total_required, 15_000_000);
        assert_eq!(selected.change_amount, 35_000_000);
        assert!(selected.has_change);
    }

    #[test]
    fn test_reservation_and_lease_expiry() {
        let seed = [15u8; 32];
        let mut wallet = PrivateNoteWallet::new(&seed);
        let path = dummy_merkle_path();

        let (_, cm1) = wallet.create_deposit_note(20_000_000, 1000).unwrap();
        wallet.confirm_deposit(&cm1, 0, path, "0xroot").unwrap();

        // Reserve for 60s at time 1000
        wallet.reserve_note(&cm1, "session_1", 1000, 60).unwrap();
        assert_eq!(wallet.balance(), 0);
        assert_eq!(wallet.reserved_balance(1000), 20_000_000);
        assert_eq!(wallet.total_balance(1000), 20_000_000);

        // Attempting to reserve again while active fails
        let res2 = wallet.reserve_note(&cm1, "session_2", 1030, 60);
        assert_eq!(res2, Err(WalletError::NoteAlreadyReserved(cm1.clone())));

        // At time 1061, lease has expired: re-reservation succeeds
        assert_eq!(wallet.reserved_balance(1061), 0);
        wallet.reserve_note(&cm1, "session_2", 1061, 60).unwrap();
    }

    #[test]
    fn test_two_phase_commit_and_rollback() {
        let seed = [20u8; 32];
        let mut wallet = PrivateNoteWallet::new(&seed);
        let path = dummy_merkle_path();

        let (_, cm1) = wallet.create_deposit_note(100_000_000, 1000).unwrap();
        wallet.confirm_deposit(&cm1, 0, path, "0xroot").unwrap();

        let selected = wallet
            .select_note_for_spend(40_000_000, 2_000_000, 1_000_000, 1000)
            .unwrap();
        wallet
            .reserve_note(&selected.input_commitment_hex, "session_tx_1", 1000, 120)
            .unwrap();

        // Simulate rollback on failure
        wallet.rollback_spend("session_tx_1").unwrap();
        assert_eq!(wallet.balance(), 100_000_000);

        // Re-reserve and simulate successful commit
        wallet
            .reserve_note(&selected.input_commitment_hex, "session_tx_2", 1050, 120)
            .unwrap();
        wallet
            .commit_spend("session_tx_2", "0xnullifier123", 1055, None, None, None)
            .unwrap();

        assert_eq!(wallet.balance(), 0);
        let note = wallet.notes.get(&cm1).unwrap();
        match &note.status {
            NoteStatus::Spent { nullifier_hex, .. } => assert_eq!(nullifier_hex, "0xnullifier123"),
            _ => panic!("Expected spent status"),
        }
    }

    #[test]
    fn test_encrypted_backup_roundtrip() {
        let seed = [42u8; 32];
        let mut wallet = PrivateNoteWallet::new(&seed);
        let path = dummy_merkle_path();

        let (_, cm1) = wallet.create_deposit_note(55_000_000, 2000).unwrap();
        wallet.confirm_deposit(&cm1, 5, path, "0xroot_abc").unwrap();

        let backup = wallet.export_backup("SuperSecretPassword123!").unwrap();
        assert!(!backup.is_empty());

        // Restore with wrong password fails
        let wrong_restore = PrivateNoteWallet::import_backup(&backup, "WrongPassword");
        assert_eq!(
            wrong_restore.err(),
            Some(WalletError::BackupDecryptionFailed)
        );

        // Restore with correct password succeeds
        let restored =
            PrivateNoteWallet::import_backup(&backup, "SuperSecretPassword123!").unwrap();
        assert_eq!(restored.balance(), 55_000_000);
        assert_eq!(
            restored.spending_key().unwrap(),
            wallet.spending_key().unwrap()
        );
        assert_eq!(
            restored.current_merkle_root_hex.as_deref(),
            Some("0xroot_abc")
        );
    }

    #[test]
    fn test_prepare_spend_proof_and_verification() {
        let seed = [99u8; 32];
        let mut wallet = PrivateNoteWallet::new(&seed);
        let path = dummy_merkle_path();

        let (_, cm1) = wallet.create_deposit_note(50_000_000, 1000).unwrap();
        wallet.confirm_deposit(&cm1, 0, path, "0xroot").unwrap();

        let selected = wallet
            .select_note_for_spend(20_000_000, 100_000, 50_000, 1000)
            .unwrap();
        assert!(selected.has_change);
        assert_eq!(selected.change_amount, 29_850_000);

        let keys = nimbus_core::generate_note_circuit_keys().unwrap();

        let payload = wallet
            .prepare_spend_proof(
                &selected,
                "session_e2e_1",
                "0x1111111111111111111111111111111111111111",
                60_000,
                "0x2222222222222222222222222222222222222222222222222222222222222222",
                421614,
                "0x3333333333333333333333333333333333333333",
                2000,
                1000,
                &keys.proving_key,
            )
            .unwrap();

        assert_eq!(payload.proof_a_neg_hex.len(), 256); // 128 bytes
        assert_eq!(payload.proof_b_hex.len(), 512); // 256 bytes
        assert_eq!(payload.proof_c_hex.len(), 256); // 128 bytes
        assert_eq!(payload.public_inputs_hex.len(), 16);
        assert_eq!(payload.leaf_count, 1);
        assert_eq!(payload.note_epoch_id, 0);
        assert!(!payload.is_rollover);
        assert_eq!(payload.merchant_amount, 20_000_000);
        assert!(payload.has_change);

        // Commit the spend
        let change_path = dummy_merkle_path();
        wallet
            .commit_spend(
                "session_e2e_1",
                &payload.input_nullifier_hex,
                1005,
                Some(1),
                Some(change_path),
                Some("0xnew_root"),
            )
            .unwrap();

        // Balance should now equal the change note value
        assert_eq!(wallet.balance(), 29_850_000);
    }

    #[test]
    fn test_note_epoch_status_lifecycle() {
        let seed = [25u8; 32];
        let mut wallet = PrivateNoteWallet::new(&seed);
        let (mut note, _) = wallet.create_deposit_note(5_000_000, 1000).unwrap();

        // When epoch_id is 2
        note.epoch_id = Some(2);

        // At epoch 2: Fresh
        assert_eq!(note.check_epoch_status(2), NoteEpochStatus::Fresh);
        assert_eq!(
            wallet.check_note_epoch_status(&note, 2),
            NoteEpochStatus::Fresh
        );

        // At epoch 3: MatureGrace (E-1 = 2)
        assert_eq!(note.check_epoch_status(3), NoteEpochStatus::MatureGrace);
        assert_eq!(
            wallet.check_note_epoch_status(&note, 3),
            NoteEpochStatus::MatureGrace
        );

        // At epoch 4: ExpiredRequiresRollover (<= E-2)
        assert_eq!(
            note.check_epoch_status(4),
            NoteEpochStatus::ExpiredRequiresRollover
        );
        assert_eq!(
            wallet.check_note_epoch_status(&note, 4),
            NoteEpochStatus::ExpiredRequiresRollover
        );

        // At epoch 1: Fresh or future
        assert_eq!(
            note.check_epoch_status(1),
            NoteEpochStatus::ExpiredRequiresRollover
        );
    }

    #[test]
    fn test_standalone_refresh_dust_rejected() {
        let seed = [26u8; 32];
        let mut wallet = PrivateNoteWallet::new(&seed);
        let path = dummy_merkle_path();
        let keys = nimbus_core::get_or_init_note_circuit_keys();

        // Note with 1.50 USDC (< 2.00 USDC minimum)
        let (_, cm) = wallet.create_deposit_note(1_500_000, 1000).unwrap();
        wallet.confirm_deposit(&cm, 0, path, "0xroot").unwrap();

        let res = wallet.build_standalone_refresh(
            &cm,
            20_000,
            25_000,
            "0x1111111111111111111111111111111111111111111111111111111111111111",
            421614,
            "0x3333333333333333333333333333333333333333",
            2000,
            1,
            1000,
            &keys.proving_key,
        );

        assert!(res.is_err());
        match res {
            Err(WalletError::CryptoError(msg)) => {
                assert!(msg.contains("DUST_NOTE_ROLLOVER_REJECTED"));
            }
            _ => panic!("Expected DUST_NOTE_ROLLOVER_REJECTED error"),
        }
    }

    #[test]
    fn test_build_in_flight_spend_rollover_and_refresh() {
        let seed = [27u8; 32];
        let mut wallet = PrivateNoteWallet::new(&seed);
        let path = dummy_merkle_path();
        let keys = nimbus_core::get_or_init_note_circuit_keys();

        // Create deposit note with epoch 0, amount 10.00 USDC
        let (_, cm) = wallet
            .create_deposit_note_with_epoch(10_000_000, 0, 1000)
            .unwrap();
        wallet.confirm_deposit(&cm, 0, path, "0xroot").unwrap();

        // In-flight rollover spend refreshing change note into epoch 1
        let payload = wallet
            .build_in_flight_spend_rollover(
                "0x2222222222222222222222222222222222222222",
                3_000_000,
                13_500, // 45 bps flat protocol fee
                20_000, // execution fee
                25_000,
                "0x1111111111111111111111111111111111111111111111111111111111111111",
                421614,
                "0x3333333333333333333333333333333333333333",
                2000,
                1, // target epoch 1
                1000,
                &keys.proving_key,
            )
            .unwrap();

        assert_eq!(payload.public_inputs_hex.len(), 16);
        assert_eq!(payload.note_epoch_id, 0); // Input note was epoch 0
        assert!(!payload.is_rollover);
        assert_eq!(payload.merchant_amount, 3_000_000);
        assert!(payload.has_change);

        // Find the newly created change note and verify its epoch is 1
        let change_note = wallet
            .notes
            .values()
            .find(|n| n.status == NoteStatus::Unconfirmed)
            .expect("change note should exist");
        assert_eq!(change_note.epoch_id, Some(1));
        assert_eq!(change_note.value, 10_000_000 - 3_000_000 - 13_500 - 20_000);
    }

    #[test]
    fn test_reclaim_expired_reservations_and_effective_balance() {
        let seed = [30u8; 32];
        let mut wallet = PrivateNoteWallet::new(&seed);
        let path = dummy_merkle_path();

        let (_, cm1) = wallet.create_deposit_note(50_000_000, 1000).unwrap();
        wallet.confirm_deposit(&cm1, 0, path, "0xroot").unwrap();

        // Reserve for 60s at time 1000 (expires at 1060)
        wallet
            .reserve_note(&cm1, "session_expired_1", 1000, 60)
            .unwrap();

        assert_eq!(wallet.balance(), 0);
        assert_eq!(wallet.effective_balance(1030), 0);
        assert_eq!(wallet.effective_balance(1061), 50_000_000);

        // Before expiry, reclaim yields 0
        let reclaimed_early = wallet.reclaim_expired_reservations(1030);
        assert_eq!(reclaimed_early, 0);
        assert_eq!(wallet.balance(), 0);

        // After expiry, reclaim yields 1 and restores balance
        let reclaimed = wallet.reclaim_expired_reservations(1061);
        assert_eq!(reclaimed, 1);
        assert_eq!(wallet.balance(), 50_000_000);

        // Explicit sync_mempool_expiry test
        wallet
            .reserve_note(&cm1, "session_manual_sync", 1100, 60)
            .unwrap();
        assert_eq!(wallet.balance(), 0);
        wallet.sync_mempool_expiry("session_manual_sync").unwrap();
        assert_eq!(wallet.balance(), 50_000_000);
    }

    #[test]
    fn test_encrypted_backup_tampering_rejected() {
        let seed = [43u8; 32];
        let mut wallet = PrivateNoteWallet::new(&seed);
        let path = dummy_merkle_path();

        let (_, cm1) = wallet.create_deposit_note(30_000_000, 2000).unwrap();
        wallet
            .confirm_deposit(&cm1, 2, path, "0xroot_tamper")
            .unwrap();

        let backup = wallet.export_backup("Password123!").unwrap();

        // 1. Tamper with ciphertext
        let mut parsed: serde_json::Value = serde_json::from_str(&backup).unwrap();
        let ct_hex = parsed["ciphertext_hex"].as_str().unwrap().to_string();
        let mut ct_bytes = hex::decode(&ct_hex).unwrap();
        ct_bytes[0] ^= 0xff; // flip bits
        parsed["ciphertext_hex"] = serde_json::Value::String(hex::encode(ct_bytes));
        let tampered_backup = serde_json::to_string(&parsed).unwrap();

        let res = PrivateNoteWallet::import_backup(&tampered_backup, "Password123!");
        assert_eq!(res.err(), Some(WalletError::BackupDecryptionFailed));

        // 2. Tamper with salt
        let mut parsed_salt: serde_json::Value = serde_json::from_str(&backup).unwrap();
        parsed_salt["salt_hex"] = serde_json::Value::String(hex::encode([0u8; 32]));
        let tampered_salt_backup = serde_json::to_string(&parsed_salt).unwrap();
        let res_salt = PrivateNoteWallet::import_backup(&tampered_salt_backup, "Password123!");
        assert_eq!(res_salt.err(), Some(WalletError::BackupDecryptionFailed));
    }

    #[test]
    fn test_update_note_witness_lifecycle() {
        let seed = [44u8; 32];
        let mut wallet = PrivateNoteWallet::new(&seed);
        let path = dummy_merkle_path();

        let (note, cm) = wallet.create_deposit_note(70_000_000, 1000).unwrap();
        assert_eq!(note.status, NoteStatus::Unconfirmed);
        assert_eq!(wallet.balance(), 0);

        // Negative: Invalid path length rejected
        let invalid_path = vec!["0x1234".to_string(); 5]; // Depth is 20, not 5
        let err_path = wallet.update_note_witness(&cm, 0, invalid_path, "0xroot1");
        assert!(err_path.is_err());
        assert_eq!(wallet.balance(), 0);

        // Negative: Non-existent note rejected
        let err_notfound = wallet.update_note_witness("0xdeadbeef", 0, path.clone(), "0xroot1");
        assert_eq!(
            err_notfound.err(),
            Some(WalletError::NoteNotFound("0xdeadbeef".into()))
        );

        // Positive: Update witness promotes note to Unspent
        wallet
            .update_note_witness(&cm, 10, path.clone(), "0xroot_synced")
            .unwrap();
        assert_eq!(wallet.balance(), 70_000_000);
        let updated_note = wallet.notes.get(&cm).unwrap();
        assert_eq!(updated_note.status, NoteStatus::Unspent);
        assert_eq!(updated_note.leaf_index, Some(10));
        assert_eq!(
            wallet.current_merkle_root_hex.as_deref(),
            Some("0xroot_synced")
        );
    }

    #[test]
    fn test_parse_fr_and_bytes32_helpers() {
        // Valid 32-byte scalar
        let valid_fr = Fr::from(123456789u64);
        let hex_fr = hex::encode(fr_to_be_bytes(&valid_fr));
        let parsed = parse_fr_from_hex(&hex_fr).unwrap();
        assert_eq!(parsed, valid_fr);

        // With 0x prefix
        let parsed_prefixed = parse_fr_from_hex(&format!("0x{hex_fr}")).unwrap();
        assert_eq!(parsed_prefixed, valid_fr);

        // Invalid hex length
        let err_len = parse_fr_from_hex("0x1234");
        assert!(err_len.is_err());

        // 20-byte EVM address padded to 32 bytes
        let addr = "0x5B38Da6a701c568545dCfcB03FcB875f56beddC4";
        let padded = parse_bytes32_from_hex(addr, true).unwrap();
        assert_eq!(&padded[0..12], &[0u8; 12]);
        assert_eq!(
            &padded[12..],
            &hex::decode("5B38Da6a701c568545dCfcB03FcB875f56beddC4").unwrap()[..]
        );

        // 20-byte address rejected when padding not allowed
        let err_pad = parse_bytes32_from_hex(addr, false);
        assert!(err_pad.is_err());
    }

    #[test]
    fn test_coin_selection_tiered_knapsack_joinsplit() {
        let seed = [50u8; 32];
        let mut wallet = PrivateNoteWallet::new(&seed);
        let path = dummy_merkle_path();

        // User has two notes of 10 USDC each (total 20 USDC)
        let (_, cm1) = wallet.create_deposit_note(10_000_000, 1000).unwrap();
        let (_, cm2) = wallet.create_deposit_note(10_000_000, 1000).unwrap();
        wallet
            .confirm_deposit(&cm1, 0, path.clone(), "0xroot")
            .unwrap();
        wallet.confirm_deposit(&cm2, 1, path, "0xroot").unwrap();

        // User spends 15 USDC total (14.5 merchant + 0.3 pfee + 0.2 efee)
        // Neither single note suffices; Branch 3 Knapsack must select both notes!
        let selected = wallet
            .select_note_for_spend(14_500_000, 300_000, 200_000, 1010)
            .unwrap();

        assert!(selected.is_joinsplit());
        assert_eq!(selected.total_input_value(), 20_000_000);
        assert_eq!(selected.total_required, 15_000_000);
        assert_eq!(selected.change_amount, 5_000_000);
        assert!(selected.has_change);
        assert!(
            (selected.input_commitment_hex == cm1
                && selected.second_input_commitment_hex.as_deref() == Some(&cm2))
                || (selected.input_commitment_hex == cm2
                    && selected.second_input_commitment_hex.as_deref() == Some(&cm1))
        );
    }

    #[test]
    fn test_coin_selection_insufficient_even_with_joinsplit() {
        let seed = [51u8; 32];
        let mut wallet = PrivateNoteWallet::new(&seed);
        let path = dummy_merkle_path();

        // Two notes of 5 USDC each (total 10 USDC)
        let (_, cm1) = wallet.create_deposit_note(5_000_000, 1000).unwrap();
        let (_, cm2) = wallet.create_deposit_note(5_000_000, 1000).unwrap();
        wallet
            .confirm_deposit(&cm1, 0, path.clone(), "0xroot")
            .unwrap();
        wallet.confirm_deposit(&cm2, 1, path, "0xroot").unwrap();

        // Spending 15 USDC fails with InsufficientBalance
        let err = wallet.select_note_for_spend(14_000_000, 700_000, 300_000, 1010);
        assert_eq!(
            err,
            Err(WalletError::InsufficientBalance {
                requested: 15_000_000,
                available: 10_000_000,
            })
        );
    }

    #[test]
    fn test_prepare_joinsplit_spend_proof_1_note_with_dummy() {
        let seed = [52u8; 32];
        let mut wallet = PrivateNoteWallet::new(&seed);
        let path = dummy_merkle_path();

        let (_, cm1) = wallet.create_deposit_note(50_000_000, 1000).unwrap();
        wallet.confirm_deposit(&cm1, 0, path, "0xroot").unwrap();

        // Single note spend (20 USDC from 50 USDC note)
        let selected = wallet
            .select_note_for_spend(20_000_000, 100_000, 50_000, 1000)
            .unwrap();
        assert!(!selected.is_joinsplit());

        let keys = nimbus_core::generate_joinsplit_circuit_keys().unwrap();

        let payload = wallet
            .prepare_joinsplit_spend_proof(
                &selected,
                "session_js_single",
                "0x1111111111111111111111111111111111111111",
                60_000,
                "0x2222222222222222222222222222222222222222222222222222222222222222",
                421614,
                "0x3333333333333333333333333333333333333333",
                2000,
                1000,
                &keys.proving_key,
            )
            .unwrap();

        assert_eq!(payload.public_inputs_hex.len(), 14);
        assert_eq!(payload.proof_a_neg_hex.len(), 256);
        assert_eq!(payload.proof_b_hex.len(), 512);
        assert_eq!(payload.proof_c_hex.len(), 256);
        assert!(payload.has_change_1);
        assert!(!payload.has_change_2);

        // Verify with nimbus_core::verify_joinsplit_proof
        let mut pis = Vec::new();
        for h in &payload.public_inputs_hex {
            pis.push(parse_fr_from_hex(h).unwrap());
        }

        let from_evm_a = nimbus_core::from_evm_g1(
            &hex::decode(&payload.proof_a_neg_hex)
                .unwrap()
                .try_into()
                .unwrap(),
        )
        .unwrap();
        let from_evm_b = nimbus_core::from_evm_g2(
            &hex::decode(&payload.proof_b_hex)
                .unwrap()
                .try_into()
                .unwrap(),
        )
        .unwrap();
        let from_evm_c = nimbus_core::from_evm_g1(
            &hex::decode(&payload.proof_c_hex)
                .unwrap()
                .try_into()
                .unwrap(),
        )
        .unwrap();

        let reconstructed_proof = ark_groth16::Proof {
            a: -from_evm_a,
            b: from_evm_b,
            c: from_evm_c,
        };

        let verified =
            nimbus_core::verify_joinsplit_proof(&keys.verifying_key, &reconstructed_proof, &pis)
                .unwrap();
        assert!(
            verified,
            "Universal JoinSplit proof with dummy 2nd note must verify"
        );
    }

    #[test]
    fn test_prepare_joinsplit_spend_proof_2_notes_and_commit() {
        let seed = [53u8; 32];
        let mut wallet = PrivateNoteWallet::new(&seed);

        let sk = wallet.spending_key().unwrap();
        let empty = nimbus_core::compute_empty_hashes();

        // Note 1: 10 USDC
        let (note1, cm1) = wallet.create_deposit_note(10_000_000, 1000).unwrap();
        // Note 2: 10 USDC
        let (note2, cm2) = wallet.create_deposit_note(10_000_000, 1000).unwrap();

        let rho1 = parse_fr_from_hex(&note1.rho_hex).unwrap();
        let rand1 = parse_fr_from_hex(&note1.randomness_hex).unwrap();
        let fr_cm1 = nimbus_core::note_commitment(10_000_000, sk, rho1, rand1);

        let rho2 = parse_fr_from_hex(&note2.rho_hex).unwrap();
        let rand2 = parse_fr_from_hex(&note2.randomness_hex).unwrap();
        let fr_cm2 = nimbus_core::note_commitment(10_000_000, sk, rho2, rand2);

        // Path for leaf 0: sibling at index 0 is cm2
        let mut path1: [Fr; MERKLE_TREE_DEPTH] = empty[..MERKLE_TREE_DEPTH].try_into().unwrap();
        path1[0] = fr_cm2;
        let root = nimbus_core::compute_merkle_root(fr_cm1, 0, &path1);

        // Path for leaf 1: sibling at index 0 is cm1
        let mut path2 = path1;
        path2[0] = fr_cm1;
        assert_eq!(nimbus_core::compute_merkle_root(fr_cm2, 1, &path2), root);

        let path1_hex: Vec<String> = path1
            .iter()
            .map(|p| hex::encode(fr_to_be_bytes(p)))
            .collect();
        let path2_hex: Vec<String> = path2
            .iter()
            .map(|p| hex::encode(fr_to_be_bytes(p)))
            .collect();
        let root_hex = format!("0x{}", hex::encode(fr_to_be_bytes(&root)));

        wallet
            .confirm_deposit(&cm1, 0, path1_hex, &root_hex)
            .unwrap();
        wallet
            .confirm_deposit(&cm2, 1, path2_hex, &root_hex)
            .unwrap();
        assert_eq!(wallet.balance(), 20_000_000);

        let keys = nimbus_core::generate_joinsplit_circuit_keys().unwrap();

        // Pay 15 USDC using 2-note JoinSplit
        let payload = wallet
            .pay_joinsplit(
                "0x1111111111111111111111111111111111111111",
                14_500_000,
                300_000,
                200_000,
                250_000,
                "0x2222222222222222222222222222222222222222222222222222222222222222",
                421614,
                "0x3333333333333333333333333333333333333333",
                2000,
                1010,
                &keys.proving_key,
            )
            .unwrap();

        assert_eq!(payload.public_inputs_hex.len(), 14);

        // Commit the JoinSplit spend
        let change_path = dummy_merkle_path();
        wallet
            .commit_joinsplit_spend(
                &payload.session_id,
                &payload.input_nullifier_1_hex,
                Some(&payload.input_nullifier_2_hex),
                1015,
                Some(2),
                Some(change_path),
                Some("0xnew_root_js"),
            )
            .unwrap();

        // Remaining balance must be change amount (5 USDC)
        assert_eq!(wallet.balance(), 5_000_000);
        let n1_status = &wallet.notes.get(&cm1).unwrap().status;
        let n2_status = &wallet.notes.get(&cm2).unwrap().status;
        assert!(matches!(n1_status, NoteStatus::Spent { .. }));
        assert!(matches!(n2_status, NoteStatus::Spent { .. }));
    }

    #[test]
    fn test_consolidate_notes_in_pool() {
        let seed = [54u8; 32];
        let mut wallet = PrivateNoteWallet::new(&seed);
        let sk = wallet.spending_key().unwrap();
        let empty = nimbus_core::compute_empty_hashes();

        let (note1, cm1) = wallet.create_deposit_note(10_000_000, 1000).unwrap();
        let (note2, cm2) = wallet.create_deposit_note(15_000_000, 1000).unwrap();

        let rho1 = parse_fr_from_hex(&note1.rho_hex).unwrap();
        let rand1 = parse_fr_from_hex(&note1.randomness_hex).unwrap();
        let fr_cm1 = nimbus_core::note_commitment(10_000_000, sk, rho1, rand1);

        let rho2 = parse_fr_from_hex(&note2.rho_hex).unwrap();
        let rand2 = parse_fr_from_hex(&note2.randomness_hex).unwrap();
        let fr_cm2 = nimbus_core::note_commitment(15_000_000, sk, rho2, rand2);

        let mut path1: [Fr; MERKLE_TREE_DEPTH] = empty[..MERKLE_TREE_DEPTH].try_into().unwrap();
        path1[0] = fr_cm2;
        let root = nimbus_core::compute_merkle_root(fr_cm1, 0, &path1);

        let mut path2 = path1;
        path2[0] = fr_cm1;

        let path1_hex: Vec<String> = path1
            .iter()
            .map(|p| hex::encode(fr_to_be_bytes(p)))
            .collect();
        let path2_hex: Vec<String> = path2
            .iter()
            .map(|p| hex::encode(fr_to_be_bytes(p)))
            .collect();
        let root_hex = format!("0x{}", hex::encode(fr_to_be_bytes(&root)));

        wallet
            .confirm_deposit(&cm1, 0, path1_hex, &root_hex)
            .unwrap();
        wallet
            .confirm_deposit(&cm2, 1, path2_hex, &root_hex)
            .unwrap();
        assert_eq!(wallet.balance(), 25_000_000);

        let keys = nimbus_core::generate_joinsplit_circuit_keys().unwrap();

        let payload = wallet
            .consolidate_notes(
                &cm1,
                &cm2,
                "session_consolidate_1",
                100_000, // 0.1 USDC exec fee
                150_000,
                "0x2222222222222222222222222222222222222222222222222222222222222222",
                421614,
                "0x3333333333333333333333333333333333333333",
                2000,
                1010,
                &keys.proving_key,
            )
            .unwrap();

        assert_eq!(payload.merchant_amount, 0);
        assert_eq!(payload.execution_fee, 100_000);

        // Commit consolidation
        let change_path = dummy_merkle_path();
        wallet
            .commit_joinsplit_spend(
                "session_consolidate_1",
                &payload.input_nullifier_1_hex,
                Some(&payload.input_nullifier_2_hex),
                1015,
                Some(2),
                Some(change_path),
                Some("0xroot_consolidated"),
            )
            .unwrap();

        // 25_000_000 - 100_000 = 24_900_000
        assert_eq!(wallet.balance(), 24_900_000);
    }

    #[test]
    fn test_mmr_proof_cache_and_deserialization_positive() {
        let seed = [99u8; 32];
        let mut wallet = PrivateNoteWallet::new(&seed);

        let sib1 = hex::encode(fr_to_be_bytes(&Fr::from(12345u64)));
        let sib2 = hex::encode(fr_to_be_bytes(&Fr::from(67890u64)));
        let peak1 = hex::encode(fr_to_be_bytes(&Fr::from(54321u64)));
        let root = hex::encode(fr_to_be_bytes(&Fr::from(99999u64)));

        let proof = MmrProofResponse {
            leaf_index: 5,
            leaf_count: 10,
            commitment: "0x1111".to_string(),
            mountain_height: 2,
            mountain_siblings: vec![sib1, sib2],
            peak_bagging_siblings: vec![peak1],
            bagged_root: format!("0x{root}"),
            block_number: 1000,
        };

        // Cache proof
        wallet.cache_mmr_proof(proof.clone());

        // Retrieve from cache
        let cached = wallet
            .get_cached_mmr_proof(5, &format!("0x{root}"))
            .unwrap();
        assert_eq!(cached.leaf_index, 5);
        assert_eq!(cached.mountain_height, 2);

        // Retrieve any for leaf
        let cached_any = wallet.get_any_cached_mmr_proof_for_leaf(5).unwrap();
        assert_eq!(cached_any.leaf_count, 10);

        // Deserialize siblings
        let (m_sibs, p_sibs) = PrivateNoteWallet::deserialize_mmr_siblings(&proof).unwrap();
        assert_eq!(m_sibs[0], Fr::from(12345u64));
        assert_eq!(m_sibs[1], Fr::from(67890u64));
        assert_eq!(m_sibs[2], Fr::from(0u64)); // zero-padded
        assert_eq!(p_sibs[0], Fr::from(54321u64));
        assert_eq!(p_sibs[1], Fr::from(0u64)); // zero-padded

        // Clear cache
        wallet.clear_mmr_proof_cache();
        assert!(wallet
            .get_cached_mmr_proof(5, &format!("0x{root}"))
            .is_none());
    }

    #[test]
    fn test_mmr_proof_deserialization_negative_bounds() {
        // Negative test 1: mountain_siblings > 32
        let invalid_m_sibs = vec!["00".repeat(32); 33];
        let proof1 = MmrProofResponse {
            leaf_index: 0,
            leaf_count: 1,
            commitment: "0x00".to_string(),
            mountain_height: 1,
            mountain_siblings: invalid_m_sibs,
            peak_bagging_siblings: vec![],
            bagged_root: "0x00".to_string(),
            block_number: 1,
        };
        let err1 = PrivateNoteWallet::deserialize_mmr_siblings(&proof1);
        assert!(err1.is_err());
        assert!(format!("{err1:?}").contains("mountain_siblings length 33 exceeds max 32"));

        // Negative test 2: peak_bagging_siblings > 32
        let invalid_p_sibs = vec!["00".repeat(32); 33];
        let proof2 = MmrProofResponse {
            leaf_index: 0,
            leaf_count: 1,
            commitment: "0x00".to_string(),
            mountain_height: 1,
            mountain_siblings: vec![],
            peak_bagging_siblings: invalid_p_sibs,
            bagged_root: "0x00".to_string(),
            block_number: 1,
        };
        let err2 = PrivateNoteWallet::deserialize_mmr_siblings(&proof2);
        assert!(err2.is_err());
        assert!(format!("{err2:?}").contains("peak_bagging_siblings length 33 exceeds max 32"));

        // Negative test 3: non-canonical scalar in siblings
        // Fr modulus + 1
        let non_canonical_hex = "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff";
        let proof3 = MmrProofResponse {
            leaf_index: 0,
            leaf_count: 1,
            commitment: "0x00".to_string(),
            mountain_height: 1,
            mountain_siblings: vec![non_canonical_hex.to_string()],
            peak_bagging_siblings: vec![],
            bagged_root: "0x00".to_string(),
            block_number: 1,
        };
        let err3 = PrivateNoteWallet::deserialize_mmr_siblings(&proof3);
        assert!(err3.is_err());
    }

    #[test]
    fn test_prepare_spend_proof_with_cached_mmr_proof() {
        let seed = [77u8; 32];
        let mut wallet = PrivateNoteWallet::new(&seed);
        let sk = wallet.spending_key().unwrap();

        // 1. Create a note and simulate on-chain MMR
        let (note, cm) = wallet.create_deposit_note(10_000_000, 1000).unwrap();
        let rho = parse_fr_from_hex(&note.rho_hex).unwrap();
        let rand = parse_fr_from_hex(&note.randomness_hex).unwrap();
        let fr_cm = nimbus_core::note_commitment(10_000_000, sk, rho, rand);

        let mut mmr = nimbus_core::MerkleMountainRange::new();
        mmr.append(fr_cm);
        let mmr_proof = mmr.generate_proof(0);
        let root = mmr.get_root();

        let m_sibs_hex: Vec<String> = mmr_proof
            .mountain_siblings
            .iter()
            .map(|s| hex::encode(fr_to_be_bytes(s)))
            .collect();
        let p_sibs_hex: Vec<String> = mmr_proof
            .peak_bagging_siblings
            .iter()
            .map(|s| hex::encode(fr_to_be_bytes(s)))
            .collect();
        let root_hex = format!("0x{}", hex::encode(fr_to_be_bytes(&root)));

        // Confirm note
        wallet
            .confirm_deposit(&cm, 0, dummy_merkle_path(), &root_hex)
            .unwrap();

        // Build MmrProofResponse
        let mmr_resp = MmrProofResponse {
            leaf_index: 0,
            leaf_count: 1,
            commitment: cm.clone(),
            mountain_height: mmr_proof.mountain_height,
            mountain_siblings: m_sibs_hex,
            peak_bagging_siblings: p_sibs_hex,
            bagged_root: root_hex.clone(),
            block_number: 100,
        };

        // Cache proof into wallet
        wallet.cache_mmr_proof(mmr_resp.clone());

        // Generate proving key
        let keys = nimbus_core::generate_note_circuit_keys().unwrap();

        let selected = wallet
            .select_note_for_spend(7_000_000, 31_500, 50_000, 1010)
            .unwrap();

        let payload = wallet
            .prepare_spend_proof_with_mmr(
                &selected,
                "session_mmr_spend_1",
                "0x1111111111111111111111111111111111111111",
                100_000,
                "0x2222222222222222222222222222222222222222222222222222222222222222",
                421614,
                "0x3333333333333333333333333333333333333333",
                2000,
                None,
                false,
                &mmr_resp,
                1010,
                &keys.proving_key,
            )
            .unwrap();

        assert_eq!(payload.note_root_hex, hex::encode(fr_to_be_bytes(&root)));
        assert_eq!(payload.leaf_count, 1);
        assert_eq!(payload.public_inputs_hex.len(), 16);
    }
}
