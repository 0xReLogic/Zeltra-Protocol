//! Contract accounting state machine — DEC-016A Gate B
//!
//! Separates liabilities into distinct categories and enforces the solvency
//! invariant through all state transitions.
//!
//! # Invariant
//!
//! ```text
//! assets >= user_note_liability + refundable_deposit_liability + accrued_execution_fee_liability
//! ```
//!
//! # State Transitions
//!
//! 1. Deposit: user sends USDC → refundable_deposit_liability increases
//! 2. Reveal: deposit confirmed → refundable → user_note_liability
//! 3. Refund: unresolved deposit → refundable_deposit_liability decreases, assets decrease
//! 4. Spend: note consumed → user_note_liability decreases, execution fee accrues
//! 5. ClaimExecutionFee: relayer claims → accrued_execution_fee_liability decreases, assets decrease
//! 6. FailedSettlement: spend reverted → user_note_liability restored, execution fee not accrued

use crate::fees;

/// Contract accounting state.
///
/// All values in USDC base units (6 decimals). All arithmetic uses checked
/// operations — overflow returns `None` from transition functions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContractAccounting {
    /// USDC balance held by the contract.
    pub assets: u64,
    /// Sum of all unspent private note values owned by users.
    pub user_note_liability: u64,
    /// Sum of unresolved deposit amounts that can still be refunded.
    pub refundable_deposit_liability: u64,
    /// Execution fees accrued by the relayer but not yet claimed.
    pub accrued_execution_fee_liability: u64,
    /// Protocol fees that have been realized (owner's equity).
    pub realized_protocol_fees: u64,
}

/// Result of a state transition: the new state and metadata about what happened.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransitionResult {
    pub new_state: ContractAccounting,
    pub protocol_fee_realized: u64,
    pub execution_fee_accrued: u64,
}

impl ContractAccounting {
    /// Create a new empty accounting state.
    pub fn new() -> Self {
        Self {
            assets: 0,
            user_note_liability: 0,
            refundable_deposit_liability: 0,
            accrued_execution_fee_liability: 0,
            realized_protocol_fees: 0,
        }
    }

    /// Check the solvency invariant:
    /// assets >= user_note_liability + refundable_deposit_liability + accrued_execution_fee_liability
    pub fn check_solvency(&self) -> bool {
        let total_liabilities = self
            .user_note_liability
            .checked_add(self.refundable_deposit_liability)
            .and_then(|s| s.checked_add(self.accrued_execution_fee_liability));

        match total_liabilities {
            Some(total) => self.assets >= total,
            None => false, // overflow = insolvent
        }
    }

    /// Total liabilities (user notes + refundable deposits + accrued exec fees).
    pub fn total_liabilities(&self) -> Option<u64> {
        self.user_note_liability
            .checked_add(self.refundable_deposit_liability)
            .and_then(|s| s.checked_add(self.accrued_execution_fee_liability))
    }

    /// Surplus = assets - total_liabilities. Should always be >= 0.
    pub fn surplus(&self) -> Option<u64> {
        self.total_liabilities()
            .and_then(|total| self.assets.checked_sub(total))
    }

    // ─── State Transitions ────────────────────────────────────────────

    /// Transition 1: Deposit.
    ///
    /// User sends `gross_amount` USDC to the contract.
    /// Deposit fee is sent to fee_recipient immediately (not held by contract).
    /// Net amount becomes a refundable deposit liability.
    ///
    /// Returns None on overflow or invalid amount.
    pub fn deposit(&self, gross_amount: u64) -> Option<TransitionResult> {
        if gross_amount == 0 {
            return None;
        }
        let deposit_fee = fees::deposit_fee(gross_amount)?;
        let net_amount = gross_amount.checked_sub(deposit_fee)?;

        let new_assets = self.assets.checked_add(net_amount)?;
        let new_refundable = self.refundable_deposit_liability.checked_add(net_amount)?;
        let new_protocol_fees = self.realized_protocol_fees.checked_add(deposit_fee)?;

        let new_state = ContractAccounting {
            assets: new_assets,
            user_note_liability: self.user_note_liability,
            refundable_deposit_liability: new_refundable,
            accrued_execution_fee_liability: self.accrued_execution_fee_liability,
            realized_protocol_fees: new_protocol_fees,
        };

        if !new_state.check_solvency() {
            return None;
        }

        Some(TransitionResult {
            new_state,
            protocol_fee_realized: deposit_fee,
            execution_fee_accrued: 0,
        })
    }

