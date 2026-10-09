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
* **Relayer Hardening & Compliance (DEC-026):** Local Groth16 preflight verification di CPU relayer (`verify_evm_note_proof`) sebagai mitigasi C-01 anti gas-griefing, serta screening sanksi publik aktif (OFAC SDN List di layer HTTP) dengan penolakan langsung (`403 / REJECTED`) pada spend handler dan validation.
* **Inflow Compliance Gate & Mempool Anti-Limbo Lifecycle (DEC-027):** 2-layer sanctions screening pada deposit ingress (OFAC in-memory + Chainalysis on-chain Oracle), dynamic 15-minute lease, mempool lease watchdog background worker dengan strict on-chain simulation (`is_nullifier_spent`), dan restorasi self-custody di `PrivateNoteWallet` (`reclaim_expired_reservations`).
* **Ingress Hardening, Flat Fee & Domain Binding (DEC-028):** Wajib 12 public inputs fail-closed (Fix R4), verifikasi domain target `chain_id` & `contract_address` (Fix R3), penyatuan fee spend flat 45 bps permanen tanpa diskon umur root (Fix R2), dan penegasan batas statement `ComplianceCircuit` (Fix R1).
* **Multi-UTXO JoinSplit & Client Hardening (DEC-030):** Universal 2-in-2-out JoinSplit Groth16 circuit terdeploy di `nimbus-core` (14 public inputs, Option A constant topology, 448 range constraints). Client SDK `nimbus-sdk` ter-upgrade penuh: Tiered Stochastic Knapsack Coin Selection (1-note & 2-note JoinSplit, toleransi ±5%), in-pool autonomous consolidation (`consolidate_notes`), Argon2id + ChaCha20Poly1305 AEAD backup, `zeroize` memory security barrier, anti-ghost note `update_note_witness`, dan canonical hex parsers (29/29 sdk tests pass).
* **Relayer Reconciliation, Solvency Observability & Quote Hardening (DEC-031):** Three-way reconciliation engine (`reconcile_execution_fees`) menyinkronkan akrual kontrak on-chain, database SQLite (`spend_batches`), dan confirmed receipts (`status == 1`); pemusnahan total fallback quote berstatus OK tanpa signing domain (fail-closed EIP-712 domain derivation); serta observabilitas solvensi makro pada `/health` (`solvency_metrics`) tanpa kebocoran data privat user.
* **Merkle Mountain Range Commitment Accumulator (DEC-032):** Menggantikan batas kaku LeanIMT depth 20 ($1.048.576$ daun) dengan in-memory dan on-chain Merkle Mountain Range (MMR). Canonical bagging backward fold mengikat total daun ($N$) via domain separator `DOMAIN_MMR_BAG` anti-malleability, mitigasi Hyperbridge out-of-bounds ($leaf\_index < leaf\_count$) di sirkuit R1CS dan Stylus WASM, verifikasi skalar EVM ($< r$), update storage Stylus `mmr_leaf_count` & `mmr_peaks`, 13 public inputs di `PrivateNoteCircuit`, serta sinkronisasi penuh di `nimbus-sdk` dan `nimbus-node` (367/367 tests pass di seluruh workspace, clippy & fmt clean).
* **Epoch-Windowed Nullifier Pruning & In-Flight Rollover Economics (DEC-033):** Mengadopsi prinsip Sean Bowe & Ian Miers (IACR ePrint 2025/2031) untuk membasmi memory bloat nullifier pada Stylus WASM: 2-Epoch Generational Window ($E$ dan $E-1$, $O(1)$ gas per rotasi slot gen), Evolving Nullifier PRF terikat `epoch_id`, 15 public inputs di `PrivateNoteCircuit` (`note_epoch_id` di index 3 & `is_rollover` di index 14), In-Flight Auto-Rollover & Standalone Refresh dengan dust elimination threshold ($2.00 USDC / `MIN_STANDALONE_ROLLOVER_THRESHOLD_USDC`), self-paying relayer reimbursement (0% protocol fee, gas reimbursement + 15% relayer markup), dan preflight fail-closed validation di `nimbus-node` serta epoch status lifecycle di `nimbus-sdk`.
* **Relayer Operational Hygiene & Health Probes:** Singleton pooling `reqwest::Client` dengan TLS dan timeout ketat (`http.rs`), periodic in-memory rate-limiter pruning anti-memory-leak, dedicated k8s `/live` & `/ready` health endpoints, dan validasi EVM address pada x402 facilitator (147/147 node tests pass, clippy clean).

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
Ref: [`docs/todos/private-note-balance.md`](file:///workspaces/Zeltra-Protocol/docs/todos/private-note-balance.md), [`DEC-016A (Frozen Spec)`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-016A-private-note-spec-freeze.md), [`DEC-016B (Shortcuts Fix)`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-016B-mvp-circuit-shortcuts.md), & [`DEC-030 (Multi-UTXO JoinSplit)`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-030-multi-utxo-joinsplit-coin-selection-zeroize-and-aead-backup.md)

- [ ] Simpan proving/verifying key sebagai artifact versioned; cache development key pada test runner.
- [ ] Jalankan MPC ceremony hanya setelah seluruh constraint dan public-input ABI dibekukan serta direview.

---

## 2. Smart Contract Stylus (`nimbus-contracts`)

> **Status Gate D:** SELESAI (100%) — Entrypoint `spend_private_note(...)` terintegrasi dengan verifier Groth16 EIP-2537 (`0x0c` + `0x0f`), pohon LeanIMT depth 20, dan multi-liability solvency invariant.

- [ ] Sinkronisasi audit report & formal verification untuk parameter curve BLS12-381 precompile compatibility di Stylus.

---

## 3. Client SDK Private Wallet State (`nimbus-sdk`)

> **Status Gate E:** SELESAI (100%) — `PrivateNoteWallet` mengelola saldo UTXO, coin selection, local proof generation, dan persistensi terenkripsi (DEC-024). Tiered Stochastic Knapsack Coin Selection & in-pool `consolidate_notes()` telah terintegrasi (DEC-030, 29/29 tests pass).

- [ ] WebAssembly / Browser packaging test runner untuk eksekusi sirkuit client-side secara non-blocking via Web Worker.

---

## 4. Relayer & Node Settlement (`nimbus-node`)

> **Status Gate F:** SELESAI (100% Full Pipeline & Solvency Observability) — Ingress endpoint `/api/v1/spend-private-note`, verifikasi skalar kanonikal, binding semantik DEC-022, dual-layer double-spend guard, direct EVM broadcast ([`DEC-025`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-025-relayer-zk-note-spend-settlement-atomic-batching-and-reconciliation.md)), Three-Way Reconciliation Engine (`reconcile_execution_fees`), pemusnahan total fallback quote tanpa signing domain, dan zero-leakage solvency observability ([`DEC-031`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-031-relayer-reconciliation-solvency-observability-quote-domain-binding.md), 68/68 node tests pass, clippy clean).

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
  - Implementasikan auto-renew token Vault (AppRole / short-lived token).

---

## 7. Integrasi Eksternal (Opsional / Secondary)

- [ ] **Polymarket Intent Settlement**:
  - Uji alur intent settlement privat dan fallback refund di testnet.

---

## 8. Mainnet Readiness Gate

- [ ] Seluruh invariant akuntansi terbukti solvent: `contract assets >= all liabilities`.
- [ ] Tidak ada fallback mock atau bypass `#[cfg(test)]` pada binary production.
- [ ] Queue dan settlement tahan restart di setiap titik transisi.
- [ ] Audit keamanan eksternal independen untuk smart contract Stylus, relayer node, dan cryptographic circuits.
