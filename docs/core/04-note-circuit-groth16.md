# Nimbus Core — 04: Private Note Circuit (Groth16 ZK-SNARK)

Dokumen ini menjelaskan desain sirkuit ZK-SNARK **Groth16** pada [`nimbus-core/src/note_circuit.rs`](file:///workspaces/Zeltra-Protocol/nimbus-core/src/note_circuit.rs) dan daftar perbaikan keamanan kritis **Gate C0** yang wajib diselesaikan sebelum integrasi ke smart contract Stylus.

---

## 1. Arsitektur Sirkuit (`PrivateNoteCircuit`)

`PrivateNoteCircuit` membuktikan validitas pembelanjaan private note (1 input note $\to$ 1 merchant payout + 1 change output) tanpa membocorkan identitas pengirim, nilai saldo awal, maupun nilai kembaliannya.

### A. Public Inputs (6 Elemen Skalar $\mathbb{F}_r$ yang Diketahui Smart Contract):
1. `merkle_root`: Root Merkle tree yang diakui oleh smart contract on-chain.
2. `nullifier`: Penanda unik note yang di-spend untuk mencegah pembelanjaan ganda.
3. `recipient`: Alamat penerima pembayaran (merchant atau dompet publik).
4. `amount`: Nominal USDC yang dibayarkan ke merchant.
5. `fee`: Total biaya transaksi (protocol fee + execution fee).
6. `change_commitment`: Komitmen note kembalian yang akan ditambahkan ke Merkle tree on-chain.

### B. Private Witnesses (Rahasia yang Hanya Diketahui Prover/Client):
* `input_note`: Data note yang sedang dibelanjakan (`owner_pk`, `value`, `blinding`, `asset_id`).
* `input_sk_owner`: Kunci rahasia pembelanjaan (*spending key*) pemilik note.
* `input_leaf_index`: Posisi indeks daun note pada Merkle tree.
* `merkle_path`: Jalur hash 20 tingkat menuju root Merkle tree.
* `has_change`: Flag boolean penanda apakah transaksi menghasilkan kembalian.
* `change_note`: Data note kembalian baru.

---

## 2. Rangkaian Constraint Matematis di Sirkuit

Sirkuit mengeksekusi dan membuktikan constraint R1CS berikut:

1. **Otoritas Kepemilikan (*Owner Authority*):**
   Membuktikan bahwa prover mengetahui `sk_owner` yang menghasilkan `owner_pk` pada input note.
2. **Derivasi Nullifier:**
   Membuktikan bahwa `nullifier` publik diturunkan secara benar dari `nullifier_key(sk_owner)` dan `leaf_index`.
3. **Keanggotaan Merkle Tree (*Membership Proof*):**
   Membuktikan bahwa komitmen input note benar-benar terdaftar di dalam Merkle tree dengan `merkle_root` yang sah.
4. **Kekekalan Nilai (*Value Conservation*):**
   $$\text{input\_value} == \text{amount} + \text{fee} + \text{change\_value}$$
5. **Kesesuaian Komitmen Kembalian:**
   $$\text{change\_commitment} == \text{has\_change} \times \text{computed\_change\_cm}$$

---

## 3. Status Gate C0 Security Repairs (SELESAI & TERVERIFIKASI)

Ref: [`docs/todos/private-note-balance.md`](file:///workspaces/Zeltra-Protocol/docs/todos/private-note-balance.md) & [`DEC-016B`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-016B-mvp-circuit-shortcuts.md)

Seluruh 3 celah keamanan kritis yang ditemukan pada audit prototype `PrivateNoteCircuit` **telah berhasil diperbaiki dan lulus pengujian regression/negative testing**:

### ✅ 1. Pengikatan Bit Merkle Path ke `leaf_index` (Pencegah Double-Spend)
* **Implementasi:** `input_leaf_index` didekomposisi secara ketat menjadi tepat 20 boolean bit variable di dalam sirkuit R1CS:
  $$\text{leaf\_index} == \sum_{i=0}^{19} \text{bit}_i \cdot 2^i, \quad \text{bit}_i \in \{0, 1\}$$
* **Arah Hashing:** Setiap $\text{bit}_i$ menentukan langsung posisi node kiri vs kanan: jika $\text{bit}_i = 0$, hash anak berada di kiri; jika $\text{bit}_i = 1$, hash anak berada di kanan.
* **Negative Test:** `test_gate_c0_tampered_leaf_index_fails_proving` membuktikan bahwa pemalsuan indeks seketika gagal saat proving/verifikasi.

### ✅ 2. Proteksi Integer Range 64-bit (Pencegah Modular Wrap-Around)
* **Implementasi:** Seluruh variabel nominal (`input_value`, `merchant_amount`, `protocol_fee`, `execution_fee`, dan `change_value`) didekomposisi menjadi 64 bit boolean gadget di sirkuit:
  $$\text{value} == \sum_{j=0}^{63} \text{bit}_j \cdot 2^j, \quad \text{value} < 2^{64}$$
* Mencegah prover mencetak uang virtual via overflow modulo $r$ pada scalar field $\mathbb{F}_r$.
* **Negative Test:** `test_gate_c0_overflow_amount_fails_proving` membuktikan bahwa injeksi nilai di luar rentang $2^{64}$ gagal membangkitkan proof.

### ✅ 3. Boolean Constraint Strict & Zero-Change Enforcement pada `has_change`
* **Implementasi:**
  1. Enforce constraint boolean: $\text{has\_change} \times (1 - \text{has\_change}) == 0$.
  2. Enforce jika `has_change == 0`, maka $\text{change\_value} == 0$ dan $\text{change\_commitment} == 0$.
  3. Enforce jika `has_change == 1`, maka $\text{change\_commitment} == \text{computed\_change\_cm}$.
* **Negative Tests:**
  - `test_gate_c0_non_boolean_has_change_fails_proving` (injeksi `has_change = 2` gagal).
  - `test_gate_c0_zero_change_with_positive_change_value_fails_proving` (injeksi `has_change = 0` dengan `change_value > 0` gagal).
  - `test_gate_c0_swapped_domain_fields_fails_verification` (swap domain field terdeteksi).
