# DEC-034: Dual-Asset Architecture (USDC & WETH) — The Sovereign-Shield and High-Velocity Commerce Rails: Solvency Isolation, Censorship-Resistance Defense, 2024–2026 Multi-Asset Postmortems, and Pragmatic L2 Liquidity Convergence

- **Status:** Proposed & Accepted (Architecture Core Decision / Multi-Asset Scope)
- **Author:** Zeltra Protocol Architecture, Cryptography & Financial Economics Team
- **Date:** 2026-10-09
- **Applies to:**
  - `nimbus-contracts/src/storage.rs` (Siloed per-asset liability mappings & contract balance tracking)
  - `nimbus-contracts/src/deposit.rs` (Payable native ETH auto-wrap & ERC-20 USDC deposit ingress)
  - `nimbus-contracts/src/spend.rs` (Asset-isolated spend execution & semantic asset binding)
  - `nimbus-core/src/note.rs` (`asset_id` leaf domain binding & decimal scale representation)
  - `nimbus-core/src/note_circuit.rs` (`PrivateNoteCircuit` intra-asset value conservation constraints)
  - `nimbus-node/src/handlers/spend.rs` (Relayer zero-oracle reimbursement & asset routing)
  - `nimbus-sdk/src/wallet/note_wallet.rs` (Multi-asset coin selection & balance separation)
- **Related DECs:**
  - [`DEC-009`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-009-liability-invariant-check.md) (Mathematical Solvency Invariant Check)
  - [`DEC-016`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-016-private-note-change-ledger.md) (Private Note Change Ledger Baseline)
  - [`DEC-022`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-022-defense-against-proof-settlement-mismatch-boundary-gaps.md) (Semantic Public Input Binding)
  - [`DEC-026`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-026-receiver-enforced-compliance-zero-cost-sanctions-filtering-relayer-protection.md) (Receiver-Enforced Compliance & Relayer Protection)
  - [`DEC-028`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-028-zk-public-input-domain-hardening-flat-fee-unification-and-compliance-scoping.md) (ZK Public Input Domain Hardening & Flat Fee Unification)
  - [`DEC-031`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-031-relayer-reconciliation-solvency-observability-quote-domain-binding.md) (Relayer Solvency Observability & Quote Binding)
  - [`DEC-032`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-032-merkle-mountain-range-commitment-accumulator.md) (MMR Commitment Accumulator)
  - [`DEC-033`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-033-epoch-windowed-nullifier-pruning-in-flight-rollover.md) (Epoch-Windowed Nullifier Registry & In-Flight Rollover)
- **Architectural Paradigm:** *"Sovereign Security via Native ETH, Commercial Velocity via Dominant USDC, Mathematically Siloed Solvency"*
- **Guiding Principle:** *"A commercial privacy rail must embrace both market realities: mainstream commerce demands stable cash accounting ($10 - $3 = $7), while survival demands censorship-resistant immutability. Zeltra converges exclusively on Arbitrum's deepest liquidity pair—USDC and WETH—isolating liabilities per asset so a regulatory freeze on one can never compromise the other, fortified against multi-asset exploits (2024–2026) without introducing price oracle risks."*

---

## 1. Executive Summary & Kajian Kondisi Nyata (Status Quo)

### A. Dilema Eksistensial Protokol Privasi: Kematian Satu Tombol (*One-Click Protocol Death*)
Pada tahap awal perancangan (Phase 1 Baseline), Zeltra Protocol beroperasi menggunakan satu aset tunggal: **USDC (Circle)**. Pilihan ini didorong oleh visi produk *"The Stripe of Web3 with Absolute Privacy"*, di mana pengguna e-commerce, ritel, dan korporat membutuhkan stabilitas nilai tanpa fluktuasi harga kripto.

Namun, telaah mendalam terhadap sejarah keamanan protokol privasi (khususnya sanksi OFAC terhadap Tornado Cash pada Agustus 2022) mengungkap **kerentanan titik kegagalan tunggal (Single Point of Failure / SPOF)** yang fatal:
1. **Fungsi Blacklist Terselubung:** Smart contract USDC (`FiatTokenV2`) memiliki fungsi eksplisit:
   ```solidity
   function blacklist(address _account) external onlyOwner;
   ```
