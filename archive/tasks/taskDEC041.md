# Task Checklist: DEC-041 In-Browser WebAssembly Groth16 Prover & Disposable Anti-OOM Architecture

## Status Tracking & Progress

- [x] **Task 1: Project Setup & Dependencies Configuration**
  - [x] Add `console_error_panic_hook` and `serde-wasm-bindgen` to `zeltra-sdk/Cargo.toml` (under wasm32 / general dependencies).
  - [x] Verify `Cargo.toml` compilation for both `x86_64-unknown-linux-gnu` and `wasm32-unknown-unknown`.

- [x] **Task 2: WASM DTOs & Types Specification (`zeltra-sdk/src/wasm_types.rs`)**
  - [x] Define `PrivateNoteWitnessInput` serde DTO matching `PrivateNoteCircuit` assignments.
  - [x] Define `JoinSplitWitnessInput` serde DTO matching `JoinSplitCircuit` assignments.
  - [x] Define `SelectiveDisclosureWitnessInput` serde DTO matching `SelectiveDisclosureCircuit` assignments.
  - [x] Define `WasmProverResult` with EVM G1/G2 hex points (`proofA`, `proofB`, `proofC`) and big-endian 32-byte public inputs.
  - [x] Define structured error enum/structs (`WasmProverError`, `InvalidScalarModulus`, `ConstraintUnsatisfied`, `DeserializationFailure`).

- [x] **Task 3: Rust WebAssembly Bindings (`zeltra-sdk/src/zk_wasm.rs`)**
  - [x] Implement `init_wasm_panic_hook()` with `console_error_panic_hook::set_once()`.
  - [x] Implement `wasm_prove_private_note(witness_json: &str, proving_key_bytes: &[u8]) -> Result<JsValue, JsValue>`:
    - [x] Deserialize compressed PK via `ark_serialize::CanonicalDeserialize::deserialize_compressed_unchecked`.
    - [x] Validate canonical scalar inputs ($< r$) and synthesize `PrivateNoteCircuit`.
    - [x] Preflight constraint satisfiability verification (`cs.is_satisfied()`) for fail-closed zero-panic defense.
    - [x] Compute Groth16 proof using `ark_groth16` and format EVM G1/G2 & 16 Public Inputs.
  - [x] Implement `wasm_prove_joinsplit(witness_json: &str, proving_key_bytes: &[u8]) -> Result<JsValue, JsValue>`:
    - [x] Deserialize compressed PK.
    - [x] Validate canonical scalar inputs and synthesize `JoinSplitCircuit`.
    - [x] Preflight constraint satisfiability verification (`cs.is_satisfied()`).
    - [x] Compute Groth16 proof and format EVM G1/G2 & 19 Public Inputs.
  - [x] Implement `wasm_prove_selective_disclosure(witness_json: &str, proving_key_bytes: &[u8]) -> Result<JsValue, JsValue>`:
    - [x] Instant proving (<25ms) for 5 Public Inputs CEX Selective Disclosure.
  - [x] Provide helper functions for native/unit testing without wasm-bindgen runtime.

- [x] **Task 4: Dedicated Web Worker Runner (`zeltra-sdk/js/zeltra_prover_worker.js`)**
  - [x] Support message actions: `PROVE_PRIVATE_NOTE`, `PROVE_JOINSPLIT`, `PROVE_DISCLOSURE`.
  - [x] Support transferable `ArrayBuffer` for zero-copy proving key transfer.
  - [x] Emit progress heartbeats (`PROGRESS`) and return `SUCCESS` or fail-closed `ERROR`.

- [x] **Task 5: Client Orchestrator Wrapper (`zeltra-sdk/js/prover_client.ts` & `.js`)**
  - [x] Create `ZeltraBrowserProver` class.
  - [x] Implement `provePrivateNote`, `proveJoinSplit`, `proveSelectiveDisclosure`.
  - [x] Enforce disposable worker lifecycle: `new Worker() -> postMessage -> await -> worker.terminate()` inside `finally`.
  - [x] Implement Watchdog timer for timeout termination (default: 30 seconds).

- [x] **Task 6: Comprehensive Automated Test Suite**
  - [x] Rust unit tests: positive proof generation & verification against Arkworks verifiers for all 3 circuits.
  - [x] Rust unit tests: negative tests for non-canonical scalars, unbalanced UTXO witness, corrupted proving key.
  - [x] Node.js lifecycle & memory stress test (`scripts/test_wasm_prover_worker_lifecycle.mjs`):
    - [x] Execute real proof generation and verify soundness.
    - [x] Stress-test 15 consecutive proof cycles with disposable worker pattern verifying flat memory profile (anti-OOM).
    - [x] Negative tests: unbalanced witness and invalid scalars rejected fail-closed.

- [x] **Task 7: Code Quality, Knowledge Graph Update & Git PR**
  - [x] Run `cargo check`, `cargo test`, `cargo clippy -- -D warnings`, and `cargo fmt --check`.
  - [x] Run `graphify update .` to synchronize knowledge graph.
  - [x] Update `todo.md`.
  - [x] Create git branch, commit with conventional commit message, push, and open PR to `0xReLogic/Zeltra-Protocol:main`.
  - [x] Provide comprehensive Executive Summary.
