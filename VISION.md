# ZELTRA PROTOCOL MANIFESTO (VISION & MISSION)

> **"The Stripe of Web3 with Absolute Privacy."**  
> *Engine: Pure ZK-UTXO on Arbitrum Stylus (Rust WASM + EIP-2537 BLS12-381)*  
> *Dual Sovereign Rails: USDC (Micro-USDC) & WETH / Native ETH (Gwei)*  
> *Transition Codebase: Zeltra*

---

## 1. Identitas & DNA Protokol

* **Nama Resmi:** **Zeltra Protocol** *(codebase transisi: Zeltra)*.
* **Kategori:** *Universal Shielded Payment & Settlement Rail* (Human, B2B & Autonomous AI Economy).
* **Visi Inti:** Menjadi infrastruktur pembayaran universal di era Web3 dan agen AI otonom. Aliran likuiditas bergerak secepat dan semudah Apple Pay / QRIS, dilindungi benteng matematika privasi zero-knowledge mutlak tanpa celah doxxing.
* **Mantra Produk:** **Privasi 10/10, Produk 10/10.**
  * Privasi sejati tidak boleh mengorbankan kenyamanan pengguna.
  * Mental model kas nyata & fleksibilitas ganda:
    * **In-Pool Peer-to-Peer:** Bayar langsung ke alamat privat merchant/lawan transaksi tanpa token keluar ke publik (*Shielded-to-Shielded In-Pool Transfer*).
    * **Public Spend with Change:** Bayar $3 dari kas $10 ke dompet publik, terima kembalian privat $7 secara otomatis (*ZK-UTXO Change Note*).
  * Tanpa diskriminasi akses: protokol melayani AI Agent, retail, whale, korporat B2B, dan merchant global pada tingkat keamanan setara.

---

## 2. Masalah yang Kami Hancurkan

1. **Surveillance & Doxxing Finansial On-Chain:**
   * Di blockchain publik, saldo, vendor penerima, dan riwayat belanja terekspos telanjang.
   * Setiap pembayaran komersial membuka celah front-running MEV, spionase kompetitor, dan hilangnya kedaulatan finansial personal.
2. **Privasi Lambat, Boros Gas & "Voucher Hell":**
   * Solusi privasi generasi pertama (mixer pool nominal tetap, rollup lambat) membebankan latensi tinggi, gas mahal, dan UX kaku ("voucher pecahan").
   * Pengguna dipaksa memecah dana ke pecahan kaku alih-alih bertransaksi layaknya uang tunai fleksibel.
3. **Dilema Kriminalisasi Privasi (Stigma Tornado Cash):**
   * Solusi privasi lawas memukul rata semua transaksi: pengguna jujur dicurigai dan dibekukan oleh bursa terpusat (CEX) karena ketiadaan alat verifikasi kriptografis selektif.
   * Zeltra menghancurkan dilema ini melalui **CEX Selective Disclosure**: pengguna jujur dapat membuktikan asal-usul dana bersih dan kepemilikan note ke bursa (Binance, Coinbase) secara matematis tanpa membocorkan riwayat transaksi lainnya.
4. **Fragmentasi & Ketiadaan Rel Ekonomi AI Agent:**
   * AI Agent otonom membutuhkan jalur pembayaran mikro mesin-ke-mesin (M2M) sub-detik tanpa risiko membocorkan API key, saldo master wallet, atau strategi perdagangan di mempool publik.

---

## 3. Pilar Filosofis & Model Ekonomi

### A. Zero-Friction Inflow (Deposit 0%)
* Masuk ke Zeltra tidak ditarik biaya: **Deposit fee 0% permanen** (DEC-038B).
* Berlaku setara untuk **USDC** dan **WETH / Native ETH** (otomatis dibungkus menjadi WETH secara payable di kontrak - DEC-034B).
* Dana pengguna masuk 100% utuh tanpa potongan admin di pintu masuk.

### B. Volume-Driven Velocity (Kecepatan Perputaran Uang)
* Zeltra menolak model parasitik yang menarik biaya atas dana diam (*idle TVL tax*). Keberlanjutan ekonomi bertumpu pada **velocity of money (kecepatan perputaran volume)**:
  * **In-Pool Shielded Transfer (Mode 2 - DEC-043):** **0% (0 bps) protocol fee** — perputaran uang internal antar pengguna/merchant 100% bebas biaya protokol guna memacu adopsi sirkular.
  * **Public Outflow Spend (Mode 3 - DEC-028):** Flat protocol fee **0.45% (45 bps)** saat kas dicairkan kembali ke alamat ERC-20 publik.
  * **Relayer Execution Fee:** Zero-oracle gas reimbursement (dalam Gwei untuk WETH, Micro-USDC untuk USDC) + margin transparan 15% (DEC-031 / DEC-034C).

