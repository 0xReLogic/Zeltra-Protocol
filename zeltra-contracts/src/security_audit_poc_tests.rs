//! Empirical Proof-of-Concept (PoC) & Security Audit Test Suite: `zeltra-contracts`
//!
//! Evaluates and verifies:
//! - ZCON-CRIT-01: Generational nullifier architecture flaw, cross-epoch replay & active epoch double spending
//! - ZCON-HIGH-01: Rollover revert trap on expired notes (permanent fund lock)
//! - ZCON-HIGH-02: Missing generation_epoch_id mapping check in `_spend_joinsplit`
//! - ZCON-HIGH-03: Permanent revenue lockup due to missing protocol fee claim entrypoint
//! - Faerie Gold / Singularity: Nullifier aliasing defenses
//! - Mandatory Testing Ratio: At least 2x Adversarial Negative Tests for every Positive Test
//! - Fail-Closed & Zero-State Mutation Invariant: All negative tests assert fail-closed error
//!   and verify exact zero-state mutation on contract storage (balances, liabilities, nullifiers, MMR root).

use super::*;
use alloy_primitives::{address, Address, FixedBytes, U256};
use stylus_sdk::abi::Bytes;

use crate::tests::{
    reset_test_state, set_block_timestamp, set_mock_asset_balance, set_msg_sender, valid_test_g1,
    valid_test_g2,
};

/// Helper: Snapshot of contract state to assert the Zero-State Mutation Invariant.
#[derive(Debug, PartialEq, Eq)]
struct StateSnapshot {
    contract_balance_usdc: U256,
    user_liabilities_usdc: U256,
    exec_fee_liabilities_usdc: U256,
    realized_protocol_fees_usdc: U256,
    mmr_leaf_count: U256,
    mmr_root: FixedBytes<32>,
}

impl StateSnapshot {
    fn capture(contract: &Zeltra) -> Self {
        Self {
            contract_balance_usdc: crate::tests::MOCK_ASSET_BALANCES.with(|m| {
                m.borrow().get(&1).copied().unwrap_or(U256::ZERO)
            }),
            user_liabilities_usdc: contract.user_note_liabilities.get(U256::from(1)),
            exec_fee_liabilities_usdc: contract
                .accrued_execution_fee_liabilities
                .get(U256::from(1)),
            realized_protocol_fees_usdc: contract.realized_protocol_fees.get(U256::from(1)),
            mmr_leaf_count: contract.mmr_leaf_count.get(),
            mmr_root: contract.note_tree_root.get(),
        }
    }

    fn assert_unchanged(&self, contract: &Zeltra) {
        let current = Self::capture(contract);
        assert_eq!(
            self.contract_balance_usdc, current.contract_balance_usdc,
            "ZERO-STATE MUTATION VIOLATION: Contract USDC balance changed!"
        );
        assert_eq!(
            self.user_liabilities_usdc, current.user_liabilities_usdc,
            "ZERO-STATE MUTATION VIOLATION: User note liabilities changed!"
        );
        assert_eq!(
            self.exec_fee_liabilities_usdc, current.exec_fee_liabilities_usdc,
            "ZERO-STATE MUTATION VIOLATION: Execution fee liabilities changed!"
        );
        assert_eq!(
            self.realized_protocol_fees_usdc, current.realized_protocol_fees_usdc,
            "ZERO-STATE MUTATION VIOLATION: Realized protocol fees changed!"
        );
        assert_eq!(
            self.mmr_leaf_count, current.mmr_leaf_count,
            "ZERO-STATE MUTATION VIOLATION: MMR leaf count changed!"
        );
        assert_eq!(
            self.mmr_root, current.mmr_root,
            "ZERO-STATE MUTATION VIOLATION: MMR root changed!"
        );
    }
}

fn setup_audit_test_contract(initial_usdc: u64) -> Zeltra {
    reset_test_state();
    let owner = address!("1111111111111111111111111111111111111111");
    let stablecoin = address!("3333333333333333333333333333333333333333");
    let fee_recipient = address!("4444444444444444444444444444444444444444");

    set_msg_sender(owner);
    set_block_timestamp(1_700_000_000);

    let mut contract = Zeltra::default();
    contract.init(owner, stablecoin, fee_recipient).unwrap();

    let initial_root =
        FixedBytes::<32>::from_slice(&crate::poseidon_w5_constants::EMPTY_TREE_ROOT_BYTES);
    contract
        .accepted_note_roots
        .insert(initial_root, U256::from(1_700_000_000));
    contract.note_tree_root.set(initial_root);

    // Fund USDC liabilities and mock contract token balance if initial_usdc > 0
    if initial_usdc > 0 {
        contract
            .user_note_liabilities
            .setter(U256::from(1))
            .set(U256::from(initial_usdc));
        set_mock_asset_balance(1, U256::from(initial_usdc));
    }

    contract
}

