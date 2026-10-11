# Task B: Client SDK Multi-UTXO Knapsack Coin Selection, In-Pool Progressive Consolidation, and Anti-Snooping MMR Tree Sync

- **Specification Reference:** [`research/decisions/DEC-036B-client-sdk-knapsack-coin-selection-consolidation-and-bulk-sync.md`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-036B-client-sdk-knapsack-coin-selection-consolidation-and-bulk-sync.md)
- **Parent Blueprint:** [`research/decisions/DEC-036-universal-2-in-2-out-joinsplit-full-stack-architecture.md`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-036-universal-2-in-2-out-joinsplit-full-stack-architecture.md)
- **Status:** COMPLETED
- **Applies to:**
  - `nimbus-sdk/src/wallet/note_wallet.rs` (`PrivateNoteWallet` state, coin selection, local proof generation, and consolidation)
  - `nimbus-sdk/src/coin_selection.rs` (Stochastic Knapsack multi-note selection algorithm)
  - `nimbus-sdk/src/client.rs` (HTTP client routines for relayer quotes, bulk MMR sync, and spend broadcast)
  - `nimbus-sdk/src/storage.rs` (SQLCipher / IndexedDB crash-safe 2PC note state transitions)
  - `nimbus-sdk/tests/` (Unit tests, knapsack simulations, crash-recovery tests)
- **Primary Objective:** Upgrade `PrivateNoteWallet` from single-note exact spends to production multi-UTXO operations: implement 4-tier Stochastic Knapsack coin selection with economic dust absorption (< 0.10 USDC), automated `consolidate_notes` pipeline for fragmented wallets ($N > 2$ notes), anti-snooping paginated bulk MMR tree sync (`GET /api/v1/mmr/leaves`) protecting user leaf indices, dual-epoch witness handling, and crash-safe Two-Phase Commit (2PC) SQLite persistence.

---

## Progress Overview

- [x] **Phase 1: Multi-UTXO Stochastic Knapsack Coin Selection (`nimbus-sdk`)** [4/4]
- [x] **Phase 2: In-Pool Progressive Consolidation (`consolidate_notes`)** [4/4]
- [x] **Phase 3: Anti-Snooping Paginated Bulk MMR Tree Sync** [4/4]
- [x] **Phase 4: Dual-Witness Generation & Dual-Epoch Sync** [4/3]
- [x] **Phase 5: Crash-Safe 2PC Note Lifecycle & AEAD Storage** [3/3]
- [x] **Phase 6: Client SDK Test Suite & Integration Tests** [4/4]

---

## Detailed Task Breakdown

### Phase 1: Multi-UTXO Stochastic Knapsack Coin Selection
- [x] **B1.1 — 4-Tier Selection Pipeline:**
  Implement tiered search in `nimbus-sdk/src/coin_selection.rs`:
  - Tier 1: Exact match single note ($v = T$).
  - Tier 2: Smallest sufficient single note ($v > T$).
  - Tier 3: 2-note combination via Stochastic Knapsack.
  - Tier 4: Return `FragmentationLockout` error advising `consolidate_notes()`.
- [x] **B1.2 — Stochastic Knapsack Cost Function:**
  Implement penalty optimization:
  $$\text{Cost}(i, j) = (v_i + v_j - T) + \alpha \cdot |v_i - v_j| + \beta \cdot \text{AgePenalty}(n_i, n_j) + \epsilon$$
  where $\epsilon$ injects random entropy to prevent Wasabi-style Knapsack fingerprinting.
- [x] **B1.3 — Economic Dust Floor Absorption:**
  If change $v_{\text{change}} < \text{min\_dust\_threshold}$ ($100\,000$ units / $0.10 USDC):
  - Absorb change into `execution_fee` markup.
  - Set `has_change_1 = false` and switch to dummy output.
- [x] **B1.4 — Note Epoch Filtering:**
  Exclude notes from fully expired epochs prior to rollover.

---

### Phase 2: In-Pool Progressive Consolidation (`consolidate_notes`)
- [x] **B2.1 — Self-Spend Zero-Payout Transaction Builder:**
  Implement `PrivateNoteWallet::consolidate_notes(&self, target_amount: Option<u64>)`.
  Construct JoinSplit with $v_{\text{merchant}} = 0$, $v_{\text{protocol\_fee}} = 0$, and `recipient = contract_address`.
- [x] **B2.2 — Value Fusion & Output Generation:**
  Combine 2 smallest notes into $v_{out,1} = v_1 + v_2 - v_{\text{execution\_fee}}$, with $v_{out,2} = 0$ (canonical dummy output).
- [x] **B2.3 — Iterative Consolidation Loop:**
  Loop progressive self-spends until wallet note count $\le 2$ or sufficient for target amount.
- [x] **B2.4 — Gas Estimation & User Approval:**
  Provide total relayer fee summary prior to execution.

---

### Phase 3: Anti-Snooping Paginated Bulk MMR Tree Sync
- [x] **B3.1 — Relayer Bulk Leaves Client Routine:**
  Implement `client.fetch_mmr_leaves(from_index: u64, limit: u64)` calling `GET /api/v1/mmr/leaves?from={idx}&limit=1000`.
- [x] **B3.2 — Client-Side Incremental MMR Appends:**
  Feed downloaded leaves in 1,000-leaf chunks into in-memory `MerkleMountainRange`.
- [x] **B3.3 — Bagged Root Assertion:**
  Assert locally computed bagged root matches on-chain `note_root` checkpoint.
- [x] **B3.4 — Deprecate Single-Leaf Query in Production:**
  Ensure production spend path never invokes single-leaf index queries.

---

### Phase 4: Dual-Witness Generation & Dual-Epoch Sync
- [x] **B4.1 — Dual-Witness Assembly:**
  Assemble `PrivateNoteWitness` 1 and 2 from synchronized local MMR proofs.
- [x] **B4.2 — Canonical Dummy Witness Synthesis:**
  If spending 1 note, generate dummy witness with `is_dummy_2 = true`, $v=0$, and `epoch_id_2 = epoch_id_1`.
- [x] **B4.3 — Dual-Epoch Nullifier PRF Alignment:**
  Derive $\text{nf}_1$ with `epoch_id_1` and $\text{nf}_2$ with `epoch_id_2`.

---

### Phase 5: Crash-Safe 2PC Note Lifecycle & AEAD Storage
- [x] **B5.1 — SQLite Schema Migration:**
  Create `private_notes` and `joinsplit_sessions` tables with status constraints.
- [x] **B5.2 — Phase 1 Pre-Broadcast Lock:**
  Atomic transaction setting input notes to `SPENT_PENDING` and change notes to `UNCONFIRMED`.
- [x] **B5.3 — Phase 2 Finalization & Rollback:**
  On confirmed tx receipt: transition to `SPENT_CONFIRMED` and `ACTIVE`. On revert/timeout: rollback inputs to `ACTIVE` and purge unconfirmed change notes.

---

### Phase 6: Client SDK Test Suite & Integration Tests
- [x] **B6.1 — Knapsack Coin Selection Unit Tests:**
  Test Tier 1, Tier 2, Tier 3, and Tier 4 boundary conditions.
- [x] **B6.2 — In-Pool Consolidation Test:**
  Simulate consolidating 6 fragmented notes down to 2 notes.
- [x] **B6.3 — Bulk Sync Benchmark:**
  Verify syncing 10,000 leaves takes $< 100 \text{ ms}$ on local CPU (and sustained throughput in memory).
- [x] **B6.4 — Crash Recovery Simulation:**
  Simulate app termination post-broadcast; verify no fund loss on restart.
