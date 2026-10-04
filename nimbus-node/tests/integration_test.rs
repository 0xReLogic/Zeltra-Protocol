//! Integration test demonstrating end-to-end database flow

use tempfile::TempDir;

#[tokio::test]
async fn test_end_to_end_deposit_and_spend_flow() {
    // Setup temporary database
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("integration_test.db");
    let db = nimbus_node::database::Database::new(&db_path)
        .await
        .unwrap();

    // Test 1: Deposit session
    let session_id = "test_session_001";
    let com_k_hex = "0xabcdef1234567890";
    let amount = 1000u64;
    let client = "0xclient123";

    let inserted = db
        .insert_session(session_id, com_k_hex, amount, client)
        .await
        .unwrap();
    assert!(inserted, "First session insert should succeed");

    // Test duplicate session (should be rejected)
    let duplicate = db
        .insert_session(session_id, com_k_hex, amount, client)
        .await
        .unwrap();
    assert!(!duplicate, "Duplicate session insert should be rejected");

    // Test 2: Reveal masking key
    let masking_key = "0xmasking_key_xyz";
    let resolved = db.resolve_session(session_id, masking_key).await.unwrap();
    assert!(resolved, "First resolve should succeed");

    // Test double resolve (should be rejected)
    let resolved_again = db.resolve_session(session_id, masking_key).await.unwrap();
    assert!(!resolved_again, "Double resolve should be rejected");

    // Test 3: Spend with nullifier check
    let nullifier = "0xnullifier_unique_001";

    // First spend should succeed
    let first_spend = db
        .check_and_insert_nullifier(nullifier, Some("0xtxhash"))
        .await
        .unwrap();
    assert!(first_spend, "First spend should succeed");

    // Check nullifier is marked as spent
    let is_spent = db.is_nullifier_spent(nullifier).await.unwrap();
    assert!(is_spent, "Nullifier should be marked as spent");

    // Second spend should fail (double-spend prevention)
    let double_spend = db
        .check_and_insert_nullifier(nullifier, Some("0xtxhash2"))
        .await
        .unwrap();
    assert!(!double_spend, "Double-spend should be rejected");

    // Test 4: Database stats
    let stats = db.get_stats().await.unwrap();
    assert_eq!(stats.total_sessions, 1, "Should have 1 session");
    assert_eq!(stats.resolved_sessions, 1, "Should have 1 resolved session");
    assert_eq!(stats.total_nullifiers, 1, "Should have 1 nullifier");

    println!("✅ End-to-end integration test passed!");
    println!("   - Sessions: {}", stats.total_sessions);
    println!("   - Resolved: {}", stats.resolved_sessions);
    println!("   - Nullifiers: {}", stats.total_nullifiers);
}

#[tokio::test]
async fn test_concurrent_double_spend_attempts() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("concurrent_test.db");
    let db = nimbus_node::database::Database::new(&db_path)
        .await
        .unwrap();

    let nullifier = "0xrace_condition_nullifier";

    // Spawn 10 concurrent spend attempts with same nullifier
    let mut handles = vec![];
    for i in 0..10 {
        let db_clone = db.clone();
        let null_clone = nullifier.to_string();
        let tx_hash = format!("0xtx_{}", i);

        let handle = tokio::spawn(async move {
            db_clone
                .check_and_insert_nullifier(&null_clone, Some(&tx_hash))
                .await
        });
        handles.push(handle);
    }

    // Collect results
    let mut successes = 0;
    let mut failures = 0;
    for handle in handles {
        match handle.await.unwrap() {
            Ok(true) => successes += 1,
            Ok(false) => failures += 1,
            Err(_) => panic!("Database error"),
        }
    }

    // Only ONE should succeed, others should fail
    assert_eq!(successes, 1, "Exactly one concurrent spend should succeed");
    assert_eq!(failures, 9, "Nine attempts should be rejected");

    println!(" Concurrent double-spend prevention test passed!");
    println!("   - Successes: {} (expected 1)", successes);
    println!("   - Rejected: {} (expected 9)", failures);
}

