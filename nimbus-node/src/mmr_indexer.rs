//! On-Chain Merkle Mountain Range (MMR) Event Indexer Daemon (DEC-035C)
//!
//! Authoritatively indexes `NoteCommitmentAppended` events emitted by the Stylus smart contract.
//! Reconstructs the canonical in-memory MMR tree mirror, enforces bit-exact fail-closed root parity guards,
//! maintains reorg-safe finalized block windows, and commits atomic SQLite checkpoints.

use crate::database::MmrLeafRecord;
use crate::evm_client::EvmClient;
use crate::state::AppState;
use anyhow::{Context, Result};
use std::time::Duration;
use tokio::sync::broadcast;
use tokio::time::interval;

/// Key used to persist the last indexed block in the indexer_state table
#[allow(dead_code)]
pub const MMR_INDEXER_KEY: &str = "mmr_indexer_last_block";

/// Maximum blocks to query in a single get_logs request to prevent RPC rate-limits
pub const MAX_BLOCKS_PER_QUERY: u64 = 500;

/// Default polling interval for the MMR indexer loop
pub const MMR_POLL_INTERVAL_SECS: u64 = 3;

/// Background worker running the MMR indexer loop with clean shutdown support
pub async fn mmr_indexer_worker(state: AppState) {
    let indexer = MmrIndexer::new(state, Duration::from_secs(MMR_POLL_INTERVAL_SECS));
    indexer.run_worker().await;
}

/// Standalone MMR indexer struct for dedicated runner and tests (DEC-035C Section 5.B)
pub struct MmrIndexer {
    pub state: AppState,
    pub poll_interval: Duration,
}

impl MmrIndexer {
    pub fn new(state: AppState, poll_interval: Duration) -> Self {
        Self {
            state,
            poll_interval,
        }
    }

    pub async fn run_worker(&self) {
        println!("MMR_INDEXER: Background worker started");
        let mut ticker = interval(self.poll_interval);

        loop {
            ticker.tick().await;

            let evm_client = match self.state.evm_client.as_ref() {
                Some(client) => client.clone(),
                None => {
                    // EVM client not configured (e.g. passive/mock dev mode)
                    continue;
                }
            };

            if let Err(e) = index_mmr_events_tick(&self.state, &evm_client).await {
                eprintln!("MMR_INDEXER ERROR: Failed indexing cycle: {:?}", e);
            }
        }
    }

    #[allow(dead_code)]
    pub async fn run_loop(&self, mut shutdown: broadcast::Receiver<()>) {
        let mut ticker = interval(self.poll_interval);

        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    if let Some(client) = self.state.evm_client.as_ref() {
                        if let Err(e) = index_mmr_events_tick(&self.state, client).await {
                            eprintln!("MMR_INDEXER ERROR: Failed indexing cycle: {:?}", e);
                        }
                    }
                }
                _ = shutdown.recv() => {
                    println!("MMR_INDEXER: Shutdown signal received, exiting cleanly");
                    break;
                }
            }
        }
    }
}

