use crate::dto::{PrivateSpendQuoteRequest, PrivateSpendQuoteResponse};
use axum::{extract::Query, Json};

pub async fn handle_private_spend_quote(
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

    let Some(quote) =
        nimbus_core::quote_private_spend(query.merchant_amount, gas_cost, relayer_markup_bps)
    else {
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
        fee_bps: nimbus_core::PRIVATE_SPEND_FEE_BPS,
        relayer_markup_bps,
        message: "Informational quote only; signed max_execution_fee is not active yet".to_string(),
    })
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
        message: message.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn private_spend_quote_preserves_merchant_amount() {
        let response = handle_private_spend_quote(Query(PrivateSpendQuoteRequest {
            merchant_amount: 100_000_000,
            estimated_gas_cost: Some(20_000),
            relayer_markup_bps: Some(1_500),
        }))
        .await;

        assert_eq!(response.0.status, "OK");
        assert_eq!(response.0.contract_amount, 100_000_000);
        assert_eq!(response.0.protocol_fee, 150_000);
        assert_eq!(response.0.gas_cost, 20_000);
        assert_eq!(response.0.relayer_markup, 3_000);
        assert_eq!(response.0.execution_fee, 23_000);
        assert_eq!(response.0.user_total_debit, 100_173_000);
    }

    #[tokio::test]
    async fn private_spend_quote_rejects_zero_merchant_amount() {
        let response = handle_private_spend_quote(Query(PrivateSpendQuoteRequest {
            merchant_amount: 0,
            estimated_gas_cost: Some(0),
            relayer_markup_bps: Some(1_500),
        }))
        .await;

        assert_eq!(response.0.status, "ERROR");
    }
}
