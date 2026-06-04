# Analisis Riset: Transaksi Offline dengan Pengungkapan Identitas Kriptografis

Dokumen riset ini mengusulkan perluasan protokol **Nimbus** untuk mendukung transaksi privat secara **offline** (tanpa koneksi internet) menggunakan skema pembagian rahasia linear untuk mendeteksi dan menghukum pelaku kecurangan *double-spending*.

---

## 1. Masalah Pembayaran Offline di Web3

Pada sistem blockchain tradisional, transaksi membutuhkan koneksi internet aktif agar validator dapat memverifikasi bahwa pengirim memiliki saldo yang cukup dan belum membelanjakan koin tersebut ke tempat lain.

Jika transaksi dilakukan secara offline (misal: di kereta bawah tanah atau area terpencil):
*   Pengirim dapat memberikan token privat yang sama ke Merchant A dan Merchant B.
*   Kedua merchant menerima transaksi tersebut secara offline secara terpisah.
*   Ketika kedua merchant kembali online, salah satu dari mereka akan mendapati bahwa token tersebut sudah hangus (*double-spent*).

Protokol ini menyelesaikan masalah tersebut dengan membiarkan transaksi offline terjadi secara anonim, tetapi secara matematis **memaksa identitas asli pelaku terbuka** jika mereka melakukan *double-spend*.

---

## 2. Formulasi Matematika Pengungkapan Identitas (Self-Revealing Identity)

Skema ini didasarkan pada konsep **Pembagian Rahasia Shamir 2-Titik** (persamaan linear pada medan hingga).

```text
            Pengguna menarik Koin dengan komitmen persamaan garis:
                           y = a * x + I (Mod p)
                                  │
         ┌────────────────────────┴────────────────────────┐
         ▼                                                 ▼
    Belanja Sekali (Jujur)                            Belanja Dua Kali (Curang)
 Merchant A menantang dengan x1:                   Merchant A menantang dengan x1:
         y1 = a * x1 + I (Mod p)                           y1 = a * x1 + I (Mod p)
                                                   Merchant B menantang dengan x2:
                                                           y2 = a * x2 + I (Mod p)
         │                                                 │
         ▼                                                 ▼
Hanya ada 1 titik (x1, y1).                       Ada 2 titik (x1, y1) & (x2, y2).
Identitas I tetap rahasia penuh.                  Sistem persamaan linear diselesaikan:
                                                  Identitas I terekspos & disita!
```

### A. Tahap Penarikan Koin (Withdrawal)
Misalkan $I$ adalah identitas rahasia pengguna (misal: tanda tangan kunci privat akun utama pengguna).
1.  Pengguna memilih nilai acak $a \in \mathbb{Z}_p$ (sebagai kemiringan garis / *slope*).
2.  Pengguna membentuk persamaan garis linear:
    $$y = a \cdot x + I \pmod p$$
3.  Pengguna menarik koin privat secara blinded dari Issuer dengan menyertakan bukti komitmen kriptografis atas nilai $a$ dan $I$.

### B. Tahap Transaksi Offline (Spending)
Ketika membayar ke Merchant secara offline:
1.  Merchant mengirimkan tantangan acak (*random challenge*) $x_1 \in \mathbb{Z}_p$.
2.  Pengguna harus merespons dengan menghitung nilai $y_1$ pada garis tersebut:
    $$y_1 = a \cdot x_1 + I \pmod p$$
3.  Pengguna memberikan tanda bukti $(x_1, y_1)$ beserta koin privat kepada Merchant.
4.  Merchant memverifikasi secara offline bahwa titik $(x_1, y_1)$ valid sesuai komitmen koin, lalu menyerahkan barang.

### C. Analisis Keamanan (Security Analysis)

#### Kasus 1: Pengguna Jujur (Membelanjakan Koin Sekali)
*   Hanya ada satu titik $(x_1, y_1)$ yang diketahui oleh publik/merchant.
*   Berdasarkan teori informasi, satu titik $(x, y)$ tidak cukup untuk menentukan koordinat potong $I$ pada persamaan garis lurus tanpa mengetahui nilai kemiringan $a$.
*   **Hasil:** Identitas $I$ aman dan privasi pengguna terjaga 100%.

#### Kasus 2: Pengguna Curang (Membelanjakan Koin Dua Kali)
*   Pengguna memberikan koin yang sama ke Merchant B secara offline.
*   Merchant B memberikan tantangan acak yang berbeda $x_2 \neq x_1$.
*   Pengguna terpaksa memberikan respons titik kedua $(x_2, y_2)$:
    $$y_2 = a \cdot x_2 + I \pmod p$$
*   Saat kedua merchant kembali online dan menyetorkan bukti ke smart contract Nimbus, smart contract mendapatkan dua titik koordinat: $(x_1, y_1)$ dan $(x_2, y_2)$.
*   Smart contract menyelesaikan sistem persamaan linear tersebut secara instan:
    $$a = \frac{y_2 - y_1}{x_2 - x_1} \pmod p$$
    $$I = y_1 - a \cdot x_1 \pmod p$$
*   **Hasil:** Identitas asli pengguna $I$ terbongkar secara otomatis oleh matematika! Kontrak segera menyita jaminan jaminan pelaku yang dikunci on-chain.

---

## 3. Integrasi ke dalam Nimbus Protocol

Untuk mengimplementasikan fitur ini pada versi Nimbus berikutnya:
1.  **nimbus-core:** Menambahkan modul fungsi matematika untuk pembagian rahasia dan interpolasi linear modulo $p$ (menggunakan medan hingga `Fr` pada BLS12-381).
2.  **nimbus-contracts:** Menambahkan fungsi `slash_double_spender` yang menerima dua tanda bukti transaksi offline $(x_1, y_1)$ dan $(x_2, y_2)$, menghitung nilai $I$, lalu mencairkan dana jaminan pelaku kecurangan kepada merchant yang dirugikan.

Inovasi ini menempatkan **Nimbus** sebagai satu-satunya protokol privasi EVM yang siap digunakan untuk transaksi retail dunia nyata dalam segala kondisi jaringan.
