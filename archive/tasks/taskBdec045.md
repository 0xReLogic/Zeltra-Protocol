# Task B: In-Memory Concurrent Accumulator, Incremental Indexer & Zero-Latency Proof Ingress

- **Specification Reference:** [`research/decisions/DEC-045B-in-memory-concurrent-accumulator-and-relayer-proof-ingress.md`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-045B-in-memory-concurrent-accumulator-and-relayer-proof-ingress.md)
- **Parent Blueprint:** [`research/decisions/DEC-045-in-memory-incremental-mmr-engine-and-relayer-proof-acceleration.md`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-045-in-memory-incremental-mmr-engine-and-relayer-proof-acceleration.md)
- **Status:** COMPLETED & VERIFIED (100% TEST PASS)
- **Applies to:**
  - `zeltra-node/src/state.rs` (`AppState` with `in_memory_mmr: Arc<RwLock<FlatMerkleMountainRange>>`)
  - `zeltra-node/src/handlers/mmr.rs` (`handle_mmr_proof`, `handle_mmr_tip`, `handle_mmr_leaves` fast serving)
  - `zeltra-node/src/mmr_indexer.rs` (Single-writer background worker incremental append and block tracking)
  - `zeltra-node/src/database.rs` (`batch_insert_mmr_leaves`, persistent snapshotting, checkpoint table)
- **Security Audit Resolusi:** AUD-15 (Relayer CPU Starvation / Proof Ingress DoS), AUD-16 (MMR Indexer Head Desync), AUD-17 (Mempool Proof Invalidation under Reorg)
- **Primary Objective:** Eliminate the $O(N)$ full database reload from `handle_mmr_proof` by storing a shared thread-safe concurrent `FlatMerkleMountainRange` in `AppState`. Fast read-paths serve proofs via reader locks in $< 100\,\mu\text{s}$, while the indexer operates as an exclusive background writer. Abstract the persistence layer behind an `MMRStore` trait for future zero-friction migration to RocksDB or `mmap`.

---

## Progress Overview

- [x] **Phase 1: Concurrent `AppState` & Cold Boot Warm-Up (`zeltra-node`)** [3/3]
- [x] **Phase 2: Sub-Microsecond Multi-Reader Proof Serving (`handlers/mmr.rs`)** [3/3]
- [x] **Phase 3: Single-Writer Incremental MMR Indexer (`mmr_indexer.rs`)** [4/4]
- [x] **Phase 4: Generic Storage Abstraction Trait (`MMRStore`)** [3/3]
- [x] **Phase 5: Exhaustive Test Matrix (1x Positive + $\ge 2\times$ Negative)** [10/10]

---

## Detailed Task Breakdown

### Phase 1: Concurrent `AppState` & Cold Boot Warm-Up
- [x] **B1.1 — Integrate In-Memory MMR into `AppState`:**
  Add `pub in_memory_mmr: Arc<tokio::sync::RwLock<zeltra_core::FlatMerkleMountainRange>>` to `AppState`.
- [x] **B1.2 — Cold Startup Pre-Warming from SQLCipher:**
  During `AppState::new()`, load existing leaves in order from `database.get_all_mmr_leaves()` and populate `FlatMerkleMountainRange`.
- [x] **B1.3 — Checkpoint Parity Verification:**
  Compare bagged root of restored tree against on-chain contract `note_tree_root`. Revert with `ColdBootDesync` if discrepancy detected.

---

### Phase 2: Sub-Microsecond Multi-Reader Proof Serving
- [x] **B2.1 — Fast-Path `handle_mmr_proof` Rewriting:**
  Acquire reader lock `.read().await` on `in_memory_mmr`. Execute `generate_proof(leaf_index)` without database I/O.
- [x] **B2.2 — Zero I/O Lock Invariant (INV-45B-1):**
  Ensure JSON serialization and network response generation occur outside or immediately upon lock release; zero blocking calls while holding lock.
- [x] **B2.3 — Optimized Tip & Leaves Handlers:**
  Serve `handle_mmr_tip` directly from `mmr.get_root()` and `mmr.leaf_count` in RAM.

---

