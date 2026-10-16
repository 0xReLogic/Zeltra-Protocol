//! Integration tests for x402 protocol facilitator endpoint (DEC-042)

use axum::Json;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use tempfile::TempDir;
use zeltra_core::{
    create_dummy_circuit, extract_public_inputs, fr_to_be_bytes, generate_note_proof,
    get_or_init_note_circuit_keys, to_evm_g1, to_evm_g2,
};
use zeltra_node::{
    dto::{X402PaymentSignatureInner, X402VerifyRequest, X402VerifyResponse, X402ZeltraPayment},
    handlers::handle_x402_verify,
    state::AppState,
};

async fn setup_test_state() -> (AppState, TempDir) {
    std::env::set_var("ZELTRA_DB_KEY", "test-x402-encryption-key");
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("x402_test.db");
    let db = zeltra_node::database::Database::new(&db_path)
        .await
        .unwrap();
    let state = AppState::new(db, None).await;
    (state, tmp)
}

fn sample_valid_x402_payment() -> X402ZeltraPayment {
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

    X402ZeltraPayment {
        asset_id: Some(1),
        note_root,
        leaf_count: Some(leaf_count),
        note_epoch_id: Some(0),
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
        expiry: Some(0),
        proof_a_neg: format!("0x{}", hex::encode(a_neg)),
        proof_b: format!("0x{}", hex::encode(b)),
        proof_c: format!("0x{}", hex::encode(c)),
        public_inputs,
        change_encrypted_memo: None,
    }
}

static ENV_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[tokio::test]
async fn test_x402_invalid_base64_payload() {
    let _guard = ENV_LOCK.lock().await;
    let (state, _tmp) = setup_test_state().await;
    let req = X402VerifyRequest {
        payment_signature_b64: "not valid base64 !!!".to_string(),
        resource_uri: Some("/api/premium-data".to_string()),
    };

    let Json(res): Json<X402VerifyResponse> =
        handle_x402_verify(axum::extract::State(state), Json(req)).await;
    assert!(!res.success);
    assert!(res.message.contains("Base64 decode error"));
}

#[tokio::test]
async fn test_x402_invalid_json_payload() {
    let _guard = ENV_LOCK.lock().await;
    let (state, _tmp) = setup_test_state().await;
    let b64 = BASE64_STANDARD.encode(b"this is definitely not json");
    let req = X402VerifyRequest {
        payment_signature_b64: b64,
        resource_uri: Some("/api/query".to_string()),
    };

    let Json(res): Json<X402VerifyResponse> =
        handle_x402_verify(axum::extract::State(state), Json(req)).await;
    assert!(!res.success);
    assert!(res.message.contains("JSON parse error"));
}

#[tokio::test]
async fn test_x402_unsupported_version() {
    let _guard = ENV_LOCK.lock().await;
    let (state, _tmp) = setup_test_state().await;
    let payment = sample_valid_x402_payment();
    let envelope = X402PaymentSignatureInner {
        x402_version: 1, // Unsupported legacy version
        scheme: "exact".to_string(),
        network: "eip155:421614".to_string(),
        payment,
    };
    let json_bytes = serde_json::to_vec(&envelope).unwrap();
    let b64 = BASE64_STANDARD.encode(&json_bytes);

    let req = X402VerifyRequest {
        payment_signature_b64: b64,
        resource_uri: None,
    };

    let Json(res): Json<X402VerifyResponse> =
        handle_x402_verify(axum::extract::State(state), Json(req)).await;
    assert!(!res.success);
    assert!(res.message.contains("Unsupported x402 version: 1"));
}

