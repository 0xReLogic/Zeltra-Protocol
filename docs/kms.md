# Panduan Key Management System (KMS) & E2E Ceremony Debugging

Dokumen ini menjelaskan analisis akar masalah (root cause) mengapa proses simulasi distributed threshold signing ceremony sempat mengalami hang, solusi perbaikan yang telah diterapkan, serta panduan langkah demi langkah cara men-deploy node Leader dan Guardian di VPS menggunakan jaringan aman Tailscale dalam lingkungan production.

---

## 1. Analisis Masalah: Mengapa Ceremony Sempat Hang?

Selama distributed signing ceremony berjalan, node coordinator (Leader) akan menghubungi seluruh node Guardian via HTTP untuk mengumpulkan partial signature. Kami menemukan dua akar masalah utama yang menyebabkan proses ini hang atau timeout:

### Masalah A: HTTP Client Terblokir Selamanya (`nimbus-node/src/http.rs`)
*   **Analisis:** Custom HTTP client sebelumnya menggunakan function `tokio::io::AsyncReadExt::read_to_string` untuk membaca HTTP response dari TCP stream.
*   **Penyebab:** Function `read_to_string` akan terus membaca data dan memblokir thread eksekusi sampai terjadi **EOF** (End of File) pada TCP socket. Karena server Axum menggunakan protokol HTTP/1.1 dengan mode Keep-Alive secara default, socket TCP tidak langsung ditutup oleh server meskipun request menyertakan header `Connection: close`. Hal ini menyebabkan client ter-block selamanya sampai batas idle keep-alive timeout server habis (biasanya 5+ detik), yang akhirnya memicu kegagalan/timeout pada script testing.
*   **Solusi Perbaikan:** 
    1.  Kami mengubah `read_http_response` untuk mem-parse HTTP header terlebih dahulu guna mendapatkan nilai `Content-Length`.
    2.  Client sekarang membaca byte data dari stream **tepat sejumlah panjang body response** yang didefinisikan dalam `Content-Length`, sehingga tidak perlu menunggu EOF.
    3.  Menambahkan timeout ketat 5 detik menggunakan `tokio::time::timeout` agar jika salah satu VPS guardian offline/bermasalah, system tidak akan hang selamanya dan dapat pulih secara otomatis.

### Masalah B: Cryptographic Flow Tidak Sesuai Spesifikasi di Mock Script (`simulate_cluster.py`)
*   **Analisis:** Script simulasi lama langsung mencoba mengambil masking key (`k_val` or `k_hex`) dari response `/api/leader/sign`.
*   **Penyebab:** Pada protokol real, **Leader dilarang keras untuk membocorkan masking key `k` di dalam endpoint signing**. Jika `k` diberikan langsung di awal, client nakal bisa langsung melakukan unmasking signature tanpa pernah melakukan transaksi escrow deposit di blockchain (potensi kerugian dana/double spend).
*   **Solusi Perbaikan:** 
    Kami menyesuaikan alur di `simulate_cluster.py` agar sesuai dengan cryptographic flow production yang aman:
    1.  **Leader Sign:** Client menembak `/api/leader/sign` untuk mendapatkan commitment `com_k_hex` dan threshold partial signatures.
    2.  **Confirm Deposit:** Client melakukan deposit on-chain (disimulasikan dengan memanggil `/api/deposit`).
    3.  **Reveal:** Setelah deposit terkonfirmasi di database, client memanggil `/api/reveal` untuk mendapatkan masking key `k` secara aman.
    4.  **Unmask & Verify:** Client melakukan unmasking signature secara lokal menggunakan `k` dan melakukan verifikasi akhir terhadap public key issuer.

---

## 2. Hasil Pengujian Simulasi E2E

Simulasi pengujian ceremony dengan 5 node ($t=3, n=5$) pada port terpisah (`8080`-`8084`) dengan database SQLite terisolasi berjalan sukses tanpa hambatan:
*   **Health Check Node:** 100% Sehat (HEALTHY/OK).
*   **Ceremony Quorum:** Sukses mengumpulkan partial signatures dari quorum minimal ($t=3$).
*   **E2E Deposit & Reveal:** Berjalan lancar dan instan.
*   **Verifikasi Akhir:** Signature BLS12-381 berhasil di-unblind dan divalidasi sukses terhadap Public Key Issuer!

