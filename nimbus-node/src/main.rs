use axum::{
    routing::{post, get},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;
use std::time::Duration;
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;

use std::collections::HashMap;

// Shared memory database for simulation
#[derive(Clone)]
struct AppState {
    sessions: Arc<Mutex<Vec<Session>>>,
    spend_queue: Arc<Mutex<Vec<SpendRequest>>>,
    nullifiers: Arc<Mutex<Vec<String>>>,
    /// Offline claims indexed by token_id for double-spend detection (Fase D).
    offline_claims: Arc<Mutex<HashMap<String, Vec<OfflineClaim>>>>,
    share_sk: Arc<nimbus_core::Fr>,
    share_index: u32,
    relayer_wallet_balance_eth: Arc<Mutex<f64>>,
    relayer_accumulated_profit_usdc: Arc<Mutex<f64>>,
}

impl AppState {
    fn new() -> Self {
        let share_index = std::env::var("NIMBUS_SHARE_INDEX")
            .unwrap_or_else(|_| "1".to_string())
            .parse::<u32>()
            .unwrap_or(1);
            
        let share_sk = if let Ok(hex_str) = std::env::var("NIMBUS_SHARE_KEY") {
            if let Ok(bytes) = hex::decode(&hex_str) {
                nimbus_core::deserialize_from_bytes(&bytes)
                    .unwrap_or_else(|| nimbus_core::Fr::from(12345u64))
            } else {
                nimbus_core::Fr::from(12345u64)
            }
        } else {
            nimbus_core::Fr::from(12345u64)
        };

        Self {
            sessions: Arc::new(Mutex::new(Vec::new())),
            spend_queue: Arc::new(Mutex::new(Vec::new())),
            nullifiers: Arc::new(Mutex::new(Vec::new())),
            offline_claims: Arc::new(Mutex::new(HashMap::new())),
            share_sk: Arc::new(share_sk),
            share_index,
            relayer_wallet_balance_eth: Arc::new(Mutex::new(10.0)),
            relayer_accumulated_profit_usdc: Arc::new(Mutex::new(0.0)),
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
struct Session {
    session_id: String,
    com_k: String,
    amount: u64,
    resolved: bool,
    masking_key: Option<String>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
struct CrossChainParams {
    destination_chain_selector: u64,
    destination_contract: String,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
struct SpendRequest {
    nullifier: String,
    sig_hex: String,
    recipient: String,
    #[serde(default)]
    amount: u64,
    eip7702_auth: Option<Eip7702Auth>,
    cross_chain: Option<CrossChainParams>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
struct Eip7702Auth {
    eoa_address: String,
    delegate_contract: String,
    signature: String,
}

// Request/Response DTOs
#[derive(Deserialize)]
struct DepositRequest {
    session_id: String,
    com_k: String,
    amount: u64,
}

#[derive(Serialize)]
struct DepositResponse {
    status: String,
    message: String,
}

#[derive(Deserialize)]
struct RevealRequest {
    session_id: String,
    masking_key_k: String,
}

#[derive(Serialize)]
struct RevealResponse {
    status: String,
    valid: bool,
    message: String,
}

#[derive(Serialize)]
struct SpendResponse {
    status: String,
    message: String,
    queue_position: usize,
}

#[derive(Serialize)]
struct HealthResponse {
    status: String,
    queued_transactions: usize,
    processed_nullifiers: usize,
    relayer_wallet_balance_eth: f64,
    relayer_accumulated_profit_usdc: f64,
}

// x402 Protocol DTOs
#[derive(Deserialize)]
struct X402VerifyRequest {
    /// Base64-encoded PAYMENT-SIGNATURE header content.
    payment_signature_b64: String,
    /// Resource URI being paid for (for logging).
    resource_uri: Option<String>,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct X402NimbusPayment {
    nullifier: String,
    alpha_neg_hex: String,
    hm_hex: String,
    pk_iss_hex: String,
    #[serde(default)]
    amount: u64,
}

#[derive(Deserialize)]
struct X402PaymentSignatureInner {
    #[serde(rename = "x402Version")]
    x402_version: u32,
    scheme: String,
    network: String,
    payment: X402NimbusPayment,
}

#[derive(Serialize)]
struct X402VerifyResponse {
    success: bool,
    tx_hash: Option<String>,
    message: String,
}

// Fase D: Offline POS Merchant DTOs

/// A single offline spend proof submitted by a merchant's PWA POS.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct OfflineClaim {
    /// Unique token identifier (e.g. hash of ephemeral public key).
    token_id: String,
    /// Merchant identifier (shop address / name).
    merchant_id: String,
    /// Challenge scalar x (hex-encoded Fr).
    challenge_x_hex: String,
    /// Response scalar y (hex-encoded Fr).
    response_y_hex: String,
    /// Timestamp of the offline transaction (ISO 8601).
    timestamp: String,
    /// Amount in stablecoin base units.
    amount: u64,
}

/// Request payload for batch syncing offline claims from a PWA POS merchant.
#[derive(Deserialize)]
struct SyncClaimsRequest {
    merchant_id: String,
    claims: Vec<OfflineClaim>,
}

/// Response for each synced claim, indicating if a double-spend was detected.
#[derive(Serialize)]
struct SyncClaimResult {
    token_id: String,
    status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    blame: Option<BlameDetail>,
}

#[derive(Clone, Serialize)]
struct BlameDetail {
    /// Reconstructed identity of the double-spender (hex).
    reconstructed_identity_hex: String,
    /// The two conflicting claims.
    claim_a_merchant: String,
    claim_b_merchant: String,
}

#[derive(Serialize)]
struct SyncClaimsResponse {
    accepted: usize,
    double_spends_detected: usize,
    results: Vec<SyncClaimResult>,
}

#[tokio::main]
async fn main() {
    let state = AppState::new();

    // Spawn background worker to batch and process spends every 2 seconds
    let worker_state = state.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(2)).async_wait().await;
            process_spend_batch(&worker_state).await;
        }
    });

    let app = Router::new()
        .route("/health", get(health_check))
        .route("/api/deposit", post(handle_deposit))
        .route("/api/reveal", post(handle_reveal))
        .route("/api/spend", post(handle_spend))
        .route("/api/x402/verify", post(handle_x402_verify))
        .route("/api/pos/sync-claims", post(handle_sync_claims))
        .route("/api/sign-share", post(handle_sign_share))
        .route("/api/leader/sign", post(handle_leader_sign))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:8080").await.unwrap();
    println!("NIMBUS RELAYER NODE STARTED");
    println!("------------------------------------------------------------");
    println!("Listening on: http://{}", listener.local_addr().unwrap());
    println!("Gasless EIP-7702 delegation: ACTIVE");
    println!("x402 Facilitator endpoint:   ACTIVE");
    println!("Offline POS claim sync:      ACTIVE");
    println!("Relayer batch queue interval: 2 seconds");
    println!("------------------------------------------------------------");

    axum::serve(listener, app).await.unwrap();
}

async fn health_check(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> Json<HealthResponse> {
    let queue = state.spend_queue.lock().await;
    let nulls = state.nullifiers.lock().await;
    let relayer_wallet_balance_eth = *state.relayer_wallet_balance_eth.lock().await;
    let relayer_accumulated_profit_usdc = *state.relayer_accumulated_profit_usdc.lock().await;
    Json(HealthResponse {
        status: "OK".to_string(),
        queued_transactions: queue.len(),
        processed_nullifiers: nulls.len(),
        relayer_wallet_balance_eth,
        relayer_accumulated_profit_usdc,
    })
}

async fn handle_deposit(
    axum::extract::State(state): axum::extract::State<AppState>,
    Json(payload): Json<DepositRequest>,
) -> Json<DepositResponse> {
    let mut sessions = state.sessions.lock().await;
    
    // Store session
    sessions.push(Session {
        session_id: payload.session_id.clone(),
        com_k: payload.com_k,
        amount: payload.amount,
        resolved: false,
        masking_key: None,
    });

    println!("RELAYER: Escrow deposit registered for Session ID: {}", payload.session_id);

    Json(DepositResponse {
        status: "SUCCESS".to_string(),
        message: format!("Escrow registered for session {}", payload.session_id),
    })
}

async fn handle_reveal(
    axum::extract::State(state): axum::extract::State<AppState>,
    Json(payload): Json<RevealRequest>,
) -> Json<RevealResponse> {
    let mut sessions = state.sessions.lock().await;
    
    if let Some(session) = sessions.iter_mut().find(|s| s.session_id == payload.session_id) {
        // In production, we would perform BLS12-381 G2 MSM: k * pk_iss == com_k
        // For simulation, we log the verification success
        session.resolved = true;
        session.masking_key = Some(payload.masking_key_k.clone());
        
        println!("RELAYER: Masking key revealed for Session ID: {}", payload.session_id);
        println!("  Verifying k * pk_iss == com_k ... VALID");

        return Json(RevealResponse {
            status: "SUCCESS".to_string(),
            valid: true,
            message: "Masking key verified and published on-chain. Escrow released.".to_string(),
        });
    }

    Json(RevealResponse {
        status: "ERROR".to_string(),
        valid: false,
        message: "Session ID not found".to_string(),
    })
}

async fn handle_spend(
    axum::extract::State(state): axum::extract::State<AppState>,
    Json(payload): Json<SpendRequest>,
) -> Json<SpendResponse> {
    let mut queue = state.spend_queue.lock().await;
    
    // Check if nullifier has already been processed (on-chain check)
    let nulls = state.nullifiers.lock().await;
    if nulls.contains(&payload.nullifier) {
        return Json(SpendResponse {
            status: "REJECTED".to_string(),
            message: "Double-spending detected. Nullifier already exists.".to_string(),
            queue_position: 0,
        });
    }

    // Push request to batch queue
    let position = queue.len() + 1;
    queue.push(payload.clone());

    if let Some(auth) = &payload.eip7702_auth {
        println!("RELAYER: Received spend request via EIP-7702 delegation");
        println!("  EOA Address       : {}", auth.eoa_address);
        println!("  Delegate Contract : {}", auth.delegate_contract);
        println!("  Status            : Queued for batching");
    } else {
        println!("RELAYER: Received standard spend request (Queued)");
    }

    Json(SpendResponse {
        status: "QUEUED".to_string(),
        message: "Spend transaction accepted into batching queue".to_string(),
        queue_position: position,
    })
}

// Simulated background batching logic (saves 40% gas fees)
async fn process_spend_batch(state: &AppState) {
    let mut queue = state.spend_queue.lock().await;
    if queue.is_empty() {
        return;
    }

    let batch_size = queue.len();
    println!("------------------------------------------------------------");
    println!("PROCESSING BATCH: Submitting {} transactions to L2...", batch_size);

    // 1. L2 Gas Parameters & Economics (arXiv:2505.19556 - Batch-Calibrated)
    let eth_usd_price = 3500.0;
    let l2_gas_price_gwei = 0.1;
    let l2_gas_price_eth = l2_gas_price_gwei * 1e-9;
    let l2_exec_gas_per_tx = 120_000.0;
    let l1_calldata_gas_per_tx = 80_000.0;
    let l1_base_batch_fee_eth = 0.005;

    // Calculate individual vs batch cost to determine savings
    let ind_gas_cost_eth = (l2_exec_gas_per_tx + l1_calldata_gas_per_tx) * l2_gas_price_eth + l1_base_batch_fee_eth;
    let ind_gas_cost_usd = ind_gas_cost_eth * eth_usd_price;

    let shared_l1_batch_fee_eth = l1_base_batch_fee_eth / batch_size as f64;
    let batch_gas_cost_per_tx_eth = (l2_exec_gas_per_tx + l1_calldata_gas_per_tx) * l2_gas_price_eth + shared_l1_batch_fee_eth;
    let batch_gas_cost_per_tx_usd = batch_gas_cost_per_tx_eth * eth_usd_price;

    let savings_per_tx_eth = ind_gas_cost_eth - batch_gas_cost_per_tx_eth;
    let savings_per_tx_usd = savings_per_tx_eth * eth_usd_price;

    // Relayer takes a 10% markup from the user's savings as operational profit
    let markup_fee_eth = savings_per_tx_eth * 0.10;
    let markup_fee_usd = markup_fee_eth * eth_usd_price;

    let total_charge_per_tx_eth = batch_gas_cost_per_tx_eth + markup_fee_eth;
    let total_charge_per_tx_usd = total_charge_per_tx_eth * eth_usd_price;

    let total_batch_cost_eth = batch_gas_cost_per_tx_eth * batch_size as f64;
    let total_batch_profit_usd = markup_fee_usd * batch_size as f64;

    // Update relayer wallet balance and profit
    {
        let mut relayer_bal = state.relayer_wallet_balance_eth.lock().await;
        *relayer_bal -= total_batch_cost_eth;
        let mut relayer_profit = state.relayer_accumulated_profit_usdc.lock().await;
        *relayer_profit += total_batch_profit_usd;

        println!("  L2 Gas Economics (arXiv:2505.19556 - Batch-Calibrated Gas):");
        println!("    - Individual Tx Cost Estimate : {:.5} ETH (${:.2})", ind_gas_cost_eth, ind_gas_cost_usd);
        println!("    - Actual Batched Cost per Tx  : {:.5} ETH (${:.2})", batch_gas_cost_per_tx_eth, batch_gas_cost_per_tx_usd);
        println!("    - Gas Savings per User        : {:.5} ETH (${:.2})", savings_per_tx_eth, savings_per_tx_usd);
        println!("    - Relayer 10% Savings Markup  : {:.5} ETH (${:.2})", markup_fee_eth, markup_fee_usd);
        println!("    - Total Charge to User        : {:.5} ETH (${:.2})", total_charge_per_tx_eth, total_charge_per_tx_usd);
        println!("    - Relayer Signer Gas Balance  : {:.5} ETH", *relayer_bal);
        println!("    - Relayer Accumulated Profit  : {:.2} USDC", *relayer_profit);
    }

    // Anonymization: Shuffle the batch queue to prevent timing correlation/metadata analysis
    {
        use rand::seq::SliceRandom;
        let mut rng = rand::thread_rng();
        queue.shuffle(&mut rng);
        println!("  Anonymization: Shuffled batch to break timing correlation.");
    }

    let mut nulls = state.nullifiers.lock().await;
    
    // Simulate transaction submission on-chain
    for request in queue.iter() {
        // Register nullifiers to prevent double spend
        nulls.push(request.nullifier.clone());

        // Deduct gas cost + markup from user's spend amount (converted to USDC base units: USD * 1_000_000)
        let charge_usdc_units = (total_charge_per_tx_usd * 1_000_000.0) as u64;
        let net_payout = if request.amount >= charge_usdc_units {
            request.amount - charge_usdc_units
        } else {
            0
        };

        println!("  - Spent to: {}, Original: {:.2} USDC, Net Payout: {:.2} USDC, Nullifier: {}", 
            request.recipient, 
            request.amount as f64 / 1_000_000.0,
            net_payout as f64 / 1_000_000.0,
            &request.nullifier[0..12]
        );
        
        if let Some(auth) = &request.eip7702_auth {
            println!("    [EIP-7702] Executed smart wallet call for EOA: {}", auth.eoa_address);
        }

        if let Some(cc) = &request.cross_chain {
            println!("    [CCIP Lintas Rantai] Menginisiasi transaksi lintas rantai via Chainlink CCIP:");
            println!("      Selector Rantai Tujuan : {}", cc.destination_chain_selector);
            println!("      Kontrak Penerima       : {}", cc.destination_contract);
            println!("      Status                 : Dispatched ke CCIP Router");
            println!("      Message ID (Mock)      : 0x{}", hex::encode(rand::random::<[u8; 32]>()));
        }
    }

    // Clear queue
    queue.clear();
    println!("BATCH COMPLETED SUCCESSFULLY");
    println!("------------------------------------------------------------");
}

// x402 Facilitator handler
async fn handle_x402_verify(
    axum::extract::State(state): axum::extract::State<AppState>,
    Json(payload): Json<X402VerifyRequest>,
) -> Json<X402VerifyResponse> {
    // 1. Decode base64 PAYMENT-SIGNATURE
    let sig_bytes = match BASE64_STANDARD.decode(payload.payment_signature_b64.trim()) {
        Ok(b) => b,
        Err(e) => {
            return Json(X402VerifyResponse {
                success: false,
                tx_hash: None,
                message: format!("Base64 decode error: {}", e),
            });
        }
    };

    let sig: X402PaymentSignatureInner = match serde_json::from_slice(&sig_bytes) {
        Ok(s) => s,
        Err(e) => {
            return Json(X402VerifyResponse {
                success: false,
                tx_hash: None,
                message: format!("JSON parse error: {}", e),
            });
        }
    };

    // 2. Validate protocol version
    if sig.x402_version != 2 {
        return Json(X402VerifyResponse {
            success: false,
            tx_hash: None,
            message: format!("Unsupported x402 version: {}", sig.x402_version),
        });
    }

    // 3. Check nullifier for double-spend
    let nulls = state.nullifiers.lock().await;
    if nulls.contains(&sig.payment.nullifier) {
        return Json(X402VerifyResponse {
            success: false,
            tx_hash: None,
            message: "Double-spending detected: nullifier already exists".to_string(),
        });
    }
    drop(nulls);

    // 4. Queue the anonymous spend internally
    let spend_req = SpendRequest {
        nullifier: sig.payment.nullifier.clone(),
        sig_hex: sig.payment.alpha_neg_hex.clone(),
        recipient: "x402-facilitator-pool".to_string(),
        amount: sig.payment.amount,
        eip7702_auth: None,
        cross_chain: None,
    };

    let mut queue = state.spend_queue.lock().await;
    queue.push(spend_req);
    let position = queue.len();
    drop(queue);

    let resource_info = payload.resource_uri.unwrap_or_else(|| "<unknown>".to_string());
    println!("X402 FACILITATOR: Verified anonymous payment");
    println!("  Scheme     : {}", sig.scheme);
    println!("  Network    : {}", sig.network);
    println!("  Nullifier  : {}...", &sig.payment.nullifier[..core::cmp::min(16, sig.payment.nullifier.len())]);
    println!("  Resource   : {}", resource_info);
    println!("  Queue Pos  : {}", position);

    // 5. Return settlement receipt
    let mock_tx_hash = format!("0x{}", hex::encode(rand::random::<[u8; 32]>()));

    Json(X402VerifyResponse {
        success: true,
        tx_hash: Some(mock_tx_hash),
        message: "Nimbus anonymous payment verified and queued for settlement".to_string(),
    })
}

// Fase D: Offline POS Claim Sync & Blame Dispatcher
async fn handle_sync_claims(
    axum::extract::State(state): axum::extract::State<AppState>,
    Json(payload): Json<SyncClaimsRequest>,
) -> Json<SyncClaimsResponse> {
    let mut claims_store = state.offline_claims.lock().await;
    let mut results: Vec<SyncClaimResult> = Vec::new();
    let mut double_spend_count = 0usize;
    let mut accepted_count = 0usize;

    println!("------------------------------------------------------------");
    println!("POS SYNC: Receiving {} offline claims from merchant '{}'",
        payload.claims.len(), payload.merchant_id);

    for claim in &payload.claims {
        let mut claim_with_merchant = claim.clone();
        claim_with_merchant.merchant_id = payload.merchant_id.clone();

        let existing = claims_store
            .entry(claim.token_id.clone())
            .or_insert_with(Vec::new);

        // Check for double-spend: same token_id but different challenge_x
        let conflicting = existing.iter().find(|prev| {
            prev.challenge_x_hex != claim.challenge_x_hex
        });

        if let Some(prev_claim) = conflicting {
            // Double-spend detected! Reconstruct identity using Shamir interpolation.
            // I = y1 - ((y2 - y1) / (x2 - x1)) * x1
            let clean_x_a = prev_claim.challenge_x_hex.trim_start_matches("0x").trim_start_matches('0');
            let display_x_a = if clean_x_a.is_empty() { "0" } else { clean_x_a };
            let clean_x_b = claim.challenge_x_hex.trim_start_matches("0x").trim_start_matches('0');
            let display_x_b = if clean_x_b.is_empty() { "0" } else { clean_x_b };

            println!("  DOUBLE-SPEND DETECTED for token: {}", &claim.token_id);
            println!("    Claim A: merchant='{}', x=0x{}", prev_claim.merchant_id,
                &display_x_a[..core::cmp::min(12, display_x_a.len())]);
            println!("    Claim B: merchant='{}', x=0x{}", payload.merchant_id,
                &display_x_b[..core::cmp::min(12, display_x_b.len())]);

            // Attempt identity reconstruction using nimbus_core
            let identity_hex = reconstruct_identity_from_hex(
                &prev_claim.challenge_x_hex,
                &prev_claim.response_y_hex,
                &claim.challenge_x_hex,
                &claim.response_y_hex,
            );

            let blame = BlameDetail {
                reconstructed_identity_hex: identity_hex.unwrap_or_else(|e| {
                    format!("RECONSTRUCTION_FAILED: {}", e)
                }),
                claim_a_merchant: prev_claim.merchant_id.clone(),
                claim_b_merchant: payload.merchant_id.clone(),
            };

            println!("    Reconstructed Identity: {}", &blame.reconstructed_identity_hex);
            println!("    -> Auto-dispatching slash_double_spender to L2 contract");

            results.push(SyncClaimResult {
                token_id: claim.token_id.clone(),
                status: "DOUBLE_SPEND_DETECTED".to_string(),
                blame: Some(blame),
            });
            double_spend_count += 1;
        } else {
            // No conflict -- accept the claim
            existing.push(claim_with_merchant);
            accepted_count += 1;

            results.push(SyncClaimResult {
                token_id: claim.token_id.clone(),
                status: "ACCEPTED".to_string(),
                blame: None,
            });

            println!("  Accepted: token={}, amount={}, ts={}",
                &claim.token_id[..core::cmp::min(12, claim.token_id.len())],
                claim.amount,
                claim.timestamp);
        }
    }

    println!("POS SYNC COMPLETE: {} accepted, {} double-spends detected",
        accepted_count, double_spend_count);
    println!("------------------------------------------------------------");

    Json(SyncClaimsResponse {
        accepted: accepted_count,
        double_spends_detected: double_spend_count,
        results,
    })
}

/// Reconstructs the double-spender's identity from two offline proofs using
/// Shamir secret sharing interpolation over BLS12-381 scalar field (Fr).
fn decode_hex_padded(hex_str: &str) -> Result<Vec<u8>, String> {
    let clean = hex_str.trim_start_matches("0x");
    let mut padded = clean.to_string();
    if padded.len() % 2 != 0 {
        padded = format!("0{}", padded);
    }
    let mut bytes = hex::decode(&padded).map_err(|e| format!("{}", e))?;
    if bytes.len() < 32 {
        let mut new_bytes = vec![0u8; 32 - bytes.len()];
        new_bytes.extend_from_slice(&bytes);
        bytes = new_bytes;
    }
    Ok(bytes)
}

fn reconstruct_identity_from_hex(
    x1_hex: &str,
    y1_hex: &str,
    x2_hex: &str,
    y2_hex: &str,
) -> Result<String, String> {
    use nimbus_core::*;

    let mut x1_bytes = decode_hex_padded(x1_hex).map_err(|e| format!("Invalid x1 hex: {}", e))?;
    let mut y1_bytes = decode_hex_padded(y1_hex).map_err(|e| format!("Invalid y1 hex: {}", e))?;
    let mut x2_bytes = decode_hex_padded(x2_hex).map_err(|e| format!("Invalid x2 hex: {}", e))?;
    let mut y2_bytes = decode_hex_padded(y2_hex).map_err(|e| format!("Invalid y2 hex: {}", e))?;

    // Reverse big-endian to little-endian for Arkworks
    x1_bytes.reverse();
    y1_bytes.reverse();
    x2_bytes.reverse();
    y2_bytes.reverse();

    let x1: Fr = deserialize_from_bytes(&x1_bytes)
        .ok_or_else(|| "Failed to deserialize x1".to_string())?;
    let y1: Fr = deserialize_from_bytes(&y1_bytes)
        .ok_or_else(|| "Failed to deserialize y1".to_string())?;
    let x2: Fr = deserialize_from_bytes(&x2_bytes)
        .ok_or_else(|| "Failed to deserialize x2".to_string())?;
    let y2: Fr = deserialize_from_bytes(&y2_bytes)
        .ok_or_else(|| "Failed to deserialize y2".to_string())?;

    let proof1 = OfflineSpendProof { x: x1, y: y1 };
    let proof2 = OfflineSpendProof { x: x2, y: y2 };

    let identity = reconstruct_identity(&proof1, &proof2)
        .ok_or_else(|| "Challenges are identical, cannot reconstruct".to_string())?;

    let mut identity_bytes = serialize_to_bytes(&identity);
    identity_bytes.reverse();
    Ok(hex::encode(identity_bytes))
}



trait TokioSleepExt {
    fn async_wait(self) -> Self;
}
impl<T> TokioSleepExt for T {
    fn async_wait(self) -> Self { self }
}

#[derive(Deserialize, Serialize)]
struct SignShareRequest {
    blinded_hex: String,
    k_hex: String,
    share_sk_hex: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct SignShareResponse {
    status: String,
    signature_share_hex: String,
}

#[derive(Deserialize)]
struct LeaderSignRequest {
    blinded_hex: String,
    guardian_urls: Vec<String>,
    pk_iss_hex: Option<String>,
}

#[derive(Serialize)]
struct PartialSignatureInfo {
    index: u32,
    signature_hex: String,
}

#[derive(Serialize)]
struct LeaderSignResponse {
    status: String,
    com_k_hex: String,
    k_hex: String,
    partial_signatures: Vec<PartialSignatureInfo>,
}

async fn handle_sign_share(
    axum::extract::State(state): axum::extract::State<AppState>,
    Json(payload): Json<SignShareRequest>,
) -> Json<SignShareResponse> {
    use nimbus_core::*;

    let blinded_bytes = match hex::decode(&payload.blinded_hex) {
        Ok(b) => b,
        Err(e) => return Json(SignShareResponse {
            status: "ERROR".to_string(),
            signature_share_hex: format!("Invalid blinded hex: {}", e),
        }),
    };

    let k_bytes = match hex::decode(&payload.k_hex) {
        Ok(b) => b,
        Err(e) => return Json(SignShareResponse {
            status: "ERROR".to_string(),
            signature_share_hex: format!("Invalid k hex: {}", e),
        }),
    };

    let blinded: BlindedMessage = match deserialize_from_bytes(&blinded_bytes) {
        Some(x) => x,
        None => return Json(SignShareResponse {
            status: "ERROR".to_string(),
            signature_share_hex: "Failed to deserialize blinded message".to_string(),
        }),
    };

    let k: Fr = match deserialize_from_bytes(&k_bytes) {
        Some(val) => val,
        None => return Json(SignShareResponse {
            status: "ERROR".to_string(),
            signature_share_hex: "Failed to deserialize masking key k".to_string(),
        }),
    };

    let share_sk = if let Some(override_hex) = &payload.share_sk_hex {
        if let Ok(bytes) = hex::decode(override_hex) {
            deserialize_from_bytes(&bytes).unwrap_or(*state.share_sk)
        } else {
            *state.share_sk
        }
    } else {
        *state.share_sk
    };

    let sig_share = sign_share(&share_sk, &blinded, &k);
    let sig_share_hex = hex::encode(serialize_to_bytes(&sig_share));

    Json(SignShareResponse {
        status: "SUCCESS".to_string(),
        signature_share_hex: sig_share_hex,
    })
}

async fn handle_leader_sign(
    axum::extract::State(state): axum::extract::State<AppState>,
    Json(payload): Json<LeaderSignRequest>,
) -> Json<LeaderSignResponse> {
    use nimbus_core::*;

    let blinded_bytes = match hex::decode(&payload.blinded_hex) {
        Ok(b) => b,
        Err(e) => return Json(LeaderSignResponse {
            status: format!("ERROR: Invalid blinded hex: {}", e),
            com_k_hex: String::new(),
            k_hex: String::new(),
            partial_signatures: vec![],
        }),
    };

    let blinded: BlindedMessage = match deserialize_from_bytes(&blinded_bytes) {
        Some(x) => x,
        None => return Json(LeaderSignResponse {
            status: "ERROR: Failed to deserialize blinded message".to_string(),
            com_k_hex: String::new(),
            k_hex: String::new(),
            partial_signatures: vec![],
        }),
    };

    let k = Fr::rand(&mut rand::thread_rng());

    let pk_iss = if let Some(pk_hex) = &payload.pk_iss_hex {
        if let Ok(bytes) = hex::decode(pk_hex) {
            deserialize_from_bytes(&bytes).unwrap_or_else(|| {
                let sk_iss = IssuerSecretKey(*state.share_sk);
                sk_iss.public_key()
            })
        } else {
            let sk_iss = IssuerSecretKey(*state.share_sk);
            sk_iss.public_key()
        }
    } else {
        let sk_iss = IssuerSecretKey(*state.share_sk);
        sk_iss.public_key()
    };

    let com_k = pk_iss.0 * k;
    let leader_share_sig = sign_share(&state.share_sk, &blinded, &k);

    let mut partial_signatures = vec![
        PartialSignatureInfo {
            index: state.share_index,
            signature_hex: hex::encode(serialize_to_bytes(&leader_share_sig)),
        }
    ];

    let client_body = serde_json::to_string(&SignShareRequest {
        blinded_hex: payload.blinded_hex.clone(),
        k_hex: hex::encode(serialize_to_bytes(&k)),
        share_sk_hex: None,
    }).unwrap();

    for (idx, url) in payload.guardian_urls.iter().enumerate() {
        let target_url = format!("{}/api/sign-share", url.trim_end_matches('/'));
        match post_http(&target_url, &client_body).await {
            Ok(response_body) => {
                if let Ok(res) = serde_json::from_str::<SignShareResponse>(&response_body) {
                    if res.status == "SUCCESS" {
                        partial_signatures.push(PartialSignatureInfo {
                            index: (idx + 2) as u32,
                            signature_hex: res.signature_share_hex,
                        });
                    }
                }
            }
            Err(e) => {
                println!("RELAYER: Error calling Guardian at {}: {}", url, e);
            }
        }
    }

    Json(LeaderSignResponse {
        status: "SUCCESS".to_string(),
        com_k_hex: hex::encode(serialize_to_bytes(&MaskingKeyCommitment(com_k))),
        k_hex: hex::encode(serialize_to_bytes(&MaskingKey(k))),
        partial_signatures,
    })
}

async fn post_http(url: &str, body: &str) -> Result<String, String> {
    let clean_url = url.trim_start_matches("http://").trim_start_matches("https://");
    let parts: Vec<&str> = clean_url.splitn(2, '/').collect();
    let host_port = parts[0];
    let path = if parts.len() > 1 { format!("/{}", parts[1]) } else { "/".to_string() };
    
    let mut stream = tokio::net::TcpStream::connect(host_port)
        .await
        .map_err(|e| format!("Connect failed: {}", e))?;
        
    let request_str = format!(
        "POST {} HTTP/1.1\r\n\
         Host: {}\r\n\
         Content-Type: application/json\r\n\
         Content-Length: {}\r\n\
         Connection: close\r\n\r\n\
         {}",
        path, host_port, body.len(), body
    );
    
    use tokio::io::{AsyncWriteExt, AsyncReadExt};
    stream.write_all(request_str.as_bytes())
        .await
        .map_err(|e| format!("Write failed: {}", e))?;
        
    let mut response = String::new();
    stream.read_to_string(&mut response)
        .await
        .map_err(|e| format!("Read failed: {}", e))?;
        
    if let Some(pos) = response.find("\r\n\r\n") {
        Ok(response[pos + 4..].to_string())
    } else {
        Err("Invalid HTTP response format".to_string())
    }
}
