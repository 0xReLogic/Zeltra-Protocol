# Inovasi Jurnal untuk Relayer & Settlement Node (`zeltra-node`)

Dokumen ini memetakan paper terpilih untuk lapisan relayer dan cluster konsensus Zeltra ([`zeltra-node`](file:///workspaces/Zeltra-Protocol/zeltra-node/)), mencakup manajemen antrian persisten SQLite, batching transaksi, koordinasi threshold guardian, dan mitigasi front-running/MEV.

---

## 1. AuditPay: Anonymous Payments with Controlled Oversight (IACR ePrint 2026/05)
* **File Jurnal:** [`jurnal/AuditPay-Anonymous-Payments-Controlled-Oversight.md`](file:///workspaces/Zeltra-Protocol/jurnal/AuditPay-Anonymous-Payments-Controlled-Oversight.md)
* **Penulis:** Elkana Tovey, Yossi Gilad, Aviv Zohar
* **Problem di Sistem Klasik:**
  Mixer dan relayer tanpa kontrol rentan terhadap sanksi hukum internasional (OFAC), sedangkan sistem dengan backdoor viewing key rentan terhadap penyalahgunaan pengintaian massal (*mass surveillance*).
* **Inovasi Paper:**
  *Auditing Budget*: Relayer mengizinkan entitas pengawas (auditor berkekuatan hukum) untuk memverifikasi maksimum $K$ transaksi per epoch secara anonim. Kontrak cerdas dan ZK circuit membatasi kuota inspeksi secara matematis. Permintaan di luar kuota otomatis *fail-closed*.
* **Implementasi di Zeltra Node:**
  * Modul endpoint baru: `/api/compliance/audit-claim` di [`zeltra-node/src/api/`](file:///workspaces/Zeltra-Protocol/zeltra-node/src/api/).
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
  * Komponen: Adaptive batch worker di [`zeltra-node/src/main.rs`](file:///workspaces/Zeltra-Protocol/zeltra-node/src/main.rs).
  * Memastikan penghematan gas dari batch settlement (2-8 spend) secara adil menekan beban biaya eksekusi aktual bagi pengguna, membatasi keuntungan relayer hanya pada markup transparan 15% tanpa ekstraksi MEV tersembunyi.

---

## 4. ESCAPE: Efficient Single-Server Online-Offline PIR without Periodic Preprocessing (IACR ePrint 2026/2033)
* **File Jurnal / ePrint:** IACR ePrint 2026/2033 (Oktober 2026)
* **Penulis:** Hoang-Dung Nguyen, Jorge Guajardo, Thang Hoang
* **Problem di Sistem Klasik:**
  Relayer `zeltra-node` yang melayani ribuan pengguna menghadapi dilema sinkronisasi state: jika klien menanyakan status leaf/note secara transparan, relayer dapat memprofilkan aset dan alamat user (merusak privasi). Namun jika menggunakan Private Information Retrieval (PIR) konvensional, server relayer harus melakukan komputasi linear yang sangat berat atau merekonstruksi hint database (periodic preprocessing) setiap kali ada penambahan daun baru pada akumulator.
* **Inovasi Paper:**
  Merumuskan *ESCAPE*, protokol Online-Offline PIR (OO-PIR) single-server pertama yang **meniadakan kebutuhan periodic preprocessing**. Klien melakukan prekomputasi tabel hint offline sekali saja yang sepenuhnya independen dari kueri, memungkinkan kueri online sublinear dengan throughput server yang sangat tinggi tanpa perlu server membangun ulang indeks database secara periodik.
* **Implementasi di Zeltra Node:**
  * Komponen: Endpoint sinkronisasi baru `/api/v1/mmr/oblivious-query` di [`zeltra-node/src/handlers/mmr.rs`](file:///workspaces/Zeltra-Protocol/zeltra-node/src/handlers/mmr.rs).
  * Memungkinkan ribuan client SDK melakukan kueri verifikasi unspent status note secara simultan dan oblivious tanpa membebani CPU relayer dengan pekerjaan periodic rebuild database.

---

## 5. Optimal Statistical Oblivious Search Trees for State Indexing (IACR ePrint 2026/2419)
* **File Jurnal / ePrint:** IACR ePrint 2026/2419 (Oktober 2026)
* **Penulis:** Andrew Krapivin and Elaine Shi
* **Problem di Sistem Klasik:**
  Pada server relayer/indexer yang mengelola jutaan komitmen note terenkripsi, pola kueri akses memori dan disk (*access pattern*) rentan dibocorkan ke penyerang yang mengamati bus data. Struktur data oblivious klasik (ORAM/Oblivious Search Trees) selama puluhan tahun terkendala oleh penalti komputasi log-kuadrat $O(\log^2 N)$, yang memperlambat throughput konkurensi node secara signifikan.
* **Inovasi Paper:**
  Elaine Shi dan Andrew Krapivin berhasil memecahkan batas log-kuadrat dengan merumuskan *Oblivious Search Tree (OST)* pertama yang mencapai batas optimal $O(\log N)$ dengan jaminan keamanan statistik (*statistical security*). Skema ini menyamakan efisiensi pencarian oblivious dengan search tree non-oblivious konvensional (*obliviousness for free*).
* **Implementasi di Zeltra Node:**
  * Komponen: Database engine di [`zeltra-node/src/database.rs`](file:///workspaces/Zeltra-Protocol/zeltra-node/src/database.rs) (tabel indexer MMR dan nullifier store) serta 2PC note store di [`zeltra-sdk/src/storage.rs`](file:///workspaces/Zeltra-Protocol/zeltra-sdk/src/storage.rs).
  * Menghilangkan kebocoran pola akses disk/RAM saat relayer melayani ribuan kueri sinkronisasi unspent note secara simultan, memberikan perlindungan privasi tingkat perangkat keras (anti-side channel storage inspection) pada node sequencer.

---

## 6. SkrrtPIR: Doubly-Stateless Batch PIR from Single-Key RLWE Unpacking (IACR ePrint 2026/2385)
* **File Jurnal / ePrint:** IACR ePrint 2026/2385 (Oktober 2026)
* **Penulis:** Keewoo Lee and Yongha Son
* **Problem Kritis:**
  Saat client SDK melakukan sinkronisasi batch unspent note atau mengunduh sekelompok daun MMR ([`recover_notes_batch`](file:///workspaces/Zeltra-Protocol/zeltra-sdk/src/wallet/note_wallet.rs)), skema Batch PIR konvensional menuntut server relayer menyimpan state per-klien (misal evaluation keys khusus per session). Hal ini menimbulkan dua kelemahan serius:
  1. Relayer dapat menghubungkan (*linkability*) kueri-kueri batch dari klien yang sama lintas sesi waktu, melemahkan anonimitas jaringan.
  2. Beban memori RAM relayer membengkak drastis saat melayani jutaan pengguna aktif konkuren.
* **Inovasi Paper:**
  Merumuskan *Doubly-Stateless Batch PIR*:
  - Server relayer **sama sekali tidak menyimpan state klien** (zero per-client server state).
  - Klien **tidak perlu menyimpan hint database** yang membebani memori lokal (zero database-dependent client hint).
  - Protokol menggunakan teknik *single-key RLWE unpacking* via linear walk pada subgrup siklik Galois (menggantikan traversal pohon RLWE multi-kunci konvensional), memangkas overhead komunikasi dan komputasi batch query secara dramatis hanya dengan sebuah kunci Galois tunggal.
* **Implementasi di Zeltra Node & SDK:**
  * Komponen: Endpoint batch MMR di [`zeltra-node/src/handlers/mmr.rs`](file:///workspaces/Zeltra-Protocol/zeltra-node/src/handlers/mmr.rs) (`/api/v1/mmr/batch-oblivious-leaves`) dan client scanner di [`zeltra-sdk/src/client.rs`](file:///workspaces/Zeltra-Protocol/zeltra-sdk/src/client.rs).
  * Menghadirkan mekanisme sinkronisasi batch note dan MMR tree yang 100% *stateless* dan *unlinkable*, mencegah relayer memprofilkan kluster UTXO milik pengguna saat melakukan trial decryption sinkronisasi massal.

---

## 7. Issuer-Hiding Anonymous Tokens for Zero-Knowledge Rate-Limiting (IACR ePrint 2026/870)
* **File Jurnal / ePrint:** IACR ePrint 2026/870 (Oktober 2026)
* **Penulis:** Andrea Flamini, Karla Friedrichs, Jonathan Katz, Watson Ladd, Anja Lehmann, Marek Sefranek
* **Problem Kritis:**
  Relayer ingress Zeltra (`/api/v1/spend-joinsplit`, `/api/quote/private-spend`) membutuhkan perlindungan terhadap serangan DDoS dan spam banjiran transaksi. Mekanisme rate limiting standar (seperti membatasi IP address pengguna atau cookie) merusak privasi pengguna secara fatal karena relayer dapat mencatat IP address pengirim note. Sebaliknya, jika rate limiter dinonaktifkan, relayer sequencer rentan mengalami crash akibat eksploitasi antrian mempool.
* **Inovasi Paper:**
  Merumuskan *Issuer-Hiding Anonymous Tokens (ATs)* berbasis *key-randomizable signatures* (menggunakan Tessaro–Zhu blind-signature / BBS). Pengguna dapat membuktikan hak akses kuota pengiriman transaksi (*one-time authorization token*) tanpa membocorkan identitas, IP address, atau issuer yang mengotorisasi token.
* **Implementasi di Zeltra Node:**
  * Komponen: Rate limiting middleware di [`zeltra-node/src/main.rs`](file:///workspaces/Zeltra-Protocol/zeltra-node/src/main.rs) dan generator tiket privat di [`zeltra-sdk/src/client.rs`](file:///workspaces/Zeltra-Protocol/zeltra-sdk/src/client.rs).
  * Menghadirkan perlindungan DoS tingkat industri bagi node relayer dengan memverifikasi token anonim satu-kali-pakai (*spend ticket*) pada setiap panggilan API spend, menjamin relayer bebas spam tanpa pernah menyimpan IP address atau jejak metadata pengguna sedikit pun.

---

## 8. Game-Theoretically Fair Distributed Randomness & Leader Election (IACR ePrint 2025/2190)
> [!NOTE]
> **DESENTRALISASI MASA DEPAN (ROADMAP):**
> Paper ini ditandai khusus sebagai fondasi arsitektur tahap transisi dari *Single Stateless Sequencer Relayer* menuju **Decentralized Multi-Relayer Network**.

* **File Jurnal / ePrint:** IACR ePrint 2025/2190 (Oktober 2026)
* **Penulis:** Pedro Branco, Pratik Soni, Sri AravindaKrishnan Thyagarajan, Ke Wu
* **Problem di Sistem Desentralisasi:**
  Saat cluster relayer Zeltra didesentralisasi ke banyak operator independen di masa depan, dibutuhkan mekanisme pemilihan sequencer pemimpin (*leader election*) dan pengurutan transaksi (*fair transaction ordering*) yang adil tanpa memicu perang MEV atau manipulasi urutan oleh relayer nakal. Protokol coin-tossing klasik sering kali rentan terhadap manipulasi jika preferensi node diketahui atau jika ada koalisi mayoritas curang (*rational dishonest majority*).
* **Inovasi Paper:**
  Merumuskan protokol *Game-Theoretically Fair Coin-Tossing With Private Preferences* pertama yang membuktikan keadilan pemilihan terdistribusi bahkan di bawah asumsi *dishonest majority*. Protokol menggabungkan enkripsi preferensi terotentikasi, layer komunikasi anonim, dan mekanisme penalti deviasi (*deviation penalty*): jika node relayer mencoba membiaskan keacakan pemilihan leader, utilitas ekonomi koalisi penyerang terbukti secara matematis turun drastis.
* **Implementasi di Roadmap Zeltra Node:**
  * Komponen Roadmap: Konsensus multi-sequencer terdesentralisasi di [`zeltra-node/`](file:///workspaces/Zeltra-Protocol/zeltra-node/).
  * Memberikan landasan algoritmik yang kokoh saat Zeltra bertransisi ke jaringan multi-relayer: pemilihan relayer pengusul blok bundle berikutnya berjalan sepenuhnya adil, tahan bias, dan mencegah monopoli MEV oleh operator node tertentu.---

## 9. Bandwidth-Balanced Reliable Broadcast for Decentralized Relayer Networks (IACR ePrint 2026/2337)
> [!NOTE]
> **DESENTRALISASI MASA DEPAN (ROADMAP):**
> Paper ini ditandai khusus sebagai fondasi protokol komunikasi P2P untuk transisi arsitektur menuju **Decentralized Multi-Relayer Network**.

* **File Jurnal / ePrint:** IACR ePrint 2026/2337 (Oktober 2026)
* **Penulis:** Victor Shoup
* **Problem di Sistem Desentralisasi:**
  Dalam jaringan relayer multi-node terdesentralisasi berbasis proof-of-stake atau bobot reputasi (*stake-weighted relayer cluster*), node-node relayer harus saling menyiarkan (*reliable broadcast*) bundle transaksi ZK-UTXO, status nullifier, dan preflight quote agar seluruh komite relayer memiliki konsistensi state. Pada protokol *erasure-coded reliable broadcast* standar (seperti Cachin-Tessaro), alokasi fragmen yang proporsional dengan stake menyebabkan relayer berbobot besar (*heavy relayer*) menanggung beban bandwidth upload (*relay-upload cost*) yang sangat timpang dan membengkak drastis dibanding relayer kecil, memicu sentralisasi infrastruktur bandwidth.
* **Inovasi Paper:**
  Merumuskan protokol **Balanced CT** dan **Balanced MiniCast** yang mempertahankan hak voting berbobot stake (*stake-weighted voting*), namun mengalokasikan tepat satu fragmen data erasure-code ke setiap validator/relayer. Pendekatan ini menghasilkan batas beban upload P2P yang seimbang dan seragam (*uniform upload bound*) di antara seluruh node jujur. Dilengkapi algoritma *Capped Swiper*, total biaya transmisi berkurang 19%–81% untuk CT dan 16%–71% untuk MiniCast, serta varian *sender-aware* di mana pengusul bundle tidak perlu me-relay ulang fragmen.
* **Implementasi di Roadmap Zeltra Node:**

---

## 10. Batched Threshold Encryption for Encrypted Mempool & Anti-MEV Sequencing (IACR ePrint 2026/2205)
> [!NOTE]
> **DESENTRALISASI MASA DEPAN (ROADMAP):**
> Paper ini ditandai khusus sebagai fondasi arsitektur tahap transisi dari *Single Stateless Sequencer Relayer* menuju **Decentralized Multi-Relayer Network**.

* **File Jurnal / ePrint:** IACR ePrint 2026/2205 (September 2026)
* **Penulis:** Guru-Vamsi Policharla
* **Problem di Sistem Desentralisasi:**
  Ketika relayer didelegasikan ke banyak operator independen (desentralisasi sequencer), muncul ancaman eksploitasi Maximal Extractable Value (MEV), *frontrunning*, dan penyensoran transaksi: operator relayer nakal dapat melihat isi transaksi (atau payload public spend) sebelum dimasukkan ke dalam blok, lalu menyalip atau menolak transaksi tersebut.
* **Inovasi Paper:**
  Merumuskan *Batched Threshold Encryption with Transparent Setup* (`DKG Is All You Need`). Berbeda dari skema enkripsi threshold konvensional yang parameternya bergantung pada ukuran batch tertentu, skema ini:
  1. Bersifat *transparent setup*: hanya membutuhkan protokol Distributed Key Generation (DKG) standar untuk membagi share rahasia.
  2. Mendukung dekripsi batch berukuran bebas tanpa batas ($B$ ciphertexts) hanya dengan **2 operasi pairing** dan agregasi skalar sederhana.
  3. Memungkinkan *ramp setting* ($t_{recon} > t_{corr}$) bebas pairing dari asumsi DDH standar.
* **Implementasi di Roadmap Zeltra Node:**
  * Komponen Roadmap: Sub-modul *Encrypted Ingress Mempool* di [`zeltra-node/src/handlers/spend.rs`](file:///workspaces/Zeltra-Protocol/zeltra-node/src/handlers/spend.rs).
  * Menghadirkan *anti-MEV commit-then-reveal mempool* pada jaringan relayer terdesentralisasi: pengguna mengirimkan payload spend terenkripsi threshold. Transaksi diurutkan terlebih dahulu secara terdistribusi tanpa bisa diintip isinya, lalu didekripsi secara massal dalam 2 pairing sebelum dibroadcast ke kontrak Stylus.

---

## 11. Randomized Early-Stopping Byzantine Agreement for Fast Sequencer Consensus (IACR ePrint 2026/2284)
> [!NOTE]
> **DESENTRALISASI MASA DEPAN (ROADMAP):**
> Paper ini ditandai khusus sebagai fondasi protokol konsensus bundle untuk transisi arsitektur menuju **Decentralized Multi-Relayer Network**.

* **File Jurnal / ePrint:** IACR ePrint 2026/2284 (September 2026)
* **Penulis:** Michele Ciampi, Pouyan Forghani, Yun Lu, Vassilis Zikas
* **Problem di Sistem Desentralisasi:**
  Untuk mempertahankan mantra produk Zeltra (*"Privacy 9.5/10, Product 10/10"* dengan kecepatan sekelas Apple Pay / QRIS <1 detik), jaringan relayer terdesentralisasi tidak boleh melambat karena protokol konsensus Byzantine Agreement yang lambat atau menunggu timeout fixed yang lama saat ada node yang offline atau melambat.
* **Inovasi Paper:**
  Merumuskan kompilator *Randomized Early-Stopping Byzantine Agreement (BA)* pertama yang menggabungkan dua keunggulan terbaik:
  1. *Expected constant rounds* ($O(1)$) pada kondisi normal (konfirmasi instan sub-detik).
  2. *Near-optimal early stopping* ($O(f)$ rounds) pada skenario terburuk (*worst-case*), di mana durasi penyelesaian murni proporsional terhadap jumlah aktual node yang terkorupsi ($f$), bukan batas toleransi maksimum ($t$).
  3. Menyediakan kompilator paralel untuk konsistensi interaktif (*parallel broadcast / Interactive Consistency*) yang mengeksekusi $m$ transaksi secara paralel dengan kompleksitas ronde konstan.
* **Implementasi di Roadmap Zeltra Node:**
  * Komponen Roadmap: Mesin konsensus P2P di [`zeltra-node/`](file:///workspaces/Zeltra-Protocol/zeltra-node/).
  * Memberikan mesin konsensus batching transaksi antar-relayer dengan latensi sub-detik (expected 1-2 round): Zeltra tetap secepat Apple Pay meskipun relayer dijalankan oleh puluhan validator terdesentralisasi secara global.

---

## 12. Bandwidth Fee Mechanisms for Certified Transaction Dissemination (IACR ePrint 2026/2285)
> [!NOTE]
> **DESENTRALISASI MASA DEPAN (ROADMAP):**
> Paper ini ditandai khusus sebagai model ekonomi bandwidth untuk transisi arsitektur menuju **Decentralized Multi-Relayer Network**.

* **File Jurnal / ePrint:** IACR ePrint 2026/2285 (September 2026)
* **Penulis:** Eleftheria Fassman, Mahimna Kelkar, Benjamin Marsh, Ke Wu
* **Problem di Sistem Desentralisasi:**
  Dalam arsitektur terdesentralisasi di mana desentralisasi penyebaran transaksi (*transaction dissemination & threshold certification*) dipisahkan dari eksekusi smart contract on-chain, mekanisme fee gas tradisional (seperti EIP-1559 untuk komputasi EVM) tidak cocok untuk memberi insentif pada relayer yang menanggung biaya transmisi bandwidth P2P. Tanpa pasar fee bandwidth yang adil, relayer rentan melakukan kolusi atau memprioritaskan transaksi secara bias.
* **Inovasi Paper:**
  Merumuskan *Bandwidth Fee Mechanisms (BFM)* formal berbasis lelang dua sisi (*two-sided procurement auction*) antara pengguna yang mengirim transaksi dan validator/relayer yang menerima serta menyiarkannya:
  1. Membuktikan batas insentif kompatibilitas (*myopic incentive compatibility*): mencegah kolusi antara dua relayer dengan skema *greedy-route posted-price mechanism*.
  2. Merumuskan aturan penyesuaian harga bandwidth dinamis bergaya EIP-1559 yang terbukti secara matematis memiliki titik ekuilibrium unik (*unique fixed point*) dan stabil secara lokal di pasar jaringan besar.

---

## 13. Overcoming Payload Blindness Pitfalls in Decentralized MEV Defense (IACR ePrint 2026/2265)
> [!NOTE]
> **DESENTRALISASI MASA DEPAN (ROADMAP):**
> Paper ini ditandai khusus sebagai panduan keamanan operasional untuk transisi arsitektur menuju **Decentralized Multi-Relayer Network**.

* **File Jurnal / ePrint:** IACR ePrint 2026/2265 (September 2026)
* **Penulis:** Simona Češková, Martin Perešíni, Ivan Homoliak
* **Problem di Sistem Desentralisasi:**
  Banyak protokol mitigasi MEV mengklaim keamanan berbasis *payload blindness* (seperti enkripsi mempool, verifikasi permutasi acak, atau penundaan blok). Namun, paper ini membuktikan secara empiris bahwa mitigasi tersebut kerap gagal total di produksi nyata (*The Illusion of Payload Blindness*) akibat 3 faktor:
  1. *Unencrypted Envelope Leakage*: Metadata pembungkus transaksi (seperti alamat pengirim, IP jaringan, batas gas, atau header relayer) membocorkan pola perilaku masa lalu pengguna sehingga pembuat blok (*builders*) tetap dapat menebak isi transaksi.
  2. *Selective Starvation*: Fitur enkripsi opsional menyebabkan relayer/proposer sengaja menahan atau mengabaikan (*starve*) transaksi terenkripsi demi mengeksekusi transaksi publik yang menghasilkan tip MEV lebih tinggi.
  3. *Incentive Misalignment*: Menekan MEV sering kali memotong pendapatan operator node tanpa model insentif kompensasi yang layak, mendorong relayer melakukan kolusi tersembunyi.
* **Inovasi & Pembelajaran Arsitektur Zeltra:**
  Paper ini menyimpulkan bahwa kunci pencegahan MEV sejati bukanlah sekadar enkripsi payload di mempool, melainkan penyelarasan struktur ekonomi dan eliminasi kebocoran metadata di tingkat desain.
* **Implementasi di Roadmap Zeltra Node:**
  * Komponen Roadmap: Pipeline pemrosesan bundle transaksi terdesentralisasi di [`zeltra-node/`](file:///workspaces/Zeltra-Protocol/zeltra-node/).

---

## 14. Rational Harm-Proof Transaction Fee Mechanisms for Decentralized Relayer Committees (IACR ePrint 2025/2247)
> [!NOTE]
> **DESENTRALISASI MASA DEPAN (ROADMAP):**
> Paper ini ditandai khusus sebagai model game-theoretic insentif fee untuk transisi arsitektur menuju **Decentralized Multi-Relayer Network**.

* **File Jurnal / ePrint:** IACR ePrint 2025/2247 (September 2026 update)
* **Penulis:** Forest Zhang, Elain Park, Ke Wu
* **Problem di Sistem Desentralisasi:**
  Mekanisme penetapan fee transaksi (*Transaction Fee Mechanism - TFM*) berbasis *Incentive Compatibility (IC)* tradisional hanya menjamin bahwa pelaku jujur memaksimalkan keuntungan sendiri. Namun, model ini menyisakan celah kritis: **Utility-Neutral Harmful Deviations**, di mana relayer nakal atau koalisi relayer-pengguna dapat mengubah tawaran atau urutan transaksi untuk merugikan pengguna lain tanpa biaya apa pun bagi penyerang (*costless harm*). Pada model penambang/relayer tunggal (*plain model*), paper ini membuktikan sebuah *tetrilemma*: tidak ada mekanisme deterministik yang mampu memenuhi pendapatan relayer positif, kompatibilitas insentif pengguna (UIC), kompatibilitas insentif relayer (MIC), dan kekebalan bahaya rasional (RHP) secara bersamaan.
* **Inovasi Paper:**
  Merumuskan konsep baru **Rational Harm-Proofness (RHP)** yang secara ketat melarang deviasi yang merugikan pihak jujur jika tidak mendatangkan penalti ekonomi bagi pelaku. Solusi utama:
  - Menggunakan model **MPC-assisted committee**: ketika lelang dan pengurutan transaksi dikelola secara kolektif oleh komite relayer terdesentralisasi via MPC acak terdistribusi, protokol berhasil mencapai UIC, MIC, dan RHP secara serentak terhadap ketiga jenis aktor strategis, serta mempertahankan pendapatan relayer yang positif.
* **Implementasi di Roadmap Zeltra Node:**
  * Komponen Roadmap: Mesin lelang dan kuotasi terdistribusi di [`zeltra-node/src/handlers/quote.rs`](file:///workspaces/Zeltra-Protocol/zeltra-node/src/handlers/quote.rs) dan agregasi multi-relayer di [`zeltra-node/`](file:///workspaces/Zeltra-Protocol/zeltra-node/).
  * Memberikan landasan teori ekonomi game mutlak bagi komite relayer terdesentralisasi Zeltra: mencegah relayer nakal melakukan griefing terhadap pengguna jujur atau menaikkan tarif gas eksekusi secara artifisial, menjamin ekosistem relayer yang adil dan tahan manipulasi.

---

## 15. Write-Public Oblivious RAM (wpORAM) for Oblivious Relayer State Querying & Dynamic PIR (IACR ePrint 2026/2163)
* **File Jurnal / ePrint:** IACR ePrint 2026/2163 (September 2026)
* **Penulis:** Ananya Appan, James Austgen, Kushal Babel, Sourav Das
* **Problem Kritis:**
  Pada Zeltra Relayer Node ([`zeltra-node/src/handlers/mmr.rs`](file:///workspaces/Zeltra-Protocol/zeltra-node/src/handlers/mmr.rs)), klien rutin melakukan query untuk bukti MMR (`/api/v1/mmr/proof/:leaf_index`), daun komitmen (`/api/v1/mmr/leaves`), atau status transaksi (`/api/tx-status`). Meskipun payload transaksi terenkripsi ZK-UTXO, query RPC biasa membocorkan *leaf index* atau ID yang sedang diambil pengguna, membuka celah korelasi alamat IP dan pola belanja. Skema Oblivious RAM (ORAM) tradisional seperti Path ORAM menimbulkan overhead bandwidth multi-roundtrip yang sangat lambat saat diaplikasikan ke node RPC blockchain.
* **Inovasi Paper:**
  Merumuskan primitif **Write-Public Oblivious RAM (wpORAM)**:
  1. *Relaksasi Asimetris Efisien:* Menyembunyikan data apa yang sedang dibaca pengguna (*read-oblivious*), namun membiarkan proses *write* atau pembaruan state di server bersifat publik (*write-public*). Karakteristik ini sangat selaras dengan sifat blockchain, di mana penambahan blok baru dan daun MMR bersifat publik bagi semua node.
  2. *Optimasi Bandwidth Signifikan:* Memangkas faktor $\log N$ pada bandwidth dan jumlah roundtrip dibanding Path ORAM konvensional saat pembaruan state.
  3. Memungkinkan node RPC blockchain melayani query pembacaan klien secara privat dengan penurunan performa (*slowdown*) hanya ~23%, sambil mempertahankan sinkronisasi state 3–4x lebih cepat dibanding Path ORAM standar.
* **Implementasi di Zeltra Node:**
  * Komponen: Handler sinkronisasi state di [`zeltra-node/src/handlers/mmr.rs`](file:///workspaces/Zeltra-Protocol/zeltra-node/src/handlers/mmr.rs) dan database relayer di [`zeltra-node/src/database.rs`](file:///workspaces/Zeltra-Protocol/zeltra-node/src/database.rs).
  * Menghadirkan *Oblivious MMR Query Endpoint*: Klien SDK dapat mengambil bukti keanggotaan Merkle Mountain Range tanpa membocorkan indeks daun (*leaf index*) yang sedang diverifikasi kepada operator relayer atau penyedia ISP, menutup celah profiling lalu lintas jaringan.

---

## 16. Asynchronous Byzantine Atomic Broadcast Under Network Message Drops (IACR ePrint 2026/2150)
> [!NOTE]
> **DESENTRALISASI MASA DEPAN (ROADMAP):**
> Paper ini ditandai khusus sebagai fondasi protokol konsensus bundle transaksi tahan kegagalan jaringan (*packet drops & mixed faults*) untuk transisi arsitektur menuju **Decentralized Multi-Relayer Network**.

* **File Jurnal / ePrint:** IACR ePrint 2026/2150 (September 2026)
* **Penulis:** Dalin Zheng, Yuchen Ye, Xiaolin Gui, Qiang Tang, Zhenliang Lu
* **Problem di Sistem Desentralisasi:**
  Ketika Zeltra bertransisi ke jaringan multi-relayer terdesentralisasi, transaksi bundle spend harus diurutkan secara atomik (*Byzantine Atomic Broadcast*). Pada jaringan internet publik antar-benua, kegagalan bukan hanya berupa node jahat (*Byzantine faults* $f$), melainkan juga paket jaringan yang hilang atau node yang mengalami *message drops* (partisi parsial / *receive-faulty* $f_r$ dan *send-faulty* $f_s$). Protokol Byzantine Agreement standar gagal memberikan output yang valid saat terjadi kegagalan campuran (*mixed-fault setting*).
* **Inovasi Paper:**
  Merumuskan **UABC (Undead Byzantine Atomic Broadcast)**, protokol *atomic broadcast* pertama dengan ketahanan optimal (*optimal resilience* $n \ge 3f + 2f_r + f_s + 1$) dalam model *mixed-fault*:
  1. *Undead Verifiable Information Dispersal (UVID):* Memungkinkan data transaksi dipecah dan disebarkan secara efisien, serta dapat direkonstruksi kembali meskipun sebagian node mengalami drop pesan atau bertindak jahat.
  2. *Opt-wUMVBA (Optimized Weak Undead Multi-Valued Validated Byzantine Agreement):* Menjamin konsensus nilai bermakna yang lolos predikat validasi dengan probabilitas kegagalan $O(2^{-\lambda})$.
  3. Mencapai kompleksitas komunikasi asimtotik optimal $O(n^2 \lambda + nl)$ untuk ukuran transaksi $l \ge n \lambda$.
* **Implementasi di Roadmap Zeltra Node:**
  * Komponen Roadmap: Mesin urutan transaksi P2P (*decentralized transaction sequencer layer*) di [`zeltra-node/`](file:///workspaces/Zeltra-Protocol/zeltra-node/).
  * Memastikan jaringan relayer terdesentralisasi Zeltra tidak pernah macet (*liveness guaranteed*) meskipun terjadi gangguan jaringan WAN lintas kawasan, transmisi paket UDP/TCP tersendat, atau serangan DoS parsial pada sebagian validator relayer.

---

## 17. Stake-Weighted Threshold Encryption with Silent Setup for Anti-MEV Mempool (IACR ePrint 2026/2148)
> [!NOTE]
> **DESENTRALISASI MASA DEPAN (ROADMAP):**
> Paper ini ditandai khusus sebagai fondasi kriptografi mempool terenkripsi berbasis Proof-of-Stake untuk transisi arsitektur menuju **Decentralized Multi-Relayer Network**.

* **File Jurnal / ePrint:** IACR ePrint 2026/2148 (September 2026)
* **Penulis:** Guru-Vamsi Policharla
* **Problem di Sistem Desentralisasi:**
  Dalam komite relayer terdesentralisasi berbasis bobot taruhan (*Proof-of-Stake / stake-weighted committee*), mencegah frontrunning dan censorship menuntut transaksi dienkripsi (*encrypted mempool*) hingga urutan blok dikunci. Skema enkripsi ambang batas (*threshold encryption*) konvensional menghadapi 2 kelemahan besar:
  1. Mengharuskan upacara *Distributed Key Generation (DKG)* interaktif yang rumit dan rentan gagal jika relayer keluar-masuk (*churn*).
  2. Bersifat *unweighted* (setiap node dianggap berbobot 1), tidak kompatibel dengan komite PoS di mana relayer memiliki bobot voting berbeda ($W$) proporsional terhadap stake.
* **Inovasi Paper:**
  Membangun skema **Weighted Threshold Encryption with Silent Setup** pertama di atas kurva bilinear:
  1. *Silent Setup:* Tidak memerlukan DKG interaktif sama sekali! CRS dan kunci publik berukuran ringkas $O(W)$, di mana $W$ adalah batas bobot komite maksimum.
  2. *Dynamic Weights & Dynamic Threshold:* Bobot masing-masing relayer dan batas ambang dekripsi ($T$) dapat ditentukan secara dinamis saat enkripsi dilakukan.
  3. *Ultra-Succinct Ciphertext & Non-Interactive Partial Decryption:* Ciphertext CPA hanya berukuran **dua elemen grup $G_1$** (rekor terpendek di dunia), dan setiap relayer menghasilkan dekripsi parsial non-interaktif hanya dengan **1 operasi perkalian skalar** di $G_1$, terlepas dari berapa besar bobot relayer tersebut. Keamanan CCA hanya menambah dua elemen lapangan.
* **Implementasi di Roadmap Zeltra Node:**
  * Komponen Roadmap: Ingress mempool terenkripsi di [`zeltra-node/src/handlers/spend.rs`](file:///workspaces/Zeltra-Protocol/zeltra-node/src/handlers/spend.rs) dan agregasi relayer di [`zeltra-node/`](file:///workspaces/Zeltra-Protocol/zeltra-node/).
  * Memungkinkan pengguna Zeltra mengenkripsi transaksi spend ke komite relayer PoS dengan payload seringkas 96 byte, didekripsi secara otomatis dalam 1 operasi skalar setelah urutan blok disahkan, menjamin eliminasi MEV tanpa overhead DKG interaktif.

---

## 18. Incremental Single-Server Private Information Retrieval (iSimplePIR) for MMR Leaf Sync (IACR ePrint 2026/030)
* **File Jurnal / ePrint:** IACR ePrint 2026/030 (September 2026)
* **Penulis:** Pengfei Lu, Guangwu Xu, Zengpeng Li, Mei Wang, Haoyu Cui
* **Problem Kritis:**
  Pada Zeltra Relayer tunggal saat ini (*Single Stateless Sequencer Relayer*), pohon Merkle Mountain Range (MMR) bertambah daunnya secara terus-menerus setiap kali ada transaksi `deposit_direct_note` atau `spend_joinsplit`. Protokol Private Information Retrieval (PIR) server tunggal yang ada saat ini (seperti SimplePIR standar) membutuhkan preprocessing penuh terhadap seluruh database setiap kali ada penambahan daun baru, menyebabkan lonjakan komputasi CPU yang tidak realistis untuk database dinamis.
* **Inovasi Paper:**
  Merumuskan konsep formal pertama **Single-Server Incremental PIR (iSimplePIR)**:
  1. Mendukung pembaruan data secara real-time (*real-time entry-level insertions & updates*) tanpa perlu memproses ulang (*re-preprocess*) seluruh database.
  2. Menghadirkan mekanisme agregasi baris (*row aggregation*) yang menghemat bandwidth pada siklus pembaruan periodik.
  3. Memangkas komputasi overhead preprocessing hingga **224x lipat** dan memangkas biaya komunikasi sebesar **4.2x lipat** untuk pembaruan 1% pada database 1GB, serta menurunkan waktu pemrosesan pembaruan hingga 86%.
* **Implementasi di Zeltra Node:**
  * Komponen: Sinkronisasi daun MMR di [`zeltra-node/src/handlers/mmr.rs`](file:///workspaces/Zeltra-Protocol/zeltra-node/src/handlers/mmr.rs) dan worker pengindeks di [`zeltra-node/src/mmr_indexer.rs`](file:///workspaces/Zeltra-Protocol/zeltra-node/src/mmr_indexer.rs).
  * Memungkinkan klien Zeltra mengunduh bukti atau daun MMR baru secara instan dan privat menggunakan PIR server-tunggal berkecepatan tinggi: relayer dapat menyisipkan komitmen note baru ke tabel database tanpa perlu mengkalkulasi ulang matriks PIR dari nol.

---

## 19. Batched and Weighted Threshold Encryption with Silent Setup for Decentralized Mempool (IACR ePrint 2026/2087)
> [!NOTE]
> **DESENTRALISASI MASA DEPAN (ROADMAP):**
> Paper ini ditandai khusus sebagai fondasi kriptografi mempool terenkripsi berbasis batched threshold encryption untuk transisi arsitektur menuju **Decentralized Multi-Relayer Network**.

* **File Jurnal / ePrint:** IACR ePrint 2026/2087 (September 2026)
* **Penulis:** Amit Agarwal, Rutchathon Chairattana-Apirom, Sourav Das, Babak Poorebrahim Gilkalaye
* **Problem di Sistem Desentralisasi:**
  Batched Threshold Encryption (BTE) memungkinkan komite relayer terdesentralisasi mendekripsi subset transaksi terpilih secara bersama-sama sambil menjaga kerahasiaan transaksi lainnya untuk mengeliminasi serangan MEV dan frontrunning di mempool. Namun, skema BTE yang ada menghadapi kendala usability besar: (a) ketergantungan pada protokol Distributed Key Generation (DKG) interaktif yang mahal dan lambat, (b) ketergantungan pada batch label saat enkripsi, (c) ciphertexts hanya bisa didekripsi jika sesuai indeks non-konflik, atau (d) ketiadaan dukungan terhadap kebijakan ambang batas berbobot (*weighted threshold*).
* **Inovasi Paper:**
  Menyajikan skema BTE pertama yang menyelesaikan keempat masalah di atas secara simultan:
  1. *Silent Setup:* Kunci publik terbentuk tanpa interaksi DKG antar-relayer.
  2. *Batch & Index Independent:* Bebas dari keterikatan label batch dan indeks posisi.
  3. *Weighted Threshold Support:* Mendukung kebijakan threshold berbobot proporsional terhadap stake relayer di jaringan PoS.
  4. Berbasis grup prima dengan pairing bilinear di Generic Group Model (GGM), menghasilkan ukuran ciphertext terpendek di dunia (hanya 2 elemen grup di luar panjang pesan).
* **Implementasi di Roadmap Zeltra Node:**
  * Komponen Roadmap: Ingress mempool terenkripsi anti-MEV di [`zeltra-node/src/handlers/spend.rs`](file:///workspaces/Zeltra-Protocol/zeltra-node/src/handlers/spend.rs) dan agregasi relayer terdesentralisasi di [`zeltra-node/`](file:///workspaces/Zeltra-Protocol/zeltra-node/).
  * Menyempurnakan roadmap desentralisasi relayer Zeltra: memungkinkan jaringan multi-relayer mengoperasikan mempool terenkripsi tanpa overhead upacara DKG interaktif yang rumit, di mana transaksi spend pengguna didekripsi secara atomik setelah urutan blok dikunci.

---

## 20. RisePIR: Incremental Keyword PIR with Segmented Cuckoo Filters for Real-Time State Sync (IACR ePrint 2026/2082)
* **File Jurnal / ePrint:** IACR ePrint 2026/2082 (September 2026)
* **Penulis:** Bao Ninh
* **Problem Kritis:**
  Pada sinkronisasi status koin privat dan pengecekan saldo di node RPC/relayer, protokol Private Information Retrieval (PIR) konvensional mengharuskan pemrosesan ulang (*re-preprocessing*) database dari awal setiap kali terjadi mutasi (insert, update, delete). Pada blockchain berkecepatan tinggi di mana daun MMR bertambah terus-menerus, skema statis seperti ChalametPIR memakan waktu 56–686 detik per pembaruan database, tidak layak untuk pemrosesan waktu nyata.
* **Inovasi Paper:**
  Memperkenalkan *RisePIR*, skema preprocessing keyword PIR pertama yang mampu menyerap operasi insert, update, dan delete pada penyimpanan key-value dengan biaya komputasi yang proporsional murni terhadap jumlah mutasi saja:
  1. Menggunakan struktur data baru *$d$-ary Segmented Cuckoo Filter* yang mengurung kandidat bucket dan dinamis di bawah *partial-key cuckoo hashing*.
  2. Diinstansiasi di atas FrodoPIR dan SimplePIR dengan patch in-place hanya dalam 0.06–4.7 ms per mutasi (5 hingga 6 orde magnitudo lebih cepat daripada re-run statis).
  3. Teruji nyata pada layanan `eth_getBalance` privat di mainnet dengan waktu respons query 0.69 detik dan absorpsi mutasi per-blok hanya 5.6 ms.
* **Implementasi di Zeltra Node:**
  * Komponen: Handler query daun MMR di [`zeltra-node/src/handlers/mmr.rs`](file:///workspaces/Zeltra-Protocol/zeltra-node/src/handlers/mmr.rs) dan database relayer di [`zeltra-node/src/database/mmr.rs`](file:///workspaces/Zeltra-Protocol/zeltra-node/src/database/mmr.rs).
  * Memungkinkan Zeltra Relayer melayani query pengecekan status note dan saldo komitmen secara *oblivious* (klien memeriksa status nullifier/leaf tanpa membocorkan indeks atau alamat kepada relayer) secara real-time, di mana relayer dapat memperbarui database MMR daun secara instan (<5 ms) seiring bertambahnya transaksi baru tanpa mengorbankan performa query privat.
