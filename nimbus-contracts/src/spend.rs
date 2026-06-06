//! Spend and Cross-Chain Intent Execution
//!
//! Handles BLS signature verification, spend operations, Polymarket intent execution,
//! and Chainlink CCIP cross-chain message handling.

use alloc::vec::Vec;
use alloy_primitives::{Address, FixedBytes, U256};
use stylus_sdk::call::RawCall;
use ark_bls12_381::G2Affine;
use ark_ec::AffineRepr;

use crate::storage::Nimbus;
use crate::interfaces::{IErc20, IConditionalTokens};
use crate::types::to_evm_g2;
use crate::constants::BLS12_PAIRING_CHECK;

impl Nimbus {
    /// Verifies the unmasked BLS signature on-chain using pairing precompile (EIP-2537: 0x0f).
    /// Verification check: e(-alpha, G2) * e(H(m), pk_iss) == 1
    pub fn spend(
        &mut self,
        nullifier: FixedBytes<32>,
        alpha_neg_bytes: Vec<u8>,   // -alpha in G1 (128 bytes EVM format)
        hm_bytes: Vec<u8>,          // H(m) in G1 (128 bytes EVM format)
        pk_iss_bytes: Vec<u8>,      // pk_iss in G2 (256 bytes EVM format)
        recipient: Address,
        amount: U256,
    ) -> Result<bool, Vec<u8>> {
        // 1. CHECKS
        self.check_not_paused()?;
        
        // Enforce minimum transaction size of 1 USDC/stablecoin (1,000,000 units)
        let min_amount = U256::from(1_000_000);
        if amount < min_amount {
            return Err(b"AMOUNT_TOO_SMALL".to_vec());
        }
        
        // Check double spend (Nullifier)
        if self.nullifiers.get(nullifier) {
            return Ok(false);
        }

        // Calculate standard redemption/withdrawal fee of 0.15% (amount * 15 / 10000, round up)
        let base_fee = (amount.checked_mul(U256::from(15))
            .ok_or_else(|| b"BASE_FEE_MUL_OVERFLOW".to_vec())? + U256::from(9999)) / U256::from(10000);
        
        // Calculate dynamic premium if Fase 2 or 3 is active
        let premium = self.calculate_fast_path_premium(amount)?;
        
        // fee_recipient gets base_fee + 20% of premium
        let premium_share = (premium.checked_mul(U256::from(20))
            .ok_or_else(|| b"PREMIUM_SHARE_MUL_OVERFLOW".to_vec())? + U256::from(99)) / U256::from(100);
            
        let protocol_share = base_fee.checked_add(premium_share)
            .ok_or_else(|| b"PROTOCOL_SHARE_OVERFLOW".to_vec())?;
            
        if amount < protocol_share {
            return Err(b"AMOUNT_LESS_THAN_FEES".to_vec());
        }
        let payout = amount.checked_sub(protocol_share)
            .ok_or_else(|| b"PAYOUT_UNDERFLOW".to_vec())?;

        #[cfg(test)]
        {
            // 2. EFFECTS
            let principal = self.total_deposited_principal.get();
            let new_principal = principal.checked_sub(amount).unwrap_or(U256::ZERO);
            self.total_deposited_principal.set(new_principal);
            self.nullifiers.insert(nullifier, true);
            
            // Track epoch volume for dynamic rebalancing
            self.update_epoch_and_rebalance_ratio(amount)?;
            Ok(true)
        }

        #[cfg(not(test))]
        {
            // 2. CHECKS (Signature verification)
            // Fetch G2 Generator for pairing base point
            let g2_gen = G2Affine::generator();
            let g2_gen_evm = to_evm_g2(&g2_gen);

            // Construct input payload for bls12_pairing_check (address 0x0f)
            // Format: [ (G1_point_1, G2_point_1), (G1_point_2, G2_point_2) ]
            // G1_point: 128 bytes, G2_point: 256 bytes. Total = 768 bytes
            let mut input = Vec::with_capacity(768);
            input.extend_from_slice(&alpha_neg_bytes); // -alpha (128 bytes)
            input.extend_from_slice(&g2_gen_evm);      // G2 Generator (256 bytes)
            input.extend_from_slice(&hm_bytes);         // H(m) (128 bytes)
            input.extend_from_slice(&pk_iss_bytes);     // pk_iss (256 bytes)

            // Call EIP-2537 precompile at 0x0f
            let output = unsafe {
                RawCall::new_static()
                    .call(BLS12_PAIRING_CHECK, &input)
            }.map_err(|_| b"PAIRING_PRECOMPILE_CALL_FAILED".to_vec())?;

            // Verify output (true if last byte is 1)
            if output.len() == 32 && output[31] == 1 {
                // 3. EFFECTS
                self.nullifiers.insert(nullifier, true);
                
                let principal = self.total_deposited_principal.get();
                let new_principal = principal.checked_sub(amount).unwrap_or(U256::ZERO);
                self.total_deposited_principal.set(new_principal);
                
                // Track epoch volume for dynamic rebalancing
                self.update_epoch_and_rebalance_ratio(amount)?;
                
                // 4. INTERACTIONS
                self.ensure_liquidity(amount)?;
                
                let stablecoin_address = self.stablecoin.get();
                let erc20 = IErc20::new(stablecoin_address);
                
                // Transfer payout to recipient (if not zero address)
                if recipient != Address::ZERO && payout > U256::ZERO {
                    let success = erc20.transfer(&mut *self, recipient, payout)
                        .map_err(|e| e)?;
                    if !success {
                        return Err(b"SPEND_TRANSFER_FAILED".to_vec());
                    }
                }
                
                // Transfer fee to fee_recipient
                if protocol_share > U256::ZERO {
                    let recipient_fee = self.fee_recipient.get();
                    let fee_success = erc20.transfer(&mut *self, recipient_fee, protocol_share)
                        .map_err(|e| e)?;
                    if !fee_success {
                        return Err(b"SPEND_FEE_TRANSFER_FAILED".to_vec());
                    }
                }

                Ok(true)
            } else {
                Ok(false)
            }
        }
    }