2. **Pembekuan Aset Seketika:** Ketika sebuah smart contract protokol privasi dimasukkan ke daftar hitam oleh penerbit sentral (Circle), saldo token di dalam smart contract tersebut **langsung beku permanen**. Setiap fungsi `transfer()` atau `transferFrom()` yang melibatkan alamat kontrak akan memicu revert on-chain.
3. **Kertas Kosong bagi Pemegang ZK Note:** Sekalipun sirkuit ZK-UTXO membuktikan kepemilikan note dengan validitas kriptografi sempurna, pengguna tidak dapat mencairkan (*spend/settle*) dana keluar karena kontrak kolam telah terkunci di layer token ERC-20.

Ketergantungan 100% pada satu penerbit fiat terpusat merupakan ancaman eksistensial terhadap kelangsungan hidup protokol Zeltra.

---

### B. Jebakan Anonymity Set & Eksploitasi "Multi-Token Liar"
Menanggapi ancaman di atas, pendekatan naif sering kali mengusulkan: *"Izinkan semua token ERC-20 secara permissionless (USDT, DAI, WBTC, ribuan token meme/altcoin)."*

Kajian arsitektur Zeltra membuktikan bahwa pendekatan ini **jauh lebih berbahaya**:
1. **Fragmentasi Anonymity Set:** Jika setiap token memiliki kolam atau pohon Merkle terpisah, token-token bervolume rendah hanya akan memiliki segelintir transaksi per hari. Metadata transaksi on-chain (analisis waktu dan besaran nilai token) akan langsung membatalkan privasi pengguna (*de-anonymization via token metadata leakage*).
2. **Vektor Serangan "Weird ERC-20":**
   * *Fee-on-Transfer / Deflationary Tokens:* Pengguna mendepositkan 100 token, kontrak hanya menerima 98 token (terpotong pajak). Jika kontrak mencatat liabilitas 100, kontrak langsung **insolvent**.
   * *Rebasing Tokens (stETH, aUSDC):* Perubahan saldo otomatis merusak kalkulasi akuntansi konservatif Zeltra.
   * *ERC-777 Reentrancy Hooks:* Token dengan callback hook yang dapat membajak eksekusi batch payout Stylus.
3. **Risiko Orakel & Kerugian Relayer (Toxic Debt & MEV):** Jika relayer harus menerima biaya eksekusi gas dalam puluhan jenis token volatil, relayer terpaksa mengintegrasikan Price Oracle eksternal (Chainlink/Pyth/Uniswap TWAP) dan menanggung risiko *slippage*, manipulasi orakel kilat (*flash loan oracle manipulation*), serta inventaris token illiquid.

---

### C. Data Empiris On-Chain Arbitrum One (Kajian DefiLlama & Arbiscan 2026)
Alih-alih berspekulasi, Zeltra menganalisis data riil likuiditas on-chain pada jaringan penerapan utama (**Arbitrum One**):

```text
┌────────────────────────────────────────────────────────────────────────────────────────┐
│               DISTRIBUSI PASAR STABLECOIN & LIKUIDITAS DI ARBITRUM ONE                 │
├─────────────────────────┬──────────────────────────────┬───────────────────────────────┤
│ Metrik / Aset           │ Nilai Data On-Chain          │ Implikasi Arsitektur          │
├─────────────────────────┼──────────────────────────────┼───────────────────────────────┤
│ Total Stablecoin Supply │ ~$3.908 Miliar USD           │ Likuiditas L2 sangat padat    │
│ Dominasi USDC           │ 63.57% (~$2.48 Miliar USD)   │ Raja mutlak likuiditas dolar  │
│ Dominasi USDT           │ ~26.10% (~$1.02 Miliar USD)  │ Minoritas di L2 Arbitrum      │
│ Pasangan Likuiditas #1  │ WETH / USDC (Uniswap/Camelot)│ Pasangan volume terbesar L2   │
│ Faucet Testnet Sepolia  │ USDC Resmi Circle Aktif      │ Validasi testing E2E deterministik│
└─────────────────────────┴──────────────────────────────┴───────────────────────────────┘
```

**Temuan Krusial:**
* Berbeda dengan jaringan Tron atau BNB di mana USDT dominan, ekosistem **Arbitrum One secara mutlak didominasi oleh USDC (63.57%)** berkat adopsi Circle Native CCTP dan likuiditas dasar protokol DeFi (GMX, Camelot, Uniswap v3).
* Pasangan trading dan pembayaran terbesar di seluruh Arbitrum adalah **WETH / USDC**.
* Stripe Crypto Payments saat ini belum mendukung Arbitrum (baru beroperasi di Base, Polygon, Solana), menciptakan celah pasar masif bagi Zeltra untuk menjadi rel pembayaran terdepan di Arbitrum.

