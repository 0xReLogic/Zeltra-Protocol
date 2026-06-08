//! Deposit and Reveal Protocol
//!
//! Handles atomic token issuance through deposit, reveal, and refund mechanisms.

use alloc::vec::Vec;
use alloy_primitives::{keccak256, FixedBytes, U256};
use stylus_sdk::abi::Bytes;
use stylus_sdk::call::RawCall;
use stylus_sdk::prelude::Call;

use crate::constants::BLS12_G2_MSM;
use crate::interfaces::IErc20;
use crate::storage::Nimbus;

impl Nimbus {
    /// Claim refund for a deposit if the timelock (24 hours) has expired.
    pub fn _claim_refund(&mut self, sid: FixedBytes<32>) -> Result<(), Vec<u8>> {
        self.check_not_paused()?;
        if !self.session_exists.get(sid) {
            return Err(b"NO_DEPOSIT_FOUND".to_vec());
        }
        if self.session_resolved.get(sid) {
            return Err(b"SESSION_ALREADY_RESOLVED".to_vec());
        }
        let client = self.session_client.get(sid);
        if client != self.msg_sender() {
            return Err(b"NOT_SESSION_CLIENT".to_vec());
        }
        let deposit_time = self.session_timestamp.get(sid);
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
                let host = Self::runtime_host();
                let success = erc20
                    .transfer(&host, Call::new_mutating(self), client, amount)
                    .map_err(|e| e)?;
                if !success {
                    return Err(b"REFUND_TRANSFER_FAILED".to_vec());
                }
            }
        }

        // Enforce invariant: contract_assets >= outstanding_liabilities
        self.check_liability_invariant()?;

        Ok(())
    }

    /// Deposit funds for atomic token issuance.
    pub fn _deposit(
        &mut self,
        sid: FixedBytes<32>,
        com_k_bytes: Bytes,
        amount: U256,
    ) -> Result<(), Vec<u8>> {
        // 1. CHECKS
        self.check_not_paused()?;

        if self.session_exists.get(sid) {
            return Err(b"SESSION_ALREADY_EXISTS".to_vec());
        }
        if com_k_bytes.len() != 256 {
            return Err(b"INVALID_COMMITMENT_LENGTH".to_vec());
        }

        let client = self.msg_sender();

        // Enforce minimum transaction size of 10 USDC/stablecoin (10,000,000 units)
        let min_amount = U256::from(10_000_000);
        if amount < min_amount {
            return Err(b"AMOUNT_TOO_SMALL".to_vec());
        }

        // Calculate deposit/minting fee of 0.1% (amount / 1000, round up)
        let fee = (amount + U256::from(999)) / U256::from(1000);
        let net_amount = amount
            .checked_sub(fee)
            .ok_or_else(|| b"NET_AMOUNT_UNDERFLOW".to_vec())?;

        // 2. EFFECTS
        self.session_client.insert(sid, client);
        self.session_amount.insert(sid, net_amount);
        self.session_resolved.insert(sid, false);
        self.session_timestamp
            .insert(sid, U256::from(self.block_timestamp()));
        self.session_commitment_hash
            .insert(sid, keccak256(&com_k_bytes));
        self.session_exists.insert(sid, true);

        let principal = self.total_deposited_principal.get();
        let new_principal = principal
            .checked_add(net_amount)
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
            let this_address = self.env_contract_address();
            let host = Self::runtime_host();

            // Call transferFrom to transfer the collateral from user to this contract
            let success = erc20
                .transfer_from(
                    &host,
                    Call::new_mutating(self),
                    client,
                    this_address,
                    amount,
                )
                .map_err(|e| e)?;
            if !success {
                return Err(b"TRANSFER_FROM_FAILED".to_vec());
            }

            // Send 0.1% fee to fee_recipient
            if fee > U256::ZERO {
                let recipient = self.fee_recipient.get();
                let fee_success = erc20
                    .transfer(&host, Call::new_mutating(self), recipient, fee)
                    .map_err(|e| e)?;
                if !fee_success {
                    return Err(b"FEE_TRANSFER_FAILED".to_vec());
                }
            }
        }

        self.allocate_reserves(net_amount)?;

        // Enforce invariant: contract_assets >= outstanding_liabilities
        self.check_liability_invariant()?;

        Ok(())
    }

    /// Verifies the masking key k on-chain using G2 MSM precompile (EIP-2537: 0x0e).
    /// Verification check: k * pk_iss == com_k
    pub fn _reveal_mask_key(
        &mut self,
        sid: FixedBytes<32>,
        k_bytes: Bytes,
        pk_iss_bytes: Bytes,
        com_k_bytes: Bytes,
    ) -> Result<bool, Vec<u8>> {
        self.check_not_paused()?;
        if !self.session_exists.get(sid) {
            return Err(b"NO_DEPOSIT_FOUND".to_vec());
        }
        if self.session_resolved.get(sid) {
            return Ok(false);
        }
        if k_bytes.len() != 32 {
            return Err(b"INVALID_SCALAR_LENGTH".to_vec());
        }
        if pk_iss_bytes.len() != 256 {
            return Err(b"INVALID_PUBLIC_KEY_LENGTH".to_vec());
        }
        if com_k_bytes.len() != 256 {
            return Err(b"INVALID_COMMITMENT_LENGTH".to_vec());
        }
        if keccak256(&com_k_bytes) != self.session_commitment_hash.get(sid) {
            return Err(b"COMMITMENT_MISMATCH".to_vec());
        }

        // Construct input payload for bls12_g2_msm (address 0x0e)
        // Format: G2_point (256 bytes) || scalar (32 bytes) = 288 bytes
        let mut input = Vec::with_capacity(288);
        input.extend_from_slice(&pk_iss_bytes);
        input.extend_from_slice(&k_bytes);

        // Call EIP-2537 precompile at 0x0e. Unit tests isolate the state
        // transition; the real precompile is covered by the testnet hard test.
        #[cfg(not(test))]
        let result = {
            let host = Self::runtime_host();
            unsafe { RawCall::new_static(&host).call(BLS12_G2_MSM, &input) }
                .map_err(|_| b"MSM_PRECOMPILE_CALL_FAILED".to_vec())?
        };

        #[cfg(test)]
        let result = com_k_bytes.to_vec();

        // Check if output equals com_k_bytes
        if result == *com_k_bytes {
            // Reveal makes the credential spendable and permanently disables
            // refund. Collateral remains backing the credential until spend.
            self.session_resolved.insert(sid, true);
            Ok(true)
        } else {
            Ok(false)
        }
    }
}
