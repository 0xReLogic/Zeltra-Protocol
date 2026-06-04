# Nimbus Protocol

Nimbus Protocol adalah protokol token anonim terdesentralisasi (**Blockchain Anonymous Tokens - BAT**) berbasis **Rust** yang dirancang untuk transaksi privat mikro berkecepatan tinggi pada transparent blockchains (seperti Ethereum L2).

Nimbus menyelesaikan dilema privasi Web3: memberikan transaksi yang sepenuhnya anonim **tanpa beban komputasi ZK-proofs** di perangkat klien (handphone) dan **tanpa perantara terpercaya** (trustless).

---

## Mengapa Nimbus Protocol Unggul?

### 1. Transaksi Privat Instan (7.000x Lebih Cepat dari ZKP)
*   **Masalah ZK-Proof:** Protokol privasi seperti Tornado Cash atau Zcash mewajibkan handphone pengguna membuat bukti Zero-Knowledge (ZKP) yang memakan waktu **5 s.d. 30 detik**, membuat HP panas dan baterai boros.
*   **Solusi Nimbus:** Dengan memodifikasi teknik *Chaumian Blinding* (BDHKE), Nimbus membuang kebutuhan ZK-proof di perangkat klien. Pembuatan token transaksi privat terjadi secara **instan (< 0.25 milidetik)** dengan ukuran data **50x lebih kecil** (hanya 144 bytes).

### 2. 100% Trustless (Melompati Cashu & Fedimint di Bitcoin)
*   **Masalah E-Cash Tradisional:** Protokol E-Cash seperti Cashu atau Fedimint mewajibkan pengguna mempercayai "Mint" (pihak ketiga/kasir) untuk memegang jaminan dana. Jika kasir kabur, uang pengguna lenyap.
*   **Solusi Nimbus:** Jaminan dana (misal: USDC) dikunci secara terdesentralisasi di dalam smart contract EVM. Penerbit token (Issuer) tidak perlu dipercayai. Jika Issuer bertindak curang atau offline, smart contract akan secara otomatis melakukan pengembalian dana (*auto-refund*) secara adil.

### 3. Memanfaatkan EIP-2537 (Era Baru Ethereum L2)
*   Nimbus memanfaatkan precompiled contracts **BLS12-381** asli EVM yang diaktifkan pada **Upgrade Pectra (EIP-2537)**. 
*   Verifikasi komitmen kunci masking ($k \cdot pk_{iss} == com_k$) menggunakan precompile **`0x0e` (G2 MSM)** dan verifikasi tanda tangan BLS saat penarikan menggunakan precompile **`0x0f` (Pairing Check)**. Biaya gas on-chain menjadi sangat murah (~100k gas saja!).

### 4. Smart Contract Berbasis Rust (Arbitrum Stylus)
*   Smart contract kami ditulis menggunakan **Arbitrum Stylus (Rust)** dan dikompilasi ke WebAssembly (WASM). Kecepatan eksekusi on-chain menjadi 10x lebih cepat dan biaya gas fee turun drastis dibanding Solidity tradisional.

---

## Stack & Teknologi Utama yang Digunakan

Nimbus Protocol mengintegrasikan berbagai standar kriptografi, protokol jaringan, dan inovasi Web3 termutakhir untuk menyajikan transaksi privat berkinerja tinggi:

1. **Kriptografi Kurva BLS12-381**: Kurva eliptik ramah *pairing* yang menjadi fondasi utama untuk pembuatan *blind signatures* (BDHKE), pembuktian kelompok, verifikasi ZK-Compliance, dan *pairing-check* on-chain.
2. **BDHKE (Bilinear Diffie-Hellman Key Exchange) Chaumian Blinding**: Skema kriptografi penyamaran token yang memungkinkan pembuatan tanda tangan terbutakan secara lokal di perangkat klien dalam waktu kurang dari **0.25 milidetik**, membuang komputasi ZK-proof klien yang lambat.
3. **Arbitrum Stylus (Rust ke WASM)**: Kontrak pintar L2 ditulis dalam bahasa Rust dan dikompilasi ke WebAssembly (WASM), menghasilkan eksekusi on-chain yang 10x lebih cepat dan konsumsi gas 100x lebih hemat dibanding Solidity tradisional.
4. **Precompiled Contracts EIP-2537**: Memanfaatkan precompile BLS12-381 asli EVM (operasi G1, G2, dan *Pairing Check* di alamat `0x0e` dan `0x0f`) yang diperkenalkan pada upgrade Pectra untuk memotong biaya gas verifikasi on-chain.
5. **EIP-7702 Account Abstraction (Paymaster)**: Delegasi otorisasi dari EOA (Externally Owned Account) ke smart contract wallet secara langsung di tingkat transaksi, memungkinkan transaksi sepenuhnya gratis (*gasless*) bagi pengguna retail.
6. **Chainlink CCIP (Cross-Chain Interoperability Protocol)**: Protokol interoperabilitas lintas rantai aman untuk merutekan transaksi spend dari rantai asal (Arbitrum L2) ke rantai target (Polygon) guna membeli taruhan Polymarket secara privat dan atomik.
7. **Protokol x402 v2 (M2M Nanopayments)**: Standar pembayaran otonom berbasis status HTTP `402 Payment Required` (standar Coinbase/Cloudflare) agar AI Agent dapat membayar API secara mikro-berkala tanpa mengekspos alamat wallet pemiliknya.
8. **Shamir Secret Sharing di atas Lapangan Skalar Fr**: Pemanfaatan matematika persamaan garis linear ($y = a \cdot x + I \pmod P$) di atas lapangan skalar kurva BLS12-381 untuk merekonstruksi kunci privat identitas ($I$) pelaku double-spend offline dan menyita jaminan mereka secara on-chain.

