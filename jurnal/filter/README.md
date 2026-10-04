# Curated Cryptographic Research Index for Nimbus Protocol

Folder `jurnal/filter/` berisi hasil kurasi dan ekstraksi analitis dari 424 jurnal kriptografi (IACR ePrint & Tier-1 Conferences 2024–2026). Setiap dokumen memetakan paper terpilih langsung ke arsitektur modul Nimbus, masalah konkret yang diselesaikan, dan mekanisme implementasinya.

---

## Struktur Kurasi Berdasarkan Lapisan Arsitektur

| File Kurasi | Lapisan Nimbus | Komponen Terkait | Fokus Utama Inovasi |
|---|---|---|---|
| [`01-contract-innovations.md`](file:///workspaces/Zeltra-Protocol/jurnal/filter/01-contract-innovations.md) | **`nimbus-contracts`** (Stylus WASM) | `spend.rs`, `deposit.rs`, `storage.rs`, EIP-2537 | Prunable nullifier storage, constant-size state accumulators, gasless rate-limiting, batch verification. |
| [`02-core-innovations.md`](file:///workspaces/Zeltra-Protocol/jurnal/filter/02-core-innovations.md) | **`nimbus-core`** (Rust Crypto) | `circuit.rs`, `bls.rs`, `poseidon.rs`, `bdhke.rs` | Defense leakage Groth16, Round-Optimal Blind Signatures, Adaptively secure threshold OPRF, Cryptographic Erasure. |
| [`03-relayer-innovations.md`](file:///workspaces/Zeltra-Protocol/jurnal/filter/03-relayer-innovations.md) | **`nimbus-node`** (Relayer & Cluster) | `settlement.rs`, `guardian.rs`, `queue.rs`, Quorum | Asynchronous robust signing (RSS), fair batch sequencing anti-MEV, Auditing Budget, WabiSabi multi-party coordination. |
| [`04-sdk-innovations.md`](file:///workspaces/Zeltra-Protocol/jurnal/filter/04-sdk-innovations.md) | **`nimbus-sdk`** (Client & Wallet) | `note_wallet.rs`, `zk_wasm.rs`, Coin Selection | Oblivious note synchronization, Auditable stealth addresses (zkBSA), zero-leakage scanning, HD stealth keys. |

---

## Peta Sinergi Antar-Lapisan

```
+-------------------------------------------------------------------------+
|                              NIMBUS SDK                                 |
|   Oblivious Note Sync (2025/2031)  |  Auditable Stealth Addr (zkBSA)    |
+------------------------------------+------------------------------------+
                                     | (Encrypted Quotes / Blind Proofs)
                                     v
+-------------------------------------------------------------------------+
|                             NIMBUS NODE                                 |
|   Robust Signing RSS (2026/1670)   |  Fair Batching Anti-MEV (Velox)    |
|   Auditing Budget Cap (AuditPay)   |  Gasless RLN Spam Defense (2025)   |
+------------------------------------+------------------------------------+
                                     | (Settlement Execution & BLS Quorum)
                                     v
+------------------------------------+------------------------------------+
|                             NIMBUS CORE                                 |
|   Groth16 Leak Defense (Argo)      |  Adaptive Threshold OPRF (2025)   |
|   Provable Key Erasure (2026/1109) |  Audited Poseidon Grain (DEC-021)  |
+------------------------------------+------------------------------------+
                                     | (Proof Points & Curve Operations)
                                     v
+-------------------------------------------------------------------------+
|                           NIMBUS CONTRACTS                              |
|   Evolving Nullifiers (2025/2031)  |  Bonsai O(1) Pruning (2026/1987)   |
|   Stylus Circular Ring Buffer      |  EIP-2537 Precompile Optimizations |
+-------------------------------------------------------------------------+
```
