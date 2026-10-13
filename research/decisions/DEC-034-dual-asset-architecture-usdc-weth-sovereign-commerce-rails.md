# DEC-034: Dual-Asset Architecture (USDC & WETH) — The Sovereign-Shield and High-Velocity Commerce Rails: Solvency Isolation, Censorship-Resistance Defense, 2024–2026 Multi-Asset Postmortems, and Pragmatic L2 Liquidity Convergence

- **Status:** Proposed & Accepted (Architecture Core Decision / Multi-Asset Scope)
- **Author:** Zeltra Protocol Architecture, Cryptography & Financial Economics Team
- **Date:** 2026-10-09 (Updated & Synced to Pure ZK-UTXO: 2026-10-13)
- **Applies to:**
  - `zeltra-contracts/src/storage.rs` (Siloed per-asset liability mappings & contract balance tracking)
  - `zeltra-contracts/src/deposit.rs` (Payable native ETH auto-wrap & ERC-20 USDC direct MMR inflow)
  - `zeltra-contracts/src/spend.rs` (Asset-isolated spend execution, WETH/USDC payout routing)
  - `zeltra-core/src/note.rs` (`asset_id` packed leaf commitment & DEC-037 48-byte `NotePlaintext` flags)
  - `zeltra-core/src/note_circuit.rs` (`PrivateNoteCircuit` intra-asset value conservation & packing constraints)
  - `zeltra-core/src/joinsplit_circuit.rs` (`JoinSplitCircuit` 19-PI `flags_packed` asset-binding constraints)
  - `zeltra-node/src/handlers/spend.rs` (Relayer zero-oracle reimbursement & asset routing)
  - `zeltra-sdk/src/wallet/note_wallet.rs` (Multi-asset coin selection & balance separation)
- **Related DECs:**
  - [`DEC-034A`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-034A-canonical-field-bit-packing-and-intra-asset-circuit-constraints.md) (Canonical Field Bit-Packing & Intra-Asset Circuit Soundness)
  - [`DEC-034B`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-034B-stylus-siloed-solvency-payable-weth-wrap-and-multi-asset-settlement.md) (Stylus Smart Contract Siloed Solvency & Payable Auto-Wrap)
  - [`DEC-034C`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-034C-relayer-zero-oracle-reimbursement-and-sdk-multi-asset-wallet.md) (Relayer Zero-Oracle Routing & Multi-Asset Client SDK)
  - [`DEC-009`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-009-liability-invariant-check.md) (Mathematical Solvency Invariant Check)
  - [`DEC-016`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-016-private-note-change-ledger.md) (Private Note Change Ledger Baseline)
  - [`DEC-022`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-022-defense-against-proof-settlement-mismatch-boundary-gaps.md) (Semantic Public Input Binding)
  - [`DEC-026`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-026-receiver-enforced-compliance-zero-cost-sanctions-filtering-relayer-protection.md) (Receiver-Enforced Compliance & Relayer Protection)
  - [`DEC-028`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-028-zk-public-input-domain-hardening-flat-fee-unification-and-compliance-scoping.md) (ZK Public Input Domain Hardening & Flat Fee Unification)
  - [`DEC-031`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-031-relayer-reconciliation-solvency-observability-quote-domain-binding.md) (Relayer Solvency Observability & Quote Binding)
  - [`DEC-032`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-032-merkle-mountain-range-commitment-accumulator.md) (MMR Commitment Accumulator)
  - [`DEC-033`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-033-epoch-windowed-nullifier-pruning-in-flight-rollover.md) (Epoch-Windowed Nullifier Registry & In-Flight Rollover)
  - [`DEC-035`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-035-cryptographic-canonicality-public-input-binding-and-mmr.md) (16-PI Public Input Layout & Two-Limb Quote Hash)
  - [`DEC-036`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-036-universal-2-in-2-out-joinsplit-full-stack-architecture.md) (Universal 2-in-2-out JoinSplit 19-PI Specification)
  - [`DEC-037`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-037-note-encryption-memo-and-viewing-key-recovery.md) (96-Byte AEAD Memo & 48-Byte Note Plaintext)
  - [`DEC-038`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-038-pure-zk-utxo-migration-and-direct-mmr-deposit.md) (Pure ZK-UTXO Direct MMR Deposit Inflow Architecture)
