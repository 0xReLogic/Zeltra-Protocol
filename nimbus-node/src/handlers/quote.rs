use crate::dto::{PrivateSpendQuoteRequest, PrivateSpendQuoteResponse};
use crate::state::AppState;
use alloy_primitives::{Address, U256};
use axum::extract::{Query, State};
use axum::Json;
use nimbus_sdk::eip712::{compute_quote_hashes, ExecutionQuote};
use std::str::FromStr;

/// Quote expiry time in seconds (5 minutes)
const QUOTE_EXPIRY_SECS: u64 = 300;

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

    // Get contract address and chain ID from state or environment (fail-closed if unavailable, DEC-028/DEC-031)
    let domain_opt: Option<(String, u64)> = if let Some(client) = state.evm_client.as_ref() {
        match client.chain_id().await {
            Ok(id) => Some((client.contract_address(), id)),
            Err(_) => None,
        }
    } else {
        None
    };

    let (contract_address, chain_id) = match domain_opt {
        Some((addr, id)) => (addr, id),
        None => {
            // Fallback to configured environment variables
            let env_chain_id = std::env::var("NIMBUS_CHAIN_ID")
                .ok()
                .and_then(|s| s.parse::<u64>().ok());
            let env_contract = std::env::var("NIMBUS_CONTRACT_ADDRESS").ok();
            match (env_contract, env_chain_id) {
                (Some(addr), Some(id)) if !addr.trim().is_empty() => (addr, id),
                _ => {
                    // DEC-031: Eliminate fallback quote with status OK without signing domain.
                    // Fail-closed: return error response when domain is missing.
                    return Json(error_response(
                        query.merchant_amount,
                        relayer_gas_cost,
                        ccip_network_fee,
                        destination_chain_selector,
                        relayer_markup_bps,
                        "EIP-712 signing domain unavailable: target chain and contract domain required",
                    ));
                }
            }
        }
    };

    let contract_addr = match Address::from_str(&contract_address) {
        Ok(addr) if addr != Address::ZERO => addr,
        _ => {
            return Json(error_response(
                query.merchant_amount,
                relayer_gas_cost,
                ccip_network_fee,
                destination_chain_selector,
                relayer_markup_bps,
                "Invalid contract address configured for EIP-712 signing domain",
            ));
        }
    };
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::Database;
    use std::collections::HashMap;

    static ENV_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

    async fn test_state() -> AppState {
        // Set environment variables for test deterministic signing domain
        std::env::set_var("NIMBUS_CHAIN_ID", "421614");
        std::env::set_var(
            "NIMBUS_CONTRACT_ADDRESS",
            "0x1111111111111111111111111111111111111111",
        );
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
        let _guard = ENV_LOCK.lock().await;
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
        assert!(response.0.domain_separator.is_some());
        assert!(response.0.struct_hash.is_some());
    }

    #[tokio::test]
    async fn private_spend_quote_cross_chain_fee_separation() {
        let _guard = ENV_LOCK.lock().await;
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
        assert!(response.0.domain_separator.is_some());
        assert!(response.0.struct_hash.is_some());
    }

    #[tokio::test]
    async fn private_spend_quote_rejects_zero_merchant_amount() {
        let _guard = ENV_LOCK.lock().await;
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

    #[tokio::test]
    async fn test_quote_fails_closed_when_signing_domain_missing() {
        let _guard = ENV_LOCK.lock().await;
        // Explicitly remove domain environment variables
        std::env::remove_var("NIMBUS_CHAIN_ID");
        std::env::remove_var("NIMBUS_CONTRACT_ADDRESS");

        let tmp = tempfile::TempDir::new().unwrap();
        let db_path = tmp.path().join("quote_test_missing_domain.db");
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

        // Restore environment variables
        std::env::set_var("NIMBUS_CHAIN_ID", "421614");
        std::env::set_var(
            "NIMBUS_CONTRACT_ADDRESS",
            "0x1111111111111111111111111111111111111111",
        );

        // DEC-031: Must fail-closed with status ERROR, never return status OK without signing domain!
        assert_eq!(response.0.status, "ERROR");
        assert!(response.0.domain_separator.is_none());
        assert!(response.0.struct_hash.is_none());
        assert!(response.0.message.contains("signing domain unavailable"));
    }

    #[tokio::test]
    async fn test_quote_fails_closed_when_contract_address_invalid() {
        let _guard = ENV_LOCK.lock().await;
        std::env::set_var("NIMBUS_CHAIN_ID", "421614");
        std::env::set_var(
            "NIMBUS_CONTRACT_ADDRESS",
            "0x0000000000000000000000000000000000000000",
        );

        let tmp = tempfile::TempDir::new().unwrap();
        let db_path = tmp.path().join("quote_test_invalid_contract.db");
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

        // Restore environment variables
        std::env::set_var("NIMBUS_CHAIN_ID", "421614");
        std::env::set_var(
            "NIMBUS_CONTRACT_ADDRESS",
            "0x1111111111111111111111111111111111111111",
        );

        assert_eq!(response.0.status, "ERROR");
        assert!(response.0.domain_separator.is_none());
        assert!(response.0.struct_hash.is_none());
        assert!(
            response.0.message.contains("Invalid contract address")
                || response.0.message.contains("signing domain unavailable")
        );
    }
}