#[tokio::test]
async fn test_x402_recipient_mismatch() {
    let _guard = ENV_LOCK.lock().await;
    let (state, _tmp) = setup_test_state().await;
    std::env::set_var(
        "ZELTRA_X402_RECIPIENT",
        "0x2222222222222222222222222222222222222222",
    );

    let payment = sample_valid_x402_payment();
    let envelope = X402PaymentSignatureInner {
        x402_version: 2,
        scheme: "exact".to_string(),
        network: "eip155:421614".to_string(),
        payment,
    };
    let json_bytes = serde_json::to_vec(&envelope).unwrap();
    let b64 = BASE64_STANDARD.encode(&json_bytes);

    let req = X402VerifyRequest {
        payment_signature_b64: b64,
        resource_uri: Some("/api/weather".to_string()),
    };

    let Json(res): Json<X402VerifyResponse> =
        handle_x402_verify(axum::extract::State(state), Json(req)).await;
    std::env::remove_var("ZELTRA_X402_RECIPIENT");

    assert!(!res.success);
    assert!(res.message.contains("does not match facilitator recipient"));
}

#[tokio::test]
async fn test_x402_groth16_preflight_rejection_on_invalid_proof() {
    let _guard = ENV_LOCK.lock().await;
    let (state, _tmp) = setup_test_state().await;
    let mut payment = sample_valid_x402_payment();
    // Tamper with proof bytes
    payment.proof_a_neg = format!("0x{}", hex::encode([0xde; 128]));

    let envelope = X402PaymentSignatureInner {
        x402_version: 2,
        scheme: "exact".to_string(),
        network: "eip155:421614".to_string(),
        payment,
    };
    let json_bytes = serde_json::to_vec(&envelope).unwrap();
    let b64 = BASE64_STANDARD.encode(&json_bytes);

    let req = X402VerifyRequest {
        payment_signature_b64: b64,
        resource_uri: Some("/api/ai-inference".to_string()),
    };

    let Json(res): Json<X402VerifyResponse> =
        handle_x402_verify(axum::extract::State(state), Json(req)).await;
    assert!(!res.success);
    assert!(res.message.contains("preflight verification failed"));
}

#[tokio::test]
async fn test_x402_valid_payment_persistence_and_double_spend_prevention() {
    let _guard = ENV_LOCK.lock().await;
    let (state, _tmp) = setup_test_state().await;
    let payment = sample_valid_x402_payment();

    let envelope = X402PaymentSignatureInner {
        x402_version: 2,
        scheme: "exact".to_string(),
        network: "eip155:421614".to_string(),
        payment: payment.clone(),
    };
    let json_bytes = serde_json::to_vec(&envelope).unwrap();
    let b64 = BASE64_STANDARD.encode(&json_bytes);

    let req = X402VerifyRequest {
        payment_signature_b64: b64.clone(),
        resource_uri: Some("/api/premium/stock-data".to_string()),
    };

    // 1. First verification should succeed and enqueue spend
    let Json(res1): Json<X402VerifyResponse> =
        handle_x402_verify(axum::extract::State(state.clone()), Json(req.clone())).await;
    assert!(
        res1.success,
        "Valid payment verification should succeed: {}",
        res1.message
    );
    assert!(res1.message.contains("settlement id"));

    // 2. Second verification with the same nullifier should be rejected immediately (fail-closed double-spend defense)
    let Json(res2): Json<X402VerifyResponse> =
        handle_x402_verify(axum::extract::State(state), Json(req)).await;
    assert!(!res2.success, "Replayed payment must be rejected");
    assert!(
        res2.message
            .contains("already queued, submitted, or confirmed")
            || res2.message.contains("Double-spending detected")
    );
}

#[tokio::test]
async fn test_x402_direct_payload_compatibility() {
    let _guard = ENV_LOCK.lock().await;
    let (state, _tmp) = setup_test_state().await;
    let payment = sample_valid_x402_payment();

    // Serialize payment directly without envelope
    let json_bytes = serde_json::to_vec(&payment).unwrap();
    let b64 = BASE64_STANDARD.encode(&json_bytes);

    let req = X402VerifyRequest {
        payment_signature_b64: b64,
        resource_uri: Some("/api/direct-gateway".to_string()),
    };

    let Json(res): Json<X402VerifyResponse> =
        handle_x402_verify(axum::extract::State(state), Json(req)).await;
    assert!(
        res.success,
        "Direct payload should be parsed and verified seamlessly: {}",
        res.message
    );
}
