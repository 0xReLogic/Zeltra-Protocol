# Deposit Fee vs Zero-Fee Inflow Policy Evaluation

**Priority:** Tier 1 (Product & Tokenomics / Mainnet Readiness)  
**Status:** Completed & Adopted (Zero-Deposit & Fixed Mathematical Constants)  
**Reference:** [`docs/bisnis.md`](file:///workspaces/Zeltra-Protocol/docs/bisnis.md) §5, [`todo.md`](file:///workspaces/Zeltra-Protocol/todo.md) Section 2 (Fee & Events)

---

## 1. Konteks & Dilema Arsitektur

Saat ini smart contract Nimbus menerapkan potongan biaya deposit sebesar **0,20% (20 bps)** pada entrypoint `_deposit()`:
$$\text{net\_amount} = \text{gross\_deposit} - \left\lceil \frac{\text{gross\_deposit} \times 20}{10000} \right\rceil$$

### Dilema Utama:
* **Ekspektasi Pengguna (UX):** Saat menyetor 100 USDC, pengguna berekspektasi saldonya tetap 100 USDC utuh. Pemotongan langsung di awal menimbulkan *friction of entry* (keengganan psikologis menyetor dana).
* **Anonimitas & TVL:** Di privacy pool (seperti Tornado Cash), deposit digratiskan (0%) agar pool menjadi tebal secepat mungkin. Semakin besar TVL, semakin tinggi derajat privasi (*anonymity set*).
* **Perbandingan Industri:** Sebagian besar dApp DeFi (Aave, Uniswap) menggratiskan deposit/inflow dan hanya memungut fee saat transaksi/swap/penarikan terjadi.

---

## 2. Analisis Komparasi Model (Pros & Cons)

### Model A: Status Quo (Deposit 0,20% + Spend 0,25%)

* **Kelebihan (Pros):**
  * **Upfront Protocol Revenue:** Kas protokol langsung menerima pemasukan begitu aset masuk ke brankas, tanpa harus menunggu pengguna membelanjakannya berbulan-bulan kemudian.
  * **DDoS & Resource Spam Defense:** Setiap deposit confirmed memicu upacara BLS blind signing pada 1 Leader + 4 Guardian serta memakan 1 daun pada Merkle tree. Fee 0,20% memberi beban finansial bagi penyerang yang mencoba membanjiri cluster.
  * **Merkle Tree Preservation:** Pohon Merkle memiliki kapasitas $2^{20} = 1.048.576$ daun. Biaya di depan mencegah pemborosan slot pohon secara cuma-cuma.

* **Kekurangan (Cons):**
  * **Friksi Adopsi Tinggi:** Pengguna awam dan developer AI agent merasa "disunat sebelum memakai layanan".
  * **Hambatan Pertumbuhan Anonymity Set:** Paus (whale) enggan memarkirkan 100.000 USDC jika langsung kehilangan 200 USDC di muka hanya untuk shielding.

---

### Model B: Zero-Fee Deposit (Deposit 0% + Spend/Withdraw 0,30%)

* **Kelebihan (Pros):**
  * **Zero-Friction Onboarding:** Pengguna setor 100 USDC mendapatkan Initial Private Note 100 USDC utuh.
  * **Likuiditas Lebih Cepat Tumbuh:** Mirip strategi Tornado Cash, orang bebas menyetor dana tanpa rasa rugi, sehingga *privacy pool* Arbitrum cepat tebal.
  * **AI Agent Friendly:** Autonomous agent yang mendanai dompet untuk pembayaran API mikro memiliki nilai saldo genap yang mudah dihitung.

* **Kekurangan & Risiko (Cons):**
  * **Risiko Spam Daun Merkle (State Bloat):** Jika biaya deposit gratis, aktor jahat dapat memecah dana menjadi ratusan deposit mikro $10 untuk membebani indexer dan memenuhi slot Merkle tree.
  * **Deferred Revenue:** Pendapatan protokol bergantung 100% pada perputaran spend. Jika dana hanya diparkir dan tidak dibelanjakan, protokol tidak mendapat fee.

---

## 3. Analisis Bug, Kompleksitas & Mitigasi Teknis

| Aspek Risiko | Tingkat Risiko | Dampak / Kerentanan | Mitigasi yang Sudah / Harus Diterapkan |
|---|---|---|---|
| **Spam Micro-Deposit** | Sedang | Memenuhi Merkle tree ($2^{20}$ slot) & membebani cluster Guardian | Kontrak sudah membatasi `min_amount = 10 USDC`. Pengirim tetap bayar gas Arbitrum L2 sendiri. |
| **Rounding Bias (Ceil vs Floor)** | Rendah | Jika fee 0%, `gross_amount == net_amount`. Tidak ada risiko debet/kredit selisih 1 base unit. | Sederhana: jika `fee_bps == 0`, bypass formula ceiling division. |
| **Solvency Invariant** | Sangat Rendah | `contract_assets >= liabilities` | Tidak terpengaruh, karena `net_liability == gross_deposit` saat fee 0%. Aset dan liabilitas tetap seimbang 1:1. |
| **Refund Lifecycle** | Rendah | Jika deposit gagal di-reveal dan di-refund setelah 24 jam | Pengguna menerima kembali 100% deposit (karena tidak ada fee yang dipotong sejak awal). Menghilangkan komplikasi refund fee. |

---

## 4. Rekomendasi Solusi: Dynamic & Configurable Deposit Fee

Daripada mengunci (hardcode) angka 0% atau 0,20% secara permanen di kode smart contract, arsitektur terbaik adalah **membuat `deposit_fee_bps` sebagai parameter penyimpanan dinamis (governance-configurable)**:

```rust
// Default saat deploy: 20 bps (0.20%) atau 0 bps (promosi peluncuran)
uint256 deposit_fee_bps;

// Maksimum hard-cap on-chain agar admin/governance tidak bisa menaikkan seenaknya:
// MAX_DEPOSIT_FEE_BPS = 50 (maksimal 0.50%)
```

### Rencana Aksi Evaluasi:
1. **Fase Launching / Kampanye Awal:** `deposit_fee_bps = 0` (0% fee shielding untuk menarik likuiditas dan pengguna pertama).
2. **Proteksi Anti-Spam:** Pertahankan `min_deposit_amount = 10 USDC`.
3. **Kompensasi Revenue:** Ambil fee pada transaksi `spend()` sebesar 0,25% - 0,30%.
4. **Fase Mainnet Maturity:** Jika pohon Merkle terancam cepat penuh karena spam, governance dapat menaikkan `deposit_fee_bps` (misal 5 - 10 bps) via Two-Step Timelock Governance yang sudah ada di kontrak.

---

## 5. Checklist Pelaksanaan di Contract & Ecosystem

- [x] Tambahkan field storage `deposit_fee_bps` di [`nimbus-contracts/src/storage.rs`](file:///workspaces/Zeltra-Protocol/nimbus-contracts/src/storage.rs).
- [x] Default deposit fee: 0 bps (0.00%) permanently (`get_deposit_fee_bps()` returns `U256::ZERO`).
- [x] Bypass formula di [`nimbus-contracts/src/deposit.rs`](file:///workspaces/Zeltra-Protocol/nimbus-contracts/src/deposit.rs) saat `fee_bps == 0`, menghasilkan `net_amount == amount`.
- [x] Update fee spend ke 45 bps (default) dan 40 bps (≥ 30 hari hold) di [`nimbus-contracts/src/spend.rs`](file:///workspaces/Zeltra-Protocol/nimbus-contracts/src/spend.rs) dan `batch_spend`.
- [x] Update core fees (`fees.rs`, `accounting.rs`) dan node quote handler (`quote.rs`).
- [x] Semua 45 contract test, 23 accounting test core, dan 43 node test pass 100%.
- [x] Eliminasi DAO fee attack vector demi immutability and mathematical solvency.