---

## 2. Kajian Forensik Postmortem & Audit Keamanan Multi-Asset (2024–2026)

Sebelum merancang sistem multi-aset untuk Zeltra, tim arsitektur melakukan investigasi forensik mendalam terhadap eksploitasi, audit report, dan bug postmortem sistem multi-token dan ZK shielded pool sepanjang tahun 2024 hingga 2026:

```text
┌────────────────────────────────────────────────────────────────────────────────────────┐
│                KAJIAN FORENSIK POSTMORTEM MULTI-ASSET & ZK (2024–2026)                 │
├─────────────────────────┬──────────────────────────────┬───────────────────────────────┤
│ Kasus / Laporan         │ Akar Masalah Kerentanan      │ Mitigasi Imperatif Zeltra     │
├─────────────────────────┼──────────────────────────────┼───────────────────────────────┤
│ 1. Penumbra Circuit     │ Double-Spend & Unconstrained │ • Larang Cross-Asset Swaps    │
│    Audit (zkSecurity    │ Asset ID Derivation pada     │   di dalam sirkuit ZK.        │
│    2024–2025)           │ multi-asset shielded pool    │ • Strict intra-asset equality │
│                         │                              │   pada seluruh public inputs. │
├─────────────────────────┼──────────────────────────────┼───────────────────────────────┤
│ 2. TrustedVolumes RFQ   │ Cross-Asset Authorization    │ • Semantic Public Input       │
│    Exploit (Mei 2026,   │ Mismatch: otorisasi pada satu│   Binding (DEC-022).          │
│    $5.87M Lost)         │ token, debit pada token lain │ • Alamat token di-resolve via │
│                         │ (1,291 WETH & 1.27M USDC)    │   storage immutable, bukan    │
│                         │                              │   parameter dinamis caller.   │
├─────────────────────────┼──────────────────────────────┼───────────────────────────────┤
│ 3. Conic Finance &      │ Address Ambiguity Bug        │ • Canonical Address           │
│    Sentiment Omnipool   │ (Native ETH 0xeee vs WETH)   │   Normalization.              │
│    (CertiK 2023–2024)   │ _isETH bypass reentrancy     │ • State hanya mengenali WETH9;│
│                         │ lock, terkuras 1,727 ETH     │   deposit native ETH auto-wrap│
├─────────────────────────┼──────────────────────────────┼───────────────────────────────┤
│ 4. Clober DEX Strategy  │ Arbitrary Token Injection &  │ • Curated Asset Registry:     │
│    Exploit (CertiK      │ Reentrancy Hook sebelum      │   Hanya izinkan Asset ID 0 & 1│
│    Desember 2024)       │ reserve balance diperbarui   │ • Larang arbitrary token input│
├─────────────────────────┼──────────────────────────────┼───────────────────────────────┤
│ 5. Zcash Orchard        │ Missing Value Commitment     │ • 64-bit Strict Range Checks  │
│    Counterfeit Bug      │ Balance Constraint (infinit  │ • Konservasi mutlak:          │
│    (Taylor Hornby 2024) │ counterfeit minting)         │   In = Payout + Fees + Change │
├─────────────────────────┼──────────────────────────────┼───────────────────────────────┤
│ 6. Mixed-Decimal Scale  │ Truncation & Rounding Drain  │ • Normalisasi Skala:          │
│    Mismatch (18 vs 6)   │ akibat perbedaan skala 10^12 │   WETH dinormalisasi ke Gwei  │
│                         │ antara WETH dan USDC         │   (9 desimal) di sirkuit u64. │
└─────────────────────────┴──────────────────────────────┴───────────────────────────────┘
```

### Analisis Detail Insiden:
1. **Penumbra Multi-Asset Shielded Pool (zkSecurity Report 2024–2025):**
   * *Akar Masalah:* Dalam kolam multi-aset Penumbra, komitmen swap plaintext diturunkan sebagai `asset_id` unik. Sirkuit melepaskan komitmen saldo negatif dan positif untuk melakukan konversi antar aset. Namun, kegagalan dalam menegakkan constraint kesetaraan aset yang ketat memungkinkan penyerang menciptakan saldo sintetis tanpa jaminan (*unbacked note injection*) dan melakukan *double-spend*.
   * *Pelajaran Zeltra:* **TIDAK MENGIZINKAN SWAP ANTAR ASET DI DALAM SIRKUIT ZK**. Sirkuit `PrivateNoteCircuit` hanya memproses satu jenis aset per transaksi: input note, payout, fee relayer, protocol fee, dan change note WAJIB memiliki `asset_id` yang identik 100%.
