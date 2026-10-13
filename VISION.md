# ZELTRA PROTOCOL MANIFESTO (VISION & MISSION)

> **"The Stripe of Web3 with Absolute Privacy."**  
> *Engine: Pure ZK-UTXO on Arbitrum Stylus (Rust WASM + EIP-2537 BLS12-381)*  
> *Transition Codebase: Zeltra*

---

## 1. Identitas & DNA Protokol

* **Nama Resmi:** **Zeltra Protocol** *(codebase transisi: Zeltra)*.
* **Kategori:** *Universal Shielded Payment & Settlement Rail* (Human & Autonomous AI Economy).
* **Visi Inti:** Menjadi infrastruktur pembayaran universal di era Web3 dan agen AI otonom. Aliran likuiditas bergerak secepat dan semudah Apple Pay / QRIS, dilindungi benteng matematika privasi zero-knowledge mutlak tanpa celah doxxing.
* **Mantra Produk:** **Privasi 9.5/10, Produk 10/10.**
  * Privasi sejati tidak boleh mengorbankan kenyamanan pengguna.
  * Mental model kas nyata: bayar $3 dari note $10, terima kembalian $7 secara instan dan privat (*ZK-UTXO Change Note*).
  * Tanpa diskriminasi akses: protokol melayani AI Agent, retail, whale, korporat B2B, dan merchant global pada tingkat keamanan setara.

---

## 2. Masalah yang Kami Hancurkan

1. **Surveillance & Doxxing Finansial On-Chain:**
   * Di blockchain publik, saldo, vendor penerima, dan riwayat belanja terekspos telanjang.
   * Setiap pembayaran komersial membuka celah front-running MEV, spionase kompetitor, dan hilangnya kedaulatan finansial personal.
2. **Privasi Lambat, Boros Gas & "Voucher Hell":**
   * Solusi privasi generasi pertama (mixer pool nominal tetap, rollup lambat) membebankan latensi tinggi, gas mahal, dan UX kaku ("voucher pecahan").
   * Pengguna dipaksa memecah dana ke pecahan kaku alih-alih bertransaksi layaknya uang tunai fleksibel.
3. **Fragmentasi & Ketiadaan Rel Ekonomi AI Agent:**
   * AI Agent otonom membutuhkan jalur pembayaran mikro mesin-ke-mesin (M2M) sub-detik tanpa risiko membocorkan API key, saldo master wallet, atau strategi perdagangan di mempool publik.

---

## 3. Pilar Filosofis & Model Ekonomi

### A. Zero-Friction Inflow (Deposit 0%)
* Masuk ke Zeltra tidak ditarik biaya: **Deposit fee 0% permanen** (DEC-038B).
* Dana pengguna masuk 100% utuh tanpa potongan admin di pintu masuk.

### B. Volume-Driven Velocity (Kecepatan Perputaran Uang)
* Zeltra menolak model parasitik yang menarik biaya atas dana diam (*idle TVL tax*).
* Keberlanjutan ekonomi bertumpu pada **velocity of money (kecepatan perputaran volume harian)**:
  * Flat protocol spend fee: **0.45% (45 bps)** (DEC-028) tanpa diskriminasi waktu tahan saldo.
  * Relayer execution fee: penggantian biaya gas aktual + margin transparan 15% (DEC-031).

### C. Solvensi Kas 100% (Anti-Fractional Reserve & Anti-FTX)
* **Prinsip Kas Nyata:** Setiap $1 USDC yang tersimpan di brankas smart contract menjamin tepat $1 hak klaim riil milik pengguna dan relayer.
* **Nol Uang Gaib:** Tidak ada fractional reserve, tidak ada pinjaman spekulatif (rehypothecation). Satu dollar masuk, tepat satu dollar keluar.
* **Verifikasi Otomatis Tiap Transaksi:** Smart contract memeriksa kecukupan cadangan kas pada setiap mutasi saldo. Jika terjadi selisih 1 sen sekalipun, transaksi langsung dibatalkan (revert) seketika oleh sistem.

### D. Kepatuhan & Auditabilitas Ganda (Permissionless Privacy + Institutional-Ready)
Zeltra memadukan kedaulatan cypherpunk dengan kebutuhan kepatuhan regulasi institusional dan korporat global:
1. **Penerimaan Terseleksi di Sisi Penerima (DEC-026):**
   * Prinsip: *"Permissionless Privacy, Permissioned Acceptance"*.
   * Protokol tidak memaksakan sensor global terpusat. Pihak penerima (merchant/institusi) yang menentukan kriteria penerimaan dana berdasarkan kebijakan kepatuhan lokal masing-masing.
