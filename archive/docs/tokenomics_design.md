# Nimbus Protocol: Desain Game Kripto-Ekonomi ($NIMB)

Dokumen ini menjelaskan strategi kripto-ekonomi dan tokenomics **Nimbus Protocol** agar dirancang secara berkelanjutan, meminimalkan modal awal pendiri (*Founders*), dan memaksimalkan nilai tangkapan (*value capture*) agar kita memenangkan persaingan di pasar Web3.

---

## 1. Win-Condition (Kondisi Kemenangan Kita)
Agar protokol dan pendiri memenangkan "game ekonomi" ini, tokenomics harus menyelesaikan 3 masalah utama Web3:
1.  **Dumping Airdrop**: Mencegah pemburu airdrop membanting harga koin hingga $0 saat listing.
2.  **Solver Staking Utility**: Memaksa para operator node (Relayer/Guardian) untuk terus membeli dan menahan token $NIMB secara on-chain.
3.  **Real Yield Revenue**: Memberikan nilai kegunaan riil (*utility*) pada token $NIMB berupa dividen stablecoin (USDC) dari yield riil DeFi/RWA, bukan sekadar token inflasi tanpa nilai.

---

### 2. Arsitektur Sirkulasi & Utilitas Token ($NIMB)

```mermaid
flowchart TD
    User[User / AI Agent] -->|Bayar Fee Spend 45 bps USDC| Contract[Nimbus L2 Contract]
    Contract -->|100% Pokok Likuid| LPBuffer[Liquidity Buffer - Full Reserve]
    
    Contract -->|Akumulasi Protocol Fee 45 bps USDC| Treasury[Protocol Treasury / Fee Splitter]
    
    Treasury -->|50% Buyback & Burn (22.5 bps)| DEX[DEX Uniswap: Buy & Burn $NIMB]
    Treasury -->|30% Real Yield Dividen (13.5 bps)| SafetyStaking[Safety Module Staking - Dividen USDC]
    Treasury -->|20% Kas Pengembang (9.0 bps)| Founders[Wallet Founders - USDC Cashflow]

    SafetyStaking -->|Staker Kunci $NIMB| SecureShield[Backstop Risiko Jaringan]
    
    Relayers[Relayers & Guardians] -->|Wajib Stake $NIMB| SlashingContract[Staking & Slashing Contract]
    SlashingContract -->|Jika Malicious / Offline| SafetyStaking
```

### A. Syarat Staking Relayer & Guardian (Kunci Utama Buy-Demand)
*   **Mekanisme**: Setiap Relayer (Leader) dan Guardian yang ingin memproses transaksi Nimbus dan menikmati keuntungan komisi eksekusi (*gas reimbursement + 15% markup*) **wajib membeli dan melakukan staking sejumlah token $NIMB** sebagai jaminan (bond).
*   **Slashing**: Jika Relayer/Guardian terbukti bersalah (misal: menolak merilis masking key $k$, mencoba double-spend, atau offline berkepanjangan), token $NIMB yang mereka stake akan di-**slash** (disita) secara otomatis oleh smart contract.
*   **Dampak Ekonomi**: Semakin tinggi volume transaksi belanja AI agent di Nimbus, semakin banyak Relayer yang ingin bergabung demi mendapatkan profit operasional. Hal ini memaksa pembelian massal token $NIMB secara terus-menerus di pasar terbuka (DEX) untuk dijadikan jaminan staking.

### B. Safety Module Staking (Dividen Real Yield USDC)
*   **Mekanisme**: Pemegang token $NIMB (termasuk retail, investor komunitas, dan founder) dapat mengunci koin mereka ke dalam **Safety Module Vault**.
*   **Fungsi**: Kunci staking ini berfungsi sebagai *backstop* (asuransi sistem) jika terjadi kegagalan CCIP lintas rantai atau anomali brankas likuiditas.
*   **Reward (Real Yield USDC)**: Sebagai kompensasi atas risiko ini, staker mendapatkan **pembagian keuntungan riil dalam bentuk USDC murni** (30% dari total protocol fee spend, yaitu setara 13,5 bps dari volume transaksi). Ini bukan token inflasi baru, melainkan dividen dolar tunai!
*   **Dampak Ekonomi**: Menawarkan dividen USDC membuat pemegang koin enggan menjual token $NIMB mereka. Mereka lebih memilih mengunci token demi mendapatkan passive income USDC pasif yang stabil setiap minggu.

