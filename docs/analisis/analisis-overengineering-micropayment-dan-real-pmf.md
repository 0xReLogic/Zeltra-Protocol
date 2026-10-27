# Analisis Arsitektur: Over-Engineering Micropayment AI Agent, UTXO Dust Trap & Real Product-Market Fit Zeltra

**Status:** APPROVED ARCHITECTURAL REVIEW  
**Tanggal:** 27 Oktober 2026  
**Penulis:** Core Protocol Engineering & Founding Team  
**Kategori:** Evaluasi Arsitektur, Kriptoekonomi, dan Analisis Pasar  
**Terkait:** [`DEC-038`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-038-pure-zk-utxo-migration-and-direct-mmr-deposit.md), [`DEC-042`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-042-privacy-preserving-x402-facilitator-and-pure-zk-utxo-micropayments.md), [`DEC-043`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-043-strict-3-mode-joinsplit-state-machine-and-fee-policy.md), [`VISION.md`](file:///workspaces/Zeltra-Protocol/VISION.md)

---

## 1. Latar Belakang & Introspeksi Teknis

Dokumen ini disusun sebagai catatan refleksi arsitektur resmi protokol Zeltra untuk mengidentifikasi dan membedah gejala **over-engineering** pada implementasi pembayaran mikro agen otonom (*autonomous AI Agent micropayments*), menepis asumsi keliru mengenai *on-chain batching*, mengurai risiko sistemik **"UTXO Dust Trap"**, serta menegaskan kembali **Product-Market Fit (PMF)** sejati protokol Zeltra di dunia nyata.

### A. Gejala Over-Engineering Awal
Pada perancangan awal integrasi protokol HTTP 402 ([`DEC-042`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-042-privacy-preserving-x402-facilitator-and-pure-zk-utxo-micropayments.md)), muncul asumsi idealis bahwa Zeltra Protocol dapat melayani transaksi sub-sen bernilai mikro (**$0.0001 s/d $0.005 USDC**) langsung ke on-chain untuk setiap panggilan API (*per-token inference / per-request scraping*) melalui mekanisme "batching" di tingkat relayer node.

Setelah dikaji secara matematis dan ekonomis, asumsi ini mengandung **cacat logika struktural** pada sistem komputasi zero-knowledge (*Groth16*) dan arsitektur *ZK-UTXO*.

---

## 2. Bedah Matematika: Biaya Gas vs Nilai Transaksi (Negative Unit Economics)

### A. Biaya Kriptografis Tetap Verifikasi Groth16
Di Arbitrum Stylus (Rust WASM), verifikasi bukti Groth16 menggunakan precompile kriptografi EIP-2537:
- Precompile `0x0c` (Multi-Scalar Multiplication / MSM G1)
- Precompile `0x0f` (BLS12-381 4-Pairing Check: $e(-A, B) \cdot e(\alpha, \beta) \cdot e(L, \gamma) \cdot e(C, \delta) == 1$)

Total biaya gas on-chain per verifikasi bukti berada di kisaran:
$$\text{Gas Cost} \approx 250.000 \text{ s/d } 400.000 \text{ gas}$$
Pada harga gas rata-rata Arbitrum L2 (0.1 gwei), biaya eksekusi transaksi on-chain berkisar antara:
$$\text{Biaya Gas Riil} \approx \$0.005 \text{ s/d } \$0.010 \text{ USDC}$$

### B. Jebakan Transaksi Mikro ($0.001)
Jika sebuah AI Agent membayar $0.001 untuk 1 panggilan inferensi:
- Nilai transaksi riil: **$0.001 USDC**
- Biaya gas on-chain: **$0.008 USDC**
- **Beban Biaya Gas:** **800%** dari nilai barang/jasa!

Transaksi dengan *unit economics* negatif seperti ini tidak akan pernah diadopsi oleh merchant maupun pengguna rasional.

---

## 3. The UTXO Dust Consolidation Trap (Bencana 1.000 Note Receh)

Dalam sistem berbasis *Unspent Transaction Output (ZK-UTXO)* dengan sirkuit Universal JoinSplit 2-in-2-out ([`DEC-043`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-043-strict-3-mode-joinsplit-state-machine-and-fee-policy.md)):
- Satu bukti JoinSplit hanya dapat menggabungkan **tepat 2 input notes** menjadi 1 atau 2 output notes.

### Simulasi Kasus Toko:
Misalkan sebuah toko menerima 1.000 transaksi mikro AI Agent sebesar $0.002 per panggilan:
1. Toko kini mengantongi **1.000 keping Private Notes**, dengan total saldo nominal $2.00 USDC.
2. Untuk mengonsolidasikan 1.000 keping note tersebut menjadi 1 keping note besar yang bisa dibelanjakan atau dicairkan, toko wajib mengeksekusi **999 kali transaksi konsolidasi beruntun (Mode 1 JoinSplit)**:
   $$\text{Jumlah Putaran Konsolidasi} = N - 1 = 1.000 - 1 = 999 \text{ transaksi}$$
3. Total biaya gas konsolidasi yang harus dibayar toko:
   $$\text{Total Gas Konsolidasi} = 999 \times \$0.008 = \$7.992 \text{ USDC}$$
4. **Hasil Akhir:** Toko **rugi bersih $5.99** hanya untuk mengklaim uang $2.00!

Saldo $2.00 tersebut secara permanen berubah menjadi **"Debu Mati" (*Unspendable Dust*)**, yakni kondisi di mana biaya pengeluaran (*cost to spend*) jauh melampaui nilai nominal aset itu sendiri.

---

## 4. Klarifikasi Komponen Kode Eksisting: Memisahkan Ilusi dari Fakta

Tinjauan kode menunjukkan bahwa **tidak ada kode inti yang rusak atau perlu dihapus**, melainkan pemahaman peruntukannya yang harus diluruskan:

| File / Modul | Persepsi Awal (Keliru) | Fungsi Nyata Sebenarnya (Benar & Vital) | Status Kode |
|---|---|---|---|
| [`zeltra-node/src/batch_processor.rs`](file:///workspaces/Zeltra-Protocol/zeltra-node/src/batch_processor.rs) | "Sistem penggabung proof ZK banyak transaksi mikro menjadi 1 tx on-chain ajaib." | **Asynchronous Mempool Queue Worker.** Mengklaim 50 spend antrean dari SQLite WAL dan membroadcast satu per satu ke blockchain secara asinkron agar API HTTP me-return status `QUEUED` instan (< 5 ms). | **TETAP DIPERTAHANKAN** (Sangat vital untuk responsivitas API). |
| [`zeltra-node/src/validation.rs`](file:///workspaces/Zeltra-Protocol/zeltra-node/src/validation.rs) | "Belum ada batasan minimal pengeluaran, rawan spam debu." | **Sudah memiliki filter anti-dust:** `MIN_SPEND_AMOUNT_USDC = 5_000_000` (5 USDC) dan min deposit 10 USDC. Transaksi di bawah 5 dollar otomatis ditolak di preflight sebelum masuk antrean. | **TETAP DIPERTAHANKAN & ENFORCED**. |
| [`zeltra-contracts/src/lib.rs`](file:///workspaces/Zeltra-Protocol/zeltra-contracts/src/lib.rs) | "Apakah ada fungsi batching di smart contract yang tertinggal?" | Tidak ada. Entrypoint legacy `batchSpend(...)` era threshold BLS sudah **dihapus bersih 100%** sejak DEC-038. Kontrak murni menjalankan `spend_private_note` dan `spend_joinsplit`. | **CLEAN & PURGED**. |

---

## 5. Riset Data Pasar Nyata: Narasi AI Agent vs Kenyataan Lapangan

Berdasarkan tinjauan data telemetri industri kuartal 2024–2026 (Coinbase AgentKit, Skyfire, Linux Foundation x402, serta riset independen Visa & Artemis):

### A. Kebenaran di Balik Angka 160 Juta+ Transaksi x402
- Angka kumulatif transaksi x402 memang tercatat melampaui **160 juta transaksi**.
- **Namun data telemetri mengungkap:** **85% s/d 90%** volume tersebut berasal dari bot internal pengembang yang menjalankan *high-frequency automation*, *web crawling micro-tasks*, dan *indexing* data sintetis.
- Transaksi organik manusia-ke-mesin atau antar-entitas komersial independen terkonsentrasi hanya pada **top 1% wallet AI Agent**.

### B. Rahasia "Gasless" di Rantai Base
- Protokol x402 di Base tampak sangat mulus bagi agen karena agen membayar **0 gas**.
- Hal ini terjadi karena **pihak fasilitator (Coinbase & Cloudflare) bertindak sebagai sponsor yang menanggung gas (*Gas Sponsorship*) 100%** melalui skema `transferWithAuthorization` EIP-3009.
- Coinbase rela menanggung biaya ini sebagai biaya akuisisi pengguna (*user acquisition cost*) karena biaya transfer ERC-20 standar di Base hanya ~$0.0001.
- **Di Zeltra:** Eksekusi ZK Groth16 di Arbitrum Stylus membutuhkan daya komputasi EIP-2537 (~$0.008). Zeltra adalah protokol independen terdesentralisasi, bukan entitas korporat yang mensubsidi gas jutaan dollar untuk membakar modal secara sia-sia.

### C. Di Mana Uang Riil Dibelanjakan oleh AI Agent?
Pelaku industri yang benar-benar membelanjakan modal nyata pada AI Agent (agen pengelola perbendaharaan kripto, bot arbitrase otonom, dan korporat penyewa inferensi AI) bertransaksi pada skala:
- **$10 s/d $500 per transaksi** untuk sewa node komputasi GPU (Together AI, RunPod, DeepSeek).
- **$50 s/d $5.000 per bulan** untuk akses feed data pasar real-time berlisensi (Bloomberg, Kaiko, Chainlink).
- **Pola Transaksi Nyata:** Menggunakan skema **Top-Up Saldo Prabayar (*Prepaid Credit Balance*)**, persis seperti model billing OpenAI, Claude, dan Cloudflare, **BUKAN** melakukan kliring on-chain setiap kali satu kata prompt di-generate.

---

## 6. Peta Product-Market Fit (PMF) Sejati Zeltra Protocol

DNA sejati Zeltra Protocol bukanlah mesin kliring receh 0.0001 sen, melainkan:
> **"The Stripe of Web3 with Absolute Privacy" — Rel pembayaran dan penyelesaian kas digital universal berkecepatan tinggi dengan jaminan privasi matematika mutlak.**

Di sinilah likuiditas dan volume nyata mengalir:

```
┌────────────────────────────────────────────────────────────────────────────────────────┐
│                                ZELTRA PROTOCOL REAL PMF                                │
├──────────────────────────┬──────────────────────────┬──────────────────────────────────┤
│ 1. Retail & E-Commerce   │ 2. Whale & Freelancer    │ 3. B2B Corporate Treasury        │
│    (Dynamic QRIS Widget) │    (Private Remittance)  │    & Payroll                     │
│    Belanja $10 - $200    │    Transfer $500 - $10K  │    Gaji & Invoice Vendor         │
│    Gas: 0.04% vs Stripe  │    Saldo Master Wallet   │    Kerahasiaan data komersial    │
│    (2.9% + 30¢)          │    100% Terlindungi      │    terlindungi dari kompetitor   │
├──────────────────────────┴──────────────────────────┴──────────────────────────────────┤
│ 4. Cross-Chain Shielded Liquidity Hub (Chainlink CCIP v2 Inbound & Outbound)           │
│    Deposit dari Base / Ethereum, belanja/payout privat di Arbitrum, cashout fleksibel  │
├────────────────────────────────────────────────────────────────────────────────────────┤
│ 5. Enterprise AI Agent Treasury (Prepaid Quota / Top-Up $10 - $100 via x402)          │
│    Otorisasi transaksi otonom tanpa kebocoran strategi modal atau alamat master agen   │
└────────────────────────────────────────────────────────────────────────────────────────┘
```

### 1. Retail & E-Commerce (Shielded Dynamic QRIS)
- Konsumen belanja barang riil: pakaian, tiket, elektronik, layanan cloud seharga **$10 s/d $200**.
- Menggunakan **Dynamic QRIS / Deep Link** (`zeltra://pay?invoice=...`):
  - Pembeli memindai QR code, memilih note via Knapsack, dan mengirim Mode 2 in-pool transfer ke merchant (`zs1...`).
  - Biaya gas Arbitrum ~$0.008 pada transaksi $20 adalah **0.04%** (Stripe menarik 2.9% + 30¢ = $0.88).
  - Penjual tidak dapat melihat saldo total wallet pembeli di blockchain explorer (*zero doxxing*).

### 2. Whale & Freelance Remittance
- Pembayaran jasa desainer, auditor, pengembang software bernilai **$500 s/d $5.000**.
- Menghilangkan stigma dan bahaya fisik di mana alamat penerima atau pengirim terekspos memiliki jutaan dollar di Etherscan.

### 3. B2B Corporate Payroll & Vendor Settlement
- Pembayaran gaji karyawan dan invoice vendor B2B tanpa risiko kebocoran struktur gaji internal ke sesama karyawan atau strategi finansial ke kompetitor.
- Didukung oleh modul audit selektif **DEC-040** untuk pelaporan pajak legal tanpa mengorbankan privasi publik.

### 4. Cross-Chain Shielded Hub (DEC-055 & DEC-056)
- Likuiditas masuk bebas biaya (0 bps) dari Base, Ethereum, Optimism melalui Router CCIP.
- Likuiditas berputar bebas biaya (0 bps) di dalam kolam Zeltra.
- Likuiditas keluar (Mode 3) dikenakan biaya protokol flat 45 bps (0.45%) dan ditransfer ke rantai tujuan.

### 5. Enterprise AI Agent Tier
- AI Agent dibekali anggaran belanja perbendaharaan (*Agent Treasury*) sebesar **$10 s/d $100** dalam bentuk private note.
- Agen melakukan top-up kuota komputasi sekali transaksi via x402, lalu mengonsumsi API secara instan di off-chain.

---

## 7. Kebijakan Protokol & Keputusan Tindak Lanjut

1. **Penegakan Prinsip Pengguna-Membayar-Gas (*User-Pays-Gas Principle*):**
   - Protokol tidak akan pernah mensubsidi biaya gas secara sepihak. Seluruh pengeluaran on-chain wajib meng-cover biaya eksekusi relayer (`execution_fee`) dari saldo note pengirim atau melalui siaran mandiri (*Direct Self-Broadcast* DEC-054).
2. **Pertahanan Batas Minimal (*Anti-Dust Guard*):**
   - Nilai pengeluaran on-chain minimal tetap dipertahankan pada ambang batas aman:
     $$\text{Minimal Pengeluaran} \ge \text{Biaya Gas Eksekusi L2}$$
     (Default relayer: `MIN_SPEND_AMOUNT_USDC = 5 USDC`, dapat dikonfigurasi fleksibel hingga batas wajar merchant misal $0.10 s/d $1.00).
3. **Penyempurnaan SDK Merchant (Dynamic QRIS Gateway):**
   - Mengembangkan modul `zeltra-sdk/src/merchant/invoice.rs` untuk memfasilitasi pembuatan invoice dinamis terstandar yang kompatibel ganda:
     - Tampilan Visual QR / Deep Link untuk interaksi manusia (HP/Kasir).
     - Payload Base64 JSON untuk interaksi mesin (AI Agent x402).
4. **Pembersihan Istilah (*Zero Bureaucratic Bloat*):**
   - Mengubah dokumentasi pada `batch_processor.rs` agar secara eksplisit dinamai sebagai **Mempool Queue Worker Asinkron**, guna menghindari kesalahpahaman pengembang di masa depan bahwa modul ini melakukan agregasi ZK on-chain.

---

*Dokumen ini sah sebagai pedoman arah pengembangan arsitektur Zeltra Protocol.*
