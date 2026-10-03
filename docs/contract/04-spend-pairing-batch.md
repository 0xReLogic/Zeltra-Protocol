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

* **Tarif Default:** `25 bps` (0.25%).
* **Holding Time ≥ 7 Hari:** Diskon ke `20 bps` (0.20%).
* **Whale Holding Time ≥ 30 Hari:** Diskon ke `10 bps` (0.10%).

### Pembuktian Waktu Tanpa Membocorkan Privasi:
Waktu kepemilikan dibuktikan melalui pendaftaran root himpunan asosiasi bersih:
$$\Delta t = \text{block.timestamp} - \text{clean\_association\_roots}[root]$$
Pengguna membuktikan keanggotaan dalam himpunan root tersebut tanpa membuka identitas kapan deposit mereka dilakukan secara persis.

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

Biaya eksekusi gas yang dibayarkan pengguna (`execution_fee`) diakumulasikan ke slot `accumulated_execution_fees`. 
* Alamat yang berhak (`execution_fee_recipient`) dapat mencairkan komisi tersebut secara berkala dalam satu transaksi hemat gas.
* Penarikan komisi diverifikasi terhadap invariant solvabilitas agar tidak menyentuh saldo kolateral milik depositor.