### C. Solvensi Kas Multi-Asset Terisolasi (Anti-Fractional Reserve & Anti-FTX)
* **Prinsip Kas Nyata Multi-Asset (DEC-034B):** Setiap token yang tersimpan di brankas smart contract menjamin tepat hak klaim riil pengguna dan relayer pada aset tersebut:
  $$\text{Balance}(\text{asset}) \ge \text{user\_note\_liabilities}(\text{asset}) + \text{accrued\_execution\_fee\_liabilities}(\text{asset}) + \text{realized\_protocol\_fees}(\text{asset})$$
* **Siloed Asset Liability:** Akuntansi USDC dan WETH dipisahkan mutlak secara on-chain. Kegagalan atau volatilitas pada satu aset tidak akan pernah mengkontaminasi aset lainnya.
* **Nol Uang Gaib:** Tidak ada fractional reserve, tidak ada pinjaman spekulatif (rehypothecation). Satu dollar/ether masuk, tepat satu dollar/ether keluar.

### D. Kepatuhan & Kedaulatan Cypherpunk (Institutional-Ready)
Zeltra memadukan kedaulatan finansial dengan kebutuhan kepatuhan regulasi institusional global:
1. **Penerimaan Terseleksi di Sisi Penerima (DEC-026):**
   * Prinsip: *"Permissionless Privacy, Permissioned Acceptance"*.
   * Protokol tidak memaksakan sensor global terpusat. Pihak penerima (merchant/institusi) yang menentukan kriteria penerimaan dana berdasarkan kebijakan kepatuhan lokal masing-masing.
2. **Penyaringan Sanksi Mandiri Tanpa Biaya Tambahan:**
   * Relayer menyaring daftar sanksi resmi internasional (OFAC SDN List) secara instan di memori lokal tanpa bergantung pada vendor kepatuhan komersial berbiaya mahal. Aktor terlarang terisolasi dari merchant legal.
3. **Pemisahan Kunci 4-Tingkat & Kepatuhan CEX (DEC-040):**
   * **Hierarki Kunci Kriptografis (DEC-040A):**
     * **Spending Key ($sk$):** Otoritas mutlak pemindahan dana (hanya di tangan pengguna).
     * **Nullifier Key ($nk$):** Deteksi pembelanjaan note tanpa hak membelanjakan.
     * **Full Viewing Key ($fvk$):** Audit pembukuan menyeluruh (debit & kredit).
     * **Incoming/Outgoing Viewing Keys ($ivk$/$ovk$):** Audit penerimaan pendapatan atau pelaporan pajak pengeluaran.
   * **CEX Selective Disclosure Circuit (DEC-040A/B/C):** Menghasilkan bukti Groth16 dengan *Verifier Binding* (mengikat alamat bursa & challenge nonce) untuk membuktikan kepemilikan dan jalur bersih dana ke CEX (Binance, Coinbase) tanpa pernah menyerahkan $sk$ atau membocorkan sisa saldo privat lainnya.
4. **Alamat Privat Standar ZSA (Zeltra Shielded Address - DEC-043A):**
   * Alamat terlindungi 64-byte dengan format BIP-350 Bech32m canonical (`zs1...`), menggabungkan public spending key dan Montgomery public viewing key untuk enkripsi otomatis.

---

## 4. Keunggulan Arsitektur & Rekayasa (Pure ZK-UTXO)

Seluruh sistem Zeltra beroperasi di atas arsitektur ZK-UTXO murni performa tinggi tanpa perantara kustodi (DEC-038):

1. **Eksekusi Secepat Kilat & Verifikasi Privasi Sub-Detik (Stylus WASM + EIP-2537):**
   * **Super Hemat Biaya (Arbitrum Stylus):** Logika kontrak ditulis dalam Rust WASM, memangkas konsumsi gas transaksi hingga 10–50x lebih murah dibanding EVM konvensional.
   * **Verifikasi Privasi Instan (EIP-2537):** Kontrak memvalidasi bukti Groth16 BLS12-381 via precompile `0x0c` (MSM) dan `0x0f` (Pairing Check) dalam waktu <250 ms.

2. **Buku Besar Komitmen Dinamis (Merkle Mountain Range - DEC-032):**
   * **Pencatatan Tanpa Batas:** Struktur data *Merkle Mountain Range* (MMR) berkapasitas miliaran transaksi tanpa degradasi performa.
   * **Sidik Jari Kriptografis Ringan (Poseidon-W5 - DEC-021):** Hash ramah ZK dengan Grain-128 LFSR round constants untuk efisiensi komputasi sirkuit.
   * **Anti-Macet & Bebas Gagal (Ring Buffer 100 Roots):** Sistem menyimpan 100 riwayat status akar pohon terbaru on-chain sehingga transaksi tidak gagal akibat persaingan blok.

