//! View and Query methods for Zeltra contract

use crate::poseidon_w5_constants;
use crate::storage::Zeltra;
use alloc::vec::Vec;
use alloy_primitives::{Address, FixedBytes};
use stylus_sdk::{abi::Bytes, alloy_primitives::U256};

impl Zeltra {
    // --- Multi-Asset & Token Queries ---

    pub fn _get_asset_token(&self, asset_id: U256) -> Result<Address, Vec<u8>> {
        Ok(self.asset_tokens.get(asset_id))
    }

    pub fn _weth9_address(&self) -> Result<Address, Vec<u8>> {
        Ok(self.weth9_address.get())
    }

    pub fn _stablecoin(&self) -> Result<Address, Vec<u8>> {
        Ok(self.stablecoin.get())
    }

    pub fn _fee_recipient(&self) -> Result<Address, Vec<u8>> {
        Ok(self.fee_recipient.get())
    }

    pub fn _ccip_router(&self) -> Result<Address, Vec<u8>> {
        Ok(self.ccip_router.get())
    }

    pub fn _is_ccip_sender_allowed(
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

    // --- Liabilities, Fees & Yield Queries ---

    pub fn _deposit_fee_bps(&self) -> Result<U256, Vec<u8>> {
        Ok(self.get_deposit_fee_bps())
    }

    pub fn _get_accumulated_fees(&self) -> Result<U256, Vec<u8>> {
        Ok(self.accrued_execution_fee_liabilities.get(U256::from(1)))
    }

    pub fn _get_accumulated_fees_of(&self, asset_id: U256) -> Result<U256, Vec<u8>> {
        Ok(self.accrued_execution_fee_liabilities.get(asset_id))
    }

    pub fn _get_execution_fee_recipient(&self) -> Result<Address, Vec<u8>> {
        Ok(self.execution_fee_recipient.get())
    }

    pub fn _user_note_liability(&self) -> Result<U256, Vec<u8>> {
        Ok(self.user_note_liabilities.get(U256::from(1)))
    }

    pub fn _user_note_liabilities(&self, asset_id: U256) -> Result<U256, Vec<u8>> {
        Ok(self.user_note_liabilities.get(asset_id))
    }

    pub fn _accrued_execution_fee_liability(&self) -> Result<U256, Vec<u8>> {
        Ok(self.accrued_execution_fee_liabilities.get(U256::from(1)))
    }

    pub fn _accrued_execution_fee_liabilities(&self, asset_id: U256) -> Result<U256, Vec<u8>> {
        Ok(self.accrued_execution_fee_liabilities.get(asset_id))
    }

    pub fn _realized_protocol_fees(&self) -> Result<U256, Vec<u8>> {
        Ok(self.realized_protocol_fees.get(U256::from(1)))
    }

    pub fn _realized_protocol_fees_of(&self, asset_id: U256) -> Result<U256, Vec<u8>> {
        Ok(self.realized_protocol_fees.get(asset_id))
    }

    pub fn _total_deposited_principal(&self) -> Result<U256, Vec<u8>> {
        Ok(self.total_deposited_principals.get(U256::from(1)))
    }

    pub fn _total_deposited_principals(&self, asset_id: U256) -> Result<U256, Vec<u8>> {
        Ok(self.total_deposited_principals.get(asset_id))
    }

    // --- MMR Tree & Roots Queries ---

    pub fn _note_tree_root(&self) -> Result<FixedBytes<32>, Vec<u8>> {
        let count = self.mmr_leaf_count.get();
        if count == U256::ZERO && self.note_tree_root.get() == FixedBytes::ZERO {
            Ok(FixedBytes::from(
                poseidon_w5_constants::EMPTY_TREE_ROOT_BYTES,
            ))
        } else {
            Ok(self.note_tree_root.get())
        }
    }

    pub fn _mmr_leaf_count(&self) -> Result<U256, Vec<u8>> {
        Ok(self.mmr_leaf_count.get())
    }

    pub fn _mmr_peak(&self, height: U256) -> Result<FixedBytes<32>, Vec<u8>> {
        Ok(self.mmr_peaks.get(height))
    }

    pub fn _note_tree_next_index(&self) -> Result<U256, Vec<u8>> {
        Ok(self.mmr_leaf_count.get())
    }

    pub fn _get_root_history(&self, slot: U256) -> Result<FixedBytes<32>, Vec<u8>> {
        let size = U256::from(poseidon_w5_constants::ROOT_HISTORY_SIZE);
        Ok(self.root_history.get(slot % size))
    }

    // --- Nullifiers Queries ---

    pub fn _is_nullifier_spent(&self, nullifier: FixedBytes<32>) -> Result<bool, Vec<u8>> {
        self._is_note_nullifier_spent(nullifier)
    }

    pub fn _are_nullifiers_spent(
        &self,
        nullifiers: Vec<FixedBytes<32>>,
    ) -> Result<Vec<bool>, Vec<u8>> {
        let mut results = Vec::with_capacity(nullifiers.len());
        for nf in nullifiers {
            results.push(self._is_nullifier_spent(nf)?);
        }
        Ok(results)
    }

    pub fn _is_note_nullifier_spent(&self, nullifier: FixedBytes<32>) -> Result<bool, Vec<u8>> {
        let current_epoch = self.current_epoch.get().to::<u32>();
        let slot_current = (current_epoch % 2) as u8;
        let gen_key_current = crate::storage::generational_nullifier_key(slot_current, nullifier);
        let slot_prev = (current_epoch.saturating_sub(1) % 2) as u8;
        let gen_key_prev = crate::storage::generational_nullifier_key(slot_prev, nullifier);
        Ok(self.generation_nullifiers.get(gen_key_current)
            || self.generation_nullifiers.get(gen_key_prev)
            || self.note_nullifiers.get(nullifier))
    }

    pub fn _is_generational_nullifier_spent(
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

    // --- Epoch & Clean Roots Queries ---

    pub fn _get_current_epoch(&self) -> Result<u32, Vec<u8>> {
        Ok(self.current_epoch.get().to::<u32>())
    }

    pub fn _epoch_start_timestamp(&self) -> Result<u64, Vec<u8>> {
        Ok(self.epoch_start_timestamp.get().to::<u64>())
    }

    pub fn _get_clean_root_timestamp(&self, root: FixedBytes<32>) -> Result<U256, Vec<u8>> {
        Ok(self.clean_association_roots.get(root))
    }
}
