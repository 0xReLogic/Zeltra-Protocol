//! Spend and Cross-Chain Intent Execution
//!
//! Handles BLS signature verification, spend operations, Polymarket intent execution,
//! and Chainlink CCIP cross-chain message handling.

use alloc::vec::Vec;
use alloy_primitives::{keccak256, Address, FixedBytes, U256};
use ark_bls12_381::G2Affine;
use ark_ec::AffineRepr;
use stylus_sdk::abi::Bytes;
use stylus_sdk::call::RawCall;
use stylus_sdk::prelude::Call;

use crate::constants::BLS12_PAIRING_CHECK;
use crate::interfaces::{IConditionalTokens, IErc20};
use crate::storage::Nimbus;
use crate::types::to_evm_g2;

impl Nimbus {
    pub(crate) fn ccip_allowlist_key(
        &self,
        source_chain_selector: u64,
        sender: &[u8],
    ) -> FixedBytes<32> {
        let mut bytes = Vec::with_capacity(8 + sender.len());
        bytes.extend_from_slice(&source_chain_selector.to_be_bytes());
        bytes.extend_from_slice(sender);
        keccak256(&bytes)
    }

    /// Verifies the unmasked BLS signature on-chain using pairing precompile (EIP-2537: 0x0f).
    /// Verification check: e(-alpha, G2) * e(H(m), pk_iss) == 1
    fn verify_bls_spend(
        &self,
        alpha_neg_bytes: &Bytes,
        hm_bytes: &Bytes,
        pk_iss_bytes: &Bytes,
    ) -> Result<bool, Vec<u8>> {
        if alpha_neg_bytes.len() != 128 || hm_bytes.len() != 128 {
            return Err(b"INVALID_G1_INPUT_LENGTH".to_vec());
        }
        if pk_iss_bytes.len() != 256 {
            return Err(b"INVALID_PUBLIC_KEY_LENGTH".to_vec());
        }
        if alpha_neg_bytes.iter().all(|byte| *byte == 0)
            || hm_bytes.iter().all(|byte| *byte == 0)
            || pk_iss_bytes.iter().all(|byte| *byte == 0)
        {
            return Err(b"POINT_AT_INFINITY_NOT_ALLOWED".to_vec());
        }
        if !self.trusted_issuer_keys.get(keccak256(pk_iss_bytes)) {
            return Err(b"UNTRUSTED_ISSUER_KEY".to_vec());
        }

        let generator = to_evm_g2(&G2Affine::generator());
        let mut input = Vec::with_capacity(768);
        input.extend_from_slice(alpha_neg_bytes);
        input.extend_from_slice(&generator);
        input.extend_from_slice(hm_bytes);
        input.extend_from_slice(pk_iss_bytes);

        let host = Self::runtime_host();
        let output = unsafe {
            RawCall::new_static(&host)
                .limit_return_data(0, 32)
                .call(BLS12_PAIRING_CHECK, &input)
        }
        .map_err(|_| b"BLS_PAIRING_PRECOMPILE_FAILED".to_vec())?;

        if output.len() != 32 || output[..31].iter().any(|byte| *byte != 0) {
            return Err(b"INVALID_PAIRING_OUTPUT".to_vec());
        }
        Ok(output[31] == 1)
    }

    pub(crate) fn verify_bls_spend_batch(
        &self,
        alpha_neg_list: &[Bytes],
        hm_list: &[Bytes],
        pk_iss_list: &[Bytes],
    ) -> Result<bool, Vec<u8>> {
        let len = alpha_neg_list.len();
        if len == 0 {
            return Ok(true);
        }

        let generator = to_evm_g2(&G2Affine::generator());
        let mut input = Vec::with_capacity(768 * len);

        for i in 0..len {
            let alpha_neg_bytes = &alpha_neg_list[i];
            let hm_bytes = &hm_list[i];
            let pk_iss_bytes = &pk_iss_list[i];

            if alpha_neg_bytes.len() != 128 || hm_bytes.len() != 128 {
                return Err(b"INVALID_G1_INPUT_LENGTH".to_vec());
            }
            if pk_iss_bytes.len() != 256 {
                return Err(b"INVALID_PUBLIC_KEY_LENGTH".to_vec());
            }
            if alpha_neg_bytes.iter().all(|byte| *byte == 0)
                || hm_bytes.iter().all(|byte| *byte == 0)
                || pk_iss_bytes.iter().all(|byte| *byte == 0)
            {
                return Err(b"POINT_AT_INFINITY_NOT_ALLOWED".to_vec());
            }
            if !self.trusted_issuer_keys.get(keccak256(pk_iss_bytes)) {
                return Err(b"UNTRUSTED_ISSUER_KEY".to_vec());
            }

            input.extend_from_slice(alpha_neg_bytes);
            input.extend_from_slice(&generator);
            input.extend_from_slice(hm_bytes);
            input.extend_from_slice(pk_iss_bytes);
        }

        let host = Self::runtime_host();
        let output = unsafe {
            RawCall::new_static(&host)
                .limit_return_data(0, 32)
                .call(BLS12_PAIRING_CHECK, &input)
        }
        .map_err(|_| b"BLS_PAIRING_PRECOMPILE_FAILED".to_vec())?;

        if output.len() != 32 || output[..31].iter().any(|byte| *byte != 0) {
            return Err(b"INVALID_PAIRING_OUTPUT".to_vec());
        }
        Ok(output[31] == 1)
    }

