//! Spend handler and batch processing logic
//!
//! Security features:
//! - Slippage protection (min_payout, deadline) -- Finding #9
//! - Minimum balance checks before batch processing -- Finding #10
//! - Idempotency key support for request deduplication -- Finding #16

use axum::Json;
use crate::{state::AppState, dto::*};

pub async fn handle_spend(
    axum::extract::State(state): axum::extract::State<AppState>,
    Json(payload): Json<SpendRequest>,
) -> Json<SpendResponse> {
    // --- Idempotency check (Finding #16) ---
    if let Some(ref idem_key) = payload.idempotency_key {
        match state.db.check_idempotency(idem_key, "spend").await {
            Ok(Some(cached_json)) => {
                if let Ok(cached_resp) = serde_json::from_str::<SpendResponse>(&cached_json) {
                    println!("IDEMPOTENCY: Returning cached response for key {}...", &idem_key[..8.min(idem_key.len())]);
                    return Json(cached_resp);
                }
            }
            Ok(None) => { /* No cached response, proceed normally */ }
            Err(e) => {
                eprintln!("RELAYER WARNING: Idempotency check failed: {}", e);
                // Proceed anyway -- idempotency is best-effort
            }
        }
    }

    // --- Deadline check (Finding #9 -- slippage protection) ---
    if let Some(deadline) = payload.deadline {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        if now > deadline {
            let response = SpendResponse {
                status: "REJECTED".to_string(),
                message: format!(
                    "Transaction deadline expired. Deadline: {}, Current: {}",
                    deadline, now
                ),
                queue_position: 0,
                estimated_gas_usdc: None,
            };
            // Cache rejection for idempotency
            if let Some(ref idem_key) = payload.idempotency_key {
                if let Ok(json) = serde_json::to_string(&response) {
                    let _ = state.db.store_idempotency(idem_key, "spend", &json).await;
                }
            }
            return Json(response);
        }
    }

    // --- Double-spend check ---
    match state.db.is_nullifier_spent(&payload.nullifier).await {
        Ok(true) => {
            return Json(SpendResponse {
                status: "REJECTED".to_string(),
                message: "Double-spending detected. Nullifier already exists.".to_string(),
                queue_position: 0,
                estimated_gas_usdc: None,
            });
        }
        Ok(false) => {
            // Nullifier not spent, proceed
        }
        Err(e) => {
            eprintln!("RELAYER ERROR: Database nullifier check failed: {}", e);
            return Json(SpendResponse {
                status: "ERROR".to_string(),
                message: "Database error".to_string(),
                queue_position: 0,
                estimated_gas_usdc: None,
            });
        }
    }

    // Push request to batch queue
    let mut queue = state.spend_queue.lock().await;
    let position = queue.len() + 1;
    queue.push(payload.clone());

    if let Some(auth) = &payload.eip7702_auth {
        println!("RELAYER: Received spend request via EIP-7702 delegation");
        println!("  EOA Address       : {}", auth.eoa_address);
        println!("  Delegate Contract : {}", auth.delegate_contract);
        println!("  Status            : Queued for batching");
    } else {
        println!("RELAYER: Received standard spend request (Queued)");
    }

    let response = SpendResponse {
        status: "QUEUED".to_string(),
        message: "Spend transaction accepted into batching queue".to_string(),
        queue_position: position,
        estimated_gas_usdc: None,
    };

    // Store idempotency response
    if let Some(ref idem_key) = payload.idempotency_key {
        if let Ok(json) = serde_json::to_string(&response) {
            let _ = state.db.store_idempotency(idem_key, "spend", &json).await;
        }
    }

    Json(response)
}

