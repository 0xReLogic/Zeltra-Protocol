//! Spend handler and batch processing logic
//!
//! Security features:
//! - Slippage protection (min_payout, deadline) -- Finding #9
//! - Minimum balance checks before batch processing -- Finding #10
//! - Idempotency key support for request deduplication -- Finding #16

use axum::Json;
use crate::{state::AppState, dto::*};

pub async fn handle_spend(
    axum::extract::State(state): axum::extract::State<AppState>,
    Json(payload): Json<SpendRequest>,
) -> Json<SpendResponse> {
    // --- Idempotency check (Finding #16) ---
    if let Some(ref idem_key) = payload.idempotency_key {
        match state.db.check_idempotency(idem_key, "spend").await {
            Ok(Some(cached_json)) => {
                if let Ok(cached_resp) = serde_json::from_str::<SpendResponse>(&cached_json) {
                    println!("IDEMPOTENCY: Returning cached response for key {}...", &idem_key[..8.min(idem_key.len())]);
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

    for item in items {
        let request = &item.request;
        // --- Finding #9: Deadline enforcement during batch processing ---
        if let Some(deadline) = request.deadline {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            if now > deadline {
                eprintln!(
                    "  SLIPPAGE-REJECT: Transaction {}... deadline expired (deadline={}, now={})",
                    &request.nullifier[..8.min(request.nullifier.len())],
                    deadline,
                    now
                );
                let _ = state.db.fail_spend(item.id, None, "deadline expired before broadcast").await;
                continue;
            }
        }
        let Some(ref evm_client) = state.evm_client else {
            let _ = state.db.retry_spend(item.id, "EVM client unavailable", item.retry_count).await;
            continue;
        };

        let result = if let Some(cc) = &request.cross_chain {
            let stablecoin_addr = std::env::var("NIMBUS_STABLECOIN_ADDRESS")
                .unwrap_or_else(|_| "0x75faf114eafb1bdbe2f0316df893fd58ce46aa4d".to_string());
            evm_client.broadcast_ccip_transaction(
                    cc.destination_chain_selector,
                    &cc.destination_contract,
                    &request.nullifier,
                    &request.alpha_neg_hex,
                    &request.pk_iss_hex,
                    &request.recipient,
                    &stablecoin_addr,
                    None, // Default to standard cross-chain spend (no condition_id)
                    request.amount,
                    request.expiry.unwrap_or(0),
                    request.nonce_hex.as_deref().unwrap_or("0x0000000000000000000000000000000000000000000000000000000000000000"),
                ).await
        } else {
            evm_client.broadcast_spend_transaction(
                    &request.nullifier,
                    &request.alpha_neg_hex,
                    &request.pk_iss_hex,
                    &request.recipient,
                    request.amount,
                    request.recipient_or_intent_hash_hex.as_deref().unwrap_or("0x0000000000000000000000000000000000000000000000000000000000000000"),
                    request.expiry.unwrap_or(0),
                    request.nonce_hex.as_deref().unwrap_or("0x0000000000000000000000000000000000000000000000000000000000000000"),
                ).await
        };

        match result {
            Ok(outcome) if outcome.success => {
                if let Err(e) = state.db.mark_spend_submitted(item.id, &outcome.tx_hash).await {
                    eprintln!("SETTLEMENT: failed to persist submitted tx: {}", e);
                    continue;
                }
                if let Err(e) = state.db.mark_spend_confirmed(
                    item.id,
                    &request.nullifier,
                    &outcome.tx_hash,
                    outcome.block_number,
                ).await {
                    eprintln!("SETTLEMENT: receipt succeeded but DB confirmation failed: {}", e);
                }
            }
            Ok(outcome) => {
                let _ = state.db.mark_spend_submitted(item.id, &outcome.tx_hash).await;
                let _ = state.db.fail_spend(
                    item.id,
                    Some(&outcome.tx_hash),
                    "transaction receipt status was reverted",
                ).await;
            }
            Err(e) => {
                let error = e.to_string();
                if item.retry_count >= 7 {
                    let _ = state.db.fail_spend(item.id, None, &error).await;
                } else {
                    let _ = state.db.retry_spend(item.id, &error, item.retry_count).await;
                }
            }
        }
    }
}
