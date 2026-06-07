//! Health check handler

use axum::Json;
use crate::{state::AppState, dto::HealthResponse};

pub async fn health_check(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> Json<HealthResponse> {
    let relayer_wallet_balance_eth = *state.relayer_wallet_balance_eth.lock().await;
    let relayer_accumulated_profit_usdc = *state.relayer_accumulated_profit_usdc.lock().await;
    
    // 1. Verify database connectivity
    let mut db_ok = true;
    let processed_nullifiers = match state.db.get_stats().await {
        Ok(stats) => stats.total_nullifiers as usize,
        Err(e) => {
            eprintln!("Health check: Failed to get database stats: {}", e);
            db_ok = false;
            0
        }
    };
    let queued_transactions = match state.db.queued_spend_count().await {
        Ok(count) => count,
        Err(e) => {
            eprintln!("Health check: Failed to count persistent queue: {}", e);
            db_ok = false;
            0
        }
    };
    
    // 2. Verify blockchain RPC connectivity
    let rpc_ok = if let Some(ref evm_client) = state.evm_client {
        evm_client.get_gas_price().await.is_ok()
    } else {
        true
    };
    
    let status = if !db_ok {
        "ERROR_DATABASE_DOWN".to_string()
    } else if !rpc_ok {
        "DEGRADED_RPC_DOWN".to_string()
    } else {
        "OK".to_string()
    };
    
    Json(HealthResponse {
        status,
        queued_transactions,
        processed_nullifiers,
        relayer_wallet_balance_eth,
        relayer_accumulated_profit_usdc,
    })
}
