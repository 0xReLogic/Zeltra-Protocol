//! x402 Protocol Integration Module
//!
//! Implements the x402 v2 payment protocol (Coinbase/Cloudflare standard) for
//! machine-to-machine (M2M) stablecoin payments. This module enables AI Agents
//! to autonomously pay for API resources using Nimbus anonymous tokens instead
//! of revealing their on-chain identity through standard EIP-3009 signatures.
//!
//! Protocol Flow:
//! 1. AI Agent requests a resource from a server.
//! 2. Server responds HTTP 402 with `PAYMENT-REQUIRED` header (base64 JSON).
//! 3. Agent parses requirements, constructs anonymous payment via Nimbus blind
//!    signature, and retries with `PAYMENT-SIGNATURE` header.
//! 4. Server/Facilitator verifies the Nimbus token and settles payment.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// x402 v2 Data Structures
// ---------------------------------------------------------------------------

/// Price specification inside a payment requirement.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct X402Price {
    /// Amount in token base units (e.g. "1000" for 0.001 USDC with 6 decimals).
    pub amount: String,
    /// ERC-20 token contract address (e.g. USDC on Arbitrum).
    pub asset: String,
    /// Optional metadata about the asset.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra: Option<X402AssetExtra>,
}

/// Additional metadata about the payment asset.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct X402AssetExtra {
    pub name: String,
    pub version: String,
    pub decimals: u8,
}

/// A single accepted payment option from the server.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct X402PaymentOption {
    /// Payment scheme identifier (e.g. "exact").
    pub scheme: String,
    /// CAIP-2 network identifier (e.g. "eip155:42161" for Arbitrum One).
    pub network: String,
    /// Price specification.
    pub price: X402Price,
    /// Recipient wallet address for the payment.
    #[serde(rename = "payTo")]
    pub pay_to: String,
}

/// The full PAYMENT-REQUIRED payload (decoded from base64 JSON in the header).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct X402PaymentRequired {
    /// List of accepted payment options.
    pub accepts: Vec<X402PaymentOption>,
    /// Human-readable description of the resource.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// MIME type of the protected resource.
    #[serde(rename = "mimeType")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
}

/// Nimbus-specific anonymous payment payload embedded inside the
/// PAYMENT-SIGNATURE header. Instead of a standard EIP-3009 authorization that
/// would expose the sender's on-chain address, Nimbus uses a BLS blind
/// signature spend proof.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NimbusPaymentPayload {
    /// Nullifier hash (hex) -- prevents double-spend.
    pub nullifier: String,
    /// Negated unblinded BLS signature -alpha (hex, 128 bytes EVM).
    pub alpha_neg_hex: String,
    /// Hash-to-curve of the ephemeral key H(m) (hex, 128 bytes EVM).
    pub hm_hex: String,
    /// Issuer public key pk_iss (hex, 256 bytes EVM).
    pub pk_iss_hex: String,
}

/// The PAYMENT-SIGNATURE header payload for Nimbus x402 transactions.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct X402PaymentSignature {
    /// Protocol version (must be 2 for x402 v2).
    #[serde(rename = "x402Version")]
    pub x402_version: u32,
    /// Payment scheme (e.g. "exact").
    pub scheme: String,
    /// CAIP-2 network identifier.
    pub network: String,
    /// Nimbus anonymous payment payload (replaces standard EVM transfer auth).
    pub payment: NimbusPaymentPayload,
}

/// Settlement receipt returned in the PAYMENT-RESPONSE header.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct X402PaymentResponse {
    /// Whether the payment was successfully settled.
    pub success: bool,
    /// On-chain transaction hash (if settled).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tx_hash: Option<String>,
    /// Human-readable status message.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

// ---------------------------------------------------------------------------
// Codec helpers (base64 <-> JSON)
// ---------------------------------------------------------------------------

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;

/// Decodes a base64-encoded PAYMENT-REQUIRED header value into the structured
/// payment requirements.
pub fn decode_payment_required(header_value: &str) -> Result<X402PaymentRequired, String> {
    let bytes = BASE64_STANDARD
        .decode(header_value.trim())
        .map_err(|e| format!("Base64 decode error: {}", e))?;
    serde_json::from_slice(&bytes)
        .map_err(|e| format!("JSON parse error: {}", e))
}

/// Encodes a PAYMENT-SIGNATURE payload into a base64 string suitable for the
/// HTTP header.
pub fn encode_payment_signature(sig: &X402PaymentSignature) -> Result<String, String> {
    let json = serde_json::to_vec(sig)
        .map_err(|e| format!("JSON serialize error: {}", e))?;
    Ok(BASE64_STANDARD.encode(&json))
}