- **Architectural Paradigm:** *"Sovereign Security via Native ETH, Commercial Velocity via Dominant USDC, Mathematically Siloed Solvency, Pure Direct MMR Inflow"*
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
│ Faucet Testnet Sepolia  │ USDC Resmi Circle Aktif      │ Validasi testing deterministik│
└─────────────────────────┴──────────────────────────────┴───────────────────────────────┘
```

**Temuan Krusial:**
* Berbeda dengan jaringan Tron atau BNB di mana USDT dominan, ekosistem **Arbitrum One secara mutlak didominasi oleh USDC (63.57%)** berkat adopsi Circle Native CCTP dan likuiditas dasar protokol DeFi (GMX, Camelot, Uniswap v3).
* Pasangan trading dan pembayaran terbesar di seluruh Arbitrum adalah **WETH / USDC**.
* WETH adalah uang asli mesin (*machine native money*) dan penyimpan nilai berdaulat (*sovereign store of value*) yang **kebal blacklist 100%**.

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
   * *Pelajaran Zeltra:* **TIDAK MENGIZINKAN SWAP ANTAR ASET DI DALAM SIRKUIT ZK**. Sirkuit `PrivateNoteCircuit` dan `JoinSplitCircuit` hanya memproses satu jenis aset per transaksi: input note, payout, fee relayer, protocol fee, dan change note WAJIB memiliki `asset_id` yang identik 100%.
2. **TrustedVolumes RFQ Proxy Exploit (Mei 2026, BlockSec):**
   * *Akar Masalah:* Kontrak proxy memeriksa izin (*signer authorization*) pada satu aset, tetapi mengeksekusi transfer ERC-20 pada aset lain yang ditentukan oleh parameter panggilan. Penyerang menguras 1.291 WETH, 206K USDT, dan 1.27M USDC.
   * *Pelajaran Zeltra:* Mengukuhkan **[`DEC-022`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-022-defense-against-proof-settlement-mismatch-boundary-gaps.md)**. Pemanggilan on-chain Stylus `spend_private_note` atau `spend_joinsplit` TIDAK PERNAH menerima alamat kontrak token dari caller! Kontrak Stylus me-resolve alamat token terdaftar di internal storage immutable berdasarkan `asset_id` yang dibuktikan oleh sirkuit.
3. **Conic Finance Incident (CertiK & Merkle Science Postmortem):**
   * *Akar Masalah:* Conic mengasumsikan koin native ETH beralamat `0xeee...eee`. Namun, kolam Curve di baliknya menggunakan alamat WETH. Pemeriksaan `_isETH` menghasilkan `false`, membypass reentrancy lock, dan menguras 1.727 ETH.
   * *Pelajaran Zeltra:* **Zero Dual-Accounting**. Di dalam database, storage, dan sirkuit Zeltra, TIDAK ADA token dengan alamat `0x000...000` atau `0xeee...eee`. Seluruh simpanan dinormalisasi secara kanonikal ke alamat **WETH9** (`0x980B62Da83Eff3D4576C647993b0c1D7faf17c73` di Sepolia / `0x82aF49447D8a07e3bd95BD0d56f35241523fBab1` di One). Deposit ETH native langsung dibungkus menjadi WETH9 pada instruksi pertama fungsi payable sebelum menyentuh state internal.
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

## 5. Spesifikasi Teknis & Solvensi Matematika (Disinkronkan ke Pure ZK-UTXO & DEC-038)

### A. Isolasi Solvensi Total (Siloed Multi-Asset Solvency)
Mengikuti pemusnahan total sesi legacy dan liability refund pada [`DEC-038B`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-038B-contract-direct-mmr-deposit-and-storage-cleanup.md), seluruh akuntansi liabilitas Zeltra kini murni terdiri dari liabilitas note dan akumulasi fee eksekusi relayer.

Untuk memastikan pembekuan atau anomali pada satu aset tidak pernah merembes ke aset lainnya, smart contract Stylus memisahkan akuntansi solvensi per `asset_id`:

$$\forall a \in \{\text{WETH}, \text{USDC}\}: \quad \text{ContractBalance}(a) \ge \text{TotalLiabilities}(a)$$

Di mana liabilitas murni dihitung sebagai:
$$\text{TotalLiabilities}(a) = \text{UserNoteLiability}(a) + \text{AccruedExecutionFeeLiability}(a) + \text{RealizedProtocolFees}(a)$$

Jika kontrak USDC mengalami pembekuan eksternal oleh Circle:
1. Payout transaksi USDC akan gagal di layer ERC-20 (`FiatTokenV2.transfer()`).
2. **Namun, kolam WETH tetap 100% aktif dan dapat dibelanjakan tanpa gangguan**, karena seluruh mapping storage, saldo token, dan verifikasi solvensi terisolasi secara kriptografis dan matematis.

```rust
// zeltra-contracts/src/storage.rs (Dual-Asset Evolution)
sol_storage! {
    #[entrypoint]
    pub struct Zeltra {
        // ... parameter sistem lainnya ...

        // Mapping token contracts per asset_id (0: WETH9, 1: USDC)
        mapping(uint256 => address) asset_tokens;

        // Multi-liability terisolasi per asset_id (DEC-034 & DEC-038B)
        mapping(uint256 => uint256) user_note_liabilities;
        mapping(uint256 => uint256) accrued_execution_fee_liabilities;
        mapping(uint256 => uint256) realized_protocol_fees;
        mapping(uint256 => uint256) accumulated_execution_fees;
        
        // Canonical WETH9 address & Gateway
        address weth9_address;
    }
}
```

---

### B. Kriptografi Komitmen: Poseidon-W5 Bit-Packing & Konservasi Arity
Struktur Poseidon-W5 (`native_poseidon_w5`) di [`zeltra-core/src/note.rs`](file:///workspaces/Zeltra-Protocol/zeltra-core/src/note.rs#L441-L448) memiliki kapasitas tepat 4 elemen input lapangan (ditambah 1 domain separator):
$$\text{State} = [D, I_0, I_1, I_2, I_3]$$

Untuk mencegah perubahan width Poseidon ke width-6 yang akan merusak round constants terverifikasi Grain-128 LFSR ([`DEC-021`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-021-formal-specification-bls12-381-poseidon-and-groth16.md)), Zeltra menerapkan **Canonical Field Bit-Packing**:

1. **Format Payload Input 0 (`value_packed`):**
   Karena rentang nilai `value` dijamin masuk dalam `u64` (64 bit), dan field Arkworks BLS12-381 $\text{Fr}$ memiliki kapasitas $\approx 254.7$ bit:
   $$\text{value\_packed} = \text{Fr::from}(value) + \text{Fr::from}(asset\_id) \cdot 2^{64}$$
2. **Formula Komitmen Leaf Note:**
   $$\text{Commitment} = \text{Poseidon\_W5}(\text{value\_packed}, \text{owner\_key}, \text{rho}, \text{randomness}; \text{domain\_note\_commitment})$$
3. **In-Circuit Range Constraints & De-aliasing:**
   Di dalam R1CS sirkuit (`PrivateNoteCircuit` dan `JoinSplitCircuit`):
   - `value` dibuktikan memenuhi range check 64-bit: $v \in [0, 2^{64}-1]$.
   - `asset_id` dibuktikan memenuhi boolean check: $a \cdot (1 - a) == 0 \implies a \in \{0, 1\}$.
   - Hubungan linear dipaksakan:
     $$\text{value\_packed\_var} == \text{value\_var} + \text{asset\_id\_var} \cdot 2^{64}$$
   Hal ini menjamin zero-aliasing dan membuktikan integritas `asset_id` tanpa menambah dimensi state Poseidon.

---

### C. Sinkronisasi dengan DEC-037: 48-Byte `NotePlaintext` & 96-Byte AEAD Memo
Pada [`DEC-037`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-037-note-encryption-memo-and-viewing-key-recovery.md), memo komitmen terenkripsi dipatok tepat **96 byte** (32 byte EPK, 48 byte ciphertext, 16 byte Poly1305 MAC tag).

Struktur `NotePlaintext` 48 byte disinkronkan sebagai berikut:
```text
┌────────────────────────────────────────────────────────────────────────┐
│                   DEC-037 48-BYTE NOTE PLAINTEXT LAYOUT                 │
├──────────────┬──────────────┬──────────────┬───────────────────────────┤
│ Offset (B)   │ Ukuran (B)   │ Tipe         │ Deskripsi                 │
├──────────────┼──────────────┼──────────────┼───────────────────────────┤
│ 00 .. 07     │ 8            │ u64 (LE)     │ Note Value (Gwei / Micro) │
│ 08 .. 39     │ 32           │ [u8; 32]     │ Note Nullifier Seed (rho) │
│ 40 .. 43     │ 4            │ u32 (LE)     │ Epoch ID                  │
│ 44           │ 1            │ u8           │ Asset ID (0: WETH, 1:USDC)│
│ 45 .. 47     │ 3            │ [u8; 3]      │ Reserved (Zero Padding)   │
└──────────────┴──────────────┴──────────────┴───────────────────────────┘
```
**Keunggulan:** Field `flags` 4-byte pada spesifikasi dasar DEC-037 dipetakan secara terstruktur: byte ke-44 menyimpan `asset_id`. Ukuran ciphertext tetap persis 48 byte, sehingga trial decryption Incoming Viewing Key (IVK) langsung mengetahui jenis aset note tanpa mengubah ukuran log on-chain `NoteCommitmentAppended`.

---

### D. Normalisasi Skala: 18 Desimal vs 6 Desimal (Gwei Precision)
* **USDC (`asset_id: 1`):** Menggunakan 6 desimal ($1\text{ USDC} = 10^6\text{ micro-units}$).
* **WETH (`asset_id: 0`):** Menggunakan 18 desimal pada kontrak ERC-20 WETH9 dasar.
* **Normalisasi In-Circuit (Gwei Scale):**
  Untuk menghindari overflow integer pada range check 64-bit sirkuit Groth16 ($2^{64}-1 \approx 18.44 \times 10^{18}$ wei):
  - Di dalam sirkuit dan private note wallet, nilai WETH dinyatakan dalam satuan **Gwei (9 desimal)**:
    $$1\text{ Gwei} = 10^9\text{ Wei}$$
  - Rentang `u64` mampu menampung hingga **$18{,}446{,}744{,}073\text{ Gwei} \approx 18.4\text{ Miliar WETH}$**, mencakup suplai sirkulasi Ethereum tanpa truncate.
  - Saat interaksi on-chain `deposit` dan `transfer` ke pengguna:
    - Deposit WETH menerima Wei, divalidasi habis dibagi $10^9$, dan dicatat sebagai Gwei di MMR commitment:
      $$\text{gwei\_value} = \text{wei\_amount} / 10^9, \quad \text{assert}(\text{wei\_amount} \pmod{10^9} == 0)$$
    - Payout WETH mengalikan Gwei kembali ke Wei saat memanggil WETH9 `transfer()`:
      $$\text{wei\_payout} = \text{gwei\_payout} \times 10^9$$

---

### E. Integrasi Public Input Sirkuit ZK (16-PI & 19-PI Preserved)

Untuk menjaga kompatibilitas verifier on-chain Stylus EIP-2537 (`0x0c` MSM + `0x0f` Pairing) tanpa menambah biaya gas publik input:

1. **Pada `PrivateNoteCircuit` (DEC-035A, 16 Public Inputs):**
   Elemen ke-15 (`flags_packed`, menggantikan boolean `has_change` dan `is_rollover` sederhana) mengemas:
   - Bit 0: `has_change` (0 atau 1)
   - Bit 1: `is_rollover` (0 atau 1)
   - Bit 2..=9: `asset_id` (0 untuk WETH, 1 untuk USDC)
2. **Pada `JoinSplitCircuit` (DEC-036A, 19 Public Inputs):**
   Elemen ke-17 (`flags_packed`) yang telah ada mengemas:
   - Bit 0: `has_change_1`
   - Bit 1: `has_change_2`
   - Bit 2: `is_rollover`
   - Bit 3..=10: `asset_id`
3. **Konservasi Nilai Intra-Aset Mutlak (Anti-Penumbra Exploit):**
   $$\text{asset\_id}_{in1} == \text{asset\_id}_{in2} == \text{asset\_id}_{out1} == \text{asset\_id}_{out2} == \text{public\_asset\_id}$$
   Tidak ada pertukaran silang aset di dalam sirkuit. Semua nilai input, payout merchant, protocol fee, relayer fee, dan change note terikat pada `asset_id` yang sama.

---

### F. Inflow Murni ZK-UTXO: Direct MMR Deposit (DEC-038B Compatible)
Tidak ada lagi sesi timelock, tidak ada lagi kunci masking $k$, dan tidak ada lagi registrasi perantara.

```rust
// zeltra-contracts/src/deposit.rs (DEC-034 + DEC-038B Direct Inflow)
impl Zeltra {
    /// 1. Direct USDC Deposit (Asset ID 1)
    pub fn deposit_direct_note(
        &mut self,
        amount: U256,
        note_commitment: FixedBytes<32>,
        encrypted_memo: Bytes,
    ) -> Result<(), Vec<u8>> {
        self._deposit_direct_erc20(1, amount, note_commitment, encrypted_memo)
    }