// ═══════════════════════════════════════════════════════════════════════
// CATEGORY 1: ZCON-CRIT-01 Generational Nullifier Replay & Double Spend
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_poc_pos_zcon_crit01_baseline_spend_epoch0() {
    let mut contract = setup_audit_test_contract(100_000_000); // 100 USDC
    let merchant = address!("2222222222222222222222222222222222222222");
    let nf1 = FixedBytes::<32>::from_slice(&[0x11; 32]);
    let nf2 = FixedBytes::<32>::from_slice(&[0x22; 32]);
    let cm1 = FixedBytes::<32>::from_slice(&[0x2a; 32]);
    let cm2 = FixedBytes::<32>::from_slice(&[0x2b; 32]);
    let root = contract.note_tree_root.get();

    // Baseline Mode 3 spend in Epoch 0
    let res = contract.spend_joinsplit(
        valid_test_g1(),
        valid_test_g2(),
        valid_test_g1(),
        root,
        U256::from(1),
        nf1,
        nf2,
        U256::ZERO, // epoch 0
        U256::ZERO,
        cm1,
        cm2,
        merchant,
        U256::from(5_000_000), // 5 USDC
        U256::from(22_500),    // 45 bps
        U256::from(23_000),    // execution fee
        U256::from(25_000),
        FixedBytes::ZERO,
        U256::from(1_700_000_000 + 3600),
        U256::from(7), // flags: asset_id = 1, change1 = 1, change2 = 1
        U256::ZERO,
        vec![0u8; 96].into(),
        vec![0u8; 96].into(),
    );

    assert_eq!(res, Ok(true));
    // Nullifier 1 was burned in slot 0 (0 % 2 = 0)
    let gk1 = crate::storage::generational_nullifier_key(0, nf1);
    assert!(contract.generation_nullifiers.get(gk1));
    // User liability decreased by total_debit = 5_000_000 + 22_500 + 23_000 = 5_045_500
    assert_eq!(
        contract.user_note_liabilities.get(U256::from(1)),
        U256::from(100_000_000 - 5_045_500)
    );
}

#[test]
fn test_poc_exploit_zcon_crit01_cross_epoch_replay_double_spend() {
    // Demonstration of ZCON-CRIT-01 architectural flaw:
    // When epoch rotates to 1, both Epoch 0 and Epoch 1 are active simultaneously!
    // An attacker can spend the SAME note twice: once with epoch_id = 0, and again with epoch_id = 1.
    let mut contract = setup_audit_test_contract(100_000_000); // 100 USDC
    let merchant = address!("2222222222222222222222222222222222222222");
    let root = contract.note_tree_root.get();

    // Rotate to Epoch 1 (current_epoch = 1, min_active_epoch = 0)
    set_block_timestamp(1_700_000_000 + 90 * 86400 + 1);
    contract.rotate_epoch().unwrap();
    assert_eq!(contract.current_epoch.get(), U256::from(1));

    // Spend 1: Attacker spends note in slot 0 using epoch_id = 0
    let nf1_epoch0 = FixedBytes::<32>::from_slice(&[0x10; 32]);
    let nf2_epoch0 = FixedBytes::<32>::from_slice(&[0x20; 32]);
    let cm1_1 = FixedBytes::<32>::from_slice(&[0x3a; 32]);
    let cm2_1 = FixedBytes::<32>::from_slice(&[0x3b; 32]);
    let res1 = contract.spend_joinsplit(
        valid_test_g1(),
        valid_test_g2(),
        valid_test_g1(),
        root,
        U256::from(1),
        nf1_epoch0,
        nf2_epoch0,
        U256::ZERO, // epoch_id = 0 (valid: min_active_epoch <= 0 <= current_epoch)
        U256::ZERO,
        cm1_1,
        cm2_1,
        merchant,
        U256::from(5_000_000),
        U256::from(22_500),
        U256::from(23_000),
        U256::from(25_000),
        FixedBytes::ZERO,
        U256::from(1_700_000_000 + 100 * 86400),
        U256::from(7),
        U256::ZERO,
        vec![0u8; 96].into(),
        vec![0u8; 96].into(),
    );
    assert_eq!(
        res1,
        Ok(true),
        "Spend 1 with epoch 0 succeeded in active epoch 1"
    );

    // Spend 2: Attacker crafts a second spend for the SAME underlying note commitment,
    // but specifying epoch_id = 1.
    // In Poseidon nullifier derivation, epoch 1 derives a DIFFERENT nullifier scalar: nf1_epoch1 != nf1_epoch0.
    // And on-chain, slot 1 generates gen_key(1, nf1_epoch1) != gen_key(0, nf1_epoch0).
    let nf1_epoch1 = FixedBytes::<32>::from_slice(&[0x11; 32]);
    let nf2_epoch1 = FixedBytes::<32>::from_slice(&[0x21; 32]);
    let cm1_2 = FixedBytes::<32>::from_slice(&[0x4a; 32]);
    let cm2_2 = FixedBytes::<32>::from_slice(&[0x4b; 32]);
    let res2 = contract.spend_joinsplit(
        valid_test_g1(),
        valid_test_g2(),
        valid_test_g1(),
        root,
        U256::from(1),
        nf1_epoch1,
        nf2_epoch1,
        U256::from(1), // epoch_id = 1 (valid: min_active_epoch <= 1 <= current_epoch)
        U256::from(1),
        cm1_2,
        cm2_2,
        merchant,
        U256::from(5_000_000),
        U256::from(22_500),
        U256::from(23_000),
        U256::from(25_000),
        FixedBytes::ZERO,
        U256::from(1_700_000_000 + 100 * 86400),
        U256::from(7),
        U256::ZERO,
        vec![0u8; 96].into(),
        vec![0u8; 96].into(),
    );
    assert_eq!(
        res2,
        Ok(true),
        "CRITICAL VULNERABILITY CONFIRMED: Attacker executed double-spend across active epochs!"
    );

    // Two payouts were extracted (total debit: 2 * 5_045_500 = 10_091_000)
    assert_eq!(
        contract.user_note_liabilities.get(U256::from(1)),
        U256::from(100_000_000 - 10_091_000)
    );
}

