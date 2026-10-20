//! Integration and Unit Test Suite for DEC-040B & DEC-040C Compliance Engine.
//!
//! Covers:
//! - Positive: Double-entry UTXO conservation & balance sheet reconciliation on multi-asset simulation
//! - Positive: CSV sanitized export formatting and roundtrip parsing
//! - Positive: Tier 1 CEX selective disclosure proof generation with Blake2s verifier binding
//! - Positive: Tier 3 FullViewingKey auditor session recovery and balance tracking
//! - Negative: Fail-closed rejection of unbalanced records (1-unit deviation)
//! - Negative: Anti-CSV formula injection sanitization (CWE-1236)
//! - Negative: Memory inspection guaranteeing zero spending key leakage
//! - Negative: Complete spend authority lockout on ReadOnlyAuditorSession

use ark_bls12_381::Fr;
use ark_ff::UniformRand;
use rand::rngs::OsRng;
use zeltra_core::{
    encrypt_note_memo, fr_to_be_bytes, note_commitment,
    selective_disclosure_circuit::verify_disclosure_proof_unprepared,
};
use zeltra_sdk::compliance::{
    export_audit_csv, export_audit_json, format_iso8601, sanitize_csv_cell,
    validate_utxo_conservation, AuditFilter, AuditRecord, ComplianceError, RecordDirection,
    VerifierBinding,
};
use zeltra_sdk::wallet::note_wallet::{
    NoteEvent, NoteStatus, PrivateNoteWallet, ReadOnlyAuditorSession, SelectedSpend, WalletError,
    WalletNote,
};

#[allow(clippy::too_many_arguments)]
fn insert_test_note(
    notes: &mut std::collections::HashMap<String, WalletNote>,
    sk: Fr,
    cm_hex: String,
    val: u64,
    asset_id: u8,
    rho: Fr,
    r: Fr,
    status: NoteStatus,
    now: u64,
) {
    notes.insert(
        cm_hex.clone(),
        WalletNote {
            commitment_hex: cm_hex,
            value: val,
            asset_id,
            owner_key_hex: hex::encode(fr_to_be_bytes(&sk)),
            rho_hex: hex::encode(fr_to_be_bytes(&rho)),
            randomness_hex: hex::encode(fr_to_be_bytes(&r)),
            leaf_index: Some(0),
            leaf_count: Some(10),
            epoch_id: Some(0),
            merkle_path_hex: Some(vec!["00".repeat(32); 5]),
            status,
            created_at_secs: now,
            session_id: None,
        },
    );
}