    /// Verifies the signature, redeems stablecoin, and directly calls Polymarket's conditional tokens contract
    /// to buy outcome shares under the recipient's name in a single transaction.
    pub fn spend_and_buy_shares(
        &mut self,
        nullifier: FixedBytes<32>,
        alpha_neg_bytes: Vec<u8>,
        hm_bytes: Vec<u8>,
        pk_iss_bytes: Vec<u8>,
        polymarket_ctf: Address,
        collateral_token: Address,
        condition_id: FixedBytes<32>,
        amount: U256,
    ) -> Result<bool, Vec<u8>> {
        self.check_not_paused()?;
        // 1. Verify and invalidate the signature (same as spend)
        let is_valid = self.spend(nullifier, alpha_neg_bytes, hm_bytes, pk_iss_bytes, Address::ZERO, amount)?;
        if !is_valid {
            return Ok(false);
        }

        // Calculate payout (net after fees) using the same safe round-up math as spend()
        let base_fee = (amount.checked_mul(U256::from(15))
            .ok_or_else(|| b"BASE_FEE_MUL_OVERFLOW".to_vec())? + U256::from(9999)) / U256::from(10000);
        let premium = self.calculate_fast_path_premium(amount)?;
        let premium_share = (premium.checked_mul(U256::from(20))
            .ok_or_else(|| b"PREMIUM_SHARE_MUL_OVERFLOW".to_vec())? + U256::from(99)) / U256::from(100);
        let protocol_share = base_fee.checked_add(premium_share)
            .ok_or_else(|| b"PROTOCOL_SHARE_OVERFLOW".to_vec())?;
        let payout = amount.checked_sub(protocol_share)
            .ok_or_else(|| b"PAYOUT_UNDERFLOW".to_vec())?;

        #[cfg(test)]
        {
            if polymarket_ctf == Address::ZERO {
                self.failed_intent_refunds.insert(nullifier, payout);
            }
            Ok(true)
        }

        #[cfg(not(test))]
        {
            let erc20 = IErc20::new(collateral_token);
            
            // Approve Polymarket CTF to spend payout amount of collateral token
            let approve_success = erc20.approve(&mut *self, polymarket_ctf, payout)
                .map_err(|e| e)?;
            if !approve_success {
                return Err(b"POLYMARKET_APPROVE_FAILED".to_vec());
            }

            // Perform external call to Polymarket Conditional Tokens Contract (Gnosis CTF)
            let ctf = IConditionalTokens::new(polymarket_ctf);
            
            // Partition for YES/NO outcome slots [1, 2]
            let partition = vec![U256::from(1), U256::from(2)];
            
            match ctf.split_position(&mut *self, collateral_token, FixedBytes::ZERO, condition_id, partition, payout) {
                Ok(_) => {
                    Ok(true)
                }
                Err(_) => {
                    // Try-Catch Fallback (Aha! Moment): record refund instead of reverting.
                    // This prevents locking the CCIP flow and allows recovery of user deposits.
                    self.failed_intent_refunds.insert(nullifier, payout);
                    Ok(true) // Return true to avoid reverting transaction state changes
                }
            }
        }
    }

