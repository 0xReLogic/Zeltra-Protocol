#![cfg_attr(all(not(feature = "export-abi"), not(test)), no_main)]
#![allow(
    unused_variables,
    dead_code,
    unused_imports,
    clippy::too_many_arguments
)]
extern crate alloc;

#[cfg(target_arch = "wasm32")]
fn reject_runtime_randomness(_: &mut [u8]) -> Result<(), getrandom::Error> {
    let code = core::num::NonZeroU32::new(getrandom::Error::CUSTOM_START)
        .expect("getrandom custom error code must be non-zero");
    Err(getrandom::Error::from(code))
}

#[cfg(target_arch = "wasm32")]
getrandom::register_custom_getrandom!(reject_runtime_randomness);

mod constants;
mod deposit;
mod events;
pub mod groth16_joinsplit_verifier;
pub mod groth16_note_verifier;
mod helpers;
mod interfaces;
mod merkle;
mod poseidon_w5_constants;
mod spend;
mod storage;
mod types;
mod vault;
mod verification;

use alloc::vec::Vec;
use ark_bls12_381::{Fr, G1Affine, G2Affine};
use ark_ec::AffineRepr;
use ark_ff::Field;

use alloy_primitives::{keccak256, Address, FixedBytes};
use stylus_sdk::{abi::Bytes, alloy_primitives::U256, call::RawCall, prelude::*};

pub use constants::*;
pub use interfaces::*;
pub use storage::Zeltra;
pub use types::{to_evm_g1, to_evm_g2, to_evm_scalar};

#[public]
impl Zeltra {
    /// Initialize the contract and set the owner, stablecoin, and fee recipient addresses.
    pub fn init(
        &mut self,
        owner: Address,
        stablecoin_addr: Address,
        fee_recipient_addr: Address,
    ) -> Result<(), Vec<u8>> {
        if self.owner.get() != Address::ZERO {
            return Err(b"ALREADY_INITIALIZED".to_vec());
        }
        if owner == Address::ZERO {
            return Err(b"INVALID_OWNER".to_vec());
        }
        self.owner.set(owner);
        self.paused.set(false);
        let default_token = if stablecoin_addr == Address::ZERO {
            Address::repeat_byte(0xee)
        } else {
            stablecoin_addr
        };
        self.stablecoin.set(default_token);
        self.asset_tokens.setter(U256::from(1)).set(default_token);
        if self.asset_tokens.get(U256::ZERO) == Address::ZERO {
            self.asset_tokens.setter(U256::ZERO).set(default_token);
            self.weth9_address.set(default_token);
        }
        self.fee_recipient.set(fee_recipient_addr);
        self.epoch_start_timestamp
            .set(U256::from(self.block_timestamp()));
        self.generation_epoch_id.setter(U256::ZERO).set(U256::ZERO);
        Ok(())
    }

    /// Multi-asset initialization configuring both WETH9 and USDC token contracts
    pub fn init_multi_asset(
        &mut self,
        owner: Address,
        weth9_addr: Address,
        usdc_addr: Address,
        fee_recipient_addr: Address,
    ) -> Result<(), Vec<u8>> {
        self.init(owner, usdc_addr, fee_recipient_addr)?;
        self.weth9_address.set(weth9_addr);
        self.asset_tokens.setter(U256::ZERO).set(weth9_addr);
        Ok(())
    }

    pub fn set_asset_token(&mut self, asset_id: U256, token_addr: Address) -> Result<(), Vec<u8>> {
        self.check_owner()?;
        if token_addr == Address::ZERO {
            return Err(b"INVALID_TOKEN_ADDRESS".to_vec());
        }
        self.asset_tokens.setter(asset_id).set(token_addr);
        if asset_id == U256::ZERO {
            self.weth9_address.set(token_addr);
        } else if asset_id == U256::from(1) {
            self.stablecoin.set(token_addr);
        }
        Ok(())
    }

    pub fn set_weth9_address(&mut self, weth9: Address) -> Result<(), Vec<u8>> {
        self.check_owner()?;
        if weth9 == Address::ZERO {
            return Err(b"INVALID_WETH9_ADDRESS".to_vec());
        }
        self.weth9_address.set(weth9);
        self.asset_tokens.setter(U256::ZERO).set(weth9);
        Ok(())
    }

    pub fn get_asset_token(&self, asset_id: U256) -> Result<Address, Vec<u8>> {
        Ok(self.asset_tokens.get(asset_id))
    }

    pub fn weth9_address(&self) -> Result<Address, Vec<u8>> {
        Ok(self.weth9_address.get())
    }

    pub fn stablecoin(&self) -> Result<Address, Vec<u8>> {
        Ok(self.stablecoin.get())
    }

    pub fn fee_recipient(&self) -> Result<Address, Vec<u8>> {
        Ok(self.fee_recipient.get())
    }

    // --- Two-Step Governance (Issue 10) ---

    pub fn propose_owner(&mut self, new_owner: Address) -> Result<(), Vec<u8>> {
        self.check_owner()?;
        if new_owner == Address::ZERO {
            return Err(b"INVALID_NEW_OWNER".to_vec());
        }
        self.pending_owner.set(new_owner);
        Ok(())
    }

    pub fn claim_ownership(&mut self) -> Result<(), Vec<u8>> {
        let pending = self.pending_owner.get();
        if pending != self.msg_sender() {
            return Err(b"NOT_PENDING_OWNER".to_vec());
        }
        self.owner.set(pending);
        self.pending_owner.set(Address::ZERO);
        Ok(())
    }

    // --- Timelocked Admin Parameter Changes (Issue 10) ---

    pub fn propose_fee_recipient(&mut self, recipient: Address) -> Result<(), Vec<u8>> {
        self.check_owner()?;
        if recipient == Address::ZERO {
            return Err(b"INVALID_RECIPIENT".to_vec());
        }
        self.proposed_fee_recipient.set(recipient);
        self.fee_recipient_eta
            .set(U256::from(self.block_timestamp() + 86400));
        Ok(())
    }

    pub fn execute_fee_recipient(&mut self) -> Result<(), Vec<u8>> {
        self.check_owner()?;
        let eta = self.fee_recipient_eta.get();
        if eta == U256::ZERO {
            return Err(b"NO_PROPOSAL_ACTIVE".to_vec());
        }
        let current_time = U256::from(self.block_timestamp());
        if current_time < eta {
            return Err(b"TIMELOCK_NOT_EXPIRED".to_vec());
        }
        let recipient = self.proposed_fee_recipient.get();
        self.fee_recipient.set(recipient);
        self.fee_recipient_eta.set(U256::ZERO);
        Ok(())
    }

    // --- Dynamic Deposit Fee Configuration (docs/todos/fee-policy-zero-deposit.md) ---

    /// Returns the current deposit fee in basis points (view)
    pub fn deposit_fee_bps(&self) -> Result<U256, Vec<u8>> {
        Ok(self.get_deposit_fee_bps())
    }

    /// Propose a new deposit fee in basis points (max 50 bps = 0.50%) with 24h timelock
    pub fn propose_deposit_fee_bps(&mut self, new_fee_bps: U256) -> Result<(), Vec<u8>> {
        self.check_owner()?;
        if new_fee_bps > U256::from(50) {
            return Err(b"DEPOSIT_FEE_EXCEEDS_MAX".to_vec());
        }
        self.proposed_deposit_fee_bps.set(new_fee_bps);
        self.deposit_fee_eta
            .set(U256::from(self.block_timestamp() + 86400));
        Ok(())
    }

    /// Execute the proposed deposit fee change after 24h timelock
    pub fn execute_deposit_fee_bps(&mut self) -> Result<(), Vec<u8>> {
        self.check_owner()?;
        let eta = self.deposit_fee_eta.get();
        if eta == U256::ZERO {
            return Err(b"NO_PROPOSAL_ACTIVE".to_vec());
        }
        let current_time = U256::from(self.block_timestamp());
        if current_time < eta {
            return Err(b"TIMELOCK_NOT_EXPIRED".to_vec());
        }
        let old_fee_bps = self.get_deposit_fee_bps();
        let new_fee_bps = self.proposed_deposit_fee_bps.get();
        self.deposit_fee_bps.set(new_fee_bps);
        self.deposit_fee_initialized.set(true);
        self.deposit_fee_eta.set(U256::ZERO);

        crate::events::emit_event(crate::events::DepositFeeConfigUpdated {
            old_fee_bps,
            new_fee_bps,
        });

        Ok(())
    }

    /// Direct setter for deposit fee in basis points (max 50 bps = 0.50%, admin only)
    pub fn set_deposit_fee_bps(&mut self, new_fee_bps: U256) -> Result<(), Vec<u8>> {
        self.check_owner()?;
        if new_fee_bps > U256::from(50) {
            return Err(b"DEPOSIT_FEE_EXCEEDS_MAX".to_vec());
        }
        let old_fee_bps = self.get_deposit_fee_bps();
        self.deposit_fee_bps.set(new_fee_bps);
        self.deposit_fee_initialized.set(true);

        crate::events::emit_event(crate::events::DepositFeeConfigUpdated {
            old_fee_bps,
            new_fee_bps,
        });

        Ok(())
    }

    /// Set execution fee recipient address (admin only)
    pub fn set_execution_fee_recipient(&mut self, recipient: Address) -> Result<(), Vec<u8>> {
        self.check_owner()?;
        if recipient == Address::ZERO {
            return Err(b"INVALID_RECIPIENT".to_vec());
        }
        self.execution_fee_recipient.set(recipient);
        Ok(())
    }

    /// Get total accrued execution fees for USDC (view)
    pub fn get_accumulated_fees(&self) -> Result<U256, Vec<u8>> {
        Ok(self.accrued_execution_fee_liabilities.get(U256::from(1)))
    }

    /// Get total accrued execution fees for a given asset (view)
    pub fn get_accumulated_fees_of(&self, asset_id: U256) -> Result<U256, Vec<u8>> {
        Ok(self.accrued_execution_fee_liabilities.get(asset_id))
    }

    /// Get execution fee recipient address (view)
    pub fn get_execution_fee_recipient(&self) -> Result<Address, Vec<u8>> {
        Ok(self.execution_fee_recipient.get())
    }

    /// Claim accrued execution fees (recipient or admin only)
    pub fn claim_execution_fees(&mut self, asset_id: u8, amount: U256) -> Result<bool, Vec<u8>> {
        let recipient = self.execution_fee_recipient.get();
        if recipient == Address::ZERO {
            return Err(b"NO_RECIPIENT_SET".to_vec());
        }
        let caller = self.msg_sender();
        if caller != recipient && caller != self.owner.get() {
            return Err(b"NOT_AUTHORIZED".to_vec());
        }
        let asset_key = U256::from(asset_id);
        let accrued = self.accrued_execution_fee_liabilities.get(asset_key);
        if amount > accrued {
            return Err(b"INSUFFICIENT_ACCUMULATED_FEES".to_vec());
        }
        if amount == U256::ZERO {
            return Err(b"ZERO_AMOUNT".to_vec());
        }

        let token_addr = self.asset_tokens.get(asset_key);
        if token_addr == Address::ZERO {
            return Err(b"ASSET_NOT_CONFIGURED".to_vec());
        }

        // Scaling: Gwei * 10^9 = Wei for WETH, raw for USDC
        let raw_transfer_amount = if asset_id == 0 {
            amount
                .checked_mul(U256::from(1_000_000_000u64))
                .ok_or_else(|| b"OVERFLOW".to_vec())?
        } else {
            amount
        };

        // Transfer token to recipient
        self.asset_safe_transfer(token_addr, recipient, raw_transfer_amount)?;

        // Deduct liability
        self.accrued_execution_fee_liabilities
            .setter(asset_key)
            .set(accrued - amount);

        // Emit FeeClaim event
        crate::events::emit_event(crate::events::FeeClaim {
            recipient,
            amount,
            fee_type: keccak256(b"EXECUTION_FEE"),
        });

        // Enforce solvency invariant
        self._check_asset_solvency_invariant(asset_id)?;

        Ok(true)
    }

    /// Returns the sum of unspent private note liabilities held by users for USDC (DEC-016).
    pub fn user_note_liability(&self) -> Result<U256, Vec<u8>> {
        Ok(self.user_note_liabilities.get(U256::from(1)))
    }

    /// Returns the sum of unspent private note liabilities for a specific asset_id (DEC-034B).
    pub fn user_note_liabilities(&self, asset_id: U256) -> Result<U256, Vec<u8>> {
        Ok(self.user_note_liabilities.get(asset_id))
    }

    /// Returns the accrued execution fee liability owed to the relayer for USDC (DEC-016).
    pub fn accrued_execution_fee_liability(&self) -> Result<U256, Vec<u8>> {
        Ok(self.accrued_execution_fee_liabilities.get(U256::from(1)))
    }

    /// Returns the accrued execution fee liability for a specific asset_id (DEC-034B).
    pub fn accrued_execution_fee_liabilities(&self, asset_id: U256) -> Result<U256, Vec<u8>> {
        Ok(self.accrued_execution_fee_liabilities.get(asset_id))
    }

    /// Returns the realized protocol fees retained as protocol equity for USDC (DEC-016).
    pub fn realized_protocol_fees(&self) -> Result<U256, Vec<u8>> {
        Ok(self.realized_protocol_fees.get(U256::from(1)))
    }

    /// Returns the realized protocol fees for a specific asset_id (DEC-034B).
    pub fn realized_protocol_fees_of(&self, asset_id: U256) -> Result<U256, Vec<u8>> {
        Ok(self.realized_protocol_fees.get(asset_id))
    }

    /// Returns total deposited principal for USDC (DEC-016).
    pub fn total_deposited_principal(&self) -> Result<U256, Vec<u8>> {
        Ok(self.total_deposited_principals.get(U256::from(1)))
    }

    /// Returns total deposited principal for a specific asset_id (DEC-034B).
    pub fn total_deposited_principals(&self, asset_id: U256) -> Result<U256, Vec<u8>> {
        Ok(self.total_deposited_principals.get(asset_id))
    }

    /// Returns the current Merkle tree root hash. If mmr_leaf_count == 0, returns the canonical empty root (DEC-032).
    pub fn note_tree_root(&self) -> Result<FixedBytes<32>, Vec<u8>> {
        let count = self.mmr_leaf_count.get();
        if count == U256::ZERO && self.note_tree_root.get() == FixedBytes::ZERO {
            Ok(FixedBytes::from(
                poseidon_w5_constants::EMPTY_TREE_ROOT_BYTES,
            ))
        } else {
            Ok(self.note_tree_root.get())
        }
    }

    /// Returns the current total leaf count in the Merkle Mountain Range (0 .. 2^64).
    pub fn mmr_leaf_count(&self) -> Result<U256, Vec<u8>> {
        Ok(self.mmr_leaf_count.get())
    }

    /// Returns the active MMR peak at a given mountain height level.
    pub fn mmr_peak(&self, height: U256) -> Result<FixedBytes<32>, Vec<u8>> {
        Ok(self.mmr_peaks.get(height))
    }

    /// Backwards compatibility alias for next index (returns mmr_leaf_count).
    pub fn note_tree_next_index(&self) -> Result<U256, Vec<u8>> {
        Ok(self.mmr_leaf_count.get())
    }

