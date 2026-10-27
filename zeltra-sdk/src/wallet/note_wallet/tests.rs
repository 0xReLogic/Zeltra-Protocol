use super::*;
use zeltra_core::*;

fn dummy_merkle_path() -> Vec<String> {
    vec!["0x04".to_string()]
}

#[test]
fn test_wallet_deterministic_keys() {
    let seed = [7u8; 32];
    let w1 = PrivateNoteWallet::new(&seed);
    let w2 = PrivateNoteWallet::new(&seed);

    assert_eq!(w1.spending_key().unwrap(), w2.spending_key().unwrap());
    assert_eq!(w1.nullifier_key().unwrap(), w2.nullifier_key().unwrap());
    assert_eq!(w1.balance(), 0);
}

#[test]
fn test_note_lifecycle_and_balance() {
    let seed = [9u8; 32];
    let mut wallet = PrivateNoteWallet::new(&seed);

    // 1. Create deposit
    let (note1, cm1) = wallet.create_deposit_note(50_000_000, 1000).unwrap();
    assert_eq!(note1.status, NoteStatus::Unconfirmed);
    assert_eq!(wallet.balance(), 0); // Unconfirmed doesn't count towards balance

    // 2. Confirm deposit
    let path = dummy_merkle_path();
    wallet
        .confirm_deposit(&cm1, 0, path.clone(), "0xroot")
        .unwrap();
    assert_eq!(wallet.balance(), 50_000_000);

    // 3. Add second note
    let (_, cm2) = wallet.create_deposit_note(30_000_000, 1005).unwrap();
    wallet.confirm_deposit(&cm2, 1, path, "0xroot").unwrap();
    assert_eq!(wallet.balance(), 80_000_000);
}

#[test]
fn test_coin_selection_exact_match_zero_change() {
    let seed = [11u8; 32];
    let mut wallet = PrivateNoteWallet::new(&seed);
    let path = dummy_merkle_path();

    let (_, cm1) = wallet.create_deposit_note(10_000_000, 1000).unwrap();
    let (_, cm2) = wallet.create_deposit_note(25_000_000, 1000).unwrap();
    wallet
        .confirm_deposit(&cm1, 0, path.clone(), "0xroot")
        .unwrap();
    wallet.confirm_deposit(&cm2, 1, path, "0xroot").unwrap();

    // Exact match for 10_000_000 total (9_800_000 + 150_000 + 50_000)
    let selected = wallet
        .select_note_for_spend(9_800_000, 150_000, 50_000, 1010)
        .unwrap();
    assert_eq!(selected.input_commitment_hex, cm1);
    assert_eq!(selected.total_required, 10_000_000);
    assert_eq!(selected.change_amount, 0);
    assert!(!selected.has_change);
}

#[test]
fn test_coin_selection_best_fit_with_change() {
    let seed = [12u8; 32];
    let mut wallet = PrivateNoteWallet::new(&seed);
    let path = dummy_merkle_path();

    let (_, cm1) = wallet.create_deposit_note(10_000_000, 1000).unwrap();
    let (_, cm2) = wallet.create_deposit_note(50_000_000, 1000).unwrap();
    wallet
        .confirm_deposit(&cm1, 0, path.clone(), "0xroot")
        .unwrap();
    wallet.confirm_deposit(&cm2, 1, path, "0xroot").unwrap();

    // Need 15_000_000: cm1 is too small, cm2 is selected
    let selected = wallet
        .select_note_for_spend(14_000_000, 700_000, 300_000, 1010)
        .unwrap();
    assert_eq!(selected.input_commitment_hex, cm2);
    assert_eq!(selected.total_required, 15_000_000);
    assert_eq!(selected.change_amount, 35_000_000);
    assert!(selected.has_change);
}

#[test]
fn test_reservation_and_lease_expiry() {
    let seed = [15u8; 32];
    let mut wallet = PrivateNoteWallet::new(&seed);
    let path = dummy_merkle_path();

    let (_, cm1) = wallet.create_deposit_note(20_000_000, 1000).unwrap();
    wallet.confirm_deposit(&cm1, 0, path, "0xroot").unwrap();

    // Reserve for 60s at time 1000
    wallet.reserve_note(&cm1, "session_1", 1000, 60).unwrap();
    assert_eq!(wallet.balance(), 0);
    assert_eq!(wallet.reserved_balance(1000), 20_000_000);
    assert_eq!(wallet.total_balance(1000), 20_000_000);

    // Attempting to reserve again while active fails
    let res2 = wallet.reserve_note(&cm1, "session_2", 1030, 60);
    assert_eq!(res2, Err(WalletError::NoteAlreadyReserved(cm1.clone())));

    // At time 1061, lease has expired: re-reservation succeeds
    assert_eq!(wallet.reserved_balance(1061), 0);
    wallet.reserve_note(&cm1, "session_2", 1061, 60).unwrap();
}

#[test]
fn test_two_phase_commit_and_rollback() {
    let seed = [20u8; 32];
    let mut wallet = PrivateNoteWallet::new(&seed);
    let path = dummy_merkle_path();

    let (_, cm1) = wallet.create_deposit_note(100_000_000, 1000).unwrap();
    wallet.confirm_deposit(&cm1, 0, path, "0xroot").unwrap();

    let selected = wallet
        .select_note_for_spend(40_000_000, 2_000_000, 1_000_000, 1000)
        .unwrap();
    wallet
        .reserve_note(&selected.input_commitment_hex, "session_tx_1", 1000, 120)
        .unwrap();

    // Simulate rollback on failure
    wallet.rollback_spend("session_tx_1").unwrap();
    assert_eq!(wallet.balance(), 100_000_000);

    // Re-reserve and simulate successful commit
    wallet
        .reserve_note(&selected.input_commitment_hex, "session_tx_2", 1050, 120)
        .unwrap();
    wallet
        .commit_spend("session_tx_2", "0xnullifier123", 1055, None, None, None)
        .unwrap();

    assert_eq!(wallet.balance(), 0);
    let note = wallet.notes.get(&cm1).unwrap();
    match &note.status {
        NoteStatus::Spent { nullifier_hex, .. } => assert_eq!(nullifier_hex, "0xnullifier123"),
        _ => panic!("Expected spent status"),
    }
}

#[test]
fn test_encrypted_backup_roundtrip() {
    let seed = [42u8; 32];
    let mut wallet = PrivateNoteWallet::new(&seed);
    let path = dummy_merkle_path();

    let (_, cm1) = wallet.create_deposit_note(55_000_000, 2000).unwrap();
    wallet.confirm_deposit(&cm1, 5, path, "0xroot_abc").unwrap();

    let backup = wallet.export_backup("SuperSecretPassword123!").unwrap();
    assert!(!backup.is_empty());

    // Restore with wrong password fails
    let wrong_restore = PrivateNoteWallet::import_backup(&backup, "WrongPassword");
    assert_eq!(
        wrong_restore.err(),
        Some(WalletError::BackupDecryptionFailed)
    );

    // Restore with correct password succeeds
    let restored = PrivateNoteWallet::import_backup(&backup, "SuperSecretPassword123!").unwrap();
    assert_eq!(restored.balance(), 55_000_000);
    assert_eq!(
        restored.spending_key().unwrap(),
        wallet.spending_key().unwrap()
    );
    assert_eq!(
        restored.current_merkle_root_hex.as_deref(),
        Some("0xroot_abc")
    );
}