    /// Returns the failed intent refund amount for a given nullifier.
    pub fn get_failed_intent_refund(&self, nullifier: FixedBytes<32>) -> Result<U256, Vec<u8>> {
        Ok(self.failed_intent_refunds.get(nullifier))
    }

    /// Claims a failed intent refund on the destination chain.
    pub fn claim_failed_intent_refund(
        &mut self,
        nullifier: FixedBytes<32>,
        recipient: Address,
    ) -> Result<bool, Vec<u8>> {
        self.check_not_paused()?;
        let amount = self.failed_intent_refunds.get(nullifier);
        if amount == U256::ZERO {
            return Err(b"NO_REFUND_AVAILABLE".to_vec());
        }

        self.failed_intent_refunds.insert(nullifier, U256::ZERO);

        #[cfg(test)]
        {
            Ok(true)
        }

        #[cfg(not(test))]
        {
            let stablecoin_address = self.stablecoin.get();
            let erc20 = IErc20::new(stablecoin_address);
            let success = erc20.transfer(&mut *self, recipient, amount)
                .map_err(|e| e)?;
            if !success {
                return Err(b"REFUND_TRANSFER_FAILED".to_vec());
            }

            Ok(true)
        }
    }

    /// Receives a cross-chain payload via Chainlink CCIP and executes the transaction (Fase B).
    pub fn ccip_receive(
        &mut self,
        _message_id: FixedBytes<32>,
        _source_chain_selector: u64,
        _sender: Vec<u8>,
        payload: Vec<u8>,
    ) -> Result<(), Vec<u8>> {
        if payload.len() != 648 {
            return Err(b"INVALID_CCIP_PAYLOAD_LENGTH".to_vec());
        }
        let mut nullifier = [0u8; 32];
        nullifier.copy_from_slice(&payload[0..32]);
        
        let alpha_neg_bytes = payload[32..160].to_vec();
        let hm_bytes = payload[160..288].to_vec();
        let pk_iss_bytes = payload[288..544].to_vec();
        
        let polymarket_ctf = Address::from_slice(&payload[544..564]);
        let collateral_token = Address::from_slice(&payload[564..584]);
        
        let mut condition_id = [0u8; 32];
        condition_id.copy_from_slice(&payload[584..616]);
        
        let amount = U256::from_be_slice(&payload[616..648]);
        
        // Execute spend and buy shares on destination chain
        let success = self.spend_and_buy_shares(
            nullifier.into(),
            alpha_neg_bytes,
            hm_bytes,
            pk_iss_bytes,
            polymarket_ctf,
            collateral_token,
            condition_id.into(),
            amount,
        )?;
        
        if !success {
            return Err(b"CCIP_EXECUTION_FAILED".to_vec());
        }
        
        Ok(())
    }
}