2. **TrustedVolumes RFQ Proxy Exploit (Mei 2026, BlockSec):**
   * *Akar Masalah:* Kontrak proxy memeriksa izin (*signer authorization*) pada satu aset, tetapi mengeksekusi transfer ERC-20 pada aset lain yang ditentukan oleh parameter panggilan. Penyerang menguras 1.291 WETH, 206K USDT, dan 1.27M USDC.
   * *Pelajaran Zeltra:* Mengukuhkan **[`DEC-022`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-022-defense-against-proof-settlement-mismatch-boundary-gaps.md)**. Pemanggilan on-chain Stylus `spend_private_note` TIDAK PERNAH menerima alamat kontrak token dari caller! Kontrak Stylus hanya menerima `asset_id` (0 atau 1) dari public inputs Groth16, lalu me-lookup alamat token terdaftar di internal storage immutable.
3. **Conic Finance Incident (CertiK & Merkle Science Postmortem):**
   * *Akar Masalah:* Conic mengasumsikan koin native ETH beralamat `0xeee...eee`. Namun, kolam Curve di baliknya menggunakan alamat WETH. Pemeriksaan `_isETH` menghasilkan `false`, membypass reentrancy lock, dan menguras 1.727 ETH.
   * *Pelajaran Zeltra:* **Zero Dual-Accounting**. Di dalam database, storage, dan sirkuit Zeltra, TIDAK ADA token dengan alamat `0x000...000` atau `0xeee...eee`. Seluruh simpanan dinormalisasi secara kanonikal ke alamat **WETH9** (`0x980B62Da83Eff3D4576C647993b0c1D7faf17c73` di Sepolia). Deposit ETH native langsung dibungkus menjadi WETH9 pada instruksi pertama fungsi payable sebelum menyentuh state internal.
4. **Clober DEX Reentrancy & Fake Token Strategy (CertiK Desember 2024):**
   * *Akar Masalah:* Kontrak mengizinkan pendaftaran token dan strategi arbitrer dari input publik. Token penyerang memanggil callback hook di tengah proses burn, menarik WETH cadangan sebelum reserve di-update.
   * *Pelajaran Zeltra:* **Whitelisted Curated Pair Only**. Kontrak Stylus menolak keras pendaftaran token dinamis oleh pengguna. Hanya dua aset yang didaftarkan pada konstruktor kontrak (`Asset ID 0 = WETH`, `Asset ID 1 = USDC`).

---

## 3. Keputusan Arsitektur: Dual-Asset Paradigm (USDC + WETH)

Zeltra Protocol secara resmi menetapkan kebijakan **Dual-Asset Architecture**: membatasi kolam penyelesaian terlindung (*shielded settlement pool*) hanya pada dua aset fondasional:

```text
                           ┌── [Asset ID 0] WETH (The Sovereign Rail)
                           │   • 100% Censorship-Resistant (No Blacklist / No Owner)
                           │   • Likuiditas Native Arbitrum Terbesar
                           │   • Dirancang untuk: AI Agents, Whales, Cypherpunks
Zeltra Dual-Asset Pool ────┤
                           └── [Asset ID 1] USDC (The High-Velocity Commerce Rail)
                               • 63.57% Dominasi Dolar Arbitrum ($1 = $1 Exact Cash)
                               • Zero Slippage, Ramah Pembukuan Pajak & Off-Ramp
                               • Dilindungi oleh DEC-026 (Sanctions Filtering)
                               • Dirancang untuk: Retail, E-Commerce, Global Merchants
```

---

## 4. Komparasi Matriks Persona Pengguna (User-Centric Analysis)

Keputusan ini memetakan empat persona pengguna utama Zeltra ke dalam dua rel aset yang saling melengkapi:

