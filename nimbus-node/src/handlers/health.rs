//! Health check handler

use crate::{
    dto::HealthResponse, dto::SigningHealthResponse, dto::StalledSessionDto, state::AppState,
};
use axum::Json;

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

pub async fn signing_health(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> Json<SigningHealthResponse> {
    // Default threshold: 10 minutes (600 seconds) for signing stall detection
    let threshold_seconds = std::env::var("NIMBUS_SIGNING_STALL_THRESHOLD")
        .unwrap_or_else(|_| "600".to_string())
        .parse()
        .unwrap_or(600);

    let stalled = match state
        .db
        .get_stalled_signing_sessions(threshold_seconds)
        .await
    {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Signing health: Failed to get stalled sessions: {}", e);
            return Json(SigningHealthResponse {
                stalled_sessions: vec![],
                stalled_count: 0,
                check_threshold_seconds: threshold_seconds,
                message: format!("Error checking stalled sessions: {}", e),
            });
        }
    };

    let stalled_dtos: Vec<StalledSessionDto> = stalled
        .into_iter()
        .map(|s| StalledSessionDto {
            session_id: s.session_id,
            amount: s.amount as u64,
            client_address: s.client_address,
            created_at: s.created_at,
            deposit_confirmed: s.deposit_confirmed == 1,
            resolved: s.resolved == 1,
        })
        .collect();

    let stalled_count = stalled_dtos.len();
    let message = if stalled_count == 0 {
        "No stalled signing sessions detected".to_string()
    } else {
        format!(
            "Warning: {} stalled signing sessions detected (threshold: {}s)",
            stalled_count, threshold_seconds
        )
    };

    Json(SigningHealthResponse {
        stalled_sessions: stalled_dtos,
        stalled_count,
        check_threshold_seconds: threshold_seconds,
        message,
    })
}
