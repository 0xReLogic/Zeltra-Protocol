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
            return Err("Verifikasi integritas payload gagal".to_string());
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
            return Err("Verifikasi integritas payload gagal".to_string());
        }
    }

    let bytes = BASE64_STANDARD
        .decode(header_value.trim())
        .map_err(|e| format!("Base64 decode error: {}", e))?;
    serde_json::from_slice(&bytes).map_err(|e| format!("JSON parse error: {}", e))
}

/// Encodes a PAYMENT-REQUIRED payload into a base64 string (used by the
/// Nimbus relayer when acting as a resource server / facilitator).
pub fn encode_payment_required(req: &X402PaymentRequired) -> Result<String, String> {
    let json = serde_json::to_vec(req).map_err(|e| format!("JSON serialize error: {}", e))?;
    Ok(BASE64_STANDARD.encode(&json))
}

/// Constructs a complete X402PaymentSignature from Nimbus spend parameters.
///
/// This is the primary entry point for AI Agents: given the anonymous token
/// spend proof (nullifier, alpha_neg, hm, pk_iss), it builds a valid x402
/// PAYMENT-SIGNATURE payload that can be attached to an HTTP request header.
pub fn build_nimbus_payment_signature(
    nullifier_hex: &str,
    alpha_neg_hex: &str,
    hm_hex: &str,
    pk_iss_hex: &str,
    amount: u64,
    scheme: &str,
    network: &str,
    recipient_or_intent_hash_hex: Option<String>,
    expiry: Option<u64>,
    nonce_hex: Option<String>,
) -> X402PaymentSignature {
    X402PaymentSignature {
        x402_version: 2,
        scheme: scheme.to_string(),
        network: network.to_string(),
        payment: NimbusPaymentPayload {
            nullifier: nullifier_hex.to_string(),
            alpha_neg_hex: alpha_neg_hex.to_string(),
            hm_hex: hm_hex.to_string(),
            pk_iss_hex: pk_iss_hex.to_string(),
            amount,
            recipient_or_intent_hash_hex,
            expiry,
            nonce_hex,
        },
    }
}

/// Builds a payment signature where the caller starts from the merchant's exact
/// requested amount. The embedded contract amount is grossed up so the on-chain
/// 0.15% spend fee does not reduce the merchant payout.
pub fn build_nimbus_payment_signature_for_merchant_amount(
    nullifier_hex: &str,
    alpha_neg_hex: &str,
    hm_hex: &str,
    pk_iss_hex: &str,
    merchant_amount: u64,
    scheme: &str,
    network: &str,
    recipient_or_intent_hash_hex: Option<String>,
    expiry: Option<u64>,
    nonce_hex: Option<String>,
) -> Result<X402PaymentSignature, String> {
    let contract_amount = nimbus_core::gross_up_private_spend_amount(merchant_amount)
        .ok_or_else(|| "Invalid merchant amount".to_string())?;
    Ok(build_nimbus_payment_signature(
        nullifier_hex,
        alpha_neg_hex,
        hm_hex,
        pk_iss_hex,
        contract_amount,
        scheme,
        network,
        recipient_or_intent_hash_hex,
        expiry,
        nonce_hex,
    ))
}