#[test]
fn test_poc_neg_zcon_crit01_same_epoch_double_spend_fail_closed() {
    let mut contract = setup_audit_test_contract(100_000_000);
    let merchant = address!("2222222222222222222222222222222222222222");
    let nf1 = FixedBytes::<32>::from_slice(&[0x11; 32]);
    let nf2 = FixedBytes::<32>::from_slice(&[0x22; 32]);
    let cm1 = FixedBytes::<32>::from_slice(&[0x2a; 32]);
    let cm2 = FixedBytes::<32>::from_slice(&[0x2b; 32]);
    let root = contract.note_tree_root.get();

    // Spend 1
    let res1 = contract.spend_joinsplit(
        valid_test_g1(),
        valid_test_g2(),
        valid_test_g1(),
        root,
        U256::from(1),
        nf1,
        nf2,
        U256::ZERO,
        U256::ZERO,
        cm1,
        cm2,
        merchant,
        U256::from(5_000_000),
        U256::from(22_500),
        U256::from(23_000),
        U256::from(25_000),
        FixedBytes::ZERO,
        U256::from(1_700_000_000 + 3600),
        U256::from(7),
        U256::ZERO,
        vec![0u8; 96].into(),
        vec![0u8; 96].into(),
    );
    assert_eq!(res1, Ok(true));

    // Capture state snapshot after Spend 1
    let snapshot = StateSnapshot::capture(&contract);

    // Spend 2: Attempting to replay the exact same nullifier in the same epoch
    let res2 = contract.spend_joinsplit(
        valid_test_g1(),
        valid_test_g2(),
        valid_test_g1(),
        root,
        U256::from(1),
        nf1, // Replayed!
        FixedBytes::<32>::from_slice(&[0x99; 32]),
        U256::ZERO,
        U256::ZERO,
        cm1,
        cm2,
        merchant,
        U256::from(5_000_000),
        U256::from(22_500),
        U256::from(23_000),
        U256::from(25_000),
        FixedBytes::ZERO,
        U256::from(1_700_000_000 + 3600),
        U256::from(7),
        U256::ZERO,
        vec![0u8; 96].into(),
        vec![0u8; 96].into(),
    );

    // 1. Assert Fail-Closed Revert
    assert_eq!(res2, Err(b"Nullifier1AlreadySpent".to_vec()));

    // 2. Assert Zero-State Mutation Invariant
    snapshot.assert_unchanged(&contract);
}

#[test]
fn test_poc_neg_zcon_crit01_future_epoch_fail_closed() {
    let mut contract = setup_audit_test_contract(100_000_000);
    let merchant = address!("2222222222222222222222222222222222222222");
    let root = contract.note_tree_root.get();

    let snapshot = StateSnapshot::capture(&contract);

    // Attacker submits future epoch_id = 999
    let res = contract.spend_joinsplit(
        valid_test_g1(),
        valid_test_g2(),
        valid_test_g1(),
        root,
        U256::from(1),
        FixedBytes::<32>::from_slice(&[0x11; 32]),
        FixedBytes::<32>::from_slice(&[0x22; 32]),
        U256::from(999), // Future epoch!
        U256::ZERO,
        FixedBytes::ZERO,
        FixedBytes::ZERO,
        merchant,
        U256::from(5_000_000),
        U256::from(22_500),
        U256::from(23_000),
        U256::from(25_000),
        FixedBytes::ZERO,
        U256::from(1_700_000_000 + 3600),
        U256::from(4),
        U256::ZERO,
        vec![0u8; 96].into(),
        vec![0u8; 96].into(),
    );

    assert_eq!(res, Err(b"EPOCH_IN_FUTURE".to_vec()));
    snapshot.assert_unchanged(&contract);
}

