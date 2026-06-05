# Desain Rebalancing Brankas Kripto-Ekonomi: Analisis Statistik & Gas Optimization

Dokumen ini menyajikan riset dan usulan desain **Statistical & Threshold-Based Vault Rebalancing** untuk Nimbus Protocol. Desain ini bertujuan untuk menyelesaikan masalah "kebocoran gas" (*gas bleeding*) akibat pemotongan dan rebalancing rasio brankas bertingkat (30/50/20) yang dilakukan secara konstan pada setiap transaksi.

---

## 1. Masalah Kritis: Kebocoran Gas On-Chain (*Gas Bleeding*)

Pada arsitektur brankas saat ini (`lib.rs`), setiap kali fungsi `deposit()` dipanggil:
* Kontrak memotong biaya minting.
* Kontrak menghitung pembagian setoran: 30% Kas, 50% Aave, 20% Ondo RWA.
* Kontrak melakukan operasi on-chain:
  1. `USDC.approve(AavePool, amount_50)` (~22,000 gas)
  2. `AavePool.supply(USDC, amount_50, ...)` (~150,000 gas)
  3. `USDC.approve(OndoRwa, amount_20)` (~22,000 gas)
  4. `OndoRwa.deposit(amount_20)` (~200,000 gas)

**Total Overhead Gas per Deposit: ~394,000 gas.**
Pada jaringan Arbitrum, meskipun biaya gas per L2 execution murah, membebankan tambahan ~400k gas pada setiap transaksi pengguna (terutama untuk transaksi retail kecil, misal $10 - $100) akan menguras margin keuntungan mereka.

Begitu pula saat penarikan/spend: jika nominal penarikan whale melebihi kas likuid 30%, kontrak secara otomatis menarik dana dari Aave/Ondo secara berurutan (*Cascading Buffer*). Ini memicu biaya gas penarikan RWA/Aave yang sangat mahal (~200k - 300k gas) yang dibebankan kepada transaksi penarikan tersebut.

---

## 2. Solusi 1: Threshold-Based Rebalancing (Hysteresis Bands)

Dibandingkan melakukan rebalancing secara kaku pada setiap transaksi, kontrak hanya akan memicu rebalancing jika rasio kas likuid menyimpang melewati batas toleransi tertentu (**Hysteresis Band** atau **Rebalancing Window**).

### Parameter Mekanisme:
* **Target Kas Likuid ($\beta_{\text{cash}}$)**: $30\%$ dari TVL.
* **Batas Toleransi Rebalancing ($\theta$)**: $\pm 5\%$ (yaitu batas bawah $25\%$, batas atas $35\%$).

```text
Kas Likuid (%) 
  ▲
  │   [Batas Atas: 35%] ───► Tarik kas berlebih, supply ke Aave/RWA
  │
  │   [Target: 30%]     
  │
  │   [Batas Bawah: 25%] ──► Tarik dari Aave/RWA ke kas likuid
  ▼
```

### Aturan Eksekusi:
1. **Saat Deposit**: Setoran USDC baru disimpan sebagai Kas di kontrak. Tidak ada transaksi eksternal ke Aave/Ondo.
   * *Rasio Kas naik*. Jika Rasio Kas melebihi $35\%$, kontrak secara otomatis memicu fungsi rebalancing internal untuk mengirimkan kelebihan kas tersebut ke Aave ($50\%$) dan Ondo RWA ($20\%$), mengembalikan Rasio Kas ke $30\%$.
2. **Saat Penarikan**: Dana diambil dari kas internal.
   * *Rasio Kas turun*. Selama Rasio Kas di atas $25\%$, tidak ada penarikan dari Aave/RWA. Jika Rasio Kas turun di bawah $25\%$, kontrak secara otomatis menarik dana dari Aave/RWA untuk mengembalikan cadangan kas ke $30\%$.

**Dampak**: Menghilangkan biaya gas rebalancing untuk **>85% transaksi retail biasa**, karena rebalancing hanya dipicu sesekali saat akumulasi volume mencapai ambang batas.

---

## 3. Solusi 2: Adaptive Cash Reserve (Skala Volatilitas Statistik)

Untuk menghadapi whale (penarikan besar mendadak), kita menggunakan statistik volume transaksi historis untuk menyesuaikan target kas likuid ($\beta_{\text{cash}}$) secara dinamis.