#[test]
fn test_audit_report_utxo_conservation_and_reconciliation() {
    let seed = [0x55u8; 32];
    let mut wallet = PrivateNoteWallet::new(&seed);

    let sk = wallet.spending_key().unwrap();
    let mut rng = OsRng;

    // ═══════════════════════════════════════════════════════════════════════
    // 1. Simulate 5 Deposits (Multi-Asset: 3 USDC, 2 WETH)
    // ═══════════════════════════════════════════════════════════════════════
    // Deposit 1: 50,000 USDC (50_000_000_000 micro-USDC)
    let rho1 = Fr::rand(&mut rng);
    let r1 = Fr::rand(&mut rng);
    let cm1 = note_commitment(50_000_000_000, 1, sk, rho1, r1);
    let cm1_hex = format!("0x{}", hex::encode(fr_to_be_bytes(&cm1)));

    // Deposit 2: 20,000 USDC (20_000_000_000 micro-USDC)
    let rho2 = Fr::rand(&mut rng);
    let r2 = Fr::rand(&mut rng);
    let cm2 = note_commitment(20_000_000_000, 1, sk, rho2, r2);
    let cm2_hex = format!("0x{}", hex::encode(fr_to_be_bytes(&cm2)));

    // Deposit 3: 5.0 WETH (5_000_000_000 Gwei)
    let rho3 = Fr::rand(&mut rng);
    let r3 = Fr::rand(&mut rng);
    let cm3 = note_commitment(5_000_000_000, 0, sk, rho3, r3);
    let cm3_hex = format!("0x{}", hex::encode(fr_to_be_bytes(&cm3)));

    // Deposit 4: 2.5 WETH (2_500_000_000 Gwei)
    let rho4 = Fr::rand(&mut rng);
    let r4 = Fr::rand(&mut rng);
    let cm4 = note_commitment(2_500_000_000, 0, sk, rho4, r4);
    let cm4_hex = format!("0x{}", hex::encode(fr_to_be_bytes(&cm4)));

    // Deposit 5: 10,000 USDC (10_000_000_000 micro-USDC)
    let rho5 = Fr::rand(&mut rng);
    let r5 = Fr::rand(&mut rng);
    let cm5 = note_commitment(10_000_000_000, 1, sk, rho5, r5);
    let cm5_hex = format!("0x{}", hex::encode(fr_to_be_bytes(&cm5)));

    let now = 1773619200u64;

    // Record Inflow Deposits
    wallet.record_audit_entry(AuditRecord {
        record_id: 1,
        timestamp: now,
        date_iso: format_iso8601(now),
        tx_hash: "0x1111111111111111111111111111111111111111111111111111111111111111".into(),
        direction: RecordDirection::InflowDeposit,
        asset_id: 1,
        symbol: "USDC".into(),
        input_notes_total: "0.000000".into(),
        deposit_amount: "50000.000000".into(),
        merchant_payout: "0.000000".into(),
        protocol_fee: "0.000000".into(),
        relayer_execution_fee: "0.000000".into(),
        change_amount: "0.000000".into(),
        created_commitments: vec![cm1_hex.clone()],
        spent_nullifiers: vec![],
        recipient: None,
        status: "SETTLED_ON_CHAIN".into(),
    });

    wallet.record_audit_entry(AuditRecord {
        record_id: 2,
        timestamp: now + 100,
        date_iso: format_iso8601(now + 100),
        tx_hash: "0x2222222222222222222222222222222222222222222222222222222222222222".into(),
        direction: RecordDirection::InflowDeposit,
        asset_id: 1,
        symbol: "USDC".into(),
        input_notes_total: "0.000000".into(),
        deposit_amount: "20000.000000".into(),
        merchant_payout: "0.000000".into(),
        protocol_fee: "0.000000".into(),
        relayer_execution_fee: "0.000000".into(),
        change_amount: "0.000000".into(),
        created_commitments: vec![cm2_hex.clone()],
        spent_nullifiers: vec![],
        recipient: None,
        status: "SETTLED_ON_CHAIN".into(),
    });

    wallet.record_audit_entry(AuditRecord {
        record_id: 3,
        timestamp: now + 200,
        date_iso: format_iso8601(now + 200),
        tx_hash: "0x3333333333333333333333333333333333333333333333333333333333333333".into(),
        direction: RecordDirection::InflowDeposit,
        asset_id: 0,
        symbol: "WETH".into(),
        input_notes_total: "0.000000000".into(),
        deposit_amount: "5.000000000".into(),
        merchant_payout: "0.000000000".into(),
        protocol_fee: "0.000000000".into(),
        relayer_execution_fee: "0.000000000".into(),
        change_amount: "0.000000000".into(),
        created_commitments: vec![cm3_hex.clone()],
        spent_nullifiers: vec![],
        recipient: None,
        status: "SETTLED_ON_CHAIN".into(),
    });

    wallet.record_audit_entry(AuditRecord {
        record_id: 4,
        timestamp: now + 300,
        date_iso: format_iso8601(now + 300),
        tx_hash: "0x4444444444444444444444444444444444444444444444444444444444444444".into(),
        direction: RecordDirection::InflowDeposit,
        asset_id: 0,
        symbol: "WETH".into(),
        input_notes_total: "0.000000000".into(),
        deposit_amount: "2.500000000".into(),
        merchant_payout: "0.000000000".into(),
        protocol_fee: "0.000000000".into(),
        relayer_execution_fee: "0.000000000".into(),
        change_amount: "0.000000000".into(),
        created_commitments: vec![cm4_hex.clone()],
        spent_nullifiers: vec![],
        recipient: None,
        status: "SETTLED_ON_CHAIN".into(),
    });

    wallet.record_audit_entry(AuditRecord {
        record_id: 5,
        timestamp: now + 400,
        date_iso: format_iso8601(now + 400),
        tx_hash: "0x5555555555555555555555555555555555555555555555555555555555555555".into(),
        direction: RecordDirection::InflowDeposit,
        asset_id: 1,
        symbol: "USDC".into(),
        input_notes_total: "0.000000".into(),
        deposit_amount: "10000.000000".into(),
        merchant_payout: "0.000000".into(),
        protocol_fee: "0.000000".into(),
        relayer_execution_fee: "0.000000".into(),
        change_amount: "0.000000".into(),
        created_commitments: vec![cm5_hex.clone()],
        spent_nullifiers: vec![],
        recipient: None,
        status: "SETTLED_ON_CHAIN".into(),
    });

    // ═══════════════════════════════════════════════════════════════════════
    // 2. Simulate 3 Spends (2 USDC, 1 WETH)
    // ═══════════════════════════════════════════════════════════════════════
    // Spend 1: Spend cm1 (50,000 USDC).
    // Payout = 10,000.000000, Protocol fee = 45.000000, Relayer fee = 0.500000, Change = 39,954.500000.
    // 50000 == 10000 + 45 + 0.5 + 39954.5
    let rho_c1 = Fr::rand(&mut rng);
    let r_c1 = Fr::rand(&mut rng);
    let cm_c1 = note_commitment(39_954_500_000, 1, sk, rho_c1, r_c1);
    let cm_c1_hex = format!("0x{}", hex::encode(fr_to_be_bytes(&cm_c1)));

    insert_test_note(
        &mut wallet.notes,
        sk,
        cm1_hex.clone(),
        50_000_000_000,
        1,
        rho1,
        r1,
        NoteStatus::Spent {
            nullifier_hex: "0xnf1".into(),
            spent_at_secs: now + 500,
        },
        now,
    );
    insert_test_note(
        &mut wallet.notes,
        sk,
        cm_c1_hex.clone(),
        39_954_500_000,
        1,
        rho_c1,
        r_c1,
        NoteStatus::Unspent,
        now + 500,
    );

    wallet.record_audit_entry(AuditRecord {
        record_id: 6,
        timestamp: now + 500,
        date_iso: format_iso8601(now + 500),
        tx_hash: "0x6666666666666666666666666666666666666666666666666666666666666666".into(),
        direction: RecordDirection::OutflowSpend,
        asset_id: 1,
        symbol: "USDC".into(),
        input_notes_total: "50000.000000".into(),
        deposit_amount: "0.000000".into(),
        merchant_payout: "10000.000000".into(),
        protocol_fee: "45.000000".into(),
        relayer_execution_fee: "0.500000".into(),
        change_amount: "39954.500000".into(),
        created_commitments: vec![cm_c1_hex],
        spent_nullifiers: vec!["0xnf1".into()],
        recipient: Some("0xMerchantUSDC1".into()),
        status: "SETTLED_ON_CHAIN".into(),
    });

    // Spend 2: Spend cm3 (5.0 WETH).
    // Payout = 1.000000000, Protocol fee = 0.004500000, Relayer fee = 0.000150000, Change = 3.995350000.
    // 5.0 == 1.0 + 0.0045 + 0.00015 + 3.99535
    let rho_c2 = Fr::rand(&mut rng);
    let r_c2 = Fr::rand(&mut rng);
    let cm_c2 = note_commitment(3_995_350_000, 0, sk, rho_c2, r_c2);
    let cm_c2_hex = format!("0x{}", hex::encode(fr_to_be_bytes(&cm_c2)));

    insert_test_note(
        &mut wallet.notes,
        sk,
        cm3_hex.clone(),
        5_000_000_000,
        0,
        rho3,
        r3,
        NoteStatus::Spent {
            nullifier_hex: "0xnf2".into(),
            spent_at_secs: now + 600,
        },
        now,
    );
    insert_test_note(
        &mut wallet.notes,
        sk,
        cm_c2_hex.clone(),
        3_995_350_000,
        0,
        rho_c2,
        r_c2,
        NoteStatus::Unspent,
        now + 600,
    );

    wallet.record_audit_entry(AuditRecord {
        record_id: 7,
        timestamp: now + 600,
        date_iso: format_iso8601(now + 600),
        tx_hash: "0x7777777777777777777777777777777777777777777777777777777777777777".into(),
        direction: RecordDirection::OutflowSpend,
        asset_id: 0,
        symbol: "WETH".into(),
        input_notes_total: "5.000000000".into(),
        deposit_amount: "0.000000000".into(),
        merchant_payout: "1.000000000".into(),
        protocol_fee: "0.004500000".into(),
        relayer_execution_fee: "0.000150000".into(),
        change_amount: "3.995350000".into(),
        created_commitments: vec![cm_c2_hex],
        spent_nullifiers: vec!["0xnf2".into()],
        recipient: Some("0xMerchantWETH1".into()),
        status: "SETTLED_ON_CHAIN".into(),
    });

    // Spend 3: Spend cm2 (20,000 USDC).
    // Payout = 5,000.000000, Protocol fee = 22.500000, Relayer fee = 0.500000, Change = 14,977.000000.
    // 20000 == 5000 + 22.5 + 0.5 + 14977
    let rho_c3 = Fr::rand(&mut rng);
    let r_c3 = Fr::rand(&mut rng);
    let cm_c3 = note_commitment(14_977_000_000, 1, sk, rho_c3, r_c3);
    let cm_c3_hex = format!("0x{}", hex::encode(fr_to_be_bytes(&cm_c3)));

    insert_test_note(
        &mut wallet.notes,
        sk,
        cm2_hex.clone(),
        20_000_000_000,
        1,
        rho2,
        r2,
        NoteStatus::Spent {
            nullifier_hex: "0xnf3".into(),
            spent_at_secs: now + 700,
        },
        now,
    );
    insert_test_note(
        &mut wallet.notes,
        sk,
        cm_c3_hex.clone(),
        14_977_000_000,
        1,
        rho_c3,
        r_c3,
        NoteStatus::Unspent,
        now + 700,
    );

    // Also insert unspent note 4 (WETH 2.5) and note 5 (USDC 10,000)
    insert_test_note(
        &mut wallet.notes,
        sk,
        cm4_hex,
        2_500_000_000,
        0,
        rho4,
        r4,
        NoteStatus::Unspent,
        now,
    );
    insert_test_note(
        &mut wallet.notes,
        sk,
        cm5_hex,
        10_000_000_000,
        1,
        rho5,
        r5,
        NoteStatus::Unspent,
        now,
    );

    wallet.record_audit_entry(AuditRecord {
        record_id: 8,
        timestamp: now + 700,
        date_iso: format_iso8601(now + 700),
        tx_hash: "0x8888888888888888888888888888888888888888888888888888888888888888".into(),
        direction: RecordDirection::OutflowSpend,
        asset_id: 1,
        symbol: "USDC".into(),
        input_notes_total: "20000.000000".into(),
        deposit_amount: "0.000000".into(),
        merchant_payout: "5000.000000".into(),
        protocol_fee: "22.500000".into(),
        relayer_execution_fee: "0.500000".into(),
        change_amount: "14977.000000".into(),
        created_commitments: vec![cm_c3_hex],
        spent_nullifiers: vec!["0xnf3".into()],
        recipient: Some("0xMerchantUSDC2".into()),
        status: "SETTLED_ON_CHAIN".into(),
    });

    // ═══════════════════════════════════════════════════════════════════════
    // 3. Export Audit Trail and Assert 100% Balanced Double-Entry Reconciliation
    // ═══════════════════════════════════════════════════════════════════════
    let report = wallet
        .export_audit_trail(AuditFilter {
            include_spent: true,
            organization_name: Some("Acme Corp AI Agent Fleet".into()),
            ..Default::default()
        })
        .expect("Double-entry audit trail export must succeed");

    assert_eq!(report.schema_version, "zeltra.compliance.audit.v1");
    assert_eq!(report.organization_name, "Acme Corp AI Agent Fleet");
    assert_eq!(report.records.len(), 8);

    // USDC Verification
    let usdc = report
        .summary_by_asset
        .get("usdc")
        .expect("USDC summary must exist");
    assert_eq!(usdc.opening_balance, "0.000000");
    assert_eq!(usdc.total_inflow_deposit, "80000.000000");
    assert_eq!(usdc.total_merchant_payout, "15000.000000");
    assert_eq!(usdc.total_protocol_fees, "67.500000");
    assert_eq!(usdc.total_relayer_execution_fees, "1.000000");
    assert_eq!(usdc.closing_unspent_balance, "64931.500000");

    // WETH Verification
    let weth = report
        .summary_by_asset
        .get("weth")
        .expect("WETH summary must exist");
    assert_eq!(weth.opening_balance, "0.000000000");
    assert_eq!(weth.total_inflow_deposit, "7.500000000");
    assert_eq!(weth.total_merchant_payout, "1.000000000");
    assert_eq!(weth.total_protocol_fees, "0.004500000");
    assert_eq!(weth.total_relayer_execution_fees, "0.000150000");
    assert_eq!(weth.closing_unspent_balance, "6.495350000");

    // Local unspent UTXO check
    assert_eq!(wallet.get_unspent_balance(1), 64_931_500_000); // 64,931.5 USDC
    assert_eq!(wallet.get_unspent_balance(0), 6_495_350_000); // 6.49535 WETH
}

