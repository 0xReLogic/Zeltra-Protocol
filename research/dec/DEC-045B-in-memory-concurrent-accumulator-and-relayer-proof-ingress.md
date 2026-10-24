# DEC-045B: In-Memory Concurrent Accumulator, Incremental Indexer & Zero-Latency Proof Ingress

- **Status:** APPROVED AS ARCHITECTURAL SUB-SPECIFICATION (Child of [`DEC-045`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-045-in-memory-incremental-mmr-engine-and-relayer-proof-acceleration.md))
- **Author:** Zeltra Protocol Architecture & Systems Engineering Team
- **Date:** 2026-10-19
- **Domain:** `zeltra-node/src/state.rs`, `handlers/mmr.rs`, `mmr_indexer.rs`, `database/mmr.rs`
- **Security Level:** CRITICAL (Proof Ingress Availability, DoS Immunity, Single-Writer Thread Safety, Zero Reorg Drift)
- **Security Audit Resolusi:** AUD-15 (Relayer CPU Starvation / Proof Ingress DoS), AUD-16 (MMR Indexer Head Desync), AUD-17 (Mempool Proof Invalidation under Reorg)
- **Architectural Paradigm:** *"Single-Writer Asynchronous Ingestion, Sub-Microsecond Multi-Reader Proof Serving, Storage Engine Agnostic"*
- **Parent & Related DECs:**
  - [`DEC-045: Master In-Memory Incremental MMR Engine Blueprint`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-045-in-memory-incremental-mmr-engine-and-relayer-proof-acceleration.md)
  - [`DEC-045A: Flat Post-Order MMR Array & Zero-Hash Proof Engine`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-045A-flat-postorder-mmr-array-and-zero-hash-proof-engine.md)
  - [`DEC-035C: Relayer MMR Indexer Worker and Client Sync Architecture`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-035C-relayer-mmr-indexer-worker-and-client-sync-architecture.md)
  - [`DEC-038C: Relayer Sequencer Architecture`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-038C-relayer-sequencer-sdk-direct-deposit-and-cli.md)
  - [`DEC-043C: Relayer Dual-Memo Ingress & SDK In-Pool Wallet`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-043C-relayer-dual-memo-ingress-and-sdk-in-pool-wallet.md)

---

## 1. Executive Summary & Dilema Konkurensi Relayer Ingress

Dalam arsitektur awal `zeltra-node` (DEC-035C), relayer bersifat stateless murni terhadap akumulator Merkle Mountain Range:
1. Setiap kueri bukti `GET /api/v1/mmr/proof/:leaf_index` memicu pembongkaran database SQLite SQLCipher terenkripsi (`load_mmr_tree()`) dan kalkulasi ulang seluruh pohon dari daun 0.
2. Background worker `mmr_indexer` juga memanggil `load_mmr_tree()` dari awal setiap kali mendeteksi ada event note baru di Arbitrum L2.
3. Berdasarkan data empiris pada DEC-045, waktu serving proof di bawah konkurensi 20 request paralel melonjak hingga **p50 = 12.383 ms (12,3 detik)** dan throughput mentok di **0,79 RPS**.

**Solusi DEC-045B:** Menerapkan **In-Memory Concurrent State Engine** di dalam `AppState`, memisahkan jalur baca (*multi-reader serving*) dan jalur tulis (*single-writer ingestion*), serta mengabstraksi lapisan penyimpanan melalui trait `MMRStore` (mengikuti preseden Nervos CKB / Substrate) agar siap bermigrasi mulus ke RocksDB atau memory-mapped file (`mmap`) di masa mendatang.

---

## 2. Model Konkurensi & Struktur `AppState`

### A. Pola Single-Writer, Multi-Reader (SWMR)
Untuk menjamin konsistensi absolut dan eliminasi race condition:
- **Jalur Tulis (Single-Writer):** Hanya satu task background (`mmr_indexer_worker`) yang memiliki otoritas memodifikasi state MMR (memperoleh *write-lock*).
- **Jalur Baca (Multi-Reader):** Ribuan request HTTP Axum yang masuk ke `handle_mmr_proof` dapat memperoleh *read-lock* secara konkuren tanpa saling memblokir.

