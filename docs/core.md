# Nimbus Core: Cryptographic Architecture & Decisions

Dokumen ini mencatat keputusan arsitektur kriptografi (Architecture Decision Record - ADR) dan status implementasi komponen core dari **Nimbus Protocol**.

---

## 1. Algoritma Kriptografi yang Dipilih

Untuk mendukung konsep **Blockchain Anonymous Tokens (BAT)**, kami menggunakan skema **Blind Diffie-Hellman Key Exchange (BDHKE)** yang dibangun di atas kurva pairing-friendly.

### A. Kurva Eliptik: BLS12-381
*   **Mengapa BLS12-381?** Kurva ini adalah standar industri untuk pairing-friendly cryptography (digunakan oleh Ethereum 2.0, Zcash, Filecoin). 
*   **Struktur Grup:**
    *   $G_1$ (Order prime $p$): Digunakan untuk representasi *Blinded Message*, *Masked Signatures*, dan *Final Unmasked Signatures*. Elemen di $G_1$ berukuran kecil (48 bytes terkompresi), sehingga hemat gas fee saat dikirim atau diverifikasi on-chain.
    *   $G_2$ (Order prime $p$): Digunakan untuk *Issuer Public Key* dan *Masking Key Commitment*.
    *   $G_T$: Target group untuk operasi bilinear pairing $e: G_1 \times G_2 \to G_T$.
*   **Library Rust:** `ark-bls12-381` (dari ekosistem **Arkworks**) karena memiliki performa komputasi aljabar kurva tercepat dan tingkat audit keamanan yang tinggi.

### B. Hash-to-Curve ($G_1$)
*   Untuk keperluan verifikasi tanda tangan BLS, pesan (ephemeral public key $pk_{eph}$) harus dipetakan ke titik di grup $G_1$.
*   **Implementasi Saat Ini:** Kami menggunakan **SHA-256** untuk melakukan hashing data pesan menjadi nilai skalar $s \in \mathbb{Z}_p$, kemudian mengalikan generator $G_1$ dengan skalar tersebut ($s \cdot G_1$).
*   *Catatan Keamanan Masa Depan:* Untuk tingkat keamanan produksi, kami akan menggantinya dengan algoritma **Hash-to-Curve standar (RFC 9380)** untuk menghindari serangan hubungan discrete-log antar hash.

---

## 2. Rincian Alur Matematika BDHKE (Telah Diuji)

Berikut adalah formula matematika yang digunakan pada kode di `nimbus-core`:

```mermaid
graph LR
    subgraph Client
        Blind["X = r * H(m)"]
        Unblind["α = (r*k)⁻¹ * σ̃"]
    end
    subgraph Issuer
        Sign["σ̃ = (k*sk_iss) * X"]
        Commit["com_k = k * pk_iss"]
    end
    Client -->|X| Issuer
    Issuer -->|σ̃, com_k| Client
```

1.  **Blinding (Client):**
    $$X = r \cdot H(m) \in G_1 \quad \text{dimana } r \leftarrow_R \mathbb{Z}_p$$
2.  **Masked Signing (Issuer):**
    $$com_k = k \cdot pk_{iss} \in G_2 \quad \text{dimana } k \leftarrow_R \mathbb{Z}_p$$
    $$\tilde{\sigma} = (k \cdot sk_{iss}) \cdot X \in G_1$$
3.  **Masked Verification (Client - Verifikasi Off-chain):**
    $$e(\tilde{\sigma}, G_2) == e(X, com_k)$$
    *Pembuktian:*
    $$e((k \cdot sk_{iss}) \cdot (r \cdot H(m)), G_2) = e(r \cdot H(m), k \cdot sk_{iss} \cdot G_2) = e(X, com_k)$$
4.  **Unmasking (Client - Setelah $k$ dipublish di blockchain):**
    $$\alpha = (r \cdot k)^{-1} \cdot \tilde{\sigma} \in G_1$$
    *Pembuktian:*
    $$\alpha = (r \cdot k)^{-1} \cdot (k \cdot sk_{iss}) \cdot (r \cdot H(m)) = sk_{iss} \cdot H(m)$$
    *(menghasilkan tanda tangan BLS standar atas $m$ oleh Issuer).*
