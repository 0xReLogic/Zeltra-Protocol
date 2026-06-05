//! Health check handler

use axum::Json;
use crate::{state::AppState, dto::HealthResponse};

pub async fn health_check(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> Json<HealthResponse> {
    let queue = state.spend_queue.lock().await;
    let nulls = state.nullifiers.lock().await;
    let relayer_wallet_balance_eth = *state.relayer_wallet_balance_eth.lock().await;
    let relayer_accumulated_profit_usdc = *state.relayer_accumulated_profit_usdc.lock().await;
    Json(HealthResponse {
        status: "OK".to_string(),
        queued_transactions: queue.len(),
        processed_nullifiers: nulls.len(),
        relayer_wallet_balance_eth,
        relayer_accumulated_profit_usdc,
    })
}
