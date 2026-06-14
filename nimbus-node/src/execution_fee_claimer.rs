//! Execution Fee Claimer Background Worker
//!
//! Periodically checks for unclaimed execution fees and claims them from the contract
//! when the accumulated amount or transaction count exceeds thresholds.

use crate::state::AppState;
use std::time::Duration;
use tokio::time::interval;

/// Minimum threshold in USDC (6 decimals) to trigger a claim
/// Testnet: $1 (1_000_000) — easy to test with small amounts
/// Production: $100+ (100_000_000) — amortize gas cost over more transactions
const CLAIM_THRESHOLD_USDC: u64 = 1_000_000; // $1 for testnet

/// Minimum number of unclaimed transactions to trigger a claim
/// Testnet: 10 transactions — quick feedback loop
/// Production: 50+ transactions — better gas amortization
const CLAIM_THRESHOLD_TX_COUNT: u64 = 10; // 10 for testnet

/// Check interval for claiming execution fees
const CHECK_INTERVAL_SECS: u64 = 300; // 5 minutes

/// Background worker that claims accumulated execution fees from the contract
pub async fn execution_fee_claimer_worker(state: AppState) {
    println!("EXECUTION_FEE_CLAIMER: Worker started");
    println!("  Threshold: {} USDC OR {} transactions", 
             CLAIM_THRESHOLD_USDC / 1_000_000, CLAIM_THRESHOLD_TX_COUNT);
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

        // Get unclaimed batch count
        let unclaimed_count = match state.db.get_unclaimed_batch_ids().await {
            Ok(ids) => ids.len() as u64,
            Err(e) => {
                eprintln!("EXECUTION_FEE_CLAIMER: Failed to query unclaimed batch count: {}", e);
                continue;
            }
        };

        // Hybrid trigger: claim if USD threshold OR transaction count threshold met
        let usdc_threshold_met = unclaimed >= CLAIM_THRESHOLD_USDC;
        let count_threshold_met = unclaimed_count >= CLAIM_THRESHOLD_TX_COUNT;

        if !usdc_threshold_met && !count_threshold_met {
            continue;
        }

        let trigger_reason = if usdc_threshold_met && count_threshold_met {
            format!("both thresholds met ({} USDC AND {} tx)", 
                    unclaimed / 1_000_000, unclaimed_count)
        } else if usdc_threshold_met {
            format!("USD threshold met ({} USDC)", unclaimed / 1_000_000)
        } else {
            format!("transaction count threshold met ({} tx)", unclaimed_count)
        };

        println!(
            "EXECUTION_FEE_CLAIMER: Claiming {} USDC from {} batches — {}",
            unclaimed / 1_000_000, unclaimed_count, trigger_reason
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
