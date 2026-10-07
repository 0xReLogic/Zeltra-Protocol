use alloy::{
    network::{EthereumWallet, TransactionBuilder},
    primitives::{Address, Bytes, U256},
    providers::{DynProvider, Provider, ProviderBuilder, WsConnect},
    rpc::types::eth::{Filter, TransactionRequest},
    signers::local::PrivateKeySigner,
    sol,
    sol_types::{SolCall, SolEvent, SolValue},
};
use anyhow::{Context, Result};
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::Mutex;

sol! {
    #[derive(Debug, PartialEq)]
    event DepositFee(
        bytes32 indexed session_id,
        bytes32 indexed com_k_hash,
        address indexed client,
        uint256 gross_amount,
        uint256 fee,
        uint256 net_amount
    );

    function spend(
        bytes32 root,
        bytes32 nullifier,
        bytes calldata alpha_neg_bytes,
        bytes calldata pk_iss_bytes,
        address recipient,
        uint256 amount,
        bytes32 recipient_or_intent_hash,
        uint256 expiry,
        bytes32 nonce,
        uint256 max_execution_fee,
        uint256 execution_fee
    ) external returns (bool);

    function batchSpend(
        bytes32[] calldata roots,
        bytes32[] calldata nullifiers,
        bytes[] calldata alpha_neg_items,
        bytes[] calldata pk_iss_items,
        address[] calldata recipients,
        uint256[] calldata amounts,
        bytes32[] calldata recipient_or_intent_hashes,
        uint256[] calldata expiries,
        bytes32[] calldata nonces
    ) external returns (bool);

    function spendPrivateNote(
        bytes32 note_root,
        bytes32 input_nullifier,
        bytes32 output_commitment,
        address recipient,
        uint256 merchant_amount,
        uint256 protocol_fee,
        uint256 execution_fee,
        uint256 max_execution_fee,
        bytes32 quote_hash,
        uint256 expiry,
        uint256 has_change,
        bytes calldata proof_a_neg,
        bytes calldata proof_b,
        bytes calldata proof_c
    ) external returns (bool);

    function claimExecutionFees(uint256 amount) external;

    function getCleanRootTimestamp(bytes32 root) external view returns (uint256);
    function isNullifierSpent(bytes32 nullifier) external view returns (bool);
    function isSanctioned(address addr) external view returns (bool);

    struct EVMTokenAmount {
        address token;
        uint256 amount;
    }

    struct EVMExtraArgsV2 {
        uint256 gasLimit;
        bool allowOutOfOrderExecution;
    }

    struct EVM2AnyMessage {
        bytes receiver;
        bytes data;
        EVMTokenAmount[] tokenAmounts;
        address feeToken;
        bytes extraArgs;
    }

    interface IRouterClient {
        function ccipSend(uint64 destinationChainSelector, EVM2AnyMessage calldata message) external payable returns (bytes32);
        function getFee(uint64 destinationChainSelector, EVM2AnyMessage calldata message) external view returns (uint256);
    }
}

pub struct EvmClient {
    provider: DynProvider,
    fallback_provider: Option<DynProvider>,
    signer_address: Address,
    contract_address: Address,
    ccip_router_address: Address,
    pub signer: PrivateKeySigner,
    nonce_lock: Arc<Mutex<Option<u64>>>,
    finality_config: crate::config::ReceiptFinalityConfig,
    db: Option<Arc<crate::database::Database>>,
}

#[derive(Clone, Debug)]
pub struct TransactionOutcome {
    pub tx_hash: String,
    pub block_number: u64,
    pub success: bool,
    pub gas_used: u64,
    pub effective_gas_price: u128,
    /// CCIP message ID extracted from CCIPMessageSent event (if CCIP transaction)
    pub ccip_message_id: Option<String>,
    #[allow(dead_code)]
    pub confirmations: u64,
}

#[derive(Clone, Debug)]
pub struct BatchSpendItem {
    pub root_hex: String,
    pub nullifier: String,
    pub alpha_neg_hex: String,
    pub pk_iss_hex: String,
    pub recipient: String,
    pub amount: u64,
    pub recipient_or_intent_hash_hex: String,
    pub expiry: u64,
    pub nonce_hex: String,
}

impl EvmClient {
    pub async fn new(rpc_url: &str, private_key: &str, contract_address: &str) -> Result<Self> {
        let signer = PrivateKeySigner::from_str(private_key).context("Private key invalid")?;

        let signer_address = signer.address();
        let wallet = EthereumWallet::from(signer.clone());

        // Dynamically choose between WebSocket and HTTP based on URL scheme
        let provider = if rpc_url.starts_with("ws://") || rpc_url.starts_with("wss://") {
            let ws = WsConnect::new(rpc_url);
            let p = ProviderBuilder::new()
                .wallet(wallet.clone())
                .connect_ws(ws)
                .await
                .context("Gagal connect ke RPC primary via WebSocket")?;
            DynProvider::new(p)
        } else {
            let url = rpc_url
                .parse::<alloy::transports::http::reqwest::Url>()
                .context("Invalid HTTP RPC URL")?;
            let p = ProviderBuilder::new()
                .wallet(wallet.clone())
                .connect_http(url);
            DynProvider::new(p)
        };

        // Fallback provider (opsional, dari env var NIMBUS_RPC_FALLBACK_URL)
        let fallback_provider = if let Ok(fallback_url) = std::env::var("NIMBUS_RPC_FALLBACK_URL") {
            let fallback_wallet = EthereumWallet::from(signer.clone());

            let p_res = if fallback_url.starts_with("ws://") || fallback_url.starts_with("wss://") {
                let fallback_ws = WsConnect::new(&fallback_url);
                ProviderBuilder::new()
                    .wallet(fallback_wallet)
                    .connect_ws(fallback_ws)
                    .await
                    .map(DynProvider::new)
                    .map_err(|e| anyhow::anyhow!("Fallback WS failed: {}", e))
            } else {
                match fallback_url.parse::<alloy::transports::http::reqwest::Url>() {
                    Ok(url) => {
                        let p = ProviderBuilder::new()
                            .wallet(fallback_wallet)
                            .connect_http(url);
                        Ok(DynProvider::new(p))
                    }
                    Err(e) => Err(anyhow::anyhow!("Invalid fallback HTTP URL: {}", e)),
                }
            };

            match p_res {
                Ok(p) => {
                    println!("  Fallback RPC: {}", fallback_url);
                    Some(p)
                }
                Err(e) => {
                    eprintln!("WARNING: Fallback RPC gagal connect: {}", e);
                    None
                }
            }
        } else {
            None
        };

        let contract_addr =
            Address::from_str(contract_address).context("Contract address invalid")?;

        let ccip_router_address = if let Ok(router_str) = std::env::var("NIMBUS_CCIP_ROUTER") {
            Address::from_str(&router_str).context("NIMBUS_CCIP_ROUTER invalid address format")?
        } else {
            // Default to Arbitrum Sepolia CCIP Router: 0x2a9C5afB0d0e4BAb2BCdaE109EC4b0c4Be15a165
            Address::from_str("0x2a9C5afB0d0e4BAb2BCdaE109EC4b0c4Be15a165").unwrap()
        };

        let nonce_lock = Arc::new(Mutex::new(None));
        let finality_config = crate::config::ReceiptFinalityConfig::from_env();

        Ok(Self {
            provider,
            fallback_provider,
            signer_address,
            contract_address: contract_addr,
            ccip_router_address,
            signer,
            nonce_lock,
            finality_config,
            db: None,
        })
    }

