mod circuit_breaker;
mod config;
mod database;
mod dto;
mod evm_client;
mod handlers;
mod http;
mod key_rotation;
mod kms;
mod state;

use axum::{
    body::Body,
    extract::DefaultBodyLimit,
    http::{Request, StatusCode},
    middleware::Next,
    response::Response,
    routing::{get, post},
    Router,
};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::OnceLock;
use std::time::Duration;
use std::time::Instant;
use tokio::sync::Mutex;

use config::NetworkConfig;
use database::Database;
use evm_client::EvmClient;
use handlers::*;
use state::AppState;

#[tokio::main]
async fn main() {
    let runtime_mode = config::runtime_mode();
    println!("Nimbus runtime mode: {}", runtime_mode.label());

    // Initialize persistent SQLite database
    let db_path =
        std::env::var("NIMBUS_DB_PATH").unwrap_or_else(|_| "./nimbus-relayer.db".to_string());

    println!("Initializing database at: {}", db_path);
    let db = Database::new(&db_path)
        .await
        .expect("Failed to initialize database");
    println!("Database initialized with WAL mode and production PRAGMAs");

    // Load threshold signature share from KMS
    let (mut share_sk, share_index) = kms::load_share_key().await;
    let issuer_public_key = kms::load_issuer_public_key(&share_sk);
    let guardian_public_keys = kms::load_guardian_public_keys(share_index);

    // Initialize EVM client for real transaction broadcasting (optional for dev mode)
    let evm_client = if let (Ok(rpc_url), Ok(private_key), Ok(contract_addr)) = (
        std::env::var("NIMBUS_RPC_URL"),
        std::env::var("NIMBUS_RELAYER_PRIVATE_KEY"),
        std::env::var("NIMBUS_CONTRACT_ADDRESS"),
    ) {
        match EvmClient::new(&rpc_url, &private_key, &contract_addr).await {
            Ok(client) => {
                println!("EVM client initialized");
                println!("  RPC URL:    {}", rpc_url);
                println!("  Contract:   {}", contract_addr);
                println!("  Signer:     {}", client.signer_address());
                Some(Arc::new(client))
            }
            Err(e) => {
                if runtime_mode.is_strict() {
                    eprintln!("CRITICAL: Failed to initialize EVM client: {}", e);
                    eprintln!(
                        "          {} mode requires working RPC, relayer key, and contract address",
                        runtime_mode.label()
                    );
                    std::process::exit(1);
                }
                eprintln!("WARNING: Failed to initialize EVM client: {}", e);
                eprintln!("         Settlement worker will keep spends retryable (dev mode)");
                None
            }
        }
    } else {
        if runtime_mode.is_strict() {
            eprintln!(
                "CRITICAL: {} mode requires NIMBUS_RPC_URL, NIMBUS_RELAYER_PRIVATE_KEY, and NIMBUS_CONTRACT_ADDRESS",
                runtime_mode.label()
            );
            std::process::exit(1);
        }
        println!("WARNING: EVM client not configured");
        println!(
            "         Set NIMBUS_RPC_URL, NIMBUS_RELAYER_PRIVATE_KEY, NIMBUS_CONTRACT_ADDRESS"
        );
        println!("         Settlement worker will keep spends retryable (dev mode)");
        None
    };

    // Initialize application state with persistent database
    let state = AppState::new(
        db,
        share_sk,
        share_index,
        issuer_public_key,
        guardian_public_keys,
        evm_client,
    )
    .await;

    // Securely zero out the key in the main stack frame immediately
    unsafe {
        std::ptr::write_volatile(&mut share_sk, nimbus_core::Fr::from(0u64));
    }

    // Spawn key rotation background task (Finding #12)
    let _rotation_handle = state.key_manager.clone().spawn_rotation_task();

    // Spawn background worker with adaptive same-chain batching window (Finding #10)
    let worker_state = state.clone();
    tokio::spawn(async move {
        let mut last_process = Instant::now();
        loop {
            let queue_count = worker_state.db.queued_spend_count().await.unwrap_or(0);

            let oldest_age_secs = if queue_count > 0 {
                if let Ok(Some(oldest)) = worker_state.db.get_oldest_queued_spend_timestamp().await
                {
                    let now_secs = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();
                    now_secs.saturating_sub(oldest as u64)
                } else {
                    0
                }
            } else {
                0
            };

            let should_process = if queue_count == 0 {
                false
            } else if queue_count >= 8 {
                // Batch is full, process immediately
                true
            } else if oldest_age_secs >= 3 {
                // Hard timeout reached
                true
            } else if oldest_age_secs >= 2 && last_process.elapsed() >= Duration::from_secs(2) {
                // Target window reached
                true
            } else {
                false
            };

            if should_process {
                process_spend_batch(&worker_state).await;
                last_process = Instant::now();
                // Sleep briefly before next check to avoid CPU spin
                tokio::time::sleep(Duration::from_millis(100)).await;
            } else {
                // Sleep 250ms and check queue status again
                tokio::time::sleep(Duration::from_millis(250)).await;
            }
        }
    });

    // Spawn background cleanup worker (runs daily)
    let cleanup_state = state.clone();
    tokio::spawn(async move {
        loop {
            // Sleep 24 hours
            tokio::time::sleep(Duration::from_secs(86400)).await;

            // Cleanup sessions older than 30 days
            match cleanup_state.db.cleanup_old_sessions(30).await {
                Ok(deleted) => {
                    if deleted > 0 {
                        println!("DATABASE CLEANUP: Deleted {} old sessions", deleted);

                        // Vacuum to reclaim space
                        if let Err(e) = cleanup_state.db.vacuum().await {
                            eprintln!("DATABASE VACUUM failed: {}", e);
                        } else {
                            println!("DATABASE VACUUM: Completed");
                        }

                        // Log database size
                        if let Ok(size) = cleanup_state.db.get_db_size().await {
                            let size_mb = size as f64 / 1_048_576.0;
                            println!("DATABASE SIZE: {:.2} MB", size_mb);
                        }
                    }
                }
                Err(e) => eprintln!("DATABASE CLEANUP failed: {}", e),
            }

            // Cleanup expired idempotency keys (Finding #16)
            match cleanup_state.db.cleanup_idempotency_cache().await {
                Ok(deleted) => {
                    if deleted > 0 {
                        println!("IDEMPOTENCY CLEANUP: Purged {} expired keys", deleted);
                    }
                }
                Err(e) => eprintln!("IDEMPOTENCY CLEANUP failed: {}", e),
            }
        }
    });

    let app = Router::new()
        .route("/health", get(health_check))
        .route("/api/signing-health", get(signing_health))
        .route("/api/deposit", post(handle_deposit))
        .route("/api/reveal", post(handle_reveal))
        .route("/api/spend", post(handle_spend))
        .route("/api/quote/private-spend", get(handle_private_spend_quote))
        .route("/api/x402/verify", post(handle_x402_verify))
        .route("/api/sign-share", post(handle_sign_share))
        .route("/api/leader/sign", post(handle_leader_sign))
        .layer(axum::middleware::from_fn(rate_limit_middleware))
        .layer(DefaultBodyLimit::max(64 * 1024)) // 64KB request body size limit
        .with_state(state.clone());

    let network_config = NetworkConfig::from_env();

    // Validate Tailscale requirement if configured
    if network_config.require_tailscale {
        if !network_config.is_tailscale_bind() {
            eprintln!("ERROR: NIMBUS_REQUIRE_TAILSCALE=true but bind_addr is not a Tailscale IP");
            eprintln!("Current bind_addr: {}", network_config.bind_addr);
            eprintln!("Tailscale IPs typically start with 100.x.x.x or fd7a:115c:a1::");
            std::process::exit(1);
        }
        println!("Tailscale binding: REQUIRED (security hardening)");
    } else if network_config.is_tailscale_bind() {
        println!("Tailscale binding: DETECTED (network security)");
    } else {
        println!("Tailscale binding: NOT CONFIGURED (using localhost only)");
    }

    let listener_addr = network_config.listener_addr();
    let listener = tokio::net::TcpListener::bind(&listener_addr)
        .await
        .unwrap_or_else(|e| {
            eprintln!("ERROR: Failed to bind to {}: {}", listener_addr, e);
            if network_config.is_tailscale_bind() {
                eprintln!("Make sure Tailscale interface is up and IP is correct");
            }
            std::process::exit(1);
        });
    println!("------------------------------------------------------------");
    println!("NIMBUS RELAYER NODE STARTED");
    println!("------------------------------------------------------------");
    println!("Listening on:   http://{}", listener.local_addr().unwrap());
    println!("Bind interface: {}", network_config.bind_addr);
    println!("Database:     {}", db_path);
    println!("Persistence:  ENABLED (SQLite WAL mode)");
    println!("Circuit Breaker: guardian-rpc, vault-kms, blockchain-rpc");
    println!(
        "Key Rotation: {}",
        if std::env::var("NIMBUS_KEY_ROTATION")
            .map(|v| v == "true")
            .unwrap_or(false)
        {
            "ENABLED"
        } else {
            "DISABLED (re-mask only)"
        }
    );
    println!("Idempotency:  ENABLED (24h TTL)");
    println!("Slippage:     ENABLED (min_payout + deadline)");
    println!("Min Balance:  {:.2} ETH", state::MIN_RELAYER_BALANCE_ETH);
    println!("Gasless EIP-7702 delegation: ACTIVE");
    println!("x402 Facilitator endpoint:   ACTIVE");
    println!("Relayer batch queue interval: 2 seconds");

    // Spawn quorum failure monitor background job (DEC-012)
    let monitor_state = state.clone();
    tokio::spawn(async move {
        quorum_failure_monitor(monitor_state).await;
    });

    let auto_refund = std::env::var("NIMBUS_AUTO_REFUND")
        .unwrap_or_else(|_| "false".to_string())
        .parse()
        .unwrap_or(false);
    if auto_refund {
        println!("Quorum Failure Monitor: ACTIVE (auto-refund enabled)");
    } else {
        println!("Quorum Failure Monitor: ACTIVE (monitoring only, auto-refund disabled)");
    }

    println!("------------------------------------------------------------");

    axum::serve(listener, app).await.unwrap();
}

