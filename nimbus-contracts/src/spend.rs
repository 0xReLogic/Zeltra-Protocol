//! Spend and Cross-Chain Intent Execution
//!
//! Handles Groth16 zero-knowledge private note spend operations (PrivateNoteCircuit & JoinSplitCircuit)
//! and Chainlink CCIP cross-chain message handling.

use alloc::vec::Vec;
use alloy_primitives::{keccak256, Address, FixedBytes, U256};
use ark_bls12_381::G2Affine;
use ark_ec::AffineRepr;
use stylus_sdk::abi::Bytes;
use stylus_sdk::call::RawCall;
use stylus_sdk::prelude::Call;

use crate::constants::BLS12_PAIRING_CHECK;
use crate::interfaces::IErc20;
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

    /// Receives a cross-chain payload via Chainlink CCIP (Fase B).
    pub fn _ccip_receive(
        &mut self,
        message_id: FixedBytes<32>,
        source_chain_selector: u64,
        sender: Bytes,
        _payload: Bytes,
    ) -> Result<(), Vec<u8>> {
        let caller = self.msg_sender();
        let configured_router = self.ccip_router.get();
        if configured_router == Address::ZERO {
            return Err(b"CCIP_ROUTER_NOT_CONFIGURED".to_vec());
        }
        if caller != configured_router {
            return Err(b"ONLY_CCIP_ROUTER_ALLOWED".to_vec());
        }
        if message_id == FixedBytes::ZERO {
            return Err(b"INVALID_CCIP_MESSAGE_ID".to_vec());
        }
        if self.ccip_processed_messages.get(message_id) {
            return Err(b"CCIP_MESSAGE_ALREADY_PROCESSED".to_vec());
        }
        if source_chain_selector == 0 {
            return Err(b"INVALID_SOURCE_CHAIN_SELECTOR".to_vec());
        }
        if sender.is_empty() {
            return Err(b"EMPTY_CCIP_SENDER".to_vec());
        }
        let key = self.ccip_allowlist_key(source_chain_selector, &sender);
        if !self.ccip_allowed_senders.get(key) {
            return Err(b"CCIP_SENDER_NOT_ALLOWED".to_vec());
        }

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
    /// - If has_change == 1, inserts output_commitment into MMR via _mmr_insert.
    /// - Transfers merchant_amount to recipient (ERC-20).
    /// - Decreases user_note_liability by (merchant_amount + protocol_fee + execution_fee).
    /// - Accrues execution_fee to accrued_execution_fee_liability and accumulated_execution_fees.
    /// - Realizes protocol_fee to realized_protocol_fees.
    /// - Enforces solvency invariant (assets >= liabilities).
    /// - Emits PrivateNoteSpend, ProtocolFee, and ExecutionFee events.
    pub(crate) fn _spend_private_note(
        &mut self,
        note_root: FixedBytes<32>,
        leaf_count: U256,
        input_nullifier: FixedBytes<32>,
        note_epoch_id: U256,
        output_commitment: FixedBytes<32>,
        recipient: Address,
        merchant_amount: U256,
        protocol_fee: U256,
        execution_fee: U256,
        max_execution_fee: U256,
        quote_hash: FixedBytes<32>,
        expiry: U256,
        has_change: U256,
        is_rollover: U256,
        proof_a_neg: Bytes,
        proof_b: Bytes,
        proof_c: Bytes,
        encrypted_memo: Bytes,
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

        // Rollover mode validation (DEC-033)
        if is_rollover != U256::ZERO && is_rollover != U256::from(1) {
            return Err(b"INVALID_IS_ROLLOVER_FLAG".to_vec());
        }

        if is_rollover == U256::ZERO {
            // Normal spend: merchant amount must be positive
            if merchant_amount == U256::ZERO {
                return Err(b"MERCHANT_AMOUNT_ZERO".to_vec());
            }
        } else {
            // Rollover mode: merchant_amount must be 0, protocol_fee must be 0
            if merchant_amount != U256::ZERO {
                return Err(b"MERCHANT_AMOUNT_MUST_BE_ZERO_IN_ROLLOVER".to_vec());
            }
            if protocol_fee != U256::ZERO {
                return Err(b"PROTOCOL_FEE_MUST_BE_ZERO_IN_ROLLOVER".to_vec());
            }
            // Rollover MUST produce a change note (the renewed note)
            if has_change != U256::from(1) {
                return Err(b"ROLLOVER_MUST_HAVE_CHANGE".to_vec());
            }
        }

        // leaf_count validation (DEC-032 anti-hyperbridge check)
        if leaf_count == U256::ZERO {
            return Err(b"INVALID_LEAF_COUNT".to_vec());
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

        // Epoch window validation (DEC-033)
        let current_epoch_u256 = self.current_epoch.get();
        if note_epoch_id > current_epoch_u256 {
            return Err(b"EPOCH_IN_FUTURE".to_vec());
        }
        let current_epoch = current_epoch_u256.to::<u32>();
        let min_active_epoch = current_epoch.saturating_sub(1);
        if note_epoch_id < U256::from(min_active_epoch) {
            return Err(b"EPOCH_EXPIRED_REQUIRE_ROLLOVER".to_vec());
        }
        let note_epoch = note_epoch_id.to::<u32>();
        let slot = (note_epoch % 2) as u8;

        // Generational nullifier check (DEC-033)
        // Verify slot is actively mapped to note_epoch
        if self.generation_epoch_id.get(U256::from(slot)) != note_epoch_id {
            return Err(b"EPOCH_SLOT_DESYNCHRONIZED".to_vec());
        }

        let gen_key = crate::storage::generational_nullifier_key(slot, input_nullifier);
        if self.generation_nullifiers.get(gen_key) || self.note_nullifiers.get(input_nullifier) {
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

        // Construct 15 public input scalars in big-endian EVM format
        let mut recipient_bytes = [0u8; 32];
        recipient_bytes[12..].copy_from_slice(recipient.as_slice());

        let mut contract_bytes = [0u8; 32];
        contract_bytes[12..].copy_from_slice(self.env_contract_address().as_slice());

        let chain_id_bytes = U256::from(self.env_chain_id()).to_be_bytes::<32>();
        let leaf_count_bytes = leaf_count.to_be_bytes::<32>();
        let note_epoch_id_bytes = note_epoch_id.to_be_bytes::<32>();
        let merchant_amount_bytes = merchant_amount.to_be_bytes::<32>();
        let protocol_fee_bytes = protocol_fee.to_be_bytes::<32>();
        let execution_fee_bytes = execution_fee.to_be_bytes::<32>();
        let expiry_bytes = expiry.to_be_bytes::<32>();
        let has_change_bytes = has_change.to_be_bytes::<32>();
        let is_rollover_bytes = is_rollover.to_be_bytes::<32>();

        // Unpack 32-byte quote_hash into two 128-bit EVM words in memory (DEC-035A)
        let mut quote_hi = [0u8; 32];
        quote_hi[16..32].copy_from_slice(&quote_hash.as_slice()[0..16]);

        let mut quote_lo = [0u8; 32];
        quote_lo[16..32].copy_from_slice(&quote_hash.as_slice()[16..32]);

        let public_inputs: [[u8; 32]; 16] = [
            note_root.0,
            leaf_count_bytes,
            input_nullifier.0,
            note_epoch_id_bytes,
            output_commitment.0,
            recipient_bytes,
            merchant_amount_bytes,
            protocol_fee_bytes,
            execution_fee_bytes,
            quote_hi,
            quote_lo,
            chain_id_bytes,
            contract_bytes,
            expiry_bytes,
            has_change_bytes,
            is_rollover_bytes,
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
        // Mark nullifier spent in both generational map and persistent fallback
        self.generation_nullifiers.insert(gen_key, true);
        self.note_nullifiers.insert(input_nullifier, true);

        // If has_change == 1, insert output_commitment into MMR (which emits NoteCommitmentAppended & ChangeCommitment)
        if has_change == U256::from(1) {
            self._mmr_insert(output_commitment, encrypted_memo.to_vec())?;
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
        if is_rollover == U256::from(1) {
            crate::events::emit_event(crate::events::NoteRolloverExecuted {
                old_nullifier: input_nullifier,
                new_commitment: output_commitment,
                fee: execution_fee.to::<u64>(),
            });
        } else {
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
        }
        crate::events::emit_event(crate::events::ExecutionFee {
            nullifier: input_nullifier,
            execution_fee,
            max_execution_fee,
        });

        // 5. SOLVENCY INVARIANT
        self.check_liability_invariant()?;

        Ok(true)
    }

    /// Executes a Universal 2-in-2-out JoinSplit private spend or consolidation (DEC-036 / DEC-036C).
    pub(crate) fn _spend_joinsplit(
        &mut self,
        proof_a_neg: Bytes,
        proof_b: Bytes,
        proof_c: Bytes,
        note_root: FixedBytes<32>,
        leaf_count: U256,
        nullifier_1: FixedBytes<32>,
        nullifier_2: FixedBytes<32>,
        epoch_id_1: U256,
        epoch_id_2: U256,
        output_cm_1: FixedBytes<32>,
        output_cm_2: FixedBytes<32>,
        recipient: Address,
        merchant_amount: U256,
        protocol_fee: U256,
        execution_fee: U256,
        max_execution_fee: U256,
        quote_hash: FixedBytes<32>,
        expiry: U256,
        flags_packed: U256,
        is_rollover: U256,
    ) -> Result<bool, Vec<u8>> {
        // 1. CHECKS
        self.check_not_paused()?;

        // Execution fee validation
        if execution_fee > max_execution_fee {
            return Err(b"EXECUTION_FEE_EXCEEDS_MAX".to_vec());
        }

        // Expiry check (C4.2)
        let current_time = U256::from(self.block_timestamp());
        if expiry != U256::ZERO && current_time > expiry {
            return Err(b"QuoteExpired".to_vec());
        }

        // Non-aliasing guard: nf1 != nf2 (Singularity Defense, Zellic 2024 / C4.4)
        if nullifier_1 == nullifier_2 {
            return Err(b"DuplicateNullifierInSameTx".to_vec());
        }

        // On-chain nullifier spent guard (C4.4)
        if self.note_nullifiers.get(nullifier_1) {
            return Err(b"Nullifier1AlreadySpent".to_vec());
        }
        if self.note_nullifiers.get(nullifier_2) {
            return Err(b"Nullifier2AlreadySpent".to_vec());
        }

        // Generational nullifiers guard (DEC-033)
        let current_epoch_u256 = self.current_epoch.get();
        if epoch_id_1 > current_epoch_u256 || epoch_id_2 > current_epoch_u256 {
            return Err(b"EPOCH_IN_FUTURE".to_vec());
        }
        let current_epoch = current_epoch_u256.to::<u32>();
        let min_active_epoch = current_epoch.saturating_sub(1);
        if epoch_id_1 < U256::from(min_active_epoch) || epoch_id_2 < U256::from(min_active_epoch) {
            return Err(b"EPOCH_EXPIRED_REQUIRE_ROLLOVER".to_vec());
        }

        let slot1 = (epoch_id_1.to::<u32>() % 2) as u8;
        let slot2 = (epoch_id_2.to::<u32>() % 2) as u8;
        let gen_key1 = crate::storage::generational_nullifier_key(slot1, nullifier_1);
        let gen_key2 = crate::storage::generational_nullifier_key(slot2, nullifier_2);
        if self.generation_nullifiers.get(gen_key1) {
            return Err(b"Nullifier1AlreadySpent".to_vec());
        }
        if self.generation_nullifiers.get(gen_key2) {
            return Err(b"Nullifier2AlreadySpent".to_vec());
        }

        // Rollover mode validation
        if is_rollover != U256::ZERO && is_rollover != U256::from(1) {
            return Err(b"INVALID_IS_ROLLOVER_FLAG".to_vec());
        }
        if is_rollover == U256::ZERO {
            if merchant_amount == U256::ZERO {
                return Err(b"MERCHANT_AMOUNT_ZERO".to_vec());
            }
        } else {
            if merchant_amount != U256::ZERO {
                return Err(b"MERCHANT_AMOUNT_MUST_BE_ZERO_IN_ROLLOVER".to_vec());
            }
            if protocol_fee != U256::ZERO {
                return Err(b"PROTOCOL_FEE_MUST_BE_ZERO_IN_ROLLOVER".to_vec());
            }
        }

        // leaf_count validation
        if leaf_count == U256::ZERO {
            return Err(b"INVALID_LEAF_COUNT".to_vec());
        }

        // flags_packed range check (0..=3)
        if flags_packed > U256::from(3) {
            return Err(b"INVALID_FLAGS_PACKED".to_vec());
        }

        // Note root must be recorded in accepted_note_roots
        let root_timestamp = self.accepted_note_roots.get(note_root);
        if root_timestamp == U256::ZERO {
            return Err(b"UNACCEPTED_NOTE_ROOT".to_vec());
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

        // Construct 19 public input scalars in big-endian EVM format (C4.3 & Phase 6)
        let mut recipient_bytes = [0u8; 32];
        recipient_bytes[12..].copy_from_slice(recipient.as_slice());

        let mut contract_bytes = [0u8; 32];
        contract_bytes[12..].copy_from_slice(self.env_contract_address().as_slice());

        let chain_id_bytes = U256::from(self.env_chain_id()).to_be_bytes::<32>();
        let leaf_count_bytes = leaf_count.to_be_bytes::<32>();
        let epoch1_bytes = epoch_id_1.to_be_bytes::<32>();
        let epoch2_bytes = epoch_id_2.to_be_bytes::<32>();
        let merchant_amount_bytes = merchant_amount.to_be_bytes::<32>();
        let protocol_fee_bytes = protocol_fee.to_be_bytes::<32>();
        let execution_fee_bytes = execution_fee.to_be_bytes::<32>();
        let expiry_bytes = expiry.to_be_bytes::<32>();
        let flags_packed_bytes = flags_packed.to_be_bytes::<32>();
        let is_rollover_bytes = is_rollover.to_be_bytes::<32>();

        // Unpack 32-byte quote_hash into two 128-bit EVM words (C4.3)
        let quote_hash_slice = quote_hash.as_slice();
        let mut quote_hi = [0u8; 32];
        quote_hi[16..32].copy_from_slice(&quote_hash_slice[0..16]);
        let mut quote_lo = [0u8; 32];
        quote_lo[16..32].copy_from_slice(&quote_hash_slice[16..32]);

        let public_inputs: [[u8; 32]; 19] = [
            note_root.0,           // 0
            leaf_count_bytes,      // 1
            nullifier_1.0,         // 2
            nullifier_2.0,         // 3
            epoch1_bytes,          // 4
            epoch2_bytes,          // 5
            output_cm_1.0,         // 6
            output_cm_2.0,         // 7
            recipient_bytes,       // 8
            merchant_amount_bytes, // 9
            protocol_fee_bytes,    // 10
            execution_fee_bytes,   // 11
            quote_hi,              // 12
            quote_lo,              // 13
            chain_id_bytes,        // 14
            contract_bytes,        // 15
            expiry_bytes,          // 16
            flags_packed_bytes,    // 17
            is_rollover_bytes,     // 18
        ];

        // Validate canonicality for each of 19 public input scalars
        for input_scalar in &public_inputs {
            if crate::types::from_evm_scalar(input_scalar).is_none() {
                return Err(b"NON_CANONICAL_PUBLIC_INPUT_SCALAR".to_vec());
            }
        }

        // Compute linear combination L in G1 via MSM precompile (0x0c, 3,200 bytes buffer)
        let public_inputs_g1 =
            crate::groth16_joinsplit_verifier::compute_joinsplit_public_inputs_g1(&public_inputs)?;

        // Verify Groth16 proof via 4-pairing precompile check (0x0f, 1,536 bytes buffer)
        let is_valid = crate::groth16_joinsplit_verifier::verify_joinsplit_groth16(
            &proof_a_neg,
            &proof_b,
            &proof_c,
            &public_inputs_g1,
        )?;
        if !is_valid {
            return Err(b"INVALID_GROTH16_JOINSPLIT_PROOF".to_vec());
        }

        // 2. EFFECTS
        // C5.1: Nullifier Burning State Update
        self.generation_nullifiers.insert(gen_key1, true);
        self.generation_nullifiers.insert(gen_key2, true);
        self.note_nullifiers.insert(nullifier_1, true);
        self.note_nullifiers.insert(nullifier_2, true);

        // C5.2: Direction B Constant-Arity MMR Appends (Always execute exactly 2 MMR insertions)
        self._mmr_insert(output_cm_1, Vec::new())?;
        self._mmr_insert(output_cm_2, Vec::new())?;

        // Multi-liability accounting
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
        crate::events::emit_event(crate::events::JoinSplitSpendExecuted {
            nullifier_1,
            nullifier_2,
            recipient,
            merchant_amount,
            output_cm_1,
            output_cm_2,
            flags_packed,
        });

        if protocol_fee > U256::ZERO {
            crate::events::emit_event(crate::events::ProtocolFee {
                nullifier: nullifier_1,
                recipient,
                amount: merchant_amount,
                fee_bps: U256::from(45),
                protocol_fee,
            });
        }

        if execution_fee > U256::ZERO {
            crate::events::emit_event(crate::events::ExecutionFee {
                nullifier: nullifier_1,
                execution_fee,
                max_execution_fee,
            });
        }

        // 5. SOLVENCY INVARIANT (C5.4)
        self.check_liability_invariant()?;

        Ok(true)
    }
}