    pub fn with_db(mut self, db: Arc<crate::database::Database>) -> Self {
        self.db = Some(db);
        self
    }

    #[allow(dead_code)]
    pub fn with_finality_config(mut self, config: crate::config::ReceiptFinalityConfig) -> Self {
        self.finality_config = config;
        self
    }

    pub fn finality_config(&self) -> &crate::config::ReceiptFinalityConfig {
        &self.finality_config
    }

    pub async fn get_block_number(&self) -> Result<u64> {
        match self.provider.get_block_number().await {
            Ok(b) => Ok(b),
            Err(e) => {
                if let Some(ref fallback) = self.fallback_provider {
                    fallback
                        .get_block_number()
                        .await
                        .context("Fallback RPC get_block_number also failed")
                } else {
                    Err(anyhow::anyhow!(
                        "Failed to get block number from primary RPC: {}",
                        e
                    ))
                }
            }
        }
    }

    pub async fn get_receipt(
        &self,
        tx_hash: &str,
    ) -> Result<Option<alloy::rpc::types::TransactionReceipt>> {
        let hash = alloy::primitives::TxHash::from_str(tx_hash)
            .map_err(|e| anyhow::anyhow!("Invalid tx hash '{}': {}", tx_hash, e))?;
        match self.provider.get_transaction_receipt(hash).await {
            Ok(Some(r)) => Ok(Some(r)),
            Ok(None) => {
                if let Some(ref fallback) = self.fallback_provider {
                    fallback
                        .get_transaction_receipt(hash)
                        .await
                        .context("Fallback get_transaction_receipt failed")
                } else {
                    Ok(None)
                }
            }
            Err(e) => {
                if let Some(ref fallback) = self.fallback_provider {
                    fallback
                        .get_transaction_receipt(hash)
                        .await
                        .context("Fallback get_transaction_receipt failed")
                } else {
                    Err(anyhow::anyhow!("Failed to get receipt: {}", e))
                }
            }
        }
    }

    async fn fetch_transaction_count(&self) -> Result<u64> {
        match self
            .provider
            .get_transaction_count(self.signer_address)
            .await
        {
            Ok(n) => Ok(n),
            Err(e) => {
                eprintln!("PRIMARY RPC ERROR (get_transaction_count): {}", e);
                if let Some(ref fallback) = self.fallback_provider {
                    println!("FALLBACK: Switching to secondary RPC to get transaction count...");
                    fallback
                        .get_transaction_count(self.signer_address)
                        .await
                        .context("Fallback RPC get_transaction_count also failed")
                } else {
                    Err(anyhow::anyhow!(
                        "Failed to get transaction count from primary RPC and no fallback: {}",
                        e
                    ))
                }
            }
        }
    }

    async fn broadcast_tx(
        &self,
        tx: TransactionRequest,
    ) -> Result<alloy::providers::PendingTransactionBuilder<alloy::network::Ethereum>> {
        match self.provider.send_transaction(tx.clone()).await {
            Ok(pending) => Ok(pending),
            Err(e) => {
                eprintln!("PRIMARY RPC ERROR (send_transaction): {}", e);
                if let Some(ref fallback) = self.fallback_provider {
                    println!("FALLBACK: Switching to secondary RPC provider...");
                    fallback
                        .send_transaction(tx)
                        .await
                        .context("Fallback RPC send_transaction also failed")
                } else {
                    Err(anyhow::anyhow!(
                        "Primary RPC failed and no fallback configured: {}",
                        e
                    ))
                }
            }
        }
    }

