# ZELTRA PROTOCOL MANIFESTO (VISION & MISSION)

> **"The Stripe of Web3 with Absolute Privacy."**  
> *Internal Engine Code: Nimbus | Arbitrum Stylus Rust WASM*

---

## 1. Identitas & DNA Protokol

* **Nama Resmi:** **Zeltra Protocol** *(secara teknis di codebase transisi bernama Nimbus)*.
* **Kategori:** *Shielded Global Payment & Settlement Rail*.
* **Visi Inti:** Menjadi infrastruktur pembayaran universal di era Web3 dan Otonom AI, di mana aliran modal bergerak secepat Apple Pay / QRIS, namun dengan perlindungan privasi matematis yang absolut.
* **Mantra Produk:** **Privasi 9.5/10, Produk 10/10.** Privasi tidak boleh mengorbankan User Experience. Tidak ada diskriminasi: Zeltra dibangun setara untuk AI Agent, retail, whale, korporat B2B, dan merchant global.

---

## 2. Masalah yang Kami Hancurkan

1. **Surveillance & Doxxing Finansial:** Di blockchain publik, seluruh saldo dan riwayat pengeluaran telanjang. Setiap transaksi membuka celah eksploitasi, front-running MEV, dan hilangnya kedaulatan individu.
2. **Privasi yang Lambat, Mahal & Rumit:** Solusi privasi generasi pertama (Tornado, Monero, dsb.) lambat, boros gas, dan membuat user awam pusing dengan konsep UX yang rumit.
3. **The "Voucher Hell" Mental Model:** Pengguna tidak berpikir dalam bentuk "voucher nominal kaku". Di dunia nyata, jika seseorang punya saldo $10 dan membeli kopi $3, mereka menuntut kembalian $7 secara instan dan mulus.

---

## 3. Pilar Filosofis & Prinsip Bisnis

### A. Zero-Friction Inflow (Deposit 0%)
* Masuk ke Zeltra tidak boleh dipalak. Deposit fee dipatok **0 bps (0.00%) permanen**.
* User tidak boleh merasa takut dananya dipotong admin saat baru melangkah masuk.

### B. Volume-Driven Over TVL (Kecepatan Perputaran Uang)
* Zeltra bukan protokol serakah yang menarik biaya diam (idle fee) ala bank konvensional.
* Keberlanjutan ekonomi protokol bertumpu pada **velocity of money (volume harian)** melalui fee spend yang sangat kompetitif (40–45 bps) dan relayer execution gas reimbursement.

### C. Mathematical Solvency (Anti-Fractional Reserve)
* **Invariant Mutlak:** `Assets On-Chain >= Total Liabilities`.
* Tidak ada uang gaib, tidak ada rehypothecation. 1 USDC yang masuk tercatat sebagai liabilitas nyata yang dijamin 100% oleh smart contract hingga terjadi spend atau valid refund.

### D. Etika & Kepatuhan: *"Permissionless Privacy, Permissioned Acceptance"*
* Zeltra membedakan privasi kedaulatan dengan kejahatan pencucian uang (Ref: [`DEC-026`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-026-receiver-enforced-compliance-zero-cost-sanctions-filtering-relayer-protection.md)).
* **Bukan Kebijakan Global:** Bukan protokol yang menentukan *"apakah uang ini bersih?"*, melainkan pihak penerima dana/merchant yang menentukan: *"bersih menurut standar siapa?"*
* **Low-Cost Public Ingress Screening:** Protokol tidak membebankan diri membayar oracle kepatuhan komersial mahal ($30k–$100k/thn). Penyaringan sanksi dilakukan secara efisien menggunakan data publik resmi (OFAC SDN List).
* **Mitigasi Risiko Relayer (Dumb Pipe):** Relayer beroperasi tanpa kustodi dan tanpa pengetahuan rahasia (*zero custodial & knowledge exposure*). Relayer hanya memproses Groth16 proof matang dengan filter sanksi publik aktif di gerbang HTTP. Pengguna sah (retail, AI agent, whale) menikmati privasi penuh, sementara aktor jahat terisolasi dari merchant legal.

---

## 4. Keunggulan Arsitektur & Rekayasa

1. **Arbitrum Stylus (Rust WASM):** Eksekusi sub-detik dengan efisiensi komputasi mendekati native. Perlindungan memory-safety Rust menghapus exploit buffer-overflow dan bug pointer.
2. **Dual-Engine Synergistic (BLS + Groth16):**
   * *Inflow Kilat (BLS Threshold 3-of-5):* Penerbitan note instan tanpa latensi pembuktian berat di sisi user.
   * *Outflow Shielded (ZK-UTXO Groth16 via EIP-2537):* Nilai kembalian fleksibel (*change note*), penyembunyian graf transaksi dengan sirkuit LeanIMT depth 20.
3. **Strict Client-Side Self-Custody:**
   * Pembuktian ZK Groth16 wajib di-generate secara lokal di SDK client (browser/device user).
   * Rahasia note dan spending key **tidak pernah menyentuh relayer**. Relayer hanya memvalidasi public input dan meneruskan transaksi ke blockchain.
4. **Resilient Settlement Pipeline:**
   * Fail-closed validation di semua perimeter.
   * Perlindungan reorg, idempotensi 24 jam, dan replacement gas-bump otomatis.

---

## 5. Peta Masa Depan (Roadmap & North Star)

* **Skala 2026–2030:** Menjadi payment gateway privat universal untuk AI Agent otonom (OpenAI, Gemini, Claude, platform Agentic), integrasi likuiditas DeFi (Uniswap, Polymarket), dan settlement lintas rantai via Chainlink CCIP.
* **Desentralisasi Relayer & Tokenomics:** Mentransisikan relayer dari status bootstrap terpusat menuju jaringan relayer terdesentralisasi berbasis *staking game theory* yang tahan sensor.
* **Tata Kelola:** Menuju DAO minimalis yang defensif dan terlindungi dari eksploitasi tata kelola (governance attack).
* **Engineering Culture:** **No Compromise. Zero Laziness. Fail-Closed.** Tidak ada jalan pintas dalam matematika dan keamanan dana.