5.  **Final Verification (Verifier/Smart Contract):**
    $$e(\alpha, G_2) == e(H(m), pk_{iss})$$

## 3. Matematika Transaksi Offline & Pembagian Rahasia
Untuk memfasilitasi transaksi offline tanpa ancaman pembelanjaan ganda (*double-spend*), `nimbus-core` menerapkan skema pembagian rahasia Shamir 2-titik pada medan skalar $\mathbb{Z}_p$ (kurva BLS12-381 `Fr`).

1. **Persamaan Garis Rahasia (Dompet Pengirim)**:
   $$y = a \cdot x + I \pmod p$$
   Di mana:
   * $I \in \mathbb{Z}_p$ adalah kunci rahasia identitas pemilik koin.
   * $a \in \mathbb{Z}_p$ adalah slope/kemiringan acak yang dibentuk oleh dompet saat minting koin.
   * $x$ adalah nilai tantangan (*challenge*) dari merchant.
   * $y$ adalah respon hasil kalkulasi skalar.

2. **Pembelanjaan Tunggal (Jujur)**:
   * Pengirim memberikan pasangan $(x_1, y_1)$ ke Merchant A.
   * Identitas $I$ aman penuh karena $a$ dan $I$ tidak dapat dicari hanya dari satu titik koordinat.

3. **Pembelanjaan Ganda (Kecurangan)**:
   * Pengirim memberikan koin yang sama ke Merchant B, menghasilkan koordinat $(x_2, y_2)$ dengan $x_1 \neq x_2$.
   * Smart contract menerima $(x_1, y_1)$ dan $(x_2, y_2)$ lalu merekonstruksi identitas:
     $$a = \frac{y_2 - y_1}{x_2 - x_1} \pmod p$$
     $$I = y_1 - a \cdot x_1 \pmod p$$

### A. Interoperabilitas EVM Big-Endian (Format Precompile)
Secara default, Arkworks memproses representasi koordinat kurva secara *little-endian*. Namun, precompile EVM EIP-2537 (`0x0e` & `0x0f`) membutuhkan data dalam format *big-endian* uncompressed (dilapisi padding 16-byte nol di setiap koordinat Fr 48-byte untuk membentuk blok 64-byte). 

Di dalam `nimbus-core`, kami menambahkan fungsi pembantu berikut:
*   `to_evm_g1`: Mengubah titik $G_1$ menjadi representasi EVM G1 128-byte.
*   `to_evm_g2`: Mengubah titik $G_2$ menjadi representasi EVM G2 256-byte.
*   `get_alpha_neg_evm`: Menghasilkan tanda tangan ter-unmask yang dinegasikan ($-alpha$) dalam format EVM G1.
*   `get_hm_evm`: Menghasilkan representasi EVM G1 dari hash pesan $H(m)$.
*   `get_pk_iss_evm`: Menghasilkan representasi EVM G2 dari kunci publik issuer $pk_{iss}$.

---

## 4. Status Pengujian & Performa

*   **Status Unit Test:** `LULUS` (semua pengujian integrasi core dan contracts lulus).
*   **Waktu Pembuatan Tanda Tangan (Client Blinding):** $< 0.1$ milidetik.
*   **Waktu Verifikasi (Pairing Check):** $\approx 1.5$ milidetik (diukur secara lokal).
*   **Ukuran Token Anonim:** $\approx 144$ bytes (sangat ringkas dibanding ZK-proof).

---

## 5. Rencana Kerja Selanjutnya (Status Milestones)

*   [x] **Milestone 1: nimbus-contracts (Arbitrum Stylus)**
    *   Implementasi logika verifikasi $k \cdot pk_{iss} == com_k$ menggunakan precompile EIP-2537.
    *   Penyimpanan Nullifier on-chain dan fungsi pemotongan jaminan `slash_double_spender` untuk transaksi offline.
*   [x] **Milestone 2: nimbus-cli**
    *   Wallet CLI lengkap untuk simulasi blinding, signing, unmasking, verifikasi, spend offline, dan slashing double spenders.
*   [x] **Milestone 3: nimbus-sdk (WASM compiler)**
    *   Kompilasi Rust ke WebAssembly (WASM) yang mengekspos semua fungsionalitas wallet (termasuk generator respon offline dan rekonstruksi identitas) ke JavaScript.

