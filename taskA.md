# Task A: Two-Limb (128-bit) Quote Hash Representation & Circuit Range Constraints

- **Specification Reference:** [`research/decisions/DEC-035A-two-limb-quote-hash-representation-and-range-constraints.md`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-035A-two-limb-quote-hash-representation-and-range-constraints.md)
- **Parent Blueprint:** [`research/decisions/DEC-035-cryptographic-canonicality-public-input-binding-and-mmr.md`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-035-cryptographic-canonicality-public-input-binding-and-mmr.md)
- **Status:** PENDING IMPLEMENTATION (Founder Authorization Checkpoint Required Before Code Edits)
- **Primary Objective:** Permanently eliminate the 256-bit scalar mismatch ($2^{256} > r_{\text{BLS12-381}}$) by decomposing EIP-712 Keccak-256 digests into two canonical 128-bit limbs ($H_{\text{hi}}, H_{\text{lo}} \in [0, 2^{128}-1] \ll r$) with strict in-circuit R1CS range constraints.

---

## Progress Overview

- [ ] **Phase 1: Core Cryptography & Circuit Implementation (`nimbus-core`)** [0/6]
- [ ] **Phase 2: Stylus Smart Contract Adaptation (`nimbus-contracts`)** [0/4]
- [ ] **Phase 3: SDK Client & Wallet Integration (`nimbus-sdk`)** [0/3]
- [ ] **Phase 4: Relayer Ingress Validation (`nimbus-node`)** [0/3]
- [ ] **Phase 5: Exhaustive Testing & Verification Suite** [0/5]

---

## Detailed Task Breakdown

### Phase 1: Core Cryptography & Circuit (`nimbus-core`)
- [ ] **A1.1 — Canonical Decomposition Utilities:**
  Implement `split_quote_hash_to_limbs(digest: &[u8; 32]) -> (Fr, Fr)` and `combine_limbs_to_quote_hash(hi: &Fr, lo: &Fr) -> [u8; 32]` in `nimbus-core/src/note.rs` or `evm.rs` with Big-Endian byte ordering.
- [ ] **A1.2 — 128-bit R1CS Range Constraint Gadget:**
  Implement `enforce_u128_range<CS: ConstraintSystem<Fr>>(&mut cs, var: &AllocatedNum<Fr>, name: &'static str)` in `nimbus-core/src/note_circuit.rs`:
  - 128 boolean constraints: $b_i \cdot (1 - b_i) = 0$.
  - 1 linear combination constraint: $\sum_{i=0}^{127} 2^i \cdot b_i - \text{var} = 0$.
- [ ] **A1.3 — Circuit Struct Layout Update:**
  In `nimbus-core/src/note_circuit.rs`:
  - Replace `quote_hash: Fr` with `quote_hash_hi: Fr` and `quote_hash_lo: Fr` in `PrivateNoteCircuit`.
- [ ] **A1.4 — Circuit Synthesis & Wire Allocation:**
  In `PrivateNoteCircuit::synthesize`:
  - Allocate `quote_hash_hi_var` and `quote_hash_lo_var` as public inputs.
  - Enforce `enforce_u128_range` on both variables.
- [ ] **A1.5 — Public Input Layout Expansion (15 $\to$ 16 inputs):**
  Update `extract_public_inputs` to return 16 scalar elements:
  - Wire 9: `quote_hash_hi`
  - Wire 10: `quote_hash_lo`
- [ ] **A1.6 — Deterministic Development Key Regeneration:**
  Regenerate proving key `ProvingKey<Bls12_381>` and verifying key `VerifyingKey<Bls12_381>` using deterministic seed `NOTE_CIRCUIT_SETUP_SEED = 0x4e696d6275734e43`. Export verifying key $IC$ (17 elements: $IC[0 \dots 16]$).

---

### Phase 2: Stylus Smart Contract (`nimbus-contracts`)
- [ ] **A2.1 — External ABI Preservation (`spend.rs`):**
  Verify external function signature `spend_private_note` retains:
  `quote_hash: FixedBytes<32>` in calldata (zero external breaking change).
