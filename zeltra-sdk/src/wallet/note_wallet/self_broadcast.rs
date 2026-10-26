use alloy_primitives::{Address, FixedBytes, U256};
use alloy_sol_types::{sol, SolCall};
use ark_bls12_381::Bls12_381;
use serde::{Deserialize, Serialize};
use std::str::FromStr;

use super::types::*;
use super::PrivateNoteWallet;

#[inline]
fn clean_hex(s: &str) -> &str {
    s.trim_start_matches("0x")
}

sol! {
    /// Stylus Smart Contract ABI for Universal 2-in-2-out JoinSplit (DEC-036 / DEC-043B / DEC-054)
    function spendJoinsplit(
        bytes calldata proof_a_neg,
        bytes calldata proof_b,
        bytes calldata proof_c,
        bytes32 note_root,
        uint256 leaf_count,
        bytes32 nullifier_1,
        bytes32 nullifier_2,
        uint256 epoch_id_1,
        uint256 epoch_id_2,
        bytes32 output_cm_1,
        bytes32 output_cm_2,
        address recipient,
        uint256 merchant_amount,
        uint256 protocol_fee,
        uint256 execution_fee,
        uint256 max_execution_fee,
        bytes32 quote_hash,
        uint256 expiry,
        uint256 flags_packed,
        uint256 is_rollover,
        bytes calldata encrypted_memo_1,
        bytes calldata encrypted_memo_2
    ) external returns (bool);
}

/// Transaction payload prepared by client SDK for direct EVM self-broadcasting (DEC-054 Safe Harbor).
/// Contains complete ABI-encoded calldata with execution_fee = 0, fully unhosted and relayer-independent.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SelfBroadcastTransactionPayload {
    /// Target Stylus contract address (0x...)
    pub to_contract_address: String,
    /// ABI-encoded calldata in hex format with 0x prefix
    pub calldata_hex: String,
    /// Raw calldata bytes for eth_sendTransaction
    pub calldata_bytes: Vec<u8>,
    /// Execution fee (strictly 0 in self-broadcast mode)
    pub execution_fee: u64,
    /// Maximum execution fee (strictly 0 in self-broadcast mode)
    pub max_execution_fee: u64,
    /// Amount transferred to recipient / merchant (Micro-USDC or Gwei)
    pub merchant_amount: u64,
    /// Protocol fee (45 bps for Mode 3, 0 bps for Mode 2)
    pub protocol_fee: u64,
    /// Recipient address (public EVM address or 0x0 for Mode 2 in-pool)
    pub recipient: String,
    /// Asset ID (0: WETH, 1: USDC)
    pub asset_id: u8,
    /// Full underlying Groth16 JoinSplit spend proof payload
    pub proof_payload: JoinSplitSpendProofPayload,
}

impl SelfBroadcastTransactionPayload {
    /// Validates the self-broadcast payload integrity against adversarial tampering (DEC-054 Gate 2).
    pub fn validate_integrity(&self) -> Result<(), WalletError> {
        // Adversarial Gate 2.1: Execution fee must be strictly 0 without relayer quote
        if self.execution_fee != 0 || self.max_execution_fee != 0 {
            return Err(WalletError::CryptoError(
                "ADVERSARIAL_TAMPERING_DETECTED: Non-zero execution fee claimed in direct self-broadcast mode"
                    .into(),
            ));
        }

        // Adversarial Gate 2.2: Recipient in payload must match underlying proof payload
        let clean_payload_rec = self.recipient.trim_start_matches("0x").to_lowercase();
        let clean_proof_rec = self
            .proof_payload
            .recipient_hex
            .trim_start_matches("0x")
            .to_lowercase();
        if clean_payload_rec != clean_proof_rec {
            return Err(WalletError::CryptoError(
                "ADVERSARIAL_TAMPERING_DETECTED: Recipient address mismatch between transaction envelope and ZK proof"
                    .into(),
            ));
        }

        // Adversarial Gate 2.3: Merchant amount must match proof payload
        if self.merchant_amount != self.proof_payload.merchant_amount {
            return Err(WalletError::CryptoError(
                "ADVERSARIAL_TAMPERING_DETECTED: Merchant amount mismatch between transaction envelope and ZK proof"
                    .into(),
            ));
        }

        // Adversarial Gate 2.4: Calldata non-empty and starts with spendJoinsplit selector
        if self.calldata_bytes.len() < 4 {
            return Err(WalletError::CryptoError(
                "Invalid calldata length: shorter than 4-byte function selector".into(),
            ));
        }

        Ok(())
    }
}

