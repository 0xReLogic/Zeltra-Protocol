use alloy::{
    network::{EthereumWallet, TransactionBuilder},
    primitives::{Address, U256, Bytes},
    providers::{Provider, ProviderBuilder, WsConnect, DynProvider},
    rpc::types::eth::TransactionRequest,
    signers::local::PrivateKeySigner,
    sol,
    sol_types::SolCall,
};
use anyhow::{Context, Result};
use std::str::FromStr;

sol! {
    function spend(
        bytes32 nullifier,
        bytes calldata alpha_neg_bytes,
        bytes calldata hm_bytes,
        bytes calldata pk_iss_bytes,
        address recipient,
        uint256 amount
    ) external returns (bool);
}

pub struct EvmClient {
    provider: DynProvider,
    fallback_provider: Option<DynProvider>,
    signer_address: Address,
    contract_address: Address,
}

impl EvmClient {
    pub async fn new(rpc_url: &str, private_key: &str, contract_address: &str) -> Result<Self> {
        let signer = PrivateKeySigner::from_str(private_key)
            .context("Private key invalid")?;
        
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
            let url = rpc_url.parse::<alloy::transports::http::reqwest::Url>()
                .context("Invalid HTTP RPC URL")?;
            let p = ProviderBuilder::new()
                .wallet(wallet.clone())
                .connect_http(url);
            DynProvider::new(p)
        };

        // Fallback provider (opsional, dari env var NIMBUS_RPC_FALLBACK_URL)
        let fallback_provider = if let Ok(fallback_url) = std::env::var("NIMBUS_RPC_FALLBACK_URL") {
            let fallback_wallet = EthereumWallet::from(signer);
            
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
                    Err(e) => Err(anyhow::anyhow!("Invalid fallback HTTP URL: {}", e))
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

        let contract_addr = Address::from_str(contract_address)
            .context("Contract address invalid")?;

        Ok(Self {
            provider,
            fallback_provider,
            signer_address,
            contract_address: contract_addr,
        })
    }