async fn rate_limit_middleware(request: Request<Body>, next: Next) -> Result<Response, StatusCode> {
    // Extract IP address from headers
    let ip = request
        .headers()
        .get("x-forwarded-for")
        .and_then(|val| val.to_str().ok())
        .map(|s| s.split(',').next().unwrap_or(s).trim().to_string())
        .unwrap_or_else(|| "127.0.0.1".to_string());

    static LIMITER: OnceLock<Mutex<HashMap<String, (u32, Instant)>>> = OnceLock::new();
    let limiter = LIMITER.get_or_init(|| Mutex::new(HashMap::new()));

    let mut map = limiter.lock().await;
    let now = Instant::now();

    let (count, last_reset) = map.entry(ip).or_insert((0, now));

    if now.duration_since(*last_reset) > Duration::from_secs(1) {
        *count = 1;
        *last_reset = now;
    } else {
        *count += 1;
        if *count > 10 {
            // Max 10 requests per second
            return Err(StatusCode::TOO_MANY_REQUESTS);
        }
    }

    drop(map); // drop lock before running handler
    Ok(next.run(request).await)
}

#[cfg(test)]
mod node_tests {
    #[tokio::test]
    async fn test_load_share_key_fallback() {
        // Test 32-byte fallback
        std::env::set_var(
            "NIMBUS_SHARE_KEY",
            "0000000000000000000000000000000000000000000000000000000000000001",
        );
        let (fr, index) = crate::kms::load_share_key().await;
        let bytes = nimbus_core::serialize_to_bytes(&fr);
        assert_eq!(bytes[bytes.len() - 1], 1);
        assert_eq!(index, 1);

        // Test 40-byte fallback (index + Fr)
        let share_tuple = (5usize, nimbus_core::Fr::from(1u64));
        let share_bytes = nimbus_core::serialize_to_bytes(&share_tuple);
        let share_hex = hex::encode(share_bytes);
        std::env::set_var("NIMBUS_SHARE_KEY", share_hex);
        let (fr2, index2) = crate::kms::load_share_key().await;
        assert_eq!(index2, 5);
        assert_eq!(fr2, nimbus_core::Fr::from(1u64));

        std::env::remove_var("NIMBUS_SHARE_KEY");
    }

