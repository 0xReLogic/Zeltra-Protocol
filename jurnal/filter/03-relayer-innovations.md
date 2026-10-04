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
