//! Direct MMR Note Inflow Protocol (DEC-038B)
//!
//! Handles single-step, atomic token issuance into the on-chain Merkle Mountain Range.

use alloc::vec::Vec;
use alloy_primitives::{FixedBytes, U256};
use stylus_sdk::abi::Bytes;
#[cfg(not(test))]
use stylus_sdk::prelude::Call;

#[cfg(not(test))]
use crate::interfaces::IErc20;
use crate::storage::Nimbus;

impl Nimbus {
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

        // Minimum deposit check: 10 USDC (10_000_000 micro-units)
        if amount < U256::from(10_000_000) {
            return Err(b"AMOUNT_TOO_SMALL".to_vec());
        }

        // Anti-Nomad zero-root exploit prevention
        if note_commitment == FixedBytes::<32>::ZERO {
            return Err(b"ZERO_COMMITMENT_REJECTED".to_vec());
        }

        // Encrypted memo length bound validation [64..=256]
        if encrypted_memo.len() < 64 || encrypted_memo.len() > 256 {
            return Err(b"INVALID_MEMO_LENGTH".to_vec());
        }

        // Canonical scalar field element check (anti-aliasing modulus EIP-2537)
        if crate::types::from_evm_scalar(&note_commitment.0).is_none() {
            return Err(b"INVALID_NOTE_COMMITMENT_SCALAR".to_vec());
        }

        #[cfg(not(test))]
        {
            let client = self.msg_sender();
            let this_address = self.env_contract_address();
            let stablecoin_address = self.stablecoin.get();
            let erc20 = IErc20::new(stablecoin_address);
            let host = Self::runtime_host();
            let success = erc20.transfer_from(
                &host,
                Call::new_mutating(self),
                client,
                this_address,
                amount,
            )?;
            if !success {
                return Err(b"TRANSFER_FROM_FAILED".to_vec());
            }
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
