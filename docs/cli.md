# Nimbus CLI: Command Line Interface & Usage Guide

Dokumen ini menjelaskan cara menjalankan utilitas command-line **Nimbus Protocol** untuk menyimulasikan alur kriptografi token anonim (BAT) secara lokal.

---

## 1. Struktur Modul

Nimbus CLI telah direfaktor menjadi struktur modular yang terorganisir dengan baik:

```
nimbus-cli/src/
├── main.rs              # Entry point minimal (8 baris)
├── cli.rs               # Definisi CLI Clap (pure definitions)
└── commands/
    ├── mod.rs           # Command dispatcher (handle_command)
    ├── keys.rs          # Key generation & splitting (2 commands)
    ├── blind.rs         # Blind signature protocol (5 commands)
    └── threshold.rs     # Threshold aggregation (1 command)
```

### Keunggulan Struktur Modular:
- **Pemisahan Tanggung Jawab**: Definisi CLI terpisah dari logika command
- **Modularitas Tinggi**: Setiap kategori command memiliki modul sendiri
- **Maintainability**: Mudah menambah command baru tanpa mengubah file lain
- **Testability**: Setiap modul command dapat ditest secara independen
- **Dokumentasi Lengkap**: 26 doc comments tersebar di semua modul

### Organisasi Command:
- **keys.rs**: Operasi kriptografi kunci (generate, split)
- **blind.rs**: Protokol blind signature penuh (blind, sign, verify, unmask)
- **threshold.rs**: Agregasi threshold signature terdistribusi

---

## 2. Instalasi & Build

Pastikan Anda berada di direktori root project, lalu jalankan perintah compile berikut:
```bash
cargo build --bin nimbus-cli
```
Hasil compile binary akan berada di lokasi: `./target/debug/nimbus-cli`.

---

## 3. Alur Simulasi Langkah-demi-Langkah (End-to-End)

### Langkah 1: Membuat Keypair Issuer
Issuer men-generate kunci rahasia ($sk_{iss}$) dan kunci publik ($pk_{iss}$):
```bash
./target/debug/nimbus-cli generate-keys
```
*Output akan menampilkan `Secret Key` dan `Public Key` dalam format Hex.*

### Langkah 2: Blinding Pesan (Client)
Client memasukkan pesan (ephemeral public key) untuk disamarkan:
```bash
./target/debug/nimbus-cli blind --message "ephemeral_public_key_12345"
```
*Output:*
*   `Blinded Message (X)` (dalam Hex)
*   `Blinding Factor (r)` (dalam Hex)

### Langkah 3: Menandatangani Pesan Masked (Issuer)
Issuer menandatangani `Blinded Message` menggunakan kunci rahasia miliknya dan kunci masking sekali pakai ($k$):
```bash
./target/debug/nimbus-cli sign --blinded <BLINDED_MESSAGE_HEX> --sk <SECRET_KEY_HEX>
```
*Output:*
*   `Masked Signature (sigma_tilde)` (dalam Hex)
*   `Masking Key (k)` (dalam Hex)
*   `Commitment (com_k)` (dalam Hex)

### Langkah 4: Verifikasi Masked Signature Off-Chain (Client)
Client memverifikasi bahwa tanda tangan dari issuer cocok dengan pesan terbutakan dan komitmen kunci masking:
```bash
./target/debug/nimbus-cli verify-masked \
  --blinded <BLINDED_MESSAGE_HEX> \
  --com-k <COMMITMENT_HEX> \
  --masked-sig <MASKED_SIGNATURE_HEX>
```
*Output:* `VALID: Masked signature matches blinded message and commitment!`

### Langkah 5: Unmasking Tanda Tangan (Client)
Setelah kunci masking $k$ dirilis on-chain, client melakukan unmasking untuk mendapatkan tanda tangan final ($\alpha$):
```bash
./target/debug/nimbus-cli unmask \
  --masked-sig <MASKED_SIGNATURE_HEX> \
  --r <BLINDING_FACTOR_HEX> \
  --k <MASKING_KEY_HEX>
```
*Output:* `Unmasked Signature (alpha)` (dalam Hex)

### Langkah 6: Verifikasi Tanda Tangan Final (Verifier)
Verifier (misal: merchant atau smart contract) memverifikasi tanda tangan final yang bersih terhadap kunci publik issuer ($pk_{iss}$):
```bash
./target/debug/nimbus-cli verify \
  --message "ephemeral_public_key_12345" \
  --sig <UNMASKED_SIGNATURE_HEX> \
  --pk <PUBLIC_KEY_HEX>
```
*Output:* `VALID: The signature is verified and authentic under the Issuer's Public Key!`

---

## 4. Simulasi Transaksi Offline & Slashing Belanja Ganda

### Langkah 1: Membuat Parameter Offline (Identitas, Slope, Challenge)
Wallet client menghasilkan kunci rahasia identitas ($I$), kemiringan acak ($a$), dan nilai tantangan awal dari merchant ($x$):
```bash
./target/debug/nimbus-cli generate-offline-params
```
*Output menampilkan `Identity Secret (I)`, `Slope Parameter (a)`, dan `Default Challenge (x)`.*

### Langkah 2: Menghasilkan Bukti Offline (Spend Offline)
Client menjawab tantangan merchant offline dengan parameter garis rahasianya:
```bash
./target/debug/nimbus-cli spend-offline \
  -a <SLOPE_A_HEX> \
  -x <CHALLENGE_X_HEX> \
  --identity <IDENTITY_I_HEX>
```
*Output:*
*   `Challenge (x)` (dalam Hex)
*   `Response (y)` (dalam Hex)

### Langkah 3: Eksekusi Slashing Kecurangan (Challenger/Smart Contract)
Jika pengguna curang dan melakukan transaksi belanja ganda ke Merchant A (tantangan $x_1$, respon $y_1$) dan Merchant B (tantangan $x_2$, respon $y_2$), smart contract memproses penyitaan saldo jaminan:
```bash
./target/debug/nimbus-cli slash \
  --x1 <CHALLENGE_X1_HEX> \
  --y1 <RESPONSE_Y1_HEX> \
  --x2 <CHALLENGE_X2_HEX> \
  --y2 <RESPONSE_Y2_HEX>
```
*Output:* `DOUBLE SPENDER DETECTED & SLASHED` disertai dengan nilai `Reconstructed Identity (I)` asli yang dibongkar dari pelaku kecurangan.

---

## 5. Mode Terdistribusi (Threshold Signature Ceremony t=3, n=5)

Selain mode single-issuer, Nimbus CLI mendukung upacara tanda tangan threshold terdistribusi menggunakan skema Shamir Secret Sharing.

### Langkah 1: Membagi Kunci Rahasia (Shamir Secret Sharing)
Bagi kunci rahasia issuer ($sk_{iss}$) menjadi $n$ share dengan threshold $t$:
```bash
./target/debug/nimbus-cli split-key \
  --sk <SECRET_KEY_HEX> \
  --threshold 3 \
  --total 5
```
*Output akan menghasilkan 5 share key terenkode hex (80 karakter / 40 byte, yang menyimpan tuple `(usize, Fr)`).*

### Langkah 2: Mengagregasi Tanda Tangan Threshold
Setelah mendapatkan minimal $t$ (dalam hal ini 3) partial signature dari relayer node/guardians, lakukan agregasi:
```bash
./target/debug/nimbus-cli aggregate \
  --indices 1,2,3 \
  --signatures <SIG1_HEX>,<SIG2_HEX>,<SIG3_HEX>
```
*Output:* `Masked Signature (sigma_tilde)` teragregasi yang siap di-unmask menggunakan perintah `unmask`.