> [!NOTE]
> **Status Implementasi (Fase Lanjutan - Ditunda hingga TGE Phase 3)**:
> Mekanisme staking Safety Module dan distribusi yield USDC ini **ditangguhkan sementara** pada rilis awal Nimbus V1 & V2 (Fase Bootstrap). Karena token $NIMB belum diluncurkan, logic distribusi ke staker belum diaktifkan di smart contract awal. Seluruh biaya transaksi yang dikumpulkan oleh kontrak (45 bps) untuk sementara dialokasikan penuh ke kas protokol (Treasury/Founder wallet) untuk membiayai operasional, pengembangan sirkuit ZK, dan audit keamanan.

### C. Protokol Deflasi: Automated Buyback & Burn
*   Secara berkala, smart contract Treasury menggunakan **50% dari total protocol fee USDC** (setara 22,5 bps dari volume transaksi spend) untuk membeli kembali (*buyback*) token $NIMB dari Uniswap liquidity pool.
*   Token $NIMB hasil buyback tersebut kemudian langsung dibakar secara permanen (*burn*) ke alamat `0x000...dead`.
*   **Dampak Ekonomi**: Pasokan token $NIMB di sirkulasi akan terus berkurang secara matematis seiring berjalannya waktu, mendorong kenaikan harga secara organik seiring pertumbuhan volume transaksi platform.

### D. Diskon Fee Transaksi bagi Holder Token $NIMB
*   **Mekanisme**: Pengguna, merchant, atau AI agent wallet yang menahan sejumlah minimum token $NIMB di dompetnya berhak atas diskon biaya transaksi:
    * Non-holder: Standard fee **45 bps** (0,45%).
    * Tier 1 Holder (Hold ≥ 1.000 $NIMB): Diskon 5 bps $\to$ **40 bps** (0,40%).
    * Tier 2 Whale/Merchant (Hold ≥ 10.000 $NIMB): Diskon 10 bps $\to$ **35 bps** (0,35%).
*   **Dampak Ekonomi**: Memberikan utilitas langsung bagi developer AI agent dan integrator pembayaran untuk terus membeli dan menahan token $NIMB di dalam dompet agen mereka.

### E. Real-Yield Value Capture Split (Formula 50 / 30 / 20)

Dari setiap transaksi belanja privat yang dikenai biaya protokol sebesar **45 bps (0,45%)**:
* **50% (22,5 bps):** Dialokasikan untuk **Buyback & Burn** otomatis di DEX demi apresiasi nilai token $NIMB.
* **30% (13,5 bps):** Dialokasikan sebagai **Real Yield Dividen USDC** bagi staker Safety Module.
* **20% (9,0 bps):** Dialokasikan ke **Founder / Core Dev Treasury (USDC)** untuk mendanai riset, server, dan operasional berkelanjutan tim pendiri.

### F. Full Reserve (100% Liquid) Model

Untuk menjamin keamanan modal depositor dan menghemat limit ukuran WASM kontrak pintar pada Arbitrum Stylus, Nimbus menerapkan model **100% Liquid Reserve (Full Reserve)**:
1. **0% Bank Run Risk**: Karena 100% dana pendukung tersimpan secara likuid di dalam kontrak pintar utama, tidak ada risiko kegagalan likuiditas atau forced liquidation. Antrean penarikan (*withdrawal queue*) dan penalty darurat dihapus demi mengoptimalkan kecepatan eksekusi.
2. **Deposit 0% (Zero-Friction Inflow)**: Pengguna menyetor 100 USDC mendapatkan 100 USDC note utuh tanpa potongan, mempercepat pertumbuhan likuiditas dan anonymity set privacy pool.
3. **Peningkatan Gas Efficiency**: Tanpa perlu melakukan pemantauan oracle harga atau rebalancing aset eksternal on-chain, biaya gas operasional terpangkas hingga 95% dibandingkan model vault dinamis.

### G. Circuit-Breaker Mechanism (Security Hardening 2026)
Sebagai antisipasi terhadap volatilitas ekstrem dan serangan manipulasi pasar, Nimbus menerapkan **Circuit-Breaker Mechanism** dengan kontrol adaptif:
1.  **Mint Cap per Epoch**: Batas maksimum token yang bisa dicetak per epoch (24 jam) untuk mencegah minting berlebihan saat anomali.
    *   **Default**: 0.5% dari total supply per epoch
    *   **Dynamic Adjustment**: Menurun saat high demand, naik saat low demand
    *   **Circuit-Breaker**: Halt minting jika volatilitas harga > 50% dalam 1 epoch
