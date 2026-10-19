# DEC-045: In-Memory Incremental MMR Engine, Persistent Frontier Checkpointing & Relayer Proof Acceleration

**Status:** APPROVED  
**Date:** 2026-10-19  
**Domain:** `zeltra-node`, `zeltra-core`, `zeltra-sdk`  
**Security Level:** CRITICAL (Proof Availability, DoS Immunity, Sub-Second Latency, Zero Reorg Drift)  
**Related Decisions:**
- [`DEC-032: Merkle Mountain Range Commitment Accumulator`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-032-merkle-mountain-range-commitment-accumulator.md)
- [`DEC-035C: Relayer MMR Indexer Worker and Client Sync Architecture`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-035C-relayer-mmr-indexer-worker-and-client-sync-architecture.md)
- [`DEC-037: Note Encryption Memo & Viewing Key Recovery`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-037-note-encryption-memo-and-viewing-key-recovery.md)
- [`DEC-038B: Contract Direct MMR Deposit & Storage Cleanup`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-038B-contract-direct-mmr-deposit-and-storage-cleanup.md)
- [`DEC-043B: Stylus Strict Three-Mode Settlement & Dual Memo MMR`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-043B-stylus-strict-three-mode-settlement-and-dual-memo-mmr.md)
- [`DEC-044: Surgical Architecture Refactoring Methodology`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-044-surgical-refactoring-methodology-and-zero-regression-pipeline.md)

---

## 1. Executive Summary & Problem Statement

Protokol Zeltra mengandalkan Merkle Mountain Range (MMR) sebagai akumulator komitmen note ZK-UTXO tak terbatas ($2^{64}$ kapasitas) dengan komputasi hash Poseidon $O(\log N)$ dan peak bagging backward-fold kanonikal (DEC-032). Di layer smart contract Stylus, efisiensi eksekusi on-chain telah terbukti optimal karena kontrak hanya memperbarui active peaks dan memancarkan log event `NoteCommitmentAppended` tanpa menyimpan node tree secara mahal di storage.

Namun, pada layer off-chain Relayer (`zeltra-node`), audit performa dan pengujian empiris membuktikan adanya **bottleneck arsitektur kritis** pada pipeline serving proof MMR:

1. **Rekonstruksi Tree $O(N)$ Pada Setiap Request (`handle_mmr_proof`):**
   Setiap kali ada request `GET /api/v1/mmr/proof/:leaf_index`, endpoint mengeksekusi `state.db.load_mmr_tree()`. Fungsi ini membaca seluruh tabel `mmr_leaves` dari database SQLite via enkripsi SQLCipher (AES-256 baris-per-baris), mem-parse seluruh skalar $Fr$, dan membangun ulang tree dari leaf 0 sampai $N$ dengan komputasi loop hash Poseidon.
2. **Worker Indexer Melakukan Full Rebuild $O(N)$:**
   Background worker `mmr_indexer` juga memanggil `load_mmr_tree()` dari awal setiap kali mendeteksi event note baru dari blockchain L2, alih-alih melakukan append secara inkremental.
3. **Re-Hashing Subtree On-The-Fly (`MerkleMountainRange`):**
   Struct `MerkleMountainRange` di `zeltra-core` hanya menyimpan raw `leaves: Vec<Fr>` di RAM tanpa menyimpan intermediate node pohon biner (*mountains*). Akibatnya, saat `generate_proof()` dipanggil, seluruh layer internal subtree dihitung ulang dari nol.
4. **Degradasi Eksponensial di Bawah Konkurensi:**
   Di bawah beban trafik konkuren, latensi serving proof melonjak hingga belasan detik dan relayer mengalami CPU/I/O starvation (vulnerabilitas Denial-of-Service fatal).

**Tujuan DEC-045:** Menetapkan arsitektur baru Relayer Proof Engine berbasis **In-Memory Incremental MMR Mirror (`Arc<RwLock<MerkleMountainRange>>`)**, persistensi frontier checkpointing $O(1)$, caching node subtree immutable, dan optimasi client scanning via 1-Byte View Tag.

---

## 2. Bukti Empiris & Hasil Benchmark (Empirical Proof)

Berdasarkan pengujian benchmark independen menggunakan harness release Rust dan Python suite (`scripts/benchmark_mmr_bottleneck.py`), diperoleh bukti matematis dan metrik riil sebagai berikut:

### A. Skalabilitas `load_mmr_tree()` Terhadap Jumlah Leaf ($N$)
Membuktikan kompleksitas strictly $O(N)$ dari rekonstruksi SQLite + Poseidon:

