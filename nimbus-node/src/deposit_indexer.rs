//! On-chain Deposit Event Indexer (DEC-018)
//!
//! Listens for `DepositFee` events emitted by the authoritative Nimbus Stylus contract.
//! Confirms deposits in SQLite database only after reaching confirmation threshold,
//! protecting against phantom deposit exploits and blockchain reorganizations.

use crate::state::AppState;
use anyhow::Result;
use std::time::Duration;
use tokio::time::interval;

/// Key used to persist the last indexed block in the database
pub const DEPOSIT_INDEXER_KEY: &str = "deposit_indexer_last_block";

/// Maximum blocks to query in a single get_logs request to avoid RPC timeouts
const MAX_BLOCKS_PER_QUERY: u64 = 500;

/// Polling interval for on-chain events
const INDEXER_POLL_INTERVAL_SECS: u64 = 5;

/// Background worker that monitors on-chain deposit events
pub async fn deposit_indexer_worker(state: AppState) {
    println!("DEPOSIT_INDEXER: Background worker started");

    let mut ticker = interval(Duration::from_secs(INDEXER_POLL_INTERVAL_SECS));

    loop {
        ticker.tick().await;

        let evm_client = match state.evm_client.as_ref() {
            Some(client) => client.clone(),
            None => {
                // EVM client not configured (e.g. passive/mock dev mode)
                continue;
            }
        };

        if let Err(e) = index_deposit_events_tick(&state, &evm_client).await {
            eprintln!("DEPOSIT_INDEXER ERROR: Failed indexing cycle: {}", e);
        }
    }
}

/// Single indexing execution cycle
async fn index_deposit_events_tick(
    state: &AppState,
    client: &crate::evm_client::EvmClient,
) -> Result<()> {
    let current_block = client.get_block_number().await?;
    let conf_threshold = client.finality_config().confirmation_threshold;

    // Only process up to safe block (reorg protection)
    let safe_block = current_block.saturating_sub(conf_threshold.saturating_sub(1));

    let last_block_opt = state.db.get_indexer_last_block(DEPOSIT_INDEXER_KEY).await?;

    let last_block = match last_block_opt {
        Some(b) => b,
        None => {
            // First run: bootstrap indexer from safe_block - 50 blocks
            let initial = safe_block.saturating_sub(50);
            state
                .db
                .set_indexer_last_block(DEPOSIT_INDEXER_KEY, initial)
                .await?;
            println!(
                "DEPOSIT_INDEXER: Initialized indexer checkpoint at block {}",
                initial
            );
            initial
        }
    };

    if safe_block <= last_block {
        return Ok(());
    }

    let from_block = last_block + 1;
    let to_block = safe_block.min(from_block + MAX_BLOCKS_PER_QUERY);

    let events = client.get_deposit_events(from_block, to_block).await?;

    for ev in events {
        if let Ok(client_addr) = std::str::FromStr::from_str(&ev.client) {
            if crate::validation::is_address_sanctioned(&client_addr) {
                eprintln!(
                    "DEPOSIT_INDEXER ALERT: Ingress deposit event for session {} skipped: depositor {} is sanctioned (DEC-027)",
                    ev.session_id, ev.client
                );
                continue;
            }
        }

        match state
            .db
            .confirm_deposit_on_chain(
                &ev.session_id,
                ev.net_amount,
                &ev.client,
                &ev.tx_hash,
                ev.block_number,
            )
            .await
        {
            Ok(true) => {
                println!(
                    "DEPOSIT_INDEXER: Confirmed deposit on-chain for session {} (net: {} units, tx: {})",
                    ev.session_id, ev.net_amount, ev.tx_hash
                );
            }
            Ok(false) => {
                println!(
                    "DEPOSIT_INDEXER: Unmatched or already-processed deposit event for session {}",
                    ev.session_id
                );
            }
            Err(e) => {
                eprintln!(
                    "DEPOSIT_INDEXER ERROR: Database confirmation failed for session {}: {}",
                    ev.session_id, e
                );
            }
        }
    }

    state
        .db
        .set_indexer_last_block(DEPOSIT_INDEXER_KEY, to_block)
        .await?;
    Ok(())
}

/// Verify a single session on-chain on demand (e.g. called from /api/deposit)
pub async fn verify_session_on_chain(
    state: &AppState,
    session_id: &str,
    tx_hash_opt: Option<&str>,
) -> Result<bool> {
    // 1. Check if already confirmed in database
    if state.db.is_deposit_confirmed(session_id).await? {
        return Ok(true);
    }

    // 2. If EVM client available and client provided a tx_hash, check on-chain directly
    if let (Some(client), Some(tx_hash)) = (state.evm_client.as_ref(), tx_hash_opt) {
        if let Some(ev) = client
            .check_deposit_tx_on_chain(tx_hash, session_id)
            .await?
        {
            if let Ok(client_addr) = std::str::FromStr::from_str(&ev.client) {
                if crate::validation::is_address_sanctioned(&client_addr) {
                    eprintln!(
                        "DEPOSIT_INDEXER ALERT: Direct verify rejected for session {}: depositor {} is sanctioned (DEC-027)",
                        ev.session_id, ev.client
                    );
                    return Ok(false);
                }
            }

            let confirmed = state
                .db
                .confirm_deposit_on_chain(
                    &ev.session_id,
                    ev.net_amount,
                    &ev.client,
                    &ev.tx_hash,
                    ev.block_number,
                )
                .await?;
            return Ok(confirmed);
        }
    }

    Ok(false)
}
