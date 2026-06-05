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

mod types;
mod codec;
mod pool;

pub use types::*;
pub use codec::*;
pub use pool::*;

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use nimbus_core::*;
    use base64::Engine;
    use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;

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
            1000,
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
