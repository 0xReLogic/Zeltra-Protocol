//! CCIP Lifecycle Monitoring Handlers (DEC-056)
//!
//! Exposes public query endpoints for:
//! - Inbound programmable deposits: GET /api/v1/ccip/deposit/{message_id}
//! - Outbound cross-chain spends: GET /api/v1/ccip/spend/{message_id}
//! - Unified status query: GET /api/v1/ccip/status/{message_id}

use axum::extract::{Path, State};
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::ccip_monitor::CcipApiClient;
use crate::state::AppState;

/// Response payload for inbound CCIP deposit status query
#[derive(Debug, Serialize, Deserialize)]
pub struct CcipDepositStatusResponse {
    pub message_id: String,
    pub status: String, // "CONFIRMED_IN_POOL", "IN_FLIGHT", "INDEXING", "FAILED"
    pub source_chain_selector: Option<u64>,
    pub asset_id: Option<u8>,
    pub amount: Option<u64>,
    pub leaf_index: Option<u64>,
    pub note_commitment: Option<String>,
    pub tx_hash: Option<String>,
    pub block_number: Option<u64>,
    pub delivery_time_ms: Option<u64>,
    pub error: Option<String>,
}

/// Response payload for outbound CCIP spend status query
#[derive(Debug, Serialize, Deserialize)]
pub struct CcipSpendStatusResponse {
    pub message_id: String,
    pub status: String, // "DELIVERED", "PENDING", "FAILED", "INDEXING"
    pub destination_chain_selector: Option<u64>,
    pub amount: Option<u64>,
    pub recipient: Option<String>,
    pub source_tx_hash: Option<String>,
    pub receipt_tx_hash: Option<String>,
    pub error: Option<String>,
}

/// Unified message status response
#[derive(Debug, Serialize, Deserialize)]
pub struct CcipUniversalStatusResponse {
    pub message_id: String,
    pub message_type: String, // "INBOUND_DEPOSIT", "OUTBOUND_SPEND", "UNKNOWN"
    pub status: String,
    pub details: serde_json::Value,
}

/// GET /api/v1/ccip/deposit/{message_id}
pub async fn handle_ccip_deposit_status(
    State(state): State<AppState>,
    Path(message_id): Path<String>,
) -> Json<CcipDepositStatusResponse> {
    let clean_id = if message_id.starts_with("0x") {
        message_id.to_string()
    } else {
        format!("0x{}", message_id)
    };

    // 1. Check local database record first
    if let Ok(Some(deposit)) = state.db.get_ccip_inbound_deposit(&clean_id).await {
        return Json(CcipDepositStatusResponse {
            message_id: clean_id,
            status: deposit.status.to_uppercase(),
            source_chain_selector: Some(deposit.source_chain_selector),
            asset_id: Some(deposit.asset_id),
            amount: Some(deposit.amount),
            leaf_index: deposit.leaf_index,
            note_commitment: Some(deposit.note_commitment),
            tx_hash: deposit.tx_hash,
            block_number: deposit.block_number,
            delivery_time_ms: None,
            error: None,
        });
    }

    // 2. Check on-chain contract processed status
    let mut is_processed_onchain = false;
    if let Some(ref client) = state.evm_client {
        if let Ok(processed) = client.is_ccip_message_processed(&clean_id).await {
            is_processed_onchain = processed;
        }
    }

    if is_processed_onchain {
        return Json(CcipDepositStatusResponse {
            message_id: clean_id,
            status: "CONFIRMED_IN_POOL".to_string(),
            source_chain_selector: None,
            asset_id: None,
            amount: None,
            leaf_index: None,
            note_commitment: None,
            tx_hash: None,
            block_number: None,
            delivery_time_ms: None,
            error: None,
        });
    }

    // 3. Fallback to querying Chainlink CCIP v2 API
    let api_client = CcipApiClient::default();
    match api_client.query_message_details(&clean_id).await {
        Ok(Some(details)) => Json(CcipDepositStatusResponse {
            message_id: clean_id,
            status: details.status,
            source_chain_selector: None,
            asset_id: None,
            amount: None,
            leaf_index: None,
            note_commitment: None,
            tx_hash: details.source_tx_hash,
            block_number: details.receipt_block_number,
            delivery_time_ms: details.delivery_time_ms,
            error: details.error,
        }),
        _ => Json(CcipDepositStatusResponse {
            message_id: clean_id,
            status: "NOT_FOUND".to_string(),
            source_chain_selector: None,
            asset_id: None,
            amount: None,
            leaf_index: None,
            note_commitment: None,
            tx_hash: None,
            block_number: None,
            delivery_time_ms: None,
            error: Some("Message ID not found or not yet indexed by CCIP".to_string()),
        }),
    }
}

