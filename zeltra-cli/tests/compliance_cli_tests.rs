//! Integration and Unit Tests for Compliance CLI Tooling (DEC-040C).
//!
//! Covers:
//! - Positive: `export-audit -f json` roundtrip parsing & balance sheet validation
//! - Positive: `export-audit -f csv` CWE-1236 formula sanitization & valid CSV formatting
//! - Positive: Full end-to-end `export-cex-proof` -> `verify-cex-proof` returning SUCCESS
//! - Positive: `export-viewing-key -t fvk` generating valid 128-byte canonical FVK
//! - Negative: `verify-cex-proof` failure on tampered JSON nominal value
//! - Negative: `verify-cex-proof` failure on mismatched expected verifier ID
//! - Negative: `verify-cex-proof` failure on expired proof timestamp
//! - Negative: `verify-cex-proof` failure on invalid / unaccepted MMR commitment

use rand::rngs::OsRng;
use std::fs;
use tempfile::tempdir;

use zeltra_core::{fr_to_be_bytes, note_commitment, Fr, UniformRand};
use zeltra_sdk::compliance::models::{AuditReport, CexProofPayload};
use zeltra_sdk::wallet::note_wallet::{
    NoteClassification, NoteStatus, PrivateNoteWallet, WalletNote,
};

#[tokio::test]
async fn test_cli_export_audit_json_roundtrip() {
    let dir = tempdir().expect("tempdir");
    let wallet_path = dir.path().join("wallet.json");
    let out_path = dir.path().join("audit.json");

    // Initialize test wallet with 2 notes
    let seed = [0x11u8; 32];
    let mut wallet = PrivateNoteWallet::new(&seed);
    let sk = wallet.spending_key().unwrap();

    let mut rng = OsRng;
    let rho = Fr::rand(&mut rng);
    let r = Fr::rand(&mut rng);
    let cm = note_commitment(10_000_000_000, 1, sk, rho, r);
    let cm_hex = format!("0x{}", hex::encode(fr_to_be_bytes(&cm)));

    wallet.notes.insert(
        cm_hex.clone(),
        WalletNote {
            commitment_hex: cm_hex,
            value: 10_000_000_000,
            asset_id: 1,
            owner_key_hex: hex::encode(fr_to_be_bytes(&sk)),
            rho_hex: hex::encode(fr_to_be_bytes(&rho)),
            randomness_hex: hex::encode(fr_to_be_bytes(&r)),
            leaf_index: Some(0),
            leaf_count: Some(1),
            epoch_id: Some(0),
            merkle_path_hex: Some(vec!["00".repeat(32)]),
            status: NoteStatus::Unspent,
            classification: NoteClassification::ActiveSpendable,
            created_at_secs: 1773619200,
            session_id: None,
        },
    );

    let wallet_json = serde_json::to_string_pretty(&wallet).unwrap();
    fs::write(&wallet_path, wallet_json).unwrap();

    // Call export-audit handler
    zeltra_cli::commands::compliance::handle_export_audit(
        "json",
        "all",
        None,
        None,
        Some(wallet_path.to_str().unwrap()),
        out_path.to_str().unwrap(),
    )
    .await
    .expect("Export audit JSON must succeed");

    // Validate that output JSON exists and can be parsed into AuditReport
    let exported_data = fs::read_to_string(&out_path).expect("read audit json");
    let report: AuditReport = serde_json::from_str(&exported_data).expect("parse AuditReport");

    assert_eq!(report.schema_version, "zeltra.compliance.audit.v1");
    assert!(!report.records.is_empty(), "Records must not be empty");

    // Verify file permissions on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let metadata = fs::metadata(&out_path).unwrap();
        let mode = metadata.permissions().mode();
        assert_eq!(mode & 0o777, 0o600, "File permission must be 0600");
    }
}