    /// Checks if a root hash is currently accepted: either in accepted_note_roots, current root, or canonical empty root.
    pub fn is_accepted_note_root(&self, root: FixedBytes<32>) -> Result<bool, Vec<u8>> {
        Ok(self._is_accepted_note_root(root))
    }

    /// Returns root from the bounded ring buffer history (size 100).
    pub fn get_root_history(&self, slot: U256) -> Result<FixedBytes<32>, Vec<u8>> {
        let size = U256::from(poseidon_w5_constants::ROOT_HISTORY_SIZE);
        Ok(self.root_history.get(slot % size))
    }

    /// Returns the CCIP Router address configured for the contract.
    pub fn ccip_router(&self) -> Result<Address, Vec<u8>> {
        Ok(self.ccip_router.get())
    }

    /// Sets the CCIP Router address (Admin only). Router must be non-zero.
    pub fn set_ccip_router(&mut self, router: Address) -> Result<(), Vec<u8>> {
        self.check_owner()?;
        if router == Address::ZERO {
            return Err(b"INVALID_ROUTER_ADDRESS".to_vec());
        }
        self.ccip_router.set(router);
        Ok(())
    }

    /// Pause the contract.
    pub fn pause(&mut self) -> Result<(), Vec<u8>> {
        self.check_owner()?;
        self.paused.set(true);
        Ok(())
    }

    /// Unpause the contract.
    pub fn unpause(&mut self) -> Result<(), Vec<u8>> {
        self.check_owner()?;
        self.paused.set(false);
        Ok(())
    }

    /// Registers a new clean association set Merkle root (Admin/Compliance Oracle).
    pub fn register_clean_root(&mut self, root: FixedBytes<32>) -> Result<(), Vec<u8>> {
        self.check_not_paused()?;
        self.check_owner()?;
        let current_time = U256::from(self.block_timestamp());
        self.clean_association_roots.insert(root, current_time);
        Ok(())
    }

    /// Returns the registration timestamp for a clean association root (0 if unregistered).
    pub fn get_clean_root_timestamp(&self, root: FixedBytes<32>) -> Result<U256, Vec<u8>> {
        Ok(self.clean_association_roots.get(root))
    }

    /// Returns whether a nullifier has already been spent on-chain (DEC-019, DEC-025, DEC-033).
    pub fn is_nullifier_spent(&self, nullifier: FixedBytes<32>) -> Result<bool, Vec<u8>> {
        self.is_note_nullifier_spent(nullifier)
    }

    /// Batch queries multiple nullifiers for double-spend status in a single call (DEC-037).
    pub fn are_nullifiers_spent(
        &self,
        nullifiers: Vec<FixedBytes<32>>,
    ) -> Result<Vec<bool>, Vec<u8>> {
        let mut results = Vec::with_capacity(nullifiers.len());
        for nf in nullifiers {
            results.push(self.is_nullifier_spent(nf)?);
        }
        Ok(results)
    }

    /// Returns whether a private note nullifier has already been spent on-chain (DEC-025 & DEC-033).
    pub fn is_note_nullifier_spent(&self, nullifier: FixedBytes<32>) -> Result<bool, Vec<u8>> {
        let current_epoch = self.current_epoch.get().to::<u32>();
        let slot_current = (current_epoch % 2) as u8;
        let gen_key_current = crate::storage::generational_nullifier_key(slot_current, nullifier);
        let slot_prev = (current_epoch.saturating_sub(1) % 2) as u8;
        let gen_key_prev = crate::storage::generational_nullifier_key(slot_prev, nullifier);
        Ok(self.generation_nullifiers.get(gen_key_current)
            || self.generation_nullifiers.get(gen_key_prev)
            || self.note_nullifiers.get(nullifier))
    }

    /// Returns whether a generational note nullifier has already been spent on-chain (DEC-033).
    pub fn is_generational_nullifier_spent(
        &self,
        epoch_id: u32,
        nullifier: FixedBytes<32>,
    ) -> Result<bool, Vec<u8>> {
        let slot = (epoch_id % 2) as u64;
        if self.generation_epoch_id.get(U256::from(slot)) != U256::from(epoch_id) {
            return Ok(false);
        }
        let gen_key = crate::storage::generational_nullifier_key(slot as u8, nullifier);
        Ok(self.generation_nullifiers.get(gen_key) || self.note_nullifiers.get(nullifier))
    }

    pub fn set_ccip_sender_allowlist(
        &mut self,
        source_chain_selector: u64,
        sender: Bytes,
        allowed: bool,
    ) -> Result<(), Vec<u8>> {
        self.check_owner()?;
        if source_chain_selector == 0 {
            return Err(b"INVALID_SOURCE_CHAIN_SELECTOR".to_vec());
        }
        if sender.is_empty() {
            return Err(b"EMPTY_CCIP_SENDER".to_vec());
        }
        let key = self.ccip_allowlist_key(source_chain_selector, &sender);
        self.ccip_allowed_senders.insert(key, allowed);
        Ok(())
    }

    pub fn is_ccip_sender_allowed(
        &self,
        source_chain_selector: u64,
        sender: Bytes,
    ) -> Result<bool, Vec<u8>> {
        if source_chain_selector == 0 || sender.is_empty() {
            return Ok(false);
        }
        let key = self.ccip_allowlist_key(source_chain_selector, &sender);
        Ok(self.ccip_allowed_senders.get(key))
    }

    // --- Liability Invariant Check ---

    /// Verifies the invariant: contract_assets >= outstanding_liabilities for a given asset (DEC-034B)
    pub fn check_asset_solvency_invariant(&self, asset_id: u8) -> Result<(), Vec<u8>> {
        self._check_asset_solvency_invariant(asset_id)
    }

    /// Verifies the invariant: contract_assets >= outstanding_liabilities (DEC-038B).
    pub fn check_liability_invariant(&self) -> Result<(), Vec<u8>> {
        self._check_asset_solvency_invariant(1)
    }

    // --- Direct MMR Note Deposit (DEC-038B & DEC-034B) ---

    pub fn deposit_direct_note(
        &mut self,
        amount: U256,
        note_commitment: FixedBytes<32>,
        encrypted_memo: Bytes,
    ) -> Result<(), Vec<u8>> {
        self._deposit_direct_note(amount, note_commitment, encrypted_memo)
    }

    pub fn deposit(
        &mut self,
        amount: U256,
        note_commitment: FixedBytes<32>,
        encrypted_memo: Bytes,
    ) -> Result<(), Vec<u8>> {
        self.deposit_direct_note(amount, note_commitment, encrypted_memo)
    }

    pub fn deposit_direct_weth(
        &mut self,
        amount_wei: U256,
        note_commitment: FixedBytes<32>,
        encrypted_memo: Bytes,
    ) -> Result<(), Vec<u8>> {
        self._deposit_direct_weth(amount_wei, note_commitment, encrypted_memo)
    }

    #[payable]
    pub fn deposit_direct_native_eth(
        &mut self,
        note_commitment: FixedBytes<32>,
        encrypted_memo: Bytes,
    ) -> Result<(), Vec<u8>> {
        self._deposit_direct_native_eth(note_commitment, encrypted_memo)
    }

    /// Executes a private spend or rollover using a Groth16 zero-knowledge proof for PrivateNoteCircuit (Gate D, DEC-032, DEC-033, DEC-034B).
    pub fn spend_private_note(
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
        self._spend_private_note(
            note_root,
            leaf_count,
            input_nullifier,
            note_epoch_id,
            output_commitment,
            recipient,
            merchant_amount,
            protocol_fee,
            execution_fee,
            max_execution_fee,
            quote_hash,
            expiry,
            has_change,
            flags_packed,
            proof_a_neg,
            proof_b,
            proof_c,
            encrypted_memo,
        )
    }

    /// Executes a Universal 2-in-2-out JoinSplit private spend or consolidation using Groth16 (DEC-036 / DEC-036C / DEC-043B).
    pub fn spend_joinsplit(
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
        self._spend_joinsplit(
            proof_a_neg,
            proof_b,
            proof_c,
            note_root,
            leaf_count,
            nullifier_1,
            nullifier_2,
            epoch_id_1,
            epoch_id_2,
            output_cm_1,
            output_cm_2,
            recipient,
            merchant_amount,
            protocol_fee,
            execution_fee,
            max_execution_fee,
            quote_hash,
            expiry,
            flags_packed,
            is_rollover,
            encrypted_memo_1,
            encrypted_memo_2,
        )
    }

    /// Returns the current active epoch ID (DEC-033).
    pub fn get_current_epoch(&self) -> Result<u32, Vec<u8>> {
        Ok(self.current_epoch.get().to::<u32>())
    }

    /// Returns the timestamp when the current epoch started (DEC-033).
    pub fn epoch_start_timestamp(&self) -> Result<u64, Vec<u8>> {
        Ok(self.epoch_start_timestamp.get().to::<u64>())
    }

    /// Rotates the generational epoch window (DEC-033).
    /// Advances current_epoch by 1, updates recycle_slot = (current_epoch + 1) % 2,
    /// and logs EpochRotated.
    pub fn rotate_epoch(&mut self) -> Result<u32, Vec<u8>> {
        self.check_not_paused()?;
        self.check_owner()?;

        let old_epoch = self.current_epoch.get().to::<u32>();
        let new_epoch = old_epoch
            .checked_add(1)
            .ok_or_else(|| b"EPOCH_OVERFLOW".to_vec())?;
        let recycle_slot = (new_epoch % 2) as u64;

        self.current_epoch.set(U256::from(new_epoch));
        let now = self.block_timestamp();
        self.epoch_start_timestamp.set(U256::from(now));
        self.generation_epoch_id
            .setter(U256::from(recycle_slot))
            .set(U256::from(new_epoch));

        crate::events::emit_event(crate::events::EpochRotated {
            new_epoch,
            timestamp: now,
        });

        Ok(new_epoch)
    }

    pub fn ccip_receive(
        &mut self,
        message_id: FixedBytes<32>,
        source_chain_selector: u64,
        sender: Bytes,
        payload: Bytes,
    ) -> Result<(), Vec<u8>> {
        self._ccip_receive(message_id, source_chain_selector, sender, payload)
    }

    pub fn verify_groth16_proof(
        &self,
        proof_a_neg_bytes: Bytes,
        proof_b_bytes: Bytes,
        proof_c_bytes: Bytes,
        public_inputs_g1_bytes: Bytes,
        vk_alpha_bytes: Bytes,
        vk_beta_bytes: Bytes,
        vk_gamma_bytes: Bytes,
        vk_delta_bytes: Bytes,
    ) -> Result<bool, Vec<u8>> {
        self._verify_groth16_proof(
            proof_a_neg_bytes,
            proof_b_bytes,
            proof_c_bytes,
            public_inputs_g1_bytes,
            vk_alpha_bytes,
            vk_beta_bytes,
            vk_gamma_bytes,
            vk_delta_bytes,
        )
    }

    pub fn verify_compliance(
        &self,
        root: FixedBytes<32>,
        nullifier: FixedBytes<32>,
        recipient: Address,
        amount: U256,
        proof_a_neg_bytes: Bytes,
        proof_b_bytes: Bytes,
        proof_c_bytes: Bytes,
    ) -> Result<bool, Vec<u8>> {
        self._verify_compliance(
            root,
            nullifier,
            recipient,
            amount,
            proof_a_neg_bytes,
            proof_b_bytes,
            proof_c_bytes,
        )
    }

    pub fn total_assets(&mut self) -> Result<U256, Vec<u8>> {
        self._total_assets()
    }