/// Decodes a base64-encoded PAYMENT-RESPONSE header value.
pub fn decode_payment_response(header_value: &str) -> Result<X402PaymentResponse, String> {
    let bytes = BASE64_STANDARD
        .decode(header_value.trim())
        .map_err(|e| format!("Base64 decode error: {}", e))?;
    serde_json::from_slice(&bytes)
        .map_err(|e| format!("JSON parse error: {}", e))
}

/// Encodes a PAYMENT-REQUIRED payload into a base64 string (used by the
/// Nimbus relayer when acting as a resource server / facilitator).
pub fn encode_payment_required(req: &X402PaymentRequired) -> Result<String, String> {
    let json = serde_json::to_vec(req)
        .map_err(|e| format!("JSON serialize error: {}", e))?;
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
    scheme: &str,
    network: &str,
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
        },
    }
}

// ---------------------------------------------------------------------------
// Agent Token Pool & Non-Interactive Refill Manager
// ---------------------------------------------------------------------------

use std::collections::HashMap;
use wasm_bindgen::prelude::*;
use nimbus_core::*;
use sha2::Digest;

#[derive(Clone, Serialize, Deserialize)]
pub struct PendingToken {
    pub session_id: String,
    pub message: String,
    pub blinded_message_hex: String,
    pub blinding_factor_hex: String,
    pub amount: u64,
    pub com_k_hex: Option<String>,
    pub masked_sig_hex: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ReadyToken {
    pub message: String,
    pub unmasked_sig_hex: String,
    pub amount: u64,
}

#[wasm_bindgen]
#[derive(Default, Serialize, Deserialize)]
pub struct AgentTokenPool {
    #[wasm_bindgen(skip)]
    pub pending_tokens: HashMap<String, PendingToken>,
    #[wasm_bindgen(skip)]
    pub ready_tokens: Vec<ReadyToken>,
}

#[wasm_bindgen]
impl AgentTokenPool {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self::default()
    }

    #[wasm_bindgen]
    pub fn serialize_pool(&self) -> Result<String, JsValue> {
        serde_json::to_string(self)
            .map_err(|e| JsValue::from_str(&format!("Serialize error: {}", e)))
    }

    #[wasm_bindgen]
    pub fn deserialize_pool(json_str: &str) -> Result<AgentTokenPool, JsValue> {
        serde_json::from_str(json_str)
            .map_err(|e| JsValue::from_str(&format!("Deserialize error: {}", e)))
    }

    #[wasm_bindgen]
    pub fn prepare_blind_token(&mut self, amount: u64, message: &str) -> Result<String, JsValue> {
        let mut rng = rand::thread_rng();
        let (blinded, r) = client_blind(message.as_bytes(), &mut rng);
        
        let blinded_hex = hex::encode(serialize_to_bytes(&blinded));
        let r_hex = hex::encode(serialize_to_bytes(&r));
        
        // Generate random session_id
        let session_id = hex::encode(rand::random::<[u8; 16]>());
        
        let pending = PendingToken {
            session_id: session_id.clone(),
            message: message.to_string(),
            blinded_message_hex: blinded_hex,
            blinding_factor_hex: r_hex,
            amount,
            com_k_hex: None,
            masked_sig_hex: None,
        };
        
        self.pending_tokens.insert(session_id.clone(), pending);
        Ok(session_id)
    }

    #[wasm_bindgen]
    pub fn get_blinded_message(&self, session_id: &str) -> Result<String, JsValue> {
        let token = self.pending_tokens.get(session_id)
            .ok_or_else(|| JsValue::from_str("Session ID not found"))?;
        Ok(token.blinded_message_hex.clone())
    }

    #[wasm_bindgen]
    pub fn register_signing_result(
        &mut self,
        session_id: &str,
        masked_sig_hex: &str,
        com_k_hex: &str,
    ) -> Result<bool, JsValue> {
        let token = self.pending_tokens.get_mut(session_id)
            .ok_or_else(|| JsValue::from_str("Session ID not found"))?;
            
        let blinded_bytes = hex::decode(&token.blinded_message_hex)
            .map_err(|e| JsValue::from_str(&format!("Invalid blinded hex: {}", e)))?;
        let com_k_bytes = hex::decode(com_k_hex)
            .map_err(|e| JsValue::from_str(&format!("Invalid com_k hex: {}", e)))?;
        let masked_sig_bytes = hex::decode(masked_sig_hex)
            .map_err(|e| JsValue::from_str(&format!("Invalid masked_sig hex: {}", e)))?;
            
        let x: BlindedMessage = deserialize_from_bytes(&blinded_bytes)
            .ok_or_else(|| JsValue::from_str("Failed to deserialize blinded message"))?;
        let commitment: MaskingKeyCommitment = deserialize_from_bytes(&com_k_bytes)
            .ok_or_else(|| JsValue::from_str("Failed to deserialize commitment"))?;
        let sig: MaskedBlindSignature = deserialize_from_bytes(&masked_sig_bytes)
            .ok_or_else(|| JsValue::from_str("Failed to deserialize masked signature"))?;
            
        let is_valid = client_verify_masked(&x, &commitment, &sig);
        if is_valid {
            token.masked_sig_hex = Some(masked_sig_hex.to_string());
            token.com_k_hex = Some(com_k_hex.to_string());
        }
        
        Ok(is_valid)
    }

