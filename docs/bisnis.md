# Model Bisnis & Arsitektur Ekonomi Zeltra Protocol

> **"The Stripe of Web3 with Absolute Privacy."**  
> *Engine: Pure ZK-UTXO on Arbitrum Stylus (Rust WASM + EIP-2537 BLS12-381)*  
> *Dual Sovereign Rails: USDC (Micro-USDC) & WETH / Native ETH (Gwei)*  
> *Mantra: Privasi 9.5/10, Produk 10/10 — Kecepatan Apple Pay / QRIS dengan Mental Kas Nyata ($10 - $3 = $7)*

---

## 1. Ringkasan Eksekutif

**Zeltra Protocol** adalah rel pembayaran dan penyelesaian privat universal (*universal shielded payment and settlement rail*) berkecepatan tinggi yang dibangun di atas **Arbitrum Stylus (WASM)**. Zeltra melayani transaksi tanpa diskriminasi untuk seluruh spektrum ekonomi: **manusia (retail & whale), autonomous AI agent, korporat B2B, dan merchant global**.

Zeltra bukan mixer pool statis, bukan protokol pinjaman berisiko (*anti-rehypothecation*), dan tidak menjual bot AI. Zeltra adalah **lapisan infrastruktur rel keuangan (financial settlement rail)** yang terdiri dari:
1. **Smart Contract Stylus WASM:** Mesin akuntansi solvensi multi-aset dengan verifikasi Groth16 instan via EIP-2537 (`0x0c` MSM + `0x0f` Pairing).
2. **Client SDK & In-Browser WASM Prover:** Dompet ZK-UTXO non-kustodian dengan siklus memori sekali pakai anti-OOM (`worker.terminate()`).
3. **Stateless Sequencer Relayer:** Jaringan relayer tanpa kustodi yang menalangi gas L2 di muka dengan penggantian *zero-oracle* (+15% markup).
4. **CEX Verifier Engine & Compliance Suite:** Alat audit mandiri untuk pembuktian asal dana bersih ke bursa terpusat (Binance/Coinbase) tanpa membocorkan spending key ($sk$) atau saldo total.

```text
       [Inflow: 0 bps]                                     [In-Pool Transfer: 0 bps (DEC-043)]
Pengguna / Funding Source (USDC / ETH)                Pengirim (zs1...) ------------> Penerima (zs1...)
            │                                                               │ (Merchant Masking)
            ▼ (Direct MMR Deposit - DEC-038B)                               ▼ (0 Token Keluar Kontrak)
┌────────────────────────────────────────────────────────────────────────────────────────┐
│                   ZELTRA SOVEREIGN PRIVACY POOL (ARBITRUM STYLUS)                      │
│                                                                                        │
│   • Dual Sovereign Rails : USDC Vault & WETH Vault (Siloed Solvency Invariant)         │
│   • Accumulator          : Dynamic Merkle Mountain Range (MMR) + 100 Ring Buffer Roots │
│   • State Machine        : Strict 3-Mode Universal JoinSplit (DEC-043B)                │
└────────────────────────────────────────────────────────────────────────────────────────┘
            │
            ├───────────────────────────────────────────────┐
            ▼ [Outflow: 45 bps (DEC-028)]                  ▼ [AI Agent Micropayments (DEC-042)]
Merchant / Dompet Publik (EVM / CCIP Rute)           HTTP 402 Facilitator (Sub-sen per call)
  + Change Note kembali ke MMR pengguna                + Dynamic Dust Threshold & Auto-settle
```

---

## 2. Nilai Utama & Solusi Masalah Pasar

| Masalah Finansial On-Chain Tradisional | Solusi Radikal Zeltra Protocol |
|---|---|
| **Surveillance & Doxxing Explorer:** Saldo, vendor penerima, gaji karyawan, dan riwayat dagang terekspos telanjang di explorer publik. | **Absolute Financial Confidentiality:** Pembayaran privat matematis via ZK-UTXO Private Note. Hubungan antara funding wallet dan pembayaran terputus 100%. |
| **Voucher Hell & Pecahan Kaku:** Mixer generasi lama memaksa pengguna memecah dana ke nominal tetap ($100, $1.000) yang kaku dan lambat. | **Mental Kas Nyata ($10 - $3 = $7):** Fleksibilitas nominal kas bebas. Belanja $3 dari lembar kas $10 menghasilkan kembalian privat $7 otomatis ke MMR (*Value Conservation*). |
| **Stigma Kriminalitas & Freeze CEX:** Pengguna privasi sering dibekukan sepihak oleh bursa (Binance, Coinbase) karena dianggap dana haram. | **CEX Selective Disclosure (DEC-040):** Pengguna jujur membuktikan kepemilikan dan jalur bersih dana ke CEX via Groth16 *Verifier Binding* tanpa membocorkan spending key atau saldo akun. |
| **Gas Barrier & Ketiadaan Rel AI:** Pengguna awam harus punya ETH untuk gas; AI agent tidak punya rel pembayaran sub-sen non-kustodian. | **Gasless UX & HTTP 402 Facilitator (DEC-042):** Relayer menalangi gas L2 di muka. AI Agent membayar API per panggilan via pure ZK-UTXO dengan delegasi session key lokal. |

