# Task C: Relayer Ingress Pipeline, Dual-Nullifier Guard, and Stylus Smart Contract 2-in-2-out Settlement with EIP-2537

- **Specification Reference:** [`research/decisions/DEC-036C-relayer-ingress-guard-and-stylus-joinsplit-settlement.md`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-036C-relayer-ingress-guard-and-stylus-joinsplit-settlement.md)
- **Parent Blueprint:** [`research/decisions/DEC-036-universal-2-in-2-out-joinsplit-full-stack-architecture.md`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-036-universal-2-in-2-out-joinsplit-full-stack-architecture.md)
- **Status:** PENDING IMPLEMENTATION
- **Applies to:**
  - `nimbus-node/src/handlers/spend.rs` (Relayer `/api/v1/spend-joinsplit` handler, dual-nullifier guards, and preflight Groth16 verification)
  - `nimbus-node/src/db.rs` (SQLite `spent_nullifiers` tracking and transactional persistence)
  - `nimbus-contracts/src/spend.rs` (Stylus on-chain `spend_joinsplit` entrypoint, multi-liability solvency invariant, and settlement)
  - `nimbus-contracts/src/merkle.rs` (On-chain MMR constant-arity dual commitment insertion `_mmr_insert`)
  - `nimbus-contracts/src/groth16_joinsplit_verifier.rs` (EIP-2537 precompile `0x0c` MSM and `0x0f` Pairing routines for 19 public inputs)
  - Integration & Negative Test Suites
- **Primary Objective:** Build the complete end-to-end execution and settlement rail for 2-in-2-out JoinSplit: implement the relayer `POST /api/v1/spend-joinsplit` ingress endpoint with fail-closed scalar validation ($< r$), local CPU Groth16 preflight verification (immune to EIP-2537 burn-all-gas-on-error gas-griefing attacks), atomic dual-nullifier guards in SQLite and on-chain simulation, and the Stylus smart contract `spend_joinsplit` entrypoint executing Direction B constant-arity dual MMR appends (`_mmr_insert` twice), dual nullifier burning, and multi-liability solvency invariant verification.

---

## Progress Overview

- [ ] **Phase 1: Relayer Ingress Pipeline (`nimbus-node`)** [0/4]
- [ ] **Phase 2: Relayer Preflight Security Guards & Double-Spend Defense** [0/4]
- [ ] **Phase 3: Relayer Local CPU Groth16 Preflight Verifier** [0/3]
- [ ] **Phase 4: Stylus Smart Contract Entrypoint & Calldata Unpacking (`nimbus-contracts`)** [0/4]
- [ ] **Phase 5: On-Chain Direction B Settlement & Multi-Liability Invariant** [0/4]
- [ ] **Phase 6: EIP-2537 Precompile Host Verifier (`0x0c` MSM & `0x0f` Pairing)** [0/3]
- [ ] **Phase 7: End-to-End Integration & Negative Test Suite** [0/5]

---

## Detailed Task Breakdown

### Phase 1: Relayer Ingress Pipeline (`nimbus-node`)
- [ ] **C1.1 — Define JoinSplit Request DTO:**
  Define `JoinSplitSpendRequest` struct in `nimbus-node/src/handlers/spend.rs` with 19 public input fields, Groth16 proof, and signed EIP-712 quote.
- [ ] **C1.2 — Ingress Route Registration:**
  Register `POST /api/v1/spend-joinsplit` in the Axum router.
- [ ] **C1.3 — Canonical EVM Scalar Validation:**
  Validate all 19 public inputs satisfy $x < r$ using `from_evm_scalar`. Reject out-of-range scalars with HTTP 400.
- [ ] **C1.4 — Nullifier Non-Aliasing Preflight:**
  Enforce `assert!(payload.input_nullifier_1 != payload.input_nullifier_2)`.

---

### Phase 2: Relayer Preflight Security Guards & Double-Spend Defense
- [ ] **C2.1 — SQLite Atomic Nullifier Reservation:**
  Atomically check and reserve both nullifiers in `spent_nullifiers` SQLite table within an isolated transaction.
- [ ] **C2.2 — On-Chain Simulation Verification:**
  Query on-chain view method `is_nullifier_spent(nf)` for both nullifiers via `eth_call`. Release SQLite lock and reject if either is already spent.
- [ ] **C2.3 — Signed EIP-712 Quote Validation:**
  Verify secp256k1 signature against relayer domain separator, verifying quote deadline and execution fee floor.
- [ ] **C2.4 — Two-Limb Quote Equality Assertion:**
  Assert `payload.quote_hash_hi` and `payload.quote_hash_lo` match the decomposed limbs of the verified quote digest.

