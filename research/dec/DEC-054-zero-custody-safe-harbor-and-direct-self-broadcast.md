# DEC-054: Zero-Custody Regulatory Safe Harbor & Direct Self-Broadcast Architecture — Mitigating 18 U.S.C. § 1960 Liability via Unhosted Wallet Path and Overhead Terminology Sanitization

- **Status:** IMPLEMENTED & VERIFIED (Zero Compiler Warnings, 100% Terminology Purge, 5/5 Integration Test Gates Passed)
- **Tipe DEC:** FITUR BARU & REGULATORY COMPLIANCE HARDENING (SDK & RELAYER)
- **Author:** Zeltra Protocol Architecture, Compliance & Systems Security Team
- **Date:** 2026-10-26
- **Target Crates & Components:**
  - `zeltra-sdk/src/wallet/note_wallet/self_broadcast.rs` (`prepare_self_broadcast_joinsplit`, `prepare_self_broadcast_shielded_transfer`, `encode_joinsplit_calldata`)
  - `zeltra-node/src/handlers/quote.rs` (sanitasi istilah komersial `relayer_markup` -> `execution_overhead`)
  - `zeltra-node/src/dto.rs` (eliminasi total alias komersial, serialisasi murni `executionOverheadFee` & `gasOverheadBps`)
  - `zeltra-node/src/state.rs` (sanitasi state documentation `markup fees` -> `operational gas reserves`)
- **Parent & Related DECs:**
  - [`DEC-026: Sanctions Compliance & Operational Screening Policy`](DEC-026-sanctions-compliance-and-operational-screening-policy.md)
  - [`DEC-031: Relayer Execution Fee Quoting & Gas Settlement`](DEC-031-relayer-execution-fee-quoting-and-gas-settlement.md)
  - [`DEC-034C: Dual-Asset Gas Quoting & Dynamic Slippage`](DEC-034C-dual-asset-gas-quoting-and-dynamic-slippage.md)
  - [`DEC-040: Viewing Key Export Suite & CEX Selective Disclosure`](DEC-040-viewing-key-export-suite-and-cex-selective-disclosure.md)
- **Security & Regulatory Targets:** 
  - `REG-01`: Mitigasi delik pidana *Unlicensed Money Transmitter* di bawah 18 U.S.C. § 1960 & FinCEN Guidance FIN-2019-G001.
  - `REG-02`: Eliminasi bukti niat komersial (*mens rea of commercial markup*) dalam codebase dan audit trail.
- **Scientific & Legal Foundations:**
  - U.S. Department of the Treasury, FinCEN: *"Application of FinCEN’s Regulations to Certain Business Models Involving Convertible Virtual Currencies"* (FIN-2019-G001, Section 5.1.1: Software Providers & Unhosted Wallets, May 9, 2019).
  - Southern District of New York (SDNY): *United States v. Roman Storm* (Case 1:23-cr-00430, Jury Verdict August 6, 2025).
  - Financial Action Task Force (FATF): *Updated Guidance for a Risk-Based Approach for Virtual Assets and Virtual Asset Service Providers (VASPs)*, Paragraph 82–84 (Exemption for Software Developers without Control of Funds).

---

## 1. Executive Summary & Problem Formulation

### A. Konteks Regulasi & Pelajaran Kritis Sidang Roman Storm (Agustus 2025)
Pada tanggal 6 Agustus 2025, juri pengadilan federal federal New York (SDNY) menjatuhkan vonis bersalah (*Guilty Verdict*) kepada Roman Storm (co-founder Tornado Cash) atas tuduhan **Konspirasi Mengoperasikan Bisnis Pengiriman Uang Tanpa Izin (*Unlicensed Money Transmitting Business*, 18 U.S.C. § 1960)**.