    /// Transition 2: Reveal (deposit confirmed, becomes user note).
    ///
    /// Moves the deposit from refundable to user note liability.
    /// The net_amount is the amount after deposit fee deduction.
    pub fn reveal(&self, net_amount: u64) -> Option<TransitionResult> {
        let new_refundable = self.refundable_deposit_liability.checked_sub(net_amount)?;
        let new_user_notes = self.user_note_liability.checked_add(net_amount)?;

        let new_state = ContractAccounting {
            assets: self.assets,
            user_note_liability: new_user_notes,
            refundable_deposit_liability: new_refundable,
            accrued_execution_fee_liability: self.accrued_execution_fee_liability,
            realized_protocol_fees: self.realized_protocol_fees,
        };

        if !new_state.check_solvency() {
            return None;
        }

        Some(TransitionResult {
            new_state,
            protocol_fee_realized: 0,
            execution_fee_accrued: 0,
        })
    }

    /// Transition 3: Refund (unresolved deposit returned to user).
    ///
    /// The net_amount is returned to the depositor. Both assets and
    /// refundable_deposit_liability decrease.
    pub fn refund(&self, net_amount: u64) -> Option<TransitionResult> {
        let new_assets = self.assets.checked_sub(net_amount)?;
        let new_refundable = self.refundable_deposit_liability.checked_sub(net_amount)?;

        let new_state = ContractAccounting {
            assets: new_assets,
            user_note_liability: self.user_note_liability,
            refundable_deposit_liability: new_refundable,
            accrued_execution_fee_liability: self.accrued_execution_fee_liability,
            realized_protocol_fees: self.realized_protocol_fees,
        };

        if !new_state.check_solvency() {
            return None;
        }

        Some(TransitionResult {
            new_state,
            protocol_fee_realized: 0,
            execution_fee_accrued: 0,
        })
    }

    /// Transition 4: Spend (private note consumed, payment made).
    ///
    /// The input note value is consumed. Merchant payout and protocol fee
    /// leave the contract. Execution fee stays as relayer liability.
    /// Change note (if any) is added back to user_note_liability.
    ///
    /// Invariant: input_value = merchant_amount + protocol_fee + execution_fee + change_value
    pub fn spend(
        &self,
        input_value: u64,
        merchant_amount: u64,
        protocol_fee: u64,
        execution_fee: u64,
        change_value: u64,
    ) -> Option<TransitionResult> {
        // Verify value conservation
        let total_debits = merchant_amount
            .checked_add(protocol_fee)?
            .checked_add(execution_fee)?
            .checked_add(change_value)?;
        if input_value != total_debits {
            return None; // value conservation violated
        }

        // User note liability: consume input, add change
        let new_user_notes = self
            .user_note_liability
            .checked_sub(input_value)?
            .checked_add(change_value)?;

        // Assets: merchant payout + protocol fee leave the contract
        let outflow = merchant_amount.checked_add(protocol_fee)?;
        let new_assets = self.assets.checked_sub(outflow)?;

        // Execution fee accrues as relayer liability
        let new_accrued = self
            .accrued_execution_fee_liability
            .checked_add(execution_fee)?;

        // Protocol fee is realized
        let new_protocol_fees = self.realized_protocol_fees.checked_add(protocol_fee)?;

        let new_state = ContractAccounting {
            assets: new_assets,
            user_note_liability: new_user_notes,
            refundable_deposit_liability: self.refundable_deposit_liability,
            accrued_execution_fee_liability: new_accrued,
            realized_protocol_fees: new_protocol_fees,
        };

        if !new_state.check_solvency() {
            return None;
        }

        Some(TransitionResult {
            new_state,
            protocol_fee_realized: protocol_fee,
            execution_fee_accrued: execution_fee,
        })
    }

