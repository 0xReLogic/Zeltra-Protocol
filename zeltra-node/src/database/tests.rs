use super::*;
use crate::dto::SpendRequest;
use tempfile::TempDir;

    fn sample_spend(nullifier: &str) -> SpendRequest {
        SpendRequest {
            nullifier: nullifier.to_string(),
            sig_hex: "0xsig".to_string(),
            recipient: "0x0000000000000000000000000000000000000001".to_string(),
            amount: 10_000_000,
            eip7702_auth: None,
            cross_chain: None,
            association_root_hex: None,
            recipient_or_intent_hash_hex: Some(format!("0x{}", "44".repeat(32))),
            expiry: Some(u64::MAX),
            nonce_hex: Some(format!("0x{}", "55".repeat(32))),
            min_payout: None,
            deadline: None,
            idempotency_key: Some(format!("idem-{}", nullifier)),
            max_execution_fee: None,
            execution_fee: None,
            quote_expiry: None,
            quote_id: None,
            quote_signature: None,
            user_address: None,
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn test_persistent_spend_queue_lifecycle() {
        std::env::set_var("ZELTRA_DB_KEY", "test-encryption-key");
        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("queue.db");
        let request = sample_spend("0xqueue-lifecycle");

        let db = Database::new(&db_path).await.unwrap();
        let queue_id = db.enqueue_spend(&request).await.unwrap().unwrap();
        assert_eq!(db.queued_spend_count().await.unwrap(), 1);
        assert!(db.enqueue_spend(&request).await.unwrap().is_none());
        drop(db);

        let reopened = Database::new(&db_path).await.unwrap();
        let claimed = reopened.claim_spends(10, 60).await.unwrap();
        assert_eq!(claimed.len(), 1);
        assert_eq!(claimed[0].id, queue_id);
        assert_eq!(claimed[0].request.nullifier, request.nullifier);
        assert!(reopened.claim_spends(10, 60).await.unwrap().is_empty());

        reopened
            .mark_spend_submitted(queue_id, "0xtx")
            .await
            .unwrap();
        reopened
            .mark_spend_confirmed(queue_id, &request.nullifier, "0xtx", 42)
            .await
            .unwrap();
        assert!(reopened
            .is_nullifier_spent(&request.nullifier)
            .await
            .unwrap());
        assert_eq!(reopened.queued_spend_count().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn test_retry_requeues_without_confirming_nullifier() {
        std::env::set_var("ZELTRA_DB_KEY", "test-encryption-key");
        let tmp = TempDir::new().unwrap();
        let db = Database::new(tmp.path().join("retry.db")).await.unwrap();
        let request = sample_spend("0xretry");
        let queue_id = db.enqueue_spend(&request).await.unwrap().unwrap();
        let item = db.claim_spends(1, 60).await.unwrap().pop().unwrap();

        db.retry_spend(item.id, "rpc unavailable", item.retry_count)
            .await
            .unwrap();
        assert!(!db.is_nullifier_spent(&request.nullifier).await.unwrap());

        // First retry uses a short backoff; force availability to avoid sleeping.
        let conn = open_connection(&db.path, &database_key().unwrap()).unwrap();
        conn.execute(
            "UPDATE spend_queue SET available_at = 0 WHERE id = ?",
            params![queue_id],
        )
        .unwrap();
        let retried = db.claim_spends(1, 60).await.unwrap();
        assert_eq!(retried.len(), 1);
        assert_eq!(retried[0].retry_count, 1);
    }

    #[tokio::test]
    async fn test_concurrent_workers_claim_only_once() {
        std::env::set_var("ZELTRA_DB_KEY", "test-encryption-key");
        let tmp = TempDir::new().unwrap();
        let db = Database::new(tmp.path().join("claim-race.db"))
            .await
            .unwrap();
        db.enqueue_spend(&sample_spend("0xclaim-race"))
            .await
            .unwrap()
            .unwrap();

        let worker_a = db.clone();
        let worker_b = db.clone();
        let (claimed_a, claimed_b) =
            tokio::join!(worker_a.claim_spends(1, 60), worker_b.claim_spends(1, 60),);
        let total_claimed = claimed_a.unwrap().len() + claimed_b.unwrap().len();
        assert_eq!(total_claimed, 1);
    }

    #[tokio::test]
    async fn test_nullifier_double_spend_prevention() {
        // Set encryption key for tests
        std::env::set_var("ZELTRA_DB_KEY", "test-encryption-key");
        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("test.db");
        let db = Database::new(&db_path).await.unwrap();

        let nullifier = "0xdeadbeef";

        // First spend should succeed
        let first = db
            .check_and_insert_nullifier(nullifier, None)
            .await
            .unwrap();
        assert!(first, "First spend should succeed");

        // Second spend should fail (double-spend detected!)
        let second = db
            .check_and_insert_nullifier(nullifier, None)
            .await
            .unwrap();
        assert!(!second, "Second spend should be rejected (double-spend!)");

        // Check should confirm it's spent
        let is_spent = db.is_nullifier_spent(nullifier).await.unwrap();
        assert!(is_spent, "Nullifier should be marked as spent");
    }

    #[tokio::test]
    async fn test_concurrent_nullifier_inserts() {
        // Set encryption key for tests
        std::env::set_var("ZELTRA_DB_KEY", "test-encryption-key");
        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("test.db");
        let db = Database::new(&db_path).await.unwrap();

        let nullifier = "0xrace_condition";

        // Simulate concurrent inserts (race condition test)
        let db1 = db.clone();
        let db2 = db.clone();
        let n1 = nullifier.to_string();
        let n2 = nullifier.to_string();

        let (r1, r2): (Result<bool>, Result<bool>) = tokio::join!(
            db1.check_and_insert_nullifier(&n1, None),
            db2.check_and_insert_nullifier(&n2, None),
        );

        // Only ONE should succeed (atomic INSERT OR IGNORE!)
        let successes = [r1.unwrap(), r2.unwrap()].iter().filter(|&&x| x).count();
        assert_eq!(successes, 1, "Only one concurrent insert should succeed");
    }

    #[tokio::test]
    async fn test_relayer_transactions_lifecycle() {
        std::env::set_var("ZELTRA_DB_KEY", "test-encryption-key");
        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("relayer_tx_test.db");
        let db = Database::new(&db_path).await.unwrap();

        let signer = "0x1111111111111111111111111111111111111111";
        let tx1 = "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        let tx2 = "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

        // Initial state: no transactions
        assert_eq!(db.get_max_relayer_nonce(signer).await.unwrap(), None);

        // Record first transaction
        db.record_relayer_tx(tx1, 5, "pending", signer)
            .await
            .unwrap();
        assert_eq!(db.get_max_relayer_nonce(signer).await.unwrap(), Some(5));

        // Update confirmations
        db.update_relayer_tx_confirmations(tx1, 2).await.unwrap();
        let rec = db.get_relayer_tx(tx1).await.unwrap().unwrap();
        assert_eq!(rec.confirmations, 2);
        assert_eq!(rec.status, "pending");

        // Mark confirmed
        db.update_relayer_tx_confirmed(tx1, 100, 21_000, 30_000_000, 4)
            .await
            .unwrap();
        let rec_confirmed = db.get_relayer_tx(tx1).await.unwrap().unwrap();
        assert_eq!(rec_confirmed.status, "confirmed");
        assert_eq!(rec_confirmed.block_number, Some(100));
        assert_eq!(rec_confirmed.gas_used, Some(21_000));
        assert_eq!(rec_confirmed.effective_gas_price, Some(30_000_000));
        assert_eq!(rec_confirmed.confirmations, 4);

        // Query via get_tx_status_any
        let any_status = db.get_tx_status_any(tx1).await.unwrap().unwrap();
        assert_eq!(any_status.status, "confirmed");
        assert_eq!(any_status.block_number, Some(100));

        // Record replacement or second tx with failure
        db.record_relayer_tx(tx2, 6, "pending", signer)
            .await
            .unwrap();
        assert_eq!(db.get_max_relayer_nonce(signer).await.unwrap(), Some(6));
        db.update_relayer_tx_failed(tx2, Some(101), Some(50_000), "reverted")
            .await
            .unwrap();
        let rec_failed = db.get_relayer_tx(tx2).await.unwrap().unwrap();
        assert_eq!(rec_failed.status, "failed");
        assert_eq!(rec_failed.error_reason, Some("reverted".to_string()));
    }

    #[tokio::test]
    async fn test_batch_profitability_metrics() {
        std::env::set_var("ZELTRA_DB_KEY", "test-encryption-key");
        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("batch_metrics_test.db");
        let db = Database::new(&db_path).await.unwrap();

        // 1. Initial empty state metrics
        let initial_metrics = db.get_batch_metrics().await.unwrap();
        assert_eq!(initial_metrics.batch_count, 0);
        assert_eq!(initial_metrics.total_batch_margin_usdc, 0.0);
        assert_eq!(initial_metrics.avg_batch_size, 0.0);
        assert_eq!(initial_metrics.total_execution_fees_usdc, 0);
        assert_eq!(initial_metrics.claimed_execution_fees_usdc, 0);
        assert_eq!(initial_metrics.unclaimed_execution_fees_usdc, 0);

        // 2. Store first batch (3 items, 5.5 USDC margin, 15,000,000 fee = 15 USDC)
        db.store_batch_metadata(
            "batch_001",
            3,
            "0xtx1",
            210_000,
            20_000_000_000,
            0.0042,
            5.5,
            15_000_000,
        )
        .await
        .unwrap();

        // 3. Store second batch (5 items, 10.5 USDC margin, 25,000,000 fee = 25 USDC)
        db.store_batch_metadata(
            "batch_002",
            5,
            "0xtx2",
            350_000,
            20_000_000_000,
            0.0070,
            10.5,
            25_000_000,
        )
        .await
        .unwrap();

        // 4. Verify aggregated metrics
        // batch_count: 2
        // total_batch_margin_usdc: 5.5 + 10.5 = 16.0
        // avg_batch_size: (3 + 5) / 2 = 4.0
        // total_execution_fees_usdc: 15_000_000 + 25_000_000 = 40_000_000
        // unclaimed: 40_000_000 (claimed: 0)
        let metrics = db.get_batch_metrics().await.unwrap();
        assert_eq!(metrics.batch_count, 2);
        assert!((metrics.total_batch_margin_usdc - 16.0).abs() < 1e-6);
        assert!((metrics.avg_batch_size - 4.0).abs() < 1e-6);
        assert_eq!(metrics.total_execution_fees_usdc, 40_000_000);
        assert_eq!(metrics.claimed_execution_fees_usdc, 0);
        assert_eq!(metrics.unclaimed_execution_fees_usdc, 40_000_000);

        // 5. Claim first batch fees
        db.mark_batches_claimed(&["batch_001".to_string()])
            .await
            .unwrap();

        // 6. Verify updated claimed & unclaimed metrics
        let updated_metrics = db.get_batch_metrics().await.unwrap();
        assert_eq!(updated_metrics.claimed_execution_fees_usdc, 15_000_000);
        assert_eq!(updated_metrics.unclaimed_execution_fees_usdc, 25_000_000);
        assert_eq!(updated_metrics.total_execution_fees_usdc, 40_000_000);
    }

    #[tokio::test]
    async fn test_mmr_persistence_lifecycle() {
        std::env::set_var("ZELTRA_DB_KEY", "test-encryption-key");
        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("mmr_lifecycle.db");
        let db = Database::new(&db_path).await.unwrap();

        // 1. Initially empty
        assert!(db.get_mmr_state().await.unwrap().is_none());
        assert!(db.get_mmr_last_indexed_block().await.unwrap().is_none());
        let tree = db.load_mmr_tree().await.unwrap();
        assert_eq!(tree.leaf_count, 0);

        // 2. Insert two leaves
        let leaf0_fr = zeltra_core::Fr::from(42u64);
        let leaf1_fr = zeltra_core::Fr::from(999u64);
        let leaf0_hex = format!("0x{}", hex::encode(zeltra_core::fr_to_be_bytes(&leaf0_fr)));
        let leaf1_hex = format!("0x{}", hex::encode(zeltra_core::fr_to_be_bytes(&leaf1_fr)));

        db.insert_mmr_leaf(0, &leaf0_hex, "0xtx0", 100, &[])
            .await
            .unwrap();
        db.insert_mmr_leaf(1, &leaf1_hex, "0xtx1", 101, &[])
            .await
            .unwrap();

        let l0 = db
            .get_mmr_leaf(0)
            .await
            .unwrap()
            .expect("leaf 0 must exist");
        assert_eq!(l0.leaf_index, 0);
        assert_eq!(l0.commitment, leaf0_hex);
        assert_eq!(l0.tx_hash, "0xtx0");
        assert_eq!(l0.block_number, 100);

        let all = db.get_all_mmr_leaves().await.unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(all[1].leaf_index, 1);

        // 3. Save MMR state snapshot with reference tree
        let mut ref_mmr = zeltra_core::MerkleMountainRange::new();
        ref_mmr.append(leaf0_fr);
        ref_mmr.append(leaf1_fr);
        db.save_mmr_state(&ref_mmr, 101).await.unwrap();

        let state = db.get_mmr_state().await.unwrap().expect("state must exist");
        assert_eq!(state.leaf_count, 2);
        assert_eq!(state.last_indexed_block, 101);
        let expected_root = format!(
            "0x{}",
            hex::encode(zeltra_core::fr_to_be_bytes(&ref_mmr.get_root()))
        );
        assert_eq!(
            state.bagged_root.to_lowercase(),
            expected_root.to_lowercase()
        );

        // 4. Re-load tree and verify reconstructed state equals reference MMR
        let loaded_tree = db.load_mmr_tree().await.unwrap();
        assert_eq!(loaded_tree.leaf_count, 2);
        assert_eq!(loaded_tree.get_root(), ref_mmr.get_root());
        assert_eq!(loaded_tree.get_peaks(), ref_mmr.get_peaks());
    }

    #[tokio::test]
    async fn test_mmr_persistence_duplicate_and_bounds_rejection() {
        std::env::set_var("ZELTRA_DB_KEY", "test-encryption-key");
        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("mmr_negative.db");
        let db = Database::new(&db_path).await.unwrap();

        let leaf0_fr = zeltra_core::Fr::from(100u64);
        let leaf0_hex = format!("0x{}", hex::encode(zeltra_core::fr_to_be_bytes(&leaf0_fr)));

        // 1. Insert leaf 0
        db.insert_mmr_leaf(0, &leaf0_hex, "0xtx0", 50, &[])
            .await
            .unwrap();

        // 2. Negative Test: Duplicate leaf_index 0 must fail (PRIMARY KEY UNIQUE constraint)
        let dup_res = db.insert_mmr_leaf(0, &leaf0_hex, "0xtx_dup", 51, &[]).await;
        assert!(
            dup_res.is_err(),
            "Duplicate leaf_index must be rejected by SQLite primary key"
        );

        // 3. Negative Test: Out-of-bounds non-contiguous leaf 2 (missing leaf 1) must fail closed upon load_mmr_tree
        let leaf2_fr = zeltra_core::Fr::from(200u64);
        let leaf2_hex = format!("0x{}", hex::encode(zeltra_core::fr_to_be_bytes(&leaf2_fr)));
        db.insert_mmr_leaf(2, &leaf2_hex, "0xtx2", 52, &[])
            .await
            .unwrap();

        let load_res = db.load_mmr_tree().await;
        assert!(
            load_res.is_err(),
            "Non-contiguous leaves in storage must fail closed on tree reconstruction"
        );
    }

    #[tokio::test]
    async fn test_mmr_persistence_reorg_rollback_safety() {
        std::env::set_var("ZELTRA_DB_KEY", "test-encryption-key");
        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("mmr_rollback.db");
        let db = Database::new(&db_path).await.unwrap();

        // Insert leaves across blocks 100, 101, 102
        let fr0 = zeltra_core::Fr::from(10u64);
        let fr1 = zeltra_core::Fr::from(20u64);
        let fr2 = zeltra_core::Fr::from(30u64);

        let hex0 = format!("0x{}", hex::encode(zeltra_core::fr_to_be_bytes(&fr0)));
        let hex1 = format!("0x{}", hex::encode(zeltra_core::fr_to_be_bytes(&fr1)));
        let hex2 = format!("0x{}", hex::encode(zeltra_core::fr_to_be_bytes(&fr2)));

        db.insert_mmr_leaf(0, &hex0, "0xtx0", 100, &[])
            .await
            .unwrap();
        db.insert_mmr_leaf(1, &hex1, "0xtx1", 101, &[])
            .await
            .unwrap();
        db.insert_mmr_leaf(2, &hex2, "0xtx2", 102, &[])
            .await
            .unwrap();

        let mut tree_3 = zeltra_core::MerkleMountainRange::new();
        tree_3.append(fr0);
        tree_3.append(fr1);
        tree_3.append(fr2);
        db.save_mmr_state(&tree_3, 102).await.unwrap();

        assert_eq!(db.get_all_mmr_leaves().await.unwrap().len(), 3);
        assert_eq!(db.get_mmr_state().await.unwrap().unwrap().leaf_count, 3);

        // Simulate Reorg: Rollback to block 100 (unwinds blocks 101 and 102)
        db.rollback_mmr_to_block(100).await.unwrap();

        // Verify remaining state
        let remaining = db.get_all_mmr_leaves().await.unwrap();
        assert_eq!(
            remaining.len(),
            1,
            "Only block 100 leaves must remain after rollback"
        );
        assert_eq!(remaining[0].leaf_index, 0);
        assert_eq!(remaining[0].block_number, 100);

        let state_after = db.get_mmr_state().await.unwrap().expect("state exists");
        assert_eq!(state_after.leaf_count, 1);
        assert_eq!(state_after.last_indexed_block, 100);

        let mut tree_1 = zeltra_core::MerkleMountainRange::new();
        tree_1.append(fr0);
        let expected_root_1 = format!(
            "0x{}",
            hex::encode(zeltra_core::fr_to_be_bytes(&tree_1.get_root()))
        );
        assert_eq!(
            state_after.bagged_root.to_lowercase(),
            expected_root_1.to_lowercase()
        );

        // Reconstructed tree must match 1-leaf tree
        let loaded = db.load_mmr_tree().await.unwrap();
        assert_eq!(loaded.leaf_count, 1);
        assert_eq!(loaded.get_root(), tree_1.get_root());
    }

    #[tokio::test]
    async fn test_mmr_atomic_append_and_state() {
        std::env::set_var("ZELTRA_DB_KEY", "test-encryption-key");
        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("mmr_atomic.db");
        let db = Database::new(&db_path).await.unwrap();

        let fr0 = zeltra_core::Fr::from(111u64);
        let fr1 = zeltra_core::Fr::from(222u64);
        let hex0 = format!("0x{}", hex::encode(zeltra_core::fr_to_be_bytes(&fr0)));
        let hex1 = format!("0x{}", hex::encode(zeltra_core::fr_to_be_bytes(&fr1)));

        let leaves = vec![
            MmrLeafRecord {
                leaf_index: 0,
                commitment: hex0,
                tx_hash: "0xtx0".to_string(),
                block_number: 500,
                encrypted_memo: vec![0xaa; 96],
                created_at: None,
            },
            MmrLeafRecord {
                leaf_index: 1,
                commitment: hex1,
                tx_hash: "0xtx1".to_string(),
                block_number: 500,
                encrypted_memo: vec![0xbb; 96],
                created_at: None,
            },
        ];

        let mut mmr = zeltra_core::MerkleMountainRange::new();
        mmr.append(fr0);
        mmr.append(fr1);

        db.atomic_append_mmr_leaves_and_state(&leaves, &mmr, 500)
            .await
            .unwrap();

        let state = db.get_mmr_state().await.unwrap().unwrap();
        assert_eq!(state.leaf_count, 2);
        assert_eq!(state.last_indexed_block, 500);
        assert_eq!(db.get_all_mmr_leaves().await.unwrap().len(), 2);
    }

    #[tokio::test]
    async fn test_get_notes_memo_chunk_pagination() {
        std::env::set_var("ZELTRA_DB_KEY", "test-encryption-key");
        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("mmr_chunk.db");
        let db = Database::new(&db_path).await.unwrap();

        for i in 0..10u64 {
            let cm = format!("0x{:064x}", i + 1);
            let memo = vec![(i as u8) + 1; 96];
            db.insert_mmr_leaf(i, &cm, &format!("0xtx{}", i), 1000 + i, &memo)
                .await
                .unwrap();
        }

        // Test chunk from 0 limit 4
        let chunk1 = db.get_notes_memo_chunk(0, 4).await.unwrap();
        assert_eq!(chunk1.len(), 4);
        assert_eq!(chunk1[0].leaf_index, 0);
        assert_eq!(chunk1[3].leaf_index, 3);
        assert!(chunk1[0].encrypted_memo.starts_with("0x0101"));

        // Test chunk from 4 limit 4
        let chunk2 = db.get_notes_memo_chunk(4, 4).await.unwrap();
        assert_eq!(chunk2.len(), 4);
        assert_eq!(chunk2[0].leaf_index, 4);
        assert_eq!(chunk2[3].leaf_index, 7);

        // Test chunk from 8 limit 10 (returns remaining 2)
        let chunk3 = db.get_notes_memo_chunk(8, 10).await.unwrap();
        assert_eq!(chunk3.len(), 2);
        assert_eq!(chunk3[0].leaf_index, 8);
        assert_eq!(chunk3[1].leaf_index, 9);

        // Test out of bounds chunk
        let chunk_empty = db.get_notes_memo_chunk(50, 10).await.unwrap();
        assert!(chunk_empty.is_empty());
    }
