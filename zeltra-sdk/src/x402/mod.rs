//! x402 Protocol Integration Module
//!
//! Implements the x402 v2 payment protocol (Coinbase/Cloudflare standard) for
//! machine-to-machine (M2M) stablecoin payments. This module enables AI Agents
//! to autonomously pay for API resources using Zeltra anonymous tokens instead
//! of revealing their on-chain identity through standard EIP-3009 signatures.
//!
//! Protocol Flow:
//! 1. AI Agent requests a resource from a server.
//! 2. Server responds HTTP 402 with `PAYMENT-REQUIRED` header (base64 JSON).
//! 3. Agent parses requirements, constructs anonymous payment via Zeltra blind
//!    signature, and retries with `PAYMENT-SIGNATURE` header.
//! 4. Server/Facilitator verifies the Zeltra token and settles payment.

mod codec;
mod types;

pub use codec::*;
pub use types::*;

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
    use base64::Engine;

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
    fn test_decode_payment_required_secure_hmac() {
        let original = X402PaymentRequired {
            accepts: vec![X402PaymentOption {
                scheme: "exact".to_string(),
                network: "eip155:42161".to_string(),
                price: X402Price {
                    amount: "1000".to_string(),
                    asset: "0xaf88d065e77c8cC2239327C5EDb3A432268e5831".to_string(),
                    extra: None,
                },
                pay_to: "0xRecipientAddress".to_string(),
            }],
            description: Some("Secure API".to_string()),
            mime_type: None,
        };

        let encoded = encode_payment_required(&original).unwrap();

        let secret = b"my_secure_shared_hmac_secret_key";
        let hmac_bytes = crate::hmac_sha256(secret, encoded.as_bytes());
        let hmac_hex = hex::encode(hmac_bytes);

        // 1. Decodes successfully with correct HMAC
        let decoded = decode_payment_required_secure(&encoded, &hmac_hex, secret).unwrap();
        assert_eq!(decoded.description.unwrap(), "Secure API");

        // 2. Fails to decode with incorrect HMAC
        let failed_hmac = "0000000000000000000000000000000000000000000000000000000000000000";
        let err = decode_payment_required_secure(&encoded, failed_hmac, secret);
        assert!(err.is_err());
        assert_eq!(err.err().unwrap(), "Payload integrity verification failed");
    }

    #[test]
    fn test_build_and_encode_payment_signature() {
        let payment = X402ZeltraPayment {
            asset_id: Some(1),
            note_root: "0x1234root".to_string(),
            leaf_count: Some(5),
            note_epoch_id: Some(1),
            input_nullifier: "0xabc123nullifier".to_string(),
            output_commitment: Some("0x5678change".to_string()),
            recipient: "0xMerchantAddress".to_string(),
            merchant_amount: 100_000_000,
            protocol_fee: 450_000,
            execution_fee: 15_000,
            max_execution_fee: 25_000,
            quote_hash: Some("0xquotehash".to_string()),
            quote_signature: None,
            quote_id: None,
            expiry: Some(1740000000),
            proof_a_neg: "0xproofA".to_string(),
            proof_b: "0xproofB".to_string(),
            proof_c: "0xproofC".to_string(),
            public_inputs: vec!["0xpi1".to_string(), "0xpi2".to_string()],
            change_encrypted_memo: Some("0xencryptedmemo".to_string()),
        };

        let sig = build_x402_zk_payment_signature("eip155:42161", "exact", payment);

        assert_eq!(sig.x402_version, 2);
        assert_eq!(sig.scheme, "exact");
        assert_eq!(sig.payment.input_nullifier, "0xabc123nullifier");
        assert_eq!(sig.payment.merchant_amount, 100_000_000);

        let encoded = encode_payment_signature(&sig).unwrap();
        assert!(!encoded.is_empty());

        // Verify round-trip via decode_payment_signature
        let decoded = decode_payment_signature(&encoded).unwrap();
        assert_eq!(decoded.payment.input_nullifier, "0xabc123nullifier");
        assert_eq!(decoded.payment.note_root, "0x1234root");
        assert_eq!(decoded.network, "eip155:42161");
        assert_eq!(decoded.payment.merchant_amount, 100_000_000);
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
    fn test_x402_value_conservation_check() {
        let input_value: u64 = 105_000_000;
        let merchant_amount: u64 = 100_000_000;
        let protocol_fee = zeltra_core::fees::fee_round_up(
            merchant_amount,
            zeltra_core::fees::PRIVATE_SPEND_FEE_BPS,
        )
        .unwrap(); // 450_000 (45 bps)
        let execution_fee: u64 = 15_000;
        let change_amount = input_value - merchant_amount - protocol_fee - execution_fee;

        assert_eq!(protocol_fee, 450_000);
        assert_eq!(change_amount, 4_535_000);
        assert_eq!(
            input_value,
            merchant_amount + protocol_fee + execution_fee + change_amount
        );
    }
}