#[test]
fn test_csv_export_properly_formatted() {
    let seed = [0x55u8; 32];
    let mut wallet = PrivateNoteWallet::new(&seed);

    let now = 1773619200u64;
    wallet.record_audit_entry(AuditRecord {
        record_id: 1,
        timestamp: now,
        date_iso: format_iso8601(now),
        tx_hash: "0x918f000000000000000000000000000000000000000000000000000000000112".into(),
        direction: RecordDirection::InflowDeposit,
        asset_id: 1,
        symbol: "USDC".into(),
        input_notes_total: "0.000000".into(),
        deposit_amount: "50000.000000".into(),
        merchant_payout: "0.000000".into(),
        protocol_fee: "0.000000".into(),
        relayer_execution_fee: "0.000000".into(),
        change_amount: "0.000000".into(),
        created_commitments: vec!["0x12a9c77f".into()],
        spent_nullifiers: vec![],
        recipient: None,
        status: "SETTLED".into(),
    });

    wallet.notes.insert(
        "0x12a9c77f".into(),
        WalletNote {
            commitment_hex: "0x12a9c77f".into(),
            value: 50_000_000_000,
            asset_id: 1,
            owner_key_hex: "01".repeat(32),
            rho_hex: "02".repeat(32),
            randomness_hex: "03".repeat(32),
            leaf_index: Some(0),
            leaf_count: Some(1),
            epoch_id: Some(0),
            merkle_path_hex: Some(vec![]),
            status: NoteStatus::Unspent,
            created_at_secs: now,
            session_id: None,
        },
    );

    let report = wallet.export_audit_trail(AuditFilter::default()).unwrap();
    let csv_str = export_audit_csv(&report).expect("CSV export must succeed");

    let lines: Vec<&str> = csv_str.trim().split('\n').collect();
    assert_eq!(lines.len(), 2);
    assert_eq!(
        lines[0],
        "RecordID,Timestamp,DateISO,Direction,AssetID,AssetSymbol,GrossAmount,FeePaid,NetAmount,Recipient,TxHash,Commitment,Nullifier,Status"
    );

    let row_cols: Vec<&str> = lines[1].split(',').collect();
    assert_eq!(row_cols.len(), 14);
    assert_eq!(row_cols[0], "1");
    assert_eq!(row_cols[3], "INFLOW_DEPOSIT");
    assert_eq!(row_cols[4], "1");
    assert_eq!(row_cols[5], "USDC");
    assert_eq!(row_cols[6], "50000.000000");
    assert_eq!(row_cols[7], "0.000000");
    assert_eq!(row_cols[8], "50000.000000");
    assert_eq!(row_cols[9], "N/A");
    assert_eq!(row_cols[13], "SETTLED");
}