    /// Transition 5: Claim execution fees.
    ///
    /// Relayer claims `amount` USDC from accrued execution fees.
    /// Both assets and accrued_execution_fee_liability decrease.
    ///
    /// MUST NOT touch user note backing or refundable deposits.
    pub fn claim_execution_fee(&self, amount: u64) -> Option<TransitionResult> {
        if amount == 0 {
            return None;
        }

        let new_accrued = self.accrued_execution_fee_liability.checked_sub(amount)?;
        let new_assets = self.assets.checked_sub(amount)?;

        // Safety: the claim must not reduce assets below user + refundable liabilities.
        // This prevents the relayer from claiming funds that back user notes.
        let user_plus_refundable = self
            .user_note_liability
            .checked_add(self.refundable_deposit_liability)?;
        if new_assets < user_plus_refundable {
            return None; // would break solvency for user funds
        }

        let new_state = ContractAccounting {
            assets: new_assets,
            user_note_liability: self.user_note_liability,
            refundable_deposit_liability: self.refundable_deposit_liability,
            accrued_execution_fee_liability: new_accrued,
            realized_protocol_fees: self.realized_protocol_fees,
        };

        if !new_state.check_solvency() {
            return None;
        }

        Some(TransitionResult {
            new_state,
            protocol_fee_realized: 0,
            execution_fee_accrued: 0,
        })
    }

    /// Transition 6: Failed settlement (spend reverted on-chain).
    ///
    /// The spend did not succeed. The input note is restored to user_note_liability.
    /// The execution fee that was accrued is reversed.
    ///
    /// NOTE: In the note model, the nullifier was published but the spend failed.
    /// The SDK must handle this by marking the note as "failed" and generating a
    /// new note with a fresh rho. This function restores the accounting only.
    pub fn failed_settlement(
        &self,
        input_value: u64,
        merchant_amount: u64,
        protocol_fee: u64,
        execution_fee: u64,
        change_value: u64,
    ) -> Option<TransitionResult> {
        // Reverse the spend: restore input note value to user_note_liability
        let new_user_notes = self
            .user_note_liability
            .checked_sub(change_value)? // remove the change note that was created
            .checked_add(input_value)?; // restore the original input note

        // Reverse the asset outflow (merchant + protocol fee come back)
        let inflow = merchant_amount.checked_add(protocol_fee)?;
        let new_assets = self.assets.checked_add(inflow)?;

        // Reverse the execution fee accrual
        let new_accrued = self
            .accrued_execution_fee_liability
            .checked_sub(execution_fee)?;

        // Reverse the protocol fee realization
        let new_protocol_fees = self.realized_protocol_fees.checked_sub(protocol_fee)?;

        let new_state = ContractAccounting {
            assets: new_assets,
            user_note_liability: new_user_notes,
            refundable_deposit_liability: self.refundable_deposit_liability,
            accrued_execution_fee_liability: new_accrued,
            realized_protocol_fees: new_protocol_fees,
        };

        if !new_state.check_solvency() {
            return None;
        }

        Some(TransitionResult {
            new_state,
            protocol_fee_realized: 0,
            execution_fee_accrued: 0,
        })
    }
}

impl Default for ContractAccounting {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh_state() -> ContractAccounting {
        ContractAccounting::new()
    }

    // ── Basic transitions ──────────────────────────────────────────

    #[test]
    fn test_empty_state_is_solvent() {
        let s = fresh_state();
        assert!(s.check_solvency());
        assert_eq!(s.total_liabilities(), Some(0));
        assert_eq!(s.surplus(), Some(0));
    }

