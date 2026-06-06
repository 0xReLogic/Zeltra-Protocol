use alloy::{
    network::{EthereumWallet, TransactionBuilder},
    primitives::{Address, U256, Bytes, utils::keccak256},
    providers::{Provider, ProviderBuilder, WsConnect, DynProvider},
    rpc::types::eth::TransactionRequest,
    signers::local::PrivateKeySigner,
};
use anyhow::{Context, Result};
use std::str::FromStr;

pub struct EvmClient {
    provider: DynProvider,
    fallback_provider: Option<DynProvider>,
    signer_address: Address,
    contract_address: Address,
}

impl EvmClient {
    pub async fn new(rpc_url: &str, private_key: &str, contract_address: &str) -> Result<Self> {
        let ws = WsConnect::new(rpc_url);
        
        let signer = PrivateKeySigner::from_str(private_key)
            .context("Private key invalid")?;
        
        let signer_address = signer.address();
        let wallet = EthereumWallet::from(signer.clone());
        
        let provider = ProviderBuilder::new()
            .wallet(wallet)
            .connect_ws(ws)
            .await
            .context("Gagal connect ke RPC primary")?;

        // Fallback provider (opsional, dari env var NIMBUS_RPC_FALLBACK_URL)
        let fallback_provider = if let Ok(fallback_url) = std::env::var("NIMBUS_RPC_FALLBACK_URL") {
            let fallback_ws = WsConnect::new(&fallback_url);
            let fallback_wallet = EthereumWallet::from(signer);
            
            match ProviderBuilder::new()
                .wallet(fallback_wallet)
                .connect_ws(fallback_ws)
                .await
            {
                Ok(p) => {
                    println!("  Fallback RPC: {}", fallback_url);
                    Some(DynProvider::new(p))
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
            provider: DynProvider::new(provider),
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
        println!("  Nullifier   : {}...", &nullifier[..core::cmp::min(12, nullifier.len())]);
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

        // Calculate function selector: spend(bytes32,bytes,bytes,bytes,address,uint256)
        let function_signature = "spend(bytes32,bytes,bytes,bytes,address,uint256)";
        let hash = keccak256(function_signature.as_bytes());
        let function_selector = [hash[0], hash[1], hash[2], hash[3]];
        
        // Encode function call data
        let mut call_data = Vec::new();
        
        // Add function selector (4 bytes)
        call_data.extend_from_slice(&function_selector);
        
        // Add nullifier (32 bytes)
        call_data.extend_from_slice(&nullifier_fixed);
        
        // Add alpha_neg_bytes (dynamic - offset + length + data)
        let offset: u32 = 32 + 32 + 32 + 32 + 20; // nullifier + offsets for 3 dynamic params + recipient
        call_data.extend_from_slice(&offset.to_be_bytes());
        
        // Add hm_bytes offset
        let offset_hm: u32 = offset + 32 + alpha_neg_bytes.len() as u32;
        call_data.extend_from_slice(&offset_hm.to_be_bytes());
        
        // Add pk_iss_bytes offset
        let offset_pk: u32 = offset_hm + 32 + hm_bytes.len() as u32;
        call_data.extend_from_slice(&offset_pk.to_be_bytes());
        
        // Add recipient (20 bytes, padded to 32)
        call_data.extend_from_slice(recipient_addr.as_slice());
        call_data.extend_from_slice(&[0u8; 12]);
        
        // Add amount (32 bytes)
        call_data.extend_from_slice(&amount_u256.to_be_bytes::<32>());
        
        // Add alpha_neg_bytes data
        call_data.extend_from_slice(&(alpha_neg_bytes.len() as u32).to_be_bytes());
        call_data.extend_from_slice(&alpha_neg_bytes);
        
        // Add hm_bytes data
        call_data.extend_from_slice(&(hm_bytes.len() as u32).to_be_bytes());
        call_data.extend_from_slice(&hm_bytes);
        
        // Add pk_iss_bytes data
        call_data.extend_from_slice(&(pk_iss_bytes.len() as u32).to_be_bytes());
        call_data.extend_from_slice(&pk_iss_bytes);

        let tx = TransactionRequest::default()
            .with_to(self.contract_address)
            .with_value(U256::ZERO)
            .with_gas_limit(1_000_000)
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
        println!("  Nullifier            : {}...", &nullifier[..core::cmp::min(12, nullifier.len())]);

        let tx = TransactionRequest::default()
            .with_to(self.contract_address)
            .with_value(U256::ZERO)
            .with_gas_limit(800_000);

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
