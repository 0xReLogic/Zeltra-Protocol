//! Preflight validation logic for private note and JoinSplit spends (DEC-044)

use alloy_primitives::Address;
use std::str::FromStr;

use crate::dto::{
    JoinSplitSpendRequest, JoinSplitSpendResponse, PrivateNoteSpendRequest,
    PrivateNoteSpendResponse,
};
use crate::state::AppState;

pub struct ValidatedJoinSplitPreflight {
    pub parsed_inputs: Vec<[u8; 32]>,
    pub leaf_count_u64: u64,
    pub flags_packed: u8,
    pub is_rollover_u64: u64,
}

pub fn parse_contract_address_to_32bytes(addr_str: &str) -> Option<[u8; 32]> {
    let clean = addr_str.trim_start_matches("0x");
    if clean.len() == 40 {
        if let Ok(b) = hex::decode(clean) {
            let mut out = [0u8; 32];
            out[12..].copy_from_slice(&b);
            return Some(out);
        }
    } else if clean.len() == 64 {
        if let Ok(b) = hex::decode(clean) {
            let mut out = [0u8; 32];
            out.copy_from_slice(&b);
            return Some(out);
        }
    }
    None
}

pub async fn validate_private_note_spend_preflight(
    state: &AppState,
    payload: &PrivateNoteSpendRequest,
) -> Result<(), PrivateNoteSpendResponse> {
    // 0. Asset ID validation (DEC-034C: 0 for WETH, 1 for USDC)
    let req_asset_id = payload.asset_id.unwrap_or(1);
    if req_asset_id > 1 {
        return Err(PrivateNoteSpendResponse {
            status: "REJECTED".to_string(),
            message: format!("UnsupportedAssetId: {}", req_asset_id),
            queue_position: 0,
            estimated_gas_usdc: None,
        });
    }

    // Anti-Cross-Asset Fee Drain (DEC-034C Section 2.B)
    let cached_quote_opt = {
        let cache = state.quote_cache.lock().await;
        let mut found = None;
        if let Some(ref qh) = payload.quote_hash {
            if let Some(q) = cache.get(qh) {
                found = Some(q.clone());
            }
        }
        if found.is_none() {
            if let Some(ref qid) = payload.quote_id {
                if let Some(q) = cache.get(qid) {
                    found = Some(q.clone());
                }
            }
        }
        found
    };

    if let Some(ref cached) = cached_quote_opt {
        if crate::validation::validate_multi_asset_preflight(req_asset_id, cached.asset_id).is_err()
        {
            return Err(PrivateNoteSpendResponse {
                status: "REJECTED".to_string(),
                message: "CrossAssetFeeMismatch: spend asset does not match quote asset"
                    .to_string(),
                queue_position: 0,
                estimated_gas_usdc: None,
            });
        }
    }

    // 2. Expiry check
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    if let Some(expiry) = payload.expiry {
        if expiry > 0 && now > expiry {
            let response = PrivateNoteSpendResponse {
                status: "REJECTED".to_string(),
                message: format!("Transaction expired. Expiry: {}, Current: {}", expiry, now),
                queue_position: 0,
                estimated_gas_usdc: None,
            };
            if let Some(ref idem_key) = payload.idempotency_key {
                if let Ok(json) = serde_json::to_string(&response) {
                    let _ = state
                        .db
                        .store_idempotency(idem_key, "private_note_spend", &json)
                        .await;
                }
            }
            return Err(response);
        }
    }

    // 3. Amount and fee invariant checks
    let is_rollover_req = payload.is_rollover == Some(1);
    if payload.merchant_amount == 0 && !is_rollover_req {
        return Err(PrivateNoteSpendResponse {
            status: "REJECTED".to_string(),
            message: "merchant_amount must be greater than zero".to_string(),
            queue_position: 0,
            estimated_gas_usdc: None,
        });
    }

    if payload.execution_fee > payload.max_execution_fee {
        return Err(PrivateNoteSpendResponse {
            status: "REJECTED".to_string(),
            message: format!(
                "Execution fee ({}) exceeds max execution fee ({})",
                payload.execution_fee, payload.max_execution_fee
            ),
            queue_position: 0,
            estimated_gas_usdc: None,
        });
    }

    // 4. has_change and commitment validation
    let has_change_u64 = match payload.has_change {
        Some(serde_json::Value::Bool(b)) => {
            if b {
                1u64
            } else {
                0u64
            }
        }
        Some(serde_json::Value::Number(ref n)) => n.as_u64().unwrap_or(0),
        _ => 0u64,
    };
    if has_change_u64 > 1 {
        return Err(PrivateNoteSpendResponse {
            status: "REJECTED".to_string(),
            message: "invalid has_change flag (must be 0 or 1)".to_string(),
            queue_position: 0,
            estimated_gas_usdc: None,
        });
    }

    if has_change_u64 == 0 {
        if let Some(ref comm) = payload.output_commitment {
            let clean = comm.trim_start_matches("0x");
            if !clean.is_empty() && clean.chars().any(|c| c != '0') {
                return Err(PrivateNoteSpendResponse {
                    status: "REJECTED".to_string(),
                    message: "nonzero output_commitment with zero change".to_string(),
                    queue_position: 0,
                    estimated_gas_usdc: None,
                });
            }
        }
    }

    // 5. Recipient address validation
    use alloy_primitives::{Address, U256};
    use std::str::FromStr;

    let recipient_addr = match Address::from_str(&payload.recipient) {
        Ok(addr) if payload.recipient.trim_start_matches("0x").len() == 40 => addr,
        _ => {
            return Err(PrivateNoteSpendResponse {
                status: "REJECTED".to_string(),
                message: format!("Invalid recipient address: {}", payload.recipient),
                queue_position: 0,
                estimated_gas_usdc: None,
            });
        }
    };

    // Sanctions screening (DEC-026 Layer 3: Relayer Operational Policy)
    // 1. Fast-path: Offline in-memory dataset
    let limits = crate::validation::SafetyLimits::default();
    if limits.is_sanctioned(&recipient_addr) {
        return Err(PrivateNoteSpendResponse {
            status: "REJECTED".to_string(),
            message: format!(
                "Recipient address {} is restricted under public sanctions policy (DEC-026)",
                payload.recipient
            ),
            queue_position: 0,
            estimated_gas_usdc: None,
        });
    }

    // 2. Dynamic-path: Live Chainalysis Sanctions Oracle query (Arbitrum on-chain)
    if let Some(ref client) = state.evm_client {
        match client.is_sanctioned_on_chain(&recipient_addr).await {
            Ok(true) => {
                return Err(PrivateNoteSpendResponse {
                    status: "REJECTED".to_string(),
                    message: format!(
                        "Recipient address {} is sanctioned per on-chain Chainalysis Oracle (DEC-026)",
                        payload.recipient
                    ),
                    queue_position: 0,
                    estimated_gas_usdc: None,
                });
            }
            Ok(false) => {}
            Err(e) => {
                eprintln!(
                    "RELAYER WARNING: On-chain sanctions oracle check failed: {}",
                    e
                );
            }
        }
    }

    // 6. Note root and nullifier format check
    let note_root_bytes = match hex::decode(payload.note_root.trim_start_matches("0x")) {
        Ok(b) if b.len() == 32 => b,
        _ => {
            return Err(PrivateNoteSpendResponse {
                status: "REJECTED".to_string(),
                message: "note_root must be a 32-byte hex string".to_string(),
                queue_position: 0,
                estimated_gas_usdc: None,
            });
        }
    };
    let nullifier_bytes = match hex::decode(payload.input_nullifier.trim_start_matches("0x")) {
        Ok(b) if b.len() == 32 => b,
        _ => {
            return Err(PrivateNoteSpendResponse {
                status: "REJECTED".to_string(),
                message: "input_nullifier must be a 32-byte hex string".to_string(),
                queue_position: 0,
                estimated_gas_usdc: None,
            });
        }
    };

    // 7. Proof byte lengths validation (EIP-2537: G1=128 bytes, G2=256 bytes)
    match hex::decode(payload.proof_a_neg.trim_start_matches("0x")) {
        Ok(b) if b.len() == 128 => {}
        _ => {
            return Err(PrivateNoteSpendResponse {
                status: "REJECTED".to_string(),
                message: "proof_a_neg must be a 128-byte hex string".to_string(),
                queue_position: 0,
                estimated_gas_usdc: None,
            });
        }
    }
    match hex::decode(payload.proof_b.trim_start_matches("0x")) {
        Ok(b) if b.len() == 256 => {}
        _ => {
            return Err(PrivateNoteSpendResponse {
                status: "REJECTED".to_string(),
                message: "proof_b must be a 256-byte hex string".to_string(),
                queue_position: 0,
                estimated_gas_usdc: None,
            });
        }
    }
    match hex::decode(payload.proof_c.trim_start_matches("0x")) {
        Ok(b) if b.len() == 128 => {}
        _ => {
            return Err(PrivateNoteSpendResponse {
                status: "REJECTED".to_string(),
                message: "proof_c must be a 128-byte hex string".to_string(),
                queue_position: 0,
                estimated_gas_usdc: None,
            });
        }
    }

    // 8. Public inputs canonical scalar check, semantic binding & target domain verification (DEC-022, DEC-025, DEC-032, DEC-033, DEC-035A)
    // DEC-035A: public_inputs is strictly required to have exactly 16 scalars (quote_hash_hi at index 9, quote_hash_lo at index 10, is_rollover at index 15).
    // Empty or omitted inputs are rejected fail-closed to prevent preflight bypass.
    if payload.public_inputs.len() != 16 {
        return Err(PrivateNoteSpendResponse {
            status: "REJECTED".to_string(),
            message: format!(
                "Expected 16 public inputs for PrivateNoteCircuit, got {}",
                payload.public_inputs.len()
            ),
            queue_position: 0,
            estimated_gas_usdc: None,
        });
    }

    let mut parsed_inputs = Vec::with_capacity(16);
    for (idx, pi_hex) in payload.public_inputs.iter().enumerate() {
        let bytes = match hex::decode(pi_hex.trim_start_matches("0x")) {
            Ok(b) if b.len() == 32 => b,
            _ => {
                return Err(PrivateNoteSpendResponse {
                    status: "REJECTED".to_string(),
                    message: format!("public_inputs[{}] must be a 32-byte hex scalar", idx),
                    queue_position: 0,
                    estimated_gas_usdc: None,
                });
            }
        };
        let mut fixed = [0u8; 32];
        fixed.copy_from_slice(&bytes);

        // Canonical scalar check: Fr < r
        if zeltra_core::from_evm_scalar(&fixed).is_none() {
            return Err(PrivateNoteSpendResponse {
                status: "REJECTED".to_string(),
                message: format!("public_inputs[{}] is a non-canonical scalar (>= r)", idx),
                queue_position: 0,
                estimated_gas_usdc: None,
            });
        }
        parsed_inputs.push(fixed);
    }

    // Semantic public input binding (DEC-022, DEC-032, DEC-033)
    // Index 0: note_root
    if parsed_inputs[0] != note_root_bytes[..] {
        return Err(PrivateNoteSpendResponse {
            status: "REJECTED".to_string(),
            message: "public_inputs[0] does not match note_root".to_string(),
            queue_position: 0,
            estimated_gas_usdc: None,
        });
    }

    // Index 1: leaf_count (DEC-032 strictly positive)
    let leaf_count_u64 = U256::from_be_bytes(parsed_inputs[1]).to::<u64>();
    if leaf_count_u64 == 0 {
        return Err(PrivateNoteSpendResponse {
            status: "REJECTED".to_string(),
            message: "leaf_count in public_inputs[1] must be greater than zero".to_string(),
            queue_position: 0,
            estimated_gas_usdc: None,
        });
    }
    if let Some(expected_lc) = payload.leaf_count {
        if leaf_count_u64 != expected_lc {
            return Err(PrivateNoteSpendResponse {
                status: "REJECTED".to_string(),
                message: "public_inputs[1] does not match leaf_count".to_string(),
                queue_position: 0,
                estimated_gas_usdc: None,
            });
        }
    }

    // Index 2: input_nullifier
    if parsed_inputs[2] != nullifier_bytes[..] {
        return Err(PrivateNoteSpendResponse {
            status: "REJECTED".to_string(),
            message: "public_inputs[2] does not match input_nullifier".to_string(),
            queue_position: 0,
            estimated_gas_usdc: None,
        });
    }

    // Index 3: note_epoch_id (DEC-033)
    let note_epoch_id = U256::from_be_bytes(parsed_inputs[3]).to::<u32>();
    if let Some(expected_epoch) = payload.note_epoch_id {
        if note_epoch_id != expected_epoch {
            return Err(PrivateNoteSpendResponse {
                status: "REJECTED".to_string(),
                message: "public_inputs[3] does not match note_epoch_id".to_string(),
                queue_position: 0,
                estimated_gas_usdc: None,
            });
        }
    }

    // Index 4: output_commitment
    let mut expected_comm = [0u8; 32];
    if let Some(ref comm) = payload.output_commitment {
        if let Ok(b) = hex::decode(comm.trim_start_matches("0x")) {
            if b.len() == 32 {
                expected_comm.copy_from_slice(&b);
            }
        }
    }
    if parsed_inputs[4] != expected_comm {
        return Err(PrivateNoteSpendResponse {
            status: "REJECTED".to_string(),
            message: "public_inputs[4] does not match output_commitment".to_string(),
            queue_position: 0,
            estimated_gas_usdc: None,
        });
    }

    // Index 5: recipient address
    let mut expected_recipient = [0u8; 32];
    expected_recipient[12..].copy_from_slice(recipient_addr.as_slice());
    if parsed_inputs[5] != expected_recipient {
        return Err(PrivateNoteSpendResponse {
            status: "REJECTED".to_string(),
            message: "public_inputs[5] does not match recipient address".to_string(),
            queue_position: 0,
            estimated_gas_usdc: None,
        });
    }

    // Index 6: merchant_amount
    let expected_merchant = U256::from(payload.merchant_amount).to_be_bytes::<32>();
    if parsed_inputs[6] != expected_merchant {
        return Err(PrivateNoteSpendResponse {
            status: "REJECTED".to_string(),
            message: "public_inputs[6] does not match merchant_amount".to_string(),
            queue_position: 0,
            estimated_gas_usdc: None,
        });
    }

    // Index 7: protocol_fee
    let expected_proto = U256::from(payload.protocol_fee).to_be_bytes::<32>();
    if parsed_inputs[7] != expected_proto {
        return Err(PrivateNoteSpendResponse {
            status: "REJECTED".to_string(),
            message: "public_inputs[7] does not match protocol_fee".to_string(),
            queue_position: 0,
            estimated_gas_usdc: None,
        });
    }

    // Index 8: execution_fee
    let expected_exec = U256::from(payload.execution_fee).to_be_bytes::<32>();
    if parsed_inputs[8] != expected_exec {
        return Err(PrivateNoteSpendResponse {
            status: "REJECTED".to_string(),
            message: "public_inputs[8] does not match execution_fee".to_string(),
            queue_position: 0,
            estimated_gas_usdc: None,
        });
    }

    // Index 9 & 10: quote_hash_hi & quote_hash_lo (DEC-035A Two-Limb Representation)
    // Preflight check: both limbs must strictly satisfy < 2^128 (upper 16 bytes must be 0)
    if parsed_inputs[9][0..16] != [0u8; 16] {
        return Err(PrivateNoteSpendResponse {
            status: "REJECTED".to_string(),
            message: "public_inputs[9] (quote_hash_hi) exceeds 128-bit range (>= 2^128)"
                .to_string(),
            queue_position: 0,
            estimated_gas_usdc: None,
        });
    }
    if parsed_inputs[10][0..16] != [0u8; 16] {
        return Err(PrivateNoteSpendResponse {
            status: "REJECTED".to_string(),
            message: "public_inputs[10] (quote_hash_lo) exceeds 128-bit range (>= 2^128)"
                .to_string(),
            queue_position: 0,
            estimated_gas_usdc: None,
        });
    }

    // Semantic match against payload.quote_hash if provided
    let mut expected_qh = [0u8; 32];
    if let Some(ref qh) = payload.quote_hash {
        if let Ok(b) = hex::decode(qh.trim_start_matches("0x")) {
            if b.len() == 32 {
                expected_qh.copy_from_slice(&b);
            }
        }
    }
    let mut expected_hi = [0u8; 32];
    expected_hi[16..32].copy_from_slice(&expected_qh[0..16]);
    let mut expected_lo = [0u8; 32];
    expected_lo[16..32].copy_from_slice(&expected_qh[16..32]);

    if parsed_inputs[9] != expected_hi {
        return Err(PrivateNoteSpendResponse {
            status: "REJECTED".to_string(),
            message: "public_inputs[9] does not match quote_hash_hi".to_string(),
            queue_position: 0,
            estimated_gas_usdc: None,
        });
    }
    if parsed_inputs[10] != expected_lo {
        return Err(PrivateNoteSpendResponse {
            status: "REJECTED".to_string(),
            message: "public_inputs[10] does not match quote_hash_lo".to_string(),
            queue_position: 0,
            estimated_gas_usdc: None,
        });
    }

    // Strict target domain verification for chain_id (index 11) and contract_address (index 12)
    let (expected_chain_bytes, expected_contract_bytes_opt) =
        if let Some(client) = state.evm_client.as_ref() {
            let target_chain_id = match client.chain_id().await {
                Ok(id) => id,
                Err(_) => {
                    return Err(PrivateNoteSpendResponse {
                        status: "ERROR".to_string(),
                        message: "Failed to query target chain ID from EVM client".to_string(),
                        queue_position: 0,
                        estimated_gas_usdc: None,
                    });
                }
            };
            let mut contract_bytes = [0u8; 32];
            contract_bytes[12..].copy_from_slice(client.contract_address_raw().as_slice());
            (
                U256::from(target_chain_id).to_be_bytes::<32>(),
                Some(contract_bytes),
            )
        } else {
            let chain_id = std::env::var("ZELTRA_CHAIN_ID")
                .ok()
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(421614);
            let contract_opt = std::env::var("ZELTRA_CONTRACT_ADDRESS")
                .ok()
                .and_then(|addr_str| parse_contract_address_to_32bytes(&addr_str));
            (U256::from(chain_id).to_be_bytes::<32>(), contract_opt)
        };

    // Index 11: chain_id
    if parsed_inputs[11] != expected_chain_bytes {
        return Err(PrivateNoteSpendResponse {
            status: "REJECTED".to_string(),
            message: "public_inputs[11] does not match target chain_id".to_string(),
            queue_position: 0,
            estimated_gas_usdc: None,
        });
    }

    // Index 12: contract_address
    if let Some(expected_contract) = expected_contract_bytes_opt {
        if parsed_inputs[12] != expected_contract {
            return Err(PrivateNoteSpendResponse {
                status: "REJECTED".to_string(),
                message: "public_inputs[12] does not match target contract_address".to_string(),
                queue_position: 0,
                estimated_gas_usdc: None,
            });
        }
    }

    // Index 13: expiry
    let expected_exp = U256::from(payload.expiry.unwrap_or(0)).to_be_bytes::<32>();
    if parsed_inputs[13] != expected_exp {
        return Err(PrivateNoteSpendResponse {
            status: "REJECTED".to_string(),
            message: "public_inputs[13] does not match expiry".to_string(),
            queue_position: 0,
            estimated_gas_usdc: None,
        });
    }

    // Index 14: has_change flag
    let expected_hc = U256::from(has_change_u64).to_be_bytes::<32>();
    if parsed_inputs[14] != expected_hc {
        return Err(PrivateNoteSpendResponse {
            status: "REJECTED".to_string(),
            message: "public_inputs[14] does not match has_change flag".to_string(),
            queue_position: 0,
            estimated_gas_usdc: None,
        });
    }

    // Index 15: flags_packed (DEC-034A: flags_packed = is_rollover + 2 * asset_id)
    let flags_packed_u64 = U256::from_be_bytes(parsed_inputs[15]).to::<u64>();
    if flags_packed_u64 > 3 {
        return Err(PrivateNoteSpendResponse {
            status: "REJECTED".to_string(),
            message: "public_inputs[15] (flags_packed) must be in 0..=3".to_string(),
            queue_position: 0,
            estimated_gas_usdc: None,
        });
    }
    let is_rollover_u64 = flags_packed_u64 & 1;
    let pi_asset_id = (flags_packed_u64 >> 1) as u8;
    if pi_asset_id != req_asset_id {
        return Err(PrivateNoteSpendResponse {
            status: "REJECTED".to_string(),
            message: format!(
                "public_inputs[15] asset_id ({}) does not match request asset_id ({})",
                pi_asset_id, req_asset_id
            ),
            queue_position: 0,
            estimated_gas_usdc: None,
        });
    }
    if let Some(expected_ro) = payload.is_rollover {
        if is_rollover_u64 != expected_ro {
            return Err(PrivateNoteSpendResponse {
                status: "REJECTED".to_string(),
                message: "public_inputs[15] does not match is_rollover flag".to_string(),
                queue_position: 0,
                estimated_gas_usdc: None,
            });
        }
    }

    // Pre-flight check: On-chain Epoch Active Window validation (DEC-033)
    let current_epoch = if let Some(client) = state.evm_client.as_ref() {
        client.get_current_epoch().await.unwrap_or(0)
    } else {
        std::env::var("ZELTRA_CURRENT_EPOCH")
            .ok()
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(0)
    };

    if note_epoch_id > current_epoch {
        return Err(PrivateNoteSpendResponse {
            status: "REJECTED".to_string(),
            message: "EPOCH_IN_FUTURE: note_epoch_id is in the future".to_string(),
            queue_position: 0,
            estimated_gas_usdc: None,
        });
    }

    if note_epoch_id < current_epoch.saturating_sub(1) {
        return Err(PrivateNoteSpendResponse {
            status: "REJECTED".to_string(),
            message:
                "EPOCH_EXPIRED_REQUIRE_ROLLOVER: note_epoch_id is older than active window (E-1)"
                    .to_string(),
            queue_position: 0,
            estimated_gas_usdc: None,
        });
    }

    // Pre-flight check: Rollover constraints & Dust Elimination (DEC-033)
    if is_rollover_u64 == 1 {
        if payload.merchant_amount != 0 {
            return Err(PrivateNoteSpendResponse {
                status: "REJECTED".to_string(),
                message: "merchant_amount must be 0 for standalone rollover".to_string(),
                queue_position: 0,
                estimated_gas_usdc: None,
            });
        }
        if payload.protocol_fee != 0 {
            return Err(PrivateNoteSpendResponse {
                status: "REJECTED".to_string(),
                message: "protocol_fee must be 0 for standalone rollover".to_string(),
                queue_position: 0,
                estimated_gas_usdc: None,
            });
        }
        if has_change_u64 != 1 {
            return Err(PrivateNoteSpendResponse {
                status: "REJECTED".to_string(),
                message: "has_change must be 1 for standalone rollover (refreshed note)"
                    .to_string(),
                queue_position: 0,
                estimated_gas_usdc: None,
            });
        }
        if let Some(input_val) = payload.input_value {
            if input_val < zeltra_core::fees::MIN_STANDALONE_ROLLOVER_THRESHOLD_USDC {
                return Err(PrivateNoteSpendResponse {
                    status: "REJECTED".to_string(),
                    message: "DUST_NOTE_ROLLOVER_REJECTED: Note value is below $2.00 USDC minimum threshold".to_string(),
                    queue_position: 0,
                    estimated_gas_usdc: None,
                });
            }
            if input_val < payload.execution_fee {
                return Err(PrivateNoteSpendResponse {
                    status: "REJECTED".to_string(),
                    message: "Note value insufficient to pay relayer execution fee".to_string(),
                    queue_position: 0,
                    estimated_gas_usdc: None,
                });
            }
        }
    }

    // 8.5. Local Groth16 Proof Preflight Verification (DEC-026 Mitigasi C-01)
    // Defends the relayer against gas-griefing attacks by verifying the Groth16 proof
    // on node CPU before enqueuing or spending gas for on-chain broadcast.
    let mut proof_a_bytes = [0u8; 128];
    if let Ok(b) = hex::decode(payload.proof_a_neg.trim_start_matches("0x")) {
        if b.len() == 128 {
            proof_a_bytes.copy_from_slice(&b);
        }
    }
    let mut proof_b_bytes = [0u8; 256];
    if let Ok(b) = hex::decode(payload.proof_b.trim_start_matches("0x")) {
        if b.len() == 256 {
            proof_b_bytes.copy_from_slice(&b);
        }
    }
    let mut proof_c_bytes = [0u8; 128];
    if let Ok(b) = hex::decode(payload.proof_c.trim_start_matches("0x")) {
        if b.len() == 128 {
            proof_c_bytes.copy_from_slice(&b);
        }
    }

    let mut fr_public_inputs = [zeltra_core::Fr::default(); 16];
    for (idx, pi) in parsed_inputs.iter().enumerate().take(16) {
        if let Some(fr) = zeltra_core::from_evm_scalar(pi) {
            fr_public_inputs[idx] = fr;
        }
    }

    // DEC-052: Strict curve point sanitization preflight check (AUD-30/AUD-32 defense)
    if let Err(sanitation_err) =
        zeltra_core::from_evm_proof_sanitized(&proof_a_bytes, &proof_b_bytes, &proof_c_bytes)
    {
        eprintln!(
            "SECURITY TELEMETRY [AUD-30/AUD-32/DEC-052]: Adversarial proof rejected during preflight! Error: {:?}",
            sanitation_err
        );
        return Err(PrivateNoteSpendResponse {
            status: "REJECTED".to_string(),
            message: format!("Invalid Groth16 proof: preflight verification failed - curve point sanitization rejected ({:?})", sanitation_err),
            queue_position: 0,
            estimated_gas_usdc: None,
        });
    }

    if !zeltra_core::verify_evm_note_proof(
        &proof_a_bytes,
        &proof_b_bytes,
        &proof_c_bytes,
        &fr_public_inputs,
    ) {
        return Err(PrivateNoteSpendResponse {
            status: "REJECTED".to_string(),
            message: "Invalid Groth16 proof: preflight verification failed (C-01 defense)"
                .to_string(),
            queue_position: 0,
            estimated_gas_usdc: None,
        });
    }

    if let Some(cached) = cached_quote_opt {
        let current_base_fee = if let Some(client) = state.evm_client.as_ref() {
            client.get_gas_price().await.unwrap_or(20_000_000) as u64
        } else {
            20_000_000
        };
        if let Err(e) = crate::validation::validate_quote_guard(
            cached.expiry,
            now,
            current_base_fee,
            cached.base_gas_fee,
        ) {
            return Err(PrivateNoteSpendResponse {
                status: "REJECTED".to_string(),
                message: format!("QuoteGuardRejection: {:?}", e),
                queue_position: 0,
                estimated_gas_usdc: None,
            });
        }
    }

    // 9. EIP-712 Quote Verification (if signed quote provided)
    if let (Some(quote_id), Some(quote_sig), Some(user_addr)) = (
        payload.quote_id.as_ref(),
        payload.quote_signature.as_ref(),
        payload.user_address.as_ref(),
    ) {
        match state
            .db
            .check_and_insert_quote_id(quote_id, user_addr)
            .await
        {
            Ok(true) => {}
            Ok(false) => {
                return Err(PrivateNoteSpendResponse {
                    status: "REJECTED".to_string(),
                    message: "Quote ID already used".to_string(),
                    queue_position: 0,
                    estimated_gas_usdc: None,
                });
            }
            Err(e) => {
                eprintln!("RELAYER ERROR: Quote ID check failed: {}", e);
                return Err(PrivateNoteSpendResponse {
                    status: "ERROR".to_string(),
                    message: "Database error".to_string(),
                    queue_position: 0,
                    estimated_gas_usdc: None,
                });
            }
        }

        use zeltra_sdk::eip712::{verify_quote_signature, ExecutionQuote};
        let (contract_address, chain_id) = match state.evm_client.as_ref() {
            Some(client) => {
                let chain_id = match client.chain_id().await {
                    Ok(id) => id,
                    Err(_) => {
                        return Err(PrivateNoteSpendResponse {
                            status: "ERROR".to_string(),
                            message: "Failed to get chain ID".to_string(),
                            queue_position: 0,
                            estimated_gas_usdc: None,
                        });
                    }
                };
                (client.contract_address(), chain_id)
            }
            None => {
                let chain_id = std::env::var("ZELTRA_CHAIN_ID")
                    .ok()
                    .and_then(|s| s.parse::<u64>().ok())
                    .unwrap_or(421614);
                let contract_addr = std::env::var("ZELTRA_CONTRACT_ADDRESS")
                    .unwrap_or_else(|_| "0x000000000000000000000000000000000000012c".to_string());
                (contract_addr, chain_id)
            }
        };

        let contract_addr = Address::from_str(&contract_address).unwrap_or(Address::ZERO);
        let user_addr_parsed = Address::from_str(user_addr).unwrap_or(Address::ZERO);
        let _relayer_addr = Address::from_str(&state.relayer_address).unwrap_or(Address::ZERO);

        let base_gas_fee = (payload.max_execution_fee * 100) / 115;
        let relayer_markup_fee = payload.max_execution_fee.saturating_sub(base_gas_fee);

        let execution_quote = ExecutionQuote {
            assetId: payload.asset_id.unwrap_or(1),
            targetContract: contract_addr,
            baseGasFee: base_gas_fee,
            relayerMarkupFee: relayer_markup_fee,
            totalExecutionFee: payload.max_execution_fee,
            expiry: payload.expiry.unwrap_or(0),
            chainId: U256::from(chain_id),
        };

        let sig_bytes = match hex::decode(quote_sig.trim_start_matches("0x")) {
            Ok(bytes) if bytes.len() == 65 => bytes,
            _ => {
                return Err(PrivateNoteSpendResponse {
                    status: "REJECTED".to_string(),
                    message: "Invalid signature format (expected 65 bytes)".to_string(),
                    queue_position: 0,
                    estimated_gas_usdc: None,
                });
            }
        };

        let v = sig_bytes[64];
        let mut r = [0u8; 32];
        let mut s = [0u8; 32];
        r.copy_from_slice(&sig_bytes[0..32]);
        s.copy_from_slice(&sig_bytes[32..64]);

        match verify_quote_signature(
            &execution_quote,
            chain_id,
            contract_addr,
            (v, r, s),
            user_addr_parsed,
        ) {
            Ok(true) => {}
            Ok(false) => {
                return Err(PrivateNoteSpendResponse {
                    status: "REJECTED".to_string(),
                    message: "Invalid quote signature".to_string(),
                    queue_position: 0,
                    estimated_gas_usdc: None,
                });
            }
            Err(e) => {
                return Err(PrivateNoteSpendResponse {
                    status: "ERROR".to_string(),
                    message: format!("Signature verification failed: {}", e),
                    queue_position: 0,
                    estimated_gas_usdc: None,
                });
            }
        }

        // Relayer Ingress Equality Guard (DEC-035B Gate B / Task B3)
        // B3.1 — Reconstruct Signing Digest in Handler from verified ExecutionQuote:
        let expected_hash_bytes =
            zeltra_sdk::eip712::compute_quote_hash(&execution_quote, chain_id, contract_addr);
        let (expected_hi_fr, expected_lo_fr) =
            zeltra_core::split_quote_hash_to_limbs(&expected_hash_bytes.0);
        let expected_hi_bytes = zeltra_core::fr_to_be_bytes(&expected_hi_fr);
        let expected_lo_bytes = zeltra_core::fr_to_be_bytes(&expected_lo_fr);

        // B3.2 & B3.3 — Bit-Exact Public Inputs Equality Assertion & Fail-Closed Rejection:
        let input_quote_hi = parsed_inputs[9];
        let input_quote_lo = parsed_inputs[10];

        if input_quote_hi != expected_hi_bytes || input_quote_lo != expected_lo_bytes {
            eprintln!(
                "RELAYER SECURITY WARNING: Public inputs quote_hash mismatch: expected (hi: 0x{}, lo: 0x{}), got (hi: 0x{}, lo: 0x{})",
                hex::encode(expected_hi_bytes),
                hex::encode(expected_lo_bytes),
                hex::encode(input_quote_hi),
                hex::encode(input_quote_lo),
            );
            return Err(PrivateNoteSpendResponse {
                status: "REJECTED".to_string(),
                message: "MismatchedQuoteHash: Public inputs quote_hash limbs do not match verified ExecutionQuote"
                    .to_string(),
                queue_position: 0,
                estimated_gas_usdc: None,
            });
        }
    }

    // 10. Double-spend check against local DB
    match state.db.is_nullifier_spent(&payload.input_nullifier).await {
        Ok(true) => {
            return Err(PrivateNoteSpendResponse {
                status: "REJECTED".to_string(),
                message: "Double-spending detected. Nullifier already exists in local database."
                    .to_string(),
                queue_position: 0,
                estimated_gas_usdc: None,
            });
        }
        Ok(false) => {}
        Err(e) => {
            eprintln!("RELAYER ERROR: Database nullifier check failed: {}", e);
            return Err(PrivateNoteSpendResponse {
                status: "ERROR".to_string(),
                message: "Database error".to_string(),
                queue_position: 0,
                estimated_gas_usdc: None,
            });
        }
    }

    // 11. Double-spend check against on-chain contract (fail-closed if spent)
    if let Some(ref client) = state.evm_client {
        match client.is_nullifier_spent(&payload.input_nullifier).await {
            Ok(true) => {
                return Err(PrivateNoteSpendResponse {
                    status: "REJECTED".to_string(),
                    message: "Double-spending detected. Nullifier already spent on-chain."
                        .to_string(),
                    queue_position: 0,
                    estimated_gas_usdc: None,
                });
            }
            Ok(false) => {}
            Err(e) => {
                eprintln!("RELAYER WARNING: On-chain nullifier check failed: {}", e);
            }
        }
    }

    Ok(())
}