---

## 3. Panduan Deployment VPS Production

Gunakan panduan berikut saat menyebarkan node Leader dan Guardians ke server VPS terpisah agar aman dan terhindar dari kendala jaringan:

### 3.1 Jaringan & Keamanan (Wajib Menggunakan Tailscale VPN)
*   **Jangan buka port HTTP node (8080-8084) ke publik/internet** (`0.0.0.0`) karena bisa diserang secara langsung.
*   **Gunakan Tailscale** (mesh VPN privat):
    1.  Install Tailscale pada semua VM/VPS.
    2.  Setting environment `NIMBUS_BIND_ADDR` agar hanya me-listen ke IP Tailscale VPS tersebut (biasanya berawalan `100.x.x.x` atau `fd7a:...`).
    3.  Set environment `NIMBUS_REQUIRE_TAILSCALE=true` agar node menolak startup jika tidak terikat ke IP privat VPN.

### 3.2 Contoh Konfigurasi Environment VPS

#### 1. Node Leader (VPS Utama - IP Tailscale: `100.64.0.1`)
```bash
# Konfigurasi Node & Database
NIMBUS_ENV=production
PORT=8080
NIMBUS_BIND_ADDR=100.64.0.1
NIMBUS_REQUIRE_TAILSCALE=true
NIMBUS_DB_PATH=/var/lib/nimbus/leader.db

# Kunci Kriptografi
NIMBUS_SHARE_INDEX=1
NIMBUS_SHARE_KEY=<secret_share_1_hex>
NIMBUS_THRESHOLD=3
NIMBUS_ISSUER_PUBLIC_KEY=<issuer_public_key_hex>

# Guardian Registry (Menyimpan URL Tailscale Guardian lainnya)
NIMBUS_GUARDIAN_PUBLIC_KEYS='{
  "2": "public_share_2_hex",
  "3": "public_share_3_hex",
  "4": "public_share_4_hex",
  "5": "public_share_5_hex"
}'

# Integrasi EVM (Diperlukan untuk Production/Mainnet)
NIMBUS_RPC_URL="https://your-arbitrum-rpc-endpoint"
NIMBUS_RELAYER_PRIVATE_KEY="<relayer_private_key>"
NIMBUS_CONTRACT_ADDRESS="0xe6430973795bb1cc3083787e554ef2a559dad7f1"
```

#### 2. Node Guardian 2 (VPS Guardian 1 - IP Tailscale: `100.64.0.2`)
```bash
NIMBUS_ENV=production
PORT=8080
NIMBUS_BIND_ADDR=100.64.0.2
NIMBUS_REQUIRE_TAILSCALE=true
NIMBUS_DB_PATH=/var/lib/nimbus/guardian_2.db

NIMBUS_SHARE_INDEX=2
NIMBUS_SHARE_KEY=<secret_share_2_hex>
NIMBUS_THRESHOLD=3
NIMBUS_ISSUER_PUBLIC_KEY=<issuer_public_key_hex>

# List Leader yang dipercaya (Address EIP-712 signer Leader)
NIMBUS_TRUSTED_LEADERS="0xLeaderSignerAddress"

# Guardian Registry (Daftar public share guardian/leader lainnya)
NIMBUS_GUARDIAN_PUBLIC_KEYS='{
  "1": "public_share_1_hex",
  "3": "public_share_3_hex",
  "4": "public_share_4_hex",
  "5": "public_share_5_hex"
}'
```

### 3.3 Administrasi Database & KMS
*   Setiap VPS **wajib** menggunakan file database SQLite terpisah lokal pada disk VPS tersebut. Jangan pernah membagi file database menggunakan network storage (seperti NFS), karena akan memicu lock collision dan merusak data SQLite.
*   Database dienkripsi secara default oleh SQLCipher. Pastikan mengonfigurasi `NIMBUS_DB_KEY` di level server OS secara aman.
