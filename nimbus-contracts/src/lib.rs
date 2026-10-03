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
pub use storage::Nimbus;
pub use types::{to_evm_g1, to_evm_g2, to_evm_scalar};

#[public]
impl Nimbus {
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
        self.stablecoin.set(stablecoin_addr);
        self.fee_recipient.set(fee_recipient_addr);
        Ok(())
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

    /// Set execution fee recipient address (admin only)
    pub fn set_execution_fee_recipient(&mut self, recipient: Address) -> Result<(), Vec<u8>> {
        self.check_owner()?;
        if recipient == Address::ZERO {
            return Err(b"INVALID_RECIPIENT".to_vec());
        }
        self.execution_fee_recipient.set(recipient);
        Ok(())
    }

    /// Get total accumulated execution fees (view)
    pub fn get_accumulated_fees(&self) -> Result<U256, Vec<u8>> {
        Ok(self.accumulated_execution_fees.get())
    }

    /// Get execution fee recipient address (view)
    pub fn get_execution_fee_recipient(&self) -> Result<Address, Vec<u8>> {
        Ok(self.execution_fee_recipient.get())
    }

    /// Claim accumulated execution fees (admin only)
    pub fn claim_execution_fees(&mut self, amount: U256) -> Result<bool, Vec<u8>> {
        self.check_owner()?;
        let recipient = self.execution_fee_recipient.get();
        if recipient == Address::ZERO {
            return Err(b"NO_RECIPIENT_SET".to_vec());
        }
        let accumulated = self.accumulated_execution_fees.get();
        if amount > accumulated {
            return Err(b"INSUFFICIENT_ACCUMULATED_FEES".to_vec());
        }
        if amount == U256::ZERO {
            return Err(b"ZERO_AMOUNT".to_vec());
        }

        // Transfer USDC to execution_fee_recipient
        let stablecoin_address = self.stablecoin.get();
        let erc20 = IErc20::new(stablecoin_address);
        let host = Self::runtime_host();
        let success = erc20
            .transfer(&host, Call::new_mutating(self), recipient, amount)
            .map_err(|_| b"TRANSFER_FAILED".to_vec())?;
        if !success {
            return Err(b"CLAIM_TRANSFER_FAILED".to_vec());
        }

        // Update accumulated and accrued fees
        self.accumulated_execution_fees
            .set(accumulated.checked_sub(amount).unwrap());
        let accrued = self.accrued_execution_fee_liability.get();
        if accrued >= amount {
            self.accrued_execution_fee_liability.set(accrued - amount);
        }

        // Enforce invariant: contract_assets >= outstanding_liabilities
        // Tolak claim on-chain yang menyentuh backing deposit user/refund (DEC-016 & Gate B)
        self.check_liability_invariant()?;

        Ok(true)
    }

    /// Returns the sum of unspent private note liabilities held by users (DEC-016).
    pub fn user_note_liability(&self) -> Result<U256, Vec<u8>> {
        Ok(self.user_note_liability.get())
    }

    /// Returns the sum of unresolved deposits that can still be refunded (DEC-016).
    pub fn refundable_deposit_liability(&self) -> Result<U256, Vec<u8>> {
        Ok(self.refundable_deposit_liability.get())
    }

    /// Returns the accrued execution fee liability owed to the relayer (DEC-016).
    pub fn accrued_execution_fee_liability(&self) -> Result<U256, Vec<u8>> {
        Ok(self.accrued_execution_fee_liability.get())
    }

    /// Returns the realized protocol fees retained as protocol equity (DEC-016).
    pub fn realized_protocol_fees(&self) -> Result<U256, Vec<u8>> {
        Ok(self.realized_protocol_fees.get())
    }

    pub fn total_deposited_principal(&self) -> Result<U256, Vec<u8>> {
        Ok(self.total_deposited_principal.get())
    }

    /// Returns the current Merkle tree root hash. If next_index == 0, returns the canonical empty root (DEC-016 Gate D).
    pub fn note_tree_root(&self) -> Result<FixedBytes<32>, Vec<u8>> {
        let next_idx = self.note_tree_next_index.get();
        if next_idx == U256::ZERO && self.note_tree_root.get() == FixedBytes::ZERO {
            Ok(FixedBytes::from(
                poseidon_w5_constants::EMPTY_TREE_ROOT_BYTES,
            ))
        } else {
            Ok(self.note_tree_root.get())
        }
    }