---

## 3. Dual Sovereign Rails: USDC & WETH (DEC-034)

Zeltra beroperasi di atas **Dua Rel Aset Berdaulat**:
1. **USDC (Stablecoin Rail):** Basis hitungan Micro-USDC (6 desimal). Dirancang untuk perdagangan retail, checkout e-commerce, penggajian B2B, dan invoice merchant dunia nyata.
2. **WETH / Native ETH (Gas & Whale Rail):** Basis hitungan Gwei (9 desimal ter-normalisasi). Mendukung **payable native ETH auto-wrap** langsung di smart contract tanpa perlu transaksi terpisah `deposit()` WETH9.

### Solvensi Multi-Asset Terisolasi (Anti-Fractional Reserve & Anti-FTX)
Setiap aset memiliki akuntansi solvensi terisolasi mutlak (*siloed asset liability*) yang diverifikasi secara on-chain pada setiap perubahan state (`_check_asset_solvency_invariant`):

$$\text{Balance}(\text{asset}) \ge \text{user\_note\_liabilities}(\text{asset}) + \text{accrued\_execution\_fee\_liabilities}(\text{asset}) + \text{realized\_protocol\_fees}(\text{asset})$$

* **Nol Uang Gaib:** Tidak ada fractional reserve, tidak ada pinjaman spekulatif (*rehypothecation*). Satu dollar/ether masuk, tepat satu dollar/ether keluar.
* **Anti-Cross Contamination:** Kegagalan atau volatilitas pada satu aset tidak akan pernah dapat mengorbankan atau menguras saldo aset lainnya.

---

## 4. Alur Bisnis & Transaksi Inti (Pure ZK-UTXO)

### 4.1 Inflow: Zero-Friction Direct MMR Deposit (DEC-038B / DEC-034B)
* Pengguna mendepositokan USDC atau native ETH ke kontrak Zeltra di Arbitrum Stylus.
* Kontrak memvalidasi skalar kanonikal ($< r$), mengeksekusi transfer/wrap, menyisipkan daun komitmen `note_commitment` langsung ke pohon Merkle Mountain Range (`_mmr_insert`), dan memancarkan event `NoteCommitmentAppended` berisi memo terenkripsi 96-byte AEAD ChaCha20Poly1305.
* **Biaya Deposit:** **0,00% (0 bps permanen)**. Tidak ada komisi masuk, dana 100% utuh menjadi Private Note unspent seketika tanpa perantara kunci masking $k$ atau threshold guardian.

### 4.2 In-Pool Shielded Transfer & Merchant Masking (Mode 2 JoinSplit - DEC-043)
* Pengirim mentransfer dana langsung ke alamat privat penerima (Zeltra Shielded Address / ZSA Bech32m `zs1...`).
* **Biaya Protokol:** **0,00% (0 bps)** untuk memacu kecepatan sirkulasi likuiditas (*volume-driven velocity*).
* **Merchant Masking:** Kontrak membakar nullifier pengirim dan menyisipkan Output 1 (milik penerima) dan Output 2 (kembalian pengirim) ke MMR on-chain. **Nol token keluar dari kontrak**. Pengamat luar mustahil mengetahui siapa merchant yang dibayar dan berapa nominalnya.

### 4.3 Public Outflow Spend & Change Note (Mode 3 JoinSplit - DEC-028)
* Pengguna membayar invoice merchant publik atau menarik saldo ke dompet publik (misal: checkout merchant, withdraw ke exchange).
* Kontrak memverifikasi bukti Groth16 BLS12-381 via EIP-2537, membakar input nullifier, menyalurkan pembayaran publik ke alamat recipient, memotong **Protocol Fee flat 0,45% (45 bps)**, mengalokasikan fee eksekusi ke relayer, dan menyisipkan Change Note privat kembali ke MMR untuk pengirim.

