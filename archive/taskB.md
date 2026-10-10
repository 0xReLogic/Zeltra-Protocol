# Task B: Formal Scope Binding Gadget & Groth16 Statement Integrity

- **Specification Reference:** [`research/decisions/DEC-035B-formal-scope-binding-gadget-and-groth16-statement-integrity.md`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-035B-formal-scope-binding-gadget-and-groth16-statement-integrity.md)
- **Parent Blueprint:** [`research/decisions/DEC-035-cryptographic-canonicality-public-input-binding-and-mmr.md`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-035-cryptographic-canonicality-public-input-binding-and-mmr.md)
- **Status:** COMPLETED & VERIFIED
- **Primary Objective:** Eliminate the dangling dead-wire `_binding` assignment by implementing an active, two-stage **Formal Scope Binding Gadget** in R1CS, mathematically guaranteeing that all contextual metadata wires (`recipient`, `chain_id`, `contract_address`, `expiry`, `quote_hash_hi`, `quote_hash_lo`) produce non-trivial verifying key elements ($IC[i] \ne \mathcal{O}_{G_1}$) and enforcing bit-exact relayer ingress quote equality.

---

## Progress Overview

- [x] **Phase 1: R1CS Scope Binding Gadget Implementation (`nimbus-core`)** [5/5]
- [x] **Phase 2: Verifying Key Sanity & Statement Binding Assurance** [3/3]
- [x] **Phase 3: Relayer Ingress Equality Guard (`nimbus-node`)** [3/3]
- [x] **Phase 4: Client SDK Witness Generation (`nimbus-sdk`)** [3/3]
- [x] **Phase 5: JoinSplit Circuit Scope Binding Parity** [2/2]
- [x] **Phase 6: Comprehensive Security & Negative Test Suite** [5/5]

---

## Detailed Task Breakdown

### Phase 1: R1CS Scope Binding Gadget (`nimbus-core`)
- [x] **B1.1 — Domain Separator Definition:**
  Define canonical domain separators in `nimbus-core/src/note_circuit.rs`:
  - `DOMAIN_SCOPE_V1 = 0x5a454c5452415f53434f50455f5631` ("ZELTRA_SCOPE_V1")
  - `DOMAIN_BINDING_V1 = 0x5a454c5452415f42494e44494e475f5631` ("ZELTRA_BINDING_V1")
- [x] **B1.2 — Stage 1 Scope Hash Synthesis:**
  Synthesize contextual scope hash via Poseidon-w5:
  $$\text{scope\_hash} = \text{Poseidon}_{w5}([\text{recipient\_var}, \text{chain\_id\_var}, \text{contract\_address\_var}, \text{expiry\_var}], \text{DOMAIN\_SCOPE\_V1})$$
- [x] **B1.3 — Stage 2 Binding Commitment Synthesis:**
  Synthesize binding commitment wire via Poseidon-w5:
  $$\text{binding\_cm} = \text{Poseidon}_{w5}([quote\_hash\_hi\_var, quote\_hash\_lo\_var, scope\_hash, zero\_var], \text{DOMAIN\_BINDING\_V1})$$
- [x] **B1.4 — Active R1CS Rank-1 Constraint Enforcement:**
  Replace the unconstrained dead-code assignment (`let _binding = ...`) with active enforcement:
  - Allocate `expected_binding_var = cs.new_witness_variable(|| Ok(self.expected_binding))?`.
  - Enforce constraint: `binding_cm * 1 == expected_binding_var`.
- [x] **B1.5 — Circuit Witness Generator Update:**
  Update witness generation in `PrivateNoteCircuit::new` to precompute `expected_binding` natively during witness synthesis.

---

### Phase 2: Verifying Key Sanity & Statement Binding Assurance
- [x] **B2.1 — Automated Non-Identity Point Test (`test_verifying_key_non_identity`):**
  Implement unit test in `nimbus-core/tests/` asserting that for all $i \in [0, 16]$:
  $$IC[i] \ne \mathcal{O}_{G_1} \quad (\text{Point at Infinity})$$
  Specifically verify wires 5 (`recipient`), 9 (`quote_hi`), 10 (`quote_lo`), 11 (`chain_id`), 12 (`contract_address`), 13 (`expiry`).