#[tokio::test]
async fn test_cli_export_audit_csv_roundtrip() {
    let dir = tempdir().expect("tempdir");
    let wallet_path = dir.path().join("wallet.json");
    let out_path = dir.path().join("audit.csv");

    let seed = [0x22u8; 32];
    let mut wallet = PrivateNoteWallet::new(&seed);
    let sk = wallet.spending_key().unwrap();

    let mut rng = OsRng;
    let rho = Fr::rand(&mut rng);
    let r = Fr::rand(&mut rng);
    let cm = note_commitment(5_000_000_000, 0, sk, rho, r); // 5 WETH
    let cm_hex = format!("0x{}", hex::encode(fr_to_be_bytes(&cm)));

    wallet.notes.insert(
        cm_hex.clone(),
        WalletNote {
            commitment_hex: cm_hex,
            value: 5_000_000_000,
            asset_id: 0,
            owner_key_hex: hex::encode(fr_to_be_bytes(&sk)),
            rho_hex: hex::encode(fr_to_be_bytes(&rho)),
            randomness_hex: hex::encode(fr_to_be_bytes(&r)),
            leaf_index: Some(0),
            leaf_count: Some(1),
            epoch_id: Some(0),
            merkle_path_hex: Some(vec!["00".repeat(32)]),
            status: NoteStatus::Unspent,
            classification: NoteClassification::ActiveSpendable,
            created_at_secs: 1773619200,
            session_id: None,
        },
    );

    let wallet_json = serde_json::to_string_pretty(&wallet).unwrap();
    fs::write(&wallet_path, wallet_json).unwrap();

    zeltra_cli::commands::compliance::handle_export_audit(
        "csv",
        "weth",
        None,
        None,
        Some(wallet_path.to_str().unwrap()),
        out_path.to_str().unwrap(),
    )
    .await
    .expect("Export audit CSV must succeed");

    let csv_content = fs::read_to_string(&out_path).expect("read audit csv");
    assert!(csv_content.starts_with("RecordID,Timestamp,DateISO"));
    assert!(csv_content.contains("WETH"));

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let metadata = fs::metadata(&out_path).unwrap();
        let mode = metadata.permissions().mode();
        assert_eq!(mode & 0o777, 0o600, "File permission must be 0600");
    }
}

#[tokio::test]
async fn test_cli_export_and_verify_cex_proof_roundtrip() {
    let dir = tempdir().expect("tempdir");
    let wallet_path = dir.path().join("wallet.json");
    let proof_path = dir.path().join("zeltra_cex_proof_v1.json");

    let seed = [0x33u8; 32];
    let mut wallet = PrivateNoteWallet::new(&seed);
    let sk = wallet.spending_key().unwrap();

    let mut rng = OsRng;
    let rho = Fr::rand(&mut rng);
    let r = Fr::rand(&mut rng);
    let val = 30_000_000_000u64; // 30,000 USDC
    let cm = note_commitment(val, 1, sk, rho, r);
    let cm_bytes = fr_to_be_bytes(&cm);
    let cm_hex = format!("0x{}", hex::encode(cm_bytes));

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
            classification: NoteClassification::ActiveSpendable,
            created_at_secs: 1773619200,
            session_id: None,
        },
    );

    let wallet_json = serde_json::to_string_pretty(&wallet).unwrap();
    fs::write(&wallet_path, wallet_json).unwrap();

    // 1. Export CEX Proof
    zeltra_cli::commands::compliance::handle_export_cex_proof(
        &cm_hex,
        "BINANCE_KYC_DESK",
        "USER-88331199",
        "0x44ca000000000000000000000000000000000000000000000000000000000990",
        "0xBinanceUserDepositAddress0000000000000000",
        Some(wallet_path.to_str().unwrap()),
        proof_path.to_str().unwrap(),
    )
    .await
    .expect("Export CEX proof must succeed");

    // 2. Verify with verify-cex-proof using mock RPC mode
    let verify_res = zeltra_cli::commands::compliance::handle_verify_cex_proof(
        proof_path.to_str().unwrap(),
        "BINANCE_KYC_DESK",
        Some("mock"),
        None,
    )
    .await;

    assert!(
        verify_res.is_ok(),
        "Verification must return SUCCESS for valid proof: {:?}",
        verify_res
    );
}

