//! Request and Response DTOs for API endpoints

use serde::{Deserialize, Serialize};

#[allow(dead_code)]
#[derive(Clone, Serialize, Deserialize)]
pub struct Session {
    pub session_id: String,
    pub com_k: String,
    pub amount: u64,
    pub resolved: bool,
    pub masking_key: Option<String>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct CrossChainParams {
    pub destination_chain_selector: u64,
    pub destination_contract: String,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct SpendRequest {
    pub nullifier: String,
    pub sig_hex: String,
    pub recipient: String,
    #[serde(default)]
    pub amount: u64,
    pub eip7702_auth: Option<Eip7702Auth>,
    pub cross_chain: Option<CrossChainParams>,
    // BLS signature components for contract verification
    #[serde(default)]
    pub alpha_neg_hex: String,
    #[serde(default)]
    pub hm_hex: String,
    #[serde(default)]
    pub pk_iss_hex: String,
    #[serde(default)]
    pub recipient_or_intent_hash_hex: Option<String>,
    #[serde(default)]
    pub expiry: Option<u64>,
    #[serde(default)]
    pub nonce_hex: Option<String>,
    /// Slippage protection: minimum acceptable payout in USDC base units (6 decimals).
    /// If net_payout < min_payout after gas deduction, the transaction is rejected.
    #[serde(default)]
    pub min_payout: Option<u64>,
    /// Slippage protection: deadline as Unix timestamp (seconds).
    /// Transaction is rejected if processing occurs after this timestamp.
    #[serde(default)]
    pub deadline: Option<u64>,
    /// Client-supplied idempotency key (UUID recommended).
    /// If provided, duplicate requests with the same key return the cached response.
    #[serde(default)]
    pub idempotency_key: Option<String>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Eip7702Auth {
    pub eoa_address: String,
    pub delegate_contract: String,
    pub signature: String,
}

// Request/Response DTOs
#[derive(Deserialize)]
pub struct DepositRequest {
    pub session_id: String,
    pub com_k: String,
    pub amount: u64,
    /// Client-supplied idempotency key (UUID recommended).
    #[serde(default)]
    pub idempotency_key: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct DepositResponse {
    pub status: String,
    pub message: String,
}

#[derive(Deserialize)]
pub struct RevealRequest {
    pub session_id: String,
}

#[derive(Serialize)]
pub struct RevealResponse {
    pub status: String,
    pub valid: bool,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub masking_key_hex: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct SpendResponse {
    pub status: String,
    pub message: String,
    pub queue_position: usize,
    /// Estimated gas charge in USDC (informational, for client-side slippage check)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub estimated_gas_usdc: Option<f64>,
}

#[derive(Deserialize)]
pub struct PrivateSpendQuoteRequest {
    pub merchant_amount: u64,
    #[serde(default)]
    pub estimated_gas_cost: Option<u64>,
    #[serde(default)]
    pub relayer_markup_bps: Option<u64>,
}

#[derive(Serialize)]
pub struct PrivateSpendQuoteResponse {
    pub status: String,
    pub merchant_amount: u64,
    pub contract_amount: u64,
    pub protocol_fee: u64,
    pub gas_cost: u64,
    pub relayer_markup: u64,
    pub execution_fee: u64,
    pub user_total_debit: u64,
    pub fee_bps: u64,
    pub relayer_markup_bps: u64,
    pub message: String,
}

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub queued_transactions: usize,
    pub processed_nullifiers: usize,
    pub relayer_wallet_balance_eth: f64,
    pub relayer_accumulated_profit_usdc: f64,
}

// x402 Protocol DTOs
#[derive(Deserialize)]
pub struct X402VerifyRequest {
    /// Base64-encoded PAYMENT-SIGNATURE header content.
    pub payment_signature_b64: String,
    /// Resource URI being paid for (for logging).
    pub resource_uri: Option<String>,
}

#[derive(Deserialize)]
#[allow(dead_code)]
pub struct X402NimbusPayment {
    pub nullifier: String,
    pub alpha_neg_hex: String,
    pub hm_hex: String,
    pub pk_iss_hex: String,
    #[serde(default)]
    pub amount: u64,
    #[serde(default)]
    pub recipient_or_intent_hash_hex: Option<String>,
    #[serde(default)]
    pub expiry: Option<u64>,
    #[serde(default)]
    pub nonce_hex: Option<String>,
}

#[derive(Deserialize)]
pub struct X402PaymentSignatureInner {
    #[serde(rename = "x402Version")]
    pub x402_version: u32,
    pub scheme: String,
    pub network: String,
    pub payment: X402NimbusPayment,
}

#[derive(Serialize)]
pub struct X402VerifyResponse {
    pub success: bool,
    pub tx_hash: Option<String>,
    pub message: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct SignShareRequest {
    pub session_id: String,
    pub amount: u64,
    pub client_address: String,
    pub com_k_hex: String,
    pub blinded_hex: String,
    pub k_hex: String,
    pub leader_address: String,
    pub timestamp: u64,
    pub signature_hex: String,
}

#[derive(Serialize, Deserialize)]
pub struct SignShareResponse {
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub share_index: Option<u32>,
    pub signature_share_hex: String,
}

#[derive(Deserialize)]
pub struct LeaderSignRequest {
    pub session_id: String,
    pub amount: u64,
    pub client_address: String,
    pub blinded_hex: String,
    pub guardian_urls: Vec<String>,
    pub pk_iss_hex: Option<String>,
}

#[derive(Serialize)]
pub struct PartialSignatureInfo {
    pub index: u32,
    pub signature_hex: String,
}

#[derive(Serialize)]
pub struct LeaderSignResponse {
    pub status: String,
    pub session_id: String,
    pub com_k_hex: String,
    pub partial_signatures: Vec<PartialSignatureInfo>,
}
