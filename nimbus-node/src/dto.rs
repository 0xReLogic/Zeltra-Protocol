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
    pub association_root_hex: Option<String>,
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
    // --- Signed Quote (EIP-712) ---
    /// Maximum execution fee user is willing to pay (from signed quote)
    #[serde(default)]
    pub max_execution_fee: Option<u64>,
    /// Actual execution fee to pass to contract (from quote response)
    #[serde(default)]
    pub execution_fee: Option<u64>,
    /// Quote ID from /quote endpoint (for replay protection)
    #[serde(default)]
    pub quote_id: Option<String>,
    /// Quote expiry timestamp (Unix seconds)
    #[serde(default)]
    pub quote_expiry: Option<u64>,
    /// EIP-712 signature over ExecutionQuote struct (v, r, s concatenated)
    #[serde(default)]
    pub quote_signature: Option<String>,
    /// User's Ethereum address (signer of the quote)
    #[serde(default)]
    pub user_address: Option<String>,
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
    /// Optional transaction hash of the on-chain deposit for expedited verification.
    #[serde(default)]
    pub tx_hash: Option<String>,
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
    #[serde(default)]
    pub association_root: Option<String>,
    /// Optional CCIP destination chain selector for cross-chain fee quote transparency
    #[serde(default)]
    pub destination_chain_selector: Option<u64>,
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
    /// Transparent fee breakdown: pure local Arbitrum relayer execution gas cost (USDC base units)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relayer_gas_cost: Option<u64>,
    /// Transparent fee breakdown: Chainlink CCIP cross-chain network fee (USDC base units)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ccip_network_fee: Option<u64>,
    /// Destination chain selector if quoting a cross-chain spend
    #[serde(skip_serializing_if = "Option::is_none")]
    pub destination_chain_selector: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fee_tier: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub discount_bps: Option<u64>,
    /// Unique quote ID for replay protection (hex string, 32 bytes)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quote_id: Option<String>,
    /// Quote expiry timestamp (Unix seconds)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quote_expiry: Option<u64>,
    /// EIP-712 domain separator hash (for client verification)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub domain_separator: Option<String>,
    /// EIP-712 struct hash of ExecutionQuote (for client verification)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub struct_hash: Option<String>,
    /// Message to display to user before signing
    pub message: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default, PartialEq)]
pub struct BatchMetricsDto {
    pub batch_count: i64,
    pub total_batch_margin_usdc: f64,
    pub avg_batch_size: f64,
    pub total_execution_fees_usdc: u64,
    pub claimed_execution_fees_usdc: u64,
    pub unclaimed_execution_fees_usdc: u64,
}

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub queued_transactions: usize,
    pub processed_nullifiers: usize,
    pub relayer_wallet_balance_eth: f64,
    pub relayer_accumulated_profit_usdc: f64,
    /// Batch profitability and operational metrics (DEC-020)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub batch_metrics: Option<BatchMetricsDto>,
    /// CCIP cross-chain spends awaiting destination confirmation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ccip_pending_count: Option<i64>,
    /// CCIP cross-chain spends that failed on destination chain
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ccip_failure_count: Option<i64>,
}

#[derive(Serialize)]
pub struct StalledSessionDto {
    pub session_id: String,
    pub amount: u64,
    pub client_address: String,
    pub created_at: i64,
    pub deposit_confirmed: bool,
    pub resolved: bool,
}

#[derive(Serialize)]
pub struct SigningHealthResponse {
    pub stalled_sessions: Vec<StalledSessionDto>,
    pub stalled_count: usize,
    pub check_threshold_seconds: i64,
    pub message: String,
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

// Transaction Status DTOs (DEC-017 / Receipt Finality)
#[derive(Debug, Deserialize)]
pub struct TxStatusQuery {
    pub tx_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxStatusResponse {
    pub tx_hash: String,
    pub status: String, // "pending", "confirmed", "failed", "not_found"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub block_number: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gas_used: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effective_gas_price: Option<u128>,
    pub confirmations: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_reason: Option<String>,
}
