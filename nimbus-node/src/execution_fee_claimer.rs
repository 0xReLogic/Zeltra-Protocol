//! Execution Fee Claimer Background Worker
//!
//! Periodically checks for unclaimed execution fees and claims them from the contract
//! when the accumulated amount exceeds a threshold.

use crate::state::AppState;
use std::time::Duration;
use tokio::time::interval;

/// Minimum threshold in USDC (6 decimals) to trigger a claim
const CLAIM_THRESHOLD_USDC: u64 = 50_000_000; // $50

/// Check interval for claiming execution fees
const CHECK_INTERVAL_SECS: u64 = 300; // 5 minutes

/// Background worker that claims accumulated execution fees from the contract
pub async fn execution_fee_claimer_worker(state: AppState) {
    println!("EXECUTION_FEE_CLAIMER: Worker started");
    println!("  Threshold: {} USDC", CLAIM_THRESHOLD_USDC / 1_000_000);
    println!("  Interval: {} seconds", CHECK_INTERVAL_SECS);

    let mut ticker = interval(Duration::from_secs(CHECK_INTERVAL_SECS));

    loop {
        ticker.tick().await;

        // Query unclaimed execution fees
        let (unclaimed, _) = match state.db.get_unclaimed_execution_fees().await {
            Ok(fees) => fees,
            Err(e) => {
                eprintln!("EXECUTION_FEE_CLAIMER: Failed to query unclaimed fees: {}", e);
                continue;
            }
        };

        if unclaimed < CLAIM_THRESHOLD_USDC {
            continue;
        }

        println!(
            "EXECUTION_FEE_CLAIMER: Found {} USDC unclaimed, attempting claim...",
            unclaimed / 1_000_000
        );

        // Get the EVM client
        let evm_client = match &state.evm_client {
            Some(client) => client,
            None => {
                eprintln!("EXECUTION_FEE_CLAIMER: No EVM client available");
                continue;
            }
        };

        // Call claim_execution_fees on the contract
        match evm_client.claim_execution_fees(unclaimed).await {
            Ok(outcome) if outcome.success => {
                println!(
                    "EXECUTION_FEE_CLAIMER: Successfully claimed {} USDC (tx: {})",
                    unclaimed / 1_000_000,
                    outcome.tx_hash
                );

                // Mark all unclaimed batches as claimed
                let batch_ids = match state.db.get_unclaimed_batch_ids().await {
                    Ok(ids) => ids,
                    Err(e) => {
                        eprintln!(
                            "EXECUTION_FEE_CLAIMER: Failed to get unclaimed batch IDs: {}",
                            e
                        );
                        continue;
                    }
                };

                if let Err(e) = state.db.mark_batches_claimed(&batch_ids).await {
                    eprintln!(
                        "EXECUTION_FEE_CLAIMER: Failed to mark batches as claimed: {}",
                        e
                    );
                }
            }
            Ok(outcome) => {
                eprintln!(
                    "EXECUTION_FEE_CLAIMER: Claim transaction reverted (tx: {})",
                    outcome.tx_hash
                );
            }
            Err(e) => {
                eprintln!("EXECUTION_FEE_CLAIMER: Failed to claim execution fees: {}", e);
            }
        }
    }
}
