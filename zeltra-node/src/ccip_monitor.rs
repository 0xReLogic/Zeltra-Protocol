//! Unified CCIP Lifecycle Monitor (DEC-056)
//!
//! Tracks cross-chain message state for both:
//! 1. Inbound Deposits (Source Chain -> Arbitrum Stylus MMR)
//! 2. Outbound Spends (Arbitrum Stylus Spend -> Destination Chain)
//!
//! Integrates:
//! - Official Chainlink CCIP v2 REST API (https://api.ccip.chain.link/v2)
//! - Arbitrum Stylus Smart Contract on-chain state (`is_ccip_message_processed`)
//! - Destination chain OffRamp `ExecutionStateChanged` logs (optional RPC fallback)

use alloy::primitives::{keccak256, Address, B256};
use alloy::providers::{Provider, ProviderBuilder};
use alloy::rpc::types::Filter;
use alloy::transports::http::reqwest::Url;
use anyhow::Result;
use std::str::FromStr;
use std::time::Duration;

use crate::config::CcipDestinationConfig;
use crate::state::AppState;

/// Default Chainlink CCIP v2 API Base URL
pub const DEFAULT_CCIP_API_BASE: &str = "https://api.ccip.chain.link/v2";

/// ExecutionStateChanged event signature from CCIP OffRamp
const EXECUTION_STATE_CHANGED_SIGNATURE: &str =
    "ExecutionStateChanged(bytes32,uint64,uint64,bytes32,uint8,bytes,uint256)";

const STATE_SUCCESS: u8 = 2;
const STATE_FAILURE: u8 = 3;

/// Polling interval for destination chain and CCIP API checks (seconds)
const POLL_INTERVAL_SECS: u64 = 15;

/// Maximum block range to scan per RPC poll cycle
const MAX_BLOCK_RANGE: u64 = 1000;

/// Normalized cross-chain message status across CCIP 1.x and 2.0+ lanes
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub enum CcipLifecycleStatus {
    /// Message broadcast on source chain, waiting for source confirmation or DON pickup
    Sent,
    /// Message undergoing DON verification / consensus / blessing
    InFlight(String),
    /// Successfully executed on destination contract
    Success,
    /// Execution failed on destination
    Failed(String),
    /// Fresh message submitted but not yet indexed by Chainlink API (HTTP 404 backoff)
    Indexing,
}

impl CcipLifecycleStatus {
    #[allow(dead_code)]
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Success | Self::Failed(_))
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Sent => "SENT",
            Self::InFlight(_) => "IN_FLIGHT",
            Self::Success => "SUCCESS",
            Self::Failed(_) => "FAILED",
            Self::Indexing => "INDEXING",
        }
    }
}

/// Detailed CCIP message status report returned by the monitor and public API
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CcipMessageDetails {
    pub message_id: String,
    pub status: String,
    pub raw_status: Option<String>,
    pub delivery_time_ms: Option<u64>,
    pub source_tx_hash: Option<String>,
    pub receipt_tx_hash: Option<String>,
    pub receipt_block_number: Option<u64>,
    pub error: Option<String>,
}

/// Lightweight client for Chainlink CCIP v2 REST API (DEC-056 / ccip-api.md)
#[derive(Clone)]
pub struct CcipApiClient {
    client: reqwest::Client,
    base_url: String,
}

impl Default for CcipApiClient {
    fn default() -> Self {
        Self::new(DEFAULT_CCIP_API_BASE)
    }
}

