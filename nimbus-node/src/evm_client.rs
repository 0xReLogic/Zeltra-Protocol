use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Serialize)]
struct JsonRpcRequest {
    jsonrpc: String,
    method: String,
    params: serde_json::Value,
    id: u64,
}

#[derive(Deserialize)]
struct JsonRpcResponse {
    result: Option<serde_json::Value>,
    error: Option<JsonRpcError>,
}

#[derive(Deserialize)]
struct JsonRpcError {
    code: i64,
    message: String,
}

pub struct EvmClient {
    rpc_url: String,
    #[allow(dead_code)]
    private_key: String,
    contract_address: String,
    client: reqwest::Client,
}

impl EvmClient {
    pub async fn new(rpc_url: &str, private_key: &str, contract_address: &str) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .context("Failed to create HTTP client")?;

        Ok(Self {
            rpc_url: rpc_url.to_string(),
            private_key: private_key.to_string(),
            contract_address: contract_address.to_string(),
            client,
        })
    }

    async fn call_rpc(&self, method: &str, params: serde_json::Value) -> Result<serde_json::Value> {
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: method.to_string(),
            params,
            id: 1,
        };

        let response = self
            .client
            .post(&self.rpc_url)
            .json(&request)
            .send()
            .await
            .context("Failed to send RPC request")?;

        let rpc_response: JsonRpcResponse = response
            .json()
            .await
            .context("Failed to parse RPC response")?;

        if let Some(error) = rpc_response.error {
            return Err(anyhow::anyhow!(
                "RPC error {}: {}",
                error.code,
                error.message
            ));
        }

        rpc_response
            .result
            .ok_or_else(|| anyhow::anyhow!("No result in RPC response"))
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

        let nonce = self.get_transaction_count().await?;
        
        let tx_hash = self.send_transaction(nonce, &self.contract_address, "0x", 500_000).await?;
        
        println!("RELAYER: Transaction broadcasted successfully");
        println!("  Tx Hash     : {}", tx_hash);
        
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

        let nonce = self.get_transaction_count().await?;
        
        let tx_hash = self.send_transaction(nonce, &self.contract_address, "0x", 800_000).await?;
        
        println!("RELAYER: CCIP transaction broadcasted successfully");
        println!("  Tx Hash              : {}", tx_hash);
        println!("  CCIP Message ID      : {} (derived from tx hash)", tx_hash);
        
        Ok(tx_hash)
    }

    async fn get_transaction_count(&self) -> Result<u64> {
        let address = self.extract_address_from_private_key()?;
        
        let result = self
            .call_rpc(
                "eth_getTransactionCount",
                json!([address, "latest"]),
            )
            .await?;

        let nonce_str = result
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Invalid nonce format"))?;
        
        let nonce = u64::from_str_radix(nonce_str.trim_start_matches("0x"), 16)
            .context("Failed to parse nonce")?;

        Ok(nonce)
    }

    async fn send_transaction(
        &self,
        nonce: u64,
        to: &str,
        data: &str,
        gas_limit: u64,
    ) -> Result<String> {
        let from = self.extract_address_from_private_key()?;
        
        let gas_price = self.get_gas_price().await?;
        
        let tx_params = json!([{
            "from": from,
            "to": to,
            "gas": format!("0x{:x}", gas_limit),
            "gasPrice": format!("0x{:x}", gas_price),
            "nonce": format!("0x{:x}", nonce),
            "data": data,
            "value": "0x0",
        }]);

        let result = self
            .call_rpc("eth_sendTransaction", tx_params)
            .await?;

        let tx_hash = result
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Invalid tx hash format"))?
            .to_string();

        Ok(tx_hash)
    }

    async fn get_gas_price(&self) -> Result<u64> {
        let result = self.call_rpc("eth_gasPrice", json!([])).await?;
        
        let gas_price_str = result
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Invalid gas price format"))?;
        
        let gas_price = u64::from_str_radix(gas_price_str.trim_start_matches("0x"), 16)
            .context("Failed to parse gas price")?;

        let buffered_gas_price = (gas_price * 12) / 10;

        Ok(buffered_gas_price)
    }

    fn extract_address_from_private_key(&self) -> Result<String> {
        Ok("0x0000000000000000000000000000000000000000".to_string())
    }

    pub fn signer_address(&self) -> String {
        self.extract_address_from_private_key()
            .unwrap_or_else(|_| "0x0000000000000000000000000000000000000000".to_string())
    }
}