- [ ] **A2.2 — In-Memory Unpacking Routine:**
  In `nimbus-contracts/src/spend.rs`, slice `quote_hash`:
  - `quote_hi`: first 16 bytes left-padded with 16 zeros.
  - `quote_lo`: last 16 bytes left-padded with 16 zeros.
- [ ] **A2.3 — Verifier Public Inputs Construction:**
  Expand internal public inputs array from `[[u8; 32]; 15]` to `[[u8; 32]; 16]`:
  - `public_inputs[9] = quote_hi`
  - `public_inputs[10] = quote_lo`
- [ ] **A2.4 — Groth16 Verifier Constants Update (`groth16_note_verifier.rs`):**
  Update verifying key elements with the newly generated 17-point $IC$ array from Phase 1. Ensure MSM precompile `0x0c` iterates over 16 public inputs + 1 constant.

---

### Phase 3: Client SDK (`nimbus-sdk`)
- [ ] **A3.1 — EIP-712 Digest Canonicality (`eip712.rs`):**
  Ensure `compute_quote_hash` returns the raw 32-byte Keccak-256 digest without any modular reduction.
- [ ] **A3.2 — Wallet Witness Builder (`note_wallet.rs`):**
  Update `PrivateNoteWallet::create_spend_proof`:
  - Decompose EIP-712 digest into `quote_hash_hi` and `quote_hash_lo`.
  - Pass both limbs into `PrivateNoteCircuit` witness generator.
- [ ] **A3.3 — Public Inputs Serialization:**
  Ensure SDK serializes 16 public inputs when constructing the spend payload for the relayer HTTP request.

---

### Phase 4: Relayer Node (`nimbus-node`)
- [ ] **A4.1 — Spend Ingress Schema Update (`spend.rs`):**
  Update `SpendRequest` deserializer to accept 16 public inputs.
- [ ] **A4.2 — Limb Range Preflight Sanitization:**
  In `nimbus-node/src/handlers/spend.rs`:
  - Validate `public_inputs[9]` and `public_inputs[10]` parse successfully via `from_evm_scalar`.
  - Validate both limbs are strictly $< 2^{128}$ in preflight.
- [ ] **A4.3 — Broadcast Calldata Re-packing:**
  In `broadcast_spend_private_note_transaction`:
  Re-pack the two 128-bit limbs into the single 32-byte `quote_hash` expected by the Stylus contract external ABI.

---

### Phase 5: Verification & Testing Suite
- [ ] **A5.1 — Unit Tests (`nimbus-core`):**
  - Boundary test vectors: $0$, $1$, $2^{128}-1$, $2^{128}$, $2^{256}-1$.
  - Negative test: witness with 129th bit set $\implies$ R1CS synthesis returns error.
- [ ] **A5.2 — Groth16 Pairing Soundness Test:**
  - Tamper 1 bit in `quote_hash_hi` or `quote_hash_lo` $\implies$ Groth16 verify reverts.
- [ ] **A5.3 — Stylus WASM Testnet Simulation:**
  - Execute `cargo test -p nimbus-contracts` verifying that MSM precompile `0x0c` processes unpacked limbs without revert.
- [ ] **A5.4 — Relayer End-to-End Test:**
  - Submit spend request with valid limbs; verify relayer preflight accepts and queues for broadcast.
- [ ] **A5.5 — Automated Lint & Quality Check:**
  - `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` clean across workspace.

---

## Invariant Verification Checklist

- [ ] **INV-1 (Entropy Preservation):** Full 256 bits of Keccak-256 digest are preserved ($H \equiv H_{\text{hi}} \cdot 2^{128} + H_{\text{lo}} \pmod{2^{256}}$).
- [ ] **INV-2 (Canonical Embedding):** $2^{128}-1 \ll r \implies$ zero danger of scalar overflow or modular reduction collisions.
- [ ] **INV-3 (Range Soundness):** Strictly 128 quadratic constraints per limb enforced in R1CS.
- [ ] **INV-4 (Calldata ABI Immutability):** External entrypoint in Stylus remains `quote_hash: FixedBytes<32>`.
- [ ] **INV-5 (Statement Binding):** Verifying key points $IC[9] \ne \mathcal{O}$ and $IC[10] \ne \mathcal{O}$.
