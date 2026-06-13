use crate::dto::{PrivateSpendQuoteRequest, PrivateSpendQuoteResponse};
use crate::state::AppState;
use axum::extract::{Query, State};
use axum::Json;

pub async fn handle_private_spend_quote(
    State(state): State<AppState>,
    Query(query): Query<PrivateSpendQuoteRequest>,
) -> Json<PrivateSpendQuoteResponse> {
    let gas_cost = query.estimated_gas_cost.unwrap_or_else(default_gas_cost);
    let relayer_markup_bps = query
        .relayer_markup_bps
        .unwrap_or_else(default_relayer_markup_bps);
    if query.merchant_amount == 0 {
        return Json(error_response(
            query.merchant_amount,
            gas_cost,
            relayer_markup_bps,
            "merchant_amount must be greater than zero",
        ));
    }

    // Determine fee tier from association_root if provided
    let (fee_bps, fee_tier, discount_bps) =
        match resolve_fee_tier(&state, query.association_root.as_deref()).await {
            Ok(info) => info,
            Err(msg) => {
                return Json(error_response(
                    query.merchant_amount,
                    gas_cost,
                    relayer_markup_bps,
                    &msg,
                ));
            }
        };

    let Some(quote) = nimbus_core::quote_private_spend_with_fee(
        query.merchant_amount,
        gas_cost,
        relayer_markup_bps,
        fee_bps,
    ) else {
        return Json(error_response(
            query.merchant_amount,
            gas_cost,
            relayer_markup_bps,
            "quote amount overflow",
        ));
    };

    Json(PrivateSpendQuoteResponse {
        status: "OK".to_string(),
        merchant_amount: quote.merchant_amount,
        contract_amount: quote.contract_amount,
        protocol_fee: quote.protocol_fee,
        gas_cost: quote.gas_cost,
        relayer_markup: quote.relayer_markup,
        execution_fee: quote.execution_fee,
        user_total_debit: quote.user_total_debit,
        fee_bps,
        relayer_markup_bps,
        fee_tier,
        discount_bps,
        message: "Informational quote only; signed max_execution_fee is not active yet".to_string(),
    })
}

/// Resolves the applicable fee tier. Returns (fee_bps, tier_label, discount_bps).
/// If no root provided or root not found, falls back to default 25 bps.
async fn resolve_fee_tier(
    state: &AppState,
    association_root: Option<&str>,
) -> Result<(u64, Option<String>, Option<u64>), String> {
    let Some(root) = association_root else {
        return Ok((nimbus_core::PRIVATE_SPEND_FEE_BPS, None, None));
    };

    let root_timestamp = get_root_timestamp(state, root).await?;
    if root_timestamp == 0 {
        return Ok((
            nimbus_core::PRIVATE_SPEND_FEE_BPS,
            Some("unregistered".to_string()),
            None,
        ));
    }

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let holding_secs = now.saturating_sub(root_timestamp);
    let fee_bps = nimbus_core::spend_fee_bps_for_holding(holding_secs);
    let discount = nimbus_core::PRIVATE_SPEND_FEE_BPS.saturating_sub(fee_bps);

    let tier = if fee_bps == nimbus_core::SPEND_FEE_30DAY_BPS {
        "30+ days (whale discount)"
    } else if fee_bps == nimbus_core::SPEND_FEE_7DAY_BPS {
        "7+ days"
    } else {
        "default"
    };

    Ok((
        fee_bps,
        Some(tier.to_string()),
        if discount > 0 { Some(discount) } else { None },
    ))
}

/// Gets root timestamp from cache or RPC, populating cache on miss.
async fn get_root_timestamp(state: &AppState, root_hex: &str) -> Result<u64, String> {
    let normalized = root_hex.trim_start_matches("0x").to_lowercase();

    // Check cache
    {
        let cache = state.root_timestamp_cache.lock().await;
        if let Some(&ts) = cache.get(&normalized) {
            return Ok(ts);
        }
    }

    // Cache miss — query RPC
    let Some(ref evm) = state.evm_client else {
        return Ok(0);
    };

    let ts = evm
        .get_clean_root_timestamp(root_hex)
        .await
        .map_err(|e| format!("RPC query failed: {}", e))?;

    // Populate cache
    {
        let mut cache = state.root_timestamp_cache.lock().await;
        cache.insert(normalized, ts);
    }

    Ok(ts)
}

fn default_gas_cost() -> u64 {
    std::env::var("NIMBUS_ESTIMATED_GAS_COST_USDC_BASE_UNITS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(0)
}

fn default_relayer_markup_bps() -> u64 {
    std::env::var("NIMBUS_RELAYER_MARKUP_BPS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(nimbus_core::DEFAULT_RELAYER_MARKUP_BPS)
}

fn error_response(
    merchant_amount: u64,
    gas_cost: u64,
    relayer_markup_bps: u64,
    message: &str,
) -> PrivateSpendQuoteResponse {
    let (relayer_markup, execution_fee) =
        nimbus_core::quote_execution_fee(gas_cost, relayer_markup_bps).unwrap_or((0, 0));
    PrivateSpendQuoteResponse {
        status: "ERROR".to_string(),
        merchant_amount,
        contract_amount: 0,
        protocol_fee: 0,
        gas_cost,
        relayer_markup,
        execution_fee,
        user_total_debit: 0,
        fee_bps: nimbus_core::PRIVATE_SPEND_FEE_BPS,
        relayer_markup_bps,
        fee_tier: None,
        discount_bps: None,
        message: message.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::Database;
    use std::collections::HashMap;

    async fn test_state() -> AppState {
        let tmp = tempfile::TempDir::new().unwrap();
        let db_path = tmp.path().join("quote_test.db");
        let db = Database::new(&db_path).await.unwrap();
        AppState::new(
            db,
            nimbus_core::Fr::from(1u64),
            1,
            nimbus_core::IssuerPublicKey(nimbus_core::G2Projective::default()),
            HashMap::new(),
            None,
        )
        .await
    }

    #[tokio::test]
    async fn private_spend_quote_preserves_merchant_amount() {
        let state = test_state().await;
        let response = handle_private_spend_quote(
            State(state),
            Query(PrivateSpendQuoteRequest {
                merchant_amount: 100_000_000,
                estimated_gas_cost: Some(20_000),
                relayer_markup_bps: Some(1_500),
                association_root: None,
            }),
        )
        .await;

        assert_eq!(response.0.status, "OK");
        assert_eq!(response.0.contract_amount, 100_000_000);
        assert_eq!(response.0.protocol_fee, 250_000);
        assert_eq!(response.0.gas_cost, 20_000);
        assert_eq!(response.0.relayer_markup, 3_000);
        assert_eq!(response.0.execution_fee, 23_000);
        assert_eq!(response.0.user_total_debit, 100_273_000);
        assert!(response.0.fee_tier.is_none());
        assert!(response.0.discount_bps.is_none());
    }

    #[tokio::test]
    async fn private_spend_quote_rejects_zero_merchant_amount() {
        let state = test_state().await;
        let response = handle_private_spend_quote(
            State(state),
            Query(PrivateSpendQuoteRequest {
                merchant_amount: 0,
                estimated_gas_cost: Some(0),
                relayer_markup_bps: Some(1_500),
                association_root: None,
            }),
        )
        .await;

        assert_eq!(response.0.status, "ERROR");
    }
}
