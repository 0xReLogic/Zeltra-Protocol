# Task A: Flat Post-Order MMR Array, Bitwise Sibling Navigation & Zero-Hash Proof Engine

- **Specification Reference:** [`research/decisions/DEC-045A-flat-postorder-mmr-array-and-zero-hash-proof-engine.md`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-045A-flat-postorder-mmr-array-and-zero-hash-proof-engine.md)
- **Parent Blueprint:** [`research/decisions/DEC-045-in-memory-incremental-mmr-engine-and-relayer-proof-acceleration.md`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-045-in-memory-incremental-mmr-engine-and-relayer-proof-acceleration.md)
- **Status:** READY FOR IMPLEMENTATION
- **Applies to:**
  - `zeltra-core/src/merkle.rs` (`FlatMerkleMountainRange`, bitwise post-order navigation, `generate_proof`, zero-hash engine)
  - `zeltra-core/src/lib.rs` (Re-exports `FlatMerkleMountainRange`, `FlatMmrError`)
  - `zeltra-core/tests/` (Parity verification tests, zero-hash proof verification, negative security exploit vectors)
- **Security Audit Resolusi:** AUD-12 (O(N) CPU Starvation DoS), AUD-13 (Out-of-Bounds Hyperbridge MMR Exploit), AUD-14 (Subtree Mutability Drift)
- **Primary Objective:** Replace the legacy $O(2^h)$ on-the-fly re-hashing MMR proof generator with a zero-hash Peter Todd flat post-order array engine (`nodes: Vec<Fr>`). Intermediate parent nodes are computed strictly once during `append()`. Inclusion proof generation performs strictly zero Poseidon hashes, dropping proof latency from 366 ms to $< 20\,\mu\text{s}$ ($O(\log N)$ memory lookups) while maintaining 100% mathematical parity with Stylus contract peak bagging.

---

## Progress Overview

- [ ] **Phase 1: Bitwise Post-Order Navigation Algorithms (`zeltra-core`)** [4/4]
- [ ] **Phase 2: Flat Post-Order MMR Struct & Append Logic** [4/4]
- [ ] **Phase 3: Zero-Hash `generate_proof()` Implementation** [3/3]
- [ ] **Phase 4: Backward Compatibility & Root Parity with Legacy MMR** [3/3]
- [ ] **Phase 5: Exhaustive Test Matrix (1x Positive + $\ge 2\times$ Negative)** [10/10]

---

## Detailed Task Breakdown

### Phase 1: Bitwise Post-Order Navigation Algorithms
- [ ] **A1.1 — Implement Peter Todd Leaf Index to Position Mapping:**
  Implement bitwise conversion `leaf_index_to_pos(leaf_index: usize) -> usize` using $pos = 2 \cdot leaf\_index - \text{count\_ones}(leaf\_index)$.
- [ ] **A1.2 — Implement Position Height Determination:**
  Implement `pos_height(mut pos: usize) -> usize` using bitwise trailing ones subtraction loop.
- [ ] **A1.3 — Implement Left/Right Sibling Predicate:**
  Implement `is_left_sibling(pos: usize, height: usize) -> bool` using bitwise shift `((pos + 1 + (2 << height)) & (2 << (height + 1))) != 0`.
- [ ] **A1.4 — Implement Peak Positions Extractor:**
  Implement `get_peak_positions(leaf_count: usize) -> Vec<usize>` decomposing `leaf_count` into powers of 2.

---

### Phase 2: Flat Post-Order MMR Struct & Append Logic
- [ ] **A2.1 — Define `FlatMerkleMountainRange` Struct & Error Enums:**
  Define struct with `nodes: Vec<Fr>`, `leaf_count: usize`, `peak_positions: Vec<usize>`, and `cached_root: Option<Fr>`. Define `FlatMmrError` enum (`LeafIndexOutOfBounds`, `EmptyTree`, `NonCanonicalScalar`).
- [ ] **A2.2 — Single-Pass Inremental `append(commitment: Fr)`:**
  Push leaf node at current array position. If previous subtree is complete, merge immediately with left sibling using `Poseidon_W5([left, right, 0, 0], DOMAIN_MMR_MERGE)` and record parent node in `nodes`. Repeat until highest peak.
- [ ] **A2.3 — Array Size Invariant Enforcement:**
  Enforce invariant $|nodes| = 2N - \text{count\_ones}(N)$ after every append operation.
