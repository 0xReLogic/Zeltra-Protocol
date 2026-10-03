# Nimbus Core — 02: BDHKE & Threshold Signatures

Dokumen ini menjelaskan alur matematika **Blind Diffie-Hellman Key Exchange (BDHKE)** dan pembagian kunci ambang batas (*Shamir Secret Sharing*) yang diimplementasikan pada [`nimbus-core/src/blind_sign.rs`](file:///workspaces/Zeltra-Protocol/nimbus-core/src/blind_sign.rs) dan [`nimbus-core/src/threshold.rs`](file:///workspaces/Zeltra-Protocol/nimbus-core/src/threshold.rs).

---

## 1. Alur Matematika Protokol BDHKE

Nimbus menggunakan varian Chaumian Blind Signature di atas kurva BLS12-381 untuk memutus hubungan pelacakan on-chain antara depositor dan pembayar (*spender*).

```text
Client                                  Issuer / Leader
  |                                           |
  | 1. Blinding:                              |
  |    X = r * H(m)                           |
  |                                           |
  |-------- Kirim Blinded Message X --------->|
  |                                           |
  |                                           | 2. Masked Signing:
  |                                           |    com_k = k * pk_iss
  |                                           |    sigma_tilde = (k * sk_iss) * X
  |                                           |
  |<--- Kirim (sigma_tilde, com_k) -----------|
  |     (Kunci k ditahan)                     |
  |                                           |
  | 3. Off-Chain Masked Verification:         |
  |    e(sigma_tilde, G2) == e(X, com_k)      |
  |                                           |
  |========== Transaksi Deposit USDC =========|
  |                                           |
  |<--- Kunci Masking k dirilis on-chain -----|
  |                                           |
  | 4. Unmasking:                             |
  |    alpha = (r * k)⁻¹ * sigma_tilde        |
  |                                           |
  | 5. Final On-Chain Verification:           |
  |    e(alpha, G2) == e(H(m), pk_iss)        |
```

### Penjelasan Langkah & Pembuktian Matematis:

1. **Blinding (Client):**
   * Client memilih faktor pembuta rahasia $r \leftarrow_R \mathbb{Z}_p$.
   * Menghitung pesan terbutakan:
     $$X = r \cdot H(m) \in G_1$$
2. **Masked Signing (Issuer):**
   * Issuer memilih kunci masking rahasia sekali pakai $k \leftarrow_R \mathbb{Z}_p$.
   * Menghasilkan komitmen publik dan tanda tangan bertopeng:
     $$com_k = k \cdot pk_{iss} = k \cdot (sk_{iss} \cdot G_2) \in G_2$$
     $$\tilde{\sigma} = (k \cdot sk_{iss}) \cdot X \in G_1$$
3. **Masked Verification (Client - Verifikasi Off-chain):**
   * Client membuktikan tanda tangan valid sebelum deposit tanpa perlu mengetahui nilai $k$:
     $$e(\tilde{\sigma}, G_2) == e(X, com_k)$$
   * *Pembuktian:*
     $$e(\tilde{\sigma}, G_2) = e((k \cdot sk_{iss}) \cdot (r \cdot H(m)), G_2) = e(r \cdot H(m), k \cdot sk_{iss} \cdot G_2) = e(X, com_k)$$
4. **Unmasking (Client):**
   * Setelah deposit confirmed dan $k$ dirilis oleh leader:
     $$\alpha = (r \cdot k)^{-1} \cdot \tilde{\sigma} \in G_1$$
   * *Pembuktian:*
     $$\alpha = (r \cdot k)^{-1} \cdot (k \cdot sk_{iss}) \cdot (r \cdot H(m)) = sk_{iss} \cdot H(m)$$
     *(Menghasilkan tanda tangan BLS standar atas $m$ yang bersih).*
5. **Final Verification (On-Chain Smart Contract):**
   * Smart contract Stylus memverifikasi tanda tangan $\alpha$ langsung terhadap $pk_{iss}$:
     $$e(-\alpha, G_2) \cdot e(H(m), pk_{iss}) == 1$$

---

## 2. Threshold Cryptography (Shamir Secret Sharing)

File referensi: [`nimbus-core/src/threshold.rs`](file:///workspaces/Zeltra-Protocol/nimbus-core/src/threshold.rs)

Untuk menghindari *single point of failure*, kunci rahasia issuer $sk_{iss}$ dipecah ke jaringan **Guardian** menggunakan skema $(t, n)$-threshold (default: 3-of-5).

### Pembagian Kunci (`split_secret_key`)
Dibuat polinomial acak berderajat $t-1$:
$$f(x) = sk_{iss} + a_1 x + a_2 x^2 + \dots + a_{t-1} x^{t-1} \pmod p$$
Di mana:
* $f(0) = sk_{iss}$
* Setiap guardian $i \in [1, n]$ menerima share privat $s_i = f(i) \in \mathbb{Z}_p$.

### Penandatanganan Parsial (`sign_share`)
Ketika client mengirimkan $X$, leader memilih $k$ acak dan meminta $t$ guardian menandatangani:
$$\tilde{\sigma}_i = (k \cdot s_i) \cdot X \in G_1$$

### Rekonstruksi Tanda Tangan (`aggregate_shares`)
Leader merekonstruksi masked signature lengkap menggunakan **Koefisien Interpolasi Lagrange**:
$$\lambda_i(0) = \prod_{j \in S, j \neq i} \frac{0 - j}{i - j} = \prod_{j \in S, j \neq i} \frac{-j}{i - j} \pmod p$$

Signature gabungan:
$$\tilde{\sigma} = \sum_{i \in S} \lambda_i(0) \cdot \tilde{\sigma}_i = \sum_{i \in S} \lambda_i(0) \cdot (k \cdot f(i)) \cdot X = k \cdot f(0) \cdot X = (k \cdot sk_{iss}) \cdot X$$

Hasil agregasi $\tilde{\sigma}$ identik secara matematis dengan tanda tangan single-issuer, sehingga smart contract on-chain tidak perlu tahu berapa jumlah guardian yang menandatangani.
