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

    /// In-memory cache for issued execution quotes: quote_hash / quote_id -> ExecutionQuoteResponse
    pub quote_cache: Arc<Mutex<HashMap<String, crate::dto::ExecutionQuoteResponse>>>,

    /// In-memory concurrent FlatMerkleMountainRange mirror for sub-microsecond proof serving (DEC-045B)
    pub in_memory_mmr: Arc<tokio::sync::RwLock<zeltra_core::FlatMerkleMountainRange>>,

    /// In-memory leaf block numbers: leaf_index -> block_number for zero-disk proof serving
    pub in_memory_leaf_blocks: Arc<tokio::sync::RwLock<Vec<u64>>>,

    /// Cached last indexed block number for zero-disk tip serving
    pub last_indexed_block: Arc<std::sync::atomic::AtomicU64>,
}

impl AppState {
    /// Initialize application state with persistent database
    pub async fn new(db: Database, evm_client: Option<Arc<EvmClient>>) -> Self {
        let pricing_config = crate::config::PricingConfig::from_env();

        // B1.2 & B1.3: Pre-warm FlatMerkleMountainRange from persistent database
        let flat_mmr = match db.load_flat_mmr_tree().await {
            Ok(tree) => {
                println!(
                    "MMR_WARMUP: Successfully pre-warmed in-memory FlatMerkleMountainRange with {} leaves. Bagged root: 0x{}",
                    tree.leaf_count(),
                    hex::encode(zeltra_core::fr_to_be_bytes(&tree.get_root()))
                );
                tree
            }
            Err(e) => {
                if db.get_mmr_leaf(0).await.unwrap_or(None).is_some() {
                    panic!("CRITICAL: ColdBootDesync during MMR warm-up: {}", e);
                }
                zeltra_core::FlatMerkleMountainRange::new()
            }
        };

        // B1.3: Checkpoint parity verification against on-chain contract if EVM client available
        if let Some(ref evm) = evm_client {
            if flat_mmr.leaf_count() > 0 {
                if let Ok(onchain_root) = evm.get_note_tree_root().await {
                    let local_root = zeltra_core::fr_to_be_bytes(&flat_mmr.get_root());
                    if local_root != onchain_root && onchain_root != [0u8; 32] {
                        eprintln!(
                            "CRITICAL: ColdBootDesync detected! Local root: 0x{}, On-chain root: 0x{}",
                            hex::encode(local_root),
                            hex::encode(onchain_root)
                        );
                        if crate::config::runtime_mode().is_strict() {
                            panic!("CRITICAL: ColdBootDesync between local MMR and on-chain contract note_tree_root!");
                        }
                    } else {
                        println!("MMR_WARMUP: On-chain root parity verified successfully against contract note_tree_root");
                    }
                }
            }
        }

        let leaves = db.get_all_mmr_leaves().await.unwrap_or_default();
        let leaf_blocks: Vec<u64> = leaves.iter().map(|l| l.block_number).collect();
        let last_block = db.get_mmr_last_indexed_block().await.unwrap_or(None).unwrap_or(0);

        Self {
            db,
            relayer_wallet_balance_eth: Arc::new(Mutex::new(10.0)),
            relayer_accumulated_profit_usdc: Arc::new(Mutex::new(0.0)),
            evm_client,
            root_timestamp_cache: Arc::new(Mutex::new(HashMap::new())),
            relayer_address: std::env::var("ZELTRA_RELAYER_ADDRESS")
                .unwrap_or_else(|_| "0x0000000000000000000000000000000000000000".to_string()),
            pricing_config: pricing_config.clone(),
            current_eth_price_usdc: Arc::new(Mutex::new(pricing_config.fallback_eth_price)),
            last_price_fetch_timestamp: Arc::new(Mutex::new(0)),
            quote_cache: Arc::new(Mutex::new(HashMap::new())),
            in_memory_mmr: Arc::new(tokio::sync::RwLock::new(flat_mmr)),
            in_memory_leaf_blocks: Arc::new(tokio::sync::RwLock::new(leaf_blocks)),
            last_indexed_block: Arc::new(std::sync::atomic::AtomicU64::new(last_block)),
        }
    }

    /// Rollback MMR leaves and peak state to a specific fork block (DEC-045B INV-1 & Reorg Protection).
    /// Atomically unwinds the database and reloads the in-memory MMR mirror under write-lock.
    pub async fn rollback_mmr_to_block(&self, fork_block: u64) -> anyhow::Result<()> {
        self.db.rollback_mmr_to_block(fork_block).await?;
        let restored_tree = self.db.load_flat_mmr_tree().await?;
        let mut tree_lock = self.in_memory_mmr.write().await;
        *tree_lock = restored_tree;
        let remaining_leaves = self.db.get_all_mmr_leaves().await.unwrap_or_default();
        let remaining_blocks: Vec<u64> = remaining_leaves.iter().map(|l| l.block_number).collect();
        *self.in_memory_leaf_blocks.write().await = remaining_blocks;
        let last_block = self.db.get_mmr_last_indexed_block().await.unwrap_or(None).unwrap_or(fork_block);
        self.last_indexed_block.store(last_block, std::sync::atomic::Ordering::SeqCst);
        println!(
            "MMR_ROLLBACK: Reorg rollback complete to block {}. New leaf count: {}, root: 0x{}",
            fork_block,
            tree_lock.leaf_count(),
            hex::encode(zeltra_core::fr_to_be_bytes(&tree_lock.get_root()))
        );
        Ok(())
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