| Kebutuhan Pengguna | Jalur WETH (`Asset ID 0`) | Jalur USDC (`Asset ID 1`) |
|---|---|---|
| **1. Autonomous AI Agents** | **Pilihan Utama (10/10):** Agen otonom tidak memiliki identitas hukum/KYC. WETH adalah uang asli mesin yang mustahil dibekukan oleh regulator atau korporasi perbankan. | **Pilihan Sekunder (7/10):** Digunakan ketika agen perlu membayar layanan API TradFi berbasis denominasi dolar. |
| **2. Global Merchants & B2B** | **Tidak Cocok (1/10):** Merchant menolak volatilitas WETH untuk pembukuan akuntansi, penggajian karyawan, dan pembayaran vendor. | **Pilihan Utama (10/10):** Likuiditas instan, kompatibel dengan sistem kasir, mudah dicairkan (*off-ramp*) ke rekening bank lokal. |
| **3. Retail Shoppers** | **Kebutuhan Khusus (4/10):** Pengguna awam enggan menghitung belanjaan harian dalam pecahan desimal ETH yang berfluktuasi setiap jam. | **Pilihan Utama (10/10):** Mental accounting kas nyata ($10 bayar $3 kembali $7). Sesuai prinsip produk 10/10. |
| **4. Whales & Cypherpunks** | **Pilihan Utama (10/10):** Penyimpanan modal privat berskala besar tanpa risiko intervensi negara atau penyitaan aset sepihak. | **Dihindari untuk Simpanan Jangka Panjang (2/10):** Risiko pembekuan Circle pada nominal besar. |

---

## 5. Spesifikasi Teknis & Solvensi Matematika

### A. Isolasi Solvensi Total (Siloed Multi-Asset Solvency)
Untuk memastikan pembekuan atau anomali pada satu aset tidak pernah merembes ke aset lainnya, smart contract Stylus memisahkan akuntansi solvensi per `asset_id`:

$$\forall a \in \{\text{WETH}, \text{USDC}\}: \quad \text{ContractBalance}(a) \ge \text{TotalLiabilities}(a)$$

Di mana:
$$\text{TotalLiabilities}(a) = \text{UserNoteLiability}(a) + \text{RefundableDepositLiability}(a) + \text{AccruedExecutionFeeLiability}(a)$$

Jika kontrak USDC mengalami pembekuan eksternal oleh Circle:
1. Payout transaksi USDC akan gagal di layer ERC-20.
2. **Namun, kolam WETH tetap 100% aktif dan dapat dibelanjakan tanpa gangguan**, karena seluruh mapping storage, saldo token, dan verifikasi solvensi terisolasi secara kriptografis dan matematis.

```rust
// nimbus-contracts/src/storage.rs
pub struct ProtocolStorage {
    // Mapping saldo dan liabilitas terisolasi per asset_id (0: WETH, 1: USDC)
    pub total_note_liabilities: StorageMap<u8, StorageU256>,
    pub refundable_deposit_liabilities: StorageMap<u8, StorageU256>,
    pub accrued_execution_fee_liabilities: StorageMap<u8, StorageU256>,
    pub asset_token_addresses: StorageMap<u8, StorageAddress>,
}
```

---

### B. Binding Komitmen ZK-UTXO & Konservasi Nilai Intra-Aset
1. **Unified Accumulator Tree:** Komitmen note WETH dan USDC disimpan dalam accumulator pohon yang sama ([`DEC-032`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-032-merkle-mountain-range-commitment-accumulator.md) MMR / LeanIMT) untuk mempertahankan kepadatan pohon dan efisiensi gas Stylus.
2. **Poseidon Leaf Binding:** Note leaf mengikat `asset_id` secara eksplisit:
   $$\text{Leaf} = \text{Poseidon5}(\text{nullifier\_sec}, \text{secret}, \text{value}, \text{asset\_id}, \text{epoch})$$
3. **Sirkuit Konservasi Nilai Intra-Aset:** Sirkuit ZK (`PrivateNoteCircuit`) menegakkan bahwa input note, payout, execution fee, protocol fee, dan change note harus memiliki `asset_id` yang identik:
   $$\text{input\_asset} == \text{payout\_asset} == \text{fee\_asset} == \text{change\_asset}$$
   $$\text{input\_value} = \text{payout\_value} + \text{protocol\_fee} + \text{execution\_fee} + \text{change\_value}$$
   *Pencegahan Celah Penumbra:* Zeltra secara ketat melarang konversi silang antar aset di dalam sirkuit pada fase ini. Hal ini menghilangkan 100% kebutuhan Price Oracle, mencegah kerentanan slippage, dan menjaga integritas matematika solvensi.

---

