//! DEC-054 Zero-Custody Safe Harbor and Direct Self-Broadcast Test Suite
//!
//! Covers:
//! - Gate 1: Happy-path Direct Self-Broadcast Spend (Mode 3, execution_fee = 0, full ABI encoding)
//! - Gate 2.1: Adversarial rejection of non-zero execution fee without relayer
//! - Gate 2.2: Adversarial rejection of recipient address tampering
//! - Gate 2.3: Adversarial rejection of merchant amount tampering
//! - Gate 2.4: Mode 2 Direct Self-Broadcast Shielded In-Pool Transfer (zero public movement)

use alloy_primitives::Address;
use alloy_sol_types::SolCall;
use std::str::FromStr;
use zeltra_core::generate_joinsplit_circuit_keys;
use zeltra_sdk::wallet::note_wallet::self_broadcast::spendJoinsplitCall;
use zeltra_sdk::wallet::note_wallet::PrivateNoteWallet;

fn dummy_merkle_path() -> Vec<String> {
    vec!["0x04".to_string()]
}

#[test]
fn test_gate1_self_broadcast_joinsplit_zero_fee_happy_path() {
    let seed = [77u8; 32];
    let mut wallet = PrivateNoteWallet::new(&seed);
    let path = dummy_merkle_path();

    // 1. User deposits 100 USDC ($100.00 = 100_000_000 Micro-USDC)
    let (_, cm1) = wallet.create_deposit_note(100_000_000, 1000).unwrap();
    wallet
        .confirm_deposit(
            &cm1,
            0,
            path,
            "0x0000000000000000000000000000000000000000000000000000000000000001",
        )
        .unwrap();

    // 2. Generate Universal JoinSplit proving key
    let keys = generate_joinsplit_circuit_keys().unwrap();

    // 3. User prepares direct self-broadcast spend of 30 USDC ($30.00) with zero relayer fee
    let recipient_evm = "0x1111111111111111111111111111111111111111";
    let contract_addr = "0x2222222222222222222222222222222222222222";
    let chain_id = 421614u64; // Arbitrum Sepolia
    let expiry = 2000u64;
    let now = 1010u64;
    let spend_amount = 30_000_000; // 30 USDC

    let payload = wallet
        .prepare_self_broadcast_joinsplit(
            recipient_evm,
            spend_amount,
            chain_id,
            contract_addr,
            expiry,
            now,
            &keys.proving_key,
        )
        .expect("Self-broadcast payload preparation must succeed");

    // 4. Verify DEC-054 Invariants
    assert_eq!(
        payload.execution_fee, 0,
        "Execution fee MUST be strictly 0 in self-broadcast"
    );
    assert_eq!(
        payload.max_execution_fee, 0,
        "Max execution fee MUST be strictly 0 in self-broadcast"
    );
    assert_eq!(payload.merchant_amount, 30_000_000);
    // Protocol monetization: flat 45 bps = (30_000_000 * 45).div_ceil(10_000) = 135_000
    assert_eq!(payload.protocol_fee, 135_000);
    assert_eq!(
        payload.recipient.to_lowercase(),
        recipient_evm.to_lowercase()
    );
    assert_eq!(payload.asset_id, 1); // USDC

    // 5. Verify Calldata formatting & 4-byte selector
    assert!(payload.calldata_hex.starts_with("0x"));
    assert_eq!(
        payload.calldata_bytes.len(),
        payload.calldata_hex.len() / 2 - 1
    );
    assert!(payload.calldata_bytes.len() >= 4);

    // SpendJoinsplit selector is first 4 bytes of abi_encode
    let decoded_call = spendJoinsplitCall::abi_decode(&payload.calldata_bytes)
        .expect("ABI calldata must decode cleanly into spendJoinsplitCall");
    assert_eq!(
        decoded_call.recipient,
        Address::from_str(recipient_evm).unwrap()
    );
    assert_eq!(decoded_call.merchant_amount.to::<u64>(), 30_000_000);
    assert_eq!(decoded_call.execution_fee.to::<u64>(), 0);
    assert_eq!(decoded_call.max_execution_fee.to::<u64>(), 0);

    // 6. Verify payload integrity
    assert!(payload.validate_integrity().is_ok());
}

#[test]
fn test_gate2_1_adversarial_rejection_nonzero_execution_fee() {
    let seed = [88u8; 32];
    let mut wallet = PrivateNoteWallet::new(&seed);
    let path = dummy_merkle_path();

    let (_, cm1) = wallet.create_deposit_note(50_000_000, 1000).unwrap();
    wallet.confirm_deposit(&cm1, 0, path, "0x01").unwrap();

    let keys = generate_joinsplit_circuit_keys().unwrap();
    let mut payload = wallet
        .prepare_self_broadcast_joinsplit(
            "0x1111111111111111111111111111111111111111",
            10_000_000,
            421614,
            "0x2222222222222222222222222222222222222222",
            2000,
            1010,
            &keys.proving_key,
        )
        .unwrap();

    // Adversarial tampering: inject execution fee without relayer
    payload.execution_fee = 23_000;
    let err = payload.validate_integrity().unwrap_err();
    assert!(err.to_string().contains("ADVERSARIAL_TAMPERING_DETECTED"));
    assert!(err.to_string().contains("Non-zero execution fee claimed"));
}