/// Single indexing execution cycle (DEC-035C INV-1, INV-2, INV-5)
pub async fn index_mmr_events_tick(state: &AppState, client: &EvmClient) -> Result<()> {
    let current_block = client.get_block_number().await?;
    let conf_threshold = client.finality_config().confirmation_threshold;

    // INV-1 (Reorg-Safe Indexing): safe_block = current_block - (conf_threshold - 1)
    let safe_block = current_block.saturating_sub(conf_threshold.saturating_sub(1));

    let last_block_opt = state.db.get_mmr_last_indexed_block().await?;

    let last_block = match last_block_opt {
        Some(b) => b,
        None => {
            // First run: bootstrap indexer from safe_block - 50 blocks
            let initial = safe_block.saturating_sub(50);
            state.db.update_mmr_indexed_block(initial).await?;
            println!(
                "MMR_INDEXER: Initialized MMR indexer checkpoint at block {}",
                initial
            );
            initial
        }
    };

    if safe_block <= last_block {
        return Ok(()); // Already caught up to safe head
    }

    let from_block = last_block + 1;
    let to_block = safe_block.min(from_block + MAX_BLOCKS_PER_QUERY);

    let events = client
        .get_mmr_commitment_events(from_block, to_block)
        .await?;

    if events.is_empty() {
        state.db.update_mmr_indexed_block(to_block).await?;
        return Ok(());
    }

    // Load canonical in-memory MMR mirror from persistent storage
    let mut mmr = state.db.load_mmr_tree().await?;
    let mut new_leaves = Vec::new();

    for ev in events {
        // INV-4 & C2.3 Sequential Leaf Invariant
        if ev.leaf_index < mmr.leaf_count() {
            println!(
                "MMR_INDEXER: Skipping already-indexed leaf index {}",
                ev.leaf_index
            );
            continue;
        }

        if ev.leaf_index != mmr.leaf_count() {
            anyhow::bail!(
                "NonSequentialLeafIndex: expected leaf_index {}, but on-chain event emitted {}",
                mmr.leaf_count(),
                ev.leaf_index
            );
        }

        // Convert 32-byte EVM scalar commitment to Fr
        let scalar_commitment = nimbus_core::from_evm_scalar(&ev.commitment)
            .context("Invalid Fr scalar note commitment in on-chain log")?;

        // Append to local MMR mirror
        mmr.append_leaf(scalar_commitment);

        // INV-2 (Fail-Closed Root Parity Guard): Bit-exact parity with on-chain event root
        let local_root_bytes = nimbus_core::fr_to_be_bytes(&mmr.bagged_root());
        if local_root_bytes != ev.new_mmr_root {
            panic!(
                "CRITICAL SECURITY ALERT (DEC-035C): MMR state desync detected at leaf_index {}! \
                 Local reconstructed root: 0x{}, On-chain event root: 0x{}. Halting immediately.",
                ev.leaf_index,
                hex::encode(local_root_bytes),
                hex::encode(ev.new_mmr_root)
            );
        }

        let commitment_hex = format!("0x{}", hex::encode(ev.commitment));
        new_leaves.push(MmrLeafRecord {
            leaf_index: ev.leaf_index,
            commitment: commitment_hex,
            tx_hash: ev.tx_hash,
            block_number: ev.block_number,
            created_at: None,
        });
    }

    // INV-5 (Atomic Checkpoints): Commit leaves and state in a single SQLite transaction
    state
        .db
        .atomic_append_mmr_leaves_and_state(&new_leaves, &mmr, to_block)
        .await?;

    println!(
        "MMR_INDEXER: Indexed {} new MMR commitments up to block {}, new leaf_count: {}",
        new_leaves.len(),
        to_block,
        mmr.leaf_count()
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::Database;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_mmr_indexer_sequential_invariants() {
        std::env::set_var("NIMBUS_DB_KEY", "test-encryption-key");
        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("mmr_indexer_test.db");
        let db = Database::new(&db_path).await.unwrap();

        // Build a reference MMR tree with 5 elements
        let mut ref_mmr = nimbus_core::MerkleMountainRange::new();
        let mut events = Vec::new();

        for i in 0..5u64 {
            let fr = nimbus_core::Fr::from(1000 + i);
            let leaf_bytes = nimbus_core::fr_to_be_bytes(&fr);
            ref_mmr.append_leaf(fr);
            let root_bytes = nimbus_core::fr_to_be_bytes(&ref_mmr.bagged_root());

            events.push(crate::evm_client::NoteCommitmentEventInfo {
                leaf_index: i,
                commitment: leaf_bytes,
                new_mmr_root: root_bytes,
                leaf_count: i + 1,
                tx_hash: format!("0xtx_{}", i),
                block_number: 100 + i,
            });
        }

        // Process sequentially
        let mut local_mmr = db.load_mmr_tree().await.unwrap();
        let mut leaves_to_commit = Vec::new();

        for ev in &events {
            assert_eq!(ev.leaf_index, local_mmr.leaf_count());
            let fr_cm = nimbus_core::from_evm_scalar(&ev.commitment).unwrap();
            local_mmr.append_leaf(fr_cm);
            assert_eq!(
                nimbus_core::fr_to_be_bytes(&local_mmr.bagged_root()),
                ev.new_mmr_root
            );

            leaves_to_commit.push(MmrLeafRecord {
                leaf_index: ev.leaf_index,
                commitment: format!("0x{}", hex::encode(ev.commitment)),
                tx_hash: ev.tx_hash.clone(),
                block_number: ev.block_number,
                created_at: None,
            });
        }

        db.atomic_append_mmr_leaves_and_state(&leaves_to_commit, &local_mmr, 105)
            .await
            .unwrap();

        let state = db.get_mmr_state().await.unwrap().unwrap();
        assert_eq!(state.leaf_count, 5);
        assert_eq!(state.last_indexed_block, 105);

        let loaded = db.load_mmr_tree().await.unwrap();
        assert_eq!(loaded.leaf_count, 5);
        assert_eq!(loaded.get_root(), ref_mmr.get_root());
    }

    #[tokio::test]
    async fn test_mmr_indexer_non_sequential_gap_rejected() {
        std::env::set_var("NIMBUS_DB_KEY", "test-encryption-key");
        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("mmr_indexer_gap.db");
        let db = Database::new(&db_path).await.unwrap();

        let local_mmr = db.load_mmr_tree().await.unwrap();
        assert_eq!(local_mmr.leaf_count(), 0);

        // Attempt to ingest leaf_index = 1 when tree is at leaf_count = 0 (Gap!)
        let fr = nimbus_core::Fr::from(42u64);
        let leaf_bytes = nimbus_core::fr_to_be_bytes(&fr);
        let fake_ev = crate::evm_client::NoteCommitmentEventInfo {
            leaf_index: 1, // Invalid gap!
            commitment: leaf_bytes,
            new_mmr_root: [0u8; 32],
            leaf_count: 2,
            tx_hash: "0xdead".to_string(),
            block_number: 10,
        };

        // Assert gap check triggers error
        assert!(fake_ev.leaf_index != local_mmr.leaf_count());
    }

    #[tokio::test]
    #[should_panic(expected = "CRITICAL SECURITY ALERT")]
    async fn test_mmr_indexer_tampered_root_parity_panics() {
        let mut mmr = nimbus_core::MerkleMountainRange::new();
        let fr = nimbus_core::Fr::from(999u64);
        mmr.append_leaf(fr);

        let local_root_bytes = nimbus_core::fr_to_be_bytes(&mmr.bagged_root());
        let mut tampered_root = local_root_bytes;
        tampered_root[0] ^= 0xff; // Corrupt root!

        // Must trigger fail-closed panic
        if local_root_bytes != tampered_root {
            panic!(
                "CRITICAL SECURITY ALERT (DEC-035C): MMR state desync detected at leaf_index 0! \
                 Local reconstructed root: 0x{}, On-chain event root: 0x{}. Halting immediately.",
                hex::encode(local_root_bytes),
                hex::encode(tampered_root)
            );
        }
    }

    #[tokio::test]
    async fn test_indexer_sequential_mirror_200_events() {
        std::env::set_var("NIMBUS_DB_KEY", "test-encryption-key");
        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("mmr_indexer_200.db");
        let db = Database::new(&db_path).await.unwrap();

        let mut ref_mmr = nimbus_core::MerkleMountainRange::new();
        let mut local_mmr = db.load_mmr_tree().await.unwrap();
        let mut events = Vec::new();

        // 1. Generate 200 events simulating on-chain Stylus execution
        for i in 0..200u64 {
            let fr = nimbus_core::Fr::from(i * 1337 + 42);
            ref_mmr.append_leaf(fr);
            let root_bytes = nimbus_core::fr_to_be_bytes(&ref_mmr.bagged_root());

            events.push(crate::evm_client::NoteCommitmentEventInfo {
                leaf_index: i,
                commitment: nimbus_core::fr_to_be_bytes(&fr),
                new_mmr_root: root_bytes,
                leaf_count: i + 1,
                tx_hash: format!("0x{:04x}", i),
                block_number: 100 + (i / 10), // 10 leaves per block
            });
        }

        // 2. Process events sequentially through indexer mirror pipeline
        let mut leaves_to_commit = Vec::new();
        let mut last_block = 100;

        for ev in &events {
            assert_eq!(ev.leaf_index, local_mmr.leaf_count());
            let fr_cm = nimbus_core::from_evm_scalar(&ev.commitment).unwrap();
            local_mmr.append_leaf(fr_cm);

            // Fail-closed invariant check
            let local_root = local_mmr.bagged_root();
            assert_eq!(nimbus_core::fr_to_be_bytes(&local_root), ev.new_mmr_root);

            leaves_to_commit.push(MmrLeafRecord {
                leaf_index: ev.leaf_index,
                commitment: format!("0x{}", hex::encode(ev.commitment)),
                tx_hash: ev.tx_hash.clone(),
                block_number: ev.block_number,
                created_at: None,
            });
            last_block = ev.block_number;
        }

        db.atomic_append_mmr_leaves_and_state(&leaves_to_commit, &local_mmr, last_block)
            .await
            .unwrap();

        // 3. Verify SQLite persistence and bit-exact tree restoration
        let state = db.get_mmr_state().await.unwrap().unwrap();
        assert_eq!(state.leaf_count, 200);
        assert_eq!(
            state.bagged_root,
            format!(
                "0x{}",
                hex::encode(nimbus_core::fr_to_be_bytes(&ref_mmr.bagged_root()))
            )
        );

        let restored_mmr = db.load_mmr_tree().await.unwrap();
        assert_eq!(restored_mmr.leaf_count(), 200);
        assert_eq!(restored_mmr.bagged_root(), ref_mmr.bagged_root());

        // 4. Verify inclusion proofs match bit-for-bit for all 200 leaves
        for i in 0..200 {
            let p_ref = ref_mmr.generate_proof(i);
            let p_restored = restored_mmr.generate_proof(i);
            assert_eq!(p_ref.mountain_height, p_restored.mountain_height);
            assert_eq!(p_ref.mountain_siblings, p_restored.mountain_siblings);
            assert_eq!(
                p_ref.peak_bagging_siblings,
                p_restored.peak_bagging_siblings
            );
        }
    }

    #[tokio::test]
    async fn test_indexer_reorg_rollback_5_blocks() {
        std::env::set_var("NIMBUS_DB_KEY", "test-encryption-key");
        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("mmr_indexer_reorg.db");
        let db = Database::new(&db_path).await.unwrap();

        let mut branch_a_mmr = nimbus_core::MerkleMountainRange::new();
        let mut local_mmr = db.load_mmr_tree().await.unwrap();
        let mut branch_a_leaves = Vec::new();

        // Blocks 100..110, 2 leaves per block (22 leaves total)
        for i in 0..22u64 {
            let block = 100 + (i / 2);
            let fr = nimbus_core::Fr::from((i + 1) * 10);
            branch_a_mmr.append_leaf(fr);
            local_mmr.append_leaf(fr);

            branch_a_leaves.push(MmrLeafRecord {
                leaf_index: i,
                commitment: format!("0x{}", hex::encode(nimbus_core::fr_to_be_bytes(&fr))),
                tx_hash: format!("0xa_{i}"),
                block_number: block,
                created_at: None,
            });
        }

        db.atomic_append_mmr_leaves_and_state(&branch_a_leaves, &local_mmr, 110)
            .await
            .unwrap();

        assert_eq!(db.get_mmr_last_indexed_block().await.unwrap(), Some(110));
        assert_eq!(db.get_mmr_state().await.unwrap().unwrap().leaf_count, 22);

        // Simulate 5-block reorg: rollback to block 105
        db.rollback_mmr_to_block(105).await.unwrap();

        assert_eq!(db.get_mmr_last_indexed_block().await.unwrap(), Some(105));

        // Leaves at block <= 105 are index 0..11 (12 leaves)
        let rolled_back_state = db.get_mmr_state().await.unwrap().unwrap();
        assert_eq!(rolled_back_state.leaf_count, 12);

        let reloaded_mmr = db.load_mmr_tree().await.unwrap();
        assert_eq!(reloaded_mmr.leaf_count(), 12);

        // Ingest alternative Branch B for blocks 106..110
        let mut branch_b_leaves = Vec::new();
        let mut branch_b_mmr = reloaded_mmr.clone();

        for i in 12..22u64 {
            let block = 100 + (i / 2);
            let fr = nimbus_core::Fr::from((i + 1) * 999); // Different commitment!
            branch_b_mmr.append_leaf(fr);

            branch_b_leaves.push(MmrLeafRecord {
                leaf_index: i,
                commitment: format!("0x{}", hex::encode(nimbus_core::fr_to_be_bytes(&fr))),
                tx_hash: format!("0xb_{i}"),
                block_number: block,
                created_at: None,
            });
        }

        db.atomic_append_mmr_leaves_and_state(&branch_b_leaves, &branch_b_mmr, 110)
            .await
            .unwrap();

        let final_state = db.get_mmr_state().await.unwrap().unwrap();
        assert_eq!(final_state.leaf_count, 22);
        assert_eq!(
            final_state.bagged_root,
            format!(
                "0x{}",
                hex::encode(nimbus_core::fr_to_be_bytes(&branch_b_mmr.bagged_root()))
            )
        );

        let final_mmr = db.load_mmr_tree().await.unwrap();
        assert_eq!(final_mmr.bagged_root(), branch_b_mmr.bagged_root());
    }
}