    pub fn _spend(
        &mut self,
        root: FixedBytes<32>,
        nullifier: FixedBytes<32>,
        alpha_neg_bytes: Bytes,
        pk_iss_bytes: Bytes,
        recipient: Address,
        amount: U256,
        recipient_or_intent_hash: FixedBytes<32>,
        expiry: U256,
        nonce: FixedBytes<32>,
    ) -> Result<bool, Vec<u8>> {
        // 1. CHECKS
        self.check_not_paused()?;

        // Expiry check
        let current_time = U256::from(self.block_timestamp());
        if expiry != U256::ZERO && current_time > expiry {
            return Err(b"TRANSACTION_EXPIRED".to_vec());
        }

        // Enforce minimum transaction size of 5 USDC/stablecoin (5,000,000 units)
        let min_amount = U256::from(5_000_000);
        if amount < min_amount {
            return Err(b"AMOUNT_TOO_SMALL".to_vec());
        }

        if recipient != Address::ZERO && recipient_or_intent_hash != recipient_hash(recipient) {
            return Err(b"RECIPIENT_INTENT_MISMATCH".to_vec());
        }

        // Check double spend (Nullifier)
        if self.nullifiers.get(nullifier) {
            return Ok(false);
        }

        // Reconstruct message hash and G1 curve point H(m) on-chain
        let chain_id = U256::from(self.env_chain_id());
        let contract_address = self.env_contract_address();

        let m_hash = crate::helpers::compute_spend_hash(
            chain_id,
            contract_address,
            amount,
            recipient_or_intent_hash,
            expiry,
            nonce,
        );
        let hm_affine = crate::helpers::hash_to_g1(&m_hash);
        let hm_evm_bytes = crate::types::to_evm_g1(&hm_affine);
        let hm_bytes = Bytes::from(hm_evm_bytes.to_vec());

        if nullifier != keccak256(&hm_bytes) {
            return Err(b"NULLIFIER_MESSAGE_MISMATCH".to_vec());
        }
        if !self.verify_bls_spend(&alpha_neg_bytes, &hm_bytes, &pk_iss_bytes)? {
            return Ok(false);
        }

        // `amount` is the exact merchant/recipient payout. Fees are debited on top.
        // Private spend / transaction fee: flat 0.25% (default), or holding-time discounts
        let mut fee_bps = U256::from(25); // 0.25%

        if root != FixedBytes::ZERO {
            let root_timestamp = self.clean_association_roots.get(root);
            if root_timestamp == U256::ZERO {
                return Err(b"INVALID_ASSOCIATION_ROOT".to_vec());
            }
            let delta_t = current_time.checked_sub(root_timestamp).unwrap_or(U256::ZERO);
            let seven_days = U256::from(7 * 24 * 60 * 60);
            let thirty_days = U256::from(30 * 24 * 60 * 60);

            if delta_t >= thirty_days {
                fee_bps = U256::from(10); // 0.10% (1 month hold)
            } else if delta_t >= seven_days {
                fee_bps = U256::from(20); // 0.20% (7 days hold)
            }
        }

        let spend_fee = (amount
            .checked_mul(fee_bps)
            .ok_or_else(|| b"SPEND_FEE_MUL_OVERFLOW".to_vec())?
            + U256::from(9999))
            / U256::from(10000);

        let protocol_share = spend_fee;

        let payout = amount;
        let total_debit = payout
            .checked_add(protocol_share)
            .ok_or_else(|| b"TOTAL_DEBIT_OVERFLOW".to_vec())?;

        let principal = self.total_deposited_principal.get();
        let new_principal = principal
            .checked_sub(total_debit)
            .ok_or_else(|| b"INSUFFICIENT_PRINCIPAL".to_vec())?;

        #[cfg(test)]
        {
            // 2. EFFECTS
            self.total_deposited_principal.set(new_principal);
            self.nullifiers.insert(nullifier, true);

            // Enforce invariant: contract_assets >= outstanding_liabilities
            self.check_liability_invariant()?;

            Ok(true)
        }

        #[cfg(not(test))]
        {
            // 2. EFFECTS
            self.nullifiers.insert(nullifier, true);
            self.total_deposited_principal.set(new_principal);

            // 3. INTERACTIONS
            self.ensure_liquidity(total_debit)?;

            let stablecoin_address = self.stablecoin.get();
            let erc20 = IErc20::new(stablecoin_address);
            let host = Self::runtime_host();

            // Transfer payout to recipient (if not zero address)
            if recipient != Address::ZERO && payout > U256::ZERO {
                let success = erc20
                    .transfer(&host, Call::new_mutating(self), recipient, payout)
                    .map_err(|e| e)?;
                if !success {
                    return Err(b"SPEND_TRANSFER_FAILED".to_vec());
                }
            }

            // Transfer fee to fee_recipient
            if protocol_share > U256::ZERO {
                let recipient_fee = self.fee_recipient.get();
                let fee_success = erc20
                    .transfer(
                        &host,
                        Call::new_mutating(self),
                        recipient_fee,
                        protocol_share,
                    )
                    .map_err(|e| e)?;
                if !fee_success {
                    return Err(b"SPEND_FEE_TRANSFER_FAILED".to_vec());
                }
            }

            // Enforce invariant: contract_assets >= outstanding_liabilities
            self.check_liability_invariant()?;

            Ok(true)
        }
    }

