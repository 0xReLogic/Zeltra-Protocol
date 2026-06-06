//! Application state with persistent SQLite database

use std::sync::Arc;
use tokio::sync::Mutex;
use crate::database::Database;
use crate::evm_client::EvmClient;
use crate::circuit_breaker::{CircuitBreaker, CircuitBreakerConfig};
use crate::key_rotation::{KeyManager, KeyRotationConfig};
use std::time::Duration;

/// Minimum relayer ETH balance required to process batches.
/// If balance drops below this, batch processing is halted to prevent insolvency.
pub const MIN_RELAYER_BALANCE_ETH: f64 = 0.1;

/// Production-ready application state with persistent database
#[derive(Clone)]
pub struct AppState {
    /// Persistent SQLite database (replaces in-memory storage)
    pub db: Database,
    
    /// In-memory spend queue (batched before on-chain submission)
    pub spend_queue: Arc<Mutex<Vec<crate::dto::SpendRequest>>>,
    
    /// Key manager with rotation and re-masking support
    pub key_manager: KeyManager,
    
    /// Relayer wallet balance in ETH (for gas fee tracking)
    pub relayer_wallet_balance_eth: Arc<Mutex<f64>>,
    
    /// Accumulated profit in USDC from batching markup fees
    pub relayer_accumulated_profit_usdc: Arc<Mutex<f64>>,
    
    /// EVM client for broadcasting real transactions (replaces mock tx hashes)
    pub evm_client: Option<Arc<EvmClient>>,
    
    /// Circuit breaker for guardian RPC calls
    pub guardian_circuit_breaker: CircuitBreaker,
    
    /// Circuit breaker for Vault/OpenBao KMS access
    #[allow(dead_code)]
    pub vault_circuit_breaker: CircuitBreaker,
    
    /// Circuit breaker for blockchain RPC provider
    pub rpc_circuit_breaker: CircuitBreaker,
}

impl AppState {
    /// Initialize application state with persistent database
    pub async fn new(
        db: Database,
        share_sk: nimbus_core::Fr,
        share_index: u32,
        evm_client: Option<Arc<EvmClient>>,
    ) -> Self {
        // Key rotation config from environment
        let rotation_enabled = std::env::var("NIMBUS_KEY_ROTATION")
            .map(|v| v == "true" || v == "1")
            .unwrap_or(false);
        
        let rotation_interval = std::env::var("NIMBUS_KEY_ROTATION_INTERVAL")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(3600);
        
        let key_config = KeyRotationConfig {
            rotation_interval: Duration::from_secs(rotation_interval),
            max_key_age: Duration::from_secs(rotation_interval * 24),
            enabled: rotation_enabled,
        };
        
        let key_manager = KeyManager::new(share_sk, share_index, key_config);
        let vault_circuit_breaker = key_manager.vault_circuit_breaker.clone();
        
        Self {
            db,
            spend_queue: Arc::new(Mutex::new(Vec::new())),
            key_manager,
            relayer_wallet_balance_eth: Arc::new(Mutex::new(10.0)),
            relayer_accumulated_profit_usdc: Arc::new(Mutex::new(0.0)),
            evm_client,
            guardian_circuit_breaker: CircuitBreaker::new(CircuitBreakerConfig {
                failure_threshold: 5,
                recovery_timeout: Duration::from_secs(30),
                success_threshold: 2,
                name: "guardian-rpc".to_string(),
            }),
            vault_circuit_breaker,
            rpc_circuit_breaker: CircuitBreaker::new(CircuitBreakerConfig {
                failure_threshold: 5,
                recovery_timeout: Duration::from_secs(15),
                success_threshold: 2,
                name: "blockchain-rpc".to_string(),
            }),
        }
    }

    /// Sign a blinded message using the key manager (with rotation support)
    pub async fn sign_share_masked(
        &self,
        blinded: &nimbus_core::BlindedMessage,
        k: &nimbus_core::Fr,
    ) -> nimbus_core::PartialBlindSignature {
        self.key_manager.sign_share(blinded, k).await
    }

    /// Get the public key of the issuer via key manager
    pub fn get_issuer_public_key(&self) -> nimbus_core::IssuerPublicKey {
        // Use a blocking approach since this is called in sync context
        
        // We need to reconstruct from the key manager synchronously
        // This is safe because we're already in an async context
        let rt = tokio::runtime::Handle::current();
        rt.block_on(self.key_manager.get_issuer_public_key())
    }

    /// Check if relayer balance is sufficient for a batch of given size
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
