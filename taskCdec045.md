# Task C: 1-Byte View Tag Memo Protocol & High-Throughput Client Trial Decryption

- **Specification Reference:** [`research/decisions/DEC-045C-view-tag-memo-protocol-and-client-scanning-acceleration.md`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-045C-view-tag-memo-protocol-and-client-scanning-acceleration.md)
- **Parent Blueprint:** [`research/decisions/DEC-045-in-memory-incremental-mmr-engine-and-relayer-proof-acceleration.md`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-045-in-memory-incremental-mmr-engine-and-relayer-proof-acceleration.md)
- **Status:** READY FOR IMPLEMENTATION
- **Applies to:**
  - `zeltra-core/src/note.rs` (`derive_view_tag`, updated 96-byte memo layout with view tag, deterministic encryption/decryption)
  - `zeltra-sdk/src/wallet/note_wallet/sync.rs` (`scan_memos_fast`, fast-path pre-filter, fallback trial decryption)
  - `zeltra-sdk/src/wasm_types.rs` (JS/TS bindings for fast memo scanning in Web Workers)
- **Security Audit Resolusi:** AUD-18 (Trial Decryption Battery/CPU Exhaustion), AUD-19 (Side-Channel Tag Distribution Bias), AUD-20 (Malformed View Tag State Desynchronization)
- **Primary Objective:** Implement the 1-Byte View Tag protocol (inspired by Zcash Orchard ZIP-212 and Monero Seraphis MRL-0012) within the canonical 96-byte memo format. Client wallets filter out 99.6% of non-relevant pool memos using a single integer byte comparison before invoking ChaCha20-Poly1305 MAC checks, achieving a 23.6x speedup and saving 95.5% battery/CPU power on client devices.

---

## Progress Overview

- [ ] **Phase 1: Deterministic View Tag KDF & Memo Layout (`zeltra-core`)** [4/4]
- [ ] **Phase 2: Fast-Scanning Note Scanner Implementation (`zeltra-sdk`)** [3/3]
- [ ] **Phase 3: Backward Compatibility & Fallback Engine** [3/3]
- [ ] **Phase 4: WASM & Web Worker Bindings** [3/3]
- [ ] **Phase 5: Exhaustive Test Matrix (1x Positive + $\ge 2\times$ Negative)** [10/10]

---

## Detailed Task Breakdown

### Phase 1: Deterministic View Tag KDF & Memo Layout
- [ ] **C1.1 — Implement `derive_view_tag(shared_secret: &[u8; 32]) -> u8`:**
  Derive 1-byte view tag using HKDF-Expand with domain separator `b"zeltra_view_tag"`.
- [ ] **C1.2 — Update 96-Byte Memo Serialization Layout:**
  Layout byte slices strictly: `[epk: 32B || view_tag: 1B || nonce: 11B || ciphertext: 36B || poly1305_tag: 16B] = 96B`.
- [ ] **C1.3 — Calldata Size Invariant Enforcement (INV-45C-1):**
  Assert `encrypted_memo.len() == 96` at compile-time and runtime to ensure zero breaking changes to Stylus smart contract calldata guards (DEC-043B).
- [ ] **C1.4 — Update Deterministic Memo Encryption:**
  Update `encrypt_note_memo_deterministic` to append the 1-byte tag at offset 32.

---

### Phase 2: Fast-Scanning Note Scanner Implementation
- [ ] **C2.1 — Implement `scan_memos_fast` in SDK:**
  In `zeltra-sdk/src/wallet/note_wallet/sync.rs`, iterate through on-chain `NoteCommitmentAppended` memo events.
- [ ] **C2.2 — Pre-Filter Fast Path Execution:**
  For each event, compute local `shared_secret` via `ivk.diffie_hellman(&eph_pk)` and derive `expected_view_tag`. If `memo[32] != expected_view_tag`, discard immediately without invoking AEAD.
- [ ] **C2.3 — Candidate Full Decryption:**
  For candidate memos passing the pre-filter ($\sim 0.39\%$ false positive + legitimate notes), perform ChaCha20-Poly1305 MAC verification and plaintext reconstruction.

