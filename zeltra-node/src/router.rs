//! Relayer Axum Router & Rate Limiting Middleware

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
use std::sync::OnceLock;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

use crate::handlers::*;
use crate::state::AppState;

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
        .route("/api/v1/ccip/deposit/{message_id}", get(handle_ccip_deposit_status))
        .route("/api/v1/ccip/spend/{message_id}", get(handle_ccip_spend_status))
        .route("/api/v1/ccip/status/{message_id}", get(handle_ccip_universal_status))
        .layer(axum::middleware::from_fn(rate_limit_middleware))
        .layer(DefaultBodyLimit::max(64 * 1024))
        .with_state(state)
}

pub async fn rate_limit_middleware(
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

    let rate_limit_max: u32 = std::env::var("ZELTRA_RATE_LIMIT_PER_SEC")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(200);

    if rate_limit_max > 0 {
        let (count, last_reset) = state.counts.entry(ip).or_insert((0, now));

        if now.duration_since(*last_reset) > Duration::from_secs(1) {
            *count = 1;
            *last_reset = now;
        } else {
            *count += 1;
            if *count > rate_limit_max {
                return Err(StatusCode::TOO_MANY_REQUESTS);
            }
        }
    }

    drop(state); // drop lock before running handler
    Ok(next.run(request).await)
}
