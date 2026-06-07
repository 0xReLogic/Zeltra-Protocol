//! x402 protocol facilitator handler

use axum::Json;
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use crate::{state::AppState, dto::*};

// x402 Facilitator handler
pub async fn handle_x402_verify(
    axum::extract::State(state): axum::extract::State<AppState>,
    Json(payload): Json<X402VerifyRequest>,
) -> Json<X402VerifyResponse> {
    // 1. Decode base64 PAYMENT-SIGNATURE
    let sig_bytes = match BASE64_STANDARD.decode(payload.payment_signature_b64.trim()) {
        Ok(b) => b,
        Err(e) => {
            return Json(X402VerifyResponse {
                success: false,
                tx_hash: None,
                message: format!("Base64 decode error: {}", e),
            });
        }
    };

    let sig: X402PaymentSignatureInner = match serde_json::from_slice(&sig_bytes) {
        Ok(s) => s,
        Err(e) => {
            return Json(X402VerifyResponse {
                success: false,
                tx_hash: None,
                message: format!("JSON parse error: {}", e),
            });
        }
    };

    // 2. Validate protocol version
    if sig.x402_version != 2 {
        return Json(X402VerifyResponse {
            success: false,
            tx_hash: None,
            message: format!("Unsupported x402 version: {}", sig.x402_version),
        });
    }

    // 3. Check nullifier for double-spend using persistent database
    match state.db.is_nullifier_spent(&sig.payment.nullifier).await {
        Ok(true) => {
            return Json(X402VerifyResponse {
                success: false,
                tx_hash: None,
                message: "Double-spending detected: nullifier already exists".to_string(),
            });
        }
        Ok(false) => {
            // Nullifier not spent, proceed
        }
        Err(e) => {
            eprintln!("X402 ERROR: Database nullifier check failed: {}", e);
            return Json(X402VerifyResponse {
                success: false,
                tx_hash: None,
                message: "Database error".to_string(),
            });
        }
    }

    // 4. Queue the anonymous spend internally
    let recipient = match std::env::var("NIMBUS_X402_RECIPIENT") {
        Ok(value) => value,
        Err(_) => {
            return Json(X402VerifyResponse {
                success: false,
                tx_hash: None,
                message: "NIMBUS_X402_RECIPIENT is not configured".to_string(),
            });
        }
    };
    let spend_req = SpendRequest {
        nullifier: sig.payment.nullifier.clone(),
        sig_hex: sig.payment.alpha_neg_hex.clone(),
        recipient,
        amount: sig.payment.amount,
        eip7702_auth: None,
        cross_chain: None,
        alpha_neg_hex: sig.payment.alpha_neg_hex.clone(),
        hm_hex: sig.payment.hm_hex.clone(),
        pk_iss_hex: sig.payment.pk_iss_hex.clone(),
        recipient_or_intent_hash_hex: sig.payment.recipient_or_intent_hash_hex.clone(),
        expiry: sig.payment.expiry,
        nonce_hex: sig.payment.nonce_hex.clone(),
        min_payout: None,
        deadline: None,
        idempotency_key: None,
    };

    let queue_id = match state.db.enqueue_spend(&spend_req).await {
        Ok(Some(id)) => id,
        Ok(None) => {
            return Json(X402VerifyResponse {
                success: false,
                tx_hash: None,
                message: "Payment nullifier already has an active settlement".to_string(),
            });
        }
        Err(e) => {
            return Json(X402VerifyResponse {
                success: false,
                tx_hash: None,
                message: format!("Failed to persist settlement: {}", e),
            });
        }
    };
    let position = state.db.queued_spend_count().await.unwrap_or(1);

    let resource_info = payload.resource_uri.unwrap_or_else(|| "<unknown>".to_string());
    println!("X402 FACILITATOR: Verified anonymous payment");
    println!("  Scheme     : {}", sig.scheme);
    println!("  Network    : {}", sig.network);
    println!("  Nullifier  : {}...", &sig.payment.nullifier[..core::cmp::min(8, sig.payment.nullifier.len())]);
    println!("  Resource   : {}", resource_info);
    println!("  Queue Pos  : {}", position);

    Json(X402VerifyResponse {
        success: true,
        tx_hash: None,
        message: format!(
            "Nimbus payment persisted as settlement {}; awaiting receipt confirmation",
            queue_id
        ),
    })
}