    pub fn claim_accumulated_yield(&mut self) -> Result<U256, Vec<u8>> {
        self._claim_accumulated_yield()
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use alloy_primitives::address;
    use ark_bls12_381::{Fr, G1Affine, G2Affine};
    use ark_ff::UniformRand;
    use ark_serialize::{CanonicalDeserialize, CanonicalSerialize};
    use rand::thread_rng;

    // --- Mock Stylus HostIO Symbols to satisfy the linker during host tests ---
    use std::cell::RefCell;
    use std::collections::HashMap;

    thread_local! {
        pub(crate) static STORAGE: RefCell<HashMap<[u8; 32], [u8; 32]>> = RefCell::new(HashMap::new());
        pub(crate) static MSG_SENDER: RefCell<Address> = const { RefCell::new(Address::ZERO) };
        pub(crate) static BLOCK_TIMESTAMP: RefCell<u64> = const { RefCell::new(0) };
        pub(crate) static MSG_VALUE: RefCell<U256> = const { RefCell::new(U256::ZERO) };
        pub(crate) static ERC20_TRANSFER_FROM_RESULT: RefCell<Result<bool, Vec<u8>>> = const { RefCell::new(Ok(true)) };
        pub(crate) static ERC20_TRANSFER_RESULT: RefCell<Result<bool, Vec<u8>>> = const { RefCell::new(Ok(true)) };
        pub(crate) static WETH9_DEPOSIT_CALLS: RefCell<Vec<(Address, U256)>> = const { RefCell::new(Vec::new()) };
        pub(crate) static MOCK_ASSET_BALANCES: RefCell<HashMap<u8, U256>> = RefCell::new(HashMap::new());
        static PAIRING_RESULT: RefCell<u8> = const { RefCell::new(1) };
        static LAST_RETURN_DATA_SIZE: RefCell<usize> = const { RefCell::new(32) };
    }

    pub(crate) fn reset_test_state() {
        STORAGE.with(|s| s.borrow_mut().clear());
        MSG_SENDER.with(|s| *s.borrow_mut() = Address::ZERO);
        BLOCK_TIMESTAMP.with(|t| *t.borrow_mut() = 0);
        MSG_VALUE.with(|v| *v.borrow_mut() = U256::ZERO);
        ERC20_TRANSFER_FROM_RESULT.with(|r| *r.borrow_mut() = Ok(true));
        ERC20_TRANSFER_RESULT.with(|r| *r.borrow_mut() = Ok(true));
        WETH9_DEPOSIT_CALLS.with(|c| c.borrow_mut().clear());
        MOCK_ASSET_BALANCES.with(|m| m.borrow_mut().clear());
        PAIRING_RESULT.with(|result| *result.borrow_mut() = 1);
        LAST_RETURN_DATA_SIZE.with(|s| *s.borrow_mut() = 32);
    }

    pub(crate) fn set_msg_sender(sender: Address) {
        MSG_SENDER.with(|s| *s.borrow_mut() = sender);
    }

    pub(crate) fn set_msg_value(value: U256) {
        MSG_VALUE.with(|v| *v.borrow_mut() = value);
    }

    pub(crate) fn set_block_timestamp(ts: u64) {
        BLOCK_TIMESTAMP.with(|t| *t.borrow_mut() = ts);
    }

    pub(crate) fn set_erc20_transfer_from_result(res: Result<bool, Vec<u8>>) {
        ERC20_TRANSFER_FROM_RESULT.with(|r| *r.borrow_mut() = res);
    }

    pub(crate) fn set_erc20_transfer_result(res: Result<bool, Vec<u8>>) {
        ERC20_TRANSFER_RESULT.with(|r| *r.borrow_mut() = res);
    }

    pub(crate) fn set_mock_asset_balance(asset_id: u8, balance: U256) {
        MOCK_ASSET_BALANCES.with(|m| {
            m.borrow_mut().insert(asset_id, balance);
        });
    }

    pub(crate) fn set_pairing_result(result: bool) {
        PAIRING_RESULT.with(|value| *value.borrow_mut() = u8::from(result));
    }

    #[no_mangle]
    pub unsafe extern "C" fn msg_value(dest: *mut u8) {
        let dest_slice = std::slice::from_raw_parts_mut(dest, 32);
        MSG_VALUE.with(|v| {
            dest_slice.copy_from_slice(&v.borrow().to_be_bytes::<32>());
        });
    }

    #[no_mangle]
    pub unsafe extern "C" fn msg_sender(dest: *mut u8) {
        let dest_slice = std::slice::from_raw_parts_mut(dest, 20);
        MSG_SENDER.with(|sender| {
            dest_slice.copy_from_slice(sender.borrow().as_slice());
        });
    }

    #[no_mangle]
    pub unsafe extern "C" fn contract_address(dest: *mut u8) {
        let dest_slice = std::slice::from_raw_parts_mut(dest, 20);
        dest_slice.copy_from_slice(&[5u8; 20]);
    }

    #[no_mangle]
    pub unsafe extern "C" fn block_timestamp() -> u64 {
        BLOCK_TIMESTAMP.with(|ts| *ts.borrow())
    }

    #[no_mangle]
    pub unsafe extern "C" fn storage_load_bytes32(key: *const u8, dest: *mut u8) {
        let key_slice = std::slice::from_raw_parts(key, 32);
        let dest_slice = std::slice::from_raw_parts_mut(dest, 32);
        let mut k = [0u8; 32];
        k.copy_from_slice(key_slice);

        STORAGE.with(|storage| {
            if let Some(val) = storage.borrow().get(&k) {
                dest_slice.copy_from_slice(val);
            } else {
                for byte in dest_slice.iter_mut() {
                    *byte = 0;
                }
            }
        });
    }

    #[no_mangle]
    pub unsafe extern "C" fn storage_cache_bytes32(key: *const u8, src: *const u8) {
        let key_slice = std::slice::from_raw_parts(key, 32);
        let src_slice = std::slice::from_raw_parts(src, 32);
        let mut k = [0u8; 32];
        k.copy_from_slice(key_slice);
        let mut v = [0u8; 32];
        v.copy_from_slice(src_slice);

        STORAGE.with(|storage| {
            storage.borrow_mut().insert(k, v);
        });
    }

    #[no_mangle]
    pub unsafe extern "C" fn native_keccak256(bytes: *const u8, len: usize, output: *mut u8) {
        use tiny_keccak::{Hasher, Keccak};
        let slice = std::slice::from_raw_parts(bytes, len);
        let mut hasher = Keccak::v256();
        hasher.update(slice);
        let out_slice = std::slice::from_raw_parts_mut(output, 32);
        hasher.finalize(out_slice);
    }

    #[no_mangle]
    pub unsafe extern "C" fn call_contract(
        _contract: *const u8,
        _calldata: *const u8,
        _calldata_len: usize,
        _value: *const u8,
        _gas: u64,
        _return_data_len: *mut usize,
    ) -> u8 {
        0
    }

    #[no_mangle]
    pub unsafe extern "C" fn delegate_call_contract(
        _contract: *const u8,
        _calldata: *const u8,
        _calldata_len: usize,
        _gas: u64,
        _return_data_len: *mut usize,
    ) -> u8 {
        0
    }

    #[no_mangle]
    pub unsafe extern "C" fn static_call_contract(
        _contract: *const u8,
        _calldata: *const u8,
        _calldata_len: usize,
        _gas: u64,
        return_data_len: *mut usize,
    ) -> u8 {
        let addr_slice = std::slice::from_raw_parts(_contract, 20);
        let len = if addr_slice[19] == 0x0c || addr_slice[19] == 0x0b {
            128
        } else {
            32
        };
        LAST_RETURN_DATA_SIZE.with(|s| *s.borrow_mut() = len);
        if !return_data_len.is_null() {
            *return_data_len = len;
        }
        0 // Status code 0 is Success
    }

    #[no_mangle]
    pub unsafe extern "C" fn storage_flush_cache() {}

    #[no_mangle]
    pub unsafe extern "C" fn return_data_size() -> usize {
        LAST_RETURN_DATA_SIZE.with(|s| *s.borrow())
    }

    #[no_mangle]
    pub unsafe extern "C" fn read_return_data(dest: *mut u8, _offset: usize, size: usize) -> usize {
        let dest_slice = std::slice::from_raw_parts_mut(dest, size);
        for byte in dest_slice.iter_mut() {
            *byte = 0;
        }
        if size == 32 {
            PAIRING_RESULT.with(|result| dest_slice[31] = *result.borrow());
        }
        size
    }

    #[test]
    fn test_to_evm_g1() {
        let mut rng = thread_rng();
        let g1 = G1Affine::rand(&mut rng);
        let evm_g1 = to_evm_g1(&g1);

        assert_eq!(evm_g1.len(), 128);
        for i in 0..16 {
            assert_eq!(evm_g1[i], 0);
            assert_eq!(evm_g1[64 + i], 0);
        }
    }

    #[test]
    fn test_to_evm_g2() {
        let mut rng = thread_rng();
        let g2 = G2Affine::rand(&mut rng);
        let evm_g2 = to_evm_g2(&g2);

        assert_eq!(evm_g2.len(), 256);
        for i in 0..16 {
            assert_eq!(evm_g2[i], 0);
            assert_eq!(evm_g2[64 + i], 0);
            assert_eq!(evm_g2[128 + i], 0);
            assert_eq!(evm_g2[192 + i], 0);
        }
    }

    #[test]
    fn test_to_evm_scalar() {
        let mut rng = thread_rng();
        let scalar = Fr::rand(&mut rng);
        let evm_scalar = to_evm_scalar(&scalar);
        assert_eq!(evm_scalar.len(), 32);
    }

    #[test]
    fn test_groth16_input_length_validation() {
        let zeltra_contract = Zeltra::default();
        let result = zeltra_contract.verify_groth16_proof(
            vec![0; 100].into(),
            vec![0; 256].into(),
            vec![0; 128].into(),
            vec![0; 128].into(),
            vec![0; 128].into(),
            vec![0; 256].into(),
            vec![0; 256].into(),
            vec![0; 256].into(),
        );
        assert_eq!(result, Err(b"INVALID_INPUT_LENGTHS".to_vec()));
    }

    #[test]
    fn test_mocked_groth16_verification() {
        let zeltra_contract = Zeltra::default();
        // Since we stubbed static_call_contract, return_data_size and read_return_data to return success,
        // this test should successfully verify the Groth16 proof with valid lengths!
        let is_valid = zeltra_contract
            .verify_groth16_proof(
                vec![0; 128].into(),
                vec![0; 256].into(),
                vec![0; 128].into(),
                vec![0; 128].into(),
                vec![0; 128].into(),
                vec![0; 256].into(),
                vec![0; 256].into(),
                vec![0; 256].into(),
            )
            .unwrap();
        assert!(is_valid, "Mocked Groth16 proof verification failed!");
    }

    #[test]
    fn test_ccip_receive_decoding_and_execution() {
        reset_test_state();
        let owner = address!("1111111111111111111111111111111111111111");
        let router = address!("2222222222222222222222222222222222222222");
        let valid_sender: Bytes = alloc::vec![0xaa; 20].into();
        set_msg_sender(owner);

        let mut zeltra_contract = Zeltra::default();
        zeltra_contract
            .init(owner, Address::ZERO, Address::ZERO)
            .unwrap();

        zeltra_contract.set_ccip_router(router).unwrap();
        zeltra_contract
            .set_ccip_sender_allowlist(1, valid_sender.clone(), true)
            .unwrap();

        set_msg_sender(router);
        let message_id = FixedBytes::repeat_byte(0x99);
        let result =
            zeltra_contract.ccip_receive(message_id, 1, valid_sender.clone(), vec![].into());
        assert!(result.is_ok());

        // Test Replay protection
        let replay_result =
            zeltra_contract.ccip_receive(message_id, 1, valid_sender, vec![].into());
        assert_eq!(
            replay_result,
            Err(b"CCIP_MESSAGE_ALREADY_PROCESSED".to_vec())
        );
    }

    #[test]
    fn test_ccip_receive_negative_cases() {
        reset_test_state();
        let owner = address!("1111111111111111111111111111111111111111");
        let router = address!("2222222222222222222222222222222222222222");
        let valid_sender: Bytes = alloc::vec![0xbb; 20].into();
        set_msg_sender(owner);

        let mut zeltra_contract = Zeltra::default();
        zeltra_contract
            .init(owner, Address::ZERO, Address::ZERO)
            .unwrap();

        let payload = vec![0u8; 584].into();
        let message_id = FixedBytes::repeat_byte(0x88);

        // Case 1: CCIP router not configured (is ZERO)
        assert_eq!(
            zeltra_contract.ccip_receive(message_id, 1, valid_sender.clone(), payload),
            Err(b"CCIP_ROUTER_NOT_CONFIGURED".to_vec())
        );

        // Router cannot be set to Address::ZERO
        assert_eq!(
            zeltra_contract.set_ccip_router(Address::ZERO),
            Err(b"INVALID_ROUTER_ADDRESS".to_vec())
        );

        // Configure router
        zeltra_contract.set_ccip_router(router).unwrap();

        // Case 2: Caller is not the router (caller is owner)
        let payload = vec![0u8; 584].into();
        assert_eq!(
            zeltra_contract.ccip_receive(message_id, 1, valid_sender.clone(), payload),
            Err(b"ONLY_CCIP_ROUTER_ALLOWED".to_vec())
        );

        // Set caller to router
        set_msg_sender(router);

        // Case 3: Message ID is ZERO
        let payload = vec![0u8; 584].into();
        assert_eq!(
            zeltra_contract.ccip_receive(FixedBytes::ZERO, 1, valid_sender.clone(), payload),
            Err(b"INVALID_CCIP_MESSAGE_ID".to_vec())
        );

        // Case 4: Source chain selector is 0
        let payload = vec![0u8; 584].into();
        assert_eq!(
            zeltra_contract.ccip_receive(message_id, 0, valid_sender.clone(), payload),
            Err(b"INVALID_SOURCE_CHAIN_SELECTOR".to_vec())
        );

        // Case 5: Sender is empty
        let payload = vec![0u8; 584].into();
        assert_eq!(
            zeltra_contract.ccip_receive(message_id, 1, vec![].into(), payload),
            Err(b"EMPTY_CCIP_SENDER".to_vec())
        );

        // Case 6: Sender is not allowlisted
        let payload = vec![0u8; 584].into();
        assert_eq!(
            zeltra_contract.ccip_receive(message_id, 1, valid_sender.clone(), payload),
            Err(b"CCIP_SENDER_NOT_ALLOWED".to_vec())
        );

        // Case 7: Allowlist rejects chain selector 0 or empty sender
        set_msg_sender(owner);
        assert_eq!(
            zeltra_contract.set_ccip_sender_allowlist(0, valid_sender.clone(), true),
            Err(b"INVALID_SOURCE_CHAIN_SELECTOR".to_vec())
        );
        assert_eq!(
            zeltra_contract.set_ccip_sender_allowlist(1, vec![].into(), true),
            Err(b"EMPTY_CCIP_SENDER".to_vec())
        );
    }

    #[test]
    fn test_initialization() {
        reset_test_state();
        let owner = address!("1111111111111111111111111111111111111111");
        set_msg_sender(owner);

        let mut contract = Zeltra::default();
        // Init with ZERO owner should fail
        assert_eq!(
            contract.init(Address::ZERO, Address::ZERO, Address::ZERO),
            Err(b"INVALID_OWNER".to_vec())
        );

        // First init should succeed
        assert!(contract.init(owner, Address::ZERO, Address::ZERO).is_ok());

        // Second init should fail
        assert_eq!(
            contract.init(owner, Address::ZERO, Address::ZERO),
            Err(b"ALREADY_INITIALIZED".to_vec())
        );
    }

    #[test]
    fn test_pause_unpause() {
        reset_test_state();
        let owner = address!("1111111111111111111111111111111111111111");
        let non_owner = address!("2222222222222222222222222222222222222222");

        set_msg_sender(owner);
        let mut contract = Zeltra::default();
        contract.init(owner, Address::ZERO, Address::ZERO).unwrap();

        // Non-owner pausing should fail
        set_msg_sender(non_owner);
        assert_eq!(contract.pause(), Err(b"NOT_OWNER".to_vec()));

        // Owner pausing should succeed
        set_msg_sender(owner);
        assert!(contract.pause().is_ok());
        assert!(contract.paused.get());

        // Non-owner unpausing should fail
        set_msg_sender(non_owner);
        assert_eq!(contract.unpause(), Err(b"NOT_OWNER".to_vec()));

        // Owner unpausing should succeed
        set_msg_sender(owner);
        assert!(contract.unpause().is_ok());
        assert!(!contract.paused.get());
    }

    #[test]
    fn test_guarded_functions_fail_when_paused() {
        reset_test_state();
        let owner = address!("1111111111111111111111111111111111111111");
        set_msg_sender(owner);

        let mut contract = Zeltra::default();
        contract.init(owner, Address::ZERO, Address::ZERO).unwrap();
        contract.pause().unwrap();

        assert_eq!(
            contract.deposit_direct_note(
                U256::from(10_000_000),
                FixedBytes::repeat_byte(0x01),
                vec![0u8; 64].into()
            ),
            Err(b"CONTRACT_PAUSED".to_vec())
        );
        assert_eq!(
            contract.register_clean_root(FixedBytes::ZERO),
            Err(b"CONTRACT_PAUSED".to_vec())
        );
    }

    #[test]
    fn test_deposit_direct_note_pos1_success() {
        reset_test_state();
        let owner = Address::repeat_byte(0x01);
        let client = Address::repeat_byte(0x02);
        set_msg_sender(owner);
        set_block_timestamp(1_700_000_000);

        let mut contract = Zeltra::default();
        contract
            .init(
                owner,
                Address::repeat_byte(0x03),
                Address::repeat_byte(0x04),
            )
            .unwrap();

        set_msg_sender(client);
        let amount = U256::from(10_000_000);
        let commitment = FixedBytes::repeat_byte(0x01);
        let memo = vec![0xaa; 96].into();

        let res = contract.deposit_direct_note(amount, commitment, memo);
        assert!(res.is_ok());

        assert_eq!(contract.user_note_liabilities.get(U256::from(1)), amount);
        assert_eq!(
            contract.total_deposited_principals.get(U256::from(1)),
            amount
        );
        assert_eq!(contract.mmr_leaf_count.get(), U256::from(1));

        let root = contract.note_tree_root.get();
        assert_ne!(root, FixedBytes::ZERO);
        assert!(contract._is_accepted_note_root(root));
        assert_eq!(
            contract.accepted_note_roots.get(root),
            U256::from(1_700_000_000)
        );
        assert_eq!(contract.root_history.get(U256::ZERO), root);
        assert_eq!(contract.root_history_index.get(), U256::from(1));
    }

    #[test]
    fn test_deposit_direct_note_neg1_non_canonical_scalar() {
        reset_test_state();
        let owner = Address::repeat_byte(0x01);
        set_msg_sender(owner);
        let mut contract = Zeltra::default();
        contract
            .init(
                owner,
                Address::repeat_byte(0x03),
                Address::repeat_byte(0x04),
            )
            .unwrap();

        let non_canonical = FixedBytes::repeat_byte(0xff);
        let res = contract.deposit_direct_note(
            U256::from(10_000_000),
            non_canonical,
            vec![0xaa; 96].into(),
        );
        assert_eq!(res, Err(b"INVALID_NOTE_COMMITMENT_SCALAR".to_vec()));
        assert_eq!(
            contract.user_note_liabilities.get(U256::from(1)),
            U256::ZERO
        );
        assert_eq!(
            contract.total_deposited_principals.get(U256::from(1)),
            U256::ZERO
        );
        assert_eq!(contract.mmr_leaf_count.get(), U256::ZERO);
        assert_eq!(contract.note_tree_root.get(), FixedBytes::ZERO);
    }

    #[test]
    fn test_deposit_direct_note_neg2_zero_commitment() {
        reset_test_state();
        let owner = Address::repeat_byte(0x01);
        set_msg_sender(owner);
        let mut contract = Zeltra::default();
        contract
            .init(
                owner,
                Address::repeat_byte(0x03),
                Address::repeat_byte(0x04),
            )
            .unwrap();

        let res = contract.deposit_direct_note(
            U256::from(10_000_000),
            FixedBytes::ZERO,
            vec![0xaa; 64].into(),
        );
        assert_eq!(res, Err(b"ZERO_COMMITMENT_REJECTED".to_vec()));
        assert_eq!(
            contract.user_note_liabilities.get(U256::from(1)),
            U256::ZERO
        );
        assert_eq!(
            contract.total_deposited_principals.get(U256::from(1)),
            U256::ZERO
        );
        assert_eq!(contract.mmr_leaf_count.get(), U256::ZERO);
        assert_eq!(contract.note_tree_root.get(), FixedBytes::ZERO);
    }

    #[test]
    fn test_deposit_direct_note_neg3_amount_too_small() {
        reset_test_state();
        let owner = Address::repeat_byte(0x01);
        set_msg_sender(owner);
        let mut contract = Zeltra::default();
        contract
            .init(
                owner,
                Address::repeat_byte(0x03),
                Address::repeat_byte(0x04),
            )
            .unwrap();

        let res = contract.deposit_direct_note(
            U256::from(9_999_999),
            FixedBytes::repeat_byte(0x01),
            vec![0xaa; 64].into(),
        );
        assert_eq!(res, Err(b"AMOUNT_TOO_SMALL".to_vec()));
        assert_eq!(
            contract.user_note_liabilities.get(U256::from(1)),
            U256::ZERO
        );
        assert_eq!(
            contract.total_deposited_principals.get(U256::from(1)),
            U256::ZERO
        );
        assert_eq!(contract.mmr_leaf_count.get(), U256::ZERO);
        assert_eq!(contract.note_tree_root.get(), FixedBytes::ZERO);
    }

    #[test]
    fn test_deposit_direct_note_neg4_invalid_memo_length() {
        reset_test_state();
        let owner = Address::repeat_byte(0x01);
        set_msg_sender(owner);
        let mut contract = Zeltra::default();
        contract
            .init(
                owner,
                Address::repeat_byte(0x03),
                Address::repeat_byte(0x04),
            )
            .unwrap();

        let res_short = contract.deposit_direct_note(
            U256::from(10_000_000),
            FixedBytes::repeat_byte(0x01),
            vec![0xaa; 95].into(),
        );
        assert_eq!(res_short, Err(b"INVALID_MEMO_LENGTH".to_vec()));
        assert_eq!(
            contract.user_note_liabilities.get(U256::from(1)),
            U256::ZERO
        );
        assert_eq!(
            contract.total_deposited_principals.get(U256::from(1)),
            U256::ZERO
        );
        assert_eq!(contract.mmr_leaf_count.get(), U256::ZERO);
        assert_eq!(contract.note_tree_root.get(), FixedBytes::ZERO);

        let res_long = contract.deposit_direct_note(
            U256::from(10_000_000),
            FixedBytes::repeat_byte(0x01),
            vec![0xaa; 97].into(),
        );
        assert_eq!(res_long, Err(b"INVALID_MEMO_LENGTH".to_vec()));
        assert_eq!(
            contract.user_note_liabilities.get(U256::from(1)),
            U256::ZERO
        );
        assert_eq!(
            contract.total_deposited_principals.get(U256::from(1)),
            U256::ZERO
        );
        assert_eq!(contract.mmr_leaf_count.get(), U256::ZERO);
        assert_eq!(contract.note_tree_root.get(), FixedBytes::ZERO);
    }

    #[test]
    fn test_deposit_direct_note_with_empty_memo() {
        reset_test_state();
        let owner = Address::repeat_byte(0x01);
        let client = Address::repeat_byte(0x02);
        set_msg_sender(owner);

        let mut contract = Zeltra::default();
        contract
            .init(
                owner,
                Address::repeat_byte(0x03),
                Address::repeat_byte(0x04),
            )
            .unwrap();

        set_msg_sender(client);
        let amount = U256::from(10_000_000);
        let commitment = FixedBytes::repeat_byte(0x01);
        let empty_memo: Bytes = vec![].into();

        set_erc20_transfer_from_result(Ok(true));
        let res = contract.deposit_direct_note(amount, commitment, empty_memo);
        assert_eq!(res, Ok(()));

        assert_eq!(contract.user_note_liabilities.get(U256::from(1)), amount);
        assert_eq!(contract.mmr_leaf_count.get(), U256::from(1));
    }

    #[test]
    fn test_deposit_direct_note_neg5_paused() {
        reset_test_state();
        let owner = Address::repeat_byte(0x01);
        set_msg_sender(owner);
        let mut contract = Zeltra::default();
        contract
            .init(
                owner,
                Address::repeat_byte(0x03),
                Address::repeat_byte(0x04),
            )
            .unwrap();
        contract.pause().unwrap();

        let res = contract.deposit_direct_note(
            U256::from(10_000_000),
            FixedBytes::repeat_byte(0x01),
            vec![0xaa; 96].into(),
        );
        assert_eq!(res, Err(b"CONTRACT_PAUSED".to_vec()));
        assert_eq!(
            contract.user_note_liabilities.get(U256::from(1)),
            U256::ZERO
        );
        assert_eq!(
            contract.total_deposited_principals.get(U256::from(1)),
            U256::ZERO
        );
        assert_eq!(contract.mmr_leaf_count.get(), U256::ZERO);
        assert_eq!(contract.note_tree_root.get(), FixedBytes::ZERO);
    }

    #[test]
    fn test_deposit_direct_note_neg6_erc20_transfer_from_fails() {
        reset_test_state();
        let owner = Address::repeat_byte(0x01);
        let client = Address::repeat_byte(0x02);
        set_msg_sender(owner);

        let mut contract = Zeltra::default();
        contract
            .init(
                owner,
                Address::repeat_byte(0x03),
                Address::repeat_byte(0x04),
            )
            .unwrap();

        set_msg_sender(client);
        let amount = U256::from(10_000_000);
        let commitment = FixedBytes::repeat_byte(0x01);
        let memo: Bytes = vec![0xaa; 96].into();

        // Subcase A: transfer_from returns boolean false (silent failure token)
        set_erc20_transfer_from_result(Ok(false));
        let res_false = contract.deposit_direct_note(amount, commitment, memo.clone());
        assert_eq!(res_false, Err(b"TRANSFER_FROM_FAILED".to_vec()));
        assert_eq!(
            contract.user_note_liabilities.get(U256::from(1)),
            U256::ZERO
        );
        assert_eq!(
            contract.total_deposited_principals.get(U256::from(1)),
            U256::ZERO
        );
        assert_eq!(contract.mmr_leaf_count.get(), U256::ZERO);
        assert_eq!(contract.note_tree_root.get(), FixedBytes::ZERO);

        // Subcase B: transfer_from reverts with low-level call failure
        set_erc20_transfer_from_result(Err(b"TRANSFER_FROM_FAILED".to_vec()));
        let res_err = contract.deposit_direct_note(amount, commitment, memo);
        assert_eq!(res_err, Err(b"TRANSFER_FROM_FAILED".to_vec()));
        assert_eq!(
            contract.user_note_liabilities.get(U256::from(1)),
            U256::ZERO
        );
        assert_eq!(
            contract.total_deposited_principals.get(U256::from(1)),
            U256::ZERO
        );
        assert_eq!(contract.mmr_leaf_count.get(), U256::ZERO);
        assert_eq!(contract.note_tree_root.get(), FixedBytes::ZERO);
    }

    #[test]
    fn test_deposit_direct_note_pos2_unverified_96_byte_memo_accepted() {
        reset_test_state();
        let owner = Address::repeat_byte(0x01);
        let client = Address::repeat_byte(0x02);
        set_msg_sender(owner);

        let mut contract = Zeltra::default();
        contract
            .init(
                owner,
                Address::repeat_byte(0x03),
                Address::repeat_byte(0x04),
            )
            .unwrap();

        set_msg_sender(client);
        let amount = U256::from(10_000_000);
        let valid_commitment = FixedBytes::from([0x01; 32]);
        let unverified_memo: Bytes = vec![0xde; 96].into();

        set_erc20_transfer_from_result(Ok(true));
        let res = contract.deposit_direct_note(amount, valid_commitment, unverified_memo);

        assert_eq!(res, Ok(()));
        assert_eq!(contract.user_note_liabilities.get(U256::from(1)), amount);
        assert_eq!(
            contract.total_deposited_principals.get(U256::from(1)),
            amount
        );
        assert_eq!(contract.mmr_leaf_count.get(), U256::from(1));

        let inserted_root = contract.note_tree_root.get();
        assert_ne!(inserted_root, FixedBytes::ZERO);
    }

    #[test]
    fn test_set_execution_fee_recipient() {
        reset_test_state();
        let mut contract = Zeltra::default();
        let owner = Address::repeat_byte(0x01);
        let new_recipient = Address::repeat_byte(0x04);

        contract
            .init(
                owner,
                Address::repeat_byte(0x02),
                Address::repeat_byte(0x03),
            )
            .unwrap();

        // Set execution fee recipient
        set_msg_sender(owner);
        contract.set_execution_fee_recipient(new_recipient).unwrap();

        let recipient = contract.get_execution_fee_recipient().unwrap();
        assert_eq!(recipient, new_recipient);
    }

    #[test]
    fn test_set_execution_fee_recipient_non_owner_fails() {
        reset_test_state();
        let mut contract = Zeltra::default();
        let owner = Address::repeat_byte(0x01);
        let non_owner = Address::repeat_byte(0x99);

        contract
            .init(
                owner,
                Address::repeat_byte(0x02),
                Address::repeat_byte(0x03),
            )
            .unwrap();

        // Try to set execution fee recipient as non-owner
        set_msg_sender(non_owner);
        let result = contract.set_execution_fee_recipient(Address::repeat_byte(0x04));

        assert_eq!(result, Err(b"NOT_OWNER".to_vec()));
    }

    #[test]
    fn test_claim_execution_fees_insufficient_balance() {
        reset_test_state();
        let mut contract = Zeltra::default();
        let owner = Address::repeat_byte(0x01);
        let recipient = Address::repeat_byte(0x04);

        contract
            .init(
                owner,
                Address::repeat_byte(0x02),
                Address::repeat_byte(0x03),
            )
            .unwrap();

        // Set execution fee recipient
        set_msg_sender(owner);
        contract.set_execution_fee_recipient(recipient).unwrap();

        // Try to claim more than accumulated (0)
        let result = contract.claim_execution_fees(1, U256::from(1000));
        assert_eq!(result, Err(b"INSUFFICIENT_ACCUMULATED_FEES".to_vec()));
    }

    #[test]
    fn test_claim_execution_fees_no_recipient_set() {
        reset_test_state();
        let mut contract = Zeltra::default();
        let owner = Address::repeat_byte(0x01);

        contract
            .init(
                owner,
                Address::repeat_byte(0x02),
                Address::repeat_byte(0x03),
            )
            .unwrap();

        // Try to claim without setting recipient
        set_msg_sender(owner);
        let result = contract.claim_execution_fees(1, U256::from(1000));
        assert_eq!(result, Err(b"NO_RECIPIENT_SET".to_vec()));
    }

    #[test]
    fn test_merkle_tree_empty_root_matches_kav() {
        reset_test_state();
        let contract = Zeltra::default();

        let expected_empty_root =
            FixedBytes::from(crate::poseidon_w5_constants::EMPTY_TREE_ROOT_BYTES);
        assert_eq!(
            contract.note_tree_root().unwrap(),
            expected_empty_root,
            "Initial root must match canonical empty tree root"
        );
        assert_eq!(contract.note_tree_next_index().unwrap(), U256::ZERO);
        assert!(
            contract.is_accepted_note_root(expected_empty_root).unwrap(),
            "Canonical empty root must be accepted"
        );
    }

    #[test]
    fn test_merkle_tree_single_insert_matches_kav() {
        reset_test_state();
        let mut contract = Zeltra::default();

        // KAV note commitment from DEC-016A
        let leaf = FixedBytes::from(alloy_primitives::hex!(
            "46d1b90c8a28c364fefdbb95bd709956e2f39a411750e6797fe1112b27635c59"
        ));
        // Canonical DEC-032 MMR bagged root after 1 insert: PoseidonW5([leaf, Fr(1), 0, 0], DOMAIN_MMR_BAG)
        let expected_root_after_1 = FixedBytes::from(alloy_primitives::hex!(
            "00cb33439376a56bb3e3cc73ef00b8cf8c14d6e69d27ecf2bd69e62ef2656844"
        ));

        set_block_timestamp(500);
        let (leaf_idx, new_root) = contract._merkle_insert(leaf).unwrap();

        assert_eq!(leaf_idx, U256::ZERO);
        assert_eq!(
            new_root, expected_root_after_1,
            "Root after 1 insert must exactly match DEC-032 canonical MMR root"
        );
        assert_eq!(contract.note_tree_root().unwrap(), expected_root_after_1);
        assert_eq!(contract.note_tree_next_index().unwrap(), U256::from(1));

        // Must be in accepted roots and in ring buffer history
        assert!(contract.is_accepted_note_root(new_root).unwrap());
        assert_eq!(
            contract.get_root_history(U256::ZERO).unwrap(),
            expected_root_after_1
        );
    }

    #[test]
    fn test_deposit_fee_governance_timelock_and_max_cap() {
        reset_test_state();
        let mut contract = Zeltra::default();
        let owner = Address::repeat_byte(0x01);
        let non_owner = Address::repeat_byte(0x99);

        contract
            .init(
                owner,
                Address::repeat_byte(0x02),
                Address::repeat_byte(0x03),
            )
            .unwrap();

        // 1. Non-owner cannot change fee
        set_msg_sender(non_owner);
        assert_eq!(
            contract.set_deposit_fee_bps(U256::from(10)),
            Err(b"NOT_OWNER".to_vec())
        );
        assert_eq!(
            contract.propose_deposit_fee_bps(U256::from(10)),
            Err(b"NOT_OWNER".to_vec())
        );

        // 2. Fee > 50 bps hard-cap is strictly rejected
        set_msg_sender(owner);
        assert_eq!(
            contract.set_deposit_fee_bps(U256::from(51)),
            Err(b"DEPOSIT_FEE_EXCEEDS_MAX".to_vec())
        );
        assert_eq!(
            contract.propose_deposit_fee_bps(U256::from(51)),
            Err(b"DEPOSIT_FEE_EXCEEDS_MAX".to_vec())
        );

        // 3. Two-step governance with 24h timelock
        set_block_timestamp(1000);
        contract.propose_deposit_fee_bps(U256::from(15)).unwrap();

        // Calling before timelock expires must fail
        set_block_timestamp(1000 + 3600);
        assert_eq!(
            contract.execute_deposit_fee_bps(),
            Err(b"TIMELOCK_NOT_EXPIRED".to_vec())
        );

        // Calling after 24h timelock succeeds
        set_block_timestamp(1000 + 86401);
        contract.execute_deposit_fee_bps().unwrap();
        assert_eq!(contract.deposit_fee_bps().unwrap(), U256::from(15));
    }

    #[test]
    fn test_default_uninitialized_deposit_fee() {
        reset_test_state();
        let contract = Zeltra::default();
        // Defaults to 0 bps (0.00%) when uninitialized (zero deposit fee policy)
        assert_eq!(contract.deposit_fee_bps().unwrap(), U256::ZERO);
    }

    #[test]
    fn test_spend_private_note_success_with_change() {
        reset_test_state();
        let owner = address!("1111111111111111111111111111111111111111");
        let merchant = address!("2222222222222222222222222222222222222222");
        let stablecoin = address!("3333333333333333333333333333333333333333");
        set_msg_sender(owner);
        set_block_timestamp(1000);

        let mut contract = Zeltra::default();
        contract.init(owner, stablecoin, Address::ZERO).unwrap();

        let initial_root =
            FixedBytes::<32>::from_slice(&crate::poseidon_w5_constants::EMPTY_TREE_ROOT_BYTES);
        contract
            .accepted_note_roots
            .insert(initial_root, U256::from(1000));
        contract
            .user_note_liabilities
            .setter(U256::ZERO)
            .set(U256::from(100_000_000)); // 100 USDC liability

        let nullifier = FixedBytes::<32>::from_slice(&[0x0a; 32]);
        let change_cm = FixedBytes::<32>::from_slice(&[0x01; 32]);
        let quote_hash = FixedBytes::<32>::from_slice(&[0x05; 32]);

        let proof_a = vec![0x11; 128].into();
        let proof_b = vec![0x22; 256].into();
        let proof_c = vec![0x33; 128].into();

        let result = contract.spend_private_note(
            initial_root,
            U256::from(1),
            nullifier,
            U256::ZERO,
            change_cm,
            merchant,
            U256::from(5_000_000), // 5 USDC
            U256::from(22_500),    // protocol fee
            U256::from(23_000),    // execution fee
            U256::from(25_000),    // max execution fee
            quote_hash,
            U256::from(2000), // expiry
            U256::from(1),    // has_change = 1
            U256::ZERO,       // is_rollover = 0
            proof_a,
            proof_b,
            proof_c,
            vec![].into(),
        );

        assert_eq!(result, Ok(true));
        assert!(contract.note_nullifiers.get(nullifier));
        // Remaining liability = 100M - 5M - 22.5k - 23k = 94_954_500
        assert_eq!(
            contract.user_note_liabilities.get(U256::ZERO),
            U256::from(94_954_500)
        );
        assert_eq!(
            contract.accrued_execution_fee_liabilities.get(U256::ZERO),
            U256::from(23_000)
        );
        assert_eq!(
            contract.realized_protocol_fees.get(U256::ZERO),
            U256::from(22_500)
        );
        assert_eq!(contract.mmr_leaf_count.get(), U256::from(1));
    }

    #[test]
    fn test_spend_private_note_double_spend_rejected() {
        reset_test_state();
        let owner = address!("1111111111111111111111111111111111111111");
        let merchant = address!("2222222222222222222222222222222222222222");
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
            .setter(U256::ZERO)
            .set(U256::from(100_000_000));

        let nullifier = FixedBytes::<32>::from_slice(&[0x0b; 32]);
        let change_cm = FixedBytes::<32>::from_slice(&[0x02; 32]);

        // First spend succeeds
        contract
            .spend_private_note(
                initial_root,
                U256::from(1),
                nullifier,
                U256::ZERO,
                change_cm,
                merchant,
                U256::from(5_000_000),
                U256::from(22_500),
                U256::from(23_000),
                U256::from(25_000),
                FixedBytes::ZERO,
                U256::from(2000),
                U256::from(1),
                U256::ZERO,
                vec![0x11; 128].into(),
                vec![0x22; 256].into(),
                vec![0x33; 128].into(),
                vec![].into(),
            )
            .unwrap();

        // Second spend with identical nullifier MUST revert
        let result = contract.spend_private_note(
            initial_root,
            U256::from(1),
            nullifier,
            U256::ZERO,
            change_cm,
            merchant,
            U256::from(5_000_000),
            U256::from(22_500),
            U256::from(23_000),
            U256::from(25_000),
            FixedBytes::ZERO,
            U256::from(2000),
            U256::from(1),
            U256::ZERO,
            vec![0x11; 128].into(),
            vec![0x22; 256].into(),
            vec![0x33; 128].into(),
            vec![].into(),
        );

        assert_eq!(result, Err(b"NOTE_ALREADY_SPENT".to_vec()));
    }

    #[test]
    fn test_spend_private_note_unaccepted_root_rejected() {
        reset_test_state();
        let owner = address!("1111111111111111111111111111111111111111");
        let mut contract = Zeltra::default();
        contract.init(owner, Address::ZERO, Address::ZERO).unwrap();

        let fake_root = FixedBytes::<32>::from_slice(&[0xee; 32]);
        let nullifier = FixedBytes::<32>::from_slice(&[0xcc; 32]);

        let result = contract.spend_private_note(
            fake_root,
            U256::from(1),
            nullifier,
            U256::ZERO,
            FixedBytes::ZERO,
            Address::ZERO,
            U256::from(1_000_000),
            U256::ZERO,
            U256::ZERO,
            U256::ZERO,
            FixedBytes::ZERO,
            U256::ZERO,
            U256::ZERO,
            U256::ZERO,
            vec![0x11; 128].into(),
            vec![0x22; 256].into(),
            vec![0x33; 128].into(),
            vec![].into(),
        );

        assert_eq!(result, Err(b"UNACCEPTED_NOTE_ROOT".to_vec()));
    }

    #[test]
    fn test_spend_private_note_expired_rejected() {
        reset_test_state();
        let owner = address!("1111111111111111111111111111111111111111");
        let mut contract = Zeltra::default();
        contract.init(owner, Address::ZERO, Address::ZERO).unwrap();

        let root =
            FixedBytes::<32>::from_slice(&crate::poseidon_w5_constants::EMPTY_TREE_ROOT_BYTES);
        contract.accepted_note_roots.insert(root, U256::from(1000));
        set_block_timestamp(5000);

        let result = contract.spend_private_note(
            root,
            U256::from(1),
            FixedBytes::ZERO,
            U256::ZERO,
            FixedBytes::ZERO,
            Address::ZERO,
            U256::from(1_000_000),
            U256::ZERO,
            U256::ZERO,
            U256::ZERO,
            FixedBytes::ZERO,
            U256::from(4999), // expired
            U256::ZERO,
            U256::ZERO,
            vec![0x11; 128].into(),
            vec![0x22; 256].into(),
            vec![0x33; 128].into(),
            vec![].into(),
        );

        assert_eq!(result, Err(b"TRANSACTION_EXPIRED".to_vec()));
    }

    #[test]
    fn test_spend_private_note_execution_fee_exceeded_rejected() {
        reset_test_state();
        let owner = address!("1111111111111111111111111111111111111111");
        let mut contract = Zeltra::default();
        contract.init(owner, Address::ZERO, Address::ZERO).unwrap();

        let root =
            FixedBytes::<32>::from_slice(&crate::poseidon_w5_constants::EMPTY_TREE_ROOT_BYTES);
        contract.accepted_note_roots.insert(root, U256::from(1000));

        let result = contract.spend_private_note(
            root,
            U256::from(1),
            FixedBytes::ZERO,
            U256::ZERO,
            FixedBytes::ZERO,
            Address::ZERO,
            U256::from(1_000_000),
            U256::ZERO,
            U256::from(30_000), // actual fee
            U256::from(25_000), // max fee
            FixedBytes::ZERO,
            U256::ZERO,
            U256::ZERO,
            U256::ZERO,
            vec![0x11; 128].into(),
            vec![0x22; 256].into(),
            vec![0x33; 128].into(),
            vec![].into(),
        );

        assert_eq!(result, Err(b"EXECUTION_FEE_EXCEEDS_MAX".to_vec()));
    }

    #[test]
    fn test_spend_private_note_zero_change_inconsistent_commitment_rejected() {
        reset_test_state();
        let owner = address!("1111111111111111111111111111111111111111");
        let mut contract = Zeltra::default();
        contract.init(owner, Address::ZERO, Address::ZERO).unwrap();

        let root =
            FixedBytes::<32>::from_slice(&crate::poseidon_w5_constants::EMPTY_TREE_ROOT_BYTES);
        contract.accepted_note_roots.insert(root, U256::from(1000));

        let non_zero_commitment = FixedBytes::<32>::from_slice(&[0x77; 32]);

        let result = contract.spend_private_note(
            root,
            U256::from(1),
            FixedBytes::ZERO,
            U256::ZERO,
            non_zero_commitment,
            Address::ZERO,
            U256::from(1_000_000),
            U256::ZERO,
            U256::ZERO,
            U256::ZERO,
            FixedBytes::ZERO,
            U256::ZERO,
            U256::ZERO, // has_change = 0 but non_zero_commitment
            U256::ZERO,
            vec![0x11; 128].into(),
            vec![0x22; 256].into(),
            vec![0x33; 128].into(),
            vec![].into(),
        );

        assert_eq!(result, Err(b"NONZERO_COMMITMENT_WITH_ZERO_CHANGE".to_vec()));
    }

    #[test]
    fn test_spend_private_note_insufficient_note_liability_rejected() {
        reset_test_state();
        let owner = address!("1111111111111111111111111111111111111111");
        let mut contract = Zeltra::default();
        contract.init(owner, Address::ZERO, Address::ZERO).unwrap();

        let root =
            FixedBytes::<32>::from_slice(&crate::poseidon_w5_constants::EMPTY_TREE_ROOT_BYTES);
        contract.accepted_note_roots.insert(root, U256::from(1000));
        contract
            .user_note_liabilities
            .setter(U256::ZERO)
            .set(U256::from(1_000_000)); // only 1 USDC liability

        let result = contract.spend_private_note(
            root,
            U256::from(1),
            FixedBytes::ZERO,
            U256::ZERO,
            FixedBytes::ZERO,
            Address::ZERO,
            U256::from(5_000_000), // requires 5 USDC
            U256::ZERO,
            U256::ZERO,
            U256::ZERO,
            FixedBytes::ZERO,
            U256::ZERO,
            U256::ZERO,
            U256::ZERO,
            vec![0x11; 128].into(),
            vec![0x22; 256].into(),
            vec![0x33; 128].into(),
            vec![].into(),
        );

        assert_eq!(result, Err(b"INSUFFICIENT_NOTE_LIABILITY".to_vec()));
    }

    #[test]
    fn test_spend_private_note_invalid_groth16_proof_rejected() {
        reset_test_state();
        let owner = address!("1111111111111111111111111111111111111111");
        let mut contract = Zeltra::default();
        contract.init(owner, Address::ZERO, Address::ZERO).unwrap();

        let root =
            FixedBytes::<32>::from_slice(&crate::poseidon_w5_constants::EMPTY_TREE_ROOT_BYTES);
        contract.accepted_note_roots.insert(root, U256::from(1000));
        contract
            .user_note_liabilities
            .setter(U256::ZERO)
            .set(U256::from(100_000_000));

        // Set mock pairing check to return false (0)
        set_pairing_result(false);

        let result = contract.spend_private_note(
            root,
            U256::from(1),
            FixedBytes::ZERO,
            U256::ZERO,
            FixedBytes::ZERO,
            Address::ZERO,
            U256::from(1_000_000),
            U256::ZERO,
            U256::ZERO,
            U256::ZERO,
            FixedBytes::ZERO,
            U256::ZERO,
            U256::ZERO,
            U256::ZERO,
            vec![0x11; 128].into(),
            vec![0x22; 256].into(),
            vec![0x33; 128].into(),
            vec![].into(),
        );

        assert_eq!(result, Err(b"INVALID_GROTH16_NOTE_PROOF".to_vec()));
    }

    #[test]
    fn test_mmr_hyperbridge_out_of_bounds_leaf_rejected() {
        reset_test_state();
        let owner = address!("1111111111111111111111111111111111111111");
        let mut contract = Zeltra::default();
        contract.init(owner, Address::ZERO, Address::ZERO).unwrap();

        let root =
            FixedBytes::<32>::from_slice(&crate::poseidon_w5_constants::EMPTY_TREE_ROOT_BYTES);
        contract.accepted_note_roots.insert(root, U256::from(1000));
        contract
            .user_note_liabilities
            .setter(U256::ZERO)
            .set(U256::from(100_000_000));

        // Attempting spend with leaf_count == 0 must fail closed
        let result = contract.spend_private_note(
            root,
            U256::ZERO, // leaf_count == 0 (illegal in DEC-032)
            FixedBytes::ZERO,
            U256::ZERO,
            FixedBytes::ZERO,
            Address::ZERO,
            U256::from(1_000_000),
            U256::ZERO,
            U256::ZERO,
            U256::ZERO,
            FixedBytes::ZERO,
            U256::ZERO,
            U256::ZERO,
            U256::ZERO,
            vec![0x11; 128].into(),
            vec![0x22; 256].into(),
            vec![0x33; 128].into(),
            vec![].into(),
        );

        assert_eq!(result, Err(b"INVALID_LEAF_COUNT".to_vec()));
    }

    #[test]
    fn test_mmr_tampered_leaf_count_rejected() {
        reset_test_state();
        let owner = address!("1111111111111111111111111111111111111111");
        let mut contract = Zeltra::default();
        contract.init(owner, Address::ZERO, Address::ZERO).unwrap();

        // Valid leaf insert
        let leaf = FixedBytes::from(alloy_primitives::hex!(
            "46d1b90c8a28c364fefdbb95bd709956e2f39a411750e6797fe1112b27635c59"
        ));
        let (_, _canonical_root) = contract._mmr_insert(leaf, vec![]).unwrap();

        // Tampering with leaf_count produces a different root that is not accepted
        let fake_root = FixedBytes::repeat_byte(0x99);
        assert!(!contract.is_accepted_note_root(fake_root).unwrap());

        let result = contract.spend_private_note(
            fake_root,
            U256::from(999), // tampered leaf count
            FixedBytes::ZERO,
            U256::ZERO,
            FixedBytes::ZERO,
            Address::ZERO,
            U256::from(1_000_000),
            U256::ZERO,
            U256::ZERO,
            U256::ZERO,
            FixedBytes::ZERO,
            U256::ZERO,
            U256::ZERO,
            U256::ZERO,
            vec![0x11; 128].into(),
            vec![0x22; 256].into(),
            vec![0x33; 128].into(),
            vec![].into(),
        );

        assert_eq!(result, Err(b"UNACCEPTED_NOTE_ROOT".to_vec()));
    }

    #[test]
    fn test_mmr_wrap_around_scalar_rejected() {
        reset_test_state();
        let mut contract = Zeltra::default();

        // 0xff...ff is >= r (modulus of BLS12-381 Fr)
        let non_canonical_leaf = FixedBytes::repeat_byte(0xff);
        let result = contract._mmr_insert(non_canonical_leaf, vec![]);

        assert_eq!(result, Err(b"INVALID_LEAF_SCALAR".to_vec()));
    }

    // ═══════════════════════════════════════════════════════════════════════
    // DEC-033: Generational Epoch Window & Rollover Tests (Positive & Negative)
    // ═══════════════════════════════════════════════════════════════════════

    #[test]
    fn test_epoch_normal_spend_in_same_epoch() {
        reset_test_state();
        let owner = address!("1111111111111111111111111111111111111111");
        let merchant = address!("2222222222222222222222222222222222222222");
        set_msg_sender(owner);
        set_block_timestamp(1000);

        let mut contract = Zeltra::default();
        contract.init(owner, Address::ZERO, Address::ZERO).unwrap();

        assert_eq!(contract.get_current_epoch().unwrap(), 0);

        let root =
            FixedBytes::<32>::from_slice(&crate::poseidon_w5_constants::EMPTY_TREE_ROOT_BYTES);
        contract.accepted_note_roots.insert(root, U256::from(1000));
        contract
            .user_note_liabilities
            .setter(U256::ZERO)
            .set(U256::from(50_000_000));

        let nullifier = FixedBytes::<32>::from_slice(&[0x1a; 32]);
        let change_cm = FixedBytes::<32>::from_slice(&[0x1b; 32]);

        // Spend in epoch 0 with note_epoch_id = 0
        let res = contract.spend_private_note(
            root,
            U256::from(1),
            nullifier,
            U256::ZERO, // note_epoch_id = 0
            change_cm,
            merchant,
            U256::from(5_000_000),
            U256::from(22_500),
            U256::from(23_000),
            U256::from(25_000),
            FixedBytes::ZERO,
            U256::from(2000),
            U256::from(1),
            U256::ZERO, // is_rollover = 0
            vec![0x11; 128].into(),
            vec![0x22; 256].into(),
            vec![0x33; 128].into(),
            vec![].into(),
        );

        assert_eq!(res, Ok(true));
        assert!(contract
            .is_generational_nullifier_spent(0, nullifier)
            .unwrap());
    }

    #[test]
    fn test_epoch_grace_window_spend_epoch_minus_one() {
        reset_test_state();
        let owner = address!("1111111111111111111111111111111111111111");
        let merchant = address!("2222222222222222222222222222222222222222");
        set_msg_sender(owner);
        set_block_timestamp(1000);

        let mut contract = Zeltra::default();
        contract.init(owner, Address::ZERO, Address::ZERO).unwrap();

        // Advance to epoch 1
        set_block_timestamp(2000);
        let new_epoch = contract.rotate_epoch().unwrap();
        assert_eq!(new_epoch, 1);
        assert_eq!(contract.get_current_epoch().unwrap(), 1);

        let root =
            FixedBytes::<32>::from_slice(&crate::poseidon_w5_constants::EMPTY_TREE_ROOT_BYTES);
        contract.accepted_note_roots.insert(root, U256::from(1000));
        contract
            .user_note_liabilities
            .setter(U256::ZERO)
            .set(U256::from(50_000_000));

        let nullifier = FixedBytes::<32>::from_slice(&[0x2a; 32]);
        let change_cm = FixedBytes::<32>::from_slice(&[0x2b; 32]);

        // Spend in epoch 1 with note from epoch 0 (E - 1 grace window)
        let res = contract.spend_private_note(
            root,
            U256::from(1),
            nullifier,
            U256::ZERO, // note_epoch_id = 0 (E-1)
            change_cm,
            merchant,
            U256::from(5_000_000),
            U256::from(22_500),
            U256::from(23_000),
            U256::from(25_000),
            FixedBytes::ZERO,
            U256::from(5000),
            U256::from(1),
            U256::ZERO,
            vec![0x11; 128].into(),
            vec![0x22; 256].into(),
            vec![0x33; 128].into(),
            vec![].into(),
        );

        assert_eq!(res, Ok(true));
        assert!(contract
            .is_generational_nullifier_spent(0, nullifier)
            .unwrap());
        assert_eq!(contract.mmr_leaf_count.get(), U256::from(1));
    }

    #[test]
    fn test_epoch_standalone_refresh_execution() {
        reset_test_state();
        let owner = address!("1111111111111111111111111111111111111111");
        set_msg_sender(owner);
        set_block_timestamp(1000);

        let mut contract = Zeltra::default();
        contract.init(owner, Address::ZERO, Address::ZERO).unwrap();

        let root =
            FixedBytes::<32>::from_slice(&crate::poseidon_w5_constants::EMPTY_TREE_ROOT_BYTES);
        contract.accepted_note_roots.insert(root, U256::from(1000));
        contract
            .user_note_liabilities
            .setter(U256::ZERO)
            .set(U256::from(50_000_000));

        let nullifier = FixedBytes::<32>::from_slice(&[0x3a; 32]);
        let renewed_cm = FixedBytes::<32>::from_slice(&[0x3b; 32]);

        // Standalone refresh: is_rollover = 1, merchant = 0, pfee = 0, has_change = 1
        let res = contract.spend_private_note(
            root,
            U256::from(1),
            nullifier,
            U256::ZERO, // note_epoch_id = 0
            renewed_cm,
            Address::ZERO,
            U256::ZERO,         // merchant_amount = 0
            U256::ZERO,         // protocol_fee = 0
            U256::from(21_275), // execution fee with 15% markup
            U256::from(25_000),
            FixedBytes::ZERO,
            U256::from(2000),
            U256::from(1), // has_change = 1
            U256::from(1), // is_rollover = 1
            vec![0x11; 128].into(),
            vec![0x22; 256].into(),
            vec![0x33; 128].into(),
            vec![].into(),
        );

        assert_eq!(res, Ok(true));
        assert!(contract
            .is_generational_nullifier_spent(0, nullifier)
            .unwrap());
        assert_eq!(
            contract.accrued_execution_fee_liabilities.get(U256::ZERO),
            U256::from(21_275)
        );
    }

    #[test]
    fn test_epoch_expired_note_direct_spend_rejected() {
        reset_test_state();
        let owner = address!("1111111111111111111111111111111111111111");
        set_msg_sender(owner);
        set_block_timestamp(1000);

        let mut contract = Zeltra::default();
        contract.init(owner, Address::ZERO, Address::ZERO).unwrap();

        // Advance epoch twice: 0 -> 1 -> 2 (active window is 2 and 1, epoch 0 is expired)
        contract.rotate_epoch().unwrap();
        contract.rotate_epoch().unwrap();
        assert_eq!(contract.get_current_epoch().unwrap(), 2);

        let root =
            FixedBytes::<32>::from_slice(&crate::poseidon_w5_constants::EMPTY_TREE_ROOT_BYTES);
        contract.accepted_note_roots.insert(root, U256::from(1000));
        contract
            .user_note_liabilities
            .setter(U256::ZERO)
            .set(U256::from(50_000_000));

        let nullifier = FixedBytes::<32>::from_slice(&[0xe1; 32]);

        // Attempting to spend note from epoch 0 (E - 2) MUST revert EPOCH_EXPIRED_REQUIRE_ROLLOVER
        let res = contract.spend_private_note(
            root,
            U256::from(1),
            nullifier,
            U256::ZERO, // note_epoch_id = 0 (< current_epoch - 1)
            FixedBytes::ZERO,
            Address::ZERO,
            U256::from(1_000_000),
            U256::ZERO,
            U256::from(10_000),
            U256::from(20_000),
            FixedBytes::ZERO,
            U256::ZERO,
            U256::ZERO,
            U256::ZERO,
            vec![0x11; 128].into(),
            vec![0x22; 256].into(),
            vec![0x33; 128].into(),
            vec![].into(),
        );

        assert_eq!(res, Err(b"EPOCH_EXPIRED_REQUIRE_ROLLOVER".to_vec()));
    }

    #[test]
    fn test_epoch_in_future_rejected() {
        reset_test_state();
        let owner = address!("1111111111111111111111111111111111111111");
        set_msg_sender(owner);
        set_block_timestamp(1000);

        let mut contract = Zeltra::default();
        contract.init(owner, Address::ZERO, Address::ZERO).unwrap();

        let root =
            FixedBytes::<32>::from_slice(&crate::poseidon_w5_constants::EMPTY_TREE_ROOT_BYTES);
        contract.accepted_note_roots.insert(root, U256::from(1000));
        contract
            .user_note_liabilities
            .setter(U256::ZERO)
            .set(U256::from(50_000_000));

        // Note epoch 5 when current_epoch is 0
        let res = contract.spend_private_note(
            root,
            U256::from(1),
            FixedBytes::<32>::repeat_byte(0x12),
            U256::from(5), // Future epoch
            FixedBytes::ZERO,
            Address::ZERO,
            U256::from(1_000_000),
            U256::ZERO,
            U256::from(10_000),
            U256::from(20_000),
            FixedBytes::ZERO,
            U256::ZERO,
            U256::ZERO,
            U256::ZERO,
            vec![0x11; 128].into(),
            vec![0x22; 256].into(),
            vec![0x33; 128].into(),
            vec![].into(),
        );

        assert_eq!(res, Err(b"EPOCH_IN_FUTURE".to_vec()));
    }

    #[test]
    fn test_epoch_slot_desynchronized_rejected() {
        reset_test_state();
        let owner = address!("1111111111111111111111111111111111111111");
        set_msg_sender(owner);
        set_block_timestamp(1000);

        let mut contract = Zeltra::default();
        contract.init(owner, Address::ZERO, Address::ZERO).unwrap();

        let root =
            FixedBytes::<32>::from_slice(&crate::poseidon_w5_constants::EMPTY_TREE_ROOT_BYTES);
        contract.accepted_note_roots.insert(root, U256::from(1000));
        contract
            .user_note_liabilities
            .setter(U256::ZERO)
            .set(U256::from(50_000_000));

        // Corrupt slot 0 epoch id to 999
        contract
            .generation_epoch_id
            .setter(U256::ZERO)
            .set(U256::from(999));

        let res = contract.spend_private_note(
            root,
            U256::from(1),
            FixedBytes::<32>::repeat_byte(0x12),
            U256::ZERO,
            FixedBytes::ZERO,
            Address::ZERO,
            U256::from(1_000_000),
            U256::ZERO,
            U256::from(10_000),
            U256::from(20_000),
            FixedBytes::ZERO,
            U256::ZERO,
            U256::ZERO,
            U256::ZERO,
            vec![0x11; 128].into(),
            vec![0x22; 256].into(),
            vec![0x33; 128].into(),
            vec![].into(),
        );

        assert_eq!(res, Err(b"EPOCH_SLOT_DESYNCHRONIZED".to_vec()));
    }

    #[test]
    fn test_epoch_rollover_with_nonzero_merchant_rejected() {
        reset_test_state();
        let owner = address!("1111111111111111111111111111111111111111");
        let merchant = address!("2222222222222222222222222222222222222222");
        set_msg_sender(owner);
        set_block_timestamp(1000);

        let mut contract = Zeltra::default();
        contract.init(owner, Address::ZERO, Address::ZERO).unwrap();

        let root =
            FixedBytes::<32>::from_slice(&crate::poseidon_w5_constants::EMPTY_TREE_ROOT_BYTES);
        contract.accepted_note_roots.insert(root, U256::from(1000));

        // is_rollover = 1 with merchant > 0 MUST fail
        let res = contract.spend_private_note(
            root,
            U256::from(1),
            FixedBytes::<32>::repeat_byte(0x33),
            U256::ZERO,
            FixedBytes::<32>::repeat_byte(0x44),
            merchant,
            U256::from(5_000_000), // Non-zero merchant
            U256::ZERO,
            U256::from(10_000),
            U256::from(20_000),
            FixedBytes::ZERO,
            U256::ZERO,
            U256::from(1),
            U256::from(1), // is_rollover = 1
            vec![0x11; 128].into(),
            vec![0x22; 256].into(),
            vec![0x33; 128].into(),
            vec![].into(),
        );

        assert_eq!(
            res,
            Err(b"MERCHANT_AMOUNT_MUST_BE_ZERO_IN_ROLLOVER".to_vec())
        );
    }

    #[test]
    fn test_epoch_rollover_with_nonzero_protocol_fee_rejected() {
        reset_test_state();
        let owner = address!("1111111111111111111111111111111111111111");
        set_msg_sender(owner);
        set_block_timestamp(1000);

        let mut contract = Zeltra::default();
        contract.init(owner, Address::ZERO, Address::ZERO).unwrap();

        let root =
            FixedBytes::<32>::from_slice(&crate::poseidon_w5_constants::EMPTY_TREE_ROOT_BYTES);
        contract.accepted_note_roots.insert(root, U256::from(1000));

        // is_rollover = 1 with protocol_fee > 0 MUST fail
        let res = contract.spend_private_note(
            root,
            U256::from(1),
            FixedBytes::<32>::repeat_byte(0x33),
            U256::ZERO,
            FixedBytes::<32>::repeat_byte(0x44),
            Address::ZERO,
            U256::ZERO,
            U256::from(22_500), // Non-zero protocol fee
            U256::from(10_000),
            U256::from(20_000),
            FixedBytes::ZERO,
            U256::ZERO,
            U256::from(1),
            U256::from(1), // is_rollover = 1
            vec![0x11; 128].into(),
            vec![0x22; 256].into(),
            vec![0x33; 128].into(),
            vec![].into(),
        );

        assert_eq!(res, Err(b"PROTOCOL_FEE_MUST_BE_ZERO_IN_ROLLOVER".to_vec()));
    }

    #[test]
    fn test_epoch_rollover_without_change_note_rejected() {
        reset_test_state();
        let owner = address!("1111111111111111111111111111111111111111");
        set_msg_sender(owner);
        set_block_timestamp(1000);

        let mut contract = Zeltra::default();
        contract.init(owner, Address::ZERO, Address::ZERO).unwrap();

        let root =
            FixedBytes::<32>::from_slice(&crate::poseidon_w5_constants::EMPTY_TREE_ROOT_BYTES);
        contract.accepted_note_roots.insert(root, U256::from(1000));

        // is_rollover = 1 with has_change = 0 MUST fail
        let res = contract.spend_private_note(
            root,
            U256::from(1),
            FixedBytes::<32>::repeat_byte(0x33),
            U256::ZERO,
            FixedBytes::ZERO,
            Address::ZERO,
            U256::ZERO,
            U256::ZERO,
            U256::from(10_000),
            U256::from(20_000),
            FixedBytes::ZERO,
            U256::ZERO,
            U256::ZERO,    // has_change = 0
            U256::from(1), // is_rollover = 1
            vec![0x11; 128].into(),
            vec![0x22; 256].into(),
            vec![0x33; 128].into(),
            vec![].into(),
        );

        assert_eq!(res, Err(b"ROLLOVER_MUST_HAVE_CHANGE".to_vec()));
    }

    #[test]
    fn test_epoch_rotation_non_owner_rejected() {
        reset_test_state();
        let owner = address!("1111111111111111111111111111111111111111");
        let non_owner = address!("9999999999999999999999999999999999999999");
        set_msg_sender(owner);

        let mut contract = Zeltra::default();
        contract.init(owner, Address::ZERO, Address::ZERO).unwrap();

        set_msg_sender(non_owner);
        let res = contract.rotate_epoch();
        assert_eq!(res, Err(b"NOT_OWNER".to_vec()));
    }

    #[test]
    fn test_epoch_generational_nullifier_double_spend_rejected() {
        reset_test_state();
        let owner = address!("1111111111111111111111111111111111111111");
        let merchant = address!("2222222222222222222222222222222222222222");
        set_msg_sender(owner);
        set_block_timestamp(1000);

        let mut contract = Zeltra::default();
        contract.init(owner, Address::ZERO, Address::ZERO).unwrap();

        let root =
            FixedBytes::<32>::from_slice(&crate::poseidon_w5_constants::EMPTY_TREE_ROOT_BYTES);
        contract.accepted_note_roots.insert(root, U256::from(1000));
        contract
            .user_note_liabilities
            .setter(U256::ZERO)
            .set(U256::from(100_000_000));

        let nullifier = FixedBytes::<32>::from_slice(&[0x4a; 32]);
        let change_cm = FixedBytes::<32>::from_slice(&[0x4b; 32]);

        // Spend succeeds
        contract
            .spend_private_note(
                root,
                U256::from(1),
                nullifier,
                U256::ZERO,
                change_cm,
                merchant,
                U256::from(5_000_000),
                U256::from(22_500),
                U256::from(23_000),
                U256::from(25_000),
                FixedBytes::ZERO,
                U256::from(2000),
                U256::from(1),
                U256::ZERO,
                vec![0x11; 128].into(),
                vec![0x22; 256].into(),
                vec![0x33; 128].into(),
                vec![].into(),
            )
            .unwrap();

        // Attempt second spend in the same epoch with same nullifier
        let res = contract.spend_private_note(
            root,
            U256::from(1),
            nullifier,
            U256::ZERO,
            change_cm,
            merchant,
            U256::from(5_000_000),
            U256::from(22_500),
            U256::from(23_000),
            U256::from(25_000),
            FixedBytes::ZERO,
            U256::from(2000),
            U256::from(1),
            U256::ZERO,
            vec![0x11; 128].into(),
            vec![0x22; 256].into(),
            vec![0x33; 128].into(),
            vec![].into(),
        );

        assert_eq!(res, Err(b"NOTE_ALREADY_SPENT".to_vec()));
    }

    #[test]
    fn test_spend_joinsplit_success_1_in_dummy_change() {
        reset_test_state();
        let owner = address!("1111111111111111111111111111111111111111");
        let merchant = address!("2222222222222222222222222222222222222222");
        let stablecoin = address!("3333333333333333333333333333333333333333");
        set_msg_sender(owner);
        set_block_timestamp(1000);

        let mut contract = Zeltra::default();
        contract.init(owner, stablecoin, Address::ZERO).unwrap();

        let initial_root =
            FixedBytes::<32>::from_slice(&crate::poseidon_w5_constants::EMPTY_TREE_ROOT_BYTES);
        contract
            .accepted_note_roots
            .insert(initial_root, U256::from(1000));
        contract
            .user_note_liabilities
            .setter(U256::ZERO)
            .set(U256::from(100_000_000)); // 100 USDC liability

        let nullifier_1 = FixedBytes::<32>::from_slice(&[0x1a; 32]);
        let nullifier_2 = FixedBytes::<32>::from_slice(&[0x1b; 32]);
        let change_cm_1 = FixedBytes::<32>::from_slice(&[0x2a; 32]);
        let change_cm_2 = FixedBytes::<32>::from_slice(&[0x2b; 32]);
        let quote_hash = FixedBytes::<32>::from_slice(&[0x3a; 32]);

        let proof_a = vec![0x11; 128].into();
        let proof_b = vec![0x22; 256].into();
        let proof_c = vec![0x33; 128].into();

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
            change_cm_1,
            change_cm_2,
            merchant,
            U256::from(5_000_000), // 5 USDC
            U256::from(22_500),    // protocol fee
            U256::from(23_000),    // execution fee
            U256::from(25_000),    // max execution fee
            quote_hash,
            U256::from(2000), // expiry
            U256::from(1),    // flags_packed: 1 change note, 1 dummy
            U256::ZERO,       // is_rollover: 0
            vec![0u8; 96].into(),
            vec![0u8; 96].into(),
        );

        assert_eq!(result, Ok(true));
        assert!(contract.note_nullifiers.get(nullifier_1));
        assert!(contract.note_nullifiers.get(nullifier_2));
        // Remaining liability = 100M - 5M - 22.5k - 23k = 94_954_500
        assert_eq!(
            contract.user_note_liabilities.get(U256::ZERO),
            U256::from(94_954_500)
        );
        assert_eq!(
            contract.accrued_execution_fee_liabilities.get(U256::ZERO),
            U256::from(23_000)
        );
        assert_eq!(
            contract.realized_protocol_fees.get(U256::ZERO),
            U256::from(22_500)
        );
        // Direction B Constant Arity: exactly 2 MMR insertions
        assert_eq!(contract.mmr_leaf_count.get(), U256::from(2));
    }

