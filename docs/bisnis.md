# Model Bisnis & Konsep Monetisasi Nimbus Protocol

Nimbus Protocol dirancang dengan model bisnis yang ramah bagi pengguna serta ekosistem terdesentralisasi, dengan memindahkan beban biaya operasional ke aktivitas bernilai tinggi (**Whales**, **AI Agents**, dan **Institusi**) serta optimalisasi yield on-chain. 

Pada saat yang sama, fitur pembayaran retail luring (offline merchant) tetap dipertahankan sebagai modul opsional bernilai tinggi jika ada perusahaan ritel khusus yang ingin mengakuisisinya.

---

## 1. Filosofi Bisnis: Zero-Cost Retail & Cross-Subsidization

Pada fase awal, tantangan terbesar protokol pembayaran baru adalah dinginnya adopsi jika pengguna dibebani biaya transaksi tinggi. Oleh karena itu, Nimbus menerapkan prinsip **subsidi silang (cross-subsidization)**:
*   **Retail/Pengguna Kasual**: Biaya transaksi sangat murah hingga mendekati **0%**.
*   **Whale & Institusi**: Menanggung biaya penarikan/minting premium kecil (0.1% - 0.2%) demi mendapatkan privasi mutlak dalam menyembunyikan strategi taruhan/aset mereka dari publik atau menjaga kepatuhan regulasi.
*   **AI Agents**: Membayar biaya routing mikro per transaksi API untuk mendapatkan kecepatan transaksi nanopayment tanpa latensi ZK-proof.

---

## 2. Struktur Biaya & Sumber Pendapatan Protokol

### A. Untuk Toko / Merchant (Retail Pembayaran Offline - Cadangan/Opsional)
*   **Merchant Discount Rate (MDR)**: 
    *   **Toko Kecil/Mikro**: **0%** biaya transaksi untuk volume di bawah batas tertentu.
    *   **Toko Menengah/Besar**: **0.1%** per transaksi (jauh lebih murah dari Visa/Mastercard atau QRIS lokal).
*   **Nilai Akuisisi**: Modul ini disimpan sebagai inovasi cadangan. Perusahaan ritel besar atau institusi keuangan yang ingin menerapkan e-cash luring dapat melisensikan atau mengakuisisi IP/teknologi mesin slashing Shamir ini dari Nimbus.

### B. Untuk Pengguna & Whale (Polymarket Private Betting - Fase B)
*   **Minting Fee (Deposit)**: **0.1%** dari nominal stablecoin yang dikunci ke dalam smart contract.
*   **Redemption Fee (Penarikan)**: **0.15%** saat mencairkan token privat kembali ke alamat publik.
*   **Mekanisme Transaksi**: Whale bersedia membayar biaya kecil ini karena nilai privasi yang mereka dapatkan mencegah strategi taruhan mereka di-frontrun atau dicopas oleh bot pelacak on-chain.

### C. Pendapatan Relayer (Gas Markup EIP-7702)
*   **Mekanisme**: Relayer batching (`nimbus-node`) menyatukan transaksi spend pengguna ke dalam satu antrean mempool (menghemat gas fee hingga 40%).
*   **Markup**: Protokol mengambil margin sekitar **5% - 10%** dari penghematan gas fee tersebut saat menagih biaya gas dalam bentuk stablecoin (USDC) ke dompet pengguna. Pengguna tetap merasa membayar gas fee lebih murah dibanding melakukan transaksi mandiri.

### D. DeFi Yield Integration (Dana Mengendap)
*   **Mekanisme**: Setiap token Nimbus yang beredar dijamin 1:1 oleh stablecoin (USDC) yang terkunci di smart contract.
*   **Optimalisasi**: Selama dana tersebut mengendap dan belum dicairkan oleh pemegang token, smart contract menyalurkan dana jaminan ini ke protokol peminjaman DeFi terpercaya (seperti Aave atau Compound) untuk menghasilkan bunga tahunan (APY ~3% s.d. 5%).
*   **Keuntungan**: Bunga yang dihasilkan sepenuhnya menjadi milik kas protokol dan penyedia likuiditas (Issuer), tanpa mengurangi saldo pengguna sepeser pun.

### E. AI Agents Nanopayments (x402 Facilitator - Fase C)
*   **Mekanisme**: AI Agent menggunakan `AgentTokenPool` untuk membayar API secara otomatis melalui rute HTTP `402 Payment Required`.
*   **Monetisasi**: Protokol memotong biaya routing mikro (**0.05%** per transaksi atau flat fee kecil per API call) untuk memfasilitasi transaksi mikro berfrekuensi tinggi ini. Pencipta AI Agent bersedia membayar karena melindungi rahasia komersial data yang mereka beli.

### F. ZK-Compliance Verification (Kepatuhan Institusi - Fase A)
*   **Mekanisme**: Sebelum melakukan deposit, pengguna korporat/institusi melakukan pembuktian ZK-SNARK secara lokal bahwa alamat mereka berada di dalam daftar Merkle Tree bersih (tidak masuk daftar OFAC/sanksi).
*   **Monetisasi**: Protokol menarik biaya verifikasi kepatuhan (*Compliance Verification Fee*) untuk memeriksa ZK-proof dan Merkle proof di kontrak on-chain L2.

---

## 3. Skema Slashing (Pencegahan Kerugian Merchant)

