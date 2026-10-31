# DEC-057: Pure Flat-UTXO Architecture, Generational Epoch Purge & Permanent Global Nullifier Registry

- **Status:** PROPOSED & RATIFIED (Architectural Paradigm Shift & Workspace-Wide Purge)
- **Tipe DEC:** BREAKING ARCHITECTURAL REFACTORING, SOUNDNESS HARDENING & PROTOCOL SIMPLIFICATION
- **Author:** Zeltra Protocol Architecture, Cryptography, Systems Security & Engineering Team
- **Date:** 2026-10-31
- **Target Crates & Components:**
  - `zeltra-contracts`: `storage.rs`, `spend.rs`, `governance.rs`, `views.rs`, `lib.rs`, `groth16_joinsplit_verifier.rs`, `groth16_note_verifier.rs`, `events.rs`, `ABI/IZeltra.sol`
  - `zeltra-core`: `note.rs`, `keys.rs`, `joinsplit_circuit.rs`, `note_circuit.rs`, `selective_disclosure_circuit.rs`, `formal_tests.rs`, `circuit_gadgets.rs`
  - `zeltra-node`: `dto.rs`, `handlers/spend.rs`, `validation/spend_preflight.rs`, `evm_client.rs`, `database/nullifiers.rs`, `batch_processor.rs`
  - `zeltra-sdk`: `storage.rs`, `coin_selection.rs`, `wallet/note_wallet/`, `zk_wasm.rs`, `wasm_types.rs`, `compliance/`, `x402/`
  - `zeltra-cli`: `commands/compliance.rs`, test harnesses
- **Supersedes & Archives:**
  - [`DEC-033: Epoch-Windowed Nullifier Registry, In-Flight Rollover Economics`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-033-epoch-windowed-nullifier-pruning-in-flight-rollover.md) (**FULLY DEPRECATED & PURGED**)