---

### Phase 3: Relayer Local CPU Groth16 Preflight Verifier
- [ ] **C3.1 — Integrate ark-groth16 Verifier in Relayer:**
  Instantiate JoinSplit verifying key in relayer memory.
- [ ] **C3.2 — Preflight Proof Execution on CPU:**
  Verify proof against the 19 public inputs before broadcasting.
- [ ] **C3.3 — Gas Griefing Immunity:**
  Ensure invalid proofs are rejected at ingress (HTTP 400) without submitting on-chain transactions, protecting relayer from EIP-2537 burn-all-gas-on-error exploits.

---

### Phase 4: Stylus Smart Contract Entrypoint & Calldata Unpacking (`nimbus-contracts`)
- [ ] **C4.1 — Stylus ABI Entrypoint Definition:**
  Implement `spend_joinsplit` in `nimbus-contracts/src/spend.rs` accepting proof, note_root, leaf_count, dual nullifiers, dual output commitments, recipient, amounts, quote hash, expiry, and flags.
- [ ] **C4.2 — Expiry & Nonce Verification:**
  Enforce `require(block::timestamp() <= expiry, "QuoteExpired")`.
- [ ] **C4.3 — Calldata Two-Limb Quote Unpacking:**
  Unpack 32-byte `quote_hash` into high and low 16-byte slices with left-zero padding to form 128-bit limbs for MSM.
- [ ] **C4.4 — On-Chain Nullifier Non-Aliasing & Double-Spend Guard:**
  Enforce:
  ```rust
  require(nullifier_1 != nullifier_2, "DuplicateNullifierInSameTx");
  require(!self.note_nullifiers.get(nullifier_1), "Nullifier1AlreadySpent");
  require(!self.note_nullifiers.get(nullifier_2), "Nullifier2AlreadySpent");
  ```

---

### Phase 5: On-Chain Direction B Settlement & Multi-Liability Invariant
- [ ] **C5.1 — Nullifier Burning State Update:**
  Set `self.note_nullifiers.setter(nullifier_1).set(true)` and `self.note_nullifiers.setter(nullifier_2).set(true)`.
- [ ] **C5.2 — Direction B Constant-Arity MMR Appends:**
  Always execute exactly 2 MMR insertions:
  ```rust
  self._mmr_insert(output_cm_1)?;
  self._mmr_insert(output_cm_2)?;
  ```
- [ ] **C5.3 — Financial Settlement & CEI Pattern:**
  Transfer USDC to `recipient`, accrue protocol and execution fees, transfer execution fee to `msg::sender()`.
- [ ] **C5.4 — Solvency Invariant Assertion:**
  Assert `self._assert_solvency_invariant()?` ensuring contract USDC balance $\ge$ total liabilities.

---

### Phase 6: EIP-2537 Precompile Host Verifier (`0x0c` MSM & `0x0f` Pairing)
- [ ] **C6.1 — G1 MSM Precompile Caller (0x0c):**
  Encode 20 points ($IC_0$ through $IC_{19}$) with 19 scalar public inputs into 3,200 bytes. Execute staticcall to `0x0c` to compute $L \in G_1$.
- [ ] **C6.2 — Pairing Check Precompile Caller (0x0f):**
  Encode 4 pairs ($[-A, B]$, $[\alpha, \beta]$, $[L, \gamma]$, $[C, \delta]$) into 1,536 bytes. Execute staticcall to `0x0f`.
- [ ] **C6.3 — Return Value & Returndatasize Assertion:**
  Assert `success == true`, `returndatasize == 32`, and result equals `0x00..01`.

---

### Phase 7: End-to-End Integration & Negative Test Suite
- [ ] **C7.1 — Positive Spend Test:**
  Confirm valid 1-in and 2-in JoinSplit transactions succeed on Arbitrum Sepolia test harness.
- [ ] **C7.2 — Negative Test: Nullifier Self-Aliasing (`nf_1 == nf_2`):**
  Assert transaction reverts with `"DuplicateNullifierInSameTx"`.
- [ ] **C7.3 — Negative Test: Spent Nullifier Re-execution:**
  Assert double spend attempt reverts with `"NullifierAlreadySpent"`.
- [ ] **C7.4 — Negative Test: Invalid Groth16 Proof (Relayer Preflight):**
  Assert relayer rejects with HTTP 400 and zero on-chain gas burned.
- [ ] **C7.5 — Negative Test: Insolvent Payout Attempt:**
  Assert transaction reverts on `_assert_solvency_invariant`.
