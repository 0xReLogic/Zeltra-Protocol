//! Execution Fee Claimer Background Worker & Three-Way Reconciliation Engine (DEC-031)
//!
//! Periodically reconciles and claims accumulated execution fees from the contract:
//! 1. Contract on-chain accrued fee liability (`accrued_execution_fee_liability` / `get_accumulated_fees`)
//! 2. Database `spend_batches` unclaimed entries (both single-spend and batch records)
//! 3. Transaction receipt confirmations (ensures no unconfirmed or reverted receipts are counted)
//!
//! Enforces:
//! - `claim_amount = min(verified_receipt_fees, contract_accrued_liability)`
//! - Prevents transaction broadcast when claimable amount is 0 (avoids `INSUFFICIENT_ACCUMULATED_FEES` revert)
//! - Marks only verified, claimed batches upon receipt confirmation

use crate::database::{Database, UnclaimedBatchInfo};
use crate::evm_client::EvmClient;
use crate::state::AppState;
use anyhow::Result;
use std::time::Duration;
use tokio::time::interval;

/// Minimum threshold in USDC (6 decimals) to trigger a claim
/// Testnet: $1 (1_000_000) — easy to test with small amounts
/// Production: $100+ (100_000_000) — amortize gas cost over more transactions
pub const CLAIM_THRESHOLD_USDC: u64 = 1_000_000; // $1 for testnet

/// Minimum number of unclaimed transactions to trigger a claim
/// Testnet: 10 transactions — quick feedback loop
/// Production: 50+ transactions — better gas amortization
pub const CLAIM_THRESHOLD_TX_COUNT: u64 = 10; // 10 for testnet

/// Check interval for claiming execution fees
pub const CHECK_INTERVAL_SECS: u64 = 300; // 5 minutes

/// Result summary of three-way reconciliation (DEC-031)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReconciliationSummary {
    /// Total on-chain accrued execution fee liability in USDC
    pub contract_accrued_usdc: u64,
    /// Total unclaimed execution fee recorded in DB
    pub db_unclaimed_usdc: u64,
    /// Fees from batches whose transaction receipt has been verified as confirmed & succeeded
    pub verified_receipt_fees_usdc: u64,
    /// Reconciled claim amount = min(verified_receipt_fees_usdc, contract_accrued_usdc)
    pub claimable_amount_usdc: u64,
    /// Batch IDs corresponding to the verified receipts that will be marked claimed
    pub claimable_batch_ids: Vec<String>,
    /// Drift between contract accrual and DB unclaimed: (contract_accrued as i64 - db_unclaimed as i64)
    pub drift_usdc: i64,
    /// Whether reconciliation found exact balance agreement
    pub is_reconciled: bool,
}

/// Pure deterministic calculation of claim amount and eligible batches based on contract accrual
/// and verified receipts (used for unit testing and reconciliation capping)
pub fn compute_reconciliation(
    contract_accrued_usdc: u64,
    verified_batches: &[UnclaimedBatchInfo],
) -> (u64, Vec<String>, i64) {
    let total_verified: u64 = verified_batches.iter().map(|b| b.execution_fee_usdc).sum();
    let claimable_amount = std::cmp::min(total_verified, contract_accrued_usdc);
    let drift = contract_accrued_usdc as i64 - total_verified as i64;

    // Select batches greedily in FIFO order up to claimable_amount
    let mut accumulated = 0u64;
    let mut selected_ids = Vec::new();

    for batch in verified_batches {
        if accumulated.saturating_add(batch.execution_fee_usdc) <= claimable_amount {
            accumulated = accumulated.saturating_add(batch.execution_fee_usdc);
            selected_ids.push(batch.batch_id.clone());
        }
    }

    (claimable_amount, selected_ids, drift)
}

