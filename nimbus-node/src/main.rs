mod dto;
mod state;
mod http;
mod kms;
mod handlers;
mod database;
mod evm_client;

use axum::{
    routing::{post, get},
    Router,
    body::Body,
    http::{Request, StatusCode},
    middleware::Next,
    response::Response,
    extract::DefaultBodyLimit,
};
use std::time::Duration;
use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::Mutex;
use std::time::Instant;
use std::sync::OnceLock;

use state::AppState;
use handlers::*;
use database::Database;
use evm_client::EvmClient;

#[tokio::main]
async fn main() {
    // Initialize persistent SQLite database
    let db_path = std::env::var("NIMBUS_DB_PATH")
        .unwrap_or_else(|_| "./nimbus-relayer.db".to_string());
    
    println!("Initializing database at: {}", db_path);
    let db = Database::new(&db_path).await.expect("Failed to initialize database");
    println!("✓ Database initialized with WAL mode and production PRAGMAs");
    
    // Load threshold signature share from KMS
    let (mut share_sk, share_index) = kms::load_share_key().await;
    
    // Initialize EVM client for real transaction broadcasting (optional for dev mode)
    let evm_client = if let (Ok(rpc_url), Ok(private_key), Ok(contract_addr)) = (
        std::env::var("NIMBUS_RPC_URL"),
        std::env::var("NIMBUS_RELAYER_PRIVATE_KEY"),
        std::env::var("NIMBUS_CONTRACT_ADDRESS")
    ) {
        match EvmClient::new(&rpc_url, &private_key, &contract_addr).await {
            Ok(client) => {
                println!("✓ EVM client initialized");
                println!("  RPC URL:    {}", rpc_url);
                println!("  Contract:   {}", contract_addr);
                println!("  Signer:     {}", client.signer_address());
                Some(Arc::new(client))
            }
            Err(e) => {
                eprintln!("WARNING: Failed to initialize EVM client: {}", e);
                eprintln!("         Relayer will use MOCK transaction hashes (dev mode)");
                None
            }
        }
    } else {
        println!("WARNING: EVM client not configured");
        println!("         Set NIMBUS_RPC_URL, NIMBUS_RELAYER_PRIVATE_KEY, NIMBUS_CONTRACT_ADDRESS");
        println!("         Relayer will use MOCK transaction hashes (dev mode)");
        None
    };
    
    // Initialize application state with persistent database
    let state = AppState::new(db, share_sk, share_index, evm_client).await;
    
    // Securely zero out the key in the main stack frame immediately
    unsafe {
        std::ptr::write_volatile(&mut share_sk, nimbus_core::Fr::from(0u64));
    }

    // Spawn background worker to batch and process spends every 2 seconds
    let worker_state = state.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(2)).async_wait().await;
            process_spend_batch(&worker_state).await;
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
        }
    });

    let app = Router::new()
        .route("/health", get(health_check))
        .route("/api/deposit", post(handle_deposit))
        .route("/api/reveal", post(handle_reveal))
        .route("/api/spend", post(handle_spend))
        .route("/api/x402/verify", post(handle_x402_verify))
        .route("/api/sign-share", post(handle_sign_share))
        .route("/api/leader/sign", post(handle_leader_sign))
        .layer(axum::middleware::from_fn(rate_limit_middleware))
        .layer(DefaultBodyLimit::max(64 * 1024)) // 64KB request body size limit
        .with_state(state);

    let port = std::env::var("PORT").unwrap_or_else(|_| "8080".to_string());
    let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{}", port)).await.unwrap();
    println!("------------------------------------------------------------");
    println!("NIMBUS RELAYER NODE STARTED ON PORT {}", port);
    println!("------------------------------------------------------------");
    println!("Listening on: http://{}", listener.local_addr().unwrap());
    println!("Database:     {}", db_path);
    println!("Persistence:  ENABLED (SQLite WAL mode)");
    println!("Gasless EIP-7702 delegation: ACTIVE");
    println!("x402 Facilitator endpoint:   ACTIVE");
    println!("Relayer batch queue interval: 2 seconds");
    println!("------------------------------------------------------------");

    axum::serve(listener, app).await.unwrap();
}

async fn rate_limit_middleware(
    request: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
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
        if *count > 10 { // Max 10 requests per second
            return Err(StatusCode::TOO_MANY_REQUESTS);
        }
    }
    
    drop(map); // drop lock before running handler
    Ok(next.run(request).await)
}

trait TokioSleepExt {
    fn async_wait(self) -> Self;
}
impl<T> TokioSleepExt for T {
    fn async_wait(self) -> Self { self }
}

#[cfg(test)]
mod node_tests {
    #[tokio::test]
    async fn test_load_share_key_fallback() {
        // Test 32-byte fallback
        std::env::set_var("NIMBUS_SHARE_KEY", "0000000000000000000000000000000000000000000000000000000000000001");
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
}
