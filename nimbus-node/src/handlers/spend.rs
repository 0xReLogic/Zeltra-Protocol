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
        } else if item.request.cross_chain.is_some() {
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
    let gas_limit = if request.cross_chain.is_some() {
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

    let result = if let Some(cc) = &request.cross_chain {
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
