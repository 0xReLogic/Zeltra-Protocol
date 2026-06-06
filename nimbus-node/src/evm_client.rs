use alloy::{
    network::{EthereumWallet, TransactionBuilder},
    primitives::{Address, U256},
    providers::{Provider, ProviderBuilder, WsConnect, DynProvider},
    rpc::types::eth::TransactionRequest,
    signers::local::PrivateKeySigner,
};
use anyhow::{Context, Result};
use std::str::FromStr;

pub struct EvmClient {
    provider: DynProvider,
    signer_address: Address,
    contract_address: Address,
}

impl EvmClient {
    pub async fn new(rpc_url: &str, private_key: &str, contract_address: &str) -> Result<Self> {
        let ws = WsConnect::new(rpc_url);
        
        let signer = PrivateKeySigner::from_str(private_key)
            .context("Private key invalid")?;
        
        let signer_address = signer.address();
        let wallet = EthereumWallet::from(signer);
        
        let provider = ProviderBuilder::new()
            .wallet(wallet)
            .connect_ws(ws)
            .await
            .context("Gagal connect ke RPC")?;

        let contract_addr = Address::from_str(contract_address)
            .context("Contract address invalid")?;

        Ok(Self {
            provider: DynProvider::new(provider),
            signer_address,
            contract_address: contract_addr,
        })
    }

    pub async fn broadcast_spend_transaction(
        &self,
        nullifier: &str,
        recipient: &str,
        amount: u64,
    ) -> Result<String> {
        println!("RELAYER: Broadcasting spend transaction");
        println!("  Nullifier   : {}...", &nullifier[..core::cmp::min(12, nullifier.len())]);
        println!("  Recipient   : {}", recipient);
        println!("  Amount      : {} USDC", amount as f64 / 1_000_000.0);

        let tx = TransactionRequest::default()
            .with_to(self.contract_address)
            .with_value(U256::ZERO)
            .with_gas_limit(500_000);

        let pending_tx = self.provider
            .send_transaction(tx)
            .await
            .context("Gagal broadcast transaction")?;

        let tx_hash = format!("0x{:x}", pending_tx.tx_hash());
        
        println!("RELAYER: Transaction broadcasted");
        println!("  Tx Hash     : {}", tx_hash);

        tokio::spawn(async move {
            if let Ok(receipt) = pending_tx.get_receipt().await {
                println!("RELAYER: Confirmed in block {}", receipt.block_number.unwrap_or(0));
                println!("  Gas Used    : {}", receipt.gas_used);
                println!("  Status      : {}", if receipt.status() { "SUCCESS" } else { "FAILED" });
            }
        });

        Ok(tx_hash)
    }

    pub async fn broadcast_ccip_transaction(
        &self,
        destination_chain_selector: &str,
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

        let pending_tx = self.provider
            .send_transaction(tx)
            .await
            .context("Gagal broadcast CCIP transaction")?;

        let tx_hash = format!("0x{:x}", pending_tx.tx_hash());
        
        println!("RELAYER: CCIP transaction broadcasted");
        println!("  Tx Hash              : {}", tx_hash);
        println!("  CCIP Message ID      : {} (derived from tx hash)", tx_hash);

        tokio::spawn(async move {
            if let Ok(receipt) = pending_tx.get_receipt().await {
                println!("RELAYER: CCIP confirmed in block {}", receipt.block_number.unwrap_or(0));
                println!("  Gas Used    : {}", receipt.gas_used);
            }
        });

        Ok(tx_hash)
    }

    pub fn signer_address(&self) -> String {
        format!("0x{:x}", self.signer_address)
    }
}