/// GET /api/v1/ccip/spend/{message_id}
pub async fn handle_ccip_spend_status(
    State(state): State<AppState>,
    Path(message_id): Path<String>,
) -> Json<CcipSpendStatusResponse> {
    let clean_id = if message_id.starts_with("0x") {
        message_id.to_string()
    } else {
        format!("0x{}", message_id)
    };

    // 1. Check local spend queue
    if let Ok(Some(spend)) = state.db.get_ccip_spend_by_message_id(&clean_id).await {
        let norm_status = match spend.destination_status.as_str() {
            "delivered" => "DELIVERED",
            "failed" => "FAILED",
            _ => "PENDING",
        };

        return Json(CcipSpendStatusResponse {
            message_id: clean_id,
            status: norm_status.to_string(),
            destination_chain_selector: Some(spend.destination_chain_selector),
            amount: Some(spend.amount),
            recipient: Some(spend.recipient),
            source_tx_hash: spend.tx_hash,
            receipt_tx_hash: None,
            error: spend.destination_error,
        });
    }

    // 2. Query Chainlink CCIP v2 API
    let api_client = CcipApiClient::default();
    match api_client.query_message_details(&clean_id).await {
        Ok(Some(details)) => Json(CcipSpendStatusResponse {
            message_id: clean_id,
            status: details.status,
            destination_chain_selector: None,
            amount: None,
            recipient: None,
            source_tx_hash: details.source_tx_hash,
            receipt_tx_hash: details.receipt_tx_hash,
            error: details.error,
        }),
        _ => Json(CcipSpendStatusResponse {
            message_id: clean_id,
            status: "NOT_FOUND".to_string(),
            destination_chain_selector: None,
            amount: None,
            recipient: None,
            source_tx_hash: None,
            receipt_tx_hash: None,
            error: Some("Outbound spend message ID not found".to_string()),
        }),
    }
}

