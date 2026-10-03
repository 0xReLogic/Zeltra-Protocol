//! Handler for GET /api/tx-status
//!
//! Exposes transaction settlement status, confirmations, block height, and gas details (DEC-017).

use crate::dto::{TxStatusQuery, TxStatusResponse};
use crate::state::AppState;
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};

pub async fn handle_tx_status(
    State(state): State<AppState>,
    Query(query): Query<TxStatusQuery>,
) -> Response {
    let tx_hash = query.tx_hash.trim();

    // Basic format validation for EVM tx hash (0x + 64 hex chars)
    if !tx_hash.starts_with("0x") || tx_hash.len() != 66 {
        return (
            StatusCode::BAD_REQUEST,
            Json(TxStatusResponse {
                tx_hash: tx_hash.to_string(),
                status: "invalid_request".to_string(),
                block_number: None,
                gas_used: None,
                effective_gas_price: None,
                confirmations: 0,
                error_reason: Some(
                    "Invalid transaction hash format. Expected 0x followed by 64 hex characters."
                        .to_string(),
                ),
            }),
        )
            .into_response();
    }

    // 1. Check local database records (relayer_transactions, spend_queue, spend_batches)
    let db_status = match state.db.get_tx_status_any(tx_hash).await {
        Ok(s) => s,
        Err(e) => {
            eprintln!("TX_STATUS: Database query error: {}", e);
            None
        }
    };

    if let Some(mut resp) = db_status {
        // If confirmed with block number, dynamically refresh confirmations against current block
        if resp.status == "confirmed" {
            if let (Some(b_num), Some(evm_client)) = (resp.block_number, state.evm_client.as_ref())
            {
                if let Ok(current_block) = evm_client.get_block_number().await {
                    let live_confirmations = current_block.saturating_sub(b_num) + 1;
                    resp.confirmations = live_confirmations;
                }
            }
        }
        return (StatusCode::OK, Json(resp)).into_response();
    }

    // 2. If not yet in database, query live RPC if EVM client is configured
    if let Some(ref evm_client) = state.evm_client {
        match evm_client.get_receipt(tx_hash).await {
            Ok(Some(receipt)) => {
                let block_number = receipt.block_number.unwrap_or(0);
                let success = receipt.status();
                let current_block = evm_client.get_block_number().await.unwrap_or(block_number);
                let confirmations = if block_number > 0 {
                    current_block.saturating_sub(block_number) + 1
                } else {
                    1
                };
                let status_str = if success { "confirmed" } else { "failed" };
                let error_reason = if success {
                    None
                } else {
                    Some("Transaction reverted on-chain".to_string())
                };

                let resp = TxStatusResponse {
                    tx_hash: tx_hash.to_string(),
                    status: status_str.to_string(),
                    block_number: Some(block_number),
                    gas_used: Some(receipt.gas_used),
                    effective_gas_price: Some(receipt.effective_gas_price),
                    confirmations,
                    error_reason,
                };

                // Asynchronously backfill into relayer_transactions
                let _ = state
                    .db
                    .record_relayer_tx(tx_hash, 0, status_str, &evm_client.signer_address())
                    .await;
                if success {
                    let _ = state
                        .db
                        .update_relayer_tx_confirmed(
                            tx_hash,
                            block_number,
                            receipt.gas_used,
                            receipt.effective_gas_price,
                            confirmations,
                        )
                        .await;
                }

                return (StatusCode::OK, Json(resp)).into_response();
            }
            Ok(None) => {
                // Known unconfirmed or not found
            }
            Err(e) => {
                eprintln!("TX_STATUS: RPC query error: {}", e);
            }
        }
    }

    // 3. Not found in relayer records or on-chain
    (
        StatusCode::NOT_FOUND,
        Json(TxStatusResponse {
            tx_hash: tx_hash.to_string(),
            status: "not_found".to_string(),
            block_number: None,
            gas_used: None,
            effective_gas_price: None,
            confirmations: 0,
            error_reason: Some("Transaction hash not found in relayer or on-chain".to_string()),
        }),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::Database;
    use axum::body::to_bytes;
    use std::collections::HashMap;

    async fn test_state() -> (AppState, tempfile::TempDir) {
        let tmp = tempfile::TempDir::new().unwrap();
        let db_path = tmp.path().join("tx_status_test.db");
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
    async fn test_invalid_tx_hash_format() {
        let (state, _tmp) = test_state().await;
        let query = TxStatusQuery {
            tx_hash: "0x123".to_string(),
        };

        let response = handle_tx_status(State(state), Query(query)).await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let body = to_bytes(response.into_body(), 1024).await.unwrap();
        let parsed: TxStatusResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(parsed.status, "invalid_request");
    }

    #[tokio::test]
    async fn test_tx_status_from_database() {
        let (state, _tmp) = test_state().await;
        let dummy_hash = "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

        // Seed transaction record into DB
        state
            .db
            .record_relayer_tx(dummy_hash, 42, "pending", "0xsigner")
            .await
            .unwrap();

        let query = TxStatusQuery {
            tx_hash: dummy_hash.to_string(),
        };

        let response = handle_tx_status(State(state.clone()), Query(query)).await;
        assert_eq!(response.status(), StatusCode::OK);

        let body = to_bytes(response.into_body(), 1024).await.unwrap();
        let parsed: TxStatusResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(parsed.status, "pending");
        assert_eq!(parsed.tx_hash, dummy_hash);

        // Update to confirmed
        state
            .db
            .update_relayer_tx_confirmed(dummy_hash, 12345, 100_000, 20_000_000, 3)
            .await
            .unwrap();

        let query2 = TxStatusQuery {
            tx_hash: dummy_hash.to_string(),
        };
        let response2 = handle_tx_status(State(state), Query(query2)).await;
        assert_eq!(response2.status(), StatusCode::OK);

        let body2 = to_bytes(response2.into_body(), 1024).await.unwrap();
        let parsed2: TxStatusResponse = serde_json::from_slice(&body2).unwrap();
        assert_eq!(parsed2.status, "confirmed");
        assert_eq!(parsed2.block_number, Some(12345));
        assert_eq!(parsed2.gas_used, Some(100_000));
        assert_eq!(parsed2.confirmations, 3);
    }
}