    async fn send_tx_with_fallback(
        &self,
        mut tx: TransactionRequest,
    ) -> Result<TransactionOutcome> {
        let mut nonce_guard = self.nonce_lock.lock().await;

        // 1. Determine nonce atomically (cached -> RPC on-chain -> DB max)
        let rpc_nonce = self.fetch_transaction_count().await?;
        let db_max_nonce = if let Some(ref db) = self.db {
            db.get_max_relayer_nonce(&format!("0x{:x}", self.signer_address))
                .await
                .unwrap_or(None)
        } else {
            None
        };
        let on_chain_base = match db_max_nonce {
            Some(db_n) => rpc_nonce.max(db_n + 1),
            None => rpc_nonce,
        };
        let mut nonce = match *nonce_guard {
            Some(cached) => cached.max(on_chain_base),
            None => on_chain_base,
        };

        // Track initial gas parameters for replacement bumps
        let mut current_max_fee = tx.max_fee_per_gas;
        let mut current_priority_fee = tx.max_priority_fee_per_gas;
        let mut current_gas_price = tx.gas_price;

        tx = tx.with_nonce(nonce);

        // 2. Broadcast with nonce collision auto-recovery
        let pending_tx = match self.broadcast_tx(tx.clone()).await {
            Ok(p) => p,

            Err(e) => {
                let err_str = e.to_string();
                if is_nonce_error(&err_str) {
                    println!(
                        "RELAYER NONCE CONFLICT ({}). Resynchronizing with on-chain nonce...",
                        err_str
                    );
                    let fresh_nonce = self.fetch_transaction_count().await?;
                    nonce = fresh_nonce;
                    *nonce_guard = Some(nonce);
                    tx = tx.with_nonce(nonce);
                    self.broadcast_tx(tx.clone()).await.context(
                        "Failed to broadcast transaction even after nonce resynchronization",
                    )?
                } else {
                    *nonce_guard = None; // Reset cache on unknown broadcast failure
                    return Err(e);
                }
            }
        };

        let mut current_tx_hash = format!("0x{:x}", pending_tx.tx_hash());
        println!(
            "RELAYER: Broadcasted tx {} (nonce {})",
            current_tx_hash, nonce
        );

        // Record initial pending transaction in database
        if let Some(ref db) = self.db {
            let _ = db
                .record_relayer_tx(
                    &current_tx_hash,
                    nonce,
                    "pending",
                    &format!("0x{:x}", self.signer_address),
                )
                .await;
        }

        // 3. Receipt polling with replacement watchdog (stuck transaction speed-up)
        let timeout_secs = self.finality_config.tx_timeout_secs;
        let timeout_duration = std::time::Duration::from_secs(timeout_secs);
        let bump_pct = self.finality_config.gas_bump_percent;
        let max_bumps = self.finality_config.max_gas_bumps;
        let mut bump_count = 0;
        let mut pending_tx_opt = Some(pending_tx);

        let receipt = loop {
            let timeout_res = if let Some(p) = pending_tx_opt.take() {
                match tokio::time::timeout(timeout_duration, p.get_receipt()).await {
                    Ok(Ok(rcpt)) => Ok(rcpt),
                    Ok(Err(e)) => {
                        if let Ok(Some(rcpt)) = self.get_receipt(&current_tx_hash).await {
                            Ok(rcpt)
                        } else {
                            return Err(anyhow::anyhow!(
                                "Transaction receipt error for {}: {}",
                                current_tx_hash,
                                e
                            ));
                        }
                    }
                    Err(_) => Err(()), // timeout
                }
            } else {
                let start = std::time::Instant::now();
                let mut found = None;
                while start.elapsed() < timeout_duration {
                    if let Ok(Some(rcpt)) = self.get_receipt(&current_tx_hash).await {
                        found = Some(rcpt);
                        break;
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                }
                match found {
                    Some(rcpt) => Ok(rcpt),
                    None => Err(()), // timeout
                }
            };

            match timeout_res {
                Ok(rcpt) => break rcpt,
                Err(_) => {
                    println!(
                        "RELAYER WATCHDOG: Tx {} stuck in mempool (> {}s)",
                        current_tx_hash, timeout_secs
                    );

                    // Check if transaction confirmed right around timeout
                    if let Ok(Some(rcpt)) = self.get_receipt(&current_tx_hash).await {
                        println!(
                            "RELAYER: Tx {} confirmed during timeout check",
                            current_tx_hash
                        );
                        break rcpt;
                    }

                    if bump_count < max_bumps {
                        bump_count += 1;
                        println!(
                            "RELAYER: Escalating gas for stuck tx (bump #{}/{}, +{}% gas) with same nonce {}",
                            bump_count, max_bumps, bump_pct, nonce
                        );

                        // Bump gas fees by at least +bump_pct% (EIP-1559 compliance)
                        if let Some(mf) = current_max_fee {
                            let bumped_mf = calculate_gas_bump(mf, bump_pct);
                            current_max_fee = Some(bumped_mf);
                            tx = tx.clone().with_max_fee_per_gas(bumped_mf);
                        }
                        if let Some(pf) = current_priority_fee {
                            let bumped_pf = calculate_gas_bump(pf, bump_pct);
                            current_priority_fee = Some(bumped_pf);
                            tx = tx.clone().with_max_priority_fee_per_gas(bumped_pf);
                        }
                        if let Some(gp) = current_gas_price {
                            let bumped_gp = calculate_gas_bump(gp, bump_pct);
                            current_gas_price = Some(bumped_gp);
                            tx = tx.clone().with_gas_price(bumped_gp);
                        }
                        if current_max_fee.is_none() && current_gas_price.is_none() {
                            let gp = self.get_gas_price().await.unwrap_or(20_000_000);
                            let bumped_mf = calculate_gas_bump(gp * 125 / 100, bump_pct);
                            let bumped_pf = calculate_gas_bump(1_000_000, bump_pct);
                            current_max_fee = Some(bumped_mf);
                            current_priority_fee = Some(bumped_pf);
                            tx = tx
                                .clone()
                                .with_max_fee_per_gas(bumped_mf)
                                .with_max_priority_fee_per_gas(bumped_pf);
                        }

                        // Maintain the exact same nonce for replacement
                        tx = tx.clone().with_nonce(nonce);

                        match self.broadcast_tx(tx.clone()).await {
                            Ok(replacement_pending) => {
                                let new_hash = format!("0x{:x}", replacement_pending.tx_hash());
                                println!("RELAYER: Replacement tx broadcasted: {}", new_hash);
                                current_tx_hash = new_hash.clone();
                                pending_tx_opt = Some(replacement_pending);
                                if let Some(ref db) = self.db {
                                    let _ = db
                                        .record_relayer_tx(
                                            &new_hash,
                                            nonce,
                                            "pending",
                                            &format!("0x{:x}", self.signer_address),
                                        )
                                        .await;
                                }
                            }
                            Err(e) => {
                                eprintln!(
                                    "RELAYER WARNING: Replacement broadcast failed: {}. Checking if original landed...",
                                    e
                                );
                                if let Ok(Some(rcpt)) = self.get_receipt(&current_tx_hash).await {
                                    break rcpt;
                                }
                            }
                        }
                    } else {
                        eprintln!(
                            "RELAYER WARNING: Max gas bumps ({}) reached for nonce {}. Continuing to wait...",
                            max_bumps, nonce
                        );
                        if let Ok(Some(rcpt)) = self.get_receipt(&current_tx_hash).await {
                            break rcpt;
                        }
                        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                    }
                }
            }
        };

        // 4. Confirmation Threshold Verification (reorg protection)
        let receipt_block = receipt.block_number.unwrap_or(0);
        let mut confirmations = 1u64;
        let target_confirmations = self.finality_config.confirmation_threshold;

        if target_confirmations > 1 && receipt_block > 0 {
            println!(
                "RELAYER: Waiting for {} confirmations (mined in block {})...",
                target_confirmations, receipt_block
            );
            for _ in 0..60 {
                let current_block = self.get_block_number().await.unwrap_or(receipt_block);
                confirmations = current_block.saturating_sub(receipt_block) + 1;
                if confirmations >= target_confirmations {
                    println!(
                        "RELAYER: Target confirmations achieved: {}/{} (current block: {})",
                        confirmations, target_confirmations, current_block
                    );
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            }
        }

        // 5. Update local nonce counter & DB record
        *nonce_guard = Some(nonce + 1);

        let success = receipt.status();
        let gas_used = receipt.gas_used;
        let effective_gas_price = receipt.effective_gas_price;

        println!("RELAYER: Confirmed in block {}", receipt_block);
        println!("  Gas Used     : {}", gas_used);
        println!("  Confirmations: {}", confirmations);
        println!(
            "  Status       : {}",
            if success { "SUCCESS" } else { "FAILED" }
        );

        if let Some(ref db) = self.db {
            if success {
                let _ = db
                    .update_relayer_tx_confirmed(
                        &current_tx_hash,
                        receipt_block,
                        gas_used,
                        effective_gas_price,
                        confirmations,
                    )
                    .await;
            } else {
                let _ = db
                    .update_relayer_tx_failed(
                        &current_tx_hash,
                        Some(receipt_block),
                        Some(gas_used),
                        "transaction reverted on-chain",
                    )
                    .await;
            }
        }

        Ok(TransactionOutcome {
            tx_hash: current_tx_hash,
            block_number: receipt_block,
            success,
            gas_used,
            effective_gas_price,
            ccip_message_id: None,
            confirmations,
        })
    }

    /// Fetch gas price from blockchain with fallback
    pub async fn get_gas_price(&self) -> Result<u128> {
        // Try primary provider
        let result = self.provider.get_gas_price().await;

        match result {
            Ok(price) => Ok(price),
            Err(e) => {
                eprintln!("PRIMARY RPC ERROR (gas price): {}", e);

                // Fallback ke secondary provider
                if let Some(ref fallback) = self.fallback_provider {
                    println!("FALLBACK: Switching to secondary RPC for gas price...");
                    fallback
                        .get_gas_price()
                        .await
                        .context("Fallback RPC gas price juga gagal")
                } else {
                    Err(anyhow::anyhow!(
                        "Primary RPC gagal dan no fallback configured for gas price: {}",
                        e
                    ))
                }
            }
        }
    }

    /// Fetch relayer wallet balance from blockchain with fallback
    pub async fn get_relayer_balance_eth(&self) -> Result<f64> {
        let result = self.provider.get_balance(self.signer_address).await;

        let wei = match result {
            Ok(bal) => bal,
            Err(e) => {
                eprintln!("PRIMARY RPC ERROR (balance): {}", e);

                if let Some(ref fallback) = self.fallback_provider {
                    println!("FALLBACK: Switching to secondary RPC for balance...");
                    fallback
                        .get_balance(self.signer_address)
                        .await
                        .context("Fallback RPC balance juga gagal")?
                } else {
                    return Err(anyhow::anyhow!(
                        "Primary RPC gagal dan no fallback configured for balance: {}",
                        e
                    ));
                }
            }
        };

        let eth = wei.to::<u128>() as f64 / 1_000_000_000_000_000_000.0;
        Ok(eth)
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn broadcast_spend_transaction(
        &self,
        root_hex: &str,
        nullifier: &str,
        alpha_neg_hex: &str,
        pk_iss_hex: &str,
        recipient: &str,
        amount: u64,
        recipient_or_intent_hash_hex: &str,
        expiry: u64,
        nonce_hex: &str,
        max_execution_fee: u64,
        execution_fee: u64,
    ) -> Result<TransactionOutcome> {
        println!("RELAYER: Broadcasting spend transaction");
        println!(
            "  Nullifier   : {}...",
            &nullifier[..core::cmp::min(8, nullifier.len())]
        );
        println!("  Recipient   : {}", recipient);
        println!("  Amount      : {} USDC", amount as f64 / 1_000_000.0);

        // Parse root to FixedBytes<32>
        let root_bytes =
            hex::decode(root_hex.trim_start_matches("0x")).context("Invalid root hex")?;
        if root_bytes.len() != 32 {
            anyhow::bail!("Invalid root length: expected 32, got {}", root_bytes.len());
        }
        let mut root_fixed = [0u8; 32];
        root_fixed.copy_from_slice(&root_bytes);

        // Parse nullifier to FixedBytes<32>
        let nullifier_bytes =
            hex::decode(nullifier.trim_start_matches("0x")).context("Invalid nullifier hex")?;
        if nullifier_bytes.len() != 32 {
            anyhow::bail!(
                "Invalid nullifier length: expected 32, got {}",
                nullifier_bytes.len()
            );
        }
        let mut nullifier_fixed = [0u8; 32];
        nullifier_fixed.copy_from_slice(&nullifier_bytes);

        // Parse BLS signature components
        let alpha_neg_bytes =
            hex::decode(alpha_neg_hex.trim_start_matches("0x")).context("Invalid alpha_neg hex")?;
        if alpha_neg_bytes.len() != 128 {
            anyhow::bail!(
                "Invalid alpha_neg length: expected 128, got {}",
                alpha_neg_bytes.len()
            );
        }
        let pk_iss_bytes =
            hex::decode(pk_iss_hex.trim_start_matches("0x")).context("Invalid pk_iss hex")?;
        if pk_iss_bytes.len() != 256 {
            anyhow::bail!(
                "Invalid pk_iss length: expected 256, got {}",
                pk_iss_bytes.len()
            );
        }

        // Parse recipient address
        let recipient_addr = Address::from_str(recipient)
            .map_err(|e| anyhow::anyhow!("Invalid recipient address '{}': {}", recipient, e))?;

        // Validate address length (should be 20 bytes = 40 hex chars)
        if recipient.len() != 42 {
            return Err(anyhow::anyhow!(
                "Invalid recipient address length: expected 42 chars (0x + 40 hex), got {}",
                recipient.len()
            ));
        }

        let amount_u256 = U256::from(amount);

        // Parse recipient_or_intent_hash to FixedBytes<32>
        let recipient_or_intent_hash_bytes =
            hex::decode(recipient_or_intent_hash_hex.trim_start_matches("0x"))
                .context("Invalid recipient_or_intent_hash hex")?;
        if recipient_or_intent_hash_bytes.len() != 32 {
            anyhow::bail!("Invalid recipient_or_intent_hash length");
        }
        let mut recipient_or_intent_hash_fixed = [0u8; 32];
        recipient_or_intent_hash_fixed.copy_from_slice(&recipient_or_intent_hash_bytes);

        // Parse nonce to FixedBytes<32>
        let nonce_bytes =
            hex::decode(nonce_hex.trim_start_matches("0x")).context("Invalid nonce hex")?;
        if nonce_bytes.len() != 32 {
            anyhow::bail!("Invalid nonce length");
        }
        let mut nonce_fixed = [0u8; 32];
        nonce_fixed.copy_from_slice(&nonce_bytes);

        // Encode calldata using alloy's sol! macro type-safely
        let call_data = spendCall {
            root: root_fixed.into(),
            nullifier: nullifier_fixed.into(),
            alpha_neg_bytes: alpha_neg_bytes.into(),
            pk_iss_bytes: pk_iss_bytes.into(),
            recipient: recipient_addr,
            amount: amount_u256,
            recipient_or_intent_hash: recipient_or_intent_hash_fixed.into(),
            expiry: U256::from(expiry),
            nonce: nonce_fixed.into(),
            max_execution_fee: U256::from(max_execution_fee),
            execution_fee: U256::from(execution_fee),
        }
        .abi_encode();

        let gas_price = self.get_gas_price().await.unwrap_or(20_000_000);
        let max_fee = gas_price * 125 / 100;

        let tx = TransactionRequest::default()
            .with_to(self.contract_address)
            .with_value(U256::ZERO)
            .with_gas_limit(1_500_000)
            .with_max_fee_per_gas(max_fee)
            .with_max_priority_fee_per_gas(1_000_000)
            .with_input(Bytes::from(call_data));

        let outcome = self.send_tx_with_fallback(tx).await?;

        println!("RELAYER: Transaction broadcasted");
        println!("  Tx Hash     : {}", outcome.tx_hash);

        Ok(outcome)
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn broadcast_spend_private_note_transaction(
        &self,
        note_root_hex: &str,
        input_nullifier_hex: &str,
        output_commitment_hex: &str,
        recipient: &str,
        merchant_amount: u64,
        protocol_fee: u64,
        execution_fee: u64,
        max_execution_fee: u64,
        quote_hash_hex: &str,
        expiry: u64,
        has_change: u64,
        proof_a_neg_hex: &str,
        proof_b_hex: &str,
        proof_c_hex: &str,
    ) -> Result<TransactionOutcome> {
        println!("RELAYER: Broadcasting private note spend transaction (DEC-025)");
        println!(
            "  Nullifier       : {}...",
            &input_nullifier_hex[..core::cmp::min(10, input_nullifier_hex.len())]
        );
        println!("  Recipient       : {}", recipient);
        println!(
            "  Merchant Amount : {} USDC",
            merchant_amount as f64 / 1_000_000.0
        );
        println!(
            "  Protocol Fee    : {} USDC",
            protocol_fee as f64 / 1_000_000.0
        );
        println!(
            "  Execution Fee   : {} USDC",
            execution_fee as f64 / 1_000_000.0
        );
        println!("  Has Change      : {}", has_change == 1);

        let note_root_bytes =
            hex::decode(note_root_hex.trim_start_matches("0x")).context("Invalid note_root hex")?;
        if note_root_bytes.len() != 32 {
            anyhow::bail!(
                "Invalid note_root length: expected 32, got {}",
                note_root_bytes.len()
            );
        }
        let mut note_root_fixed = [0u8; 32];
        note_root_fixed.copy_from_slice(&note_root_bytes);

        let nullifier_bytes = hex::decode(input_nullifier_hex.trim_start_matches("0x"))
            .context("Invalid input_nullifier hex")?;
        if nullifier_bytes.len() != 32 {
            anyhow::bail!(
                "Invalid input_nullifier length: expected 32, got {}",
                nullifier_bytes.len()
            );
        }
        let mut nullifier_fixed = [0u8; 32];
        nullifier_fixed.copy_from_slice(&nullifier_bytes);

        let output_cm_bytes = hex::decode(output_commitment_hex.trim_start_matches("0x"))
            .unwrap_or_else(|_| vec![0u8; 32]);
        let mut output_cm_fixed = [0u8; 32];
        if output_cm_bytes.len() == 32 {
            output_cm_fixed.copy_from_slice(&output_cm_bytes);
        }

        let recipient_addr = Address::from_str(recipient)
            .map_err(|e| anyhow::anyhow!("Invalid recipient address '{}': {}", recipient, e))?;

        let quote_hash_bytes =
            hex::decode(quote_hash_hex.trim_start_matches("0x")).unwrap_or_else(|_| vec![0u8; 32]);
        let mut quote_hash_fixed = [0u8; 32];
        if quote_hash_bytes.len() == 32 {
            quote_hash_fixed.copy_from_slice(&quote_hash_bytes);
        }

        let proof_a_neg = hex::decode(proof_a_neg_hex.trim_start_matches("0x"))
            .context("Invalid proof_a_neg hex")?;
        let proof_b =
            hex::decode(proof_b_hex.trim_start_matches("0x")).context("Invalid proof_b hex")?;
        let proof_c =
            hex::decode(proof_c_hex.trim_start_matches("0x")).context("Invalid proof_c hex")?;

        let call_data = spendPrivateNoteCall {
            note_root: note_root_fixed.into(),
            input_nullifier: nullifier_fixed.into(),
            output_commitment: output_cm_fixed.into(),
            recipient: recipient_addr,
            merchant_amount: U256::from(merchant_amount),
            protocol_fee: U256::from(protocol_fee),
            execution_fee: U256::from(execution_fee),
            max_execution_fee: U256::from(max_execution_fee),
            quote_hash: quote_hash_fixed.into(),
            expiry: U256::from(expiry),
            has_change: U256::from(has_change),
            proof_a_neg: proof_a_neg.into(),
            proof_b: proof_b.into(),
            proof_c: proof_c.into(),
        }
        .abi_encode();

        let gas_price = self.get_gas_price().await.unwrap_or(20_000_000);
        let max_fee = gas_price * 125 / 100;

        let tx = TransactionRequest::default()
            .with_to(self.contract_address)
            .with_value(U256::ZERO)
            .with_gas_limit(2_500_000)
            .with_max_fee_per_gas(max_fee)
            .with_max_priority_fee_per_gas(1_000_000)
            .with_input(Bytes::from(call_data));

        let outcome = self.send_tx_with_fallback(tx).await?;

        println!("RELAYER: Private note spend transaction broadcasted");
        println!("  Tx Hash     : {}", outcome.tx_hash);

        Ok(outcome)
    }

    pub async fn broadcast_spend_batch(
        &self,
        items: &[BatchSpendItem],
    ) -> Result<TransactionOutcome> {
        if !(2..=8).contains(&items.len()) {
            anyhow::bail!("batch size must be between 2 and 8");
        }

        let mut roots = Vec::with_capacity(items.len());
        let mut nullifiers = Vec::with_capacity(items.len());
        let mut alpha_neg_items = Vec::with_capacity(items.len());
        let mut pk_iss_items = Vec::with_capacity(items.len());
        let mut recipients = Vec::with_capacity(items.len());
        let mut amounts = Vec::with_capacity(items.len());
        let mut recipient_or_intent_hashes = Vec::with_capacity(items.len());
        let mut expiries = Vec::with_capacity(items.len());
        let mut nonces = Vec::with_capacity(items.len());

        for item in items {
            let root = hex::decode(item.root_hex.trim_start_matches("0x"))
                .context("Invalid batch root hex")?;
            if root.len() != 32 {
                anyhow::bail!("Invalid batch root length");
            }
            let mut root_fixed = [0u8; 32];
            root_fixed.copy_from_slice(&root);
            roots.push(root_fixed.into());

            let nullifier = hex::decode(item.nullifier.trim_start_matches("0x"))
                .context("Invalid batch nullifier hex")?;
            if nullifier.len() != 32 {
                anyhow::bail!("Invalid batch nullifier length");
            }
            let mut nullifier_fixed = [0u8; 32];
            nullifier_fixed.copy_from_slice(&nullifier);
            nullifiers.push(nullifier_fixed.into());

            let alpha_neg = hex::decode(item.alpha_neg_hex.trim_start_matches("0x"))
                .context("Invalid batch alpha_neg hex")?;
            if alpha_neg.len() != 128 {
                anyhow::bail!("Invalid batch alpha_neg length");
            }
            alpha_neg_items.push(Bytes::from(alpha_neg));

            let pk_iss = hex::decode(item.pk_iss_hex.trim_start_matches("0x"))
                .context("Invalid batch pk_iss hex")?;
            if pk_iss.len() != 256 {
                anyhow::bail!("Invalid batch pk_iss length");
            }
            pk_iss_items.push(Bytes::from(pk_iss));

            recipients.push(Address::from_str(&item.recipient).context("Invalid batch recipient")?);
            amounts.push(U256::from(item.amount));

            let intent_hash =
                hex::decode(item.recipient_or_intent_hash_hex.trim_start_matches("0x"))
                    .context("Invalid batch recipient_or_intent_hash hex")?;
            if intent_hash.len() != 32 {
                anyhow::bail!("Invalid batch recipient_or_intent_hash length");
            }
            let mut intent_hash_fixed = [0u8; 32];
            intent_hash_fixed.copy_from_slice(&intent_hash);
            recipient_or_intent_hashes.push(intent_hash_fixed.into());

            expiries.push(U256::from(item.expiry));

            let nonce = hex::decode(item.nonce_hex.trim_start_matches("0x"))
                .context("Invalid batch nonce hex")?;
            if nonce.len() != 32 {
                anyhow::bail!("Invalid batch nonce length");
            }
            let mut nonce_fixed = [0u8; 32];
            nonce_fixed.copy_from_slice(&nonce);
            nonces.push(nonce_fixed.into());
        }

        let call_data = batchSpendCall {
            roots,
            nullifiers,
            alpha_neg_items,
            pk_iss_items,
            recipients,
            amounts,
            recipient_or_intent_hashes,
            expiries,
            nonces,
        }
        .abi_encode();

        let gas_price = self.get_gas_price().await.unwrap_or(20_000_000);
        let max_fee = gas_price * 125 / 100;
        let gas_limit = 250_000u64.saturating_add(850_000u64.saturating_mul(items.len() as u64));

        let tx = TransactionRequest::default()
            .with_to(self.contract_address)
            .with_value(U256::ZERO)
            .with_gas_limit(gas_limit)
            .with_max_fee_per_gas(max_fee)
            .with_max_priority_fee_per_gas(1_000_000)
            .with_input(Bytes::from(call_data));

        println!(
            "RELAYER: Broadcasting batch of {} same-chain spends",
            items.len()
        );
        self.send_tx_with_fallback(tx).await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn broadcast_ccip_transaction(
        &self,
        destination_chain_selector: u64,
        destination_contract: &str,
        nullifier_hex: &str,
        alpha_neg_hex: &str,
        pk_iss_hex: &str,
        recipient_hex: &str,
        collateral_token_hex: &str,
        condition_id_hex: Option<&str>,
        amount: u64,
        expiry: u64,
        nonce_hex: &str,
    ) -> Result<TransactionOutcome> {
        println!("RELAYER: Preparing CCIP Transaction");
        println!("  Destination Chain    : {}", destination_chain_selector);
        println!("  Destination Contract : {}", destination_contract);
        println!(
            "  Nullifier            : {}...",
            &nullifier_hex[..core::cmp::min(8, nullifier_hex.len())]
        );

        // 1. Construct the 584-byte payload
        let mut payload = vec![0u8; 584];

        let nullifier_bytes =
            hex::decode(nullifier_hex.trim_start_matches("0x")).context("Invalid nullifier hex")?;
        if nullifier_bytes.len() != 32 {
            anyhow::bail!("Invalid nullifier length");
        }
        payload[0..32].copy_from_slice(&nullifier_bytes);

        let alpha_neg_bytes =
            hex::decode(alpha_neg_hex.trim_start_matches("0x")).context("Invalid alpha_neg hex")?;
        if alpha_neg_bytes.len() != 128 {
            anyhow::bail!(
                "Invalid alpha_neg length: expected 128, got {}",
                alpha_neg_bytes.len()
            );
        }
        payload[32..160].copy_from_slice(&alpha_neg_bytes);

        let pk_iss_bytes =
            hex::decode(pk_iss_hex.trim_start_matches("0x")).context("Invalid pk_iss hex")?;
        if pk_iss_bytes.len() != 256 {
            anyhow::bail!(
                "Invalid pk_iss length: expected 256, got {}",
                pk_iss_bytes.len()
            );
        }
        payload[160..416].copy_from_slice(&pk_iss_bytes);

        let recipient_addr = Address::from_str(recipient_hex)
            .map_err(|e| anyhow::anyhow!("Invalid recipient address '{}': {}", recipient_hex, e))?;
        payload[416..436].copy_from_slice(recipient_addr.as_slice());

        let collateral_addr = Address::from_str(collateral_token_hex).map_err(|e| {
            anyhow::anyhow!(
                "Invalid collateral token address '{}': {}",
                collateral_token_hex,
                e
            )
        })?;
        payload[436..456].copy_from_slice(collateral_addr.as_slice());

        if let Some(cond_hex) = condition_id_hex {
            let cond_bytes = hex::decode(cond_hex.trim_start_matches("0x"))
                .context("Invalid condition_id hex")?;
            if cond_bytes.len() != 32 {
                anyhow::bail!("Invalid condition_id length");
            }
            payload[456..488].copy_from_slice(&cond_bytes);
        }

        let amount_u256 = U256::from(amount);
        let amount_bytes = amount_u256.to_be_bytes::<32>();
        payload[488..520].copy_from_slice(&amount_bytes);

        let expiry_u256 = U256::from(expiry);
        let expiry_bytes = expiry_u256.to_be_bytes::<32>();
        payload[520..552].copy_from_slice(&expiry_bytes);

        let nonce_bytes =
            hex::decode(nonce_hex.trim_start_matches("0x")).context("Invalid nonce hex")?;
        if nonce_bytes.len() != 32 {
            anyhow::bail!("Invalid nonce length");
        }
        payload[552..584].copy_from_slice(&nonce_bytes);

        // 2. Encode EVMExtraArgsV2 with allowOutOfOrderExecution = true (Aha! Moment - Cyfrin/Chainlink 2026 Audit)
        // Tag bytes: 0x181dcf10
        let extra_args_struct = EVMExtraArgsV2 {
            gasLimit: U256::from(750_000), // safe margin for BLS + Polymarket execution on dest chain
            allowOutOfOrderExecution: true,
        };
        let mut extra_args = vec![0x18, 0x1d, 0xcf, 0x10];
        extra_args.extend_from_slice(&extra_args_struct.abi_encode());

        // 3. Construct the receiver bytes (abi.encode(address))
        let dest_addr = Address::from_str(destination_contract).map_err(|e| {
            anyhow::anyhow!(
                "Invalid destination contract '{}': {}",
                destination_contract,
                e
            )
        })?;
        let mut receiver_bytes = vec![0u8; 32];
        receiver_bytes[12..32].copy_from_slice(dest_addr.as_slice());

        // 4. Construct EVM2AnyMessage
        let message = EVM2AnyMessage {
            receiver: receiver_bytes.into(),
            data: payload.into(),
            tokenAmounts: vec![],
            feeToken: Address::ZERO, // Pay in native gas token (ETH)
            extraArgs: extra_args.into(),
        };

        // 5. Query CCIP Fee from the Router Contract
        println!(
            "RELAYER: Querying CCIP fee from Router at {}...",
            self.ccip_router_address
        );
        let get_fee_call = IRouterClient::getFeeCall {
            destinationChainSelector: destination_chain_selector,
            message: message.clone(),
        }
        .abi_encode();

        let fee_tx = TransactionRequest::default()
            .with_to(self.ccip_router_address)
            .with_input(Bytes::from(get_fee_call));

        let fee_hex = self
            .provider
            .call(fee_tx)
            .await
            .context("Failed to call getFee on CCIP Router")?;

        let ccip_fee = if fee_hex.len() >= 32 {
            U256::from_be_slice(&fee_hex[..32])
        } else {
            U256::ZERO
        };
        println!(
            "  CCIP Fee (Native): {} wei ({:.6} ETH)",
            ccip_fee,
            ccip_fee.to::<u128>() as f64 * 1e-18
        );

        // 6. Build and broadcast the ccipSend call transaction
        let ccip_send_call = IRouterClient::ccipSendCall {
            destinationChainSelector: destination_chain_selector,
            message,
        }
        .abi_encode();

        let gas_price = self.get_gas_price().await.unwrap_or(20_000_000);
        let max_fee = gas_price * 125 / 100;

        let tx = TransactionRequest::default()
            .with_to(self.ccip_router_address)
            .with_value(ccip_fee)
            .with_gas_limit(1_200_000) // CCIP router calls require substantial gas on source chain
            .with_max_fee_per_gas(max_fee)
            .with_max_priority_fee_per_gas(1_000_000)
            .with_input(Bytes::from(ccip_send_call));

        let mut outcome = self.send_tx_with_fallback(tx).await?;

        // Parse CCIPMessageSent event to extract messageId
        if outcome.success {
            if let Ok(Some(receipt)) = self
                .provider
                .get_transaction_receipt(outcome.tx_hash.parse().unwrap())
                .await
            {
                // CCIPMessageSent event signature: keccak256("CCIPMessageSent((bytes32,uint64,address,bytes,bytes,address,uint256,uint256,(address,uint256)[],bytes))")
                let event_signature =
                    alloy::primitives::keccak256("CCIPMessageSent((bytes32,uint64,address,bytes,bytes,address,uint256,uint256,(address,uint256)[],bytes))");

                for log in receipt.inner.logs() {
                    let log_address = log.address();
                    if log_address == self.ccip_router_address {
                        if let Some(topic0) = log.topics().first() {
                            if *topic0 == event_signature {
                                // messageId is the first indexed topic (topic1)
                                if let Some(topic1) = log.topics().get(1) {
                                    let topic1_bytes: &[u8] = topic1.as_slice();
                                    let message_id = format!("0x{}", hex::encode(topic1_bytes));
                                    println!("  CCIP Message ID      : {}", message_id);
                                    outcome.ccip_message_id = Some(message_id);
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }

        if outcome.ccip_message_id.is_none() {
            println!("  CCIP Message ID      : (not found in logs)");
        }

        println!("RELAYER: Real CCIP transaction broadcasted successfully");
        println!("  Source Tx Hash       : {}", outcome.tx_hash);

        Ok(outcome)
    }

    pub fn signer_address(&self) -> String {
        format!("0x{:x}", self.signer_address)
    }

    #[allow(dead_code)]
    #[allow(clippy::too_many_arguments)]
    pub async fn sign_leader_payload(
        &self,
        session_id: &str,
        blinded_hex: &str,
        k_hex: &str,
        timestamp: u64,
        amount: u64,
        client_address: &str,
        com_k_hex: &str,
    ) -> Result<String, anyhow::Error> {
        use alloy::signers::Signer;
        let message = format!(
            "{}:{}:{}:{}:{}:{}:{}",
            session_id, blinded_hex, k_hex, timestamp, amount, client_address, com_k_hex
        );
        let signature = self.signer.sign_message(message.as_bytes()).await?;
        Ok(hex::encode(signature.as_bytes()))
    }

    pub async fn get_clean_root_timestamp(&self, root_hex: &str) -> Result<u64> {
        let root_bytes =
            hex::decode(root_hex.trim_start_matches("0x")).context("Invalid root hex")?;
        if root_bytes.len() != 32 {
            anyhow::bail!("Root must be 32 bytes, got {}", root_bytes.len());
        }
        let mut root = [0u8; 32];
        root.copy_from_slice(&root_bytes);

        let call_data = getCleanRootTimestampCall { root: root.into() }.abi_encode();

        let tx = TransactionRequest::default()
            .with_to(self.contract_address)
            .with_input(Bytes::from(call_data));

        let result = self
            .provider
            .call(tx)
            .await
            .context("Failed to call getCleanRootTimestamp")?;

        if result.len() >= 32 {
            Ok(U256::from_be_slice(&result[..32]).to::<u64>())
        } else {
            Ok(0)
        }
    }

    /// Check if a nullifier has already been spent on-chain (DEC-019)
    pub async fn is_nullifier_spent(&self, nullifier_hex: &str) -> Result<bool> {
        let clean_hex = nullifier_hex.trim_start_matches("0x");
        let nullifier_bytes =
            hex::decode(clean_hex).map_err(|e| anyhow::anyhow!("Invalid nullifier hex: {}", e))?;
        if nullifier_bytes.len() != 32 {
            anyhow::bail!("Nullifier must be 32 bytes, got {}", nullifier_bytes.len());
        }
        let mut nullifier = [0u8; 32];
        nullifier.copy_from_slice(&nullifier_bytes);

        let call_data = isNullifierSpentCall {
            nullifier: nullifier.into(),
        }
        .abi_encode();

        let tx = TransactionRequest::default()
            .with_to(self.contract_address)
            .with_input(Bytes::from(call_data));

        let result = self
            .provider
            .call(tx)
            .await
            .context("Failed to call isNullifierSpent on-chain")?;

        if !result.is_empty() {
            // ABI bool is decoded as a 32-byte word with last byte 0 or 1
            Ok(result.last().copied().unwrap_or(0) != 0)
        } else {
            Ok(false)
        }
    }

    /// Official Chainalysis Sanctions Oracle address on Arbitrum and Ethereum
    pub const CHAINALYSIS_SANCTIONS_ORACLE_ADDRESS: &'static str =
        "0x40C57923924B5c5c5455c48D93317139ADDaC8fb";

    /// Check if an address is sanctioned on-chain via the official Chainalysis Sanctions Oracle.
    /// Performs a free view call via eth_call (0 gas cost).
    pub async fn is_sanctioned_on_chain(&self, address: &Address) -> Result<bool> {
        let oracle_addr =
            Address::from_str(Self::CHAINALYSIS_SANCTIONS_ORACLE_ADDRESS).unwrap_or(Address::ZERO);

        let call_data = isSanctionedCall { addr: *address }.abi_encode();

        let tx = TransactionRequest::default()
            .with_to(oracle_addr)
            .with_input(Bytes::from(call_data));

        let result = self
            .provider
            .call(tx)
            .await
            .context("Failed to query Chainalysis Sanctions Oracle on-chain")?;

        if !result.is_empty() {
            // ABI bool return is decoded from the last byte of the 32-byte word
            Ok(result.last().copied().unwrap_or(0) != 0)
        } else {
            Ok(false)
        }
    }

    /// Get the contract address as a hex string
    pub fn contract_address(&self) -> String {
        format!("{:?}", self.contract_address)
    }

    /// Get the chain ID from the provider
    pub async fn chain_id(&self) -> Result<u64> {
        self.provider
            .get_chain_id()
            .await
            .context("Failed to get chain ID")
    }

    /// Claim accumulated execution fees from the contract
    /// Returns the transaction outcome
    pub async fn claim_execution_fees(&self, amount: u64) -> Result<TransactionOutcome> {
        println!(
            "EVM_CLIENT: Claiming {} USDC execution fees from contract",
            amount / 1_000_000
        );

        let call_data = claimExecutionFeesCall {
            amount: U256::from(amount),
        }
        .abi_encode();

        let tx = TransactionRequest::default()
            .with_to(self.contract_address)
            .with_input(Bytes::from(call_data))
            .with_gas_limit(100_000);

        self.send_tx_with_fallback(tx).await
    }

    /// Query on-chain DepositFee events in a block range [from_block, to_block] (DEC-018)
    pub async fn get_deposit_events(
        &self,
        from_block: u64,
        to_block: u64,
    ) -> Result<Vec<DepositEventInfo>> {
        let filter = Filter::new()
            .address(self.contract_address)
            .event_signature(DepositFee::SIGNATURE_HASH)
            .from_block(from_block)
            .to_block(to_block);

        let logs = self
            .provider
            .get_logs(&filter)
            .await
            .context("Failed to get deposit logs")?;

        let mut events = Vec::new();
        for log in logs {
            if log.address() != self.contract_address {
                continue;
            }
            if let Ok(decoded) = DepositFee::decode_raw_log(log.topics(), &log.data().data) {
                let session_id_hex = format!("0x{}", hex::encode(decoded.session_id.as_slice()));
                let com_k_hash_hex = format!("0x{}", hex::encode(decoded.com_k_hash.as_slice()));
                let client_hex = format!("{:?}", decoded.client);
                let tx_hash = log
                    .transaction_hash
                    .map(|h| format!("{:#x}", h))
                    .unwrap_or_default();
                let block_number = log.block_number.unwrap_or(0);

                events.push(DepositEventInfo {
                    session_id: session_id_hex,
                    com_k_hash: com_k_hash_hex,
                    client: client_hex,
                    gross_amount: decoded.gross_amount.to::<u64>(),
                    fee: decoded.fee.to::<u64>(),
                    net_amount: decoded.net_amount.to::<u64>(),
                    tx_hash,
                    block_number,
                });
            }
        }

        Ok(events)
    }

    /// Check if a specific transaction contains a verified on-chain deposit event (DEC-018)
    pub async fn check_deposit_tx_on_chain(
        &self,
        tx_hash: &str,
        session_id: &str,
    ) -> Result<Option<DepositEventInfo>> {
        let hash: alloy::primitives::TxHash =
            tx_hash.parse().context("Invalid transaction hash")?;
        let receipt = match self.provider.get_transaction_receipt(hash).await? {
            Some(rcpt) => rcpt,
            None => return Ok(None),
        };

        if !receipt.status() {
            return Ok(None);
        }

        let block_num = receipt.block_number.unwrap_or(0);
        let current_block = self.get_block_number().await.unwrap_or(block_num);
        let confirmations = if current_block >= block_num {
            current_block - block_num + 1
        } else {
            1
        };

        if confirmations < self.finality_config.confirmation_threshold {
            return Ok(None);
        }

        let norm_sid = session_id.trim_start_matches("0x").to_lowercase();

        for log in receipt.inner.logs() {
            if log.address() == self.contract_address {
                if let Ok(decoded) = DepositFee::decode_raw_log(log.topics(), &log.data().data) {
                    let ev_sid = hex::encode(decoded.session_id.as_slice()).to_lowercase();
                    if ev_sid == norm_sid {
                        let session_id_hex = format!("0x{}", ev_sid);
                        let com_k_hash_hex =
                            format!("0x{}", hex::encode(decoded.com_k_hash.as_slice()));
                        let client_hex = format!("{:?}", decoded.client);

                        return Ok(Some(DepositEventInfo {
                            session_id: session_id_hex,
                            com_k_hash: com_k_hash_hex,
                            client: client_hex,
                            gross_amount: decoded.gross_amount.to::<u64>(),
                            fee: decoded.fee.to::<u64>(),
                            net_amount: decoded.net_amount.to::<u64>(),
                            tx_hash: tx_hash.to_string(),
                            block_number: block_num,
                        }));
                    }
                }
            }
        }

        Ok(None)
    }
}

/// Information extracted from an on-chain DepositFee event (DEC-018)
#[derive(Clone, Debug, PartialEq)]
pub struct DepositEventInfo {
    pub session_id: String,
    pub com_k_hash: String,
    pub client: String,
    pub gross_amount: u64,
    pub fee: u64,
    pub net_amount: u64,
    pub tx_hash: String,
    pub block_number: u64,
}

/// Detects whether an error message indicates an EVM transaction nonce conflict or mismatch.
pub fn is_nonce_error(err_str: &str) -> bool {
    let lower = err_str.to_lowercase();
    lower.contains("nonce too low")
        || lower.contains("nonce too high")
        || lower.contains("already known")
        || lower.contains("transaction already exists")
        || lower.contains("replacement transaction underpriced")
        || lower.contains("nonce has already been used")
        || lower.contains("invalid transaction nonce")
}

/// Calculates the escalated gas fee for transaction replacement, ensuring at least +1 wei increase.
pub fn calculate_gas_bump(base: u128, bump_pct: u64) -> u128 {
    let bump = (base.saturating_mul(bump_pct as u128)) / 100;
    base.saturating_add(bump.max(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_nonce_error_detection() {
        assert!(is_nonce_error("execution reverted: nonce too low"));
        assert!(is_nonce_error(
            "nonce too high (gap in Nitro sequencer buffer)"
        ));
        assert!(is_nonce_error("transaction already exists in mempool"));
        assert!(is_nonce_error("replacement transaction underpriced"));
        assert!(is_nonce_error("ALREADY KNOWN"));
        assert!(!is_nonce_error(
            "insufficient funds for gas * price + value"
        ));
        assert!(!is_nonce_error("execution reverted: invalid proof"));
    }

    #[test]
    fn test_calculate_gas_bump_minimum_and_percentage() {
        // Zero or small base gets at least +1 wei
        assert_eq!(calculate_gas_bump(0, 15), 1);
        assert_eq!(calculate_gas_bump(1, 15), 2);

        // Standard gas price: 20 Gwei + 15% bump = 23 Gwei
        let twenty_gwei = 20_000_000_000u128;
        let expected_twenty_three = 23_000_000_000u128;
        assert_eq!(calculate_gas_bump(twenty_gwei, 15), expected_twenty_three);

        // 100 + 10% = 110
        assert_eq!(calculate_gas_bump(100, 10), 110);
    }

    #[test]
    fn test_transaction_outcome_confirmations_and_config() {
        let outcome = TransactionOutcome {
            tx_hash: "0x123".to_string(),
            block_number: 100,
            success: true,
            gas_used: 50_000,
            effective_gas_price: 20_000_000,
            ccip_message_id: None,
            confirmations: 5,
        };
        assert_eq!(outcome.confirmations, 5);
        assert!(outcome.success);

        let cfg = crate::config::ReceiptFinalityConfig::default();
        assert_eq!(cfg.confirmation_threshold, 1);
        assert_eq!(cfg.tx_timeout_secs, 30);
        assert_eq!(cfg.gas_bump_percent, 15);
        assert_eq!(cfg.max_gas_bumps, 3);
    }

    #[test]
    fn test_chainalysis_oracle_call_encoding() {
        let addr = Address::from_str("0x8576acc5c05d6ce88f4e49bf65bdf0c62f91353c").unwrap();
        let call = isSanctionedCall { addr };
        let encoded = call.abi_encode();
        // Selector for isSanctioned(address) should be 4 bytes followed by 32 bytes address word
        assert_eq!(encoded.len(), 36);

        let oracle_addr =
            Address::from_str(EvmClient::CHAINALYSIS_SANCTIONS_ORACLE_ADDRESS).unwrap();
        assert_ne!(oracle_addr, Address::ZERO);
    }
}