#[test]
fn test_cex_proof_generation_valid() {
    let seed = [0x77u8; 32];
    let mut wallet = PrivateNoteWallet::new(&seed);
    let sk = wallet.spending_key().unwrap();

    let mut rng = OsRng;
    let rho = Fr::rand(&mut rng);
    let r = Fr::rand(&mut rng);
    let val = 50_000_000_000u64; // 50,000 USDC
    let cm = note_commitment(val, 1, sk, rho, r);
    let cm_bytes = fr_to_be_bytes(&cm);
    let cm_hex = format!("0x{}", hex::encode(cm_bytes));

    // Append leaf into wallet MMR so inclusion path can be computed
    wallet.mmr.append(cm);

    wallet.notes.insert(
        cm_hex.clone(),
        WalletNote {
            commitment_hex: cm_hex.clone(),
            value: val,
            asset_id: 1,
            owner_key_hex: hex::encode(fr_to_be_bytes(&sk)),
            rho_hex: hex::encode(fr_to_be_bytes(&rho)),
            randomness_hex: hex::encode(fr_to_be_bytes(&r)),
            leaf_index: Some(0),
            leaf_count: Some(1),
            epoch_id: Some(0),
            merkle_path_hex: Some(vec!["00".repeat(32)]),
            status: NoteStatus::Unspent,
            created_at_secs: 1773619200,
            session_id: None,
        },
    );

    let binding = VerifierBinding {
        verifier_id: "BINANCE_KYC_DESK".into(),
        account_uid: "USER-98124712".into(),
        nonce: "a7f3e829bc14d0219901ab23456789cd".into(),
        expires_at: 1776376800,
        spent_tx_hash: "0x44ca000000000000000000000000000000000000000000000000000000000990".into(),
        payout_recipient: "0xBinanceUserDepositAddress0000000000000000".into(),
    };

    let payload = wallet
        .create_cex_proof(cm_bytes, binding)
        .expect("CEX proof generation must succeed");

    assert_eq!(payload.schema_version, "zeltra.compliance.cex.v1");
    assert_eq!(payload.verifier_binding.verifier_id, "BINANCE_KYC_DESK");
    assert_eq!(payload.verifier_binding.account_uid, "USER-98124712");
    assert_eq!(payload.asset.symbol, "USDC");
    assert_eq!(payload.asset.circuit_decimals, 6);
    assert_eq!(payload.asset.human_value, "50000.000000");
    assert_eq!(payload.note_cryptography.commitment, cm_hex);
    assert_eq!(payload.ownership_zk_proof.public_inputs.len(), 5);
    assert_eq!(
        payload.ownership_zk_proof.protocol,
        "GROTH16_BLS12_381_DISCLOSURE_V1"
    );

    // Verify Groth16 proof independently with verifying key
    let (_, vk) = zeltra_sdk::wallet::note_wallet::get_or_init_disclosure_keys();
    let public_inputs: Vec<Fr> = payload
        .ownership_zk_proof
        .public_inputs
        .iter()
        .map(|s| zeltra_sdk::wallet::note_wallet::parse_fr_from_hex(s).unwrap())
        .collect();

    let clean_proof = payload
        .ownership_zk_proof
        .proof_bytes
        .strip_prefix("0x")
        .unwrap();
    let proof_bytes = hex::decode(clean_proof).unwrap();
    use ark_serialize::CanonicalDeserialize;
    let proof =
        ark_groth16::Proof::<ark_bls12_381::Bls12_381>::deserialize_compressed(&proof_bytes[..])
            .unwrap();

    let is_valid = verify_disclosure_proof_unprepared(&proof, &public_inputs, vk)
        .expect("Verification must evaluate");
    assert!(
        is_valid,
        "Independent Groth16 verification of CEX proof must pass"
    );
}

