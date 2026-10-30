#![cfg_attr(all(not(feature = "export-abi"), not(test)), no_main)]
#![recursion_limit = "512"]
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

mod ccip_deposit;
mod constants;
mod deposit;
mod events;
mod governance;
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
mod views;

#[cfg(test)]
pub(crate) mod tests;

use alloc::vec::Vec;
use alloy_primitives::{Address, FixedBytes};
use stylus_sdk::{abi::Bytes, alloy_primitives::U256, prelude::*};

pub use ccip_deposit::{
    CcipAny2EVMMessage, CcipEVMTokenAmount, CCIP_RECEIVER_INTERFACE_ID, ERC165_INTERFACE_ID,
};

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
        self._init(owner, stablecoin_addr, fee_recipient_addr)
    }

    /// Multi-asset initialization configuring both WETH9 and USDC token contracts
    pub fn init_multi_asset(
        &mut self,
        owner: Address,
        weth9_addr: Address,
        usdc_addr: Address,
        fee_recipient_addr: Address,
    ) -> Result<(), Vec<u8>> {
        self._init_multi_asset(owner, weth9_addr, usdc_addr, fee_recipient_addr)
    }

    pub fn set_asset_token(&mut self, asset_id: U256, token_addr: Address) -> Result<(), Vec<u8>> {
        self._set_asset_token(asset_id, token_addr)
    }

    pub fn set_weth9_address(&mut self, weth9: Address) -> Result<(), Vec<u8>> {
        self._set_weth9_address(weth9)
    }

    pub fn get_asset_token(&self, asset_id: U256) -> Result<Address, Vec<u8>> {
        self._get_asset_token(asset_id)
    }

    pub fn weth9_address(&self) -> Result<Address, Vec<u8>> {
        self._weth9_address()
    }

    pub fn stablecoin(&self) -> Result<Address, Vec<u8>> {
        self._stablecoin()
    }

    pub fn fee_recipient(&self) -> Result<Address, Vec<u8>> {
        self._fee_recipient()
    }

    // --- Two-Step Governance (Issue 10) ---

    pub fn propose_owner(&mut self, new_owner: Address) -> Result<(), Vec<u8>> {
        self._propose_owner(new_owner)
    }

    pub fn claim_ownership(&mut self) -> Result<(), Vec<u8>> {
        self._claim_ownership()
    }

    // --- Timelocked Admin Parameter Changes (Issue 10) ---

    pub fn propose_fee_recipient(&mut self, recipient: Address) -> Result<(), Vec<u8>> {
        self._propose_fee_recipient(recipient)
    }

    pub fn execute_fee_recipient(&mut self) -> Result<(), Vec<u8>> {
        self._execute_fee_recipient()
    }

    // --- Dynamic Deposit Fee Configuration (docs/todos/fee-policy-zero-deposit.md) ---

    /// Returns the current deposit fee in basis points (view)
    pub fn deposit_fee_bps(&self) -> Result<U256, Vec<u8>> {
        self._deposit_fee_bps()
    }

    /// Propose a new deposit fee in basis points (max 50 bps = 0.50%) with 24h timelock
    pub fn propose_deposit_fee_bps(&mut self, new_fee_bps: U256) -> Result<(), Vec<u8>> {
        self._propose_deposit_fee_bps(new_fee_bps)
    }

    /// Execute the proposed deposit fee change after 24h timelock
    pub fn execute_deposit_fee_bps(&mut self) -> Result<(), Vec<u8>> {
        self._execute_deposit_fee_bps()
    }

    /// Direct setter for deposit fee in basis points (max 50 bps = 0.50%, admin only)
    pub fn set_deposit_fee_bps(&mut self, new_fee_bps: U256) -> Result<(), Vec<u8>> {
        self._set_deposit_fee_bps(new_fee_bps)
    }

    /// Set execution fee recipient address (admin only)
    pub fn set_execution_fee_recipient(&mut self, recipient: Address) -> Result<(), Vec<u8>> {
        self._set_execution_fee_recipient(recipient)
    }

    /// Get total accrued execution fees for USDC (view)
    pub fn get_accumulated_fees(&self) -> Result<U256, Vec<u8>> {
        self._get_accumulated_fees()
    }

    /// Get total accrued execution fees for a given asset (view)
    pub fn get_accumulated_fees_of(&self, asset_id: U256) -> Result<U256, Vec<u8>> {
        self._get_accumulated_fees_of(asset_id)
    }

    /// Get execution fee recipient address (view)
    pub fn get_execution_fee_recipient(&self) -> Result<Address, Vec<u8>> {
        self._get_execution_fee_recipient()
    }

    /// Claim accrued execution fees (recipient or admin only)
    pub fn claim_execution_fees(&mut self, asset_id: u8, amount: U256) -> Result<bool, Vec<u8>> {
        self._claim_execution_fees(asset_id, amount)
    }

    /// Returns the sum of unspent private note liabilities held by users for USDC (DEC-016).
    pub fn user_note_liability(&self) -> Result<U256, Vec<u8>> {
        self._user_note_liability()
    }

    /// Returns the sum of unspent private note liabilities for a specific asset_id (DEC-034B).
    pub fn user_note_liabilities(&self, asset_id: U256) -> Result<U256, Vec<u8>> {
        self._user_note_liabilities(asset_id)
    }

    /// Returns the accrued execution fee liability owed to the relayer for USDC (DEC-016).
    pub fn accrued_execution_fee_liability(&self) -> Result<U256, Vec<u8>> {
        self._accrued_execution_fee_liability()
    }

    /// Returns the accrued execution fee liability for a specific asset_id (DEC-034B).
    pub fn accrued_execution_fee_liabilities(&self, asset_id: U256) -> Result<U256, Vec<u8>> {
        self._accrued_execution_fee_liabilities(asset_id)
    }

    /// Returns the realized protocol fees retained as protocol equity for USDC (DEC-016).
    pub fn realized_protocol_fees(&self) -> Result<U256, Vec<u8>> {
        self._realized_protocol_fees()
    }

    /// Returns the realized protocol fees for a specific asset_id (DEC-034B).
    pub fn realized_protocol_fees_of(&self, asset_id: U256) -> Result<U256, Vec<u8>> {
        self._realized_protocol_fees_of(asset_id)
    }

    /// Returns total deposited principal for USDC (DEC-016).
    pub fn total_deposited_principal(&self) -> Result<U256, Vec<u8>> {
        self._total_deposited_principal()
    }

    /// Returns total deposited principal for a specific asset_id (DEC-034B).
    pub fn total_deposited_principals(&self, asset_id: U256) -> Result<U256, Vec<u8>> {
        self._total_deposited_principals(asset_id)
    }

    /// Returns the current Merkle tree root hash. If mmr_leaf_count == 0, returns the canonical empty root (DEC-032).
    pub fn note_tree_root(&self) -> Result<FixedBytes<32>, Vec<u8>> {
        self._note_tree_root()
    }

    /// Returns the current total leaf count in the Merkle Mountain Range (0 .. 2^64).
    pub fn mmr_leaf_count(&self) -> Result<U256, Vec<u8>> {
        self._mmr_leaf_count()
    }

    /// Returns the active MMR peak at a given mountain height level.
    pub fn mmr_peak(&self, height: U256) -> Result<FixedBytes<32>, Vec<u8>> {
        self._mmr_peak(height)
    }

    /// Backwards compatibility alias for next index (returns mmr_leaf_count).
    pub fn note_tree_next_index(&self) -> Result<U256, Vec<u8>> {
        self._note_tree_next_index()
    }

    /// Checks if a root hash is currently accepted: either in accepted_note_roots, current root, or canonical empty root.
    pub fn is_accepted_note_root(&self, root: FixedBytes<32>) -> Result<bool, Vec<u8>> {
        Ok(self._is_accepted_note_root(root))
    }

    /// Returns root from the bounded ring buffer history (size 100).
    pub fn get_root_history(&self, slot: U256) -> Result<FixedBytes<32>, Vec<u8>> {
        self._get_root_history(slot)
    }

    /// Returns the CCIP Router address configured for the contract.
    pub fn ccip_router(&self) -> Result<Address, Vec<u8>> {
        self._ccip_router()
    }

    /// Sets the CCIP Router address (Admin only). Router must be non-zero.
    pub fn set_ccip_router(&mut self, router: Address) -> Result<(), Vec<u8>> {
        self._set_ccip_router(router)
    }

    /// Returns whether a nullifier has already been spent on-chain (DEC-019, DEC-025, DEC-033).
    pub fn is_nullifier_spent(&self, nullifier: FixedBytes<32>) -> Result<bool, Vec<u8>> {
        self._is_nullifier_spent(nullifier)
    }

    /// Batch queries multiple nullifiers for double-spend status in a single call (DEC-037).
    pub fn are_nullifiers_spent(
        &self,
        nullifiers: Vec<FixedBytes<32>>,
    ) -> Result<Vec<bool>, Vec<u8>> {
        self._are_nullifiers_spent(nullifiers)
    }

    /// Returns whether a private note nullifier has already been spent on-chain (DEC-025 & DEC-033).
    pub fn is_note_nullifier_spent(&self, nullifier: FixedBytes<32>) -> Result<bool, Vec<u8>> {
        self._is_note_nullifier_spent(nullifier)
    }

    /// Returns whether a generational note nullifier has already been spent on-chain (DEC-033).
    pub fn is_generational_nullifier_spent(
        &self,
        epoch_id: u32,
        nullifier: FixedBytes<32>,
    ) -> Result<bool, Vec<u8>> {
        self._is_generational_nullifier_spent(epoch_id, nullifier)
    }

    pub fn set_ccip_sender_allowlist(
        &mut self,
        source_chain_selector: u64,
        sender: Bytes,
        allowed: bool,
    ) -> Result<(), Vec<u8>> {
        self._set_ccip_sender_allowlist(source_chain_selector, sender, allowed)
    }

    pub fn is_ccip_sender_allowed(
        &self,
        source_chain_selector: u64,
        sender: Bytes,
    ) -> Result<bool, Vec<u8>> {
        self._is_ccip_sender_allowed(source_chain_selector, sender)
    }

    /// Returns whether a CCIP message ID has already been executed on-chain (DEC-055 / DEC-056).
    pub fn is_ccip_message_processed(&self, message_id: FixedBytes<32>) -> Result<bool, Vec<u8>> {
        self._is_ccip_message_processed(message_id)
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
        expected_amount_gwei: U256,
        note_commitment: FixedBytes<32>,
        encrypted_memo: Bytes,
    ) -> Result<(), Vec<u8>> {
        self._deposit_direct_native_eth(expected_amount_gwei, note_commitment, encrypted_memo)
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
        self._get_current_epoch()
    }

    /// Returns the timestamp when the current epoch started (DEC-033).
    pub fn epoch_start_timestamp(&self) -> Result<u64, Vec<u8>> {
        self._epoch_start_timestamp()
    }

    /// Rotates the generational epoch window (DEC-033).
    /// Advances current_epoch by 1, updates recycle_slot = (current_epoch + 1) % 2,
    /// and logs EpochRotated.
    pub fn rotate_epoch(&mut self) -> Result<u32, Vec<u8>> {
        self._rotate_epoch()
    }

    /// ERC-165 Standard Interface Detection
    pub fn supports_interface(&self, interface_id: FixedBytes<4>) -> Result<bool, Vec<u8>> {
        Ok(self._supports_interface(interface_id))
    }

    /// Receives a cross-chain programmable deposit via Chainlink CCIP (DEC-055 / IAny2EVMMessageReceiver).
    pub fn ccip_receive(&mut self, message: CcipAny2EVMMessage) -> Result<(), Vec<u8>> {
        self._ccip_receive_programmable(message)
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

    pub fn total_assets(&mut self) -> Result<U256, Vec<u8>> {
        self._total_assets()
    }
}