#[test]
fn test_prepare_spend_proof_and_verification() {
    let seed = [99u8; 32];
    let mut wallet = PrivateNoteWallet::new(&seed);
    let path = dummy_merkle_path();

    let (_, cm1) = wallet.create_deposit_note(50_000_000, 1000).unwrap();
    wallet.confirm_deposit(&cm1, 0, path, "0xroot").unwrap();

    let selected = wallet
        .select_note_for_spend(20_000_000, 100_000, 50_000, 1000)
        .unwrap();
    assert!(selected.has_change);
    assert_eq!(selected.change_amount, 29_850_000);

    let keys = zeltra_core::generate_note_circuit_keys().unwrap();

    let payload = wallet
        .prepare_spend_proof(
            &selected,
            "session_e2e_1",
            "0x1111111111111111111111111111111111111111",
            60_000,
            "0x2222222222222222222222222222222222222222222222222222222222222222",
            421614,
            "0x3333333333333333333333333333333333333333",
            2000,
            1000,
            &keys.proving_key,
        )
        .unwrap();

    assert_eq!(payload.proof_a_neg_hex.len(), 256); // 128 bytes
    assert_eq!(payload.proof_b_hex.len(), 512); // 256 bytes
    assert_eq!(payload.proof_c_hex.len(), 256); // 128 bytes
    assert_eq!(payload.public_inputs_hex.len(), 16);
    assert_eq!(payload.leaf_count, 1);
    assert_eq!(payload.note_epoch_id, 0);
    assert!(!payload.is_rollover);
    assert_eq!(payload.merchant_amount, 20_000_000);
    assert!(payload.has_change);

    // Commit the spend
    let change_path = dummy_merkle_path();
    wallet
        .commit_spend(
            "session_e2e_1",
            &payload.input_nullifier_hex,
            1005,
            Some(1),
            Some(change_path),
            Some("0xnew_root"),
        )
        .unwrap();

    // Balance should now equal the change note value
    assert_eq!(wallet.balance(), 29_850_000);
}

#[test]
fn test_note_epoch_status_lifecycle() {
    let seed = [25u8; 32];
    let mut wallet = PrivateNoteWallet::new(&seed);
    let (mut note, _) = wallet.create_deposit_note(5_000_000, 1000).unwrap();

    // When epoch_id is 2
    note.epoch_id = Some(2);

    // At epoch 2: Fresh
    assert_eq!(note.check_epoch_status(2), NoteEpochStatus::Fresh);
    assert_eq!(
        wallet.check_note_epoch_status(&note, 2),
        NoteEpochStatus::Fresh
    );

    // At epoch 3: MatureGrace (E-1 = 2)
    assert_eq!(note.check_epoch_status(3), NoteEpochStatus::MatureGrace);
    assert_eq!(
        wallet.check_note_epoch_status(&note, 3),
        NoteEpochStatus::MatureGrace
    );

    // At epoch 4: ExpiredRequiresRollover (<= E-2)
    assert_eq!(
        note.check_epoch_status(4),
        NoteEpochStatus::ExpiredRequiresRollover
    );
    assert_eq!(
        wallet.check_note_epoch_status(&note, 4),
        NoteEpochStatus::ExpiredRequiresRollover
    );

    // At epoch 1: Fresh or future
    assert_eq!(
        note.check_epoch_status(1),
        NoteEpochStatus::ExpiredRequiresRollover
    );
}

#[test]
fn test_standalone_refresh_dust_rejected() {
    let seed = [26u8; 32];
    let mut wallet = PrivateNoteWallet::new(&seed);
    let path = dummy_merkle_path();
    let keys = zeltra_core::get_or_init_note_circuit_keys();

    // Note with 1.50 USDC (< 2.00 USDC minimum)
    let (_, cm) = wallet.create_deposit_note(1_500_000, 1000).unwrap();
    wallet.confirm_deposit(&cm, 0, path, "0xroot").unwrap();

    let res = wallet.build_standalone_refresh(
        &cm,
        20_000,
        25_000,
        "0x1111111111111111111111111111111111111111111111111111111111111111",
        421614,
        "0x3333333333333333333333333333333333333333",
        2000,
        1,
        1000,
        &keys.proving_key,
    );

    assert!(res.is_err());
    match res {
        Err(WalletError::CryptoError(msg)) => {
            assert!(msg.contains("DUST_NOTE_ROLLOVER_REJECTED"));
        }
        _ => panic!("Expected DUST_NOTE_ROLLOVER_REJECTED error"),
    }
}

#[test]
fn test_build_in_flight_spend_rollover_and_refresh() {
    let seed = [27u8; 32];
    let mut wallet = PrivateNoteWallet::new(&seed);
    let path = dummy_merkle_path();
    let keys = zeltra_core::get_or_init_note_circuit_keys();

    // Create deposit note with epoch 0, amount 10.00 USDC
    let (_, cm) = wallet
        .create_deposit_note_with_epoch(10_000_000, 0, 1000)
        .unwrap();
    wallet.confirm_deposit(&cm, 0, path, "0xroot").unwrap();

    // In-flight rollover spend refreshing change note into epoch 1
    let payload = wallet
        .build_in_flight_spend_rollover(
            "0x2222222222222222222222222222222222222222",
            3_000_000,
            13_500, // 45 bps flat protocol fee
            20_000, // execution fee
            25_000,
            "0x1111111111111111111111111111111111111111111111111111111111111111",
            421614,
            "0x3333333333333333333333333333333333333333",
            2000,
            1, // target epoch 1
            1000,
            &keys.proving_key,
        )
        .unwrap();

    assert_eq!(payload.public_inputs_hex.len(), 16);
    assert_eq!(payload.note_epoch_id, 0); // Input note was epoch 0
    assert!(!payload.is_rollover);
    assert_eq!(payload.merchant_amount, 3_000_000);
    assert!(payload.has_change);

    // Find the newly created change note and verify its epoch is 1
    let change_note = wallet
        .notes
        .values()
        .find(|n| n.status == NoteStatus::Unconfirmed)
        .expect("change note should exist");
    assert_eq!(change_note.epoch_id, Some(1));
    assert_eq!(change_note.value, 10_000_000 - 3_000_000 - 13_500 - 20_000);
}

#[test]
fn test_reclaim_expired_reservations_and_effective_balance() {
    let seed = [30u8; 32];
    let mut wallet = PrivateNoteWallet::new(&seed);
    let path = dummy_merkle_path();

    let (_, cm1) = wallet.create_deposit_note(50_000_000, 1000).unwrap();
    wallet.confirm_deposit(&cm1, 0, path, "0xroot").unwrap();

    // Reserve for 60s at time 1000 (expires at 1060)
    wallet
        .reserve_note(&cm1, "session_expired_1", 1000, 60)
        .unwrap();

    assert_eq!(wallet.balance(), 0);
    assert_eq!(wallet.effective_balance(1030), 0);
    assert_eq!(wallet.effective_balance(1061), 50_000_000);

    // Before expiry, reclaim yields 0
    let reclaimed_early = wallet.reclaim_expired_reservations(1030);
    assert_eq!(reclaimed_early, 0);
    assert_eq!(wallet.balance(), 0);

    // After expiry, reclaim yields 1 and restores balance
    let reclaimed = wallet.reclaim_expired_reservations(1061);
    assert_eq!(reclaimed, 1);
    assert_eq!(wallet.balance(), 50_000_000);

    // Explicit sync_mempool_expiry test
    wallet
        .reserve_note(&cm1, "session_manual_sync", 1100, 60)
        .unwrap();
    assert_eq!(wallet.balance(), 0);
    wallet.sync_mempool_expiry("session_manual_sync").unwrap();
    assert_eq!(wallet.balance(), 50_000_000);
}

