# Nimbus Protocol TODO (Active Backlog)

> **Catatan:** File history lengkap sebelum pembersihan tersimpan di [`archive/todo.full-archive.md`](file:///workspaces/Zeltra-Protocol/archive/todo.full-archive.md).
> Seluruh task yang **sudah selesai (`[x]`)** telah diarsipkan agar dokumen ini murni berisi **task pending (`[ ]`)** dan arah kerja yang jelas.

---

## Ringkasan Status Saat Ini (Yang Sudah Berhasil)

Komponen inti berikut **sudah selesai dan terbukti di Arbitrum Sepolia**:
* **Smart Contract Stylus:** EIP-2537 BLS pairing check (`e(-alpha, G2) * e(H(m), pk_iss) == 1`), 13/13 testnet negative tests lolos.
* **BLS Threshold Cluster:** 1 Leader + 4 Guardians (3-of-5 threshold) dengan Tailscale binding dan atomic masking key release.
* **Persistent Queue:** SQLite/SQLCipher persistent spend queue dengan leasing dan exponential backoff.
* **Adaptive Batch Spend:** Entrypoint `batch_spend()` (2-8 item) terdeploy di Stylus dengan EIP-712 quote validation.
* **CCIP Tracking:** Event parsing untuk message ID dan background monitor destination.

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

- [ ] **Cryptographic parameters**:
  - [x] Generator parameter standar/audited Grain-128 LFSR (`GrainLfsr`) terimplementasi di [`nimbus-core/src/poseidon.rs`](file:///workspaces/Zeltra-Protocol/nimbus-core/src/poseidon.rs) sesuai [`DEC-021`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-021-audited-poseidon-parameters-grain-lfsr-defense.md) (108/108 tests pass).
  - [ ] Simpan proving/verifying key sebagai artifact versioned; cache development key pada test runner.

---

## 2. Smart Contract Stylus (`nimbus-contracts`)

- [x] **Private Note ZK Spend Entrypoint (Gate D Blocker)**:
  - Implementasikan entrypoint `spend_private_note(...)` di [`nimbus-contracts/src/spend.rs`](file:///workspaces/Zeltra-Protocol/nimbus-contracts/src/spend.rs):
    * Verifikasi proof Groth16 on-chain (`PrivateNoteCircuit`) via precompiles **EIP-2537 `0x0c` (`BLS12_G1MSM`)** dan **`0x0f` (`BLS12_PAIRING_CHECK`)** untuk kurva BLS12-381 (mengadopsi arsitektur lean verifier dari `zk-sunade`).
    * Enforce `accepted_note_roots` berisi `merkle_root` publik dari proof.
    * Catat `note_nullifier` ke storage `note_nullifiers` (revert jika double-spend).
    * Jika `has_change == 1`, masukkan `change_commitment` ke pohon **LeanIMT depth 20** (`merkle.rs`).
    * Transfer `payout` ke `recipient` dan alokasikan protocol & execution fees.
  - Verifikasi Stylus WASM target (`wasm32-unknown-unknown` pass) dan test suite (53/53 tests pass termasuk 8 test positif & negatif `spend_private_note`).

---

## 3. Client SDK Private Wallet State (`nimbus-sdk` - Gate E Blocker)

- [x] **Private Note Wallet & UTXO Selection (Gate E Blocker)** (Rincian lengkap di [`docs/todos/private-note-balance.md`](file:///workspaces/Zeltra-Protocol/docs/todos/private-note-balance.md)):
  - Ganti `AgentTokenPool` (voucher BDHKE nominal kaku) dengan model Note UTXO: akumulasi seluruh unspent notes menjadi satu saldo gabungan.
  - Implementasikan note lifecycle state machine: `unconfirmed -> unspent -> reserved -> spent`.
  - Privacy-aware coin selection & change calculation: otomatis memilih input note dan menghitung nilai kembalian (*change note*).
  - Local proof generator: buat proof Groth16 (`PrivateNoteCircuit`) langsung di client SDK (jangan pernah kirim spending key / note preimage ke relayer).
  - Client API methods: `deposit(amount)`, `pay(recipient, amount)`, `send_to_wallet(recipient, amount)`, dan `withdraw_all(owner_wallet)`.

---

## 4. Relayer & Node Settlement (`nimbus-node` - Gate F Blocker)

- [ ] **Relayer ZK Note Spend & Batching (Gate F Blocker)** (Rincian lengkap di [`docs/todos/private-note-balance.md`](file:///workspaces/Zeltra-Protocol/docs/todos/private-note-balance.md)):
  - Handler spend menerima proof ZK dan memverifikasi signed EIP-712 execution quote secara fail-closed.
  - Dukung `batch_spend()` untuk multi-item ZK note (membawa output change commitments atomic).
  - Sinkronisasi DB relayer saat receipt transaksi confirmed: input note `spent`, change note `unspent`.
  - Claim worker melakukan rekonsiliasi berkala antara contract accrual, DB accrual, dan receipt on-chain.

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
