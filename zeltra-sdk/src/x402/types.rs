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

/// Zeltra-specific anonymous payment payload embedded inside the
/// PAYMENT-SIGNATURE header. Instead of a standard EIP-3009 authorization that
/// would expose the sender's on-chain address, Zeltra uses a Pure ZK-UTXO
/// Private Note spend proof (DEC-038 / DEC-042).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct X402ZeltraPayment {
    /// Asset ID: 0 for WETH, 1 for USDC (defaults to 1 for USDC)
    #[serde(default)]
    pub asset_id: Option<u8>,
    /// On-chain Merkle Mountain Range (MMR) root
    pub note_root: String,
    /// Tree leaf count at time of witness generation
    #[serde(default)]
    pub leaf_count: Option<u64>,
    /// Note epoch ID
    #[serde(default)]
    pub note_epoch_id: Option<u32>,
    /// Input nullifier hash (hex) -- prevents double-spend
    #[serde(alias = "nullifier")]
    pub input_nullifier: String,
    /// Change note commitment (if any)
    #[serde(default)]
    pub output_commitment: Option<String>,
    /// Recipient merchant address
    pub recipient: String,
    /// Net merchant amount
    #[serde(alias = "amount")]
    pub merchant_amount: u64,
    /// Protocol spend fee (flat 45 bps)
    #[serde(default)]
    pub protocol_fee: u64,
    /// Relayer execution fee reimbursement
    #[serde(default)]
    pub execution_fee: u64,
    /// Max execution fee user is willing to pay
    #[serde(default)]
    pub max_execution_fee: u64,
    /// EIP-712 quote hash
    #[serde(default)]
    pub quote_hash: Option<String>,
    /// EIP-712 quote signature
    #[serde(default)]
    pub quote_signature: Option<String>,
    /// Quote ID
    #[serde(default)]
    pub quote_id: Option<String>,
    /// Expiry timestamp
    #[serde(default)]
    pub expiry: Option<u64>,
    /// Groth16 Proof A (negated G1, 128 bytes EVM hex)
    pub proof_a_neg: String,
    /// Groth16 Proof B (G2, 256 bytes EVM hex)
    pub proof_b: String,
    /// Groth16 Proof C (G1, 128 bytes EVM hex)
    pub proof_c: String,
    /// 16 Public Inputs in canonical Big-Endian EVM hex
    pub public_inputs: Vec<String>,
    /// 96-byte AEAD encrypted change memo (DEC-037)
    #[serde(default)]
    pub change_encrypted_memo: Option<String>,
}

/// Backward compatibility type alias
pub type ZeltraPaymentPayload = X402ZeltraPayment;

/// The PAYMENT-SIGNATURE header payload for Zeltra x402 transactions.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct X402PaymentSignature {
    /// Protocol version (must be 2 for x402 v2).
    #[serde(rename = "x402Version")]
    pub x402_version: u32,
    /// Payment scheme (e.g. "exact").
    pub scheme: String,
    /// CAIP-2 network identifier.
    pub network: String,
    /// Zeltra anonymous payment payload (replaces standard EVM transfer auth).
    #[serde(alias = "payload")]
    pub payment: X402ZeltraPayment,
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
