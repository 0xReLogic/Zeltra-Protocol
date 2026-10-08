//! Mempool Lease Watchdog (DEC-027)
//!
//! Monitors in-flight/broadcasting spends whose lease duration has expired.
//! Enforces ERC-7562 strict simulation rules:
//! 1. Checks on-chain `is_nullifier_spent(nullifier)`.
//! 2. If spent on-chain (late inclusion) -> reconciles as `confirmed` and retains nullifier lock.
//! 3. If unspent on-chain -> marks as `failed_expired` and releases the nullifier lease,
//!    restoring self-custody for the user / SDK wallet to re-spend or bump gas.

use crate::state::AppState;
use std::time::Duration;
use tokio::time::interval;

/// Default interval between watchdog polling cycles (seconds)
pub const DEFAULT_WATCHDOG_INTERVAL_SECS: u64 = 15;

/// Background worker that executes the mempool lease watchdog loop (DEC-027)
pub async fn mempool_lease_watchdog_worker(state: AppState) {
    let interval_secs = std::env::var("NIMBUS_WATCHDOG_INTERVAL_SECS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(DEFAULT_WATCHDOG_INTERVAL_SECS);

    println!(
        "MEMPOOL WATCHDOG: Background worker initialized (polling interval: {}s)",
        interval_secs
    );

    let mut ticker = interval(Duration::from_secs(interval_secs));

    loop {
        ticker.tick().await;

        if let Err(e) = run_watchdog_tick(&state).await {
            eprintln!("MEMPOOL WATCHDOG ERROR: Watchdog cycle failed: {}", e);
        }
    }
}

/// Executes a single watchdog tick over expired leases
pub async fn run_watchdog_tick(state: &AppState) -> anyhow::Result<usize> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    let expired_spends = state.db.get_expired_leased_spends(now).await?;
    if expired_spends.is_empty() {
        return Ok(0);
    }

    let mut processed_count = 0;
    for spend in expired_spends {
        if let Some(ref client) = state.evm_client {
            match client.is_nullifier_spent(&spend.nullifier).await {
                Ok(true) => {
                    // Late inclusion! Transaction was mined on-chain before lease cleanup.
                    println!(
                        "MEMPOOL WATCHDOG: Late inclusion detected for spend {} (nullifier {}...). Reconciling as confirmed.",
                        spend.id,
                        &spend.nullifier[..8.min(spend.nullifier.len())]
                    );
                    if let Err(e) = state
                        .db
                        .reconcile_spend_as_confirmed(
                            spend.id,
                            &spend.nullifier,
                            spend.tx_hash.as_deref(),
                        )
                        .await
                    {
                        eprintln!(
                            "MEMPOOL WATCHDOG ERROR: Failed to reconcile spend {}: {}",
                            spend.id, e
                        );
                    } else {
                        processed_count += 1;
                    }
                }
                Ok(false) => {
                    // Confirmed unspent on-chain! Safely expire lease and restore user self-custody.
                    println!(
                        "MEMPOOL WATCHDOG: Spend {} lease expired and nullifier is unspent on-chain. Setting failed_expired to release lease.",
                        spend.id
                    );
                    if let Err(e) = state.db.expire_unspent_lease(spend.id).await {
                        eprintln!(
                            "MEMPOOL WATCHDOG ERROR: Failed to expire lease for spend {}: {}",
                            spend.id, e
                        );
                    } else {
                        processed_count += 1;
                    }
                }
                Err(e) => {
                    // Fail-closed against transient RPC error: retain lease until next cycle
                    eprintln!(
                        "MEMPOOL WATCHDOG WARNING: Failed to query on-chain nullifier for spend {}: {}. Retaining lease.",
                        spend.id, e
                    );
                }
            }
        } else {
            // Offline/dev mode without EVM client
            let _ = state.db.expire_unspent_lease(spend.id).await;
            processed_count += 1;
        }
    }

    Ok(processed_count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::Database;
    use crate::dto::SpendRequest;
    use std::collections::HashMap;
    use tempfile::TempDir;

    async fn test_state() -> (AppState, TempDir) {
        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("watchdog_test.db");
        let db = Database::new(&db_path).await.unwrap();
        let state = AppState::new(
            db,
            nimbus_core::Fr::from(1u64),
            1,
            nimbus_core::IssuerPublicKey(nimbus_core::G2Projective::default()),
            HashMap::new(),
            None,
        )
        .await;
        (state, tmp)
    }

    #[tokio::test]
    async fn test_watchdog_expires_unspent_lease() {
        let (state, _tmp) = test_state().await;

        let req = SpendRequest {
            nullifier: "0xwatchdog_nullifier_1".to_string(),
            sig_hex: "0xsig".to_string(),
            recipient: "0x0000000000000000000000000000000000000001".to_string(),
            amount: 10_000_000,
            ..Default::default()
        };

        let spend_id = state.db.enqueue_spend(&req).await.unwrap().unwrap();

        // Claim with a 1-second lease
        let claimed = state.db.claim_spends(1, 1).await.unwrap();
        assert_eq!(claimed.len(), 1);
        assert_eq!(claimed[0].id, spend_id);

        // Mark as submitted
        state
            .db
            .mark_spend_submitted(spend_id, "0xtx_stuck")
            .await
            .unwrap();

        // Wait 2 seconds for lease to expire
        tokio::time::sleep(Duration::from_secs(2)).await;

        // Run watchdog tick
        let processed = run_watchdog_tick(&state).await.unwrap();
        assert_eq!(processed, 1);

        // Verify that spend_queue has status 'failed_expired'
        let unreconciled = state.db.get_unreconciled_spends().await.unwrap();
        assert!(!unreconciled.iter().any(|s| s.id == spend_id));

        // Verify nullifier is not spent in local database
        assert!(!state.db.is_nullifier_spent(&req.nullifier).await.unwrap());

        // Verify user can now re-enqueue with the same nullifier (self-custody restored!)
        let re_enqueue_id = state.db.enqueue_spend(&req).await.unwrap();
        assert!(re_enqueue_id.is_some());
    }

    #[tokio::test]
    async fn test_watchdog_ignores_active_lease() {
        let (state, _tmp) = test_state().await;

        let req = SpendRequest {
            nullifier: "0xwatchdog_active_nullifier".to_string(),
            sig_hex: "0xsig".to_string(),
            recipient: "0x0000000000000000000000000000000000000001".to_string(),
            amount: 5_000_000,
            ..Default::default()
        };

        let spend_id = state.db.enqueue_spend(&req).await.unwrap().unwrap();

        // Claim with 300 seconds lease (5 minutes)
        let claimed = state.db.claim_spends(1, 300).await.unwrap();
        assert_eq!(claimed.len(), 1);
        assert_eq!(claimed[0].id, spend_id);

        state
            .db
            .mark_spend_submitted(spend_id, "0xtx_active")
            .await
            .unwrap();

        // Run watchdog tick immediately: should process 0 items because lease is active
        let processed = run_watchdog_tick(&state).await.unwrap();
        assert_eq!(processed, 0);

        // Verify spend is still in unreconciled active list
        let unreconciled = state.db.get_unreconciled_spends().await.unwrap();
        assert!(unreconciled.iter().any(|s| s.id == spend_id));
    }
}