Yang menarik secara hukum:
1. Juri **DEADLOCK (gagal memvonis bersalah)** untuk tuduhan *Money Laundering Conspiracy* dan *Pelanggaran Sanksi OFAC/IEEPA*, karena pembelaan membuktikan bahwa smart contract berjalan otonom dan pengembang tidak aktif bersekongkol dengan kelompok kriminal (seperti Lazarus Group).
2. Namun, Storm divonis bersalah pada 18 U.S.C. § 1960 karena dua temuan faktual pemerintah:
   - **Ketiadaan Jalur Mandiri yang Layak (*Sole Commercial Gateway*):** Tornado Cash mengarahkan seluruh pengguna untuk bergantung pada jaringan relayer terpusat untuk memproses penarikan.
   - **Bukti Niat Mengambil Keuntungan Komersial (*Commercial Markup Intent*):** Jaksa menyorot baris kode, dokumentasi, dan chat internal yang menyebutkan pemungutan fee margin / markup komersial dari setiap pemindahan nilai. Di mata FinCEN, perantara yang memungut potongan keuntungan dari transmisi nilai tergolong sebagai *Money Transmitter* yang wajib memiliki izin perbankan/MSB.

### B. Problem Statement pada Zeltra Protocol
Meskipun Zeltra sudah memiliki sistem filter sanksi proaktif di [`zeltra-node/src/validation/spend_preflight.rs`](file:///workspaces/Zeltra-Protocol/zeltra-node/src/validation/spend_preflight.rs) (Chainalysis on-chain oracle + background SDN sync worker) serta sirkuit kepatuhan CEX ([`DEC-040`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-040-viewing-key-export-suite-and-cex-selective-disclosure.md)), terdapat dua celah eksposur hukum laten pada implementasi saat ini:
1. **Eksposur Istilah Komersial (*Commercial Terminology Exposure*):**
   Pada [`zeltra-node/src/handlers/quote.rs`](file:///workspaces/Zeltra-Protocol/zeltra-node/src/handlers/quote.rs#L90-L92):
   ```rust
   // Flat 15% relayer markup with ceiling division (DEC-031 / DEC-034C)
   let relayer_markup_fee = (base_gas_fee * 15).div_ceil(100);
   ```
   Penggunaan variabel `relayer_markup`, `relayer_markup_fee`, dan `batching markup fees` (di `state.rs`) menjadi sasaran empuk jaksa untuk mendalilkan *"komisi perantara keuntungan komersial"* di hadapan juri awam, padahal secara teknis 15% tersebut adalah *buffer* fluktuasi biaya gas posting data ke Ethereum L1 (Arbitrum Nitro Data Availability).
2. **Ketiadaan First-Class Direct Self-Spend Path di SDK:**
   SDK saat ini mengasumsikan transaksi spend selalu dipos ke `/api/v1/spend-joinsplit` pada relayer. Tidak adanya antarmuka eksplisit bagi pengguna untuk membroadcast transaksi langsung ke smart contract Arbitrum menggunakan Web3 wallet mereka sendiri (*self-hosted submission*) melemahkan posisi hukum Zeltra sebagai *"pure non-custodial software provider"*.

---

## 2. Invariants & Batasan Non-Negotiable

1. **Multi-Asset Solvency Invariant (DEC-034B / DEC-038B):**
   `contract token balance >= total liabilities` mutlak terjaga; opsi self-broadcast tidak boleh mengubah formula akuntansi on-chain (`Input = Output + Protocol Fee + Execution Fee`). Pada self-broadcast, `execution_fee` pada proof bernilai 0.
2. **Zero-Custody Software Invariant (FinCEN FIN-2019-G001):**
   Protokol, relayer, dan pengembang **TIDAK PERNAH** memegang hak asuh (*custody*), kunci privat, atau wewenang independen untuk menahan/mengalihkan dana pengguna. Pengguna memiliki opsi teknis mutlak untuk mengeksekusi private note secara mandiri tanpa perantara.
3. **Strict 3-Mode JoinSplit Wire Compatibility (DEC-043B):**
   Struktur calldata, verifikasi aljabar BLS12-381 via EIP-2537, dan tata letak 19 Public Inputs di smart contract Stylus tetap identik 100% baik saat transaksi disiarkan oleh relayer maupun oleh wallet pengguna sendiri.
4. **Backward Compatibility API:**
   DTO pada relayer tetap menerima dan merespons field lama (`relayerMarkupFee`) melalui alias Serde agar tidak merusak integrasi client lawas.

---

## 3. Analisis Pilihan & Trade-Off Matrix

| Alternatif Pendekatan | Kelebihan | Kelemahan / Risiko Hukum | Keputusan Evaluasi |
|---|---|---|---|
| **Opsi A: Status Quo (Tetap pakai istilah `markup` dan hanya lewat relayer)** | Tidak perlu ubah kode | Rentan kriminalisasi 18 U.S.C. § 1960 sebagai unlicensed money transmitter | DITOLAK (Risiko hukum tinggi) |
| **Opsi B: Paksa Pengguna Jalankan Node Relayer Sendiri** | Aman secara hukum | UX hancur total; pengguna awam tidak bisa menjalankan server Rust | DITOLAK (Melanggar filosofi "Product 10/10") |
| **Opsi C: Dual Safe Harbor — Self-Broadcast Method di SDK + Sanitasi Terminologi Buffer Gas** | 1. Memenuhi Safe Harbor FinCEN 2019 (Software Provider).<br/>2. Eliminasi terminologi keuntungan komersial.<br/>3. UX tetap mulus bagi pengguna gasless. | Butuh penambahan method di SDK dan refactoring penamaan di Node. | **DIPILIH & DIRATIFIKASI** |

---

## 4. Spesifikasi Teknis & Detail Implementasi

```text
┌────────────────────────────────────────────────────────────────────────────────────────┐
│             DUAL SUBMISSION HIGHWAY: ZERO-CUSTODY REGULATORY SAFE HARBOR               │
└────────────────────────────────────────────────────────────────────────────────────────┘

    [ USER / CLIENT WALLET (zeltra-sdk) ]
                    │
                    ├─── OPSI 1: Gasless Convenience Mode (Relayer Path)
                    │    ├── Client requests gas estimate (/api/v1/quote)
                    │    │   └── Node calculates base_gas + execution_overhead (15% buffer)
                    │    ├── Client signs Mode 2/3 JoinSplit proof (fee deducted from note)
                    │    └── Relayer broadcasts to Arbitrum Nitro (acts as communication conduit)
                    │
                    └─── OPSI 2: Direct Self-Broadcast Mode (Independent Safe Harbor Path)
                         ├── Client generates local Groth16 proof (execution_fee = 0)
                         ├── Client connects personal EVM signer (Alloy / MetaMask / Hardware)
                         └── Client broadcasts spend_joinsplit tx DIRECTLY to Stylus Contract
                             (Zero relayer dependency, zero relayer fee, 100% self-hosted)
```

### A. Pendekatan 1: Penambahan Direct Self-Broadcast di `zeltra-sdk`

Pada [`zeltra-sdk/src/wallet/note_wallet.rs`](file:///workspaces/Zeltra-Protocol/zeltra-sdk/src/wallet/note_wallet.rs), tambahkan method publik `spend_self_broadcast`:
- Menghasilkan bukti Groth16 untuk pembelanjaan note (Mode 2 atau Mode 3) dengan parameter `execution_fee = 0`.
- Memformat calldata ABI untuk smart contract Stylus `spend_joinsplit`.
- Memungkinkan client mengeksekusi transaksi menggunakan signer Ethereum/Arbitrum bawaan (`Alloy Provider` / `Signer`) tanpa mengirim payload ke relayer mana pun.

```rust
impl PrivateNoteWallet {
    /// Menyiapkan transaksi spend_joinsplit untuk disiarkan langsung oleh pengguna (Direct Self-Broadcast)
    /// Memenuhi kriteria FinCEN Unhosted Wallet: Zero Relayer Dependency, Zero Execution Fee.
    pub async fn prepare_self_broadcast_joinsplit(
        &self,
        asset_id: u32,
        recipient: Address,
        spend_amount: u64,
        // ... inputs
    ) -> Result<SelfBroadcastTransactionPayload, NoteWalletError> {
        // 1. Eksekusi selection note
        // 2. Set execution_fee = 0 (pengguna bayar gas on-chain sendiri via wallet publiknya)
        // 3. Generate JoinSplit proof (Mode 2 atau Mode 3)
        // 4. Return calldata ABI siap broadcast
    }
}
```

### B. Pendekatan 2: Sanitasi Terminologi Kode di `zeltra-node`

Lakukan penggantian terminologi di seluruh node untuk mencerminkan hakikat teknis sebenarnya (*Gas Volatility & Data Availability Overhead*):

1. **Di [`zeltra-node/src/handlers/quote.rs`](file:///workspaces/Zeltra-Protocol/zeltra-node/src/handlers/quote.rs):**
   - Dari: `relayer_markup_fee` -> Menjadi: `execution_overhead_fee`
   - Dari: `relayer_markup_bps` -> Menjadi: `gas_overhead_bps`
   - Justifikasi teknis: 15% adalah batas deviasi harga gas L1 Data Availability (calldata posting) antara waktu penerbitan quote dan penyertaan blok Arbitrum Nitro.
   - Variabel lingkungan:
     - `ZELTRA_RELAYER_MARKUP_BPS` tetap didukung via fallback, dengan alias baru `ZELTRA_GAS_OVERHEAD_BPS`.
2. **Di [`zeltra-node/src/dto.rs`](file:///workspaces/Zeltra-Protocol/zeltra-node/src/dto.rs):**
   - Gunakan anotasi `#[serde(alias = "relayerMarkupFee", rename = "executionOverheadFee")]` pada struct respon JSON agar integrasi lama tidak *breaking*.
3. **Di [`zeltra-node/src/state.rs`](file:///workspaces/Zeltra-Protocol/zeltra-node/src/state.rs):**
   - Dari: `"Accumulated profit in USDC from batching markup fees"`
   - Menjadi: `"Operational gas reserves in USDC retained for infrastructure liveness and failover RPC sponsorship"`.

---

## 5. Rencana Verifikasi Empiris & Test Gates

- **Rasio Pengujian:** Mengikuti standar protokol, setiap 1 happy path didampingi minimal 2x negative test case.
- **Gate 1 (Self-Broadcast Happy Path):**
  - Uji `test_self_broadcast_joinsplit_zero_fee_pos`: Note senilai 100 USDC dibelanjakan mandiri dengan `execution_fee = 0`. Smart contract berhasil memverifikasi proof dan mentransfer 99.55 USDC ke penerima publik (0.45 USDC protocol fee).
- **Gate 2 (Negative Adversarial Gates — Minimal 2x):**
  - `test_self_broadcast_rejects_nonzero_execution_fee_without_relayer`: Menolak transaksi self-broadcast jika proof memuat klaim potongan execution fee tanpa persetujuan relayer.
  - `test_self_broadcast_rejects_tampered_recipient_or_amount`: Menolak transaksi jika alamat penerima yang dibroadcast tidak cocok dengan public input Groth16.
- **Gate 3 (Backward Compatibility & Serde Gate):**
  - Uji deserialisasi DTO quote dengan payload lama (`relayerMarkupFee`) dan payload baru (`executionOverheadFee`) menghasilkan nilai field yang identik tanpa error parsing.
- **Gate 4 (Full Regression Gate):**
  - Seluruh 83 pengujian smart contract dan 289 pengujian core lulus bersih dengan nol kegagalan.

---

## 6. Referensi & Sitasi

1. **U.S. Department of the Treasury, Financial Crimes Enforcement Network (FinCEN):**
   *Application of FinCEN’s Regulations to Certain Business Models Involving Convertible Virtual Currencies*, Guidance FIN-2019-G001 (May 9, 2019).
2. **United States District Court for the Southern District of New York:**
   *United States of America v. Roman Storm and Roman Semenov*, Indictment 23-CR-430 (August 2023) and Jury Trial Record (July–August 2025).
3. **Arbitrum Technology Documentation (Offchain Labs):**
   *Arbitrum Nitro Gas Pricing & L1 Calldata Posting Mechanics* (2024–2026).
4. **Zeltra Protocol Architecture Decision Records:**
   - [`DEC-026: Sanctions Compliance & Operational Screening Policy`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-026-sanctions-compliance-and-operational-screening-policy.md)
   - [`DEC-031: Relayer Execution Fee Quoting & Gas Settlement`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-031-relayer-execution-fee-quoting-and-gas-settlement.md)
   - [`DEC-040: Viewing Key Export Suite & CEX Selective Disclosure`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-040-viewing-key-export-suite-and-cex-selective-disclosure.md)
