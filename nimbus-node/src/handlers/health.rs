//! Health check handler

use axum::Json;
use crate::{state::AppState, dto::HealthResponse};

pub async fn health_check(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> Json<HealthResponse> {
    let queue = state.spend_queue.lock().await;
    let relayer_wallet_balance_eth = *state.relayer_wallet_balance_eth.lock().await;
    let relayer_accumulated_profit_usdc = *state.relayer_accumulated_profit_usdc.lock().await;
    
    // Get nullifier count from database
    let processed_nullifiers = match state.db.get_stats().await {
        Ok(stats) => stats.total_nullifiers as usize,
        Err(e) => {
            eprintln!("Health check: Failed to get database stats: {}", e);
            0
        }
    };
    
    Json(HealthResponse {
        status: "OK".to_string(),
        queued_transactions: queue.len(),
        processed_nullifiers,
        relayer_wallet_balance_eth,
        relayer_accumulated_profit_usdc,
    })
}