    #[wasm_bindgen]
    pub fn unmask_token(&mut self, session_id: &str, masking_key_hex: &str) -> Result<bool, JsValue> {
        let token = self.pending_tokens.get(session_id)
            .ok_or_else(|| JsValue::from_str("Session ID not found"))?;
            
        let masked_sig_hex = token.masked_sig_hex.as_ref()
            .ok_or_else(|| JsValue::from_str("Token has not been signed or registered yet"))?;
            
        let sig_bytes = hex::decode(masked_sig_hex)
            .map_err(|e| JsValue::from_str(&format!("Invalid masked_sig hex: {}", e)))?;
        let r_bytes = hex::decode(&token.blinding_factor_hex)
            .map_err(|e| JsValue::from_str(&format!("Invalid r hex: {}", e)))?;
        let k_bytes = hex::decode(masking_key_hex)
            .map_err(|e| JsValue::from_str(&format!("Invalid k hex: {}", e)))?;
            
        let sig: MaskedBlindSignature = deserialize_from_bytes(&sig_bytes)
            .ok_or_else(|| JsValue::from_str("Failed to deserialize masked signature"))?;
        let r_factor: BlindingFactor = deserialize_from_bytes(&r_bytes)
            .ok_or_else(|| JsValue::from_str("Failed to deserialize blinding factor"))?;
        let k_key: MaskingKey = deserialize_from_bytes(&k_bytes)
            .ok_or_else(|| JsValue::from_str("Failed to deserialize masking key"))?;
            
        let unmasked = client_unmask(&sig, &r_factor, &k_key)
            .ok_or_else(|| JsValue::from_str("Failed to compute unmask key inverse"))?;
            
        let unmasked_sig_hex = hex::encode(serialize_to_bytes(&unmasked));
        
        // Move from pending to ready
        let ready = ReadyToken {
            message: token.message.clone(),
            unmasked_sig_hex,
            amount: token.amount,
        };
        
        self.ready_tokens.push(ready);
        self.pending_tokens.remove(session_id);
        
        Ok(true)
    }

    #[wasm_bindgen]
    pub fn get_ready_token_count(&self, amount: u64) -> usize {
        self.ready_tokens.iter().filter(|t| t.amount == amount).count()
    }

    #[wasm_bindgen]
    pub fn get_pending_token_count(&self) -> usize {
        self.pending_tokens.len()
    }

