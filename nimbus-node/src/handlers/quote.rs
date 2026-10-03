use crate::dto::{PrivateSpendQuoteRequest, PrivateSpendQuoteResponse};
use crate::state::AppState;
use alloy_primitives::{Address, U256};
use axum::extract::{Query, State};
use axum::Json;
use nimbus_sdk::eip712::{compute_quote_hashes, ExecutionQuote};
use std::str::FromStr;

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

    // Total gas cost passed into execution quote calculation = relayer gas + ccip network fee
    let total_gas_cost = relayer_gas_cost.saturating_add(ccip_network_fee.unwrap_or(0));

    if query.merchant_amount == 0 {
        return Json(error_response(
            query.merchant_amount,
            total_gas_cost,
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
                    total_gas_cost,
                    relayer_markup_bps,
                    &msg,
                ));
            }
        };

    let Some(quote) = nimbus_core::quote_private_spend_with_fee(
        query.merchant_amount,
        total_gas_cost,
        relayer_markup_bps,
        fee_bps,
    ) else {
        return Json(error_response(
            query.merchant_amount,
            total_gas_cost,
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
        + 300; // 5 minutes expiry

    // Get contract address and chain ID from state (optional for testing)
    let (contract_address, chain_id) = match state.evm_client.as_ref() {
        Some(client) => {
            let chain_id = match client.chain_id().await {
                Ok(id) => id,
                Err(_) => {
                    // Fallback: return quote without EIP-712 hashes
                    return Json(PrivateSpendQuoteResponse {
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
                        domain_separator: None,
                        struct_hash: None,
                        message: "Quote generated (EIP-712 hashes unavailable)".to_string(),
                    });
                }
            };
            (client.contract_address(), chain_id)
        }
        None => {
            // Fallback: return quote without EIP-712 hashes (for testing)
            return Json(PrivateSpendQuoteResponse {
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
                domain_separator: None,
                struct_hash: None,
                message: "Quote generated (EIP-712 hashes unavailable)".to_string(),
            });
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
        "30+ days (long-term discount)"
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

/// Generate a random 32-byte quote ID as hex string
fn generate_quote_id() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let mut bytes = [0u8; 32];
    rng.fill(&mut bytes);
    format!("0x{}", hex::encode(bytes))
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
        relayer_gas_cost: None,
        ccip_network_fee: None,
        destination_chain_selector: None,
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
        // 15% markup on 820_000 = 123_000
        assert_eq!(response.0.relayer_markup, 123_000);
        assert_eq!(response.0.execution_fee, 820_000 + 123_000);
        assert_eq!(
            response.0.user_total_debit,
            100_000_000 + 450_000 + 820_000 + 123_000
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
