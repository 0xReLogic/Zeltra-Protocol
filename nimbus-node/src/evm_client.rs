use alloy::{
    network::{EthereumWallet, TransactionBuilder},
    primitives::{Address, Bytes, U256},
    providers::{DynProvider, Provider, ProviderBuilder, WsConnect},
    rpc::types::eth::TransactionRequest,
    signers::local::PrivateKeySigner,
    sol,
    sol_types::{SolCall, SolValue},
};
use anyhow::{Context, Result};
use std::str::FromStr;

sol! {
    function spend(
        bytes32 root,
        bytes32 nullifier,
        bytes calldata alpha_neg_bytes,
        bytes calldata pk_iss_bytes,
        address recipient,
        uint256 amount,
        bytes32 recipient_or_intent_hash,
        uint256 expiry,
        bytes32 nonce
    ) external returns (bool);

    function batchSpend(
        bytes32[] calldata roots,
        bytes32[] calldata nullifiers,
        bytes[] calldata alphaNegItems,
        bytes[] calldata pkIssItems,
        address[] calldata recipients,
        uint256[] calldata amounts,
        bytes32[] calldata recipientOrIntentHashes,
        uint256[] calldata expiries,
        bytes32[] calldata nonces
    ) external returns (bool);

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
}

#[derive(Clone, Debug)]
pub struct TransactionOutcome {
    pub tx_hash: String,
    pub block_number: u64,
    pub success: bool,
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

        Ok(Self {
            provider,
            fallback_provider,
            signer_address,
            contract_address: contract_addr,
            ccip_router_address,
            signer,
        })
    }

    async fn send_tx_with_fallback(&self, tx: TransactionRequest) -> Result<TransactionOutcome> {
        // Try primary provider
        let result = self.provider.send_transaction(tx.clone()).await;

        let pending_tx = match result {
            Ok(pending) => pending,
            Err(e) => {
                eprintln!("PRIMARY RPC ERROR: {}", e);

                // Fallback ke secondary provider
                if let Some(ref fallback) = self.fallback_provider {
                    println!("FALLBACK: Switching to secondary RPC provider...");
                    fallback
                        .send_transaction(tx)
                        .await
                        .context("Fallback RPC juga gagal")?
                } else {
                    return Err(anyhow::anyhow!(
                        "Primary RPC gagal dan no fallback configured: {}",
                        e
                    ));
                }
            }
        };

        let tx_hash = format!("0x{:x}", pending_tx.tx_hash());
        let receipt = pending_tx
            .get_receipt()
            .await
            .context("transaction broadcasted but receipt was not confirmed")?;
        let block_number = receipt.block_number.unwrap_or(0);
        let success = receipt.status();
        println!("RELAYER: Confirmed in block {}", block_number);
        println!("  Gas Used    : {}", receipt.gas_used);
        println!(
            "  Status      : {}",
            if success { "SUCCESS" } else { "FAILED" }
        );

        Ok(TransactionOutcome {
            tx_hash,
            block_number,
            success,
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
            anyhow::bail!(
                "Invalid root length: expected 32, got {}",
                root_bytes.len()
            );
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
        }
        .abi_encode();

        let gas_price = self.get_gas_price().await.unwrap_or(20_000_000);
        let max_fee = gas_price * 125 / 100;

        let tx = TransactionRequest::default()
            .with_to(self.contract_address)
            .with_value(U256::ZERO)
            .with_gas_limit(1_000_000)
            .with_max_fee_per_gas(max_fee)
            .with_max_priority_fee_per_gas(1_000_000)
            .with_input(Bytes::from(call_data));

        let outcome = self.send_tx_with_fallback(tx).await?;

        println!("RELAYER: Transaction broadcasted");
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
            alphaNegItems: alpha_neg_items,
            pkIssItems: pk_iss_items,
            recipients,
            amounts,
            recipientOrIntentHashes: recipient_or_intent_hashes,
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

        let outcome = self.send_tx_with_fallback(tx).await?;

        println!("RELAYER: Real CCIP transaction broadcasted successfully");
        println!("  Source Tx Hash       : {}", outcome.tx_hash);
        println!("  CCIP Message ID      : pending event parsing");

        Ok(outcome)
    }

    pub fn signer_address(&self) -> String {
        format!("0x{:x}", self.signer_address)
    }

    #[allow(dead_code)]
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
}
