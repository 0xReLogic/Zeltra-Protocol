# Nimbus Contract — 04: Spend, Pairing Check & Adaptive Batching

Dokumen ini menjelaskan logika eksekusi pembelanjaan (*spend*), verifikasi pairing on-chain EIP-2537, kalkulasi diskon fee berbasis holding-time, dan penggabungan batch pada [`nimbus-contracts/src/spend.rs`](file:///workspaces/Zeltra-Protocol/nimbus-contracts/src/spend.rs).

---

## 1. Verifikasi Pairing BLS On-Chain (EIP-2537)

Smart contract Nimbus memverifikasi tanda tangan BLS secara langsung di atas kurva BLS12-381 menggunakan precompile native **`BLS12_PAIRING_CHECK` (alamat `0x0f`)**:

$$\text{Persamaan Pairing:} \quad e(-\alpha, G_2) \cdot e(H(m), pk_{iss}) == 1$$

### Pembentukan Calldata Precompile (768 Bytes):
Input calldata berukuran tepat 768 bytes yang disusun dari 2 pasang titik $(G_1, G_2)$:

```text
Offset    Panjang   Komponen                 Keterangan
0..127    128 B     -alpha (G1)              Negated unmasked signature dari client
128..383  256 B     G2 Generator (G2)        Titik generator kurva G2 standar
384..511  128 B     H(m) (G1)                Hash pesan transaksi yang direkonstruksi on-chain
512..767  256 B     pk_iss (G2)              Kunci publik issuer terdaftar
```

* **Validasi Keras:** Kontrak menolak input berukuran salah, titik tak hingga (*point at infinity*), serta issuer key yang tidak terdaftar di `trusted_issuer_keys`.
* **Output Precompile:** Array 32 bytes di mana byte terakhir wajib bernilai `1` (`output[31] == 1`).

---

## 2. Rekonstruksi Pesan Mandiri On-Chain (Anti-Tamper)

Kontrak tidak mempercayai hash pesan yang dikirimkan oleh pemanggil transaksi. Kontrak **merekonstruksi sendiri** hash canonical `NIMBUS_SPEND_V1`:

```text
m_hash = keccak256(
    "NIMBUS_SPEND_V1",
    chain_id,
    contract_address,
    amount,
    recipient_or_intent_hash,
    expiry,
    nonce
)
```

Lalu memetakannya ke kurva menggunakan fungsi `hash_to_g1(&m_hash)` on-chain:
$$H(m) = \text{MapToCurve}(m\_hash) \in G_1$$

Kontrak memverifikasi kecocokan nullifier:
$$\text{nullifier} == \text{keccak256}(\text{to\_evm\_g1}(H(m)))$$
Jika nullifier tidak cocok dengan parameter transaksi, kontrak seketika revert dengan `NULLIFIER_MESSAGE_MISMATCH`.

---

## 3. Diskon Fee Berbasis Waktu (*Holding-Time Discount*)

Besaran biaya protokol (*protocol fee*) dihitung secara transparan di atas nominal invoice:

* **Tarif Default (< 30 Hari):** `45 bps` (0.45%).
* **Holding Time ≥ 30 Hari:** Diskon ke `40 bps` (0.40%).
* **Tier 7 Hari Dieliminasi:** Mencegah kompleksitas percabangan kode dan menutup celah manipulasi batas waktu.

### Pembuktian Waktu Tanpa Membocorkan Privasi:
Waktu kepemilikan dibuktikan melalui pendaftaran root himpunan asosiasi bersih:
$$\Delta t = \text{block.timestamp} - \text{clean\_association\_roots}[root]$$
Pengguna membuktikan keanggotaan dalam himpunan root tersebut tanpa membuka identitas kapan deposit mereka dilakukan secara persis. Kontrak meng-emit event `ProtocolFee` dan `ExecutionFee`.

---

## 4. Adaptive Batch Spend (`batch_spend`)

Untuk menghemat gas on-chain secara drastis, relayer dapat menggabungkan **2 sampai 8 transaksi spend** ke dalam satu transaksi on-chain via `batch_spend()`:

### Efisiensi Multi-Pairing:
Precompile EIP-2537 mendukung multi-pairing dalam satu pemanggilan call tunggal:
$$\text{Input Batch} = \text{Pair}_1 \parallel \text{Pair}_2 \parallel \dots \parallel \text{Pair}_N \quad (768 \times N \text{ bytes})$$

$$\prod_{i=1}^N \left( e(-\alpha_i, G_2) \cdot e(H(m_i), pk_{iss\_i}) \right) == 1$$

* **Penghematan Gas:** Biaya *base overhead* precompile hanya dibayar satu kali untuk seluruh batch, menghemat gas hingga 30-40% per transaksi.
* **Isolasi Kegagalan:** Jika ada satu item dalam batch yang revert, relayer memecah batch dan memproses item valid secara terpisah.

---

## 5. Klaim Komisi Relayer (`claim_execution_fees`)

Biaya eksekusi gas yang dibayarkan pengguna (`execution_fee`) diakumulasikan ke slot `accrued_execution_fee_liability` dan `accumulated_execution_fees`.
* Alamat yang berhak (`execution_fee_recipient`) dapat mencairkan komisi tersebut secara berkala dalam satu transaksi hemat gas.
* Penarikan komisi diverifikasi secara ketat terhadap invariant solvabilitas (`assets >= total_liabilities`) agar tidak menyentuh saldo kolateral milik depositor atau unspent note pengguna.
* Kontrak meng-emit event `FeeClaim(recipient, amount)`.

---

## 6. Private Note Spend & Groth16 Verifier (`spend_private_note` — Gate D)

Untuk pembelanjaan saldo privat ZK-UTXO dengan kembalian otomatis (*change note*), kontrak mengekspos entrypoint `spend_private_note(...)` yang memverifikasi proof Groth16 kurva BLS12-381 secara langsung on-chain via module [`groth16_note_verifier.rs`](file:///workspaces/Zeltra-Protocol/nimbus-contracts/src/groth16_note_verifier.rs).

### A. Format Proof & 12 Public Inputs
- **Proof:** $A \in \mathbb{G}_1$ (128B), $B \in \mathbb{G}_2$ (256B), $C \in \mathbb{G}_1$ (128B) — total 512 bytes uncompressed EVM.
- **12 Public Input Scalars (32B masing-masing, wajib kanonikal $< r$):**
  1. `merkle_root`: Root pohon LeanIMT depth 20 yang tercatat di `accepted_note_roots`.
  2. `input_nullifier`: Nullifier unik pencegah double spend di `note_nullifiers`.
  3. `output_commitment`: Komitmen kembalian (wajib 0 jika `has_change == 0`).
  4. `recipient`: Alamat EVM penerima merchant payout.
  5. `merchant_amount`: Nominal USDC yang diterima merchant.
  6. `protocol_fee`: Biaya protokol yang direalisasikan ke kas.
  7. `execution_fee`: Biaya gas relayer yang diakumulasikan.
  8. `quote_hash`: Hash penawaran eksekusi EIP-712 yang disetujui pengguna.
  9. `chain_id`: Domain chain EVM anti-cross-chain replay.
  10. `contract_address`: Alamat smart contract anti-cross-contract replay.
  11. `expiry`: Batas waktu transaksi block timestamp.
  12. `has_change`: Flag boolean ($0$ atau $1$) keberadaan kembalian.

### B. Pipeline Eksekusi EIP-2537:
1. **MSM `0x0c` (`BLS12_G1MSM`):** Menghitung kombinasi linear $\mathcal{L} = \text{IC}_0 + \sum_{i=1}^{12} x_i \text{IC}_i$ dalam 1 panggilan host batch.
2. **Pairing Check `0x0f` (`BLS12_PAIRING_CHECK`):** Mengevaluasi persamaan 4-pairing dalam buffer 1536 bytes:
   $$e(-A, B) \cdot e(\alpha, \beta) \cdot e(\mathcal{L}, \gamma) \cdot e(C, \delta) == 1$$

### C. Checks-Effects-Interactions:
- **Checks:** Validasi non-paused, non-expired, `execution_fee <= max_execution_fee`, `merchant_amount > 0`, input skalar kanonikal, root sah, dan nullifier belum pernah terpakai.
- **Effects:** Tandai `note_nullifiers.insert(input_nullifier, true)`, sisipkan `output_commitment` ke LeanIMT via `_merkle_insert` jika `has_change == 1`, kurangi `user_note_liability`, tambah liabilities komisi relayer dan protokol, serta verifikasi invariansi solvabilitas on-chain `check_solvency()`.
- **Interactions:** Transfer ERC-20 `merchant_amount` ke alamat `recipient`.
- **Events:** Emit event terindeks `PrivateNoteSpend`, `ProtocolFee`, dan `ExecutionFee`.