    #[test]
    fn test_spend_joinsplit_singularity_aliasing_rejected() {
        reset_test_state();
        let owner = address!("1111111111111111111111111111111111111111");
        let merchant = address!("2222222222222222222222222222222222222222");
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
            .setter(U256::ZERO)
            .set(U256::from(100_000_000));

        let same_nullifier = FixedBytes::<32>::from_slice(&[0x77; 32]);
        let change_cm_1 = FixedBytes::<32>::from_slice(&[0x2a; 32]);
        let change_cm_2 = FixedBytes::<32>::from_slice(&[0x2b; 32]);

        // Attempt spend with identical nullifier_1 and nullifier_2
        let result = contract.spend_joinsplit(
            vec![0x11; 128].into(),
            vec![0x22; 256].into(),
            vec![0x33; 128].into(),
            initial_root,
            U256::from(1),
            same_nullifier,
            same_nullifier,
            U256::ZERO,
            U256::ZERO,
            change_cm_1,
            change_cm_2,
            merchant,
            U256::from(5_000_000),
            U256::from(22_500),
            U256::from(23_000),
            U256::from(25_000),
            FixedBytes::ZERO,
            U256::from(2000),
            U256::ZERO,
            U256::ZERO,
            vec![0u8; 96].into(),
            vec![0u8; 96].into(),
        );

        assert_eq!(result, Err(b"DuplicateNullifierInSameTx".to_vec()));
        assert!(!contract.note_nullifiers.get(same_nullifier));
        assert_eq!(
            contract.user_note_liabilities.get(U256::ZERO),
            U256::from(100_000_000)
        );
        assert_eq!(contract.mmr_leaf_count.get(), U256::ZERO);
    }

