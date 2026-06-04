# Analisis Keamanan & Model Ancaman (Security Threat Model)

Keamanan adalah prioritas nomor satu dalam Nimbus Protocol karena sistem ini memegang hak asuh (*custody*) atas dana riil stablecoin (USDC) milik pengguna di on-chain. Dokumen ini memetakan model ancaman (*threat model*), batas kepercayaan (*trust boundaries*), dan mekanisme perlindungan kriptografi untuk mengamankan protokol dari peretasan.

---

## 1. Arsitektur Komponen & Batas Kepercayaan (Trust Boundaries)

Sistem Nimbus terbagi menjadi 3 zona keamanan utama:
1.  **Zona Klien (WASM SDK)**: Berjalan di perangkat pengguna / AI Agent. Zona ini memiliki kerahasiaan penuh (*private zone*). Kunci identitas rahasia ($I$) dan faktor blinding ($r$) tidak pernah keluar dari zona ini.
2.  **Zona Perantara (Relayer Node)**: Berjalan di server VPS/Awan. Zona ini mengumpulkan transaksi, menangani paymaster EIP-7702, dan memicu penandatanganan. Ini adalah zona semi-terpercaya (*semi-trusted*).
3.  **Zona Konsensus (Arbitrum L2 Contract)**: Brankas kustodian utama yang 100% *trustless* dan dijalankan secara terdistribusi oleh validator blockchain.

---

## 2. Pemetaan Titik Masuk Data (Entry Points & Untrusted Inputs)

Semua titik di mana data luar masuk ke dalam sistem divalidasi secara ketat untuk mencegah eksploitasi:

| Titik Masuk (Entry Point) | Penerima | Jenis Input | Status Validasi Keamanan |
| :--- | :--- | :--- | :--- |
| **`POST /api/deposit`** | Relayer Node | Untrusted (User) | Memeriksa format Session ID dan keunikan komitmen komposit kunci masking ($com_k$). |
| **`POST /api/spend`** | Relayer Node | Untrusted (User) | Menyaring panjang nullifier dan menyimpannya di antrean mempool sebelum diacak (*shuffled*). |
| **`POST /api/pos/sync-claims`** | Relayer Node | Untrusted (Kasir) | Memeriksa kecocokan data token ID dan membandingkan variasi nilai tantangan $x$ untuk deteksi double-spend. |
| **Fungsi `spend()`** | L2 Contract | Parameter BLS | Kontrak L2 memvalidasi panjang array byte secara kaku: $a_{neg}$ G1 (128 bytes), $H(m)$ G1 (128 bytes), dan $pk_{iss}$ G2 (256 bytes) sebelum diteruskan ke mesin precompile EIP-2537. |
| **Fungsi `ccip_receive()`** | L2 Contract | Payload CCIP | Hanya dapat dipanggil oleh alamat Router CCIP resmi. Payload divalidasi wajib memiliki panjang tepat 648 byte untuk mencegah serangan korupsi memori. |

---

## 3. Analisis Skenario Serangan Utama & Langkah Mitigasi

### Skenario A: Peretasan / Pencurian Kunci Relayer (Key Compromise)
*   **Ancaman**: Hacker meretas server Relayer dan mencuri kunci rahasia Issuer ($sk_{iss}$) untuk mencetak token palsu tak terbatas.
*   **Mitigasi**:
    1.  **Threshold Cryptography**: Kita merancang skema **Threshold BDHKE** di mana kunci $sk_{iss}$ dipecah menggunakan Shamir Secret Sharing ke 5 server independen. Hacker harus meretas minimal 3 server secara bersamaan untuk merekonstruksi kunci.
    2.  **Multisig Escrow**: Kontrak pintar kustodian membatasi volume pencairan harian (*rate-limiting*) untuk meminimalkan dampak jika terjadi kebocoran kunci validator.

### Skenario B: Relayer Nakal Menolak Mengungkap Kunci Masking (Relayer Hostage Attack)
*   **Ancaman**: Relayer menerima deposit USDC dari pengguna, tetapi sengaja mati atau offline sehingga tidak pernah mempublikasikan kunci masking $k$ untuk membuka tanda tangan token. Uang pengguna terancam terjebak selamanya di kontrak.
*   **Mitigasi**:
    *   **Auto-Refund Escrow (Timelock)**: Kontrak pintar memiliki mekanisme kunci waktu (*timelock*). Jika relayer tidak mempublikasikan kunci $k$ di blockchain dalam waktu 24 jam (misalnya), pengguna dapat memicu fungsi `claim_refund()` di smart contract untuk menarik kembali USDC mereka langsung dari brankas secara otomatis tanpa persetujuan relayer.

### Skenario C: Serangan Pembelanjaan Ganda Luring (Offline Double-Spend)
*   **Ancaman**: Pengguna membelanjakan tiket digital yang sama di dua toko berbeda secara offline (tidak terkoneksi internet).
*   **Mitigasi**:
    *   **Slashing & Blame Dispatcher**: Kriptografi linear Shamir ($y = a \cdot x + I \pmod P$) menjamin bahwa jika token yang sama digunakan dengan tantangan berbeda ($x_1 \neq x_2$), rahasia identitas pelaku ($I$) dihitung secara instan dari perbedaan respons ($y_1, y_2$). Kunci $I$ ini dikirim ke smart contract L2 untuk memotong jaminan $100$ USDC pelaku dan diberikan ke merchant sebagai ganti rugi.

### Skenario D: Serangan Masuk Kembali (Reentrancy Attack)
*   **Ancaman**: Saat mengeksekusi pembelian Polymarket di rantai target (`spend_and_buy_shares`), hacker mencoba memanggil kembali kontrak secara berulang (*reentrancy*) sebelum status token diperbarui, agar bisa menarik dana berkali-kali.
*   **Mitigasi**:
    *   **Mekanisme Checks-Effects-Interactions**: Di kontrak L2 kita, penandaan nullifier dilakukan terlebih dahulu di memori penyimpanan sebelum panggilan eksternal (*raw call*) ke kontrak Polymarket dijalankan ([nimbus-contracts/src/lib.rs:L205](file:///home/azureuser/crypto/nimbus-contracts/src/lib.rs#L205)). Ini mencegah serangan reentrancy secara absolut.

---

## 4. Rekomendasi Langkah Pengamanan Tambahan untuk Produksi

1.  **Audit Pihak Ketiga (External Auditing)**: Sebelum meluncurkan aplikasi ke publik dengan dana pengguna nyata, seluruh modul matematika Rust (`nimbus-core`) dan smart contract Stylus wajib diaudit oleh auditor terkemuka (seperti OpenZeppelin, Trail of Bits, atau ConsenSys Diligence).
2.  **Multi-Party Computation (MPC)**: Menggunakan teknologi MPC untuk mengelola kunci parsial validator di dalam lingkungan TEE (Trusted Execution Environment) seperti Intel SGX.
3.  **Circuit Breaker (Emergency Pause)**: Menambahkan fungsi darurat `pause()` yang dikendalikan oleh Multisig Dewan Keamanan (Security Council) untuk membekukan penarikan sementara jika terdeteksi aktivitas anomali pada kontrak.
