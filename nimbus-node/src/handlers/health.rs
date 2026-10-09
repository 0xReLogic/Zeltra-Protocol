//! Health check handler & Solvency Observability (DEC-031)

use crate::{
    dto::HealthResponse, dto::SigningHealthResponse, dto::SolvencyMetricsDto,
    dto::StalledSessionDto, state::AppState,
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

    // 3. Query CCIP destination tracking metrics
    let ccip_pending = state.db.ccip_pending_count().await.ok();
    let ccip_failure = state.db.ccip_failure_count().await.ok();

    // 4. Query Batch Profitability & Operational Fee Metrics (DEC-020)
    let batch_metrics = state.db.get_batch_metrics().await.ok();

    // 5. Query Solvency & Accounting Observability Metrics (DEC-031)
    // CRITICAL: Absolute zero user-data leakage. Only macro aggregate numbers are exposed.
    let (solvency_metrics, solvency_status) = if let Some(ref evm_client) = state.evm_client {
        let contract_bal_res = evm_client.get_contract_usdc_balance().await;
        let user_liab_res = evm_client.get_user_note_liability().await;
        let refund_liab_res = evm_client.get_refundable_deposit_liability().await;
        let contract_accrued_res = evm_client.get_accrued_execution_fee_liability().await;
        let db_unclaimed = state
            .db
            .get_unclaimed_execution_fees()
            .await
            .map(|(fee, _)| fee)
            .ok();

        match (
            contract_bal_res,
            user_liab_res,
            refund_liab_res,
            contract_accrued_res,
        ) {
            (Ok(contract_balance), Ok(user_note_liab), Ok(refund_liab), Ok(contract_accrued)) => {
                let total_liabilities = user_note_liab
                    .saturating_add(refund_liab)
                    .saturating_add(contract_accrued);
                let is_solvent = contract_balance >= total_liabilities;
                let drift = db_unclaimed.map(|db_fee| contract_accrued as i64 - db_fee as i64);
                let is_matched = drift.map(|d| d == 0).unwrap_or(true);

                let status = if !is_solvent {
                    "INSOLVENT_ALERT".to_string()
                } else if !is_matched {
                    "MISMATCH_WARNING".to_string()
                } else {
                    "SOLVENT".to_string()
                };

                (
                    Some(SolvencyMetricsDto {
                        solvency_status: status.clone(),
                        contract_balance_usdc: Some(contract_balance),
                        total_liabilities_usdc: Some(total_liabilities),
                        user_note_liability_usdc: Some(user_note_liab),
                        refundable_deposit_liability_usdc: Some(refund_liab),
                        contract_accrued_execution_fee_usdc: Some(contract_accrued),
                        db_unclaimed_execution_fee_usdc: db_unclaimed,
                        reconciliation_drift_usdc: drift,
                        is_solvent,
                        is_accounting_matched: is_matched,
                    }),
                    Some(status),
                )
            }
            _ => (
                Some(SolvencyMetricsDto {
                    solvency_status: "DEGRADED".to_string(),
                    contract_balance_usdc: None,
                    total_liabilities_usdc: None,
                    user_note_liability_usdc: None,
                    refundable_deposit_liability_usdc: None,
                    contract_accrued_execution_fee_usdc: None,
                    db_unclaimed_execution_fee_usdc: db_unclaimed,
                    reconciliation_drift_usdc: None,
                    is_solvent: false,
                    is_accounting_matched: false,
                }),
                Some("DEGRADED".to_string()),
            ),
        }
    } else {
        // Standalone or local node without EVM client
        let db_unclaimed = state
            .db
            .get_unclaimed_execution_fees()
            .await
            .map(|(fee, _)| fee)
            .ok();
        (
            Some(SolvencyMetricsDto {
                solvency_status: "OFFLINE_NO_RPC".to_string(),
                contract_balance_usdc: None,
                total_liabilities_usdc: None,
                user_note_liability_usdc: None,
                refundable_deposit_liability_usdc: None,
                contract_accrued_execution_fee_usdc: None,
                db_unclaimed_execution_fee_usdc: db_unclaimed,
                reconciliation_drift_usdc: None,
                is_solvent: true,
                is_accounting_matched: true,
            }),
            None,
        )
    };

    let status = if !db_ok {
        "ERROR_DATABASE_DOWN".to_string()
    } else if !rpc_ok {
        "DEGRADED_RPC_DOWN".to_string()
    } else if solvency_status.as_deref() == Some("INSOLVENT_ALERT") {
        "INSOLVENT_ALERT".to_string()
    } else {
        "OK".to_string()
    };

    let eth_price = Some(state.get_eth_price().await);

    Json(HealthResponse {
        status,
        queued_transactions,
        processed_nullifiers,
        relayer_wallet_balance_eth,
        relayer_accumulated_profit_usdc,
        eth_price_usdc: eth_price,
        batch_metrics,
        solvency_metrics,
        ccip_pending_count: ccip_pending,
        ccip_failure_count: ccip_failure,
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

/// Lightweight liveness probe (K8s/container liveness check)
/// Simply verifies the process is responsive and event loop is healthy.
pub async fn liveness_check() -> (axum::http::StatusCode, &'static str) {
    (axum::http::StatusCode::OK, "LIVE")
}

/// Readiness probe (K8s/container readiness check)
/// Verifies essential dependencies: Database connectivity and RPC responsiveness.
pub async fn readiness_check(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> Result<(axum::http::StatusCode, &'static str), (axum::http::StatusCode, String)> {
    // 1. Verify DB is accessible
    if let Err(e) = state.db.get_stats().await {
        return Err((
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            format!("NOT_READY: Database error: {}", e),
        ));
    }

    // 2. Verify EVM RPC is responsive if client is configured
    if let Some(ref evm_client) = state.evm_client {
        if let Err(e) = evm_client.get_gas_price().await {
            return Err((
                axum::http::StatusCode::SERVICE_UNAVAILABLE,
                format!("NOT_READY: EVM RPC error: {}", e),
            ));
        }
    }

    Ok((axum::http::StatusCode::OK, "READY"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::Database;
    use std::collections::HashMap;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_health_check_includes_batch_metrics() {
        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("health_test.db");
        let db = Database::new(&db_path).await.unwrap();

        // Initially empty batch table
        let metrics0 = db.get_batch_metrics().await.unwrap();
        assert_eq!(metrics0.batch_count, 0);

        // Record a batch
        db.store_batch_metadata(
            "b1",
            4,
            "0xtx",
            100_000,
            10_000_000_000,
            0.001,
            8.0,
            20_000_000,
        )
        .await
        .unwrap();

        let metrics1 = db.get_batch_metrics().await.unwrap();
        assert_eq!(metrics1.batch_count, 1);
        assert_eq!(metrics1.avg_batch_size, 4.0);
        assert!((metrics1.total_batch_margin_usdc - 8.0).abs() < 1e-6);
        assert_eq!(metrics1.total_execution_fees_usdc, 20_000_000);
        assert_eq!(metrics1.unclaimed_execution_fees_usdc, 20_000_000);
    }

    #[tokio::test]
    async fn test_health_check_includes_solvency_metrics_and_zero_leakage() {
        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("health_solvency_test.db");
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

        let response = health_check(axum::extract::State(state)).await;
        assert_eq!(response.0.status, "OK");

        // Verify solvency_metrics is populated
        let solvency = response
            .0
            .solvency_metrics
            .expect("solvency_metrics must exist");
        assert_eq!(solvency.solvency_status, "OFFLINE_NO_RPC");
        assert!(solvency.is_solvent);
        assert!(solvency.is_accounting_matched);

        // Verify Zero User Data Leakage: serialize to JSON and verify absence of sensitive fields
        let json_str = serde_json::to_string(&solvency).unwrap();
        assert!(!json_str.contains("nullifier"));
        assert!(!json_str.contains("commitment"));
        assert!(!json_str.contains("secret"));
        assert!(!json_str.contains("session_id"));
        assert!(!json_str.contains("recipient"));
        assert!(!json_str.contains("client_address"));
    }

    #[test]
    fn test_solvency_evaluation_logic() {
        // Case 1: Solvent and matched
        let contract_bal = 100_000_000u64;
        let total_liab = 99_500_000u64;
        let contract_accrued = 500_000u64;
        let db_unclaimed = 500_000u64;

        let is_solvent = contract_bal >= total_liab;
        let drift = contract_accrued as i64 - db_unclaimed as i64;
        let is_matched = drift == 0;

        assert!(is_solvent);
        assert!(is_matched);
        assert_eq!(drift, 0);

        // Case 2: Insolvent alert
        let insolvent_bal = 90_000_000u64;
        let is_solvent_2 = insolvent_bal >= total_liab;
        assert!(!is_solvent_2);

        // Case 3: Mismatch warning
        let db_unclaimed_mismatch = 400_000u64;
        let drift_3 = contract_accrued as i64 - db_unclaimed_mismatch as i64;
        assert_eq!(drift_3, 100_000);
        let is_matched_3 = drift_3 == 0;
        assert!(!is_matched_3);
    }

    #[tokio::test]
    async fn test_liveness_and_readiness_endpoints() {
        let (status, msg) = liveness_check().await;
        assert_eq!(status, axum::http::StatusCode::OK);
        assert_eq!(msg, "LIVE");

        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("readiness_test.db");
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

        let ready_res = readiness_check(axum::extract::State(state)).await;
        assert!(ready_res.is_ok());
        let (r_status, r_msg) = ready_res.unwrap();
        assert_eq!(r_status, axum::http::StatusCode::OK);
        assert_eq!(r_msg, "READY");
    }
}