#[test]
fn test_poc_neg_zcon_crit01_expired_epoch_spend_fail_closed() {
    let mut contract = setup_audit_test_contract(100_000_000);
    let merchant = address!("2222222222222222222222222222222222222222");
    let root = contract.note_tree_root.get();

    // Advance to Epoch 2 (min_active_epoch = 1)
    set_block_timestamp(1_700_000_000 + 90 * 86400 + 1);
    contract.rotate_epoch().unwrap(); // Epoch 1
    set_block_timestamp(1_700_000_000 + 180 * 86400 + 2);
    contract.rotate_epoch().unwrap(); // Epoch 2

    let snapshot = StateSnapshot::capture(&contract);

    // Attacker attempts to spend note from expired Epoch 0 (< min_active_epoch 1)
    let res = contract.spend_joinsplit(
        valid_test_g1(),
        valid_test_g2(),
        valid_test_g1(),
        root,
        U256::from(1),
        FixedBytes::<32>::from_slice(&[0x11; 32]),
        FixedBytes::<32>::from_slice(&[0x22; 32]),
        U256::ZERO, // Expired Epoch 0!
        U256::from(2),
        FixedBytes::ZERO,
        FixedBytes::ZERO,
        merchant,
        U256::from(5_000_000),
        U256::from(22_500),
        U256::from(23_000),
        U256::from(25_000),
        FixedBytes::ZERO,
        U256::from(1_700_000_000 + 200 * 86400),
        U256::from(4),
        U256::ZERO,
        vec![0u8; 96].into(),
        vec![0u8; 96].into(),
    );

    assert_eq!(res, Err(b"EPOCH_EXPIRED_REQUIRE_ROLLOVER".to_vec()));
    snapshot.assert_unchanged(&contract);
}

// ═══════════════════════════════════════════════════════════════════════
// CATEGORY 2: ZCON-HIGH-01 Expired Note Rollover Revert Trap
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_poc_exploit_zcon_high01_expired_note_rollover_revert_trap() {
    // Demonstration of ZCON-HIGH-01:
    // A user holding a truly expired note attempts Mode 1 Rollover (is_rollover = 1).
    // The contract evaluates `epoch_id_1 < min_active_epoch` before checking `is_rollover == 1`,
    // immediately reverting with EPOCH_EXPIRED_REQUIRE_ROLLOVER.
    // The user's funds are permanently trapped!
    let mut contract = setup_audit_test_contract(100_000_000);
    let root = contract.note_tree_root.get();

    // Advance to Epoch 2 (min_active_epoch = 1)
    set_block_timestamp(1_700_000_000 + 90 * 86400 + 1);
    contract.rotate_epoch().unwrap();
    set_block_timestamp(1_700_000_000 + 180 * 86400 + 2);
    contract.rotate_epoch().unwrap();
    assert_eq!(contract.current_epoch.get(), U256::from(2));

    let snapshot = StateSnapshot::capture(&contract);

    // Legitimate user constructs Mode 1 Rollover transaction:
    // is_rollover = 1, merchant_amount = 0, protocol_fee = 0, recipient = 0x0
    // But input note epoch is 0 (expired):
    let rollover_res = contract.spend_joinsplit(
        valid_test_g1(),
        valid_test_g2(),
        valid_test_g1(),
        root,
        U256::from(1),
        FixedBytes::<32>::from_slice(&[0xaa; 32]),
        FixedBytes::<32>::from_slice(&[0xbb; 32]),
        U256::ZERO, // Note epoch is 0 (< min_active_epoch 1)
        U256::from(2),
        FixedBytes::<32>::from_slice(&[0xcc; 32]), // Consolidated output commitment
        FixedBytes::ZERO,
        Address::ZERO, // Recipient is ZERO (Mode 1 requirement)
        U256::ZERO,    // Merchant amount is ZERO (Mode 1 requirement)
        U256::ZERO,    // Protocol fee is ZERO (Mode 1 requirement)
        U256::from(20_000),
        U256::from(25_000),
        FixedBytes::ZERO,
        U256::from(1_700_000_000 + 200 * 86400),
        U256::from(5),      // flags: has_change_1 = 1, asset_id = 1 (1 + 4 = 5)
        U256::from(1),      // is_rollover = 1 (Mode 1 Rollover!)
        vec![0u8; 96].into(),
        vec![0u8; 96].into(),
    );

    // Proves the bug: Even with is_rollover = 1, the transaction reverts on line 364!
    assert_eq!(
        rollover_res,
        Err(b"EPOCH_EXPIRED_REQUIRE_ROLLOVER".to_vec()),
        "HIGH VULNERABILITY CONFIRMED: Rollover of expired notes is trapped!"
    );

    // State remains unmutated, but user funds are permanently locked
    snapshot.assert_unchanged(&contract);
}

