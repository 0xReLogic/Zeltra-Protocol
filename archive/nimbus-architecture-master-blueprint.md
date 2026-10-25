# ZELTRA PROTOCOL (NIMBUS) — MASTER BLUEPRINT ARSITEKTUR & ARAH MASA DEPAN
## Sintesis Komprehensif: Pure ZK-UTXO, Merkle Mountain Range, Viewing Keys, Komparasi Industri (Railgun & Aztec), dan Positioning B2B/AI Commerce

**Status:** Canonical Strategic & Architectural Roadmap  
**Target Ekosistem:** Arbitrum Stylus (Rust WASM), L2 Ecosystem, Global Financial Rails  

---

## DAFTAR ISI
1. [Eksekutif Ringkasan & Misi Strategis](#1-eksekutif-ringkasan--misi-strategis)
2. [Evolusi Arsitektur: Migrasi Penuh ke Pure ZK-UTXO (DEC-038)](#2-evolusi-arsitektur-migrasi-penuh-ke-pure-zk-utxo-dec-038)
   - 2.1 Masalah Legacy Threshold Blind Signing
   - 2.2 Direct MMR Inflow: Zero-Friction & Zero-Ceremony
   - 2.3 Solvensi Matematika Absolut & Nilai Konservasi
3. [Arsitektur Privasi ZK-UTXO (Inflow, Pool, Outflow)](#3-arsitektur-privasi-zk-utxo-inflow-pool-outflow)
   - 3.1 Entry Inflow: Direct Note Commitment & AEAD Encrypted Memo
   - 3.2 Internal Pool: Merkle Mountain Range (MMR) & Universal JoinSplit
   - 3.3 Exit Outflow: ZK Spend, Generational Nullifiers, & Instant Merchant Settlement
4. [Kepatuhan & Pemulihan Tanpa ASP (The "No-ASP" Model & Viewing Keys)](#4-kepatuhan--pemulihan-tanpa-asp-the-no-asp-model--viewing-keys)
   - 4.1 Mengapa Model ASP Konvensional Gagal
   - 4.2 Viewing Key Recovery Protocol (DEC-037)
   - 4.3 Receiver-Enforced Modular ASP & Screening Sanksi Fail-Closed (DEC-026)
   - 4.4 Alur Kerja Verifikasi CEX & Audit B2B
5. [Studi Komparasi Industri: Belajar dari Railgun & Aztec](#5-studi-komparasi-industri-belajar-dari-railgun--aztec)
   - 5.1 Apa yang Diadopsi dari Railgun (UTXO Pool & Untrusted Relayers)
   - 5.2 Apa yang Diadopsi dari Aztec (Viewing Keys & Encrypted Note Discovery)
   - 5.3 Jebakan Railgun & Aztec yang Dihindari Zeltra
   - 5.4 Matriks Head-to-Head Komparatif
6. [Arsitektur Node Relayer: Stateless Single-Sequencer](#6-arsitektur-node-relayer-stateless-single-sequencer)
   - 6.1 Eliminasi Klaster Kompleks Menuju Stateless Sequencer
   - 6.2 Preflight CPU Verification & Anti Gas-Griefing (DEC-026 C-01)
   - 6.3 Three-Way Reconciliation & Solvency Observability (DEC-031)
7. [Dampak Ekonomi Nyata: Dari Niche AI Menuju Infrastruktur B2B Global](#7-dampak-ekonomi-nyata-dari-niche-ai-menuju-infrastruktur-b2b-global)
   - 7.1 Solusi Penggajian Karyawan (Private Corporate Payroll)
   - 7.2 Pembayaran Vendor B2B & Invoice Dagang
   - 7.3 Privasi Kas Merchant Retail & E-Commerce
   - 7.4 Ekonomi Agen AI Otonom Berkecepatan Sub-Detik
8. [Matematika Nilai, Konstanta Imutabel & Kebijakan Biaya](#8-matematika-nilai-konstanta-imutabel--kebijakan-biaya)
   - 8.1 Invarian Pembukuan & Konservasi Solvensi
   - 8.2 Struktur Fee Imutabel (Zero-Deposit & Flat 45 bps Spend)
   - 8.3 Pemisahan Biaya CCIP Network Fee vs Relayer Gas
   - 8.4 Efisiensi Komputasi WASM Stylus via Precompile EIP-2537
9. [Peta Jalan Implementasi Teknis](#9-peta-jalan-implementasi-teknis)

---

## 1. Eksekutif Ringkasan & Misi Strategis

Nimbus Protocol didirikan atas satu prinsip dasar yang tidak dapat ditawar:
$$\mathbf{Satu\ deposit\ membayar\ tepat\ satu\ kali,\ tidak\ ada\ sisa\ uang\ yang\ hilang,\ dan\ tidak\ ada\ aset\ tanpa\ backing.}$$

Selama ini, lanskap privasi Web3 terbelah ke dalam dua kutub ekstrim:
1. **Mixer Kripto Anarkis (misal: Tornado Cash, Railgun murni):** Memberikan privasi kriptografis tinggi, namun terisolasi dari ekonomi dunia nyata karena dana yang keluar dianggap beracun (*tainted*) dan akun pengguna dibekukan (*freeze*) oleh Centralized Exchanges (CEX) dan perbankan karena tidak memiliki jalur audit kepatuhan.
2. **Blockchain Transparan Konvensional (EVM):** Menghancurkan hak privasi finansial dasar. Perusahaan yang membayar gaji karyawan atau melunasi invoice vendor menggunakan transfer publik di Arbitrum/Ethereum membuka seluruh rahasia kas, laba kotor, dan kompensasi ke hadapan kompetitor dan peretas on-chain.

**Nimbus Protocol hadir sebagai sintesis penutup:** Sebuah infrastruktur perbankan privat (*Private Banking Rails*) generasi baru yang berjalan secara native di atas **Arbitrum Stylus (Rust WASM)**. Nimbus memadukan kecepatan sub-detik, biaya masuk 0% (*Zero-Friction Inflow*), privasi kriptografis absolut (ZK-UTXO + Stealth Routing), dengan sistem **Selective Disclosure Tanpa ASP** yang memungkinkan pengguna membuktikan legalitas dana mereka secara selektif ke CEX dan regulator tanpa pernah membuka saldo atau riwayat transaksi publik mereka.

---

## 2. Evolusi Arsitektur: Migrasi Penuh ke Pure ZK-UTXO (DEC-038)

Analisis forensik dan evaluasi performa sistem awal membuktikan perlunya perombakan total dari model hibrida *Threshold BLS* menuju **Pure ZK-UTXO Architecture (DEC-038)**. Tiga kelemahan kritis arsitektur lama telah dieliminasi secara permanen:

### 2.1 Masalah Legacy Threshold Blind Signing
Pada model legacy (DEC-018), pengguna bergantung pada kuorum 3-of-5 Guardian dan 1 Leader untuk pelepasan kunci masking $k$:
* **Kelemahan Kolusi & Single Leader:** Jika Leader tumbang, pelepasan kunci macet dan pengguna harus menunggu timelock refund 24 jam. Ketergantungan ini tidak dapat diterima untuk agen AI berfrekuensi tinggi.
* **Overhead Storage On-Chain:** Kontrak Stylus sebelumnya membebani 11 storage slot per sesi deposit (status sesi, hash komitmen, timestamp, dll.) yang boros gas dan membatasi throughput.
* **Solusi DEC-038A:** Seluruh kode guardian, blind signing, dan tabel sesi dihapus bersih dari codebase. Kontrak kini sepenuhnya stateless terhadap deposit individu.

### 2.2 Direct MMR Inflow: Zero-Friction & Zero-Ceremony (DEC-038B / DEC-038C)
Melalui fungsi `deposit_direct_note(...)`, pengguna menyetor dana langsung ke dalam Merkle Mountain Range (MMR) kontrak:
* **Zero Threshold Ceremony:** Pengguna membuat komitmen note off-chain via Poseidon-W5 ($< 15\ \mu\text{s}$) dan menyetorkannya langsung ke on-chain MMR accumulator (`_mmr_insert`).
* **Instant Unspent State:** Note seketika aktif dan dapat langsung dibelanjakan tanpa menunggu persetujuan entitas off-chain apa pun.
* **96-Byte AEAD Encrypted Memo:** Melalui event `NoteCommitmentAppended`, SDK menyertakan payload ChaCha20Poly1305 terenkripsi via ephemeral ECDH X25519 (DEC-037). Nol storage bloat karena memo hanya disimpan di event log Arbitrum.

### 2.3 Solvensi Matematika Absolut & Nilai Konservasi
Invarian solvensi diperiksa pada setiap mutasi state di smart contract Stylus:
$$\text{USDC Balance} \ge \text{user\_note\_liability} + \text{accrued\_execution\_fee\_liability} + \text{realized\_protocol\_fees}$$
$$\text{Input Note(s)} = \text{Payout} + \text{Protocol Fee} + \text{Execution Fee} + \text{Change Note(s)}$$
* **0 bps Deposit Fee:** Setoran 100 USDC menambahkan tepat 100 USDC liability (`gross_deposit = deposit_amount = net_liability`).
* **Satu Deposit = Tepat Satu Payout:** Satu komitmen menambah liability tepat satu kali di MMR; satu nullifier memusnahkan liability tepat satu kali di tabel `note_nullifiers`.

---

## 3. Arsitektur Privasi ZK-UTXO (Inflow, Pool, Outflow)

Untuk mencapai privasi maksimal 10/10 dan kemudahan produk 10/10, Zeltra menerapkan perlindungan berlapis di tiga zona:

```
[ PINTU MASUK: INFLOW ]           [ INTERNAL POOL: ACCUMULATOR ]       [ PINTU KELUAR: OUTFLOW ]
Direct Note Creation (<15 µs)      Merkle Mountain Range (MMR)           Groth16 Verification (EIP-2537)
        │                                        │                                       │
        ▼                                        ▼                                       ▼
Poseidon-W5 Note Commitment        On-Chain Dynamic MMR Leaves           Nullifier Burning (Double-Spend Lock)
96-Byte AEAD Memo (DEC-037)        Anti-OOB Peak Bagging (DEC-032)       Direct USDC Payout + Change Note
(0 bps Fee, Instant Unspent)       Universal JoinSplit (DEC-036A)        Flat 45 bps Protocol Fee (DEC-028)
```

### 3.1 Entry Inflow: Direct Note Commitment & AEAD Encrypted Memo (DEC-038B & DEC-037)
* **Pemisahan Identitas Penyetor:** Penyetor menyertakan komitmen note `H(value, owner_pk, rho, r)` ke kontrak. Alamat pengirim deposit tidak memiliki tautan kriptografis dengan kepemilikan note.
* **Log-Only Memo Delivery:** Memo 96-byte berisi ciphertext `NotePlaintext` (value, rho, r) dienkripsi menggunakan Ephemeral Diffie-Hellman X25519 penerima. Hanya pemilik kunci yang dapat mendekripsi note tersebut melalui pemindaian event log.

### 3.2 Internal Pool: Merkle Mountain Range (MMR) & Universal JoinSplit
* **Merkle Mountain Range Akumulator (DEC-032):** Menggantikan struktur pohon statis dengan MMR yang hemat gas ($O(\log N)$ pembuktian, $O(1)$ amortized insert on-chain).
* **Canonical Peak Bagging Backward Fold:** Bagging puncak MMR menggunakan domain separation `DOMAIN_MMR_BAG` dengan validasi rentang ketat ($leaf\_index < leaf\_count$) untuk mencegah celah *out-of-bounds* (Hyperbridge exploit mitigation).
* **Universal 2-in-2-out JoinSplit (DEC-036A):** Mendukung topologi penggabungan konstan: 1-in-1, 1-in-2, 2-in-1, dan 2-in-2 note melalui kanonikal dummy slots dan 448 quadratic range gates.

### 3.3 Exit Outflow: ZK Spend, Generational Nullifiers, & Instant Merchant Settlement
* **Verifikasi Groth16 On-Chain (EIP-2537):** Eksekusi verifikasi via host precompile Arbitrum Stylus (`0x0c` MSM + `0x0f` 4-pairing check) berlangsung di bawah 1 detik dengan efisiensi gas WASM native.
* **Siloed Nullifier Protection:** Pengeluaran note menghasilkan nullifier unik yang disimpan di `note_nullifiers[nullifier] = true`. Percobaan double-spend ditolak secara absolut.
* **Generational 2-Epoch Nullifiers (DEC-033):** Memungkinkan rotasi epoch dan kompresi state nullifier jangka panjang tanpa membatalkan hak kepemilikan note aktif.
* **Merchant Masking & Stealth Settlement (Anti-Clustering):**
  - *Masalah Publik:* Jika ribuan pembeli mentransfer pembayaran langsung ke alamat publik merchant yang sama (`0xMerchant...`), grafik transaksi on-chain akan membentuk gugus (*clustering*) yang mengekspos omset penjualan dan volume kas merchant ke publik dan kompetitor.
  - *Dua Mode Pelindung Privasi Merchant:*
    1. **In-Pool Private Transfer (Zero-Footprint):** Pembeli mengirimkan private note baru langsung ke viewing key merchant di dalam pool MMR tanpa pernah keluar ke publik. Saldo merchant bertambah secara privat.
    2. **Stealth Payout Routing (ERC-5564 Ephemeral Exit):** Jika merchant menghendaki pencairan ke wallet publik, dana diarahkan ke alamat siluman sekali pakai (*one-time stealth address*) yang diturunkan dari spending public key merchant. Publik hanya melihat pembayaran acak yang tidak dapat ditautkan ke dompet utama toko.

---

## 4. Kepatuhan & Pemulihan Tanpa ASP (The "No-ASP" Model & Viewing Keys)

Zeltra mengeliminasi ketergantungan pada pihak ketiga (Association Set Provider) dengan prinsip: *"Permissionless Privacy, Permissioned Acceptance"*.

### 4.1 Mengapa Model ASP Konvensional Gagal
1. **Risiko Hukum & Sentralisasi:** Penilai ASP pihak ketiga menjadi titik tunggal sensor (*single point of censorship*) dan target hukum otoritas regulator.
2. **Penolakan Bursa Utama (CEX):** CEX besar (Binance, Coinbase, Kraken) tidak mempercayai label ASP eksternal; mereka mengandalkan mesin KYT in-house (Chainalysis, TRM Labs).

### 4.2 Viewing Key Recovery Protocol (DEC-037)
Sistem Zeltra menyediakan hierarki kunci deterministik berbasis HKDF:
* **Incoming Viewing Key (`ivk`):** Memungkinkan dompet memindai dan mendekripsi note masuk tanpa hak membelanjakan dana.
* **Outcoming Viewing Key (`ovk`):** Memungkinkan pembuktian audit transaksi keluar ke entitas berwenang atau akuntan perusahaan.
* **Pemulihan Saldo Otomatis (`recover_notes`):** Klien SDK dapat melakukan trial-decryption atas seluruh event `NoteCommitmentAppended` di Arbitrum untuk memulihkan seluruh saldo dan unspent note secara instan pada perangkat baru.

### 4.3 Receiver-Enforced Modular ASP & Screening Sanksi Fail-Closed (DEC-026)
* **Kepatuhan di Sisi Penerima:** Merchant atau institusi penerima dapat menetapkan kriteria kepatuhan sendiri saat menerima transfer privat.
* **Sinkronisasi Sanksi OFAC Otomatis:** Relayer node menyinkronkan daftar sanksi OFAC secara otomatis setiap 24 jam (`sanctions_sync.rs`). Jika alamat penerima atau penyetor terindikasi dalam sanksi, relayer menolak pemrosesan transaksi secara *fail-closed*.

### 4.4 Alur Kerja Verifikasi CEX & Audit B2B
Pengguna yang menyetor ke bursa teregulasi dapat mengekspor bukti kepemilikan note dan jejak deposit legal tanpa membuka saldo sisa atau riwayat pengeluaran lainnya ke publik. Hal ini menjamin dana tidak dibekukan CEX saat pencairan ke fiat.

---

## 5. Studi Komparasi Industri: Belajar dari Railgun & Aztec

### 5.1 Apa yang Diadopsi & Ditingkatkan dari Railgun
1. **Model Relayer Untrusted (Tanpa Hak Istimewa):** Pada Railgun, relayer hanyalah perantara pembayaran gas tanpa wewenang custodial. Jika relayer mati, pengguna dapat melakukan *self-relay* langsung ke smart contract. Zeltra mengadopsi model ini untuk memutus ketergantungan pada sequencer.
2. **Inovasi Akumulator: Melompati LeanIMT Railgun Menuju Merkle Mountain Range (MMR):**
   * *Railgun:* Menggunakan pohon Merkle bertingkat tetap (LeanIMT / fixed-depth Merkle tree tingkat 16), yang membatasi kapasitas maksimum leaf dan mahal saat verifikasi kedalaman statis.
   * *Zeltra Innovation (DEC-032):* Zeltra meninggalkan LeanIMT dan bermigrasi ke **Merkle Mountain Range (MMR)**. MMR tumbuh dinamis tanpa batas kedalaman ($2^{64}$ kapasitas), insertion on-chain $O(1)$ amortized, dan pembuktian puncak (*peak bagging*) super hemat gas di Stylus WASM.

### 5.2 Apa yang Diadopsi dari Aztec (Viewing Keys & Encrypted Note Discovery)
1. **Pemisahan Spending Key vs Viewing Key:**
   * *Spending Key:* Digunakan untuk mengotorisasi pengeluaran dana via sirkuit Groth16.
   * *Viewing Key (`ivk`, `ovk`):* Kunci khusus yang hanya dapat mendekripsi riwayat transaksi dan saldo (DEC-037), namun **tidak memiliki hak membelanjakan dana sepeser pun**. Kunci ini menjadi senjata utama kepatuhan B2B (diserahkan ke auditor eksternal / kantor pajak).
2. **Encrypted Note Ciphertext:** Pengiriman note dilakukan dengan menyematkan 96-byte memo terenkripsi (Diffie-Hellman ephemeral X25519) pada event log transaksi. Dompet penerima secara otomatis memindai dan mengenali dana masuk tanpa mengekspos alamat penerima di ledger publik.

### 5.3 Jebakan Railgun & Aztec yang Dihindari Zeltra
* **Menghindari Perangkap Railgun (CEX Death Trap & Fixed LeanIMT):** Railgun murni berfokus pada privasi DeFi tanpa mekanisme audit terstruktur (sering dibekukan CEX) dan terikat pada fixed-depth LeanIMT. Zeltra melengkapinya dengan hierarki Viewing Keys, Receiver-Enforced ASP (DEC-026), dan dynamic MMR.
* **Menghindari Over-Engineering Aztec (Rollup Baru & Bahasa Baru):** Aztec menghabiskan waktu bertahun-tahun dan ratusan juta dolar membangun Layer-2 rollup mandiri dan bahasa pemrograman baru (Noir), yang berakibat pada fragmentasi likuiditas dan waktu konfirmasi lambat (~72 detik). Zeltra memilih menjadi **protokol native di Arbitrum Stylus (Rust)**, langsung memanfaatkan likuiditas USDC Arbitrum dan kecepatan sub-detik.

### 5.4 Matriks Head-to-Head Komparatif

| Indikator Evaluasi | **RAILGUN** | **AZTEC NETWORK** | **ZELTRA PROTOCOL** |
|---|---|---|---|
| **Struktur Akumulator** | Fixed-Depth Tree / LeanIMT (Tingkat 16) | Historic Tree Rollup | **Merkle Mountain Range (MMR Dynamic Accumulator)** |
| **Model Fondasi** | Shielded Pool EVM | Private L2 Rollup Mandiri | **Stylus WASM Smart Contract di Arbitrum** |
| **Kecepatan Finalitas** | 12 - 15 detik (Blok EVM) | ~72 detik (L2 Batch) | **< 1 detik (Arbitrum Nitro Sub-second)** |
| **Biaya Deposit (Inflow)** | 0.25% - 0.50% (Dipenggal di muka) | Biaya bridge L1 $\rightarrow$ L2 | **0.00% (GRATIS - Zero Inflow Friction)** |
| **Kepatuhan CEX / Institusi** | Sangat Rentan (Sering di-ban CEX) | Terisolasi di ekosistem rollup | **Sangat Ramah (Viewing Keys + Receiver-Enforced ASP)** |
| **Audit B2B / Pajak** | Terbatas & Kompleks | Viewing Keys | **Viewing Keys (`ivk`/`ovk`) + Selective Export** |
| **Dukungan AI Agent** | Kurang Optimal (EVM Gas Tinggi) | Kompleksitas Sirkuit Tinggi | **Sangat Dioptimalkan (Gasless & WASM Batch)** |
| **Bahasa Kontrak** | Solidity EVM | Noir DSL | **Rust Native (WASM)** |

---

## 6. Arsitektur Node Relayer: Stateless Single-Sequencer (DEC-038C)

Dalam arsitektur Pure ZK-UTXO, relayer node (`nimbus-node`) beroperasi sebagai **stateless single-sequencer** yang efisien, aman, dan tangguh:

```
[ KLIEN SDK ]
User / AI Agent
     │  1. Submit Spend Proof + Public Inputs (/api/v1/spend-private-note)
     ▼
[ NIMBUS NODE: STATELESS SEQUENCER ]
├── Preflight Sanitasi & CPU Groth16 Verification (DEC-026 C-01)
├── Dual-Layer Nullifier Guard (SQLite WAL + On-Chain Check)
├── Sanctions Screening Otomatis (OFAC Daily Sync)
└── EVM Client (Alloy): Nonce Lock, Gas Reimbursement (+15% Markup), L2 Broadcast
     │  2. Broadcast ke Arbitrum Stylus
     ▼
[ ARBITRUM STYLUS CONTRACT ]
Groth16 Verification (EIP-2537) ──► Burn Nullifier ──► Append Change to MMR ──► Pay USDC
```

### 6.1 Eliminasi Klaster Kompleks Menuju Stateless Sequencer
* **Tanpa Kunci Rahasia Protokol:** Relayer tidak memegang kunci pembelanjaan pengguna atau kunci master issuer. Relayer murni bertindak sebagai fasilitator transaksi berbayar.
* **Tujuh Background Worker Mandiri:**
  1. `mmr_indexer`: Mengindeks event on-chain `NoteCommitmentAppended` & memo AEAD.
  2. `mempool_watchdog`: Memulihkan lease stalled dan memantau nullifier on-chain.
  3. `sanctions_sync`: Memperbarui daftar sanksi OFAC dari GitHub setiap hari.
  4. `execution_fee_claimer`: Mengklaim akumulasi execution fee dari kontrak Stylus.
  5. `ccip_monitor`: Melacak status transaksi cross-chain & penanganan refund.
  6. `adaptive batch worker`: Mengelompokkan spend transaksi jika antrean tinggi.
  7. `cleanup_worker`: Membersihkan data transien yang telah terselesaikan.

### 6.2 Preflight CPU Verification & Anti Gas-Griefing (DEC-026 C-01)
* **Verifikasi Sebelum Broadcast:** Relayer memverifikasi proof Groth16 secara lokal pada CPU menggunakan `nimbus-core` sebelum membakar gas di Arbitrum.
* **Perlindungan Terhadap DoS/Griefing:** Proof palsu, skalar non-kanonikal ($s \ge r$), atau nullifier yang telah terpakai langsung ditolak pada layer HTTP dengan kode status 400/409 tanpa ada biaya on-chain yang terbuang.

### 6.3 Three-Way Reconciliation & Solvency Observability (DEC-031)
* **Pengecekan Tiga Arah:** Engine `reconcile_execution_fees` membandingkan quote EIP-712 yang ditandatangani, gas aktual yang dikonsumsi transaksi L2, dan fee yang diakui on-chain.
* **Observabilitas Solvensi Real-Time:** Endpoint `/health` dan `/ready` mengekspos metrik solvensi makro: rasio aset USDC terhadap liabilitas note aktif.

---

## 7. Dampak Ekonomi Nyata: Dari Niche AI Menuju Infrastruktur B2B Global

Nimbus bukan sekadar alat privasi individual, melainkan **infrastruktur rel pembayaran privat untuk perputaran uang dunia nyata**:

### 7.1 Solusi Penggajian Karyawan (Private Corporate Payroll)
* **Masalah Industri Saat Ini:** Perusahaan Web3 yang membayar gaji karyawan dengan transfer USDC publik menciptakan bencana privasi internal: karyawan saling membandingkan gaji via Etherscan, dan kompetitor mengetahui besaran burn-rate perusahaan. Menggunakan Tornado Cash menyebabkan rekening bank karyawan dibekukan.
* **Solusi Nimbus:** 
  1. Perusahaan menyetor dana payroll ke Nimbus (0% deposit fee).
  2. Gaji didistribusikan secara privat ke karyawan.
  3. Karyawan memegang *Selective Disclosure Proof* yang membuktikan ke CEX lokal/bank bahwa dana mereka berasal dari penggajian sah PT ABC.
  4. Akuntan perusahaan memegang *Viewing Key* untuk pembukuan laporan keuangan resmi.

### 7.2 Pembayaran Vendor B2B & Invoice Dagang
* **Perlindungan Rahasia Dapur Dagang:** Perusahaan dapat melunasi tagihan supplier internasional tanpa mengekspos daftar vendor, volume pesanan, dan diskon rahasia ke publik.
* **Kepatuhan Pajak Sukarela:** Saat pelaporan SPT Pajak Badan, bagian keuangan mengekspor laporan kepatuhan ZK yang dapat diverifikasi oleh otoritas pajak tanpa mengekspos wallet operasional harian.

### 7.3 Privasi Kas Merchant Retail & E-Commerce
* **Mencegah Spionase Omset:** Merchant online menerima pembayaran stablecoin dari jutaan pelanggan tanpa khawatir saldo kas toko dan alamat penyedia barangnya ditelusuri oleh kompetitor bisnis.

### 7.4 Ekonomi Agen AI Otonom Berkecepatan Sub-Detik
* **Mesin Bayar Mesin:** Agen AI yang menyewa daya komputasi GPU, membeli dataset, atau membayar kueri LLM per detik mendapatkan jalur transaksi tanpa friksi:
  * Saldo deposit tidak terpotong (0% fee).
  * Saldo dipecah ke kompartemen ephemeril (*ERC-8000.4 / ERC-8183*) untuk mencegah pelacakan profil agen.
  * Transaksi dieksekusi secara instan (< 1 detik) melalui rollup Arbitrum Nitro.

---

## 8. Matematika Nilai, Konstanta Imutabel & Kebijakan Biaya

### 8.1 Invarian Pembukuan & Konservasi Solvensi
Setiap transisi status di dalam smart contract Stylus wajib memenuhi invarian matematika mutlak (DEC-009 / DEC-038B):
$$\text{USDC Balance} \ge \text{user\_note\_liability} + \text{accrued\_execution\_fee\_liability} + \text{realized\_protocol\_fees}$$
$$\text{Input Note(s)} = \text{Payout} + \text{Protocol Fee} + \text{Execution Fee} + \text{Change Note(s)}$$

Jika terjadi deviasi sebesar 1 base unit sekalipun, eksekusi kontrak akan melakukan *revert* seketika untuk mencegah insolvensi.

### 8.2 Struktur Fee Imutabel (Zero-Deposit & Flat 45 bps Spend)
Berdasarkan DEC-028, diskon holding-time dihapuskan demi mencegah manipulasi usia akar dan menjaga transparansi mental accounting kas ($10 - $3 = $7). Konstanta biaya dikunci secara permanen di tingkat kode (`nimbus-core/src/fees.rs`):

| Parameter Biaya | Nilai Parameter | Rasionalisasi Game Theory |
|---|---|---|
| `DEPOSIT_FEE_BPS` | **0 bps (0.00%)** | *Zero-friction inflow* untuk memaksimalkan pertumbuhan volume dan kedalaman set privasi |
| `PRIVATE_SPEND_FEE_BPS` | **45 bps (0.45%)** | Fee seragam flat di seluruh spend note untuk perbendaharaan protokol |
| `DEFAULT_RELAYER_MARKUP_BPS` | **1,500 bps (15.00%)** | Kompensasi wajar relayer untuk menanggung risiko volatilitas gas L2 |

* **Ceiling Division Rounding:** Seluruh pembagian biaya menggunakan pembulatan ke atas (*ceiling division*), menjamin protokol tidak pernah dirugikan oleh fraksi desimal:
  $$\text{fee} = \left\lceil \frac{\text{amount} \times \text{fee\_bps}}{10,000} \right\rceil$$

### 8.3 Pemisahan Biaya CCIP Network Fee vs Relayer Gas (Transparansi UX)
Berlandaskan DEC-020, biaya pada transaksi lintas rantai (Cross-Chain CCIP) dipisahkan secara transparan pada endpoint `/api/quote/private-spend`:
$$\text{Execution Fee} = \text{Relayer Gas Cost} + \text{Relayer Markup (15\% dari Relayer Gas)} + \text{CCIP Network Fee}$$

* **Insulasi Markup:** Markup 15% relayer **HANYA dikenakan pada gas relayer lokal Arbitrum**, TIDAK PERNAH mem-markup biaya jaringan Chainlink CCIP (yang merupakan biaya pass-through murni pihak ketiga).

### 8.4 Efisiensi Komputasi WASM Stylus via Precompile EIP-2537
Dengan memanfaatkan Arbitrum Stylus (kompilasi biner WebAssembly):
* Verifikasi pasangan kurva eliptis (*BLS Pairing Check*) dan sirkuit Groth16 dieksekusi via host precompile EIP-2537 (`0x0c` MSM + `0x0f` 4-pairing check) mendekati kecepatan native mesin.
* Biaya verifikasi on-chain jauh lebih murah dibandingkan EVM konvensional, memungkinkan settlement instan tanpa membebani kas pengguna.

---

## 9. Peta Jalan Implementasi Teknis

```text
┌─────────────────────────────────────────────────────────────────────────────┐
│ FASE 1: Fondasi Kriptografi & Pure ZK-UTXO (SELESAI)                        │
├─────────────────────────────────────────────────────────────────────────────┤
│ [x] Smart Contract Stylus: Verifikasi pairing BLS12-381 EIP-2537 on-chain. │
│ [x] Sirkuit Groth16: 1-in-1-out (16 PIs) & Universal JoinSplit (19 PIs).    │
│ [x] Akumulator MMR: In-memory & on-chain dengan domain separation bagging.  │
│ [x] Direct Note Inflow: deposit_direct_note & 96-byte AEAD encrypted memo.  │
│ [x] Zero-deposit fee imutabel (0 bps) & Flat 45 bps spend fee (DEC-028).    │
│ [x] Stateless Sequencer Relayer: Preflight CPU verify & 7 background workers│
└─────────────────────────────────────────────────────────────────────────────┘
                                       │
                                       ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│ FASE 2: Hard-Testing & Verifikasi Testnet Sepolia (FOKUS AKTIF)             │
├─────────────────────────────────────────────────────────────────────────────┤
│ [ ] E2E Testnet Sepolia: Deposit direct -> spend note -> change -> payout.  │
│ [ ] Cross-Chain CCIP: Testnet broadcast -> router -> destination execution. │
│ [ ] Testing batas idempotensi & crash-recovery relayer node.                │
│ [ ] Freeze ABI sirkuit & batasan constraints (Gate C).                      │
│ [ ] Persiapan Trusted Setup Ceremony (MPC Phase 2).                         │
└─────────────────────────────────────────────────────────────────────────────┘
                                       │
                                       ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│ FASE 3: Kepatuhan Institusional & Ekosistem SDK (DESENTRALISASI)             │
├─────────────────────────────────────────────────────────────────────────────┤
│ [ ] Shielded-to-Shielded In-Pool Transfer: Pengiriman private note antar    │
│     wallet Zeltra via JoinSplit 2-output tanpa withdraw USDC publik         │
│     (Merchant Masking & Zero-Footprint Commerce).                           │
│ [ ] Viewing Key Export Suite: Export compliance report untuk pajak/CEX.     │
│ [ ] SDK Helper: "Verify & Export SoF for Binance / Coinbase".               │
│ [ ] In-browser WebAssembly SDK prover untuk mobile/web client.              │
│ [ ] Integrasi x402 Facilitator untuk payment streaming mikro AI Agent.      │
└─────────────────────────────────────────────────────────────────────────────┘
                                       │
                                       ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│ FASE 4: Audit & Peluncuran Mainnet Arbitrum                                  │
├─────────────────────────────────────────────────────────────────────────────┤
│ [ ] Audit Keamanan Penuh oleh Lembaga Audit ZK Tier-1.                      │
│ [ ] Program Bug Bounty Publik di Arbitrum One.                              │
│ [ ] Pilot Payroll B2B & Kemitraan Merchant Commerce.                        │
│ [ ] Rilis SDK Agen AI Otonom Terkompartementalisasi.                        │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 10. Pernyataan Penutup: Mengapa Nimbus Menang

Privasi finansial bukan kejahatan; privasi finansial adalah syarat mutlak agar perdagangan dunia nyata dapat berfungsi di atas blockchain. 

Dengan menggabungkan:
1. **Kecepatan dan Efisiensi Arbitrum Stylus (Rust WASM)**,
2. **Kenyamanan Ekonomi 0% Deposit Fee**,
3. **Privasi Absolut ZK-UTXO & Stealth Routing**, serta
4. **Paspor Kepatuhan CEX Tanpa Ketergantungan ASP**,

Nimbus Protocol berdiri sebagai arsitektur paling kokoh dan siap-pakai untuk memimpin revolusi pembayaran privat—menghubungkan kebutuhan korporat B2B, penggajian karyawan, perdagangan retail, hingga transaksi otonom agen kecerdasan buatan dalam satu rel perbankan privat global yang tak terhentikan.
