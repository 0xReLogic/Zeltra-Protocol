# Roadmap Prioritas Rekayasa Zeltra Protocol (2026)

Dokumen ini adalah kompas urutan pengerjaan tim.

> 💡 **Prinsip Utama Rekayasa (Anti-Burnout / Zero Gas Waste):**  
> **"Local Simulation & Off-Chain Verification First, Single-Pass Testnet Last."**  
> Jangan deploy kontrak ke Arbitrum Sepolia berulang-ulang saat sirkuit, invariant, atau SDK masih terus dimodifikasi. Selesaikan seluruh constraint, verifikasi formal, SDK tooling, dan Trusted Setup secara lokal. Testnet hanya dijalankan **SEKALI SAJA** sebagai gladi bersih sebelum peluncuran mainnet.

---

## Fase 1: SDK, Browser Prover & Ekosistem Fitur [SELESAI 100% & TERVERIFIKASI]
Ref: [`research/dec/DEC-040`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-040-viewing-key-export-suite-and-cex-selective-disclosure.md), [`DEC-041`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-041-in-browser-wasm-groth16-prover-and-disposable-worker.md), [`DEC-042`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-042-privacy-preserving-x402-facilitator-and-pure-zk-utxo-micropayments.md), [`DEC-043`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-043-shielded-to-shielded-in-pool-transfer-and-merchant-masking.md)

- [x] **Client SDK & Browser Runner (`zeltra-sdk`):**
  - In-Browser WebAssembly SDK Prover: Eksekusi Groth16 client-side non-blocking via Web Worker dengan isolasi memori anti-OOM linear (`worker.terminate()`) (DEC-041).
  - Viewing Key Recovery Suite: Ekstraksi dan dekripsi saldo note via HKDF `ovk`/`ivk` dari memo on-chain, tooling export laporan audit kas & SPT pajak B2B (DEC-037 & DEC-040B).
  - CEX Helper Tooling: Modul "Export Source of Funds" untuk pembuktian selektif asal-usul dana bersih ke bursa terpusat (Binance/Coinbase) dengan Verifier Binding (DEC-040A/C).
- [x] **Shielded-to-Shielded In-Pool Transfer (DEC-043):**
  - Transfer note privat langsung antar akun/merchant Zeltra (`output_commitment_1` penerima, `output_commitment_2` kembalian) tanpa konversi ke USDC publik on-chain (Merchant Masking). 44/44 test Python real-case lolos.
- [x] **HTTP 402 Pure ZK-UTXO Micropayments (x402 Facilitator - DEC-042):**
  - Streaming payment mikro untuk AI Agent via Pure ZK-UTXO Groth16 1-in-1-out, dynamic dust threshold tracking, zero-oracle reimbursement.
- [x] **Cross-Chain CCIP Simulation:**
  - Verifikasi isolasi logic CCIP router, mapping message ID, dan skenario penolakan double-spend secara lokal.

---

## Fase 2: Circuit Freeze, Security Audit & Trusted Setup MPC (FOKUS AKTIF SAAT INI)
Ref: [`research/dec/DEC-035`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-035-cryptographic-canonicality-public-input-binding-and-mmr.md) & [`DEC-036`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-036-universal-2-in-2-out-joinsplit-full-stack-architecture.md)

*Kunci mati seluruh spesifikasi sebelum menyentuh jaringan publik:*

- [ ] **Circuit Constraints & ABI Freeze:**
  - Kunci constraint R1CS 16 PI (`PrivateNoteCircuit`) dan 19 PI (`JoinSplitCircuit`).
  - Finalisasi parameter Two-Limb quote hash, Formal Scope Binding Gadget, dan 448 quadratic range gates.
- [ ] **Independent Circuit & Contract Audit:**
  - Review formal invariant solvensi matematis (`contract balance >= total_liabilities`).
  - Analisis keamanan curve BLS12-381 precompile EIP-2537 di Stylus WASM.
- [ ] **Production MPC Ceremony (Phase 2 Trusted Setup):**
  - Eksekusi MPC Trusted Setup multi-partai untuk menghasilkan final proving & verifying keys resmi.
  - Simpan verifying keys resmi sebagai artifact versioned ke dalam smart contract Stylus.