    #[tokio::test]
    async fn test_alloy_signature() {
        use alloy::signers::{local::PrivateKeySigner, Signer};
        let signer = PrivateKeySigner::random();
        let msg = b"hello world";
        let sig = signer.sign_message(msg).await.unwrap();
        let sig_hex = hex::encode(sig.as_bytes());
        let sig_bytes = hex::decode(&sig_hex).unwrap();
        let parsed_sig = alloy::primitives::Signature::try_from(sig_bytes.as_slice()).unwrap();
        let recovered = parsed_sig.recover_address_from_msg(msg).unwrap();
        assert_eq!(recovered, signer.address());
    }

    #[tokio::test]
    async fn test_secure_sign_share() {
        use crate::database::Database;
        use crate::dto::SignShareRequest;
        use crate::handlers::handle_sign_share;
        use crate::state::AppState;
        use alloy::signers::{local::PrivateKeySigner, Signer};
        use nimbus_core::*;
        use tempfile::TempDir;

        std::env::set_var("NIMBUS_DB_KEY", "test-encryption-key");
        std::env::set_var("NIMBUS_ENV", "test");

        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("secure_sign.db");
        let db = Database::new(&db_path).await.unwrap();

        // Generate keys
        let share_sk = Fr::from(1u64);
        let share_index = 1;
        let issuer_public_key = IssuerSecretKey(share_sk).public_key();
        let state = AppState::new(
            db,
            share_sk,
            share_index,
            issuer_public_key,
            std::collections::HashMap::new(),
            None,
        )
        .await;

        // Create leader identity
        let leader_signer = PrivateKeySigner::random();
        let leader_address = format!("0x{:x}", leader_signer.address());

        // Setup payload values
        let session_id = "test-session-123".to_string();
        let amount = 1000u64;
        let client_address = "0x0000000000000000000000000000000000000001".to_string();

        // Setup mock blinded point and masking key
        let (blinded, _blinding_factor) =
            nimbus_core::client_blind(b"test", &mut rand::thread_rng());
        let blinded_hex = hex::encode(serialize_to_bytes(&blinded));
        let k = Fr::from(5u64);
        let k_hex = hex::encode(serialize_to_bytes(&MaskingKey(k)));

        let pk_iss = state.key_manager.get_issuer_public_key().await;
        let com_k = pk_iss.0 * k;
        let com_k_hex = hex::encode(serialize_to_bytes(&MaskingKeyCommitment(com_k)));
        let timestamp = 1234567890u64;

        // 1. Verify that valid signature works
        let message = format!(
            "{}:{}:{}:{}:{}:{}:{}",
            session_id, blinded_hex, k_hex, timestamp, amount, client_address, com_k_hex
        );
        let signature = leader_signer
            .sign_message(message.as_bytes())
            .await
            .unwrap();
        let signature_hex = hex::encode(signature.as_bytes());

        let request = SignShareRequest {
            session_id: session_id.clone(),
            amount,
            client_address: client_address.clone(),
            com_k_hex: com_k_hex.clone(),
            blinded_hex: blinded_hex.clone(),
            k_hex: k_hex.clone(),
            leader_address: leader_address.clone(),
            timestamp,
            signature_hex: signature_hex.clone(),
        };

        // With default dev/test environment, NIMBUS_TRUSTED_LEADERS is bypassed if unset.
        let response = handle_sign_share(
            axum::extract::State(state.clone()),
            axum::Json(request.clone()),
        )
        .await;
        assert_eq!(response.0.status, "SUCCESS");
        assert_eq!(response.0.share_index, Some(share_index));

        // 2. Test duplicate session rejection (replay protection)
        let response_dup = handle_sign_share(
            axum::extract::State(state.clone()),
            axum::Json(request.clone()),
        )
        .await;
        assert_eq!(response_dup.0.status, "ERROR");
        assert!(
            response_dup
                .0
                .signature_share_hex
                .contains("already exists")
                || response_dup.0.signature_share_hex.contains("double sign")
        );

        // 3. Test invalid commitment validation
        let new_session_id = "test-session-456".to_string();
        let wrong_com_k_hex = hex::encode(serialize_to_bytes(&MaskingKeyCommitment(
            pk_iss.0 * Fr::from(999u64),
        )));
        let wrong_msg = format!(
            "{}:{}:{}:{}:{}:{}:{}",
            new_session_id, blinded_hex, k_hex, timestamp, amount, client_address, wrong_com_k_hex
        );
        let wrong_signature = leader_signer
            .sign_message(wrong_msg.as_bytes())
            .await
            .unwrap();
        let wrong_signature_hex = hex::encode(wrong_signature.as_bytes());

        let request_wrong_com = SignShareRequest {
            session_id: new_session_id.clone(),
            amount,
            client_address: client_address.clone(),
            com_k_hex: wrong_com_k_hex.clone(),
            blinded_hex: blinded_hex.clone(),
            k_hex: k_hex.clone(),
            leader_address: leader_address.clone(),
            timestamp,
            signature_hex: wrong_signature_hex,
        };

        let response_wrong_com = handle_sign_share(
            axum::extract::State(state.clone()),
            axum::Json(request_wrong_com),
        )
        .await;
        assert_eq!(response_wrong_com.0.status, "ERROR");
        assert!(response_wrong_com
            .0
            .signature_share_hex
            .contains("does not match pk_iss * k"));

        // 4. Test wrong signature recovery rejection
        let invalid_sig_request = SignShareRequest {
            session_id: "test-session-789".to_string(),
            amount,
            client_address: client_address.clone(),
            com_k_hex: com_k_hex.clone(),
            blinded_hex: blinded_hex.clone(),
            k_hex: k_hex.clone(),
            leader_address: leader_address.clone(),
            timestamp,
            signature_hex: "00".repeat(65), // dummy invalid sig
        };
        let response_invalid_sig = handle_sign_share(
            axum::extract::State(state.clone()),
            axum::Json(invalid_sig_request),
        )
        .await;
        assert_eq!(response_invalid_sig.0.status, "ERROR");
    }
}