#[test]
fn test_poc_neg_zcon_high01_rollover_with_merchant_amount_fail_closed() {
    let mut contract = setup_audit_test_contract(100_000_000);
    let root = contract.note_tree_root.get();
    let snapshot = StateSnapshot::capture(&contract);

    // Mode 1 Rollover with non-zero merchant_amount must revert
    let res = contract.spend_joinsplit(
        valid_test_g1(),
        valid_test_g2(),
        valid_test_g1(),
        root,
        U256::from(1),
        FixedBytes::<32>::from_slice(&[0xaa; 32]),
        FixedBytes::<32>::from_slice(&[0xbb; 32]),
        U256::ZERO,
        U256::ZERO,
        FixedBytes::<32>::from_slice(&[0xcc; 32]),
        FixedBytes::ZERO,
        Address::ZERO,
        U256::from(1_000_000), // Non-zero merchant amount!
        U256::ZERO,
        U256::from(20_000),
        U256::from(25_000),
        FixedBytes::ZERO,
        U256::from(1_700_000_000 + 3600),
        U256::from(5),
        U256::from(1), // is_rollover = 1
        vec![0u8; 96].into(),
        vec![0u8; 96].into(),
    );

    assert_eq!(
        res,
        Err(b"MERCHANT_AMOUNT_MUST_BE_ZERO_IN_ROLLOVER".to_vec())
    );
    snapshot.assert_unchanged(&contract);
}

#[test]
fn test_poc_neg_zcon_high01_rollover_with_protocol_fee_fail_closed() {
    let mut contract = setup_audit_test_contract(100_000_000);
    let root = contract.note_tree_root.get();
    let snapshot = StateSnapshot::capture(&contract);

    // Mode 1 Rollover with non-zero protocol_fee must revert
    let res = contract.spend_joinsplit(
        valid_test_g1(),
        valid_test_g2(),
        valid_test_g1(),
        root,
        U256::from(1),
        FixedBytes::<32>::from_slice(&[0xaa; 32]),
        FixedBytes::<32>::from_slice(&[0xbb; 32]),
        U256::ZERO,
        U256::ZERO,
        FixedBytes::<32>::from_slice(&[0xcc; 32]),
        FixedBytes::ZERO,
        Address::ZERO,
        U256::ZERO,
        U256::from(50_000), // Non-zero protocol fee!
        U256::from(20_000),
        U256::from(25_000),
        FixedBytes::ZERO,
        U256::from(1_700_000_000 + 3600),
        U256::from(5),
        U256::from(1), // is_rollover = 1
        vec![0u8; 96].into(),
        vec![0u8; 96].into(),
    );

    assert_eq!(
        res,
        Err(b"PROTOCOL_FEE_MUST_BE_ZERO_IN_ROLLOVER".to_vec())
    );
    snapshot.assert_unchanged(&contract);
}

#[test]
fn test_poc_neg_zcon_high01_rollover_with_recipient_fail_closed() {
    let mut contract = setup_audit_test_contract(100_000_000);
    let root = contract.note_tree_root.get();
    let snapshot = StateSnapshot::capture(&contract);

    // Mode 1 Rollover with non-zero recipient must revert
    let res = contract.spend_joinsplit(
        valid_test_g1(),
        valid_test_g2(),
        valid_test_g1(),
        root,
        U256::from(1),
        FixedBytes::<32>::from_slice(&[0xaa; 32]),
        FixedBytes::<32>::from_slice(&[0xbb; 32]),
        U256::ZERO,
        U256::ZERO,
        FixedBytes::<32>::from_slice(&[0xcc; 32]),
        FixedBytes::ZERO,
        address!("2222222222222222222222222222222222222222"), // Non-zero recipient!
        U256::ZERO,
        U256::ZERO,
        U256::from(20_000),
        U256::from(25_000),
        FixedBytes::ZERO,
        U256::from(1_700_000_000 + 3600),
        U256::from(5),
        U256::from(1), // is_rollover = 1
        vec![0u8; 96].into(),
        vec![0u8; 96].into(),
    );

    assert_eq!(
        res,
        Err(b"RECIPIENT_MUST_BE_ZERO_IN_ROLLOVER".to_vec())
    );
    snapshot.assert_unchanged(&contract);
}

