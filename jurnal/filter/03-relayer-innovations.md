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

## 3. Velox: Fair Asynchronous MPC & Transaction Ordering (IACR ePrint 2025/1630)
* **File Jurnal:** [`jurnal/Velox-Fair-Asynchronous-MPC.md`](file:///workspaces/Zeltra-Protocol/jurnal/Velox-Fair-Asynchronous-MPC.md)
* **Problem di Sistem Klasik:**
  Relayer yang mengumpulkan spend request di mempool rentan dicurigai melakukan MEV front-running atau memprioritaskan transaksi tertentu secara curang.
* **Inovasi Paper:**
  Algoritma *Blind Batch Ordering*: Urutan transaksi di dalam batch diacak menggunakan seed terkomitmen (*threshold VRF / blind shuffle*) sehingga relayer atau miner tidak dapat mengeksploitasi urutan eksekusi note.
* **Implementasi di Nimbus Node:**
  * Komponen: [`nimbus-node/src/settlement/batcher.rs`](file:///workspaces/Zeltra-Protocol/nimbus-node/src/settlement/batcher.rs).
  * Menjamin urutan eksekusi `batch_spend()` di Arbitrum transparan dan tahan terhadap tuduhan front-running oleh user atau auditor independen.

---

## 4. WabiSabi: Centrally Coordinated Anonymous CoinJoins (IACR ePrint 2021/206)
* **File Jurnal:** [`jurnal/WabiSabi-Centrally-Coordinated-CoinJoins.md`](file:///workspaces/Zeltra-Protocol/jurnal/WabiSabi-Centrally-Coordinated-CoinJoins.md)
* **Problem di Sistem Klasik:**
  Mengkombinasikan banyak note dari pengguna yang berbeda dalam satu transaksi batch on-chain sering kali bocor jika nominal input dan output dapat dicocokkan (*amount correlation analysis*).
* **Inovasi Paper:**
  Algoritma *Variable-Amount Balanced Decomposition*: Memecah note multi-input dan multi-output secara homogen sehingga memutus korelasi nilai transaksi.
* **Implementasi di Nimbus Node:**
  * Digunakan saat relayer menyusun multi-item batch settlement, memastikan fragmentasi nilai Change Note tidak dapat di-link balik ke transaksi input.

---

## 5. 3PaaS: Privacy-Preserving Post-Compromise Security as a Service (IACR ePrint 2026/966)
* **File Jurnal:** [`jurnal/3PaaS-Privacy-Preserving-Post-Compromise-Security.md`](file:///workspaces/Zeltra-Protocol/jurnal/3PaaS-Privacy-Preserving-Post-Compromise-Security.md)
* **Penulis:** Cas Cremers, Abhinav Nakarmi, Aleksi Peltonen, Eyal Ronen
* **Problem di Sistem Klasik:**
  Blueprint Nimbus menangani *static compromise* — jika guardian ketahuan berperilaku jahat, ia di-slash dan dikeluarkan via DKG resharing. Namun blueprint diam tentang *retroactive compromise*: bagaimana jika guardian yang sudah dikeluarkan menyimpan copy partial blind signature lama dan mencoba mengkorelasikan sesi historis? Setelah guardian kick-out, semua credential yang pernah ia bantu terbitkan tetap terancam secara retroaktif.
* **Inovasi Paper:**
  3PaaS (Post-Compromise Security as a Service) mengadaptasi mekanisme *double ratchet* dari end-to-end messaging (Signal Protocol) ke domain threshold signing service. Setiap ronde issuance baru menggunakan *forward-secret ratchet key* sehingga material kriptografi dari ronde sebelumnya tidak dapat digunakan ulang. Bahkan jika guardian lama menyimpan semua partial signature historis, mereka tidak bisa mengkorelasikannya dengan session baru.
* **Implementasi di Nimbus Node:**
  * Komponen: [`nimbus-node/src/cluster/dkg_manager.rs`](file:///workspaces/Zeltra-Protocol/nimbus-node/src/cluster/dkg_manager.rs) — tambahkan *epoch ratchet* setiap N sesi issuance atau setiap guardian rotation.
  * Setiap epoch, guardian cluster melakukan mini-DKG untuk me-refresh share tanpa mengubah public key issuer (`pk_iss`) yang terdaftar di kontrak. Credential lama tetap valid di-chain (verifikasi pairing tidak berubah), tapi material internal guardian yang lama menjadi buta terhadap issuance baru.
  * Efek: Guardian yang dikompromisi dan dikeluarkan kehilangan kemampuan forensik terhadap sesi masa depan secara kriptografis, bukan hanya secara operasional.

---

## 6. VITARIT: Paying for Threshold Services on Bitcoin and Friends (IACR ePrint 2025/174)
* **File Jurnal:** [`jurnal/VITARIT-Paying-Threshold-Services.md`](file:///workspaces/Zeltra-Protocol/jurnal/VITARIT-Paying-Threshold-Services.md)
* **Penulis:** Sri AravindaKrishnan Thyagarajan, Easwar Vivek Mangipudi, Lucjan Hanzlik, Aniket Kate, Pratyay Mukherjee
* **Problem di Sistem Klasik:**
  Blueprint Nimbus sekarang mengasumsikan relayer membayar guardian fee secara terpusat — relayer tahu volume issuance, biaya per guardian, dan pola traffic. Ini menciptakan surveillance point: relayer dapat menyimpulkan kapan traffic tinggi (banyak deposit) dan mengkorelasikannya dengan on-chain event untuk de-anonymisasi timing.
* **Inovasi Paper:**
  VITARIT memperkenalkan primitive *fair payment untuk threshold service* — user membayar guardian cluster secara langsung dan anonim via atomic payment channel. Pembayaran hanya released jika dan hanya jika threshold quorum valid tercapai (fairness guarantee): user tidak bisa underpay, guardian tidak bisa kabur dengan payment tanpa memberikan partial signature.
* **Implementasi di Nimbus Node:**
  * Alur baru opsional di Fase 3: User menyertakan *service payment note* kecil (misal 0.01 USDC) bersamaan dengan issuance request. Guardian atomically receive payment saat mereka submit partial signature valid.
  * Komponen: [`nimbus-node/src/cluster/guardian.rs`](file:///workspaces/Zeltra-Protocol/nimbus-node/src/cluster/guardian.rs) — tambahkan payment channel state machine per guardian.
  * Dampak: Relayer tidak lagi menjadi single point of payment knowledge. Volume transaksi guardian tersebar dan tidak bisa dikorelasikan dengan deposit flow oleh pihak manapun.