    #[test]
    fn test_spend_joinsplit_double_spend_nullifier_1_rejected() {
        reset_test_state();
        let owner = address!("1111111111111111111111111111111111111111");
        let merchant = address!("2222222222222222222222222222222222222222");
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
            .setter(U256::ZERO)
            .set(U256::from(100_000_000));

        let nf1 = FixedBytes::<32>::from_slice(&[0x88; 32]);
        let nf2 = FixedBytes::<32>::from_slice(&[0x89; 32]);

        // Pre-mark nullifier 1 as spent
        contract.note_nullifiers.insert(nf1, true);

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
            merchant,
            U256::from(5_000_000),
            U256::from(22_500),
            U256::from(23_000),
            U256::from(25_000),
            FixedBytes::ZERO,
            U256::from(2000),
            U256::ZERO,
            U256::ZERO,
            vec![0u8; 96].into(),
            vec![0u8; 96].into(),
        );

        assert_eq!(result, Err(b"Nullifier1AlreadySpent".to_vec()));
        assert!(!contract.note_nullifiers.get(nf2));
        assert_eq!(
            contract.user_note_liabilities.get(U256::ZERO),
            U256::from(100_000_000)
        );
    }

    #[test]
    fn test_spend_joinsplit_double_spend_nullifier_2_rejected() {
        reset_test_state();
        let owner = address!("1111111111111111111111111111111111111111");
        let merchant = address!("2222222222222222222222222222222222222222");
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
            .setter(U256::ZERO)
            .set(U256::from(100_000_000));

        let nf1 = FixedBytes::<32>::from_slice(&[0x98; 32]);
        let nf2 = FixedBytes::<32>::from_slice(&[0x99; 32]);

        // Pre-mark nullifier 2 as spent
        contract.note_nullifiers.insert(nf2, true);

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
            merchant,
            U256::from(5_000_000),
            U256::from(22_500),
            U256::from(23_000),
            U256::from(25_000),
            FixedBytes::ZERO,
            U256::from(2000),
            U256::ZERO,
            U256::ZERO,
            vec![0u8; 96].into(),
            vec![0u8; 96].into(),
        );

        assert_eq!(result, Err(b"Nullifier2AlreadySpent".to_vec()));
        assert!(!contract.note_nullifiers.get(nf1));
        assert_eq!(
            contract.user_note_liabilities.get(U256::ZERO),
            U256::from(100_000_000)
        );
    }

    #[test]
    fn test_spend_joinsplit_quote_expired_rejected() {
        reset_test_state();
        let owner = address!("1111111111111111111111111111111111111111");
        let merchant = address!("2222222222222222222222222222222222222222");
        set_msg_sender(owner);
        set_block_timestamp(5000);

        let mut contract = Zeltra::default();
        contract.init(owner, Address::ZERO, Address::ZERO).unwrap();

        let initial_root =
            FixedBytes::<32>::from_slice(&crate::poseidon_w5_constants::EMPTY_TREE_ROOT_BYTES);
        contract
            .accepted_note_roots
            .insert(initial_root, U256::from(1000));
        contract
            .user_note_liabilities
            .setter(U256::ZERO)
            .set(U256::from(100_000_000));

        let nf1 = FixedBytes::<32>::from_slice(&[0xa1; 32]);
        let nf2 = FixedBytes::<32>::from_slice(&[0xa2; 32]);

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
            merchant,
            U256::from(5_000_000),
            U256::from(22_500),
            U256::from(23_000),
            U256::from(25_000),
            FixedBytes::ZERO,
            U256::from(4999), // expiry < 5000
            U256::ZERO,
            U256::ZERO,
            vec![0u8; 96].into(),
            vec![0u8; 96].into(),
        );

        assert_eq!(result, Err(b"QuoteExpired".to_vec()));
    }

    #[test]
    fn test_spend_joinsplit_insufficient_liability_rejected() {
        reset_test_state();
        let owner = address!("1111111111111111111111111111111111111111");
        let merchant = address!("2222222222222222222222222222222222222222");
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
            .setter(U256::ZERO)
            .set(U256::from(1_000_000)); // Only 1 USDC liability

        let nf1 = FixedBytes::<32>::from_slice(&[0xb1; 32]);
        let nf2 = FixedBytes::<32>::from_slice(&[0xb2; 32]);

        // Attempt spend of 5 USDC (> 1 USDC liability)
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
            merchant,
            U256::from(5_000_000),
            U256::from(22_500),
            U256::from(23_000),
            U256::from(25_000),
            FixedBytes::ZERO,
            U256::from(2000),
            U256::ZERO,
            U256::ZERO,
            vec![0u8; 96].into(),
            vec![0u8; 96].into(),
        );

        assert_eq!(result, Err(b"INSUFFICIENT_NOTE_LIABILITY".to_vec()));
    }

    #[test]
    fn test_spend_joinsplit_invalid_proof_rejected() {
        reset_test_state();
        let owner = address!("1111111111111111111111111111111111111111");
        let merchant = address!("2222222222222222222222222222222222222222");
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
            .setter(U256::ZERO)
            .set(U256::from(100_000_000));

        let nf1 = FixedBytes::<32>::from_slice(&[0x1c; 32]);
        let nf2 = FixedBytes::<32>::from_slice(&[0x1d; 32]);

        set_pairing_result(false);

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
            merchant,
            U256::from(5_000_000),
            U256::from(22_500),
            U256::from(23_000),
            U256::from(25_000),
            FixedBytes::ZERO,
            U256::from(2000),
            U256::ZERO,
            U256::ZERO,
            vec![0u8; 96].into(),
            vec![0u8; 96].into(),
        );

        assert_eq!(result, Err(b"INVALID_GROTH16_JOINSPLIT_PROOF".to_vec()));
    }

    #[test]
    fn test_spend_joinsplit_non_canonical_scalar_rejected() {
        reset_test_state();
        let owner = address!("1111111111111111111111111111111111111111");
        let merchant = address!("2222222222222222222222222222222222222222");
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
            .setter(U256::ZERO)
            .set(U256::from(100_000_000));

        // Non-canonical scalar: 0xFF >= r
        let non_canonical_nf = FixedBytes::<32>::from_slice(&[0xff; 32]);
        let nf2 = FixedBytes::<32>::from_slice(&[0x1d; 32]);

        let result = contract.spend_joinsplit(
            vec![0x11; 128].into(),
            vec![0x22; 256].into(),
            vec![0x33; 128].into(),
            initial_root,
            U256::from(1),
            non_canonical_nf,
            nf2,
            U256::ZERO,
            U256::ZERO,
            FixedBytes::ZERO,
            FixedBytes::ZERO,
            merchant,
            U256::from(5_000_000),
            U256::from(22_500),
            U256::from(23_000),
            U256::from(25_000),
            FixedBytes::ZERO,
            U256::from(2000),
            U256::ZERO,
            U256::ZERO,
            vec![0u8; 96].into(),
            vec![0u8; 96].into(),
        );

        assert_eq!(result, Err(b"NON_CANONICAL_PUBLIC_INPUT_SCALAR".to_vec()));
    }

    // ═══════════════════════════════════════════════════════════════════════
    // DEC-037: Note Encryption Memo & Viewing Key Event Pipeline
    // ═══════════════════════════════════════════════════════════════════════

    #[test]
    fn test_dec037_encrypted_memo_lifecycle() {
        reset_test_state();
        let owner = address!("1111111111111111111111111111111111111111");
        let stablecoin = address!("3333333333333333333333333333333333333333");
        let client = address!("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
        let merchant = address!("2222222222222222222222222222222222222222");
        set_msg_sender(owner);
        set_block_timestamp(1000);

        let mut contract = Zeltra::default();
        contract.init(owner, stablecoin, Address::ZERO).unwrap();

        // 1. Direct deposit with note commitment and memo (DEC-038B)
        set_msg_sender(client);
        let initial_cm = FixedBytes::<32>::from_slice(&[0x0a; 32]);
        let deposit_amount = U256::from(20_000_000); // 20 USDC
        let deposit_memo: Bytes = vec![0x7a; 96].into();

        contract
            .deposit_direct_note(deposit_amount, initial_cm, deposit_memo)
            .unwrap();

        assert_eq!(contract.mmr_leaf_count().unwrap(), U256::from(1));
        assert_eq!(contract.user_note_liability().unwrap(), deposit_amount);

        // 3. Spend private note and append change note with change_encrypted_memo
        let current_root = contract.note_tree_root().unwrap();
        let nullifier = FixedBytes::<32>::from_slice(&[0x0a; 32]);
        let change_cm = FixedBytes::<32>::from_slice(&[0x0b; 32]);
        let change_memo: Bytes = vec![0x8b; 96].into();

        let spend_ok = contract
            .spend_private_note(
                current_root,
                U256::from(1),
                nullifier,
                U256::ZERO,
                change_cm,
                merchant,
                U256::from(5_000_000), // 5 USDC
                U256::from(22_500),    // 0.45% protocol fee
                U256::from(23_000),    // execution fee
                U256::from(25_000),    // max execution fee
                FixedBytes::ZERO,
                U256::from(5000),
                U256::from(1), // has_change = 1
                U256::from(2), // flags_packed: is_rollover = 0, asset_id = 1 (USDC)
                vec![0x11; 128].into(),
                vec![0x22; 256].into(),
                vec![0x33; 128].into(),
                change_memo,
            )
            .unwrap();

        assert!(spend_ok);
        assert!(contract.is_nullifier_spent(nullifier).unwrap());
        let unspent_nf = FixedBytes::repeat_byte(0xfe);
        let batch_res = contract
            .are_nullifiers_spent(vec![nullifier, unspent_nf])
            .unwrap();
        assert_eq!(batch_res, vec![true, false]);
        // MMR leaf count incremented to 2 (1 initial deposit + 1 change note)
        assert_eq!(contract.mmr_leaf_count().unwrap(), U256::from(2));
    }

    // ═══════════════════════════════════════════════════════════════════════
    // TASK B (DEC-034B): Multi-Asset Siloed Accounting & Solvency Tests
    // ═══════════════════════════════════════════════════════════════════════

    #[test]
    fn test_weth_payable_wrap_success_cei_order() {
        reset_test_state();
        let owner = address!("1111111111111111111111111111111111111111");
        let weth9 = address!("2222222222222222222222222222222222222222");
        let usdc = address!("3333333333333333333333333333333333333333");
        let fee_recipient = address!("4444444444444444444444444444444444444444");
        let user = address!("5555555555555555555555555555555555555555");

        let mut contract = Zeltra::default();
        contract
            .init_multi_asset(owner, weth9, usdc, fee_recipient)
            .unwrap();

        // 0.01 ETH = 10^16 Wei = 10_000_000 Gwei
        let amount_wei = U256::from(10_000_000_000_000_000u64);
        let amount_gwei = U256::from(10_000_000u64);
        set_msg_sender(user);
        set_msg_value(amount_wei);

        let note_cm = FixedBytes::repeat_byte(0x01);
        let memo: Bytes = vec![0x77; 96].into();

        // Pre-condition
        assert_eq!(
            contract.user_note_liabilities(U256::ZERO).unwrap(),
            U256::ZERO
        );
        assert_eq!(
            contract.total_deposited_principals(U256::ZERO).unwrap(),
            U256::ZERO
        );
        assert_eq!(contract.mmr_leaf_count.get(), U256::ZERO);

        // Execute payable native ETH auto-wrap deposit
        contract.deposit_direct_native_eth(note_cm, memo).unwrap();

        // Verify post-conditions
        assert_eq!(
            contract.user_note_liabilities(U256::ZERO).unwrap(),
            amount_gwei
        );
        assert_eq!(
            contract.total_deposited_principals(U256::ZERO).unwrap(),
            amount_gwei
        );
        assert_eq!(contract.mmr_leaf_count.get(), U256::from(1));

        // Verify IWeth9::deposit{value: amount_wei}() interaction was recorded with exact Wei
        let deposit_calls = WETH9_DEPOSIT_CALLS.with(|c| c.borrow().clone());
        assert_eq!(deposit_calls.len(), 1);
        assert_eq!(deposit_calls[0], (weth9, amount_wei));
    }

    #[test]
    fn test_unaligned_gwei_wei_rejection() {
        reset_test_state();
        let owner = address!("1111111111111111111111111111111111111111");
        let weth9 = address!("2222222222222222222222222222222222222222");
        let usdc = address!("3333333333333333333333333333333333333333");
        let user = address!("5555555555555555555555555555555555555555");

        let mut contract = Zeltra::default();
        contract
            .init_multi_asset(owner, weth9, usdc, Address::ZERO)
            .unwrap();

        set_msg_sender(user);
        let note_cm = FixedBytes::repeat_byte(0x01);
        let memo: Bytes = vec![0x77; 96].into();

        // 1. Unaligned Native ETH: 0.005 ETH + 1 wei = 5_000_000_000_000_001 Wei
        let unaligned_eth = U256::from(5_000_000_000_000_001u64);
        set_msg_value(unaligned_eth);
        let err_eth = contract.deposit_direct_native_eth(note_cm, memo.clone());
        assert_eq!(err_eth, Err(b"ETH_VALUE_NOT_GWEI_ALIGNED".to_vec()));

        // 2. Unaligned WETH ERC-20: 5_000_000_000_000_999 Wei
        let unaligned_weth = U256::from(5_000_000_000_000_999u64);
        let err_weth = contract.deposit_direct_weth(unaligned_weth, note_cm, memo);
        assert_eq!(err_weth, Err(b"WETH_AMOUNT_NOT_GWEI_ALIGNED".to_vec()));

        // Verify state is completely unchanged
        assert_eq!(
            contract.user_note_liabilities(U256::ZERO).unwrap(),
            U256::ZERO
        );
        assert_eq!(contract.mmr_leaf_count.get(), U256::ZERO);
    }

    #[test]
    fn test_caller_token_injection_impossible() {
        reset_test_state();
        let owner = address!("1111111111111111111111111111111111111111");
        let weth9 = address!("2222222222222222222222222222222222222222");
        let usdc = address!("3333333333333333333333333333333333333333");
        let merchant = address!("6666666666666666666666666666666666666666");

        let mut contract = Zeltra::default();
        contract
            .init_multi_asset(owner, weth9, usdc, Address::ZERO)
            .unwrap();

        // Verify storage registration: calldata cannot alter target_token
        assert_eq!(contract.get_asset_token(U256::ZERO).unwrap(), weth9);
        assert_eq!(contract.get_asset_token(U256::from(1)).unwrap(), usdc);

        let initial_root =
            FixedBytes::<32>::from_slice(&crate::poseidon_w5_constants::EMPTY_TREE_ROOT_BYTES);
        contract
            .accepted_note_roots
            .insert(initial_root, U256::from(1000));
        contract
            .user_note_liabilities
            .setter(U256::from(1))
            .set(U256::from(50_000_000));

        // When unconfigured asset is accessed via flags_packed
        // (e.g. asset_tokens mapping at slot 1 is Address::ZERO)
        contract
            .asset_tokens
            .setter(U256::from(1))
            .set(Address::ZERO);

        let res = contract.spend_private_note(
            initial_root,
            U256::from(1),
            FixedBytes::repeat_byte(0x11),
            U256::ZERO,
            FixedBytes::repeat_byte(0x22),
            merchant,
            U256::from(5_000_000),
            U256::from(22_500),
            U256::from(23_000),
            U256::from(25_000),
            FixedBytes::ZERO,
            U256::from(2000),
            U256::from(1),
            U256::from(2), // asset_id = 1, is_rollover = 0
            vec![0x11; 128].into(),
            vec![0x22; 256].into(),
            vec![0x33; 128].into(),
            vec![].into(),
        );

        assert_eq!(res, Err(b"ASSET_NOT_CONFIGURED".to_vec()));
    }

    #[test]
    fn test_weth_relayer_fee_claim_scaling_exactness() {
        reset_test_state();
        let owner = address!("1111111111111111111111111111111111111111");
        let weth9 = address!("2222222222222222222222222222222222222222");
        let usdc = address!("3333333333333333333333333333333333333333");
        let fee_recipient = address!("4444444444444444444444444444444444444444");

        set_msg_sender(owner);
        let mut contract = Zeltra::default();
        contract
            .init_multi_asset(owner, weth9, usdc, fee_recipient)
            .unwrap();
        contract.set_execution_fee_recipient(fee_recipient).unwrap();

        // 25_000 Gwei accrued execution fee for WETH (asset_id = 0)
        let fee_gwei = U256::from(25_000u64);
        contract
            .accrued_execution_fee_liabilities
            .setter(U256::ZERO)
            .set(fee_gwei);

        // Unauthorized caller fails
        let hacker = address!("9999999999999999999999999999999999999999");
        set_msg_sender(hacker);
        let err_auth = contract.claim_execution_fees(0, fee_gwei);
        assert_eq!(err_auth, Err(b"NOT_AUTHORIZED".to_vec()));

        // Amount > accrued fails
        set_msg_sender(fee_recipient);
        let err_over = contract.claim_execution_fees(0, fee_gwei + U256::from(1));
        assert_eq!(err_over, Err(b"INSUFFICIENT_ACCUMULATED_FEES".to_vec()));

        // Zero amount fails
        let err_zero = contract.claim_execution_fees(0, U256::ZERO);
        assert_eq!(err_zero, Err(b"ZERO_AMOUNT".to_vec()));

        // Valid claim of full fee
        let ok = contract.claim_execution_fees(0, fee_gwei).unwrap();
        assert!(ok);
        assert_eq!(
            contract
                .accrued_execution_fee_liabilities(U256::ZERO)
                .unwrap(),
            U256::ZERO
        );
    }

    #[test]
    fn test_flags_packed_malleable_bits_rejection() {
        reset_test_state();
        let owner = address!("1111111111111111111111111111111111111111");
        let stablecoin = address!("3333333333333333333333333333333333333333");
        let merchant = address!("2222222222222222222222222222222222222222");
        set_msg_sender(owner);
        set_block_timestamp(1000);

        let mut contract = Zeltra::default();
        contract.init(owner, stablecoin, Address::ZERO).unwrap();

        let initial_root =
            FixedBytes::<32>::from_slice(&crate::poseidon_w5_constants::EMPTY_TREE_ROOT_BYTES);
        contract
            .accepted_note_roots
            .insert(initial_root, U256::from(1000));
        contract
            .user_note_liabilities
            .setter(U256::ZERO)
            .set(U256::from(100_000_000));

        // 1. Private note spend: valid range is 0..=3
        // flags_packed = 4 (> 3) must revert FLAGS_PACKED_MALLEABLE_BITS
        let res_spend_4 = contract.spend_private_note(
            initial_root,
            U256::from(1),
            FixedBytes::repeat_byte(0x01),
            U256::ZERO,
            FixedBytes::repeat_byte(0x02),
            merchant,
            U256::from(5_000_000),
            U256::from(22_500),
            U256::from(23_000),
            U256::from(25_000),
            FixedBytes::ZERO,
            U256::from(2000),
            U256::from(1),
            U256::from(4), // MALLEABLE BIT (> 3)
            vec![0x11; 128].into(),
            vec![0x22; 256].into(),
            vec![0x33; 128].into(),
            vec![].into(),
        );
        assert_eq!(res_spend_4, Err(b"FLAGS_PACKED_MALLEABLE_BITS".to_vec()));

        // flags_packed = 255 must also revert
        let res_spend_255 = contract.spend_private_note(
            initial_root,
            U256::from(1),
            FixedBytes::repeat_byte(0x01),
            U256::ZERO,
            FixedBytes::repeat_byte(0x02),
            merchant,
            U256::from(5_000_000),
            U256::from(22_500),
            U256::from(23_000),
            U256::from(25_000),
            FixedBytes::ZERO,
            U256::from(2000),
            U256::from(1),
            U256::from(255), // MALLEABLE BITS (> 3)
            vec![0x11; 128].into(),
            vec![0x22; 256].into(),
            vec![0x33; 128].into(),
            vec![].into(),
        );
        assert_eq!(res_spend_255, Err(b"FLAGS_PACKED_MALLEABLE_BITS".to_vec()));

        // 2. JoinSplit spend: valid range is 0..=7
        // flags_packed = 8 (> 7) must revert FLAGS_PACKED_MALLEABLE_BITS
        let res_js_8 = contract.spend_joinsplit(
            vec![0x11; 128].into(),
            vec![0x22; 256].into(),
            vec![0x33; 128].into(),
            initial_root,
            U256::from(1),
            FixedBytes::repeat_byte(0x03),
            FixedBytes::repeat_byte(0x04),
            U256::ZERO,
            U256::ZERO,
            FixedBytes::ZERO,
            FixedBytes::ZERO,
            merchant,
            U256::from(5_000_000),
            U256::from(22_500),
            U256::from(23_000),
            U256::from(25_000),
            FixedBytes::ZERO,
            U256::from(2000),
            U256::from(8), // MALLEABLE BIT (> 7)
            U256::ZERO,
            vec![0u8; 96].into(),
            vec![0u8; 96].into(),
        );
        assert_eq!(res_js_8, Err(b"FLAGS_PACKED_MALLEABLE_BITS".to_vec()));

        // flags_packed = 255 must also revert
        let res_js_255 = contract.spend_joinsplit(
            vec![0x11; 128].into(),
            vec![0x22; 256].into(),
            vec![0x33; 128].into(),
            initial_root,
            U256::from(1),
            FixedBytes::repeat_byte(0x03),
            FixedBytes::repeat_byte(0x04),
            U256::ZERO,
            U256::ZERO,
            FixedBytes::ZERO,
            FixedBytes::ZERO,
            merchant,
            U256::from(5_000_000),
            U256::from(22_500),
            U256::from(23_000),
            U256::from(25_000),
            FixedBytes::ZERO,
            U256::from(2000),
            U256::from(255), // MALLEABLE BITS (> 7)
            U256::ZERO,
            vec![0u8; 96].into(),
            vec![0u8; 96].into(),
        );
        assert_eq!(res_js_255, Err(b"FLAGS_PACKED_MALLEABLE_BITS".to_vec()));
    }

    #[test]
    fn test_usdc_revert_leaves_weth_solvent() {
        reset_test_state();
        let owner = address!("1111111111111111111111111111111111111111");
        let weth9 = address!("2222222222222222222222222222222222222222");
        let usdc = address!("3333333333333333333333333333333333333333");
        let user = address!("5555555555555555555555555555555555555555");

        let mut contract = Zeltra::default();
        contract
            .init_multi_asset(owner, weth9, usdc, Address::ZERO)
            .unwrap();

        // 1. Establish WETH pool state (0.01 ETH = 10_000_000 Gwei)
        set_msg_sender(user);
        let amount_wei = U256::from(10_000_000_000_000_000u64);
        set_msg_value(amount_wei);
        let note_cm = FixedBytes::repeat_byte(0x05);
        let memo: Bytes = vec![0x77; 96].into();
        contract
            .deposit_direct_native_eth(note_cm, memo.clone())
            .unwrap();

        // 2. Simulate Circle freezing / blacklisting USDC transfers
        set_erc20_transfer_from_result(Err(b"USDC_BLACKLISTED".to_vec()));

        // USDC deposit fails
        let res_usdc = contract.deposit_direct_note(
            U256::from(20_000_000),
            FixedBytes::repeat_byte(0x06),
            memo,
        );
        assert_eq!(res_usdc, Err(b"USDC_BLACKLISTED".to_vec()));

        // 3. Theorem Verification: WETH pool accounting and solvency invariant are completely unaffected!
        assert!(contract.check_asset_solvency_invariant(0).is_ok());
        assert_eq!(
            contract.user_note_liabilities(U256::ZERO).unwrap(),
            U256::from(10_000_000)
        );
        assert_eq!(
            contract.user_note_liabilities(U256::from(1)).unwrap(),
            U256::ZERO
        );
    }
}