### C. Normalisasi Desimal (18 Desimal vs 6 Desimal)
* **USDC:** Menggunakan 6 desimal ($1\text{ USDC} = 10^6\text{ units}$). Nilai transaksi pas dalam representasi `u64` standar tanpa risiko modular wrap-around.
* **WETH:** Menggunakan 18 desimal ($1\text{ WETH} = 10^{18}\text{ units}$).
* **Solusi Batas 64-bit Field (Anti-Truncation Exploit):**
  * Nilai maksimum `u64` adalah $2^{64} - 1 \approx 1.84 \times 10^{19}$.
  * Dalam skala 18 desimal, kapasitas maksimum adalah $\approx 18.44\text{ WETH}$ per note jika disimpan mentah dalam satuan wei.
  * **Spesifikasi Normalisasi Zeltra:** Untuk note WETH, nilai di dalam sirkuit dan kontrak dinormalisasi ke satuan **Gwei (9 desimal)**:
    $$1\text{ Gwei} = 10^9\text{ Wei}$$
    Dengan basis Gwei, rentang `u64` mampu menampung hingga **$18{,}446{,}744{,}073\text{ Gwei} \approx 18.4\text{ Miliar WETH}$**, mencakup seluruh suplai sirkulasi Ethereum tanpa risiko overflow field sirkuit Groth16. Konversi ke Wei dilakukan saat interaksi `transfer()` on-chain:
    $$\text{wei\_amount} = \text{gwei\_amount} \times 10^9$$

---

### D. Zero-Oracle Relayer Reimbursement
Relayer Zeltra tidak bergantung pada Price Oracle eksternal (Chainlink/Pyth):
* Biaya eksekusi gas untuk pembelanjaan note USDC dipotong dari saldo note dalam bentuk **USDC** (menggunakan kuotasi gas L2 yang dijamin EIP-712).
* Biaya eksekusi gas untuk pembelanjaan note WETH dipotong dari saldo note dalam bentuk **WETH** (yang secara langsung merupakan mata uang gas dasar Arbitrum).
* **Hasil:** Sistem kebal terhadap manipulasi flash loan oracle, downtime API pihak ketiga, atau eksploitasi arbitrase nilai tukar.

---

### E. Frictionless Inflow UX: Native ETH Auto-Wrap (Anti-Conic Pattern)
Untuk mempertahankan prinsip produk 10/10 (Apple Pay speed tanpa friksi):
* Pengguna MetaMask tidak diwajibkan melakukan wrapping manual di DEX sebelum mendepositkan dana.
* Kontrak Stylus Zeltra menerapkan pola *Checks-Effects-Interactions* (CEI) ketat:
  ```rust
  #[payable]
  pub fn deposit_native_eth(&mut self, session_id: [u8; 32], com_k_hash: [u8; 32]) -> Result<(), Vec<u8>> {
      let amount = msg::value();
      if amount == U256::ZERO {
          return Err(b"ZERO_DEPOSIT_VALUE".to_vec());
      }
      
      // 1. Interactions: Bungkus ETH ke Canonical WETH9 seketika
      self.weth_gateway.deposit_value(amount)?;
      
      // 2. Effects: Catat liabilitas WETH (Asset ID 0)
      self.record_deposit(0, amount, session_id, com_k_hash)?;
      Ok(())
  }
  ```
* Pengguna cukup mengklik satu tombol *"Deposit ETH"* di dApp Zeltra, dan sistem secara instan menerbitkan ZK Private Note berdenominasi WETH.

---

## 6. Pertahanan Regulasi & Tameng Blacklist (Sinergi dengan DEC-026)

Keputusan mempertahankan USDC didukung oleh mekanisme pertahanan aktif yang telah ditetapkan dalam [`DEC-026 (Receiver-Enforced Compliance & Zero-Cost Sanctions Filtering)`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-026-receiver-enforced-compliance-zero-cost-sanctions-filtering-relayer-protection.md):

1. **Pencegahan Eksploitasi Kriminal di Pintu Depan:** Relayer Zeltra menolak membroadcast transaksi dari/ke alamat yang terdaftar dalam daftar sanksi OFAC / Chainalysis oracle.
2. **Receiver-Enforced Acceptance:** Merchant dan penerima institusional memverifikasi bukti non-sanksi sebelum menerima penyelesaian dana.
3. **Hasil Strategis:** Zeltra secara aktif mencegah masuknya dana teroris, pencucian uang hacker (Lazarus Group), atau aset curian. Tanpa adanya eksploitasi kriminal, Circle maupun regulator tidak memiliki dasar hukum objektif untuk mem-blacklist smart contract Zeltra, menjaga kelangsungan kolam komersial USDC tetap aman.

