# DEC-044: Surgical Architecture Refactoring Methodology, Four-Stage Decomposition & Zero-Regression Verification Workflow

**Status:** APPROVED  
**Date:** 2026-10-17  
**Domain:** Cross-Crate Architecture (`zeltra-sdk`, `zeltra-node`, `zeltra-contracts`, `zeltra-core`), Codebase Modularity & Integrity  
**Security Level:** CRITICAL (Preservasi Invariant Finansial, Proteksi Freeze Kriptografi, Zero-Breaking-Change API)  
**Related Decisions:**
- [`DEC-034: Dual Sovereign Rails & Isolated Solvency Invariants`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-034-dual-sovereign-rails-weth-usdc-and-unified-settlement.md)
- [`DEC-035: Cryptographic Canonicality, Public Input Binding & MMR`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-035-cryptographic-canonicality-public-input-binding-and-mmr.md)
- [`DEC-036: Universal JoinSplit Circuit Architecture`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-036-universal-joinsplit-circuit-architecture.md)
- [`DEC-038: Pure ZK-UTXO Migration & Direct MMR Deposit`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-038-pure-zk-utxo-migration-and-direct-mmr-deposit.md)
- [`DEC-040: Viewing Key Export Suite & CEX Selective Disclosure`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-040-viewing-key-export-suite-and-cex-selective-disclosure.md)
- [`DEC-041: In-Browser WebAssembly Groth16 Prover & Disposable Worker`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-041-in-browser-wasm-groth16-prover-and-disposable-worker.md)
- [`DEC-043: Shielded-to-Shielded In-Pool Transfer & Strict 3-Mode Settlement`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-043-shielded-to-shielded-in-pool-transfer-and-merchant-masking.md)

---

## 1. Executive Summary & Problem Statement

Protokol Zeltra telah berhasil mencapai kematangan fungsional penuh pada seluruh pilar inti: arsitektur murni ZK-UTXO, dual sovereign rails (USDC & WETH), deposit direct MMR 0 bps, transfer in-pool shielded 0 bps (Strict 3-Mode JoinSplit), export kepatuhan CEX, dan prover in-browser WASM. Seluruh rangkaian tes (162 unit tests Rust dan 44/44 Python test harness) berstatus 100% lulus.

Namun, kecepatan inovasi yang masif menghasilkan **akumulasi kompleksitas struktural** di mana total codebase telah mencapai **39.269 LoC kode murni (48.591 total baris)** pada 88 file `.rs`. Berdasarkan audit topologi sistem menggunakan Graphify, teridentifikasi 4 "God Files" / Monolith raksasa di workspace:
1. `zeltra-sdk/src/wallet/note_wallet.rs` (**5.476 LoC murni / 6.557 total baris**) — God Node sentral (91 edges), mencampur manajemen kunci, sinkronisasi MMR, persiapan bukti Groth16, state machine 2PC, dan ekspor CEX dalam satu file tunggal.
2. `zeltra-node/src/handlers/spend.rs` (**2.834 LoC**) dan `zeltra-node/src/database.rs` (**2.453 LoC**) — Mencampur HTTP ingress, klasifikasi mode JoinSplit, validasi memo 192 hex char, Groth16 preflight, dan puluhan kueri SQLCipher heterogen.
3. `zeltra-contracts/src/lib.rs` (**3.064 LoC murni / 3.656 total baris**) — God Node on-chain (63 edges), menampung entrypoint Stylus, validasi invariant, helper bitwise, dan logika dispatcher dalam satu unit kompilasi.
4. `zeltra-core/src/note.rs` (**2.132 LoC murni / 2.876 total baris**) — Monolith primitif kriptografi, menampung derivasi kunci ZSA Bech32m, protokol memo ChaCha20Poly1305, in-memory Merkle Mountain Range, struct `PrivateNoteV1`, dan 1.404 baris unit test embedded.

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
3. **Domain Separator Constants (`zeltra-core`):**
   - Seluruh konstanta domain Poseidon (`DOMAIN_NOTE_COMMITMENT`, `DOMAIN_NULLIFIER`, `DOMAIN_SCOPE_V1`, `DOMAIN_BINDING_V1`, `DOMAIN_MMR_BAG`, dll.) **dilarang dimodifikasi nilai string / byte-nya**.

