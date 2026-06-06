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
    
    /// BLS12-381 threshold signature share split into two masked parts
    share_sk_part1: nimbus_core::Fr,
    share_sk_part2: nimbus_core::Fr,
    
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
        use nimbus_core::UniformRand;
        
        // Split the key in memory using a random mask
        let mut rng = rand::thread_rng();
        let part1 = nimbus_core::Fr::rand(&mut rng);
        let part2 = share_sk - part1;
        
        Self {
            db,
            spend_queue: Arc::new(Mutex::new(Vec::new())),
            share_sk_part1: part1,
            share_sk_part2: part2,
            share_index,
            relayer_wallet_balance_eth: Arc::new(Mutex::new(10.0)),
            relayer_accumulated_profit_usdc: Arc::new(Mutex::new(0.0)),
            evm_client,
        }
    }

    /// Sign a blinded message using the split private key share, zeroizing temporary variables immediately
    pub fn sign_share_masked(
        &self,
        blinded: &nimbus_core::BlindedMessage,
        k: &nimbus_core::Fr,
    ) -> nimbus_core::PartialBlindSignature {
        use nimbus_core::sign_share;
        
        // Reconstruct key in memory
        let mut sk = self.share_sk_part1 + self.share_sk_part2;
        let sig = sign_share(&sk, blinded, k);
        
        // Securely zero out the reconstructed key in memory
        unsafe {
            std::ptr::write_volatile(&mut sk, nimbus_core::Fr::from(0u64));
        }
        
        sig
    }

    /// Get the public key of the issuer dynamically from the split key, zeroizing intermediate key
    pub fn get_issuer_public_key(&self) -> nimbus_core::IssuerPublicKey {
        use nimbus_core::IssuerSecretKey;
        
        let mut sk = self.share_sk_part1 + self.share_sk_part2;
        let sk_iss = IssuerSecretKey(sk);
        let pk = sk_iss.public_key();
        
        // Securely zero out temporary key
        unsafe {
            std::ptr::write_volatile(&mut sk, nimbus_core::Fr::from(0u64));
        }
        
        pk
    }

    /// Get the reconstructed share key for manual overrides (e.g. testing)
    #[allow(dead_code)]
    pub fn get_reconstructed_key(&self) -> nimbus_core::Fr {
        self.share_sk_part1 + self.share_sk_part2
    }
}