2. **Penyaringan Sanksi Mandiri Tanpa Biaya Tambahan:**
   * Relayer menyaring daftar sanksi resmi internasional (OFAC SDN List) secara instan di memori lokal tanpa bergantung pada vendor kepatuhan komersial berbiaya mahal. Aktor terlarang terisolasi dari merchant legal.
3. **Kunci Audit Selektif untuk Laporan Resmi (Dual Viewing Keys - DEC-037):**
   * Pemisahan derivasi kunci kriptografis:
     * **Incoming Viewing Key (IVK):** Mendekripsi riwayat dana masuk untuk bukti tanda terima pendapatan.
     * **Outgoing Viewing Key (OVK):** Mendekripsi riwayat dana keluar untuk pelaporan audit pajak dan pembukuan korporat.
   * Memberikan keterbukaan penuh kepada auditor atau otoritas pajak secara sukarela **tanpa pernah menyerahkan kunci belanja dompet (Spending Key)** atau membocorkan privasi ke publik.

---

## 4. Keunggulan Arsitektur & Rekayasa (Pure ZK-UTXO)

Seluruh sistem Zeltra beroperasi di atas arsitektur ZK-UTXO murni performa tinggi tanpa perantara kustodi (DEC-038):

1. **Eksekusi Secepat Kilat & Verifikasi Privasi Sub-Detik (Stylus WASM + EIP-2537):**
   * **Super Hemat Biaya (Arbitrum Stylus):** Logika kontrak ditulis dalam bahasa Rust berkecepatan tinggi (WASM), memangkas konsumsi gas transaksi hingga 10–50x lebih murah dibandingkan kontrak konvensional.
   * **Verifikasi Privasi Instan (EIP-2537):** Kontrak memanfaatkan instruksi prosesor bawaan Ethereum (precompile EIP-2537) untuk memeriksa bukti Zero-Knowledge (Groth16 BLS12-381) dalam waktu <250 milidetik.
   * **Fungsi Inti:** Smart contract dapat memvalidasi keabsahan uang yang Anda belanjakan tanpa pernah mengetahui siapa Anda, siapa merchant penerima, atau berapa sisa tabungan Anda.

2. **Buku Besar Komitmen Dinamis (Merkle Mountain Range - DEC-032):**
   * **Pencatatan Tanpa Batas:** Menggunakan struktur data *Merkle Mountain Range* (MMR) berkapasitas hingga miliaran transaksi tanpa pernah memperlambat sistem.
   * **Sidik Jari Kriptografis Ringan (Poseidon-W5 - DEC-021):** Menggunakan fungsi hash ramah ZK yang sangat hemat komputasi untuk membungkus data uang menjadi komitmen tak terlacak.
   * **Anti-Macet & Bebas Gagal (Ring Buffer 100 Roots):** Sistem menyimpan 100 riwayat status pohon terbaru on-chain. Transaksi Anda tidak akan pernah gagal meskipun ribuan transaksi lain masuk bersamaan di detik yang sama.

3. **Deposit Seketika Tanpa Potongan (Direct MMR Inflow - DEC-038B):**
   * **Nol Biaya Masuk (0% Deposit):** Pengguna menyetor USDC (minimal $10) dan langsung menerima lembaran kas digital privat (*Private Note*) bernilai 100% utuh dalam 1 kali transaksi L2.
   * **Langsung Aktif Tanpa Menunggu:** Tidak ada penundaan, tidak ada perantara admin, dan tidak ada masa tunggu (timelock). Saldo kas privat Anda seketika aktif dan siap dibelanjakan.
   * **Catatan Kas Terenkripsi (96-Byte Memo):** Data kembalian dan bukti kepemilikan dienkripsi militer (ChaCha20-Poly1305) dan disiarkan via log blockchain. Hanya pemilik kunci yang bisa membacanya—bahkan pihak validator dan relayer pun buta.