### B. Zona Hijau: Target Restrukturisasi Struktural
- Dekomposisi organisasi kode internal pada `zeltra-sdk` (submodul internal `impl PrivateNoteWallet`).
- Modularisasi database queries (Repository Pattern) dan HTTP ingress pipeline pada `zeltra-node`.
- Perampingan dispatcher entrypoint pada `zeltra-contracts/src/lib.rs` (Thin Dispatcher) dan isolasi test suite ke `src/tests.rs`.
- Dekomposisi domain non-sirkuit pada `zeltra-core/src/note.rs` (`keys`, `memo`, `merkle`, `note`).

---

## 3. Strategi Dekomposisi Empat Tahap (Four-Stage Strategy)

Refaktor dilakukan secara sekuensial dari lapisan terluar (klien) menuju lapisan terdalam (kontrak dan core):

```
+-------------------------------------------------------------------------------+
|                        ALUR EKSEKUSI EMPAT TAHAP                              |
+-------------------------------------------------------------------------------+
|                                                                               |
|  [TAHAP 1: SDK Monolith] (SELESAI - 100%)                                     |
|  zeltra-sdk/src/wallet/note_wallet.rs (5.4k LoC)                              |
|  --> Dekomposisi multi-file 'impl PrivateNoteWallet' per domain               |
|      (keys, deposit, spend, joinsplit, sync, 2pc, compliance)                 |
|                                                                               |
|                                     │                                         |
|                                     ▼                                         |
|  [TAHAP 2: Relayer Node Pipeline] (SELESAI - 100%)                            |
|  zeltra-node/src/database.rs (2.4k LoC) & handlers/spend.rs (2.8k LoC)        |
|  --> Pemisahan Repository Pattern domain DB & isolasi Ingress Validation      |
|                                                                               |
|                                     │                                         |
|                                     ▼                                         |
|  [TAHAP 3: Contract Thin Dispatcher] (SELESAI - 100%)                         |
|  zeltra-contracts/src/lib.rs (3.0k LoC)                                       |
|  --> Pendelegasian fungsi eksternal ke internal modules (deposit, spend, view)|
|      serta isolasi 2.800 baris tests ke src/tests.rs                          |
|                                                                               |
|                                     │                                         |
|                                     ▼                                         |
|  [TAHAP 4: Core Primitives Decomposition]                                     |
|  zeltra-core/src/note.rs (2.8k LoC)                                           |
|  --> Dekomposisi domain non-sirkuit (keys, memo, merkle, note)                |
|      serta isolasi 1.400 baris tests tanpa menyentuh R1CS constraints         |
|                                                                               |
+-------------------------------------------------------------------------------+
```

### Tahap 1: Dekomposisi God Object `PrivateNoteWallet` (`zeltra-sdk`) — Status: SELESAI
- Memanfaatkan fitur Rust di mana blok `impl PrivateNoteWallet` dipecah per file domain (`keys.rs`, `deposit.rs`, `spend.rs`, `joinsplit.rs`, `sync.rs`, `state_2pc.rs`, `compliance.rs`, `types.rs`). 100% backward compatible.

### Tahap 2: Dekomposisi Pipeline Relayer Node (`zeltra-node`) — Status: SELESAI
- Penerapan Repository Pattern terpisah per domain (`nullifiers.rs`, `spends.rs`, `mmr.rs`, `quotes.rs`, `relayer_tx.rs`, `ccip.rs`, `idempotency.rs`) di bawah struct sentral `Database`.
- Isolasi validasi berat (memo calldata 192 hex, canonical scalar $< r$, Groth16 local preflight) ke `validation/spend_preflight.rs`, menjadikan `handlers/spend.rs` murni sebagai Thin HTTP Controller.

### Tahap 3: Perampingan Entrypoint Smart Contract (`zeltra-contracts`) — Status: SELESAI
- Menerapkan Thin Dispatcher Pattern pada `lib.rs`: memindahkan 2.880 baris test ke `src/tests.rs`, serta mendelegasikan view dan governance ke `src/views.rs` dan `src/governance.rs`.
- `lib.rs` menyusut dari 3.656 baris menjadi 532 baris tanpa mengubah satu byte pun dari selector Keccak-256 ABI atau storage layout.

