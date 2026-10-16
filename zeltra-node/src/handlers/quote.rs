use crate::dto::{
    ExecutionQuoteRequest, ExecutionQuoteResponse, PrivateSpendQuoteRequest,
    PrivateSpendQuoteResponse,
};
use crate::state::AppState;
use alloy_primitives::{Address, U256};
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::Json;
use std::str::FromStr;
use zeltra_sdk::eip712::{compute_quote_hash, compute_quote_hashes, sign_quote, ExecutionQuote};

/// Quote expiry time in seconds (DEC-034C: 20 seconds strict anti-arbitrage window)
pub const QUOTE_EXPIRY_SECS: u64 = 20;

/// Create an EIP-712 signed execution quote via POST (DEC-034C / DEC-038C)
pub async fn create_quote(
    State(state): State<AppState>,
    Json(payload): Json<ExecutionQuoteRequest>,
) -> Result<Json<ExecutionQuoteResponse>, (StatusCode, Json<serde_json::Value>)> {
    if payload.asset_id > 1 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "status": "ERROR",
                "message": format!("Unsupported asset_id: {}. Only 0 (WETH) and 1 (USDC) are supported", payload.asset_id)
            })),
        ));
    }

    // Resolve target contract and chain ID
    let (contract_address, chain_id) = if let Some(client) = state.evm_client.as_ref() {
        let id = client.chain_id().await.unwrap_or(421614);
        let addr = if !payload.target_contract.trim().is_empty() {
            payload.target_contract.clone()
        } else {
            client.contract_address()
        };
        (addr, id)
    } else {
        let env_chain_id = std::env::var("ZELTRA_CHAIN_ID")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(421614);
        let env_contract = std::env::var("ZELTRA_CONTRACT_ADDRESS").unwrap_or_else(|_| {
            if !payload.target_contract.trim().is_empty() {
                payload.target_contract.clone()
            } else {
                "0x000000000000000000000000000000000000012c".to_string()
            }
        });
        (env_contract, env_chain_id)
    };

    let contract_addr = match Address::from_str(&contract_address) {
        Ok(addr) if addr != Address::ZERO => addr,
        _ => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "status": "ERROR",
                    "message": "Invalid contract address configured for EIP-712 signing domain"
                })),
            ));
        }
    };

    let gas_limit = if payload.estimated_gas_limit > 0 {
        payload.estimated_gas_limit
    } else {
        500_000
    };

    // Calculate base gas fee in native asset units (DEC-034C Zero-Oracle):
    // asset_id 0 (WETH): Gwei (9 decimals)
    // asset_id 1 (USDC): Micro-USDC (6 decimals)
    let base_gas_fee: u64 = if payload.asset_id == 0 {
        let gas_price_wei = if let Some(client) = state.evm_client.as_ref() {
            client.get_gas_price().await.unwrap_or(20_000_000)
        } else {
            20_000_000
        };
        let total_wei = (gas_limit as u128) * gas_price_wei;
        let gwei = total_wei.div_ceil(1_000_000_000) as u64;
        gwei.max(1)
    } else {
        default_gas_cost().max(20_000)
    };

    // Flat 15% relayer markup with ceiling division (DEC-031 / DEC-034C)
    let relayer_markup_fee = (base_gas_fee * 15).div_ceil(100);
    let total_execution_fee = base_gas_fee + relayer_markup_fee;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let expiry = now + QUOTE_EXPIRY_SECS;

    let execution_quote = ExecutionQuote {
        assetId: payload.asset_id,
        targetContract: contract_addr,
        baseGasFee: base_gas_fee,
        relayerMarkupFee: relayer_markup_fee,
        totalExecutionFee: total_execution_fee,
        expiry,
        chainId: U256::from(chain_id),
    };

    let quote_hash_b256 = compute_quote_hash(&execution_quote, chain_id, contract_addr);
    let quote_hash_hex = format!("0x{}", hex::encode(quote_hash_b256.0));

    // Sign with relayer private key
    let relayer_pk = get_relayer_private_key()?;
    let (v, r, s) =
        sign_quote(&execution_quote, chain_id, contract_addr, &relayer_pk).map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "status": "ERROR",
                    "message": format!("Failed to sign quote: {}", e)
                })),
            )
        })?;

    let sig_hex = format!("0x{}{}{:02x}", hex::encode(r), hex::encode(s), v);

    let quote_resp = ExecutionQuoteResponse {
        asset_id: payload.asset_id,
        base_gas_fee,
        relayer_markup_fee,
        total_execution_fee,
        quote_hash: quote_hash_hex.clone(),
        expiry,
        relayer_signature: sig_hex,
    };

    // Cache the quote in AppState to prevent cross-asset quote tampering
    state
        .quote_cache
        .lock()
        .await
        .insert(quote_hash_hex, quote_resp.clone());

    Ok(Json(quote_resp))
}