- **Related & Affected DECs:**
  - [`DEC-036A: Universal 2-in-2-out JoinSplit R1CS Circuit Specification`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-036A-universal-2-in-2-out-joinsplit-r1cs-circuit-specification.md) (Public inputs reduced from 19 to 16)
  - [`DEC-035A: Public Input Representation & Two-Limb Quote Hash`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-035A-two-limb-quote-hash-representation-and-range-constraints.md) (PrivateNoteCircuit PIs reduced from 16 to 14)
  - [`DEC-040A: Selective Disclosure Circuit Specification`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-040A-selective-disclosure-circuit-and-key-separation.md) (Elimination of `epoch_id` witness)
  - [`DEC-043B: Stylus Strict Settlement & Dual-Memo MMR`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-043B-stylus-strict-three-mode-settlement-and-dual-memo-mmr.md) (Strict 3-Mode simplified to Strict 2-Mode: In-Pool vs Public Spend)
  - [`AUD-001: Comprehensive Security Audit of Core Circuits and Stylus Contracts`](file:///workspaces/Zeltra-Protocol/research/aud/AUD-001-core-circuits-and-stylus-contracts-security-audit.md) (Formal resolution and elimination of `ZCON-CRIT-01`, `ZCON-HIGH-01`, and `ZCON-HIGH-02`)

---

## 1. Executive Summary & Root Cause Analysis

### A. The Flaw of Academic Overengineering (DEC-033 Postmortem)
In October 2026, DEC-033 introduced an *Epoch-Windowed Generational Nullifier Registry* inspired by theoretical academic literature on state growth management (Bowe & Miers, IACR 2025/2031). The theoretical premise was to bound on-chain storage growth to $O(1)$ by partitioning nullifier sets into discrete 90-day epoch slots ($E \pmod 2$), pruning historical nullifiers, and requiring sleeping notes ($>180$ days) to perform rollover transactions.

However, formal security audit [`AUD-001`](file:///workspaces/Zeltra-Protocol/research/aud/AUD-001-core-circuits-and-stylus-contracts-security-audit.md) revealed that this mechanism introduced severe structural attack vectors and operational debt:
1. **Critical Cross-Epoch Replay (`ZCON-CRIT-01`):** Partitioning double-spend protection into generational slots (`generation_nullifiers[(slot, nf)]`) opened windows where cross-epoch aliasing permitted double-spending user funds.
2. **Expired Note Revert Traps (`ZCON-HIGH-01`):** Users holding notes across epoch boundaries experienced fatal contract reverts, violating Zeltra's core product manifesto: *"Real-cash mental accounting ($10 - $3 = $7 change) with Apple Pay speed"*.
3. **Desynchronized Generational Slot Parity (`ZCON-HIGH-02`):** State transitions during epoch rotations created race conditions between the mempool, relayer sequencers, and on-chain contract slots.
4. **Circuit Bloat & Prover Latency:** The JoinSplit circuit was forced to allocate, witness, and range-constrain multiple synthetic variables (`epoch_id_1`, `epoch_id_2`, `is_rollover`, Mode 1 dummy gates), increasing prover time on client devices.

### B. The L2 Reality: Storage Bloat vs Financial Solvency
On Arbitrum Nitro/Stylus (Layer 2), storing a 32-byte nullifier permanently is negligible in economic cost and disk overhead:
- 100,000,000 lifetime transactions consume only ~3.2 GB of random key-value storage.
- Arbitrum Sequencer nodes operate enterprise-grade NVMe drives with dozens of Terabytes of capacity.
- The financial and reputational cost of a single double-spending exploit or stuck user note is infinitely higher than storing flat nullifiers.
- Proven battle-tested zero-knowledge privacy protocols (e.g. Tornado Cash, Railgun, Zcash Orchard) enforce **flat, permanent global nullifier registries** without note expiry.

### C. The Paradigm Shift: Pure Flat-UTXO (Zero Expiry, Zero Rollover)
DEC-057 comprehensively purges all concepts of `epoch`, `is_rollover`, `generation_nullifiers`, and Mode 1 rollover across every layer of the Zeltra Protocol stack.

```text
┌────────────────────────────────────────────────────────────────────────────────────────┐
│                        ZELTRA PURE FLAT-UTXO ARCHITECTURE                              │
├────────────────────────────────┬───────────────────────────────────────────────────────┤
│ 1. Permanent Global Nullifier  │ self.note_nullifiers: StorageMap<FixedBytes<32>, bool>│
│ 2. Zero Expiry Cash Model      │ A note minted today is spendable 10 years from now.   │
│ 3. Strict Dual-Mode JoinSplit  │ Mode 1 (In-Pool Transfer) & Mode 2 (Public Outflow)   │
│ 4. Lean R1CS Circuits          │ Public Inputs reduced: JoinSplit (16 PI), Note (14 PI)│
└────────────────────────────────┴───────────────────────────────────────────────────────┘
```

---

## 2. Complete Cross-Crate Technical Mapping

This section defines the verbatim mapping of every file, function, struct, public input, and test across the workspace that must be refactored to purge epoch logic.

### Layer 1: Stylus Smart Contracts (`zeltra-contracts`)

| Component / File | Current Implementation (DEC-033) | DEC-057 Flat-UTXO Replacement |
|---|---|---|
| **Storage Layout** (`src/storage.rs`) | `current_epoch: StorageU256`<br>`epoch_start_timestamp: StorageU256`<br>`generation_nullifiers: StorageMap<bytes32, bool>`<br>`generation_epoch_id: StorageMap<uint256, uint256>`<br>`generational_nullifier_key(slot, nf)` | **PURGED**. Replace with:<br>`pub note_nullifiers: StorageMap<FixedBytes<32>, bool>;`<br>Delete `generational_nullifier_key`. |
| **JoinSplit Calldata & Verification** (`src/spend.rs`, `src/lib.rs`) | Takes 22 parameters including `epoch_id_1: U256`, `epoch_id_2: U256`, `is_rollover: U256`. Checks epoch bounds, validates Mode 1, hashes 19 PIs. | Takes 19 parameters (`epoch_id_1`, `epoch_id_2`, `is_rollover` **PURGED**). Public inputs array reduced to 16 elements. Nullifiers burned directly in `self.note_nullifiers.insert(nf, true)`. |
| **State Machine Modes** (`src/spend.rs`) | 3 Modes (Mode 1: Rollover, Mode 2: In-Pool, Mode 3: Public Spend). | **Strict 2-Mode Mutually Exclusive State Machine**:<br>- **Mode 1 (In-Pool Shielded Transfer)**: `recipient == Address::ZERO`, `merchant_amount == 0`, `protocol_fee == 0`, `has_change_1 == 1`.<br>- **Mode 2 (Public Outflow Spend)**: `recipient != Address::ZERO`, `merchant_amount > 0`, `protocol_fee == 45 bps`. |
| **Single Note Spend** (`src/spend.rs`, `src/lib.rs`) | `spend_private_note`: Takes `note_epoch_id: U256`, `is_rollover: U256` (16 PIs). | Takes 14 parameters (`note_epoch_id`, `is_rollover` **PURGED**). Public inputs reduced to 14 elements. |
| **Governance & Lifecycle** (`src/governance.rs`) | `_rotate_epoch()` function, `EpochRotated` event, `epoch_start_timestamp` initialization in `initialize()`. | **PURGED**. Delete `_rotate_epoch`, delete `EpochRotated` event in `events.rs`. |
| **Public Views & ABI** (`src/views.rs`, `src/lib.rs`, `abi/IZeltra.sol`) | `_is_generational_nullifier_spent(epoch, nf)`<br>`_is_note_nullifier_spent(nf)`<br>`_get_current_epoch()`<br>`_epoch_start_timestamp()` | **PURGED**. Replace with single view:<br>`pub fn is_nullifier_spent(&self, nullifier: FixedBytes<32>) -> Result<bool, Vec<u8>>`<br>`pub fn are_nullifiers_spent(&self, nullifiers: Vec<FixedBytes<32>>) -> Result<Vec<bool>, Vec<u8>>` |
| **MSM Verifier** (`src/groth16_joinsplit_verifier.rs`) | Computes MSM over 20 points (IC[0] + 19 PIs) for 3,200 bytes. | Computes MSM over 17 points (IC[0] + 16 PIs) for 2,720 bytes (saves 480 bytes calldata & gas). |
| **Note Verifier** (`src/groth16_note_verifier.rs`) | Computes MSM over 17 points (IC[0] + 16 PIs) for 2,720 bytes. | Computes MSM over 15 points (IC[0] + 14 PIs) for 2,400 bytes. |

---

### Layer 2: Arkworks BLS12-381 ZK Circuits (`zeltra-core`)

| Component / File | Current Implementation (DEC-033) | DEC-057 Flat-UTXO Replacement |
|---|---|---|
| **Nullifier Derivation** (`src/note.rs`, `src/keys.rs`) | `derive_nullifier(nk, cm, leaf_idx, epoch_id)`: $\text{Poseidon\_W5}([nk, cm, \text{leaf}, \text{Fr}(epoch)])$<br>`derive_joinsplit_nullifier(nk, rho, epoch)`: $\text{Poseidon\_W5}([nk, \rho, \text{Fr}(epoch), 0])$ | **PURE CANONICAL DERIVATION**:<br>`pub fn derive_nullifier(nk: Fr, cm: Fr, leaf_idx: u64) -> Fr`: $\text{Poseidon\_W4}([nk, cm, \text{Fr}(\text{leaf\_idx}), 0])$ or $\text{Poseidon\_W3}([nk, \rho])$ without epoch input.<br>`pub fn derive_joinsplit_nullifier(nk: Fr, rho: Fr) -> Fr`: $\text{Poseidon\_W3}([nk, \rho])$ or $\text{Poseidon\_W5}([nk, \rho, 0, 0])$. |
| **Universal JoinSplit Circuit** (`src/joinsplit_circuit.rs`) | **19 Public Inputs**:<br>PI[4]: `epoch_id_1`, PI[5]: `epoch_id_2`, PI[18]: `is_rollover`.<br>Includes constraints: `is_dummy_2 * (epoch_id_2 - epoch_id_1) == 0`, `is_rollover * (1 - is_rollover) == 0`, `is_rollover * merchant == 0`. | **16 Public Inputs**:<br>Delete `epoch_id_1`, `epoch_id_2`, and `is_rollover`.<br>Delete epoch equality and rollover constraint gates.<br>Enforce pure value conservation and in-pool vs public spend flags. |
| **Private Note Circuit** (`src/note_circuit.rs`) | **16 Public Inputs**:<br>PI[3]: `note_epoch_id`, PI[15]: `is_rollover`. | **14 Public Inputs**:<br>Delete `note_epoch_id` and `is_rollover` from public inputs and witness structs. |
| **Selective Disclosure Circuit** (`src/selective_disclosure_circuit.rs`) | Proves nullifier derivation using `epoch_id` witness: $\text{nf} == \text{Poseidon}([nk, \rho, epoch, 0])$. | Eliminates `epoch_id`. Proves flat nullifier derivation directly from $nk$ and $\rho$. |
| **Formal Soundness Tests** (`src/formal_tests.rs`, `src/circuit_gadgets.rs`) | `setup_fam5_rollover_circuit`, epoch-bound tangent rigidity checks. | Refactor Jacobian soundness checks to evaluate the 16-PI Universal JoinSplit and 14-PI Note circuits across all 6 adversarial families. |

---

### Layer 3: Client SDK & Note Wallet (`zeltra-sdk`)

| Component / File | Current Implementation (DEC-033) | DEC-057 Flat-UTXO Replacement |
|---|---|---|
| **SQLCipher Note Schema** (`src/storage.rs`) | `CREATE TABLE notes (... epoch_id INTEGER NOT NULL ...)` | Purge `epoch_id` column or set default constant 0. Migrations drop epoch filtering. |
| **Stochastic Knapsack Selection** (`src/coin_selection.rs`) | Scores notes with `age_i = current_epoch - epoch_i` bonus to prioritize spending old notes before expiry; rejects `ExpiredRequiresRollover`. | **PURE STOCHASTIC KNAPSACK**:<br>All unspent notes are equally spendable regardless of creation date.<br>Zero age degradation, zero expired note rejections. |
| **Wallet Note Types** (`src/wallet/note_wallet/types.rs`) | `NoteEpochStatus` enum (`Fresh`, `Mature`, `ExpiredRequiresRollover`), `check_epoch_status(current_epoch)`. | **PURGED**. Delete `NoteEpochStatus`, delete `check_epoch_status`. |
| **Spend & JoinSplit Workflows** (`src/wallet/note_wallet/spend.rs`, `joinsplit.rs`) | Queries `/api/v1/epoch`, checks note age, generates Mode 1 rollover proofs if expired. | Eliminates epoch queries and rollover branches. All spends execute directly via Mode 1 (In-Pool) or Mode 2 (Public). |
| **WebAssembly Prover** (`src/zk_wasm.rs`, `src/wasm_types.rs`) | Deserializes `note_epoch_id`, `in1_epoch_id`, `in2_epoch_id`, `is_rollover`. | Purges epoch and rollover fields from JSON witness interfaces and WASM binders. |
| **B2B & x402 Micropayments** (`src/compliance/`, `src/x402/`) | Carried `note_epoch_id` in invoices and selective disclosure proofs. | Cleaned to pure asset, value, and commitment identifiers. |

---

### Layer 4: Relayer Sequencer Node (`zeltra-node`)

| Component / File | Current Implementation (DEC-033) | DEC-057 Flat-UTXO Replacement |
|---|---|---|
| **Preflight Sanitizer** (`src/validation/spend_preflight.rs`) | Checks `client.get_current_epoch()`, validates `EPOCH_IN_FUTURE`, rejects `EPOCH_EXPIRED_REQUIRE_ROLLOVER`, enforces dust limits on standalone rollover. | **PURGED**. Delete all epoch comparisons, delete rollover dust thresholds. Only validate canonical scalars, non-duplicate nullifiers, unspent nullifiers, valid quote signatures, and Groth16 proofs. |
| **HTTP DTOs & Handlers** (`src/dto.rs`, `src/handlers/spend.rs`) | `epoch_id_1`, `epoch_id_2`, `note_epoch_id`, `is_rollover` in JSON request/response payloads. | Purge fields from DTO schemas. Return clean transaction receipts. |
| **EVM Client Interop** (`src/evm_client.rs`) | `get_current_epoch()` querying contract `current_epoch()`, ABI bindings for 22-arg `spend_joinsplit`. | Delete `get_current_epoch()`. Update ABI bindings to 19-arg flat `spend_joinsplit`. |
| **Database & Mempool** (`src/database/nullifiers.rs`, `mempool_watchdog.rs`) | Tracked epoch metadata per spent nullifier. | Pure flat `(nullifier_hash, tx_hash, spent_at)` table. |

---

### Layer 5: CLI Utility (`zeltra-cli`)

| Component / File | Current Implementation (DEC-033) | DEC-057 Flat-UTXO Replacement |
|---|---|---|
| **Compliance CLI** (`src/commands/compliance.rs`) | Passed `epoch_id: Some(0)` to disclosure circuit harnesses. | Purged `epoch_id` parameter, passes clean witness to `SelectiveDisclosureCircuit`. |
| **CLI Test Harnesses** (`tests/compliance_cli_tests.rs`) | Sample circuit harnesses with epoch fixtures. | Updated to flat nullifier keys and inputs. |

---

## 3. Public Input Vector Layout Reductions

### A. Universal JoinSplit Circuit: 19 PIs $\rightarrow$ 16 PIs
By eliminating `epoch_id_1` (old PI 4), `epoch_id_2` (old PI 5), and `is_rollover` (old PI 18), the public inputs layout collapses to a compact, highly optimized 16-element vector:

```text
Old Layout (19 PIs - DEC-036A):
[0: note_root, 1: leaf_count, 2: nullifier_1, 3: nullifier_2, 4: epoch_id_1, 5: epoch_id_2,
 6: out_cm_1, 7: out_cm_2, 8: recipient, 9: merchant_amount, 10: protocol_fee, 11: exec_fee,
 12: quote_hi, 13: quote_lo, 14: chain_id, 15: contract_addr, 16: expiry, 17: flags_packed, 18: is_rollover]

DEC-057 New Layout (16 PIs - Canonical Flat):
┌──────┬────────────────────────┬────────────────┬────────────────────────────────────────────────────────┐
│ Slot │ Public Input Signal    │ Field / Type   │ Description & Bounds                                   │
├──────┼────────────────────────┼────────────────┼────────────────────────────────────────────────────────┤
│ 0    │ note_root              │ Fr / bytes32   │ Bagged MMR Root hash at current commitment accumulator │
│ 1    │ leaf_count             │ Fr / uint64    │ Total commitment count in MMR (strictly in [1, 2^64))  │
│ 2    │ input_nullifier_1      │ Fr / bytes32   │ Unique spent nullifier for Input Note 1                │
│ 3    │ input_nullifier_2      │ Fr / bytes32   │ Unique spent nullifier for Input Note 2 (or dummy nf2) │
│ 4    │ output_commitment_1    │ Fr / bytes32   │ Output commitment 1 (recipient or change)              │
│ 5    │ output_commitment_2    │ Fr / bytes32   │ Output commitment 2 (sender change or dummy change)    │
│ 6    │ recipient              │ Fr / address   │ Public EVM payout recipient (0x0 for in-pool transfer) │
│ 7    │ merchant_amount        │ Fr / uint64    │ Public token payout amount (0 for in-pool transfer)    │
│ 8    │ protocol_fee           │ Fr / uint64    │ Protocol monetization fee (45 bps public, 0 in-pool)   │
│ 9    │ execution_fee          │ Fr / uint64    │ Relayer execution fee reimbursement in native units    │
│ 10   │ quote_hash_hi          │ Fr / uint128   │ High 128 bits of two-limb relayer quote hash           │
│ 11   │ quote_hash_lo          │ Fr / uint128   │ Low 128 bits of two-limb relayer quote hash            │
│ 12   │ chain_id               │ Fr / uint64    │ Target EVM chain ID (anti-cross-chain replay)          │
│ 13   │ contract_address       │ Fr / address   │ Address of Zeltra Stylus contract                      │
│ 14   │ expiry                 │ Fr / uint64    │ Unix timestamp deadline for quote and spend execution  │
│ 15   │ flags_packed           │ Fr / uint8     │ Bit 0: has_ch1, Bit 1: has_ch2, Bit 2: asset_id (0..7) │
└──────┴────────────────────────┴────────────────┴────────────────────────────────────────────────────────┘
```

### B. Private Note Circuit: 16 PIs $\rightarrow$ 14 PIs
Old 16 PIs (PI[3]: `note_epoch_id`, PI[15]: `is_rollover`) collapses to 14 PIs:
`[root, leaf_count, nullifier, out_cm, recipient, merchant_amount, protocol_fee, exec_fee, quote_hi, quote_lo, chain_id, contract_addr, expiry, flags_packed]`.

---

## 4. Verification and Security Mandates

1. **Elimination of Audit Findings (`AUD-001` Closure):**
   - `ZCON-CRIT-01` (Cross-Epoch Replay) is fundamentally eliminated because generational slots no longer exist; nullifiers are burned in `self.note_nullifiers`.
   - `ZCON-HIGH-01` (Expired Note Revert Trap) is fundamentally eliminated because notes never expire.
   - `ZCON-HIGH-02` (Generation Epoch Slot Desynchronization) is eliminated with the deletion of `generation_epoch_id`.
2. **Tangent Rigidity & DEC-047 Jacobian Soundness:**
   - The refactored 16-PI JoinSplit circuit must be subjected to the full 6-family adversarial witness-locality sweep in `formal_tests.rs`.
   - Financial target nullity $r_{T_{\text{financial}}} = 0$ must be confirmed across the reduced constraint system.
3. **Zero Breaking Changes for Public Safe-Harbor Interfaces:**
   - Maintain strict unhosted direct self-broadcast endpoints (`prepare_self_broadcast_joinsplit`) under DEC-054.

---

## 5. Implementation Roadmap & Execution Order

1. **Phase 1: Architecture Decision & Specification (DEC-057 authoring)**: Establish complete cross-repo blueprint and update `research/dec/README.md`.
2. **Phase 2: Core Circuits Refactor (`zeltra-core`)**: Update nullifier formulas, trim public inputs in `joinsplit_circuit.rs` and `note_circuit.rs`, regenerate trusted setup fixtures.
3. **Phase 3: Stylus Contracts Refactor (`zeltra-contracts`)**: Replace `EpochNullifierState` with `note_nullifiers`, adjust `spend_joinsplit` parameters, update Groth16 verifiers, and clean ABI.
4. **Phase 4: Node & Ingress Sanitization (`zeltra-node`)**: Purge epoch validations from `spend_preflight.rs`, update DTOs, and simplify mempool tracking.
5. **Phase 5: SDK & Client Wallet (`zeltra-sdk`)**: Remove epoch scoring from Knapsack, eliminate rollover routines, update WASM provers.
6. **Phase 6: CLI & Full Regression Verification (`zeltra-cli`, tests)**: Run full workspace test passes, clippy audits, and verify zero regressions.
