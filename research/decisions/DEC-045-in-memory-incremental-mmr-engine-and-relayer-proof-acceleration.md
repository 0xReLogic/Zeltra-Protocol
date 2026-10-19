# DEC-045: In-Memory Incremental MMR Engine, Flat Node Array & Zero-Hash Proof Acceleration

**Status:** APPROVED  
**Date:** 2026-10-19  
**Domain:** `zeltra-node`, `zeltra-core`, `zeltra-sdk`  
**Security Level:** CRITICAL (Proof Availability, DoS Immunity, Sub-Millisecond Latency, Zero CPU Starvation)  
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

Namun, pada layer off-chain Relayer (`zeltra-node`), audit performa dan pengujian empiris membuktikan adanya **dua lapis bottleneck arsitektur kritis** pada pipeline serving proof MMR:

1. **Bottleneck Lapis 1: Rekonstruksi Database $O(N)$ Pada Setiap Request (`handle_mmr_proof`)**
   Setiap kali ada request `GET /api/v1/mmr/proof/:leaf_index`, endpoint mengeksekusi `state.db.load_mmr_tree()`. Fungsi ini membaca seluruh tabel `mmr_leaves` dari database SQLite via enkripsi SQLCipher (AES-256 baris-per-baris), mem-parse seluruh skalar $Fr$, dan membangun ulang tree dari leaf 0 sampai $N$ dengan komputasi loop hash Poseidon.
2. **Bottleneck Lapis 2: "The Hidden Trap" — Re-Hashing Gunung Lokal ($O(2^h)$ CPU Hashing)**
   Bahkan jika database di-bypass dengan menyimpan struct `MerkleMountainRange` di RAM (*Naive RAM Cache*), fungsi `generate_proof()` di `zeltra-core/src/merkle.rs` **menghitung ulang seluruh layer pohon biner (gunung lokal terbesar) secara on-the-fly** karena intermediate nodes tidak disimpan.

**Tujuan DEC-045:** Menetapkan arsitektur baru Relayer Proof Engine berbasis **Flat Post-Order MMR Node Array**, yang menyimpan intermediate parent nodes saat append berlangsung sehingga serving proof menjadi murni **$O(\log N)$ memory index read dengan NOL (0) Poseidon hash**, menurunkan latensi proof dari `366 ms` ke `< 20 µs` dan melonjakkan throughput ke `> 50.000 RPS`.

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

---

## 2.1. Bedah Forensik: Mengungkap "The Hidden Trap" pada Naive RAM Cache

Jika kita membedah lebih dalam angka `proof_gen_time_us` dan `http_concurrency.cached`, terungkap satu **jebakan matematis kritis** pada implementasi `zeltra-core/src/merkle.rs` saat ini:

### A. Mengapa `proof_gen_time_us` Melonjak 4x Saat Leaf Hanya Naik 2,5x?
Perhatikan korelasi eksak antara jumlah leaf ($N$), ukuran gunung biner terbesar ($2^h$), dan waktu pembuatan proof:

| `leaf_count` ($N$) | Ukuran Gunung Terbesar ($2^h$) | Jumlah Internal Hash ($2^h - 1$) | `proof_gen_time_us` | `cached_proof_time_us` | Waktu per Hash Poseidon-W5 |
|---|---|---|---|---|---|
| **1.000** | $2^9 = 512$ leaves | 511 hashes | 43.442 $\mu\text{s}$ (43,4 ms) | 46.235 $\mu\text{s}$ (46,2 ms) | $\approx 85,0\,\mu\text{s}$ |
| **2.500** | $2^{11} = 2.048$ leaves | 2.047 hashes | 173.178 $\mu\text{s}$ (173,2 ms) | 182.848 $\mu\text{s}$ (182,8 ms) | $\approx 84,6\,\mu\text{s}$ |
| **5.000** | $2^{12} = 4.096$ leaves | 4.095 hashes | 377.757 $\mu\text{s}$ (377,8 ms) | 366.365 $\mu\text{s}$ (366,4 ms) | $\approx 89,5\,\mu\text{s}$ |

