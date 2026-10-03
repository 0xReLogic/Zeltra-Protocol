# Nimbus Relayer — 02: Database & SQLCipher Encryption

Dokumen ini menjelaskan konfigurasi basis data terenkripsi, skema tabel, dan optimasi performa pada [`nimbus-node/src/database.rs`](file:///workspaces/Zeltra-Protocol/nimbus-node/src/database.rs).

---

## 1. Enkripsi Basis Data: SQLCipher AES-256

Untuk melindungi data antrian transaksi, nullifier, dan sesi sebelum on-chain dari pencurian fisik pada storage VPS:

* **Engine:** Rusqlite dengan fitur `bundled-sqlcipher`.
* **Algoritma:** Enkripsi tingkat disk penuh menggunakan **AES-256-CBC**.
* **Kunci Database (`NIMBUS_DB_KEY`):** Kunci dibaca saat startup melalui environment variable atau OpenBao KMS. Dalam mode *strict production*, node akan menolak menyala (*fail-closed*) jika kunci masih menggunakan default `"default-change-in-production"`.
* **Izin Berkas Ketat:** Node secara otomatis menetapkan perizinan `chmod 600` (hanya pemilik proses yang dapat membaca/menulis) pada berkas `.db`, berkas WAL (`-wal`), dan berkas SHM (`-shm`).
* **Secure Delete:** Mengaktifkan `PRAGMA secure_delete = ON` agar data yang dihapus langsung ditimpa (*zero-overwrite*), bukan sekadar dilepas dari index.

---

## 2. Tuning PRAGMA untuk Skalabilitas Tinggi

Untuk menangani beban transaksi paralel tanpa mengorbankan durabilitas:

```sql
-- Mengizinkan proses pembacaan (reads) berjalan konkuren tanpa memblokir penulisan (writes)
PRAGMA journal_mode = WAL;

-- Menyeimbangkan kecepatan disk I/O dan keamanan crash
PRAGMA synchronous = NORMAL;

-- Mengalokasikan 64 MB memori RAM sebagai cache query
PRAGMA cache_size = -64000;

-- Menegakkan integritas relasi antar tabel
PRAGMA foreign_keys = ON;

-- Menyimpan temporary table dan indeks sorting di RAM
PRAGMA temp_store = MEMORY;

-- Menunggu hingga 5 detik jika ada transaksi lain yang sedang menulis (Mencegah error 'database is locked')
PRAGMA busy_timeout = 5000;
```

---

## 3. Skema Tabel Utama

### A. Tabel `spend_queue` (Antrian Penyelesaian Transaksi)
Menyimpan antrian permintaan spend sebelum dan saat dibroadcast ke blockchain:
* `id`: Primary key urutan antrian.
* `request_json`: Serialisasi payload transaksi lengkap.
* `status`: Status transisi (`queued`, `broadcasting`, `submitted`, `confirmed`, `failed`, `retryable`).
* `retry_count`: Jumlah percobaan broadcast ulang.
* `last_error`: Pesan kesalahan terakhir jika terjadi kegagalan RPC/revert.
* `lease_expires_at`: Timestamp kedaluwarsa hak klaim worker (mencegah double-broadcast).
* `batch_id`: ID kelompok batch jika digabungkan bersama transaksi lain.
* `tx_hash`: Hash transaksi on-chain setelah berhasil dibroadcast.

### B. Tabel `nullifiers` (Pencegah Double-Spend Lokal)
Merekam seluruh nullifier yang sudah mendapatkan konfirmasi receipt sukses di on-chain:
* `nullifier`: Hash 32-byte unik (PRIMARY KEY).
* `spent_at`: Timestamp transaksi confirmed.
* `tx_hash` & `block_number`: Bukti transaksi blockchain.

### C. Tabel `sessions` (Sesi Deposit & Reveal Masking Key)
* `session_id`: ID sesi transaksi (PRIMARY KEY).
* `client_address`: Alamat depositor USDC.
* `amount`: Nominal bersih deposit.
* `com_k`: Komitmen masking key yang diserahkan.
* `k_encrypted`: Kunci masking $k$ yang disimpan aman sebelum dirilis.
* `deposit_confirmed`: Flag apakah transaksi deposit sudah verified di on-chain.
* `resolved`: Flag apakah masking key sudah dirilis ke client.

### D. Tabel `spend_batches` (Pencatatan Batching & Komisi)
* `batch_id`: UUID batch unik.
* `item_count`: Jumlah transaksi yang digabungkan (2 s/d 8).
* `execution_fee`: Total komisi gas yang berhak diklaim oleh relayer.
* `claimed`: Status apakah komisi ini sudah ditarik via `claimExecutionFee()` di smart contract.

### E. Tabel `quote_ids_used` (Pencegah Replay Quote EIP-712)
* `quote_id`: ID unik signed quote (PRIMARY KEY).
* `user_address`: Alamat pengirim/signer quote.
* `used_at`: Timestamp saat kuotasi dikonsumsi oleh transaksi spend.

### F. Tabel `relayer_transactions` (Pelacakan Nonce & Finalitas Receipt - DEC-017)
Merekam seluruh transaksi on-chain yang dibroadcast oleh relayer untuk pelacakan nonce atomik, eskalasi gas, dan verifikasi receipt:
* `tx_hash`: Hash transaksi on-chain (PRIMARY KEY).
* `nonce`: Nomor urut transaksi EVM relayer yang di-lock secara atomik.
* `signer_address`: Alamat akun relayer penandatangan transaksi.
* `status`: Status eksekusi transaksi (`pending`, `confirmed`, `failed`).
* `block_number`: Tinggi blok tempat transaksi dieksekusi.
* `gas_used` & `effective_gas_price`: Biaya gas riil untuk pembukuan dan margin tracking.
* `confirmations`: Jumlah blok konfirmasi yang telah dilewati.
* `error_reason`: Pesan revert atau kegagalan jika transaksi gagal di on-chain.
* `created_at` & `updated_at`: Timestamp untuk keperluan timeout watchdog & gas bump replacement.