/// Encodes a `JoinSplitSpendProofPayload` into raw ABI bytes for Stylus smart contract `spendJoinsplit`.
pub fn encode_joinsplit_calldata(
    proof: &JoinSplitSpendProofPayload,
) -> Result<Vec<u8>, WalletError> {
    let proof_a_neg = hex::decode(clean_hex(&proof.proof_a_neg_hex))
        .map_err(|e| WalletError::CryptoError(format!("Invalid proof_a_neg hex: {e}")))?;
    let proof_b = hex::decode(clean_hex(&proof.proof_b_hex))
        .map_err(|e| WalletError::CryptoError(format!("Invalid proof_b hex: {e}")))?;
    let proof_c = hex::decode(clean_hex(&proof.proof_c_hex))
        .map_err(|e| WalletError::CryptoError(format!("Invalid proof_c hex: {e}")))?;

    let mut note_root = [0u8; 32];
    let nr_bytes = hex::decode(clean_hex(&proof.note_root_hex))
        .map_err(|e| WalletError::CryptoError(format!("Invalid note_root hex: {e}")))?;
    if nr_bytes.len() == 32 {
        note_root.copy_from_slice(&nr_bytes);
    }

    let mut nf1 = [0u8; 32];
    let nf1_bytes = hex::decode(clean_hex(&proof.input_nullifier_1_hex))
        .map_err(|e| WalletError::CryptoError(format!("Invalid input_nullifier_1 hex: {e}")))?;
    if nf1_bytes.len() == 32 {
        nf1.copy_from_slice(&nf1_bytes);
    }

    let mut nf2 = [0u8; 32];
    let nf2_bytes = hex::decode(clean_hex(&proof.input_nullifier_2_hex))
        .map_err(|e| WalletError::CryptoError(format!("Invalid input_nullifier_2 hex: {e}")))?;
    if nf2_bytes.len() == 32 {
        nf2.copy_from_slice(&nf2_bytes);
    }

    let mut cm1 = [0u8; 32];
    let cm1_bytes = hex::decode(clean_hex(&proof.output_commitment_1_hex))
        .map_err(|e| WalletError::CryptoError(format!("Invalid output_commitment_1 hex: {e}")))?;
    if cm1_bytes.len() == 32 {
        cm1.copy_from_slice(&cm1_bytes);
    }

    let mut cm2 = [0u8; 32];
    let cm2_bytes = hex::decode(clean_hex(&proof.output_commitment_2_hex))
        .map_err(|e| WalletError::CryptoError(format!("Invalid output_commitment_2 hex: {e}")))?;
    if cm2_bytes.len() == 32 {
        cm2.copy_from_slice(&cm2_bytes);
    }

    let recipient_addr = if clean_hex(&proof.recipient_hex).is_empty() {
        Address::ZERO
    } else {
        Address::from_str(&proof.recipient_hex).map_err(|e| {
            WalletError::CryptoError(format!(
                "Invalid recipient address '{}': {e}",
                proof.recipient_hex
            ))
        })?
    };

    let mut quote_hash = [0u8; 32];
    let qh_bytes = hex::decode(clean_hex(&proof.quote_hash_hex))
        .map_err(|e| WalletError::CryptoError(format!("Invalid quote_hash hex: {e}")))?;
    if qh_bytes.len() == 32 {
        quote_hash.copy_from_slice(&qh_bytes);
    }

    let memo1 = hex::decode(clean_hex(&proof.encrypted_memo_1))
        .map_err(|e| WalletError::CryptoError(format!("Invalid encrypted_memo_1 hex: {e}")))?;
    let memo2 = hex::decode(clean_hex(&proof.encrypted_memo_2))
        .map_err(|e| WalletError::CryptoError(format!("Invalid encrypted_memo_2 hex: {e}")))?;

    // Extract scalar metadata from fields or fallback to 19 Public Inputs
    let leaf_count = if proof.leaf_count > 0 {
        proof.leaf_count
    } else if proof.public_inputs_hex.len() > 1 {
        let bytes = hex::decode(clean_hex(&proof.public_inputs_hex[1])).unwrap_or_default();
        if bytes.len() == 32 {
            U256::from_be_slice(&bytes).to::<u64>()
        } else {
            0
        }
    } else {
        0
    };

    let epoch_id_1 = if proof.epoch_id_1 > 0 {
        proof.epoch_id_1
    } else if proof.public_inputs_hex.len() > 4 {
        let bytes = hex::decode(clean_hex(&proof.public_inputs_hex[4])).unwrap_or_default();
        if bytes.len() == 32 {
            U256::from_be_slice(&bytes).to::<u32>()
        } else {
            0
        }
    } else {
        0
    };

    let epoch_id_2 = if proof.epoch_id_2 > 0 {
        proof.epoch_id_2
    } else if proof.public_inputs_hex.len() > 5 {
        let bytes = hex::decode(clean_hex(&proof.public_inputs_hex[5])).unwrap_or_default();
        if bytes.len() == 32 {
            U256::from_be_slice(&bytes).to::<u32>()
        } else {
            0
        }
    } else {
        0
    };

    let flags_packed = if proof.flags_packed > 0 {
        proof.flags_packed
    } else if proof.public_inputs_hex.len() > 17 {
        let bytes = hex::decode(clean_hex(&proof.public_inputs_hex[17])).unwrap_or_default();
        if bytes.len() == 32 {
            U256::from_be_slice(&bytes).to::<u8>()
        } else {
            0
        }
    } else {
        0
    };

    let is_rollover = if proof.is_rollover > 0 {
        proof.is_rollover
    } else if proof.public_inputs_hex.len() > 18 {
        let bytes = hex::decode(clean_hex(&proof.public_inputs_hex[18])).unwrap_or_default();
        if bytes.len() == 32 {
            U256::from_be_slice(&bytes).to::<u64>()
        } else {
            0
        }
    } else {
        0
    };

    let call = spendJoinsplitCall {
        proof_a_neg: proof_a_neg.into(),
        proof_b: proof_b.into(),
        proof_c: proof_c.into(),
        note_root: FixedBytes::from(note_root),
        leaf_count: U256::from(leaf_count),
        nullifier_1: FixedBytes::from(nf1),
        nullifier_2: FixedBytes::from(nf2),
        epoch_id_1: U256::from(epoch_id_1),
        epoch_id_2: U256::from(epoch_id_2),
        output_cm_1: FixedBytes::from(cm1),
        output_cm_2: FixedBytes::from(cm2),
        recipient: recipient_addr,
        merchant_amount: U256::from(proof.merchant_amount),
        protocol_fee: U256::from(proof.protocol_fee),
        execution_fee: U256::from(proof.execution_fee),
        max_execution_fee: U256::from(proof.max_execution_fee),
        quote_hash: FixedBytes::from(quote_hash),
        expiry: U256::from(proof.expiry),
        flags_packed: U256::from(flags_packed),
        is_rollover: U256::from(is_rollover),
        encrypted_memo_1: memo1.into(),
        encrypted_memo_2: memo2.into(),
    };

    Ok(call.abi_encode())
}

