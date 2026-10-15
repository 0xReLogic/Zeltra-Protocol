//! Direct MMR Note Inflow Protocol (DEC-038B)
//!
//! Handles single-step, atomic token issuance into the on-chain Merkle Mountain Range.

use alloc::vec::Vec;
use alloy_primitives::{FixedBytes, U256};
use stylus_sdk::abi::Bytes;

use crate::storage::Zeltra;

impl Zeltra {
    /// Returns the current deposit fee in basis points (defaults to 0 bps = 0.00% if uninitialized)
    pub fn get_deposit_fee_bps(&self) -> U256 {
        if self.deposit_fee_initialized.get() {
            self.deposit_fee_bps.get()
        } else {
            U256::ZERO
        }
    }

    /// Single-step atomic deposit into the Merkle Mountain Range (DEC-038B).
    pub fn _deposit_direct_note(
        &mut self,
        amount: U256,
        note_commitment: FixedBytes<32>,
        encrypted_memo: Bytes,
    ) -> Result<(), Vec<u8>> {
        self.check_not_paused()?;

        if amount < U256::from(10_000_000) {
            return Err(b"AMOUNT_TOO_SMALL".to_vec());
        }

        if note_commitment == FixedBytes::<32>::ZERO {
            return Err(b"ZERO_COMMITMENT_REJECTED".to_vec());
        }

        if crate::types::from_evm_scalar(&note_commitment.0).is_none() {
            return Err(b"INVALID_NOTE_COMMITMENT_SCALAR".to_vec());
        }

        if !encrypted_memo.is_empty() && encrypted_memo.len() != 96 {
            return Err(b"INVALID_MEMO_LENGTH".to_vec());
        }

        let client = self.msg_sender();
        let this_address = self.env_contract_address();
        let success = self.erc20_transfer_from(client, this_address, amount)?;
        if !success {
            return Err(b"TRANSFER_FROM_FAILED".to_vec());
        }

        // Mutate liabilities and principal
        let principal = self.total_deposited_principal.get();
        self.total_deposited_principal.set(
            principal
                .checked_add(amount)
                .ok_or_else(|| b"OVERFLOW".to_vec())?,
        );

        let user_liab = self.user_note_liability.get();
        self.user_note_liability.set(
            user_liab
                .checked_add(amount)
                .ok_or_else(|| b"OVERFLOW".to_vec())?,
        );

        // Direct MMR leaf insertion
        self._mmr_insert(note_commitment, encrypted_memo.to_vec())?;

        // Enforce solvency invariant
        self.check_liability_invariant()?;

        Ok(())
    }
}