- [x] **B2.2 — QAP Density Verification:**
  Verify that the QAP constraint matrices $A, B, C$ contain non-zero coefficients for all 6 contextual public input columns.
- [x] **B2.3 — Contract Verifier Constants Consistency:**
  Ensure `nimbus-contracts/src/groth16_note_verifier.rs` is updated with the non-trivial $IC$ elements.

---

### Phase 3: Relayer Ingress Equality Guard (`nimbus-node`)
- [x] **B3.1 — Reconstruct Signing Digest in Handler:**
  In `nimbus-node/src/handlers/spend.rs`, re-derive the expected EIP-712 quote hash from the verified client quote:
  `expected_hash = compute_quote_hash(&quote, chain_id, contract_addr)`
- [x] **B3.2 — Bit-Exact Public Inputs Equality Assertion:**
  Split `expected_hash` into limbs and assert:
  `public_inputs[9] == expected_hi && public_inputs[10] == expected_lo`
- [x] **B3.3 — Fail-Closed Rejection Handling:**
  If equality fails, reject request immediately with `400 Bad Request` and error code `MismatchedQuoteHash`. Log security warning with expected vs submitted limb hashes.

---

### Phase 4: Client SDK Integration (`nimbus-sdk`)
- [x] **B4.1 — Wallet Scope Hash Helper:**
  In `nimbus-sdk/src/wallet/note_wallet.rs`, compute `scope_hash` and `expected_binding` during local proof generation.
- [x] **B4.2 — EIP-712 Context Alignment:**
  Ensure client passes the active target `chain_id` and `contract_address` matching the connected network.
- [x] **B4.3 — Public Inputs Array Layout Parity:**
  Ensure client SDK formats all 16 public inputs in exact sequence matching Section 6 of DEC-035B.

---

### Phase 5: JoinSplit Circuit Scope Binding Parity
- [x] **B5.1 — Port Scope Binding to JoinSplit (`joinsplit_circuit.rs`):**
  Implement identical Scope Binding Gadget logic in `nimbus-core/src/joinsplit_circuit.rs` to maintain cryptographic parity.
- [x] **B5.2 — JoinSplit Verifying Key Non-Identity Test:**
  Assert all public input wires in JoinSplit verifying key satisfy $IC[i] \ne \mathcal{O}_{G_1}$.

---

### Phase 6: Comprehensive Security & Negative Test Suite
- [x] **B6.1 — Recipient Substitution Negative Test:**
  Mutate `recipient` in public inputs without regenerating proof $\implies$ Groth16 verify MUST return `false`.
- [x] **B6.2 — Quote Hash Substitution Negative Test:**
  Mutate `quote_hash_hi` or `quote_hash_lo` in public inputs $\implies$ Groth16 verify MUST return `false`.
- [x] **B6.3 — Cross-Chain Replay Negative Test:**
  Mutate `chain_id` (e.g. `421614` $\to$ `1`) in public inputs $\implies$ Groth16 verify MUST return `false`.
- [x] **B6.4 — Relayer Preflight Ingress Negative Test:**
  Submit valid proof for Quote A with signature for Quote B $\implies$ Relayer MUST return `400 Bad Request` (`MismatchedQuoteHash`).
- [x] **B6.5 — Workspace Hygiene Check:**
  `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` clean across workspace.

---

## Invariant Verification Checklist

- [x] **INV-1 (Non-Trivial VK Points):** $IC[i] \ne \mathcal{O}_{G_1}$ for all $i \in [0, 16]$.
- [x] **INV-2 (Deterministic Scope Commitment):** All 6 contextual parameters are non-malleably bound into proof witness via Poseidon S-boxes.
- [x] **INV-3 (Relayer Ingress Completeness):** Bit-exact equality enforced between signed quote digest and public inputs limbs.
- [x] **INV-4 (Domain Separation Soundness):** Distinct tags used for scope (`DOMAIN_SCOPE_V1`) and binding (`DOMAIN_BINDING_V1`).