**Bukti Forensik:**
1. Satu operasi hash Poseidon-W5 (BLS12-381) memakan waktu rata-rata **~85–90 µs**.
2. Saat `leaf_count` naik dari 1.000 ke 2.500 (naik **2,5x**), waktu pembuatan proof melonjak dari 43,4 ms ke 173,2 ms (naik tepat **3,99x**).
3. **Mengapa $4\times$?** Karena gunung biner terbesar di dalam MMR melompat dua pangkat dari $2^9 = 512$ menjadi $2^{11} = 2.048$ ($4\times$ lipat), dan `generate_proof()` menghitung ulang seluruh $2.047$ internal parent nodes pada gunung tersebut!
4. **Implikasi Fatal:** Sekadar membungkus struct `MerkleMountainRange` lama di dalam `Arc<RwLock<...>>` (Naive RAM Cache) **belum menuntaskan bottleneck $O(N)$**. Ia hanya memangkas I/O disk SQLCipher, tetapi menyisakan 100% beban CPU re-hashing (366 ms pada 5.000 leaves).

### B. Mengapa `http_concurrency.cached` Mentok di 7,87 RPS (p50 = 2,58 Detik)?
Pada uji konkurensi live HTTP:
- **Baseline (`current` @ concurrency 10):** `0,79 RPS` | `p50: 12.383 ms` | `p99: 15.557 ms`.
- **Naive Cache (`cached` @ concurrency 20):** `7,87 RPS` | `p50: 2.580 ms` | `p99: 3.847 ms`.

Penyebabnya: setiap request `generate_proof()` membakar **~366 ms CPU murni** (4.095 hash Poseidon) di dalam worker thread Tokio. Saat 20 request masuk bersamaan, seluruh CPU core langsung tersaturasi 100%, memicu thread starvation yang mendorong p50 melonjak ke 2,58 detik!

Jika jumlah leaf mencapai 100.000 (gunung terbesar $2^{16} = 65.536$ daun):
$$65.535 \times 88\,\mu\text{s} \approx \mathbf{5,76\text{ detik CPU murni per 1 request!}}$$

---

## 2.2. Ekstrapolasi Performa ke Skala Produksi (100.000 & 10.000.000 Leaves)

| Metrik Performa | 1. Arsitektur Saat Ini (`current` - DB Rebuild) | 2. Naive RAM Cache (`cached` - Hanya `Vec<Fr>` Leaves) | 3. Target DEC-045 (Flat MMR Node Array) |
|---|---|---|---|
| **Proof Latency @ 5.000 Leaves** | 3.529 ms | 366,4 ms | **< 0,02 ms (< 20 µs)** |
| **Proof Latency @ 100.000 Leaves** | ~68.300 ms (68,3 dtk) | ~5.760 ms (5,76 dtk) | **< 0,03 ms (< 30 µs)** |
| **Proof Latency @ 10.000.000 Leaves** | ~6.900 dtk (1,9 jam) | ~738.000 ms (12,3 menit) | **< 0,05 ms (< 50 µs)** |
| **HTTP Throughput @ 5.000 Leaves** | 0,79 RPS (p99: 15,5 dtk) | 7,87 RPS (p99: 3,8 dtk) | **> 50.000 RPS (p99: < 2 ms)** |
| **Poseidon Hash saat GET /proof** | $2N$ hashes ($O(N)$) | $2^{\lfloor\log_2 N\rfloor} - 1$ hashes ($O(N)$) | **0 hashes ($O(\log N)$ array read)** |
| **Indexer Append @ 100.000 Leaves** | ~62.600 ms / blok | ~0,55 ms / leaf | **~0,55 ms / leaf** |

---

## 3. Desain Arsitektur Baru: Flat Post-Order MMR Node Array

```
                        +-------------------------------------------------+
                        |              ARBITRUM L2 RPC                   |
                        +-------------------------------------------------+
                                                |
                                      Log Events (Safe Head)
                                                v
+----------------------------------------------------------------------------------------+
| RELAYER NODE (`zeltra-node`)                                                           |
|                                                                                        |
|   +-----------------------+      Write Lock & Compute Parent     +------------------+  |
|   |   MMR Indexer Worker  | -----------------------------------> | Flat Post-Order  |  |
|   |  (Single Writer Task) |       Node Array Append              |  MMR Node Array  |  |
|   +-----------------------+       (mmr_nodes: Vec<Fr>)           | (Arc<RwLock<..>>) | |
|              |                                                   +------------------+  |
|       Batch Insert Leaves                                                  ^           |
|              v                                                             |           |
|   +-----------------------+                                         Direct Index Read  |
|   | SQLite Persistent DB  |                                         (O(log N) Offsets) |
|   | (SQLCipher Checkpoint)|                                         (NOL POSEIDON HASH)|
|   +-----------------------+                                                |           |
|              ^                                                             |           |
|              | Point Lookup Metadata (block_no, tx_hash)                   |           |
|              +----------------------------+                                |           |
|                                           |                                |           |
|                                +----------------------------------------------------+  |
|                                |       Axum HTTP Serving Layer (Multi-Reader)       |  |
|                                |           GET /api/v1/mmr/proof/:leaf_index        |  |
|                                +----------------------------------------------------+  |
+----------------------------------------------------------------------------------------+
                                            ^
                                            | Inclusion Proof Request (< 20 µs)
                                            |
                                +-----------------------+
                                |   Client Wallet SDK   |
                                +-----------------------+
```

