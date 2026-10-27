//! Spend and Cross-Chain Intent Execution
//!
//! Handles Groth16 zero-knowledge private note spend operations (PrivateNoteCircuit & JoinSplitCircuit)
//! and Chainlink CCIP cross-chain message handling.

use alloc::vec::Vec;
use alloy_primitives::{Address, FixedBytes, U256};
use ark_bls12_381::G2Affine;
use ark_ec::AffineRepr;
use stylus_sdk::abi::Bytes;
use stylus_sdk::call::RawCall;
use stylus_sdk::prelude::Call;

use crate::constants::BLS12_PAIRING_CHECK;
use crate::interfaces::IErc20;
use crate::storage::Zeltra;

impl Zeltra {

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
        flags_packed: U256,
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

        // Strict bit-bounds check on flags_packed (PATCH AUDIT KRITIS #2)
        // Bit 0: is_rollover, Bit 1: asset_id => valid range: 0..=3
        if flags_packed > U256::from(3) {
            return Err(b"FLAGS_PACKED_MALLEABLE_BITS".to_vec());
        }
        let is_rollover = flags_packed & U256::from(1);
        let asset_id: u8 = ((flags_packed >> 1usize) & U256::from(1)).byte(0);

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

        // NOL CALLER TOKEN INJECTION: Resolve token purely from storage registry
        let target_token = self.asset_tokens.get(U256::from(asset_id));
        if target_token == Address::ZERO {
            return Err(b"ASSET_NOT_CONFIGURED".to_vec());
        }

        // Fail-Fast Check: Validate total debit and note liability BEFORE expensive ZK precompile calls
        let total_debit = merchant_amount
            .checked_add(protocol_fee)
            .ok_or_else(|| b"TOTAL_DEBIT_OVERFLOW".to_vec())?
            .checked_add(execution_fee)
            .ok_or_else(|| b"TOTAL_DEBIT_OVERFLOW".to_vec())?;

        let asset_key = U256::from(asset_id);
        let user_liab = self.user_note_liabilities.get(asset_key);
        if user_liab < total_debit {
            return Err(b"INSUFFICIENT_NOTE_LIABILITY".to_vec());
        }

        // Construct 16 public input scalars in big-endian EVM format
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
        let flags_packed_bytes = flags_packed.to_be_bytes::<32>();

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
            flags_packed_bytes,
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

        // Multi-liability accounting per asset (DEC-034B & DEC-038B)
        self.user_note_liabilities
            .setter(asset_key)
            .set(user_liab - total_debit);

        let cur_accrued = self.accrued_execution_fee_liabilities.get(asset_key);
        let new_accrued = cur_accrued
            .checked_add(execution_fee)
            .ok_or_else(|| b"ACCRUED_FEE_OVERFLOW".to_vec())?;
        self.accrued_execution_fee_liabilities
            .setter(asset_key)
            .set(new_accrued);

        let cur_realized = self.realized_protocol_fees.get(asset_key);
        let new_realized = cur_realized
            .checked_add(protocol_fee)
            .ok_or_else(|| b"PROTOCOL_FEE_OVERFLOW".to_vec())?;
        self.realized_protocol_fees
            .setter(asset_key)
            .set(new_realized);

        // 3. INTERACTIONS (Token transfer to recipient with scaling)
        if recipient != Address::ZERO && merchant_amount > U256::ZERO {
            self.ensure_asset_liquidity(asset_id, merchant_amount)?;
            let raw_payout = if asset_id == 0 {
                merchant_amount
                    .checked_mul(U256::from(1_000_000_000u64))
                    .ok_or_else(|| b"OVERFLOW".to_vec())?
            } else {
                merchant_amount
            };
            self.asset_safe_transfer(target_token, recipient, raw_payout)?;
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
        self._check_asset_solvency_invariant(asset_id)?;

        Ok(true)
    }

