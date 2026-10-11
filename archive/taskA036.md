# Task A: Universal 2-in-2-out JoinSplit R1CS Circuit, Strict Constant-Arity Topology, and 19-Public-Input Soundness

- **Specification Reference:** [`research/decisions/DEC-036A-universal-2-in-2-out-joinsplit-r1cs-circuit-specification.md`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-036A-universal-2-in-2-out-joinsplit-r1cs-circuit-specification.md)
- **Parent Blueprint:** [`research/decisions/DEC-036-universal-2-in-2-out-joinsplit-full-stack-architecture.md`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-036-universal-2-in-2-out-joinsplit-full-stack-architecture.md)
- **Status:** COMPLETED (ALL PHASES VERIFIED & 100% PASSING)
- **Applies to:**
  - `nimbus-core/src/joinsplit_circuit.rs` (Universal 2-in-2-out JoinSplit R1CS circuit, gadgets, and witness generation)
  - `nimbus-core/src/note.rs` (`MerkleMountainRange` verification and peak bagging gadgets, canonical nullifier derivations)
  - `nimbus-core/src/poseidon.rs` (Grain-128 Poseidon W5 parameters and domain constants)
  - `nimbus-core/tests/` (Satisfiability tests, negative exploit test vectors, VK non-identity tests)
- **Primary Objective:** Upgrade `JoinSplitCircuit` from partial prototype (40%) to production-grade zero-knowledge soundness: implement 448 quadratic boolean range constraints for exact value conservation anti wrap-around, Direction B strict constant-arity dummy output commitment, canonical dummy input 2 gadget (Penumbra bug defense), in-circuit MMR peak bagging gadget, Two-Limb quote hash (DEC-035A), and 19 canonical public inputs layout with non-trivial verifying key elements ($IC[0..19] \ne \mathcal{O}$).

---

## Progress Overview

- [x] **Phase 1: Value Conservation & Range Constraint Gadget (`nimbus-core`)** [4/4]
- [x] **Phase 2: Canonical Dummy Second Input Gadget (Penumbra Defense)** [4/4]
- [x] **Phase 3: Direction B Constant-Arity Dummy Output Gadget (ZIP 315 Defense)** [3/3]
- [x] **Phase 4: In-Circuit MMR Peak Bagging Gadget** [3/3]
- [x] **Phase 5: Two-Limb Quote Hash (128-bit) & Scope Binding Integration** [3/3]
- [x] **Phase 6: Canonical 19 Public Inputs Layout & Verifying Key Sanity** [3/3]
- [x] **Phase 7: Comprehensive Security & Negative Test Suite** [5/5]

---

## Detailed Task Breakdown

### Phase 1: Value Conservation & Range Constraint Gadget
- [x] **A1.1 — 64-bit Bit-Decomposition Range Check:**
  Implement active rank-1 quadratic boolean constraints $b_i \cdot (1 - b_i) = 0$ for all 7 value terms ($v_{in,1}, v_{in,2}, v_{\text{merchant}}, v_{\text{protocol\_fee}}, v_{\text{execution\_fee}}, v_{out,1}, v_{out,2}$). Ensure $7 \times 64 = 448$ constraints are synthesized in production release builds.
- [x] **A1.2 — Integer Conservation Equality:**
  Enforce linear combination equality:
  $$(v_{in,1} + v_{in,2}) - (v_{\text{merchant}} + v_{\text{protocol\_fee}} + v_{\text{execution\_fee}} + v_{out,1} + v_{out,2}) = 0$$
  Verify mathematically that $\max\left(\sum V\right) \approx 1.29 \times 10^{20} \ll r \approx 5.24 \times 10^{77}$ (zero wrap-around risk).
- [x] **A1.3 — Flat 45 bps Protocol Fee Ceiling Division Gadget:**
  Enforce protocol fee calculation with remainder decomposition:
  $$10\,000 \cdot v_{\text{protocol\_fee}} - 45 \cdot v_{\text{merchant}} = \text{rem}, \quad 0 \le \text{rem} < 10\,000$$
- [x] **A1.4 — Value-to-Commitment Poseidon W5 Binding:**
  Bind $v_{in,1}$ and $v_{in,2}$ to input note commitments using `Poseidon_W5([v, owner, rho, rand], DOMAIN_NOTE)`.

---

### Phase 2: Canonical Dummy Second Input Gadget (Penumbra Defense)
- [x] **A2.1 — Boolean Selector & Zero Value Enforcement:**
  Enforce `is_dummy_2 * (1 - is_dummy_2) == 0` and `is_dummy_2 * in2_value == 0`.
- [x] **A2.2 — Conditional MMR Path Verification:**
  Enforce that only the Merkle membership check is released on dummy slot:
  $$(1 - \text{is\_dummy\_2}) \cdot (\text{computed\_peak}_2 - \text{expected\_peak}_2) == 0$$