#[test]
fn test_encrypted_backup_tampering_rejected() {
    let seed = [43u8; 32];
    let mut wallet = PrivateNoteWallet::new(&seed);
    let path = dummy_merkle_path();

    let (_, cm1) = wallet.create_deposit_note(30_000_000, 2000).unwrap();
    wallet
        .confirm_deposit(&cm1, 2, path, "0xroot_tamper")
        .unwrap();

    let backup = wallet.export_backup("Password123!").unwrap();

    // 1. Tamper with ciphertext
    let mut parsed: serde_json::Value = serde_json::from_str(&backup).unwrap();
    let ct_hex = parsed["ciphertext_hex"].as_str().unwrap().to_string();
    let mut ct_bytes = hex::decode(&ct_hex).unwrap();
    ct_bytes[0] ^= 0xff; // flip bits
    parsed["ciphertext_hex"] = serde_json::Value::String(hex::encode(ct_bytes));
    let tampered_backup = serde_json::to_string(&parsed).unwrap();

    let res = PrivateNoteWallet::import_backup(&tampered_backup, "Password123!");
    assert_eq!(res.err(), Some(WalletError::BackupDecryptionFailed));

    // 2. Tamper with salt
    let mut parsed_salt: serde_json::Value = serde_json::from_str(&backup).unwrap();
    parsed_salt["salt_hex"] = serde_json::Value::String(hex::encode([0u8; 32]));
    let tampered_salt_backup = serde_json::to_string(&parsed_salt).unwrap();
    let res_salt = PrivateNoteWallet::import_backup(&tampered_salt_backup, "Password123!");
    assert_eq!(res_salt.err(), Some(WalletError::BackupDecryptionFailed));
}

#[test]
fn test_update_note_witness_lifecycle() {
    let seed = [44u8; 32];
    let mut wallet = PrivateNoteWallet::new(&seed);
    let path = dummy_merkle_path();

    let (note, cm) = wallet.create_deposit_note(70_000_000, 1000).unwrap();
    assert_eq!(note.status, NoteStatus::Unconfirmed);
    assert_eq!(wallet.balance(), 0);

    // Negative: Invalid path rejected (empty path)
    let invalid_path = vec![];
    let err_path = wallet.update_note_witness(&cm, 0, invalid_path, "0xroot1");
    assert!(err_path.is_err());
    assert_eq!(wallet.balance(), 0);

    // Negative: Non-existent note rejected
    let err_notfound = wallet.update_note_witness("0xdeadbeef", 0, path.clone(), "0xroot1");
    assert_eq!(
        err_notfound.err(),
        Some(WalletError::NoteNotFound("0xdeadbeef".into()))
    );

    // Positive: Update witness promotes note to Unspent
    wallet
        .update_note_witness(&cm, 10, path.clone(), "0xroot_synced")
        .unwrap();
    assert_eq!(wallet.balance(), 70_000_000);
    let updated_note = wallet.notes.get(&cm).unwrap();
    assert_eq!(updated_note.status, NoteStatus::Unspent);
    assert_eq!(updated_note.leaf_index, Some(10));
    assert_eq!(
        wallet.current_merkle_root_hex.as_deref(),
        Some("0xroot_synced")
    );
}

#[test]
fn test_parse_fr_and_bytes32_helpers() {
    // Valid 32-byte scalar
    let valid_fr = Fr::from(123456789u64);
    let hex_fr = hex::encode(fr_to_be_bytes(&valid_fr));
    let parsed = parse_fr_from_hex(&hex_fr).unwrap();
    assert_eq!(parsed, valid_fr);

    // With 0x prefix
    let parsed_prefixed = parse_fr_from_hex(&format!("0x{hex_fr}")).unwrap();
    assert_eq!(parsed_prefixed, valid_fr);

    // Invalid hex length
    let err_len = parse_fr_from_hex("0x1234");
    assert!(err_len.is_err());

    // 20-byte EVM address padded to 32 bytes
    let addr = "0x5B38Da6a701c568545dCfcB03FcB875f56beddC4";
    let padded = parse_bytes32_from_hex(addr, true).unwrap();
    assert_eq!(&padded[0..12], &[0u8; 12]);
    assert_eq!(
        &padded[12..],
        &hex::decode("5B38Da6a701c568545dCfcB03FcB875f56beddC4").unwrap()[..]
    );

    // 20-byte address rejected when padding not allowed
    let err_pad = parse_bytes32_from_hex(addr, false);
    assert!(err_pad.is_err());
}

#[test]
fn test_coin_selection_tiered_knapsack_joinsplit() {
    let seed = [50u8; 32];
    let mut wallet = PrivateNoteWallet::new(&seed);
    let path = dummy_merkle_path();

    // User has two notes of 10 USDC each (total 20 USDC)
    let (_, cm1) = wallet.create_deposit_note(10_000_000, 1000).unwrap();
    let (_, cm2) = wallet.create_deposit_note(10_000_000, 1000).unwrap();
    wallet
        .confirm_deposit(&cm1, 0, path.clone(), "0xroot")
        .unwrap();
    wallet.confirm_deposit(&cm2, 1, path, "0xroot").unwrap();

    // User spends 15 USDC total (14.5 merchant + 0.3 pfee + 0.2 efee)
    // Neither single note suffices; Branch 3 Knapsack must select both notes!
    let selected = wallet
        .select_note_for_spend(14_500_000, 300_000, 200_000, 1010)
        .unwrap();

    assert!(selected.is_joinsplit());
    assert_eq!(selected.total_input_value(), 20_000_000);
    assert_eq!(selected.total_required, 15_000_000);
    assert_eq!(selected.change_amount, 5_000_000);
    assert!(selected.has_change);
    assert!(
        (selected.input_commitment_hex == cm1
            && selected.second_input_commitment_hex.as_deref() == Some(&cm2))
            || (selected.input_commitment_hex == cm2
                && selected.second_input_commitment_hex.as_deref() == Some(&cm1))
    );
}

#[test]
fn test_coin_selection_insufficient_even_with_joinsplit() {
    let seed = [51u8; 32];
    let mut wallet = PrivateNoteWallet::new(&seed);
    let path = dummy_merkle_path();

    // Two notes of 5 USDC each (total 10 USDC)
    let (_, cm1) = wallet.create_deposit_note(5_000_000, 1000).unwrap();
    let (_, cm2) = wallet.create_deposit_note(5_000_000, 1000).unwrap();
    wallet
        .confirm_deposit(&cm1, 0, path.clone(), "0xroot")
        .unwrap();
    wallet.confirm_deposit(&cm2, 1, path, "0xroot").unwrap();

    // Spending 15 USDC fails with InsufficientBalance
    let err = wallet.select_note_for_spend(14_000_000, 700_000, 300_000, 1010);
    assert_eq!(
        err,
        Err(WalletError::InsufficientBalance {
            requested: 15_000_000,
            available: 10_000_000,
        })
    );
}

