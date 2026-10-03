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

## 3. Titik Macet: Audit Security Blocker (Gate C0)

Ref: [`docs/todos/private-note-balance.md`](file:///workspaces/Zeltra-Protocol/docs/todos/private-note-balance.md) & [`DEC-016B`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-016B-mvp-circuit-shortcuts.md)

Audit independen menemukan 3 celah fatal pada prototype MVP `PrivateNoteCircuit` yang harus diperbaiki pada **Gate C0**:

### ⚠️ Blocker 1: Path Merkle Belum Terikat ke `leaf_index`
* **Masalah:** Arah hashing kiri/kanan pada Merkle path saat ini belum dipaksa identik dengan bit-bit representasi biner `input_leaf_index`.
* **Dampak Keamanan:** Attacker dapat memanipulasi `leaf_index` palsu untuk note yang sama, sehingga menghasilkan dua nullifier berbeda dari satu note $\to$ **Double-Spending Exploit**.
* **Solusi Perbaikan:** Dekomposisi `leaf_index` menjadi 20 bit boolean di dalam sirkuit dan enforce:
  $$\text{leaf\_index} == \sum_{i=0}^{19} \text{bit}_i \cdot 2^i$$
  Gunakan bit yang sama untuk menentukan urutan hashing cabang kiri/kanan.

### ⚠️ Blocker 2: Rentan Modular Wrap-Around (Aritmatika Field Scalar)
* **Masalah:** Nilai `amount`, `fee`, dan `change_value` dihitung di atas finite field $\mathbb{F}_r$ tanpa pemeriksaan batas integer 64-bit.
* **Dampak Keamanan:** Jika ada nilai yang overflow modulo $p$, prover bisa mencetak uang virtual dari hasil *wrap-around* aljabar medan skalar.
* **Solusi Perbaikan:** Pasang range constraint strict $\in [0, 2^{64})$ pada seluruh variabel nominal menggunakan gadget bit-decomposition Arkworks.

### ⚠️ Blocker 3: `has_change` Belum Di-Boolean-Constrain
* **Masalah:** Hubungan output saat ini: $\text{output} == \text{has\_change} \times \text{change\_cm}$.
* **Dampak Keamanan:** Jika prover menyuntikkan `has_change = 2` (bukan 0 atau 1), output menjadi dua kali lipat komitmen.
* **Solusi Perbaikan:** Tambahkan constraint boolean 1 baris:
  $$\text{has\_change} \times (1 - \text{has\_change}) == 0$$