    #[test]
    fn test_deposit_100_usdc() {
        let s = fresh_state();
        let r = s.deposit(100_000_000).unwrap(); // 100 USDC

        // Deposit fee = 0 (0% deposit policy)
        // Net amount = 100_000_000 (100 USDC )
        assert_eq!(r.new_state.assets, 100_000_000);
        assert_eq!(r.new_state.refundable_deposit_liability, 100_000_000);
        assert_eq!(r.new_state.user_note_liability, 0);
        assert_eq!(r.protocol_fee_realized, 0);
        assert!(r.new_state.check_solvency());
    }

    #[test]
    fn test_deposit_zero_rejected() {
        let s = fresh_state();
        assert!(s.deposit(0).is_none());
    }

    #[test]
    fn test_reveal_after_deposit() {
        let s = fresh_state();
        let r1 = s.deposit(100_000_000).unwrap();
        let r2 = r1.new_state.reveal(100_000_000).unwrap();

        assert_eq!(r2.new_state.refundable_deposit_liability, 0);
        assert_eq!(r2.new_state.user_note_liability, 100_000_000);
        assert_eq!(r2.new_state.assets, 100_000_000);
        assert!(r2.new_state.check_solvency());
    }

    #[test]
    fn test_refund_before_reveal() {
        let s = fresh_state();
        let r1 = s.deposit(100_000_000).unwrap();
        let r2 = r1.new_state.refund(100_000_000).unwrap();

        assert_eq!(r2.new_state.assets, 0);
        assert_eq!(r2.new_state.refundable_deposit_liability, 0);
        assert!(r2.new_state.check_solvency());
    }

    #[test]
    fn test_refund_after_reveal_rejected() {
        let s = fresh_state();
        let r1 = s.deposit(100_000_000).unwrap();
        let r2 = r1.new_state.reveal(100_000_000).unwrap();
        // After reveal, refundable is 0, so refund should fail
        assert!(r2.new_state.refund(100_000_000).is_none());
    }

    #[test]
    fn test_spend_after_reveal() {
        let s = fresh_state();
        let r1 = s.deposit(100_000_000).unwrap();
        let r2 = r1.new_state.reveal(100_000_000).unwrap();

        // Spend 5 USDC: merchant=5M, protocol_fee=22500 (45 bps), exec_fee=23000
        // Change = 100_000_000 - 5_000_000 - 22_500 - 23_000 = 94_954_500
        let r3 = r2
            .new_state
            .spend(100_000_000, 5_000_000, 22_500, 23_000, 94_954_500)
            .unwrap();

        assert_eq!(r3.new_state.user_note_liability, 94_954_500);
        // Assets: 100_000_000 - 5_000_000 - 22_500 = 94_977_500
        assert_eq!(r3.new_state.assets, 94_977_500);
        assert_eq!(r3.new_state.accrued_execution_fee_liability, 23_000);
        assert_eq!(r3.execution_fee_accrued, 23_000);
        assert!(r3.new_state.check_solvency());
    }

    #[test]
    fn test_spend_value_conservation_violated() {
        let s = fresh_state();
        let r1 = s.deposit(100_000_000).unwrap();
        let r2 = r1.new_state.reveal(100_000_000).unwrap();

        // Wrong change value (too high) — should be rejected
        let result = r2
            .new_state
            .spend(100_000_000, 5_000_000, 22_500, 23_000, 95_000_000);
        assert!(result.is_none(), "Value conservation must be enforced");
    }

    #[test]
    fn test_spend_value_conservation_too_low() {
        let s = fresh_state();
        let r1 = s.deposit(100_000_000).unwrap();
        let r2 = r1.new_state.reveal(100_000_000).unwrap();

        // Wrong change value (too low) — should be rejected
        let result = r2
            .new_state
            .spend(100_000_000, 5_000_000, 22_500, 23_000, 94_000_000);
        assert!(result.is_none(), "Value conservation must be enforced");
    }