#[test]
fn test_prepare_joinsplit_spend_proof_1_note_with_dummy() {
    let seed = [52u8; 32];
    let mut wallet = PrivateNoteWallet::new(&seed);
    let path = dummy_merkle_path();

    let (_, cm1) = wallet.create_deposit_note(50_000_000, 1000).unwrap();
    wallet.confirm_deposit(&cm1, 0, path, "0xroot").unwrap();

    // Single note spend (20 USDC from 50 USDC note)
    // DEC-028 / DEC-036A: Flat 45 bps protocol fee ceiling division: (20_000_000 * 45).div_ceil(10_000) = 90_000
    let selected = wallet
        .select_note_for_spend(20_000_000, 90_000, 50_000, 1000)
        .unwrap();
    assert!(!selected.is_joinsplit());

    let keys = zeltra_core::generate_joinsplit_circuit_keys().unwrap();

    let payload = wallet
        .prepare_joinsplit_spend_proof(
            &selected,
            "session_js_single",
            "0x1111111111111111111111111111111111111111",
            60_000,
            "0x2222222222222222222222222222222222222222222222222222222222222222",
            421614,
            "0x3333333333333333333333333333333333333333",
            2000,
            1000,
            &keys.proving_key,
        )
        .unwrap();

    assert_eq!(payload.public_inputs_hex.len(), 19);
    assert_eq!(payload.proof_a_neg_hex.len(), 256);
    assert_eq!(payload.proof_b_hex.len(), 512);
    assert_eq!(payload.proof_c_hex.len(), 256);
    assert!(payload.has_change_1);
    assert!(!payload.has_change_2);

    // Verify with zeltra_core::verify_joinsplit_proof
    let mut pis = Vec::new();
    for h in &payload.public_inputs_hex {
        pis.push(parse_fr_from_hex(h).unwrap());
    }

    let from_evm_a = zeltra_core::from_evm_g1(
        &hex::decode(&payload.proof_a_neg_hex)
            .unwrap()
            .try_into()
            .unwrap(),
    )
    .unwrap();
    let from_evm_b = zeltra_core::from_evm_g2(
        &hex::decode(&payload.proof_b_hex)
            .unwrap()
            .try_into()
            .unwrap(),
    )
    .unwrap();
    let from_evm_c = zeltra_core::from_evm_g1(
        &hex::decode(&payload.proof_c_hex)
            .unwrap()
            .try_into()
            .unwrap(),
    )
    .unwrap();

    let reconstructed_proof = ark_groth16::Proof {
        a: -from_evm_a,
        b: from_evm_b,
        c: from_evm_c,
    };

    let verified =
        zeltra_core::verify_joinsplit_proof(&keys.verifying_key, &reconstructed_proof, &pis)
            .unwrap();
    assert!(
        verified,
        "Universal JoinSplit proof with dummy 2nd note must verify"
    );
}

#[test]
fn test_prepare_joinsplit_spend_proof_2_notes_and_commit() {
    let seed = [53u8; 32];
    let mut wallet = PrivateNoteWallet::new(&seed);

    // Note 1: 10 USDC
    let (_note1, cm1) = wallet.create_deposit_note(10_000_000, 1000).unwrap();
    // Note 2: 10 USDC
    let (_note2, cm2) = wallet.create_deposit_note(10_000_000, 1000).unwrap();

    let root_hex = "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef";
    let path1_hex = vec!["0x04".to_string()];
    let path2_hex = vec!["0x04".to_string()];

    wallet
        .confirm_deposit(&cm1, 0, path1_hex, root_hex)
        .unwrap();
    wallet
        .confirm_deposit(&cm2, 1, path2_hex, root_hex)
        .unwrap();
    assert_eq!(wallet.balance(), 20_000_000);

    let keys = zeltra_core::generate_joinsplit_circuit_keys().unwrap();

    // Pay using 2-note JoinSplit
    // DEC-028 / DEC-036A: Flat 45 bps protocol fee ceiling division: (14_500_000 * 45).div_ceil(10_000) = 65_250
    let payload = wallet
        .pay_joinsplit(
            "0x1111111111111111111111111111111111111111",
            14_500_000,
            65_250,
            200_000,
            250_000,
            "0x2222222222222222222222222222222222222222222222222222222222222222",
            421614,
            "0x3333333333333333333333333333333333333333",
            2000,
            1010,
            &keys.proving_key,
        )
        .unwrap();

    assert_eq!(payload.public_inputs_hex.len(), 19);

    // Commit the JoinSplit spend
    let change_path = dummy_merkle_path();
    wallet
        .commit_joinsplit_spend(
            &payload.session_id,
            &payload.input_nullifier_1_hex,
            Some(&payload.input_nullifier_2_hex),
            1015,
            Some(2),
            Some(change_path),
            Some("0xnew_root_js"),
        )
        .unwrap();

    // Remaining balance must be change amount (20_000_000 - 14_500_000 - 65_250 - 200_000 = 5_234_750)
    assert_eq!(wallet.balance(), 5_234_750);
    let n1_status = &wallet.notes.get(&cm1).unwrap().status;
    let n2_status = &wallet.notes.get(&cm2).unwrap().status;
    assert!(matches!(n1_status, NoteStatus::Spent { .. }));
    assert!(matches!(n2_status, NoteStatus::Spent { .. }));
}

#[test]
fn test_consolidate_notes_in_pool() {
    let seed = [54u8; 32];
    let mut wallet = PrivateNoteWallet::new(&seed);
    let (_note1, cm1) = wallet.create_deposit_note(10_000_000, 1000).unwrap();
    let (_note2, cm2) = wallet.create_deposit_note(15_000_000, 1000).unwrap();

    let root_hex = "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef";
    let path1_hex = vec!["0x04".to_string()];
    let path2_hex = vec!["0x04".to_string()];

    wallet
        .confirm_deposit(&cm1, 0, path1_hex, root_hex)
        .unwrap();
    wallet
        .confirm_deposit(&cm2, 1, path2_hex, root_hex)
        .unwrap();
    assert_eq!(wallet.balance(), 25_000_000);

    let keys = zeltra_core::generate_joinsplit_circuit_keys().unwrap();

    let payload = wallet
        .consolidate_note_pair(
            &cm1,
            &cm2,
            "session_consolidate_1",
            100_000, // 0.1 USDC exec fee
            150_000,
            "0x2222222222222222222222222222222222222222222222222222222222222222",
            421614,
            "0x3333333333333333333333333333333333333333",
            2000,
            1010,
            &keys.proving_key,
        )
        .unwrap();

    assert_eq!(payload.merchant_amount, 0);
    assert_eq!(payload.execution_fee, 100_000);

    // Commit consolidation
    let change_path = dummy_merkle_path();
    wallet
        .commit_joinsplit_spend(
            "session_consolidate_1",
            &payload.input_nullifier_1_hex,
            Some(&payload.input_nullifier_2_hex),
            1015,
            Some(2),
            Some(change_path),
            Some("0xroot_consolidated"),
        )
        .unwrap();

    // 25_000_000 - 100_000 = 24_900_000
    assert_eq!(wallet.balance(), 24_900_000);
}

#[test]
fn test_mmr_proof_cache_and_deserialization_positive() {
    let seed = [99u8; 32];
    let mut wallet = PrivateNoteWallet::new(&seed);

    let sib1 = hex::encode(fr_to_be_bytes(&Fr::from(12345u64)));
    let sib2 = hex::encode(fr_to_be_bytes(&Fr::from(67890u64)));
    let peak1 = hex::encode(fr_to_be_bytes(&Fr::from(54321u64)));
    let root = hex::encode(fr_to_be_bytes(&Fr::from(99999u64)));

    let proof = MmrProofResponse {
        leaf_index: 5,
        leaf_count: 10,
        commitment: "0x1111".to_string(),
        mountain_height: 2,
        mountain_siblings: vec![sib1, sib2],
        peak_bagging_siblings: vec![peak1],
        bagged_root: format!("0x{root}"),
        block_number: 1000,
    };

    // Cache proof
    wallet.cache_mmr_proof(proof.clone());

    // Retrieve from cache
    let cached = wallet
        .get_cached_mmr_proof(5, &format!("0x{root}"))
        .unwrap();
    assert_eq!(cached.leaf_index, 5);
    assert_eq!(cached.mountain_height, 2);

    // Retrieve any for leaf
    let cached_any = wallet.get_any_cached_mmr_proof_for_leaf(5).unwrap();
    assert_eq!(cached_any.leaf_count, 10);

    // Deserialize siblings
    let (m_sibs, p_sibs) = PrivateNoteWallet::deserialize_mmr_siblings(&proof).unwrap();
    assert_eq!(m_sibs[0], Fr::from(12345u64));
    assert_eq!(m_sibs[1], Fr::from(67890u64));
    assert_eq!(m_sibs[2], Fr::from(0u64)); // zero-padded
    assert_eq!(p_sibs[0], Fr::from(54321u64));
    assert_eq!(p_sibs[1], Fr::from(0u64)); // zero-padded

    // Clear cache
    wallet.clear_mmr_proof_cache();
    assert!(wallet
        .get_cached_mmr_proof(5, &format!("0x{root}"))
        .is_none());
}