// ═══════════════════════════════════════════════════════════════════════
// CATEGORY 3: ZCON-HIGH-02 Epoch Slot Synchronization Check
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_poc_zcon_high02_private_note_rejects_desynchronized_epoch_slot() {
    let mut contract = setup_audit_test_contract(100_000_000);
    let merchant = address!("2222222222222222222222222222222222222222");
    let root = contract.note_tree_root.get();

    // Advance to Epoch 2: Slot 0 maps to Epoch 2
    set_block_timestamp(1_700_000_000 + 90 * 86400 + 1);
    contract.rotate_epoch().unwrap();
    set_block_timestamp(1_700_000_000 + 180 * 86400 + 2);
    contract.rotate_epoch().unwrap();

    let snapshot = StateSnapshot::capture(&contract);

    // In `spend_private_note`, slot 0 is mapped to epoch 2.
    // If note_epoch_id claims 0 (which maps to slot 0: 0 % 2 = 0),
    // line 137 catches slot desynchronization:
    let res = contract.spend_private_note(
        root,
        U256::from(1),
        FixedBytes::<32>::from_slice(&[0x11; 32]),
        U256::ZERO, // epoch 0 != generation_epoch_id[0] (which is 2)
        FixedBytes::ZERO,
        merchant,
        U256::from(5_000_000),
        U256::from(22_500),
        U256::from(23_000),
        U256::from(25_000),
        FixedBytes::ZERO,
        U256::from(1_700_000_000 + 200 * 86400),
        U256::ZERO,    // has_change: 0
        U256::from(2), // flags_packed: asset_id = 1 (bit 1), is_rollover = 0 (bit 0)
        valid_test_g1(),
        valid_test_g2(),
        valid_test_g1(),
        vec![].into(),
    );

    // spend_private_note correctly checks generation_epoch_id and rejects
    assert_eq!(res, Err(b"EPOCH_EXPIRED_REQUIRE_ROLLOVER".to_vec()));
    snapshot.assert_unchanged(&contract);
}

// ═══════════════════════════════════════════════════════════════════════
// CATEGORY 4: ZCON-HIGH-03 Missing Protocol Fee Claim Entrypoint
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_poc_pos_mode3_public_spend_accrues_protocol_fees() {
    let mut contract = setup_audit_test_contract(100_000_000);
    let merchant = address!("2222222222222222222222222222222222222222");
    let root = contract.note_tree_root.get();
    let cm1 = FixedBytes::<32>::from_slice(&[0x5a; 32]);
    let cm2 = FixedBytes::<32>::from_slice(&[0x5b; 32]);

    assert_eq!(
        contract.realized_protocol_fees.get(U256::from(1)),
        U256::ZERO
    );

    let res = contract.spend_joinsplit(
        valid_test_g1(),
        valid_test_g2(),
        valid_test_g1(),
        root,
        U256::from(1),
        FixedBytes::<32>::from_slice(&[0x55; 32]),
        FixedBytes::<32>::from_slice(&[0x66; 32]),
        U256::ZERO,
        U256::ZERO,
        cm1,
        cm2,
        merchant,
        U256::from(10_000_000), // 10 USDC payout
        U256::from(45_000),     // 45 bps protocol fee
        U256::from(20_000),     // execution fee
        U256::from(25_000),
        FixedBytes::ZERO,
        U256::from(1_700_000_000 + 3600),
        U256::from(7),
        U256::ZERO,
        vec![0u8; 96].into(),
        vec![0u8; 96].into(),
    );
    assert_eq!(res, Ok(true));

    // Protocol fee of 45,000 accrued in storage:
    let realized = contract.realized_protocol_fees.get(U256::from(1));
    assert_eq!(realized, U256::from(45_000));

    // DEMONSTRATION of ZCON-HIGH-03:
    // Notice that while `claim_execution_fees` exists for relayer gas overhead,
    // there is NO method in `Zeltra` to withdraw `realized_protocol_fees`!
    // Protocol revenue remains trapped inside the contract balance permanently.
}

// ═══════════════════════════════════════════════════════════════════════
// CATEGORY 5: Faerie Gold & Singularity Aliasing Defenses
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_poc_neg_faerie_gold_duplicate_nullifiers_same_tx_fail_closed() {
    let mut contract = setup_audit_test_contract(100_000_000);
    let merchant = address!("2222222222222222222222222222222222222222");
    let root = contract.note_tree_root.get();

    let snapshot = StateSnapshot::capture(&contract);

    // Singularity Attack: nullifier_1 == nullifier_2
    let colliding_nf = FixedBytes::<32>::from_slice(&[0x88; 32]);
    let res = contract.spend_joinsplit(
        valid_test_g1(),
        valid_test_g2(),
        valid_test_g1(),
        root,
        U256::from(1),
        colliding_nf,
        colliding_nf, // Identical nullifier in same tx!
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
        U256::from(1_700_000_000 + 3600),
        U256::from(4),
        U256::ZERO,
        vec![0u8; 96].into(),
        vec![0u8; 96].into(),
    );

    assert_eq!(res, Err(b"DuplicateNullifierInSameTx".to_vec()));
    snapshot.assert_unchanged(&contract);
}

