# Task C: Relayer MMR Indexer Worker & Client Sync Synchronization Architecture

- **Specification Reference:** [`research/decisions/DEC-035C-relayer-mmr-indexer-worker-and-client-sync-architecture.md`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-035C-relayer-mmr-indexer-worker-and-client-sync-architecture.md)
- **Parent Blueprint:** [`research/decisions/DEC-035-cryptographic-canonicality-public-input-binding-and-mmr.md`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-035-cryptographic-canonicality-public-input-binding-and-mmr.md)
- **Status:** PENDING IMPLEMENTATION (Founder Authorization Checkpoint Required Before Code Edits)
- **Primary Objective:** Build an end-to-end, reorg-safe Merkle Mountain Range (MMR) synchronization pipeline connecting the on-chain Stylus contract to off-chain client wallets via an authoritative relayer daemon and privacy-preserving REST API (`GET /api/v1/mmr/proof/{leaf_index}`), enabling multi-wallet private note spends on Arbitrum Sepolia.

---

## Progress Overview

- [ ] **Phase 1: SQLite Storage & Schema Migrations (`nimbus-node/src/db/`)** [0/4]
- [ ] **Phase 2: Background MMR Event Indexer Daemon (`nimbus-node/src/mmr_indexer.rs`)** [0/5]
- [ ] **Phase 3: Relayer Client Sync REST API (`nimbus-node/src/handlers/mmr.rs`)** [0/4]
- [ ] **Phase 4: Client SDK Dynamic Inclusion Sync (`nimbus-sdk`)** [0/4]
- [ ] **Phase 5: Exhaustive Testing & Testnet Hardening Suite** [0/5]

---

## Detailed Task Breakdown

### Phase 1: SQLite Storage & Schema Migrations (`nimbus-node`)
- [ ] **C1.1 — Schema Definition (`db/schema.rs`):**
  Create tables `mmr_leaves` and `mmr_state` in SQLite:
  ```sql
  CREATE TABLE IF NOT EXISTS mmr_leaves (
      leaf_index INTEGER PRIMARY KEY,
      commitment TEXT NOT NULL,
      tx_hash TEXT NOT NULL,
      block_number INTEGER NOT NULL,
      created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
  );
  CREATE INDEX IF NOT EXISTS idx_mmr_leaves_block ON mmr_leaves(block_number);

  CREATE TABLE IF NOT EXISTS mmr_state (
      id INTEGER PRIMARY KEY CHECK (id = 1),
      leaf_count INTEGER NOT NULL,
      bagged_root TEXT NOT NULL,
      peaks_json TEXT NOT NULL,
      last_indexed_block INTEGER NOT NULL,
      updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
  );
  ```
- [ ] **C1.2 — Leaf Persistence Operations:**
  Implement `insert_mmr_leaf(leaf_index, commitment, tx_hash, block_number)` and `get_mmr_leaf(leaf_index)` in `nimbus-node/src/db/`.
- [ ] **C1.3 — MMR State Snapshot Operations:**
  Implement `save_mmr_state(mmr: &MerkleMountainRange, block_number: u64)` and `load_mmr_tree() -> Result<MerkleMountainRange>` to persist and restore active peak states.
- [ ] **C1.4 — Reorg Rollback Query:**
  Implement `rollback_mmr_to_block(fork_block: u64)` to delete leaves above the reorg fork height and re-derive peak state.

---

### Phase 2: Background MMR Indexer Daemon (`nimbus-node`)
- [ ] **C2.1 — Indexer Struct & Lifecycle Loop (`mmr_indexer.rs`):**
  Implement `MmrIndexer` background worker with:
  - Event polling loop with configurable interval.
  - Safe finalized block window: $\text{safe\_block} = \text{latest\_block} - \text{SAFE\_CONFIRMATIONS}$.
  - Clean shutdown receiver (`broadcast::Receiver<()>`).
- [ ] **C2.2 — On-Chain Log Filtering (`Alloy`):**
  Filter events matching `NoteCommitmentAppended(uint256 indexed leaf_index, bytes32 commitment, bytes32 bagged_root)` from the deployed Stylus contract.
- [ ] **C2.3 — Sequential Leaf Append & Invariant Check:**
  For each detected event, assert $\text{event.leaf\_index} == \text{mmr.leaf\_count()}$.
  Append scalar commitment via `mmr.append_leaf(from_evm_scalar(&commitment)?)`.