#[test]
fn test_mmr_proof_deserialization_negative_bounds() {
    // Negative test 1: mountain_siblings > 32
    let invalid_m_sibs = vec!["00".repeat(32); 33];
    let proof1 = MmrProofResponse {
        leaf_index: 0,
        leaf_count: 1,
        commitment: "0x00".to_string(),
        mountain_height: 1,
        mountain_siblings: invalid_m_sibs,
        peak_bagging_siblings: vec![],
        bagged_root: "0x00".to_string(),
        block_number: 1,
    };
    let err1 = PrivateNoteWallet::deserialize_mmr_siblings(&proof1);
    assert!(err1.is_err());
    assert!(format!("{err1:?}").contains("mountain_siblings length 33 exceeds max 32"));

    // Negative test 2: peak_bagging_siblings > 32
    let invalid_p_sibs = vec!["00".repeat(32); 33];
    let proof2 = MmrProofResponse {
        leaf_index: 0,
        leaf_count: 1,
        commitment: "0x00".to_string(),
        mountain_height: 1,
        mountain_siblings: vec![],
        peak_bagging_siblings: invalid_p_sibs,
        bagged_root: "0x00".to_string(),
        block_number: 1,
    };
    let err2 = PrivateNoteWallet::deserialize_mmr_siblings(&proof2);
    assert!(err2.is_err());
    assert!(format!("{err2:?}").contains("peak_bagging_siblings length 33 exceeds max 32"));

    // Negative test 3: non-canonical scalar in siblings
    // Fr modulus + 1
    let non_canonical_hex = "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff";
    let proof3 = MmrProofResponse {
        leaf_index: 0,
        leaf_count: 1,
        commitment: "0x00".to_string(),
        mountain_height: 1,
        mountain_siblings: vec![non_canonical_hex.to_string()],
        peak_bagging_siblings: vec![],
        bagged_root: "0x00".to_string(),
        block_number: 1,
    };
    let err3 = PrivateNoteWallet::deserialize_mmr_siblings(&proof3);
    assert!(err3.is_err());
}

#[test]
fn test_prepare_spend_proof_with_cached_mmr_proof() {
    let seed = [77u8; 32];
    let mut wallet = PrivateNoteWallet::new(&seed);
    let sk = wallet.spending_key().unwrap();

    // 1. Create a note and simulate on-chain MMR
    let (note, cm) = wallet.create_deposit_note(10_000_000, 1000).unwrap();
    let rho = parse_fr_from_hex(&note.rho_hex).unwrap();
    let rand = parse_fr_from_hex(&note.randomness_hex).unwrap();
    let fr_cm = zeltra_core::note_commitment(10_000_000, 1, sk, rho, rand);

    let mut mmr = zeltra_core::MerkleMountainRange::new();
    mmr.append(fr_cm);
    let mmr_proof = mmr.generate_proof(0);
    let root = mmr.get_root();

    let m_sibs_hex: Vec<String> = mmr_proof
        .mountain_siblings
        .iter()
        .map(|s| hex::encode(fr_to_be_bytes(s)))
        .collect();
    let p_sibs_hex: Vec<String> = mmr_proof
        .peak_bagging_siblings
        .iter()
        .map(|s| hex::encode(fr_to_be_bytes(s)))
        .collect();
    let root_hex = format!("0x{}", hex::encode(fr_to_be_bytes(&root)));

    // Confirm note
    wallet
        .confirm_deposit(&cm, 0, dummy_merkle_path(), &root_hex)
        .unwrap();

    // Build MmrProofResponse
    let mmr_resp = MmrProofResponse {
        leaf_index: 0,
        leaf_count: 1,
        commitment: cm.clone(),
        mountain_height: mmr_proof.mountain_height,
        mountain_siblings: m_sibs_hex,
        peak_bagging_siblings: p_sibs_hex,
        bagged_root: root_hex.clone(),
        block_number: 100,
    };

    // Cache proof into wallet
    wallet.cache_mmr_proof(mmr_resp.clone());

    // Generate proving key
    let keys = zeltra_core::generate_note_circuit_keys().unwrap();

    let selected = wallet
        .select_note_for_spend(7_000_000, 31_500, 50_000, 1010)
        .unwrap();

    let payload = wallet
        .prepare_spend_proof_with_mmr(
            &selected,
            "session_mmr_spend_1",
            "0x1111111111111111111111111111111111111111",
            100_000,
            "0x2222222222222222222222222222222222222222222222222222222222222222",
            421614,
            "0x3333333333333333333333333333333333333333",
            2000,
            None,
            false,
            &mmr_resp,
            1010,
            &keys.proving_key,
        )
        .unwrap();

    assert_eq!(payload.note_root_hex, hex::encode(fr_to_be_bytes(&root)));
    assert_eq!(payload.leaf_count, 1);
    assert_eq!(payload.public_inputs_hex.len(), 16);
}

#[test]
fn test_dec037_wallet_viewing_key_derivation_and_export() {
    let seed = [0x42u8; 32];
    let wallet = PrivateNoteWallet::new(&seed);

    let ivk = wallet.incoming_viewing_key().expect("IVK should be valid");
    let pk = wallet
        .incoming_viewing_public_key()
        .expect("Public key should be valid");
    assert_eq!(ivk.public_key(), pk);
    assert_eq!(wallet.incoming_viewing_public_key_hex(), hex::encode(pk));

    let ovk = wallet.outcoming_viewing_key().expect("OVK should be valid");
    assert_eq!(ovk.as_bytes().len(), 32);
}

#[test]
fn test_dec037_wallet_recover_notes_unspent_and_spent() {
    let seed = [0x55u8; 32];
    let mut wallet = PrivateNoteWallet::new(&seed);
    let ivk = wallet.incoming_viewing_key().unwrap();
    let sk = wallet.spending_key().unwrap();
    let nk = wallet.nullifier_key().unwrap();

    // Simulate 2 incoming notes: note 1 (10 USDC), note 2 (5 USDC)
    let rho1 = Fr::from(111u64);
    let val1 = 10_000_000u64;
    let r1 = zeltra_core::derive_note_randomness(sk, rho1);
    let cm1 = note_commitment(val1, 1, sk, rho1, r1);
    let memo1 = encrypt_note_memo(val1, rho1, 1, &ivk);

    let rho2 = Fr::from(222u64);
    let val2 = 5_000_000u64;
    let r2 = zeltra_core::derive_note_randomness(sk, rho2);
    let cm2 = note_commitment(val2, 1, sk, rho2, r2);
    let memo2 = encrypt_note_memo(val2, rho2, 1, &ivk);

    // Third memo from an unrelated user (should be ignored by trial decryption)
    let (other_ivk, _) = derive_viewing_keys(b"other_unrelated_user_seed");
    let memo_other = encrypt_note_memo(100_000_000, Fr::from(999u64), 1, &other_ivk);

    // Pre-compute nullifier for note 2 to simulate it as SPENT on-chain
    let nf2 = derive_nullifier(nk, cm2, 1, 0);
    let nf2_hex = format!("0x{}", hex::encode(fr_to_be_bytes(&nf2)));

    let events = vec![
        NoteEvent {
            leaf_index: 0,
            commitment_hex: format!("0x{}", hex::encode(fr_to_be_bytes(&cm1))),
            new_mmr_root_hex: "0x123".into(),
            leaf_count: 1,
            encrypted_memo: memo1,
            block_number: Some(10),
        },
        NoteEvent {
            leaf_index: 1,
            commitment_hex: format!("0x{}", hex::encode(fr_to_be_bytes(&cm2))),
            new_mmr_root_hex: "0x456".into(),
            leaf_count: 2,
            encrypted_memo: memo2,
            block_number: Some(11),
        },
        NoteEvent {
            leaf_index: 2,
            commitment_hex: "0x789".into(),
            new_mmr_root_hex: "0x789".into(),
            leaf_count: 3,
            encrypted_memo: memo_other,
            block_number: Some(12),
        },
    ];

    // Execute recovery
    let res = wallet
        .recover_notes(&events, |nf| nf == nf2_hex)
        .expect("Recovery must succeed");

    assert_eq!(res.scanned_events, 3);
    assert_eq!(res.recovered_notes.len(), 2);
    assert_eq!(res.total_unspent_balance, val1);
    assert_eq!(res.total_spent_balance, val2);

    // Note 1 is Unspent, Note 2 is Spent
    let cm1_hex = format!("0x{}", hex::encode(fr_to_be_bytes(&cm1)));
    let cm2_hex = format!("0x{}", hex::encode(fr_to_be_bytes(&cm2)));
    assert_eq!(
        wallet.notes.get(&cm1_hex).unwrap().status,
        NoteStatus::Unspent
    );
    assert_eq!(
        wallet.notes.get(&cm2_hex).unwrap().status,
        NoteStatus::Spent {
            nullifier_hex: nf2_hex.clone(),
            spent_at_secs: 0,
        }
    );
    assert_eq!(wallet.balance(), val1);

    // Standalone recovery also returns identical result
    let standalone_res = recover_from_viewing_key(&ivk, Some(sk), &events, |nf| nf == nf2_hex);
    assert_eq!(standalone_res.recovered_notes.len(), 2);
    assert_eq!(standalone_res.total_unspent_balance, val1);
    assert_eq!(standalone_res.total_spent_balance, val2);
}

