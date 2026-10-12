//! Application state with persistent SQLite database

use crate::database::Database;
use crate::evm_client::EvmClient;
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Minimum relayer ETH balance required to process batches.
/// If balance drops below this, batch processing is halted to prevent insolvency.
pub const MIN_RELAYER_BALANCE_ETH: f64 = 0.1;

/// Production-ready application state with persistent database
#[derive(Clone)]
pub struct AppState {
    /// Persistent SQLite database (replaces in-memory storage)
    pub db: Database,

    /// Relayer wallet balance in ETH (for gas fee tracking)
    pub relayer_wallet_balance_eth: Arc<Mutex<f64>>,

    /// Accumulated profit in USDC from batching markup fees
    pub relayer_accumulated_profit_usdc: Arc<Mutex<f64>>,

    /// EVM client for broadcasting real transactions (replaces mock tx hashes)
    pub evm_client: Option<Arc<EvmClient>>,

    /// Cache for clean_association_roots: root_hex -> registration timestamp
    #[allow(dead_code)]
    pub root_timestamp_cache: Arc<Mutex<HashMap<String, u64>>>,

    /// Relayer Ethereum address (for EIP-712 quote signing)
    pub relayer_address: String,

    /// Pricing and oracle configuration (DEC-029)
    pub pricing_config: crate::config::PricingConfig,

    /// Cached current ETH price in USDC (updated periodically or on-demand from Chainlink)
    pub current_eth_price_usdc: Arc<Mutex<f64>>,

    /// Timestamp of the last successful price fetch (Unix seconds)
    pub last_price_fetch_timestamp: Arc<Mutex<u64>>,
}

impl AppState {
    /// Initialize application state with persistent database
    pub async fn new(db: Database, evm_client: Option<Arc<EvmClient>>) -> Self {
        let pricing_config = crate::config::PricingConfig::from_env();

        Self {
            db,
            relayer_wallet_balance_eth: Arc::new(Mutex::new(10.0)),
            relayer_accumulated_profit_usdc: Arc::new(Mutex::new(0.0)),
            evm_client,
            root_timestamp_cache: Arc::new(Mutex::new(HashMap::new())),
            relayer_address: std::env::var("NIMBUS_RELAYER_ADDRESS")
                .unwrap_or_else(|_| "0x0000000000000000000000000000000000000000".to_string()),
            pricing_config: pricing_config.clone(),
            current_eth_price_usdc: Arc::new(Mutex::new(pricing_config.fallback_eth_price)),
            last_price_fetch_timestamp: Arc::new(Mutex::new(0)),
        }
    }

    /// Fetch current ETH price in USDC using Chainlink oracle with fallback (DEC-029).
    pub async fn get_eth_price(&self) -> f64 {
        let now_secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // Check if cached price is still fresh (within cache_ttl_secs)
        let last_fetch = *self.last_price_fetch_timestamp.lock().await;
        if last_fetch > 0
            && now_secs.saturating_sub(last_fetch) < self.pricing_config.cache_ttl_secs
        {
            return *self.current_eth_price_usdc.lock().await;
        }

        if let (Some(ref evm), Some(ref feed_str)) =
            (&self.evm_client, &self.pricing_config.eth_feed_address)
        {
            if let Ok(feed_addr) = alloy::primitives::Address::from_str(feed_str) {
                match evm
                    .get_chainlink_eth_price(
                        &feed_addr,
                        self.pricing_config.staleness_threshold_secs,
                    )
                    .await
                {
                    Ok(price) => {
                        *self.current_eth_price_usdc.lock().await = price;
                        *self.last_price_fetch_timestamp.lock().await = now_secs;
                        return price;
                    }
                    Err(e) => {
                        eprintln!(
                            "PRICING WARNING: Chainlink oracle query failed: {}. Using fallback/cached price.",
                            e
                        );
                    }
                }
            }
        }

        *self.current_eth_price_usdc.lock().await
    }

    /// Check if relayer balance is sufficient for a batch of given size
    #[allow(dead_code)]
    pub async fn check_balance_for_batch(&self, estimated_cost_eth: f64) -> Result<f64, String> {
        let bal = *self.relayer_wallet_balance_eth.lock().await;

        if bal < MIN_RELAYER_BALANCE_ETH {
            return Err(format!(
                "Relayer balance ({:.5} ETH) is below minimum threshold ({:.5} ETH). Refusing to process batch.",
                bal, MIN_RELAYER_BALANCE_ETH
            ));
        }

        if bal < estimated_cost_eth + MIN_RELAYER_BALANCE_ETH {
            return Err(format!(
                "Insufficient balance for batch: need {:.5} ETH + {:.5} ETH reserve, have {:.5} ETH",
                estimated_cost_eth, MIN_RELAYER_BALANCE_ETH, bal
            ));
        }

        Ok(bal)
    }
}