    #[wasm_bindgen]
    pub fn spend_any_token(
        &mut self,
        amount: u64,
        scheme: &str,
        network: &str,
        pk_iss_hex: &str,
    ) -> Result<String, JsValue> {
        // Find index of first token with matching amount
        let index = self.ready_tokens.iter().position(|t| t.amount == amount)
            .ok_or_else(|| JsValue::from_str("No ready tokens with the requested amount available"))?;
            
        let token = self.ready_tokens.remove(index);
        
        // Compute EVM inputs
        let alpha_neg_bytes = crate::client_get_alpha_neg_evm(&token.unmasked_sig_hex)?;
        let hm_bytes = crate::client_get_hm_evm(&token.message);
        let pk_iss_evm = crate::client_get_pk_iss_evm(pk_iss_hex)?;
        
        // Use message hash as nullifier candidate
        let nullifier = hex::encode(sha2::Sha256::digest(token.message.as_bytes()));
        
        let sig = build_nimbus_payment_signature(
            &nullifier,
            &alpha_neg_bytes,
            &hm_bytes,
            &pk_iss_evm,
            scheme,
            network,
        );
        
        let header_value = encode_payment_signature(&sig)
            .map_err(|e| JsValue::from_str(&e))?;
            
        Ok(header_value)
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_decode_payment_required_roundtrip() {
        let original = X402PaymentRequired {
            accepts: vec![X402PaymentOption {
                scheme: "exact".to_string(),
                network: "eip155:42161".to_string(),
                price: X402Price {
                    amount: "1000".to_string(),
                    asset: "0xaf88d065e77c8cC2239327C5EDb3A432268e5831".to_string(),
                    extra: Some(X402AssetExtra {
                        name: "USDC".to_string(),
                        version: "1".to_string(),
                        decimals: 6,
                    }),
                },
                pay_to: "0xRecipientAddress".to_string(),
            }],
            description: Some("Market data API".to_string()),
            mime_type: Some("application/json".to_string()),
        };

        let encoded = encode_payment_required(&original).unwrap();
        let decoded = decode_payment_required(&encoded).unwrap();

        assert_eq!(decoded.accepts.len(), 1);
        assert_eq!(decoded.accepts[0].scheme, "exact");
        assert_eq!(decoded.accepts[0].network, "eip155:42161");
        assert_eq!(decoded.accepts[0].price.amount, "1000");
        assert_eq!(decoded.description.unwrap(), "Market data API");
    }

    #[test]
    fn test_build_and_encode_payment_signature() {
        let sig = build_nimbus_payment_signature(
            "0xabc123nullifier",
            "0xalpha_neg_hex_data",
            "0xhm_hex_data",
            "0xpk_iss_hex_data",
            "exact",
            "eip155:42161",
        );

        assert_eq!(sig.x402_version, 2);
        assert_eq!(sig.scheme, "exact");
        assert_eq!(sig.payment.nullifier, "0xabc123nullifier");

        let encoded = encode_payment_signature(&sig).unwrap();
        assert!(!encoded.is_empty());

        // Verify round-trip via raw base64 decode
        let decoded_bytes = BASE64_STANDARD.decode(&encoded).unwrap();
        let decoded: X402PaymentSignature = serde_json::from_slice(&decoded_bytes).unwrap();
        assert_eq!(decoded.payment.nullifier, "0xabc123nullifier");
        assert_eq!(decoded.network, "eip155:42161");
    }

    #[test]
    fn test_payment_response_roundtrip() {
        let resp = X402PaymentResponse {
            success: true,
            tx_hash: Some("0xdeadbeef".to_string()),
            message: Some("Payment settled".to_string()),
        };

        let json = serde_json::to_vec(&resp).unwrap();
        let encoded = BASE64_STANDARD.encode(&json);
        let decoded = decode_payment_response(&encoded).unwrap();

        assert!(decoded.success);
        assert_eq!(decoded.tx_hash.unwrap(), "0xdeadbeef");
    }

    #[test]
    fn test_agent_token_pool_flow() {
        let mut pool = AgentTokenPool::new();
        
        // 1. Prepare token
        let session_id = pool.prepare_blind_token(1000, "agent_secret_payment_id_1").unwrap();
        assert_eq!(pool.get_pending_token_count(), 1);
        assert_eq!(pool.get_ready_token_count(1000), 0);
        
        // 2. Issuer signs blinded message
        let blinded_hex = pool.get_blinded_message(&session_id).unwrap();
        let blinded_bytes = hex::decode(blinded_hex).unwrap();
        let x: BlindedMessage = deserialize_from_bytes(&blinded_bytes).unwrap();
        
        let mut rng = rand::thread_rng();
        let sk_iss = IssuerSecretKey::generate(&mut rng);
        let pk_iss = sk_iss.public_key();
        
        let (masked_sig, k, com_k) = issuer_sign_blinded(&sk_iss, &x, &mut rng);
        let masked_sig_hex = hex::encode(serialize_to_bytes(&masked_sig));
        let com_k_hex = hex::encode(serialize_to_bytes(&com_k));
        let pk_iss_hex = hex::encode(serialize_to_bytes(&pk_iss));
        
        // 3. Register signature result in pool
        let is_valid = pool.register_signing_result(&session_id, &masked_sig_hex, &com_k_hex).unwrap();
        assert!(is_valid);
        
        // 4. Unmask token (once masking key revealed)
        let k_hex = hex::encode(serialize_to_bytes(&k));
        let success = pool.unmask_token(&session_id, &k_hex).unwrap();
        assert!(success);
        
        assert_eq!(pool.get_pending_token_count(), 0);
        assert_eq!(pool.get_ready_token_count(1000), 1);
        
        // 5. Spend token (x402 header generation)
        let header_val = pool.spend_any_token(1000, "exact", "eip155:42161", &pk_iss_hex).unwrap();
        assert!(!header_val.is_empty());
        assert_eq!(pool.get_ready_token_count(1000), 0);
    }
}