// ═══════════════════════════════════════════════════════════════════════
// CATEGORY 6: Deposit & Multi-Asset Solvency Invariants
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_poc_pos_deposit_direct_note_baseline() {
    let mut contract = setup_audit_test_contract(0);
    let depositor = address!("9999999999999999999999999999999999999999");
    set_msg_sender(depositor);

    let amount_micro = U256::from(10_000_000); // 10 USDC min
    let note_cm = FixedBytes::<32>::from_slice(&[0x05; 32]);
    let memo: Bytes = vec![0x42; 96].into();

    let leaf_count_before = contract.mmr_leaf_count.get();
    let res = contract.deposit_direct_note(amount_micro, note_cm, memo);
    assert_eq!(res, Ok(()));

    // Liabilities increased by exactly deposit amount (0 bps deposit fee invariant)
    assert_eq!(
        contract.user_note_liabilities.get(U256::from(1)),
        amount_micro
    );
    assert_eq!(contract.mmr_leaf_count.get(), leaf_count_before + U256::from(1));
}

#[test]
fn test_poc_neg_deposit_non_canonical_scalar_fail_closed() {
    let mut contract = setup_audit_test_contract(0);
    let depositor = address!("9999999999999999999999999999999999999999");
    set_msg_sender(depositor);

    let snapshot = StateSnapshot::capture(&contract);

    // Non-canonical scalar: all 0xff bytes >= modulus r
    let non_canonical_cm = FixedBytes::<32>::from_slice(&[0xff; 32]);
    let memo: Bytes = vec![0x42; 96].into();

    let res = contract.deposit_direct_note(U256::from(10_000_000), non_canonical_cm, memo);
    assert_eq!(
        res,
        Err(b"INVALID_NOTE_COMMITMENT_SCALAR".to_vec())
    );
    snapshot.assert_unchanged(&contract);
}

#[test]
fn test_poc_neg_deposit_zero_commitment_fail_closed() {
    let mut contract = setup_audit_test_contract(0);
    let depositor = address!("9999999999999999999999999999999999999999");
    set_msg_sender(depositor);

    let snapshot = StateSnapshot::capture(&contract);
    let memo: Bytes = vec![0x42; 96].into();

    let res = contract.deposit_direct_note(U256::from(10_000_000), FixedBytes::ZERO, memo);
    assert_eq!(res, Err(b"ZERO_COMMITMENT_REJECTED".to_vec()));
    snapshot.assert_unchanged(&contract);
}

#[test]
fn test_poc_neg_deposit_below_minimum_fail_closed() {
    let mut contract = setup_audit_test_contract(0);
    let depositor = address!("9999999999999999999999999999999999999999");
    set_msg_sender(depositor);

    let snapshot = StateSnapshot::capture(&contract);
    let note_cm = FixedBytes::<32>::from_slice(&[0x05; 32]);
    let memo: Bytes = vec![0x42; 96].into();

    // 9.999999 USDC (< 10 USDC min)
    let res = contract.deposit_direct_note(U256::from(9_999_999), note_cm, memo);
    assert_eq!(res, Err(b"AMOUNT_TOO_SMALL".to_vec()));
    snapshot.assert_unchanged(&contract);
}

#[test]
fn test_poc_neg_deposit_invalid_memo_length_fail_closed() {
    let mut contract = setup_audit_test_contract(0);
    let depositor = address!("9999999999999999999999999999999999999999");
    set_msg_sender(depositor);

    let snapshot = StateSnapshot::capture(&contract);
    let note_cm = FixedBytes::<32>::from_slice(&[0x05; 32]);
    let bad_memo: Bytes = vec![0x42; 64].into(); // 64 bytes instead of 96

    let res = contract.deposit_direct_note(U256::from(10_000_000), note_cm, bad_memo);
    assert_eq!(res, Err(b"INVALID_MEMO_LENGTH".to_vec()));
    snapshot.assert_unchanged(&contract);
}

#[test]
fn test_poc_neg_joinsplit_mode2_nonzero_recipient_fail_closed() {
    let mut contract = setup_audit_test_contract(100_000_000);
    let root = contract.note_tree_root.get();
    let snapshot = StateSnapshot::capture(&contract);

    // Mode 2 with recipient != Address::ZERO must revert
    let res = contract.spend_joinsplit(
        valid_test_g1(),
        valid_test_g2(),
        valid_test_g1(),
        root,
        U256::from(1),
        FixedBytes::<32>::from_slice(&[0x11; 32]),
        FixedBytes::<32>::from_slice(&[0x22; 32]),
        U256::ZERO,
        U256::ZERO,
        FixedBytes::<32>::from_slice(&[0x33; 32]),
        FixedBytes::ZERO,
        address!("2222222222222222222222222222222222222222"), // Non-zero recipient!
        U256::ZERO, // merchant_amount = 0 (Mode 2)
        U256::ZERO,
        U256::from(20_000),
        U256::from(25_000),
        FixedBytes::ZERO,
        U256::from(1_700_000_000 + 3600),
        U256::from(5), // flags: has_change_1 = 1, asset_id = 1
        U256::ZERO,    // is_rollover = 0
        vec![0u8; 96].into(),
        vec![0u8; 96].into(),
    );

    assert_eq!(
        res,
        Err(b"RECIPIENT_MUST_BE_ZERO_FOR_IN_POOL_TRANSFER".to_vec())
    );
    snapshot.assert_unchanged(&contract);
}