/// Perform asynchronous three-way reconciliation between:
/// 1. Contract on-chain accrued fee liability
/// 2. DB spend_batches unclaimed entries
/// 3. Confirmed on-chain or local receipts
pub async fn reconcile_execution_fees(
    db: &Database,
    evm_client: &EvmClient,
) -> Result<ReconciliationSummary> {
    // 1. Query on-chain contract state
    let contract_accrued_usdc = evm_client.get_accrued_execution_fee_liability().await?;

    // 2. Query DB unclaimed batches
    let unclaimed_batches = db.get_unclaimed_batches_with_details().await?;
    let db_unclaimed_usdc: u64 = unclaimed_batches.iter().map(|b| b.execution_fee_usdc).sum();

    // 3. Verify transaction receipts
    let mut verified_batches = Vec::new();
    for batch in unclaimed_batches {
        // Check local relayer_transactions table first
        let local_status = match db.get_relayer_tx(&batch.tx_hash).await {
            Ok(Some(tx)) => Some(tx.status),
            _ => None,
        };

        let is_confirmed = match local_status.as_deref() {
            Some("confirmed") => true,
            Some("failed") => false, // explicitly reverted
            _ => {
                // If not marked confirmed locally, check on-chain receipt directly
                match evm_client.get_receipt(&batch.tx_hash).await {
                    Ok(Some(receipt)) => receipt.status(),
                    _ => false, // missing or pending in mempool
                }
            }
        };

        if is_confirmed {
            verified_batches.push(batch);
        }
    }

    let verified_receipt_fees_usdc: u64 =
        verified_batches.iter().map(|b| b.execution_fee_usdc).sum();
    let (claimable_amount_usdc, claimable_batch_ids, drift_usdc) =
        compute_reconciliation(contract_accrued_usdc, &verified_batches);

    let is_reconciled = drift_usdc == 0 && verified_receipt_fees_usdc == db_unclaimed_usdc;

    Ok(ReconciliationSummary {
        contract_accrued_usdc,
        db_unclaimed_usdc,
        verified_receipt_fees_usdc,
        claimable_amount_usdc,
        claimable_batch_ids,
        drift_usdc,
        is_reconciled,
    })
}