#[tokio::test]
async fn test_startup_nullifier_reconciliation() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("reconciliation_test.db");
    let db = nimbus_node::database::Database::new(&db_path)
        .await
        .unwrap();

    let req1 = nimbus_node::dto::SpendRequest {
        nullifier: "0xreconcile_nullifier_already_spent".to_string(),
        sig_hex: "0xsig1".to_string(),
        recipient: "0x1111111111111111111111111111111111111111".to_string(),
        amount: 10_000_000,
        eip7702_auth: None,
        cross_chain: None,
        association_root_hex: None,
        alpha_neg_hex: "0xalpha1".to_string(),
        hm_hex: "0xhm1".to_string(),
        pk_iss_hex: "0xpk1".to_string(),
        recipient_or_intent_hash_hex: None,
        expiry: None,
        nonce_hex: None,
        min_payout: None,
        deadline: None,
        idempotency_key: None,
        max_execution_fee: None,
        execution_fee: None,
        quote_id: None,
        quote_expiry: None,
        quote_signature: None,
        user_address: None,
        ..Default::default()
    };

    let req2 = nimbus_node::dto::SpendRequest {
        nullifier: "0xreconcile_nullifier_not_yet_spent".to_string(),
        sig_hex: "0xsig2".to_string(),
        recipient: "0x2222222222222222222222222222222222222222".to_string(),
        amount: 15_000_000,
        eip7702_auth: None,
        cross_chain: None,
        association_root_hex: None,
        alpha_neg_hex: "0xalpha2".to_string(),
        hm_hex: "0xhm2".to_string(),
        pk_iss_hex: "0xpk2".to_string(),
        recipient_or_intent_hash_hex: None,
        expiry: None,
        nonce_hex: None,
        min_payout: None,
        deadline: None,
        idempotency_key: None,
        max_execution_fee: None,
        execution_fee: None,
        quote_id: None,
        quote_expiry: None,
        quote_signature: None,
        user_address: None,
        ..Default::default()
    };

    // Enqueue both spends
    let id1 = db
        .enqueue_spend(&req1)
        .await
        .unwrap()
        .expect("id1 should be inserted");
    let id2 = db
        .enqueue_spend(&req2)
        .await
        .unwrap()
        .expect("id2 should be inserted");

    // Verify both are returned by get_unreconciled_spends
    let pending = db.get_unreconciled_spends().await.unwrap();
    assert_eq!(pending.len(), 2);

    // Simulate scenario:
    // spend 1 is detected as ALREADY SPENT on-chain -> reconcile as confirmed
    db.reconcile_spend_as_confirmed(id1, &req1.nullifier, Some("0xactual_tx_hash"))
        .await
        .unwrap();

    // Verify nullifier 1 is now marked in database nullifiers table
    assert!(db.is_nullifier_spent(&req1.nullifier).await.unwrap());

    // Simulate scenario:
    // spend 2 was broadcasting when node died -> lease reset back to queued
    db.reset_unspent_lease(id2).await.unwrap();

    // Now unreconciled spends should only contain spend 2 (spend 1 is confirmed)
    let remaining = db.get_unreconciled_spends().await.unwrap();
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].id, id2);
    assert_eq!(remaining[0].nullifier, req2.nullifier);
    assert_eq!(remaining[0].status, "queued");

    println!(" Post-restart reconciliation test passed!");
}

#[tokio::test]
async fn test_private_note_spend_enqueue_and_queue_lifecycle() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("private_note_test.db");
    let db = nimbus_node::database::Database::new(&db_path)
        .await
        .unwrap();

    let note_root = "0x0000000000000000000000000000000000000000000000000000000000000001";
    let input_nullifier = "0x0000000000000000000000000000000000000000000000000000000000000002";
    let output_commitment = "0x0000000000000000000000000000000000000000000000000000000000000003";
    let recipient = "0x1111111111111111111111111111111111111111";

    let priv_req = nimbus_node::dto::PrivateNoteSpendRequest {
        session_id: Some("session_private_1".to_string()),
        note_root: note_root.to_string(),
        input_nullifier: input_nullifier.to_string(),
        output_commitment: Some(output_commitment.to_string()),
        recipient: recipient.to_string(),
        merchant_amount: 5_000_000,
        protocol_fee: 22_500,
        execution_fee: 20_000,
        max_execution_fee: 25_000,
        quote_hash: Some(
            "0x0000000000000000000000000000000000000000000000000000000000000004".to_string(),
        ),
        quote_signature: None,
        quote_id: None,
        user_address: None,
        expiry: Some(9999999999),
        has_change: Some(serde_json::Value::Number(1.into())),
        proof_a_neg: format!("0x{}", "11".repeat(128)),
        proof_b: format!("0x{}", "22".repeat(256)),
        proof_c: format!("0x{}", "33".repeat(128)),
        public_inputs: vec![
            note_root.to_string(),
            input_nullifier.to_string(),
            output_commitment.to_string(),
            format!("0x000000000000000000000000{}", &recipient[2..]),
            format!("0x{:064x}", 5_000_000u64),
            format!("0x{:064x}", 22_500u64),
            format!("0x{:064x}", 20_000u64),
            "0x0000000000000000000000000000000000000000000000000000000000000004".to_string(),
            format!("0x{:064x}", 421614u64),
            format!(
                "0x000000000000000000000000{}",
                "3333333333333333333333333333333333333333"
            ),
            format!("0x{:064x}", 9999999999u64),
            format!("0x{:064x}", 1u64),
        ],
        idempotency_key: Some("idem_private_1".to_string()),
    };

    let spend_req = nimbus_node::dto::SpendRequest::from(priv_req.clone());
    assert!(spend_req.is_private_note());
    assert_eq!(spend_req.nullifier, input_nullifier);
    assert_eq!(spend_req.amount, 5_000_000);
    assert_eq!(spend_req.note_root_hex.as_deref(), Some(note_root));

    // Enqueue into SQLite
    let id = db
        .enqueue_spend(&spend_req)
        .await
        .unwrap()
        .expect("Should be enqueued successfully");

    // Duplicate enqueue with same nullifier should be rejected
    let dup = db.enqueue_spend(&spend_req).await.unwrap();
    assert!(dup.is_none(), "Duplicate nullifier should be rejected");

    // Claim spend from queue
    let claimed = db.claim_spends(10, 60).await.unwrap();
    assert_eq!(claimed.len(), 1);
    assert_eq!(claimed[0].id, id);
    assert!(claimed[0].request.is_private_note());
    assert_eq!(claimed[0].request.note_root_hex.as_deref(), Some(note_root));

    // Mark submitted and confirmed
    db.mark_spend_submitted(id, "0xtx_private_spend")
        .await
        .unwrap();
    db.mark_spend_confirmed(id, input_nullifier, "0xtx_private_spend", 12345)
        .await
        .unwrap();

    // Check nullifier is marked as spent in local database
    assert!(db.is_nullifier_spent(input_nullifier).await.unwrap());

    println!(" Gate F Private Note Spend lifecycle test passed!");
}