/// GET /api/v1/ccip/status/{message_id}
pub async fn handle_ccip_universal_status(
    State(state): State<AppState>,
    Path(message_id): Path<String>,
) -> Json<CcipUniversalStatusResponse> {
    let clean_id = if message_id.starts_with("0x") {
        message_id.to_string()
    } else {
        format!("0x{}", message_id)
    };

    // Check deposit first
    if let Ok(Some(deposit)) = state.db.get_ccip_inbound_deposit(&clean_id).await {
        return Json(CcipUniversalStatusResponse {
            message_id: clean_id,
            message_type: "INBOUND_DEPOSIT".to_string(),
            status: deposit.status.to_uppercase(),
            details: serde_json::to_value(&deposit).unwrap_or_default(),
        });
    }

    // Check spend queue next
    if let Ok(Some(spend)) = state.db.get_ccip_spend_by_message_id(&clean_id).await {
        return Json(CcipUniversalStatusResponse {
            message_id: clean_id,
            message_type: "OUTBOUND_SPEND".to_string(),
            status: spend.destination_status.to_uppercase(),
            details: serde_json::to_value(&spend).unwrap_or_default(),
        });
    }

    // Query Chainlink CCIP v2 API
    let api_client = CcipApiClient::default();
    if let Ok(Some(details)) = api_client.query_message_details(&clean_id).await {
        return Json(CcipUniversalStatusResponse {
            message_id: clean_id,
            message_type: "CROSS_CHAIN_MESSAGE".to_string(),
            status: details.status.clone(),
            details: serde_json::to_value(&details).unwrap_or_default(),
        });
    }

    Json(CcipUniversalStatusResponse {
        message_id: clean_id,
        message_type: "UNKNOWN".to_string(),
        status: "NOT_FOUND".to_string(),
        details: serde_json::json!({ "error": "Message not found in local index or CCIP network" }),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::Database;
    use crate::state::AppState;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_ccip_deposit_and_universal_status_handlers() {
        std::env::set_var("ZELTRA_DB_KEY", "test-encryption-key");
        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("handler_test.db");
        let db = Database::new(&db_path).await.unwrap();
        let state = AppState::new(db.clone(), None).await;

        let msg_id = "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

        // 1. Initially message does not exist -> NOT_FOUND or INDEXING
        let resp1 = handle_ccip_deposit_status(State(state.clone()), Path(msg_id.to_string())).await;
        assert!(resp1.status == "NOT_FOUND" || resp1.status == "INDEXING");

        // 2. Insert inbound deposit
        let deposit_rec = crate::database::ccip::CcipInboundDepositRecord {
            message_id: msg_id.to_string(),
            source_chain_selector: 16015286601757825753,
            asset_id: 1,
            amount: 50_000_000,
            leaf_index: Some(15),
            note_commitment: "0xcomm123".to_string(),
            status: "confirmed_in_pool".to_string(),
            tx_hash: Some("0xtx789".to_string()),
            block_number: Some(12345),
            updated_at: 1000,
        };
        db.record_ccip_inbound_deposit(&deposit_rec).await.unwrap();

        // 3. Query deposit status endpoint -> returns CONFIRMED_IN_POOL
        let resp2 = handle_ccip_deposit_status(State(state.clone()), Path(msg_id.to_string())).await;
        assert_eq!(resp2.status, "CONFIRMED_IN_POOL");
        assert_eq!(resp2.leaf_index, Some(15));
        assert_eq!(resp2.amount, Some(50_000_000));

        // 4. Query universal status endpoint -> returns INBOUND_DEPOSIT
        let resp3 = handle_ccip_universal_status(State(state.clone()), Path(msg_id.to_string())).await;
        assert_eq!(resp3.message_type, "INBOUND_DEPOSIT");
        assert_eq!(resp3.status, "CONFIRMED_IN_POOL");
    }

    #[tokio::test]
    async fn test_ccip_spend_and_universal_status_handlers() {
        std::env::set_var("ZELTRA_DB_KEY", "test-encryption-key");
        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("spend_handler_test.db");
        let db = Database::new(&db_path).await.unwrap();
        let state = AppState::new(db.clone(), None).await;

        let msg_id = "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
        let nullifier = "0xnf123456789";

        let req = crate::dto::SpendRequest {
            nullifier: nullifier.to_string(),
            sig_hex: "0xsig".to_string(),
            recipient: "0xmerchant".to_string(),
            amount: 10_000_000,
            cross_chain: Some(crate::dto::CrossChainParams {
                destination_chain_selector: 10344971235874465080,
                destination_contract: "0xdest".to_string(),
            }),
            ..Default::default()
        };

        let queue_id = db.enqueue_spend(&req).await.unwrap().unwrap();
        db.set_ccip_tracking(queue_id, msg_id, 10344971235874465080).await.unwrap();

        // 1. Initial status: PENDING
        let resp1 = handle_ccip_spend_status(State(state.clone()), Path(msg_id.to_string())).await;
        assert_eq!(resp1.status, "PENDING");
        assert_eq!(resp1.destination_chain_selector, Some(10344971235874465080));

        // 2. Universal status returns OUTBOUND_SPEND
        let resp2 = handle_ccip_universal_status(State(state.clone()), Path(msg_id.to_string())).await;
        assert_eq!(resp2.message_type, "OUTBOUND_SPEND");
        assert_eq!(resp2.status, "PENDING");

        // 3. Mark delivered
        db.update_destination_status(queue_id, "delivered", None).await.unwrap();
        let resp3 = handle_ccip_spend_status(State(state.clone()), Path(msg_id.to_string())).await;
        assert_eq!(resp3.status, "DELIVERED");
    }

    #[tokio::test]
    async fn test_ccip_status_negative_unknown_message() {
        std::env::set_var("ZELTRA_DB_KEY", "test-encryption-key");
        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("unknown_msg.db");
        let db = Database::new(&db_path).await.unwrap();
        let state = AppState::new(db, None).await;

        let unknown_id = "0x9999999999999999999999999999999999999999999999999999999999999999";
        let resp = handle_ccip_universal_status(State(state), Path(unknown_id.to_string())).await;
        // In isolated local test without live internet to CCIP API, returns UNKNOWN / NOT_FOUND
        assert!(resp.status == "NOT_FOUND" || resp.status == "INDEXING");
    }
}