    /// Executes a Universal 2-in-2-out JoinSplit private spend or consolidation (DEC-036 / DEC-036C / DEC-043B).
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
        encrypted_memo_1: Bytes,
        encrypted_memo_2: Bytes,
    ) -> Result<bool, Vec<u8>> {
        // 1. CHECKS
        self.check_not_paused()?;

        // Calldata Length Guard (AUD-5 - Anti Relayer Gas Griefing)
        if encrypted_memo_1.len() != 96 || encrypted_memo_2.len() != 96 {
            return Err(b"INVALID_MEMO_LENGTH".to_vec());
        }

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

        // Strict bit-bounds check on flags_packed
        // Bit 0: has_change_1, Bit 1: has_change_2, Bit 2: asset_id => valid range: 0..=7
        if flags_packed > U256::from(7) {
            return Err(b"FLAGS_PACKED_MALLEABLE_BITS".to_vec());
        }
        let has_change_1 = flags_packed & U256::from(1);
        let has_change_2 = (flags_packed >> 1usize) & U256::from(1);
        let asset_id: u8 = ((flags_packed >> 2usize) & U256::from(1)).byte(0);

        // Rollover mode validation & Strict 3-Mode Mutually Exclusive State Machine (AUD-4 - Anti Silent Burn)
        if is_rollover != U256::ZERO && is_rollover != U256::from(1) {
            return Err(b"INVALID_IS_ROLLOVER_FLAG".to_vec());
        }
        if is_rollover == U256::from(1) {
            // Mode 1: Epoch Rollover (DEC-033)
            if merchant_amount != U256::ZERO {
                return Err(b"MERCHANT_AMOUNT_MUST_BE_ZERO_IN_ROLLOVER".to_vec());
            }
            if protocol_fee != U256::ZERO {
                return Err(b"PROTOCOL_FEE_MUST_BE_ZERO_IN_ROLLOVER".to_vec());
            }
            if recipient != Address::ZERO {
                return Err(b"RECIPIENT_MUST_BE_ZERO_IN_ROLLOVER".to_vec());
            }
        } else if merchant_amount == U256::ZERO {
            // Mode 2: In-Pool Transfer (DEC-043)
            if has_change_1 == U256::ZERO {
                return Err(b"IN_POOL_TRANSFER_REQUIRES_OUTPUT_1".to_vec());
            }
            if protocol_fee != U256::ZERO {
                return Err(b"PROTOCOL_FEE_MUST_BE_ZERO_FOR_IN_POOL_TRANSFER".to_vec());
            }
            if recipient != Address::ZERO {
                return Err(b"RECIPIENT_MUST_BE_ZERO_FOR_IN_POOL_TRANSFER".to_vec());
            }
        } else {
            // Mode 3: Public Outflow Spend (DEC-036) - Anti-Silent-Burn!
            if recipient == Address::ZERO {
                return Err(b"ZERO_RECIPIENT_ON_PUBLIC_SPEND".to_vec());
            }
        }

        // leaf_count validation
        if leaf_count == U256::ZERO {
            return Err(b"INVALID_LEAF_COUNT".to_vec());
        }

        // Note root must be recorded in accepted_note_roots
        let root_timestamp = self.accepted_note_roots.get(note_root);
        if root_timestamp == U256::ZERO {
            return Err(b"UNACCEPTED_NOTE_ROOT".to_vec());
        }

        // NOL CALLER TOKEN INJECTION: Resolve token purely from storage registry
        let target_token = self.asset_tokens.get(U256::from(asset_id));
        if target_token == Address::ZERO {
            return Err(b"ASSET_NOT_CONFIGURED".to_vec());
        }

        // Fail-Fast Check: Validate total debit and note liability BEFORE expensive ZK precompile calls
        let total_debit = merchant_amount
            .checked_add(protocol_fee)
            .ok_or_else(|| b"TOTAL_DEBIT_OVERFLOW".to_vec())?
            .checked_add(execution_fee)
            .ok_or_else(|| b"TOTAL_DEBIT_OVERFLOW".to_vec())?;

        let asset_key = U256::from(asset_id);
        let user_liab = self.user_note_liabilities.get(asset_key);
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

        // C5.2: Direction B Constant-Arity MMR Appends with Real Memos (DEC-043B)
        self._mmr_insert(output_cm_1, encrypted_memo_1.to_vec())?;
        self._mmr_insert(output_cm_2, encrypted_memo_2.to_vec())?;

        // Multi-liability accounting per asset (DEC-034B & DEC-038B)
        self.user_note_liabilities
            .setter(asset_key)
            .set(user_liab - total_debit);

        let cur_accrued = self.accrued_execution_fee_liabilities.get(asset_key);
        let new_accrued = cur_accrued
            .checked_add(execution_fee)
            .ok_or_else(|| b"ACCRUED_FEE_OVERFLOW".to_vec())?;
        self.accrued_execution_fee_liabilities
            .setter(asset_key)
            .set(new_accrued);

        let cur_realized = self.realized_protocol_fees.get(asset_key);
        let new_realized = cur_realized
            .checked_add(protocol_fee)
            .ok_or_else(|| b"PROTOCOL_FEE_OVERFLOW".to_vec())?;
        self.realized_protocol_fees
            .setter(asset_key)
            .set(new_realized);

        // 3. INTERACTIONS (Token transfer to recipient with scaling - Mode 3 only)
        if is_rollover == U256::ZERO && merchant_amount > U256::ZERO && recipient != Address::ZERO {
            self.ensure_asset_liquidity(asset_id, merchant_amount)?;
            let raw_payout = if asset_id == 0 {
                merchant_amount
                    .checked_mul(U256::from(1_000_000_000u64))
                    .ok_or_else(|| b"OVERFLOW".to_vec())?
            } else {
                merchant_amount
            };
            self.asset_safe_transfer(target_token, recipient, raw_payout)?;
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
        self._check_asset_solvency_invariant(asset_id)?;

        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::*;
    use alloy_primitives::{address, Address, FixedBytes, U256};
    use stylus_sdk::abi::Bytes;

    #[test]
    fn test_spend_joinsplit_in_pool_usdc_success() {
        reset_test_state();
        let owner = address!("1111111111111111111111111111111111111111");
        let stablecoin = address!("3333333333333333333333333333333333333333");
        let weth9 = address!("4444444444444444444444444444444444444444");
        set_msg_sender(owner);
        set_block_timestamp(1000);

        let mut contract = Zeltra::default();
        contract
            .init_multi_asset(owner, weth9, stablecoin, Address::ZERO)
            .unwrap();

        let initial_root =
            FixedBytes::<32>::from_slice(&crate::poseidon_w5_constants::EMPTY_TREE_ROOT_BYTES);
        contract
            .accepted_note_roots
            .insert(initial_root, U256::from(1000));

        // Setup 100 USDC liability and backing balance (asset_id = 1)
        let initial_liab = U256::from(100_000_000); // 100 USDC
        contract
            .user_note_liabilities
            .setter(U256::from(1))
            .set(initial_liab);
        set_mock_asset_balance(1, initial_liab);

        let nullifier_1 = FixedBytes::<32>::from_slice(&[0x1a; 32]);
        let nullifier_2 = FixedBytes::<32>::from_slice(&[0x1b; 32]);
        let transfer_cm_1 = FixedBytes::<32>::from_slice(&[0x2a; 32]);
        let change_cm_2 = FixedBytes::<32>::from_slice(&[0x2b; 32]);
        let quote_hash = FixedBytes::<32>::from_slice(&[0x3a; 32]);

        let proof_a = crate::tests::valid_test_g1();
        let proof_b = crate::tests::valid_test_g2();
        let proof_c = crate::tests::valid_test_g1();

        // Dual 96-byte AEAD memos (DEC-043B / AUD-5)
        let memo_1: Bytes = vec![0xaa; 96].into();
        let memo_2: Bytes = vec![0xbb; 96].into();

        let execution_fee = U256::from(20_000); // 0.02 USDC execution fee
        let max_execution_fee = U256::from(25_000);

        // flags_packed = 7: has_change_1 = 1, has_change_2 = 1, asset_id = 1 (USDC)
        let flags_packed = U256::from(7);

        // Mode 2 (In-Pool Transfer): is_rollover = 0, merchant_amount = 0, protocol_fee = 0, recipient = ZERO
        let result = contract.spend_joinsplit(
            proof_a,
            proof_b,
            proof_c,
            initial_root,
            U256::from(1),
            nullifier_1,
            nullifier_2,
            U256::ZERO,
            U256::ZERO,
            transfer_cm_1,
            change_cm_2,
            Address::ZERO,
            U256::ZERO, // merchant_amount == 0
            U256::ZERO, // protocol_fee == 0
            execution_fee,
            max_execution_fee,
            quote_hash,
            U256::from(2000), // expiry
            flags_packed,
            U256::ZERO, // is_rollover == 0
            memo_1,
            memo_2,
        );

        assert_eq!(result, Ok(true));
        assert!(contract.note_nullifiers.get(nullifier_1));
        assert!(contract.note_nullifiers.get(nullifier_2));

        // MMR leaf count incremented by 2
        assert_eq!(contract.mmr_leaf_count.get(), U256::from(2));

        // Liability debited by exactly execution fee
        assert_eq!(
            contract.user_note_liabilities.get(U256::from(1)),
            initial_liab - execution_fee
        );
        assert_eq!(
            contract
                .accrued_execution_fee_liabilities
                .get(U256::from(1)),
            execution_fee
        );
        assert_eq!(
            contract.realized_protocol_fees.get(U256::from(1)),
            U256::ZERO
        );
    }

    #[test]
    fn test_spend_joinsplit_in_pool_weth_success() {
        reset_test_state();
        let owner = address!("1111111111111111111111111111111111111111");
        let stablecoin = address!("3333333333333333333333333333333333333333");
        let weth9 = address!("4444444444444444444444444444444444444444");
        set_msg_sender(owner);
        set_block_timestamp(1000);

        let mut contract = Zeltra::default();
        contract
            .init_multi_asset(owner, weth9, stablecoin, Address::ZERO)
            .unwrap();

        let initial_root =
            FixedBytes::<32>::from_slice(&crate::poseidon_w5_constants::EMPTY_TREE_ROOT_BYTES);
        contract
            .accepted_note_roots
            .insert(initial_root, U256::from(1000));

        // Setup 50 WETH liability and backing balance (asset_id = 0)
        let initial_liab = U256::from(50_000_000_000u64); // 50 Gwei / WETH note units
        contract
            .user_note_liabilities
            .setter(U256::ZERO)
            .set(initial_liab);
        set_mock_asset_balance(0, initial_liab);

        let nullifier_1 = FixedBytes::<32>::from_slice(&[0x2a; 32]);
        let nullifier_2 = FixedBytes::<32>::from_slice(&[0x2b; 32]);
        let transfer_cm_1 = FixedBytes::<32>::from_slice(&[0x3a; 32]);
        let change_cm_2 = FixedBytes::<32>::from_slice(&[0x3b; 32]);
        let quote_hash = FixedBytes::<32>::from_slice(&[0x4a; 32]);

        let proof_a = crate::tests::valid_test_g1();
        let proof_b = crate::tests::valid_test_g2();
        let proof_c = crate::tests::valid_test_g1();

        let memo_1: Bytes = vec![0xcc; 96].into();
        let memo_2: Bytes = vec![0xdd; 96].into();

        let execution_fee = U256::from(10_000_000);
        let max_execution_fee = U256::from(15_000_000);

        // flags_packed = 3: has_change_1 = 1, has_change_2 = 1, asset_id = 0 (WETH)
        let flags_packed = U256::from(3);

        let result = contract.spend_joinsplit(
            proof_a,
            proof_b,
            proof_c,
            initial_root,
            U256::from(1),
            nullifier_1,
            nullifier_2,
            U256::ZERO,
            U256::ZERO,
            transfer_cm_1,
            change_cm_2,
            Address::ZERO,
            U256::ZERO,
            U256::ZERO,
            execution_fee,
            max_execution_fee,
            quote_hash,
            U256::from(2000),
            flags_packed,
            U256::ZERO,
            memo_1,
            memo_2,
        );

        assert_eq!(result, Ok(true));
        assert!(contract.note_nullifiers.get(nullifier_1));
        assert!(contract.note_nullifiers.get(nullifier_2));
        assert_eq!(contract.mmr_leaf_count.get(), U256::from(2));

        assert_eq!(
            contract.user_note_liabilities.get(U256::ZERO),
            initial_liab - execution_fee
        );
        assert_eq!(
            contract.accrued_execution_fee_liabilities.get(U256::ZERO),
            execution_fee
        );
        assert_eq!(contract.realized_protocol_fees.get(U256::ZERO), U256::ZERO);
    }

    #[test]
    fn test_in_pool_rejects_non_zero_recipient() {
        reset_test_state();
        let owner = address!("1111111111111111111111111111111111111111");
        let non_zero_recipient = address!("9999999999999999999999999999999999999999");
        set_msg_sender(owner);
        set_block_timestamp(1000);

        let mut contract = Zeltra::default();
        contract.init(owner, Address::ZERO, Address::ZERO).unwrap();

        let initial_root =
            FixedBytes::<32>::from_slice(&crate::poseidon_w5_constants::EMPTY_TREE_ROOT_BYTES);
        contract
            .accepted_note_roots
            .insert(initial_root, U256::from(1000));
        contract
            .user_note_liabilities
            .setter(U256::from(1))
            .set(U256::from(100_000_000));

        let nf1 = FixedBytes::<32>::from_slice(&[0x51; 32]);
        let nf2 = FixedBytes::<32>::from_slice(&[0x52; 32]);

        // Mode 2 with recipient != Address::ZERO must revert RECIPIENT_MUST_BE_ZERO_FOR_IN_POOL_TRANSFER
        let result = contract.spend_joinsplit(
            vec![0x11; 128].into(),
            vec![0x22; 256].into(),
            vec![0x33; 128].into(),
            initial_root,
            U256::from(1),
            nf1,
            nf2,
            U256::ZERO,
            U256::ZERO,
            FixedBytes::ZERO,
            FixedBytes::ZERO,
            non_zero_recipient,
            U256::ZERO, // merchant_amount == 0
            U256::ZERO, // protocol_fee == 0
            U256::from(10_000),
            U256::from(20_000),
            FixedBytes::ZERO,
            U256::from(2000),
            U256::from(5), // has_change_1 = 1, asset_id = 1
            U256::ZERO,    // is_rollover == 0
            vec![0xaa; 96].into(),
            vec![0xbb; 96].into(),
        );

        assert_eq!(
            result,
            Err(b"RECIPIENT_MUST_BE_ZERO_FOR_IN_POOL_TRANSFER".to_vec())
        );
        assert!(!contract.note_nullifiers.get(nf1));
        assert!(!contract.note_nullifiers.get(nf2));
        assert_eq!(contract.mmr_leaf_count.get(), U256::ZERO);
    }

    #[test]
    fn test_spend_joinsplit_rejects_non_zero_recipient() {
        test_in_pool_rejects_non_zero_recipient();
    }

    #[test]
    fn test_in_pool_rejects_non_zero_protocol_fee() {
        reset_test_state();
        let owner = address!("1111111111111111111111111111111111111111");
        set_msg_sender(owner);
        set_block_timestamp(1000);

        let mut contract = Zeltra::default();
        contract.init(owner, Address::ZERO, Address::ZERO).unwrap();

        let initial_root =
            FixedBytes::<32>::from_slice(&crate::poseidon_w5_constants::EMPTY_TREE_ROOT_BYTES);
        contract
            .accepted_note_roots
            .insert(initial_root, U256::from(1000));
        contract
            .user_note_liabilities
            .setter(U256::from(1))
            .set(U256::from(100_000_000));

        let nf1 = FixedBytes::<32>::from_slice(&[0x61; 32]);
        let nf2 = FixedBytes::<32>::from_slice(&[0x62; 32]);

        // Mode 2 with protocol_fee > 0 must revert PROTOCOL_FEE_MUST_BE_ZERO_FOR_IN_POOL_TRANSFER
        let result = contract.spend_joinsplit(
            vec![0x11; 128].into(),
            vec![0x22; 256].into(),
            vec![0x33; 128].into(),
            initial_root,
            U256::from(1),
            nf1,
            nf2,
            U256::ZERO,
            U256::ZERO,
            FixedBytes::ZERO,
            FixedBytes::ZERO,
            Address::ZERO,
            U256::ZERO,         // merchant_amount == 0
            U256::from(22_500), // protocol_fee > 0
            U256::from(10_000),
            U256::from(20_000),
            FixedBytes::ZERO,
            U256::from(2000),
            U256::from(5), // has_change_1 = 1, asset_id = 1
            U256::ZERO,    // is_rollover == 0
            vec![0xaa; 96].into(),
            vec![0xbb; 96].into(),
        );

        assert_eq!(
            result,
            Err(b"PROTOCOL_FEE_MUST_BE_ZERO_FOR_IN_POOL_TRANSFER".to_vec())
        );
        assert!(!contract.note_nullifiers.get(nf1));
        assert!(!contract.note_nullifiers.get(nf2));
        assert_eq!(contract.mmr_leaf_count.get(), U256::ZERO);
    }

    #[test]
    fn test_spend_joinsplit_rejects_non_zero_protocol_fee() {
        test_in_pool_rejects_non_zero_protocol_fee();
    }

    #[test]
    fn test_in_pool_rejects_missing_output_1() {
        reset_test_state();
        let owner = address!("1111111111111111111111111111111111111111");
        set_msg_sender(owner);
        set_block_timestamp(1000);

        let mut contract = Zeltra::default();
        contract.init(owner, Address::ZERO, Address::ZERO).unwrap();

        let initial_root =
            FixedBytes::<32>::from_slice(&crate::poseidon_w5_constants::EMPTY_TREE_ROOT_BYTES);
        contract
            .accepted_note_roots
            .insert(initial_root, U256::from(1000));
        contract
            .user_note_liabilities
            .setter(U256::from(1))
            .set(U256::from(100_000_000));

        let nf1 = FixedBytes::<32>::from_slice(&[0x71; 32]);
        let nf2 = FixedBytes::<32>::from_slice(&[0x72; 32]);

        // Mode 2 with has_change_1 == 0 (flags_packed = 4: asset_id = 1, has_change_1 = 0, has_change_2 = 0)
        // must revert IN_POOL_TRANSFER_REQUIRES_OUTPUT_1
        let result = contract.spend_joinsplit(
            vec![0x11; 128].into(),
            vec![0x22; 256].into(),
            vec![0x33; 128].into(),
            initial_root,
            U256::from(1),
            nf1,
            nf2,
            U256::ZERO,
            U256::ZERO,
            FixedBytes::ZERO,
            FixedBytes::ZERO,
            Address::ZERO,
            U256::ZERO,
            U256::ZERO,
            U256::from(10_000),
            U256::from(20_000),
            FixedBytes::ZERO,
            U256::from(2000),
            U256::from(4), // has_change_1 = 0!
            U256::ZERO,    // is_rollover == 0
            vec![0xaa; 96].into(),
            vec![0xbb; 96].into(),
        );

        assert_eq!(result, Err(b"IN_POOL_TRANSFER_REQUIRES_OUTPUT_1".to_vec()));
        assert!(!contract.note_nullifiers.get(nf1));
        assert!(!contract.note_nullifiers.get(nf2));
        assert_eq!(contract.mmr_leaf_count.get(), U256::ZERO);
    }

    #[test]
    fn test_spend_joinsplit_rejects_missing_output_1() {
        test_in_pool_rejects_missing_output_1();
    }

    #[test]
    fn test_public_spend_rejects_zero_recipient() {
        reset_test_state();
        let owner = address!("1111111111111111111111111111111111111111");
        set_msg_sender(owner);
        set_block_timestamp(1000);

        let mut contract = Zeltra::default();
        contract.init(owner, Address::ZERO, Address::ZERO).unwrap();

        let initial_root =
            FixedBytes::<32>::from_slice(&crate::poseidon_w5_constants::EMPTY_TREE_ROOT_BYTES);
        contract
            .accepted_note_roots
            .insert(initial_root, U256::from(1000));
        contract
            .user_note_liabilities
            .setter(U256::from(1))
            .set(U256::from(100_000_000));

        let nf1 = FixedBytes::<32>::from_slice(&[0x81; 32]);
        let nf2 = FixedBytes::<32>::from_slice(&[0x82; 32]);

        // Mode 3 (merchant_amount > 0) with recipient == Address::ZERO (Anti Silent Burn AUD-4)
        // must revert ZERO_RECIPIENT_ON_PUBLIC_SPEND
        let result = contract.spend_joinsplit(
            vec![0x11; 128].into(),
            vec![0x22; 256].into(),
            vec![0x33; 128].into(),
            initial_root,
            U256::from(1),
            nf1,
            nf2,
            U256::ZERO,
            U256::ZERO,
            FixedBytes::ZERO,
            FixedBytes::ZERO,
            Address::ZERO,         // RECIPIENT ZERO ON PUBLIC SPEND!
            U256::from(5_000_000), // merchant_amount > 0
            U256::from(22_500),
            U256::from(10_000),
            U256::from(20_000),
            FixedBytes::ZERO,
            U256::from(2000),
            U256::from(5), // asset_id = 1
            U256::ZERO,
            vec![0xaa; 96].into(),
            vec![0xbb; 96].into(),
        );

        assert_eq!(result, Err(b"ZERO_RECIPIENT_ON_PUBLIC_SPEND".to_vec()));
        assert!(!contract.note_nullifiers.get(nf1));
        assert!(!contract.note_nullifiers.get(nf2));
        assert_eq!(contract.mmr_leaf_count.get(), U256::ZERO);
    }

    #[test]
    fn test_spend_joinsplit_rejects_zero_recipient_on_public_spend() {
        test_public_spend_rejects_zero_recipient();
    }

    #[test]
    fn test_spend_joinsplit_rejects_invalid_memo_length() {
        reset_test_state();
        let owner = address!("1111111111111111111111111111111111111111");
        set_msg_sender(owner);
        set_block_timestamp(1000);

        let mut contract = Zeltra::default();
        contract.init(owner, Address::ZERO, Address::ZERO).unwrap();

        let initial_root =
            FixedBytes::<32>::from_slice(&crate::poseidon_w5_constants::EMPTY_TREE_ROOT_BYTES);
        contract
            .accepted_note_roots
            .insert(initial_root, U256::from(1000));
        contract
            .user_note_liabilities
            .setter(U256::from(1))
            .set(U256::from(100_000_000));

        let nf1 = FixedBytes::<32>::from_slice(&[0x91; 32]);
        let nf2 = FixedBytes::<32>::from_slice(&[0x92; 32]);

        // Memo 1 is 95 bytes (not 96)
        let res_95 = contract.spend_joinsplit(
            vec![0x11; 128].into(),
            vec![0x22; 256].into(),
            vec![0x33; 128].into(),
            initial_root,
            U256::from(1),
            nf1,
            nf2,
            U256::ZERO,
            U256::ZERO,
            FixedBytes::ZERO,
            FixedBytes::ZERO,
            Address::ZERO,
            U256::ZERO,
            U256::ZERO,
            U256::from(10_000),
            U256::from(20_000),
            FixedBytes::ZERO,
            U256::from(2000),
            U256::from(5),
            U256::ZERO,
            vec![0xaa; 95].into(), // 95 BYTES
            vec![0xbb; 96].into(),
        );
        assert_eq!(res_95, Err(b"INVALID_MEMO_LENGTH".to_vec()));

        // Memo 2 is 120 bytes (not 96)
        let res_120 = contract.spend_joinsplit(
            vec![0x11; 128].into(),
            vec![0x22; 256].into(),
            vec![0x33; 128].into(),
            initial_root,
            U256::from(1),
            nf1,
            nf2,
            U256::ZERO,
            U256::ZERO,
            FixedBytes::ZERO,
            FixedBytes::ZERO,
            Address::ZERO,
            U256::ZERO,
            U256::ZERO,
            U256::from(10_000),
            U256::from(20_000),
            FixedBytes::ZERO,
            U256::from(2000),
            U256::from(5),
            U256::ZERO,
            vec![0xaa; 96].into(),
            vec![0xbb; 120].into(), // 120 BYTES
        );
        assert_eq!(res_120, Err(b"INVALID_MEMO_LENGTH".to_vec()));

        // Memo 1 is 0 bytes (empty)
        let res_empty = contract.spend_joinsplit(
            vec![0x11; 128].into(),
            vec![0x22; 256].into(),
            vec![0x33; 128].into(),
            initial_root,
            U256::from(1),
            nf1,
            nf2,
            U256::ZERO,
            U256::ZERO,
            FixedBytes::ZERO,
            FixedBytes::ZERO,
            Address::ZERO,
            U256::ZERO,
            U256::ZERO,
            U256::from(10_000),
            U256::from(20_000),
            FixedBytes::ZERO,
            U256::from(2000),
            U256::from(5),
            U256::ZERO,
            vec![].into(), // 0 BYTES
            vec![0xbb; 96].into(),
        );
        assert_eq!(res_empty, Err(b"INVALID_MEMO_LENGTH".to_vec()));
    }
}
