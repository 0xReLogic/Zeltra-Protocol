//! Integration test demonstrating end-to-end database flow

use tempfile::TempDir;

#[tokio::test]
async fn test_end_to_end_deposit_and_spend_flow() {
    std::env::set_var("NIMBUS_DB_KEY", "test-encryption-key");
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
    std::env::set_var("NIMBUS_DB_KEY", "test-encryption-key");
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
        leaf_count: Some(1),
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
        note_epoch_id: Some(1),
        is_rollover: Some(0),
        input_value: Some(10_000_000),
        proof_a_neg: format!("0x{}", "11".repeat(128)),
        proof_b: format!("0x{}", "22".repeat(256)),
        proof_c: format!("0x{}", "33".repeat(128)),
        public_inputs: vec![
            note_root.to_string(),
            format!("0x{:064x}", 1u64),
            input_nullifier.to_string(),
            format!("0x{:064x}", 1u64),
            output_commitment.to_string(),
            format!("0x000000000000000000000000{}", &recipient[2..]),
            format!("0x{:064x}", 5_000_000u64),
            format!("0x{:064x}", 22_500u64),
            format!("0x{:064x}", 20_000u64),
            format!("0x{:064x}", 0u64),
            format!("0x{:064x}", 4u64),
            format!("0x{:064x}", 421614u64),
            format!(
                "0x000000000000000000000000{}",
                "3333333333333333333333333333333333333333"
            ),
            format!("0x{:064x}", 9999999999u64),
            format!("0x{:064x}", 1u64),
            format!("0x{:064x}", 0u64),
        ],
        idempotency_key: Some("idem_private_1".to_string()),
    };

    let spend_req = nimbus_node::dto::SpendRequest::from(priv_req.clone());
    assert_eq!(spend_req.note_epoch_id, Some(1));
    assert_eq!(spend_req.is_rollover, Some(0));
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

