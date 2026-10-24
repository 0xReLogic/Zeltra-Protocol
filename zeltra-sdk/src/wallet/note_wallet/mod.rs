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
use ark_bls12_381::Fr;
use ark_ff::PrimeField;
use chacha20poly1305::{
    aead::{Aead, KeyInit},
    ChaCha20Poly1305, Nonce,
};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use zeltra_core::{derive_nullifier_key, derive_viewing_keys, fr_to_be_bytes};

use crate::compliance::AuditRecord;

pub mod compliance;
pub mod deposit;
pub mod joinsplit;
pub mod keys;
pub mod spend;
pub mod state_2pc;
pub mod sync;
pub mod types;

#[cfg(test)]
mod tests;

pub use compliance::{
    build_audit_report_inner, check_nullifiers_spent_batch, get_or_init_disclosure_keys,
    recover_from_viewing_key, recover_from_viewing_key_batch, ReadOnlyAuditorSession,
};
pub use types::*;

/// Private Note Wallet State
#[derive(Clone, Serialize, Deserialize)]
pub struct PrivateNoteWallet {
    pub(crate) spending_key_hex: String,
    pub(crate) nullifier_key_hex: String,
    #[serde(default)]
    pub(crate) incoming_viewing_key_hex: String,
    #[serde(default)]
    pub(crate) incoming_viewing_public_key_hex: String,
    #[serde(default)]
    pub(crate) outcoming_viewing_key_hex: String,
    pub notes: HashMap<String, WalletNote>,
    #[serde(default)]
    pub usdc_notes: Vec<PrivateNoteV1>,
    #[serde(default)]
    pub weth_notes: Vec<PrivateNoteV1>,
    #[serde(default)]
    pub audit_records: Vec<AuditRecord>,
    pub current_merkle_root_hex: Option<String>,
    #[serde(skip)]
    pub mmr: zeltra_core::MerkleMountainRange,
    #[serde(skip)]
    pub mmr_proof_cache: HashMap<(u64, String), MmrProofResponse>,
}

impl Drop for PrivateNoteWallet {
    fn drop(&mut self) {
        crate::secure_zeroize_string(&mut self.spending_key_hex);
        crate::secure_zeroize_string(&mut self.nullifier_key_hex);
        crate::secure_zeroize_string(&mut self.incoming_viewing_key_hex);
        crate::secure_zeroize_string(&mut self.outcoming_viewing_key_hex);
    }
}

impl PrivateNoteWallet {
    /// Initialize a new wallet deterministically from a 32-byte master seed.
    ///
    /// Derives spending key, nullifier key, and viewing keys using domain separation (DEC-024, DEC-037).
    pub fn new(seed: &[u8; 32]) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(b"zeltra.wallet.seed.v1");
        hasher.update(seed);
        let sk_bytes = hasher.finalize();

        let sk_fr = Fr::from_be_bytes_mod_order(&sk_bytes);
        let nk_fr = derive_nullifier_key(sk_fr);

        let sk_hex = hex::encode(fr_to_be_bytes(&sk_fr));
        let nk_hex = hex::encode(fr_to_be_bytes(&nk_fr));

        let (ivk, ovk) = derive_viewing_keys(seed);
        let ivk_hex = hex::encode(ivk.secret_bytes());
        let ivk_pub_hex = hex::encode(ivk.public_key());
        let ovk_hex = hex::encode(ovk.as_bytes());

