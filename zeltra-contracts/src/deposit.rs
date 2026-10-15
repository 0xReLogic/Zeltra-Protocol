//! Direct MMR Note Inflow Protocol (DEC-038B & DEC-034B)
//!
//! Handles single-step, atomic token issuance into the on-chain Merkle Mountain Range
//! with support for multi-asset settlement (USDC, WETH, and Native ETH auto-wrap).

use alloc::vec::Vec;
use alloy_primitives::{Address, FixedBytes, U256};
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

    /// 1. Direct USDC Deposit (Asset ID 1: Raw = Micro-USDC, Norm = Micro-USDC)
    pub fn _deposit_direct_note(
        &mut self,
        amount_micro: U256,
        note_commitment: FixedBytes<32>,
        encrypted_memo: Bytes,
    ) -> Result<(), Vec<u8>> {
        self.check_not_paused()?;
        if amount_micro < U256::from(10_000_000u64) {
            return Err(b"AMOUNT_TOO_SMALL".to_vec());
        }
        if amount_micro > U256::from(u64::MAX) {
            return Err(b"AMOUNT_EXCEEDS_U64_CIRCUIT_BOUND".to_vec());
        }
        self._execute_direct_erc20_inflow(
            1,
            amount_micro,
            amount_micro,
            note_commitment,
            encrypted_memo,
        )
    }

    /// 2. Direct WETH ERC-20 Deposit (Asset ID 0: Raw = Wei, Norm = Gwei)
    pub fn _deposit_direct_weth(
        &mut self,
        amount_wei: U256,
        note_commitment: FixedBytes<32>,
        encrypted_memo: Bytes,
    ) -> Result<(), Vec<u8>> {
        self.check_not_paused()?;
        // Min 0.005 WETH (5_000_000 Gwei = 5 * 10^15 Wei)
        if amount_wei < U256::from(5_000_000_000_000_000u64) {
            return Err(b"AMOUNT_TOO_SMALL".to_vec());
        }
        let gwei_scale = U256::from(1_000_000_000u64);
        if amount_wei % gwei_scale != U256::ZERO {
            return Err(b"WETH_AMOUNT_NOT_GWEI_ALIGNED".to_vec());
        }
        let amount_gwei = amount_wei / gwei_scale;
        if amount_gwei > U256::from(u64::MAX) {
            return Err(b"AMOUNT_EXCEEDS_U64_CIRCUIT_BOUND".to_vec());
        }
        // PENTING (Patch Audit #3): oper amount_wei untuk ERC-20 transferFrom, dan amount_gwei untuk liabilitas!
        self._execute_direct_erc20_inflow(
            0,
            amount_wei,
            amount_gwei,
            note_commitment,
            encrypted_memo,
        )
    }

    /// Helper Internal Eksekusi Inflow ERC-20 (Strict CEI Compliant)
    pub(crate) fn _execute_direct_erc20_inflow(
        &mut self,
        asset_id: u8,
        raw_erc20_amount: U256,
        norm_liability_amount: U256,
        note_commitment: FixedBytes<32>,
        encrypted_memo: Bytes,
    ) -> Result<(), Vec<u8>> {
        // 1. CHECKS
        if note_commitment == FixedBytes::<32>::ZERO {
            return Err(b"ZERO_COMMITMENT_REJECTED".to_vec());
        }
        if crate::types::from_evm_scalar(&note_commitment.0).is_none() {
            return Err(b"INVALID_NOTE_COMMITMENT_SCALAR".to_vec());
        }
        if !encrypted_memo.is_empty() && encrypted_memo.len() != 96 {
            return Err(b"INVALID_MEMO_LENGTH".to_vec());
        }
        let token_addr = self.asset_tokens.get(U256::from(asset_id));
        if token_addr == Address::ZERO {
            return Err(b"ASSET_NOT_CONFIGURED".to_vec());
        }

        // 2. INTERACTIONS: Pull tokens first
        let client = self.msg_sender();
        let this_address = self.env_contract_address();
        self.asset_safe_transfer_from(token_addr, client, this_address, raw_erc20_amount)?;

        // 3. EFFECTS (Catat liabilitas dalam satuan normalisasi sirkuit: Gwei atau Micro-USDC)
        let asset_key = U256::from(asset_id);
        let cur_liab = self.user_note_liabilities.get(asset_key);
        self.user_note_liabilities.setter(asset_key).set(
            cur_liab
                .checked_add(norm_liability_amount)
                .ok_or_else(|| b"OVERFLOW".to_vec())?,
        );

        let cur_principal = self.total_deposited_principals.get(asset_key);
        self.total_deposited_principals.setter(asset_key).set(
            cur_principal
                .checked_add(norm_liability_amount)
                .ok_or_else(|| b"OVERFLOW".to_vec())?,
        );

        // Direct MMR leaf insertion
        self._mmr_insert(note_commitment, encrypted_memo.to_vec())?;

        // 4. SOLVENCY INVARIANT CHECK
        self._check_asset_solvency_invariant(asset_id)?;

        Ok(())
    }

    /// 3. Direct Native ETH Auto-Wrap Deposit (Payable - Asset ID 0) — Strict CEI Compliant
    pub fn _deposit_direct_native_eth(
        &mut self,
        note_commitment: FixedBytes<32>,
        encrypted_memo: Bytes,
    ) -> Result<(), Vec<u8>> {
        // --- 1. CHECKS ---
        self.check_not_paused()?;
        let amount_wei = self.msg_value();

        // Min 0.005 ETH (5 * 10^15 Wei = 5_000_000 Gwei)
        if amount_wei < U256::from(5_000_000_000_000_000u64) {
            return Err(b"AMOUNT_TOO_SMALL".to_vec());
        }
        let gwei_scale = U256::from(1_000_000_000u64);
        if amount_wei % gwei_scale != U256::ZERO {
            return Err(b"ETH_VALUE_NOT_GWEI_ALIGNED".to_vec());
        }
        let amount_gwei = amount_wei / gwei_scale;
        if amount_gwei > U256::from(u64::MAX) {
            return Err(b"AMOUNT_EXCEEDS_U64_CIRCUIT_BOUND".to_vec());
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

        let weth_addr = self.weth9_address.get();
        if weth_addr == Address::ZERO {
            return Err(b"WETH9_NOT_CONFIGURED".to_vec());
        }

        // --- 2. EFFECTS (State mutations precede external interactions — Strict CEI) ---
        let cur_liab = self.user_note_liabilities.get(U256::ZERO);
        self.user_note_liabilities.setter(U256::ZERO).set(
            cur_liab
                .checked_add(amount_gwei)
                .ok_or_else(|| b"OVERFLOW".to_vec())?,
        );

        let cur_principal = self.total_deposited_principals.get(U256::ZERO);
        self.total_deposited_principals.setter(U256::ZERO).set(
            cur_principal
                .checked_add(amount_gwei)
                .ok_or_else(|| b"OVERFLOW".to_vec())?,
        );

        // Direct MMR leaf insertion
        self._mmr_insert(note_commitment, encrypted_memo.to_vec())?;

        // --- 3. INTERACTIONS (External call to canonical WETH9 wrapper: deposit{value: amount_wei}()) ---
        self.weth9_deposit(weth_addr, amount_wei)?;

        // --- 4. SOLVENCY ASSERTION ---
        self._check_asset_solvency_invariant(0)?;

        Ok(())
    }
}
