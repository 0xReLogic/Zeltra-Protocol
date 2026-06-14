//! Execution Fee Claimer Background Worker
//!
//! Periodically checks for unclaimed execution fees and claims them from the contract
//! when either USD threshold or transaction count threshold is reached.
//!
//! Hybrid trigger logic:
//! - Testnet (development/test/hard-test): $1 USD OR 10 transactions
//! - Production (production/mainnet): $100 USD OR 50 transactions

use crate::config::RuntimeMode;
use crate::state::AppState;
use std::time::Duration;
use tokio::time::interval;

/// Threshold configuration based on runtime mode
struct ClaimThreshold {
    usdc: u64,           // USDC in 6 decimals
    tx_count: usize,
}

impl ClaimThreshold {
    fn for_mode(mode: RuntimeMode) -> Self {
        match mode {
            RuntimeMode::Development | RuntimeMode::Test | RuntimeMode::HardTest => {
                // Testnet: claim early for testing
                Self {
                    usdc: 1_000_000,      // $1 USD
                    tx_count: 10,
                }
            }
            RuntimeMode::Production | RuntimeMode::Mainnet => {
                // Production: optimize for gas efficiency
                Self {
                    usdc: 100_000_000,    // $100 USD
                    tx_count: 50,
                }
            }
        }
    }
}

/// Check interval for claiming execution fees
const CHECK_INTERVAL_SECS: u64 = 300; // 5 minutes

/// Background worker that claims accumulated execution fees from the contract
pub async fn execution_fee_claimer_worker(state: AppState) {
    let mode = RuntimeMode::from_env();
    let threshold = ClaimThreshold::for_mode(mode);
    
    println!("EXECUTION_FEE_CLAIMER: Worker started (mode: {})", mode.label());
    println!("  Threshold: {} USDC OR {} transactions", 
             threshold.usdc / 1_000_000, threshold.tx_count);
    println!("  Interval: {} seconds", CHECK_INTERVAL_SECS);

    let mut ticker = interval(Duration::from_secs(CHECK_INTERVAL_SECS));

    loop {
        ticker.tick().await;

        // Query unclaimed execution fees and batch IDs
        let (unclaimed, batch_ids) = match state.db.get_unclaimed_execution_fees().await {
            Ok(fees) => fees,
            Err(e) => {
                eprintln!("EXECUTION_FEE_CLAIMER: Failed to query unclaimed fees: {}", e);
                continue;
            }
        };

        let batch_count = batch_ids.len();
        
        // Hybrid trigger: claim if USD threshold OR transaction count threshold reached
        if unclaimed < threshold.usdc && batch_count < threshold.tx_count {
            continue;
        }

        let reason = if unclaimed >= threshold.usdc {
            format!("USD threshold reached ({} >= {} USDC)", 
                   unclaimed / 1_000_000, threshold.usdc / 1_000_000)
        } else {
            format!("Transaction count threshold reached ({} >= {})", 
                   batch_count, threshold.tx_count)
        };
        
        println!(
            "EXECUTION_FEE_CLAIMER: {} — {} USDC from {} batches, attempting claim...",
            reason,
            unclaimed / 1_000_000,
            batch_count
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
                    "EXECUTION_FEE_CLAIMER: Successfully claimed {} USDC from {} batches (tx: {})",
                    unclaimed / 1_000_000,
                    batch_count,
                    outcome.tx_hash
                );

                // Mark all unclaimed batches as claimed (we already have the IDs)
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
