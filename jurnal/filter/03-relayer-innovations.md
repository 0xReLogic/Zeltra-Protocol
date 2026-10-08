# Inovasi Jurnal untuk Relayer & Settlement Node (`nimbus-node`)

Dokumen ini memetakan paper terpilih untuk lapisan relayer dan cluster konsensus Nimbus ([`nimbus-node`](file:///workspaces/Zeltra-Protocol/nimbus-node/)), mencakup manajemen antrian persisten SQLite, batching transaksi, koordinasi threshold guardian, dan mitigasi front-running/MEV.

---

## 1. AuditPay: Anonymous Payments with Controlled Oversight (IACR ePrint 2026/05)
* **File Jurnal:** [`jurnal/AuditPay-Anonymous-Payments-Controlled-Oversight.md`](file:///workspaces/Zeltra-Protocol/jurnal/AuditPay-Anonymous-Payments-Controlled-Oversight.md)
* **Penulis:** Elkana Tovey, Yossi Gilad, Aviv Zohar
* **Problem di Sistem Klasik:**
  Mixer dan relayer tanpa kontrol rentan terhadap sanksi hukum internasional (OFAC), sedangkan sistem dengan backdoor viewing key rentan terhadap penyalahgunaan pengintaian massal (*mass surveillance*).
* **Inovasi Paper:**
  *Auditing Budget*: Relayer mengizinkan entitas pengawas (auditor berkekuatan hukum) untuk memverifikasi maksimum $K$ transaksi per epoch secara anonim. Kontrak cerdas dan ZK circuit membatasi kuota inspeksi secara matematis. Permintaan di luar kuota otomatis *fail-closed*.
* **Implementasi di Nimbus Node:**
  * Modul endpoint baru: `/api/compliance/audit-claim` di [`nimbus-node/src/api/`](file:///workspaces/Zeltra-Protocol/nimbus-node/src/api/).
  * Memungkinkan Zeltra Protocol beroperasi legal di Arbitrum tanpa pernah bisa dipaksa membuka semua transaksi pengguna sekaligus.

---

## 2. RSS: Robust Signing Service for Threshold Networks (IACR ePrint 2026/1670)
* **File Jurnal:** [`jurnal/RSS-Robust-Signing-Service.md`](file:///workspaces/Zeltra-Protocol/jurnal/RSS-Robust-Signing-Service.md)
* **Problem di Sistem Klasik:**
  Pada cluster 1 Leader + 4 Guardian (3-of-5 threshold), jika 2 guardian mengalami network partition, latency lonjak drastis atau ronde penandatanganan macet (*hanging HTTP leases*).
* **Inovasi Paper:**
  Protokol penjadwalan *Optimistic Fast-Path with Hedged Requests*: Leader mengirim request ke $t+1$ guardian tercepat secara simultan dengan timeout adaptif, dan secara otomatis memotong guardian lambat tanpa perlu membatalkan seluruh sesi penandatanganan.
* **Implementasi di Nimbus Node:**
  * Komponen: [`nimbus-node/src/cluster/leader.rs`](file:///workspaces/Zeltra-Protocol/nimbus-node/src/cluster/leader.rs).
  * Meningkatkan throughput blind signing issuance dari 12 TPS menjadi >85 TPS dalam cluster terdistribusi lintas benua.

---

## 3. WabiSabi: Variable-Amount CoinJoin Decomposition (IACR ePrint 2021/206)
* **File Jurnal:** [`jurnal/WabiSabi-Centrally-Coordinated-CoinJoins.md`](file:///workspaces/Zeltra-Protocol/jurnal/WabiSabi-Centrally-Coordinated-CoinJoins.md)
* **Penulis:** Ádám Ficsór, Yuval Kogman, Lucas Ontivero, István András Seres
* **Problem di Sistem Klasik:**
  Mengkombinasikan banyak note dari pengguna yang berbeda dalam satu transaksi batch on-chain sering kali bocor jika nominal input dan output dapat dicocokkan (*amount correlation analysis*).
* **Inovasi Paper:**
  Algoritma dekomposisi nilai variabel untuk memecah note input dan output sehingga memutus korelasi nilai transaksi.
* **Implementasi di Nimbus Node / SDK:**
  * Digunakan sebagai acuan dekomposisi pecahan Change Note di SDK & pembentukan batch settlement di relayer agar nominal change note homogen dan tidak dapat di-link balik ke transaksi input.

---

## 4. VITARIT: Paying for Threshold Services on Bitcoin and Friends (IACR ePrint 2025/174)
* **File Jurnal:** [`jurnal/VITARIT-Paying-Threshold-Services.md`](file:///workspaces/Zeltra-Protocol/jurnal/VITARIT-Paying-Threshold-Services.md)
* **Penulis:** Sri AravindaKrishnan Thyagarajan, Easwar Vivek Mangipudi, Lucjan Hanzlik, Aniket Kate, Pratyay Mukherjee
* **Problem di Sistem Klasik:**
  Blueprint Nimbus sekarang mengasumsikan relayer membayar guardian fee secara terpusat — relayer tahu volume issuance, biaya per guardian, dan pola traffic. Ini menciptakan surveillance point: relayer dapat menyimpulkan kapan traffic tinggi (banyak deposit) dan mengkorelasikannya dengan on-chain event untuk de-anonymisasi timing.
* **Inovasi Paper:**
  VITARIT memperkenalkan primitive *fair payment untuk threshold service* — user membayar guardian cluster secara langsung dan anonim via atomic payment channel. Pembayaran hanya released jika dan hanya jika threshold quorum valid tercapai (fairness guarantee): user tidak bisa underpay, guardian tidak bisa kabur dengan payment tanpa memberikan partial signature.
* **Implementasi di Nimbus Node:**
  * Alur baru opsional di Fase 3: User menyertakan *service payment note* kecil (misal 0.01 USDC) bersamaan dengan issuance request. Guardian atomically receive payment saat mereka submit partial signature valid.
  * Komponen: [`nimbus-node/src/cluster/guardian.rs`](file:///workspaces/Zeltra-Protocol/nimbus-node/src/cluster/guardian.rs) — payment channel state machine per guardian.
  * Dampak: Relayer tidak lagi menjadi single point of payment knowledge. Volume transaksi guardian tersebar dan tidak bisa dikorelasikan dengan deposit flow oleh pihak manapun.

---

## 5. Otter: A Provably MEV-Resilient Automated Market Maker via Surplus Redistribution (IACR ePrint 2026/1877)
* **File Jurnal / PDF:** [`jurnal/pdf/2026-1877.pdf`](file:///workspaces/Zeltra-Protocol/jurnal/pdf/2026-1877.pdf)
* **Penulis:** Elaine Shi, Mengqian Zhang, Hao Chung, Yuhao Li (September 2026)
* **Problem di Sistem Klasik:**
  Dalam sistem bundling dan batching transaksi (misal Flashbots bundle reordering, MEV sandwiching), relayer atau bot pencari MEV sering kali mengekstraksi keuntungan kotor dari selisih penghematan gas skala ekonomis (*batch amortization surplus*) tanpa mengembalikannya kepada pengguna akhir, atau memanipulasi urutan transaksi.
* **Inovasi Paper:**
  Merumuskan model desain AMM dan mekanisme lelang/bundling yang tahan MEV secara terbukti (*provably MEV-resilient*) melalui *Surplus Redistribution*. Surplus efisiensi eksekusi dari batching secara otomatis diredistribusikan secara proporsional kepada para partisipan transaksi, memitigasi ekstraksi nilai gelap oleh intermediary.
* **Implementasi di Nimbus Node:**
  * Komponen: [`nimbus-node/src/settlement/batcher.rs`](file:///workspaces/Zeltra-Protocol/nimbus-node/src/settlement/batcher.rs) dan antrian `batch_spend()`.
  * Memastikan penghematan gas dari batch settlement (2-8 spend) secara adil menekan beban biaya eksekusi aktual bagi pengguna, membatasi keuntungan relayer hanya pada markup transparan 15% tanpa ekstraksi MEV tersembunyi.

---

## 6. Budgeted Threshold Signatures: Velocity Limiting Layer (IACR ePrint 2026/2176)
* **File Jurnal:** [`jurnal/Budgeted-Threshold-Signatures.md`](file:///workspaces/Zeltra-Protocol/jurnal/Budgeted-Threshold-Signatures.md)
* **Problem di Sistem Klasik:**
  Guardian cluster di Nimbus bertindak sebagai penandatangan buta (*stateless blind signer* 3-of-5) yang tidak boleh melihat saldo atau nominal transaksi. Namun sistem tetap membutuhkan pembatasan laju (*rate/velocity limiting*) untuk mencegah spamming request tanda tangan atau pengurasan modal secara mendadak. Menerapkan batas nominal akan merusak *blindness* kriptografis.
* **Inovasi Paper:**
  Budgeted Threshold Signatures (BTS): Memungkinkan kuorum threshold menandatangani pesan hanya jika pemohon belum melampaui kuota tertentu (misal: "Epoch 24H, Tiket ke-N") menggunakan bukti tag kuota terpisah di finite field. Guardian memverifikasi kuota frekuensi secara deterministik tanpa pernah mengetahui identitas pemohon atau jumlah dana yang ditransaksikan.
* **Implementasi di Nimbus Node / Guardian:**
  * Komponen: Modul penjadwalan kuota di [`nimbus-node/src/cluster/guardian.rs`](file:///workspaces/Zeltra-Protocol/nimbus-node/src/cluster/guardian.rs) dan agregator tanda tangan di [`nimbus-node/src/cluster/leader.rs`](file:///workspaces/Zeltra-Protocol/nimbus-node/src/cluster/leader.rs).
  * Menjamin kuorum Guardian dapat membatasi kecepatan (*velocity rate limit*) serangan flooding tanda tangan tanpa pernah mengorbankan kebutaan (blindness) data pengguna.

---

## 7. High-Throughput Verifiable Distributed OPRF: Quorum Leaderless (n, t) = (9, 4) (IACR ePrint 2026/1953)
* **File Jurnal:** [`jurnal/reference/High-Throughput-Verifiable-Distributed-OPRF.md`](file:///workspaces/Zeltra-Protocol/jurnal/reference/High-Throughput-Verifiable-Distributed-OPRF.md)
* **Penulis:** IACR ePrint 2026/1953 (September 2026)
* **Problem di Sistem Klasik:**
  Transisi dari 3-of-5 (Leader-centric) ke 5-of-9 (Leaderless) di Blueprint Fase 2 membutuhkan protokol threshold OPRF / blind signing terdistribusi yang efisien. Pada $(n, t) = (9, 4)$, skema threshold OPRF konvensional mengalami kemacetan bandwidth dan memori yang sangat tinggi jika ada guardian nakal (*malicious servers*).
* **Inovasi Paper:**
  Protokol Verifiable Distributed OPRF (dOPRF) pertama dengan throughput tinggi yang tahan terhadap $t < n/2$ server jahat dan klien jahat. Menggunakan paradigma offline-online dengan verifikasi DZKP konstan-ronde, terbukti praktis dan terukur hingga skala kuorum $(n, t) = (9, 4)$ tanpa overhead bandwidth yang membengkak.
* **Implementasi di Nimbus Node:**
  * Komponen: Arsitektur cluster tanpa leader di [`nimbus-node/src/cluster/`](file:///workspaces/Zeltra-Protocol/nimbus-node/src/cluster/).
  * Memungkinkan ekspansi Guardian Cluster dari 3-of-5 menjadi 5-of-9 independen di Fase 2 secara aman, memutus ketergantungan pada single leader, dan menjamin operasional asinkron yang tahan terhadap 4 guardian yang offline atau berkolusi.