    #[test]
    fn test_claim_execution_fee() {
        let s = fresh_state();
        let r1 = s.deposit(100_000_000).unwrap();
        let r2 = r1.new_state.reveal(100_000_000).unwrap();
        let r3 = r2
            .new_state
            .spend(100_000_000, 5_000_000, 22_500, 23_000, 94_954_500)
            .unwrap();
        let r4 = r3.new_state.claim_execution_fee(23_000).unwrap();

        assert_eq!(r4.new_state.accrued_execution_fee_liability, 0);
        // Assets: 94_977_500 - 23_000 = 94_954_500
        assert_eq!(r4.new_state.assets, 94_954_500);
        assert_eq!(r4.new_state.user_note_liability, 94_954_500);
        assert!(r4.new_state.check_solvency());
        assert_eq!(r4.new_state.surplus(), Some(0)); // exact balance
    }

    #[test]
    fn test_claim_more_than_accrued_rejected() {
        let s = fresh_state();
        let r1 = s.deposit(100_000_000).unwrap();
        let r2 = r1.new_state.reveal(100_000_000).unwrap();
        let r3 = r2
            .new_state
            .spend(100_000_000, 5_000_000, 22_500, 23_000, 94_954_500)
            .unwrap();
        // Try to claim more than accrued
        assert!(r3.new_state.claim_execution_fee(24_000).is_none());
    }

    #[test]
    fn test_claim_zero_rejected() {
        let s = fresh_state();
        assert!(s.claim_execution_fee(0).is_none());
    }

    #[test]
    fn test_claim_cannot_touch_user_funds() {
        // Build a valid state through normal transitions
        let s = fresh_state();
        let r1 = s.deposit(100_000_000).unwrap(); // net 100_000_000
        let r2 = r1.new_state.reveal(100_000_000).unwrap();
        let r3 = r2
            .new_state
            .spend(100_000_000, 5_000_000, 22_500, 23_000, 94_954_500)
            .unwrap();

        // State: assets=94_977_500, user_note=94_954_500, accrued=23_000
        // Claiming 23_000 leaves assets=94_954_500 == user_note (exact, valid)
        let r4 = r3.new_state.claim_execution_fee(23_000).unwrap();
        assert_eq!(r4.new_state.assets, 94_954_500);
        assert_eq!(r4.new_state.user_note_liability, 94_954_500);
        assert_eq!(r4.new_state.accrued_execution_fee_liability, 0);
        assert!(r4.new_state.check_solvency());

        // Now try to claim 1 more — nothing accrued, should fail
        assert!(r4.new_state.claim_execution_fee(1).is_none());
    }

    #[test]
    fn test_failed_settlement_reverses_spend() {
        let s = fresh_state();
        let r1 = s.deposit(100_000_000).unwrap();
        let r2 = r1.new_state.reveal(100_000_000).unwrap();
        let r3 = r2
            .new_state
            .spend(100_000_000, 5_000_000, 22_500, 23_000, 94_954_500)
            .unwrap();

        // Now the settlement fails — reverse everything
        let r4 = r3
            .new_state
            .failed_settlement(100_000_000, 5_000_000, 22_500, 23_000, 94_954_500)
            .unwrap();

        // State should be identical to before the spend
        assert_eq!(r4.new_state.user_note_liability, 100_000_000);
        assert_eq!(r4.new_state.assets, 100_000_000);
        assert_eq!(r4.new_state.accrued_execution_fee_liability, 0);
        assert!(r4.new_state.check_solvency());
    }

    // ── Multi-step scenarios ────────────────────────────────────────