    async fn send_tx_with_fallback(&self, tx: TransactionRequest) -> Result<String> {
        // Try primary provider
        let result = self.provider.send_transaction(tx.clone()).await;
        
        let pending_tx = match result {
            Ok(pending) => pending,
            Err(e) => {
                eprintln!("PRIMARY RPC ERROR: {}", e);
                
                // Fallback ke secondary provider
                if let Some(ref fallback) = self.fallback_provider {
                    println!("FALLBACK: Switching to secondary RPC provider...");
                    fallback.send_transaction(tx)
                        .await
                        .context("Fallback RPC juga gagal")?
                } else {
                    return Err(anyhow::anyhow!("Primary RPC gagal dan no fallback configured: {}", e));
                }
            }
        };

        let tx_hash = format!("0x{:x}", pending_tx.tx_hash());
        
        // Spawn async receipt monitoring
        tokio::spawn(async move {
            if let Ok(receipt) = pending_tx.get_receipt().await {
                println!("RELAYER: Confirmed in block {}", receipt.block_number.unwrap_or(0));
                println!("  Gas Used    : {}", receipt.gas_used);
                println!("  Status      : {}", if receipt.status() { "SUCCESS" } else { "FAILED" });
            }
        });

        Ok(tx_hash)
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
                    fallback.get_gas_price()
                        .await
                        .context("Fallback RPC gas price juga gagal")
                } else {
                    Err(anyhow::anyhow!("Primary RPC gagal dan no fallback configured for gas price: {}", e))
                }
            }
        }
    }

    pub async fn broadcast_spend_transaction(
        &self,
        nullifier: &str,
        alpha_neg_hex: &str,
        hm_hex: &str,
        pk_iss_hex: &str,
        recipient: &str,
        amount: u64,
    ) -> Result<String> {
        println!("RELAYER: Broadcasting spend transaction");
        println!("  Nullifier   : {}...", &nullifier[..core::cmp::min(8, nullifier.len())]);
        println!("  Recipient   : {}", recipient);
        println!("  Amount      : {} USDC", amount as f64 / 1_000_000.0);

        // Parse nullifier to FixedBytes<32>
        let nullifier_bytes = hex::decode(nullifier.trim_start_matches("0x"))
            .context("Invalid nullifier hex")?;
        let mut nullifier_fixed = [0u8; 32];
        if nullifier_bytes.len() >= 32 {
            nullifier_fixed.copy_from_slice(&nullifier_bytes[..32]);
        }

        // Parse BLS signature components
        let alpha_neg_bytes = hex::decode(alpha_neg_hex.trim_start_matches("0x"))
            .context("Invalid alpha_neg hex")?;
        let hm_bytes = hex::decode(hm_hex.trim_start_matches("0x"))
            .context("Invalid hm hex")?;
        let pk_iss_bytes = hex::decode(pk_iss_hex.trim_start_matches("0x"))
            .context("Invalid pk_iss hex")?;

        // Parse recipient address
        let recipient_addr = Address::from_str(recipient)
            .map_err(|e| anyhow::anyhow!("Invalid recipient address '{}': {}", recipient, e))?;
        
        // Validate address length (should be 20 bytes = 40 hex chars)
        if recipient.len() != 42 {
            return Err(anyhow::anyhow!("Invalid recipient address length: expected 42 chars (0x + 40 hex), got {}", recipient.len()));
        }

        let amount_u256 = U256::from(amount);

        // Encode calldata using alloy's sol! macro type-safely
        let call_data = spendCall {
            nullifier: nullifier_fixed.into(),
            alpha_neg_bytes: alpha_neg_bytes.into(),
            hm_bytes: hm_bytes.into(),
            pk_iss_bytes: pk_iss_bytes.into(),
            recipient: recipient_addr,
            amount: amount_u256,
        }.abi_encode();

        let gas_price = self.get_gas_price().await.unwrap_or(20_000_000);
        let max_fee = gas_price * 125 / 100;

        let tx = TransactionRequest::default()
            .with_to(self.contract_address)
            .with_value(U256::ZERO)
            .with_gas_limit(1_000_000)
            .with_max_fee_per_gas(max_fee)
            .with_max_priority_fee_per_gas(1_000_000)
            .with_input(Bytes::from(call_data));

        let tx_hash = self.send_tx_with_fallback(tx).await?;
        
        println!("RELAYER: Transaction broadcasted");
        println!("  Tx Hash     : {}", tx_hash);

        Ok(tx_hash)
    }

    pub async fn broadcast_ccip_transaction(
        &self,
        destination_chain_selector: u64,
        destination_contract: &str,
        nullifier: &str,
        _amount: u64,
    ) -> Result<String> {
        println!("RELAYER: Broadcasting CCIP transaction");
        println!("  Destination Chain    : {}", destination_chain_selector);
        println!("  Destination Contract : {}", destination_contract);
        println!("  Nullifier            : {}...", &nullifier[..core::cmp::min(8, nullifier.len())]);

        let gas_price = self.get_gas_price().await.unwrap_or(20_000_000);
        let max_fee = gas_price * 125 / 100;

        let tx = TransactionRequest::default()
            .with_to(self.contract_address)
            .with_value(U256::ZERO)
            .with_gas_limit(800_000)
            .with_max_fee_per_gas(max_fee)
            .with_max_priority_fee_per_gas(1_000_000);

        let tx_hash = self.send_tx_with_fallback(tx).await?;
        
        println!("RELAYER: CCIP transaction broadcasted");
        println!("  Tx Hash              : {}", tx_hash);
        println!("  CCIP Message ID      : {} (derived from tx hash)", tx_hash);

        Ok(tx_hash)
    }

    pub fn signer_address(&self) -> String {
        format!("0x{:x}", self.signer_address)
    }
}
