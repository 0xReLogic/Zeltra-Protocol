# Curated Cryptographic Research Index for Zeltra Protocol

Folder `jurnal/filter/` berisi hasil kurasi dan ekstraksi analitis dari jurnal kriptografi terpilih (IACR ePrint & Tier-1 Conferences 2024–2026). Setiap dokumen memetakan paper terpilih langsung ke arsitektur modul Zeltra, masalah konkret yang diselesaikan, dan mekanisme implementasinya.

---

## Struktur Kurasi Berdasarkan Lapisan Arsitektur

| File Kurasi | Lapisan Zeltra | Komponen Terkait | Fokus Utama Inovasi |
|---|---|---|---|
| [`01-contract-innovations.md`](file:///workspaces/Zeltra-Protocol/jurnal/filter/01-contract-innovations.md) | **`zeltra-contracts`** (Stylus WASM) | `spend.rs`, `deposit.rs`, `storage.rs`, EIP-2537 | Merkle Mountain Range accumulator, gasless rate-limiting, EIP-2537 batch verification, ring buffer pruning. |
| [`02-core-innovations.md`](file:///workspaces/Zeltra-Protocol/jurnal/filter/02-core-innovations.md) | **`zeltra-core`** (Rust Crypto) | `note_circuit.rs`, `joinsplit_circuit.rs`, `poseidon.rs` | Sanitasi subgroup Groth16 (Argo), Jacobian circuit diagnostics, Cryptographic Erasure, Decoy dummy slots. |
| [`03-relayer-innovations.md`](file:///workspaces/Zeltra-Protocol/jurnal/filter/03-relayer-innovations.md) | **`zeltra-node`** (Stateless Sequencer) | `validation.rs`, `database.rs`, `evm_client.rs` | Preflight CPU verification, Auditing Budget (AuditPay), fair batch sequencing anti-MEV, WabiSabi decomposition. |
| [`04-sdk-innovations.md`](file:///workspaces/Zeltra-Protocol/jurnal/filter/04-sdk-innovations.md) | **`zeltra-sdk`** (Client & Wallet) | `note_wallet.rs`, `zk_wasm.rs`, Viewing Keys | Oblivious note synchronization, Auditable stealth addresses (zkBSA), zero-leakage scanning, MinMandate agentic workflows. |

---

## Peta Sinergi Antar-Lapisan (Pure ZK-UTXO)

```
+-------------------------------------------------------------------------+
|                               ZELTRA SDK                                |
|   Oblivious Note Sync (2025/2031)  |  Auditable Stealth Addr (zkBSA)    |
|   MinMandate Agentic Spending      |  Viewing Key Trial Decryption      |
+------------------------------------+------------------------------------+
                                     | (ZK Proofs & Public Inputs)
                                     v
+-------------------------------------------------------------------------+
|                       ZELTRA STATELESS SEQUENCER                        |
|   Local CPU Groth16 Preflight      |  Fair Batching Anti-MEV (Otter)    |
|   Auditing Budget Cap (AuditPay)   |  Three-Way Reconciliation          |
+------------------------------------+------------------------------------+
                                     | (L2 Broadcast via Alloy)
                                     v
+------------------------------------+------------------------------------+
|                               ZELTRA CORE                               |
|   Groth16 Leak Defense (Argo)      |  Jacobian Soundness Diagnostics    |
|   Provable Key Erasure (2026/1109) |  Audited Poseidon Grain (DEC-021)  |
+------------------------------------+------------------------------------+
                                     | (Proof Points & Curve Verification)
                                     v
+-------------------------------------------------------------------------+
|                            ZELTRA CONTRACTS                             |
|   Merkle Mountain Range (DEC-032)  |  2-Epoch Nullifiers (DEC-033)      |
|   Stylus Circular Ring Buffer      |  EIP-2537 Precompile Host Calls    |
+-------------------------------------------------------------------------+
```