#[test]
fn test_dec037_prepare_spend_proof_includes_change_memo() {
    let mut wallet = PrivateNoteWallet::new(&[0x77u8; 32]);
    let root = Fr::from(100u64);
    let root_hex = format!("0x{}", hex::encode(fr_to_be_bytes(&root)));

    // Deposit a 10 USDC note
    let (_, cm) = wallet.create_deposit_note(10_000_000, 1000).unwrap();
    wallet
        .confirm_deposit(&cm, 0, dummy_merkle_path(), &root_hex)
        .unwrap();

    let keys = zeltra_core::generate_note_circuit_keys().unwrap();
    let selected = wallet
        .select_note_for_spend(6_000_000, 27_000, 50_000, 1010)
        .unwrap();
    assert!(selected.has_change);

    let payload = wallet
        .prepare_spend_proof(
            &selected,
            "session_dec037_memo_1",
            "0x1111111111111111111111111111111111111111",
            100_000,
            "0x2222222222222222222222222222222222222222222222222222222222222222",
            421614,
            "0x3333333333333333333333333333333333333333",
            2000,
            1010,
            &keys.proving_key,
        )
        .unwrap();

    assert!(payload.has_change);
    let memo_hex = payload
        .change_encrypted_memo_hex
        .expect("Change memo must be present");
    let memo_bytes = hex::decode(memo_hex).expect("Memo must be valid hex");
    assert_eq!(memo_bytes.len(), 96);

    // Trial decrypt change memo with wallet's IVK
    let ivk = wallet.incoming_viewing_key().unwrap();
    let decrypted = decrypt_note_memo(&memo_bytes, &ivk).expect("Decryption must succeed");
    assert_eq!(decrypted.value, selected.change_amount);
}

#[test]
fn test_dec037_wallet_recover_notes_batch_and_sync() {
    let seed = [0x88u8; 32];
    let mut wallet = PrivateNoteWallet::new(&seed);
    let ivk = wallet.incoming_viewing_key().unwrap();
    let sk = wallet.spending_key().unwrap();
    let nk = wallet.nullifier_key().unwrap();

    let val1 = 7_000_000u64;
    let rho1 = Fr::from(555u64);
    let r1 = zeltra_core::derive_note_randomness(sk, rho1);
    let cm1 = note_commitment(val1, 1, sk, rho1, r1);
    let memo1 = encrypt_note_memo(val1, rho1, 1, &ivk);

    let val2 = 3_000_000u64;
    let rho2 = Fr::from(777u64);
    let r2 = zeltra_core::derive_note_randomness(sk, rho2);
    let cm2 = note_commitment(val2, 1, sk, rho2, r2);
    let memo2 = encrypt_note_memo(val2, rho2, 1, &ivk);

    let nf1 = derive_nullifier(nk, cm1, 0, 0);
    let nf1_hex = format!("0x{}", hex::encode(fr_to_be_bytes(&nf1)));

    let events = vec![
        NoteEvent {
            leaf_index: 0,
            commitment_hex: format!("0x{}", hex::encode(fr_to_be_bytes(&cm1))),
            new_mmr_root_hex: "0x1".into(),
            leaf_count: 1,
            encrypted_memo: memo1,
            block_number: Some(100),
        },
        NoteEvent {
            leaf_index: 1,
            commitment_hex: format!("0x{}", hex::encode(fr_to_be_bytes(&cm2))),
            new_mmr_root_hex: "0x2".into(),
            leaf_count: 2,
            encrypted_memo: memo2,
            block_number: Some(101),
        },
    ];

    // Batch test: nf1 is spent, nf2 is unspent
    let mut batch_called_count = 0;
    let res = wallet
        .recover_notes_batch(&events, |nfs| {
            batch_called_count += 1;
            assert_eq!(nfs.len(), 2);
            Ok(vec![nfs[0] == nf1_hex, false])
        })
        .expect("Batch recovery must succeed");

    assert_eq!(
        batch_called_count, 1,
        "Exactly 1 batch call must be made for all discovered notes"
    );
    assert_eq!(res.recovered_notes.len(), 2);
    assert_eq!(res.total_spent_balance, val1);
    assert_eq!(res.total_unspent_balance, val2);
    assert_eq!(wallet.balance(), val2);
}

#[test]
fn test_sdk_trial_decryption_partitions_notes_correctly() {
    let seed = [0x99u8; 32];
    let mut wallet = PrivateNoteWallet::new(&seed);
    let shared_secret = [0x42u8; 32];

    // 1. Prepare WETH Note (asset_id = 0, 1.5 ETH = 1_500_000_000 Gwei)
    let weth_val = 1_500_000_000u64;
    let weth_rho = Fr::from(12345u64);
    let weth_rho_bytes = fr_to_be_bytes(&weth_rho);
    let mut weth_pt_bytes = [0u8; 48];
    weth_pt_bytes[0..8].copy_from_slice(&weth_val.to_be_bytes());
    weth_pt_bytes[8..40].copy_from_slice(&weth_rho_bytes);
    weth_pt_bytes[40..48].copy_from_slice(&0u64.to_be_bytes()); // asset_id = 0

    // 2. Prepare USDC Note (asset_id = 1, 25.00 USDC = 25_000_000 Micro-USDC)
    let usdc_val = 25_000_000u64;
    let usdc_rho = Fr::from(67890u64);
    let usdc_rho_bytes = fr_to_be_bytes(&usdc_rho);
    let mut usdc_pt_bytes = [0u8; 48];
    usdc_pt_bytes[0..8].copy_from_slice(&usdc_val.to_be_bytes());
    usdc_pt_bytes[8..40].copy_from_slice(&usdc_rho_bytes);
    usdc_pt_bytes[40..48].copy_from_slice(&1u64.to_be_bytes()); // asset_id = 1

    // Process both plaintexts
    wallet
        .process_decrypted_plaintext(&weth_pt_bytes, &shared_secret)
        .expect("WETH plaintext deserialization must succeed");
    wallet
        .process_decrypted_plaintext(&usdc_pt_bytes, &shared_secret)
        .expect("USDC plaintext deserialization must succeed");

    // Verify partitioned collections
    assert_eq!(wallet.weth_notes.len(), 1);
    assert_eq!(wallet.usdc_notes.len(), 1);
    assert_eq!(wallet.weth_notes[0].asset_id, 0);
    assert_eq!(wallet.usdc_notes[0].asset_id, 1);
    assert_eq!(wallet.weth_notes[0].value, weth_val);
    assert_eq!(wallet.usdc_notes[0].value, usdc_val);

    // Verify balance queries
    assert_eq!(wallet.get_balance(0).unwrap(), 1_500_000_000);
    assert_eq!(wallet.get_balance(1).unwrap(), 25_000_000);
    assert!(matches!(
        wallet.get_balance(2),
        Err(WalletError::UnsupportedAssetId(2))
    ));

    // Verify human formatted balances
    assert_eq!(wallet.get_human_balance(0).unwrap(), "1.500000 ETH");
    assert_eq!(wallet.get_human_balance(1).unwrap(), "$25.00 USDC");

    // Verify invalid asset_id in plaintext is rejected
    let mut invalid_pt_bytes = [0u8; 48];
    invalid_pt_bytes[40..48].copy_from_slice(&2u64.to_be_bytes()); // asset_id = 2
    assert!(matches!(
        wallet.process_decrypted_plaintext(&invalid_pt_bytes, &shared_secret),
        Err(WalletError::UnsupportedAssetId(2))
    ));
}