### Tahap 4: Dekomposisi Primitif Kriptografi Non-Sirkuit (`zeltra-core`) — Status: SELESAI
- **Masalah:** File `zeltra-core/src/note.rs` menampung 2.132 LoC murni dan 1.404 baris test embedded (total 2.876 baris), mencampuradukkan derivasi kunci ZSA, enkripsi memo, akumulator MMR, dan primitive note.
- **Metode Refaktor:**
  Pecah `note.rs` menjadi submodul mandiri di bawah `zeltra-core/src/`:
  - `src/keys.rs` (~450 LoC):
    * Derivasi kunci hierarki: `derive_master_secret`, `derive_spending_key`, `derive_owner_key`, `derive_nullifier_key`, `derive_viewing_keys`.
    * Viewing Key structs: `IncomingViewingKey`, `OutcomingViewingKey`, `FullViewingKey`.
    * Alamat ZSA: `ZeltraShieldedAddress` (codec Bech32m `zs1...`), `KeyFormatError`.
  - `src/memo.rs` (~350 LoC):
    * Plaintext DTO: `NotePlaintext`.
    * Enkripsi/Dekripsi Memo: `encrypt_note_memo`, `decrypt_note_memo`, `encrypt_note_memo_deterministic`, `decrypt_note_memo_deterministic`, `derive_ecdh_note_randomness` (ChaCha20Poly1305 + X25519 ECDH).
  - `src/merkle.rs` (~300 LoC):
    * In-memory MMR: struct `MerkleMountainRange`, `MMRProof`, `bag_peaks`, `generate_proof`, `verify_proof`, `merkle_hash`.
  - `src/note.rs` (Dirampingkan menjadi ~400 LoC):
    * Domain separators Poseidon (`domain_note_commitment`, `domain_nullifier`, dll.).
    * Note Primitive: `PrivateNoteV1`, `value_packed`, `commitment`, `note_commitment`, `note_commitment_packed`.
    * Derivasi nullifier: `derive_nullifier`, `derive_nullifier_epoch`, `derive_joinsplit_nullifier`, `derive_joinsplit_dummy_nullifier`.
    * Field serialization utilities: `fr_to_be_bytes`, `fr_from_be_bytes`, `split_quote_hash_to_limbs`, `combine_limbs_to_quote_hash`.
  - `src/tests/note_tests.rs` (~1.400 LoC):
    * Pindahkan seluruh blok `#[cfg(test)] mod tests { ... }` keluar dari `src/note.rs` ke file test khusus.
- **Jaminan Zero-Breaking-Change:**
  Seluruh tipe, fungsi, dan konstanta di atas wajib di-reexport secara transparan pada `zeltra-core/src/lib.rs` (misal: `pub use keys::*; pub use memo::*; pub use merkle::*; pub use note::*;`).
  Akibatnya, seluruh modul pemanggil di `zeltra-sdk`, `zeltra-node`, `zeltra-contracts`, dan `zeltra-cli` tetap meng-import dengan path yang sama persis (misal: `zeltra_core::PrivateNoteV1`, `zeltra_core::MerkleMountainRange`, `zeltra_core::fr_to_be_bytes`).

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
   - `note.rs` di `zeltra-core` terurai dari 2.876 baris menjadi modul-modul ramping (`keys.rs`, `memo.rs`, `merkle.rs`, `note.rs`) $\le 450$ LoC per domain.
2. **Zero Breaking Changes:**
   - Seluruh integrasi eksternal (`zeltra-cli`, script automasi Python, dan web worker WASM) berjalan normal tanpa perubahan kode pemanggil.
3. **Verifikasi Lulus 100%:**
   - `cargo check --workspace --all-targets` bebas error.
   - `cargo clippy --workspace --all-targets -- -D warnings` bersih dari peringatan.
   - Test harness Python `test_real_case_dec043c_inpool_wallet.py` tetap berstatus **44/44 passed**.
4. **Penyusutan Degree Graphify:**
   - Node `PrivateNoteWallet`, `Database`, dan `Zeltra` mengalami penurunan degree konektivitas langsung yang signifikan pada laporan `graphify god-nodes`, mencerminkan arsitektur yang terdekopel secara sehat.