---

## Fase 3: Sanitasi Kredensial & Pembersihan Git History

- [ ] **Credential Rotation & History Cleanup:**
  - Rotasi seluruh secret/private key dev historis dan pembersihan git commit history sebelum peluncuran testnet publik.

---

## Fase 4: Unified Hard-Test Arbitrum Sepolia (GLADI BERSIH AKHIR)
Ref: [`docs/todos/testnet-gate-g-checklist.md`](file:///workspaces/Zeltra-Protocol/docs/todos/testnet-gate-g-checklist.md)

*Deploy kontrak dan jalankan relayer node **SEKALI SAJA** dengan verifying key resmi hasil MPC:*

- [ ] **Matrix Validasi End-to-End On-Chain:**
  - Deposit direct (0 bps fee) $\rightarrow$ in-pool transfer $\rightarrow$ partial spend $\rightarrow$ change note $\rightarrow$ second spend $\rightarrow$ withdraw-all.
  - Verifikasi saldo kontrak reconcile exact terhadap total liabilities (USDC & WETH).
- [ ] **Crash Recovery & Idempotency Boundary:**
  - Uji mati paksa relayer node di setiap titik transisi: sebelum broadcast, setelah broadcast, sebelum local output commit.
  - Uji penolakan mutlak input nullifier replay dan proof invalid.
- [ ] **Cross-Chain CCIP Testnet Flow:**
  - Eksekusi source broadcast $\rightarrow$ router CCIP $\rightarrow$ destination execution $\rightarrow$ destination confirmation.

---

## Fase 5: Mainnet Arbitrum One Launch & Go-To-Market Ecosystem
Ref: [`docs/bisnis.md`](file:///workspaces/Zeltra-Protocol/docs/bisnis.md)

- [ ] **Deploy Production Arbitrum One:**
  - Deployment Stylus contract resmi, registrasi address ke dokumentasi publik, dan peluncuran Bug Bounty.

### Batasan Keras & Invarian Arsitektur Produk (Non-Negotiable)
- [ ] **Pemisahan Domain Rel Pembayaran:** Zero credit, zero unsecured lending, zero algorithmic token swap, zero escrow tersentralisasi, dan zero fiat on/off-ramp pada rel transfer pembayaran instan (Mode 2/3). Seluruh fitur akumulasi yield RWA diisolasi penuh pada Product 8 Sovereign Gateway tanpa mencemari likuiditas kas operasional.
- [ ] **Circuit Rigidity:** Zero perubahan pada sirkuit Groth16, jumlah public inputs (16 PI & 19 PI), parameter Poseidon LFSR, maupun layout storage kontrak on-chain yang telah teruji.
- [ ] **Cryptographic Intent Boundary:** Tidak mengklaim invoice ID atau deskripsi terikat secara kriptografis ke proof ZK kecuali ada komitmen quote/public-input terverifikasi yang membuktikannya. Seluruh integrasi dimulai dengan payment intent off-chain tervalidasi dan batas kepercayaan (*trust boundary*) terdokumentasi eksplisit.

---

### Product 2 — Zeltra Checkout (Merchant & API Integration)
**Target:** Integrasi mulus bagi merchant online dan penyedia API tanpa memaksa merchant memahami internal ZK-UTXO Private Note.
- [ ] **Kontrak API & HTTP Versioned:** Definisikan kontrak API versioned untuk pembuatan payment intent (`create intent`), pengecekan status (`get status`), verifikasi pembayaran on-chain (`verify payment`), pengambilan tanda terima (`fetch receipt`), serta pengiriman webhook terenkripsi.
- [ ] **SDK Klien Bertipe Ketat:** Rancang interface SDK/client dengan penanganan error bertipe (`typed errors`); pisahkan decoding payload kriptografi protokol dari representasi objek domain merchant.
- [ ] **Paket Checkout Klien & Aplikasi Merchant:** Sediakan contoh aplikasi merchant serta paket JS/TS checkout mandiri atau hosted checkout prototype; tentukan struktur paket bersih terpisah dari SDK worker bridge.
- [ ] **Webhook Aman & Idempoten:** Tanda tangan webhook menyertakan timestamp dan proteksi replay; lengkapi dengan retry exponential backoff, deduplication ID, tracking status delivery, serta kebijakan batas waktu (*timeout policy*) terdokumentasi.
- [ ] **Konfirmasi Berbasis Finalitas Nyata:** Sistem tidak mengembalikan status sukses/lunas (`paid/confirmed`) sebelum tracker settlement mengonfirmasi transaksi sesuai kebijakan finalitas L2 yang disepakati.
- [ ] **Transparansi Struktur Biaya:** Tampilkan biaya transaksi secara gamblang kepada pembeli. Protokol fee 0 bps pada in-pool transfer (Mode 2) bukan berarti biaya total nol; biaya eksekusi relayer dan network gas outflow wajib ditampilkan transparan jika berlaku.
- [ ] **Kompatibilitas Envelope x402:** Sediakan modul interoperabilitas terhadap envelope HTTP 402 v2 yang sudah ada dengan tetap menjaga backward compatibility bagi klien aktif.

---

### Product 3 — Zeltra Business Workspace (Cash Operations & Merchant Ledger)
**Target:** Memberdayakan bisnis mengelola pembayaran masuk, memantau antrean settlement, dan melakukan rekonsiliasi kas tanpa membangun software akuntansi monolitik yang membengkak.
- [ ] **Manajemen Status Invoice:** Dashboard pelacakan invoice lengkap dengan filter status: *draft, awaiting payment, queued, submitted, confirmed, expired, failed, cancelled*.
- [ ] **Download Tanda Terima & Ekspor Rekonsiliasi:** Fitur unduh receipt dan ekspor laporan rekonsiliasi (CSV/JSON); integrasikan dengan modul kepatuhan [`zeltra-sdk/src/compliance/exporter.rs`](file:///workspaces/Zeltra-Protocol/zeltra-sdk/src/compliance/exporter.rs) setelah memvalidasi kecocokan skema dengan pembukuan merchant.
- [ ] **Label & Kategori Kas Lokal:** Label, kategori, dan catatan privat tersimpan di penyimpanan lokal merchant. Metadata pembukuan internal terpisah mutlak dari proof ZK dan public inputs protokol.
- [ ] **Proyeksi Buku Kas Minimalis:** Proyeksi ledger mencatat nilai invoice, jenis aset (USDC/WETH), payment intent ID, referensi settlement on-chain, rincian fee, dan timestamp. Entitas ledger wajib direkonsiliasi terhadap bukti transaksi riil, bukan diasumsikan sebagai bukti pergerakan dana independen.
- [ ] **Isolasi Multi-Tenant Bersih:** Penegakan pemisahan akun/tenant yang ketat jika beberapa merchant berbagi instance; portal multi-tenant dilarang langsung menumpang di database inti relayer tanpa arsitektur keamanan terpisah.
- [ ] **Privasi Data Pelanggan (Anti-PII):** Tidak menyimpan data pribadi pelanggan (PII) secara default. Jika data tertentu dibutuhkan oleh merchant, tetapkan kebijakan retensi, hak hapus/ekspor data, kontrol akses terenkripsi, dan manajemen kunci sebelum rilis.

---

### Product 4 — Zeltra Personal: Pockets & Financial Insights (Local-First)
**Target:** Memberikan pengalaman manajemen kas harian yang hidup dan interaktif bagi pengguna tanpa menambah kompleksitas atau risiko di lapisan smart contract on-chain.
- [ ] **Alokasi Kantong Kas Lokal (Pockets):** Pockets diimplementasikan murni sebagai label alokasi dan pembukuan lokal di perangkat pengguna (misal: belanja harian, langganan, tabungan, alokasi AI Agent), bukan brankas kontrak on-chain tambahan dan bukan pembatas saldo on-chain yang kaku.
- [ ] **Validasi Saldo Berbasis Sinkronisasi:** Tampilan saldo hanya dihitung dari Private Notes yang telah selesai dipindai dan didekripsi penuh oleh wallet; sertakan indikator status sinkronisasi dan timestamp pemindaian terakhir. Jangan pernah menginterpolasi saldo perkiraan jika pemindaian belum tuntas.
- [ ] **Analitik Finansial On-Device (Insights):** Komputasi kategori dan total pengeluaran diproses 100% lokal di perangkat pengguna; label merchant yang diinput pengguna tetap tersimpan privat di perangkat.
- [ ] **Ekspor Data Bersih & Teredaksi:** Fitur ekspor data bersifat opt-in dengan sensor otomatis (*mandatory redaction*) terhadap rahasia note, material seed, private keys, nullifier mentah yang tidak relevan, memo terenkripsi, dan metadata pengenal yang berlebihan.
- [ ] **Perlindungan Privasi AI (Zero-Leakage):** Menolak saran finansial otomatis atau kategorisasi AI cloud yang mengunggah riwayat transaksi lengkap pengguna ke server eksternal tanpa persetujuan eksplisit.
- [ ] **Zero Circuit Changes:** Tidak ada perubahan sirkuit ZK untuk kebutuhan pelabelan lokal atau pembukuan kantong kas.

---

### Product 5 — Autopay & Pembayaran Berulang (Scheduled Approvals & Autonomous Spend)
**Target:** Rel pembayaran berulang yang aman dengan otorisasi terikat, dimulai dari alur persetujuan berkala sebelum melangkah ke eksekusi otonom.
- [ ] **Skema Paket Langganan:** Definisikan parameter rencana langganan: identitas merchant, aset (USDC/WETH), batas nominal atau nilai tetap, frekuensi/periode, jatuh tempo berikutnya, masa kedaluwarsa, aturan pembatalan, dan status otorisasi saat ini.
- [ ] **Alur Persetujuan & Notifikasi Manual:** Terapkan alur pengingat berkala dengan persetujuan eksplisit dari pengguna untuk setiap eksekusi pengeluaran; tidak ada private key belanja yang disimpan di server relayer atau merchant.
- [ ] **Desain Otorisasi Pengeluaran Otonom Terdelegasi:** Untuk pengeluaran berulang tanpa intervensi manual, wajib menyusun Architecture Decision Record (ADR) tersendiri yang merumuskan penegakan kriptografis atas cakupan belanja, batas maksimum saldo, kedaluwarsa, proteksi replay, pembatalan (*revocation*), dan pengikatan ke alamat penerima (*recipient binding*).
- [ ] **Klarifikasi Batasan Proteksi:** Aturan di sisi aplikasi semata bukanlah batas pembelanjaan kriptografis. Aturan anggaran lokal yang dapat dilewati oleh client lain tidak boleh diklaim sebagai batas pengeluaran yang terenkripsi.
- [ ] **Ketahanan Uji Edge Cases:** Test suite mencakup kondisi balapan antara pembatalan vs pembayaran (*cancellation-vs-payment race*), duplikasi antrean jadwal, otorisasi kedaluwarsa, perbedaan zona waktu/kalender, relayer offline, saldo note tidak mencukupi, dan pemulihan status pasca-restart wallet.

---

### Product 6 — Zeltra Agent Treasury (Bounded AI Agent Payments)
**Target:** Memungkinkan autonomous AI Agent membelanjakan kas dalam batas anggaran yang disetujui untuk pemanggilan layanan API tanpa pernah memegang master spending key milik pemilik.
- [ ] **Hardening Protokol x402:** Lakukan penguatan menyeluruh pada pipeline x402 yang ada terlebih dahulu; hindari ekspansi skema baru selama semantik settlement dan receipt masih memiliki ambiguitas.
- [ ] **Model Kebijakan Agen Bertipe (Typed Policy Model):** Susun struktur data kebijakan: `agent_id`, referensi akun pemilik, versi kebijakan, cakupan merchant/resource yang diizinkan, jenis aset, nominal batas per transaksi, batas anggaran kumulatif, waktu kedaluwarsa, pembatas frekuensi (*rate limit*), dan status pencabutan (*revocation*).
- [ ] **Pemisahan Tegas Kebijakan Advisory vs Enforced:** Bedakan secara eksplisit antara kebijakan off-chain yang bersifat anjuran (*advisory*) dengan pembatasan yang terikat secara kriptografis (*cryptographically enforced*) pada kode program dan dokumentasi produk.
- [ ] **Isolasi Alokasi Sub-Wallet/Note:** Alokasikan dana operasional agen pada note terpisah atau sub-wallet terisolasi; jangan pernah memberikan akses master spending key kepada runtime agen.
- [ ] **Proses Formal Perubahan Sirkuit (Jika Diperlukan):** Jika pengikatan merchant atau pembatasan nominal keras menuntut penambahan constraint sirkuit/public-input baru, wajib melalui ADR terpisah dan analisis model ancaman formal; hanya diterapkan setelah review matang tanpa menyelundupkan semantik ke quote hash tanpa pembuktian matematis.
- [ ] **Proteksi Replay & Uji Pencabutan:** Lengkapi dengan proteksi replay request/response, kedaluwarsa quote, rate-limiting spesifik agen, bukti settlement receipt, uji pencabutan izin seketika (*revocation tests*), dan uji batas anggaran habis (*budget exhaustion*).
- [ ] **Transparansi Operasional Penuh:** Mulai dengan kontrol anggaran yang transparan, pelaporan penyebab kegagalan transaksi yang jelas, dan batas nilai yang ditentukan pengguna.

---

### Product 7 — Payroll, Membership & Team Payments (Ekspansi Lanjutan)
**Target:** Rel pembayaran gaji terdistribusi, keanggotaan privat, dan pembayaran tim multi-pihak setelah fondasi checkout dan rekonsiliasi kas stabil sempurna.
- [ ] **Kriteria Peluncuran Bertahap:** Implementasi baru dimulai setelah alur invoice/checkout digunakan secara konsisten dan proses penerbitan receipt serta rekonsiliasi kas terbukti stabil tanpa kegagalan.
- [ ] **Rel Penggajian Privat (Privacy-Aware Payroll):** Menuntut onboarding penerima berbasis privasi (ZSA Bech32m), mekanisme pemulihan jika pembayaran tertunda, tinjauan batch payout, serta kepatuhan hukum dan regulasi pajak pada yurisdiksi peluncuran.
- [ ] **Keanggotaan & Perks Eksklusif (Membership Credentials):** Menggunakan verifikasi identitas merchant dan bukti kepemilikan privat atau tanda terima ZK yang membuktikan hak akses tanpa membuka riwayat belanja lain milik pengguna.
- [ ] **Pembagian Tagihan Tim (Group Split):** Mendukung batas waktu kedaluwarsa undangan/klaim, kekebalan terhadap penyelesaian ganda (*duplicate settlement resistance*), penanganan pembayaran sebagian (*partial payments*), dan kejelasan alokasi dana yang tidak diklaim.

---

### Product 8 — Sovereign Shielded RWA Gateway (Confidential Treasury Yield)
**Target:** Memberikan pintu gerbang opt-in institusional bagi korporat B2B, perbendaharaan DAO, dan whale untuk mengalokasikan idle treasury ke instrumen Real-World Asset (RWA) berbunga (seperti US Treasury yield via Ondo USDY atau BlackRock BUIDL) tanpa mengekspos posisi neraca kas kepada kompetitor on-chain.
- [ ] **Desain Pembungkus Note Berdaulat (Sovereign Note Wrapper):** Unit aset berbunga dibungkus ke dalam komitmen Private Note ZK (`asset_id` terpisah di luar USDC & WETH standar) sehingga kepemilikan saham/token RWA bersifat fully confidential.
- [ ] **Solvensi Terisolasi Mutlak (Siloed Yield Accounting):** Hasil yield dari instrumen RWA dihitung secara deterministik dan terisolasi dari pool kas likuid USDC/WETH; tidak ada percampuran liabilitas (*anti-rehypothecation invariant*).
- [ ] **Selective Compliance & Audit Trail (DEC-040 Integration):** Integrasi langsung dengan modul CEX/B2B Selective Disclosure untuk mengekspor bukti kepemilikan legal RWA ke auditor pajak atau kustodian institusi tanpa membuka private spending key ($sk$).
- [ ] **Penebusan Kas Instan & Partial Unwrapping:** Mekanisme penukaran kembali (*redemption*) dari note RWA ke USDC pool kas operasional dengan verifikasi proof kepemilikan note yang sah, memungkinkan perpindahan modal yang mulus antara kantong yield dan rel checkout merchant.