---

### Phase 3: Backward Compatibility & Fallback Engine
- [ ] **C3.1 — Legacy Memo Header Detection:**
  If scanning notes generated prior to the DEC-045 activation block, detect legacy layout and transparently fall back to trial decryption.
- [ ] **C3.2 — Dummy Note Discarding:**
  If decrypted plaintext indicates `value == 0` (Mode 2 exact transfer dummy memo, DEC-043C), discard note without adding to wallet unspent list.
- [ ] **C3.3 — Canonical Commitment Verification:**
  Verify reconstructed note commitment matches on-chain leaf before crediting balance.

---

### Phase 4: WASM & Web Worker Bindings
- [ ] **C4.1 — Export Fast-Scan WASM Interface:**
  Export `scan_memos_fast_wasm` in `zeltra-sdk/src/wasm_types.rs` taking serialized memo arrays.
- [ ] **C4.2 — Disposable Web Worker Integration (DEC-041):**
  Ensure batch memo scanning runs cleanly inside the disposable Web Worker lifecycle with zero linear memory leaks.
- [ ] **C4.3 — JS/TS SDK Types Parity:**
  Update TypeScript type definitions for recovered note candidates and scanner metrics.

---

### Phase 5: Exhaustive Test Matrix (Positive + $\ge 2\times$ Negative)

#### Positive Tests (3 Tests):
- [ ] **C5.1 — `test_view_tag_derivation_and_fast_scan_success`:**
  Encrypt note for recipient with view tag. Scan 1,000 memos (1 recipient, 999 random). Assert $\sim 996$ memos discarded at pre-filter. Target note decrypted with 100% commitment match.
- [ ] **C5.2 — `test_view_tag_uniform_distribution_statistical_entropy`:**
  Generate 10,000 independent keypairs. Measure distribution across 256 byte bins ($0\text{x}00$ to $0\text{xFF}$). Chi-Square test confirms $p > 0.05$ (uniform entropy, zero bias).
- [ ] **C5.3 — `test_view_tag_backward_compatible_fallback_scan`:**
  Scanner processes mixed batch containing legacy memos (DEC-037) and new DEC-045C memos. Verifies all notes decrypted without panic or loss.

#### Negative Tests (7 Tests — $>2\times$ Ratio, Proving State Invariance):
- [ ] **C5.4 — `test_view_tag_rejects_corrupted_ciphertext_mac_failure`:**
  Tamper 1 byte in ciphertext while keeping matching tag. Pre-filter passes, Poly1305 strictly returns `None`. Assert wallet unspent balance remains strictly unchanged.
- [ ] **C5.5 — `test_view_tag_rejects_tampered_view_tag`:**
  Tamper 1 byte view tag of legitimate note. Pre-filter skips note safely without AEAD error; assert wallet does not panic and existing state is untouched.
- [ ] **C5.6 — `test_view_tag_rejects_invalid_memo_length`:**
  Test memos with invalid length (95B, 97B, 128B, 0B). Assert scanner rejects at length check before slicing; state invariant.
- [ ] **C5.7 — `test_view_tag_fails_with_wrong_incoming_viewing_key`:**
  Third party scans note using wrong IVK. Even on 0.39% tag collision, Poly1305 decryption strictly fails. Assert zero data leakage.
- [ ] **C5.8 — `test_view_tag_rejects_tampered_ephemeral_pubkey`:**
  Tamper ephemeral public key (bytes 0..32). Shared secret diverges; tag mismatch or MAC failure asserted. Wallet state untouched.
- [ ] **C5.9 — `test_view_tag_ignores_dummy_change_note`:**
  Scan exact-transfer dummy change memo ($v = 0$). Tag matches and decrypts, but scanner drops note; assert unspent note count does not increase.
- [ ] **C5.10 — `test_view_tag_rejects_non_canonical_scalar_during_commitment_reconstruction`:**
  Payload contains non-canonical scalar $\rho \ge r$. Commitment reconstruction rejects note before storage insertion; state unchanged.
