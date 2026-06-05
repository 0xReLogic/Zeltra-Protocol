//! Integration test demonstrating end-to-end database flow

use tempfile::TempDir;

#[tokio::test]
async fn test_end_to_end_deposit_and_spend_flow() {
    // Setup temporary database
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("integration_test.db");
    let db = nimbus_node::database::Database::new(&db_path).await.unwrap();
    
    // Test 1: Deposit session
    let session_id = "test_session_001";
    let com_k_hex = "0xabcdef1234567890";
    let amount = 1000u64;
    let client = "0xclient123";
    
    let inserted = db.insert_session(session_id, com_k_hex, amount, client).await.unwrap();
    assert!(inserted, "First session insert should succeed");
    
    // Test duplicate session (should be rejected)
    let duplicate = db.insert_session(session_id, com_k_hex, amount, client).await.unwrap();
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
    let first_spend = db.check_and_insert_nullifier(nullifier, Some("0xtxhash")).await.unwrap();
    assert!(first_spend, "First spend should succeed");
    
    // Check nullifier is marked as spent
    let is_spent = db.is_nullifier_spent(nullifier).await.unwrap();
    assert!(is_spent, "Nullifier should be marked as spent");
    
    // Second spend should fail (double-spend prevention)
    let double_spend = db.check_and_insert_nullifier(nullifier, Some("0xtxhash2")).await.unwrap();
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
    let db = nimbus_node::database::Database::new(&db_path).await.unwrap();
    
    let nullifier = "0xrace_condition_nullifier";
    
    // Spawn 10 concurrent spend attempts with same nullifier
    let mut handles = vec![];
    for i in 0..10 {
        let db_clone = db.clone();
        let null_clone = nullifier.to_string();
        let tx_hash = format!("0xtx_{}", i);
        
        let handle = tokio::spawn(async move {
            db_clone.check_and_insert_nullifier(&null_clone, Some(&tx_hash)).await
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
    
    println!("✅ Concurrent double-spend prevention test passed!");
    println!("   - Successes: {} (expected 1)", successes);
    println!("   - Rejected: {} (expected 9)", failures);
}