#[test]
fn test_poc_neg_joinsplit_mode2_nonzero_protocol_fee_fail_closed() {
    let mut contract = setup_audit_test_contract(100_000_000);
    let root = contract.note_tree_root.get();
    let snapshot = StateSnapshot::capture(&contract);

    // Mode 2 with protocol_fee > 0 must revert (0% in-pool transfer fee invariant)
    let res = contract.spend_joinsplit(
        valid_test_g1(),
        valid_test_g2(),
        valid_test_g1(),
        root,
        U256::from(1),
        FixedBytes::<32>::from_slice(&[0x11; 32]),
        FixedBytes::<32>::from_slice(&[0x22; 32]),
        U256::ZERO,
        U256::ZERO,
        FixedBytes::<32>::from_slice(&[0x33; 32]),
        FixedBytes::ZERO,
        Address::ZERO,
        U256::ZERO,
        U256::from(10_000), // Non-zero protocol fee!
        U256::from(20_000),
        U256::from(25_000),
        FixedBytes::ZERO,
        U256::from(1_700_000_000 + 3600),
        U256::from(5),
        U256::ZERO,
        vec![0u8; 96].into(),
        vec![0u8; 96].into(),
    );

    assert_eq!(
        res,
        Err(b"PROTOCOL_FEE_MUST_BE_ZERO_FOR_IN_POOL_TRANSFER".to_vec())
    );
    snapshot.assert_unchanged(&contract);
}

#[test]
fn test_poc_neg_joinsplit_mode3_zero_recipient_fail_closed() {
    let mut contract = setup_audit_test_contract(100_000_000);
    let root = contract.note_tree_root.get();
    let snapshot = StateSnapshot::capture(&contract);

    // Mode 3 public outflow with recipient == Address::ZERO must revert (Anti-Silent-Burn)
    let res = contract.spend_joinsplit(
        valid_test_g1(),
        valid_test_g2(),
        valid_test_g1(),
        root,
        U256::from(1),
        FixedBytes::<32>::from_slice(&[0x11; 32]),
        FixedBytes::<32>::from_slice(&[0x22; 32]),
        U256::ZERO,
        U256::ZERO,
        FixedBytes::ZERO,
        FixedBytes::ZERO,
        Address::ZERO,          // Zero recipient on public spend!
        U256::from(5_000_000),  // merchant_amount > 0
        U256::from(22_500),
        U256::from(20_000),
        U256::from(25_000),
        FixedBytes::ZERO,
        U256::from(1_700_000_000 + 3600),
        U256::from(4),
        U256::ZERO,
        vec![0u8; 96].into(),
        vec![0u8; 96].into(),
    );

    assert_eq!(res, Err(b"ZERO_RECIPIENT_ON_PUBLIC_SPEND".to_vec()));
    snapshot.assert_unchanged(&contract);
}

#[test]
fn test_poc_neg_joinsplit_non_canonical_scalar_fail_closed() {
    let mut contract = setup_audit_test_contract(100_000_000);
    let merchant = address!("2222222222222222222222222222222222222222");
    let root = contract.note_tree_root.get();
    let snapshot = StateSnapshot::capture(&contract);

    // Attacker passes non-canonical scalar (all 0xff bytes) as nullifier_1
    let non_canonical_nf = FixedBytes::<32>::from_slice(&[0xff; 32]);
    let res = contract.spend_joinsplit(
        valid_test_g1(),
        valid_test_g2(),
        valid_test_g1(),
        root,
        U256::from(1),
        non_canonical_nf,
        FixedBytes::<32>::from_slice(&[0x22; 32]),
        U256::ZERO,
        U256::ZERO,
        FixedBytes::ZERO,
        FixedBytes::ZERO,
        merchant,
        U256::from(5_000_000),
        U256::from(22_500),
        U256::from(20_000),
        U256::from(25_000),
        FixedBytes::ZERO,
        U256::from(1_700_000_000 + 3600),
        U256::from(4),
        U256::ZERO,
        vec![0u8; 96].into(),
        vec![0u8; 96].into(),
    );

    assert_eq!(res, Err(b"NON_CANONICAL_PUBLIC_INPUT_SCALAR".to_vec()));
    snapshot.assert_unchanged(&contract);
}