/// Background worker that claims accumulated execution fees from the contract
pub async fn execution_fee_claimer_worker(state: AppState) {
    println!("EXECUTION_FEE_CLAIMER: Worker started");
    println!(
        "  Threshold: {} USDC OR {} transactions",
        CLAIM_THRESHOLD_USDC / 1_000_000,
        CLAIM_THRESHOLD_TX_COUNT
    );
    println!("  Interval: {} seconds", CHECK_INTERVAL_SECS);

    let mut ticker = interval(Duration::from_secs(CHECK_INTERVAL_SECS));

    loop {
        ticker.tick().await;

        // Get the EVM client
        let evm_client = match &state.evm_client {
            Some(client) => client,
            None => {
                eprintln!("EXECUTION_FEE_CLAIMER: No EVM client available, skipping claim cycle");
                continue;
            }
        };

        // Run Three-Way Reconciliation Engine (DEC-031)
        let summary = match reconcile_execution_fees(&state.db, evm_client).await {
            Ok(s) => s,
            Err(e) => {
                eprintln!(
                    "EXECUTION_FEE_CLAIMER: Three-way reconciliation failed: {}",
                    e
                );
                continue;
            }
        };

        if summary.drift_usdc != 0 {
            println!(
                "EXECUTION_FEE_CLAIMER: Notice: Accounting drift detected: contract_accrued={} USDC, db_unclaimed={} USDC (drift: {} USDC)",
                summary.contract_accrued_usdc / 1_000_000,
                summary.db_unclaimed_usdc / 1_000_000,
                summary.drift_usdc as f64 / 1_000_000.0
            );
        }

        // Hybrid trigger: claim if USD threshold OR transaction count threshold met
        let usdc_threshold_met = summary.claimable_amount_usdc >= CLAIM_THRESHOLD_USDC;
        let count_threshold_met =
            summary.claimable_batch_ids.len() >= CLAIM_THRESHOLD_TX_COUNT as usize;

        if (!usdc_threshold_met && !count_threshold_met) || summary.claimable_amount_usdc == 0 {
            continue;
        }

        let trigger_reason = if usdc_threshold_met && count_threshold_met {
            format!(
                "both thresholds met ({} USDC AND {} tx)",
                summary.claimable_amount_usdc / 1_000_000,
                summary.claimable_batch_ids.len()
            )
        } else if usdc_threshold_met {
            format!(
                "USD threshold met ({} USDC)",
                summary.claimable_amount_usdc / 1_000_000
            )
        } else {
            format!(
                "transaction count threshold met ({} tx)",
                summary.claimable_batch_ids.len()
            )
        };

        println!(
            "EXECUTION_FEE_CLAIMER: Claiming {} USDC from {} verified batches — {}",
            summary.claimable_amount_usdc / 1_000_000,
            summary.claimable_batch_ids.len(),
            trigger_reason
        );

        // Call claim_execution_fees on the contract
        match evm_client
            .claim_execution_fees(summary.claimable_amount_usdc)
            .await
        {
            Ok(outcome) if outcome.success => {
                println!(
                    "EXECUTION_FEE_CLAIMER: Successfully claimed {} USDC (tx: {})",
                    summary.claimable_amount_usdc / 1_000_000,
                    outcome.tx_hash
                );

                // Mark reconciled batches as claimed
                if let Err(e) = state
                    .db
                    .mark_batches_claimed(&summary.claimable_batch_ids)
                    .await
                {
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
                eprintln!(
                    "EXECUTION_FEE_CLAIMER: Failed to claim execution fees: {}",
                    e
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_batch(id: &str, tx: &str, fee: u64) -> UnclaimedBatchInfo {
        UnclaimedBatchInfo {
            batch_id: id.to_string(),
            tx_hash: tx.to_string(),
            execution_fee_usdc: fee,
            item_count: 1,
            created_at: 1000,
        }
    }

    #[test]
    fn test_compute_reconciliation_exact_match() {
        let batches = vec![
            sample_batch("batch_1", "0xaaa", 500_000),
            sample_batch("batch_2", "0xbbb", 500_000),
        ];

        let contract_accrued = 1_000_000;
        let (claimable, ids, drift) = compute_reconciliation(contract_accrued, &batches);

        assert_eq!(claimable, 1_000_000);
        assert_eq!(ids.len(), 2);
        assert_eq!(drift, 0);
    }

    #[test]
    fn test_compute_reconciliation_caps_at_contract_accrual_when_db_higher() {
        // DB has $1.50 in unclaimed receipts, but contract only recorded $1.00 accrued
        let batches = vec![
            sample_batch("batch_1", "0xaaa", 500_000),
            sample_batch("batch_2", "0xbbb", 500_000),
            sample_batch("batch_3", "0xccc", 500_000),
        ];

        let contract_accrued = 1_000_000; // Contract only has $1.00
        let (claimable, ids, drift) = compute_reconciliation(contract_accrued, &batches);

        // DEC-031: Must cap claimable amount strictly at contract accrual ($1.00)
        assert_eq!(claimable, 1_000_000);
        assert_eq!(ids.len(), 2); // only first two batches fit in $1.00
        assert_eq!(drift, -500_000); // DB is 500k higher than contract
    }

    #[test]
    fn test_compute_reconciliation_zero_when_contract_accrual_zero() {
        // Contract has 0 accrued fee
        let batches = vec![sample_batch("batch_1", "0xaaa", 1_000_000)];

        let contract_accrued = 0;
        let (claimable, ids, drift) = compute_reconciliation(contract_accrued, &batches);

        // Fail-closed: claimable must be 0 to avoid contract revert
        assert_eq!(claimable, 0);
        assert!(ids.is_empty());
        assert_eq!(drift, -1_000_000);
    }

    #[test]
    fn test_compute_reconciliation_handles_contract_accrual_higher_than_db() {
        // Contract has $2.00, DB only verified $1.00
        let batches = vec![sample_batch("batch_1", "0xaaa", 1_000_000)];

        let contract_accrued = 2_000_000;
        let (claimable, ids, drift) = compute_reconciliation(contract_accrued, &batches);

        // Claimable is capped at verified DB receipts ($1.00)
        assert_eq!(claimable, 1_000_000);
        assert_eq!(ids.len(), 1);
        assert_eq!(drift, 1_000_000); // Contract is 1M higher than DB
    }

    #[test]
    fn test_compute_reconciliation_empty_verified_batches() {
        let batches: Vec<UnclaimedBatchInfo> = vec![];

        let contract_accrued = 1_000_000;
        let (claimable, ids, drift) = compute_reconciliation(contract_accrued, &batches);

        assert_eq!(claimable, 0);
        assert!(ids.is_empty());
        assert_eq!(drift, 1_000_000);
    }
}
