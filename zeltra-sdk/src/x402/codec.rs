//! Codec helpers for x402 protocol (base64 <-> JSON conversion)

use super::types::*;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;

/// Decodes a base64-encoded PAYMENT-REQUIRED header value into the structured
/// payment requirements.
pub fn decode_payment_required(header_value: &str) -> Result<X402PaymentRequired, String> {
    decode_payment_required_secure(header_value, "", &[])
}

/// Decodes a base64-encoded PAYMENT-REQUIRED header value and verifies its HMAC integrity.
pub fn decode_payment_required_secure(
    header_value: &str,
    expected_hmac: &str,
    secret_key: &[u8],
) -> Result<X402PaymentRequired, String> {
    if !expected_hmac.is_empty() {
        let calculated_hmac = hex::encode(crate::hmac_sha256(secret_key, header_value.as_bytes()));
        if calculated_hmac != expected_hmac {
            return Err("Payload integrity verification failed".to_string());
        }
    }

    let bytes = BASE64_STANDARD
        .decode(header_value.trim())
        .map_err(|e| format!("Base64 decode error: {}", e))?;
    serde_json::from_slice(&bytes).map_err(|e| format!("JSON parse error: {}", e))
}

/// Encodes a PAYMENT-SIGNATURE payload into a base64 string suitable for the
/// HTTP header.
pub fn encode_payment_signature(sig: &X402PaymentSignature) -> Result<String, String> {
    let json = serde_json::to_vec(sig).map_err(|e| format!("JSON serialize error: {}", e))?;
    Ok(BASE64_STANDARD.encode(&json))
}

/// Decodes a base64-encoded PAYMENT-RESPONSE header value.
pub fn decode_payment_response(header_value: &str) -> Result<X402PaymentResponse, String> {
    decode_payment_response_secure(header_value, "", &[])
}

/// Decodes a base64-encoded PAYMENT-RESPONSE header value and verifies its HMAC integrity.
pub fn decode_payment_response_secure(
    header_value: &str,
    expected_hmac: &str,
    secret_key: &[u8],
) -> Result<X402PaymentResponse, String> {
    if !expected_hmac.is_empty() {
        let calculated_hmac = hex::encode(crate::hmac_sha256(secret_key, header_value.as_bytes()));
        if calculated_hmac != expected_hmac {
            return Err("Payload integrity verification failed".to_string());
        }
    }

    let bytes = BASE64_STANDARD
        .decode(header_value.trim())
        .map_err(|e| format!("Base64 decode error: {}", e))?;
    serde_json::from_slice(&bytes).map_err(|e| format!("JSON parse error: {}", e))
}

/// Encodes a PAYMENT-REQUIRED payload into a base64 string (used by the
/// Zeltra relayer when acting as a resource server / facilitator).
pub fn encode_payment_required(req: &X402PaymentRequired) -> Result<String, String> {
    let json = serde_json::to_vec(req).map_err(|e| format!("JSON serialize error: {}", e))?;
    Ok(BASE64_STANDARD.encode(&json))
}

/// Decodes a base64-encoded PAYMENT-SIGNATURE header value into the structured
/// payment signature.
pub fn decode_payment_signature(header_value: &str) -> Result<X402PaymentSignature, String> {
    let bytes = BASE64_STANDARD
        .decode(header_value.trim())
        .map_err(|e| format!("Base64 decode error: {}", e))?;
    serde_json::from_slice(&bytes).map_err(|e| format!("JSON parse error: {}", e))
}

/// Constructs a complete X402PaymentSignature from Zeltra ZK-UTXO spend parameters (DEC-042).
///
/// This is the primary entry point for AI Agents: given the ZK-UTXO spend proof
/// (PrivateNoteCircuit / JoinSplitCircuit), it builds a valid x402 PAYMENT-SIGNATURE
/// payload that can be attached to an HTTP request header.
pub fn build_x402_zk_payment_signature(
    network: &str,
    scheme: &str,
    payment: X402ZeltraPayment,
) -> X402PaymentSignature {
    X402PaymentSignature {
        x402_version: 2,
        scheme: scheme.to_string(),
        network: network.to_string(),
        payment,
    }
}