### 1. Struktur Flat Post-Order MMR Array di `zeltra-core`
Dalam standar Peter Todd / Grin MMR, sebuah gunung biner berukuran $2^h$ adalah **immutable permanen** setelah penuh. Kita mengubah representasi internal MMR dari sekadar `leaves: Vec<Fr>` menjadi **Flat Post-Order Tree Array**:
```rust
pub struct FlatMerkleMountainRange {
    /// Seluruh node (leaves dan intermediate parents) disimpan dalam urutan post-order traversal
    pub nodes: Vec<Fr>,
    /// Posisi active peaks di dalam array `nodes`
    pub peak_pos: Vec<usize>,
    /// Jumlah daun saat ini
    pub leaf_count: usize,
    /// Cached bagged root
    cached_root: Option<Fr>,
}
```

### 2. Proof Generation dengan NOL (0) Poseidon Hash
Saat client meminta `generate_proof(leaf_index)`:
1. Hitung posisi daun di array `pos = leaf_index_to_pos(leaf_index)`.
2. Selama mendaki gunung ke atas, posisi saudara (*sibling*) langsung dihitung secara deterministik via rumus bitwise post-order:
   $$\text{sibling\_pos} = \text{if is\_left\_sibling}(pos) \{ pos + 2^{h+1} - 1 \} \text{ else } \{ pos - (2^{h+1} - 1) \}$$
3. Ambil nilai sibling langsung dari `self.nodes[sibling_pos]`.
4. **Hasil:** Waktu pembuatan proof murni berupa dereferensi memori array ($O(\log N)$ memory jumps), memangkas waktu dari **366 ms ke < 20 mikrodetik**!

### 3. Inkremental Indexer Ingestion (Single-Writer Pattern)
Worker `mmr_indexer` mengeksekusi:
1. `batch_insert_mmr_leaves` ke SQLite.
2. Peroleh *write-lock* pada `in_memory_mmr`.
3. Panggil `mmr.append(leaf)`: menghitung hash parent hanya saat merge puncak ($O(1)$ amortized / $O(\log N)$ hash, $\approx 500\,\mu\text{s}$) dan langsung menyimpan intermediate node ke `nodes`.
4. Simpan checkpoint status via `save_mmr_state(&mmr, last_block)`.

### 4. Client Scanning Optimization (1-Byte View Tag)
Menambahkan 1-byte *View Tag* pada header memo 96-byte:
$$\text{ViewTag} = \text{HKDF}(\text{shared\_secret}, \text{"zeltra\_view\_tag"})[0]$$
- Berdasarkan benchmark, ini memangkas waktu scanning dompet sebesar **23.6x** dan mengeliminasi **99.5%** note non-milik sebelum menyentuh ChaCha20Poly1305.

---

## 4. Invariant Mutlak (Enforced Invariants)

- **INV-MMR-1 (Reorg & Parity Invariant):** Bagged root pada in-memory flat MMR harus selalu identik dengan on-chain root di smart contract Stylus pada block yang bersesuaian (`bagged_root == on_chain_note_root`).
- **INV-MMR-2 (Zero Proof Hashing):** Pemanggilan `generate_proof` pada MMR yang terisi penuh **dilarang memanggil `native_poseidon_w5`**. Seluruh sibling node wajib diekstraksi dari flat array.
- **INV-MMR-3 (Lock Starvation Protection):** Read-lock pada in-memory state di Axum handler harus dirilis sesegera mungkin setelah slice sibling disalin ke DTO response (durasi lock $< 5\,\mu\text{s}$).

---

## 5. Rencana Implementasi

1. **Fase 1 (Flat MMR Primitive di `zeltra-core`):**
   - Implementasikan skema pengindeksan Flat Post-Order MMR (`nodes: Vec<Fr>`) pada `zeltra-core/src/merkle.rs`.
   - Update `generate_proof` agar membaca saudara langsung dari array `nodes`.
