mod ccip_monitor;
mod config;
mod database;
mod dto;
mod evm_client;
mod execution_fee_claimer;
mod handlers;
mod mempool_watchdog;
mod mmr_indexer;
mod sanctions_sync;
mod state;
mod validation;

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

    // Initialize EVM client for real transaction broadcasting (optional for dev mode)
    let evm_client = if let (Ok(rpc_url), Ok(private_key), Ok(contract_addr)) = (
        std::env::var("NIMBUS_RPC_URL"),
        std::env::var("NIMBUS_RELAYER_PRIVATE_KEY"),
        std::env::var("NIMBUS_CONTRACT_ADDRESS"),
    ) {
        match EvmClient::new(&rpc_url, &private_key, &contract_addr).await {
            Ok(client) => {
                let client = client.with_db(Arc::new(db.clone()));
                println!("EVM client initialized");
                println!("  RPC URL:    {}", rpc_url);
                println!("  Contract:   {}", contract_addr);
                println!("  Signer:     {}", client.signer_address());
                println!(
                    "  Finality:   {} block(s) threshold, {}s timeout, +{}% gas bump",
                    client.finality_config().confirmation_threshold,
                    client.finality_config().tx_timeout_secs,
                    client.finality_config().gas_bump_percent
                );
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
    let state = AppState::new(db, evm_client).await;

    // --- Post-Restart Nullifier Reconciliation (DEC-019) ---
    if let Some(ref evm) = state.evm_client {
        println!(
            "RECONCILIATION: Checking active/in-flight spends against on-chain contract state..."
        );
        match state.db.get_unreconciled_spends().await {
            Ok(unreconciled) if !unreconciled.is_empty() => {
                println!(
                    "RECONCILIATION: Found {} active/in-flight spend(s) to verify.",
                    unreconciled.len()
                );
                for spend in unreconciled {
                    match evm.is_nullifier_spent(&spend.nullifier).await {
                        Ok(true) => {
                            println!(
                                "RECONCILIATION: Spend {} (nullifier {}...) was ALREADY SPENT on-chain! Marking local spend as confirmed.",
                                spend.id,
                                &spend.nullifier[..8.min(spend.nullifier.len())]
                            );
                            if let Err(e) = state
                                .db
                                .reconcile_spend_as_confirmed(
                                    spend.id,
                                    &spend.nullifier,
                                    spend.tx_hash.as_deref(),
                                )
                                .await
                            {
                                eprintln!(
                                    "RECONCILIATION ERROR: Failed to confirm spend {}: {}",
                                    spend.id, e
                                );
                            }
                        }
                        Ok(false) => {
                            if spend.status == "broadcasting" || spend.status == "submitted" {
                                println!(
                                    "RECONCILIATION: Spend {} in '{}' state not yet spent on-chain. Resetting lease for retry.",
                                    spend.id, spend.status
                                );
                                let _ = state.db.reset_unspent_lease(spend.id).await;
                            }
                        }
                        Err(e) => {
                            eprintln!(
                                "RECONCILIATION WARNING: Failed to query on-chain nullifier status for spend {}: {}",
                                spend.id, e
                            );
                        }
                    }
                }
                println!("RECONCILIATION: Startup check complete.");
            }
            Ok(_) => {
                println!("RECONCILIATION: No active/in-flight spends found in queue.");
            }
            Err(e) => {
                eprintln!(
                    "RECONCILIATION ERROR: Failed to fetch unreconciled spends: {}",
                    e
                );
            }
        }
    }

    // Spawn background sanctions list sync worker (07:00 UTC daily refresh from GitHub)
    let _sanctions_handle = sanctions_sync::spawn_sanctions_sync_task();

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

    // Spawn CCIP destination monitor (if configured)
    let ccip_db = state.db.clone();
    let ccip_config = config::CcipDestinationConfig::from_env();
    tokio::spawn(async move {
        ccip_monitor::ccip_monitor_loop(ccip_db, ccip_config).await;
    });

    // Spawn execution fee claimer background worker
    let fee_claimer_state = state.clone();
    tokio::spawn(async move {
        execution_fee_claimer::execution_fee_claimer_worker(fee_claimer_state).await;
    });

    // Spawn mempool lease watchdog background worker (DEC-027)
    let mempool_watchdog_state = state.clone();
    tokio::spawn(async move {
        mempool_watchdog::mempool_lease_watchdog_worker(mempool_watchdog_state).await;
    });

    // Spawn on-chain MMR event indexer background worker (DEC-035C)
    let mmr_indexer_state = state.clone();
    tokio::spawn(async move {
        mmr_indexer::mmr_indexer_worker(mmr_indexer_state).await;
    });

    let app = create_relayer_router(state.clone());

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

    println!("------------------------------------------------------------");

    axum::serve(listener, app).await.unwrap();
}

pub fn create_relayer_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health_check))
        .route("/live", get(liveness_check))
        .route("/ready", get(readiness_check))
        .route("/api/v1/quote", post(create_quote))
        .route("/api/quote/private-spend", get(handle_private_spend_quote))
        .route("/api/v1/spend-private-note", post(spend_private_note))
        .route("/api/v1/spend-joinsplit", post(spend_joinsplit))
        .route("/api/v1/mmr/proof/{leaf_index}", get(get_mmr_proof))
        .route("/api/v1/mmr/leaves", get(get_bulk_leaves))
        .route("/api/v1/mmr/tip", get(handle_mmr_tip))
        .route("/api/v1/notes/sync", get(handle_notes_sync))
        .route("/api/tx-status", get(handle_tx_status))
        .route("/api/x402/verify", post(handle_x402_verify))
        .route("/api/ccip/refund", post(handle_ccip_refund))
        .layer(axum::middleware::from_fn(rate_limit_middleware))
        .layer(DefaultBodyLimit::max(64 * 1024))
        .with_state(state)
}

async fn rate_limit_middleware(request: Request<Body>, next: Next) -> Result<Response, StatusCode> {
    // Extract IP address from headers
    let ip = request
        .headers()
        .get("x-forwarded-for")
        .and_then(|val| val.to_str().ok())
        .map(|s| s.split(',').next().unwrap_or(s).trim().to_string())
        .unwrap_or_else(|| "127.0.0.1".to_string());

    struct RateLimiterState {
        counts: HashMap<String, (u32, Instant)>,
        last_cleanup: Instant,
    }

    static LIMITER: OnceLock<Mutex<RateLimiterState>> = OnceLock::new();
    let limiter = LIMITER.get_or_init(|| {
        Mutex::new(RateLimiterState {
            counts: HashMap::new(),
            last_cleanup: Instant::now(),
        })
    });

    let mut state = limiter.lock().await;
    let now = Instant::now();

    // Periodic cleanup of stale entries (> 60s inactivity) to prevent unbounded memory growth
    if now.duration_since(state.last_cleanup) > Duration::from_secs(60) {
        state
            .counts
            .retain(|_, (_, last_seen)| now.duration_since(*last_seen) < Duration::from_secs(60));
        state.last_cleanup = now;
    }

    let (count, last_reset) = state.counts.entry(ip).or_insert((0, now));

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

    drop(state); // drop lock before running handler
    Ok(next.run(request).await)
}

#[cfg(test)]
mod node_tests {
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
}