2.  **Burn Circuit-Breaker**: Halt burn mechanism jika:
    *   Harga token drop > 30% dalam 24 jam
    *   Liquidity pool depth < minimum threshold
    *   Oracle manipulation terdeteksi
3.  **Emergency Pause**: Governance dapat pause seluruh minting/burning dalam 1 block jika terdeteksi exploit critical
4.  **Reference**: Mengikuti pendekatan Akash Network untuk BME dengan circuit-breaker dan collateral controls

### H. Oracle Security & Price Feed Protection
Untuk mencegah oracle manipulation attacks (CertiK Oracle Wars 2026):
1.  **Multi-Source Oracles**: Menggunakan minimal 3 oracle sources berbeda:
    *   Chainlink Price Feeds
    *   Band Protocol
    *   Pyth Network
2.  **Time-Weighted Average Price (TWAP)**: Menggunakan TWAP 1 jam untuk price stabilization
3.  **Deviation Threshold**: Reject oracle updates jika deviasi > 5% dari rata-rata sources
4.  **Oracle Rotation**: Automatic rotation jika oracle source gagal memberikan update
5.  **On-Chain Verification**: Smart contract verify signature dari oracle providers

### I. MEV Protection Mechanisms
Untuk mencegah MEV extraction sekitar epoch minting:
1.  **Randomized Epoch Timing**: Epoch boundaries tidak predictable (± 15 menit random)
2.  **Commit-Reveal Scheme**: Minting menggunakan 2-step commit-reveal untuk mencegah front-running
3.  **Batch Processing**: Process minting dalam batch untuk mencegah sandwich attacks
4.  **Private Mempool**: Menggunakan private mempool untuk critical transactions
5.  **MEV-Resistant Design**: Calldata encoding yang tidak expose sensitive parameters

### J. Parameter Optimization Ranges (2026 Best Practices)
Berdasarkan simulation dan empirical data dari protocols sukses:
1.  **Burn Fraction (β)**: 0.15 - 0.25 (NodeOps suggests start at 0.20)
2.  **Mint Cap per Epoch**: 0.3% - 0.7% dari total supply
3.  **Target Stake Percentage**: 25% - 35% dari total supply (Polygon targets ~30%)
4.  **Smoothing Window**: 3 - 7 epochs untuk net emissions
5.  **Volatility Threshold**: 40% - 60% untuk circuit-breaker trigger
6.  **Cash Buffer Range**: 15% - 45% dari TVL (dynamic adjustment)
7.  **Rebalancing Band**: ±5% dari target allocation

### K. Simulation Framework & Testing Strategy (2026 Standard)
Untuk memvalidasi dan mengoptimalkan parameter tokenomics sebelum mainnet launch:
1.  **Simulation Tools**:
    *   **cadCAD**: Monte-Carlo dan agent-based simulation untuk scenario testing
    *   **Cenit Finance**: Tokenomics simulator dengan template BME
    *   **Token Terminal**: On-chain metrics untuk calibration
    *   **Bitquery**: Blockchain API untuk historical data
2.  **Sensitivity Analysis**:
    *   **Sobol Decomposition**: Variance-based analysis untuk atribusi output variance ke inputs
    *   **Parameters to test**: Burn fraction β, mint cap, staking reward g, target stake fraction
3.  **Experiment Matrix**:
    *   **Demand Scenarios**: Low/Medium/High growth (γ)
    *   **Burn Fractions**: Conservative (0.15), Baseline (0.20), Aggressive (0.25)
    *   **Mint Cap Policies**: Fixed, Smoothing, Circuit-breaker
    *   **Runs**: 10,000 Monte-Carlo runs per scenario
4.  **KPIs to Monitor**:
    *   Net issuance rate (mint - burn) per epoch
    *   APR to stakers vs effective inflation rate
    *   Runway (time until treasury depleted)
    *   TVL sensitivity to reward rate changes
    *   Revenue capture ratio: on-chain revenue / total compensation
5.  **Validation Targets**:
    *   Net deflationary saat high demand (burn > mint)
    *   Sustainable APR untuk stakers (5-15% target)
    *   Minimum 12 months runway di low demand scenario
    *   Revenue capture ratio > 50%


---
## 3. Distribusi Cap Table & Rencana Penguncian (Vesting)

