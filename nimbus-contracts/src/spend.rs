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

use crate::constants::{BLS12_G2_GENERATOR_EVM, BLS12_PAIRING_CHECK};
use crate::interfaces::{IConditionalTokens, IErc20};
use crate::storage::Nimbus;

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

        let mut input = Vec::with_capacity(768);
        input.extend_from_slice(alpha_neg_bytes);
        input.extend_from_slice(&BLS12_G2_GENERATOR_EVM);
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
            input.extend_from_slice(&BLS12_G2_GENERATOR_EVM);
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
        max_execution_fee: U256,
        execution_fee: U256,
    ) -> Result<bool, Vec<u8>> {
        // 1. CHECKS
        self.check_not_paused()?;

        // Execution fee validation: actual fee must not exceed user-signed maximum
        if execution_fee > max_execution_fee {
            return Err(b"EXECUTION_FEE_EXCEEDED".to_vec());
        }

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
        // Private spend / transaction fee: unified flat 0.45% (DEC-028)
        let fee_bps = U256::from(45); // 0.45% Flat

        if root != FixedBytes::ZERO {
            let root_timestamp = self.clean_association_roots.get(root);
            if root_timestamp == U256::ZERO {
                return Err(b"INVALID_ASSOCIATION_ROOT".to_vec());
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

        // Total debit from user funds: payout + protocol fee + execution fee (DEC-016 Gate B)
        let total_user_debit = total_debit
            .checked_add(execution_fee)
            .ok_or_else(|| b"TOTAL_USER_DEBIT_OVERFLOW".to_vec())?;

        let principal = self.total_deposited_principal.get();
        let new_principal = principal
            .checked_sub(total_user_debit)
            .or_else(|| principal.checked_sub(total_debit))
            .ok_or_else(|| b"INSUFFICIENT_PRINCIPAL".to_vec())?;

        // Multi-liability tracking (DEC-016 Gate B)
        let user_liab = self.user_note_liability.get();
        if user_liab >= total_user_debit {
            self.user_note_liability.set(user_liab - total_user_debit);
        } else if user_liab >= total_debit {
            self.user_note_liability.set(user_liab - total_debit);
        }

        // Accumulate execution fee for later claiming by relayer
        let new_accumulated_fees = self
            .accumulated_execution_fees
            .get()
            .checked_add(execution_fee)
            .ok_or_else(|| b"EXECUTION_FEE_OVERFLOW".to_vec())?;

        let new_accrued_fees = self
            .accrued_execution_fee_liability
            .get()
            .checked_add(execution_fee)
            .ok_or_else(|| b"ACCRUED_FEE_OVERFLOW".to_vec())?;

        let new_realized_fees = self
            .realized_protocol_fees
            .get()
            .checked_add(protocol_share)
            .unwrap_or_else(|| self.realized_protocol_fees.get());

        #[cfg(test)]
        {
            // 2. EFFECTS
            self.total_deposited_principal.set(new_principal);
            self.nullifiers.insert(nullifier, true);
            self.accumulated_execution_fees.set(new_accumulated_fees);
            self.accrued_execution_fee_liability.set(new_accrued_fees);
            self.realized_protocol_fees.set(new_realized_fees);

            // Emit ProtocolFee and ExecutionFee events
            crate::events::emit_event(crate::events::ProtocolFee {
                nullifier,
                recipient,
                amount: payout,
                fee_bps,
                protocol_fee: protocol_share,
            });
            crate::events::emit_event(crate::events::ExecutionFee {
                nullifier,
                execution_fee,
                max_execution_fee,
            });

            // Enforce invariant: contract_assets >= outstanding_liabilities
            self.check_liability_invariant()?;

            Ok(true)
        }

        #[cfg(not(test))]
        {
            // 2. EFFECTS
            self.nullifiers.insert(nullifier, true);
            self.total_deposited_principal.set(new_principal);
            self.accumulated_execution_fees.set(new_accumulated_fees);
            self.accrued_execution_fee_liability.set(new_accrued_fees);
            self.realized_protocol_fees.set(new_realized_fees);

            // 3. INTERACTIONS
            self.ensure_liquidity(total_debit)?;

            let stablecoin_address = self.stablecoin.get();
            let erc20 = IErc20::new(stablecoin_address);
            let host = Self::runtime_host();

            // Transfer payout to recipient (if not zero address)
            if recipient != Address::ZERO && payout > U256::ZERO {
                let success = erc20.transfer(&host, Call::new_mutating(self), recipient, payout)?;
                if !success {
                    return Err(b"SPEND_TRANSFER_FAILED".to_vec());
                }
            }

            // Transfer fee to fee_recipient
            if protocol_share > U256::ZERO {
                let recipient_fee = self.fee_recipient.get();
                let fee_success = erc20.transfer(
                    &host,
                    Call::new_mutating(self),
                    recipient_fee,
                    protocol_share,
                )?;
                if !fee_success {
                    return Err(b"SPEND_FEE_TRANSFER_FAILED".to_vec());
                }
            }

            // Emit ProtocolFee and ExecutionFee events
            crate::events::emit_event(crate::events::ProtocolFee {
                nullifier,
                recipient,
                amount: payout,
                fee_bps,
                protocol_fee: protocol_share,
            });
            crate::events::emit_event(crate::events::ExecutionFee {
                nullifier,
                execution_fee,
                max_execution_fee,
            });

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
        max_execution_fee: U256,
        execution_fee: U256,
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
            max_execution_fee,
            execution_fee,
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
            let approve_success =
                erc20.approve(&host, Call::new_mutating(self), polymarket_ctf, payout)?;
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
            let success = erc20.transfer(&host, Call::new_mutating(self), recipient, amount)?;
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
        // Enforce caller verification: CCIP Router MUST be configured and non-zero
        let caller = self.msg_sender();
        let configured_router = self.ccip_router.get();
        if configured_router == Address::ZERO {
            return Err(b"CCIP_ROUTER_NOT_CONFIGURED".to_vec());
        }
        if caller != configured_router {
            return Err(b"ONLY_CCIP_ROUTER_ALLOWED".to_vec());
        }

        // Validate CCIP message_id is non-zero
        if message_id == FixedBytes::ZERO {
            return Err(b"INVALID_CCIP_MESSAGE_ID".to_vec());
        }

        // Replay protection: Check if message has already been processed
        if self.ccip_processed_messages.get(message_id) {
            return Err(b"CCIP_MESSAGE_ALREADY_PROCESSED".to_vec());
        }

        // Validate source chain selector and sender
        if source_chain_selector == 0 {
            return Err(b"INVALID_SOURCE_CHAIN_SELECTOR".to_vec());
        }
        if sender.is_empty() {
            return Err(b"EMPTY_CCIP_SENDER".to_vec());
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
        // Note: CCIP has its own fee mechanism, so we pass zero execution fees here
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
            U256::ZERO, // max_execution_fee (CCIP has its own fee structure)
            U256::ZERO, // execution_fee (CCIP has its own fee structure)
        )?;

        if !success {
            return Err(b"CCIP_EXECUTION_FAILED".to_vec());
        }

        // Mark message as processed successfully
        self.ccip_processed_messages.insert(message_id, true);

        Ok(())
    }

    /// Executes a private note spend using Groth16 proof verification for PrivateNoteCircuit (Gate D).
    ///
    /// Verifies:
    /// 1. Contract not paused & transaction not expired.
    /// 2. execution_fee <= max_execution_fee.
    /// 3. merchant_amount > 0.
    /// 4. has_change in {0, 1} and output_commitment consistency.
    /// 5. Note root is valid and recorded in accepted_note_roots.
    /// 6. Nullifier is unspent (note_nullifiers).
    /// 7. 12 public input scalars are canonical (< Fr modulus).
    /// 8. Groth16 proof verifies against NOTE_VK via EIP-2537 precompiles.
    ///
    /// Effects:
    /// - Marks nullifier as spent in note_nullifiers.
    /// - If has_change == 1, inserts output_commitment into LeanIMT depth 20 tree via _merkle_insert.
    /// - Transfers merchant_amount to recipient (ERC-20).
    /// - Decreases user_note_liability by (merchant_amount + protocol_fee + execution_fee).
    /// - Accrues execution_fee to accrued_execution_fee_liability and accumulated_execution_fees.
    /// - Realizes protocol_fee to realized_protocol_fees.
    /// - Enforces solvency invariant (assets >= liabilities).
    /// - Emits PrivateNoteSpend, ProtocolFee, and ExecutionFee events.
    pub(crate) fn _spend_private_note(
        &mut self,
        note_root: FixedBytes<32>,
        input_nullifier: FixedBytes<32>,
        output_commitment: FixedBytes<32>,
        recipient: Address,
        merchant_amount: U256,
        protocol_fee: U256,
        execution_fee: U256,
        max_execution_fee: U256,
        quote_hash: FixedBytes<32>,
        expiry: U256,
        has_change: U256,
        proof_a_neg: Bytes,
        proof_b: Bytes,
        proof_c: Bytes,
    ) -> Result<bool, Vec<u8>> {
        // 1. CHECKS
        self.check_not_paused()?;

        // Execution fee validation
        if execution_fee > max_execution_fee {
            return Err(b"EXECUTION_FEE_EXCEEDS_MAX".to_vec());
        }

        // Expiry check
        let current_time = U256::from(self.block_timestamp());
        if expiry != U256::ZERO && current_time > expiry {
            return Err(b"TRANSACTION_EXPIRED".to_vec());
        }

        // Merchant amount must be positive
        if merchant_amount == U256::ZERO {
            return Err(b"MERCHANT_AMOUNT_ZERO".to_vec());
        }

        // has_change boolean validation (0 or 1)
        if has_change != U256::ZERO && has_change != U256::from(1) {
            return Err(b"INVALID_HAS_CHANGE_FLAG".to_vec());
        }

        // If has_change == 0, output_commitment must be zero
        if has_change == U256::ZERO && output_commitment != FixedBytes::ZERO {
            return Err(b"NONZERO_COMMITMENT_WITH_ZERO_CHANGE".to_vec());
        }

        // Note root must be recorded in accepted_note_roots
        let root_timestamp = self.accepted_note_roots.get(note_root);
        if root_timestamp == U256::ZERO {
            return Err(b"UNACCEPTED_NOTE_ROOT".to_vec());
        }

        // Nullifier must not have been spent
        if self.note_nullifiers.get(input_nullifier) {
            return Err(b"NOTE_ALREADY_SPENT".to_vec());
        }

        // Fail-Fast Check: Validate total debit and note liability BEFORE expensive ZK precompile calls
        let total_debit = merchant_amount
            .checked_add(protocol_fee)
            .ok_or_else(|| b"TOTAL_DEBIT_OVERFLOW".to_vec())?
            .checked_add(execution_fee)
            .ok_or_else(|| b"TOTAL_DEBIT_OVERFLOW".to_vec())?;

        let user_liab = self.user_note_liability.get();
        if user_liab < total_debit {
            return Err(b"INSUFFICIENT_NOTE_LIABILITY".to_vec());
        }

        // Construct 12 public input scalars in big-endian EVM format
        let mut recipient_bytes = [0u8; 32];
        recipient_bytes[12..].copy_from_slice(recipient.as_slice());

        let mut contract_bytes = [0u8; 32];
        contract_bytes[12..].copy_from_slice(self.env_contract_address().as_slice());

        let chain_id_bytes = U256::from(self.env_chain_id()).to_be_bytes::<32>();
        let merchant_amount_bytes = merchant_amount.to_be_bytes::<32>();
        let protocol_fee_bytes = protocol_fee.to_be_bytes::<32>();
        let execution_fee_bytes = execution_fee.to_be_bytes::<32>();
        let expiry_bytes = expiry.to_be_bytes::<32>();
        let has_change_bytes = has_change.to_be_bytes::<32>();

        let public_inputs: [[u8; 32]; 12] = [
            note_root.0,
            input_nullifier.0,
            output_commitment.0,
            recipient_bytes,
            merchant_amount_bytes,
            protocol_fee_bytes,
            execution_fee_bytes,
            quote_hash.0,
            chain_id_bytes,
            contract_bytes,
            expiry_bytes,
            has_change_bytes,
        ];

        // Validate canonicality for each public input scalar
        for input_scalar in &public_inputs {
            if crate::types::from_evm_scalar(input_scalar).is_none() {
                return Err(b"NON_CANONICAL_PUBLIC_INPUT_SCALAR".to_vec());
            }
        }

        // Compute linear combination L in G1 via MSM & ADD precompiles
        let public_inputs_g1 =
            crate::groth16_note_verifier::compute_note_public_inputs_g1(&public_inputs)?;

        // Verify Groth16 proof via 4-pairing precompile check
        let is_valid = crate::groth16_note_verifier::verify_private_note_groth16(
            &proof_a_neg,
            &proof_b,
            &proof_c,
            &public_inputs_g1,
        )?;
        if !is_valid {
            return Err(b"INVALID_GROTH16_NOTE_PROOF".to_vec());
        }

        // 2. EFFECTS
        // Mark nullifier spent
        self.note_nullifiers.insert(input_nullifier, true);

        // If has_change == 1, insert output_commitment into LeanIMT Merkle tree (which emits ChangeCommitment)
        if has_change == U256::from(1) {
            self._merkle_insert(output_commitment)?;
        }

        // Multi-liability accounting (DEC-016 Gate B)
        self.user_note_liability.set(user_liab - total_debit);

        let new_accumulated_fees = self
            .accumulated_execution_fees
            .get()
            .checked_add(execution_fee)
            .ok_or_else(|| b"EXECUTION_FEE_OVERFLOW".to_vec())?;
        self.accumulated_execution_fees.set(new_accumulated_fees);

        let new_accrued_fees = self
            .accrued_execution_fee_liability
            .get()
            .checked_add(execution_fee)
            .ok_or_else(|| b"ACCRUED_FEE_OVERFLOW".to_vec())?;
        self.accrued_execution_fee_liability.set(new_accrued_fees);

        let new_realized_fees = self
            .realized_protocol_fees
            .get()
            .checked_add(protocol_fee)
            .unwrap_or_else(|| self.realized_protocol_fees.get());
        self.realized_protocol_fees.set(new_realized_fees);

        // 3. INTERACTIONS (Token transfer to recipient)
        #[cfg(not(test))]
        {
            if recipient != Address::ZERO && merchant_amount > U256::ZERO {
                self.ensure_liquidity(merchant_amount)?;
                let stablecoin_address = self.stablecoin.get();
                let erc20 = IErc20::new(stablecoin_address);
                let host = Self::runtime_host();
                let call_res =
                    erc20.transfer(&host, Call::new_mutating(self), recipient, merchant_amount);
                match call_res {
                    Ok(true) => {}
                    _ => return Err(b"ERC20_TRANSFER_FAILED".to_vec()),
                }
            }
        }

        // 4. EVENTS
        crate::events::emit_event(crate::events::PrivateNoteSpend {
            nullifier: input_nullifier,
            note_root,
            recipient,
            merchant_amount,
            output_commitment,
            has_change: has_change == U256::from(1),
        });
        crate::events::emit_event(crate::events::ProtocolFee {
            nullifier: input_nullifier,
            recipient,
            amount: merchant_amount,
            fee_bps: U256::ZERO,
            protocol_fee,
        });
        crate::events::emit_event(crate::events::ExecutionFee {
            nullifier: input_nullifier,
            execution_fee,
            max_execution_fee,
        });

        // 5. SOLVENCY INVARIANT
        self.check_liability_invariant()?;

        Ok(true)
    }
}

pub(crate) fn recipient_hash(recipient: Address) -> FixedBytes<32> {
    let mut out = [0u8; 32];
    out[12..].copy_from_slice(recipient.as_slice());
    FixedBytes::from(out)
}
