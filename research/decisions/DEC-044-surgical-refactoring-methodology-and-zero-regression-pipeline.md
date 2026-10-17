# DEC-044: Surgical Architecture Refactoring Methodology, Three-Stage Decomposition & Zero-Regression Verification Workflow

**Status:** APPROVED  
**Date:** 2026-10-17  
**Domain:** Cross-Crate Architecture (`zeltra-sdk`, `zeltra-node`, `zeltra-contracts`), Codebase Modularity & Integrity  
**Security Level:** CRITICAL (Preservasi Invariant Finansial, Proteksi Freeze Kriptografi, Zero-Breaking-Change API)  
**Related Decisions:**
- [`DEC-034: Dual Sovereign Rails & Isolated Solvency Invariants`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-034-dual-sovereign-rails-weth-usdc-and-unified-settlement.md)
- [`DEC-035: Cryptographic Canonicality, Public Input Binding & MMR`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-035-cryptographic-canonicality-public-input-binding-and-mmr.md)
- [`DEC-036: Universal JoinSplit Circuit Architecture`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-036-universal-joinsplit-circuit-architecture.md)
- [`DEC-038: Pure ZK-UTXO Migration & Direct MMR Deposit`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-038-pure-zk-utxo-migration-and-direct-mmr-deposit.md)
- [`DEC-040: Viewing Key Export Suite & CEX Selective Disclosure`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-040-viewing-key-export-suite-and-cex-selective-disclosure.md)
- [`DEC-041: In-Browser WebAssembly Groth16 Prover & Disposable Worker`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-041-in-browser-wasm-groth16-prover-and-disposable-worker.md)
- [`DEC-043: Shielded-to-Shielded In-Pool Transfer & Strict 3-Mode Settlement`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-043-shielded-to-shielded-in-pool-transfer-and-merchant-masking.md)

---

## 1. Executive Summary & Problem Statement

Protokol Zeltra telah berhasil mencapai kematangan fungsional penuh pada seluruh pilar inti: arsitektur murni ZK-UTXO, dual sovereign rails (USDC & WETH), deposit direct MMR 0 bps, transfer in-pool shielded 0 bps (Strict 3-Mode JoinSplit), export kepatuhan CEX, dan prover in-browser WASM. Seluruh rangkaian tes (162 unit tests Rust dan 44/44 Python test harness) berstatus 100% lulus.

Namun, kecepatan inovasi yang masif menghasilkan **akumulasi kompleksitas struktural** di mana total codebase telah mencapai **39.269 LoC kode murni (48.591 total baris)** pada 88 file `.rs`. Berdasarkan audit topologi sistem menggunakan Graphify, teridentifikasi 3 "God Files" / Monolith raksasa:
1. `zeltra-sdk/src/wallet/note_wallet.rs` (**5.476 LoC murni / 6.557 total baris**) — God Node sentral (91 edges), mencampur manajemen kunci, sinkronisasi MMR, persiapan bukti Groth16, state machine 2PC, dan ekspor CEX dalam satu file tunggal.
2. `zeltra-node/src/handlers/spend.rs` (**2.834 LoC**) dan `zeltra-node/src/database.rs` (**2.453 LoC**) — Mencampur HTTP ingress, klasifikasi mode JoinSplit, validasi memo 192 hex char, Groth16 preflight, dan puluhan kueri SQLCipher heterogen.
3. `zeltra-contracts/src/lib.rs` (**3.064 LoC murni / 3.656 total baris**) — God Node on-chain (63 edges), menampung entrypoint Stylus, validasi invariant, helper bitwise, dan logika dispatcher dalam satu unit kompilasi.

**Tujuan DEC-044:** Menetapkan metodologi refaktor bedah (*surgical refactoring*) dan alur kerja verifikasi presisi agar seluruh modul monolitik di atas dapat didekomposisi menjadi unit-unit domain yang bersih, modular, dan mudah diaudit, dengan **garansi mutlak 0% breaking change pada Public API** dan **nol regresi fungsional (tidak ada wire/koneksi yang terlepas)**.

---

## 2. Invariant Terkunci & Zona Larangan (Frozen Boundaries)

Refaktor yang presisi menuntut kejelasan batas wilayah yang boleh disentuh dan wilayah yang **HARUS DIKUNCI MATI (STRICTLY FROZEN)**.

