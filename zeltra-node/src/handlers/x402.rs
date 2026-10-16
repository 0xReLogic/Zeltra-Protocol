//! x402 protocol facilitator handler (DEC-042 Pure ZK-UTXO Micropayments)

use crate::{dto::*, handlers::spend::handle_private_note_spend, state::AppState};
use axum::Json;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;

/// x402 Facilitator verification endpoint
/// POST /api/x402/verify
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

    // 2. Parse payload: Support standard x402 v2 envelope (X402PaymentSignatureInner)
    // with fallback to direct X402ZeltraPayment for maximum interop.
    let (payment, scheme_info, network_info) =
        if let Ok(envelope) = serde_json::from_slice::<X402PaymentSignatureInner>(&sig_bytes) {
            if envelope.x402_version != 2 {
                return Json(X402VerifyResponse {
                    success: false,
                    tx_hash: None,
                    message: format!("Unsupported x402 version: {}", envelope.x402_version),
                });
            }
            (envelope.payment, envelope.scheme, envelope.network)
        } else if let Ok(direct_payment) = serde_json::from_slice::<X402ZeltraPayment>(&sig_bytes) {
            (direct_payment, "exact".to_string(), "unknown".to_string())
        } else {
            return Json(X402VerifyResponse {
                success: false,
                tx_hash: None,
                message: "JSON parse error: invalid x402 payment signature format".to_string(),
            });
        };

    // 3. Optional facilitator recipient restriction (if ZELTRA_X402_RECIPIENT is configured)
    if let Ok(expected_recipient) = std::env::var("ZELTRA_X402_RECIPIENT") {
        if !payment.recipient.eq_ignore_ascii_case(&expected_recipient) {
            return Json(X402VerifyResponse {
                success: false,
                tx_hash: None,
                message: format!(
                    "Payment recipient {} does not match facilitator recipient {}",
                    payment.recipient, expected_recipient
                ),
            });
        }
    }

    // 4. Convert to PrivateNoteSpendRequest for canonical preflight checks & settlement queueing
    let note_spend_req = PrivateNoteSpendRequest::from(payment);

    let res = handle_private_note_spend(axum::extract::State(state), Json(note_spend_req))
        .await
        .0;

    let resource_info = payload
        .resource_uri
        .unwrap_or_else(|| "<unknown>".to_string());

    if res.status == "QUEUED" {
        println!(
            "X402 FACILITATOR: Successfully verified and enqueued payment (scheme: {}, net: {}) for resource: {}",
            scheme_info, network_info, resource_info
        );
        Json(X402VerifyResponse {
            success: true,
            tx_hash: None,
            message: res.message,
        })
    } else {
        println!(
            "X402 FACILITATOR: Payment rejected for resource: {}. Reason: {}",
            resource_info, res.message
        );
        Json(X402VerifyResponse {
            success: false,
            tx_hash: None,
            message: res.message,
        })
    }
}