```rust
use std::sync::Arc;
use tokio::sync::RwLock;
use zeltra_core::FlatMerkleMountainRange;

#[derive(Clone)]
pub struct AppState {
    /// Database persisten SQLite (SQLCipher encrypted) sebagai append-only log.
    pub db: Database,

    /// In-memory accumulator MMR mirror untuk serving proof berlatensi sub-milidetik.
    pub in_memory_mmr: Arc<RwLock<FlatMerkleMountainRange>>,

    // Field state lainnya...
}
```

> [!NOTE]
> Pemanfaatan `tokio::sync::RwLock` menjamin bahwa pembacaan bukti inklusi dari memori tidak memblokir Tokio async runtime. Karena pemanggilan `generate_proof()` pada `FlatMerkleMountainRange` (DEC-045A) tidak melakukan hash Poseidon (0 hash), durasi kepemilikan *read-lock* sangat singkat: **$< 5\,\mu\text{s}$**.

---

## 3. Alur Cold Startup & Bootstrap Relayer

Saat server relayer pertama kali dinyalakan (`zeltra-node/src/main.rs`):
1. **Pemeriksaan Checkpoint:** Kueri tabel `mmr_state` untuk mendapatkan `leaf_count`, `bagged_root`, dan `last_indexed_block`.
2. **Rekonstruksi Satu Kali (Cold Boot):**
   - Jika database memiliki data, panggil `state.db.load_flat_mmr_tree().await`.
   - Bangun struct `FlatMerkleMountainRange` di RAM satu kali saja saat inisialisasi server.
3. **Penyimpanan ke `AppState`:** Masukkan instance yang telah terisi ke dalam `Arc<RwLock<FlatMerkleMountainRange>>`.
4. **Jaminan Kesiapan:** Endpoint HTTP dan worker background baru mulai memproses trafik setelah pohon di memori dinyatakan 100% tersinkronisasi (*ready probe status = 200 OK*).

---

## 4. Refaktor Endpoint `handle_mmr_proof` (Read-Path $< 20\,\mu\text{s}$)

Jalur pembacaan bukti pada `zeltra-node/src/handlers/mmr.rs` dirombak total:

```rust
pub async fn handle_mmr_proof(
    State(state): State<AppState>,
    Path(leaf_index): Path<u64>,
) -> Response {
    // 1. Ekstraksi proof langsung dari in-memory state via read-lock (Zero Disk I/O & Zero Poseidon Hash)
    let (proof, leaf_count, bagged_root) = {
        let tree = state.in_memory_mmr.read().await;
        let count = tree.leaf_count as u64;

        // INV-4: Upper Bound Check (Hyperbridge exploit defense)
        if count == 0 || leaf_index >= count {
            return (
                StatusCode::NOT_FOUND,
                Json(MmrErrorResponse {
                    code: "LeafIndexOutOfBounds".to_string(),
                    message: format!("leaf_index {} exceeds current leaf_count {}", leaf_index, count),
                }),
            ).into_response();
        }

        let p = match tree.generate_proof(leaf_index as usize) {
            Ok(proof) => proof,
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(MmrErrorResponse {
                        code: "ProofGenerationError".to_string(),
                        message: e.to_string(),
                    }),
                ).into_response();
            }
        };

        let root_hex = format!("0x{}", hex::encode(zeltra_core::fr_to_be_bytes(&tree.get_root())));
        (p, count, root_hex)
        // Read-lock otomatis lepas di sini (< 5 µs)
    };

    // 2. Kueri metadata pendukung (commitment, block_number) via point lookup SQLite
    let leaf_record = match state.db.get_mmr_leaf(leaf_index).await {
        Ok(Some(r)) => r,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(MmrErrorResponse {
                    code: "LeafNotFound".to_string(),
                    message: format!("leaf_index {} record not found in storage", leaf_index),
                }),
            ).into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(MmrErrorResponse {
                    code: "InternalStorageError".to_string(),
                    message: e.to_string(),
                }),
            ).into_response();
        }
    };

    // 3. Format DTO respons JSON
    let mountain_siblings: Vec<String> = proof
        .mountain_siblings
        .iter()
        .map(|s| format!("0x{}", hex::encode(zeltra_core::fr_to_be_bytes(s))))
        .collect();

    let peak_bagging_siblings: Vec<String> = proof
        .peak_bagging_siblings
        .iter()
        .map(|s| format!("0x{}", hex::encode(zeltra_core::fr_to_be_bytes(s))))
        .collect();

    (
        StatusCode::OK,
        Json(MmrProofResponse {
            leaf_index,
            leaf_count,
            commitment: leaf_record.commitment,
            mountain_height: proof.mountain_height,
            mountain_siblings,
            peak_bagging_siblings,
            bagged_root,
            block_number: leaf_record.block_number,
        }),
    ).into_response()
}
```