3. **Deposit Seketika Multi-Aset (Direct MMR Inflow - DEC-038B / DEC-034B):**
   * **Nol Biaya Masuk (0% Deposit):** Setoran USDC atau WETH/Native ETH langsung menyisipkan lembaran kas (*Private Note*) bernilai 100% utuh ke dalam pohon MMR dalam 1 kali transaksi L2.
   * **Catatan Kas Terenkripsi (96-Byte Memo):** Data kas dienkripsi AEAD ChaCha20-Poly1305 via ephemeral ECDH X25519 dan disiarkan via log blockchain (`NoteCommitmentAppended`).

4. **Mesin Transaksi Fleksibel 3-Mode (Universal JoinSplit - DEC-036 / DEC-043B):**
   Smart contract Stylus menerapkan **Strict 3-Mode State Machine** yang seragam dan anti-kebocoran:
   * **Mode 1 (Mint / Consolidation / Rollover):** Penggabungan lembar-lembar kas kecil atau perpanjangan epoch kas tertidur (2-epoch windowing - DEC-033) tanpa potongan biaya protokol.
   * **Mode 2 (In-Pool Transfer & Merchant Masking - DEC-043):**
     * Pengiriman langsung antar alamat ZSA tanpa ada token publik yang ditarik dari smart contract (`recipient = 0x0`, `protocol_fee = 0`).
     * Output 1 berisi komitmen & memo terenkripsi penerima, Output 2 berisi kembalian pengirim.
   * **Mode 3 (Public Outflow Spend):**
     * Penarikan token publik ke alamat recipient on-chain dengan potongan protocol fee 45 bps.
   * **Privasi Seragam (Constant-Arity 2-in-2-out & Dummy Padding):** Semua transaksi di pool memiliki bentuk topologi identik (2 input nullifiers, 2 output commitments, 2 memo 96-byte). Pengamat luar mustahil membedakan transfer merchant, belanja dengan kembalian, atau konsolidasi internal.

5. **Jaringan Relayer Tanpa Kustodi & Zero-Oracle (Stateless Sequencer - DEC-038C):**
   * **Pengalaman Belanja Gasless:** Pengguna tidak perlu memiliki saldo gas L2. Relayer menalangi gas on-chain dan menerima penggantian langsung dari nilai note (+ margin transparan 15%).
   * **Zero-Oracle Reimbursement (DEC-034C):** Pembayaran gas WETH dihitung murni dalam Gwei L2 aktual tanpa ketergantungan harga oracle eksternal.
   * **Pencegahan Spam Fail-Closed (Preflight Guard):** Relayer memverifikasi bukti Groth16 dan validitas calldata lokal sebelum transaksi disiarkan ke mempool L2.

6. **Dompet Mandiri Klien & In-Browser WASM Prover (Client SDK - DEC-041):**
   * **Kedaulatan 100% di Perangkat Pengguna:** Kunci rahasia tersimpan aman terenkripsi (Argon2id + ChaCha20-Poly1305) dan tidak pernah meninggalkan perangkat.
   * **In-Browser Web Worker Prover (DEC-041):** Pembuatan bukti matematika Groth16 dapat berjalan langsung di web browser / mobile klien menggunakan WebAssembly dengan siklus hidup memori terisolasi (`worker.terminate()`) guna mencegah OOM.
   * **Pemilihan Koin Cerdas (Stochastic Knapsack - DEC-030/036B):** Algoritma memilih kombinasi lembaran kas secara optimal guna memutus jejak pola nilai.

7. **Rel Pembayaran AI Agent Otonom (Pure ZK-UTXO HTTP 402 - DEC-042):**
   * Agen AI otonom dapat membayar kuota API sub-sen per panggilan via protokol HTTP 402 murni ZK-UTXO, dengan pelacakan pagu dinamis tanpa risiko pembocoran kunci dompet master.

---

## 5. Peta Masa Depan (North Star & Roadmap 2026–2030)

* **Rel Utama AI Agentic Economy:** Standar penyelesaian mikro-transaksi privat untuk agen cerdas (OpenAI, Claude, Gemini workflows, dan framework otonom).
* **Cross-Chain Shielded Settlement via Chainlink CCIP:** Perutean likuiditas privat lintas EVM dan L2 tanpa kebocoran graf transaksi.
* **Desentralisasi Relayer & Fair Sequencing:** Transisi relayer bootstrap menuju konsorsium stateless sequencer terdesentralisasi dengan perlindungan anti-MEV batching.
* **Oblivious Scanning & Zero-Leakage Sync:** Sinkronisasi note terenkripsi skala besar yang oblivious terhadap ISP maupun penyedia RPC.
* **Budaya Rekayasa:** **No Compromise. Zero Laziness. Fail-Closed.** Integritas matematika dan solvensi di atas segalanya.