fn get_relayer_private_key() -> Result<[u8; 32], (StatusCode, Json<serde_json::Value>)> {
    if let Ok(pk_str) = std::env::var("ZELTRA_RELAYER_PRIVATE_KEY") {
        let clean = pk_str.trim().trim_start_matches("0x");
        if let Ok(bytes) = hex::decode(clean) {
            if bytes.len() == 32 {
                let mut pk = [0u8; 32];
                pk.copy_from_slice(&bytes);
                return Ok(pk);
            }
        }
    }
    // In test or non-strict dev environments, provide deterministic test key if env set
    Err((
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(serde_json::json!({
            "status": "ERROR",
            "message": "Relayer private key unavailable for EIP-712 quote signing"
        })),
    ))
}

pub async fn handle_private_spend_quote(
    State(state): State<AppState>,
    Query(query): Query<PrivateSpendQuoteRequest>,
) -> Json<PrivateSpendQuoteResponse> {
    compute_quote(&state, query).await
}

pub async fn compute_quote(
    state: &AppState,
    query: PrivateSpendQuoteRequest,
) -> Json<PrivateSpendQuoteResponse> {
    let relayer_gas_cost = query.estimated_gas_cost.unwrap_or_else(default_gas_cost);
    let relayer_markup_bps = query
        .relayer_markup_bps
        .unwrap_or_else(default_relayer_markup_bps);

    // CCIP fee breakdown: if destination_chain_selector is provided, determine CCIP network fee
    let (ccip_network_fee, destination_chain_selector) = match query.destination_chain_selector {
        Some(selector) => {
            let fee = std::env::var("ZELTRA_CCIP_NETWORK_FEE_USDC")
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

    let fee_bps = zeltra_core::PRIVATE_SPEND_FEE_BPS;
    let fee_tier = None;
    let discount_bps = None;

    let quote_opt = match ccip_network_fee {
        Some(ccip_fee) => zeltra_core::quote_cross_chain_private_spend_with_fee(
            query.merchant_amount,
            relayer_gas_cost,
            ccip_fee,
            relayer_markup_bps,
            fee_bps,
        ),
        None => zeltra_core::quote_private_spend_with_fee(
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

    let quote_id_hex = generate_quote_id();
    let quote_expiry = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
        + QUOTE_EXPIRY_SECS;

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
            let env_chain_id = std::env::var("ZELTRA_CHAIN_ID")
                .ok()
                .and_then(|s| s.parse::<u64>().ok());
            let env_contract = std::env::var("ZELTRA_CONTRACT_ADDRESS").ok();
            match (env_contract, env_chain_id) {
                (Some(addr), Some(id)) if !addr.trim().is_empty() => (addr, id),
                _ => {
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

    // Build EIP-712 ExecutionQuote struct with assetId = 1 (USDC for private-spend endpoint)
    let execution_quote = ExecutionQuote {
        assetId: 1,
        targetContract: contract_addr,
        baseGasFee: relayer_gas_cost,
        relayerMarkupFee: quote.relayer_markup,
        totalExecutionFee: quote.execution_fee,
        expiry: quote_expiry,
        chainId: U256::from(chain_id),
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
    std::env::var("ZELTRA_ESTIMATED_GAS_COST_USDC_BASE_UNITS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(0)
}

fn default_relayer_markup_bps() -> u64 {
    std::env::var("ZELTRA_RELAYER_MARKUP_BPS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(zeltra_core::DEFAULT_RELAYER_MARKUP_BPS)
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
        Some(ccip_fee) => zeltra_core::quote_execution_fee_with_pass_through(
            relayer_gas_cost,
            ccip_fee,
            relayer_markup_bps,
        )
        .unwrap_or((0, 0)),
        None => {
            zeltra_core::quote_execution_fee(relayer_gas_cost, relayer_markup_bps).unwrap_or((0, 0))
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
        fee_bps: zeltra_core::PRIVATE_SPEND_FEE_BPS,
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

    static ENV_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

    async fn test_state() -> AppState {
        // Set environment variables for test deterministic signing domain
        std::env::set_var("ZELTRA_CHAIN_ID", "421614");
        std::env::set_var(
            "ZELTRA_CONTRACT_ADDRESS",
            "0x1111111111111111111111111111111111111111",
        );
        std::env::set_var(
            "ZELTRA_RELAYER_PRIVATE_KEY",
            "0x0101010101010101010101010101010101010101010101010101010101010101",
        );
        let tmp = tempfile::TempDir::new().unwrap();
        let db_path = tmp.path().join("quote_test.db");
        let db = Database::new(&db_path).await.unwrap();
        AppState::new(db, None).await
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
        std::env::remove_var("ZELTRA_CHAIN_ID");
        std::env::remove_var("ZELTRA_CONTRACT_ADDRESS");

        let tmp = tempfile::TempDir::new().unwrap();
        let db_path = tmp.path().join("quote_test_missing_domain.db");
        let db = Database::new(&db_path).await.unwrap();
        let state = AppState::new(db, None).await;

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
        std::env::set_var("ZELTRA_CHAIN_ID", "421614");
        std::env::set_var(
            "ZELTRA_CONTRACT_ADDRESS",
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
        std::env::set_var("ZELTRA_CHAIN_ID", "421614");
        std::env::set_var(
            "ZELTRA_CONTRACT_ADDRESS",
            "0x0000000000000000000000000000000000000000",
        );

        let tmp = tempfile::TempDir::new().unwrap();
        let db_path = tmp.path().join("quote_test_invalid_contract.db");
        let db = Database::new(&db_path).await.unwrap();
        let state = AppState::new(db, None).await;

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
        std::env::set_var("ZELTRA_CHAIN_ID", "421614");
        std::env::set_var(
            "ZELTRA_CONTRACT_ADDRESS",
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

    #[tokio::test]
    async fn test_relayer_quote_dual_asset() {
        let _guard = ENV_LOCK.lock().await;
        let state = test_state().await;

        // 1. WETH quote (asset_id = 0, units: Gwei)
        let weth_req = ExecutionQuoteRequest {
            asset_id: 0,
            target_contract: "0x1111111111111111111111111111111111111111".to_string(),
            estimated_gas_limit: 500_000,
        };
        let weth_resp = create_quote(State(state.clone()), Json(weth_req))
            .await
            .expect("WETH quote");
        assert_eq!(weth_resp.0.asset_id, 0);
        assert!(weth_resp.0.base_gas_fee > 0);
        assert_eq!(
            weth_resp.0.relayer_markup_fee,
            (weth_resp.0.base_gas_fee * 15).div_ceil(100)
        );
        assert_eq!(
            weth_resp.0.total_execution_fee,
            weth_resp.0.base_gas_fee + weth_resp.0.relayer_markup_fee
        );
        assert!(weth_resp.0.quote_hash.starts_with("0x"));
        assert!(weth_resp.0.relayer_signature.starts_with("0x"));

        // 2. USDC quote (asset_id = 1, units: Micro-USDC)
        let usdc_req = ExecutionQuoteRequest {
            asset_id: 1,
            target_contract: "0x1111111111111111111111111111111111111111".to_string(),
            estimated_gas_limit: 500_000,
        };
        let usdc_resp = create_quote(State(state.clone()), Json(usdc_req))
            .await
            .expect("USDC quote");
        assert_eq!(usdc_resp.0.asset_id, 1);
        assert!(usdc_resp.0.base_gas_fee >= 20_000);
        assert_eq!(
            usdc_resp.0.relayer_markup_fee,
            (usdc_resp.0.base_gas_fee * 15).div_ceil(100)
        );
        assert_eq!(
            usdc_resp.0.total_execution_fee,
            usdc_resp.0.base_gas_fee + usdc_resp.0.relayer_markup_fee
        );
        assert!(usdc_resp.0.quote_hash.starts_with("0x"));
        assert!(usdc_resp.0.relayer_signature.starts_with("0x"));

        // Verify EIP-712 signature over WETH quote
        let contract_addr =
            Address::from_str("0x1111111111111111111111111111111111111111").unwrap();
        let signer = alloy::signers::local::PrivateKeySigner::from_bytes(
            &alloy_primitives::B256::from([1u8; 32]),
        )
        .unwrap();
        let expected_signer = signer.address();

        let sig_bytes =
            hex::decode(weth_resp.0.relayer_signature.trim_start_matches("0x")).unwrap();
        let r: [u8; 32] = sig_bytes[0..32].try_into().unwrap();
        let s: [u8; 32] = sig_bytes[32..64].try_into().unwrap();
        let v = sig_bytes[64];
        let eq = ExecutionQuote {
            assetId: 0,
            targetContract: contract_addr,
            baseGasFee: weth_resp.0.base_gas_fee,
            relayerMarkupFee: weth_resp.0.relayer_markup_fee,
            totalExecutionFee: weth_resp.0.total_execution_fee,
            expiry: weth_resp.0.expiry,
            chainId: U256::from(421614),
        };
        let valid = zeltra_sdk::eip712::verify_quote_signature(
            &eq,
            421614,
            contract_addr,
            (v, r, s),
            expected_signer,
        )
        .unwrap();
        assert!(valid, "EIP-712 signature must verify successfully");
    }

    #[tokio::test]
    async fn test_relayer_quote_expiry_strict_window() {
        let _guard = ENV_LOCK.lock().await;
        let state = test_state().await;

        let req = ExecutionQuoteRequest {
            asset_id: 1,
            target_contract: "0x1111111111111111111111111111111111111111".to_string(),
            estimated_gas_limit: 500_000,
        };
        let now_before = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let resp = create_quote(State(state), Json(req))
            .await
            .expect("quote created");
        let now_after = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Must be at most 20 seconds from creation time (strict window)
        assert!(resp.0.expiry >= now_before + 20);
        assert!(resp.0.expiry <= now_after + 20);
        assert!(resp.0.expiry - now_before <= 21);
    }
}
