use crate::dto::{PrivateSpendQuoteRequest, PrivateSpendQuoteResponse};
use crate::state::AppState;
use alloy_primitives::{Address, U256};
use axum::extract::{Query, State};
use axum::Json;
use nimbus_sdk::eip712::{compute_quote_hashes, ExecutionQuote};
use std::str::FromStr;

/// Quote expiry time in seconds (5 minutes)
const QUOTE_EXPIRY_SECS: u64 = 300;

/// Parameters for building a fallback quote response (when EIP-712 hashes unavailable)
struct FallbackQuoteParams<'a> {
    quote: &'a nimbus_core::SpendQuote,
    fee_bps: u64,
    relayer_markup_bps: u64,
    relayer_gas_cost: u64,
    ccip_network_fee: Option<u64>,
    destination_chain_selector: Option<u64>,
    fee_tier: Option<String>,
    discount_bps: Option<u64>,
    quote_id: Option<String>,
    quote_expiry: Option<u64>,
}

pub async fn handle_private_spend_quote(
    State(state): State<AppState>,
    Query(query): Query<PrivateSpendQuoteRequest>,
) -> Json<PrivateSpendQuoteResponse> {
    let relayer_gas_cost = query.estimated_gas_cost.unwrap_or_else(default_gas_cost);
    let relayer_markup_bps = query
        .relayer_markup_bps
        .unwrap_or_else(default_relayer_markup_bps);

    // CCIP fee breakdown: if destination_chain_selector is provided, determine CCIP network fee
    let (ccip_network_fee, destination_chain_selector) = match query.destination_chain_selector {
        Some(selector) => {
            // Default CCIP network fee is 800,000 base units (0.80 USDC) or configurable via env
            let fee = std::env::var("NIMBUS_CCIP_NETWORK_FEE_USDC")
                .ok()
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(800_000);
            (Some(fee), Some(selector))
        }
        None => (None, None),
    };

    if query.merchant_amount == 0 {
        return Json(error_response(
            query.merchant_amount,
            relayer_gas_cost,
            ccip_network_fee,
            destination_chain_selector,
            relayer_markup_bps,
            "merchant_amount must be greater than zero",
        ));
    }

    // DEC-028: Protocol spend fee is unified to flat 45 bps (0.45%) permanently.
    // Holding-time discount tiers and root aging gaming are abolished.
    let fee_bps = nimbus_core::PRIVATE_SPEND_FEE_BPS;
    let fee_tier = None;
    let discount_bps = None;

    // Calculate spend quote: markup is applied ONLY to relayer gas, CCIP fee is pass-through
    let quote_opt = match ccip_network_fee {
        Some(ccip_fee) => nimbus_core::quote_cross_chain_private_spend_with_fee(
            query.merchant_amount,
            relayer_gas_cost,
            ccip_fee,
            relayer_markup_bps,
            fee_bps,
        ),
        None => nimbus_core::quote_private_spend_with_fee(
            query.merchant_amount,
            relayer_gas_cost,
            relayer_markup_bps,
            fee_bps,
        ),
    };

    let Some(quote) = quote_opt else {
        return Json(error_response(
            query.merchant_amount,
            relayer_gas_cost,
            ccip_network_fee,
            destination_chain_selector,
            relayer_markup_bps,
            "quote amount overflow",
        ));
    };

    // Generate quote ID and expiry for EIP-712 signing
    let quote_id_hex = generate_quote_id();
    let quote_expiry = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
        + QUOTE_EXPIRY_SECS;

    // Get contract address and chain ID from state (optional for testing)
    let (contract_address, chain_id) = match state.evm_client.as_ref() {
        Some(client) => {
            let chain_id = match client.chain_id().await {
                Ok(id) => id,
                Err(_) => {
                    return Json(fallback_quote_response(FallbackQuoteParams {
                        quote: &quote,
                        fee_bps,
                        relayer_markup_bps,
                        relayer_gas_cost,
                        ccip_network_fee,
                        destination_chain_selector,
                        fee_tier,
                        discount_bps,
                        quote_id: Some(quote_id_hex),
                        quote_expiry: Some(quote_expiry),
                    }));
                }
            };
            (client.contract_address(), chain_id)
        }
        None => {
            return Json(fallback_quote_response(FallbackQuoteParams {
                quote: &quote,
                fee_bps,
                relayer_markup_bps,
                relayer_gas_cost,
                ccip_network_fee,
                destination_chain_selector,
                fee_tier,
                discount_bps,
                quote_id: Some(quote_id_hex),
                quote_expiry: Some(quote_expiry),
            }));
        }
    };

    let contract_addr = Address::from_str(&contract_address).unwrap_or(Address::ZERO);
    let relayer_addr = Address::from_str(&state.relayer_address).unwrap_or(Address::ZERO);

    // Build EIP-712 ExecutionQuote struct
    let execution_quote = ExecutionQuote {
        quoteId: U256::from_str(&quote_id_hex).unwrap_or(U256::ZERO),
        maxExecutionFee: U256::from(quote.execution_fee),
        merchantAmount: U256::from(quote.merchant_amount),
        quoteExpiry: U256::from(quote_expiry),
        relayerAddress: relayer_addr,
    };

    // Compute EIP-712 hashes for client verification
    let (domain_separator, struct_hash) =
        compute_quote_hashes(&execution_quote, chain_id, contract_addr);

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
        relayer_gas_cost: Some(relayer_gas_cost),
        ccip_network_fee,
        destination_chain_selector,
        fee_tier,
        discount_bps,
        quote_id: Some(quote_id_hex),
        quote_expiry: Some(quote_expiry),
        domain_separator: Some(format!("0x{}", hex::encode(domain_separator))),
        struct_hash: Some(format!("0x{}", hex::encode(struct_hash))),
        message: "Sign the ExecutionQuote message in your wallet to authorize this spend"
            .to_string(),
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

/// Generate a random 32-byte quote ID as hex string
fn generate_quote_id() -> String {
    let bytes: [u8; 32] = rand::random();
    format!("0x{}", hex::encode(bytes))
}

fn error_response(
    merchant_amount: u64,
    relayer_gas_cost: u64,
    ccip_network_fee: Option<u64>,
    destination_chain_selector: Option<u64>,
    relayer_markup_bps: u64,
    message: &str,
) -> PrivateSpendQuoteResponse {
    let (relayer_markup, execution_fee) = match ccip_network_fee {
        Some(ccip_fee) => nimbus_core::quote_execution_fee_with_pass_through(
            relayer_gas_cost,
            ccip_fee,
            relayer_markup_bps,
        )
        .unwrap_or((0, 0)),
        None => {
            nimbus_core::quote_execution_fee(relayer_gas_cost, relayer_markup_bps).unwrap_or((0, 0))
        }
    };
    let gas_cost = relayer_gas_cost.saturating_add(ccip_network_fee.unwrap_or(0));
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
        relayer_gas_cost: Some(relayer_gas_cost),
        ccip_network_fee,
        destination_chain_selector,
        fee_tier: None,
        discount_bps: None,
        quote_id: None,
        quote_expiry: None,
        domain_separator: None,
        struct_hash: None,
        message: message.to_string(),
    }
}