Menggunakan prinsip **Value at Risk (VaR)** sederhana atau standar deviasi volume harian ($\sigma_{\text{volume}}$) selama $N$ hari terakhir:

$$\beta_{\text{cash}} = \text{clamp}\left( 20\% + k \cdot \sigma_{\text{volume}}, \; 15\%, \; 60\% \right)$$

* Di mana $k$ adalah faktor pengali keamanan (confidence interval, misal $k = 1.96$ untuk tingkat kepercayaan $95\%$).
* **Ketika Volatilitas Rendah (Predictable)**: Target kas likuid diturunkan hingga $15\%$ (sisanya dialokasikan ke DeFi/RWA untuk memaksimalkan APY yield).
* **Ketika Volatilitas Tinggi (Whale Season)**: Target kas likuid dinaikkan secara otomatis hingga $50\% - 60\%$. Cadangan kas yang tebal ini memastikan transaksi penarikan besar oleh whale dapat ditangani langsung oleh kas internal tanpa memicu biaya gas penarikan Aave/RWA.

---

## 4. Solusi 3: Asynchronous Batch Rebalancing (Keeper Model)

Alternatif terbaik untuk menghilangkan seluruh overhead gas rebalancing dari pengguna adalah **Asynchronous Batch Rebalancing**:
1. Seluruh transaksi deposit/penarikan pengguna hanya berinteraksi dengan kas internal kontrak Nimbus (sangat murah, hanya transfer ERC20 biasa).
2. Kita menyediakan fungsi eksternal `rebalance()` yang hanya bisa dipanggil oleh bot otomatis (**Keeper** seperti Chainlink Automation) atau admin protokol secara berkala (misal setiap 24 jam sekali atau setiap 100 block).
3. Keeper menanggung biaya gas rebalancing secara terpusat, dibiayai menggunakan sebagian kecil dari akumulasi yield bunga USDC brankas.

```text
User Deposits (USDC) ──► [ Nimbus Contract Cash ] ◄── User Withdrawals (USDC)
                                ▲   │
               24-Hour Keeper   │   │  (30/50/20 Allocation)
              rebalance() call  │   ▼
                        [ Aave V3 / RWA Vaults ]
```

---

## 5. Estimasi Penghematan Biaya Operasional (Proyeksi 2026)

Asumsi volume harian: 1,000 transaksi deposit rata-rata $100 USDC.
Gas Price Arbitrum: $0.1$ Gwei.

| Skenario | Rata-rata Gas per Transaksi | Biaya Gas Harian (ETH) | Total Biaya Gas Tahunan | Efisiensi |
| :--- | :--- | :--- | :--- | :--- |
| **Sistem Lama** (Rebalance Instan) | ~394,000 gas | ~0.0394 ETH | ~14.38 ETH (~$50,000) | Baseline |
| **Hysteresis Band ($\pm 5\%$)** | ~40,000 gas (amortisasi) | ~0.0040 ETH | ~1.46 ETH (~$5,000) | **90% Hemat** |
| **Batch Rebalancing** (24 jam) | ~1,500 gas (amortisasi) | ~0.00015 ETH | ~0.05 ETH (~$200) | **99.6% Hemat** |

---

## 6. Rekomendasi Rencana Aksi

Untuk fase pengembangan testnet dan mainnet Nimbus V1 mendatang, kita merekomendasikan:
1.  **Fase 1 (✅ Selesai)**: Menjaga logic cascading rebalancing on-demand tetap menyala untuk menjamin keamanan likuiditas mentah.
2.  **Fase 2 (✅ Selesai)**: Mengimplementasikan **7-Epoch Moving Average Volume** di dalam kontrak (`update_epoch_and_rebalance_ratio`). Kontrak secara dinamis mengubah `target_cash_pct` (15% / 30% / 45%) berdasarkan analisis volume 7 epoch terakhir. Fungsi `allocate_reserves()` sekarang membagi non-cash portion 5:2 antara Aave dan RWA secara otomatis berdasarkan `target_cash_pct` terkini. Hook dipasang di `deposit()` dan `spend()`.
3.  **Fase 3 (Berikutnya)**: Mengimplementasikan fungsi `rebalance()` berbasis Keeper (Chainlink Automation) dan mengalihkan seluruh pemanggilan Aave/Ondo keluar dari method `deposit()` dan `spend()` untuk meminimalkan gas pengguna ke titik terendah (Batch Rebalancing 24 jam).