    /// Returns the next leaf index in the incremental Merkle tree (0 .. 2^20).
    pub fn note_tree_next_index(&self) -> Result<U256, Vec<u8>> {
        Ok(self.note_tree_next_index.get())
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

    /// Returns the note commitment bound to a deposit session.
    pub fn session_note_commitment(&self, sid: FixedBytes<32>) -> Result<FixedBytes<32>, Vec<u8>> {
        Ok(self.session_note_commitment.get(sid))
    }

    /// Returns the CCIP Router address configured for the contract.
    pub fn ccip_router(&self) -> Result<Address, Vec<u8>> {
        Ok(self.ccip_router.get())
    }

    /// Sets the CCIP Router address (Admin only).
    pub fn set_ccip_router(&mut self, router: Address) -> Result<(), Vec<u8>> {
        self.check_owner()?;
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

    pub fn register_issuer_key(&mut self, pk_iss_bytes: Bytes) -> Result<(), Vec<u8>> {
        self.check_owner()?;
        if pk_iss_bytes.len() != 256 {
            return Err(b"INVALID_PUBLIC_KEY_LENGTH".to_vec());
        }
        if pk_iss_bytes.iter().all(|byte| *byte == 0) {
            return Err(b"POINT_AT_INFINITY_NOT_ALLOWED".to_vec());
        }
        self.trusted_issuer_keys
            .insert(keccak256(&pk_iss_bytes), true);
        Ok(())
    }

    pub fn revoke_issuer_key(&mut self, pk_iss_bytes: Bytes) -> Result<(), Vec<u8>> {
        self.check_owner()?;
        if pk_iss_bytes.len() != 256 {
            return Err(b"INVALID_PUBLIC_KEY_LENGTH".to_vec());
        }
        self.trusted_issuer_keys
            .insert(keccak256(&pk_iss_bytes), false);
        Ok(())
    }

    pub fn is_issuer_key_trusted(&self, pk_iss_bytes: Bytes) -> Result<bool, Vec<u8>> {
        if pk_iss_bytes.len() != 256 {
            return Err(b"INVALID_PUBLIC_KEY_LENGTH".to_vec());
        }
        Ok(self.trusted_issuer_keys.get(keccak256(&pk_iss_bytes)))
    }

    pub fn set_ccip_sender_allowlist(
        &mut self,
        source_chain_selector: u64,
        sender: Bytes,
        allowed: bool,
    ) -> Result<(), Vec<u8>> {
        self.check_owner()?;
        let key = self.ccip_allowlist_key(source_chain_selector, &sender);
        self.ccip_allowed_senders.insert(key, allowed);
        Ok(())
    }

    pub fn is_ccip_sender_allowed(
        &self,
        source_chain_selector: u64,
        sender: Bytes,
    ) -> Result<bool, Vec<u8>> {
        let key = self.ccip_allowlist_key(source_chain_selector, &sender);
        Ok(self.ccip_allowed_senders.get(key))
    }

    // --- Liability Invariant Check ---

    /// Verifies the invariant: contract_assets >= outstanding_liabilities
    /// where outstanding_liabilities = user_note_liability + refundable_deposit_liability + accrued_execution_fee_liability (DEC-016 Gate B)
    /// This ensures the contract can always cover all user notes, unresolved refunds, and accrued relayer fees.
    fn check_liability_invariant(&self) -> Result<(), Vec<u8>> {
        #[cfg(not(test))]
        {
            use stylus_sdk::prelude::Call;

            let stablecoin_address = self.stablecoin.get();
            let contract_address = self.env_contract_address();
            let erc20 = IErc20::new(stablecoin_address);
            let host = Self::runtime_host();
            let contract_balance = erc20
                .balance_of(&host, Call::new(), contract_address)
                .map_err(|_| b"INVARIANT_BALANCE_CHECK_FAILED".to_vec())?;

            let user_liab = self.user_note_liability.get();
            let refund_liab = self.refundable_deposit_liability.get();
            let fee_liab = self.accrued_execution_fee_liability.get();
            let multi_liabilities = user_liab
                .checked_add(refund_liab)
                .and_then(|s| s.checked_add(fee_liab))
                .unwrap_or(U256::MAX);

            let legacy_principal = self.total_deposited_principal.get();
            let legacy_liabilities = legacy_principal
                .checked_add(self.accumulated_execution_fees.get())
                .unwrap_or(U256::MAX);

            let outstanding_liabilities = multi_liabilities.max(legacy_liabilities);

            if contract_balance < outstanding_liabilities {
                return Err(b"INVARIANT_VIOLATED_CONTRACT_ASSETS_LESS_THAN_LIABILITIES".to_vec());
            }
        }

        Ok(())
    }

    // --- EVM Public Delegates to Modular Component Implementations ---

    pub fn deposit(
        &mut self,
        sid: FixedBytes<32>,
        com_k_bytes: Bytes,
        amount: U256,
    ) -> Result<(), Vec<u8>> {
        self._deposit(sid, com_k_bytes, amount)
    }

    /// Deposit funds with an initial private note commitment bound to the session (DEC-016 Gate D).
    pub fn deposit_with_commitment(
        &mut self,
        sid: FixedBytes<32>,
        com_k_bytes: Bytes,
        amount: U256,
        note_commitment: FixedBytes<32>,
    ) -> Result<(), Vec<u8>> {
        self._deposit_with_commitment(sid, com_k_bytes, amount, note_commitment)
    }

    pub fn reveal_mask_key(
        &mut self,
        sid: FixedBytes<32>,
        k_bytes: Bytes,
        pk_iss_bytes: Bytes,
        com_k_bytes: Bytes,
    ) -> Result<bool, Vec<u8>> {
        self._reveal_mask_key(sid, k_bytes, pk_iss_bytes, com_k_bytes)
    }

    pub fn claim_refund(&mut self, sid: FixedBytes<32>) -> Result<(), Vec<u8>> {
        self._claim_refund(sid)
    }

    pub fn spend(
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
        self._spend(
            root,
            nullifier,
            alpha_neg_bytes,
            pk_iss_bytes,
            recipient,
            amount,
            recipient_or_intent_hash,
            expiry,
            nonce,
            max_execution_fee,
            execution_fee,
        )
    }

    pub fn batch_spend(
        &mut self,
        roots: Vec<FixedBytes<32>>,
        nullifiers: Vec<FixedBytes<32>>,
        alpha_neg_items: Vec<Bytes>,
        pk_iss_items: Vec<Bytes>,
        recipients: Vec<Address>,
        amounts: Vec<U256>,
        recipient_or_intent_hashes: Vec<FixedBytes<32>>,
        expiries: Vec<U256>,
        nonces: Vec<FixedBytes<32>>,
    ) -> Result<bool, Vec<u8>> {
        const MAX_BATCH_SIZE: usize = 8;
        let len = nullifiers.len();
        if len == 0 {
            return Err(b"EMPTY_BATCH".to_vec());
        }
        if len > MAX_BATCH_SIZE {
            return Err(b"BATCH_TOO_LARGE".to_vec());
        }
        if roots.len() != len
            || alpha_neg_items.len() != len
            || pk_iss_items.len() != len
            || recipients.len() != len
            || amounts.len() != len
            || recipient_or_intent_hashes.len() != len
            || expiries.len() != len
            || nonces.len() != len
        {
            return Err(b"BATCH_LENGTH_MISMATCH".to_vec());
        }

        // 1. CHECKS & PREPARATION
        self.check_not_paused()?;

        let current_time = U256::from(self.block_timestamp());
        let chain_id = U256::from(self.env_chain_id());
        let contract_address = self.env_contract_address();

        let mut hm_items = Vec::with_capacity(len);

        for i in 0..len {
            // Expiry check
            if expiries[i] != U256::ZERO && current_time > expiries[i] {
                return Err(b"TRANSACTION_EXPIRED".to_vec());
            }

            // Enforce minimum transaction size of 5 USDC
            let min_amount = U256::from(5_000_000);
            if amounts[i] < min_amount {
                return Err(b"AMOUNT_TOO_SMALL".to_vec());
            }

            // Recipient or intent mismatch check
            if recipients[i] != Address::ZERO
                && recipient_or_intent_hashes[i] != crate::spend::recipient_hash(recipients[i])
            {
                return Err(b"RECIPIENT_INTENT_MISMATCH".to_vec());
            }

            // Check double spend (Nullifier)
            if self.nullifiers.get(nullifiers[i]) {
                return Err(b"BATCH_ITEM_INVALID".to_vec());
            }

            // Reconstruct message hash and G1 curve point H(m) on-chain
            let m_hash = crate::helpers::compute_spend_hash(
                chain_id,
                contract_address,
                amounts[i],
                recipient_or_intent_hashes[i],
                expiries[i],
                nonces[i],
            );
            let hm_affine = crate::helpers::hash_to_g1(&m_hash);
            let hm_evm_bytes = crate::types::to_evm_g1(&hm_affine);
            let hm_bytes = Bytes::from(hm_evm_bytes.to_vec());

            if nullifiers[i] != keccak256(&hm_bytes) {
                return Err(b"NULLIFIER_MESSAGE_MISMATCH".to_vec());
            }

            hm_items.push(hm_bytes);
        }

        // 2. BATCH SIGNATURE VERIFICATION
        if !self.verify_bls_spend_batch(&alpha_neg_items, &hm_items, &pk_iss_items)? {
            return Err(b"BATCH_ITEM_INVALID".to_vec());
        }

        // 3. EXECUTE EFFECTS AND INTERACTIONS
        for i in 0..len {
            let mut fee_bps = U256::from(25); // 0.25%

            if roots[i] != FixedBytes::ZERO {
                let root_timestamp = self.clean_association_roots.get(roots[i]);
                if root_timestamp == U256::ZERO {
                    return Err(b"INVALID_ASSOCIATION_ROOT".to_vec());
                }
                let delta_t = current_time
                    .checked_sub(root_timestamp)
                    .unwrap_or(U256::ZERO);
                let seven_days = U256::from(7 * 24 * 60 * 60);
                let thirty_days = U256::from(30 * 24 * 60 * 60);

                if delta_t >= thirty_days {
                    fee_bps = U256::from(10); // 0.10% (1 month hold)
                } else if delta_t >= seven_days {
                    fee_bps = U256::from(20); // 0.20% (7 days hold)
                }
            }

            let spend_fee = (amounts[i]
                .checked_mul(fee_bps)
                .ok_or_else(|| b"SPEND_FEE_MUL_OVERFLOW".to_vec())?
                + U256::from(9999))
                / U256::from(10000);

            let protocol_share = spend_fee;
            let payout = amounts[i];
            let total_debit = payout
                .checked_add(protocol_share)
                .ok_or_else(|| b"TOTAL_DEBIT_OVERFLOW".to_vec())?;

            let principal = self.total_deposited_principal.get();
            let new_principal = principal
                .checked_sub(total_debit)
                .ok_or_else(|| b"INSUFFICIENT_PRINCIPAL".to_vec())?;

            // State changes
            self.nullifiers.insert(nullifiers[i], true);
            self.total_deposited_principal.set(new_principal);

            // Multi-liability tracking (DEC-016 Gate B)
            let user_liab = self.user_note_liability.get();
            if user_liab >= total_debit {
                self.user_note_liability.set(user_liab - total_debit);
            }
            let current_realized = self.realized_protocol_fees.get();
            if let Some(new_realized) = current_realized.checked_add(protocol_share) {
                self.realized_protocol_fees.set(new_realized);
            }

            #[cfg(not(test))]
            {
                self.ensure_liquidity(total_debit)?;

                let stablecoin_address = self.stablecoin.get();
                let erc20 = IErc20::new(stablecoin_address);
                let host = Self::runtime_host();

                // Transfer payout to recipient (if not zero address)
                if recipients[i] != Address::ZERO && payout > U256::ZERO {
                    let success =
                        erc20.transfer(&host, Call::new_mutating(self), recipients[i], payout)?;
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
            }
        }

        // Final invariant enforcement
        self.check_liability_invariant()?;

        Ok(true)
    }

    pub fn spend_and_buy_shares(
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
        self._spend_and_buy_shares(
            root,
            nullifier,
            alpha_neg_bytes,
            pk_iss_bytes,
            polymarket_ctf,
            collateral_token,
            condition_id,
            amount,
            expiry,
            nonce,
            max_execution_fee,
            execution_fee,
        )
    }

    pub fn get_failed_intent_refund(&self, nullifier: FixedBytes<32>) -> Result<U256, Vec<u8>> {
        self._get_failed_intent_refund(nullifier)
    }

    pub fn claim_failed_intent_refund(
        &mut self,
        nullifier: FixedBytes<32>,
        recipient: Address,
    ) -> Result<bool, Vec<u8>> {
        self._claim_failed_intent_refund(nullifier, recipient)
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
mod tests {
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
        pub(crate) static MSG_SENDER: RefCell<Address> = RefCell::new(Address::ZERO);
        pub(crate) static BLOCK_TIMESTAMP: RefCell<u64> = RefCell::new(0);
        static PAIRING_RESULT: RefCell<u8> = RefCell::new(1);
    }

    fn reset_test_state() {
        STORAGE.with(|s| s.borrow_mut().clear());
        MSG_SENDER.with(|s| *s.borrow_mut() = Address::ZERO);
        BLOCK_TIMESTAMP.with(|t| *t.borrow_mut() = 0);
        PAIRING_RESULT.with(|result| *result.borrow_mut() = 1);
    }

    fn set_msg_sender(sender: Address) {
        MSG_SENDER.with(|s| *s.borrow_mut() = sender);
    }

    fn set_block_timestamp(ts: u64) {
        BLOCK_TIMESTAMP.with(|t| *t.borrow_mut() = ts);
    }

    fn set_pairing_result(result: bool) {
        PAIRING_RESULT.with(|value| *value.borrow_mut() = u8::from(result));
    }

    fn register_mock_issuer_with_nonce(
        contract: &mut Nimbus,
        owner: Address,
        amount: U256,
        recipient_or_intent_hash: FixedBytes<32>,
        nonce: FixedBytes<32>,
    ) -> (Vec<u8>, Vec<u8>, Vec<u8>, FixedBytes<32>) {
        let alpha_neg = vec![0x11; 128];
        let pk_iss = vec![0x33; 256];
        set_msg_sender(owner);
        contract.register_issuer_key(pk_iss.clone().into()).unwrap();

        let chain_id = U256::from(1337);
        let contract_address = Address::ZERO;
        let expiry = U256::ZERO;

        let m_hash = helpers::compute_spend_hash(
            chain_id,
            contract_address,
            amount,
            recipient_or_intent_hash,
            expiry,
            nonce,
        );
        let hm_affine = helpers::hash_to_g1(&m_hash);
        let hm_evm_bytes = types::to_evm_g1(&hm_affine);
        let nullifier = keccak256(&hm_evm_bytes);

        (alpha_neg, hm_evm_bytes.to_vec(), pk_iss, nullifier)
    }

    fn register_mock_issuer(
        contract: &mut Nimbus,
        owner: Address,
        amount: U256,
        recipient_or_intent_hash: FixedBytes<32>,
    ) -> (Vec<u8>, Vec<u8>, Vec<u8>, FixedBytes<32>) {
        register_mock_issuer_with_nonce(
            contract,
            owner,
            amount,
            recipient_or_intent_hash,
            FixedBytes::ZERO,
        )
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
        if !return_data_len.is_null() {
            *return_data_len = 32;
        }
        0 // Status code 0 is Success
    }

    #[no_mangle]
    pub unsafe extern "C" fn storage_flush_cache() {}

    #[no_mangle]
    pub unsafe extern "C" fn return_data_size() -> usize {
        32
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
        let nimbus_contract = Nimbus::default();
        let result = nimbus_contract.verify_groth16_proof(
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
        let nimbus_contract = Nimbus::default();
        // Since we stubbed static_call_contract, return_data_size and read_return_data to return success,
        // this test should successfully verify the Groth16 proof with valid lengths!
        let is_valid = nimbus_contract
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
        set_msg_sender(owner);

        let mut nimbus_contract = Nimbus::default();
        nimbus_contract
            .init(owner, Address::ZERO, Address::ZERO)
            .unwrap();

        // Configure CCIP router and allowlist sender
        nimbus_contract.set_ccip_router(router).unwrap();
        nimbus_contract
            .set_ccip_sender_allowlist(1, vec![].into(), true)
            .unwrap();

        nimbus_contract
            .deposit(
                FixedBytes::repeat_byte(0x66),
                vec![0x42; 256].into(),
                U256::from(20_000_000),
            )
            .unwrap();
        let amount = U256::from(10_000_000);
        let (alpha_neg, hm_evm_bytes, pk_iss, nullifier) =
            register_mock_issuer(&mut nimbus_contract, owner, amount, FixedBytes::ZERO);

        let mut payload = vec![0u8; 584];
        payload[0..32].copy_from_slice(nullifier.as_slice());
        payload[32..160].copy_from_slice(&alpha_neg);
        payload[160..416].copy_from_slice(&pk_iss);
        // payload[416..436] is polymarket_ctf (zeros)
        // payload[436..456] is collateral_token (zeros)
        // payload[456..488] is condition_id (zeros)
        payload[488..520].copy_from_slice(&amount.to_be_bytes::<32>());
        // payload[520..552] is expiry (zeros)
        // payload[552..584] is nonce (zeros)

        // Set message sender to CCIP Router before execution
        set_msg_sender(router);

        let message_id = FixedBytes::repeat_byte(0x99);
        let result = nimbus_contract.ccip_receive(message_id, 1, vec![].into(), payload.into());
        match result {
            Ok(_) => {}
            Err(e) => {
                panic!(
                    "ccip_receive returned error: {:?}",
                    String::from_utf8_lossy(&e)
                );
            }
        }

        // Test Replay protection
        let replay_result =
            nimbus_contract.ccip_receive(message_id, 1, vec![].into(), vec![0; 584].into());
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
        set_msg_sender(owner);

        let mut nimbus_contract = Nimbus::default();
        nimbus_contract
            .init(owner, Address::ZERO, Address::ZERO)
            .unwrap();

        let payload = vec![0u8; 584].into();
        let message_id = FixedBytes::repeat_byte(0x88);

        // Case 1: CCIP router not configured (is ZERO)
        assert_eq!(
            nimbus_contract.ccip_receive(message_id, 1, vec![].into(), payload),
            Err(b"CCIP_ROUTER_NOT_CONFIGURED".to_vec())
        );

        // Configure router
        nimbus_contract.set_ccip_router(router).unwrap();

        // Case 2: Caller is not the router (caller is owner)
        let payload = vec![0u8; 584].into();
        assert_eq!(
            nimbus_contract.ccip_receive(message_id, 1, vec![].into(), payload),
            Err(b"ONLY_CCIP_ROUTER_ALLOWED".to_vec())
        );

        // Set caller to router
        set_msg_sender(router);

        // Case 3: Sender is not allowlisted
        let payload = vec![0u8; 584].into();
        assert_eq!(
            nimbus_contract.ccip_receive(message_id, 1, vec![].into(), payload),
            Err(b"CCIP_SENDER_NOT_ALLOWED".to_vec())
        );
    }

    #[test]
    fn test_initialization() {
        reset_test_state();
        let owner = address!("1111111111111111111111111111111111111111");
        set_msg_sender(owner);

        let mut contract = Nimbus::default();
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
        let mut contract = Nimbus::default();
        contract.init(owner, Address::ZERO, Address::ZERO).unwrap();

        // Non-owner pausing should fail
        set_msg_sender(non_owner);
        assert_eq!(contract.pause(), Err(b"NOT_OWNER".to_vec()));

        // Owner pausing should succeed
        set_msg_sender(owner);
        assert!(contract.pause().is_ok());
        assert_eq!(contract.paused.get(), true);

        // Non-owner unpausing should fail
        set_msg_sender(non_owner);
        assert_eq!(contract.unpause(), Err(b"NOT_OWNER".to_vec()));

        // Owner unpausing should succeed
        set_msg_sender(owner);
        assert!(contract.unpause().is_ok());
        assert_eq!(contract.paused.get(), false);
    }

    #[test]
    fn test_guarded_functions_fail_when_paused() {
        reset_test_state();
        let owner = address!("1111111111111111111111111111111111111111");
        set_msg_sender(owner);

        let mut contract = Nimbus::default();
        contract.init(owner, Address::ZERO, Address::ZERO).unwrap();
        contract.pause().unwrap();

        // Try guarded operations
        let sid = FixedBytes::ZERO;
        assert_eq!(
            contract.deposit(sid, vec![].into(), U256::from(10_000_000)),
            Err(b"CONTRACT_PAUSED".to_vec())
        );
        assert_eq!(
            contract.reveal_mask_key(sid, vec![].into(), vec![].into(), vec![].into()),
            Err(b"CONTRACT_PAUSED".to_vec())
        );
        assert_eq!(
            contract.spend(
                FixedBytes::ZERO,
                sid,
                vec![].into(),
                vec![].into(),
                Address::ZERO,
                U256::ZERO,
                FixedBytes::ZERO,
                U256::ZERO,
                FixedBytes::ZERO,
                U256::ZERO,
                U256::ZERO,
            ),
            Err(b"CONTRACT_PAUSED".to_vec())
        );
        assert_eq!(
            contract.spend_and_buy_shares(
                FixedBytes::ZERO,
                sid,
                vec![].into(),
                vec![].into(),
                Address::ZERO,
                Address::ZERO,
                FixedBytes::ZERO,
                U256::ZERO,
                U256::ZERO,
                FixedBytes::ZERO,
                U256::ZERO,
                U256::ZERO,
            ),
            Err(b"CONTRACT_PAUSED".to_vec())
        );

        assert_eq!(
            contract.register_clean_root(FixedBytes::ZERO),
            Err(b"CONTRACT_PAUSED".to_vec())
        );
        assert_eq!(contract.claim_refund(sid), Err(b"CONTRACT_PAUSED".to_vec()));
    }

    #[test]
    fn test_claim_refund_timelock() {
        reset_test_state();
        let owner = address!("1111111111111111111111111111111111111111");
        let client = address!("2222222222222222222222222222222222222222");
        let other = address!("3333333333333333333333333333333333333333");

        set_msg_sender(owner);
        let mut contract = Nimbus::default();
        contract.init(owner, Address::ZERO, Address::ZERO).unwrap();

        let sid = FixedBytes::repeat_byte(0xab);

        // Deposit (must be >= 10_000_000)
        set_msg_sender(client);
        set_block_timestamp(1000);
        contract
            .deposit(sid, vec![0x42; 256].into(), U256::from(10_000_000))
            .unwrap();

        // Claim refund from other address should fail
        set_msg_sender(other);
        assert_eq!(
            contract.claim_refund(sid),
            Err(b"NOT_SESSION_CLIENT".to_vec())
        );

        // Claim refund from client before timelock (24 hours = 86400 secs)
        set_msg_sender(client);
        set_block_timestamp(1000 + 86399); // 1 second before expiry
        assert_eq!(
            contract.claim_refund(sid),
            Err(b"TIMELOCK_NOT_EXPIRED".to_vec())
        );

        // Claim refund at expiry should succeed
        set_block_timestamp(1000 + 86400);
        assert!(contract.claim_refund(sid).is_ok());

        // Claiming again should fail
        assert_eq!(
            contract.claim_refund(sid),
            Err(b"SESSION_ALREADY_RESOLVED".to_vec())
        );
    }

    #[test]
    fn test_deposit_binds_commitment_and_rejects_duplicate_session() {
        reset_test_state();
        let owner = address!("1111111111111111111111111111111111111111");
        let client = address!("2222222222222222222222222222222222222222");

        set_msg_sender(owner);
        let mut contract = Nimbus::default();
        contract.init(owner, Address::ZERO, Address::ZERO).unwrap();

        let sid = FixedBytes::repeat_byte(0xac);
        let commitment = vec![0x42; 256];
        set_msg_sender(client);

        assert_eq!(
            contract.deposit(sid, vec![0x42; 255].into(), U256::from(10_000_000)),
            Err(b"INVALID_COMMITMENT_LENGTH".to_vec())
        );
        contract
            .deposit(sid, commitment.clone().into(), U256::from(10_000_000))
            .unwrap();
        assert_eq!(
            contract.deposit(sid, commitment.into(), U256::from(10_000_000)),
            Err(b"SESSION_ALREADY_EXISTS".to_vec())
        );
    }

    #[test]
    fn test_reveal_cannot_release_collateral_or_change_commitment() {
        reset_test_state();
        let owner = address!("1111111111111111111111111111111111111111");
        let client = address!("2222222222222222222222222222222222222222");
        let guardian = address!("3333333333333333333333333333333333333333");

        set_msg_sender(owner);
        let mut contract = Nimbus::default();
        contract.init(owner, Address::ZERO, Address::ZERO).unwrap();

        let sid = FixedBytes::repeat_byte(0xad);
        let commitment = vec![0x42; 256];
        set_msg_sender(client);
        contract
            .deposit(sid, commitment.clone().into(), U256::from(10_000_000))
            .unwrap();
        let principal = contract.total_deposited_principal().unwrap();

        set_msg_sender(guardian);
        assert_eq!(
            contract.reveal_mask_key(
                sid,
                vec![0x11; 32].into(),
                vec![0x22; 256].into(),
                vec![0x43; 256].into(),
            ),
            Err(b"COMMITMENT_MISMATCH".to_vec())
        );
        assert_eq!(contract.total_deposited_principal().unwrap(), principal);
        assert!(!contract.session_resolved.get(sid));

        assert!(contract
            .reveal_mask_key(
                sid,
                vec![0x11; 32].into(),
                vec![0x22; 256].into(),
                commitment.into(),
            )
            .unwrap());
        assert!(contract.session_resolved.get(sid));
        assert_eq!(contract.total_deposited_principal().unwrap(), principal);

        set_msg_sender(client);
        assert_eq!(
            contract.claim_refund(sid),
            Err(b"SESSION_ALREADY_RESOLVED".to_vec())
        );
    }

    #[test]
    fn test_register_clean_root_authorization() {
        reset_test_state();
        let owner = address!("1111111111111111111111111111111111111111");
        let non_owner = address!("2222222222222222222222222222222222222222");

        set_msg_sender(owner);
        let mut contract = Nimbus::default();
        contract.init(owner, Address::ZERO, Address::ZERO).unwrap();

        // Non-owner should not be able to register clean root
        set_msg_sender(non_owner);
        assert_eq!(
            contract.register_clean_root(FixedBytes::ZERO),
            Err(b"NOT_OWNER".to_vec())
        );

        // Owner should be able to register clean root
        set_msg_sender(owner);
        assert!(contract.register_clean_root(FixedBytes::ZERO).is_ok());
    }

    #[test]
    fn test_issuer_key_registration_authorization_and_revocation() {
        reset_test_state();
        let owner = address!("1111111111111111111111111111111111111111");
        let other = address!("2222222222222222222222222222222222222222");
        let pk_iss = vec![0x33; 256];

        set_msg_sender(owner);
        let mut contract = Nimbus::default();
        contract.init(owner, Address::ZERO, Address::ZERO).unwrap();

        set_msg_sender(other);
        assert_eq!(
            contract.register_issuer_key(pk_iss.clone().into()),
            Err(b"NOT_OWNER".to_vec())
        );

        set_msg_sender(owner);
        assert_eq!(
            contract.register_issuer_key(vec![0x33; 255].into()),
            Err(b"INVALID_PUBLIC_KEY_LENGTH".to_vec())
        );
        assert_eq!(
            contract.register_issuer_key(vec![0; 256].into()),
            Err(b"POINT_AT_INFINITY_NOT_ALLOWED".to_vec())
        );
        contract.register_issuer_key(pk_iss.clone().into()).unwrap();
        assert!(contract
            .is_issuer_key_trusted(pk_iss.clone().into())
            .unwrap());
        contract.revoke_issuer_key(pk_iss.clone().into()).unwrap();
        assert!(!contract.is_issuer_key_trusted(pk_iss.into()).unwrap());
    }

    #[test]
    fn test_spend_rejects_untrusted_key_and_message_replay() {
        reset_test_state();
        let owner = address!("1111111111111111111111111111111111111111");
        set_msg_sender(owner);
        let mut contract = Nimbus::default();
        contract.init(owner, Address::ZERO, Address::ZERO).unwrap();

        let alpha_neg = vec![0x11; 128];
        let pk_iss = vec![0x33; 256];

        let amount = U256::from(5_000_000);
        let recipient_or_intent_hash = FixedBytes::ZERO;
        let expiry = U256::ZERO;
        let nonce = FixedBytes::ZERO;

        let chain_id = U256::from(1337);
        let contract_address = Address::ZERO;

        let m_hash = helpers::compute_spend_hash(
            chain_id,
            contract_address,
            amount,
            recipient_or_intent_hash,
            expiry,
            nonce,
        );
        let hm_affine = helpers::hash_to_g1(&m_hash);
        let hm_evm_bytes = types::to_evm_g1(&hm_affine);
        let nullifier = keccak256(&hm_evm_bytes);

        assert_eq!(
            contract.spend(
                FixedBytes::ZERO,
                nullifier,
                alpha_neg.clone().into(),
                pk_iss.clone().into(),
                Address::ZERO,
                amount,
                recipient_or_intent_hash,
                expiry,
                nonce,
                U256::ZERO,
                U256::ZERO,
            ),
            Err(b"UNTRUSTED_ISSUER_KEY".to_vec())
        );

        contract.register_issuer_key(pk_iss.clone().into()).unwrap();
        assert_eq!(
            contract.spend(
                FixedBytes::ZERO,
                FixedBytes::repeat_byte(0x99),
                alpha_neg.into(),
                pk_iss.into(),
                Address::ZERO,
                amount,
                recipient_or_intent_hash,
                expiry,
                nonce,
                U256::ZERO,
                U256::ZERO,
            ),
            Err(b"NULLIFIER_MESSAGE_MISMATCH".to_vec())
        );
        assert!(!contract.nullifiers.get(nullifier));
    }

    #[test]
    fn test_spend_rejects_malformed_infinity_and_false_pairing() {
        reset_test_state();
        let owner = address!("1111111111111111111111111111111111111111");
        set_msg_sender(owner);
        let mut contract = Nimbus::default();
        contract.init(owner, Address::ZERO, Address::ZERO).unwrap();
        let (alpha_neg, hm, pk_iss, nullifier) = register_mock_issuer(
            &mut contract,
            owner,
            U256::from(5_000_000),
            FixedBytes::ZERO,
        );

        assert_eq!(
            contract.spend(
                FixedBytes::ZERO,
                nullifier,
                vec![0x11; 127].into(),
                pk_iss.clone().into(),
                Address::ZERO,
                U256::from(5_000_000),
                FixedBytes::ZERO,
                U256::ZERO,
                FixedBytes::ZERO,
                U256::ZERO,
                U256::ZERO,
            ),
            Err(b"INVALID_G1_INPUT_LENGTH".to_vec())
        );

        let infinity_alpha = vec![0; 128];
        assert_eq!(
            contract.spend(
                FixedBytes::ZERO,
                nullifier,
                infinity_alpha.into(),
                pk_iss.clone().into(),
                Address::ZERO,
                U256::from(5_000_000),
                FixedBytes::ZERO,
                U256::ZERO,
                FixedBytes::ZERO,
                U256::ZERO,
                U256::ZERO,
            ),
            Err(b"POINT_AT_INFINITY_NOT_ALLOWED".to_vec())
        );

        set_pairing_result(false);
        assert!(!contract
            .spend(
                FixedBytes::ZERO,
                nullifier,
                alpha_neg.into(),
                pk_iss.into(),
                Address::ZERO,
                U256::from(5_000_000),
                FixedBytes::ZERO,
                U256::ZERO,
                FixedBytes::ZERO,
                U256::ZERO,
                U256::ZERO,
            )
            .unwrap());
        assert!(!contract.nullifiers.get(nullifier));
        assert_eq!(contract.total_deposited_principal().unwrap(), U256::ZERO);
    }

    #[test]
    fn test_spend_rejects_recipient_substitution() {
        reset_test_state();
        let owner = address!("1111111111111111111111111111111111111111");
        let intended_recipient = address!("2222222222222222222222222222222222222222");
        let substituted_recipient = address!("3333333333333333333333333333333333333333");
        set_msg_sender(owner);
        let mut contract = Nimbus::default();
        contract.init(owner, Address::ZERO, Address::ZERO).unwrap();

        let mut recipient_hash = [0u8; 32];
        recipient_hash[12..].copy_from_slice(intended_recipient.as_slice());
        let recipient_hash = FixedBytes::from(recipient_hash);
        let (alpha_neg, _hm, pk_iss, nullifier) =
            register_mock_issuer(&mut contract, owner, U256::from(5_000_000), recipient_hash);

        assert_eq!(
            contract.spend(
                FixedBytes::ZERO,
                nullifier,
                alpha_neg.into(),
                pk_iss.into(),
                substituted_recipient,
                U256::from(5_000_000),
                recipient_hash,
                U256::ZERO,
                FixedBytes::ZERO,
                U256::ZERO,
                U256::ZERO,
            ),
            Err(b"RECIPIENT_INTENT_MISMATCH".to_vec())
        );
        assert!(!contract.nullifiers.get(nullifier));
    }

    #[test]
    fn test_spend_replay_and_insufficient_principal_preserve_state() {
        reset_test_state();
        let owner = address!("1111111111111111111111111111111111111111");
        set_msg_sender(owner);
        let mut contract = Nimbus::default();
        contract.init(owner, Address::ZERO, Address::ZERO).unwrap();
        let (alpha_neg, hm, pk_iss, nullifier) = register_mock_issuer(
            &mut contract,
            owner,
            U256::from(5_000_000),
            FixedBytes::ZERO,
        );

        assert_eq!(
            contract.spend(
                FixedBytes::ZERO,
                nullifier,
                alpha_neg.clone().into(),
                pk_iss.clone().into(),
                Address::ZERO,
                U256::from(5_000_000),
                FixedBytes::ZERO,
                U256::ZERO,
                FixedBytes::ZERO,
                U256::ZERO,
                U256::ZERO,
            ),
            Err(b"INSUFFICIENT_PRINCIPAL".to_vec())
        );
        assert!(!contract.nullifiers.get(nullifier));

        contract
            .deposit(
                FixedBytes::repeat_byte(0x77),
                vec![0x42; 256].into(),
                U256::from(10_000_000),
            )
            .unwrap();
        assert!(contract
            .spend(
                FixedBytes::ZERO,
                nullifier,
                alpha_neg.clone().into(),
                pk_iss.clone().into(),
                Address::ZERO,
                U256::from(5_000_000),
                FixedBytes::ZERO,
                U256::ZERO,
                FixedBytes::ZERO,
                U256::ZERO,
                U256::ZERO,
            )
            .unwrap());
        let principal = contract.total_deposited_principal().unwrap();
        assert!(!contract
            .spend(
                FixedBytes::ZERO,
                nullifier,
                alpha_neg.into(),
                pk_iss.into(),
                Address::ZERO,
                U256::from(5_000_000),
                FixedBytes::ZERO,
                U256::ZERO,
                FixedBytes::ZERO,
                U256::ZERO,
                U256::ZERO,
            )
            .unwrap());
        assert_eq!(contract.total_deposited_principal().unwrap(), principal);
    }

    #[test]
    fn test_liquid_vault_deposit_spend_yield() {
        reset_test_state();
        let owner = address!("1111111111111111111111111111111111111111");
        set_msg_sender(owner);

        let mut contract = Nimbus::default();
        contract.init(owner, Address::ZERO, Address::ZERO).unwrap();

        assert_eq!(contract.total_deposited_principal().unwrap(), U256::ZERO);

        // Test deposit increases principal (must be >= 10_000_000)
        let sid = FixedBytes::repeat_byte(0xde);
        contract
            .deposit(sid, vec![0x42; 256].into(), U256::from(20_000_000))
            .unwrap();

        // fee = (20,000,000 * 20 + 9999) / 10000 = 40,000
        // net_amount = 20,000,000 - 40,000 = 19_960_000 net
        assert_eq!(
            contract.total_deposited_principal().unwrap(),
            U256::from(19_960_000)
        );

        // Test spend decreases principal
        let (alpha_neg, hm, pk_iss, nullifier) = register_mock_issuer(
            &mut contract,
            owner,
            U256::from(10_000_000),
            FixedBytes::ZERO,
        );
        let is_valid = contract
            .spend(
                FixedBytes::ZERO,
                nullifier,
                alpha_neg.into(),
                pk_iss.into(),
                Address::ZERO,
                U256::from(10_000_000),
                FixedBytes::ZERO,
                U256::ZERO,
                FixedBytes::ZERO,
                U256::ZERO,
                U256::ZERO,
            )
            .unwrap();

        assert!(is_valid);
        // spend fee = (10,000,000 * 25 + 9999) / 10000 = 25,000
        // total debit = 10,025_000
        // new principal = 19_960_000 - 10,025_000 = 9_935_000
        assert_eq!(
            contract.total_deposited_principal().unwrap(),
            U256::from(9_935_000)
        );

        // Test yield claim under test (where total assets = principal, so yield is 0)
        assert_eq!(contract.claim_accumulated_yield().unwrap(), U256::ZERO);
    }

    #[test]
    fn test_polymarket_fallback_refund() {
        reset_test_state();
        let owner = address!("1111111111111111111111111111111111111111");
        set_msg_sender(owner);

        let mut contract = Nimbus::default();
        contract.init(owner, Address::ZERO, Address::ZERO).unwrap();

        // Set principal
        contract
            .deposit(
                FixedBytes::repeat_byte(0x99),
                vec![0x42; 256].into(),
                U256::from(20_000_000),
            )
            .unwrap();
        let (alpha_neg, hm, pk_iss, nullifier) = register_mock_issuer(
            &mut contract,
            owner,
            U256::from(10_000_000),
            FixedBytes::ZERO,
        );

        // Call spend_and_buy_shares with polymarket_ctf = Address::ZERO (which triggers mock fallback in tests)
        let success = contract
            .spend_and_buy_shares(
                FixedBytes::ZERO,
                nullifier,
                alpha_neg.into(),
                pk_iss.into(),
                Address::ZERO, // triggers fallback simulation in test block
                Address::ZERO,
                FixedBytes::ZERO,
                U256::from(10_000_000),
                U256::ZERO,
                FixedBytes::ZERO,
                U256::ZERO,
                U256::ZERO,
            )
            .unwrap();

        // Under our mock try-catch, it should return true (gracefully handled)
        assert!(success);

        // The refund records the exact payout; protocol fees are debited separately.
        let expected_payout = U256::from(10_000_000);
        assert_eq!(
            contract.get_failed_intent_refund(nullifier).unwrap(),
            expected_payout
        );

        // Claim the refund to a recipient
        let recipient = address!("4444444444444444444444444444444444444444");
        let claim_ok = contract
            .claim_failed_intent_refund(nullifier, recipient)
            .unwrap();
        assert!(claim_ok);

        // The refund amount should now be cleared (zero)
        assert_eq!(
            contract.get_failed_intent_refund(nullifier).unwrap(),
            U256::ZERO
        );

        // Claiming again should fail
        assert_eq!(
            contract.claim_failed_intent_refund(nullifier, recipient),
            Err(b"NO_REFUND_AVAILABLE".to_vec())
        );
    }

    #[test]
    fn test_verify_compliance_flow() {
        reset_test_state();
        let owner = address!("1111111111111111111111111111111111111111");
        let mut contract = Nimbus::default();
        contract.init(owner, Address::ZERO, Address::ZERO).unwrap();

        let root = FixedBytes::repeat_byte(0x11);
        let nullifier = FixedBytes::repeat_byte(0x22);
        let recipient = address!("3333333333333333333333333333333333333333");
        let amount = U256::from(10_000_000);

        // When root is not registered, verify_compliance should return Ok(false)
        let is_valid_unregistered = contract
            .verify_compliance(
                root,
                nullifier,
                recipient,
                amount,
                vec![0; 128].into(),
                vec![0; 256].into(),
                vec![0; 128].into(),
            )
            .unwrap();
        assert!(!is_valid_unregistered);

        // Register clean root
        contract.clean_association_roots.insert(root, U256::from(1));

        // When root is registered, it calls get_compliance_vk, compute_public_inputs_g1, and verify_groth16_proof.
        // Under #[cfg(test)], these are mocked to succeed, so it should return Ok(true).
        let is_valid_registered = contract
            .verify_compliance(
                root,
                nullifier,
                recipient,
                amount,
                vec![0; 128].into(),
                vec![0; 256].into(),
                vec![0; 128].into(),
            )
            .unwrap();
        assert!(is_valid_registered);
    }

    #[test]
    fn test_batch_spend_rejects_invalid_sizes() {
        let mut contract = Nimbus::default();
        assert_eq!(
            contract.batch_spend(
                vec![],
                vec![],
                vec![],
                vec![],
                vec![],
                vec![],
                vec![],
                vec![],
                vec![],
            ),
            Err(b"EMPTY_BATCH".to_vec())
        );

        let empty = || Bytes::from(Vec::<u8>::new());
        assert_eq!(
            contract.batch_spend(
                vec![FixedBytes::ZERO; 9],
                vec![FixedBytes::ZERO; 9],
                (0..9).map(|_| empty()).collect(),
                (0..9).map(|_| empty()).collect(),
                vec![Address::ZERO; 9],
                vec![U256::ZERO; 9],
                vec![FixedBytes::ZERO; 9],
                vec![U256::ZERO; 9],
                vec![FixedBytes::ZERO; 9],
            ),
            Err(b"BATCH_TOO_LARGE".to_vec())
        );

        assert_eq!(
            contract.batch_spend(
                vec![FixedBytes::ZERO; 2],
                vec![FixedBytes::ZERO; 1],
                vec![empty(); 2],
                vec![empty(); 2],
                vec![Address::ZERO; 2],
                vec![U256::ZERO; 2],
                vec![FixedBytes::ZERO; 2],
                vec![U256::ZERO; 2],
                vec![FixedBytes::ZERO; 2],
            ),
            Err(b"BATCH_LENGTH_MISMATCH".to_vec())
        );
    }

    #[test]
    fn test_batch_spend_success() {
        reset_test_state();
        let owner = address!("1111111111111111111111111111111111111111");
        set_msg_sender(owner);

        let mut contract = Nimbus::default();
        contract.init(owner, Address::ZERO, Address::ZERO).unwrap();

        // Deposit enough funds to cover both spends
        contract
            .deposit(
                FixedBytes::repeat_byte(0xaa),
                vec![0x42; 256].into(),
                U256::from(50_000_000),
            )
            .unwrap();

        // Create 2 mock spends of 10,000,000 each
        let (alpha_neg1, hm1, pk_iss1, nullifier1) = register_mock_issuer_with_nonce(
            &mut contract,
            owner,
            U256::from(10_000_000),
            FixedBytes::ZERO,
            FixedBytes::repeat_byte(0x01),
        );

        let (alpha_neg2, hm2, pk_iss2, nullifier2) = register_mock_issuer_with_nonce(
            &mut contract,
            owner,
            U256::from(10_000_000),
            FixedBytes::ZERO,
            FixedBytes::repeat_byte(0x02),
        );

        // Execute batch spend
        let ok = contract
            .batch_spend(
                vec![FixedBytes::ZERO, FixedBytes::ZERO],
                vec![nullifier1, nullifier2],
                vec![alpha_neg1.into(), alpha_neg2.into()],
                vec![pk_iss1.into(), pk_iss2.into()],
                vec![Address::ZERO, Address::ZERO],
                vec![U256::from(10_000_000), U256::from(10_000_000)],
                vec![FixedBytes::ZERO, FixedBytes::ZERO],
                vec![U256::ZERO, U256::ZERO],
                vec![FixedBytes::repeat_byte(0x01), FixedBytes::repeat_byte(0x02)],
            )
            .unwrap();

        assert!(ok);

        // Verify nullifiers are marked spent
        assert!(contract.nullifiers.get(nullifier1));
        assert!(contract.nullifiers.get(nullifier2));

        // Verify principal reduction:
        // deposit net principal: 49,900,000
        // spend 1: 10,000,000 + fee (25,000) = 10,025,000 debit
        // spend 2: 10,000,000 + fee (25,000) = 10,025,000 debit
        // expected final principal: 49,900,000 - 20,050,000 = 29,850,000
        assert_eq!(
            contract.total_deposited_principal().unwrap(),
            U256::from(29_850_000)
        );
    }

    #[test]
    fn test_batch_spend_reverts_on_any_invalid() {
        reset_test_state();
        let owner = address!("1111111111111111111111111111111111111111");
        set_msg_sender(owner);

        let mut contract = Nimbus::default();
        contract.init(owner, Address::ZERO, Address::ZERO).unwrap();

        // Deposit funds
        contract
            .deposit(
                FixedBytes::repeat_byte(0xaa),
                vec![0x42; 256].into(),
                U256::from(50_000_000),
            )
            .unwrap();

        // Item 1: Valid
        let (alpha_neg1, hm1, pk_iss1, nullifier1) = register_mock_issuer_with_nonce(
            &mut contract,
            owner,
            U256::from(10_000_000),
            FixedBytes::ZERO,
            FixedBytes::repeat_byte(0x01),
        );

        // Item 2: Invalid
        let (alpha_neg2, hm2, pk_iss2, nullifier2) = register_mock_issuer_with_nonce(
            &mut contract,
            owner,
            U256::from(10_000_000),
            FixedBytes::ZERO,
            FixedBytes::repeat_byte(0x02),
        );

        // Let's set pairing result to false during verification
        set_pairing_result(false);

        let res = contract.batch_spend(
            vec![FixedBytes::ZERO, FixedBytes::ZERO],
            vec![nullifier1, nullifier2],
            vec![alpha_neg1.clone().into(), alpha_neg2.clone().into()],
            vec![pk_iss1.clone().into(), pk_iss2.clone().into()],
            vec![Address::ZERO, Address::ZERO],
            vec![U256::from(10_000_000), U256::from(10_000_000)],
            vec![FixedBytes::ZERO, FixedBytes::ZERO],
            vec![U256::ZERO, U256::ZERO],
            vec![FixedBytes::repeat_byte(0x01), FixedBytes::repeat_byte(0x02)],
        );

        assert_eq!(res, Err(b"BATCH_ITEM_INVALID".to_vec()));

        // Restore pairing result
        set_pairing_result(true);

        // Verify state is preserved
        assert!(!contract.nullifiers.get(nullifier1));
        assert!(!contract.nullifiers.get(nullifier2));
        assert_eq!(
            contract.total_deposited_principal().unwrap(),
            U256::from(49_900_000)
        );
    }

    #[test]
    fn test_single_deposit_single_payout_no_double_payout() {
        reset_test_state();
        let owner = address!("1111111111111111111111111111111111111111");
        let client = address!("2222222222222222222222222222222222222222");
        let recipient = address!("3333333333333333333333333333333333333333");

        set_msg_sender(owner);
        let mut contract = Nimbus::default();
        contract.init(owner, Address::ZERO, Address::ZERO).unwrap();

        let sid = FixedBytes::repeat_byte(0xde);
        let commitment = vec![0x99; 256];
        let deposit_amount = U256::from(10_000_000); // 10 USDC

        // 1. Deposit - principal should increase
        set_msg_sender(client);
        let initial_principal = contract.total_deposited_principal().unwrap();
        contract
            .deposit(sid, commitment.clone().into(), deposit_amount)
            .unwrap();
        let after_deposit_principal = contract.total_deposited_principal().unwrap();
        // Fee is 0.20%, so net amount = 10_000_000 - 20_000 = 9_980_000
        assert!(after_deposit_principal > initial_principal);
        let net_amount = after_deposit_principal - initial_principal;
        assert_eq!(net_amount, U256::from(9_980_000));

        // 2. Reveal - principal should NOT decrease (DEC-001)
        set_msg_sender(owner);
        let before_reveal_principal = contract.total_deposited_principal().unwrap();
        assert!(contract
            .reveal_mask_key(
                sid,
                vec![0x11; 32].into(),
                vec![0x22; 256].into(),
                commitment.into(),
            )
            .unwrap());
        let after_reveal_principal = contract.total_deposited_principal().unwrap();
        assert_eq!(
            before_reveal_principal, after_reveal_principal,
            "Reveal must not decrease principal"
        );
        assert!(contract.session_resolved.get(sid));

        // 3. Register issuer key and perform spend - principal should decrease exactly once
        let mut recipient_hash = [0u8; 32];
        recipient_hash[12..].copy_from_slice(recipient.as_slice());
        let recipient_hash = FixedBytes::from(recipient_hash);
        let spend_amount = U256::from(5_000_000);
        // flat 0.25% fee: (5_000_000 * 25 + 9999) / 10000 = 12_500
        let spend_fee = U256::from(12_500);
        let total_spend_debit = spend_amount + spend_fee;
        let (alpha_neg, hm, pk_iss, nullifier) =
            register_mock_issuer(&mut contract, owner, spend_amount, recipient_hash);
        let before_spend_principal = contract.total_deposited_principal().unwrap();

        assert!(contract
            .spend(
                FixedBytes::ZERO,
                nullifier,
                alpha_neg.clone().into(),
                pk_iss.clone().into(),
                recipient,
                spend_amount,
                recipient_hash,
                U256::ZERO,
                FixedBytes::ZERO,
                U256::ZERO,
                U256::ZERO,
            )
            .unwrap());

        let after_spend_principal = contract.total_deposited_principal().unwrap();
        assert_eq!(
            after_spend_principal,
            before_spend_principal - total_spend_debit,
            "Spend must decrease principal by payout plus fee"
        );

        // 4. Verify cannot spend again with same nullifier (replay protection)
        assert_eq!(
            contract.spend(
                FixedBytes::ZERO,
                nullifier,
                alpha_neg.clone().into(),
                pk_iss.clone().into(),
                recipient,
                spend_amount,
                recipient_hash,
                U256::ZERO,
                FixedBytes::ZERO,
                U256::ZERO,
                U256::ZERO,
            ),
            Ok(false), // spend returns false when nullifier already used
            "Double spend with same nullifier must fail"
        );
        // Principal should remain unchanged after failed double spend
        assert_eq!(
            contract.total_deposited_principal().unwrap(),
            after_spend_principal
        );

        // 5. Verify cannot refund after spend
        set_msg_sender(client);
        assert_eq!(
            contract.claim_refund(sid),
            Err(b"SESSION_ALREADY_RESOLVED".to_vec()),
            "Refund must fail after session resolved"
        );
        // Principal should remain unchanged
        assert_eq!(
            contract.total_deposited_principal().unwrap(),
            after_spend_principal
        );

        // 6. Invariant check: partial spend keeps the remaining principal as liability.
        assert_eq!(
            contract.total_deposited_principal().unwrap(),
            after_deposit_principal - total_spend_debit
        );
    }

    #[test]
    fn test_refund_decreases_principal_once_prevents_spend() {
        reset_test_state();
        let owner = address!("1111111111111111111111111111111111111111");
        let client = address!("2222222222222222222222222222222222222222");

        set_msg_sender(owner);
        let mut contract = Nimbus::default();
        contract.init(owner, Address::ZERO, Address::ZERO).unwrap();

        let sid = FixedBytes::repeat_byte(0xef);
        let commitment = vec![0xaa; 256];
        let deposit_amount = U256::from(10_000_000);

        // 1. Deposit
        set_msg_sender(client);
        let initial_principal = contract.total_deposited_principal().unwrap();
        contract
            .deposit(sid, commitment.into(), deposit_amount)
            .unwrap();
        let after_deposit_principal = contract.total_deposited_principal().unwrap();
        let net_amount = after_deposit_principal - initial_principal;

        // 2. Wait for timelock (24 hours)
        set_block_timestamp(86400 + 1000);

        // 3. Refund - principal should decrease
        let before_refund_principal = contract.total_deposited_principal().unwrap();
        contract.claim_refund(sid).unwrap();
        let after_refund_principal = contract.total_deposited_principal().unwrap();
        assert_eq!(
            after_refund_principal,
            before_refund_principal - net_amount,
            "Refund must decrease principal"
        );

        // 4. Verify cannot refund again
        assert_eq!(
            contract.claim_refund(sid),
            Err(b"SESSION_ALREADY_RESOLVED".to_vec()),
            "Double refund must fail"
        );

        // 5. Principal should remain unchanged after failed double refund
        assert_eq!(
            contract.total_deposited_principal().unwrap(),
            after_refund_principal
        );

        // 6. Invariant check: principal should be back to initial state
        assert_eq!(
            contract.total_deposited_principal().unwrap(),
            initial_principal
        );
    }

    #[test]
    fn test_holding_time_fee_discounts() {
        reset_test_state();
        let owner = address!("1111111111111111111111111111111111111111");
        set_msg_sender(owner);

        let mut contract = Nimbus::default();
        contract.init(owner, Address::ZERO, Address::ZERO).unwrap();

        // 1. Setup a deposit to get some principal
        let sid = FixedBytes::repeat_byte(0x77);
        contract
            .deposit(sid, vec![0x42; 256].into(), U256::from(50_000_000)) // 50 USDC
            .unwrap();

        // 2. Register a mock root at block_timestamp = 1000
        let root = FixedBytes::repeat_byte(0xcc);
        set_block_timestamp(1000);
        contract.register_clean_root(root).unwrap();

        // Check registered timestamp
        assert_eq!(contract.clean_association_roots.get(root), U256::from(1000));

        let spend_amount = U256::from(10_000_000); // 10 USDC

        // Scenario A: Spend using non-registered root -> should revert
        let invalid_root = FixedBytes::repeat_byte(0xee);
        let nonce_a = FixedBytes::repeat_byte(0x01);
        let (alpha_neg, _hm, pk_iss, nullifier) = register_mock_issuer_with_nonce(
            &mut contract,
            owner,
            spend_amount,
            FixedBytes::ZERO,
            nonce_a,
        );
        assert_eq!(
            contract.spend(
                invalid_root,
                nullifier,
                alpha_neg.clone().into(),
                pk_iss.clone().into(),
                Address::ZERO,
                spend_amount,
                FixedBytes::ZERO,
                U256::ZERO,
                nonce_a,
                U256::ZERO,
                U256::ZERO,
            ),
            Err(b"INVALID_ASSOCIATION_ROOT".to_vec())
        );

        // Scenario B: Spend using valid root but with delta_t < 7 days
        // block_timestamp = 1000 + 100 = 1100 (delta_t = 100) -> 0.25% fee (25,000)
        set_block_timestamp(1100);
        let nonce_b = FixedBytes::repeat_byte(0x02);
        let (alpha_neg2, _hm2, pk_iss2, nullifier2) = register_mock_issuer_with_nonce(
            &mut contract,
            owner,
            spend_amount,
            FixedBytes::ZERO,
            nonce_b,
        );
        let initial_principal = contract.total_deposited_principal().unwrap();
        assert!(contract
            .spend(
                root,
                nullifier2,
                alpha_neg2.into(),
                pk_iss2.into(),
                Address::ZERO,
                spend_amount,
                FixedBytes::ZERO,
                U256::ZERO,
                nonce_b,
                U256::ZERO,
                U256::ZERO,
            )
            .unwrap());
        let after_spend_principal = contract.total_deposited_principal().unwrap();
        // spend_fee = (10,000,000 * 25 + 9999) / 10000 = 25,000
        assert_eq!(
            initial_principal - after_spend_principal,
            U256::from(10_025_000)
        );

        // Scenario C: Spend using valid root with delta_t = 8 days (>= 7 days, < 30 days)
        // registration = 1000, current = 1000 + 8 * 86400 = 692200 (delta_t = 691200) -> 0.20% fee (20,000)
        set_block_timestamp(1000 + 8 * 86400);
        let nonce_c = FixedBytes::repeat_byte(0x03);
        let (alpha_neg3, _hm3, pk_iss3, nullifier3) = register_mock_issuer_with_nonce(
            &mut contract,
            owner,
            spend_amount,
            FixedBytes::ZERO,
            nonce_c,
        );
        let initial_principal = contract.total_deposited_principal().unwrap();
        assert!(contract
            .spend(
                root,
                nullifier3,
                alpha_neg3.into(),
                pk_iss3.into(),
                Address::ZERO,
                spend_amount,
                FixedBytes::ZERO,
                U256::ZERO,
                nonce_c,
                U256::ZERO,
                U256::ZERO,
            )
            .unwrap());
        let after_spend_principal = contract.total_deposited_principal().unwrap();
        // spend_fee = (10,000,000 * 20 + 9999) / 10000 = 20,000
        assert_eq!(
            initial_principal - after_spend_principal,
            U256::from(10_020_000)
        );

        // Scenario D: Spend using valid root with delta_t = 31 days (>= 30 days)
        // registration = 1000, current = 1000 + 31 * 86400 = 2679400 -> 0.10% fee (10,000)
        set_block_timestamp(1000 + 31 * 86400);
        let nonce_d = FixedBytes::repeat_byte(0x04);
        let (alpha_neg4, _hm4, pk_iss4, nullifier4) = register_mock_issuer_with_nonce(
            &mut contract,
            owner,
            spend_amount,
            FixedBytes::ZERO,
            nonce_d,
        );
        let initial_principal = contract.total_deposited_principal().unwrap();
        assert!(contract
            .spend(
                root,
                nullifier4,
                alpha_neg4.into(),
                pk_iss4.into(),
                Address::ZERO,
                spend_amount,
                FixedBytes::ZERO,
                U256::ZERO,
                nonce_d,
                U256::ZERO,
                U256::ZERO,
            )
            .unwrap());
        let after_spend_principal = contract.total_deposited_principal().unwrap();
        // spend_fee = (10,000,000 * 10 + 9999) / 10000 = 10,000
        assert_eq!(
            initial_principal - after_spend_principal,
            U256::from(10_010_000)
        );
    }

    // --- Execution Fee Tests ---

    #[test]
    fn test_execution_fee_exceeded_reverts() {
        reset_test_state();
        let mut contract = Nimbus::default();
        let owner = Address::repeat_byte(0x01);

        contract
            .init(
                owner,
                Address::repeat_byte(0x02),
                Address::repeat_byte(0x03),
            )
            .unwrap();

        let spend_amount = U256::from(10_000_000);
        let nonce = FixedBytes::repeat_byte(0x01);
        let (alpha_neg, _hm, pk_iss, nullifier) = register_mock_issuer_with_nonce(
            &mut contract,
            owner,
            spend_amount,
            FixedBytes::ZERO,
            nonce,
        );

        // Try to spend with execution_fee > max_execution_fee
        let result = contract.spend(
            FixedBytes::ZERO,
            nullifier,
            alpha_neg.into(),
            pk_iss.into(),
            Address::ZERO,
            spend_amount,
            FixedBytes::ZERO,
            U256::ZERO,
            nonce,
            U256::from(100), // max_execution_fee
            U256::from(200), // execution_fee (exceeds max)
        );

        assert_eq!(result, Err(b"EXECUTION_FEE_EXCEEDED".to_vec()));
    }

    #[test]
    fn test_execution_fee_accumulation() {
        reset_test_state();
        let mut contract = Nimbus::default();
        let owner = Address::repeat_byte(0x01);
        let fee_recipient = Address::repeat_byte(0x03);

        contract
            .init(owner, Address::repeat_byte(0x02), fee_recipient)
            .unwrap();

        // Set initial principal
        contract
            .total_deposited_principal
            .set(U256::from(100_000_000));

        let spend_amount = U256::from(10_000_000);
        let nonce = FixedBytes::repeat_byte(0x01);
        let (alpha_neg, _hm, pk_iss, nullifier) = register_mock_issuer_with_nonce(
            &mut contract,
            owner,
            spend_amount,
            FixedBytes::ZERO,
            nonce,
        );

        let initial_fees = contract.get_accumulated_fees().unwrap();

        // Spend with execution_fee = 500
        assert!(contract
            .spend(
                FixedBytes::ZERO,
                nullifier,
                alpha_neg.into(),
                pk_iss.into(),
                Address::ZERO,
                spend_amount,
                FixedBytes::ZERO,
                U256::ZERO,
                nonce,
                U256::from(1000), // max_execution_fee
                U256::from(500),  // execution_fee
            )
            .unwrap());

        let final_fees = contract.get_accumulated_fees().unwrap();
        assert_eq!(final_fees - initial_fees, U256::from(500));
    }

    #[test]
    fn test_set_execution_fee_recipient() {
        reset_test_state();
        let mut contract = Nimbus::default();
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
        let mut contract = Nimbus::default();
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
        let mut contract = Nimbus::default();
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
        let result = contract.claim_execution_fees(U256::from(1000));
        assert_eq!(result, Err(b"INSUFFICIENT_ACCUMULATED_FEES".to_vec()));
    }

    #[test]
    fn test_claim_execution_fees_no_recipient_set() {
        reset_test_state();
        let mut contract = Nimbus::default();
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
        let result = contract.claim_execution_fees(U256::from(1000));
        assert_eq!(result, Err(b"NO_RECIPIENT_SET".to_vec()));
    }

    #[test]
    fn test_multi_liability_deposit_reveal_spend_lifecycle() {
        reset_test_state();
        let mut contract = Nimbus::default();
        let owner = Address::repeat_byte(0x01);
        let client = Address::repeat_byte(0x09);
        let fee_recipient = Address::repeat_byte(0x03);

        contract
            .init(owner, Address::repeat_byte(0x02), fee_recipient)
            .unwrap();

        let sid = FixedBytes::repeat_byte(0xaa);
        let deposit_amount = U256::from(100_000_000); // 100 USDC
        let dummy_com_k = Bytes::from(alloc::vec![0u8; 256]);

        set_msg_sender(client);
        set_block_timestamp(1000);
        contract
            .deposit(sid, dummy_com_k.clone(), deposit_amount)
            .unwrap();

        // 0.20% fee on 100_000_000 is 200_000. Net deposit = 99_800_000
        let net_deposit = U256::from(99_800_000);
        assert_eq!(
            contract.refundable_deposit_liability().unwrap(),
            net_deposit
        );
        assert_eq!(contract.user_note_liability().unwrap(), U256::ZERO);
        assert_eq!(
            contract.accrued_execution_fee_liability().unwrap(),
            U256::ZERO
        );

        // Reveal the session
        let k_bytes = Bytes::from(alloc::vec![0u8; 32]);
        let pk_iss_bytes = Bytes::from(alloc::vec![0u8; 256]);
        assert!(contract
            .reveal_mask_key(sid, k_bytes, pk_iss_bytes, dummy_com_k.clone())
            .unwrap());

        // After reveal: refundable drops to 0, user_note_liability increases to net_deposit
        assert_eq!(contract.refundable_deposit_liability().unwrap(), U256::ZERO);
        assert_eq!(contract.user_note_liability().unwrap(), net_deposit);

        // Perform spend of 10 USDC with 500 execution_fee
        let spend_amount = U256::from(10_000_000);
        let nonce = FixedBytes::repeat_byte(0xbb);
        let (alpha_neg, _hm, pk_iss, nullifier) = register_mock_issuer_with_nonce(
            &mut contract,
            owner,
            spend_amount,
            FixedBytes::ZERO,
            nonce,
        );

        let initial_user_liab = contract.user_note_liability().unwrap();
        assert!(contract
            .spend(
                FixedBytes::ZERO,
                nullifier,
                alpha_neg.into(),
                pk_iss.into(),
                Address::ZERO,
                spend_amount,
                FixedBytes::ZERO,
                U256::ZERO,
                nonce,
                U256::from(1000), // max_execution_fee
                U256::from(500),  // execution_fee
            )
            .unwrap());

        // 0.25% protocol fee on 10_000_000 is 25_000
        let protocol_share = U256::from(25_000);
        let exec_fee = U256::from(500);
        let total_user_debit = spend_amount + protocol_share + exec_fee;

        assert_eq!(
            contract.user_note_liability().unwrap(),
            initial_user_liab - total_user_debit
        );
        assert_eq!(
            contract.accrued_execution_fee_liability().unwrap(),
            exec_fee
        );
        assert_eq!(contract.realized_protocol_fees().unwrap(), protocol_share);
    }

    #[test]
    fn test_multi_liability_refund_reduces_refundable_liability() {
        reset_test_state();
        let mut contract = Nimbus::default();
        let owner = Address::repeat_byte(0x01);
        let client = Address::repeat_byte(0x09);

        contract
            .init(
                owner,
                Address::repeat_byte(0x02),
                Address::repeat_byte(0x03),
            )
            .unwrap();

        let sid = FixedBytes::repeat_byte(0xcc);
        let deposit_amount = U256::from(50_000_000); // 50 USDC
        let dummy_com_k = Bytes::from(alloc::vec![0u8; 256]);

        set_msg_sender(client);
        set_block_timestamp(1000);
        contract.deposit(sid, dummy_com_k, deposit_amount).unwrap();

        let net_deposit = contract.session_amount.get(sid);
        assert_eq!(
            contract.refundable_deposit_liability().unwrap(),
            net_deposit
        );

        // Advance 24h + 1s timelock
        set_block_timestamp(1000 + 86401);
        set_msg_sender(client);
        contract.claim_refund(sid).unwrap();

        assert_eq!(contract.refundable_deposit_liability().unwrap(), U256::ZERO);
        assert_eq!(contract.user_note_liability().unwrap(), U256::ZERO);
    }

    #[test]
    fn test_claim_execution_fees_updates_accrued_liability() {
        reset_test_state();
        let mut contract = Nimbus::default();
        let owner = Address::repeat_byte(0x01);
        let relayer = Address::repeat_byte(0x04);

        contract
            .init(
                owner,
                Address::repeat_byte(0x02),
                Address::repeat_byte(0x03),
            )
            .unwrap();

        set_msg_sender(owner);
        contract.set_execution_fee_recipient(relayer).unwrap();

        // Simulate 500 execution fee accrued
        contract.accumulated_execution_fees.set(U256::from(500));
        contract
            .accrued_execution_fee_liability
            .set(U256::from(500));

        // Claim 300
        set_msg_sender(owner);
        contract.claim_execution_fees(U256::from(300)).unwrap();

        assert_eq!(contract.accumulated_execution_fees.get(), U256::from(200));
        assert_eq!(
            contract.accrued_execution_fee_liability().unwrap(),
            U256::from(200)
        );

        // Claim remaining 200
        contract.claim_execution_fees(U256::from(200)).unwrap();
        assert_eq!(contract.accumulated_execution_fees.get(), U256::ZERO);
        assert_eq!(
            contract.accrued_execution_fee_liability().unwrap(),
            U256::ZERO
        );
    }

    // ═══════════════════════════════════════════════════════════════════════
    // Gate D: Append-Only Note Commitment Tree & Gated Minting Tests
    // ═══════════════════════════════════════════════════════════════════════

    #[test]
    fn test_merkle_tree_empty_root_matches_kav() {
        reset_test_state();
        let contract = Nimbus::default();

        let expected_empty_root =
            FixedBytes::from(crate::poseidon_w5_constants::EMPTY_TREE_ROOT_BYTES);
        assert_eq!(
            contract.note_tree_root().unwrap(),
            expected_empty_root,
            "Initial root must match canonical empty tree root (depth 20)"
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
        let mut contract = Nimbus::default();

        // KAV note commitment from DEC-016A
        let leaf = FixedBytes::from(alloy_primitives::hex!(
            "46d1b90c8a28c364fefdbb95bd709956e2f39a411750e6797fe1112b27635c59"
        ));
        let expected_root_after_1 = FixedBytes::from(alloy_primitives::hex!(
            "3ed6d45ee8da74b055fa37a13ec3459e9fa97db5f89d014fdb0e41013fb56bfd"
        ));

        set_block_timestamp(500);
        let (leaf_idx, new_root) = contract._merkle_insert(leaf).unwrap();

        assert_eq!(leaf_idx, U256::ZERO);
        assert_eq!(
            new_root, expected_root_after_1,
            "Root after 1 insert must exactly match DEC-016A KAV"
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
    fn test_merkle_tree_gated_deposit_and_reveal_minting() {
        reset_test_state();
        let mut contract = Nimbus::default();
        let owner = Address::repeat_byte(0x01);
        let client = Address::repeat_byte(0x05);
        let fee_recipient = Address::repeat_byte(0x03);

        contract
            .init(owner, Address::repeat_byte(0x02), fee_recipient)
            .unwrap();

        let sid = FixedBytes::repeat_byte(0xdd);
        let deposit_amount = U256::from(100_000_000); // 100 USDC
        let dummy_com_k = Bytes::from(alloc::vec![0u8; 256]);
        let note_commitment = FixedBytes::from(alloy_primitives::hex!(
            "46d1b90c8a28c364fefdbb95bd709956e2f39a411750e6797fe1112b27635c59"
        ));
        let expected_root_after_1 = FixedBytes::from(alloy_primitives::hex!(
            "3ed6d45ee8da74b055fa37a13ec3459e9fa97db5f89d014fdb0e41013fb56bfd"
        ));

        // 1. User deposits with bound note commitment
        set_msg_sender(client);
        set_block_timestamp(1000);
        contract
            .deposit_with_commitment(sid, dummy_com_k.clone(), deposit_amount, note_commitment)
            .unwrap();

        // Before reveal: commitment is recorded, but Merkle tree is still empty (gated!)
        assert_eq!(
            contract.session_note_commitment(sid).unwrap(),
            note_commitment
        );
        assert_eq!(contract.note_tree_next_index().unwrap(), U256::ZERO);
        assert_eq!(
            contract.note_tree_root().unwrap(),
            FixedBytes::from(crate::poseidon_w5_constants::EMPTY_TREE_ROOT_BYTES)
        );

        // 2. Leader & guardians reveal masking key k
        let k_bytes = Bytes::from(alloc::vec![0u8; 32]);
        let pk_iss_bytes = Bytes::from(alloc::vec![0u8; 256]);
        set_block_timestamp(1050);
        let revealed = contract
            .reveal_mask_key(sid, k_bytes, pk_iss_bytes, dummy_com_k.clone())
            .unwrap();
        assert!(revealed);

        // After reveal: commitment is automatically and atomically minted into tree
        assert_eq!(contract.note_tree_next_index().unwrap(), U256::from(1));
        assert_eq!(contract.note_tree_root().unwrap(), expected_root_after_1);
        assert!(contract
            .is_accepted_note_root(expected_root_after_1)
            .unwrap());

        // Liabilities shifted properly
        assert_eq!(contract.refundable_deposit_liability().unwrap(), U256::ZERO);
        assert_eq!(
            contract.user_note_liability().unwrap(),
            U256::from(99_800_000)
        );
    }

    #[test]
    fn test_merkle_tree_refunded_deposit_cannot_mint_leaf() {
        reset_test_state();
        let mut contract = Nimbus::default();
        let owner = Address::repeat_byte(0x01);
        let client = Address::repeat_byte(0x05);

        contract
            .init(
                owner,
                Address::repeat_byte(0x02),
                Address::repeat_byte(0x03),
            )
            .unwrap();

        let sid = FixedBytes::repeat_byte(0xee);
        let deposit_amount = U256::from(50_000_000); // 50 USDC
        let dummy_com_k = Bytes::from(alloc::vec![0u8; 256]);
        let note_commitment = FixedBytes::from(alloy_primitives::hex!(
            "46d1b90c8a28c364fefdbb95bd709956e2f39a411750e6797fe1112b27635c59"
        ));

        // 1. User deposits
        set_msg_sender(client);
        set_block_timestamp(1000);
        contract
            .deposit_with_commitment(sid, dummy_com_k.clone(), deposit_amount, note_commitment)
            .unwrap();

        // 2. Issuance fails / leader unresponsive. 24h timelock expires
        set_block_timestamp(1000 + 86400 + 1);
        contract.claim_refund(sid).unwrap();

        // 3. Leader attempts late reveal: must return Ok(false) and NEVER mint leaf
        let k_bytes = Bytes::from(alloc::vec![0u8; 32]);
        let pk_iss_bytes = Bytes::from(alloc::vec![0u8; 256]);
        let revealed = contract
            .reveal_mask_key(sid, k_bytes, pk_iss_bytes, dummy_com_k)
            .unwrap();
        assert!(!revealed, "Late reveal after refund must return false");

        // Merkle tree remains completely unmolested
        assert_eq!(contract.note_tree_next_index().unwrap(), U256::ZERO);
        assert_eq!(
            contract.note_tree_root().unwrap(),
            FixedBytes::from(crate::poseidon_w5_constants::EMPTY_TREE_ROOT_BYTES)
        );
        assert_eq!(contract.user_note_liability().unwrap(), U256::ZERO);
    }

    #[test]
    fn test_merkle_tree_invalid_commitment_scalar_rejected() {
        reset_test_state();
        let mut contract = Nimbus::default();
        let owner = Address::repeat_byte(0x01);
        let client = Address::repeat_byte(0x05);

        contract
            .init(
                owner,
                Address::repeat_byte(0x02),
                Address::repeat_byte(0x03),
            )
            .unwrap();

        let sid = FixedBytes::repeat_byte(0xff);
        let deposit_amount = U256::from(50_000_000);
        let dummy_com_k = Bytes::from(alloc::vec![0u8; 256]);

        // Scalar >= modulus p for BLS12-381 Fr: 0xffffff...
        let invalid_commitment = FixedBytes::repeat_byte(0xff);

        set_msg_sender(client);
        let err = contract
            .deposit_with_commitment(sid, dummy_com_k, deposit_amount, invalid_commitment)
            .unwrap_err();

        assert_eq!(err, b"INVALID_NOTE_COMMITMENT_SCALAR".to_vec());
        assert_eq!(contract.note_tree_next_index().unwrap(), U256::ZERO);
    }
}
