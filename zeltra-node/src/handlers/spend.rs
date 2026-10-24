//! Spend handler and batch processing logic
//!
//! Security features:
//! - Slippage protection (min_payout, deadline) -- Finding #9
//! - Minimum balance checks before batch processing -- Finding #10
//! - Idempotency key support for request deduplication -- Finding #16

pub use crate::batch_processor::process_spend_batch;
use crate::validation::spend_preflight::{
    validate_joinsplit_spend_preflight, validate_private_note_spend_preflight,
};
use crate::{dto::*, state::AppState};
use axum::Json;

/// Handler for ZK Private Note Spend (DEC-025 Gate F)
///
/// POST /api/v1/spend-private-note
pub use handle_private_note_spend as spend_private_note;
pub use handle_spend_joinsplit as spend_joinsplit;

pub async fn handle_private_note_spend(
    axum::extract::State(state): axum::extract::State<AppState>,
    Json(payload): Json<PrivateNoteSpendRequest>,
) -> Json<PrivateNoteSpendResponse> {
    // 1. Idempotency check
    if let Some(ref idem_key) = payload.idempotency_key {
        match state
            .db
            .check_idempotency(idem_key, "private_note_spend")
            .await
        {
            Ok(Some(cached_json)) => {
                if let Ok(cached_resp) =
                    serde_json::from_str::<PrivateNoteSpendResponse>(&cached_json)
                {
                    println!(
                        "IDEMPOTENCY: Returning cached response for private note spend key {}...",
                        &idem_key[..8.min(idem_key.len())]
                    );
                    return Json(cached_resp);
                }
            }
            Ok(None) => {}
            Err(e) => {
                eprintln!("RELAYER WARNING: Idempotency check failed: {}", e);
            }
        }
    }

    // 2. Preflight validation (skalar, quote EIP-712 cache, binding Groth16 Arkworks)
    if let Err(rejected_response) = validate_private_note_spend_preflight(&state, &payload).await {
        if let Some(ref idem_key) = payload.idempotency_key {
            if let Ok(json) = serde_json::to_string(&rejected_response) {
                let _ = state
                    .db
                    .store_idempotency(idem_key, "private_note_spend", &json)
                    .await;
            }
        }
        return Json(rejected_response);
    }

    // 3. Enqueue into database
    let spend_req = SpendRequest::from(payload.clone());
    let queue_id = match state.db.enqueue_spend(&spend_req).await {
        Ok(Some(id)) => id,
        Ok(None) => {
            return Json(PrivateNoteSpendResponse {
                status: "REJECTED".to_string(),
                message: "Nullifier already queued, submitted, or confirmed.".to_string(),
                queue_position: 0,
                estimated_gas_usdc: None,
            });
        }
        Err(e) => {
            eprintln!("RELAYER ERROR: Failed to persist private note spend: {}", e);
            return Json(PrivateNoteSpendResponse {
                status: "ERROR".to_string(),
                message: "Failed to persist settlement request".to_string(),
                queue_position: 0,
                estimated_gas_usdc: None,
            });
        }
    };

    let position = state.db.queued_spend_count().await.unwrap_or(1);
    println!(
        "RELAYER: Received private note spend request (Queued id={})",
        queue_id
    );

    let response = PrivateNoteSpendResponse {
        status: "QUEUED".to_string(),
        message: format!(
            "Private note spend persisted with settlement id {}",
            queue_id
        ),
        queue_position: position,
        estimated_gas_usdc: None,
    };

    if let Some(ref idem_key) = payload.idempotency_key {
        if let Ok(json) = serde_json::to_string(&response) {
            let _ = state
                .db
                .store_idempotency(idem_key, "private_note_spend", &json)
                .await;
        }
    }

    Json(response)
}

/// CCIP refund endpoint.
///
/// POST /api/ccip/refund
/// Body: { "nullifier": "0x..." }
///
/// Authorizes a refund for a failed CCIP cross-chain spend by freeing the nullifier
/// so the user can re-submit the spend. Only allowed if:
/// 1. destination_status = 'failed' (confirmed by CCIP monitor)
/// 2. Refund timeout has elapsed (default 24h, prevents manual execution race)
#[derive(serde::Deserialize)]
pub struct CcipRefundRequest {
    pub nullifier: String,
}

