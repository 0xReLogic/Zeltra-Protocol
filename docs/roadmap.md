# Roadmap Pengembangan Teknis & Inovasi: Nimbus Protocol (Fase A - D)

Dokumen ini memetakan rencana inovasi strategis Nimbus Protocol untuk bertransisi dari fungsionalitas dasar (MVP) menjadi protokol privasi tingkat institusional yang patuh regulasi, beroperasi lintas rantai, ramah kecerdasan buatan (AI), dan siap digunakan secara retail.

---

## Fase A: ZK-Compliance & Privacy-Preserving Compliance (Kepatuhan Regulasi yang Menjaga Privasi) -- [DIIMPLEMENTASIKAN]

### 1. Landasan Kajian & Teori (Academic Reference)
Kami merujuk pada konsep **"Privacy Pools"** yang dipublikasikan oleh Vitalik Buterin, Ameen Soleimani, Jacob Illum, Matthias Nadler, dan Fabian Schär (2023/2024), serta perkembangan **ZK-Proof of Innocence (PoI)** pada tahun 2025/2026. 
*   **Inti Teori**: Alih-alih menyembunyikan transaksi secara mutlak tanpa filter (seperti Tornado Cash yang akhirnya disanksi OFAC), pengguna membuktikan secara kriptografis bahwa dana mereka berasal dari kumpulan alamat yang bersih (*association sets*), tanpa membongkar alamat asli mereka.

### 2. Spesifikasi Implementasi pada Nimbus
Kami akan mengintegrasikan sirkuit ZK-SNARK (menggunakan Groth16 atau Plonky3) ke dalam fungsi deposit kontrak Nimbus:
*   **Merkle Tree of Clean Addresses**: Protokol memelihara Merkle Tree berisi alamat-alamat yang tidak masuk daftar sanksi (misal dari basis data sanksi Chainanalysis/OFAC).
*   **ZK-Proof Generation**: Sebelum pengguna diizinkan melakukan deposit, SDK (`nimbus-sdk`) menghasilkan ZK-proof secara lokal di browser yang membuktikan bahwa alamat pendeposit adalah anggota dari Merkle Tree bersih tersebut.
*   **Verification on Stylus**: Kontrak di Arbitrum Stylus memverifikasi ZK-proof tersebut dalam fungsi `deposit()`. Jika valid, deposit diterima. Ini menjamin bahwa dana yang masuk ke Nimbus 100% legal tanpa mengorbankan kerahasiaan identitas pengguna.
*   **Auditable View Key**: Implementasi kunci pemantau (*View Key*) berbasis kriptografi kunci publik asimetris, yang memungkinkan pengguna mendekripsi sejarah transaksi mereka sendiri untuk kebutuhan laporan pajak/auditor secara mandiri.

---

## Fase B: Eksekusi Lintas Rantai Privat (Cross-Chain Private Execution) -- [DIIMPLEMENTASIKAN]

### 1. Landasan Kajian & Teori (Technology Reference)
Mengeksploitasi teknologi **Chainlink CCIP (Cross-Chain Interoperability Protocol)**, **Chainlink ACE (Associated Contract Execution)**, dan mekanisme jembatan pesan aman lintas L2.
*   **Inti Teori**: Eksekusi kontrak di rantai A (misalnya Arbitrum Stylus untuk komputasi precompile BLS12-381 yang murah) dapat memicu panggilan fungsi kontrak di rantai B (misalnya Polygon PoS tempat Polymarket beroperasi) secara atomik dan terenkripsi tanpa kebocoran data alamat perantara.

### 2. Spesifikasi Implementasi pada Nimbus
*   **Cross-Chain Relayer Bridge**: Saat pengguna melakukan spend token privat di Arbitrum, relayer mengirimkan transaksi CCIP yang membawa pesan terenkripsi ke Polygon.
*   **Ephemeral Target Wallet Execution**: Di Polygon, kontrak penerima CCIP Nimbus menerima instruksi, menyetor USDC ke Polymarket, dan membeli opsi taruhan atas nama dompet sekali pakai (*ephemeral wallet*) yang diotorisasi secara aman melalui pesan tanda tangan BLS lintas rantai.
*   **Atomic Multicall**: Proses penarikan (Redemption) dari L2 asal hingga pembelian shares di target chain terjadi dalam satu bundle transaksi terpadu, menghilangkan risiko kegagalan transaksi di tengah jalan (*partial execution risk*).

---

## Fase C: SDK Pembayaran AI Agents (x402 Protocol Integration) -- [DIIMPLEMENTASIKAN]

### 1. Landasan Kajian & Teori (Industry Reference)
Mengadopsi **Protokol x402 v2** (standar pembayaran mesin-ke-mesin/M2M yang diluncurkan oleh Coinbase dan Cloudflare berbasis HTTP 402 Payment Required) serta infrastruktur pembayaran mikro otonom (Nanopayment Stack).
*   **Inti Teori**: Agen AI otonom memerlukan cara untuk membayar layanan API, komputasi awan, atau data pasar secara mandiri menggunakan stablecoin dengan latensi sangat rendah dan privasi tinggi untuk melindungi proprietary trading model milik penciptanya.
*   **Referensi Industri 2026**: Coinbase CDP facilitator (`api.cdp.coinbase.com/platform/v2/x402`), Cloudflare Agents SDK + MCP, Stripe x402 middleware, Exa API x402 endpoint.