### 4.4 Cross-Chain Settlement via Chainlink CCIP (Active Outflow & Roadmap Inflow - DEC-015 / DEC-019)
* **Cross-Chain Outflow Spend (Fitur Aktif):**
  - Pengguna memegang Private Note di Arbitrum Stylus dan ingin membayar merchant atau mengirim dana ke dompet publik di rantai lain (Ethereum, Base, Optimism, Polygon).
  - Pembuktian ZK Groth16 diverifikasi di Arbitrum Stylus (membakar nullifier & menyisipkan change note ke MMR pengirim). Relayer kemudian menyiarkan payload settlement lintas-rantai melalui **Chainlink CCIP Router** (`Router.ccipSend`).
  - Merchant di rantai tujuan menerima dana publik (USDC/ETH) secara instan.
  - Relayer background worker ([`ccip_monitor.rs`](file:///workspaces/Zeltra-Protocol/zeltra-node/src/ccip_monitor.rs)) memantau status finalitas eksekusi destination chain (`ExecutionStateChanged`) dan mengeksekusi proteksi refund otomatis jika terjadi kegagalan jaringan di rantai tujuan.
* **Cross-Chain Direct Inflow / Deposit (Roadmap Lanjutan):**
  - Memungkinkan pengguna mendepositokan dana dari rantai luar (Base, Ethereum, Polygon) langsung menjadi Private Note di Arbitrum tanpa perlu bridging manual terlebih dahulu.
  - Smart contract Stylus Zeltra telah memiliki modul [`_ccip_receive`](file:///workspaces/Zeltra-Protocol/zeltra-contracts/src/spend.rs) dengan proteksi anti-replay (`ccip_processed_messages`) dan sender allowlist (`ccip_allowed_senders`) yang siap menyisipkan komitmen deposit ke MMR saat infrastruktur deposit inbound diluncurkan.

### 4.5 Consolidation & Rollover (Mode 1 JoinSplit - DEC-033 / DEC-043B)
* Pengguna menggabungkan lembaran-lembaran kas kecil (*dust consolidation*) atau memperpanjang masa aktif note yang mendekati batas 2-epoch rollover.
* **Biaya Protokol:** **0,00% (0 bps)**.

---

## 5. Model Biaya & Sumber Monetisasi (Fee Architecture)

Zeltra mengadopsi model **konstanta matematis terjamin (*hardcoded immutable constants*)** guna menutup total celah eksploitasi tata kelola (*governance takeover*, flashloan attack):

| Aliran / Transaksi | Tarif Biaya | Penerima | Rationale & Kode Sumber |
|---|:---:|---|---|
| **Inflow (Deposit)** | **0,00% (0 bps)** | N/A | **Zero-Friction Inflow (DEC-038B / DEC-034B).** Dana masuk 100% utuh tanpa friksi psikologis. |
| **In-Pool Transfer (Mode 2)** | **0,00% (0 bps)** | N/A | **Volume-Driven Velocity (DEC-043B).** Memaksimalkan perputaran transaksi internal antar ZSA. |
| **Consolidation / Rollover (Mode 1)**| **0,00% (0 bps)** | N/A | Perapihan UTXO kas pengguna bebas biaya protokol. |
| **Public Outflow Spend (Mode 3)** | **0,45% (45 bps Flat)** | Protocol Treasury | **Unified Flat Outflow Fee (DEC-028).** Monetisasi saat dana ditarik kembali ke ekosistem publik. |
| **Relayer Execution Fee** | Gas aktual + 15% markup | Relayer Operator | **Zero-Oracle Gas Reimbursement (DEC-031 / DEC-034C).** Ditagih dalam unit native (Gwei untuk WETH, Micro-USDC untuk USDC). |
| **Chainlink CCIP Network Fee** | Biaya aktual jaringan CCIP (~$0.80) | Chainlink Network | **Zero-Markup Pass-Through (DEC-020 / DEC-031).** Pass-through 100% murni tanpa markup relayer (hanya pada transfer menyeberang rantai). |
| **B2B Institutional Query (Opsional)** | Biaya kuota API / SLA | Protocol Treasury | Akses verifikasi audit instan dan manajemen compliance batching untuk entitas berlisensi. |

---

## 6. Arsitektur AI Agent: HTTP 402 Facilitator (DEC-042)

Autonomous AI Agent bukan sekadar gimmick promosi; agen adalah **warga kelas satu (*first-class citizens*)** di Zeltra:
* **Kendala Agen Konvensional:** AI Agent tidak boleh memegang private key master wallet manusia, dan transaksi sub-sen ($0.002) haram disiarkan langsung ke mempool L2 karena akan memicu ledakan biaya gas dan *nullifier state bloat*.
* **Solusi Pure ZK-UTXO x402:**
  1. *Delegated Key:* Pemilik mendelegasikan note privat dengan plafon nominal tertentu ke agen.
  2. *Streaming Mikropembayaran:* Agen menerima tantangan `HTTP 402 Payment Required` dari penyedia API/LLM, lalu menghasilkan bukti ZK 1-in-1-out secara lokal melalui Zeltra SDK.
  3. *Dynamic Dust Threshold:* Pembayaran sub-sen diagregasi secara off-chain dan diselesaikan secara on-chain hanya saat mencapai ambang batas efisiensi gas.
  4. *Zero Master Wallet Exposure:* Kegagalan atau pembajakan logika pada AI agent hanya berisiko kehilangan saldo note yang didelegasikan, tanpa pernah membahayakan brankas utama pemilik.

---

## 7. Kelembagaan, Kepatuhan & CEX Selective Disclosure (DEC-040)

Zeltra memadukan privasi cypherpunk berdaulat dengan kebutuhan regulasi institusional melalui filosofi:
> **"Permissionless Privacy, Permissioned Acceptance" (DEC-026)**

### A. Hierarki Kunci 4-Tingkat (DEC-040A)
1. **Spending Key ($sk$):** Kunci belanja mutlak, 100% privat di perangkat pengguna.
2. **Nullifier Key ($nk$):** Kunci pemantau pembelanjaan note tanpa hak memindahkan saldo.
3. **Full Viewing Key ($fvk$):** Kunci audit menyeluruh untuk pembukuan akuntansi debit/kredit internal.
4. **Incoming/Outgoing Viewing Keys ($ivk$/$ovk$):** Kunci pemantau khusus penerimaan kas atau nota pengeluaran belanja.

### B. CEX Selective Disclosure Suite
Saat pengguna perlu menyetorkan dana ke bursa terpusat (Binance, Coinbase) atau diaudit oleh otoritas pajak:
* SDK membuat bukti `SelectiveDisclosureCircuit` Groth16 dengan **Verifier Binding** (mengikat alamat bursa & challenge nonce).
* Bursa memverifikasi bukti menggunakan **CEX Verifier Engine** mandiri.
* **Hasil:** Bursa mendapatkan kepastian matematis bahwa dana berasal dari deposit bersih dan dimiliki sah oleh penyetor, **TANPA** bursa dapat melihat spending key, tanpa melihat sisa saldo di privacy pool, dan tanpa melihat riwayat belanja pengguna lainnya.

---

## 8. Unit Economics & Proyeksi Finansial

Model bisnis Zeltra bertumpu pada **Velocity-Driven Monetization** (volume perputaran tinggi di atas TVL pasif):

```text
[Simulasi Tahunan Sederhana]
Deposit Volume (Inflow)          : $500.000.000 x 0,00% = $0 (Zero-Friction Liquidity Magnet)
In-Pool Velocity Volume (Mode 2) : $750.000.000 x 0,00% = $0 (Adopsi Merchant & Sirkulasi Kas)
Public Spend Volume (Mode 3)     : $300.000.000 x 0,45% = $1.350.000 (Gross Protocol Revenue)
                                                          ──────────────
Total Pendapatan Bersih Protokol                         : $1.350.000 / tahun
```

### Alokasi Arus Kas:
* **Protocol Treasury:** Menerima 100% dari 45 bps spend fee untuk membiayai audit keamanan independen, program bug bounty, server RPC/infrastruktur, dan insentif likuiditas.
* **Relayer:** Menerima 100% penggantian gas aktual ditambah markup 15% untuk menjamin keberlanjutan ekonomi operator relayer tanpa subsidi protokol.

---

## 9. Tiga Pilar Produk Komersial (Go-To-Market)

```text
┌────────────────────────────────────────────────────────────────────────────────────────┐
│                          3 PILAR PRODUK KOMERSIAL ZELTRA                               │
├───────────────────────────────┬───────────────────────────────┬────────────────────────┤
│ 1. Universal Shielded Checkout│ 2. HTTP 402 Agent Gateway     │ 3. Sovereign Shielded  │
│    "<ZeltraCheckout />"       │    "AI Agent Machine Rail"    │    RWA Gateway         │
└───────────────────────────────┴───────────────────────────────┴────────────────────────┘
```

1. **Universal Shielded Checkout (`<ZeltraCheckout />`):**
   * Komponen widget 1-baris kode bagi merchant e-commerce, dApp, dan kasir Web3.
   * Pembeli membayar dari saldo privat Zeltra di Arbitrum; merchant menerima pembayaran publik instan (USDC/ETH) di Arbitrum maupun rantai lain via Chainlink CCIP.
2. **HTTP 402 Facilitator (AI Economy Rail):**
   * Modul plug-and-play untuk penyedia server GPU (Akash/Render), kueri LLM, dan data scraping untuk menerima mikropembayaran instan dari agen AI otonom.
3. **Sovereign Shielded RWA Gateway (Opt-In Confidential Yield):**
   * Pintu gerbang opt-in bagi korporat B2B dan whale untuk menaruh kas menganggur ke instrumen RWA berbunga (seperti Ondo USDY / BlackRock BUIDL) di mana posisi kepemilikan dibungkus ke dalam komitmen Private Note ZK tanpa membocorkan neraca keuangan ke kompetitor.

---

## 10. Status Implementasi Riil & Roadmap 2026

Mengacu pada master checklist [`todo.md`](file:///workspaces/Zeltra-Protocol/todo.md) dan [`docs/roadmap.md`](file:///workspaces/Zeltra-Protocol/docs/roadmap.md), berikut status rekayasa saat ini:

### 🟢 Telah Selesai, Diaudit & Terverifikasi (100% Lulus)
- **Pure ZK-UTXO Architecture (DEC-038):** Seluruh warisan legacy BLS/guardian/voucher telah dimusnahkan 100%.
- **Dual Sovereign Rails (DEC-034):** USDC & WETH/Native ETH auto-wrap, multi-asset storage, dan zero-oracle gas reimbursement.
- **Direct MMR Inflow (DEC-038B):** Deposit langsung 250 ms ke pohon MMR on-chain dengan emisi memo 96-byte AEAD.
- **Shielded-to-Shielded In-Pool Transfer (DEC-043):** Strict 3-Mode JoinSplit state machine (Mode 2 transfer antar ZSA Bech32m lolos 44/44 test Python).
- **Cross-Chain CCIP Settlement Rail (DEC-015 / DEC-019):** Rel outflow belanja lintas-rantai (Arbitrum $\rightarrow$ Ethereum, Base, Polygon, OP) dengan auto-refund monitor ([`ccip_monitor.rs`](file:///workspaces/Zeltra-Protocol/zeltra-node/src/ccip_monitor.rs)) dan zero-markup fee pass-through.
- **Viewing Key Suite & CEX Selective Disclosure (DEC-040):** 4-tier key hierarchy dan engine verifikasi bursa independen.
- **In-Browser WebAssembly SDK Prover (DEC-041):** Runner Groth16 Web Worker anti-OOM linear (`worker.terminate()`).
- **Pure ZK-UTXO HTTP 402 Facilitator (DEC-042):** Mikropembayaran AI agent terverifikasi.
- **Surgical Codebase Architecture (DEC-044):** 4 pilar (SDK, Node, Contracts, Core) terdekopel rapi dengan nol God Files.

### 🟡 Prioritas Aktif Saat Ini & Roadmap
1. **Fase 2 — Circuit Freeze & Trusted Setup MPC:**
   * Kunci mati R1CS constraints 16 PI (`PrivateNoteCircuit`) & 19 PI (`JoinSplitCircuit`).
   * Eksekusi multi-party computation ceremony (Phase 2 Trusted Setup) untuk menghasilkan verifying keys resmi.
   * Audit keamanan independen Tier-1 smart contract Stylus & sirkuit Groth16.
2. **Fase 3 — Sanitasi Kredensial:**
   * Rotasi private key dev historis dan pembersihan git commit history.
3. **Fase 4 — Unified Hard-Test Arbitrum Sepolia (Gladi Bersih Akhir):**
   * Deploy kontrak dan relayer **sekali saja** menggunakan final verifying keys resmi hasil MPC.
   * Pengujian end-to-end full lifecycle matrix (deposit $\rightarrow$ in-pool transfer $\rightarrow$ spend $\rightarrow$ withdraw-all) serta boundary crash recovery.
4. **Fase 5 — Mainnet Arbitrum One Launch & Cross-Chain Inbound (Roadmap):**
   * Go-live produksi, peluncuran Bug Bounty publik, dan integrasi merchant checkout widget `<ZeltraCheckout />`.
   * **Cross-Chain Inbound CCIP Deposit (Roadmap):** Pembentukan Private Note langsung di Arbitrum Stylus saat pengguna mendepositokan token di jaringan Base, Ethereum Mainnet, atau Polygon tanpa bridging manual.