impl CcipApiClient {
    pub fn new(base_url: &str) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap_or_default();
        Self {
            client,
            base_url: base_url.trim_end_matches('/').to_string(),
        }
    }

    /// Query message status from Chainlink CCIP v2 API
    /// Canonical endpoint: GET /v2/messages/{messageId}
    pub async fn query_message(&self, message_id: &str) -> Result<CcipLifecycleStatus> {
        let clean_id = if message_id.starts_with("0x") {
            message_id.to_string()
        } else {
            format!("0x{}", message_id)
        };

        let url = format!("{}/messages/{}", self.base_url, clean_id);
        let resp = self.client.get(&url).send().await;

        match resp {
            Ok(response) => {
                let status_code = response.status();
                if status_code == reqwest::StatusCode::OK {
                    let body: serde_json::Value = response.json().await?;
                    let raw_status = body
                        .get("status")
                        .and_then(|s| s.as_str())
                        .unwrap_or("UNKNOWN")
                        .to_uppercase();

                    match raw_status.as_str() {
                        "SUCCESS" => Ok(CcipLifecycleStatus::Success),
                        "FAILED" => {
                            let reason = body
                                .get("error")
                                .and_then(|e| e.as_str())
                                .unwrap_or("destination execution failed");
                            Ok(CcipLifecycleStatus::Failed(reason.to_string()))
                        }
                        "SENT" => Ok(CcipLifecycleStatus::Sent),
                        "SOURCE_FINALIZED" | "COMMITTED" | "BLESSED" | "VERIFYING" | "VERIFIED" => {
                            Ok(CcipLifecycleStatus::InFlight(raw_status))
                        }
                        other => Ok(CcipLifecycleStatus::InFlight(other.to_string())),
                    }
                } else if status_code == reqwest::StatusCode::NOT_FOUND {
                    // 404 indicates fresh unindexed message in Chainlink DON
                    Ok(CcipLifecycleStatus::Indexing)
                } else {
                    let err_text = response.text().await.unwrap_or_default();
                    anyhow::bail!("CCIP API error (HTTP {}): {}", status_code, err_text);
                }
            }
            Err(e) => {
                anyhow::bail!("Failed to connect to CCIP API: {}", e);
            }
        }
    }

    /// Query full message details from Chainlink CCIP v2 API
    pub async fn query_message_details(&self, message_id: &str) -> Result<Option<CcipMessageDetails>> {
        let clean_id = if message_id.starts_with("0x") {
            message_id.to_string()
        } else {
            format!("0x{}", message_id)
        };

        let url = format!("{}/messages/{}", self.base_url, clean_id);
        let resp = self.client.get(&url).send().await;

        match resp {
            Ok(response) => {
                if response.status() == reqwest::StatusCode::OK {
                    let body: serde_json::Value = response.json().await?;
                    let raw_status = body
                        .get("status")
                        .and_then(|s| s.as_str())
                        .unwrap_or("UNKNOWN")
                        .to_string();

                    let status_enum = match raw_status.to_uppercase().as_str() {
                        "SUCCESS" => CcipLifecycleStatus::Success,
                        "FAILED" => CcipLifecycleStatus::Failed("execution failed".to_string()),
                        "SENT" => CcipLifecycleStatus::Sent,
                        other => CcipLifecycleStatus::InFlight(other.to_string()),
                    };

                    let delivery_time_ms = body.get("deliveryTime").and_then(|v| v.as_u64());
                    let source_tx_hash = body
                        .get("sendTransactionHash")
                        .and_then(|s| s.as_str())
                        .map(str::to_string);
                    let receipt_tx_hash = body
                        .get("receiptTransactionHash")
                        .and_then(|s| s.as_str())
                        .map(str::to_string);
                    let receipt_block_number =
                        body.get("receiptBlockNumber").and_then(|b| b.as_u64());

                    Ok(Some(CcipMessageDetails {
                        message_id: clean_id,
                        status: status_enum.as_str().to_string(),
                        raw_status: Some(raw_status),
                        delivery_time_ms,
                        source_tx_hash,
                        receipt_tx_hash,
                        receipt_block_number,
                        error: None,
                    }))
                } else if response.status() == reqwest::StatusCode::NOT_FOUND {
                    Ok(Some(CcipMessageDetails {
                        message_id: clean_id,
                        status: "INDEXING".to_string(),
                        raw_status: Some("NOT_INDEXED_YET".to_string()),
                        delivery_time_ms: None,
                        source_tx_hash: None,
                        receipt_tx_hash: None,
                        receipt_block_number: None,
                        error: None,
                    }))
                } else {
                    Ok(None)
                }
            }
            Err(_) => Ok(None),
        }
    }
}