| Leaf Count ($N$) | Waktu Populate DB (ms) | `load_mmr_tree` (ms) | Throughput Rekonstruksi (leaves/detik) |
|---|---|---|---|
| **100** | 69.87 ms | 54.42 ms | 1,837.4 |
| **500** | 272.97 ms | 241.85 ms | 2,067.4 |
| **1,000** | 662.89 ms | 534.99 ms | 1,869.2 |
| **2,500** | 1,546.61 ms | 1,435.34 ms | 1,741.7 |
| **5,000** | 3,373.95 ms | **3,130.40 ms** | 1,597.2 |

*Temuan:* Pada 5,000 leaf, satu pemanggilan `load_mmr_tree()` membutuhkan waktu **3.13 detik**. Pada 50,000 leaf, satu request diperkirakan memakan waktu >30 detik!

### B. Dekomposisi Waktu Request Proof (Kemana Waktu Terbuang?)

| Leaf Count ($N$) | DB Load & Rebuild (ms) | Pure Proof Gen ($\mu\text{s}$) | Total Latency (ms) | Cached Proof ($\mu\text{s}$) | Speedup Murni |
|---|---|---|---|---|---|
| **1,000** | 522.84 ms | 43,442 $\mu\text{s}$ | 566.28 ms | 46,235 $\mu\text{s}$ | **12.2x** |
| **2,500** | 1,455.05 ms | 173,178 $\mu\text{s}$ | 1,628.23 ms | 182,848 $\mu\text{s}$ | **8.9x** |
| **5,000** | 3,151.26 ms | 377,757 $\mu\text{s}$ | 3,529.02 ms | 366,365 $\mu\text{s}$ | **9.6x** |

> [!CAUTION]
> **Fakta Empiris:** Sebanyak **89.30% hingga 92.33%** waktu pemrosesan pada endpoint `/api/v1/mmr/proof` **terbuang sia-sia** hanya untuk membaca database dan membangun ulang tree yang identik berulang-ulang!

### C. Ingestion Indexer Worker: Rebuild vs Inkremental

| Leaf Count ($N$) | Current Rebuild + Append ($\mu\text{s}$) | Pure In-Memory Append ($\mu\text{s}$) | Speedup Factor |
|---|---|---|---|
| **1,000** | 553,385 $\mu\text{s}$ (0.55 s) | 584 $\mu\text{s}$ | **947.6x** |
| **2,500** | 1,458,707 $\mu\text{s}$ (1.46 s) | 504 $\mu\text{s}$ | **2,894.3x** |
| **5,000** | 3,173,967 $\mu\text{s}$ (3.17 s) | **523 $\mu\text{s}$** | **6,068.8x** |

*Temuan:* Menambahkan 1 leaf baru pada in-memory tree hanya butuh **0.52 ms**, sedangkan mekanisme reload saat ini membuang waktu **3.17 detik** per event batch.

### D. Live HTTP Concurrency Test (Axum 0.8 — 2,500 Leaves, 20 Concurrent Workers)

| Arsitektur Endpoint | p50 Latency (ms) | p95 Latency (ms) | p99 Latency (ms) | Throughput (RPS) |
|---|---|---|---|---|
| **Current (`/api/v1/mmr/proof`)** | 12,383.02 ms (12.3 s) | 14,414.05 ms | 15,556.94 ms | **0.8 RPS** |
| **Proposed (`/api/v1/mmr/cached-proof`)** | **2,580.22 ms** | 3,539.25 ms | 3,847.35 ms | **7.9 RPS** |
| **Improvement** | **4.8x Lebih Cepat** | **4.1x Lebih Cepat** | **4.0x Lebih Cepat** | **10.0x Peningkatan RPS** |

### E. Optimasi Client Wallet Sync: ChaCha20Poly1305 vs 1-Byte View Tag

| Strategi Scanning | Waktu Scan 2,000 Memo (ms) | Catatan yang Diproses Kriptografi | Speedup |
|---|---|---|---|
| **Current: Linear Trial Decryption** | 2.72 ms | 2,000 notes | **1.0x** |
| **Proposed: 1-Byte View Tag Filter** | **0.12 ms** | 10 notes (99.5% tereliminasi) | **23.6x** |

---

## 3. Desain Arsitektur Baru (DEC-045 Design)