- [ ] **C2.4 — Fail-Closed Root Parity Guard:**
  Assert `mmr.bagged_root()? == event.bagged_root`.
  If there is any discrepancy, log a critical security alert and panic immediately (halting proof serving).
- [ ] **C2.5 — Atomic Transaction Commits:**
  Commit `mmr_leaves` insertions and `mmr_state` checkpoint in a single atomic database transaction.

---

### Phase 3: Relayer Client Sync REST API (`nimbus-node`)
- [ ] **C3.1 — Endpoint `GET /api/v1/mmr/proof/{leaf_index}`:**
  Implement handler in `nimbus-node/src/handlers/mmr.rs`:
  - Validate bounds: if $\text{leaf\_index} \ge \text{current\_leaf\_count}$, return `404 Not Found` with `code: "LeafIndexOutOfBounds"`.
  - Reconstruct mountain inclusion path and peak bagging siblings using `nimbus-core::MerkleMountainRange`.
  - Return JSON with `mountain_height`, `mountain_siblings`, `peak_bagging_siblings`, `bagged_root`, and `block_number`.
- [ ] **C3.2 — Endpoint `GET /api/v1/mmr/tip`:**
  Implement lightweight status endpoint returning current `leaf_count`, `bagged_root`, and `last_indexed_block`.
- [ ] **C3.3 — Route Registration (`routes.rs`):**
  Register MMR endpoints in Axum application router.
- [ ] **C3.4 — Privacy Boundary Assurance:**
  Ensure request logger records only the integer position, never accepting or logging private commitments, nullifiers, or user identifiers.

---

### Phase 4: Client SDK Dynamic Sync (`nimbus-sdk`)
- [ ] **C4.1 — REST Proof Fetcher (`note_wallet.rs`):**
  Implement `PrivateNoteWallet::fetch_mmr_proof(relayer_url, leaf_index)` in `nimbus-sdk`:
  - Query `GET /api/v1/mmr/proof/{leaf_index}`.
  - Handle `404 Not Found` cleanly with typed error `NoteNotYetIndexed`.
- [ ] **C4.2 — Sibling Deserialization:**
  Deserialize hex string siblings into `ark_bls12_381::Fr` scalars via `from_evm_scalar`.
- [ ] **C4.3 — Circuit Witness Assembly:**
  Populate `PrivateNoteCircuitWitness` with:
  - `note_root = bagged_root`
  - `leaf_count = proof_data.leaf_count`
  - `mountain_height = proof_data.mountain_height`
  - `mountain_siblings`
  - `peak_bagging_siblings`
- [ ] **C4.4 — Local Witness Cache:**
  Cache retrieved inclusion proofs locally by `(leaf_index, bagged_root)` to allow offline proving when the tree root has not changed.

---

### Phase 5: Verification & Testnet Hardening Suite
- [ ] **C5.1 — Indexer Sequential Mirror Unit Test:**
  Ingest 200 simulated events; assert SQLite mirror produces bit-exact root and peak matching `MerkleMountainRange`.
- [ ] **C5.2 — Reorg Rollback Simulation Test:**
  Simulate 5-block chain reorg in test environment; assert indexer unwinds state and resumes without corruption.
- [ ] **C5.3 — Hyperbridge Out-of-Bounds Negative Test:**
  Query `leaf_index = 50` on a 50-leaf tree $\implies$ assert API returns HTTP 404 (`LeafIndexOutOfBounds`).
- [ ] **C5.4 — End-to-End Multi-Wallet Spend Test:**
  Simulate Wallet A depositing Note 1 $\to$ Wallet B depositing Note 2 $\to$ Wallet A querying relayer sync API $\to$ generating valid Groth16 spend proof $\to$ verified on Stylus contract.
- [ ] **C5.5 — Automated Lint & Quality Check:**
  `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` clean across workspace.

---

## Invariant Verification Checklist

- [ ] **INV-1 (Reorg-Safe Indexing):** Indexer only advances up to $\text{latest\_block} - \text{SAFE\_CONFIRMATIONS}$.
- [ ] **INV-2 (Fail-Closed Root Parity):** Every indexed leaf's local bagged root matches on-chain event root bit-for-bit.
- [ ] **INV-3 (Privacy Preservation):** Queries accepted only by public integer position (`leaf_index`), zero secret exposure.
- [ ] **INV-4 (Strict Upper Bound):** Queries with $\text{leaf\_index} \ge \text{leaf\_count}$ are strictly rejected (anti-Hyperbridge exploit).
- [ ] **INV-5 (Atomic Checkpoints):** Database state updates are fully transactional and crash-safe.