fn fallback_quote_response(params: FallbackQuoteParams) -> PrivateSpendQuoteResponse {
    PrivateSpendQuoteResponse {
        status: "OK".to_string(),
        merchant_amount: params.quote.merchant_amount,
        contract_amount: params.quote.contract_amount,
        protocol_fee: params.quote.protocol_fee,
        gas_cost: params.quote.gas_cost,
        relayer_markup: params.quote.relayer_markup,
        execution_fee: params.quote.execution_fee,
        user_total_debit: params.quote.user_total_debit,
        fee_bps: params.fee_bps,
        relayer_markup_bps: params.relayer_markup_bps,
        relayer_gas_cost: Some(params.relayer_gas_cost),
        ccip_network_fee: params.ccip_network_fee,
        destination_chain_selector: params.destination_chain_selector,
        fee_tier: params.fee_tier,
        discount_bps: params.discount_bps,
        quote_id: params.quote_id,
        quote_expiry: params.quote_expiry,
        domain_separator: None,
        struct_hash: None,
        message: "Quote generated (EIP-712 hashes unavailable)".to_string(),
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
                destination_chain_selector: None,
            }),
        )
        .await;

        assert_eq!(response.0.status, "OK");
        assert_eq!(response.0.contract_amount, 100_000_000);
        assert_eq!(response.0.protocol_fee, 450_000);
        assert_eq!(response.0.gas_cost, 20_000);
        assert_eq!(response.0.relayer_markup, 3_000);
        assert_eq!(response.0.execution_fee, 23_000);
        assert_eq!(response.0.user_total_debit, 100_473_000);
        assert_eq!(response.0.relayer_gas_cost, Some(20_000));
        assert!(response.0.ccip_network_fee.is_none());
        assert!(response.0.destination_chain_selector.is_none());
        assert!(response.0.fee_tier.is_none());
        assert!(response.0.discount_bps.is_none());
    }

    #[tokio::test]
    async fn private_spend_quote_cross_chain_fee_separation() {
        let state = test_state().await;
        let base_sepolia_selector = 10344971235874465080u64;
        let response = handle_private_spend_quote(
            State(state),
            Query(PrivateSpendQuoteRequest {
                merchant_amount: 100_000_000,
                estimated_gas_cost: Some(20_000),
                relayer_markup_bps: Some(1_500),
                association_root: None,
                destination_chain_selector: Some(base_sepolia_selector),
            }),
        )
        .await;

        assert_eq!(response.0.status, "OK");
        assert_eq!(response.0.merchant_amount, 100_000_000);
        assert_eq!(response.0.contract_amount, 100_000_000);
        assert_eq!(response.0.protocol_fee, 450_000);
        // CCIP network fee is 800_000, relayer gas is 20_000 -> total gas cost = 820_000
        assert_eq!(response.0.relayer_gas_cost, Some(20_000));
        assert_eq!(response.0.ccip_network_fee, Some(800_000));
        assert_eq!(
            response.0.destination_chain_selector,
            Some(base_sepolia_selector)
        );
        assert_eq!(response.0.gas_cost, 820_000);
        // Relayer markup applies ONLY to relayer gas (15% of 20_000 = 3_000), NOT to CCIP fee!
        assert_eq!(response.0.relayer_markup, 3_000);
        assert_eq!(response.0.execution_fee, 20_000 + 3_000 + 800_000);
        assert_eq!(
            response.0.user_total_debit,
            100_000_000 + 450_000 + 20_000 + 3_000 + 800_000
        );
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
                destination_chain_selector: None,
            }),
        )
        .await;

        assert_eq!(response.0.status, "ERROR");
    }
}