### A. Zona Merah: Kunci Mati Kriptografi & Storage (Dilarang Diubah)
1. **Sirkuit ZK & R1CS Gadgets (`zeltra-core`):**
   - File: `note_circuit.rs`, `joinsplit_circuit.rs`, `selective_disclosure_circuit.rs`, `circuit_gadgets.rs`.
   - *Rasional:* Protokol sedang memasuki Fase 2 (Circuit Freeze & MPC Trusted Setup). R1CS constraint topology (16 PI untuk Note Circuit, 19 PI untuk JoinSplit Circuit, serta 448 quadratic range gates) telah terikat secara matematis. Modifikasi pada baris constraint apa pun akan mengubah hash verifying key dan menggugurkan artefak pembuktian.
2. **Layout Storage Kontrak Stylus (`zeltra-contracts`):**
   - File: `src/storage.rs` (`sol_storage!`).
   - *Rasional:* Urutan slot penyimpanan on-chain, ring buffer 100 accepted roots, dan bit-packing multi-asset adalah pondasi konsistensi L2. Struktur ini tidak boleh digeser atau diubah tipe datanya.

### B. Zona Hijau: Target Restrukturisasi Struktural
- Dekomposisi organisasi kode internal pada `zeltra-sdk` (submodul internal).
- Modularisasi database queries dan HTTP ingress pipeline pada `zeltra-node`.
- Perampingan dispatcher entrypoint pada `zeltra-contracts/src/lib.rs` melalui pendelegasian ke submodul internal tanpa mengubah selector ABI Stylus.

---

## 3. Strategi Dekomposisi Tiga Tahap (Three-Stage Strategy)

Refaktor dilakukan secara sekuensial dari lapisan terluar (klien) menuju lapisan terdalam (kontrak):

```
+-------------------------------------------------------------------------------+
|                        ALUR EKSEKUSI TIGA TAHAP                               |
+-------------------------------------------------------------------------------+
|                                                                               |
|  [TAHAP 1: SDK Monolith]                                                     |
|  zeltra-sdk/src/wallet/note_wallet.rs (5.4k LoC)                              |
|  --> Dekomposisi multi-file 'impl PrivateNoteWallet' per domain               |
|      (keys, deposit, spend, joinsplit, sync, 2pc, compliance)                 |
|                                                                               |
|                                     |                                         |
|                                     v                                         |
|  [TAHAP 2: Relayer Node Pipeline]                                             |
|  zeltra-node/src/database.rs (2.4k LoC) & handlers/spend.rs (2.8k LoC)        |
|  --> Pemisahan Repository Pattern domain DB & isolasi Ingress Validation      |
|                                                                               |
|                                     |                                         |
|                                     v                                         |
|  [TAHAP 3: Contract Thin Dispatcher]                                          |
|  zeltra-contracts/src/lib.rs (3.0k LoC)                                       |
|  --> Pendelegasian fungsi eksternal ke internal modules (deposit, spend, view)|
|      Mempertahankan 100% ABI selector dan storage layout                      |
|                                                                               |
+-------------------------------------------------------------------------------+
```

### Tahap 1: Dekomposisi God Object `PrivateNoteWallet` (`zeltra-sdk`)
- **Masalah:** File `note_wallet.rs` menampung lebih dari 5.400 LoC kode murni yang menangani 7 domain fungsional berbeda.
- **Metode Refaktor:**
  Memanfaatkan fitur bahasa Rust di mana satu struct dapat memiliki beberapa blok `impl` yang tersebar di beberapa file dalam modul yang sama.
  Struktur struct `PrivateNoteWallet` dan tipe pendukungnya tetap didefinisikan secara publik pada `wallet/mod.rs` atau `wallet/types.rs`, sedangkan implementasi metodenya dipisah menjadi sub-file domain:
  - `wallet/keys.rs`: Khusus derivasi kunci spending, nullifier key, viewing key hierarchy, dan alamat ZSA Bech32m (`zs1...`).
  - `wallet/deposit.rs`: Khusus lifecycle pembuatan note direct deposit, staging, dan konfirmasi indexer.
  - `wallet/spend.rs`: Khusus seleksi note, kalkulasi fee, pembuatan single note proof, dan in-flight rollover.
  - `wallet/joinsplit.rs`: Khusus alur Mode 2 (in-pool transfer) dan Mode 3 (public outflow spend) dengan dual memo 96-byte.
  - `wallet/sync.rs`: Khusus background worker indexer sync, batch trial decryption, dan MMR proof fetching.
  - `wallet/state_2pc.rs`: Khusus manajemen note reservation, crash-safe 2-phase commit, dan rollback.
  - `wallet/compliance.rs`: Khusus integrasi ekspor audit trail B2B dan pembuatan bukti selektif CEX.
