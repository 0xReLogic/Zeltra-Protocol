use axum::{extract::Query, Json};
use crate::dto::{PrivateSpendQuoteRequest, PrivateSpendQuoteResponse};

pub async fn handle_private_spend_quote(
    Query(query): Query<PrivateSpendQuoteRequest>,
) -> Json<PrivateSpendQuoteResponse> {
    let execution_fee = query.execution_fee.unwrap_or_else(default_execution_fee);
    if query.merchant_amount == 0 {
        return Json(error_response(
            query.merchant_amount,
            execution_fee,
            "merchant_amount must be greater than zero",
        ));
    }

    let Some(quote) =
        nimbus_core::quote_private_spend(query.merchant_amount, execution_fee)
    else {
        return Json(error_response(
            query.merchant_amount,
            execution_fee,
            "quote amount overflow",
        ));
    };

    Json(PrivateSpendQuoteResponse {
        status: "OK".to_string(),
        merchant_amount: quote.merchant_amount,
        contract_amount: quote.contract_amount,
        protocol_fee: quote.protocol_fee,
        execution_fee: quote.execution_fee,
        user_total_debit: quote.user_total_debit,
        fee_bps: nimbus_core::PRIVATE_SPEND_FEE_BPS,
        message: "Informational quote only; signed max_execution_fee is not active yet"
            .to_string(),
    })
}

fn default_execution_fee() -> u64 {
    std::env::var("NIMBUS_EXECUTION_FEE_USDC_BASE_UNITS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(0)
}

fn error_response(
    merchant_amount: u64,
    execution_fee: u64,
    message: &str,
) -> PrivateSpendQuoteResponse {
    PrivateSpendQuoteResponse {
        status: "ERROR".to_string(),
        merchant_amount,
        contract_amount: 0,
        protocol_fee: 0,
        execution_fee,
        user_total_debit: 0,
        fee_bps: nimbus_core::PRIVATE_SPEND_FEE_BPS,
        message: message.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn private_spend_quote_grosses_up_merchant_amount() {
        let response = handle_private_spend_quote(Query(PrivateSpendQuoteRequest {
            merchant_amount: 100_000_000,
            execution_fee: Some(25_000),
        }))
        .await;

        assert_eq!(response.0.status, "OK");
        assert_eq!(response.0.contract_amount, 100_150_226);
        assert_eq!(response.0.protocol_fee, 150_226);
        assert_eq!(response.0.user_total_debit, 100_175_226);
    }

    #[tokio::test]
    async fn private_spend_quote_rejects_zero_merchant_amount() {
        let response = handle_private_spend_quote(Query(PrivateSpendQuoteRequest {
            merchant_amount: 0,
            execution_fee: Some(0),
        }))
        .await;

        assert_eq!(response.0.status, "ERROR");
    }
}