/// Background worker loop for unified CCIP monitoring
pub async fn ccip_monitor_loop(state: AppState, config: CcipDestinationConfig) {
    let api_client = CcipApiClient::default();

    println!("CCIP_MONITOR: Unified Lifecycle Monitor daemon started");
    println!("  Poll Interval : {}s", POLL_INTERVAL_SECS);
    if config.is_enabled() {
        println!(
            "  RPC Fallback  : Enabled ({})",
            config.destination_rpc_url.as_deref().unwrap_or("")
        );
    } else {
        println!("  RPC Fallback  : Disabled (using CCIP v2 API & Arbitrum on-chain state)");
    }

    let mut last_rpc_block: Option<u64> = None;

    loop {
        tokio::time::sleep(Duration::from_secs(POLL_INTERVAL_SECS)).await;

        // 1. Monitor Inbound Deposits (Source Chain -> Arbitrum Stylus MMR)
        if let Err(e) = monitor_inbound_deposits_tick(&state, &api_client).await {
            eprintln!("CCIP_MONITOR_ERROR (Inbound): {:?}", e);
        }

        // 2. Monitor Outbound Spends (Arbitrum Stylus -> Destination Chain)
        if let Err(e) = monitor_outbound_spends_tick(&state, &api_client, &config, &mut last_rpc_block).await {
            eprintln!("CCIP_MONITOR_ERROR (Outbound): {:?}", e);
        }
    }
}

/// Tick cycle for inbound programmable deposits (DEC-055)
pub async fn monitor_inbound_deposits_tick(
    state: &AppState,
    api_client: &CcipApiClient,
) -> Result<()> {
    let pending_deposits = state.db.get_pending_ccip_deposits().await?;
    if pending_deposits.is_empty() {
        return Ok(());
    }

    println!(
        "CCIP_MONITOR: Tracking {} pending inbound deposit(s)",
        pending_deposits.len()
    );

    for deposit in pending_deposits {
        // Priority 1: Check if already processed on-chain on Arbitrum Stylus contract
        if let Some(ref client) = state.evm_client {
            match client.is_ccip_message_processed(&deposit.message_id).await {
                Ok(true) => {
                    // Message processed on-chain! Look up leaf in MMR table by note commitment
                    let clean_cm = deposit.note_commitment.trim_start_matches("0x");
                    if let Ok(leaves) = state.db.get_all_mmr_leaves().await {
                        if let Some(matching_leaf) = leaves.iter().find(|l| {
                            l.commitment.trim_start_matches("0x").eq_ignore_ascii_case(clean_cm)
                        }) {
                            println!(
                                "CCIP_MONITOR: Deposit {} CONFIRMED IN POOL (leaf_index={})",
                                deposit.message_id, matching_leaf.leaf_index
                            );
                            let _ = state
                                .db
                                .confirm_ccip_inbound_deposit(
                                    &deposit.message_id,
                                    matching_leaf.leaf_index,
                                    &matching_leaf.tx_hash,
                                    matching_leaf.block_number,
                                )
                                .await;
                            continue;
                        }
                    }
                    // If leaf indexer hasn't reached block yet, mark confirmed with provisional index 0
                    let _ = state
                        .db
                        .confirm_ccip_inbound_deposit(&deposit.message_id, 0, "", 0)
                        .await;
                    continue;
                }
                Ok(false) => {
                    // Not yet processed on Arbitrum
                }
                Err(e) => {
                    eprintln!(
                        "CCIP_MONITOR: Failed to check on-chain processed status for {}: {}",
                        deposit.message_id, e
                    );
                }
            }
        }

        // Priority 2: Query Chainlink CCIP v2 API
        match api_client.query_message(&deposit.message_id).await {
            Ok(CcipLifecycleStatus::Success) => {
                println!(
                    "CCIP_MONITOR: Deposit {} reached destination according to CCIP API",
                    deposit.message_id
                );
            }
            Ok(CcipLifecycleStatus::Failed(err)) => {
                eprintln!(
                    "CCIP_MONITOR: Deposit {} FAILED: {}",
                    deposit.message_id, err
                );
                // Mark failed
                let mut failed_rec = deposit.clone();
                failed_rec.status = "failed".to_string();
                let _ = state.db.record_ccip_inbound_deposit(&failed_rec).await;
            }
            Ok(CcipLifecycleStatus::InFlight(phase)) => {
                println!(
                    "CCIP_MONITOR: Deposit {} in flight (phase: {})",
                    deposit.message_id, phase
                );
            }
            Ok(CcipLifecycleStatus::Sent | CcipLifecycleStatus::Indexing) => {
                // Normal pending state
            }
            Err(e) => {
                eprintln!(
                    "CCIP_MONITOR: API query failed for {}: {}",
                    deposit.message_id, e
                );
            }
        }
    }

    Ok(())
}