#[tokio::test]
async fn test_end_to_end_multi_wallet_spend_with_mmr_sync() {
    std::env::set_var("NIMBUS_DB_KEY", "test-encryption-key");
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("e2e_multi_wallet.db");
    let db = nimbus_node::database::Database::new(&db_path)
        .await
        .unwrap();

    // 1. Initialize Wallet A and Wallet B
    let seed_a = [11u8; 32];
    let mut wallet_a = nimbus_sdk::wallet::note_wallet::PrivateNoteWallet::new(&seed_a);
    let sk_a = wallet_a.spending_key().unwrap();

    let seed_b = [22u8; 32];
    let mut wallet_b = nimbus_sdk::wallet::note_wallet::PrivateNoteWallet::new(&seed_b);
    let sk_b = wallet_b.spending_key().unwrap();

    // 2. Wallet A deposits Note 1 (10 USDC) -> leaf_index 0
    let (note_a, cm_a_hex) = wallet_a.create_deposit_note(10_000_000, 1000).unwrap();
    let rho_a = nimbus_sdk::wallet::note_wallet::parse_fr_from_hex(&note_a.rho_hex).unwrap();
    let rand_a =
        nimbus_sdk::wallet::note_wallet::parse_fr_from_hex(&note_a.randomness_hex).unwrap();
    let cm_a = nimbus_core::note_commitment(10_000_000, sk_a, rho_a, rand_a);

    // Simulate on-chain MMR: append Note 1
    let mut on_chain_mmr = nimbus_core::MerkleMountainRange::new();
    on_chain_mmr.append_leaf(cm_a);
    let root_1 = on_chain_mmr.bagged_root();

    // Indexer indexes Note 1
    let leaf_0_record = nimbus_node::database::MmrLeafRecord {
        leaf_index: 0,
        commitment: cm_a_hex.clone(),
        tx_hash: "0xtx_deposit_a".to_string(),
        block_number: 100,
        created_at: None,
    };
    db.atomic_append_mmr_leaves_and_state(&[leaf_0_record], &on_chain_mmr, 100)
        .await
        .unwrap();

    // 3. Wallet B deposits Note 2 (25 USDC) -> leaf_index 1
    let (note_b, cm_b_hex) = wallet_b.create_deposit_note(25_000_000, 1000).unwrap();
    let rho_b = nimbus_sdk::wallet::note_wallet::parse_fr_from_hex(&note_b.rho_hex).unwrap();
    let rand_b =
        nimbus_sdk::wallet::note_wallet::parse_fr_from_hex(&note_b.randomness_hex).unwrap();
    let cm_b = nimbus_core::note_commitment(25_000_000, sk_b, rho_b, rand_b);

    // Simulate on-chain MMR: append Note 2
    on_chain_mmr.append_leaf(cm_b);
    let root_2 = on_chain_mmr.bagged_root();
    assert_ne!(
        root_1, root_2,
        "MMR root must change when new leaf is added"
    );

    // Indexer indexes Note 2
    let leaf_1_record = nimbus_node::database::MmrLeafRecord {
        leaf_index: 1,
        commitment: cm_b_hex.clone(),
        tx_hash: "0xtx_deposit_b".to_string(),
        block_number: 101,
        created_at: None,
    };
    db.atomic_append_mmr_leaves_and_state(&[leaf_1_record], &on_chain_mmr, 101)
        .await
        .unwrap();

    // 4. Wallet A queries Relayer REST Sync API for leaf 0 inclusion proof
    let relayer_mmr = db.load_mmr_tree().await.unwrap();
    assert_eq!(relayer_mmr.leaf_count(), 2);
    let mmr_proof_0 = relayer_mmr.generate_proof(0);

    let m_sibs_hex: Vec<String> = mmr_proof_0
        .mountain_siblings
        .iter()
        .map(|s| format!("0x{}", hex::encode(nimbus_core::fr_to_be_bytes(s))))
        .collect();
    let p_sibs_hex: Vec<String> = mmr_proof_0
        .peak_bagging_siblings
        .iter()
        .map(|s| format!("0x{}", hex::encode(nimbus_core::fr_to_be_bytes(s))))
        .collect();
    let root_2_hex = format!("0x{}", hex::encode(nimbus_core::fr_to_be_bytes(&root_2)));

    let proof_response = nimbus_sdk::wallet::note_wallet::MmrProofResponse {
        leaf_index: 0,
        leaf_count: 2,
        commitment: cm_a_hex.clone(),
        mountain_height: mmr_proof_0.mountain_height,
        mountain_siblings: m_sibs_hex,
        peak_bagging_siblings: p_sibs_hex,
        bagged_root: root_2_hex.clone(),
        block_number: 101,
    };

    // Confirm deposit in Wallet A
    wallet_a
        .confirm_deposit(&cm_a_hex, 0, vec!["0x00".to_string(); 20], &root_2_hex)
        .unwrap();
    assert_eq!(wallet_a.balance(), 10_000_000);

    // Wallet A caches the MMR proof
    wallet_a.cache_mmr_proof(proof_response.clone());

    // 5. Wallet A generates valid Groth16 spend proof spending 6 USDC with 4 USDC change
    let keys = nimbus_core::get_or_init_note_circuit_keys();
    let selected = wallet_a
        .select_note_for_spend(6_000_000, 27_000, 50_000, 1010)
        .unwrap();

    let spend_payload = wallet_a
        .prepare_spend_proof_with_mmr(
            &selected,
            "session_e2e_spend_0",
            "0x1111111111111111111111111111111111111111",
            100_000,
            "0x2222222222222222222222222222222222222222222222222222222222222222",
            421614,
            "0x3333333333333333333333333333333333333333",
            2000,
            None,
            false,
            &proof_response,
            1010,
            &keys.proving_key,
        )
        .unwrap();

    // 6. Verify proof with EVM Verifier (EIP-2537)
    let proof_a_bytes: [u8; 128] = hex::decode(&spend_payload.proof_a_neg_hex)
        .unwrap()
        .try_into()
        .unwrap();
    let proof_b_bytes: [u8; 256] = hex::decode(&spend_payload.proof_b_hex)
        .unwrap()
        .try_into()
        .unwrap();
    let proof_c_bytes: [u8; 128] = hex::decode(&spend_payload.proof_c_hex)
        .unwrap()
        .try_into()
        .unwrap();

    let mut pis = [nimbus_core::Fr::from(0u64); 16];
    for (i, h) in spend_payload.public_inputs_hex.iter().enumerate() {
        pis[i] = nimbus_sdk::wallet::note_wallet::parse_fr_from_hex(h).unwrap();
    }

    assert_eq!(
        pis[0], root_2,
        "Public input note_root must equal the 2-leaf bagged MMR root"
    );
    assert_eq!(
        pis[1],
        nimbus_core::Fr::from(2u64),
        "Public input leaf_count must equal 2"
    );

    let is_valid =
        nimbus_core::verify_evm_note_proof(&proof_a_bytes, &proof_b_bytes, &proof_c_bytes, &pis);
    assert!(
        is_valid,
        "Groth16 spend proof must verify against EVM precompile verifier"
    );

    // 7. Relayer enqueues spend and enforces nullifier integrity
    let priv_req = nimbus_node::dto::PrivateNoteSpendRequest {
        session_id: Some(spend_payload.session_id.clone()),
        note_root: format!("0x{}", spend_payload.note_root_hex),
        leaf_count: Some(spend_payload.leaf_count),
        input_nullifier: format!("0x{}", spend_payload.input_nullifier_hex),
        output_commitment: Some(format!("0x{}", spend_payload.output_commitment_hex)),
        recipient: spend_payload.recipient_hex.clone(),
        merchant_amount: spend_payload.merchant_amount,
        protocol_fee: spend_payload.protocol_fee,
        execution_fee: spend_payload.execution_fee,
        max_execution_fee: spend_payload.max_execution_fee,
        quote_hash: Some(spend_payload.quote_hash_hex.clone()),
        quote_signature: None,
        quote_id: None,
        user_address: None,
        expiry: Some(spend_payload.expiry),
        has_change: Some(serde_json::Value::Number(1.into())),
        note_epoch_id: Some(spend_payload.note_epoch_id),
        is_rollover: Some(0),
        input_value: Some(10_000_000),
        proof_a_neg: format!("0x{}", spend_payload.proof_a_neg_hex),
        proof_b: format!("0x{}", spend_payload.proof_b_hex),
        proof_c: format!("0x{}", spend_payload.proof_c_hex),
        public_inputs: spend_payload
            .public_inputs_hex
            .iter()
            .map(|p| format!("0x{p}"))
            .collect(),
        idempotency_key: Some("idem_e2e_multi_spend".to_string()),
    };

    let spend_req = nimbus_node::dto::SpendRequest::from(priv_req);
    let queue_id = db
        .enqueue_spend(&spend_req)
        .await
        .unwrap()
        .expect("Enqueue must succeed");

    // Enforcing double-spend guard: second spend attempt must fail
    let dup_enqueue = db.enqueue_spend(&spend_req).await.unwrap();
    assert!(
        dup_enqueue.is_none(),
        "Double-spend enqueue must be rejected"
    );

    // Claim spend from worker queue (DEC-017 lease model)
    let claimed = db.claim_spends(10, 60).await.unwrap();
    assert_eq!(claimed.len(), 1);
    assert_eq!(claimed[0].id, queue_id);

    db.mark_spend_submitted(queue_id, "0xtx_spend_confirmed")
        .await
        .unwrap();

    db.mark_spend_confirmed(
        queue_id,
        &spend_payload.input_nullifier_hex,
        "0xtx_spend_confirmed",
        102,
    )
    .await
    .unwrap();

    assert!(db
        .is_nullifier_spent(&spend_payload.input_nullifier_hex)
        .await
        .unwrap());
    println!("✅ End-to-end multi-wallet spend with MMR sync test passed successfully!");
}