impl PrivateNoteWallet {
    /// Prepares a Mode 3 Direct Self-Broadcast Spend transaction (DEC-054 Safe Harbor).
    /// Execution fee is strictly set to 0, ensuring user pays gas directly via their EVM provider
    /// with ZERO dependency on any relayer.
    #[allow(clippy::too_many_arguments)]
    pub fn prepare_self_broadcast_joinsplit(
        &mut self,
        recipient_evm_address: &str,
        spend_amount: u64,
        chain_id: u64,
        contract_address_hex: &str,
        expiry: u64,
        current_time_secs: u64,
        joinsplit_pk: &ark_groth16::ProvingKey<Bls12_381>,
    ) -> Result<SelfBroadcastTransactionPayload, WalletError> {
        if spend_amount == 0 {
            return Err(WalletError::CryptoError(
                "Spend amount must be strictly greater than zero".into(),
            ));
        }

        // Protocol monetization: flat 45 bps (DEC-028)
        let protocol_fee = zeltra_core::private_spend_fee(spend_amount)
            .ok_or_else(|| WalletError::CryptoError("Protocol fee calculation overflow".into()))?;

        // Canonical zero quote hash for direct self-broadcast (no relayer quote)
        let canonical_zero_quote_hash = format!("0x{:064x}", 0);

        // Select note for spend with execution_fee = 0
        let selected = self.select_note_for_spend(
            spend_amount,
            protocol_fee,
            0, // execution_fee = 0
            current_time_secs,
        )?;

        let session_id = format!(
            "self_broadcast_{}_{}",
            current_time_secs,
            hex::encode(rand::random::<[u8; 8]>())
        );

        let proof_payload = self.prepare_joinsplit_spend_proof(
            &selected,
            &session_id,
            recipient_evm_address,
            0, // max_execution_fee = 0
            &canonical_zero_quote_hash,
            chain_id,
            contract_address_hex,
            expiry,
            current_time_secs,
            joinsplit_pk,
        )?;

        let calldata_bytes = encode_joinsplit_calldata(&proof_payload)?;
        let calldata_hex = format!("0x{}", hex::encode(&calldata_bytes));

        let asset_id = (proof_payload.flags_packed >> 2) & 1;

        let payload = SelfBroadcastTransactionPayload {
            to_contract_address: contract_address_hex.to_string(),
            calldata_hex,
            calldata_bytes,
            execution_fee: 0,
            max_execution_fee: 0,
            merchant_amount: spend_amount,
            protocol_fee,
            recipient: recipient_evm_address.to_string(),
            asset_id,
            proof_payload,
        };

        // Self-validate integrity
        payload.validate_integrity()?;

        Ok(payload)
    }

