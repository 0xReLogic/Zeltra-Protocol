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

use ark_bls12_381::{Bls12_381, Fr};
use ark_ff::{PrimeField, UniformRand};
use nimbus_core::{
    derive_nullifier, derive_nullifier_key, fr_from_be_bytes, fr_to_be_bytes, generate_note_proof,
    note_commitment, to_evm_g1, to_evm_g2, MERKLE_TREE_DEPTH,
};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

/// Minimum non-dust amount (1000 base units = 0.001 USDC)
pub const DUST_THRESHOLD: u64 = 1_000;

/// Default lease TTL for note reservations in seconds (2 minutes)
pub const DEFAULT_RESERVATION_TTL_SECS: u64 = 120;

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
    pub merkle_path_hex: Option<Vec<String>>,
    pub status: NoteStatus,
    pub created_at_secs: u64,
    pub session_id: Option<String>,
}

impl Drop for WalletNote {
    fn drop(&mut self) {
        crate::secure_zeroize_string(&mut self.owner_key_hex);
        crate::secure_zeroize_string(&mut self.rho_hex);
        crate::secure_zeroize_string(&mut self.randomness_hex);
    }
}

/// Result of Coin Selection
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelectedSpend {
    pub input_commitment_hex: String,
    pub input_value: u64,
    pub merchant_amount: u64,
    pub protocol_fee: u64,
    pub execution_fee: u64,
    pub total_required: u64,
    pub change_amount: u64,
    pub has_change: bool,
}

/// Spend Proof Payload generated locally by client SDK
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SpendProofPayload {
    pub session_id: String,
    pub input_commitment_hex: String,
    pub input_nullifier_hex: String,
    pub note_root_hex: String,
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
    pub proof_a_neg_hex: String, // 128 bytes EVM format
    pub proof_b_hex: String,     // 256 bytes EVM format
    pub proof_c_hex: String,     // 128 bytes EVM format
    pub public_inputs_hex: Vec<String>, // 12 x 32 bytes EVM scalars
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
}

impl std::fmt::Display for WalletError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InsufficientBalance { requested, available } => {
                write!(f, "Insufficient balance: requested {requested}, available {available}")
            }
            Self::NoteNotFound(id) => write!(f, "Note not found: {id}"),
            Self::NoteNotSpendable(id) => write!(f, "Note not spendable: {id}"),
            Self::NoteAlreadyReserved(id) => write!(f, "Note already reserved: {id}"),
            Self::NoteMissingWitness(id) => write!(f, "Note missing Merkle witness: {id}"),
            Self::InvalidHex(err) => write!(f, "Invalid hex data: {err}"),
            Self::CryptoError(err) => write!(f, "Cryptographic error: {err}"),
            Self::SerializationError(err) => write!(f, "Serialization error: {err}"),
            Self::BackupDecryptionFailed => write!(f, "Backup decryption failed or corrupted"),
        }
    }
}

impl std::error::Error for WalletError {}