    #[test]
    fn test_full_lifecycle_deposit_spend_withdraw() {
        let s = fresh_state();

        // Deposit 100 USDC
        let r1 = s.deposit(100_000_000).unwrap();
        let r2 = r1.new_state.reveal(100_000_000).unwrap();

        // Spend 5 USDC
        let r3 = r2
            .new_state
            .spend(100_000_000, 5_000_000, 22_500, 23_000, 94_954_500)
            .unwrap();

        // Spend remaining (withdraw all) — full balance spend, no change
        let r4 = r3
            .new_state
            .spend(94_954_500, 94_529_118, 425_382, 0, 0)
            .unwrap();

        assert_eq!(r4.new_state.user_note_liability, 0);
        // Assets: 94_977_500 - 94_529_118 - 425_382 = 23_000 (only exec fee remains)
        assert_eq!(r4.new_state.assets, 23_000);
        assert_eq!(r4.new_state.accrued_execution_fee_liability, 23_000);

        // Claim exec fee
        let r5 = r4.new_state.claim_execution_fee(23_000).unwrap();
        assert_eq!(r5.new_state.assets, 0);
        assert_eq!(r5.new_state.accrued_execution_fee_liability, 0);
        assert_eq!(r5.new_state.user_note_liability, 0);
        assert!(r5.new_state.check_solvency());
        assert_eq!(r5.new_state.surplus(), Some(0));
    }

    #[test]
    fn test_multiple_deposits_and_spends() {
        let s = fresh_state();

        // Deposit 1: 50 USDC
        let r1 = s.deposit(50_000_000).unwrap();
        let r2 = r1.new_state.reveal(50_000_000).unwrap();

        // Deposit 2: 30 USDC
        let r3 = r2.new_state.deposit(30_000_000).unwrap();
        let r4 = r3.new_state.reveal(30_000_000).unwrap();

        // Total user notes: 50_000_000 + 30_000_000 = 80_000_000
        assert_eq!(r4.new_state.user_note_liability, 80_000_000);
        // Total assets: 50_000_000 + 30_000_000 = 80_000_000
        assert_eq!(r4.new_state.assets, 80_000_000);
        assert!(r4.new_state.check_solvency());

        // Spend from first note: 10 USDC (fee 45 bps = 45_000)
        let r5 = r4
            .new_state
            .spend(50_000_000, 10_000_000, 45_000, 50_000, 39_905_000)
            .unwrap();

        assert_eq!(r5.new_state.user_note_liability, 39_905_000 + 30_000_000);
        assert!(r5.new_state.check_solvency());
    }

    // ── Adversarial tests ──────────────────────────────────────────

    #[test]
    fn test_adversarial_claim_before_accrual() {
        let s = fresh_state();
        let r1 = s.deposit(100_000_000).unwrap();
        let r2 = r1.new_state.reveal(100_000_000).unwrap();
        // No spend yet — nothing accrued
        assert!(r2.new_state.claim_execution_fee(1).is_none());
    }

    #[test]
    fn test_adversarial_double_claim() {
        let s = fresh_state();
        let r1 = s.deposit(100_000_000).unwrap();
        let r2 = r1.new_state.reveal(100_000_000).unwrap();
        let r3 = r2
            .new_state
            .spend(100_000_000, 5_000_000, 22_500, 23_000, 94_954_500)
            .unwrap();
        let r4 = r3.new_state.claim_execution_fee(23_000).unwrap();
        // Second claim of same amount should fail
        assert!(r4.new_state.claim_execution_fee(23_000).is_none());
    }

    #[test]
    fn test_adversarial_spend_and_claim_near_block() {
        // Spend and claim happen in close succession
        let s = fresh_state();
        let r1 = s.deposit(100_000_000).unwrap();
        let r2 = r1.new_state.reveal(100_000_000).unwrap();

        // First spend
        let r3 = r2
            .new_state
            .spend(100_000_000, 5_000_000, 22_500, 23_000, 94_954_500)
            .unwrap();

        // Claim immediately
        let r4 = r3.new_state.claim_execution_fee(23_000).unwrap();

        // Second spend from change
        let r5 = r4
            .new_state
            .spend(94_954_500, 5_000_000, 22_500, 23_000, 89_909_000)
            .unwrap();

        // Claim second exec fee
        let r6 = r5.new_state.claim_execution_fee(23_000).unwrap();

        assert!(r6.new_state.check_solvency());
        assert_eq!(r6.new_state.accrued_execution_fee_liability, 0);
    }