        Self {
            spending_key_hex: sk_hex,
            nullifier_key_hex: nk_hex,
            incoming_viewing_key_hex: ivk_hex,
            incoming_viewing_public_key_hex: ivk_pub_hex,
            outcoming_viewing_key_hex: ovk_hex,
            notes: HashMap::new(),
            usdc_notes: Vec::new(),
            weth_notes: Vec::new(),
            audit_records: Vec::new(),
            current_merkle_root_hex: None,
            mmr: zeltra_core::MerkleMountainRange::new(),
            mmr_proof_cache: HashMap::new(),
        }
    }

    /// Returns total active balance per asset ID (DEC-034C / DEC-051).
    /// - `asset_id == 0`: WETH total balance in Gwei (9 decimals)
    /// - `asset_id == 1`: USDC total balance in Micro-USDC (6 decimals)
    pub fn get_balance(&self, asset_id: u8) -> Result<u64, WalletError> {
        match asset_id {
            0 => {
                let from_notes: u64 = self
                    .notes
                    .values()
                    .filter(|n| n.status == NoteStatus::Unspent && n.asset_id == 0 && !n.is_decoy())
                    .map(|n| n.value)
                    .sum();
                if from_notes > 0 {
                    Ok(from_notes)
                } else {
                    Ok(self.weth_notes.iter().map(|n| n.value).sum())
                }
            }
            1 => {
                let from_notes: u64 = self
                    .notes
                    .values()
                    .filter(|n| n.status == NoteStatus::Unspent && n.asset_id == 1 && !n.is_decoy())
                    .map(|n| n.value)
                    .sum();
                if from_notes > 0 {
                    Ok(from_notes)
                } else {
                    Ok(self.usdc_notes.iter().map(|n| n.value).sum())
                }
            }
            _ => Err(WalletError::UnsupportedAssetId(asset_id)),
        }
    }

    /// Returns human-readable formatted balance string (DEC-034C).
    /// - `asset_id == 0`: format as "{:.6} ETH"
    /// - `asset_id == 1`: format as "${:.2} USDC"
    pub fn get_human_balance(&self, asset_id: u8) -> Result<String, WalletError> {
        let raw = self.get_balance(asset_id)?;
        match asset_id {
            0 => Ok(format!("{:.6} ETH", raw as f64 / 1_000_000_000.0)),
            1 => Ok(format!("${:.2} USDC", raw as f64 / 1_000_000.0)),
            _ => Err(WalletError::UnsupportedAssetId(asset_id)),
        }
    }

    /// Returns the exact unspent balance in base integer units for the specified asset (excluding decoy dummy notes).
    pub fn get_unspent_balance(&self, asset_id: u8) -> u64 {
        let from_notes: u64 = self
            .notes
            .values()
            .filter(|n| n.status == NoteStatus::Unspent && n.asset_id == asset_id && !n.is_decoy())
            .map(|n| n.value)
            .sum();
        if from_notes > 0 {
            from_notes
        } else {
            self.get_balance(asset_id).unwrap_or(0)
        }
    }

    /// Aggregate balance of all fully confirmed, spendable notes (excluding decoy dummy notes).
    pub fn balance(&self) -> u64 {
        self.notes
            .values()
            .filter(|n| n.status == NoteStatus::Unspent && !n.is_decoy())
            .map(|n| n.value)
            .sum()
    }

    /// Total spendable balance across all assets strictly excluding DecoyDummy notes (DEC-051).
    pub fn spendable_balance(&self) -> u64 {
        self.balance()
    }

    /// Spendable balance for a specific asset strictly excluding DecoyDummy notes (DEC-051).
    pub fn spendable_balance_for_asset(&self, asset_id: u8) -> u64 {
        self.get_unspent_balance(asset_id)
    }

    /// Generates a cryptographically indistinguishable decoy dummy note (v = 0, DEC-051 / Nopenena).
    pub fn generate_decoy_dummy_note(
        &self,
        asset_id: u8,
    ) -> Result<(WalletNote, [u8; 96]), WalletError> {
        use ark_ff::UniformRand;
        let mut rng = rand::rngs::OsRng;
        let rho_dummy = Fr::rand(&mut rng);
        let r_dummy = Fr::rand(&mut rng);
        let sk = self.spending_key()?;
        let cm = zeltra_core::note_commitment(0, asset_id, sk, rho_dummy, r_dummy);

        let sender_pk_ivk = self.incoming_viewing_public_key()?;
        let (memo_bytes, _r) = zeltra_core::encrypt_committing_note_memo(
            0,
            rho_dummy,
            asset_id as u64,
            &sender_pk_ivk,
        )
        .map_err(|e| WalletError::CryptoError(format!("Decoy memo encryption failed: {e}")))?;

        let memo_arr: [u8; 96] = memo_bytes
            .try_into()
            .map_err(|_| WalletError::CryptoError("Decoy memo byte length must be 96".into()))?;

        let note = WalletNote {
            commitment_hex: hex::encode(fr_to_be_bytes(&cm)),
            value: 0,
            asset_id,
            owner_key_hex: hex::encode(fr_to_be_bytes(&sk)),
            rho_hex: hex::encode(fr_to_be_bytes(&rho_dummy)),
            randomness_hex: hex::encode(fr_to_be_bytes(&r_dummy)),
            leaf_index: None,
            leaf_count: None,
            epoch_id: Some(0),
            merkle_path_hex: None,
            status: NoteStatus::Unconfirmed,
            classification: NoteClassification::DecoyDummy,
            created_at_secs: 0,
            session_id: None,
        };

        Ok((note, memo_arr))
    }

    /// Balance of notes currently locked in active spend reservations (excluding decoy dummy notes).
    pub fn reserved_balance(&self, current_time_secs: u64) -> u64 {
        self.notes
            .values()
            .filter(|n| {
                if n.is_decoy() {
                    return false;
                }
                match &n.status {
                    NoteStatus::Reserved {
                        lease_expiry_secs, ..
                    } => *lease_expiry_secs > current_time_secs,
                    _ => false,
                }
            })
            .map(|n| n.value)
            .sum()
    }

    /// Total balance (unspent + currently reserved).
    pub fn total_balance(&self, current_time_secs: u64) -> u64 {
        self.balance() + self.reserved_balance(current_time_secs)
    }

    /// Calculates the effective spendable balance, treating expired reservations as unspent (DEC-027).
    pub fn effective_balance(&self, current_time_secs: u64) -> u64 {
        self.notes
            .values()
            .filter(|n| {
                if n.is_decoy() {
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

            let key = crate::hmac_sha256(password.as_bytes(), b"zeltra.wallet.backup.v1");

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
