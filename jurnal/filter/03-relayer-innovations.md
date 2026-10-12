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

## 2. WabiSabi: Variable-Amount CoinJoin Decomposition (IACR ePrint 2021/206)
* **File Jurnal:** [`jurnal/WabiSabi-Centrally-Coordinated-CoinJoins.md`](file:///workspaces/Zeltra-Protocol/jurnal/WabiSabi-Centrally-Coordinated-CoinJoins.md)
* **Penulis:** Ádám Ficsór, Yuval Kogman, Lucas Ontivero, István András Seres
* **Problem di Sistem Klasik:**
  Mengkombinasikan banyak note dari pengguna yang berbeda dalam satu transaksi batch on-chain sering kali bocor jika nominal input dan output dapat dicocokkan (*amount correlation analysis*).
* **Inovasi Paper:**
  Algoritma dekomposisi nilai variabel untuk memecah note input dan output sehingga memutus korelasi nilai transaksi.
* **Implementasi di Zeltra Node / SDK:**
  * Digunakan sebagai acuan dekomposisi pecahan Change Note di SDK & pembentukan batch settlement di relayer agar nominal change note homogen dan tidak dapat di-link balik ke transaksi input.

---

## 3. Otter: A Provably MEV-Resilient Automated Market Maker via Surplus Redistribution (IACR ePrint 2026/1877)
* **File Jurnal / PDF:** [`jurnal/pdf/2026-1877.pdf`](file:///workspaces/Zeltra-Protocol/jurnal/pdf/2026-1877.pdf)
* **Penulis:** Elaine Shi, Mengqian Zhang, Hao Chung, Yuhao Li (September 2026)
* **Problem di Sistem Klasik:**
  Dalam sistem bundling dan batching transaksi (misal Flashbots bundle reordering, MEV sandwiching), relayer atau bot pencari MEV sering kali mengekstraksi keuntungan kotor dari selisih penghematan gas skala ekonomis (*batch amortization surplus*) tanpa mengembalikannya kepada pengguna akhir, atau memanipulasi urutan transaksi.
* **Inovasi Paper:**
  Merumuskan model desain AMM dan mekanisme lelang/bundling yang tahan MEV secara terbukti (*provably MEV-resilient*) melalui *Surplus Redistribution*. Surplus efisiensi eksekusi dari batching secara otomatis diredistribusikan secara proporsional kepada para partisipan transaksi, memitigasi ekstraksi nilai gelap oleh intermediary.
* **Implementasi di Zeltra Node:**
  * Komponen: Adaptive batch worker di [`nimbus-node/src/main.rs`](file:///workspaces/Zeltra-Protocol/nimbus-node/src/main.rs).
  * Memastikan penghematan gas dari batch settlement (2-8 spend) secara adil menekan beban biaya eksekusi aktual bagi pengguna, membatasi keuntungan relayer hanya pada markup transparan 15% tanpa ekstraksi MEV tersembunyi.


