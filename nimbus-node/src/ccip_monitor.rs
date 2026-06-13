//! CCIP Destination Monitor
//!
//! Background worker that polls the destination chain for CCIP message execution status.
//! Queries `ExecutionStateChanged` events from the CCIP OffRamp contract.
//!
//! Execution states (from CCIP v1.6.1):
//! - 0: UNTOUCHED (never executed)
//! - 1: IN_PROGRESS (currently executing)
//! - 2: SUCCESS (end state)
//! - 3: FAILURE (end state, manual execution possible)

use alloy::primitives::{keccak256, Address, B256};
use alloy::providers::{Provider, ProviderBuilder};
use alloy::rpc::types::Filter;
use alloy::transports::http::reqwest::Url;
use std::str::FromStr;
use std::time::Duration;

use crate::config::CcipDestinationConfig;
use crate::database::Database;

/// ExecutionStateChanged event signature:
/// ExecutionStateChanged(bytes32 indexed messageId, uint64 sourceChainSelector, uint64 nonce,
///                       bytes32 messageHash, uint8 state, bytes returnData, uint256 gasUsed)
const EXECUTION_STATE_CHANGED_SIGNATURE: &str =
    "ExecutionStateChanged(bytes32,uint64,uint64,bytes32,uint8,bytes,uint256)";

/// CCIP execution states
const STATE_SUCCESS: u8 = 2;
const STATE_FAILURE: u8 = 3;

/// Polling interval for destination chain checks
const POLL_INTERVAL_SECS: u64 = 30;

/// Maximum block range to scan per poll cycle (avoid RPC timeouts)
const MAX_BLOCK_RANGE: u64 = 1000;

/// Run the CCIP destination monitor loop.
/// This is spawned as a background task in main.rs.
///
/// If config is disabled (no RPC URL or off-ramp address), this function returns immediately.
pub async fn ccip_monitor_loop(db: Database, config: CcipDestinationConfig) {
    if !config.is_enabled() {
        println!(
            "CCIP_MONITOR: Disabled (missing NIMBUS_DESTINATION_RPC_URL or NIMBUS_CCIP_OFFRAMP)"
        );
        return;
    }

    let rpc_url = config.destination_rpc_url.as_ref().unwrap();
    let offramp_addr_str = config.ccip_offramp_address.as_ref().unwrap();

    println!("CCIP_MONITOR: Starting destination tracking");
    println!("  Destination RPC : {}", rpc_url);
    println!("  OffRamp Address : {}", offramp_addr_str);
    println!("  Poll Interval   : {}s", POLL_INTERVAL_SECS);
    println!("  Refund Timeout  : {}s", config.refund_timeout_secs);

    let offramp = match Address::from_str(offramp_addr_str) {
        Ok(addr) => addr,
        Err(e) => {
            eprintln!("CCIP_MONITOR: Invalid off-ramp address: {}", e);
            return;
        }
    };

    let url = match rpc_url.parse::<Url>() {
        Ok(u) => u,
        Err(e) => {
            eprintln!("CCIP_MONITOR: Invalid destination RPC URL: {}", e);
            return;
        }
    };

    let provider = ProviderBuilder::new().connect_http(url);

    let event_topic = keccak256(EXECUTION_STATE_CHANGED_SIGNATURE);

    // Track the last scanned block to avoid re-scanning
    let mut last_block: Option<u64> = None;

    loop {
        tokio::time::sleep(Duration::from_secs(POLL_INTERVAL_SECS)).await;

        // Get pending CCIP spends from DB
        let pending = match db.get_pending_ccip_spends().await {
            Ok(items) => items,
            Err(e) => {
                eprintln!("CCIP_MONITOR: Failed to query pending spends: {}", e);
                continue;
            }
        };

        if pending.is_empty() {
            continue;
        }

        println!(
            "CCIP_MONITOR: Checking {} pending CCIP spends",
            pending.len()
        );

        // Determine block range to scan
        let current_block = match provider.get_block_number().await {
            Ok(b) => b,
            Err(e) => {
                eprintln!("CCIP_MONITOR: Failed to get block number: {}", e);
                continue;
            }
        };

        let from_block = last_block.unwrap_or(current_block.saturating_sub(100));
        let to_block = current_block.min(from_block + MAX_BLOCK_RANGE);

        if from_block >= to_block {
            continue;
        }

        // Build event filter for ExecutionStateChanged
        let filter = Filter::new()
            .address(offramp)
            .event_signature(event_topic)
            .from_block(from_block)
            .to_block(to_block);

        let logs = match provider.get_logs(&filter).await {
            Ok(logs) => logs,
            Err(e) => {
                eprintln!("CCIP_MONITOR: Failed to get logs: {}", e);
                continue;
            }
        };

        last_block = Some(to_block + 1);

        // Match logs against pending message IDs
        for log in &logs {
            if log.topics().len() < 3 {
                continue;
            }

            // topic[0] = event signature
            // topic[1] = messageId (indexed)
            let log_message_id: B256 = log.topics()[1];
            let message_id_hex = format!("0x{:x}", log_message_id);

            // Find matching pending spend
            let matching = pending
                .iter()
                .find(|(_, msg_id, _)| msg_id.eq_ignore_ascii_case(&message_id_hex));

            if let Some((id, _, _)) = matching {
                // topic[2] contains nonce, state is in the first data byte after topics
                // ExecutionStateChanged has non-indexed fields: messageHash, state, returnData, gasUsed
                // state is at offset 32 in the data (after messageHash)
                let state = if log.data().data.len() >= 33 {
                    log.data().data[32]
                } else {
                    continue;
                };

                match state {
                    STATE_SUCCESS => {
                        println!(
                            "CCIP_MONITOR: Message {} DELIVERED (queue id={})",
                            message_id_hex, id
                        );
                        if let Err(e) = db.update_destination_status(*id, "delivered", None).await {
                            eprintln!("CCIP_MONITOR: Failed to update status: {}", e);
                        }
                    }
                    STATE_FAILURE => {
                        let error_msg = "destination execution failed";
                        println!(
                            "CCIP_MONITOR: Message {} FAILED (queue id={}) — {}",
                            message_id_hex, id, error_msg
                        );
                        if let Err(e) = db
                            .update_destination_status(*id, "failed", Some(error_msg))
                            .await
                        {
                            eprintln!("CCIP_MONITOR: Failed to update status: {}", e);
                        }
                    }
                    _ => {
                        // IN_PROGRESS or UNTOUCHED — skip, will check again next cycle
                    }
                }
            }
        }
    }
}
