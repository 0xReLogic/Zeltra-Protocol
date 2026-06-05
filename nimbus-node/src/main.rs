mod dto;
mod state;
mod http;
mod kms;
mod handlers;

use axum::{
    routing::{post, get},
    Router,
};
use std::time::Duration;

use state::AppState;
use handlers::*;

#[tokio::main]
async fn main() {
    let (share_sk, share_index) = kms::load_share_key().await;
    let state = AppState::new(share_sk, share_index);

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
        .route("/api/sign-share", post(handle_sign_share))
        .route("/api/leader/sign", post(handle_leader_sign))
        .with_state(state);

    let port = std::env::var("PORT").unwrap_or_else(|_| "8080".to_string());
    let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{}", port)).await.unwrap();
    println!("NIMBUS RELAYER NODE STARTED ON PORT {}", port);
    println!("------------------------------------------------------------");
    println!("Listening on: http://{}", listener.local_addr().unwrap());
    println!("Gasless EIP-7702 delegation: ACTIVE");
    println!("x402 Facilitator endpoint:   ACTIVE");
    println!("Relayer batch queue interval: 2 seconds");
    println!("------------------------------------------------------------");

    axum::serve(listener, app).await.unwrap();
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