- [ ] **A2.4 — Peak Bagging Aggregation & Root Caching:**
  Bag active peaks backward-fold from right to left using `DOMAIN_MMR_BAG` matching DEC-032 specification. Cache bagged root in `cached_root`.

---

### Phase 3: Zero-Hash `generate_proof()` Implementation
- [ ] **A3.1 — Internal Mountain Climbing without Hashing:**
  Extract internal sibling nodes from `self.nodes[sibling_pos]` directly via memory indexing. Enforce zero calls to Poseidon hash function.
- [ ] **A3.2 — Peak Bagging Siblings Extraction:**
  Collect sibling peaks for outer bagging from `self.peak_positions` without modifying array memory.
- [ ] **A3.3 — Proof Struct Construction:**
  Construct canonical `MMRProof` containing `mountain_height`, `mountain_siblings`, `peak_bagging_siblings`, and `peak_index`.

---

### Phase 4: Backward Compatibility & Root Parity with Legacy MMR
- [ ] **A4.1 — Cross-Type Conversion Primitives:**
  Provide `FlatMerkleMountainRange::from_leaves(leaves: &[Fr])` and conversion from legacy `MerkleMountainRange`.
- [ ] **A4.2 — Mathematical Parity Verification:**
  Assert `flat_mmr.get_root() == legacy_mmr.get_root()` across all leaf ranges $N \in [1, 2048]$.
- [ ] **A4.3 — Zero Regression Stylus On-Chain Verifier Compatibility:**
  Ensure output of `generate_proof()` verifies successfully against `verify_proof()` without modifying contract EIP-2537 verifier.

---

### Phase 5: Exhaustive Test Matrix (Positive + $\ge 2\times$ Negative)

#### Positive Tests (3 Tests):
- [ ] **A5.1 — `test_flat_mmr_append_and_bagged_root_parity`:**
  Gradual leaf insertion from $N=1$ to $N=1000$. Assert exact 100% root equality with legacy MMR on every insertion.
- [ ] **A5.2 — `test_flat_mmr_zero_hash_proof_generation_and_verification`:**
  Build tree with $N=513$ leaves (crossing $2^9$ mountain boundary). Generate proof for all $i \in [0, 513)$. Verify all proofs pass `verify_proof`.
- [ ] **A5.3 — `test_flat_mmr_single_leaf_boundary`:**
  Tree with exactly 1 leaf ($N=1$): `nodes.len() == 1`, single peak at pos 0, empty mountain and peak siblings, proof verifies.

#### Negative Tests (7 Tests — $>2\times$ Ratio, Proving State Invariance):
- [ ] **A5.4 — `test_flat_mmr_rejects_out_of_bounds_leaf_index`:**
  Query proof for `leaf_index >= leaf_count`. Assert `Err(FlatMmrError::LeafIndexOutOfBounds)`. Assert `nodes.len()` and `leaf_count` remain strictly unchanged.
- [ ] **A5.5 — `test_flat_mmr_rejects_empty_tree_proof`:**
  Query proof on $N=0$ tree. Assert `Err(FlatMmrError::EmptyTree)`. Assert tree remains empty.
- [ ] **A5.6 — `test_flat_mmr_rejects_tampered_sibling_proof`:**
  Mutate 1 bit in `mountain_siblings`. Assert `verify_proof` strictly returns `false`.
- [ ] **A5.7 — `test_flat_mmr_rejects_tampered_leaf_count_hyperbridge_exploit`:**
  Inject falsified `leaf_count` greater than actual (Hyperbridge 2026 pattern). Assert strict boundary check rejects proof (`false`).
- [ ] **A5.8 — `test_flat_mmr_rejects_tampered_peak_bagging_sibling`:**
  Mutate 1 bit in `peak_bagging_siblings`. Assert `verify_proof` fails (`false`).
- [ ] **A5.9 — `test_flat_mmr_subtree_immutability_assertion`:**
  Take snapshot of `nodes` at $N=8$. Append to $N=16$. Assert `nodes[0..15]` are 100% bit-identical (zero subtree mutation).
- [ ] **A5.10 — `test_flat_mmr_rejects_non_canonical_leaf_input`:**
  Attempt to append scalar $\ge r$. Assert input rejected before insertion; `nodes.len()` remains unchanged.
