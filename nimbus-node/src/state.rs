//! Application state with persistent SQLite database

use std::sync::Arc;
use tokio::sync::Mutex;
use crate::database::Database;
use crate::evm_client::EvmClient;

/// Production-ready application state with persistent database
#[derive(Clone)]
pub struct AppState {
    /// Persistent SQLite database (replaces in-memory storage)
    pub db: Database,
    
    /// In-memory spend queue (batched before on-chain submission)
    pub spend_queue: Arc<Mutex<Vec<crate::dto::SpendRequest>>>,
    
    /// BLS12-381 threshold signature share (secret key for this relayer node)
    pub share_sk: Arc<nimbus_core::Fr>,
    
    /// Index of this relayer node in the threshold signature scheme (1-indexed)
    pub share_index: u32,
    
    /// Relayer wallet balance in ETH (for gas fee tracking)
    pub relayer_wallet_balance_eth: Arc<Mutex<f64>>,
    
    /// Accumulated profit in USDC from batching markup fees
    pub relayer_accumulated_profit_usdc: Arc<Mutex<f64>>,
    
    /// EVM client for broadcasting real transactions (replaces mock tx hashes)
    pub evm_client: Option<Arc<EvmClient>>,
}

impl AppState {
    /// Initialize application state with persistent database
    pub async fn new(
        db: Database,
        share_sk: nimbus_core::Fr,
        share_index: u32,
        evm_client: Option<Arc<EvmClient>>,
    ) -> Self {
        Self {
            db,
            spend_queue: Arc::new(Mutex::new(Vec::new())),
            share_sk: Arc::new(share_sk),
            share_index,
            relayer_wallet_balance_eth: Arc::new(Mutex::new(10.0)),
            relayer_accumulated_profit_usdc: Arc::new(Mutex::new(0.0)),
            evm_client,
        }
    }
}