---

## 5. Refaktor Background Worker `mmr_indexer` (Single-Writer Ingestion)

Worker `mmr_indexer` di `zeltra-node/src/mmr_indexer.rs` mengonsumsi event on-chain secara inkremental tanpa memanggil `load_mmr_tree()`:

```rust
// Ketika batch event baru diterima dari blockchain L2:
if !events.is_empty() {
    // 1. Simpan commitmen ke database SQLite dalam 1 transaksi ACID (batch insert)
    let batch_data: Vec<_> = events.iter().map(|ev| {
        (ev.leaf_index, ev.commitment_hex.clone(), ev.tx_hash.clone(), ev.block_number, ev.encrypted_memo.clone())
    }).collect();
    state.db.batch_insert_mmr_leaves(batch_data).await?;

    // 2. Peroleh write-lock dan append secara inkremental ke memori RAM (~500 µs per leaf)
    {
        let mut tree = state.in_memory_mmr.write().await;
        for ev in &events {
            if ev.leaf_index >= tree.leaf_count as u64 {
                let fr = zeltra_core::from_evm_scalar(&ev.commitment_bytes).expect("Canonical scalar");
                tree.append(fr);
            }
        }
        // Simpan snapshot status checkpoint ke SQLite
        state.db.save_mmr_state(&tree, to_block).await?;
    }
}
```

---

## 6. Abstraksi Trait `MMRStore` (Persiapan Migrasi RocksDB / LMDB)

Agar codebase `zeltra-node` tidak terkunci (*vendor lock-in*) pada SQLite, kita mendefinisikan interface penyimpanan generik yang mengikuti arsitektur CKB / Substrate:

```rust
#[async_trait::async_trait]
pub trait MMRStore: Send + Sync {
    /// Mengambil node hash pada posisi pos tertentu
    async fn get_node(&self, pos: u64) -> anyhow::Result<Option<Fr>>;
    
    /// Menyimpan sekumpulan node baru hasil operasi append
    async fn append_nodes(&self, start_pos: u64, nodes: &[Fr]) -> anyhow::Result<()>;
    
    /// Menyimpan status snapshot checkpoint
    async fn save_checkpoint(&self, leaf_count: u64, bagged_root: &str, block_number: u64) -> anyhow::Result<()>;
}
```
* **Fase Saat Ini:** Diimplementasikan oleh struct `SqliteMmrStore` (memakai SQLite + SQLCipher).
* **Fase Masa Depan:** Jika throughput melebihi 10 juta leaf, tim dapat mengimplementasikan `RocksDbMmrStore` atau `MmapMmrStore` tanpa mengubah 1 baris pun kode pada Axum handler atau worker indexer.

---

## 7. Reorg Safety & Rollback Handling

Arbitrum Nitro menggunakan deterministic sequencer dengan reorg yang sangat jarang terjadi. Namun, untuk menjaga integritas di level safe head:
1. Indexer selalu beroperasi pada `safe_block = current_block - (conf_threshold - 1)` (INV-1).
2. Jika terdeteksi hash block on-chain berubah pada rentang blok konfirmasi:
   - Worker indexer mengunci write-lock `in_memory_mmr`.
   - Mengambil checkpoint terakhir yang valid dari database sebelum reorg.
   - Mereset flat array memori ke state snapshot tersebut.

---

## 8. Invariant Sistem (Enforced Invariants)