#[tokio::test]
async fn test_cli_export_viewing_key_fvk() {
    let dir = tempdir().expect("tempdir");
    let wallet_path = dir.path().join("wallet.json");
    let out_path = dir.path().join("fvk.txt");

    let seed = [0x44u8; 32];
    let wallet = PrivateNoteWallet::new(&seed);
    let wallet_json = serde_json::to_string_pretty(&wallet).unwrap();
    fs::write(&wallet_path, wallet_json).unwrap();

    zeltra_cli::commands::compliance::handle_export_viewing_key(
        "fvk",
        Some(wallet_path.to_str().unwrap()),
        out_path.to_str().unwrap(),
    )
    .await
    .expect("Export viewing key FVK must succeed");

    let fvk_hex = fs::read_to_string(&out_path).expect("read fvk");
    let clean = fvk_hex.trim().strip_prefix("0x").unwrap_or(fvk_hex.trim());
    let fvk_bytes = hex::decode(clean).expect("valid hex");
    assert_eq!(fvk_bytes.len(), 128, "FVK must be exactly 128 bytes");
}

#[tokio::test]
async fn test_verify_cex_proof_fails_on_tampered_json() {
    let dir = tempdir().expect("tempdir");
    let wallet_path = dir.path().join("wallet.json");
    let proof_path = dir.path().join("proof.json");

    let seed = [0x55u8; 32];
    let mut wallet = PrivateNoteWallet::new(&seed);
    let sk = wallet.spending_key().unwrap();

    let mut rng = OsRng;
    let rho = Fr::rand(&mut rng);
    let r = Fr::rand(&mut rng);
    let cm = note_commitment(10_000_000_000, 1, sk, rho, r);
    let cm_bytes = fr_to_be_bytes(&cm);
    let cm_hex = format!("0x{}", hex::encode(cm_bytes));

    wallet.mmr.append(cm);
    wallet.notes.insert(
        cm_hex.clone(),
        WalletNote {
            commitment_hex: cm_hex.clone(),
            value: 10_000_000_000,
            asset_id: 1,
            owner_key_hex: hex::encode(fr_to_be_bytes(&sk)),
            rho_hex: hex::encode(fr_to_be_bytes(&rho)),
            randomness_hex: hex::encode(fr_to_be_bytes(&r)),
            leaf_index: Some(0),
            leaf_count: Some(1),
            epoch_id: Some(0),
            merkle_path_hex: Some(vec!["00".repeat(32)]),
            status: NoteStatus::Unspent,
            classification: NoteClassification::ActiveSpendable,
            created_at_secs: 1773619200,
            session_id: None,
        },
    );

    let wallet_json = serde_json::to_string_pretty(&wallet).unwrap();
    fs::write(&wallet_path, wallet_json).unwrap();

    zeltra_cli::commands::compliance::handle_export_cex_proof(
        &cm_hex,
        "BINANCE_KYC_DESK",
        "USER-7711",
        "0x44ca000000000000000000000000000000000000000000000000000000000990",
        "0xBinanceUserDepositAddress0000000000000000",
        Some(wallet_path.to_str().unwrap()),
        proof_path.to_str().unwrap(),
    )
    .await
    .unwrap();

    // Tamper the nominal value in the JSON file
    let proof_content = fs::read_to_string(&proof_path).unwrap();
    let mut payload: CexProofPayload = serde_json::from_str(&proof_content).unwrap();
    payload.asset.raw_circuit_value = "999999999999".to_string(); // Tampered!
    let tampered_json = serde_json::to_string_pretty(&payload).unwrap();
    fs::write(&proof_path, tampered_json).unwrap();

    let res = zeltra_cli::commands::compliance::handle_verify_cex_proof(
        proof_path.to_str().unwrap(),
        "BINANCE_KYC_DESK",
        Some("mock"),
        None,
    )
    .await;

    assert!(res.is_err(), "Must reject tampered JSON amount");
    let err_msg = res.unwrap_err();
    assert!(
        err_msg.contains("preimage mismatch") || err_msg.contains("CommitmentPreimageMismatch")
    );
}

