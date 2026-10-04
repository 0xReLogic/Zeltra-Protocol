//! Spend handler and batch processing logic
//!
//! Security features:
//! - Slippage protection (min_payout, deadline) -- Finding #9
//! - Minimum balance checks before batch processing -- Finding #10
//! - Idempotency key support for request deduplication -- Finding #16

use crate::database::QueuedSpend;
use crate::evm_client::BatchSpendItem;
use crate::{dto::*, state::AppState};
use axum::Json;

pub async fn handle_spend(
    axum::extract::State(state): axum::extract::State<AppState>,
    Json(payload): Json<SpendRequest>,
) -> Json<SpendResponse> {
    // --- Idempotency check (Finding #16) ---
    if let Some(ref idem_key) = payload.idempotency_key {
        match state.db.check_idempotency(idem_key, "spend").await {
            Ok(Some(cached_json)) => {
                if let Ok(cached_resp) = serde_json::from_str::<SpendResponse>(&cached_json) {
                    println!(
                        "IDEMPOTENCY: Returning cached response for key {}...",
                        &idem_key[..8.min(idem_key.len())]
                    );
                    return Json(cached_resp);
                }
            }
            Ok(None) => { /* No cached response, proceed normally */ }
            Err(e) => {
                eprintln!("RELAYER WARNING: Idempotency check failed: {}", e);
                // Proceed anyway -- idempotency is best-effort
            }
        }
    }

    // --- Deadline check (Finding #9 -- slippage protection) ---
    if let Some(deadline) = payload.deadline {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        if now > deadline {
            let response = SpendResponse {
                status: "REJECTED".to_string(),
                message: format!(
                    "Transaction deadline expired. Deadline: {}, Current: {}",
                    deadline, now
                ),
                queue_position: 0,
                estimated_gas_usdc: None,
            };
            // Cache rejection for idempotency
            if let Some(ref idem_key) = payload.idempotency_key {
                if let Ok(json) = serde_json::to_string(&response) {
                    let _ = state.db.store_idempotency(idem_key, "spend", &json).await;
                }
            }
            return Json(response);
        }
    }

    // --- Input Validation & Safety Bounds (DEC-019) ---
    let safety_limits = crate::validation::SafetyLimits::default();
    if let Err(val_err) = crate::validation::validate_spend_request(
        &payload.recipient,
        payload.amount,
        payload.cross_chain.as_ref(),
        &safety_limits,
    ) {
        let response = SpendResponse {
            status: "REJECTED".to_string(),
            message: format!("Input validation failed: {}", val_err),
            queue_position: 0,
            estimated_gas_usdc: None,
        };
        if let Some(ref idem_key) = payload.idempotency_key {
            if let Ok(json) = serde_json::to_string(&response) {
                let _ = state.db.store_idempotency(idem_key, "spend", &json).await;
            }
        }
        return Json(response);
    }

    // --- Double-spend check ---
    match state.db.is_nullifier_spent(&payload.nullifier).await {
        Ok(true) => {
            return Json(SpendResponse {
                status: "REJECTED".to_string(),
                message: "Double-spending detected. Nullifier already exists.".to_string(),
                queue_position: 0,
                estimated_gas_usdc: None,
            });
        }
        Ok(false) => {
            // Nullifier not spent, proceed
        }
        Err(e) => {
            eprintln!("RELAYER ERROR: Database nullifier check failed: {}", e);
            return Json(SpendResponse {
                status: "ERROR".to_string(),
                message: "Database error".to_string(),
                queue_position: 0,
                estimated_gas_usdc: None,
            });
        }
    }

    // --- EIP-712 Quote Verification (if signed quote provided) ---
    if let (Some(max_fee), Some(quote_id), Some(quote_expiry), Some(signature), Some(user_addr)) = (
        payload.max_execution_fee,
        payload.quote_id.as_ref(),
        payload.quote_expiry,
        payload.quote_signature.as_ref(),
        payload.user_address.as_ref(),
    ) {
        // Check expiry
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        if now > quote_expiry {
            return Json(SpendResponse {
                status: "REJECTED".to_string(),
                message: format!(
                    "Quote expired. Quote expiry: {}, Current: {}",
                    quote_expiry, now
                ),
                queue_position: 0,
                estimated_gas_usdc: None,
            });
        }

        // Check quote_id replay protection
        match state
            .db
            .check_and_insert_quote_id(quote_id, user_addr)
            .await
        {
            Ok(true) => {
                // Successfully inserted, not used before
            }
            Ok(false) => {
                return Json(SpendResponse {
                    status: "REJECTED".to_string(),
                    message: "Quote ID already used".to_string(),
                    queue_position: 0,
                    estimated_gas_usdc: None,
                });
            }
            Err(e) => {
                eprintln!("RELAYER ERROR: Quote ID check failed: {}", e);
                return Json(SpendResponse {
                    status: "ERROR".to_string(),
                    message: "Database error".to_string(),
                    queue_position: 0,
                    estimated_gas_usdc: None,
                });
            }
        }

        // Verify EIP-712 signature
        use alloy_primitives::{Address, U256};
        use nimbus_sdk::eip712::{verify_quote_signature, ExecutionQuote};
        use std::str::FromStr;

        let (contract_address, chain_id) = match state.evm_client.as_ref() {
            Some(client) => {
                let chain_id = match client.chain_id().await {
                    Ok(id) => id,
                    Err(_) => {
                        return Json(SpendResponse {
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
                return Json(SpendResponse {
                    status: "ERROR".to_string(),
                    message: "EVM client not configured".to_string(),
                    queue_position: 0,
                    estimated_gas_usdc: None,
                });
            }
        };

        let contract_addr = Address::from_str(&contract_address).unwrap_or(Address::ZERO);
        let user_addr_parsed = Address::from_str(user_addr).unwrap_or(Address::ZERO);
        let relayer_addr = Address::from_str(&state.relayer_address).unwrap_or(Address::ZERO);

        let execution_quote = ExecutionQuote {
            quoteId: U256::from_str(quote_id).unwrap_or(U256::ZERO),
            maxExecutionFee: U256::from(max_fee),
            merchantAmount: U256::from(payload.amount),
            quoteExpiry: U256::from(quote_expiry),
            relayerAddress: relayer_addr,
        };

        // Parse signature (v, r, s concatenated)
        let sig_bytes = match hex::decode(signature.trim_start_matches("0x")) {
            Ok(bytes) if bytes.len() == 65 => bytes,
            _ => {
                return Json(SpendResponse {
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
            Ok(true) => {
                // Signature valid, proceed
            }
            Ok(false) => {
                return Json(SpendResponse {
                    status: "REJECTED".to_string(),
                    message: "Invalid quote signature".to_string(),
                    queue_position: 0,
                    estimated_gas_usdc: None,
                });
            }
            Err(e) => {
                return Json(SpendResponse {
                    status: "ERROR".to_string(),
                    message: format!("Signature verification failed: {}", e),
                    queue_position: 0,
                    estimated_gas_usdc: None,
                });
            }
        }
    }

    // Persist before acknowledging the request. The database queue is the
    // source of truth and also reserves the nullifier while settlement is active.
    let queue_id = match state.db.enqueue_spend(&payload).await {
        Ok(Some(id)) => id,
        Ok(None) => {
            return Json(SpendResponse {
                status: "REJECTED".to_string(),
                message: "Nullifier already queued, submitted, or confirmed.".to_string(),
                queue_position: 0,
                estimated_gas_usdc: None,
            });
        }
        Err(e) => {
            eprintln!("RELAYER ERROR: Failed to persist spend: {}", e);
            return Json(SpendResponse {
                status: "ERROR".to_string(),
                message: "Failed to persist settlement request".to_string(),
                queue_position: 0,
                estimated_gas_usdc: None,
            });
        }
    };
    let position = state.db.queued_spend_count().await.unwrap_or(1);

    if let Some(auth) = &payload.eip7702_auth {
        println!("RELAYER: Received spend request via EIP-7702 delegation");
        println!("  EOA Address       : {}", auth.eoa_address);
        println!("  Delegate Contract : {}", auth.delegate_contract);
        println!("  Status            : Queued for batching");
    } else {
        println!("RELAYER: Received standard spend request (Queued)");
    }

    let response = SpendResponse {
        status: "QUEUED".to_string(),
        message: format!("Spend persisted with settlement id {}", queue_id),
        queue_position: position,
        estimated_gas_usdc: None,
    };

    // Store idempotency response
    if let Some(ref idem_key) = payload.idempotency_key {
        if let Ok(json) = serde_json::to_string(&response) {
            let _ = state.db.store_idempotency(idem_key, "spend", &json).await;
        }
    }

    Json(response)
}

/// Handler for ZK Private Note Spend (DEC-025 Gate F)
///
/// POST /api/v1/spend-private-note
pub async fn handle_private_note_spend(
    axum::extract::State(state): axum::extract::State<AppState>,
    Json(payload): Json<PrivateNoteSpendRequest>,
) -> Json<PrivateNoteSpendResponse> {
    // 1. Idempotency check
    if let Some(ref idem_key) = payload.idempotency_key {
        match state
            .db
            .check_idempotency(idem_key, "private_note_spend")
            .await
        {
            Ok(Some(cached_json)) => {
                if let Ok(cached_resp) =
                    serde_json::from_str::<PrivateNoteSpendResponse>(&cached_json)
                {
                    println!(
                        "IDEMPOTENCY: Returning cached response for private note spend key {}...",
                        &idem_key[..8.min(idem_key.len())]
                    );
                    return Json(cached_resp);
                }
            }
            Ok(None) => {}
            Err(e) => {
                eprintln!("RELAYER WARNING: Idempotency check failed: {}", e);
            }
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
            return Json(response);
        }
    }

    // 3. Amount and fee invariant checks
    if payload.merchant_amount == 0 {
        return Json(PrivateNoteSpendResponse {
            status: "REJECTED".to_string(),
            message: "merchant_amount must be greater than zero".to_string(),
            queue_position: 0,
            estimated_gas_usdc: None,
        });
    }

    if payload.execution_fee > payload.max_execution_fee {
        return Json(PrivateNoteSpendResponse {
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
        return Json(PrivateNoteSpendResponse {
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
                return Json(PrivateNoteSpendResponse {
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
            return Json(PrivateNoteSpendResponse {
                status: "REJECTED".to_string(),
                message: format!("Invalid recipient address: {}", payload.recipient),
                queue_position: 0,
                estimated_gas_usdc: None,
            });
        }
    };

    // 6. Note root and nullifier format check
    let note_root_bytes = match hex::decode(payload.note_root.trim_start_matches("0x")) {
        Ok(b) if b.len() == 32 => b,
        _ => {
            return Json(PrivateNoteSpendResponse {
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
            return Json(PrivateNoteSpendResponse {
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
            return Json(PrivateNoteSpendResponse {
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
            return Json(PrivateNoteSpendResponse {
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
            return Json(PrivateNoteSpendResponse {
                status: "REJECTED".to_string(),
                message: "proof_c must be a 128-byte hex string".to_string(),
                queue_position: 0,
                estimated_gas_usdc: None,
            });
        }
    }

    // 8. Public inputs canonical scalar check & semantic binding (DEC-022 & DEC-025)
    if !payload.public_inputs.is_empty() {
        if payload.public_inputs.len() != 12 {
            return Json(PrivateNoteSpendResponse {
                status: "REJECTED".to_string(),
                message: format!(
                    "Expected 12 public inputs for PrivateNoteCircuit, got {}",
                    payload.public_inputs.len()
                ),
                queue_position: 0,
                estimated_gas_usdc: None,
            });
        }

        let mut parsed_inputs = Vec::with_capacity(12);
        for (idx, pi_hex) in payload.public_inputs.iter().enumerate() {
            let bytes = match hex::decode(pi_hex.trim_start_matches("0x")) {
                Ok(b) if b.len() == 32 => b,
                _ => {
                    return Json(PrivateNoteSpendResponse {
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
            if nimbus_core::from_evm_scalar(&fixed).is_none() {
                return Json(PrivateNoteSpendResponse {
                    status: "REJECTED".to_string(),
                    message: format!("public_inputs[{}] is a non-canonical scalar (>= r)", idx),
                    queue_position: 0,
                    estimated_gas_usdc: None,
                });
            }
            parsed_inputs.push(fixed);
        }

        // Semantic public input binding (DEC-022 boundary gap defense)
        if parsed_inputs[0] != note_root_bytes[..] {
            return Json(PrivateNoteSpendResponse {
                status: "REJECTED".to_string(),
                message: "public_inputs[0] does not match note_root".to_string(),
                queue_position: 0,
                estimated_gas_usdc: None,
            });
        }
        if parsed_inputs[1] != nullifier_bytes[..] {
            return Json(PrivateNoteSpendResponse {
                status: "REJECTED".to_string(),
                message: "public_inputs[1] does not match input_nullifier".to_string(),
                queue_position: 0,
                estimated_gas_usdc: None,
            });
        }

        let mut expected_comm = [0u8; 32];
        if let Some(ref comm) = payload.output_commitment {
            if let Ok(b) = hex::decode(comm.trim_start_matches("0x")) {
                if b.len() == 32 {
                    expected_comm.copy_from_slice(&b);
                }
            }
        }
        if parsed_inputs[2] != expected_comm {
            return Json(PrivateNoteSpendResponse {
                status: "REJECTED".to_string(),
                message: "public_inputs[2] does not match output_commitment".to_string(),
                queue_position: 0,
                estimated_gas_usdc: None,
            });
        }

        let mut expected_recipient = [0u8; 32];
        expected_recipient[12..].copy_from_slice(recipient_addr.as_slice());
        if parsed_inputs[3] != expected_recipient {
            return Json(PrivateNoteSpendResponse {
                status: "REJECTED".to_string(),
                message: "public_inputs[3] does not match recipient address".to_string(),
                queue_position: 0,
                estimated_gas_usdc: None,
            });
        }

        let expected_merchant = U256::from(payload.merchant_amount).to_be_bytes::<32>();
        if parsed_inputs[4] != expected_merchant {
            return Json(PrivateNoteSpendResponse {
                status: "REJECTED".to_string(),
                message: "public_inputs[4] does not match merchant_amount".to_string(),
                queue_position: 0,
                estimated_gas_usdc: None,
            });
        }

        let expected_proto = U256::from(payload.protocol_fee).to_be_bytes::<32>();
        if parsed_inputs[5] != expected_proto {
            return Json(PrivateNoteSpendResponse {
                status: "REJECTED".to_string(),
                message: "public_inputs[5] does not match protocol_fee".to_string(),
                queue_position: 0,
                estimated_gas_usdc: None,
            });
        }

        let expected_exec = U256::from(payload.execution_fee).to_be_bytes::<32>();
        if parsed_inputs[6] != expected_exec {
            return Json(PrivateNoteSpendResponse {
                status: "REJECTED".to_string(),
                message: "public_inputs[6] does not match execution_fee".to_string(),
                queue_position: 0,
                estimated_gas_usdc: None,
            });
        }

        let mut expected_qh = [0u8; 32];
        if let Some(ref qh) = payload.quote_hash {
            if let Ok(b) = hex::decode(qh.trim_start_matches("0x")) {
                if b.len() == 32 {
                    expected_qh.copy_from_slice(&b);
                }
            }
        }
        if parsed_inputs[7] != expected_qh {
            return Json(PrivateNoteSpendResponse {
                status: "REJECTED".to_string(),
                message: "public_inputs[7] does not match quote_hash".to_string(),
                queue_position: 0,
                estimated_gas_usdc: None,
            });
        }

        let expected_exp = U256::from(payload.expiry.unwrap_or(0)).to_be_bytes::<32>();
        if parsed_inputs[10] != expected_exp {
            return Json(PrivateNoteSpendResponse {
                status: "REJECTED".to_string(),
                message: "public_inputs[10] does not match expiry".to_string(),
                queue_position: 0,
                estimated_gas_usdc: None,
            });
        }

        let expected_hc = U256::from(has_change_u64).to_be_bytes::<32>();
        if parsed_inputs[11] != expected_hc {
            return Json(PrivateNoteSpendResponse {
                status: "REJECTED".to_string(),
                message: "public_inputs[11] does not match has_change flag".to_string(),
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
                return Json(PrivateNoteSpendResponse {
                    status: "REJECTED".to_string(),
                    message: "Quote ID already used".to_string(),
                    queue_position: 0,
                    estimated_gas_usdc: None,
                });
            }
            Err(e) => {
                eprintln!("RELAYER ERROR: Quote ID check failed: {}", e);
                return Json(PrivateNoteSpendResponse {
                    status: "ERROR".to_string(),
                    message: "Database error".to_string(),
                    queue_position: 0,
                    estimated_gas_usdc: None,
                });
            }
        }

        use nimbus_sdk::eip712::{verify_quote_signature, ExecutionQuote};
        let (contract_address, chain_id) = match state.evm_client.as_ref() {
            Some(client) => {
                let chain_id = match client.chain_id().await {
                    Ok(id) => id,
                    Err(_) => {
                        return Json(PrivateNoteSpendResponse {
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
                return Json(PrivateNoteSpendResponse {
                    status: "ERROR".to_string(),
                    message: "EVM client not configured".to_string(),
                    queue_position: 0,
                    estimated_gas_usdc: None,
                });
            }
        };

        let contract_addr = Address::from_str(&contract_address).unwrap_or(Address::ZERO);
        let user_addr_parsed = Address::from_str(user_addr).unwrap_or(Address::ZERO);
        let relayer_addr = Address::from_str(&state.relayer_address).unwrap_or(Address::ZERO);

        let execution_quote = ExecutionQuote {
            quoteId: U256::from_str(quote_id).unwrap_or(U256::ZERO),
            maxExecutionFee: U256::from(payload.max_execution_fee),
            merchantAmount: U256::from(payload.merchant_amount),
            quoteExpiry: U256::from(payload.expiry.unwrap_or(0)),
            relayerAddress: relayer_addr,
        };

        let sig_bytes = match hex::decode(quote_sig.trim_start_matches("0x")) {
            Ok(bytes) if bytes.len() == 65 => bytes,
            _ => {
                return Json(PrivateNoteSpendResponse {
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
                return Json(PrivateNoteSpendResponse {
                    status: "REJECTED".to_string(),
                    message: "Invalid quote signature".to_string(),
                    queue_position: 0,
                    estimated_gas_usdc: None,
                });
            }
            Err(e) => {
                return Json(PrivateNoteSpendResponse {
                    status: "ERROR".to_string(),
                    message: format!("Signature verification failed: {}", e),
                    queue_position: 0,
                    estimated_gas_usdc: None,
                });
            }
        }
    }

    // 10. Double-spend check against local DB
    match state.db.is_nullifier_spent(&payload.input_nullifier).await {
        Ok(true) => {
            return Json(PrivateNoteSpendResponse {
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
            return Json(PrivateNoteSpendResponse {
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
                return Json(PrivateNoteSpendResponse {
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

    // 12. Enqueue into database
    let spend_req = SpendRequest::from(payload.clone());
    let queue_id = match state.db.enqueue_spend(&spend_req).await {
        Ok(Some(id)) => id,
        Ok(None) => {
            return Json(PrivateNoteSpendResponse {
                status: "REJECTED".to_string(),
                message: "Nullifier already queued, submitted, or confirmed.".to_string(),
                queue_position: 0,
                estimated_gas_usdc: None,
            });
        }
        Err(e) => {
            eprintln!("RELAYER ERROR: Failed to persist private note spend: {}", e);
            return Json(PrivateNoteSpendResponse {
                status: "ERROR".to_string(),
                message: "Failed to persist settlement request".to_string(),
                queue_position: 0,
                estimated_gas_usdc: None,
            });
        }
    };

    let position = state.db.queued_spend_count().await.unwrap_or(1);
    println!(
        "RELAYER: Received private note spend request (Queued id={})",
        queue_id
    );

    let response = PrivateNoteSpendResponse {
        status: "QUEUED".to_string(),
        message: format!(
            "Private note spend persisted with settlement id {}",
            queue_id
        ),
        queue_position: position,
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

    Json(response)
}

// Background batching logic with slippage protection and balance checks
pub async fn process_spend_batch(state: &AppState) {
    let items = match state.db.claim_spends(50, 120).await {
        Ok(items) => items,
        Err(e) => {
            eprintln!("SETTLEMENT: failed to claim persistent queue: {}", e);
            return;
        }
    };
    if items.is_empty() {
        return;
    }

    let mut same_chain = Vec::new();
    let mut direct = Vec::new();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    for item in items {
        let is_expired = item.request.deadline.is_some_and(|deadline| now > deadline)
            || item
                .request
                .expiry
                .is_some_and(|expiry| expiry > 0 && now > expiry);

        if is_expired {
            let _ = state
                .db
                .fail_spend(item.id, None, "deadline or expiry expired before broadcast")
                .await;
        } else if item.request.cross_chain.is_some() || item.request.is_private_note() {
            direct.push(item);
        } else {
            // Check if deadline/expiry is approaching (near is defined as <= 30 seconds remaining)
            let is_near = item
                .request
                .deadline
                .is_some_and(|d| d.saturating_sub(now) <= 30)
                || item
                    .request
                    .expiry
                    .is_some_and(|e| e > 0 && e.saturating_sub(now) <= 30);

            if is_near {
                println!(
                    "RELAYER: Deadline/expiry is near for item {}. Bypassing batching.",
                    item.id
                );
                direct.push(item);
            } else {
                same_chain.push(item);
            }
        }
    }

    let batch_enabled = std::env::var("NIMBUS_BATCH_ENABLED")
        .map(|value| value == "true" || value == "1")
        .unwrap_or(false);
    if batch_enabled {
        while same_chain.len() >= 2 {
            let take = same_chain.len().min(8);
            let batch: Vec<_> = same_chain.drain(..take).collect();
            process_same_chain_batch(state, batch).await;
        }
    }
    direct.extend(same_chain);
    for item in direct {
        process_single_spend(state, item).await;
    }
}

fn batch_item(item: &QueuedSpend) -> BatchSpendItem {
    const ZERO_WORD: &str = "0x0000000000000000000000000000000000000000000000000000000000000000";
    BatchSpendItem {
        root_hex: item
            .request
            .association_root_hex
            .clone()
            .unwrap_or_else(|| ZERO_WORD.to_string()),
        nullifier: item.request.nullifier.clone(),
        alpha_neg_hex: item.request.alpha_neg_hex.clone(),
        pk_iss_hex: item.request.pk_iss_hex.clone(),
        recipient: item.request.recipient.clone(),
        amount: item.request.amount,
        recipient_or_intent_hash_hex: item
            .request
            .recipient_or_intent_hash_hex
            .clone()
            .unwrap_or_else(|| ZERO_WORD.to_string()),
        expiry: item.request.expiry.unwrap_or(0),
        nonce_hex: item
            .request
            .nonce_hex
            .clone()
            .unwrap_or_else(|| ZERO_WORD.to_string()),
    }
}

async fn process_same_chain_batch(state: &AppState, items: Vec<QueuedSpend>) {
    let Some(ref evm_client) = state.evm_client else {
        for item in &items {
            let _ = state
                .db
                .retry_spend(item.id, "EVM client unavailable", item.retry_count)
                .await;
        }
        return;
    };

    // --- ETH balance validation ---
    let current_bal = evm_client.get_relayer_balance_eth().await.unwrap_or(0.0);
    *state.relayer_wallet_balance_eth.lock().await = current_bal;

    let gas_price = evm_client.get_gas_price().await.unwrap_or(20_000_000);
    let gas_limit = 250_000f64 + 850_000f64 * items.len() as f64;
    let estimated_cost_eth = (gas_limit * gas_price as f64) / 1_000_000_000_000_000_000.0;

    if let Err(msg) = state.check_balance_for_batch(estimated_cost_eth).await {
        eprintln!("SETTLEMENT BATCH REJECTED: {}", msg);
        for item in &items {
            let _ = state.db.retry_spend(item.id, &msg, item.retry_count).await;
        }
        return;
    }

    let payload: Vec<_> = items.iter().map(batch_item).collect();
    match evm_client.broadcast_spend_batch(&payload).await {
        Ok(outcome) if outcome.success => {
            // Atomic DB confirmation
            let confirm_items: Vec<_> = items
                .iter()
                .map(|item| (item.id, item.request.nullifier.clone()))
                .collect();
            if let Err(e) = state
                .db
                .mark_batch_spend_confirmed(confirm_items, &outcome.tx_hash, outcome.block_number)
                .await
            {
                eprintln!("SETTLEMENT: failed to confirm batch in DB: {}", e);
            }

            // Storing batch metadata & margin calculation (Finding #10)
            let mut total_revenue_usdc = 0.0;
            let mut total_execution_fee_usdc = 0u64;
            for item in &items {
                let amount_usdc = item.request.amount as f64 / 1_000_000.0;
                let fee_usdc = amount_usdc * 0.0025; // 0.25% standard fee
                total_revenue_usdc += fee_usdc;
                // Sum execution fees from signed quotes
                if let Some(max_fee) = item.request.max_execution_fee {
                    total_execution_fee_usdc += max_fee;
                }
            }

            let total_cost_eth = (outcome.gas_used as f64 * outcome.effective_gas_price as f64)
                / 1_000_000_000_000_000_000.0;
            let eth_price = std::env::var("NIMBUS_ETH_PRICE_USDC")
                .ok()
                .and_then(|v| v.parse::<f64>().ok())
                .unwrap_or(3500.0);
            let actual_gas_cost_usdc = total_cost_eth * eth_price;
            let margin_usdc = total_revenue_usdc - actual_gas_cost_usdc;

            if let Err(e) = state
                .db
                .store_batch_metadata(
                    &outcome.tx_hash,
                    items.len(),
                    &outcome.tx_hash,
                    outcome.gas_used,
                    outcome.effective_gas_price,
                    total_cost_eth,
                    margin_usdc,
                    total_execution_fee_usdc,
                )
                .await
            {
                eprintln!("SETTLEMENT: failed to store batch metadata: {}", e);
            }
        }
        Ok(outcome) => {
            // Batch transaction reverted on-chain. Splitting batch for individual execution
            println!("RELAYER WARNING: Batch reverted on-chain (tx: {}). Splitting batch to process items individually...", outcome.tx_hash);
            for item in items {
                let _ = state
                    .db
                    .mark_spend_submitted(item.id, &outcome.tx_hash)
                    .await;
                // Retry individual execution
                process_single_spend(state, item).await;
            }
        }
        Err(error) => {
            // Broadcast error. Splitting batch for individual execution
            let message = error.to_string();
            println!("RELAYER WARNING: Batch broadcast failed: {}. Splitting batch to process items individually...", message);
            for item in items {
                if item.retry_count >= 7 {
                    let _ = state.db.fail_spend(item.id, None, &message).await;
                } else {
                    let _ = state
                        .db
                        .retry_spend(item.id, &message, item.retry_count)
                        .await;
                }
            }
        }
    }
}

async fn process_single_spend(state: &AppState, item: QueuedSpend) {
    const ZERO_WORD: &str = "0x0000000000000000000000000000000000000000000000000000000000000000";
    let Some(ref evm_client) = state.evm_client else {
        let _ = state
            .db
            .retry_spend(item.id, "EVM client unavailable", item.retry_count)
            .await;
        return;
    };
    let request = &item.request;

    // --- ETH balance validation ---
    let current_bal = evm_client.get_relayer_balance_eth().await.unwrap_or(0.0);
    *state.relayer_wallet_balance_eth.lock().await = current_bal;

    let gas_price = evm_client.get_gas_price().await.unwrap_or(20_000_000);
    let gas_limit = if request.is_private_note() {
        2_500_000f64
    } else if request.cross_chain.is_some() {
        1_200_000f64
    } else {
        1_500_000f64
    };
    let estimated_cost_eth = (gas_limit * gas_price as f64) / 1_000_000_000_000_000_000.0;

    if let Err(msg) = state.check_balance_for_batch(estimated_cost_eth).await {
        eprintln!("SETTLEMENT SINGLE REJECTED: {}", msg);
        let _ = state.db.retry_spend(item.id, &msg, item.retry_count).await;
        return;
    }

    let result = if request.is_private_note() {
        let (Some(note_root), Some(proof_a), Some(proof_b), Some(proof_c)) = (
            request.note_root_hex.as_deref(),
            request.proof_a_neg_hex.as_deref(),
            request.proof_b_hex.as_deref(),
            request.proof_c_hex.as_deref(),
        ) else {
            let _ = state
                .db
                .fail_spend(item.id, None, "Missing private note proof parameters")
                .await;
            return;
        };

        evm_client
            .broadcast_spend_private_note_transaction(
                note_root,
                &request.nullifier,
                request
                    .output_commitment_hex
                    .as_deref()
                    .unwrap_or(ZERO_WORD),
                &request.recipient,
                request.merchant_amount.unwrap_or(request.amount),
                request.protocol_fee.unwrap_or(0),
                request.execution_fee.unwrap_or(0),
                request.max_execution_fee.unwrap_or(0),
                request.quote_hash_hex.as_deref().unwrap_or(ZERO_WORD),
                request.expiry.unwrap_or(0),
                request.has_change.unwrap_or(0),
                proof_a,
                proof_b,
                proof_c,
            )
            .await
    } else if let Some(cc) = &request.cross_chain {
        let stablecoin_addr = std::env::var("NIMBUS_STABLECOIN_ADDRESS")
            .unwrap_or_else(|_| "0x75faf114eafb1bdbe2f0316df893fd58ce46aa4d".to_string());
        evm_client
            .broadcast_ccip_transaction(
                cc.destination_chain_selector,
                &cc.destination_contract,
                &request.nullifier,
                &request.alpha_neg_hex,
                &request.pk_iss_hex,
                &request.recipient,
                &stablecoin_addr,
                None,
                request.amount,
                request.expiry.unwrap_or(0),
                request.nonce_hex.as_deref().unwrap_or(ZERO_WORD),
            )
            .await
    } else {
        let root = request.association_root_hex.as_deref().unwrap_or(ZERO_WORD);
        evm_client
            .broadcast_spend_transaction(
                root,
                &request.nullifier,
                &request.alpha_neg_hex,
                &request.pk_iss_hex,
                &request.recipient,
                request.amount,
                request
                    .recipient_or_intent_hash_hex
                    .as_deref()
                    .unwrap_or(ZERO_WORD),
                request.expiry.unwrap_or(0),
                request.nonce_hex.as_deref().unwrap_or(ZERO_WORD),
                request.max_execution_fee.unwrap_or(0),
                request.execution_fee.unwrap_or(0),
            )
            .await
    };

    let is_ccip = request.cross_chain.is_some();
    let ccip_chain_selector = request
        .cross_chain
        .as_ref()
        .map(|cc| cc.destination_chain_selector);

    match result {
        Ok(outcome) if outcome.success => {
            let _ = state
                .db
                .mark_spend_submitted(item.id, &outcome.tx_hash)
                .await;

            if is_ccip {
                // CCIP cross-chain: track destination, don't mark confirmed yet
                if let Some(message_id) = &outcome.ccip_message_id {
                    let chain_sel = ccip_chain_selector.unwrap_or(0);
                    if let Err(e) = state
                        .db
                        .set_ccip_tracking(item.id, message_id, chain_sel)
                        .await
                    {
                        eprintln!(
                            "CCIP_TRACKING: Failed to set tracking for id={}: {}",
                            item.id, e
                        );
                    }
                    // Also insert nullifier to prevent double-spend while awaiting destination
                    let _ = state
                        .db
                        .mark_spend_confirmed(
                            item.id,
                            &request.nullifier,
                            &outcome.tx_hash,
                            outcome.block_number,
                        )
                        .await;
                    println!(
                        "CCIP: Spend {} queued for destination tracking (msg_id={})",
                        item.id, message_id
                    );
                } else {
                    // No message ID parsed — fall back to immediate confirm (graceful degradation)
                    let _ = state
                        .db
                        .mark_spend_confirmed(
                            item.id,
                            &request.nullifier,
                            &outcome.tx_hash,
                            outcome.block_number,
                        )
                        .await;
                }
            } else {
                // Same-chain: mark confirmed immediately
                let _ = state
                    .db
                    .mark_spend_confirmed(
                        item.id,
                        &request.nullifier,
                        &outcome.tx_hash,
                        outcome.block_number,
                    )
                    .await;
            }
        }
        Ok(outcome) => {
            let _ = state
                .db
                .mark_spend_submitted(item.id, &outcome.tx_hash)
                .await;
            let _ = state
                .db
                .fail_spend(
                    item.id,
                    Some(&outcome.tx_hash),
                    "transaction receipt status was reverted",
                )
                .await;
        }
        Err(error) if item.retry_count >= 7 => {
            let _ = state.db.fail_spend(item.id, None, &error.to_string()).await;
        }
        Err(error) => {
            let _ = state
                .db
                .retry_spend(item.id, &error.to_string(), item.retry_count)
                .await;
        }
    }
}

/// CCIP refund endpoint.
///
/// POST /api/ccip/refund
/// Body: { "nullifier": "0x..." }
///
/// Authorizes a refund for a failed CCIP cross-chain spend by freeing the nullifier
/// so the user can re-submit the spend. Only allowed if:
/// 1. destination_status = 'failed' (confirmed by CCIP monitor)
/// 2. Refund timeout has elapsed (default 24h, prevents manual execution race)
#[derive(serde::Deserialize)]
pub struct CcipRefundRequest {
    pub nullifier: String,
}

#[derive(serde::Serialize)]
pub struct CcipRefundResponse {
    pub status: String,
    pub nullifier: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

pub async fn handle_ccip_refund(
    axum::extract::State(state): axum::extract::State<AppState>,
    Json(payload): Json<CcipRefundRequest>,
) -> Json<CcipRefundResponse> {
    let refund_timeout = crate::config::CcipDestinationConfig::from_env().refund_timeout_secs;

    match state
        .db
        .authorize_ccip_refund(&payload.nullifier, refund_timeout)
        .await
    {
        Ok(true) => {
            println!(
                "CCIP_REFUND: Nullifier freed for re-spend: {}...",
                &payload.nullifier[..8.min(payload.nullifier.len())]
            );
            Json(CcipRefundResponse {
                status: "refund_authorized".to_string(),
                nullifier: payload.nullifier,
                error: None,
            })
        }
        Ok(false) => Json(CcipRefundResponse {
            status: "refund_denied".to_string(),
            nullifier: payload.nullifier,
            error: Some(
                "Not eligible: destination not failed, or refund timeout not yet elapsed"
                    .to_string(),
            ),
        }),
        Err(e) => Json(CcipRefundResponse {
            status: "error".to_string(),
            nullifier: payload.nullifier,
            error: Some(e.to_string()),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::extract::State;
    use nimbus_core::{Fr, PrimeGroup};
    use tempfile::TempDir;

    async fn setup_test_state() -> (AppState, TempDir) {
        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("spend_test.db");
        let db = crate::database::Database::new(&db_path).await.unwrap();

        let share_sk = Fr::from(12345u64);
        let share_index = 1;
        let issuer_pk =
            nimbus_core::IssuerPublicKey(nimbus_core::G2Projective::generator() * share_sk);

        let state = AppState::new(
            db,
            share_sk,
            share_index,
            issuer_pk,
            std::collections::HashMap::new(),
            None,
        )
        .await;

        (state, tmp)
    }

    fn sample_valid_private_note_request() -> PrivateNoteSpendRequest {
        let note_root = "0x0000000000000000000000000000000000000000000000000000000000000001";
        let input_nullifier = "0x0000000000000000000000000000000000000000000000000000000000000002";
        let output_commitment =
            "0x0000000000000000000000000000000000000000000000000000000000000003";
        let recipient = "0x1111111111111111111111111111111111111111";

        PrivateNoteSpendRequest {
            session_id: Some("session_1".to_string()),
            note_root: note_root.to_string(),
            input_nullifier: input_nullifier.to_string(),
            output_commitment: Some(output_commitment.to_string()),
            recipient: recipient.to_string(),
            merchant_amount: 5_000_000,
            protocol_fee: 22_500,
            execution_fee: 20_000,
            max_execution_fee: 25_000,
            quote_hash: Some(
                "0x0000000000000000000000000000000000000000000000000000000000000004".to_string(),
            ),
            quote_signature: None,
            quote_id: None,
            user_address: None,
            expiry: Some(9999999999),
            has_change: Some(serde_json::Value::Number(1.into())),
            proof_a_neg: format!("0x{}", "11".repeat(128)),
            proof_b: format!("0x{}", "22".repeat(256)),
            proof_c: format!("0x{}", "33".repeat(128)),
            public_inputs: vec![
                note_root.to_string(),
                input_nullifier.to_string(),
                output_commitment.to_string(),
                format!("0x000000000000000000000000{}", &recipient[2..]),
                format!("0x{:064x}", 5_000_000u64),
                format!("0x{:064x}", 22_500u64),
                format!("0x{:064x}", 20_000u64),
                "0x0000000000000000000000000000000000000000000000000000000000000004".to_string(),
                format!("0x{:064x}", 421614u64),
                format!(
                    "0x000000000000000000000000{}",
                    "3333333333333333333333333333333333333333"
                ),
                format!("0x{:064x}", 9999999999u64),
                format!("0x{:064x}", 1u64),
            ],
            idempotency_key: Some("idem_valid_1".to_string()),
        }
    }

    #[tokio::test]
    async fn test_handle_private_note_spend_success() {
        let (state, _tmp) = setup_test_state().await;
        let req = sample_valid_private_note_request();

        let resp = handle_private_note_spend(State(state.clone()), Json(req.clone())).await;
        assert_eq!(resp.0.status, "QUEUED");
        assert!(resp.0.message.contains("settlement id"));

        // Test idempotency returns cached response
        let resp_cached = handle_private_note_spend(State(state.clone()), Json(req)).await;
        assert_eq!(resp_cached.0.status, "QUEUED");
    }

    #[tokio::test]
    async fn test_handle_private_note_spend_reject_non_canonical_public_input() {
        let (state, _tmp) = setup_test_state().await;
        let mut req = sample_valid_private_note_request();
        // Replace public input at index 0 with field overflow >= r
        req.public_inputs[0] =
            "0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff".to_string();

        let resp = handle_private_note_spend(State(state), Json(req)).await;
        assert_eq!(resp.0.status, "REJECTED");
        assert!(resp.0.message.contains("non-canonical scalar"));
    }

    #[tokio::test]
    async fn test_handle_private_note_spend_reject_semantic_mismatch() {
        let (state, _tmp) = setup_test_state().await;
        let mut req = sample_valid_private_note_request();
        // Tamper with merchant_amount in payload (5M vs 6M in public input)
        req.merchant_amount = 6_000_000;

        let resp = handle_private_note_spend(State(state), Json(req)).await;
        assert_eq!(resp.0.status, "REJECTED");
        assert!(resp.0.message.contains("does not match merchant_amount"));
    }

    #[tokio::test]
    async fn test_handle_private_note_spend_reject_execution_fee_exceeds_max() {
        let (state, _tmp) = setup_test_state().await;
        let mut req = sample_valid_private_note_request();
        req.execution_fee = 30_000;
        req.max_execution_fee = 25_000;

        let resp = handle_private_note_spend(State(state), Json(req)).await;
        assert_eq!(resp.0.status, "REJECTED");
        assert!(resp.0.message.contains("exceeds max execution fee"));
    }

    #[tokio::test]
    async fn test_handle_private_note_spend_reject_nonzero_commitment_with_zero_change() {
        let (state, _tmp) = setup_test_state().await;
        let mut req = sample_valid_private_note_request();
        req.has_change = Some(serde_json::Value::Number(0.into()));
        req.output_commitment =
            Some("0x0000000000000000000000000000000000000000000000000000000000000001".to_string());

        let resp = handle_private_note_spend(State(state), Json(req)).await;
        assert_eq!(resp.0.status, "REJECTED");
        assert!(resp.0.message.contains("nonzero output_commitment"));
    }

    #[tokio::test]
    async fn test_handle_private_note_spend_reject_double_spend() {
        let (state, _tmp) = setup_test_state().await;
        let req = sample_valid_private_note_request();

        // Mark nullifier as spent in DB first
        state
            .db
            .check_and_insert_nullifier(&req.input_nullifier, Some("0xtx"))
            .await
            .unwrap();

        let resp = handle_private_note_spend(State(state), Json(req)).await;
        assert_eq!(resp.0.status, "REJECTED");
        assert!(resp.0.message.contains("Double-spending detected"));
    }
}