Untuk menjamin kelangsungan hidup proyek dan meyakinkan investor besar, pembagian 100.000.000 token $NIMB diatur secara ketat dengan kontrak vesting otomatis:

| Alokasi | Porsi (%) | Jumlah Token | Cliff (Jeda Penguncian) | Vesting (Pencairan) | Fungsi Ekonomi |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **Founders & Team** | 30% | 30.000.000 | 12 Bulan | 24 Bulan Linier | Insentif jangka panjang bagi pendiri agar fokus membangun produk. |
| **Seed Investors** | 20% | 20.000.000 | 6 Bulan | 18 Bulan Linier | Memberikan kepastian ROI bagi investor awal dengan rilis bertahap. |
| **Community Airdrop** | 10% | 10.000.000 | 0 Bulan (TGE) | 4 Bulan Bertahap | Menarik user awal. Pembagian bertahap mencegah penjualan massal (*dump*) instan. |
| **Safety & Node Rewards**| 15% | 15.000.000 | 0 Bulan | Emisi Linier (4 tahun)| Hadiah untuk relayer node dan staker awal Safety Module. |
| **Treasury & LP Incentives**| 15% | 15.000.000 | 0 Bulan | Sesuai Kebutuhan | Cadangan likuiditas Uniswap dan kerja sama ekosistem. |
| **Public Sale / IDO** | 10% | 10.000.000 | 0 Bulan (TGE) | 100% Unlocked | Likuiditas awal pasar saat listing di DEX. |

---

## 4. Strategi Go-To-Market & Penggalangan Dana

Bagaimana kita mengamankan modal dan membangun proyek ini dari nol tanpa boncos secara finansial?

```mermaid
gantt
    title Peta Jalan Pendanaan & Rilis Ekonomi Nimbus
    dateFormat  YYYY-MM
    section Fase 1: Bootstrap & Point
    Rilis Nimbus V1 di Testnet/Mainnet      :active, f1_1, 2026-06, 30d
    Peluncuran Off-chain Point System       :active, f1_2, 2026-06, 90d
    section Fase 2: Investasi SAFTE
    Penggalangan Dana Benih ($250k)         :f2_1, 2026-08, 60d
    Audit Keamanan ZK & Smart Contract     :f2_2, 2026-09, 30d
    section Fase 3: Peluncuran & TGE
    Pembuatan Uniswap LP & Listing $NIMB   :f3_1, 2026-10, 15d
    Konversi Poin Loyalitas & Airdrop       :f3_2, 2026-10, 30d
```

### Fase 1: Bootstrap Tanpa Modal (Poin Loyalitas)
1.  Luncurkan Nimbus V1 menggunakan stablecoin USDC murni.
2.  Buka program **Nimbus Points** untuk pengguna yang bertransaksi atau mendepositkan dana ke Brankas Dinamis.
3.  Poin ini dihitung secara off-chain (gratis biaya database). Ini mengumpulkan data pasar asli (*Product-Market Fit*) tanpa mengeluarkan uang sepeser pun untuk tokenomics.

### Fase 2: Penggalangan Dana Investor ($250k) via SAFTE
1.  Gunakan data transaksi nyata dan volume points dari Fase 1 sebagai bahan presentasi (*pitch deck*) ke VC (Venture Capital) atau Angel Investor.
2.  Tawarkan investasi **$250,000** menggunakan dokumen hukum **SAFTE (Simple Agreement for Future Tokens)**.
3.  Investor menyetorkan cash/USDC, dan mereka berhak mendapatkan 20% alokasi token $NIMB saat rilis resmi nanti, dengan penguncian vesting ketat (Cliff 6 bulan, 18 bulan linier).

### Fase 3: Peluncuran Token & Likuidasi
1.  Dari dana $250k investor, gunakan **$50,000 USDC** untuk dipasangkan dengan **5.000.000 $NIMB** (5% alokasi publik) ke Uniswap Liquidity Pool.
2.  Gunakan **$25,000** untuk mengaudit sirkuit ZK dan smart contract.
3.  Sisa **$175,000 USDC** disimpan di kas sebagai modal Anda untuk operasional dan gaji tim bulanan.
4.  Lakukan airdrop token secara berkala kepada pemegang Nimbus Points sebagai bentuk hadiah loyalitas.
5.  Karena likuiditas pool Uniswap dalam ($50k) dan airdrop dibagikan bertahap, harga koin $NIMB akan stabil dan cenderung naik seiring buyback otomatis dari yield kas protokol.