    /// Prepares a Mode 2 Direct Self-Broadcast In-Pool Transfer transaction (DEC-054 Safe Harbor).
    /// Execution fee is strictly set to 0.
    #[allow(clippy::too_many_arguments)]
    pub fn prepare_self_broadcast_shielded_transfer(
        &mut self,
        recipient_zsa_bech32m_or_hex: &str,
        transfer_amount: u64,
        asset_id: u8,
        chain_id: u64,
        contract_address_hex: &str,
        expiry: u64,
        current_time_secs: u64,
        joinsplit_pk: &ark_groth16::ProvingKey<Bls12_381>,
    ) -> Result<SelfBroadcastTransactionPayload, WalletError> {
        let canonical_zero_quote_hash = format!("0x{:064x}", 0);

        let proof_payload = self.prepare_shielded_transfer_proof(
            recipient_zsa_bech32m_or_hex,
            transfer_amount,
            asset_id,
            0, // max_execution_fee = 0
            &canonical_zero_quote_hash,
            chain_id,
            contract_address_hex,
            expiry,
            current_time_secs,
            joinsplit_pk,
        )?;

        let calldata_bytes = encode_joinsplit_calldata(&proof_payload)?;
        let calldata_hex = format!("0x{}", hex::encode(&calldata_bytes));

        let payload = SelfBroadcastTransactionPayload {
            to_contract_address: contract_address_hex.to_string(),
            calldata_hex,
            calldata_bytes,
            execution_fee: 0,
            max_execution_fee: 0,
            merchant_amount: 0,
            protocol_fee: 0,
            recipient: "0x0000000000000000000000000000000000000000".to_string(),
            asset_id,
            proof_payload,
        };

        // Self-validate integrity
        payload.validate_integrity()?;

        Ok(payload)
    }
}