/// Tick cycle for outbound cross-chain spends (DEC-056)
pub async fn monitor_outbound_spends_tick(
    state: &AppState,
    api_client: &CcipApiClient,
    config: &CcipDestinationConfig,
    last_rpc_block: &mut Option<u64>,
) -> Result<()> {
    let pending_spends = state.db.get_pending_ccip_spends().await?;
    if pending_spends.is_empty() {
        return Ok(());
    }

    println!(
        "CCIP_MONITOR: Tracking {} pending outbound spend(s)",
        pending_spends.len()
    );

    // Primary path: Chainlink CCIP v2 REST API (multi-chain, zero RPC cost)
    for (id, message_id, chain_selector) in &pending_spends {
        match api_client.query_message(message_id).await {
            Ok(CcipLifecycleStatus::Success) => {
                println!(
                    "CCIP_MONITOR: Outbound spend {} (msg_id={}) DELIVERED to chain {}",
                    id, message_id, chain_selector
                );
                let _ = state
                    .db
                    .update_destination_status(*id, "delivered", None)
                    .await;
                continue;
            }
            Ok(CcipLifecycleStatus::Failed(err)) => {
                println!(
                    "CCIP_MONITOR: Outbound spend {} (msg_id={}) FAILED on destination: {}",
                    id, message_id, err
                );
                let _ = state
                    .db
                    .update_destination_status(*id, "failed", Some(&err))
                    .await;
                continue;
            }
            Ok(CcipLifecycleStatus::InFlight(phase)) => {
                println!(
                    "CCIP_MONITOR: Outbound spend {} in flight (phase: {})",
                    id, phase
                );
            }
            Ok(CcipLifecycleStatus::Sent | CcipLifecycleStatus::Indexing) => {
                // Pending normal
            }
            Err(e) => {
                eprintln!(
                    "CCIP_MONITOR: API check error for spend {}: {}",
                    id, e
                );
            }
        }
    }

    // Secondary fallback path: Direct RPC log scanning (if destination RPC is configured)
    if let (Some(rpc_url), Some(offramp_str)) = (
        config.destination_rpc_url.as_ref(),
        config.ccip_offramp_address.as_ref(),
    ) {
        if let (Ok(offramp), Ok(url)) = (Address::from_str(offramp_str), rpc_url.parse::<Url>()) {
            let provider = ProviderBuilder::new().connect_http(url);
            let event_topic = keccak256(EXECUTION_STATE_CHANGED_SIGNATURE);

            if let Ok(current_block) = provider.get_block_number().await {
                let from_block = last_rpc_block.unwrap_or(current_block.saturating_sub(100));
                let to_block = current_block.min(from_block + MAX_BLOCK_RANGE);

                if from_block < to_block {
                    let filter = Filter::new()
                        .address(offramp)
                        .event_signature(event_topic)
                        .from_block(from_block)
                        .to_block(to_block);

                    if let Ok(logs) = provider.get_logs(&filter).await {
                        *last_rpc_block = Some(to_block + 1);

                        for log in &logs {
                            if log.topics().len() >= 2 {
                                let log_msg_id: B256 = log.topics()[1];
                                let msg_hex = format!("0x{:x}", log_msg_id);

                                if let Some((id, _, _)) = pending_spends
                                    .iter()
                                    .find(|(_, m, _)| m.eq_ignore_ascii_case(&msg_hex))
                                {
                                    if log.data().data.len() >= 33 {
                                        let state_byte = log.data().data[32];
                                        match state_byte {
                                            STATE_SUCCESS => {
                                                let _ = state
                                                    .db
                                                    .update_destination_status(*id, "delivered", None)
                                                    .await;
                                            }
                                            STATE_FAILURE => {
                                                let _ = state
                                                    .db
                                                    .update_destination_status(
                                                        *id,
                                                        "failed",
                                                        Some("offramp execution failure"),
                                                    )
                                                    .await;
                                            }
                                            _ => {}
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(())
}