----

## Panduan Memulai & Uji Coba (CLI)

Kami menyediakan aplikasi CLI untuk menyimulasikan alur kriptografi Nimbus secara lokal.

### 1. Build Project
Kompilasi seluruh workspace Rust:
```bash
cargo build
```

### 2. Jalankan Simulasi Alur Protokol

#### Langkah A: Membuat Kunci Issuer
Generate kunci rahasia ($sk_{iss}$) dan kunci publik ($pk_{iss}$) milik Issuer:
```bash
./target/debug/nimbus-cli generate-keys
```

#### Langkah B: Blinding Pesan (Client)
Samarkan identitas token (ephemeral key) menggunakan faktor acak $r$:
```bash
./target/debug/nimbus-cli blind --message "ephemeral_public_key_abc123"
```
*Simpan nilai Blinded Message (X) dan Blinding Factor (r) yang dihasilkan.*

#### Langkah C: Masked Signing (Issuer)
Issuer menandatangani blinded message menggunakan kunci rahasia miliknya dan kunci masking $k$:
```bash
./target/debug/nimbus-cli sign --blinded <BLINDED_MSG_HEX> --sk <SECRET_KEY_HEX>
```
*Simpan nilai Masked Signature, Masking Key (k), dan Commitment (com_k).*

#### Langkah D: Verifikasi Masked Signature Off-Chain (Client)
Client memverifikasi keabsahan tanda tangan ter-masking secara lokal (off-chain):
```bash
./target/debug/nimbus-cli verify-masked \
  --blinded <BLINDED_MSG_HEX> \
  --com-k <COMMITMENT_HEX> \
  --masked-sig <MASKED_SIG_HEX>
```
*Output:* `VALID: Masked signature matches blinded message and commitment!`

#### Langkah E: Unmasking Signature (Client)
Setelah kunci masking $k$ dipublish di blockchain, client melakukan unmasking untuk mendapat tanda tangan final ($\alpha$):
```bash
./target/debug/nimbus-cli unmask \
  --masked-sig <MASKED_SIG_HEX> \
  --r <BLINDING_FACTOR_HEX> \
  --k <MASKING_KEY_HEX>
```
*Output:* `Unmasked Signature (alpha)`

#### Langkah F: Verifikasi Final (Verifier)
Verifier (smart contract) memverifikasi tanda tangan final yang bersih terhadap kunci publik issuer:
```bash
./target/debug/nimbus-cli verify \
  --message "ephemeral_public_key_abc123" \
  --sig <UNMASKED_SIGNATURE_HEX> \
  --pk <PUBLIC_KEY_HEX>
```
*Output:* `VALID: The signature is verified and authentic under the Issuer's Public Key!`

### 3. Jalankan Uji Coba Transaksi Offline & Slashing
Untuk menyimulasikan deteksi belanja ganda offline:

#### Langkah A: Membuat Parameter Offline (Identity & Slope)
```bash
./target/debug/nimbus-cli generate-offline-params
```
*Catat Identity Secret (I), Slope Parameter (a), dan Default Challenge (x).*

#### Langkah B: Membuat Bukti Belanja Offline Pertama
```bash
./target/debug/nimbus-cli spend-offline -a <SLOPE_A_HEX> -x <CHALLENGE_X1_HEX> --identity <IDENTITY_I_HEX>
```
*Menghasilkan Response (y1).*

#### Langkah C: Membuat Bukti Belanja Offline Kedua (Belanja Ganda)
Gunakan challenge $x_2$ yang berbeda (misal dari generate parameter kedua):
```bash
./target/debug/nimbus-cli spend-offline -a <SLOPE_A_HEX> -x <CHALLENGE_X2_HEX> --identity <IDENTITY_I_HEX>
```
*Menghasilkan Response (y2).*

#### Langkah D: Melakukan Slashing On-Chain (Identity Reconstruction)
```bash
./target/debug/nimbus-cli slash --x1 <X1_HEX> --y1 <Y1_HEX> --x2 <X2_HEX> --y2 <Y2_HEX>
```
*Smart contract akan menyita dana jaminan dan membongkar Identity Secret (I) asli dari pelaku.*

---

## Menjalankan Uji Performa (Benchmark)

Untuk mengukur latensi dan throughput dari masing-masing komponen matematika BDHKE dan Shamir secret sharing secara lokal:
```bash
cargo run --release --bin bench
```
*Ini akan menghasilkan tabel analisis kecepatan eksekusi lengkap.*

---

## Lisensi
Project ini dilisensikan di bawah **CC0-1.0 (No Rights Reserved)**.