    /// Verifies the signature, redeems stablecoin, and directly calls Polymarket's conditional tokens contract
    /// to buy outcome shares under the recipient's name in a single transaction.
    pub fn _spend_and_buy_shares(
        &mut self,
        root: FixedBytes<32>,
        nullifier: FixedBytes<32>,
        alpha_neg_bytes: Bytes,
        pk_iss_bytes: Bytes,
        polymarket_ctf: Address,
        collateral_token: Address,
        condition_id: FixedBytes<32>,
        amount: U256,
        expiry: U256,
        nonce: FixedBytes<32>,
    ) -> Result<bool, Vec<u8>> {
        self.check_not_paused()?;

        let recipient_or_intent_hash = if condition_id == FixedBytes::ZERO {
            let mut buf = [0u8; 32];
            buf[12..].copy_from_slice(polymarket_ctf.as_slice());
            FixedBytes::from(buf)
        } else {
            let mut buf = Vec::with_capacity(20 + 20 + 32);
            buf.extend_from_slice(polymarket_ctf.as_slice());
            buf.extend_from_slice(collateral_token.as_slice());
            buf.extend_from_slice(condition_id.as_slice());
            keccak256(&buf)
        };

        let transfer_recipient = if condition_id == FixedBytes::ZERO {
            polymarket_ctf
        } else {
            Address::ZERO
        };

        // 1. Verify and invalidate the signature (same as spend)
        let is_valid = self._spend(
            root,
            nullifier,
            alpha_neg_bytes,
            pk_iss_bytes,
            transfer_recipient,
            amount,
            recipient_or_intent_hash,
            expiry,
            nonce,
        )?;
        if !is_valid {
            return Ok(false);
        }

        let payout = amount;

        #[cfg(test)]
        {
            if polymarket_ctf == Address::ZERO {
                self.failed_intent_refunds.insert(nullifier, payout);
            } else if condition_id == FixedBytes::ZERO {
                // Standard cross-chain transfer mock (do nothing in mock)
            }
            Ok(true)
        }

        #[cfg(not(test))]
        {
            let erc20 = IErc20::new(collateral_token);
            let host = Self::runtime_host();

            // If condition_id is zero, this is a standard cross-chain transfer (not Polymarket).
            // In this case, _spend() already transferred the exact payout to polymarket_ctf.
            if condition_id == FixedBytes::ZERO {
                return Ok(true);
            }

            // Approve Polymarket CTF to spend payout amount of collateral token
            let approve_success = erc20
                .approve(&host, Call::new_mutating(self), polymarket_ctf, payout)
                .map_err(|e| e)?;
            if !approve_success {
                return Err(b"POLYMARKET_APPROVE_FAILED".to_vec());
            }

            // Perform external call to Polymarket Conditional Tokens Contract (Gnosis CTF)
            let ctf = IConditionalTokens::new(polymarket_ctf);

            // Partition for YES/NO outcome slots [1, 2]
            let partition = vec![U256::from(1), U256::from(2)];

            match ctf.split_position(
                &host,
                Call::new_mutating(self),
                collateral_token,
                FixedBytes::ZERO,
                condition_id,
                partition,
                payout,
            ) {
                Ok(_) => Ok(true),
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
    pub fn _get_failed_intent_refund(&self, nullifier: FixedBytes<32>) -> Result<U256, Vec<u8>> {
        Ok(self.failed_intent_refunds.get(nullifier))
    }

    /// Claims a failed intent refund on the destination chain.
    pub fn _claim_failed_intent_refund(
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
            let host = Self::runtime_host();
            let success = erc20
                .transfer(&host, Call::new_mutating(self), recipient, amount)
                .map_err(|e| e)?;
            if !success {
                return Err(b"REFUND_TRANSFER_FAILED".to_vec());
            }

            // Enforce invariant: contract_assets >= outstanding_liabilities
            self.check_liability_invariant()?;

            Ok(true)
        }
    }

    /// Receives a cross-chain payload via Chainlink CCIP and executes the transaction (Fase B).
    pub fn _ccip_receive(
        &mut self,
        message_id: FixedBytes<32>,
        source_chain_selector: u64,
        sender: Bytes,
        payload: Bytes,
    ) -> Result<(), Vec<u8>> {
        // Enforce caller verification if CCIP Router address is configured (Production Best Practice)
        let caller = self.msg_sender();
        let configured_router = self.ccip_router.get();
        if configured_router == Address::ZERO {
            return Err(b"CCIP_ROUTER_NOT_CONFIGURED".to_vec());
        }
        if caller != configured_router {
            return Err(b"ONLY_CCIP_ROUTER_ALLOWED".to_vec());
        }

        // Replay protection: Check if message has already been processed
        if self.ccip_processed_messages.get(message_id) {
            return Err(b"CCIP_MESSAGE_ALREADY_PROCESSED".to_vec());
        }

        // Verify sender and source chain selector are on the allowlist
        let key = self.ccip_allowlist_key(source_chain_selector, &sender);
        if !self.ccip_allowed_senders.get(key) {
            return Err(b"CCIP_SENDER_NOT_ALLOWED".to_vec());
        }

        if payload.len() != 584 {
            return Err(b"INVALID_CCIP_PAYLOAD_LENGTH".to_vec());
        }
        let mut nullifier = [0u8; 32];
        nullifier.copy_from_slice(&payload[0..32]);

        let alpha_neg_bytes = payload[32..160].to_vec();
        let pk_iss_bytes = payload[160..416].to_vec();

        let polymarket_ctf = Address::from_slice(&payload[416..436]);
        let collateral_token = Address::from_slice(&payload[436..456]);

        let mut condition_id = [0u8; 32];
        condition_id.copy_from_slice(&payload[456..488]);

        let amount = U256::from_be_slice(&payload[488..520]);
        let expiry = U256::from_be_slice(&payload[520..552]);

        let mut nonce = [0u8; 32];
        nonce.copy_from_slice(&payload[552..584]);

        // Execute spend and buy shares on destination chain
        let success = self._spend_and_buy_shares(
            FixedBytes::ZERO,
            nullifier.into(),
            Bytes::from(alpha_neg_bytes),
            Bytes::from(pk_iss_bytes),
            polymarket_ctf,
            collateral_token,
            condition_id.into(),
            amount,
            expiry,
            nonce.into(),
        )?;

        if !success {
            return Err(b"CCIP_EXECUTION_FAILED".to_vec());
        }

        // Mark message as processed successfully
        self.ccip_processed_messages.insert(message_id, true);

        Ok(())
    }
}

pub(crate) fn recipient_hash(recipient: Address) -> FixedBytes<32> {
    let mut out = [0u8; 32];
    out[12..].copy_from_slice(recipient.as_slice());
    FixedBytes::from(out)
}
