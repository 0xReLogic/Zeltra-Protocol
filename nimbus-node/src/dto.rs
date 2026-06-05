//! Request and Response DTOs for API endpoints

use serde::{Deserialize, Serialize};

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
}

#[derive(Serialize)]
pub struct DepositResponse {
    pub status: String,
    pub message: String,
}

#[derive(Deserialize)]
pub struct RevealRequest {
    pub session_id: String,
    pub masking_key_k: String,
}

#[derive(Serialize)]
pub struct RevealResponse {
    pub status: String,
    pub valid: bool,
    pub message: String,
}

#[derive(Serialize)]
pub struct SpendResponse {
    pub status: String,
    pub message: String,
    pub queue_position: usize,
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

#[derive(Deserialize, Serialize)]
pub struct SignShareRequest {
    pub blinded_hex: String,
    pub k_hex: String,
    pub share_sk_hex: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct SignShareResponse {
    pub status: String,
    pub signature_share_hex: String,
}

#[derive(Deserialize)]
pub struct LeaderSignRequest {
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
    pub com_k_hex: String,
    pub k_hex: String,
    pub partial_signatures: Vec<PartialSignatureInfo>,
}