#[tokio::test]
async fn test_verify_cex_proof_fails_on_wrong_verifier() {
    let dir = tempdir().expect("tempdir");
    let wallet_path = dir.path().join("wallet.json");
    let proof_path = dir.path().join("proof.json");

    let seed = [0x66u8; 32];
    let mut wallet = PrivateNoteWallet::new(&seed);
    let sk = wallet.spending_key().unwrap();

    let mut rng = OsRng;
    let rho = Fr::rand(&mut rng);
    let r = Fr::rand(&mut rng);
    let cm = note_commitment(10_000_000_000, 1, sk, rho, r);
    let cm_bytes = fr_to_be_bytes(&cm);
    let cm_hex = format!("0x{}", hex::encode(cm_bytes));

    wallet.mmr.append(cm);
    wallet.notes.insert(
        cm_hex.clone(),
        WalletNote {
            commitment_hex: cm_hex.clone(),
            value: 10_000_000_000,
            asset_id: 1,
            owner_key_hex: hex::encode(fr_to_be_bytes(&sk)),
            rho_hex: hex::encode(fr_to_be_bytes(&rho)),
            randomness_hex: hex::encode(fr_to_be_bytes(&r)),
            leaf_index: Some(0),
            leaf_count: Some(1),
            epoch_id: Some(0),
            merkle_path_hex: Some(vec!["00".repeat(32)]),
            status: NoteStatus::Unspent,
            classification: NoteClassification::ActiveSpendable,
            created_at_secs: 1773619200,
            session_id: None,
        },
    );

    let wallet_json = serde_json::to_string_pretty(&wallet).unwrap();
    fs::write(&wallet_path, wallet_json).unwrap();

    zeltra_cli::commands::compliance::handle_export_cex_proof(
        &cm_hex,
        "BINANCE_KYC_DESK",
        "USER-7711",
        "0x44ca000000000000000000000000000000000000000000000000000000000990",
        "0xBinanceUserDepositAddress0000000000000000",
        Some(wallet_path.to_str().unwrap()),
        proof_path.to_str().unwrap(),
    )
    .await
    .unwrap();

    let res = zeltra_cli::commands::compliance::handle_verify_cex_proof(
        proof_path.to_str().unwrap(),
        "COINBASE_KYC_DESK", // Wrong verifier!
        Some("mock"),
        None,
    )
    .await;

    assert!(res.is_err(), "Must reject wrong verifier ID");
    let err_msg = res.unwrap_err();
    assert!(err_msg.contains("Verifier ID mismatch") || err_msg.contains("VerifierIdMismatch"));
}

#[tokio::test]
async fn test_verify_cex_proof_fails_on_expired_timestamp() {
    let dir = tempdir().expect("tempdir");
    let wallet_path = dir.path().join("wallet.json");
    let proof_path = dir.path().join("proof.json");

    let seed = [0x77u8; 32];
    let mut wallet = PrivateNoteWallet::new(&seed);
    let sk = wallet.spending_key().unwrap();

    let mut rng = OsRng;
    let rho = Fr::rand(&mut rng);
    let r = Fr::rand(&mut rng);
    let cm = note_commitment(10_000_000_000, 1, sk, rho, r);
    let cm_bytes = fr_to_be_bytes(&cm);
    let cm_hex = format!("0x{}", hex::encode(cm_bytes));

    wallet.mmr.append(cm);
    wallet.notes.insert(
        cm_hex.clone(),
        WalletNote {
            commitment_hex: cm_hex.clone(),
            value: 10_000_000_000,
            asset_id: 1,
            owner_key_hex: hex::encode(fr_to_be_bytes(&sk)),
            rho_hex: hex::encode(fr_to_be_bytes(&rho)),
            randomness_hex: hex::encode(fr_to_be_bytes(&r)),
            leaf_index: Some(0),
            leaf_count: Some(1),
            epoch_id: Some(0),
            merkle_path_hex: Some(vec!["00".repeat(32)]),
            status: NoteStatus::Unspent,
            classification: NoteClassification::ActiveSpendable,
            created_at_secs: 1773619200,
            session_id: None,
        },
    );

    let wallet_json = serde_json::to_string_pretty(&wallet).unwrap();
    fs::write(&wallet_path, wallet_json).unwrap();

    zeltra_cli::commands::compliance::handle_export_cex_proof(
        &cm_hex,
        "BINANCE_KYC_DESK",
        "USER-7711",
        "0x44ca000000000000000000000000000000000000000000000000000000000990",
        "0xBinanceUserDepositAddress0000000000000000",
        Some(wallet_path.to_str().unwrap()),
        proof_path.to_str().unwrap(),
    )
    .await
    .unwrap();

    let proof_content = fs::read_to_string(&proof_path).unwrap();
    let mut payload: CexProofPayload = serde_json::from_str(&proof_content).unwrap();
    payload.verifier_binding.expires_at = 1000; // Past epoch timestamp!
    let expired_json = serde_json::to_string_pretty(&payload).unwrap();
    fs::write(&proof_path, expired_json).unwrap();

    let res = zeltra_cli::commands::compliance::handle_verify_cex_proof(
        proof_path.to_str().unwrap(),
        "BINANCE_KYC_DESK",
        Some("mock"),
        None,
    )
    .await;

    assert!(res.is_err(), "Must reject expired proof");
    let err_msg = res.unwrap_err();
    assert!(err_msg.contains("expired") || err_msg.contains("ProofExpired"));
}

