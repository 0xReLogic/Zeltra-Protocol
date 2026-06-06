//! Deposit and Reveal Protocol
//!
//! Handles atomic token issuance through deposit, reveal, and refund mechanisms.

use alloc::vec::Vec;
use alloy_primitives::{FixedBytes, U256};
use stylus_sdk::call::RawCall;

use crate::storage::Nimbus;
use crate::interfaces::IErc20;
use crate::constants::BLS12_G2_MSM;

impl Nimbus {
    /// Claim refund for a deposit if the timelock (24 hours) has expired.
    pub fn claim_refund(&mut self, sid: FixedBytes<32>) -> Result<(), Vec<u8>> {
        self.check_not_paused()?;
        if self.session_resolved.get(sid) {
            return Err(b"SESSION_ALREADY_RESOLVED".to_vec());
        }
        let client = self.session_client.get(sid);
        if client != self.msg_sender() {
            return Err(b"NOT_SESSION_CLIENT".to_vec());
        }
        let deposit_time = self.session_timestamp.get(sid);
        if deposit_time == U256::ZERO {
            return Err(b"NO_DEPOSIT_FOUND".to_vec());
        }
        let current_time = U256::from(self.block_timestamp());
        // Enforce 24-hour timelock (86400 seconds)
        if current_time < deposit_time + U256::from(86400) {
            return Err(b"TIMELOCK_NOT_EXPIRED".to_vec());
        }
        self.session_resolved.insert(sid, true);

        let amount = self.session_amount.get(sid);

        let principal = self.total_deposited_principal.get();
        if principal >= amount {
            self.total_deposited_principal.set(principal - amount);
        }

        if amount > U256::ZERO {
            self.ensure_liquidity(amount)?;
            
            #[cfg(not(test))]
            {
                let stablecoin_address = self.stablecoin.get();
                let erc20 = IErc20::new(stablecoin_address);
                let success = erc20.transfer(&mut *self, client, amount)
                    .map_err(|e| e)?;
                if !success {
                    return Err(b"REFUND_TRANSFER_FAILED".to_vec());
                }
            }
        }

        Ok(())
    }

    /// Deposit funds for atomic token issuance.
    pub fn deposit(&mut self, sid: FixedBytes<32>, _com_k_bytes: Vec<u8>, amount: U256) -> Result<(), Vec<u8>> {
        // 1. CHECKS
        self.check_not_paused()?;
        
        let client = self.msg_sender();
        
        // Enforce minimum transaction size of 1 USDC/stablecoin (1,000,000 units)
        let min_amount = U256::from(1_000_000);
        if amount < min_amount {
            return Err(b"AMOUNT_TOO_SMALL".to_vec());
        }
        
        // Calculate deposit/minting fee of 0.1% (amount / 1000, round up)
        let fee = (amount + U256::from(999)) / U256::from(1000);
        let net_amount = amount.checked_sub(fee)
            .ok_or_else(|| b"NET_AMOUNT_UNDERFLOW".to_vec())?;
        
        // 2. EFFECTS
        self.session_client.insert(sid, client);
        self.session_amount.insert(sid, net_amount);
        self.session_resolved.insert(sid, false);
        self.session_timestamp.insert(sid, U256::from(self.block_timestamp()));

        let principal = self.total_deposited_principal.get();
        let new_principal = principal.checked_add(net_amount)
            .ok_or_else(|| b"PRINCIPAL_OVERFLOW".to_vec())?;
        self.total_deposited_principal.set(new_principal);

        // Track epoch volume for dynamic rebalancing (7-epoch moving average)
        self.update_epoch_and_rebalance_ratio(net_amount)?;

        // 3. INTERACTIONS
        #[cfg(not(test))]
        {
            // Transfer full amount from client to this contract
            let stablecoin_address = self.stablecoin.get();
            let erc20 = IErc20::new(stablecoin_address);
            let this_address = stylus_sdk::contract::address();
            
            // Call transferFrom to transfer the collateral from user to this contract
            let success = erc20.transfer_from(&mut *self, client, this_address, amount)
                .map_err(|e| e)?;
            if !success {
                return Err(b"TRANSFER_FROM_FAILED".to_vec());
            }
            
            // Send 0.1% fee to fee_recipient
            if fee > U256::ZERO {
                let recipient = self.fee_recipient.get();
                let fee_success = erc20.transfer(&mut *self, recipient, fee)
                    .map_err(|e| e)?;
                if !fee_success {
                    return Err(b"FEE_TRANSFER_FAILED".to_vec());
                }
            }
        }
        
        self.allocate_reserves(net_amount)?;

        Ok(())
    }

    /// Verifies the masking key k on-chain using G2 MSM precompile (EIP-2537: 0x0e).
    /// Verification check: k * pk_iss == com_k
    pub fn reveal_mask_key(
        &mut self,
        sid: FixedBytes<32>,
        k_bytes: Vec<u8>,
        pk_iss_bytes: Vec<u8>,
        com_k_bytes: Vec<u8>,
    ) -> Result<bool, Vec<u8>> {
        self.check_not_paused()?;
        if self.session_resolved.get(sid) {
            return Ok(false);
        }

        // Construct input payload for bls12_g2_msm (address 0x0e)
        // Format: G2_point (256 bytes) || scalar (32 bytes) = 288 bytes
        let mut input = Vec::with_capacity(288);
        input.extend_from_slice(&pk_iss_bytes);
        input.extend_from_slice(&k_bytes);

        // Call EIP-2537 precompile at 0x0e
        let result = unsafe {
            RawCall::new_static()
                .call(BLS12_G2_MSM, &input)
        }.map_err(|_| b"MSM_PRECOMPILE_CALL_FAILED".to_vec())?;

        // Check if output equals com_k_bytes
        if result == com_k_bytes {
            // 1. EFFECTS
            self.session_resolved.insert(sid, true);
            
            let amount = self.session_amount.get(sid);
            let recipient = self.msg_sender();
            
            let principal = self.total_deposited_principal.get();
            let new_principal = principal.checked_sub(amount).unwrap_or(U256::ZERO);
            self.total_deposited_principal.set(new_principal);
            
            // 2. INTERACTIONS
            if amount > U256::ZERO {
                self.ensure_liquidity(amount)?;
                
                #[cfg(not(test))]
                {
                    let stablecoin_address = self.stablecoin.get();
                    let erc20 = IErc20::new(stablecoin_address);
                    let success = erc20.transfer(&mut *self, recipient, amount)
                        .map_err(|e| e)?;
                    if !success {
                        return Err(b"REVEAL_TRANSFER_FAILED".to_vec());
                    }
                }
            }

            Ok(true)
        } else {
            Ok(false)
        }
    }
}