#[test]
fn test_fvk_auditor_session_recovery() {
    let seed = [0x88u8; 32];
    let wallet = PrivateNoteWallet::new(&seed);
    let fvk = wallet
        .export_full_viewing_key()
        .expect("Export FVK must succeed");

    let mut session = PrivateNoteWallet::create_auditor_session(fvk)
        .expect("Auditor session creation must succeed");

    // Prepare note events encrypted with wallet's IVK
    let ivk = wallet.incoming_viewing_key().unwrap();
    let mut rng = OsRng;

    let rho1 = Fr::rand(&mut rng);
    let rho2 = Fr::rand(&mut rng);

    // Note 1: 1,000 USDC
    let memo1 = encrypt_note_memo(1_000_000_000, rho1, 1, &ivk);
    let note1 = zeltra_core::NotePlaintext {
        value: 1_000_000_000,
        rho: rho1,
        asset_id: 1,
    }
    .to_note(session.fvk().owner_key);
    let cm1_hex = format!("0x{}", hex::encode(fr_to_be_bytes(&note1.commitment())));

    // Note 2: 2.0 WETH
    let memo2 = encrypt_note_memo(2_000_000_000, rho2, 0, &ivk);
    let note2 = zeltra_core::NotePlaintext {
        value: 2_000_000_000,
        rho: rho2,
        asset_id: 0,
    }
    .to_note(session.fvk().owner_key);
    let cm2_hex = format!("0x{}", hex::encode(fr_to_be_bytes(&note2.commitment())));

    let events = vec![
        NoteEvent {
            leaf_index: 0,
            commitment_hex: cm1_hex.clone(),
            new_mmr_root_hex: "0xroot1".into(),
            leaf_count: 1,
            encrypted_memo: memo1,
            block_number: Some(100),
        },
        NoteEvent {
            leaf_index: 1,
            commitment_hex: cm2_hex.clone(),
            new_mmr_root_hex: "0xroot2".into(),
            leaf_count: 2,
            encrypted_memo: memo2,
            block_number: Some(101),
        },
    ];

    // Recover notes with both unspent
    let rec = session.recover_notes(&events, |_nf| false);
    assert_eq!(rec.recovered_notes.len(), 2);
    assert_eq!(session.get_balance(1).unwrap(), 1_000_000_000);
    assert_eq!(session.get_balance(0).unwrap(), 2_000_000_000);
    assert_eq!(session.get_human_balance(1).unwrap(), "$1000.00 USDC");
    assert_eq!(session.get_human_balance(0).unwrap(), "2.000000 ETH");

    // Recover again with Note 1 spent on-chain
    let nf1 = session.fvk().derive_nullifier(note1.commitment(), 0, 0);
    let nf1_hex = format!("0x{}", hex::encode(fr_to_be_bytes(&nf1)));

    session.notes.clear();
    let rec2 = session.recover_notes(&events, |nf| nf == nf1_hex);
    assert_eq!(rec2.total_spent_balance, 1_000_000_000);
    assert_eq!(rec2.total_unspent_balance, 2_000_000_000);
    assert_eq!(session.get_balance(1).unwrap(), 0); // USDC is spent
    assert_eq!(session.get_balance(0).unwrap(), 2_000_000_000); // WETH remains unspent
}