4. **Mesin Pembelanjaan Kas & Kembalian Fleksibel (ZK Circuits - zeltra-core):**
   * **Belanja Tunggal Presisi (PrivateNoteCircuit - DEC-035A/B):**
     * Membayar transaksi belanja 1 note secara instan dengan kembalian privat.
     * **Kunci Pengaman Anti-Pembajakan (Scope Binding - DEC-035B):** Nilai belanja, alamat merchant, dan batas waktu kadaluarsa dikunci mati di dalam bukti matematika ZK. Tidak ada perantara atau pihak jahat yang bisa membelokkan dana Anda ke alamat lain.
     * **Pencegahan Pembengkakan Memori & Tiket Hangus (Nullifier Pruning - DEC-033):**
       * Setiap lembar kas yang dibelanjakan membakar tiket unik (*nullifier*) agar tidak bisa dibelanjakan dua kali (*anti-double spend*).
       * **Bebas Memori Sampah Abadi (Rotasi 2-Epoch):** Berbeda dari sistem privasi lawas yang menumpuk data tiket usang selamanya, Zeltra membersihkan memori secara berkala dalam jendela 2-epoch sehingga blockchain tetap ringan.
       * **Saldo Tertidur Tidak Pernah Hangus (In-Flight Rollover 0% Fee):** Uang kas lama yang tersimpan bertahun-tahun otomatis diperbarui (*rollover*) ke epoch terkini saat dibelanjakan, tanpa potongan biaya protokol tambahan.
   * **Gabung & Pecah Saldo Bebas (JoinSplitCircuit - DEC-036A):**
     * Menggabungkan 2 lembar kas kecil untuk belanja besar, atau memecah 1 lembar kas besar menjadi belanja + kembalian baru secara mulus ($10 bayar $3, kembali $7).
     * **Privasi Seragam (Constant-Arity & Penumbra Dummy):** Semua transaksi Zeltra terlihat identik dari luar (selalu 2 input dan 2 output). Pengamat luar dan bot pelacak on-chain tidak bisa membedakan mana pembayaran sungguhan dan mana saldo sisa kembalian.
     * **Benteng Validitas Matematika (448 Range Gates):** Memastikan seluruh nominal transaksi diperiksa secara ketat dan mustahil dimanipulasi menjadi angka palsu atau negatif.

5. **Jaringan Relayer Tanpa Kustodi (Stateless Sequencer Pipeline):**
   * **Belanja Tanpa Saldo Gas (Gasless User Experience):** Pengguna tidak perlu memegang saldo ETH/Arbitrum untuk belanja. Relayer menalangi gas on-chain dan menerima penggantian langsung dari nilai note (+ margin transparan 15% - DEC-031).
   * **Relayer Sebagai Pipa Transmisi Buta:** Relayer tidak memiliki hak kustodi atas dana, tidak tahu kunci rahasia Anda, dan tidak bisa mengubah tujuan transaksi.
   * **Pencegahan Spam Mandiri (Preflight Check - DEC-026 C-01):** Relayer memverifikasi keabsahan bukti matematika di komputasi lokal terlebih dahulu sebelum membayarkan gas on-chain, menjamin jaringan beroperasi efisien tanpa risiko kerugian gas.

6. **Dompet Mandiri Klien & Rel Pembayaran AI Agent (Client SDK & x402):**
   * **Kedaulatan 100% di Tangan Anda (Self-Custody):** Bukti matematika (ZK Proof) dibuat 100% di browser atau aplikasi ponsel pengguna. Kunci rahasia tersimpan aman terenkripsi (Argon2id + ChaCha20-Poly1305) dan tidak pernah meninggalkan perangkat.
   * **Pemilihan Kas Cerdas (Stochastic Knapsack - DEC-030/036B):** Dompet secara otomatis memilih kombinasi lembaran kas yang paling efisien untuk memutus pola sidik jari transaksi dompet.
   * **Rel Pembayaran AI Agent Otonom (Protokol HTTP 402 & MinMandate):** Agen AI dapat membayar layanan API mikro per panggilan secara otomatis dengan pagu anggaran terbatas, tanpa risiko dompet induk dikuras jika kode agen mengalami error atau dieksploitasi.

---

## 5. Peta Masa Depan (North Star & Roadmap 2026–2030)

* **Rel Utama AI Agentic Economy:** Menjadi protokol standar penyelesaian mikro-transaksi privat untuk platform agen cerdas (OpenAI, Anthropic Claude, Gemini agent workflows, dan framework otonom).
* **Cross-Chain Shielded Settlement via Chainlink CCIP:** Routing likuiditas lintas rantai EVM dan L2, memungkinkan deposit di satu chain dan klaim terlindungi di chain lain tanpa kebocoran graf transaksi.
* **Desentralisasi Relayer & Fair Sequencing:** Transisi relayer bootstrap menuju konsorsium stateless sequencer terdesentralisasi berbasis *staking game theory* dengan perlindungan anti-MEV batching.
* **Oblivious Scanning & Zero-Leakage Wallet:** Implementasi sinkronisasi note terenkripsi skala besar yang sepenuhnya oblivious terhadap ISP maupun penyedia RPC.
* **Budaya Rekayasa:** **No Compromise. Zero Laziness. Fail-Closed.** Bukti matematika dan integritas solvensi selalu di atas kenyamanan kompromi.