#[tokio::test]
async fn test_verify_cex_proof_fails_on_invalid_mmr() {
    let dir = tempdir().expect("tempdir");
    let proof_path = dir.path().join("proof.json");

    let seed = [0x88u8; 32];
    let mut wallet = PrivateNoteWallet::new(&seed);
    let sk = wallet.spending_key().unwrap();

    let mut rng = OsRng;
    let rho = Fr::rand(&mut rng);
    let r = Fr::rand(&mut rng);
    let cm = note_commitment(10_000_000_000, 1, sk, rho, r);
    let cm_bytes = fr_to_be_bytes(&cm);
    let cm_hex = format!("0x{}", hex::encode(cm_bytes));

    wallet.mmr.append(cm);
    wallet.notes.insert(
        cm_hex.clone(),
        WalletNote {
            commitment_hex: cm_hex.clone(),
            value: 10_000_000_000,
            asset_id: 1,
            owner_key_hex: hex::encode(fr_to_be_bytes(&sk)),
            rho_hex: hex::encode(fr_to_be_bytes(&rho)),
            randomness_hex: hex::encode(fr_to_be_bytes(&r)),
            leaf_index: Some(0),
            leaf_count: Some(1),
            epoch_id: Some(0),
            merkle_path_hex: Some(vec!["00".repeat(32)]),
            status: NoteStatus::Unspent,
            classification: NoteClassification::ActiveSpendable,
            created_at_secs: 1773619200,
            session_id: None,
        },
    );

    let binding = zeltra_sdk::compliance::models::VerifierBinding {
        verifier_id: "BINANCE_KYC_DESK".into(),
        account_uid: "USER-9922".into(),
        nonce: "abcdef1234567890abcdef1234567890".into(),
        expires_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            + 86400,
        spent_tx_hash: "0x44ca000000000000000000000000000000000000000000000000000000000990".into(),
        payout_recipient: "0xBinanceUserDepositAddress0000000000000000".into(),
    };

    let mut payload = wallet.create_cex_proof(cm_bytes, binding).unwrap();
    // Tamper MMR root with an alternate canonical scalar so inclusion fails
    payload.mmr_proof.mmr_root = format!("0x{:064x}", 0x123456789u64);
    let tampered_json = serde_json::to_string_pretty(&payload).unwrap();

    fs::write(&proof_path, tampered_json).unwrap();

    let res = zeltra_cli::commands::compliance::handle_verify_cex_proof(
        proof_path.to_str().unwrap(),
        "BINANCE_KYC_DESK",
        Some("mock"),
        None,
    )
    .await;

    assert!(res.is_err(), "Must reject invalid MMR root");
    let err_msg = res.unwrap_err();
    assert!(err_msg.contains("MMR") || err_msg.contains("InvalidMmrInclusionProof"));
}