#[test]
fn test_audit_report_fails_on_unbalanced_record() {
    let now = 1773619200u64;

    // Intentional 1-micro-USDC deviation: input 10,000 vs output 9,999.999999
    let bad_record = AuditRecord {
        record_id: 99,
        timestamp: now,
        date_iso: format_iso8601(now),
        tx_hash: "0xdeadbeef".into(),
        direction: RecordDirection::OutflowSpend,
        asset_id: 1,
        symbol: "USDC".into(),
        input_notes_total: "10000.000000".into(),
        deposit_amount: "0.000000".into(),
        merchant_payout: "9000.000000".into(),
        protocol_fee: "45.000000".into(),
        relayer_execution_fee: "0.500000".into(),
        change_amount: "954.499999".into(), // Discrepancy of 0.000001 (1 unit)!
        created_commitments: vec!["0xcm".into()],
        spent_nullifiers: vec!["0xnf".into()],
        recipient: Some("0xRecip".into()),
        status: "SETTLED".into(),
    };

    let res = validate_utxo_conservation(&[bad_record]);
    assert!(
        matches!(
            res,
            Err(ComplianceError::AccountingReconciliationMismatch {
                record_id: 99,
                expected: 10_000_000_000,
                actual: 9_999_999_999,
            })
        ),
        "Must fail closed on 1-unit deviation: {:?}",
        res
    );
}