- **Jaminan Zero-Breaking-Change:**
  Nama struct, method signature, tipe error (`WalletError`), visibilitas `pub`, dan path import (`zeltra_sdk::wallet::PrivateNoteWallet`) tetap identik 100%. Kode eksternal (CLI, browser WASM bindings, dan integration tests) tidak mengalami perubahan sintaks satu karakter pun.

### Tahap 2: Dekomposisi Pipeline Relayer Node (`zeltra-node`)
- **Masalah:**
  - `database.rs` menumpuk 76 koneksi dependency untuk kueri tabel yang sangat heterogen.
  - `handlers/spend.rs` menggabungkan parsing request HTTP, verifikasi integritas memo (192 hex char), klasifikasi Strict 3-Mode, eksekusi Groth16 local preflight, reservasi nullifier, hingga antrian broadcast EVM.
- **Metode Refaktor:**
  1. *Database Repository Pattern:* Struct `Database` dipertahankan sebagai wrapper terpusat dari pool koneksi SQLCipher terenkripsi, tetapi implementasi kueri didelegasikan ke sub-domain traits/modules:
     - `database/nullifiers.rs`: Kueri reservasi, deduplikasi, dan pengecekan status double-spend nullifier.
     - `database/spends.rs`: Kueri antrian transaksi spend, state transition, dan histori receipt.
     - `database/mmr.rs`: Kueri leaf commitments, dynamic root cache, dan MMR peak tracking.
     - `database/quotes.rs`: Kueri EIP-712 signed execution quotes dan anti-replay nonce.
  2. *Ingress Validation Isolation:* Memisahkan logika verifikasi komputasi dari handler HTTP:
     - Logika validasi memo calldata, batasan canonical scalar ($< r$), dan verifikasi bukti Groth16 lokal dipindahkan ke `validation/spend_preflight.rs`.
     - Handler `handlers/spend.rs` murni berfungsi sebagai HTTP controller tipis: menerima DTO Axum $\rightarrow$ memanggil preflight validator $\rightarrow$ menyisipkan ke antrian database $\rightarrow$ mengembalikan response status.

### Tahap 3: Perampingan Entrypoint Smart Contract (`zeltra-contracts`)
- **Masalah:** File `lib.rs` memiliki 3.064 LoC murni karena mengimplementasikan logika eksekusi langsung di dalam blok antarmuka Stylus `#[external]`.
- **Metode Refaktor (Thin Dispatcher Pattern):**
  - Seluruh fungsi bertanda `#[external]` pada struct `Zeltra` dipertahankan persis tanpa mengubah nama fungsi, tipe parameter, atau nilai balik (agar function selector EVM / Keccak-256 ABI tidak berubah).
  - Badan fungsi (*function body*) di dalam `lib.rs` dirampingkan menjadi delegasi panggilan 1–3 baris ke modul logika internal yang sudah terisolasi:
    - Logika deposit didelegasikan ke `deposit.rs` (`_handle_deposit_direct_note`, `_handle_native_eth_deposit`).
    - Logika spend dan JoinSplit didelegasikan ke `spend.rs` (`_handle_spend_joinsplit`, `_handle_spend_private_note`).
    - Kueri view MMR dan root ring buffer didelegasikan ke modul baru `views.rs`.
    - Pengecekan invariant solvensi terisolasi multi-aset (`_check_asset_solvency_invariant`) dipanggil sebagai guard penutup sebelum transaksi selesai.

---

## 4. Metodologi Refaktor Bedah (Surgical Refactoring Rules)

Untuk memastikan tidak ada dependensi yang putus (*zero disconnected wires*), seluruh proses refaktor tunduk pada 4 prinsip wajib:

1. **Prinsip Behavior-Preserving Multi-File Struct:**
   Dilarang memecah `PrivateNoteWallet` menjadi struct-struct baru yang berbeda bagi pengguna publik. Pemecahan hanya terjadi di tingkat organisasi file kode sumber (`impl` blocks), bukan pada arsitektur tipe publik.