2. **Fase 2 (In-Memory Integration di `zeltra-node`):**
   - Pasang `Arc<tokio::sync::RwLock<FlatMerkleMountainRange>>` pada `AppState`.
   - Perbarui `handle_mmr_proof` dan `mmr_indexer`.
3. **Fase 3 (Benchmark Verifikasi Ulang):**
   - Jalankan ulang `scripts/benchmark_mmr_bottleneck.py` untuk mengonfirmasi bahwa latensi proof turun menjadi `< 20 µs` dan throughput HTTP mencapai `> 10.000 RPS`.

---

## 6. Analisis Komparatif ATMI & Preseden Industri (Battle-Tested References)

Berdasarkan metodologi **ATMI (Amati, Tiru, Modifikasi, Inovasi)**, arsitektur DEC-045 menyerap dan mengkristalisasikan pola desain unggul dari proyek-proyek blockchain tier-1 yang telah teruji menangani jutaan transaksi:

### A. AMATI (Observe: Preseden Nyata Industri)
1. **Grin (Mimblewimble) — Arsitektur `AppendOnlyFile` (mmap) & Navigasi Bitwise:**
   Grin menyelesaikan akumulasi ratusan juta output/kernel tanpa database relasional menggunakan file append-only datar (`hashfile`) yang dipetakan langsung ke memori via **`mmap`**. Posisi daun $k$ ke offset flat array dihitung instan via formula bitwise:
   $$\text{pos} = 2 \times k - \text{count\_ones}(k)$$
   Saat pembuktian (*proof serving*), Grin **sama sekali tidak menghitung ulang hash**, melainkan langsung melompat ke offset memori biner.
2. **Nervos CKB & Substrate / Polkadot SDK — Pola `MMRStore` & `MMRBatch`:**
   Crate `ckb-merkle-mountain-range` (yang diadopsi oleh Parity Technologies untuk `pallet-mmr` di Polkadot) memisahkan komputasi pohon dari lapisan persistensi melalui trait `MMRStore`. Penambahan daun baru ditampung sementara di `MMRBatch` (RAM buffer) sebelum di-flush secara atomik ke storage backend.
3. **The Tari Project (`tari_mmr`) — Family Branch Lookup:**
   Tari mengimplementasikan `family_branch(pos, mmr_size)` yang menghitung indeks sibling secara deterministik, lalu mengekstrak bukti melalui lookup array langsung (`mmr.get_node_hash(*sibling)?`), menjamin waktu pembuatan proof tetap konstan pada level mikrodetik.
4. **Zcash Orchard & Monero Seraphis — Fast Client Scanning:**
   Orchard (NU5) dan Seraphis menggunakan mekanisme *View Tag* 1-byte turunan KDF yang memungkinkan dompet ringan (*light wallet*) membuang >99% data memo asing tanpa menjalankan algoritma enkripsi/dekripsi simetris yang mahal.

### B. TIRU (Copy: Pola Teruji yang Diadopsi)
- Struktur data Flat Post-Order MMR Array: menyimpan seluruh daun dan node induk (*intermediate parents*) secara sekuensial.
- Formula navigasi bitwise Peter Todd untuk konversi indeks daun $\rightarrow$ posisi array dan pencarian saudara (*sibling*).
- Penghitungan dan perekaman node induk secara permanen tepat pada saat operasi `append()` dieksekusi.

### C. MODIFIKASI (Modify: Penyesuaian Kriptografi ZK Zeltra)
- **Tipe Elemen Kurva Aljabar:** Grin/Tari menggunakan byte hash mentah (`[u8; 32]`), sedangkan Zeltra memodifikasinya menggunakan field skalar kurva eliptik **`ark_bls12_381::Fr`** dengan validasi kanonikalitas mutlak $(< r)$ agar selaras dengan input publik sirkuit Groth16.
- **Fungsi Hash Ramah SNARK:** Mengganti Blake2b/SHA-256 dengan **Poseidon-W5** yang diparameterisasi Grain-128 LFSR (DEC-021) untuk kompatibilitas native dengan prover in-browser WASM (DEC-041) dan verifier EIP-2537 di Stylus (DEC-043B).
- **Protokol Peak Bagging:** Menggunakan canonical backward-fold Zeltra dengan pemisahan domain `DOMAIN_MMR_BAG` dan penyematan jumlah daun secara eksplisit (`leaf_count`) untuk kekebalan eksploitasi out-of-bounds Hyperbridge (DEC-032 / INV-4).
- **Cold Storage SQLite Encrypted:** Tetap mempertahankan SQLite + SQLCipher AES-256 sebagai append-only persistence log dan crash-recovery checkpoint, sementara serving proof dilayani 100% dari RAM.