#[test]
fn test_prepare_transfer_rejects_zero_amount() {
    let seed = [11u8; 32];
    let mut wallet = PrivateNoteWallet::new(&seed);
    let recipient_seed = [12u8; 32];
    let recipient_wallet = PrivateNoteWallet::new(&recipient_seed);
    let zsa = recipient_wallet.shielded_address().unwrap();
    let zsa_str = zsa.to_bech32m().unwrap();
    let keys = zeltra_core::generate_joinsplit_circuit_keys().unwrap();

    let err = wallet.prepare_shielded_transfer_proof(
        &zsa_str,
        0, // zero amount
        1,
        20_000,
        "0x1111111111111111111111111111111111111111111111111111111111111111",
        421614,
        "0x2222222222222222222222222222222222222222",
        2000,
        1000,
        &keys.proving_key,
    );
    assert!(matches!(err, Err(WalletError::CryptoError(_))));
}

#[test]
fn test_prepare_transfer_rejects_insufficient_balance() {
    let seed = [13u8; 32];
    let mut wallet = PrivateNoteWallet::new(&seed);
    let (_, cm) = wallet.create_deposit_note(5_000_000, 1000).unwrap();
    wallet
        .confirm_deposit(&cm, 0, dummy_merkle_path(), "0xroot")
        .unwrap();

    let recipient_wallet = PrivateNoteWallet::new(&[14u8; 32]);
    let zsa = recipient_wallet.shielded_address().unwrap();
    let zsa_str = zsa.to_bech32m().unwrap();
    let keys = zeltra_core::generate_joinsplit_circuit_keys().unwrap();

    // 5M in wallet, trying to transfer 5M with 20_000 fee requires 5_020_000
    let err = wallet.prepare_shielded_transfer_proof(
        &zsa_str,
        5_000_000,
        1,
        20_000,
        "0x1111111111111111111111111111111111111111111111111111111111111111",
        421614,
        "0x2222222222222222222222222222222222222222",
        2000,
        1000,
        &keys.proving_key,
    );
    assert!(matches!(err, Err(WalletError::InsufficientBalance { .. })));
}

#[test]
fn test_prepare_transfer_rejects_corrupted_zsa() {
    let seed = [15u8; 32];
    let mut wallet = PrivateNoteWallet::new(&seed);
    let (_, cm) = wallet.create_deposit_note(10_000_000, 1000).unwrap();
    wallet
        .confirm_deposit(&cm, 0, dummy_merkle_path(), "0xroot")
        .unwrap();

    let recipient_wallet = PrivateNoteWallet::new(&[16u8; 32]);
    let zsa = recipient_wallet.shielded_address().unwrap();
    let mut zsa_str = zsa.to_bech32m().unwrap();
    // Corrupt the last character
    let last_char = if zsa_str.ends_with('0') { '1' } else { '0' };
    zsa_str.pop();
    zsa_str.push(last_char);

    let keys = zeltra_core::generate_joinsplit_circuit_keys().unwrap();

    let err = wallet.prepare_shielded_transfer_proof(
        &zsa_str,
        5_000_000,
        1,
        20_000,
        "0x1111111111111111111111111111111111111111111111111111111111111111",
        421614,
        "0x2222222222222222222222222222222222222222",
        2000,
        1000,
        &keys.proving_key,
    );
    assert!(matches!(err, Err(WalletError::CryptoError(_))));
}

#[test]
fn test_e2e_shielded_transfer_flow() {
    let alice_seed = [21u8; 32];
    let mut alice = PrivateNoteWallet::new(&alice_seed);
    let bob_seed = [22u8; 32];
    let mut bob = PrivateNoteWallet::new(&bob_seed);

    // 1. Alice deposits 10 USDC (10_000_000)
    let (_, cm_alice) = alice.create_deposit_note(10_000_000, 1000).unwrap();
    alice
        .confirm_deposit(&cm_alice, 0, dummy_merkle_path(), "0xroot")
        .unwrap();
    assert_eq!(alice.balance(), 10_000_000);
    assert_eq!(bob.balance(), 0);

    // 2. Bob exports shielded address
    let bob_zsa = bob.shielded_address().unwrap();
    let bob_zsa_bech32m = bob_zsa.to_bech32m().unwrap();

    // 3. Alice prepares shielded transfer proof: 5 USDC with 20_000 fee
    let keys = zeltra_core::generate_joinsplit_circuit_keys().unwrap();
    let payload = alice
        .prepare_shielded_transfer_proof(
            &bob_zsa_bech32m,
            5_000_000, // 5 USDC
            1,         // USDC
            20_000,    // fee
            "0x1111111111111111111111111111111111111111111111111111111111111111",
            421614,
            "0x2222222222222222222222222222222222222222",
            2000,
            1000,
            &keys.proving_key,
        )
        .unwrap();

    // Verify dual memo 96 bytes (192 hex chars)
    assert_eq!(payload.encrypted_memo_1.len(), 192);
    assert_eq!(payload.encrypted_memo_2.len(), 192);
    assert_eq!(
        payload.recipient_hex,
        "0x0000000000000000000000000000000000000000"
    );
    assert_eq!(payload.merchant_amount, 0);
    assert_eq!(payload.protocol_fee, 0);
    assert!(payload.has_change_1);
    assert!(payload.has_change_2);

    // 4. Verify Groth16 proof
    let mut pis = Vec::new();
    for h in &payload.public_inputs_hex {
        pis.push(parse_fr_from_hex(h).unwrap());
    }
    let from_evm_a = zeltra_core::from_evm_g1(
        &hex::decode(&payload.proof_a_neg_hex)
            .unwrap()
            .try_into()
            .unwrap(),
    )
    .unwrap();
    let from_evm_b = zeltra_core::from_evm_g2(
        &hex::decode(&payload.proof_b_hex)
            .unwrap()
            .try_into()
            .unwrap(),
    )
    .unwrap();
    let from_evm_c = zeltra_core::from_evm_g1(
        &hex::decode(&payload.proof_c_hex)
            .unwrap()
            .try_into()
            .unwrap(),
    )
    .unwrap();
    let proof = ark_groth16::Proof {
        a: -from_evm_a,
        b: from_evm_b,
        c: from_evm_c,
    };
    assert!(
        zeltra_core::verify_joinsplit_proof(&keys.verifying_key, &proof, &pis).unwrap(),
        "Groth16 Mode 2 transfer proof must be valid"
    );

    // 5. Alice commits spend -> Alice balance = 10_000_000 - 5_000_000 - 20_000 = 4_980_000
    alice
        .commit_spend(
            &payload.session_id,
            &payload.input_nullifier_1_hex,
            1005,
            Some(2),
            Some(dummy_merkle_path()),
            Some("0xnew_root"),
        )
        .unwrap();
    assert_eq!(alice.balance(), 4_980_000);

    // 6. Bob scans MMR logs with trial decryption
    let memo_1_bytes = hex::decode(&payload.encrypted_memo_1).unwrap();
    let event_bob = NoteEvent {
        leaf_index: 1,
        commitment_hex: payload.output_commitment_1_hex.clone(),
        new_mmr_root_hex: "0xroot2".into(),
        leaf_count: 2,
        encrypted_memo: memo_1_bytes,
        block_number: Some(101),
    };
    let rec_bob = bob
        .recover_notes_batch(&[event_bob], |_| Ok(vec![false]))
        .unwrap();
    assert_eq!(rec_bob.recovered_notes.len(), 1);
    assert_eq!(bob.balance(), 5_000_000);
}