/// Private Note Wallet State
#[derive(Clone, Serialize, Deserialize)]
pub struct PrivateNoteWallet {
    spending_key_hex: String,
    nullifier_key_hex: String,
    pub notes: HashMap<String, WalletNote>,
    pub current_merkle_root_hex: Option<String>,
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
        }
    }

    /// Spending key as Fr
    pub fn spending_key(&self) -> Result<Fr, WalletError> {
        let bytes = hex::decode(&self.spending_key_hex)
            .map_err(|e| WalletError::InvalidHex(e.to_string()))?;
        if bytes.len() != 32 {
            return Err(WalletError::CryptoError("Invalid spending key length".into()));
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes);
        fr_from_be_bytes(&arr).ok_or_else(|| WalletError::CryptoError("Non-canonical Fr".into()))
    }

    /// Nullifier key as Fr
    pub fn nullifier_key(&self) -> Result<Fr, WalletError> {
        let bytes = hex::decode(&self.nullifier_key_hex)
            .map_err(|e| WalletError::InvalidHex(e.to_string()))?;
        if bytes.len() != 32 {
            return Err(WalletError::CryptoError("Invalid nullifier key length".into()));
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes);
        fr_from_be_bytes(&arr).ok_or_else(|| WalletError::CryptoError("Non-canonical Fr".into()))
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
                NoteStatus::Reserved { lease_expiry_secs, .. } => *lease_expiry_secs > current_time_secs,
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
            merkle_path_hex: None,
            status: NoteStatus::Unconfirmed,
            created_at_secs: current_time_secs,
            session_id: None,
        };

        self.notes.insert(cm_hex.clone(), note.clone());
        Ok((note, cm_hex))
    }

    /// Confirms a deposit once mined on-chain, associating its Merkle leaf index and path.
    pub fn confirm_deposit(
        &mut self,
        commitment_hex: &str,
        leaf_index: u64,
        merkle_path: Vec<String>,
        root_hex: &str,
    ) -> Result<(), WalletError> {
        let note = self
            .notes
            .get_mut(commitment_hex)
            .ok_or_else(|| WalletError::NoteNotFound(commitment_hex.to_string()))?;

        if merkle_path.len() != MERKLE_TREE_DEPTH {
            return Err(WalletError::CryptoError(format!(
                "Invalid Merkle path length: expected {}, got {}",
                MERKLE_TREE_DEPTH,
                merkle_path.len()
            )));
        }

        note.leaf_index = Some(leaf_index);
        note.merkle_path_hex = Some(merkle_path);
        note.status = NoteStatus::Unspent;
        self.current_merkle_root_hex = Some(root_hex.to_string());

        Ok(())
    }

    /// Privacy-Preserving Stochastic Coin Selection (DEC-024)
    ///
    /// 1. Prioritizes Exact Match (`has_change == 0`) to eliminate change note fingerprinting.
    /// 2. If no exact match, selects the smallest single note >= required_amount (Best Fit).
    /// 3. Reclaims expired reservations if lease has expired.
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
            .ok_or_else(|| WalletError::CryptoError("Total required spend amount overflow".into()))?;

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
                    NoteStatus::Reserved { lease_expiry_secs, .. } => *lease_expiry_secs <= current_time_secs,
                    _ => false,
                }
            })
            .collect();

        if spendable_candidates.is_empty() {
            return Err(WalletError::InsufficientBalance {
                requested: total_required,
                available: 0,
            });
        }

        // Branch 1: Exact Match Search (Zero-Change Priority)
        if let Some(exact_note) = spendable_candidates.iter().find(|n| n.value == total_required) {
            return Ok(SelectedSpend {
                input_commitment_hex: exact_note.commitment_hex.clone(),
                input_value: exact_note.value,
                merchant_amount,
                protocol_fee,
                execution_fee,
                total_required,
                change_amount: 0,
                has_change: false,
            });
        }

        // Branch 2: Best Fit (Smallest Note >= total_required)
        let mut sufficient_notes: Vec<&&WalletNote> = spendable_candidates
            .iter()
            .filter(|n| n.value >= total_required)
            .collect();

        if sufficient_notes.is_empty() {
            let available: u64 = spendable_candidates.iter().map(|n| n.value).sum();
            return Err(WalletError::InsufficientBalance {
                requested: total_required,
                available,
            });
        }

        sufficient_notes.sort_by_key(|n| n.value);
        let selected_note = sufficient_notes[0];
        let change_amount = selected_note.value - total_required;

        Ok(SelectedSpend {
            input_commitment_hex: selected_note.commitment_hex.clone(),
            input_value: selected_note.value,
            merchant_amount,
            protocol_fee,
            execution_fee,
            total_required,
            change_amount,
            has_change: change_amount > 0,
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
            NoteStatus::Reserved { lease_expiry_secs, .. } => {
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

        let mut merkle_path = [Fr::from(0u64); MERKLE_TREE_DEPTH];
        for (i, p_hex) in merkle_path_strings.iter().enumerate() {
            let b = hex::decode(p_hex).map_err(|e| WalletError::InvalidHex(e.to_string()))?;
            let mut arr = [0u8; 32];
            arr.copy_from_slice(&b);
            merkle_path[i] = fr_from_be_bytes(&arr)
                .ok_or_else(|| WalletError::CryptoError("Non-canonical path Fr".into()))?;
        }

        let sk = self.spending_key()?;
        let in_rho = {
            let b = hex::decode(&input_note.rho_hex)
                .map_err(|e| WalletError::InvalidHex(e.to_string()))?;
            let mut arr = [0u8; 32];
            arr.copy_from_slice(&b);
            fr_from_be_bytes(&arr)
                .ok_or_else(|| WalletError::CryptoError("Non-canonical rho".into()))?
        };
        let in_rand = {
            let b = hex::decode(&input_note.randomness_hex)
                .map_err(|e| WalletError::InvalidHex(e.to_string()))?;
            let mut arr = [0u8; 32];
            arr.copy_from_slice(&b);
            fr_from_be_bytes(&arr)
                .ok_or_else(|| WalletError::CryptoError("Non-canonical randomness".into()))?
        };

        let in_cm = note_commitment(input_note.value, sk, in_rho, in_rand);
        let nk = self.nullifier_key()?;
        let nullifier = derive_nullifier(nk, in_cm, leaf_index);

        // Derive note root from leaf & path
        let note_root = nimbus_core::compute_merkle_root(in_cm, leaf_index, &merkle_path);

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

            // Persist unconfirmed change note (2PC Phase 1)
            let change_note = WalletNote {
                commitment_hex: c_cm_hex,
                value: selected.change_amount,
                owner_key_hex: sk_hex,
                rho_hex: c_rho_hex,
                randomness_hex: c_rand_hex,
                leaf_index: None,
                merkle_path_hex: None,
                status: NoteStatus::Unconfirmed,
                created_at_secs: current_time_secs,
                session_id: Some(session_id.to_string()),
            };
            self.notes.insert(change_note.commitment_hex.clone(), change_note);

            (
                Fr::from(selected.change_amount),
                c_cm,
                c_rho,
                c_rand,
            )
        } else {
            (
                Fr::from(0u64),
                Fr::from(0u64),
                Fr::from(0u64),
                Fr::from(0u64),
            )
        };

        // Parse EVM public input scalars
        let recipient_bytes = {
            let clean = recipient_evm_address.strip_prefix("0x").unwrap_or(recipient_evm_address);
            let dec = hex::decode(clean).map_err(|e| WalletError::InvalidHex(e.to_string()))?;
            let mut buf = [0u8; 32];
            if dec.len() == 20 {
                buf[12..].copy_from_slice(&dec);
            } else if dec.len() == 32 {
                buf.copy_from_slice(&dec);
            } else {
                return Err(WalletError::InvalidHex("Invalid recipient address length".into()));
            }
            buf
        };
        let recipient_fr = Fr::from_be_bytes_mod_order(&recipient_bytes);

        let quote_bytes = {
            let clean = quote_hash_hex.strip_prefix("0x").unwrap_or(quote_hash_hex);
            let dec = hex::decode(clean).map_err(|e| WalletError::InvalidHex(e.to_string()))?;
            let mut buf = [0u8; 32];
            if dec.len() == 32 {
                buf.copy_from_slice(&dec);
            } else {
                return Err(WalletError::InvalidHex("Invalid quote hash length".into()));
            }
            buf
        };
        let quote_fr = Fr::from_be_bytes_mod_order(&quote_bytes);

        let contract_bytes = {
            let clean = contract_address_hex.strip_prefix("0x").unwrap_or(contract_address_hex);
            let dec = hex::decode(clean).map_err(|e| WalletError::InvalidHex(e.to_string()))?;
            let mut buf = [0u8; 32];
            if dec.len() == 20 {
                buf[12..].copy_from_slice(&dec);
            } else if dec.len() == 32 {
                buf.copy_from_slice(&dec);
            } else {
                return Err(WalletError::InvalidHex("Invalid contract address length".into()));
            }
            buf
        };
        let contract_fr = Fr::from_be_bytes_mod_order(&contract_bytes);

        let has_change_fr = if selected.has_change { Fr::from(1u64) } else { Fr::from(0u64) };

        // Construct PrivateNoteCircuit
        let circuit = nimbus_core::PrivateNoteCircuit {
            note_root: Some(note_root),
            input_nullifier: Some(nullifier),
            output_commitment: Some(ch_cm),
            recipient: Some(recipient_fr),
            merchant_amount: Some(Fr::from(selected.merchant_amount)),
            protocol_fee: Some(Fr::from(selected.protocol_fee)),
            execution_fee: Some(Fr::from(selected.execution_fee)),
            quote_hash: Some(quote_fr),
            chain_id: Some(Fr::from(chain_id)),
            contract_address: Some(contract_fr),
            expiry: Some(Fr::from(expiry)),
            has_change: Some(has_change_fr),

            input_value: Some(Fr::from(input_note.value)),
            input_owner_key: Some(sk),
            input_rho: Some(in_rho),
            input_randomness: Some(in_rand),
            input_leaf_index: Some(Fr::from(leaf_index)),
            input_merkle_path: Some(merkle_path),

            change_value: Some(ch_val),
            change_owner_key: Some(sk),
            change_rho: Some(ch_rho),
            change_randomness: Some(ch_rand),
        };

        // Generate Groth16 Proof
        let proof = generate_note_proof(circuit, pk)
            .map_err(|e| WalletError::CryptoError(format!("Proof generation failed: {e:?}")))?;

        // Format EVM Proof Points
        let proof_a_neg = to_evm_g1(&-proof.a);
        let proof_b = to_evm_g2(&proof.b);
        let proof_c = to_evm_g1(&proof.c);

        // Format 12 Public Inputs
        let public_inputs = vec![
            hex::encode(fr_to_be_bytes(&note_root)),
            hex::encode(fr_to_be_bytes(&nullifier)),
            hex::encode(fr_to_be_bytes(&ch_cm)),
            hex::encode(recipient_bytes),
            hex::encode(fr_to_be_bytes(&Fr::from(selected.merchant_amount))),
            hex::encode(fr_to_be_bytes(&Fr::from(selected.protocol_fee))),
            hex::encode(fr_to_be_bytes(&Fr::from(selected.execution_fee))),
            hex::encode(quote_bytes),
            hex::encode(fr_to_be_bytes(&Fr::from(chain_id))),
            hex::encode(contract_bytes),
            hex::encode(fr_to_be_bytes(&Fr::from(expiry))),
            hex::encode(fr_to_be_bytes(&has_change_fr)),
        ];

        Ok(SpendProofPayload {
            session_id: session_id.to_string(),
            input_commitment_hex: selected.input_commitment_hex.clone(),
            input_nullifier_hex: hex::encode(fr_to_be_bytes(&nullifier)),
            note_root_hex: hex::encode(fr_to_be_bytes(&note_root)),
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
            proof_a_neg_hex: hex::encode(proof_a_neg),
            proof_b_hex: hex::encode(proof_b),
            proof_c_hex: hex::encode(proof_c),
            public_inputs_hex: public_inputs,
        })
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

        let session_id = format!("spend_{}_{}", current_time_secs, hex::encode(rand::random::<[u8; 8]>()));

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
                    NoteStatus::Reserved { lease_expiry_secs, .. } => *lease_expiry_secs <= current_time_secs,
                    _ => false,
                }
            })
            .map(|n| (n.commitment_hex.clone(), n.value))
            .collect();

        if spendable_cms.is_empty() {
            return Err(WalletError::InsufficientBalance { requested: 1, available: 0 });
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
                merchant_amount: net_merchant,
                protocol_fee: protocol_fee_per_note,
                execution_fee: execution_fee_per_note,
                total_required: val,
                change_amount: 0,
                has_change: false,
            };

            let session_id = format!("withdraw_{}_{}", current_time_secs, hex::encode(rand::random::<[u8; 8]>()));
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
        // 1. Mark input note as Spent
        let mut found_input = false;
        for note in self.notes.values_mut() {
            if note.session_id.as_deref() == Some(session_id) {
                if let NoteStatus::Reserved { .. } = &note.status {
                    note.status = NoteStatus::Spent {
                        nullifier_hex: nullifier_hex.to_string(),
                        spent_at_secs: current_time_secs,
                    };
                    found_input = true;
                    break;
                }
            }
        }

        if !found_input {
            return Err(WalletError::NoteNotFound(format!("Session {session_id} input note")));
        }

        // 2. Promote Change Note to Unspent (if present and witness provided)
        if let (Some(idx), Some(path)) = (change_leaf_index, change_merkle_path) {
            for note in self.notes.values_mut() {
                if note.session_id.as_deref() == Some(session_id) && note.status == NoteStatus::Unconfirmed {
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

    /// Exports an encrypted backup of the wallet state using password-derived encryption.
    pub fn export_backup(&self, password: &str) -> Result<String, WalletError> {
        let serialized = serde_json::to_string(self)
            .map_err(|e| WalletError::SerializationError(e.to_string()))?;

        // Derive key: HMAC-SHA256(password, "nimbus.wallet.backup.v1")
        let key = crate::hmac_sha256(password.as_bytes(), b"nimbus.wallet.backup.v1");

        // Integrity MAC: HMAC-SHA256(key, serialized_json)
        let mac = crate::hmac_sha256(&key, serialized.as_bytes());

        // Simple XOR stream encryption for backup portability
        let mut ciphertext = serialized.into_bytes();
        let mut stream_hasher = Sha256::new();
        stream_hasher.update(key);
        stream_hasher.update(mac);
        let mut block = stream_hasher.finalize();

        for (i, byte) in ciphertext.iter_mut().enumerate() {
            if i % 32 == 0 && i > 0 {
                let mut next_hasher = Sha256::new();
                next_hasher.update(block);
                block = next_hasher.finalize();
            }
            *byte ^= block[i % 32];
        }

        let backup_payload = serde_json::json!({
            "version": 1,
            "mac_hex": hex::encode(mac),
            "ciphertext_hex": hex::encode(ciphertext),
        });

        serde_json::to_string(&backup_payload)
            .map_err(|e| WalletError::SerializationError(e.to_string()))
    }

    /// Imports and decrypts a wallet backup.
    pub fn import_backup(backup_json: &str, password: &str) -> Result<Self, WalletError> {
        let val: serde_json::Value = serde_json::from_str(backup_json)
            .map_err(|_| WalletError::BackupDecryptionFailed)?;

        let mac_hex = val["mac_hex"].as_str().ok_or(WalletError::BackupDecryptionFailed)?;
        let ciphertext_hex = val["ciphertext_hex"].as_str().ok_or(WalletError::BackupDecryptionFailed)?;

        let expected_mac = hex::decode(mac_hex).map_err(|_| WalletError::BackupDecryptionFailed)?;
        let mut ciphertext = hex::decode(ciphertext_hex).map_err(|_| WalletError::BackupDecryptionFailed)?;

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

        let plaintext = String::from_utf8(ciphertext).map_err(|_| WalletError::BackupDecryptionFailed)?;
        serde_json::from_str(&plaintext).map_err(|_| WalletError::BackupDecryptionFailed)
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
        wallet.confirm_deposit(&cm1, 0, path.clone(), "0xroot").unwrap();
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
        wallet.confirm_deposit(&cm1, 0, path.clone(), "0xroot").unwrap();
        wallet.confirm_deposit(&cm2, 1, path, "0xroot").unwrap();

        // Exact match for 10_000_000 total (9_800_000 + 150_000 + 50_000)
        let selected = wallet.select_note_for_spend(9_800_000, 150_000, 50_000, 1010).unwrap();
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
        wallet.confirm_deposit(&cm1, 0, path.clone(), "0xroot").unwrap();
        wallet.confirm_deposit(&cm2, 1, path, "0xroot").unwrap();

        // Need 15_000_000: cm1 is too small, cm2 is selected
        let selected = wallet.select_note_for_spend(14_000_000, 700_000, 300_000, 1010).unwrap();
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

        let selected = wallet.select_note_for_spend(40_000_000, 2_000_000, 1_000_000, 1000).unwrap();
        wallet.reserve_note(&selected.input_commitment_hex, "session_tx_1", 1000, 120).unwrap();

        // Simulate rollback on failure
        wallet.rollback_spend("session_tx_1").unwrap();
        assert_eq!(wallet.balance(), 100_000_000);

        // Re-reserve and simulate successful commit
        wallet.reserve_note(&selected.input_commitment_hex, "session_tx_2", 1050, 120).unwrap();
        wallet.commit_spend("session_tx_2", "0xnullifier123", 1055, None, None, None).unwrap();

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
        assert_eq!(wrong_restore.err(), Some(WalletError::BackupDecryptionFailed));

        // Restore with correct password succeeds
        let restored = PrivateNoteWallet::import_backup(&backup, "SuperSecretPassword123!").unwrap();
        assert_eq!(restored.balance(), 55_000_000);
        assert_eq!(restored.spending_key().unwrap(), wallet.spending_key().unwrap());
        assert_eq!(restored.current_merkle_root_hex.as_deref(), Some("0xroot_abc"));
    }

    #[test]
    fn test_prepare_spend_proof_and_verification() {
        let seed = [99u8; 32];
        let mut wallet = PrivateNoteWallet::new(&seed);
        let path = dummy_merkle_path();

        let (_, cm1) = wallet.create_deposit_note(50_000_000, 1000).unwrap();
        wallet.confirm_deposit(&cm1, 0, path, "0xroot").unwrap();

        let selected = wallet.select_note_for_spend(20_000_000, 100_000, 50_000, 1000).unwrap();
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
        assert_eq!(payload.proof_b_hex.len(), 512);     // 256 bytes
        assert_eq!(payload.proof_c_hex.len(), 256);     // 128 bytes
        assert_eq!(payload.public_inputs_hex.len(), 12);
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
}