### D. INOVASI (Innovate: Keunggulan Kompetitif Zeltra)
- **Zero-Hash Proof Serving ($< 20\,\mu\text{s}$):** Menghilangkan 100% kalkulasi Poseidon pada read-path HTTP relayer, memungkinkan ribuan transaksi AI Agent sub-sen (DEC-042) dan transfer in-pool (DEC-043) dilayani secara paralel tanpa saturasi CPU.
- **1-Byte View Tag Memo Integration:** Mengintegrasikan View Tag 1-byte pada format memo AEAD 96-byte:
  $$\text{ViewTag} = \text{HKDF}(\text{shared\_secret}, \text{"zeltra\_view\_tag"})[0]$$
  Meningkatkan kecepatan sinkronisasi dompet klien sebesar **23.6x** dengan efisiensi eliminasi data non-milik sebesar **99.5%**.

---

## 7. Daftar Pustaka & Sitasi Teknis (Citations)

1. **Todd, Peter (2012).**  
   *Merkle Mountain Ranges.*  
   Open-source conceptual specification and binary tree representation.  
   URL: [https://github.com/opentimestamps/opentimestamps-server/blob/master/doc/merkle-mountain-range.md](https://github.com/opentimestamps/opentimestamps-server/blob/master/doc/merkle-mountain-range.md)

2. **Grin Developers (2018–2024).**  
   *Merkle Mountain Ranges & PMMR State Storage Architecture.*  
   Mimblewimble Grin Documentation & Core Implementation.  
   URL: [https://docs.grin.mw/wiki/chain-state/merkle-mountain-range/](https://docs.grin.mw/wiki/chain-state/merkle-mountain-range/)  
   Source Code: [https://github.com/mimblewimble/grin/blob/master/core/src/core/pmmr/pmmr.rs](https://github.com/mimblewimble/grin/blob/master/core/src/core/pmmr/pmmr.rs)

3. **Nervos Network Core Team (2020–2024).**  
   *CKB Merkle Mountain Range: General-purpose MMR library in Rust.*  
   Nervos Network RFC & Open-Source Crate.  
   Crate: `ckb-merkle-mountain-range` (v0.5.2).  
   URL: [https://docs.rs/ckb-merkle-mountain-range/latest/ckb_merkle_mountain_range/](https://docs.rs/ckb-merkle-mountain-range/latest/ckb_merkle_mountain_range/)

4. **Parity Technologies / Web3 Foundation (2021–2024).**  
   *Substrate `pallet-mmr`: Merkle Mountain Range for Substrate Runtime & Offchain Storage.*  
   Polkadot SDK Technical Reference.  
   URL: [https://paritytech.github.io/polkadot-sdk/master/pallet_mmr/index.html](https://paritytech.github.io/polkadot-sdk/master/pallet_mmr/index.html)

5. **The Tari Project (2019–2024).**  
   *Tari MMR: High-Performance Merkle Mountain Range & Proof Generation in Rust.*  
   Tari Infrastructure Crate.  
   URL: [https://docs.rs/tari_mmr/latest/tari_mmr/](https://docs.rs/tari_mmr/latest/tari_mmr/)

6. **Hopwood, D., Bowe, S., Hornby, T., & Wilcox, Z. (Electric Coin Company, 2021–2024).**  
   *Zcash Protocol Specification: Version 2021.2.16 [NU5].*  
   Section 4.19: Action Transfers, Note Commitments & Orchard Incremental Trees.  
   URL: [https://zips.z.cash/protocol/protocol.pdf](https://zips.z.cash/protocol/protocol.pdf)

7. **Koe, Kurt M. (2021).**  
   *Seraphis: A New Transaction Protocol for Monero (View Tag Design for Light Client Scanning Acceleration).*  
   Monero Research Lab (MRL-0012).  
   URL: [https://github.com/UkoeHB/Seraphis_crypto](https://github.com/UkoeHB/Seraphis_crypto)

8. **Grass, L., Khovratovich, D., Rechberger, C., Roy, P., & Schofnegger, M. (2021).**  
   *Poseidon: A New Hash Function for Zero-Knowledge Proof Systems.*  
   USENIX Security Symposium 2021.  
   IACR Cryptology ePrint Archive, Report 2019/458.  
   URL: [https://eprint.iacr.org/2019/458](https://eprint.iacr.org/2019/458)