### 2. Spesifikasi Implementasi pada Nimbus
*   **x402 Native Header Handling** [SELESAI]: Modul `nimbus-sdk/src/x402.rs` mengimplementasikan parser/encoder untuk header `PAYMENT-REQUIRED`, `PAYMENT-SIGNATURE`, dan `PAYMENT-RESPONSE` sesuai spesifikasi x402 v2 (Base64-encoded JSON, skema `exact`, jaringan CAIP-2).
*   **Nimbus Anonymous Payment Payload** [SELESAI]: Struktur `NimbusPaymentPayload` menggantikan otorisasi EIP-3009 standar dengan bukti spend anonim BLS (`nullifier`, `alpha_neg_hex`, `hm_hex`, `pk_iss_hex`), menjaga privasi identitas AI Agent di on-chain.
*   **x402 Facilitator Endpoint** [SELESAI]: Endpoint `POST /api/x402/verify` di `nimbus-node` menerima, mendekode, memvalidasi nullifier, dan menjadwalkan settlement pembayaran anonim AI Agent.
*   **Non-Interactive Blind Signing** [SELESAI]: Mengembangkan manager pool token (`AgentTokenPool` di `nimbus-sdk/src/x402.rs`) yang mengotomatisasi siklus hidup blinding, registrasi tanda tangan, unmasking key, dan spend token secara luring & non-interaktif bagi AI Agent.
*   **Agent Anonymization Pool** [SELESAI]: Mengimplementasikan background batching shuffler (`nimbus-node/src/main.rs`) yang mengacak antrean transaksi sebelum disubmit ke blockchain untuk memutus korelasi metadata waktu/indeks dari agen.

---

## Fase D: Portal Merchant POS & Blame Slashing Offline (PWA) -- [DIIMPLEMENTASIKAN]

### 1. Landasan Kajian & Teori (UX & Cryptography Reference)
Penerapan Shamir Secret Sharing ($y = a \cdot x + I \pmod p$) untuk sistem kasir retail offline tanpa konektivitas internet konstan (Desentralisasi E-Cash Offline).
*   **Inti Teori**: Keberhasilan sistem pembayaran offline retail bergantung pada kemudahan penggunaan (UX) tingkat tinggi untuk merchant, di mana kasir fisik cukup menggunakan smartphone murah tanpa hardware tambahan.
*   **Referensi Industri 2025/2026**:
    *   Bank of England -- "Digital Pound Experiment Report: Offline Payments" (2025): Memakai secure element hardware + offline transaction limits (jumlah, waktu, nilai) + local transaction record + sync saat reconnect.
    *   EDPB -- "Digital Euro Token-Based Offline Modality" (2025): Anomaly detection privasi-preserving; identitas hanya terungkap saat double-spend terdeteksi.
    *   MDPI Sensors 2026 -- "Continuous Dual-Offline Payment of Cryptocurrency Based on Asset Credentials": Hash deduplication di payment center, zero-sum verification, credential decomposition.
    *   Web NFC API (April 2026): Hanya ~6% global browser support (Chromium Android). QR Code harus menjadi metode transfer data utama; NFC bersifat opsional.
    *   PWA 2026: Service Worker Background Sync API + Periodic Background Sync + IndexedDB tersedia di semua browser modern (Chrome, Edge, Firefox, Safari).

### 2. Spesifikasi Implementasi pada Nimbus
*   **Offline Claim Sync Endpoint** [SELESAI]: Endpoint `POST /api/pos/sync-claims` di `nimbus-node` menerima batch bukti transaksi offline dari PWA merchant. Data disimpan di `offline_claims` HashMap yang diindeks oleh `token_id`.
*   **Blame Dispatcher & Identity Reconstruction** [SELESAI]: Saat klaim masuk, handler secara otomatis mencocokkan `token_id` yang sudah ada. Jika ditemukan dua klaim dengan `challenge_x_hex` berbeda, sistem merekonstruksi identitas pelaku double-spend menggunakan `nimbus_core::reconstruct_identity` (interpolasi Shamir Fr).
*   **Programmatic API for POS Claim Sync** [SELESAI]: Membangun API merchant sync endpoint (`/api/pos/sync-claims`) pada relayer node untuk menerima, memvalidasi, dan mengantrekan data transaksi luring merchant secara efisien.
*   **Programmatic Testing & Verification Harness** [SELESAI]: Membuat skrip Python test harness untuk memecahkan persamaan garis linear $y = a \cdot x + I \pmod P$, menyimulasikan transaksi luring ganda, dan memverifikasi API `/api/pos/sync-claims`.
