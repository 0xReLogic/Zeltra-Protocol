# Nimbus Protocol TODO (Active Backlog)

> **Catatan:** File history lengkap sebelum pembersihan tersimpan di [`archive/todo.full-archive.md`](file:///workspaces/Zeltra-Protocol/archive/todo.full-archive.md).
> Seluruh task yang **sudah selesai (`[x]`)** telah diarsipkan agar dokumen ini murni berisi **task pending (`[ ]`)** dan arah kerja yang jelas.

---

## Ringkasan Status Saat Ini (Yang Sudah Berhasil)

Komponen inti berikut **sudah selesai dan diverifikasi**:
* **Smart Contract Stylus:** EIP-2537 BLS pairing check (`e(-alpha, G2) * e(H(m), pk_iss) == 1`), 13/13 testnet negative tests lolos.
* **BLS Threshold Cluster:** 1 Leader + 4 Guardians (3-of-5 threshold) dengan Tailscale binding dan atomic masking key release.
* **Persistent Queue:** SQLite/SQLCipher persistent spend queue dengan leasing dan exponential backoff.
* **Adaptive Batch Spend:** Entrypoint `batch_spend()` (2-8 item) terdeploy di Stylus dengan EIP-712 quote validation.
* **CCIP Tracking:** Event parsing untuk message ID dan background monitor destination.
* **ZK-UTXO Gate C0 (Security Repair):** Poseidon Grain-128 LFSR ([`DEC-021`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-021-audited-poseidon-parameters-grain-lfsr-defense.md)), leaf index Merkle binding, dan 64-bit integer range constraints.
* **ZK-UTXO Gate D (Contract Note Ledger):** Verifier Groth16 BLS12-381 via EIP-2537 (`0x0c` MSM + `0x0f` Pairing Check), LeanIMT depth 20 Merkle tree, dan entrypoint `spend_private_note(...)` di Stylus (53/53 tests pass).
* **ZK-UTXO Gate E (Client Note Wallet):** `PrivateNoteWallet` di SDK dengan local Groth16 proof generation, note lifecycle state machine, encrypted storage, dan privacy-aware coin selection (20/20 tests pass).
* **ZK-UTXO Gate F (Relayer Note Settlement):** Endpoint `/api/v1/spend-private-note`, fail-closed pre-flight checks, dual-layer nullifier double-spend prevention (SQLite & on-chain), dan single-item settlement dispatcher `broadcast_spend_private_note_transaction` ([`DEC-025`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-025-relayer-zk-note-spend-settlement-atomic-batching-and-reconciliation.md)).

---

## 1. Top Priority: Solusi Saldo & Kembalian (The "Voucher" Fix)

Arsitektur yang dipilih secara definitif: **Arah B (ZK-UTXO Model - Gates A s/d G)** sesuai [`research/decisions/DEC-016-private-note-change-ledger.md`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-016-private-note-change-ledger.md) dan Master Architecture Blueprint.

> ⚠️ **POINTER MODULAR UTAMA:**
> Pengerjaan spesifikasi ZK-UTXO, sirkuit, smart contract ledger, SDK wallet state (Gate E), relayer batching (Gate F), dan skenario hard-test Arbitrum Sepolia (Gate G) **mengacu 100% secara modular ke [`docs/todos/private-note-balance.md`](file:///workspaces/Zeltra-Protocol/docs/todos/private-note-balance.md)**.
> 
> **Referensi Repositori Kloning / ATM (Amati, Tiru, Modifikasi):**
> 1. [`https://github.com/supernovahs/zk-sunade`](https://github.com/supernovahs/zk-sunade) — Verifier Groth16 di Arbitrum Stylus via `RawCall` host precompile (WASM <25KB, gas ~250k).
> 2. [`https://github.com/Railgun-Privacy/contract`](https://github.com/Railgun-Privacy/contract) — Smart Contract Railgun (Commitments accumulator, nullifier mapping, root history, token transfer logic).
> 3. [`https://github.com/zk-kit/zk-kit`](https://github.com/zk-kit/zk-kit) — Monorepo reusable ZK libraries Ethereum Foundation (LeanIMT, IMT, Poseidon hashing, tree proof).

### Gate C0 - Security Repair Private Note Circuit
Ref: [`docs/todos/private-note-balance.md`](file:///workspaces/Zeltra-Protocol/docs/todos/private-note-balance.md), [`DEC-016A (Frozen Spec)`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-016A-private-note-spec-freeze.md), & [`DEC-016B (Shortcuts Fix)`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-016B-mvp-circuit-shortcuts.md)

- [ ] Simpan proving/verifying key sebagai artifact versioned; cache development key pada test runner.
- [ ] Jalankan MPC ceremony hanya setelah seluruh constraint dan public-input ABI dibekukan serta direview.

---

## 2. Smart Contract Stylus (`nimbus-contracts`)

> **Status Gate D:** SELESAI (100%) — Entrypoint `spend_private_note(...)` terintegrasi dengan verifier Groth16 EIP-2537 (`0x0c` + `0x0f`), pohon LeanIMT depth 20, dan multi-liability solvency invariant.

- [ ] Sinkronisasi audit report & formal verification untuk parameter curve BLS12-381 precompile compatibility di Stylus.

---

## 3. Client SDK Private Wallet State (`nimbus-sdk`)

> **Status Gate E:** SELESAI (100%) — `PrivateNoteWallet` mengelola saldo UTXO, coin selection, local proof generation, dan persistensi terenkripsi (DEC-024).

- [ ] WebAssembly / Browser packaging test runner untuk eksekusi sirkuit client-side secara non-blocking via Web Worker.

---

## 4. Relayer & Node Settlement (`nimbus-node`)

> **Status Gate F:** SELESAI (Phase 1 Direct Pipeline) — Ingress endpoint `/api/v1/spend-private-note`, verifikasi skalar kanonikal, binding semantik DEC-022, dual-layer double-spend guard, dan direct EVM broadcast ([`DEC-025`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-025-relayer-zk-note-spend-settlement-atomic-batching-and-reconciliation.md)).

- [ ] Claim worker reconcile contract accrual, DB accrual, dan receipts.
- [ ] Hilangkan fallback quote yang mengembalikan status OK tanpa signing domain.
- [ ] Health endpoint expose solvency/accounting mismatch tanpa membuka user data.

---

## 5. Cross-Chain CCIP Testing & Verification

- [ ] **Testnet E2E Hard-Test (Arbitrum Sepolia)**:
  - Uji alur lengkap: source broadcast $\rightarrow$ router $\rightarrow$ destination execution $\rightarrow$ destination confirmation.
  - Uji jalur kegagalan: destination failure memicu alur refund 24h dan double payout rejection.

---

## 6. Security, Secrets & Infrastructure

- [ ] **Credential Rotation & Git Cleanup**:
  - Rotasi seluruh secret/private key yang pernah ter-commit di masa lalu.
  - Hapus credential lama dari git history.
- [ ] **KMS / Vault TLS Hardening**:
  - Ref: [`docs/todos/kms-tls.md`](file:///workspaces/Zeltra-Protocol/docs/todos/kms-tls.md)
  - Ganti raw TCP HTTP client dengan HTTPS client tervalidasi untuk Vault dan guardian RPC.
  - Verifikasi certificate TLS.
- [ ] **Operasional API**:
  - Terapkan timeout pada guardian, Vault, dan RPC requests.
  - Bersihkan in-memory rate-limit map secara berkala agar tidak memory leak.
  - Tambahkan readiness dan liveness endpoint terpisah.

---

## 7. Integrasi Eksternal (Opsional / Secondary)

- [ ] **x402 Facilitator**:
  - Ref: [`docs/todos/x402.md`](file:///workspaces/Zeltra-Protocol/docs/todos/x402.md)
  - Ganti mock tx hash dengan status settlement nyata.
  - Ganti recipient `"x402-facilitator-pool"` dengan konfigurasi address EVM nyata.
- [ ] **Polymarket Intent Settlement**:
  - Uji alur intent settlement privat dan fallback refund di testnet.

---

## 8. Mainnet Readiness Gate

- [ ] Seluruh invariant akuntansi terbukti solvent: `contract assets >= all liabilities`.
- [ ] Tidak ada fallback mock atau bypass `#[cfg(test)]` pada binary production.
- [ ] Queue dan settlement tahan restart di setiap titik transisi.
- [ ] Audit keamanan eksternal independen untuk smart contract Stylus, relayer node, dan cryptographic circuits.