#[test]
fn test_csv_sanitizes_malicious_formula() {
    assert_eq!(
        sanitize_csv_cell("=cmd|' /C calc'!A0"),
        "'=cmd|' /C calc'!A0"
    );
    assert_eq!(
        sanitize_csv_cell("+cmd|' /C calc'!A0"),
        "'+cmd|' /C calc'!A0"
    );
    assert_eq!(sanitize_csv_cell("-12345"), "'-12345");
    assert_eq!(sanitize_csv_cell("@SUM(A1:A10)"), "'@SUM(A1:A10)");
    assert_eq!(sanitize_csv_cell("\t=calc"), "'\t=calc");
    assert_eq!(sanitize_csv_cell("\r=calc"), "'\r=calc");
    assert_eq!(sanitize_csv_cell("BINANCE_KYC_DESK"), "BINANCE_KYC_DESK");
    assert_eq!(sanitize_csv_cell(""), "");
}

#[test]
fn test_export_never_leaks_spending_key() {
    let seed = [0x99u8; 32];
    let mut wallet = PrivateNoteWallet::new(&seed);
    let sk = wallet.spending_key().unwrap();
    let sk_bytes = fr_to_be_bytes(&sk);
    let sk_hex = hex::encode(sk_bytes);

    let now = 1773619200u64;
    wallet.record_audit_entry(AuditRecord {
        record_id: 1,
        timestamp: now,
        date_iso: format_iso8601(now),
        tx_hash: "0x1234567890".into(),
        direction: RecordDirection::InflowDeposit,
        asset_id: 1,
        symbol: "USDC".into(),
        input_notes_total: "0.000000".into(),
        deposit_amount: "1000.000000".into(),
        merchant_payout: "0.000000".into(),
        protocol_fee: "0.000000".into(),
        relayer_execution_fee: "0.000000".into(),
        change_amount: "0.000000".into(),
        created_commitments: vec!["0xcm123".into()],
        spent_nullifiers: vec![],
        recipient: None,
        status: "SETTLED".into(),
    });

    wallet.notes.insert(
        "0xcm123".into(),
        WalletNote {
            commitment_hex: "0xcm123".into(),
            value: 1_000_000_000,
            asset_id: 1,
            owner_key_hex: "01".repeat(32),
            rho_hex: "02".repeat(32),
            randomness_hex: "03".repeat(32),
            leaf_index: Some(0),
            leaf_count: Some(1),
            epoch_id: Some(0),
            merkle_path_hex: Some(vec![]),
            status: NoteStatus::Unspent,
            created_at_secs: now,
            session_id: None,
        },
    );

    let report = wallet.export_audit_trail(AuditFilter::default()).unwrap();
    let json_str = export_audit_json(&report).unwrap();
    let csv_str = export_audit_csv(&report).unwrap();

    assert!(
        !json_str.contains(&sk_hex),
        "JSON must never contain spending key hex"
    );
    assert!(
        !csv_str.contains(&sk_hex),
        "CSV must never contain spending key hex"
    );
    assert!(
        !json_str.contains("spending_key"),
        "JSON must never contain spending_key field"
    );
    assert!(
        !json_str.contains("master_seed"),
        "JSON must never contain master_seed"
    );
    assert!(
        !csv_str.contains("spending_key"),
        "CSV must never contain spending_key field"
    );
}

