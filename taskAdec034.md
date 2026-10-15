# TASK A (DEC-034A): Kriptografi, Field Bit-Packing & Sirkuit ZK (`zeltra-core`)

> **Scope:** `zeltra-core/` (Arkworks BLS12-381, Poseidon-W5, Groth16 R1CS Circuits)  
> **Spesifikasi Referensi:** [`research/decisions/DEC-034A-canonical-field-bit-packing-and-intra-asset-circuit-constraints.md`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-034A-canonical-field-bit-packing-and-intra-asset-circuit-constraints.md) & [`research/decisions/DEC-034-dual-asset-architecture-usdc-weth-sovereign-commerce-rails.md`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-034-dual-asset-architecture-usdc-weth-sovereign-commerce-rails.md)  
> **Prinsip:** *"Zero-Overhead Field Bit-Packing, Constant-Arity Width-5 Preservation, Strict Intra-Asset Value Conservation"*

---

## 1. Latar Belakang & Tujuan Arsitektur
Mendukung dual-asset (**Asset ID 0: WETH**, **Asset ID 1: USDC**) tanpa merusak parameter hash Poseidon width-5 (`native_poseidon_w5`) yang sudah diaudit via Grain-128 LFSR (DEC-021). Mengemas `asset_id` ke dalam elemen $I_0$ (`value_packed = value + asset_id * 2^64`) dan mengikat variabel saksi ke slot public input `flags_packed` tanpa menambah dimensi array public input (16-PI untuk Private Note, 19-PI untuk Universal JoinSplit).

---

## 2. Checklist Implementasi Rinci

### A. Primitif Note & Bit-Packing (`zeltra-core/src/note.rs`)
- [x] **Pembaruan Struct `PrivateNoteV1`:**
  - Tambahkan field `pub asset_id: u8` (0: WETH, 1: USDC).
  - Pastikan field kunci adalah `pub owner_key: Fr` (bukan raw `spending_key`).
  - Implementasikan fungsi kanonikal:
    ```rust
    pub fn value_packed(&self) -> Fr {
        let two_pow_64 = Fr::from(1u128 << 64);
        Fr::from(self.value) + (Fr::from(self.asset_id as u64) * two_pow_64)
    }
    ```
  - Perbarui konstruktor `PrivateNoteV1::new(value, asset_id, owner_key, rho, randomness)`.
  - Perbarui penghitungan komitmen leaf Poseidon-W5 agar menggunakan `self.value_packed()` pada elemen $I_0$.
- [x] **Ekstensi `NotePlaintext` 48-Byte (DEC-037 & Big-Endian Canonical):**
  - Pastikan serialisasi 48 byte:
    - Byte `0..8`: `value: u64` (Big-Endian)
    - Byte `8..40`: `rho: [u8; 32]` (Big-Endian scalar)
    - Byte `40..48`: `asset_id: u64` (Big-Endian, nilai 0 atau 1)
  - Validasi bahwa deserialisasi menolak `asset_id > 1`.
  - Fungsi `to_note()` atau rekonsiliasi plaintext menurunkan `randomness` via `derive_note_randomness(shared_secret, &rho)` dan `owner_key` dari IVK.

### B. Gadget R1CS Sirkuit (`zeltra-core/src/circuit_gadgets.rs` atau `note_circuit.rs`)
- [x] **Gadget Bit-Packing Nilai & Aset (`enforce_packed_value_and_asset`):**
  - Boolean constraint pada `asset_id_var`: `asset_id * (asset_id - 1) == 0`.
  - Range constraint 64-bit pada `value_var` (`enforce_u64_range`).
  - Linear combination: `value_packed_var == value_var + asset_id_var * 2^64`.
- [x] **Gadget R1CS `flags_packed` Private Note (16-PI, Slot 15) — Anti-Miss-Wire:**
  - Implementasikan `enforce_private_note_flags_packed`:
    - Boolean constraint: `is_rollover_var` & `asset_id_var`.
    - Linear combination: `flags_packed_pi == is_rollover_var + 2 * asset_id_var`.
- [x] **Gadget R1CS `flags_packed` JoinSplit (19-PI, Slot 17) — Anti-Miss-Wire:**
  - Implementasikan `enforce_joinsplit_flags_packed`:
    - Boolean constraints: `has_change_1_var`, `has_change_2_var`, `asset_id_var`.
    - Linear combination: `flags_packed_pi == has_change_1_var + 2 * has_change_2_var + 4 * asset_id_var`.