#[derive(serde::Serialize)]
pub struct CcipRefundResponse {
    pub status: String,
    pub nullifier: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

pub async fn handle_ccip_refund(
    axum::extract::State(state): axum::extract::State<AppState>,
    Json(payload): Json<CcipRefundRequest>,
) -> Json<CcipRefundResponse> {
    let refund_timeout = crate::config::CcipDestinationConfig::from_env().refund_timeout_secs;

    match state
        .db
        .authorize_ccip_refund(&payload.nullifier, refund_timeout)
        .await
    {
        Ok(true) => {
            println!(
                "CCIP_REFUND: Authorized refund for nullifier {}",
                &payload.nullifier[..8.min(payload.nullifier.len())]
            );
            Json(CcipRefundResponse {
                status: "AUTHORIZED".to_string(),
                nullifier: payload.nullifier,
                error: None,
            })
        }
        Ok(false) => Json(CcipRefundResponse {
            status: "REJECTED".to_string(),
            nullifier: payload.nullifier,
            error: Some(
                "Refund not eligible: destination spend not failed or timeout not elapsed"
                    .to_string(),
            ),
        }),
        Err(e) => {
            eprintln!("CCIP_REFUND: Error authorizing refund: {}", e);
            Json(CcipRefundResponse {
                status: "ERROR".to_string(),
                nullifier: payload.nullifier,
                error: Some(format!("Internal error: {}", e)),
            })
        }
    }
}

pub async fn handle_spend_joinsplit(
    axum::extract::State(state): axum::extract::State<AppState>,
    Json(payload): Json<JoinSplitSpendRequest>,
) -> Json<JoinSplitSpendResponse> {
    // 1. Idempotency check
    if let Some(ref idem_key) = payload.idempotency_key {
        match state
            .db
            .check_idempotency(idem_key, "joinsplit_spend")
            .await
        {
            Ok(Some(cached_json)) => {
                if let Ok(cached_resp) =
                    serde_json::from_str::<JoinSplitSpendResponse>(&cached_json)
                {
                    println!(
                        "IDEMPOTENCY: Returning cached response for JoinSplit spend key {}...",
                        &idem_key[..8.min(idem_key.len())]
                    );
                    return Json(cached_resp);
                }
            }
            Ok(None) => {}
            Err(e) => {
                eprintln!("RELAYER WARNING: Idempotency check failed: {}", e);
            }
        }
    }

    // 2. Preflight validation (skalar kanonikal, strict 3-mode, memo 192 hex, Groth16 verify)
    let preflight = match validate_joinsplit_spend_preflight(&state, &payload).await {
        Ok(data) => data,
        Err(err_resp) => {
            if let Some(ref idem) = payload.idempotency_key {
                let _ = state
                    .db
                    .store_idempotency(
                        idem,
                        "joinsplit_spend",
                        &serde_json::to_string(&err_resp).unwrap_or_default(),
                    )
                    .await;
            }
            return Json(err_resp);
        }
    };

    // 3. On-Chain Simulation Verification (Phase 2: C2.2)
    if let Some(ref client) = state.evm_client {
        let is_spent_1 = client
            .is_nullifier_spent(&payload.input_nullifier_1)
            .await
            .unwrap_or(false);
        let is_spent_2 = client
            .is_nullifier_spent(&payload.input_nullifier_2)
            .await
            .unwrap_or(false);
        if is_spent_1 || is_spent_2 {
            let _ = state
                .db
                .release_nullifiers(&payload.input_nullifier_1, &payload.input_nullifier_2)
                .await;
            let resp = JoinSplitSpendResponse {
                status: "REJECTED".to_string(),
                message: "Nullifier already spent on-chain".to_string(),
                tx_hash: None,
                queue_position: 0,
                estimated_gas_usdc: None,
            };
            if let Some(ref idem) = payload.idempotency_key {
                let _ = state
                    .db
                    .store_idempotency(
                        idem,
                        "joinsplit_spend",
                        &serde_json::to_string(&resp).unwrap_or_default(),
                    )
                    .await;
            }
            return Json(resp);
        }
    }

    // 4. Broadcast transaction to chain or accept in pool
    if let Some(ref client) = state.evm_client {
        let epoch_1 = payload.epoch_id_1.unwrap_or_else(|| {
            alloy_primitives::U256::from_be_bytes(preflight.parsed_inputs[4]).to::<u32>()
        });
        let epoch_2 = payload.epoch_id_2.unwrap_or_else(|| {
            alloy_primitives::U256::from_be_bytes(preflight.parsed_inputs[5]).to::<u32>()
        });
        let quote_hash_hex = payload.quote_hash.as_deref().unwrap_or("0x00");

        match client
            .broadcast_spend_joinsplit_transaction(
                &payload.proof_a_neg,
                &payload.proof_b,
                &payload.proof_c,
                &payload.note_root,
                preflight.leaf_count_u64,
                &payload.input_nullifier_1,
                &payload.input_nullifier_2,
                epoch_1,
                epoch_2,
                &payload.output_commitment_1,
                &payload.output_commitment_2,
                &payload.recipient,
                payload.merchant_amount,
                payload.protocol_fee,
                payload.execution_fee,
                payload.max_execution_fee,
                quote_hash_hex,
                payload.expiry.unwrap_or(0),
                preflight.flags_packed,
                preflight.is_rollover_u64,
                &payload.encrypted_memo_1,
                &payload.encrypted_memo_2,
            )
            .await
        {
            Ok(outcome) => {
                let resp = JoinSplitSpendResponse {
                    status: "CONFIRMED".to_string(),
                    message: format!(
                        "JoinSplit transaction confirmed in block {}",
                        outcome.block_number
                    ),
                    tx_hash: Some(outcome.tx_hash),
                    queue_position: 0,
                    estimated_gas_usdc: None,
                };
                if let Some(ref idem) = payload.idempotency_key {
                    let _ = state
                        .db
                        .store_idempotency(
                            idem,
                            "joinsplit_spend",
                            &serde_json::to_string(&resp).unwrap_or_default(),
                        )
                        .await;
                }
                Json(resp)
            }
            Err(e) => {
                let _ = state
                    .db
                    .release_nullifiers(&payload.input_nullifier_1, &payload.input_nullifier_2)
                    .await;
                Json(JoinSplitSpendResponse {
                    status: "ERROR".to_string(),
                    message: format!("Broadcast transaction failed: {}", e),
                    tx_hash: None,
                    queue_position: 0,
                    estimated_gas_usdc: None,
                })
            }
        }
    } else {
        let resp = JoinSplitSpendResponse {
            status: "ACCEPTED".to_string(),
            message: "JoinSplit transaction accepted and verified by relayer preflight".to_string(),
            tx_hash: Some(format!(
                "0xmockjoinsplit_{}",
                hex::encode(&preflight.parsed_inputs[2][0..8])
            )),
            queue_position: 1,
            estimated_gas_usdc: None,
        };
        if let Some(ref idem) = payload.idempotency_key {
            let _ = state
                .db
                .store_idempotency(
                    idem,
                    "joinsplit_spend",
                    &serde_json::to_string(&resp).unwrap_or_default(),
                )
                .await;
        }
        Json(resp)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::extract::State;
    use tempfile::TempDir;
    use zeltra_core::Fr;

    async fn setup_test_state() -> (AppState, TempDir) {
        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("spend_test.db");
        let db = crate::database::Database::new(&db_path).await.unwrap();

        let state = AppState::new(db, None).await;

        (state, tmp)
    }

    fn sample_valid_private_note_request() -> PrivateNoteSpendRequest {
        use zeltra_core::{
            create_dummy_circuit, extract_public_inputs, fr_to_be_bytes, generate_note_proof,
            get_or_init_note_circuit_keys, to_evm_g1, to_evm_g2,
        };

        static CACHED_REQ: std::sync::OnceLock<PrivateNoteSpendRequest> =
            std::sync::OnceLock::new();
        CACHED_REQ
            .get_or_init(|| {
                let keys = get_or_init_note_circuit_keys();
                let circuit = create_dummy_circuit();
                let pis = extract_public_inputs(&circuit);
                let proof = generate_note_proof(circuit, &keys.proving_key).expect("prove");

                let mut a_neg = [0u8; 128];
                a_neg.copy_from_slice(&to_evm_g1(&-proof.a));
                let mut b = [0u8; 256];
                b.copy_from_slice(&to_evm_g2(&proof.b));
                let mut c = [0u8; 128];
                c.copy_from_slice(&to_evm_g1(&proof.c));

                let note_root = format!("0x{}", hex::encode(fr_to_be_bytes(&pis[0])));
                let leaf_count = 1u64;
                let input_nullifier = format!("0x{}", hex::encode(fr_to_be_bytes(&pis[2])));
                let output_commitment = format!("0x{}", hex::encode(fr_to_be_bytes(&pis[4])));
                let recipient = "0x0000000000000000000000000000000000000064".to_string();

                let public_inputs: Vec<String> = pis
                    .iter()
                    .map(|fr| format!("0x{}", hex::encode(fr_to_be_bytes(fr))))
                    .collect();

                PrivateNoteSpendRequest {
                    session_id: Some("session_1".to_string()),
                    note_root,
                    leaf_count: Some(leaf_count),
                    note_epoch_id: Some(0),
                    is_rollover: Some(0),
                    input_value: Some(10_000_000),
                    asset_id: Some(1),
                    input_nullifier,
                    output_commitment: Some(output_commitment),
                    recipient,
                    merchant_amount: 5_000_000,
                    protocol_fee: 12_500,
                    execution_fee: 23_000,
                    max_execution_fee: 25_000,
                    quote_hash: Some(format!(
                        "0x{}",
                        hex::encode(zeltra_core::combine_limbs_to_quote_hash(&pis[9], &pis[10]))
                    )),
                    quote_signature: None,
                    quote_id: None,
                    user_address: None,
                    expiry: Some(0),
                    has_change: Some(serde_json::Value::Number(1.into())),
                    proof_a_neg: format!("0x{}", hex::encode(a_neg)),
                    proof_b: format!("0x{}", hex::encode(b)),
                    proof_c: format!("0x{}", hex::encode(c)),
                    public_inputs,
                    idempotency_key: Some("idem_valid_1".to_string()),
                    change_encrypted_memo: None,
                }
            })
            .clone()
    }

    #[tokio::test]
    async fn test_handle_private_note_spend_success() {
        let (state, _tmp) = setup_test_state().await;
        let req = sample_valid_private_note_request();

        let resp = handle_private_note_spend(State(state.clone()), Json(req.clone())).await;
        assert_eq!(resp.0.status, "QUEUED");
        assert!(resp.0.message.contains("settlement id"));

        // Test idempotency returns cached response
        let resp_cached = handle_private_note_spend(State(state.clone()), Json(req)).await;
        assert_eq!(resp_cached.0.status, "QUEUED");
    }

    #[tokio::test]
    async fn test_handle_private_note_spend_reject_non_canonical_public_input() {
        let (state, _tmp) = setup_test_state().await;
        let mut req = sample_valid_private_note_request();
        // Replace public input at index 0 with field overflow >= r
        req.public_inputs[0] =
            "0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff".to_string();

        let resp = handle_private_note_spend(State(state), Json(req)).await;
        assert_eq!(resp.0.status, "REJECTED");
        assert!(resp.0.message.contains("non-canonical scalar"));
    }

    #[tokio::test]
    async fn test_handle_private_note_spend_reject_semantic_mismatch() {
        let (state, _tmp) = setup_test_state().await;
        let mut req = sample_valid_private_note_request();
        // Tamper with merchant_amount in payload (5M vs 6M in public input)
        req.merchant_amount = 6_000_000;

        let resp = handle_private_note_spend(State(state), Json(req)).await;
        assert_eq!(resp.0.status, "REJECTED");
        assert!(resp.0.message.contains("does not match merchant_amount"));
    }

    #[tokio::test]
    async fn test_handle_private_note_spend_reject_execution_fee_exceeds_max() {
        let (state, _tmp) = setup_test_state().await;
        let mut req = sample_valid_private_note_request();
        req.execution_fee = 30_000;
        req.max_execution_fee = 25_000;

        let resp = handle_private_note_spend(State(state), Json(req)).await;
        assert_eq!(resp.0.status, "REJECTED");
        assert!(resp.0.message.contains("exceeds max execution fee"));
    }

    #[tokio::test]
    async fn test_handle_private_note_spend_reject_nonzero_commitment_with_zero_change() {
        let (state, _tmp) = setup_test_state().await;
        let mut req = sample_valid_private_note_request();
        req.has_change = Some(serde_json::Value::Number(0.into()));
        req.output_commitment =
            Some("0x0000000000000000000000000000000000000000000000000000000000000001".to_string());

        let resp = handle_private_note_spend(State(state), Json(req)).await;
        assert_eq!(resp.0.status, "REJECTED");
        assert!(resp.0.message.contains("nonzero output_commitment"));
    }

    #[tokio::test]
    async fn test_handle_private_note_spend_reject_double_spend() {
        let (state, _tmp) = setup_test_state().await;
        let req = sample_valid_private_note_request();

        // Mark nullifier as spent in DB first
        state
            .db
            .check_and_insert_nullifier(&req.input_nullifier, Some("0xtx"))
            .await
            .unwrap();

        let resp = handle_private_note_spend(State(state), Json(req)).await;
        assert_eq!(resp.0.status, "REJECTED");
        assert!(resp.0.message.contains("Double-spending detected"));
    }

    #[tokio::test]
    async fn test_handle_private_note_spend_reject_invalid_proof() {
        let (state, _tmp) = setup_test_state().await;
        let mut req = sample_valid_private_note_request();
        // Replace with fake proof (DEC-026 C-01 griefing attempt)
        req.proof_a_neg = format!("0x{}", "11".repeat(128));

        let resp = handle_private_note_spend(State(state), Json(req)).await;
        assert_eq!(resp.0.status, "REJECTED");
        assert!(resp.0.message.contains("Invalid Groth16 proof"));
    }

    #[tokio::test]
    async fn test_handle_private_note_spend_reject_sanctioned_recipient() {
        let (state, _tmp) = setup_test_state().await;
        let mut req = sample_valid_private_note_request();
        // Tornado Cash router address
        req.recipient = crate::validation::sanctioned_evm_addresses()
            .iter()
            .next()
            .unwrap()
            .to_string();

        let resp = handle_private_note_spend(State(state), Json(req)).await;
        assert_eq!(resp.0.status, "REJECTED");
        assert!(resp
            .0
            .message
            .contains("restricted under public sanctions policy"));
    }

    #[tokio::test]
    async fn test_handle_private_note_spend_reject_empty_public_inputs() {
        let (state, _tmp) = setup_test_state().await;
        let mut req = sample_valid_private_note_request();
        // DEC-035A: empty public inputs must be rejected immediately, not bypass preflight
        req.public_inputs = vec![];

        let resp = handle_private_note_spend(State(state), Json(req)).await;
        assert_eq!(resp.0.status, "REJECTED");
        assert!(resp.0.message.contains("Expected 16 public inputs"));
    }

    #[tokio::test]
    async fn test_handle_private_note_spend_reject_wrong_public_inputs_count() {
        let (state, _tmp) = setup_test_state().await;
        let mut req = sample_valid_private_note_request();
        req.public_inputs.truncate(15);

        let resp = handle_private_note_spend(State(state), Json(req)).await;
        assert_eq!(resp.0.status, "REJECTED");
        assert!(resp
            .0
            .message
            .contains("Expected 16 public inputs for PrivateNoteCircuit, got 15"));
    }

    #[tokio::test]
    async fn test_handle_private_note_spend_reject_leaf_count_zero() {
        let (state, _tmp) = setup_test_state().await;
        let mut req = sample_valid_private_note_request();
        req.public_inputs[1] = format!("0x{:064x}", 0);

        let resp = handle_private_note_spend(State(state), Json(req)).await;
        assert_eq!(resp.0.status, "REJECTED");
        assert!(resp
            .0
            .message
            .contains("leaf_count in public_inputs[1] must be greater than zero"));
    }

    #[tokio::test]
    async fn test_handle_private_note_spend_reject_leaf_count_mismatch() {
        let (state, _tmp) = setup_test_state().await;
        let mut req = sample_valid_private_note_request();
        req.leaf_count = Some(999);

        let resp = handle_private_note_spend(State(state), Json(req)).await;
        assert_eq!(resp.0.status, "REJECTED");
        assert!(resp
            .0
            .message
            .contains("public_inputs[1] does not match leaf_count"));
    }

    #[tokio::test]
    async fn test_handle_private_note_spend_reject_domain_mismatch_chain_id() {
        let (state, _tmp) = setup_test_state().await;
        let mut req = sample_valid_private_note_request();
        // DEC-035A: chain_id is at index 11
        req.public_inputs[11] = format!("0x{:064x}", 1);

        let resp = handle_private_note_spend(State(state), Json(req)).await;
        assert_eq!(resp.0.status, "REJECTED");
        assert!(resp
            .0
            .message
            .contains("public_inputs[11] does not match target chain_id"));
    }

    #[tokio::test]
    async fn test_handle_private_note_spend_reject_domain_mismatch_contract_address() {
        let (state, _tmp) = setup_test_state().await;
        let mut req = sample_valid_private_note_request();
        // DEC-035A: contract_address is at index 12
        std::env::set_var(
            "ZELTRA_CONTRACT_ADDRESS",
            "0x000000000000000000000000000000000000012c",
        );
        req.public_inputs[12] = format!("0x{:064x}", 999);

        let resp = handle_private_note_spend(State(state), Json(req)).await;
        std::env::remove_var("ZELTRA_CONTRACT_ADDRESS");
        assert_eq!(resp.0.status, "REJECTED");
        assert!(resp
            .0
            .message
            .contains("public_inputs[12] does not match target contract_address"));
    }

    #[tokio::test]
    async fn test_handle_private_note_spend_reject_quote_hash_hi_out_of_range() {
        let (state, _tmp) = setup_test_state().await;
        let mut req = sample_valid_private_note_request();
        // DEC-035A: quote_hash_hi at index 9 >= 2^128 (high 16 bytes nonzero)
        req.public_inputs[9] = format!("0x{:032x}{:032x}", 1u64, 0u64);

        let resp = handle_private_note_spend(State(state), Json(req)).await;
        assert_eq!(resp.0.status, "REJECTED");
        assert!(resp
            .0
            .message
            .contains("public_inputs[9] (quote_hash_hi) exceeds 128-bit range"));
    }

    #[tokio::test]
    async fn test_handle_private_note_spend_reject_quote_hash_lo_out_of_range() {
        let (state, _tmp) = setup_test_state().await;
        let mut req = sample_valid_private_note_request();
        // DEC-035A: quote_hash_lo at index 10 >= 2^128 (high 16 bytes nonzero)
        req.public_inputs[10] = format!("0x{:032x}{:032x}", 1u64, 0u64);

        let resp = handle_private_note_spend(State(state), Json(req)).await;
        assert_eq!(resp.0.status, "REJECTED");
        assert!(resp
            .0
            .message
            .contains("public_inputs[10] (quote_hash_lo) exceeds 128-bit range"));
    }

    #[tokio::test]
    async fn test_handle_private_note_spend_reject_quote_hash_hi_mismatch() {
        let (state, _tmp) = setup_test_state().await;
        let mut req = sample_valid_private_note_request();
        // Tamper with quote_hash_hi at index 9
        req.public_inputs[9] = format!("0x{:064x}", 99999u64);

        let resp = handle_private_note_spend(State(state), Json(req)).await;
        assert_eq!(resp.0.status, "REJECTED");
        assert!(resp
            .0
            .message
            .contains("public_inputs[9] does not match quote_hash_hi"));
    }

    #[tokio::test]
    async fn test_handle_private_note_spend_reject_quote_hash_lo_mismatch() {
        let (state, _tmp) = setup_test_state().await;
        let mut req = sample_valid_private_note_request();
        // Tamper with quote_hash_lo at index 10
        req.public_inputs[10] = format!("0x{:064x}", 99999u64);

        let resp = handle_private_note_spend(State(state), Json(req)).await;
        assert_eq!(resp.0.status, "REJECTED");
        assert!(resp
            .0
            .message
            .contains("public_inputs[10] does not match quote_hash_lo"));
    }

    #[tokio::test]
    async fn test_handle_private_note_spend_reject_dust_rollover() {
        let (state, _tmp) = setup_test_state().await;
        let mut req = sample_valid_private_note_request();
        req.idempotency_key = Some("idem_dust_rollover".to_string());
        // DEC-033: Standalone rollover with dust input value (< $2.00 USDC)
        req.is_rollover = Some(1);
        req.public_inputs[15] = format!("0x{:064x}", 3);
        req.merchant_amount = 0;
        req.public_inputs[6] = format!("0x{:064x}", 0);
        req.protocol_fee = 0;
        req.public_inputs[7] = format!("0x{:064x}", 0);
        req.input_value = Some(1_500_000); // 1.50 USDC < 2.00 USDC

        let resp = handle_private_note_spend(State(state), Json(req)).await;
        assert_eq!(resp.0.status, "REJECTED");
        assert!(resp.0.message.contains("DUST_NOTE_ROLLOVER_REJECTED"));
    }

    #[tokio::test]
    async fn test_handle_private_note_spend_reject_rollover_with_merchant_amount() {
        let (state, _tmp) = setup_test_state().await;
        let mut req = sample_valid_private_note_request();
        req.idempotency_key = Some("idem_rollover_merchant".to_string());
        // DEC-033: Standalone rollover must have merchant_amount == 0
        req.is_rollover = Some(1);
        req.public_inputs[15] = format!("0x{:064x}", 3);
        req.merchant_amount = 5_000_000;
        req.public_inputs[6] = format!("0x{:064x}", 5_000_000);

        let resp = handle_private_note_spend(State(state), Json(req)).await;
        assert_eq!(resp.0.status, "REJECTED");
        assert!(resp
            .0
            .message
            .contains("merchant_amount must be 0 for standalone rollover"));
    }

    #[tokio::test]
    async fn test_handle_private_note_spend_reject_future_epoch() {
        let (state, _tmp) = setup_test_state().await;
        let mut req = sample_valid_private_note_request();
        req.idempotency_key = Some("idem_future_epoch".to_string());
        // DEC-033: Note epoch in the future
        req.note_epoch_id = Some(10);
        req.public_inputs[3] = format!("0x{:064x}", 10);

        let resp = handle_private_note_spend(State(state), Json(req)).await;
        assert_eq!(resp.0.status, "REJECTED");
        assert!(resp.0.message.contains("EPOCH_IN_FUTURE"));
    }

    #[tokio::test]
    async fn test_handle_private_note_spend_reject_insufficient_balance_for_gas() {
        let (state, _tmp) = setup_test_state().await;
        let mut req = sample_valid_private_note_request();
        req.idempotency_key = Some("idem_insufficient_gas".to_string());
        // DEC-033: Standalone rollover where input_value < execution_fee
        req.is_rollover = Some(1);
        req.public_inputs[15] = format!("0x{:064x}", 3);
        req.merchant_amount = 0;
        req.public_inputs[6] = format!("0x{:064x}", 0);
        req.protocol_fee = 0;
        req.public_inputs[7] = format!("0x{:064x}", 0);
        req.execution_fee = 2_500_000;
        req.max_execution_fee = 3_000_000;
        req.public_inputs[8] = format!("0x{:064x}", 2_500_000);
        req.input_value = Some(2_000_000); // 2.00 USDC < 2.50 USDC fee

        let resp = handle_private_note_spend(State(state), Json(req)).await;
        assert_eq!(resp.0.status, "REJECTED");
        assert!(resp
            .0
            .message
            .contains("insufficient to pay relayer execution fee"));
    }

    #[tokio::test]
    async fn test_handle_private_note_spend_reject_mismatched_signed_quote_hash() {
        // B6.4: Submit valid proof for Quote A with signature for Quote B -> MUST return 400 MismatchedQuoteHash
        use alloy_primitives::{Address, U256};
        use zeltra_sdk::eip712::{sign_quote, ExecutionQuote};

        let (state, _tmp) = setup_test_state().await;
        let mut req = sample_valid_private_note_request();
        req.idempotency_key = Some("idem_quote_mismatch".to_string());

        // Create a valid signed ExecutionQuote for Quote B (different quoteId, maxExecutionFee, etc.)
        let user_pk: [u8; 32] = [42u8; 32];
        let signer = alloy::signers::local::PrivateKeySigner::from_bytes(
            &alloy_primitives::B256::from(user_pk),
        )
        .unwrap();
        let user_address = format!("{:?}", signer.address());
        let _relayer_addr =
            std::str::FromStr::from_str(&state.relayer_address).unwrap_or(Address::ZERO);

        let chain_id = 421614u64;
        let contract_addr =
            std::str::FromStr::from_str("0x000000000000000000000000000000000000012c").unwrap();

        let quote_b = ExecutionQuote {
            assetId: 1,
            targetContract: contract_addr,
            baseGasFee: 43478,
            relayerMarkupFee: 6522,
            totalExecutionFee: 50_000,
            expiry: 0,
            chainId: U256::from(chain_id),
        };
        let (v, r, s) =
            sign_quote(&quote_b, chain_id, contract_addr, &user_pk).expect("sign quote B");
        let mut sig_bytes = Vec::with_capacity(65);
        sig_bytes.extend_from_slice(&r);
        sig_bytes.extend_from_slice(&s);
        sig_bytes.push(v);
        let sig_hex = format!("0x{}", hex::encode(sig_bytes));

        // Submit req with valid signature for Quote B, but whose proof & public_inputs are bound to Quote A
        req.quote_id = Some("999999".to_string());
        req.quote_signature = Some(sig_hex);
        req.user_address = Some(user_address.to_string());
        req.max_execution_fee = 50_000;

        let resp = handle_private_note_spend(State(state.clone()), Json(req.clone())).await;
        assert_eq!(resp.0.status, "REJECTED");
        assert!(
            resp.0.message.contains("MismatchedQuoteHash"),
            "Expected MismatchedQuoteHash error, got: {}",
            resp.0.message
        );

        // Prove database state remains unchanged: nullifier is not spent
        let is_spent = state
            .db
            .is_nullifier_spent(&req.input_nullifier)
            .await
            .unwrap();
        assert!(
            !is_spent,
            "Negative test proof: nullifier MUST NOT be marked spent upon rejection!"
        );
    }

    fn create_test_joinsplit_circuit_with_expiry(
        exp_val: u64,
    ) -> zeltra_core::joinsplit_circuit::JoinSplitCircuit {
        use zeltra_core::{
            compute_binding_commitment, compute_scope_hash, derive_joinsplit_dummy_nullifier,
            derive_joinsplit_nullifier, derive_nullifier_key, note_commitment, MerkleMountainRange,
        };

        let sk = Fr::from(100u64);
        let nk = derive_nullifier_key(sk);
        let rho1 = Fr::from(200u64);
        let rand1 = Fr::from(300u64);
        let v_in1 = 10_000_000u64; // 10 USDC

        let cm1 = note_commitment(v_in1, 1, sk, rho1, rand1);

        let mut mmr = MerkleMountainRange::new();
        let (leaf_idx1, root) = mmr.append(cm1);
        let proof1 = mmr.generate_proof(leaf_idx1);

        let mut in1_sibs = [Fr::from(0u64); 32];
        for (i, sib) in proof1.mountain_siblings.iter().enumerate() {
            in1_sibs[i] = *sib;
        }

        let mut peaks = [Fr::from(0u64); 32];
        let all_peaks = mmr.get_peaks();
        for (i, p) in all_peaks.iter().enumerate() {
            peaks[i] = *p;
        }

        let epoch1 = 10u64;
        let nf1 = derive_joinsplit_nullifier(nk, rho1, epoch1);

        // DEC-046: Dummy second input uses canonical dummy constants
        let nonce = Fr::from(999u64);
        let owner_dummy = zeltra_core::circuit_gadgets::CANONICAL_DUMMY_OWNER;
        let rho_dummy = zeltra_core::circuit_gadgets::CANONICAL_DUMMY_RHO;
        let rand_dummy = zeltra_core::circuit_gadgets::CANONICAL_DUMMY_RANDOMNESS;
        let dummy_nk = derive_nullifier_key(owner_dummy);
        let nf2 = derive_joinsplit_dummy_nullifier(dummy_nk, nonce);

        // DEC-046: Canonical dummy output commitments (v=0, owner=0, rho=0, rand=0)
        let cm_out1 = note_commitment(0, 1, owner_dummy, rho_dummy, rand_dummy);
        let cm_out2 = note_commitment(0, 1, owner_dummy, rho_dummy, rand_dummy);

        let merch_amt = 9_900_000u64;
        let proto_fee = (merch_amt * 45).div_ceil(10_000); // 44_550
        let exec_fee = v_in1 - merch_amt - proto_fee; // 55_450

        let rec = Fr::from(0x1234u64);
        let cid = Fr::from(421614u64);
        let caddr = Fr::from(0x5678u64);
        let exp = Fr::from(exp_val);
        let q_hi = Fr::from(0x1111u64);
        let q_lo = Fr::from(0x2222u64);

        let scope_hash = compute_scope_hash(rec, cid, caddr, exp);
        let binding = compute_binding_commitment(q_hi, q_lo, scope_hash);

        zeltra_core::joinsplit_circuit::JoinSplitCircuit {
            note_root: Some(root),
            leaf_count: Some(Fr::from(mmr.leaf_count())),
            input_nullifier_1: Some(nf1),
            input_nullifier_2: Some(nf2),
            epoch_id_1: Some(Fr::from(epoch1)),
            epoch_id_2: Some(Fr::from(epoch1)),
            output_commitment_1: Some(cm_out1),
            output_commitment_2: Some(cm_out2),
            recipient: Some(rec),
            merchant_amount: Some(Fr::from(merch_amt)),
            protocol_fee: Some(Fr::from(proto_fee)),
            execution_fee: Some(Fr::from(exec_fee)),
            quote_hash_hi: Some(q_hi),
            quote_hash_lo: Some(q_lo),
            chain_id: Some(cid),
            contract_address: Some(caddr),
            expiry: Some(exp),
            flags_packed: Some(Fr::from(4u64)),
            is_rollover: Some(Fr::from(0u64)),

            in1_value: Some(Fr::from(v_in1)),
            in1_asset_id: Some(Fr::from(1u64)),
            in1_owner_key: Some(sk),
            in1_rho: Some(rho1),
            in1_randomness: Some(rand1),
            in1_leaf_index: Some(Fr::from(leaf_idx1 as u64)),
            in1_mountain_height: Some(proof1.mountain_height as u8),
            in1_mountain_siblings: Some(in1_sibs),

            in2_value: Some(Fr::from(0u64)),
            in2_asset_id: Some(Fr::from(1u64)),
            in2_owner_key: Some(owner_dummy),
            in2_rho: Some(rho_dummy),
            in2_randomness: Some(rand_dummy),
            in2_leaf_index: Some(Fr::from(
                zeltra_core::circuit_gadgets::CANONICAL_DUMMY_LEAF_INDEX,
            )),
            in2_mountain_height: Some(0),
            in2_mountain_siblings: Some([Fr::from(0u64); 32]),
            in2_is_dummy: Some(Fr::from(1u64)),
            in2_session_nonce: Some(nonce),

            mmr_peaks: Some(peaks),

            out1_value: Some(Fr::from(0u64)),
            out1_asset_id: Some(Fr::from(1u64)),
            out1_owner_key: Some(owner_dummy),
            out1_rho: Some(rho_dummy),
            out1_randomness: Some(rand_dummy),

            out2_value: Some(Fr::from(0u64)),
            out2_asset_id: Some(Fr::from(1u64)),
            out2_owner_key: Some(owner_dummy),
            out2_rho: Some(rho_dummy),
            out2_randomness: Some(rand_dummy),

            has_change_1: Some(Fr::from(0u64)),
            has_change_2: Some(Fr::from(0u64)),
            public_asset_id: Some(Fr::from(1u64)),
            expected_binding: Some(binding),
            perm_bit: Some(Fr::from(0u64)),
            is_decoy: Some(Fr::from(0u64)),
        }
    }

    fn sample_valid_joinsplit_request() -> JoinSplitSpendRequest {
        use zeltra_core::joinsplit_circuit::{
            extract_joinsplit_public_inputs, generate_joinsplit_proof,
            get_or_init_joinsplit_circuit_keys,
        };
        use zeltra_core::{fr_to_be_bytes, to_evm_g1, to_evm_g2};

        static CACHED_JS_REQ: std::sync::OnceLock<JoinSplitSpendRequest> =
            std::sync::OnceLock::new();
        CACHED_JS_REQ
            .get_or_init(|| {
                let keys = get_or_init_joinsplit_circuit_keys();
                let circuit = create_test_joinsplit_circuit_with_expiry(0);
                let pis = extract_joinsplit_public_inputs(&circuit);
                let proof = generate_joinsplit_proof(circuit, &keys.proving_key).expect("prove");

                let mut a_neg = [0u8; 128];
                a_neg.copy_from_slice(&to_evm_g1(&-proof.a));
                let mut b = [0u8; 256];
                b.copy_from_slice(&to_evm_g2(&proof.b));
                let mut c = [0u8; 128];
                c.copy_from_slice(&to_evm_g1(&proof.c));

                let note_root = format!("0x{}", hex::encode(fr_to_be_bytes(&pis[0])));
                let leaf_count = 1u64;
                let input_nullifier_1 = format!("0x{}", hex::encode(fr_to_be_bytes(&pis[2])));
                let input_nullifier_2 = format!("0x{}", hex::encode(fr_to_be_bytes(&pis[3])));
                let epoch_id_1 = 10u32;
                let epoch_id_2 = 10u32;
                let output_commitment_1 = format!("0x{}", hex::encode(fr_to_be_bytes(&pis[6])));
                let output_commitment_2 = format!("0x{}", hex::encode(fr_to_be_bytes(&pis[7])));
                let recipient = "0x0000000000000000000000000000000000001234".to_string();
                let merchant_amount = 9_900_000u64;
                let protocol_fee = 44_550u64;
                let execution_fee = 55_450u64;
                let max_execution_fee = 60_000u64;
                let quote_hash = format!(
                    "0x{}",
                    hex::encode(zeltra_core::combine_limbs_to_quote_hash(&pis[12], &pis[13]))
                );
                let chain_id = 421614u64;
                let contract_address = "0x0000000000000000000000000000000000005678".to_string();
                let expiry = 0u64;
                let flags_packed = 4u8;
                let is_rollover = 0u64;

                let public_inputs: Vec<String> = pis
                    .iter()
                    .map(|fr| format!("0x{}", hex::encode(fr_to_be_bytes(fr))))
                    .collect();

                JoinSplitSpendRequest {
                    session_id: None,
                    input_commitment_1: None,
                    input_commitment_2: None,
                    note_root,
                    leaf_count: Some(leaf_count),
                    input_nullifier_1,
                    input_nullifier_2,
                    epoch_id_1: Some(epoch_id_1),
                    epoch_id_2: Some(epoch_id_2),
                    asset_id: Some(1),
                    output_commitment_1,
                    output_commitment_2,
                    recipient,
                    merchant_amount,
                    protocol_fee,
                    execution_fee,
                    max_execution_fee,
                    quote_hash: Some(quote_hash),
                    quote_signature: None,
                    quote_id: None,
                    user_address: None,
                    chain_id: Some(chain_id),
                    contract_address: Some(contract_address),
                    expiry: Some(expiry),
                    has_change_1: Some(serde_json::Value::Number(0.into())),
                    has_change_2: Some(serde_json::Value::Number(0.into())),
                    flags_packed: Some(flags_packed),
                    is_rollover: Some(is_rollover),
                    proof_a_neg: format!("0x{}", hex::encode(a_neg)),
                    proof_b: format!("0x{}", hex::encode(b)),
                    proof_c: format!("0x{}", hex::encode(c)),
                    public_inputs,
                    idempotency_key: Some("idem_js_valid_1".to_string()),
                    encrypted_memo_1: "00".repeat(96),
                    encrypted_memo_2: "00".repeat(96),
                }
            })
            .clone()
    }

    #[tokio::test]
    async fn test_handle_spend_joinsplit_success() {
        let (state, _tmp) = setup_test_state().await;
        let req = sample_valid_joinsplit_request();

        let resp = handle_spend_joinsplit(State(state.clone()), Json(req.clone())).await;
        assert_eq!(resp.0.status, "ACCEPTED");
        assert!(resp.0.message.contains("JoinSplit transaction accepted"));

        // Verify nullifiers are reserved in DB
        let is_spent_1 = state
            .db
            .is_nullifier_spent(&req.input_nullifier_1)
            .await
            .unwrap();
        let is_spent_2 = state
            .db
            .is_nullifier_spent(&req.input_nullifier_2)
            .await
            .unwrap();
        assert!(is_spent_1, "input_nullifier_1 must be marked spent");
        assert!(is_spent_2, "input_nullifier_2 must be marked spent");

        // Verify idempotency cache return
        let resp_cached = handle_spend_joinsplit(State(state), Json(req)).await;
        assert_eq!(resp_cached.0.status, "ACCEPTED");
    }

    #[tokio::test]
    async fn test_handle_spend_joinsplit_aliasing_rejected() {
        // Singularity Defense (Zellic 2024): input_nullifier_1 == input_nullifier_2 must be rejected
        let (state, _tmp) = setup_test_state().await;
        let mut req = sample_valid_joinsplit_request();
        req.input_nullifier_2 = req.input_nullifier_1.clone();

        let resp = handle_spend_joinsplit(State(state.clone()), Json(req.clone())).await;
        assert_eq!(resp.0.status, "REJECTED");
        assert!(resp.0.message.contains("DuplicateNullifierInSameTx"));

        // Verify neither nullifier is spent
        let is_spent = state
            .db
            .is_nullifier_spent(&req.input_nullifier_1)
            .await
            .unwrap();
        assert!(
            !is_spent,
            "Nullifier must not be marked spent on aliasing failure"
        );
    }

    #[tokio::test]
    async fn test_handle_spend_joinsplit_invalid_proof_preflight_rejected() {
        // Gas griefing defense against EIP-2537: valid curve points that fail Groth16 pairing
        let (state, _tmp) = setup_test_state().await;
        let mut req = sample_valid_joinsplit_request();
        // Replace proof_c with proof_a_neg: valid G1 point on curve, but invalidates pairing equation
        req.proof_c = req.proof_a_neg.clone();

        let resp = handle_spend_joinsplit(State(state.clone()), Json(req.clone())).await;
        assert_eq!(resp.0.status, "REJECTED");
        assert!(resp
            .0
            .message
            .contains("Invalid Groth16 proof: preflight verification failed"));

        // Verify nullifiers are NOT reserved
        let is_spent_1 = state
            .db
            .is_nullifier_spent(&req.input_nullifier_1)
            .await
            .unwrap();
        let is_spent_2 = state
            .db
            .is_nullifier_spent(&req.input_nullifier_2)
            .await
            .unwrap();
        assert!(
            !is_spent_1,
            "nullifier 1 must not be spent on invalid proof"
        );
        assert!(
            !is_spent_2,
            "nullifier 2 must not be spent on invalid proof"
        );
    }

    #[tokio::test]
    async fn test_handle_spend_joinsplit_malformed_proof_deserialization_rejected() {
        let (state, _tmp) = setup_test_state().await;
        let mut req = sample_valid_joinsplit_request();
        req.proof_a_neg = format!("0x{}", "11".repeat(128));

        let resp = handle_spend_joinsplit(State(state.clone()), Json(req.clone())).await;
        assert_eq!(resp.0.status, "REJECTED");
        assert!(resp
            .0
            .message
            .contains("Failed to deserialize Groth16 proof points"));

        // Verify nullifiers are NOT reserved
        let is_spent_1 = state
            .db
            .is_nullifier_spent(&req.input_nullifier_1)
            .await
            .unwrap();
        assert!(
            !is_spent_1,
            "nullifier must not be spent on malformed proof"
        );
    }

    #[tokio::test]
    async fn test_handle_spend_joinsplit_double_spend_rejected() {
        let (state, _tmp) = setup_test_state().await;
        let req = sample_valid_joinsplit_request();

        // Mark nullifier 1 as already spent in DB
        state
            .db
            .check_and_insert_nullifier(&req.input_nullifier_1, Some("0xexisting_tx"))
            .await
            .unwrap();

        let resp = handle_spend_joinsplit(State(state), Json(req)).await;
        assert_eq!(resp.0.status, "REJECTED");
        assert!(resp
            .0
            .message
            .contains("Nullifier already spent in relayer database"));
    }

    #[tokio::test]
    async fn test_handle_spend_joinsplit_non_canonical_scalar_rejected() {
        let (state, _tmp) = setup_test_state().await;
        let mut req = sample_valid_joinsplit_request();
        // Replace public input at index 0 with field overflow >= r
        req.public_inputs[0] =
            "0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff".to_string();

        let resp = handle_spend_joinsplit(State(state), Json(req)).await;
        assert_eq!(resp.0.status, "REJECTED");
        assert!(resp.0.message.contains("non-canonical scalar"));
    }

    #[tokio::test]
    async fn test_handle_spend_joinsplit_semantic_mismatch_rejected() {
        let (state, _tmp) = setup_test_state().await;
        let mut req = sample_valid_joinsplit_request();
        // Tamper with merchant_amount in payload
        req.merchant_amount = 5_000_000;

        let resp = handle_spend_joinsplit(State(state), Json(req)).await;
        assert_eq!(resp.0.status, "REJECTED");
        assert!(resp.0.message.contains("does not match merchant_amount"));
    }

    #[tokio::test]
    async fn test_handle_spend_joinsplit_flags_packed_mismatch_rejected() {
        let (state, _tmp) = setup_test_state().await;
        let mut req = sample_valid_joinsplit_request();
        // Change flags_packed from 0 to 3 while has_change is 0
        req.flags_packed = Some(3);

        let resp = handle_spend_joinsplit(State(state), Json(req)).await;
        assert_eq!(resp.0.status, "REJECTED");
        assert!(resp
            .0
            .message
            .contains("does not match decomposed change flags"));
    }

    #[tokio::test]
    async fn test_handle_spend_joinsplit_sanctioned_recipient_rejected() {
        let (state, _tmp) = setup_test_state().await;
        let mut req = sample_valid_joinsplit_request();
        req.recipient = crate::validation::sanctioned_evm_addresses()
            .iter()
            .next()
            .unwrap()
            .to_string();

        let resp = handle_spend_joinsplit(State(state), Json(req)).await;
        assert_eq!(resp.0.status, "REJECTED");
        assert!(resp
            .0
            .message
            .contains("restricted under public sanctions policy"));
    }

    #[tokio::test]
    async fn test_relayer_rejects_cross_asset_quote_tampering() {
        let (state, _tmp) = setup_test_state().await;

        // 1. Relayer creates a valid quote for USDC (asset_id = 1)
        let usdc_quote = crate::dto::ExecutionQuoteResponse {
            asset_id: 1,
            base_gas_fee: 20_000,
            relayer_markup_fee: 3_000,
            total_execution_fee: 23_000,
            quote_hash: "0xabcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890"
                .to_string(),
            expiry: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs()
                + 20,
            relayer_signature: "0x1234".to_string(),
        };

        // Cache this quote in state
        state
            .quote_cache
            .lock()
            .await
            .insert(usdc_quote.quote_hash.clone(), usdc_quote.clone());

        // 2. Client attempts to spend a WETH note (asset_id = 0) referencing the USDC quote
        let mut req = sample_valid_private_note_request();
        req.asset_id = Some(0); // WETH spend
        req.quote_hash = Some(usdc_quote.quote_hash.clone());

        let resp = handle_private_note_spend(State(state), Json(req)).await;
        assert_eq!(resp.0.status, "REJECTED");
        assert!(
            resp.0.message.contains("CrossAssetFeeMismatch"),
            "Expected CrossAssetFeeMismatch, got: {}",
            resp.0.message
        );
    }

    #[tokio::test]
    async fn test_relayer_rejects_invalid_memo_hex_length() {
        let (state, _tmp) = setup_test_state().await;
        let mut req = sample_valid_joinsplit_request();
        // Encrypted memo must be exactly 96 bytes = 192 hex chars
        req.encrypted_memo_1 = "01020304".to_string(); // Invalid length

        let resp = handle_spend_joinsplit(State(state), Json(req)).await;
        assert_eq!(resp.0.status, "REJECTED");
        assert!(
            resp.0.message.contains("192 hex chars"),
            "Expected 192 hex error, got: {}",
            resp.0.message
        );
    }

    #[tokio::test]
    async fn test_relayer_rejects_non_zero_recipient_in_pool() {
        let (state, _tmp) = setup_test_state().await;
        let mut req = sample_valid_joinsplit_request();
        // In-pool mode: merchant_amount == 0, protocol_fee == 0
        req.merchant_amount = 0;
        req.protocol_fee = 0;
        // Non-zero recipient in in-pool mode must be rejected
        req.recipient = "0x0000000000000000000000000000000000001234".to_string();

        let resp = handle_spend_joinsplit(State(state), Json(req)).await;
        assert_eq!(resp.0.status, "REJECTED");
        assert!(
            resp.0.message.contains("recipient=0x0"),
            "Expected zero address error, got: {}",
            resp.0.message
        );
    }
}