- [x] **A2.3 — Deterministic Dummy Nullifier PRF:**
  Derive dummy nullifier strictly via:
  $$\text{nf}_2 = \text{is\_dummy\_2} \cdot \text{Poseidon\_W5}([\text{nk}_2, \text{session\_nonce}, 0, 0], \text{DOMAIN\_DUMMY\_NULLIFIER}) + (1 - \text{is\_dummy\_2}) \cdot \text{Poseidon\_W5}([\text{nk}_2, \rho_2, \text{epoch\_id}_2, 0], \text{DOMAIN\_NULLIFIER})$$
- [x] **A2.4 — Epoch Alignment Constraint:**
  Enforce `is_dummy_2 * (epoch_id_2 - epoch_id_1) == 0` to prevent public input leakage.

---

### Phase 3: Direction B Constant-Arity Dummy Output Gadget (ZIP 315 Defense)
- [x] **A3.1 — Output 2 Canonical Zero-Value Enforcement:**
  If `has_change_2 == false`, constrain $v_{out,2} = 0$.
- [x] **A3.2 — Canonical Dummy Output Commitment Synthesis:**
  If `has_change_2 == false`, compute $\text{cm}_{out,2} = \text{Poseidon\_W5}([0, \text{dummy\_owner}, \rho_2, \text{rand}_2], \text{DOMAIN\_NOTE})$.
- [x] **A3.3 — Constant Arity Output Guarantees:**
  Ensure the circuit always exports exactly 2 output commitments (`output_commitment_1`, `output_commitment_2`), matching contract Direction B requirements.

---

### Phase 4: In-Circuit MMR Peak Bagging Gadget
- [x] **A4.1 — Local Merkle Path to Peak Computation:**
  Verify path from commitment leaf to local peak $P_j$ across up to 32 levels.
- [x] **A4.2 — Bagging Aggregation Loop:**
  Iteratively bag peaks $P_0, \dots, P_{m-1}$ from right to left using `Poseidon_W5([P_{i}, B_{i-1}, 0, 0], DOMAIN_MMR_BAG)`.
- [x] **A4.3 — Root Equality Constraint:**
  Assert bagged result equals public input `note_root`.

---

### Phase 5: Two-Limb Quote Hash (128-bit) & Scope Binding Integration
- [x] **A5.1 — 128-bit Range Gadgets on Quote Limbs:**
  Decompose `quote_hash_hi` and `quote_hash_lo` into 128 boolean bits each ($2 \times 128 = 256$ constraints).
- [x] **A5.2 — Contextual Scope Hash Synthesis:**
  Synthesize scope hash from `recipient`, `chain_id`, `contract_address`, `expiry`.
- [x] **A5.3 — Binding Commitment Equality:**
  Synthesize binding commitment wire from quote limbs and scope hash, constraining it against witness wire to guarantee non-trivial statement binding.

---

### Phase 6: Canonical 19 Public Inputs Layout & Verifying Key Sanity
- [x] **A6.1 — 19 Public Inputs Sequential Vector Layout:**
  Layout indices 0..18 exactly as specified in DEC-036A (note_root, leaf_count, nf1, nf2, epoch1, epoch2, out_cm1, out_cm2, recipient, amount, proto_fee, exec_fee, quote_hi, quote_lo, chain_id, contract_addr, expiry, flags_packed, is_rollover).
- [x] **A6.2 — Verifying Key Non-Identity Test (`test_joinsplit_vk_non_identity`):**
  Assert that for all $i \in [0, 19]$: $IC[i] \ne \mathcal{O}_{G_1}$ in generated verifying key.
- [x] **A6.3 — Constraint Budget Audit:**
  Verify total R1CS constraints synthesized matches production architecture.

---

### Phase 7: Comprehensive Security & Negative Test Suite
- [x] **A7.1 — Positive 1-In-2-Out & 2-In-2-Out Test:**
  Confirm valid witness satisfies R1CS for both 1-note spend (with dummy) and 2-note spend.
- [x] **A7.2 — Negative Test: Modular Wrap-Around ($v_{in} = 0, v_{out} = r-1$):**
  Assert circuit panics or returns `ConstraintUnsatisfied`.
- [x] **A7.3 — Negative Test: Penumbra Dummy Spend Exploitation:**
  Inject $v_{in,2} > 0$ with `is_dummy_2 = 1`; assert constraint failure.
- [x] **A7.4 — Negative Test: Singularity Nullifier Aliasing:**
  Inject identical nullifiers; assert circuit rejects duplicate preimage.
- [x] **A7.5 — Negative Test: Out-of-Range Quote Limb:**
  Inject $H_{hi} \ge 2^{128}$; assert 128-bit range gadget failure.