Salah satu ketakutan terbesar toko dalam menerima pembayaran offline adalah risiko double-spending saat mereka tidak terhubung ke internet. Nimbus menyelesaikan ini secara kriptografis menggunakan skema Shamir Secret Sharing:
*   Jika pembeli nakal mencoba membelanjakan token yang sama dua kali, identitas dompet mereka (Identity Secret) otomatis terbongkar secara instan di rantai L2.
*   Smart contract akan menyita (slash) uang jaminan (collateral) pembeli tersebut.
*   **Pembagian Dana Sitaan**: 90% diberikan kepada merchant sebagai ganti rugi + kompensasi ketidaknyamanan, dan 10% diambil oleh protokol sebagai biaya penegakan keamanan jaringan.

---

## 4. Keuntungan bagi Masing-Masing Pihak

| Pihak | Keuntungan Menggunakan Nimbus | Biaya yang Dikenakan |
| :--- | :--- | :--- |
| **Merchant / Toko** | Transaksi instan (< 1 detik), tanpa risiko penipuan/double-spend, tanpa biaya sewa alat EDC. | 0% (toko kecil) atau 0.1% (toko besar). |
| **Whale / Bettor** | Melindungi alpha taruhan Polymarket, anti copy-trading, taruhan gasless satu klik. | 0.1% - 0.2% per aktivitas minting/redemption. |
| **AI Agents** | Pembayaran API otonom tanpa MetaMask, menjaga kerahasiaan kueri data komersial. | 0.05% routing fee per transaksi API. |
| **Institusi** | Transaksi privat yang patuh hukum (OFAC/AML-compliant). | ZK-Compliance Verification Fee. |
| **Protokol (Kita)** | Akumulasi pendapatan dari yield DeFi, markup gas relayer, biaya routing x402, dan biaya kepatuhan. | N/A (Penerima laba). |

---

## 5. Token Tata Kelola $NIMB (2026 Real Yield & Sustainable Tokenomics)

Untuk menyelaraskan insentif jangka panjang antara pendiri, investor, dan komunitas tanpa menimbulkan risiko klasifikasi regulasi (*securities/investasi pasif*) atau ketidakstabilan kas, Nimbus mengadopsi model **Real Yield & Risk-Based Staking**:

```text
Pendapatan Protokol (USDC)
       │
       ├──► 40% ──► Buyback & Burn $NIMB (Tekanan Deflasi Pasar)
       │
       ├──► 40% ──► Safety Module Staking (USDC Yield untuk Penanggung Risiko)
       │
       └──► 20% ──► Treasury Reserve Fund (Biaya Operasional & R&D)
```

### Mekanisme Akumulasi Nilai yang Berkelanjutan:

1. **Buyback & Burn Dinamis (40% Pendapatan)**:
   * Sebesar 40% dari total fee USDC yang dikumpulkan protokol digunakan untuk membeli kembali (*buyback*) token $NIMB dari pasar terbuka secara otomatis, lalu memusnahkannya (*burn*). 
   * **Manfaat**: Menciptakan tekanan beli dan mengurangi total suplai token secara organik, meningkatkan nilai jangka panjang bagi seluruh pemegang token tanpa memicu klasifikasi sekuritas karena dividen pasif.

2. **Safety Module Staking Yield (40% Pendapatan)**:
   * Dividen USDC **tidak dibagikan secara pasif** kepada semua pemegang koin. Pengguna harus mengunci token $NIMB mereka ke dalam **Safety Module (Modul Pengaman)**.
   * **Peran Penanggung Risiko**: Staker di Safety Module bertindak sebagai penyedia jaminan asuransi (*backstop*) jika protokol mengalami kegagalan teknis, eksploitasi smart contract, atau defisit akibat kegagalan pemotongan slashing luring.
   * **Imbal Dagang**: Sebagai kompensasi atas risiko penjaminan tersebut, staker menerima yield USDC secara rutin. Hal ini mengubah dividen menjadi **biaya jasa penjaminan aktif** yang sehat secara ekonomi dan aman secara hukum.

3. **Treasury Reserve Fund (20% Pendapatan)**:
   * Sebesar 20% pendapatan disimpan secara ketat di kas protokol (*Treasury*) untuk membiayai operasional, audit kode berkala, riset kriptografi lanjutan, serta pemasaran.

---

## 6. Rencana Aksi Go-To-Market Awal

1.  **Fase 1: Tarik Volume dari Whale Polymarket (Fase B)**
    *   Fokus memasarkan fitur taruhan privat Polymarket. Ini akan menghasilkan volume transaksi besar (TVL) awal di smart contract kita, yang menghasilkan yield DeFi instan untuk mendanai operasional awal protokol.
2.  **Fase 2: Integrasi SDK AI Agent untuk API Provider (Fase C)**
    *   Menawarkan Nimbus x402 SDK kepada penyedia API kecerdasan buatan agar AI Agent dapat membayar kueri secara privat dan otonom.
3.  **Fase 3: Layanan Kepatuhan Institusional (Fase A)**
    *   Membuka pintu gerbang bagi dana korporat dengan menyediakan fitur ZK-Compliance agar mereka bisa bertransaksi secara privat namun tetap lolos audit hukum.
4.  **Fase D (Opsi Lisensi): Ritel Raksasa / Merchant Khusus**
    *   Menawarkan modul pembayaran luring ter-slashing kepada konsorsium ritel besar yang membutuhkan sistem kasir offline mandiri.