### Phase 3: Single-Writer Incremental MMR Indexer
- [x] **B3.1 — Asynchronous Single-Writer Lock Acquisition:**
  In `mmr_indexer.rs`, acquire `.write().await` only when on-chain `NoteCommitmentAppended` logs are ready for insertion.
- [x] **B3.2 — Batch Persistence to SQLite:**
  Persist leaves to database via `batch_insert_mmr_leaves` inside the same indexing transaction.
- [x] **B3.3 — Atomic Incremental Tree Append:**
  Append new leaf commitments into `in_memory_mmr` sequentially.
- [x] **B3.4 — Reorg Detection & Checkpoint Rollback:**
  If block hash reorganization detected within confirmation window, rollback `in_memory_mmr` to canonical checkpoint state.

---

### Phase 4: Generic Storage Abstraction Trait
- [x] **B4.1 — Define `MMRStore` Asynchronous Trait:**
  Define trait with `get_node(pos: u64)`, `append_nodes(start_pos: u64, nodes: &[Fr])`, and `save_checkpoint()`.
- [x] **B4.2 — Implement `SqliteMmrStore`:**
  Wrap existing SQLCipher database queries inside `SqliteMmrStore` adhering to `MMRStore`.
- [x] **B4.3 — Architectural Readiness for RocksDB:**
  Verify that swapping `SqliteMmrStore` with a future `RocksDbMmrStore` requires zero modifications to Axum handlers or indexer tasks.

---

### Phase 5: Exhaustive Test Matrix (Positive + $\ge 2\times$ Negative)

#### Positive Tests (3 Tests):
- [x] **B5.1 — `test_relayer_handle_mmr_proof_cached_success`:**
  Endpoint returns `200 OK` with valid `MmrProofResponse`. Serving latency under concurrency verified $< 100\,\mu\text{s}$ (zero DB scan).
- [x] **B5.2 — `test_relayer_mmr_indexer_incremental_append_success`:**
  Worker receives batch of 50 on-chain note events. Appends to DB and RAM; asserts `leaf_count` increments by 50 and root remains identical.
- [x] **B5.3 — `test_relayer_cold_startup_restores_canonical_state`:**
  Populate DB with 500 leaves, terminate process. Re-boot server; assert `AppState::new()` loads tree matching checkpoint without drift.

#### Negative Tests (7 Tests — $>2\times$ Ratio, Proving State Invariance):
- [x] **B5.4 — `test_relayer_handle_mmr_proof_rejects_out_of_bounds`:**
  Request `GET /api/v1/mmr/proof/:leaf_index` with `leaf_index >= leaf_count`. Assert `404 Not Found` with `LeafIndexOutOfBounds`. State in RAM and DB remains unchanged.
- [x] **B5.5 — `test_relayer_handle_mmr_proof_rejects_empty_tree`:**
  Request proof on empty tree ($N=0$). Assert `404 Not Found`. State remains unchanged.
- [x] **B5.6 — `test_relayer_handle_mmr_proof_rejects_malformed_index_path`:**
  Request proof with invalid path string (`/proof/abc` or `/proof/-1`). Assert Axum rejects with `400 Bad Request` or `404 Not Found`.
- [x] **B5.7 — `test_relayer_mmr_indexer_skips_duplicate_leaves`:**
  Indexer encounters event with `leaf_index < current_leaf_count`. Assert warning logged and event skipped without modifying RAM or DB.
- [x] **B5.8 — `test_relayer_mmr_indexer_rejects_non_contiguous_leaf_gap`:**
  Indexer receives event with non-contiguous leaf index (gap detected). Assert indexer fails closed (`anyhow::bail!`) to prevent tree fragmentation.
- [x] **B5.9 — `test_relayer_mmr_indexer_recovers_from_shallow_reorg`:**
  Simulate 2-block L2 reorg. Assert indexer detects desync and restores state to pre-reorg checkpoint.
- [x] **B5.10 — `test_relayer_concurrency_stress_zero_deadlock`:**
  50 parallel tasks continuously querying proofs while indexer appends new leaves for 5 seconds. Assert 0 deadlocks, 0 panics, 0 starvation.
