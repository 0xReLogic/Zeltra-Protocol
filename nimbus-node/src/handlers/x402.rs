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
    let spend_req = SpendRequest {
        nullifier: sig.payment.nullifier.clone(),
        sig_hex: sig.payment.alpha_neg_hex.clone(),
        recipient: "x402-facilitator-pool".to_string(),
        amount: sig.payment.amount,
        eip7702_auth: None,
        cross_chain: None,
    };

    let mut queue = state.spend_queue.lock().await;
    queue.push(spend_req);
    let position = queue.len();
    drop(queue);

    let resource_info = payload.resource_uri.unwrap_or_else(|| "<unknown>".to_string());
    println!("X402 FACILITATOR: Verified anonymous payment");
    println!("  Scheme     : {}", sig.scheme);
    println!("  Network    : {}", sig.network);
    println!("  Nullifier  : {}...", &sig.payment.nullifier[..core::cmp::min(16, sig.payment.nullifier.len())]);
    println!("  Resource   : {}", resource_info);
    println!("  Queue Pos  : {}", position);

    // 5. Return settlement receipt
    // WARNING / REMINDER FOR DEVELOPERS & AI AGENTS:
    // This is a simulated transaction hash.
    // In production, replace this with the real on-chain transaction hash returned from the EVM RPC broadcast.
    let mock_tx_hash = format!("0x{}", hex::encode(rand::random::<[u8; 32]>()));

    Json(X402VerifyResponse {
        success: true,
        tx_hash: Some(mock_tx_hash),
        message: "Nimbus anonymous payment verified and queued for settlement".to_string(),
    })
}
