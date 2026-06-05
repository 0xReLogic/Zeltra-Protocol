//! Application state and in-memory storage

use std::sync::Arc;
use tokio::sync::Mutex;
use crate::dto::Session;

// WARNING / REMINDER FOR DEVELOPERS & AI AGENTS:
// THIS IS AN IN-MEMORY SIMULATION DATABASE FOR PROOF OF CONCEPT (PoC) / TESTING.
// - Sessions and nullifiers are stored in memory and will be lost on restart.
// - TO UPGRADE TO PRODUCTION: Replace this with a persistent DB (e.g. SQLite, PostgreSQL via sqlx/rusqlite).
#[derive(Clone)]
pub struct AppState {
    pub sessions: Arc<Mutex<Vec<Session>>>,
    pub spend_queue: Arc<Mutex<Vec<crate::dto::SpendRequest>>>,
    pub nullifiers: Arc<Mutex<Vec<String>>>,
    pub share_sk: Arc<nimbus_core::Fr>,
    pub share_index: u32,
    pub relayer_wallet_balance_eth: Arc<Mutex<f64>>,
    pub relayer_accumulated_profit_usdc: Arc<Mutex<f64>>,
}

impl AppState {
    pub fn new(share_sk: nimbus_core::Fr, share_index: u32) -> Self {
        Self {
            sessions: Arc::new(Mutex::new(Vec::new())),
            spend_queue: Arc::new(Mutex::new(Vec::new())),
            nullifiers: Arc::new(Mutex::new(Vec::new())),
            share_sk: Arc::new(share_sk),
            share_index,
            relayer_wallet_balance_eth: Arc::new(Mutex::new(10.0)),
            relayer_accumulated_profit_usdc: Arc::new(Mutex::new(0.0)),
        }
    }
}
