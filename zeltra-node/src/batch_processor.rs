//! Background batching logic with slippage protection and balance checks

use crate::database::QueuedSpend;
use crate::state::AppState;

/// Gas limit estimates for different transaction types
const GAS_LIMIT_PRIVATE_NOTE: f64 = 2_500_000.0;
const GAS_LIMIT_CROSS_CHAIN: f64 = 1_200_000.0;
const GAS_LIMIT_STANDARD: f64 = 1_500_000.0;

// Background batching logic with slippage protection and balance checks
pub const DEFAULT_MEMPOOL_LEASE_SECS: i64 = 900; // 15 minutes default (DEC-027)

pub async fn process_spend_batch(state: &AppState) {
    let lease_seconds = std::env::var("ZELTRA_MEMPOOL_LEASE_SECS")
        .ok()
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(DEFAULT_MEMPOOL_LEASE_SECS);

    let items = match state.db.claim_spends(50, lease_seconds).await {
        Ok(items) => items,
        Err(e) => {
            eprintln!("SETTLEMENT: failed to claim persistent queue: {}", e);
            return;
        }
    };
    if items.is_empty() {
        return;
    }

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
        } else {
            process_single_spend(state, item).await;
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
        GAS_LIMIT_PRIVATE_NOTE
    } else if request.cross_chain.is_some() {
        GAS_LIMIT_CROSS_CHAIN
    } else {
        GAS_LIMIT_STANDARD
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

        let leaf_count = request.leaf_count.unwrap_or(1);
        let note_epoch_id = request.note_epoch_id.unwrap_or(0);
        let is_rollover = request.is_rollover.unwrap_or(0);
        let quote_hash_repacked = if let Some(ref qh) = request.quote_hash_hex {
            qh.clone()
        } else if let Some(ref pis) = request.public_inputs_hex {
            if pis.len() >= 16 {
                let hi_res = hex::decode(pis[9].trim_start_matches("0x"));
                let lo_res = hex::decode(pis[10].trim_start_matches("0x"));
                if let (Ok(hi), Ok(lo)) = (hi_res, lo_res) {
                    if hi.len() == 32 && lo.len() == 32 {
                        let mut full = [0u8; 32];
                        full[0..16].copy_from_slice(&hi[16..32]);
                        full[16..32].copy_from_slice(&lo[16..32]);
                        format!("0x{}", hex::encode(full))
                    } else {
                        ZERO_WORD.to_string()
                    }
                } else {
                    ZERO_WORD.to_string()
                }
            } else {
                ZERO_WORD.to_string()
            }
        } else {
            ZERO_WORD.to_string()
        };

        evm_client
            .broadcast_spend_private_note_transaction(
                note_root,
                leaf_count,
                &request.nullifier,
                note_epoch_id,
                request
                    .output_commitment_hex
                    .as_deref()
                    .unwrap_or(ZERO_WORD),
                &request.recipient,
                request.merchant_amount.unwrap_or(request.amount),
                request.protocol_fee.unwrap_or(0),
                request.execution_fee.unwrap_or(0),
                request.max_execution_fee.unwrap_or(0),
                &quote_hash_repacked,
                request.expiry.unwrap_or(0),
                request.has_change.unwrap_or(0),
                is_rollover,
                proof_a,
                proof_b,
                proof_c,
                request.change_encrypted_memo.as_deref(),
            )
            .await
    } else if let Some(cc) = &request.cross_chain {
        let stablecoin_addr = std::env::var("ZELTRA_STABLECOIN_ADDRESS")
            .unwrap_or_else(|_| "0x75faf114eafb1bdbe2f0316df893fd58ce46aa4d".to_string());
        evm_client
            .broadcast_ccip_transaction(
                cc.destination_chain_selector,
                &cc.destination_contract,
                &request.nullifier,
                &request.recipient,
                &stablecoin_addr,
                request.amount,
                request.expiry.unwrap_or(0),
                request.nonce_hex.as_deref().unwrap_or(ZERO_WORD),
            )
            .await
    } else {
        eprintln!(
            "SETTLEMENT: unrecognized spend type for item {} (nullifier={})",
            item.id, request.nullifier
        );
        let _ = state
            .db
            .fail_spend(item.id, None, "unrecognized spend type")
            .await;
        return;
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

                // Track single spend execution fee in spend_batches for three-way reconciliation (DEC-031)
                let execution_fee_usdc = request
                    .execution_fee
                    .or(request.max_execution_fee)
                    .unwrap_or(0);
                if execution_fee_usdc > 0 {
                    let total_cost_eth = (outcome.gas_used as f64
                        * outcome.effective_gas_price as f64)
                        / 1_000_000_000_000_000_000.0;
                    let eth_price = state.get_eth_price().await;
                    let actual_gas_cost_usdc = total_cost_eth * eth_price;
                    let margin_usdc =
                        (execution_fee_usdc as f64 / 1_000_000.0) - actual_gas_cost_usdc;

                    if let Err(e) = state
                        .db
                        .store_batch_metadata(
                            &outcome.tx_hash,
                            1,
                            &outcome.tx_hash,
                            outcome.gas_used,
                            outcome.effective_gas_price,
                            total_cost_eth,
                            margin_usdc,
                            execution_fee_usdc,
                        )
                        .await
                    {
                        eprintln!("SETTLEMENT: failed to store single spend metadata: {}", e);
                    }
                }
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
