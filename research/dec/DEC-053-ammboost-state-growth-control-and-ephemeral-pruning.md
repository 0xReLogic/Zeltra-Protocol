# DEC-053: ammBoost State Growth Control & Ephemeral Settlement Pruning — Bounding On-Chain Storage Bloat to O(1) via Ring-Buffer Eviction and Eliminating Unbounded MMR Root Accumulation

- **Status:** APPROVED ARCHITECTURAL SPECIFICATION
- **Tipe DEC:** OPTIMASI PERFORMA & SECURITY HARDENING (CONTRACT STORAGE & RELAYER)
- **Author:** Zeltra Protocol Architecture, Cryptography & Systems Security Team
- **Date:** 2026-10-26
- **Target Crates & Components:**
  - `zeltra-contracts/src/merkle.rs` (`_mmr_insert` ring-buffer eviction & `_is_accepted_note_root` bounded freshness window)
  - `zeltra-contracts/src/storage.rs` (Stylus `sol_storage!` layout documentation & bounded mapping invariant)
  - `zeltra-node/src/database.rs` (Relayer SQLite ephemeral spend receipt pruning post-on-chain confirmation)
- **Parent & Related DECs:**
  - [`DEC-032: Merkle Mountain Range Commitment Accumulator`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-032-merkle-mountain-range-commitment-accumulator.md)
  - [`DEC-033: Epoch-Windowed Nullifier Pruning & In-Flight Rollover`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-033-epoch-windowed-nullifier-pruning-in-flight-rollover.md)
  - [`DEC-038B: Contract Direct MMR Deposit & Storage Cleanup`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-038B-contract-direct-mmr-deposit-and-storage-cleanup.md)
- **Security Audit Targets:** `AUD-28` (Unbounded Contract Storage Leak via Accepted MMR Roots Mapping)
- **Scientific Foundation & Primary Citations:**
  - Nicolas Michel, Mohamed E. Najd, Ghada Almashaqbeh (University of Connecticut, 2024/2025): *"ammBoost: State Growth Control for AMMs"*, IEEE Transactions / IACR ePrint 2024/1021.

---

## 1. Executive Summary & Problem Formulation

### A. Konteks & Kajian Postmortem Paper (IACR ePrint 2024/1021)
Paper IACR 2024/1021 berjudul resmi **"ammBoost: State Growth Control for AMMs"** (oleh Nicolas Michel, Mohamed E. Najd, dan Ghada Almashaqbeh) meneliti masalah fundamental pada aplikasi Web3: **pertumbuhan state blockchain tak terbatas (*unbounded state growth*)**.

Dalam paper aslinya:
1. **Model Sistem:** Paper ini merancang arsitektur L2/sidechain untuk Automated Market Maker (AMM) dengan memisahkan transaksi menjadi *ephemeral meta-blocks* ($B_1^{\text{meta}}, \dots, B_n^{\text{meta}}$) dan *permanent summary-blocks*.
2. **Block Suppression Pruning:** Setelah transaksi ringkasan (`Sync`) terkonfirmasi di rantai utama (mainchain) via Threshold Signature Quorum Certificate (TSQC), seluruh *meta-blocks* pada epoch tersebut langsung di-prune dari riwayat aktif. Hasil empiris paper menunjukkan **penurunan pertumbuhan state sebesar 93.42%** dan **penghematan gas hingga 96.05%**.
3. **Peringatan Kritis Kriptografi (Anti-Halusinasi Wires):** Paper ini adalah paper arsitektur state L2 & pruning blockchain, **BUKAN paper sirkuit ZK-SNARK**. Paper ini memiliki **NOL (0) gerbang R1CS, NOL wires Groth16, dan NOL relasi polinomial sirkuit**. Menciptakan "wire sirkuit" palsu untuk ammBoost adalah kesalahan fatal.

### B. Problem Statement di Zeltra Protocol (`AUD-28`)
Meskipun Zeltra adalah protokol pembayaran ZK-UTXO murni (bukan AMM), prinsip *State Growth Control* dari `ammBoost` mengungkap celah kebocoran memori on-chain laten pada smart contract Stylus Zeltra:

Pada [`zeltra-contracts/src/merkle.rs`](file:///workspaces/Zeltra-Protocol/zeltra-contracts/src/merkle.rs):
```rust
// 1. Root baru disimpan ke mapping accepted_note_roots
let timestamp = U256::from(self.block_timestamp());
self.accepted_note_roots.insert(new_root_bytes, timestamp);

// 2. Root baru dicatat ke ring buffer history (size 100)
let hist_idx = self.root_history_index.get();
let slot = U256::from(hist_idx.to::<u64>() % (ROOT_HISTORY_SIZE as u64));
self.root_history.insert(slot, new_root_bytes);
self.root_history_index.set(hist_idx + U256::from(1));
```

**Titik Kelemahan Fatal (`AUD-28`):**
1. Array ring-buffer `root_history` memang dibatasi 100 slot (`ROOT_HISTORY_SIZE = 100`).
2. Namun, mapping `accepted_note_roots` **TIDAK PERNAH DI-PRUNE SAMA SEKALI**!
3. Setiap kali ada setoran (`deposit_direct_note`) atau pembelanjaan dengan change note (`spend_joinsplit`), root baru disisipkan ke `accepted_note_roots`.
4. Jika terdapat 1.000.000 transaksi, smart contract Stylus akan mengalokasikan **1.000.000 slot storage acak `SSTORE` ($O(N)$ memory leak)** di Arbitrum ArbOS.
5. Fungsi `_is_accepted_note_root` memeriksa `self.accepted_note_roots.get(root) > 0`, yang berarti root berusia 2 tahun lalu yang sudah terdorong keluar dari ring buffer 100 slot masih dianggap sah selamanya, melanggar prinsip bounded freshness window yang dirancang di [`DEC-032`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-032-merkle-mountain-range-commitment-accumulator.md).

---

## 2. Invariants & Batasan Non-Negotiable

1. **Multi-Asset Solvency Invariant (DEC-034B / DEC-038B):**
   `contract token balance >= total liabilities` mutlak terjaga; pembersihan state storage tidak boleh mengubah saldo kewajiban atau token riil.
2. **Strict O(1) Storage Bound Invariant:**
   Jumlah entri aktif di `accepted_note_roots` tidak boleh melebihi $K = \text{ROOT\_HISTORY\_SIZE} + 1$ (maksimal 101 entri: 1 root saat ini + 100 root historis).
3. **No-Zero-Root Pruning Invariant:**
   Slot kosong (`FixedBytes::ZERO`) atau root pohon kosong (`EMPTY_TREE_ROOT_BYTES`) tidak boleh menghapus root aktif yang sah.
4. **Zero Circuit Impact:**
   Tidak ada perubahan pada sirkuit ZK-UTXO `PrivateNoteCircuit` atau `JoinSplitCircuit`. Verifikasi ZK tetap membuktikan inklusi terhadap root publik `rt` yang sah.

---

## 3. Analisis Pilihan & Trade-Off Matrix

| Alternatif Pendekatan | Kelebihan | Kelemahan / Trade-Off Teknis | Keputusan Evaluasi |
|---|---|---|---|
| **Opsi A: Status Quo (Unbounded Mapping)** | Sederhana, tidak perlu eviksi | Alokasi storage $O(N)$ membengkak tanpa batas, biaya gas ArbOS validator naik permanen | **DITOLAK (`AUD-28`)** |
| **Opsi B: Linear Scan Ring Buffer (Hapus Mapping)** | Tidak perlu mapping sama sekali | Scan linear 100 slot di WASM memakan gas komputasi setiap kali spend verifikasi | **DITOLAK (Inefisien Gas)** |
| **Opsi C: ammBoost Eviction-on-Insert (O(1) Bounded Mapping)** | Storage $O(1)$ mutlak, lookup $O(1)$, evict root usang tepat saat slot ring-buffer ter-overwrite | Membutuhkan 1 read slot lama sebelum overwrite di `_mmr_insert` | **DIPILIH & DIRATIFIKASI** |

---

## 4. Spesifikasi Teknis & Detail Implementasi

### A. Mekanisme Eviksi Ring-Buffer di `_mmr_insert`
Mengadopsi paradigma *State Growth Control* dari paper IACR 2024/1021, saat `hist_idx >= ROOT_HISTORY_SIZE`, slot yang akan ditimpa menyimpan root lama `old_root = self.root_history.get(slot)`.

Jika `old_root` valid (bukan nol dan bukan root aktif terkini), kontrak langsung mem-prune slot tersebut dari `accepted_note_roots`:
$$\text{accepted\_note\_roots}[\text{old\_root}] \leftarrow 0$$

```text
┌────────────────────────────────────────────────────────────────────────────────────────┐
│               AMMBOOST-STYLE BOUNDED STORAGE RING BUFFER (SIZE 100)                     │
├────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                        │
│   New Root [idx = 105] ──► slot = 105 % 100 = 5                                        │
│                              │                                                         │
│                              ├─► Read old_root at slot 5 (Root from idx = 5)           │
│                              ├─► Prune: accepted_note_roots[old_root] = 0 (EVICTED!)   │
│                              ├─► Overwrite: root_history[5] = New Root                 │
│                              └─► Insert: accepted_note_roots[New Root] = timestamp     │
│                                                                                        │
│   Result: State storage size stays exactly capped at 100 active slots (O(1) Space)     │
└────────────────────────────────────────────────────────────────────────────────────────┘
```

### B. Implementasi Kode Rust Referensi di `zeltra-contracts/src/merkle.rs`

```rust
// Record in ring buffer history with ammBoost-style bounded state eviction (DEC-053)
let hist_idx = self.root_history_index.get();
let slot = U256::from(hist_idx.to::<u64>() % (ROOT_HISTORY_SIZE as u64));

// If ring buffer has completed a full cycle, prune the evicted root from accepted_note_roots
if hist_idx >= U256::from(ROOT_HISTORY_SIZE as u64) {
    let old_root = self.root_history.get(slot);
    if old_root != FixedBytes::ZERO && old_root != new_root_bytes {
        self.accepted_note_roots.insert(old_root, U256::ZERO);
    }
}

// Write new root to the bounded slot and mark accepted
self.root_history.insert(slot, new_root_bytes);
self.root_history_index.set(hist_idx + U256::from(1));
self.accepted_note_roots.insert(new_root_bytes, timestamp);
```

### C. Relayer Ingress Ephemeral Pruning (`zeltra-node`)
Sama halnya dengan *meta-block suppression* di `ammBoost`, node relayer Zeltra mem-prune entri antrean transaksi privat SQLite (`spend_queue`) yang telah mencapai finalitas konfirmasi on-chain (L2 finalized receipt) dan telah direkonsiliasi dalam laporan solvensi harian, mempertahankan ukuran database relayer tetap ramping dan kebal dari pembengkakan disk IO.

---

## 5. Rencana Verifikasi Empiris & Test Gates

- **Rasio Pengujian:** 1 pengujian positif didampingi minimal 2x pengujian negatif.
- **Gate 1 (Positive - Bounded Turnover Test):**
  Penyisipan 150 leaves berturut-turut pada `_mmr_insert`.
  - Daun 1 s/d 50 menghasilkan root yang terdaftar di `accepted_note_roots`.
  - Setelah melewati 100 daun (misal leaf index 101..150), root awal (leaf 1..50) terbukti ter-eviksi (`_is_accepted_note_root(root_awal) == false`).
  - 100 root terbaru terbukti tetap sah (`_is_accepted_note_root == true`).
- **Gate 2 (Negative - Evicted Root Spend Rejection):**
  - Percobaan eksekusi `spend_joinsplit` menggunakan Merkle proof terhadap root yang sudah ter-eviksi wajib REVERT dengan pesan error `INVALID_MERKLE_ROOT`.
  - Pembuktian bahwa state contract liabilities sama sekali tidak berubah saat transaksi kadaluarsa ditolak.
- **Gate 3 (Negative - Zero-Root Eviction Immunity):**
  - Penyisipan daun ke-0 s/d ke-99 (sebelum putaran pertama lengkap) tidak boleh memicu eviksi root aktif atau salah menghapus root nol.
- **Gate 4 (Solvency & Regresi Multi-Asset):**
  - Seluruh lifecycle test multi-asset (USDC & WETH) 61/61 lulus 100% tanpa regresi.

---

## 6. Referensi & Sitasi Akademik

1. **Nicolas Michel, Mohamed E. Najd, Ghada Almashaqbeh (2024/2025):** *"ammBoost: State Growth Control for AMMs"*, IEEE Transactions on Services Computing / IACR Cryptology ePrint Archive, Report 2024/1021, [https://eprint.iacr.org/2024/1021](https://eprint.iacr.org/2024/1021).
2. **DEC-032 (Zeltra Architecture):** [`DEC-032-merkle-mountain-range-commitment-accumulator.md`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-032-merkle-mountain-range-commitment-accumulator.md) — *Merkle Mountain Range Note Commitment Accumulator*.
3. **DEC-033 (Zeltra Architecture):** [`DEC-033-epoch-windowed-nullifier-pruning-in-flight-rollover.md`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-033-epoch-windowed-nullifier-pruning-in-flight-rollover.md) — *Epoch-Windowed Nullifier Registry & Pruning*.