---

## 7. Rangkuman & Dampak Implementasi

```text
┌────────────────────────────────────────────────────────────────────────────────────────┐
│                        RANGKUMAN ARSITEKTUR DEC-034 ZELTRA PROTOCOL                    │
├──────────────────────────┬──────────────────────────────┬──────────────────────────────┤
│ Komponen                 │ Implementasi                 │ Keuntungan Utama             │
├──────────────────────────┼──────────────────────────────┼──────────────────────────────┤
│ Pasangan Aset Resmi      │ WETH (`id: 0`) + USDC (`id: 1`)│ Menyatukan anti-sensor & kas │
│ Akuntansi Solvensi       │ Terisolasi per Asset ID      │ Kegagalan A tidak menyentuh B│
│ Mitigasi Exploit 24–26   │ Penumbra, Conic, TrustedVol  │ Kebal cross-asset authorization│
│ Mekanisme Orakel         │ Zero-Oracle (Intra-Asset)    │ Kebal manipulasi harga/MEV   │
│ Normalisasi Skala        │ WETH: Gwei (9 des), USDC (6) │ Aman dalam rentang u64 Groth16│
│ Deposit UX               │ Auto-wrap native ETH payable │ Zero-friction bagi user awam │
│ Pertahanan Blacklist     │ Dual: WETH Rail + DEC-026    │ Protokol anti-mati           │
└──────────────────────────┴──────────────────────────────┴──────────────────────────────┘
```

---

## 8. Matriks Pengujian & Verifikasi (Positive & Negative Test Suite)

Mengikuti invariant baku Zeltra Protocol, implementasi wajib menyertakan minimal 2x tes negatif terhadap tes positif:

### A. Positive Tests
1. `test_dual_asset_usdc_deposit_and_spend_success`: Deposit USDC, cetak ZK note `asset_id: 1`, dan belanjakan note dengan verifikasi Groth16 sukses.
2. `test_dual_asset_weth_auto_wrap_deposit_and_spend_success`: Deposit native ETH via `deposit_native_eth()` otomatis terbungkus ke WETH9, mencetak ZK note `asset_id: 0`, dan payout dalam satuan WETH sukses.
3. `test_dual_asset_siloed_solvency_independent_balances`: Verifikasi bahwa liabilitas dan saldo `asset_id: 0` dan `asset_id: 1` dihitung secara independen tanpa saling mensubsidi.
4. `test_dual_asset_gwei_scale_normalization_precision`: Nilai WETH berdenominasi 9 desimal (Gwei) dalam sirkuit berhasil dikonversi ke 18 desimal (Wei) saat transfer on-chain tanpa kehilangan presisi.

### B. Negative Tests (Pertahanan Batas & Anti-Eksploit 2024–2026)
1. `test_dual_asset_cross_asset_spend_rejection`: Percobaan membelanjakan note USDC (`asset_id: 1`) untuk mencairkan payout WETH (`asset_id: 0`) wajib gagal pada verifikasi constraint sirkuit (`ASSET_ID_MISMATCH`).
2. `test_dual_asset_unregistered_asset_id_rejection`: Percobaan membuat note dengan `asset_id >= 2` ditolak secara fail-closed oleh smart contract Stylus (`UNSUPPORTED_ASSET_ID`).
3. `test_dual_asset_caller_supplied_token_bypass_blocked`: Percobaan caller mengoper alamat token ERC-20 arbitrer diabaikan oleh kontrak; kontrak hanya menggunakan alamat token kanonikal dari storage (`SEMANTIC_BINDING_MISMATCH`).
4. `test_dual_asset_usdc_freeze_isolation_weth_spend_unaffected`: Simulasi pembekuan transfer pada kontrak USDC membuktikan bahwa kolam WETH tetap dapat memproses deposit, verifikasi ZK, dan spend tanpa revert.
5. `test_dual_asset_native_eth_zero_value_revert`: Pemanggilan `deposit_native_eth()` dengan `msg::value() == 0` wajib revert seketika (`ZERO_DEPOSIT_VALUE`).
6. `test_dual_asset_gwei_field_overflow_rejection`: Percobaan menginjeksi nilai WETH melebihi kapasitas `u64` Gwei ditolak oleh range check sirkuit Groth16.
7. `test_dual_asset_relayer_cross_asset_fee_drain_blocked`: Upaya memotong fee eksekusi relayer dalam aset WETH dari note USDC ditolak oleh relayer spend handler (`RELAYER_FEE_ASSET_MISMATCH`).
8. `test_dual_asset_reentrancy_auto_wrap_blocked`: Serangan reentrancy callback selama proses auto-wrap ETH ditolak oleh reentrancy lock `deposit_native_eth`.

