# Task C: Node Indexer, Relayer Ingress & Full Verification (DEC-048)

> **Session Scope:** `zeltra-node`, `zeltra-cli`, `scripts/` (Relayer validation, end-to-end Python test matrix, and final audit signoff)  
> **Parent ADR:** [`research/decisions/DEC-048-committing-aead-partitioning-oracle-defense-and-key-binding-memos.md`](research/decisions/DEC-048-committing-aead-partitioning-oracle-defense-and-key-binding-memos.md)  
> **Target Files:**
> - `zeltra-node/src/indexer/mmr_indexer.rs`
> - `zeltra-node/src/validation.rs`
> - `scripts/test_dec048_committing_aead.py`
> - `scripts/test_real_case_dec043c_inpool_wallet.py`

---

## 1. Objectives for Session C

1. **Relayer & Node Memo Ingress Validation:**
   - Verify that `zeltra-node` ingress guards accept 96-byte committing memos without rejecting valid calldata.
   - Ensure the MMR event indexer correctly captures and broadcasts the 96-byte memo logs.
2. **Execute Full Multi-Asset Lifecycle Regression:**
   - Run `python3 scripts/test_real_case_dec043c_inpool_wallet.py` with large-value notes (> 2,147 USDC, e.g. 5,000 USDC and 10 WETH).
   - Verify that the contract balance and liabilities remain perfectly solvent.
3. **Run Automated Adversarial Test Harness:**
   - Execute `python3 scripts/test_dec048_committing_aead.py` covering all 11 positive and negative test suites (Partitioning resistance, bit tampering, non-contributory points, boundary overflows).
4. **Final Workspace & Backlog Signoff:**
   - Mark DEC-048 complete in `todo.md`.
   - Run `graphify update .`.

---

## 2. Step-by-Step Implementation Checklist

- [x] **Step C1: Relayer Ingress & Indexer Validation:**
  - Check `zeltra-node/src/validation/spend_preflight.rs` for memo length assertions (strictly 96 bytes / 192 hex chars). Verified zero rejection on valid committing memos and strict rejection on non-96B inputs.
- [x] **Step C2: Real-Case Multi-Asset Lifecycle Test:**
  - Execute end-to-end simulation with large note values (5,000 USDC and 10 WETH) via `scripts/test_real_case_dec043c_inpool_wallet.py` (Section 8: 61/61 tests passed) to confirm zero balance desynchronization.
- [x] **Step C3: Adversarial Security Suite Execution:**
  - Run `python3 scripts/test_dec048_committing_aead.py` covering all 11 positive and negative suites (100% passed).
- [x] **Step C4: Final Review Signoff:**
  - Verified audit status, backward-compatible trial decryption fallbacks, DEC-046 canonical dummy constants, and documentation integrity.

---

## 3. Definition of Done for Session C
- All Python integration scripts pass 100%.
- Relayer daemon and indexer confirm 96-byte committing memo compatibility.
- Zero state desynchronization across the full payment lifecycle.