#[test]
fn test_gate2_2_adversarial_rejection_recipient_tampering() {
    let seed = [99u8; 32];
    let mut wallet = PrivateNoteWallet::new(&seed);
    let path = dummy_merkle_path();

    let (_, cm1) = wallet.create_deposit_note(50_000_000, 1000).unwrap();
    wallet.confirm_deposit(&cm1, 0, path, "0x01").unwrap();

    let keys = generate_joinsplit_circuit_keys().unwrap();
    let mut payload = wallet
        .prepare_self_broadcast_joinsplit(
            "0x1111111111111111111111111111111111111111",
            10_000_000,
            421614,
            "0x2222222222222222222222222222222222222222",
            2000,
            1010,
            &keys.proving_key,
        )
        .unwrap();

    // Adversarial tampering: switch recipient in outer envelope
    payload.recipient = "0x9999999999999999999999999999999999999999".to_string();
    let err = payload.validate_integrity().unwrap_err();
    assert!(err.to_string().contains("ADVERSARIAL_TAMPERING_DETECTED"));
    assert!(err.to_string().contains("Recipient address mismatch"));
}

#[test]
fn test_gate2_3_adversarial_rejection_amount_tampering() {
    let seed = [111u8; 32];
    let mut wallet = PrivateNoteWallet::new(&seed);
    let path = dummy_merkle_path();

    let (_, cm1) = wallet.create_deposit_note(50_000_000, 1000).unwrap();
    wallet.confirm_deposit(&cm1, 0, path, "0x01").unwrap();

    let keys = generate_joinsplit_circuit_keys().unwrap();
    let mut payload = wallet
        .prepare_self_broadcast_joinsplit(
            "0x1111111111111111111111111111111111111111",
            10_000_000,
            421614,
            "0x2222222222222222222222222222222222222222",
            2000,
            1010,
            &keys.proving_key,
        )
        .unwrap();

    // Adversarial tampering: inflate merchant amount in envelope
    payload.merchant_amount = 20_000_000;
    let err = payload.validate_integrity().unwrap_err();
    assert!(err.to_string().contains("ADVERSARIAL_TAMPERING_DETECTED"));
    assert!(err.to_string().contains("Merchant amount mismatch"));
}

#[test]
fn test_gate2_4_mode2_in_pool_self_broadcast_transfer() {
    let sender_seed = [123u8; 32];
    let recipient_seed = [234u8; 32];
    let mut sender_wallet = PrivateNoteWallet::new(&sender_seed);
    let recipient_wallet = PrivateNoteWallet::new(&recipient_seed);
    let path = dummy_merkle_path();

    // 1. Sender deposits 80 USDC
    let (_, cm1) = sender_wallet.create_deposit_note(80_000_000, 1000).unwrap();
    sender_wallet
        .confirm_deposit(&cm1, 0, path, "0x01")
        .unwrap();

    // 2. Recipient shielded address (ZSA Bech32m)
    let recipient_zsa = recipient_wallet
        .shielded_address()
        .unwrap()
        .to_bech32m()
        .unwrap();
    assert!(recipient_zsa.starts_with("zs1"));

    let keys = generate_joinsplit_circuit_keys().unwrap();

    // 3. Sender performs Mode 2 In-Pool Transfer of 25 USDC via direct self-broadcast
    let contract_addr = "0x2222222222222222222222222222222222222222";
    let payload = sender_wallet
        .prepare_self_broadcast_shielded_transfer(
            &recipient_zsa,
            25_000_000,
            1, // asset_id: 1 (USDC)
            421614,
            contract_addr,
            2000,
            1010,
            &keys.proving_key,
        )
        .expect("Mode 2 self-broadcast payload preparation must succeed");

    // 4. Verify Mode 2 invariants
    assert_eq!(
        payload.recipient,
        "0x0000000000000000000000000000000000000000"
    );
    assert_eq!(
        payload.merchant_amount, 0,
        "Public merchant amount must be 0 in Mode 2"
    );
    assert_eq!(
        payload.protocol_fee, 0,
        "Protocol fee must be 0 bps in Mode 2 in-pool transfer"
    );
    assert_eq!(
        payload.execution_fee, 0,
        "Execution fee must be 0 in direct self-broadcast"
    );
    assert_eq!(payload.max_execution_fee, 0);

    // 5. Verify calldata integrity and ABI decoding
    assert!(payload.validate_integrity().is_ok());
    let decoded = spendJoinsplitCall::abi_decode(&payload.calldata_bytes)
        .expect("Mode 2 calldata must decode cleanly");
    assert_eq!(decoded.recipient, Address::ZERO);
    assert_eq!(decoded.merchant_amount.to::<u64>(), 0);
    assert_eq!(decoded.protocol_fee.to::<u64>(), 0);
    assert_eq!(decoded.execution_fee.to::<u64>(), 0);
}