    /// 2. Direct WETH ERC-20 Deposit (Asset ID 0)
    pub fn deposit_direct_weth(
        &mut self,
        amount_wei: U256,
        note_commitment: FixedBytes<32>,
        encrypted_memo: Bytes,
    ) -> Result<(), Vec<u8>> {
        if amount_wei % U256::from(1_000_000_000) != U256::ZERO {
            return Err(b"WETH_AMOUNT_NOT_GWEI_ALIGNED".to_vec());
        }
        self._deposit_direct_erc20(0, amount_wei, note_commitment, encrypted_memo)
    }

    /// 3. Direct Native ETH Auto-Wrap Deposit (Payable - Asset ID 0)
    #[payable]
    pub fn deposit_direct_native_eth(
        &mut self,
        note_commitment: FixedBytes<32>,
        encrypted_memo: Bytes,
    ) -> Result<(), Vec<u8>> {
        self.check_not_paused()?;
        let amount_wei = self.msg_value();
        
        // Min 0.005 ETH (5_000_000 Gwei)
        if amount_wei < U256::from(5_000_000_000_000_000u64) {
            return Err(b"AMOUNT_TOO_SMALL".to_vec());
        }
        if amount_wei % U256::from(1_000_000_000) != U256::ZERO {
            return Err(b"ETH_VALUE_NOT_GWEI_ALIGNED".to_vec());
        }

        // CEI 1: Wrap ETH ke WETH9 seketika via deposit() payable
        let weth_addr = self.weth9_address.get();
        let iweth = IWeth9::new(weth_addr);
        let host = Self::runtime_host();
        iweth.deposit(&host, Call::new_mutating(self).value(amount_wei))
            .map_err(|_| b"WETH_WRAP_FAILED".to_vec())?;

        // CEI 2: Effects - Insert ke MMR & mutasi user_note_liabilities[0]
        let gwei_amount = amount_wei / U256::from(1_000_000_000);
        self._mmr_insert(note_commitment, encrypted_memo.to_vec())?;
        
        let current_liab = self.user_note_liabilities.get(U256::ZERO);
        self.user_note_liabilities.insert(U256::ZERO, current_liab + gwei_amount);

        // Verifikasi Solvensi Terisolasi
        self.check_asset_solvency_invariant(0)?;
        Ok(())
    }
}
```

---

### G. Zero-Oracle Relayer Reimbursement
Relayer Zeltra tidak bergantung pada Price Oracle eksternal (Chainlink/Pyth):
* Biaya eksekusi gas untuk pembelanjaan note USDC dipotong dari saldo note dalam bentuk **USDC** (menggunakan kuotasi gas L2 yang dijamin EIP-712).
* Biaya eksekusi gas untuk pembelanjaan note WETH dipotong dari saldo note dalam bentuk **WETH** (yang secara langsung merupakan mata uang gas dasar Arbitrum).
* **Hasil:** Sistem kebal terhadap manipulasi flash loan oracle, downtime API pihak ketiga, atau eksploitasi arbitrase nilai tukar.

---

## 6. Pertahanan Regulasi & Tameng Blacklist (Sinergi dengan DEC-026)

Keputusan mempertahankan USDC didukung oleh mekanisme pertahanan aktif yang telah ditetapkan dalam [`DEC-026 (Receiver-Enforced Compliance & Zero-Cost Sanctions Filtering)`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-026-receiver-enforced-compliance-zero-cost-sanctions-filtering-relayer-protection.md):

1. **Pencegahan Eksploitasi Kriminal di Pintu Depan:** Relayer Zeltra menolak membroadcast transaksi dari/ke alamat yang terdaftar dalam daftar sanksi OFAC / Chainalysis oracle.
2. **Receiver-Enforced Acceptance:** Merchant dan penerima institusional memverifikasi bukti non-sanksi sebelum menerima penyelesaian dana.
3. **Hasil Strategis:** Zeltra secara aktif mencegah masuknya dana teroris, pencucian uang hacker (Lazarus Group), atau aset curian. Tanpa adanya eksploitasi kriminal, Circle maupun regulator tidak memiliki dasar hukum objektif untuk mem-blacklist smart contract Zeltra, menjaga kelangsungan kolam komersial USDC tetap aman.

---

## 7. Rangkuman Perubahan Dibandingkan Draf Lama

```text
┌────────────────────────────────────────────────────────────────────────────────────────┐
│               PERBANDINGAN DEC-034 LAMA VS DEC-034 DISINKRONKAN CODEBASE              │
├──────────────────────────┬──────────────────────────────┬──────────────────────────────┤
│ Aspek                    │ Draf DEC-034 Lama            │ DEC-034 Baru (Code Synced)   │
├──────────────────────────┼──────────────────────────────┼──────────────────────────────┤
│ Arsitektur Inflow        │ Session ID + Masking Key k   │ Direct MMR Inflow (DEC-038B) │
│ Liabilitas Refund        │ Ada (RefundableLiability)    │ Dihapus total (DEC-038A)     │
│ Struktur Poseidon        │ Width-5 5 input (Over-arity) │ Canonical Bit-Packing u64+u8 │
│ Public Inputs Layout     │ Belum terdefinisi ketat      │ Preserved 16-PI & 19-PI      │
│ Note Memo Metadata       │ Belum terdefinisi            │ DEC-037 48-B Plaintext Byte44│
│ Akuntansi Solvensi       │ Global + Refund              │ Siloed per-asset (Invariant) │
│ Wrap Inflow UX           │ Session-based auto-wrap      │ Direct Atomic Payable Wrap   │
└──────────────────────────┴──────────────────────────────┴──────────────────────────────┘
```

---

## 8. Matriks Pengujian & Verifikasi (Positive & Negative Test Suite)

Mengikuti invariant baku Zeltra Protocol, implementasi wajib menyertakan minimal 2x tes negatif terhadap tes positif:

### A. Positive Tests
1. `test_dual_asset_usdc_direct_deposit_and_spend_success`: Direct deposit USDC, cetak ZK note `asset_id: 1`, dan belanjakan note dengan verifikasi Groth16 sukses.
2. `test_dual_asset_weth_payable_wrap_deposit_and_spend_success`: Direct deposit native ETH via `deposit_direct_native_eth()` otomatis terbungkus ke WETH9, mencetak ZK note `asset_id: 0`, dan payout WETH sukses.
3. `test_dual_asset_siloed_solvency_independent_balances`: Verifikasi bahwa liabilitas dan saldo `asset_id: 0` dan `asset_id: 1` dihitung secara independen tanpa saling mensubsidi.
4. `test_dual_asset_gwei_scale_normalization_precision`: Nilai WETH berdenominasi 9 desimal (Gwei) dalam sirkuit berhasil dikonversi ke 18 desimal (Wei) saat transfer on-chain tanpa kehilangan presisi.

### B. Negative Tests (Pertahanan Batas & Anti-Eksploit 2024–2026)
1. `test_dual_asset_cross_asset_spend_rejection`: Percobaan membelanjakan note USDC (`asset_id: 1`) untuk mencairkan payout WETH (`asset_id: 0`) wajib gagal pada verifikasi constraint sirkuit (`ASSET_ID_MISMATCH`).
2. `test_dual_asset_unregistered_asset_id_rejection`: Percobaan membuat note dengan `asset_id >= 2` ditolak secara fail-closed oleh smart contract Stylus (`UNSUPPORTED_ASSET_ID`).
3. `test_dual_asset_caller_supplied_token_bypass_blocked`: Percobaan caller mengoper alamat token ERC-20 arbitrer diabaikan oleh kontrak; kontrak hanya menggunakan alamat token kanonikal dari storage (`SEMANTIC_BINDING_MISMATCH`).
4. `test_dual_asset_usdc_freeze_isolation_weth_spend_unaffected`: Simulasi pembekuan transfer pada kontrak USDC membuktikan bahwa kolam WETH tetap dapat memproses deposit, verifikasi ZK, dan spend tanpa revert.
5. `test_dual_asset_native_eth_zero_value_revert`: Pemanggilan `deposit_direct_native_eth()` dengan `msg_value() == 0` wajib revert seketika (`AMOUNT_TOO_SMALL`).
6. `test_dual_asset_gwei_unaligned_wei_revert`: Deposit native ETH atau WETH yang tidak habis dibagi $10^9$ wei wajib revert (`ETH_VALUE_NOT_GWEI_ALIGNED`).
7. `test_dual_asset_gwei_field_overflow_rejection`: Percobaan menginjeksi nilai WETH melebihi kapasitas `u64` Gwei ditolak oleh range check sirkuit Groth16.
8. `test_dual_asset_relayer_cross_asset_fee_drain_blocked`: Upaya memotong fee eksekusi relayer dalam aset WETH dari note USDC ditolak oleh relayer spend handler (`RELAYER_FEE_ASSET_MISMATCH`).

---

## 9. Kesimpulan

Dengan disinkronkannya **DEC-034** ke arsitektur **Pure ZK-UTXO (DEC-038)**:
1. Seluruh rancangan multi-asset kini **100% selaras dengan fondasi codebase yang ada**, menghilangkan seluruh kode usang sesi dan threshold BLS.
2. Poseidon-W5 tetap mempertahankan efisiensi state width-5 berkat **canonical field bit-packing** (`value_packed = value + asset_id * 2^64`).
3. Kompatibel penuh dengan **DEC-037 Viewing Key Recovery (96-byte AEAD memo)** dan susunan **Public Inputs 16-PI / 19-PI**.
4. Menjadi cetak biru arsitektur resmi ketika protokol beralih dari single-asset USDC ke dual-asset USDC + WETH.

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