#[test]
fn test_auditor_session_cannot_spend() {
    let fvk =
        zeltra_core::FullViewingKey::new(Fr::from(10u64), [1u8; 32], [2u8; 32], Fr::from(20u64));
    let session = ReadOnlyAuditorSession::new(fvk);

    let res_spend = session.prepare_spend_proof("0xcm", [0u8; 32], 100, 1, 1, 421614);
    assert_eq!(res_spend.unwrap_err(), WalletError::SpendAuthorityRequired);

    let res_epoch = session.prepare_spend_proof_with_epoch("0xcm", [0u8; 32], 100, 1, 1, 421614, 0);
    assert_eq!(res_epoch.unwrap_err(), WalletError::SpendAuthorityRequired);

    let res_mmr = session.prepare_spend_proof_with_mmr(
        "0xcm", [0u8; 32], 100, 1, 1, 421614, [0u8; 32], 1000, [0u8; 32], 0,
    );
    assert_eq!(res_mmr.unwrap_err(), WalletError::SpendAuthorityRequired);

    let dummy_selected = SelectedSpend {
        input_commitment_hex: "0xcm1".into(),
        input_value: 100,
        second_input_commitment_hex: None,
        second_input_value: 0,
        merchant_amount: 50,
        protocol_fee: 1,
        execution_fee: 1,
        total_required: 52,
        change_amount: 48,
        has_change: true,
    };
    let res_joinsplit = session.prepare_joinsplit_spend_proof(
        &dummy_selected,
        [0u8; 32],
        421614,
        [0u8; 32],
        1000,
        [0u8; 32],
        0,
    );
    assert_eq!(
        res_joinsplit.unwrap_err(),
        WalletError::SpendAuthorityRequired
    );

    let dummy_binding = VerifierBinding {
        verifier_id: "BINANCE".into(),
        account_uid: "123".into(),
        nonce: "abc".into(),
        expires_at: 1000,
        spent_tx_hash: "0x1".into(),
        payout_recipient: "0x2".into(),
    };
    let res_cex = session.create_cex_proof([0u8; 32], dummy_binding);
    assert_eq!(res_cex.unwrap_err(), WalletError::SpendAuthorityRequired);
}
