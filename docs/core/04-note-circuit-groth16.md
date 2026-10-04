# Nimbus Core — 04: Private Note Circuit (Groth16 ZK-SNARK)

Dokumen ini menjelaskan desain sirkuit ZK-SNARK **Groth16** pada [`nimbus-core/src/note_circuit.rs`](file:///workspaces/Zeltra-Protocol/nimbus-core/src/note_circuit.rs), format 12 public inputs yang dibinding ke smart contract Stylus, serta ekstraksi Verifying Key EVM.

---

## 1. Arsitektur Sirkuit (`PrivateNoteCircuit`)

`PrivateNoteCircuit` membuktikan validitas pembelanjaan private note (1 input note $\to$ 1 merchant payout + 1 change output) tanpa membocorkan identitas pengirim, nilai saldo awal, maupun nilai kembaliannya.

### A. Public Inputs (12 Elemen Skalar $\mathbb{F}_r$ yang Dibinding ke Smart Contract):
1. `merkle_root`: Root pohon LeanIMT depth 20 yang sah di contract.
2. `input_nullifier`: Penanda unik note input pencegah double spend.
3. `output_commitment`: Komitmen note kembalian (wajib $0$ jika `has_change == 0`).
4. `recipient`: Alamat EVM penerima merchant payout.
5. `merchant_amount`: Nominal USDC yang dibayarkan ke merchant.
6. `protocol_fee`: Biaya protokol yang direalisasikan.
7. `execution_fee`: Biaya gas relayer yang diakumulasikan.
8. `quote_hash`: Hash penawaran eksekusi EIP-712 yang disetujui pengguna.
9. `chain_id`: Domain chain EVM anti-cross-chain replay.
10. `contract_address`: Alamat smart contract anti-cross-contract replay.
11. `expiry`: Batas waktu transaksi block timestamp.
12. `has_change`: Flag boolean ($0$ atau $1$) keberadaan kembalian.

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

## 3. Ekstraksi Verifying Key untuk Precompile EIP-2537

Untuk memverifikasi proof on-chain tanpa overhead memori arkworks, verifier on-chain mengekspor verifying key dalam format big-endian EVM uncompressed:
* Generator fungsi: `generate_note_circuit_evm_vk` pada [`nimbus-core/src/note_circuit.rs`](file:///workspaces/Zeltra-Protocol/nimbus-core/src/note_circuit.rs).
* **Komponen Konstanta Kunci:**
  - $\alpha \in \mathbb{G}_1$ (128 bytes)
  - $\beta, \gamma, \delta \in \mathbb{G}_2$ (masing-masing 256 bytes)
  - $\text{IC}_0 \dots \text{IC}_{12} \in \mathbb{G}_1$ (13 titik, masing-masing 128 bytes)
* Di-embed langsung sebagai byte arrays di [`nimbus-contracts/src/groth16_note_verifier.rs`](file:///workspaces/Zeltra-Protocol/nimbus-contracts/src/groth16_note_verifier.rs), siap diproses oleh precompile host Stylus `0x0c` (MSM) dan `0x0f` (Pairing Check).