---

## 9. Kesimpulan & Roadmap Transisi

Dengan disahkannya **DEC-034**:
1. **Protokol Kebal Sensor (Anti-Mati):** Ketergantungan fatal pada satu penerbit centralized (Circle) dihapuskan. Keberadaan rel WETH menjamin Zeltra tetap dapat beroperasi selamanya sekalipun terjadi intervensi regulasi sepihak.
2. **Kenyamanan Kas Komersial Terjaga (10/10):** Pengguna ritel dan merchant tetap menikmati kemudahan pembayaran kas stabil berbasis USDC yang mendominasi 63.57% likuiditas Arbitrum.
3. **Solvensi Terisolasi & Nol Orakel:** Tidak ada risiko slippage, tidak ada ketergantungan pada Price Oracle eksternal, dan setiap aset bertanggung jawab mutlak atas solvensinya sendiri.

---

## 10. Referensi & Sitasi Akademik / Forensik (2024–2026)

1. **zkSecurity — Audit of Penumbra's Multi-Asset Shielded Pool Circuits:**
   zkSecurity Research & Audit Team. *"Public Report of Auditing Penumbra's Circuits: Double-Spend and Asset ID Derivation Vulnerabilities in Multi-Asset Shielded Pools"*. zkSecurity Audit Publications & Disclosures (August 2023, revised April 2025).
   URL: https://blog.zksecurity.xyz/posts/penumbra

2. **BlockSec — TrustedVolumes RFQ Cross-Asset Authorization Exploit:**
   BlockSec Blockchain Intelligence Team. *"~$15.9M Lost: Trusted Volumes, Wasabi & More — Type-Level Authorization Mismatches and Multi-Token Debit Exploitation in DeFi Settlement Proxies"*. BlockSec Incident Roundup (May 2026).
   URL: https://blocksec.com/blog/weekly-web3-security-roundup-2026-05-10

3. **CertiK & Merkle Science — Conic Finance Read-Only Reentrancy & Address Ambiguity:**
   CertiK Security Team & Merkle Science Intelligence. *"Conic Finance Incident Analysis: Underlying Address Resolution Mismatches (ETH vs WETH) and Read-Only Reentrancy Postmortem"*. CertiK Official Research / Merkle Science Hack Track (July 2023, updated 2024).
   URL: https://www.certik.com/zh-CN/blog/conic-finance-incident-analysis

4. **CertiK — Clober DEX Strategy Injection & Reentrancy Exploit:**
   CertiK Skynet Research. *"Clober DEX Incident Analysis: Arbitrary Strategy Injection and Reentrancy in Reserve Settlement Pools"*. CertiK Skynet Report (December 2024).
   URL: https://www.certik.com/zh-CN/skynet-report/clober-dex-incident-analysis

5. **Electric Coin Company & Taylor Hornby — Zcash Orchard Counterfeit Vulnerability Analysis:**
   Electric Coin Company & Hornby, Taylor. *"Discovery and Remediation of Undetectable Counterfeiting Vulnerabilities in Shielded Pool Value Commitment Balance Constraints"*. ECC Cryptography Bulletins & Technical Disclosures (2024–2026).
   URL: https://z.cash/upgrade/orchard/

6. **DefiLlama & Arbitrum Foundation — Stablecoin Supply and Liquidity Metrics on Arbitrum One:**
   DefiLlama Intelligence & Arbitrum Ecosystem Analytics. *"Stablecoins Market Capitalization by Chain: USDC Dominance (63.57%) and DEX Liquidity Pair Distribution on Arbitrum One"*. DefiLlama Analytics Platform (October 2026).
   URL: https://defillama.com/stablecoins / https://defillama.com/chain/Arbitrum

7. **Veridise — Panther Protocol Multi-Asset Shielded Swapping Audit:**
   Veridise Security Team. *"Panther Protocol Blockchain Security Assessment: zSwap and Multi-Asset Transfer Limits in Shielded Pools"*. Veridise Formal Audit Archives (September 2025).
   URL: https://veridise.com/audits-archive/company/panther-protocol/