2. **Prinsip Single-Responsibility per File:**
   Satu file sumber maksimal memiliki panjang $\le 800$ LoC. Jika suatu domain melebihi batas ini, file tersebut harus dipecah menjadi sub-komponen fungsional yang spesifik.
3. **Prinsip Fail-Closed Invariant Integrity:**
   Setiap fungsi yang memvalidasi kondisi keamanan finansial (misal: sanitasi skalar kanonikal, invariant solvensi aset, atau verifikasi nullifier ganda) dilarang dipersingkat atau dimodifikasi alur logika logisnya saat dipindahkan.
4. **Prinsip No-Orphaned-Symbols (Pembersihan Graf Bersih):**
   Setiap pemindahan fungsi harus segera disertai dengan pembaruan import (`use`), penghapusan `#[allow(dead_code)]` yang tidak perlu, dan verifikasi bahwa tidak ada modul yatim-piatu (*orphaned module*) di dalam tree.

---

## 5. Alur Kerja Presisi & Gerbang Validasi (Verification Gates)

Setiap tahapan refaktor dieksekusi menggunakan siklus **Surgical Slice-by-Slice Loop** yang ketat:

```
[Mulai Slice] 
      │
      ▼
1. Capture Baseline Invariant (Catat status tes & snapshot edge graf)
      │
      ▼
2. Ekstrak & Pindahkan 1 Domain Logika (Misal: keys.rs dari note_wallet.rs)
      │
      ▼
3. Gerbang Kompilasi & Linter:
   • cargo check --workspace --all-targets
   • cargo clippy --workspace --all-targets -- -D warnings
      │
      ├─ Gagal ──> Rollback Instan (git checkout) & Investigasi
      ▼
4. Gerbang Pengujian Fungsional Crate:
   • cargo test -p <crate_terkait> --lib
      │
      ├─ Gagal ──> Rollback Instan & Perbaiki Mapping Wire
      ▼
5. Gerbang Integrasi Mutlak (End-to-End Interop):
   • python3 scripts/test_real_case_dec043c_inpool_wallet.py (44/44 Wajib Lolos)
      │
      ├─ Gagal ──> Revert Total Slice
      ▼
6. Sinkronisasi Knowledge Graph:
   • graphify update .
   • Verifikasi penurunan degree coupling pada God Node
      │
      ▼
[Commit Slice & Lanjut ke Slice Berikutnya]
```

### Kebijakan Rollback Instan (Zero-Tolerance Policy)
Jika sebuah slice refaktor menyebabkan kegagalan kompilasi yang tidak terpecahkan dalam 1 iterasi, atau menyebabkan satu saja tes pada suite interop gagal:
- **Dilarang menambal dengan hack sementara.**
- Wajib melakukan `git checkout` kembali ke status hijau terakhir dari slice tersebut.
- Urai kembali rantai ketergantungan menggunakan `graphify query` sebelum mencoba kembali.

---

## 6. Metrik Keberhasilan & Kriteria Selesai

Refaktor dinyatakan sukses secara definitif jika memenuhi kriteria objektif berikut:

1. **Penurunan Ukuran God Files:**
   - Tidak ada satu pun file di seluruh workspace yang memiliki panjang $> 1.000$ LoC murni (selain file konstanta tabel numerik pre-computed seperti Poseidon MDS).
   - `note_wallet.rs` berkurang dari 5.476 LoC menjadi kumpulan modul $\le 700$ LoC per domain.
   - `lib.rs` kontrak Stylus berkurang dari 3.064 LoC menjadi $\le 600$ LoC (murni thin dispatcher).
   - `database.rs` dan `spend.rs` terurai menjadi repositori dan modul validasi modular.
2. **Zero Breaking Changes:**
   - Seluruh integrasi eksternal (`zeltra-cli`, script automasi Python, dan web worker WASM) berjalan normal tanpa perubahan kode pemanggil.
3. **Verifikasi Lulus 100%:**
   - `cargo check --workspace --all-targets` bebas error.
   - `cargo clippy --workspace --all-targets -- -D warnings` bersih dari peringatan.
   - Test harness Python `test_real_case_dec043c_inpool_wallet.py` tetap berstatus **44/44 passed**.
4. **Penyusutan Degree Graphify:**
   - Node `PrivateNoteWallet`, `Database`, dan `Zeltra` mengalami penurunan degree konektivitas langsung yang signifikan pada laporan `graphify god-nodes`, mencerminkan arsitektur yang terdekopel secara sehat.