    #[test]
    fn test_adversarial_rounding_one_base_unit() {
        // Spend with minimal amounts — test rounding edge cases
        let s = fresh_state();
        // Deposit 1 USDC (minimum)
        let r1 = s.deposit(1_000_000).unwrap();
        // Deposit fee = 0
        // Net = 1_000_000
        assert_eq!(r1.protocol_fee_realized, 0);

        let r2 = r1.new_state.reveal(1_000_000).unwrap();

        // Spend 1 micro-USDC (the absolute minimum)
        // Protocol fee = ceil(1 * 45 / 10000) = 1
        // Exec fee = 0
        // Change = 1_000_000 - 1 - 1 - 0 = 999_998
        let r3 = r2.new_state.spend(1_000_000, 1, 1, 0, 999_998).unwrap();
        assert!(r3.new_state.check_solvency());
    }

    #[test]
    fn test_adversarial_overflow_spend() {
        let s = ContractAccounting {
            assets: u64::MAX,
            user_note_liability: u64::MAX - 1000,
            refundable_deposit_liability: 0,
            accrued_execution_fee_liability: 0,
            realized_protocol_fees: 0,
        };
        // Spend that would overflow
        assert!(s.spend(u64::MAX - 1000, u64::MAX, 0, 0, 0).is_none());
    }

    #[test]
    fn test_adversarial_fee_overflow() {
        let s = fresh_state();
        let r1 = s.deposit(100_000_000).unwrap();
        let _r2 = r1.new_state.reveal(100_000_000).unwrap();

        // Execution fee that would overflow when added to accrued
        let s2 = ContractAccounting {
            assets: u64::MAX,
            user_note_liability: 100_000_000,
            refundable_deposit_liability: 0,
            accrued_execution_fee_liability: u64::MAX - 100,
            realized_protocol_fees: 0,
        };
        assert!(s2
            .spend(100_000_000, 5_000_000, 22_500, 200, 94_977_300)
            .is_none());
    }

    #[test]
    fn test_solvency_holds_through_arbitrary_sequence() {
        // Property test: random sequence of valid transitions maintains solvency
        use ark_std::rand::rngs::StdRng;
        use ark_std::rand::{Rng, SeedableRng};

        let mut rng = StdRng::seed_from_u64(42);
        let mut state = fresh_state();

        for _ in 0..100 {
            let action = rng.gen_range(0..5);
            match action {
                0 => {
                    // Deposit random amount (10-1000 USDC)
                    let amount = rng.gen_range(10_000_000..1_000_000_000);
                    if let Some(r) = state.deposit(amount) {
                        state = r.new_state;
                    }
                }
                1 => {
                    // Reveal if there's something to reveal
                    if state.refundable_deposit_liability > 0 {
                        let amount = state.refundable_deposit_liability;
                        if let Some(r) = state.reveal(amount) {
                            state = r.new_state;
                        }
                    }
                }
                2 => {
                    // Spend if there are user notes
                    if state.user_note_liability > 5_000_000 + 12_500 + 23_000 {
                        let merchant = 5_000_000;
                        let pfee = 12_500;
                        let efee = 23_000;
                        let input = state.user_note_liability;
                        let change = input - merchant - pfee - efee;
                        if let Some(r) = state.spend(input, merchant, pfee, efee, change) {
                            state = r.new_state;
                        }
                    }
                }
                3 => {
                    // Claim if there's something to claim
                    if state.accrued_execution_fee_liability > 0 {
                        let amount = state.accrued_execution_fee_liability;
                        if let Some(r) = state.claim_execution_fee(amount) {
                            state = r.new_state;
                        }
                    }
                }
                _ => {
                    // Refund if possible
                    if state.refundable_deposit_liability > 0 {
                        let amount = state.refundable_deposit_liability;
                        if let Some(r) = state.refund(amount) {
                            state = r.new_state;
                        }
                    }
                }
            }
            assert!(
                state.check_solvency(),
                "Solvency broken after action {}",
                action
            );
        }
    }
}
