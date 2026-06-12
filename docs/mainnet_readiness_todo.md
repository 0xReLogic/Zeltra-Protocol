# Roadmap & TODO List: Kesiapan Smart Contract & Relayer untuk Mainnet Launch

Dokumen ini memetakan seluruh tugas kritis (TODO) yang wajib diselesaikan dan dimatangkan sebelum melakukan deployment Smart Contract ke Mainnet. Tujuannya adalah untuk memastikan kontrak bersifat permanen, aman, efisien (gas-optimized), dan tidak membutuhkan modifikasi ulang (upgrade) di kemudian hari.

---

## 1. Fokus Utama: Kematangan Smart Contract (Stylus Contract)

Modifikasi smart contract pasca-deploy di mainnet sangat berisiko dan mahal. Berikut adalah daftar TODO kontrak yang harus diselesaikan terlebih dahulu:

### 1.1 Optimasi Gas & Batch Spending (`batchSpend`)
*   [x] **Implementasi Fitur `batchSpend()`:** Saat ini transaksi spend dikirimkan satu per satu. Untuk menghemat gas fee secara signifikan (meningkatkan margin profit relayer), contract harus mendukung pemrosesan banyak spend sekaligus dalam satu batch.
*   [x] **Sinkronisasi ABI & Selector:** Lakukan kompilasi ulang contract dengan fitur batching, generate ABI terbaru, dan pastikan method selector `batchSpend` cocok 100% dengan pemicu di relayer.
*   [ ] **Benchmark Gas Realk:** Uji perbandingan konsumsi gas antara transaksi single vs batch (ukuran 2, 4, 8, dan 9) pada Arbitrum Sepolia sebelum rilis.

### 1.2 Validasi Pengiriman Lintas Rantai (CCIP Security)
*   [x] **Enforce `ccip_router != Address::ZERO`**: Kontrak harus menolak semua pesan lintas rantai jika alamat router CCIP belum dikonfigurasi oleh admin.
*   [x] **Allowlist Validasi**: 
    *   [x] Validasi `source_chain_selector` untuk memastikan pesan hanya datang dari chain yang disetujui.
    *   [x] Cocokkan alamat pengirim CCIP dengan daftar kontrak relayer yang sah di source chain (mencegah spoofing deposit/spend).
*   [x] **Replay Protection via Message ID**: Simpan dan verifikasi CCIP `messageId` sebagai pengaman agar pesan yang sama tidak bisa dieksekusi dua kali.

### 1.3 Integrasi ZK Compliance (Fase A)
*   [ ] **Penyelesaian Verifying Key (VK) Sirkuit:** Skema proving dan sirkuit compliance (Merkle membership proof untuk clean association set) harus sudah bersifat final.
*   [ ] **Hardcode VK pada Kontrak:** Masukkan/embed Verifying Key permanen ke dalam smart contract. *VK ini tidak boleh diubah tanpa upgrade sirkuit.*
*   [ ] **Validasi Input Keras:** Kontrak harus memvalidasi format data input secara ketat sebelum melakukan pairing check:
    *   [ ] Nullifier wajib tepat 32 byte (tidak boleh auto-padded).
    *   [ ] Point $G_1$ (`alpha_neg`, `hm`) wajib tepat 128 byte.
    *   [ ] Public key (`pk_iss`) wajib tepat 256 byte.

### 1.4 Kebijakan Pause & Timelock (Governance)
*   [ ] **Emergency Pause Policy:** Tentukan apakah fungsi `claim_refund` (penarikan dana escrow setelah timeout 24 jam) tetap boleh diakses oleh pengguna saat kontrak sedang di-pause oleh admin. *Rekomendasi: Tetap bolehkan refund untuk menjaga trust pengguna.*
*   [ ] **Two-Step Governance Timelock:** Pastikan perubahan parameter krusial seperti pergantian `owner` atau `fee_recipient` dilindungi oleh timelock minimal 24 jam (`fee_recipient_eta`) untuk mencegah eksploitasi admin key secara instan.

---

## 2. Kesiapan Relayer Node & Jaringan Produksi

Selain smart contract, fungsionalitas relayer harus ditingkatkan ke standar keamanan produksi:

### 2.1 Peningkatan Keamanan Komunikasi VPS
*   [ ] **Enkripsi HTTPS & TLS:** Ganti alur komunikasi plain TCP HTTP client di `http.rs` dengan client HTTPS yang memvalidasi sertifikat TLS. *Sangat penting untuk mencegah man-in-the-middle attack saat ceremony.*
*   [ ] **Kerentanan IP Spoofing:** Jangan percaya header `X-Forwarded-For` pada rate limiter kecuali request divalidasi datang dari reverse proxy terpercaya. Gunakan extractor peer socket address langsung untuk koneksi VPS.

### 2.2 Manajemen Kunci Database & Secrets
*   [ ] **Ambil DB Key dari Vault KMS:** Saat ini `NIMBUS_DB_KEY` (untuk enkripsi SQLite SQLCipher) masih dibaca dari environment. Pada mainnet, key ini harus diambil langsung dari OpenBao/Vault saat startup dan segera di-zeroize dari memory setelah database terbuka.
*   [ ] **Backup & Rekonsiliasi State:** 
    *   [ ] Tambahkan mekanisme auto-backup database relayer dan deteksi kerusakan file (db corruption check).
    *   [ ] Buat worker rekonsiliasi yang mencocokkan status nullifier di database dengan on-chain state setelah relayer restart.

### 2.3 Resiliensi Pengiriman Transaksi (Settlement Worker)
*   [ ] **Deadline-Near Bypass:** Jika suatu transaksi dalam antrean (`spend_queue`) mendekati waktu kedaluwarsa (expiry), relayer harus me-bypass antrean normal dan langsung membroadcast transaksi tersebut secara instan.
*   [ ] **Pencegahan Double Broadcast:** Pastikan dua thread worker tidak dapat memproses antrean transaksi yang sama secara bersamaan (gunakan row locking pada database SQLite).
*   [ ] **Penanganan Nonce & Gas Spikes:** Terapkan strategi adaptif untuk otomatis mengirim ulang transaksi (replacement transaction / speed-up) dengan gas price lebih tinggi jika transaksi stuck di mempool Arbitrum.
