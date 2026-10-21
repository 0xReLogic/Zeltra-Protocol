# Task B: Client SDK Scanner & Wallet Integration (DEC-048)

> **Session Scope:** `zeltra-sdk` (Client trial decryption, fast View Tag scanner, and payment note construction)  
> **Parent ADR:** [`research/decisions/DEC-048-committing-aead-partitioning-oracle-defense-and-key-binding-memos.md`](research/decisions/DEC-048-committing-aead-partitioning-oracle-defense-and-key-binding-memos.md)  
> **Target Files:**
> - `zeltra-sdk/src/wallet/note_wallet/sync.rs`
> - `zeltra-sdk/src/wallet/note_wallet/joinsplit.rs`
> - `zeltra-sdk/src/wallet/note_wallet/spend.rs`
> - `zeltra-sdk/tests/view_tag_tests.rs`

---

## 1. Objectives for Session B

1. **Integrate Committing Memo Decryption into `sync.rs`:**
   - Update `PrivateNoteWallet::scan_memos_fast` and `recover_notes_batch` to use `decrypt_committing_note_memo`.
   - Preserve dual-path fallback:
     - Primary path: DEC-048 (EtH-KS committing AEAD, 40-byte payload).
     - Secondary fallback path: DEC-045C / DEC-037 legacy memos for historical blocks.
2. **Update Note Creation for In-Pool Transfer & Public Spend:**
   - In `joinsplit.rs` (Strict Mode 2): generate dual 96-byte committing memos using the new 40-byte payload serializer.
   - In `spend.rs` (Strict Mode 3): generate change memo using committing AEAD.
3. **Verify End-to-End SDK Integrity:**
   - Ensure `sync.rs` note commitment verification against `event.commitment_hex` functions cleanly with values exceeding $2^{31}$ (e.g. 5,000 USDC deposits).

---

## 2. Step-by-Step Implementation Checklist

- [x] **Step B1: Update `sync.rs` trial decryption pipeline:**
  - Fast-path 1-byte View Tag comparison.
  - Committing tag verification before decryption.
  - Transparent fallback to legacy memo format if committing decryption returns `None`.
- [x] **Step B2: Update memo generation in `joinsplit.rs` and `spend.rs`:**
  - Call `encrypt_committing_note_memo` for Output 1 and Output 2.
  - Ensure zero regressions in Mode 1 (Rollover), Mode 2 (In-Pool), and Mode 3 (Spend).
- [x] **Step B3: SDK Regression Tests:**
  - Run `cargo test -p zeltra-sdk view_tag_tests`.
  - Add test case verifying a deposit of 5,000 USDC is recovered with exact matching commitment.

---

## 3. Definition of Done for Session B
- `cargo test -p zeltra-sdk` passes completely.
- Seamless recovery of both legacy memos and new DEC-048 committing memos.
