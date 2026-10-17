# Curated Cryptographic Research Index for Zeltra Protocol

Folder `jurnal/filter/` berisi hasil kurasi dan ekstraksi analitis dari jurnal kriptografi terpilih (IACR ePrint & Tier-1 Conferences 2024–2026). Setiap dokumen memetakan paper terpilih langsung ke arsitektur modul Zeltra, masalah konkret yang diselesaikan, dan mekanisme implementasinya.

---

## Struktur Kurasi Berdasarkan Lapisan Arsitektur

| File Kurasi | Lapisan Zeltra | Komponen Terkait | Fokus Utama Inovasi |
|---|---|---|---|
| File Kurasi | Lapisan Zeltra | Komponen Terkait | Jumlah Paper & Fokus Utama Inovasi |
|---|---|---|---|
| [`01-contract-innovations.md`](file:///workspaces/Zeltra-Protocol/jurnal/filter/01-contract-innovations.md) | **`zeltra-contracts`** (Stylus WASM) | `spend.rs`, `deposit.rs`, `storage.rs`, EIP-2537 | **5 Paper**: FlatMerkle gas optimization, Merkle Mountain Range, gasless rate-limiting, EIP-2537 batch verification, ring buffer pruning. |
| [`02-core-innovations.md`](file:///workspaces/Zeltra-Protocol/jurnal/filter/02-core-innovations.md) | **`zeltra-core`** (Rust Crypto) | `note_circuit.rs`, `joinsplit_circuit.rs`, `poseidon.rs`, `evm.rs` | **14 Paper**: Committing AEAD, polynomial arithmetic without roots of unity, additive accumulators, Poseidon subspace cryptanalysis & Dixon resultants, Jacobian circuit diagnostics, regular $\tau$-NAF on BLS12-381, null-branch dummy slot pinning. |
| [`03-relayer-innovations.md`](file:///workspaces/Zeltra-Protocol/jurnal/filter/03-relayer-innovations.md) | **`zeltra-node`** (Sequencer & Relayer) | `validation.rs`, `database.rs`, `evm_client.rs`, `handlers/` | **18 Paper**: Oblivious search trees, SkrrtPIR & iSimplePIR (incremental single-server PIR), anonymous tokens, wpORAM for oblivious RPC, plus **7 Paper Fondasi Desentralisasi** (Fair leader election, bandwidth-balanced broadcast, batched threshold encryption, early-stopping Byzantine agreement, bandwidth fee mechanisms, payload blindness anti-MEV, harm-proof TFM, UABC atomic broadcast, dan stake-weighted threshold encryption). |
| [`04-sdk-innovations.md`](file:///workspaces/Zeltra-Protocol/jurnal/filter/04-sdk-innovations.md) | **`zeltra-sdk`** (Client & Wallet) | `note_wallet.rs`, `zk_wasm.rs`, Viewing Keys, Compliance | **17 Paper**: Bounded-memory Groth16, ZKPoSP HD provenance proofs & CCIP anti-hijacking, secure MSM outsourcing, strong resettable NIZK (anti-memory reset), viewing key provable anonymity, DuraShare manual BIP-39 recovery, auditable stealth addresses (zkBSA), MinMandate. |

---

## Ringkasan Metrik Kurasi
- **Total Paper Dianalisis:** 1.000+ paper dari repositori IACR ePrint 2024–2026.
- **Total Paper Lolos Kurasi Mendalam:** **54 Paper Kriptografi Terpilih**.
- **Status Antrean `raw.md`:** **100% Selesai & Dikosongkan (Queue Cleared)**.
- **Kategori Khusus Roadmap Desentralisasi:** 7 paper ditandai dengan badge `[DESENTRALISASI MASA DEPAN (ROADMAP)]` di `03-relayer-innovations.md` untuk memandu fase transisi dari Single Stateless Sequencer menuju Decentralized Multi-Relayer Network.

---

## Peta Sinergi Antar-Lapisan (Pure ZK-UTXO)

```
+-------------------------------------------------------------------------+
|                               ZELTRA SDK                                |
|   Bounded-Memory Groth16 (2026/1758) | HD Seed Provenance (2026/2204)   |
|   Oblivious Note Sync (2025/2031)    | Auditable Stealth Addr (zkBSA)   |
|   Secure MSM Outsourcing (2026/686)  | DuraShare Paper Backup (2026/2300)|
|   Strong Resettable NIZK (2026/1036) | MinMandate Agentic Spending      |
+--------------------------------------+----------------------------------+
                                       | (ZK Proofs & Public Inputs)
                                       v
+-------------------------------------------------------------------------+
|                       ZELTRA STATELESS SEQUENCER                        |
|   Incremental iSimplePIR (2026/030)  | Write-Public ORAM (2026/2163)    |
|   SkrrtPIR RLWE Unpacking (2026/2385)| Anonymous Rate-Tokens (2026/870) |
|   [ROADMAP DESENTRALISASI]:                                             |
|   - UABC Atomic Broadcast (2026/2150) | Silent Threshold Enc (2026/2148)|
|   - Early-Stopping BA (2026/2284)    | Harm-Proof TFM (2025/2247)       |
+--------------------------------------+----------------------------------+
                                       | (L2 Broadcast via Alloy)
                                       v
+-------------------------------------------------------------------------+
|                               ZELTRA CORE                               |
|   Dixon Resultants on Poseidon (2026/1969)| Committing AEAD (2026/240)  |
|   Jacobian Circuit Check (2026/1852) | Null-Branch Pinning (2026/2235)  |
|   Regular tau-NAF BLS12-381(2024/1906)| Poseidon LFSR Margins (2026/1692)|
+--------------------------------------+----------------------------------+
                                       | (Proof Points & Curve Verification)
                                       v
+-------------------------------------------------------------------------+
|                            ZELTRA CONTRACTS                             |
|   FlatMerkle Gas Benchmark (2026/2468)| Merkle Mountain Range (DEC-032) |
|   Stylus Circular Ring Buffer        | EIP-2537 Precompile Host Calls   |
+-------------------------------------------------------------------------+
```