- **INV-45B-1 (Lock Non-Blocking Invariant):** Axum handler tidak boleh melakukan I/O disk atau operasi jaringan saat memegang read-lock `in_memory_mmr`.
- **INV-45B-2 (Single Writer Exclusivity):** Hanya task indexer yang diizinkan memanggil `.write().await` pada `in_memory_mmr`.
- **INV-45B-3 (Parity with On-Chain Root):** Root pada `in_memory_mmr` wajib cocok dengan `note_tree_root` kontrak Stylus pada checkpoint block yang sama.
- **INV-45B-4 (State Immutability on Read Failure):** Setiap kegagalan pada endpoint kueri bukti tidak boleh memodifikasi memori RAM atau database SQLite.

---

## 9. Matriks Pengujian Unit & Soundness (`zeltra-node`)

Sesuai aturan engineering protokol Zeltra, implementasi DEC-045B wajib disertai **Positive Tests + Minimal 2x Negative Tests (Minimal 2x Negative = Gagal Harus Membuktikan State Tidak Berubah)**:

### A. Positive Tests (3 Tests):
1. `test_relayer_handle_mmr_proof_cached_success`:
   - Endpoint `handle_mmr_proof` mengembalikan status `200 OK` dengan payload JSON `MmrProofResponse`.
   - Waktu respon di bawah konkurensi terbukti $< 100\,\mu\text{s}$ (zero database scan).
2. `test_relayer_mmr_indexer_incremental_append_success`:
   - Worker indexer menerima batch 50 event note on-chain.
   - Melakukan `batch_insert_mmr_leaves` ke SQLite dan append inkremental ke `in_memory_mmr`.
   - Membuktikan bahwa `leaf_count` bertambah tepat 50 dan root memori sinkron dengan root database.
3. `test_relayer_cold_startup_restores_canonical_state`:
   - Mengisi database dengan $N=500$ leaf, lalu mematikan server.
   - Booting ulang server relayer: `AppState::new()` memuat seluruh pohon ke memori dan memverifikasi root cocok dengan checkpoint tanpa drift.

### B. Negative Tests (7 Tests — >2x Rasio Pengujian):
1. `test_relayer_handle_mmr_proof_rejects_out_of_bounds`:
   - Mengirim request `GET /api/v1/mmr/proof/:leaf_index` dengan `leaf_index >= leaf_count`.
   - Handler mengembalikan `404 Not Found` dengan kode `LeafIndexOutOfBounds`. State RAM dan DB tidak berubah.
2. `test_relayer_handle_mmr_proof_rejects_empty_tree`:
   - Mengirim request saat pohon masih kosong (`leaf_count == 0`).
   - Handler mengembalikan `404 Not Found` dengan kode `LeafIndexOutOfBounds`.
3. `test_relayer_handle_mmr_proof_rejects_malformed_index_path`:
   - Mengirim request dengan parameter string non-angka (`/proof/abc` atau `/proof/-1`).
   - Handler Axum menolak pada lapisan routing (`400 Bad Request` atau `404 Not Found`).
4. `test_relayer_mmr_indexer_skips_duplicate_leaves`:
   - Indexer mendeteksi log event on-chain dengan `leaf_index < current_leaf_count`.
   - Indexer mencatat peringatan dan mengabaikan event tersebut tanpa menyisipkan duplikat ke RAM maupun DB.
5. `test_relayer_mmr_indexer_rejects_non_contiguous_leaf_gap`:
   - Indexer menerima event dengan index melompat (misal leaf 10 diterima saat posisi masih leaf 5).
   - Indexer me-revert transaksi dan fail-closed (`anyhow::bail!`) untuk mencegah fragmentasi pohon.
6. `test_relayer_mmr_indexer_recovers_from_shallow_reorg`:
   - Mensimulasikan reorg 2 blok pada Arbitrum L2.
   - Indexer mendeteksi desync hash blok dan memulihkan state memori kembali ke checkpoint sebelum reorg.
7. `test_relayer_concurrency_stress_zero_deadlock`:
   - Menjalankan 50 worker task Axum yang meminta proof secara paralel tanpa henti selama 5 detik, sementara worker indexer melakukan append daun baru.
   - Mengonfirmasi **0 deadlock, 0 panic, dan 0 starvation** pada `RwLock`.