### C. Sirkuit 1-in-1-out `PrivateNoteCircuit` (`zeltra-core/src/note_circuit.rs`)
- [x] Alokasikan Public Input slot 15 sebagai `flags_packed`.
- [x] Ekstrak dan buktikan korelasi witness `is_rollover` dan `asset_id` dengan slot 15 via gadget `enforce_private_note_flags_packed`.
- [x] Buktikan `value_packed` input note dan output change note terikat pada `asset_id` yang sama.
- [x] Konservasi nilai intra-aset:
  $$\text{value}_{in} == \text{merchant\_amount} + \text{protocol\_fee} + \text{execution\_fee} + (\text{has\_change} \cdot \text{value}_{out})$$
- [x] Pastikan tidak ada unconstrained variables pada slot 15.

### D. Sirkuit Universal 2-in-2-out `JoinSplitCircuit` (`zeltra-core/src/joinsplit_circuit.rs`)
- [x] Alokasikan Public Input slot 17 sebagai `flags_packed`.
- [x] Alokasikan Public Input slot 18 sebagai `is_rollover`.
- [x] Ekstrak dan buktikan korelasi witness `has_change_1`, `has_change_2`, dan `asset_id` dengan slot 17 via gadget `enforce_joinsplit_flags_packed`.
- [x] **ATURAN KANONIKAL DUMMY & ZERO-VALUE NOTE (Patch Audit #1):**
  - **Dummy Input Note (`in2` bernilai $0$):**
    Saat belanja hanya menggunakan 1 input note, input ke-2 bernilai `value = 0` **WAJIB** tetap menggunakan `asset_id` transaksi (`asset_id_in2 == public_asset_id`).
    Contoh: Pada transaksi USDC (`asset_id = 1`), dummy note memiliki `value_packed = 0 + 1 * 2^64 = 2^64`.
  - **Zero-Value Output Note (`has_change_i == 0`):**
    Slot kembalian tanpa sisa (`value = 0`) **WAJIB** tetap menggunakan `asset_id` transaksi (`asset_id_out_i == public_asset_id`).
  - **Strict Intra-Asset Equality:**
    Tegakkan constraint mutlak tanpa percabangan kondisional:
    $$\text{asset\_id}_{in1} == \text{asset\_id}_{in2} == \text{asset\_id}_{out1} == \text{asset\_id}_{out2} == \text{public\_asset\_id}$$
- [x] Konservasi nilai 2-in-2-out:
  $$\text{value}_{in1} + \text{value}_{in2} == \text{merchant\_amount} + \text{protocol\_fee} + \text{execution\_fee} + \text{value}_{out1} + \text{value}_{out2}$$

### E. Fee Engine & Skala Presisi (`zeltra-core/src/fees.rs`)
- [x] Dukung perhitungan fee untuk USDC (6 desimal) dan WETH (9 desimal - Gwei).
- [x] Formula fee protokol flat 45 bps dengan pembagian ke atas (*ceiling division*) anti-dust:
  $$\text{protocol\_fee} = \frac{\text{merchant\_amount} \times 45 + 9999}{10000}$$

---

## 3. Matriks Pengujian Wajib (Unit & Formal Tests)
Jalankan dan pastikan 100% lulus:
1. [x] `test_bit_packing_roundtrip_soundness`: Enkoding & dekoding `(value, asset_id)` pada field $\text{Fr}$ menghasilkan nilai kanonikal identik.
2. [x] `test_asset_id_boolean_constraint_rejects_greater_than_one`: Bukti dengan `asset_id >= 2` ditolak oleh R1CS constraint.
3. [x] `test_intra_asset_conservation_rejects_cross_spend`: Input note WETH dengan output commitment USDC memicu kegagalan constraint.
4. [x] `test_joinsplit_dummy_input_usdc_value_packed`: Transaksi 1-input USDC (`asset_id = 1`) dengan dummy note input-2 bernilai 0 (`value_packed = 2^64`) membuktikan satisfiability constraint tanpa error.
5. [x] `test_flags_packed_r1cs_gadget_enforcement`: Memastikan Public Input slot 15 (16-PI) dan slot 17 (19-PI) menolak saksi yang tidak cocok dengan nilai linear.
6. [x] `test_dec037_note_plaintext_roundtrip_with_asset_id`: Serialisasi dan dekripsi memo 96-byte dengan 48-byte Big-Endian plaintext mengembalikan `asset_id` dengan benar.
7. [x] `test_gwei_normalization_precision_bounds`: Pengujian batas atas dan bawah konversi Wei-Gwei tanpa sisa modulo.

---

## 4. Definisi Selesai (Definition of Done)
- [x] `cargo check -p zeltra-core` & `cargo clippy -p zeltra-core` bersih tanpa warning (0 warnings, 0 errors).
- [x] Seluruh unit test di `zeltra-core` lulus: `cargo test -p zeltra-core --lib` (162 tests passed, 0 failed).
- [x] Tidak ada `#[cfg(test)]` bypass pada jalur constraint produksi.
- [x] Status: **COMPLETED & AUDITED**.