pub async fn validate_joinsplit_spend_preflight(
    state: &AppState,
    payload: &JoinSplitSpendRequest,
) -> Result<ValidatedJoinSplitPreflight, JoinSplitSpendResponse> {
    // 0. Asset ID validation (DEC-034C: 0 for WETH, 1 for USDC)
    let req_asset_id = payload.asset_id.unwrap_or(1);
    if req_asset_id > 1 {
        return Err(JoinSplitSpendResponse {
            status: "REJECTED".to_string(),
            message: format!("UnsupportedAssetId: {}", req_asset_id),
            tx_hash: None,
            queue_position: 0,
            estimated_gas_usdc: None,
        });
    }

    // 0a. Calldata Memo Length Guard (AUD-5: exactly 96 bytes / 192 hex chars)
    let clean_m1 = payload
        .encrypted_memo_1
        .strip_prefix("0x")
        .unwrap_or(&payload.encrypted_memo_1);
    let clean_m2 = payload
        .encrypted_memo_2
        .strip_prefix("0x")
        .unwrap_or(&payload.encrypted_memo_2);
    if clean_m1.len() != 192
        || clean_m2.len() != 192
        || hex::decode(clean_m1).is_err()
        || hex::decode(clean_m2).is_err()
    {
        return Err(JoinSplitSpendResponse {
            status: "REJECTED".to_string(),
            message:
                "encrypted_memo_1 and encrypted_memo_2 must be exactly 96 bytes (192 hex chars)"
                    .to_string(),
            tx_hash: None,
            queue_position: 0,
            estimated_gas_usdc: None,
        });
    }

    // Anti-Cross-Asset Fee Drain (DEC-034C Section 2.B)
    let cached_quote_opt = {
        let cache = state.quote_cache.lock().await;
        let mut found = None;
        if let Some(ref qh) = payload.quote_hash {
            if let Some(q) = cache.get(qh) {
                found = Some(q.clone());
            }
        }
        if found.is_none() {
            if let Some(ref qid) = payload.quote_id {
                if let Some(q) = cache.get(qid) {
                    found = Some(q.clone());
                }
            }
        }
        found
    };

    if let Some(ref cached) = cached_quote_opt {
        if crate::validation::validate_multi_asset_preflight(req_asset_id, cached.asset_id).is_err()
        {
            return Err(JoinSplitSpendResponse {
                status: "REJECTED".to_string(),
                message: "CrossAssetFeeMismatch: spend asset does not match quote asset"
                    .to_string(),
                tx_hash: None,
                queue_position: 0,
                estimated_gas_usdc: None,
            });
        }
    }

    // 2. Anti-Aliasing Guard (Zellic 2024 / Singularity Defense)
    if payload.input_nullifier_1 == payload.input_nullifier_2 {
        let response = JoinSplitSpendResponse {
            status: "REJECTED".to_string(),
            message: "DuplicateNullifierInSameTx: input_nullifier_1 and input_nullifier_2 must not be identical".to_string(),
            tx_hash: None,
            queue_position: 0,
            estimated_gas_usdc: None,
        };
        return Err(response);
    }

    // 3. Expiry check
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    if let Some(expiry) = payload.expiry {
        if expiry > 0 && now > expiry {
            let response = JoinSplitSpendResponse {
                status: "REJECTED".to_string(),
                message: format!("Transaction expired. Expiry: {}, Current: {}", expiry, now),
                tx_hash: None,
                queue_position: 0,
                estimated_gas_usdc: None,
            };
            if let Some(ref idem_key) = payload.idempotency_key {
                if let Ok(json) = serde_json::to_string(&response) {
                    let _ = state
                        .db
                        .store_idempotency(idem_key, "joinsplit_spend", &json)
                        .await;
                }
            }
            return Err(response);
        }
    }

    // 4. Strict 3-Mode State Machine Validation (AUD-4)
    let is_rollover_req = payload.is_rollover == Some(1);
    let hc1 = match payload.has_change_1 {
        Some(serde_json::Value::Bool(b)) => {
            if b {
                1u8
            } else {
                0u8
            }
        }
        Some(serde_json::Value::Number(ref n)) => n.as_u64().unwrap_or(0) as u8,
        _ => 0u8,
    };
    let hc2 = match payload.has_change_2 {
        Some(serde_json::Value::Bool(b)) => {
            if b {
                1u8
            } else {
                0u8
            }
        }
        Some(serde_json::Value::Number(ref n)) => n.as_u64().unwrap_or(0) as u8,
        _ => 0u8,
    };
    let has_change_1 = hc1 == 1;

    let clean_recipient = payload.recipient.trim_start_matches("0x");
    let is_zero_rec = clean_recipient.is_empty() || clean_recipient.chars().all(|c| c == '0');

    if is_rollover_req {
        // Mode 1: Rollover Mode
        if payload.merchant_amount != 0 || payload.protocol_fee != 0 || !is_zero_rec {
            return Err(JoinSplitSpendResponse {
                status: "REJECTED".to_string(),
                message: "Rollover transactions must have merchant_amount=0, protocol_fee=0, recipient=0x0".to_string(),
                tx_hash: None,
                queue_position: 0,
                estimated_gas_usdc: None,
            });
        }
    } else if payload.merchant_amount == 0 {
        // Mode 2: In-Pool Transfer Mode
        if !has_change_1 || payload.protocol_fee != 0 || !is_zero_rec {
            return Err(JoinSplitSpendResponse {
                status: "REJECTED".to_string(),
                message:
                    "In-Pool Transfer requires has_change_1=1, protocol_fee=0, and recipient=0x0"
                        .to_string(),
                tx_hash: None,
                queue_position: 0,
                estimated_gas_usdc: None,
            });
        }
    } else {
        // Mode 3: Public Outflow Mode
        if is_zero_rec {
            return Err(JoinSplitSpendResponse {
                status: "REJECTED".to_string(),
                message: "Public spend cannot have zero recipient address".to_string(),
                tx_hash: None,
                queue_position: 0,
                estimated_gas_usdc: None,
            });
        }
    }

    if payload.execution_fee > payload.max_execution_fee {
        return Err(JoinSplitSpendResponse {
            status: "REJECTED".to_string(),
            message: format!(
                "Execution fee ({}) exceeds max execution fee ({})",
                payload.execution_fee, payload.max_execution_fee
            ),
            tx_hash: None,
            queue_position: 0,
            estimated_gas_usdc: None,
        });
    }

    // 5. Change flags and flags_packed validation
    let expected_flags = hc1 + 2 * hc2 + 4 * req_asset_id;
    let flags_packed = payload.flags_packed.unwrap_or(expected_flags);
    if flags_packed != expected_flags {
        return Err(JoinSplitSpendResponse {
            status: "REJECTED".to_string(),
            message: format!(
                "flags_packed ({}) does not match decomposed change flags (has_change_1: {}, has_change_2: {})",
                flags_packed, hc1, hc2
            ),
            tx_hash: None,
            queue_position: 0,
            estimated_gas_usdc: None,
        });
    }

    // 6. Recipient address validation & Sanctions screening (DEC-026)
    use alloy_primitives::U256;
    let recipient_addr = if is_zero_rec {
        Address::ZERO
    } else {
        match Address::from_str(&payload.recipient) {
            Ok(addr) if clean_recipient.len() == 40 => addr,
            _ => {
                return Err(JoinSplitSpendResponse {
                    status: "REJECTED".to_string(),
                    message: format!("Invalid recipient address: {}", payload.recipient),
                    tx_hash: None,
                    queue_position: 0,
                    estimated_gas_usdc: None,
                });
            }
        }
    };

    if !is_zero_rec {
        let limits = crate::validation::SafetyLimits::default();
        if limits.is_sanctioned(&recipient_addr) {
            return Err(JoinSplitSpendResponse {
                status: "REJECTED".to_string(),
                message: format!(
                    "Recipient address {} is restricted under public sanctions policy (DEC-026)",
                    payload.recipient
                ),
                tx_hash: None,
                queue_position: 0,
                estimated_gas_usdc: None,
            });
        }

        if let Some(ref client) = state.evm_client {
            if client
                .is_sanctioned_on_chain(&recipient_addr)
                .await
                .unwrap_or(false)
            {
                return Err(JoinSplitSpendResponse {
                    status: "REJECTED".to_string(),
                    message: format!(
                        "Recipient address {} is sanctioned per on-chain Chainalysis Oracle (DEC-026)",
                        payload.recipient
                    ),
                    tx_hash: None,
                    queue_position: 0,
                    estimated_gas_usdc: None,
                });
            }
        }
    }

    // 7. Note root and nullifier format check
    let note_root_bytes = match hex::decode(payload.note_root.trim_start_matches("0x")) {
        Ok(b) if b.len() == 32 => b,
        _ => {
            return Err(JoinSplitSpendResponse {
                status: "REJECTED".to_string(),
                message: "note_root must be a 32-byte hex string".to_string(),
                tx_hash: None,
                queue_position: 0,
                estimated_gas_usdc: None,
            });
        }
    };

    let nf1_bytes = match hex::decode(payload.input_nullifier_1.trim_start_matches("0x")) {
        Ok(b) if b.len() == 32 => b,
        _ => {
            return Err(JoinSplitSpendResponse {
                status: "REJECTED".to_string(),
                message: "input_nullifier_1 must be a 32-byte hex string".to_string(),
                tx_hash: None,
                queue_position: 0,
                estimated_gas_usdc: None,
            });
        }
    };

    let nf2_bytes = match hex::decode(payload.input_nullifier_2.trim_start_matches("0x")) {
        Ok(b) if b.len() == 32 => b,
        _ => {
            return Err(JoinSplitSpendResponse {
                status: "REJECTED".to_string(),
                message: "input_nullifier_2 must be a 32-byte hex string".to_string(),
                tx_hash: None,
                queue_position: 0,
                estimated_gas_usdc: None,
            });
        }
    };

    // 8. Proof byte lengths validation (EIP-2537: G1=128 bytes, G2=256 bytes)
    let proof_a_vec = match hex::decode(payload.proof_a_neg.trim_start_matches("0x")) {
        Ok(b) if b.len() == 128 => b,
        _ => {
            return Err(JoinSplitSpendResponse {
                status: "REJECTED".to_string(),
                message: "proof_a_neg must be a 128-byte hex string".to_string(),
                tx_hash: None,
                queue_position: 0,
                estimated_gas_usdc: None,
            });
        }
    };
    let proof_b_vec = match hex::decode(payload.proof_b.trim_start_matches("0x")) {
        Ok(b) if b.len() == 256 => b,
        _ => {
            return Err(JoinSplitSpendResponse {
                status: "REJECTED".to_string(),
                message: "proof_b must be a 256-byte hex string".to_string(),
                tx_hash: None,
                queue_position: 0,
                estimated_gas_usdc: None,
            });
        }
    };
    let proof_c_vec = match hex::decode(payload.proof_c.trim_start_matches("0x")) {
        Ok(b) if b.len() == 128 => b,
        _ => {
            return Err(JoinSplitSpendResponse {
                status: "REJECTED".to_string(),
                message: "proof_c must be a 128-byte hex string".to_string(),
                tx_hash: None,
                queue_position: 0,
                estimated_gas_usdc: None,
            });
        }
    };

    // 9. Public inputs canonical scalar check & semantic binding (19 public inputs)
    if payload.public_inputs.len() != 19 {
        return Err(JoinSplitSpendResponse {
            status: "REJECTED".to_string(),
            message: format!(
                "Expected 19 public inputs for JoinSplitCircuit, got {}",
                payload.public_inputs.len()
            ),
            tx_hash: None,
            queue_position: 0,
            estimated_gas_usdc: None,
        });
    }

    let mut parsed_inputs = Vec::with_capacity(19);
    for (idx, pi_hex) in payload.public_inputs.iter().enumerate() {
        let bytes = match hex::decode(pi_hex.trim_start_matches("0x")) {
            Ok(b) if b.len() == 32 => b,
            _ => {
                return Err(JoinSplitSpendResponse {
                    status: "REJECTED".to_string(),
                    message: format!("public_inputs[{}] must be a 32-byte hex scalar", idx),
                    tx_hash: None,
                    queue_position: 0,
                    estimated_gas_usdc: None,
                });
            }
        };
        let mut fixed = [0u8; 32];
        fixed.copy_from_slice(&bytes);

        // Canonical scalar check: Fr < r
        if zeltra_core::from_evm_scalar(&fixed).is_none() {
            return Err(JoinSplitSpendResponse {
                status: "REJECTED".to_string(),
                message: format!("public_inputs[{}] is a non-canonical scalar (>= r)", idx),
                tx_hash: None,
                queue_position: 0,
                estimated_gas_usdc: None,
            });
        }
        parsed_inputs.push(fixed);
    }

    // Semantic public input binding (DEC-036A, DEC-036C)
    // Index 0: note_root
    if parsed_inputs[0] != note_root_bytes[..] {
        return Err(JoinSplitSpendResponse {
            status: "REJECTED".to_string(),
            message: "public_inputs[0] does not match note_root".to_string(),
            tx_hash: None,
            queue_position: 0,
            estimated_gas_usdc: None,
        });
    }

    // Index 1: leaf_count (> 0)
    let leaf_count_u64 = U256::from_be_bytes(parsed_inputs[1]).to::<u64>();
    if leaf_count_u64 == 0 {
        return Err(JoinSplitSpendResponse {
            status: "REJECTED".to_string(),
            message: "leaf_count in public_inputs[1] must be greater than zero".to_string(),
            tx_hash: None,
            queue_position: 0,
            estimated_gas_usdc: None,
        });
    }
    if let Some(expected_lc) = payload.leaf_count {
        if leaf_count_u64 != expected_lc {
            return Err(JoinSplitSpendResponse {
                status: "REJECTED".to_string(),
                message: "public_inputs[1] does not match leaf_count".to_string(),
                tx_hash: None,
                queue_position: 0,
                estimated_gas_usdc: None,
            });
        }
    }

    // Index 2: input_nullifier_1
    if parsed_inputs[2] != nf1_bytes[..] {
        return Err(JoinSplitSpendResponse {
            status: "REJECTED".to_string(),
            message: "public_inputs[2] does not match input_nullifier_1".to_string(),
            tx_hash: None,
            queue_position: 0,
            estimated_gas_usdc: None,
        });
    }

    // Index 3: input_nullifier_2
    if parsed_inputs[3] != nf2_bytes[..] {
        return Err(JoinSplitSpendResponse {
            status: "REJECTED".to_string(),
            message: "public_inputs[3] does not match input_nullifier_2".to_string(),
            tx_hash: None,
            queue_position: 0,
            estimated_gas_usdc: None,
        });
    }

    // Index 4 & 5: epoch_id_1 & epoch_id_2
    if let Some(ep1) = payload.epoch_id_1 {
        let actual_ep1 = U256::from_be_bytes(parsed_inputs[4]).to::<u32>();
        if actual_ep1 != ep1 {
            return Err(JoinSplitSpendResponse {
                status: "REJECTED".to_string(),
                message: "public_inputs[4] does not match epoch_id_1".to_string(),
                tx_hash: None,
                queue_position: 0,
                estimated_gas_usdc: None,
            });
        }
    }
    if let Some(ep2) = payload.epoch_id_2 {
        let actual_ep2 = U256::from_be_bytes(parsed_inputs[5]).to::<u32>();
        if actual_ep2 != ep2 {
            return Err(JoinSplitSpendResponse {
                status: "REJECTED".to_string(),
                message: "public_inputs[5] does not match epoch_id_2".to_string(),
                tx_hash: None,
                queue_position: 0,
                estimated_gas_usdc: None,
            });
        }
    }

    // Index 6 & 7: output_commitment_1 & output_commitment_2
    let mut expected_cm1 = [0u8; 32];
    if let Ok(b) = hex::decode(payload.output_commitment_1.trim_start_matches("0x")) {
        if b.len() == 32 {
            expected_cm1.copy_from_slice(&b);
        }
    }
    if parsed_inputs[6] != expected_cm1 {
        return Err(JoinSplitSpendResponse {
            status: "REJECTED".to_string(),
            message: "public_inputs[6] does not match output_commitment_1".to_string(),
            tx_hash: None,
            queue_position: 0,
            estimated_gas_usdc: None,
        });
    }

    let mut expected_cm2 = [0u8; 32];
    if let Ok(b) = hex::decode(payload.output_commitment_2.trim_start_matches("0x")) {
        if b.len() == 32 {
            expected_cm2.copy_from_slice(&b);
        }
    }
    if parsed_inputs[7] != expected_cm2 {
        return Err(JoinSplitSpendResponse {
            status: "REJECTED".to_string(),
            message: "public_inputs[7] does not match output_commitment_2".to_string(),
            tx_hash: None,
            queue_position: 0,
            estimated_gas_usdc: None,
        });
    }

    // Index 8: recipient address
    let mut expected_recipient = [0u8; 32];
    expected_recipient[12..].copy_from_slice(recipient_addr.as_slice());
    if parsed_inputs[8] != expected_recipient {
        return Err(JoinSplitSpendResponse {
            status: "REJECTED".to_string(),
            message: "public_inputs[8] does not match recipient address".to_string(),
            tx_hash: None,
            queue_position: 0,
            estimated_gas_usdc: None,
        });
    }

    // Index 9: merchant_amount
    let expected_merchant = U256::from(payload.merchant_amount).to_be_bytes::<32>();
    if parsed_inputs[9] != expected_merchant {
        return Err(JoinSplitSpendResponse {
            status: "REJECTED".to_string(),
            message: "public_inputs[9] does not match merchant_amount".to_string(),
            tx_hash: None,
            queue_position: 0,
            estimated_gas_usdc: None,
        });
    }

    // Index 10: protocol_fee
    let expected_proto = U256::from(payload.protocol_fee).to_be_bytes::<32>();
    if parsed_inputs[10] != expected_proto {
        return Err(JoinSplitSpendResponse {
            status: "REJECTED".to_string(),
            message: "public_inputs[10] does not match protocol_fee".to_string(),
            tx_hash: None,
            queue_position: 0,
            estimated_gas_usdc: None,
        });
    }

    // Index 11: execution_fee
    let expected_exec = U256::from(payload.execution_fee).to_be_bytes::<32>();
    if parsed_inputs[11] != expected_exec {
        return Err(JoinSplitSpendResponse {
            status: "REJECTED".to_string(),
            message: "public_inputs[11] does not match execution_fee".to_string(),
            tx_hash: None,
            queue_position: 0,
            estimated_gas_usdc: None,
        });
    }

    // Index 12 & 13: quote_hash_hi & quote_hash_lo (Two-Limb Representation)
    if parsed_inputs[12][0..16] != [0u8; 16] {
        return Err(JoinSplitSpendResponse {
            status: "REJECTED".to_string(),
            message: "public_inputs[12] (quote_hash_hi) exceeds 128-bit range (>= 2^128)"
                .to_string(),
            tx_hash: None,
            queue_position: 0,
            estimated_gas_usdc: None,
        });
    }
    if parsed_inputs[13][0..16] != [0u8; 16] {
        return Err(JoinSplitSpendResponse {
            status: "REJECTED".to_string(),
            message: "public_inputs[13] (quote_hash_lo) exceeds 128-bit range (>= 2^128)"
                .to_string(),
            tx_hash: None,
            queue_position: 0,
            estimated_gas_usdc: None,
        });
    }

    if let Some(ref qh) = payload.quote_hash {
        if let Ok(b) = hex::decode(qh.trim_start_matches("0x")) {
            if b.len() == 32 {
                let mut expected_hi = [0u8; 32];
                expected_hi[16..32].copy_from_slice(&b[0..16]);
                let mut expected_lo = [0u8; 32];
                expected_lo[16..32].copy_from_slice(&b[16..32]);

                if parsed_inputs[12] != expected_hi {
                    return Err(JoinSplitSpendResponse {
                        status: "REJECTED".to_string(),
                        message: "public_inputs[12] does not match quote_hash_hi".to_string(),
                        tx_hash: None,
                        queue_position: 0,
                        estimated_gas_usdc: None,
                    });
                }
                if parsed_inputs[13] != expected_lo {
                    return Err(JoinSplitSpendResponse {
                        status: "REJECTED".to_string(),
                        message: "public_inputs[13] does not match quote_hash_lo".to_string(),
                        tx_hash: None,
                        queue_position: 0,
                        estimated_gas_usdc: None,
                    });
                }
            }
        }
    }

    // Index 14: chain_id
    if let Some(expected_cid) = payload.chain_id {
        let actual_cid = U256::from_be_bytes(parsed_inputs[14]).to::<u64>();
        if actual_cid != expected_cid {
            return Err(JoinSplitSpendResponse {
                status: "REJECTED".to_string(),
                message: "public_inputs[14] does not match chain_id".to_string(),
                tx_hash: None,
                queue_position: 0,
                estimated_gas_usdc: None,
            });
        }
    }

    // Index 15: contract_address
    if let Some(ref caddr_str) = payload.contract_address {
        if let Some(expected_caddr) = parse_contract_address_to_32bytes(caddr_str) {
            if parsed_inputs[15] != expected_caddr {
                return Err(JoinSplitSpendResponse {
                    status: "REJECTED".to_string(),
                    message: "public_inputs[15] does not match contract_address".to_string(),
                    tx_hash: None,
                    queue_position: 0,
                    estimated_gas_usdc: None,
                });
            }
        }
    }

    // Index 16: expiry
    let expected_exp = U256::from(payload.expiry.unwrap_or(0)).to_be_bytes::<32>();
    if parsed_inputs[16] != expected_exp {
        return Err(JoinSplitSpendResponse {
            status: "REJECTED".to_string(),
            message: "public_inputs[16] does not match expiry".to_string(),
            tx_hash: None,
            queue_position: 0,
            estimated_gas_usdc: None,
        });
    }

    // Index 17: flags_packed
    let expected_fp = U256::from(flags_packed).to_be_bytes::<32>();
    if parsed_inputs[17] != expected_fp {
        return Err(JoinSplitSpendResponse {
            status: "REJECTED".to_string(),
            message: "public_inputs[17] does not match flags_packed".to_string(),
            tx_hash: None,
            queue_position: 0,
            estimated_gas_usdc: None,
        });
    }

    // Index 18: is_rollover
    let is_rollover_u64 = U256::from_be_bytes(parsed_inputs[18]).to::<u64>();
    if is_rollover_u64 != 0 && is_rollover_u64 != 1 {
        return Err(JoinSplitSpendResponse {
            status: "REJECTED".to_string(),
            message: "public_inputs[18] (is_rollover) must be 0 or 1".to_string(),
            tx_hash: None,
            queue_position: 0,
            estimated_gas_usdc: None,
        });
    }
    if let Some(expected_ro) = payload.is_rollover {
        if is_rollover_u64 != expected_ro {
            return Err(JoinSplitSpendResponse {
                status: "REJECTED".to_string(),
                message: "public_inputs[18] does not match is_rollover flag".to_string(),
                tx_hash: None,
                queue_position: 0,
                estimated_gas_usdc: None,
            });
        }
    }

    // 10. Local CPU Groth16 Preflight Verifier (Phase 3: Gas Griefing Defense against EIP-2537)
    let mut proof_a_bytes = [0u8; 128];
    proof_a_bytes.copy_from_slice(&proof_a_vec);
    let mut proof_b_bytes = [0u8; 256];
    proof_b_bytes.copy_from_slice(&proof_b_vec);
    let mut proof_c_bytes = [0u8; 128];
    proof_c_bytes.copy_from_slice(&proof_c_vec);

    let mut fr_public_inputs = [zeltra_core::Fr::default(); 19];
    for (idx, pi) in parsed_inputs.iter().enumerate().take(19) {
        if let Some(fr) = zeltra_core::from_evm_scalar(pi) {
            fr_public_inputs[idx] = fr;
        }
    }

    // DEC-052: Strict curve point sanitization preflight check (AUD-30/AUD-32 defense)
    let proof = match zeltra_core::from_evm_proof_sanitized(
        &proof_a_bytes,
        &proof_b_bytes,
        &proof_c_bytes,
    ) {
        Ok(p) => p,
        Err(sanitation_err) => {
            eprintln!(
                "SECURITY TELEMETRY [AUD-30/AUD-32/DEC-052]: Adversarial JoinSplit proof rejected during preflight! Error: {:?}",
                sanitation_err
            );
            return Err(JoinSplitSpendResponse {
                status: "REJECTED".to_string(),
                message: format!(
                    "Invalid Groth16 proof: Failed to deserialize Groth16 proof points ({:?})",
                    sanitation_err
                ),
                tx_hash: None,
                queue_position: 0,
                estimated_gas_usdc: None,
            });
        }
    };

    let keys = zeltra_core::joinsplit_circuit::get_or_init_joinsplit_circuit_keys();
    let is_valid = zeltra_core::joinsplit_circuit::verify_joinsplit_proof(
        &keys.verifying_key,
        &proof,
        &fr_public_inputs,
    )
    .unwrap_or(false);

    if !is_valid {
        return Err(JoinSplitSpendResponse {
            status: "REJECTED".to_string(),
            message: "Invalid Groth16 proof: preflight verification failed (gas griefing defense)"
                .to_string(),
            tx_hash: None,
            queue_position: 0,
            estimated_gas_usdc: None,
        });
    }

    // 11. SQLite Atomic Dual Nullifier Reservation (Phase 2: C2.1)
    let reserved = match state
        .db
        .check_and_insert_dual_nullifiers(
            &payload.input_nullifier_1,
            &payload.input_nullifier_2,
            None,
        )
        .await
    {
        Ok(res) => res,
        Err(e) => {
            eprintln!("Database error checking nullifiers: {}", e);
            return Err(JoinSplitSpendResponse {
                status: "ERROR".to_string(),
                message: "Database error during nullifier check".to_string(),
                tx_hash: None,
                queue_position: 0,
                estimated_gas_usdc: None,
            });
        }
    };
    if !reserved {
        return Err(JoinSplitSpendResponse {
            status: "REJECTED".to_string(),
            message: "Nullifier already spent in relayer database".to_string(),
            tx_hash: None,
            queue_position: 0,
            estimated_gas_usdc: None,
        });
    }

    Ok(ValidatedJoinSplitPreflight {
        parsed_inputs,
        leaf_count_u64,
        flags_packed,
        is_rollover_u64,
    })
}