// Background batching logic with slippage protection and balance checks
pub async fn process_spend_batch(state: &AppState) {
    let mut queue = state.spend_queue.lock().await;
    if queue.is_empty() {
        return;
    }

    let batch_size = queue.len();
    println!("------------------------------------------------------------");
    println!("PROCESSING BATCH: Submitting {} transactions to L2...", batch_size);

    // 1. Fetch dynamic gas price from blockchain (via circuit breaker)
    let l2_gas_price_wei = if let Some(ref evm_client) = state.evm_client {
        let evm = evm_client.clone();
        let cb = state.rpc_circuit_breaker.clone();
        match cb.call(|| async move {
            evm.get_gas_price().await.map_err(|e| format!("{}", e))
        }).await {
            Ok(price) => {
                println!("  Dynamic gas price: {} wei ({} gwei)", price, price as f64 / 1e9);
                price as f64
            }
            Err(e) => {
                eprintln!("WARNING: Gas price fetch failed (circuit breaker: {}), using fallback", e);
                0.2e9 // Fallback to 0.2 gwei
            }
        }
    } else {
        eprintln!("WARNING: No EVM client configured, using default gas price");
        0.2e9 // Default fallback
    };

    // 2. L2 Gas Parameters & Economics (arXiv:2505.19556 - Batch-Calibrated)
    let eth_usd_price = 3500.0;
    let l2_gas_price_eth = l2_gas_price_wei * 1e-18; // Convert wei to ETH
    let l2_exec_gas_per_tx = 120_000.0;
    let l1_calldata_gas_per_tx = 80_000.0;
    let l1_base_batch_fee_eth = 0.0001; // Lower for testnet (was 0.005 = $17.50)

    // Calculate individual vs batch cost to determine savings
    let ind_gas_cost_eth = (l2_exec_gas_per_tx + l1_calldata_gas_per_tx) * l2_gas_price_eth + l1_base_batch_fee_eth;
    let ind_gas_cost_usd = ind_gas_cost_eth * eth_usd_price;

    let shared_l1_batch_fee_eth = l1_base_batch_fee_eth / batch_size as f64;
    let batch_gas_cost_per_tx_eth = (l2_exec_gas_per_tx + l1_calldata_gas_per_tx) * l2_gas_price_eth + shared_l1_batch_fee_eth;
    let batch_gas_cost_per_tx_usd = batch_gas_cost_per_tx_eth * eth_usd_price;

    let savings_per_tx_eth = ind_gas_cost_eth - batch_gas_cost_per_tx_eth;

    // Relayer takes a 10% markup from the user's savings as operational profit
    let markup_fee_eth = savings_per_tx_eth * 0.10;
    let markup_fee_usd = markup_fee_eth * eth_usd_price;

    let total_charge_per_tx_eth = batch_gas_cost_per_tx_eth + markup_fee_eth;
    let total_charge_per_tx_usd = total_charge_per_tx_eth * eth_usd_price;

    let total_batch_cost_eth = batch_gas_cost_per_tx_eth * batch_size as f64;
    let total_batch_profit_usd = markup_fee_usd * batch_size as f64;

    // --- Finding #10: Minimum balance check before processing ---
    match state.check_balance_for_batch(total_batch_cost_eth).await {
        Ok(_) => { /* Balance sufficient, proceed */ }
        Err(e) => {
            eprintln!("BATCH REJECTED: {}", e);
            println!("  Action: Batch of {} transactions deferred until balance is replenished", batch_size);
            println!("------------------------------------------------------------");
            // Do NOT clear the queue -- keep transactions for retry when balance is replenished
            return;
        }
    }

    // Update relayer wallet balance and profit
    {
        let mut relayer_bal = state.relayer_wallet_balance_eth.lock().await;
        *relayer_bal -= total_batch_cost_eth;
        let mut relayer_profit = state.relayer_accumulated_profit_usdc.lock().await;
        *relayer_profit += total_batch_profit_usd;

        println!("  L2 Gas Economics (arXiv:2505.19556 - Batch-Calibrated Gas):");
        println!("    - Individual Tx Cost Estimate : {:.5} ETH (${:.2})", ind_gas_cost_eth, ind_gas_cost_usd);
        println!("    - Actual Batched Cost per Tx  : {:.5} ETH (${:.2})", batch_gas_cost_per_tx_eth, batch_gas_cost_per_tx_usd);
        println!("    - Savings per User            : {:.5} ETH (${:.2})", savings_per_tx_eth, savings_per_tx_eth * eth_usd_price);
        println!("    - Relayer 10% Savings Markup  : {:.5} ETH (${:.2})", markup_fee_eth, markup_fee_usd);
        println!("    - Total Charge to User        : {:.5} ETH (${:.2})", total_charge_per_tx_eth, total_charge_per_tx_usd);
        println!("    - Relayer Signer Gas Balance  : {:.5} ETH", *relayer_bal);
        println!("    - Relayer Accumulated Profit  : {:.2} USDC", *relayer_profit);
    }

    // Anonymization: Shuffle the batch queue to prevent timing correlation/metadata analysis
    {
        use rand::seq::SliceRandom;
        let mut rng = rand::thread_rng();
        queue.shuffle(&mut rng);
        println!("  Anonymization: Shuffled batch to break timing correlation.");
    }
    
    // Process each transaction with slippage protection
    let mut rejected_indices: Vec<usize> = Vec::new();
    
    for (idx, request) in queue.iter().enumerate() {
        // --- Finding #9: Deadline enforcement during batch processing ---
        if let Some(deadline) = request.deadline {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            if now > deadline {
                eprintln!(
                    "  SLIPPAGE-REJECT: Transaction {}... deadline expired (deadline={}, now={})",
                    &request.nullifier[..8.min(request.nullifier.len())],
                    deadline,
                    now
                );
                rejected_indices.push(idx);
                continue;
            }
        }

        // --- Finding #9: Minimum payout check ---
        let charge_usdc_units = (total_charge_per_tx_usd * 1_000_000.0) as u64;
        let net_payout = if request.amount >= charge_usdc_units {
            request.amount - charge_usdc_units
        } else {
            0
        };

        if let Some(min_payout) = request.min_payout {
            if net_payout < min_payout {
                eprintln!(
                    "  SLIPPAGE-REJECT: Transaction {}... net_payout ({:.2} USDC) < min_payout ({:.2} USDC)",
                    &request.nullifier[..8.min(request.nullifier.len())],
                    net_payout as f64 / 1_000_000.0,
                    min_payout as f64 / 1_000_000.0
                );
                rejected_indices.push(idx);
                continue;
            }
        }

        // Register nullifier in persistent database to prevent double-spend
        match state.db.check_and_insert_nullifier(&request.nullifier, None).await {
            Ok(true) => {
                // Nullifier successfully registered (first time spend)
            }
            Ok(false) => {
                // This should never happen as we check in handle_spend, but log it
                eprintln!("WARNING: Nullifier {}... already exists during batch processing!", &request.nullifier[..8.min(request.nullifier.len())]);
                continue;
            }
            Err(e) => {
                eprintln!("ERROR: Failed to register nullifier {}...: {}", &request.nullifier[..8.min(request.nullifier.len())], e);
                continue;
            }
        }

        println!("  - Spent to: {}, Original: {:.2} USDC, Net Payout: {:.2} USDC, Nullifier: {}...", 
            request.recipient, 
            request.amount as f64 / 1_000_000.0,
            net_payout as f64 / 1_000_000.0,
            &request.nullifier[..8.min(request.nullifier.len())]
        );
        
        if let Some(auth) = &request.eip7702_auth {
            println!("    [EIP-7702] Executed smart wallet call for EOA: {}", auth.eoa_address);
        }

        // Broadcast transaction to L2 (regular spend or CCIP)
        if let Some(cc) = &request.cross_chain {
            println!("    [CCIP Lintas Rantai] Menginisiasi transaksi lintas rantai via Chainlink CCIP:");
            println!("      Selector Rantai Tujuan : {}", cc.destination_chain_selector);
            println!("      Kontrak Penerima       : {}", cc.destination_contract);
            println!("      Status                 : Dispatched ke CCIP Router");
            
            // Real CCIP transaction broadcasting (or fallback to mock)
            let ccip_message_id = if let Some(ref evm_client) = state.evm_client {
                match evm_client.broadcast_ccip_transaction(
                    cc.destination_chain_selector,
                    &cc.destination_contract,
                    &request.nullifier,
                    request.amount,
                ).await {
                    Ok(message_id) => message_id,
                    Err(e) => {
                        eprintln!("CCIP ERROR: Failed to broadcast CCIP transaction: {}", e);
                        format!("0x{} (FAILED)", hex::encode(rand::random::<[u8; 16]>()))
                    }
                }
            } else {
                println!("      WARNING: Using mock CCIP message ID (dev mode - EVM client not configured)");
                format!("0x{}", hex::encode(rand::random::<[u8; 32]>()))
            };
            
            println!("      Message ID             : {}", ccip_message_id);
        } else {
            // Regular spend transaction (non-CCIP)
            if let Some(ref evm_client) = state.evm_client {
                match evm_client.broadcast_spend_transaction(
                    &request.nullifier,
                    &request.alpha_neg_hex,
                    &request.hm_hex,
                    &request.pk_iss_hex,
                    &request.recipient,
                    net_payout,
                ).await {
                    Ok(tx_hash) => {
                        println!("    [SPEND] Transaction broadcasted successfully");
                        println!("      Tx Hash: {}", tx_hash);
                    }
                    Err(e) => {
                        eprintln!("    [SPEND] ERROR: Failed to broadcast transaction: {}", e);
                    }
                }
            } else {
                println!("    [SPEND] WARNING: Using mock mode (dev mode - EVM client not configured)");
            }
        }
    }

    if !rejected_indices.is_empty() {
        println!("  SLIPPAGE: {} transactions rejected due to slippage/deadline protection", rejected_indices.len());
    }

    // Clear queue
    queue.clear();
    println!("BATCH COMPLETED SUCCESSFULLY");
    println!("------------------------------------------------------------");
}
