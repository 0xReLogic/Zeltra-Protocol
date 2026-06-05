//! x402 Protocol Data Structures
//!
//! Defines the core types for x402 v2 payment protocol (Coinbase/Cloudflare standard).

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
    /// The payment amount.
    pub amount: u64,
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