```
                       +-------------------------------------------------+
                       |              ARBITRUM L2 RPC                   |
                       +-------------------------------------------------+
                                               |
                                     Log Events (Safe Head)
                                               v
+---------------------------------------------------------------------------------------+
| RELAYER NODE (`zeltra-node`)                                                          |
|                                                                                       |
|  +-----------------------+           Write Lock (O(1))        +--------------------+  |
|  |   MMR Indexer Worker  | ---------------------------------> |   In-Memory MMR    |  |
|  |  (Single Writer Task) |                                    |    Accumulator     |  |
|  +-----------------------+                                    |  (Arc<RwLock<MMR>>) |  |
|             |                                                 +--------------------+  |
|      Batch Append & Snapshot                                             ^            |
|             v                                                            |            |
|  +-----------------------+                                         Read Lock          |
|  | SQLite Persistent DB  |                                           (O(log N))       |
|  | (SQLCipher Checkpoint)|                                               |            |
|  +-----------------------+                                               |            |
|             ^                                                            |            |
|             | Point Lookup Metadata (block_no, tx_hash)                  |            |
|             +----------------------------+                               |            |
|                                          |                               |            |
|                               +----------------------------------------------------+  |
|                               |       Axum HTTP Serving Layer (Multi-Reader)       |  |
|                               |           GET /api/v1/mmr/proof/:leaf_index        |  |
|                               +----------------------------------------------------+  |
+---------------------------------------------------------------------------------------+
                                           ^
                                           | Inclusion Proof Request (<5ms)
                                           |
                               +-----------------------+
                               |   Client Wallet SDK   |
                               +-----------------------+
```

### 1. In-Memory Persistent State di `AppState`
`AppState` diperluas dengan field `in_memory_mmr`:
```rust
pub struct AppState {
    pub db: Database,
    pub in_memory_mmr: Arc<tokio::sync::RwLock<MerkleMountainRange>>,
    // ... field lainnya ...
}
```
- **Lifecycle Startup:** Saat booting server (`main.rs`), relayer memuat database **hanya 1 kali** (`load_mmr_tree()`), menginisialisasi `in_memory_mmr`, lalu menyimpannya dalam `Arc<RwLock<...>>`.
- **Serving Path:** Endpoint `handle_mmr_proof` memperoleh *read-lock* dari RAM:
  `let proof = state.in_memory_mmr.read().await.generate_proof(leaf_index as usize);`
  Zero database table scans! Zero Poseidon recalculation!

### 2. Inkremental Indexer Ingestion (Single-Writer Pattern)
Worker `mmr_indexer` tidak lagi memanggil `load_mmr_tree()`. 
Ketika batch event ditemukan on-chain:
1. Simpan commitmen ke database via `batch_insert_mmr_leaves`.
2. Peroleh *write-lock* pada `in_memory_mmr`.
3. Panggil `mmr.append_leaf(fr)` secara inkremental ($O(1)$ amortized / $O(\log N)$).
4. Simpan checkpoint status via `save_mmr_state(&mmr, last_block)`.

### 3. Frontier Checkpoint di SQLite ($O(1)$ Cold Startup)
Di tabel `mmr_state`, simpan snapshot active peaks dan leaf count.
Saat startup: jika snapshot valid dan `leaf_count` cocok dengan baris di tabel `mmr_leaves`, relayer dapat memuat state peaks dalam $O(\log N)$ operasi tanpa mem-parse seluruh leaf dari awal.

### 4. Client Scanning Optimization (1-Byte View Tag)
Menambahkan 1-byte *View Tag* pada format payload encrypted memo di event log (atau menyematkannya pada header byte plaintext note):
$$\text{ViewTag} = \text{HKDF}(\text{shared\_secret}, \text{"zeltra\_view\_tag"})[0]$$
- Wallet SDK membandingkan `memo[0] == expected_view_tag`.
- Mengeliminasi $255/256$ ($99.6\%$) non-owned memo secara instan tanpa memanggil komputasi ChaCha20Poly1305.

---

## 4. Invariant Mutlak (Enforced Invariants)

- **INV-MMR-1 (Reorg & Parity Invariant):** Bagged root pada `in_memory_mmr` harus selalu identik dengan on-chain root di smart contract Stylus pada block yang bersesuaian (`bagged_root == on_chain_note_root`).
- **INV-MMR-2 (Zero Proof Mutability):** `generate_proof` pada in-memory state tidak boleh mengubah state accumulator (`&self`).
- **INV-MMR-3 (Lock Starvation Protection):** Read-lock pada `in_memory_mmr` di Axum handler harus dirilis sesegera mungkin setelah proof di-clone ke DTO response (rentang lock $< 100\,\mu\text{s}$).

---

## 5. Rencana Migrasi & Implementasi

1. **Fase 1 (In-Memory Integration):**
   - Tambahkan `Arc<RwLock<MerkleMountainRange>>` ke `AppState`.
   - Update `zeltra-node/src/handlers/mmr.rs` agar melayani proof murni dari in-memory state.
   - Update `zeltra-node/src/mmr_indexer.rs` untuk menggunakan inkremental `append_leaf`.
2. **Fase 2 (Verification):**
   - Jalankan `scripts/benchmark_mmr_bottleneck.py` untuk mengonfirmasi latensi proof turun ke target $< 5\text{ ms}$.
   - Verifikasi seluruh rangkaian tes unit dan interop Python tetap 100% pass (nol regresi).
3. **Fase 3 (SDK View Tag):**
   - Integrasikan 1-byte View Tag pada modul `zeltra-sdk` untuk akselerasi scanning light client.