#[test]
fn test_exact_shielded_transfer_without_change() {
    let alice_seed = [31u8; 32];
    let mut alice = PrivateNoteWallet::new(&alice_seed);
    let bob_seed = [32u8; 32];
    let mut bob = PrivateNoteWallet::new(&bob_seed);

    // Alice has 10 USDC (10_000_000)
    let (_, cm_alice) = alice.create_deposit_note(10_000_000, 1000).unwrap();
    alice
        .confirm_deposit(&cm_alice, 0, dummy_merkle_path(), "0xroot")
        .unwrap();

    let bob_zsa = bob.shielded_address().unwrap();
    let bob_zsa_bech32m = bob_zsa.to_bech32m().unwrap();
    let keys = zeltra_core::generate_joinsplit_circuit_keys().unwrap();

    // Exact transfer: 9_980_000 + 20_000 fee = 10_000_000 (change_amount == 0)
    let payload = alice
        .prepare_shielded_transfer_proof(
            &bob_zsa_bech32m,
            9_980_000,
            1,
            20_000,
            "0x1111111111111111111111111111111111111111111111111111111111111111",
            421614,
            "0x2222222222222222222222222222222222222222",
            2000,
            1000,
            &keys.proving_key,
        )
        .unwrap();

    assert!(!payload.has_change_2);
    assert_eq!(payload.encrypted_memo_2.len(), 192); // Still 96 bytes random dummy

    // Alice commits spend
    alice
        .commit_spend(
            &payload.session_id,
            &payload.input_nullifier_1_hex,
            1005,
            None,
            None,
            None,
        )
        .unwrap();
    assert_eq!(alice.balance(), 0);

    // Bob scans Output 1
    let memo_1_bytes = hex::decode(&payload.encrypted_memo_1).unwrap();
    let event_bob = NoteEvent {
        leaf_index: 1,
        commitment_hex: payload.output_commitment_1_hex.clone(),
        new_mmr_root_hex: "0xroot2".into(),
        leaf_count: 2,
        encrypted_memo: memo_1_bytes,
        block_number: Some(101),
    };
    bob.recover_notes_batch(&[event_bob], |_| Ok(vec![false]))
        .unwrap();
    assert_eq!(bob.balance(), 9_980_000);

    // Alice scans Output 2 (dummy zero note) -> must be skipped, not adding phantom note
    let memo_2_bytes = hex::decode(&payload.encrypted_memo_2).unwrap();
    let event_dummy = NoteEvent {
        leaf_index: 2,
        commitment_hex: payload.output_commitment_2_hex.clone(),
        new_mmr_root_hex: "0xroot3".into(),
        leaf_count: 3,
        encrypted_memo: memo_2_bytes,
        block_number: Some(102),
    };
    let rec_alice = alice
        .recover_notes_batch(&[event_dummy], |_| Ok(vec![false]))
        .unwrap();
    assert_eq!(rec_alice.recovered_notes.len(), 0);
    assert_eq!(alice.balance(), 0);
}

#[test]
fn test_ccip_inbound_deposit_usdc_preparation_and_staging() {
    let seed = [21u8; 32];
    let mut wallet = PrivateNoteWallet::new(&seed);
    let dest_contract = [0x11u8; 20];
    let base_usdc = [0x22u8; 20];
    let base_chain_selector = 10344971235874465080u64;

    // 100 USDC = 100_000_000 micro-units
    let deposit = wallet
        .prepare_ccip_inbound_deposit(
            100_000_000,
            1, // USDC
            base_chain_selector,
            dest_contract,
            base_usdc,
            None,
        )
        .unwrap();

    // Verify wire payload structure (DEC-055 exact 128 bytes)
    assert_eq!(deposit.wire_payload_128.len(), 128);
    assert_eq!(&deposit.wire_payload_128[0..32], &deposit.commitment_bytes);
    assert_eq!(&deposit.wire_payload_128[32..128], &deposit.encrypted_memo[..]);
    assert_eq!(deposit.encrypted_memo.len(), 96);
    assert_eq!(deposit.destination_chain_selector, base_chain_selector);
    assert_eq!(deposit.destination_contract, dest_contract);
    assert_eq!(deposit.source_token, base_usdc);

    // Note must be staged as PendingDeposit (not affecting spendable balance)
    assert_eq!(wallet.balance(), 0);
    let cm_hex = hex::encode(deposit.commitment_bytes);
    let note = wallet.notes.get(&cm_hex).expect("Staged note must exist");
    assert_eq!(note.status, NoteStatus::PendingDeposit);
    assert_eq!(note.value, 100_000_000);
    assert_eq!(note.asset_id, 1);

    // Once mined on Arbitrum and confirmed via event, note transitions to Unspent
    let path = dummy_merkle_path();
    wallet
        .confirm_deposit(&cm_hex, 0, path, "0xmmr_root")
        .unwrap();
    assert_eq!(wallet.balance(), 100_000_000);
    let confirmed_note = wallet.notes.get(&cm_hex).unwrap();
    assert_eq!(confirmed_note.status, NoteStatus::Unspent);
}

#[test]
fn test_ccip_inbound_deposit_weth_and_negative_bounds() {
    let seed = [22u8; 32];
    let mut wallet = PrivateNoteWallet::new(&seed);
    let dest_contract = [0x33u8; 20];
    let eth_token = [0x44u8; 20];
    let eth_chain_selector = 16015286601757825753u64;

    // 0.5 ETH = 500_000_000 Gwei (valid)
    let deposit = wallet
        .prepare_ccip_inbound_deposit(
            500_000_000,
            0, // WETH
            eth_chain_selector,
            dest_contract,
            eth_token,
            None,
        )
        .unwrap();
    assert_eq!(deposit.wire_payload_128.len(), 128);
    assert_eq!(deposit.asset_id, 0);

    // Negative: USDC amount too small (< 10 USDC)
    let too_small_usdc = wallet.prepare_ccip_inbound_deposit(
        5_000_000,
        1,
        eth_chain_selector,
        dest_contract,
        eth_token,
        None,
    );
    assert!(too_small_usdc.is_err());

    // Negative: WETH amount too small (< 0.005 ETH = 5_000_000 Gwei)
    let too_small_weth = wallet.prepare_ccip_inbound_deposit(
        4_999_999,
        0,
        eth_chain_selector,
        dest_contract,
        eth_token,
        None,
    );
    assert!(too_small_weth.is_err());

    // Negative: Unsupported asset_id
    let invalid_asset = wallet.prepare_ccip_inbound_deposit(
        100_000_000,
        2,
        eth_chain_selector,
        dest_contract,
        eth_token,
        None,
    );
    assert!(invalid_asset.is_err());
}

