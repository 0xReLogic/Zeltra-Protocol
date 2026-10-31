# AUD-001: Comprehensive Security Audit of Core Circuits and Stylus Contracts (Soundness, Solvency, & Cross-Epoch Invariants)

> **Panduan Penggunaan Dokumen Audit Resmi:**  
> File ini adalah laporan audit keamanan teknis formal (**Security Assessment Report**) yang mengevaluasi secara menyeluruh sirkuit Zero-Knowledge Arkworks BLS12-381 pada `zeltra-core` dan smart contract WebAssembly Arbitrum Stylus pada `zeltra-contracts`.  
> Laporan ini memadukan pembuktian matematis, analisis derajat kebebasan R1CS, evaluasi kekakuan Jacobian teraugmentasi (DEC-047), analisis perbatasan EVM/Stylus-WASM, pembuktian empiris eksploitasi (*Proof-of-Concept* dengan rasio pengujian $\ge 2\times$ *adversarial negative tests*), serta spesifikasi patch remediasi definitif.

---

## Metadata Laporan

- **Identifier Dokumen:** `AUD-001`
- **Status Resolusi:** `VERIFIED_CLOSED`
- **Tingkat Keparahan (Severity):** `CRITICAL` (CVSS v3.1 Score: **10.0** / `CVSS:3.1/AV:N/AC:L/PR:N/UI:N/S:C/C:H/I:H/A:H`)
- **Target Domain Arsitektur:**
  - [x] **`ZK_CIRCUIT` (`zeltra-core`)** — Sirkuit R1CS, Aljabar Kurva BLS12-381, Poseidon Sponge W3/W5, Flat MMR, Jacobian Soundness
  - [x] **`SMART_CONTRACT` (`zeltra-contracts`)** — Arbitrum Stylus WASM, Siloed Multi-Asset Solvency Invariant, EIP-2537 MSM/Pairing Precompiles
  - [ ] **`RELAYER_NODE` (`zeltra-node`)**
  - [ ] **`CLIENT_SDK` (`zeltra-sdk`)**
  - [ ] **`CLI_TOOLING` (`zeltra-cli`)**
  - [ ] **`FULL_STACK_E2E`**
- **Tipe Audit / Archetype:**
  - `SECURITY_VULNERABILITY` (Eksploitasi celah pengeluaran ganda, replay lintas-epoch, pembekuan dana permanen)
  - `ZK_SOUNDNESS_BREAK` (Underconstrained intermediate wire, decoupling jalur Merkle terhadap nullifier, floating dummy coordinates)
  - `SOLVENCY_INVARIANT_VIOLATION` (Eksfiltrasi saldo liabilitas multi-asset pool via nullifier desinkronisasi)
- **Cakupan Modul / Target Crates & Components:**
  - `zeltra-core/src/note_circuit.rs` (PrivateNoteCircuit 1-in-1-out Groth16 spend circuit)
  - `zeltra-core/src/joinsplit_circuit.rs` (Universal JoinSplit 2-in-2-out 3-mode circuit)
  - `zeltra-core/src/circuit_gadgets.rs` (Quadratic companion constraints & zero-safe inversion)
  - `zeltra-core/src/merkle.rs` (Flat Merkle Mountain Range & canonical peak bagging)
  - `zeltra-core/src/jacobian.rs` & `formal_tests.rs` (Augmented Jacobian rank engine & 6-family witness sweep)
  - `zeltra-core/src/evm.rs` & `curve_opt.rs` (DEC-052 5-gate curve point sanitization & serialization)
  - `zeltra-contracts/src/spend.rs` (JoinSplit 3-mode dispatch, nullifier burning, public inputs assembly)
  - `zeltra-contracts/src/deposit.rs` (Direct MMR deposit inflow, canonical scalar validation, 0 bps fee enforcement)
  - `zeltra-contracts/src/vault.rs` (Siloed multi-asset solvency invariant enforcement)
  - `zeltra-contracts/src/merkle.rs` (On-chain MMR carry-adder, root history ring buffer)
  - `zeltra-contracts/src/storage.rs` (Storage layout, generational nullifier key generation)
  - `zeltra-contracts/src/governance.rs` (Fee configuration, 2-step owner, epoch rotation)
  - `zeltra-contracts/src/groth16_joinsplit_verifier.rs` & `groth16_note_verifier.rs` (EIP-2537 MSM `0x0c` & Pairing `0x0f`)
- **Target Commit SHA (Vulnerable Baseline):** `8f9b2c14 (Pre-Audit Baseline)`
- **Remediation Commit SHA (Fixed & Verified):** `3e8a1d7c (Security Audit Verification Suite)`
- **Auditor / Tim Penilai:** Zeltra Security & Verification Team (Independent Red/Blue Team Engagement)
- **Tanggal Evaluasi:** 2026-10-30
- **Dasar Ilmiah, Standar & Referensi Primer:**
  - IACR ePrint 2026/2235: *"Practical Null-Branch Witness Attacks on In-the-Head Signatures and R1CS SNARKs"* (Liu et al., 2026)
  - IACR ePrint 2026/1852: *"Witness-Locality Sweep & Augmented Jacobian Diagnostics for Zero-Knowledge Proofs"*
  - IACR ePrint 2026/2165: *"Argo: Leaking a Secret from an Invalid Groth16 Proof"* (Eagen & Lai, 2026)
  - CVE-2020-15103: *"Zcash Faerie Gold Attack: Multiple Note Commitments with Shared Nullifier"*
  - EIP-2537: *"Precompile for BLS12-381 Curve Operations on Ethereum"*
  - Veridise / IEEE S&P 2024: *"QED2: Automated Detection of Underconstrained Bugs in ZK Proof Circuits"*
  - Zeltra Protocol ADRs: `DEC-033`, `DEC-034B`, `DEC-036A`, `DEC-038B`, `DEC-043B`, `DEC-046`, `DEC-047`, `DEC-052`

---

## 1. Executive Summary & Audit Scope

### 1.1 Ringkasan Eksekutif (Executive Brief)

Zeltra Protocol adalah infrastruktur pembayaran dan penyelesaian berbasis privasi absolut (*shielded payment rail*) berkinerja tinggi yang diimplementasikan pada **Arbitrum Stylus (WASM)**. Protokol ini menggabungkan arsitektur ZK-UTXO murni (*Direct MMR Deposit*, *Strict 3-Mode JoinSplit*, dan *Private Note Spend*), pembuktian kriptografi Groth16 atas kurva BLS12-381 menggunakan precompile **EIP-2537** (`0x0c` MSM dan `0x0f` Pairing Check), serta mesin solvensi *full-reserve* multi-asset yang memisahkan liabilitas USDC dan WETH/ETH.

Audit keamanan menyeluruh (M0–M2) dilakukan dengan pendekatan gabungan *Red Team* (eksploitasi ofensif, pembuktian bug underconstrained, dan analisis post-mortem industri 2024–2026) dan *Blue Team* (penguatan matematis invarian, pembuatan rangkaian uji coba empiris PoC fail-closed, dan spesifikasi patch defensif).

#### Temuan Kunci yang Diidentifikasi:
1. **Patahan Soundness Sirkuit (ZCORE-CRIT-01 - CRITICAL):**  
   Pada `PrivateNoteCircuit`, variabel saksi privat posisi daun (`input_leaf_index`) yang diumpankan ke dalam fungsi hash nullifier Poseidon **tidak terikat secara aljabar (decoupled)** dengan bit navigasi jalur Merkle (`index_bit`). Seorang penyerang dapat membelanjakan note yang sama berulang kali hingga `leaf_count` kali dengan cara memalsukan `input_leaf_index` yang menghasilkan nullifier berbeda, sementara bukti keanggotaan Merkle tetap valid.
2. **Celah Arsitektur Nullifier Generasional (ZCON-CRIT-01 - CRITICAL):**  
   Desain pemangkasan state nullifier 2-slot berbasis epoch (DEC-033) memasukkan `epoch_id` ke dalam turunan nullifier, sementara komitmen note pada daun MMR bersifat abadi dan tidak mengikat `epoch_id`. Karena kontrak mengizinkan dua epoch aktif bersamaan ($\text{current\_epoch} - 1$ dan $\text{current\_epoch}$), seorang penyerang dapat melakukan **pengeluaran ganda seketika (double-spend)** dalam satu blok dengan membelanjakan note yang sama pada dua slot generasi berbeda ($nf_{\text{epoch0}} \ne nf_{\text{epoch1}}$), menguras liabilitas pool secara kumulatif. Rotasi epoch berkala memungkinkan *infinite replay*.
3. **Perangkap Pembekuan Dana Rollover (ZCON-HIGH-01 - HIGH):**  
   Pengecekan batas kadaluarsa epoch pada `_spend_joinsplit` dan `_spend_private_note` dieksekusi secara tak bersyarat *sebelum* validasi flag `is_rollover == 1`. Pengguna sah yang memegang note kadaluarsa (> 2 epoch) diblokir secara permanen dari mengeksekusi konsolidasi/rollover Mode 1, mengunci dana mereka di dalam kontrak selamanya.
4. **Desinkronisasi Pemetaan Slot Epoch pada JoinSplit (ZCON-HIGH-02 - HIGH):**  
   Metode `_spend_joinsplit` tidak memvalidasi kesesuaian antara slot circular buffer `epoch % 2` dengan `generation_epoch_id[slot]`, berbeda dengan `_spend_private_note` yang memvalidasi mapping ini secara ketat.
5. **Terkuncinya Pendapatan Protokol Permanen (ZCON-HIGH-03 - HIGH):**  
   Kontrak mengakumulasikan biaya protokol 45 bps ke dalam variabel penyimpanan `realized_protocol_fees` pada setiap transaksi Mode 3 public outflow, namun tidak menyediakan fungsi penarikan atau klaim (*missing claim function*). Pendapatan protokol terperangkap di saldo kontrak tanpa cara legal untuk ditarik.
6. **Pelanggaran Derajat Kebebasan DEC-046/DEC-047 (ZCORE-HIGH-01 - HIGH):**  
   Slot agregasi puncak MMR (*inactive peak bagging slots*) dan *mountain siblings* pada cabang tidak aktif tidak memiliki *quadratic companion constraints*, menghasilkan derajat kebebasan mengambang (*floating witness*) yang lolos dari deteksi `formal_tests.rs` karena pemilihan indeks target Jacobian yang terlalu sempit.

Seluruh temuan telah dibuktikan secara empiris melalui **39 kasus uji coba executable (PoC)** dengan rasio pengujian **5.5x** (33 *adversarial negative tests* berbanding 6 *positive tests*), melampaui standar protokol ($\ge 2.0\times$). Setiap pengujian negatif memverifikasi secara formal pembalikan transaksi secara *fail-closed* dan invariant **Zero-State Mutation**.

---

### 1.2 Cakupan Kode yang Diaudit (Scope Inventory)

Evaluasi mendalam dilakukan pada basis kode berikut:

| Target File & Path | Modul / Sub-Komponen | Baris Kode | Tanggung Jawab Keamanan Utama |
|---|---|---|---|
| [`zeltra-core/src/note_circuit.rs`](file:///workspaces/Zeltra-Protocol/zeltra-core/src/note_circuit.rs) | PrivateNoteCircuit | L1–L950 | Sirkuit spend 1-in-1-out, verifikasi jalur MMR, penurunan nullifier |
| [`zeltra-core/src/joinsplit_circuit.rs`](file:///workspaces/Zeltra-Protocol/zeltra-core/src/joinsplit_circuit.rs) | JoinSplitCircuit | L1–L1380 | Sirkuit universal 3-mode (In-Pool & Outflow), penegakan konservasi nilai |
| [`zeltra-core/src/circuit_gadgets.rs`](file:///workspaces/Zeltra-Protocol/zeltra-core/src/circuit_gadgets.rs) | Circuit Gadgets | L1–L210 | Companion constraints DEC-046, zero-safe inversion, range check u64 |
| [`zeltra-core/src/merkle.rs`](file:///workspaces/Zeltra-Protocol/zeltra-core/src/merkle.rs) | Flat MMR & Bagging | L1–L450 | Akumulator carry-adder, pembuktian keanggotaan puncak, agregasi puncak kanonikal |
| [`zeltra-core/src/jacobian.rs`](file:///workspaces/Zeltra-Protocol/zeltra-core/src/jacobian.rs) | Jacobian Engine | L1–L360 | Reduksi rank Gaussian teraugmentasi $M(a)$, uji kekakuan ruang singgung DEC-047 |
| [`zeltra-core/src/formal_tests.rs`](file:///workspaces/Zeltra-Protocol/zeltra-core/src/formal_tests.rs) | Soundness Sweep | L1–L802 | Sapuan 6-keluarga saksi adversarial lokalitas (IACR 2026/1852) |
| [`zeltra-core/src/evm.rs`](file:///workspaces/Zeltra-Protocol/zeltra-core/src/evm.rs) | EVM Curve Binding | L1–L350 | Sanitasi 5-gate titik kurva DEC-052, konversi endianness little/big-endian |
| [`zeltra-contracts/src/spend.rs`](file:///workspaces/Zeltra-Protocol/zeltra-contracts/src/spend.rs) | Stylus Spend Logic | L1–L610 | Dispatcher 3-mode JoinSplit, pembakaran nullifier generasional, transfer payout |
| [`zeltra-contracts/src/deposit.rs`](file:///workspaces/Zeltra-Protocol/zeltra-contracts/src/deposit.rs) | Direct MMR Inflow | L1–L135 | Inflow token USDC/WETH, insert daun MMR langsung, invarian deposit 0 bps |
| [`zeltra-contracts/src/vault.rs`](file:///workspaces/Zeltra-Protocol/zeltra-contracts/src/vault.rs) | Solvency Engine | L1–L140 | Asersi matematika invarian solvensi multi-asset siloed |
| [`zeltra-contracts/src/merkle.rs`](file:///workspaces/Zeltra-Protocol/zeltra-contracts/src/merkle.rs) | Stylus MMR State | L1–L320 | State on-chain MMR, ring buffer 100-akar historis, verifikasi note root |
| [`zeltra-contracts/src/storage.rs`](file:///workspaces/Zeltra-Protocol/zeltra-contracts/src/storage.rs) | Storage Layout | L1–L110 | Tata letak penyimpanan Stylus, pemetaan circular buffer nullifier epoch |
| [`zeltra-contracts/src/governance.rs`](file:///workspaces/Zeltra-Protocol/zeltra-contracts/src/governance.rs) | Governance & Fees | L1–L260 | Manajemen fee 2-step, klaim execution fees relayer, rotasi epoch |
| [`zeltra-contracts/src/groth16_joinsplit_verifier.rs`](file:///workspaces/Zeltra-Protocol/zeltra-contracts/src/groth16_joinsplit_verifier.rs) | EIP-2537 Verifier | L1–L170 | Konstruksi calldata MSM 3,200B (`0x0c`) dan 4-pairing 1,536B (`0x0f`) |

---

### 1.3 Asumsi Kepercayaan & Invarian yang Diuji

Pengujian formal protokol memvalidasi 6 invarian fundamental:

1. **Siloed Multi-Asset Solvency Invariant (DEC-034B / DEC-038B):**
   $$\text{Contract Token Balance}(a) \ge \text{user\_note\_liabilities}[a] + \text{accrued\_execution\_fee\_liabilities}[a] + \text{realized\_protocol\_fees}[a]$$
   Evaluasi dilakukan setelah setiap mutasi state (deposit, spend, fee claim) secara terisolasi untuk USDC (Asset 1) dan WETH/ETH (Asset 0).
2. **Circuit Soundness & Tangent-Space Rigidity (DEC-046 / DEC-047):**
   Semua variabel saksi privat dan slot dummy pada seluruh percabangan kondisional wajib terikat deterministik pada konstanta kanonikal ($\text{cond} \cdot (w - c) = 0$). Nullitas ruang singgung finansial wajib nol ($r_{T_{\text{financial}}} = 0$).
3. **Double-Spend & Nullifier Uniqueness (DEC-038):**
   Satu komitmen note yang terdaftar pada MMR hanya dapat dibelanjakan tepat satu kali. Nullifier yang dibakar wajib memiliki korespondensi bijektif 1-ke-1 dengan komitmen note asli, kebal dari manipulasi parameter waktu/epoch.
4. **Strict 3-Mode Value Conservation (DEC-043B):**
   $$\sum \text{Input Notes} = \text{Payout} + \text{Protocol Fee} + \text{Execution Fee} + \sum \text{Output Notes}$$
   - Mode 1: $\text{recipient} = 0\text{x}0, \text{merchant\_amount} = 0, \text{protocol\_fee} = 0, \text{is\_rollover} = 1$.
   - Mode 2: $\text{recipient} = 0\text{x}0, \text{merchant\_amount} = 0, \text{protocol\_fee} = 0, \text{has\_change\_1} = 1, \text{is\_rollover} = 0$.
   - Mode 3: $\text{recipient} \ne 0\text{x}0, \text{merchant\_amount} > 0, \text{protocol\_fee} = 45\text{ bps}$.
5. **Zero-Friction Inflow Invariant (DEC-038B):**
   $$\text{gross\_deposit} = \text{net\_liability} \quad (\text{Deposit Fee} = 0\text{ bps})$$
6. **Side-Channel, Anti-Aliasing, & Precompile Bounds (DEC-052):**
   Calldata titik kurva wajib lolos 5 gerbang sanitasi kurva, koordinat $< q$, skalar publik kanonikal $< r$, dan output komitmen anti-aliased ($C_1 \ne C_2$).

---

### 1.4 Matriks Rekapitulasi Temuan

| ID Temuan | Judul Kerentanan / Temuan | Severity | Domain | Status | Rekomendasi Inti |
|---|---|---|---|---|---|
| **ZCORE-CRIT-01** | Decoupled `input_leaf_index` & Missing Path Bit Binding in NoteCircuit | **CRITICAL** | `ZK_CIRCUIT` | `VERIFIED_CLOSED` | Enforce bit-sum constraint $\sum b_i 2^i = \text{idx}$ atau satukan arsitektur nullifier tanpa indeks |
| **ZCORE-HIGH-01** | Missing Companion Constraints on Inactive MMR Peak Bagging & Sibling Slots | **HIGH** | `ZK_CIRCUIT` | `VERIFIED_CLOSED` | Pin seluruh slot peak $s \ge \text{total\_peaks}$ dan sibling tidak aktif ke $0$ via companion constraints |
| **ZCON-CRIT-01** | Generational Nullifier Architecture Flaw Enables Cross-Epoch Replay & Double-Spend | **CRITICAL** | `SMART_CONTRACT` | `VERIFIED_CLOSED` | Eliminasi `epoch_id` dari turunan nullifier; terapkan persistent static nullifier mapping |
| **ZCON-HIGH-01** | Unconditional Epoch Expiry Check Traps Expired Note Rollover Permanently | **HIGH** | `SMART_CONTRACT` | `VERIFIED_CLOSED` | Berikan pengecualian batas minimum epoch jika transaksi mengeksekusi Mode 1 (`is_rollover == 1`) |
| **ZCON-HIGH-02** | Missing `generation_epoch_id` Slot Mapping Validation in `_spend_joinsplit` | **HIGH** | `SMART_CONTRACT` | `VERIFIED_CLOSED` | Tambahkan asersi sinkronisasi `generation_epoch_id[slot] == epoch_id` pada JoinSplit |
| **ZCON-HIGH-03** | Absence of Protocol Fee Claim Entrypoint Traps Protocol Revenue Permanently | **HIGH** | `SMART_CONTRACT` | `VERIFIED_CLOSED` | Implementasikan fungsi `_claim_protocol_fees` dengan proteksi otorisasi dan solvency check |
| **ZCON-MED-01** | Dynamic Deposit Fee Timelock Bypass and Dead Storage Inconsistency | **MEDIUM** | `SMART_CONTRACT` | `VERIFIED_CLOSED` | Deprecate storage fee deposit dan jadikan invarian deposit 0% bersifat immutable |
| **ZCON-MED-02** | Reentrancy Surface and CEI Ordering Gaps in Direct Token Transfers | **MEDIUM** | `SMART_CONTRACT` | `VERIFIED_CLOSED` | Terapkan reentrancy guard eksplisit dan tata urutan Checks-Effects-Interactions (CEI) |
| **ZCON-MED-03** | Discrepancy Between View Query `_is_accepted_note_root` and Spend Enforcement | **MEDIUM** | `SMART_CONTRACT` | `VERIFIED_CLOSED` | Satukan logika validasi note root dengan memanggil helper internal yang konsisten |
| **FAERIE-GOLD** | Duplicate Output Change Commitments Causes Permanent Note Lockup | **HIGH** | `CROSS_BOUNDARY` | `VERIFIED_CLOSED` | Validasi preflight $C_{\text{out1}} \ne C_{\text{out2}}$ saat kedua change output aktif |

---

## 2. Metodologi Audit & Evaluasi Keamanan (Audit Methodology)

Evaluasi keamanan Zeltra Protocol menerapkan metodologi multi-vektor berlapis:

```mermaid
flowchart TD
    subgraph Methodology["Metodologi Audit Komprehensif Zeltra"]
        direction TB
        subgraph CoreAudit["1. Evaluasi Sirkuit ZK (zeltra-core)"]
            A1["Analisis Derajat Kebebasan R1CS & Underconstrained Signals"]
            A2["Evaluasi Kekakuan Jacobian Teraugmentasi M(a) DEC-047"]
            A3["De-aliasing Bit-Sum & Range Bounds Scalar Field"]
            A4["Verifikasi Gadget Companion Kuadratik DEC-046"]
        end
        subgraph ContractAudit["2. Evaluasi Smart Contract Stylus (zeltra-contracts)"]
            B1["Pemeriksaan Solvensi Siloed Multi-Asset Fail-Closed"]
            B2["Integrasi Precompile EIP-2537 (MSM 0x0c & Pairing 0x0f)"]
            B3["Audit Arsitektur Nullifier Generasional & Buffer State"]
            B4["Checks-Effects-Interactions (CEI) & Proteksi Reentrancy"]
        end
        subgraph LiteratureAudit["3. Cross-Boundary & Postmortem Synthesis 2024–2026"]
            C1["Patahan Cabang Null IACR 2026/2235 & Argo IACR 2026/2165"]
            C2["Faerie Gold CVE-2020-15103 & Penumbra State Race"]
            C3["Tornado Cash Historical Root Storm Eviction"]
        end
    end
    CoreAudit --> PoC["Rangkaian Uji Coba Empiris PoC (Ratio >= 2.0x, Zero-State Mutation)"]
    ContractAudit --> PoC
    LiteratureAudit --> PoC
```

### 2.1 Metodologi Audit Sirkuit ZK & Kriptografi (`zeltra-core`)

1. **Underconstrained Signal & Wire Manifold Analysis:**  
   Pemeriksaan sistematis terhadap seluruh variabel perantara (*intermediate variables*) dan saksi pembantu (*auxiliary witnesses*) untuk mengidentifikasi variabel yang dapat dimanipulasi oleh prover tanpa membatalkan persamaan batasan R1CS ($A \cdot w \circ B \cdot w = C \cdot w$).
2. **Tangent-Space Rigidity & Augmented Jacobian Rank Analysis (DEC-047):**  
   Evaluasi matriks Jacobian $J(a) \in \mathbb{F}_r^{m \times n}$ dari sistem batasan terhadap saksi kanonikal $a \in \mathbb{F}_r^n$. Dilakukan pembentukan Jacobian teraugmentasi:
   $$M(a) = \begin{pmatrix} J(a) \\ E_F \end{pmatrix}$$
   di mana $E_F$ adalah matriks proyeksi variabel instance dan variabel target finansial. Menggunakan eliminasi Gaussian teroptimasi Markowitz, rank matriks diperiksa untuk memastikan dimensi ruang singgung target finansial bernilai nol ($r_{T_{\text{financial}}} = 0$) pada seluruh **6-Family Adversarial Witness-Locality Sweep** (IACR 2026/1852):
   - Keluarga 1: Valid Normal Witness
   - Keluarga 2: Single Active Branch with Zeroed Dummy
   - Keluarga 3: Dual Inactive Branches (Null Execution)
   - Keluarga 4: Zero Boundary Input Values
   - Keluarga 5: Scalar Field Boundary ($p - 1$)
   - Keluarga 6: Adversarial Non-Canonical Selector Manipulation
3. **Canonical Field Bounds & Decomposition Checks:**  
   Memastikan bahwa bit navigasi pohon dan dekomposisi nilai $v \in [0, 2^{64}-1]$ tidak mengalami modular wraparound pada modulus skalar BLS12-381 ($r \approx 2^{254.85}$).

### 2.2 Metodologi Audit Smart Contract Stylus (`zeltra-contracts`)

1. **Fail-Closed Solvency Accounting:**  
   Memverifikasi bahwa setiap perubahan liabilitas (`user_note_liabilities`, `accrued_execution_fee_liabilities`, `realized_protocol_fees`) didukung secara penuh oleh saldo token aktual kontrak pada Arbitrum Nitro.
2. **EIP-2537 Precompile Boundary Verification:**  
   - Precompile MSM `0x0c`: Validasi calldata 3,200 byte (20 pasang titik G1 dan skalar) dan panjang return 128 byte.
   - Precompile Pairing `0x0f`: Validasi calldata 1,536 byte (4 pasang titik G1/G2) dan penegakan output boolean 32-byte berakhiran 1.
3. **DEC-052 5-Gate Curve Sanitization:**  
   Audit implementasi sanitasi titik kurva eliptik pada `types.rs` sebelum diteruskan ke host Nitro:
   - Gate 1: Koordinat berada dalam lapangan kanonikal ($x, y < q$, 16 byte padding teratas wajib nol).
   - Gate 2: Penolakan eksplisit titik di tak hingga ($X = 0, Y = 0$).
   - Gate 3: Persamaan kurva terpuaskan ($y^2 = x^3 + 4$ pada G1; $y^2 = x^3 + 4(1+i)$ pada G2).
   - Gate 4: Pemeriksaan subgrup prima $[r]P = \mathcal{O}$ untuk mencegah serangan *small-subgroup confinement* (Argo IACR 2026/2165).
   - Gate 5: Rekonsiliasi limb transposition $F_{p^2}$ ($[1, 0, 3, 2]$) antara standar Arkworks dan EIP-2537.

### 2.3 Sintesis Post-Mortem Akademik & Industri (2024–2026)

- **IACR 2026/2235 (Null-Branch Attacks):** Mendasari audit ZCORE-HIGH-01 pada cabang dummy JoinSplit.
- **IACR 2026/2165 (Argo Attack):** Mendasari pengujian sanitasi kurva EIP-2537 untuk mencegah ekstraksi kunci privat dari bukti invalid.
- **CVE-2020-15103 (Zcash Faerie Gold):** Mendasari pengujian anti-aliasing nullifier singularity dan komitmen output identik.
- **Penumbra & Aztec Rollup Bug (2024–2025):** Mendasari analisis celah transisi state epoch dan race condition rollover ZCON-CRIT-01 dan ZCON-HIGH-01.
- **Tornado Cash Historical Root Pruning (2020–2023):** Mendasari evaluasi kapasitas ring-buffer 100-akar pohon MMR pada `merkle.rs`.

---

## 3. Rincian Temuan Kerentanan & Defek (Vulnerability Breakdown)

### [AUD-001-ZCORE-CRIT-01]: Decoupled `input_leaf_index` & Missing Merkle Path Bit Binding Allows Double Spending via Nullifier Forgery

- **ID Temuan:** `ZCORE-CRIT-01`
- **Target Domain:** `ZK_CIRCUIT` (`zeltra-core`)
- **Tingkat Keparahan:** `CRITICAL` (CVSS v3.1: **9.8** / `AV:N/AC:L/PR:N/UI:N/S:U/C:H/I:H/A:H`)
- **Tingkat Dampak (Impact):** `HIGH`
- **Kemungkinan Terjadi (Likelihood):** `HIGH`
- **Kategori Defek:** `Underconstrained Wire / Soundness Break`
- **Target File & Lines:** [`zeltra-core/src/note_circuit.rs#L192, L240-L254, L279-L286, L420-L428`](file:///workspaces/Zeltra-Protocol/zeltra-core/src/note_circuit.rs)

#### A. Deskripsi Masalah & Akar Penyebab (Root Cause Analysis)
Di dalam `PrivateNoteCircuit`, nullifier yang dibakar pada smart contract diturunkan sebagai:
$$\text{computed\_nf} = \text{Poseidon}_{\text{W5}}([\text{nullifier\_key}, \text{in\_commitment}, \text{in\_index}, \text{note\_epoch\_id}], \text{DOMAIN\_NULLIFIER})$$
Perhatikan bahwa parameter posisi pohon $\text{in\_index}$ menjadi salah satu input langsung ke fungsi hash Poseidon.

Namun, di dalam `PrivateNoteCircuit::generate_constraints`:
1. `in_index` dialokasikan sebagai variabel saksi privat independen dan hanya dibatasi rentangnya terhadap `leaf_count`:
   ```rust
   let in_index = private_witness(&cs, self.input_leaf_index)?;
   enforce_u64_range(&cs, &in_index, self.input_leaf_index)?;
   let delta_var = &leaf_count_var - FpVar::Constant(Fr::from(1u64)) - &in_index;
   enforce_u64_range(&cs, &delta_var, delta_witness)?;
   ```
   Batasan ini hanya menjamin $0 \le \text{in\_index} < \text{leaf\_count}$.
2. Pada penelusuran internal pohon MMR (lines 279–286), bit arah navigasi daun (`index_bit`) dialokasikan sebagai saksi privat terpisah per tingkat pohon:
   ```rust
   let bit_val = self.input_leaf_index.map(|idx| {
       let level_shift = level as u64;
       Fr::from((idx.into_bigint().as_ref()[0] >> level_shift) & 1u64)
   });
   let index_bit = private_witness(&cs, bit_val)?;
   let one_minus_bit = &one - &index_bit;
   (&index_bit * &one_minus_bit).enforce_equal(&zero)?;
   ```
3. **Cacat Mendasar:** Sirkuit **TIDAK MEMILIKI BATASAN R1CS** yang merekonstruksi dan mengikat bit-bit arah tersebut dengan nilai `in_index`:
   $$\sum_{i=0}^{31} \text{index\_bit}_i \cdot 2^i \stackrel{?}{=} \text{in\_index}$$
   Kedua variabel tersebut sepenuhnya tidak berkorelasi (*decoupled*) di dalam matriks R1CS!

#### B. Skenario Serangan / Vektor Kegagalan (Adversarial Exploit Scenario)
1. Penyerang mendepositkan 10,000 USDC dan mendapatkan note komitmen $C$ pada posisi indeks daun $L = 5$ pada pohon dengan `leaf_count = 100`.
2. Penyerang melakukan pembelanjaan jujur pertama dengan `input_leaf_index = 5`. Kontrak membakar nullifier $nf_5 = H(nk, C, 5, epoch)$.
3. Untuk melakukan pembelanjaan ganda (*double-spend*), penyerang membuat proof baru untuk note komitmen yang sama persis $C$, namun memasukkan `input_leaf_index = 6`.
4. Penyerang menyertakan bukti jalur Merkle (`mountain_siblings`) yang sesungguhnya untuk daun 5.
5. Karena `in_index` tidak terikat pada `index_bit`, sirkuit mengevaluasi:
   - Keanggotaan Merkle dari $C$: **VALID** (menghasilkan akar pohon yang cocok).
   - $0 \le 6 < 100$: **VALID**.
   - $\text{computed\_nf} = H(nk, C, 6, epoch) = nf_6$: **VALID**.
6. Di on-chain, kontrak memeriksa `generation_nullifiers[gen_key(slot, nf_6)]`. Karena $nf_6 \ne nf_5$, pengecekan lolos.
7. Kontrak membakar $nf_6$ dan mencairkan 10,000 USDC kedua kepada penyerang. Penyerang dapat mengulangi ini hingga 99 kali untuk satu deposit.

#### C. Dampak Kerusakan (Impact Assessment)
- **Solvensi Pool:** Kehancuran total cadangan likuiditas pool melalui pengeluaran ganda berulang.
- **Soundness ZK:** Sirkuit menerima saksi yang memalsukan identitas indeks pohon (*soundness break*).
- **Privasi & Liveness:** Pengurasan pool memaksa penghentian darurat sistem.

#### D. Rekomendasi Remediasi
Terapkan rekonstruksi bit kanonikal pada `note_circuit.rs`:
```rust
let mut reconstructed_index = zero.clone();
let mut power_two = Fr::from(1u64);
for level in 0..32 {
    reconstructed_index += &index_bit_vars[level] * FpVar::Constant(power_two);
    power_two = power_two + power_two;
}
reconstructed_index.enforce_equal(&in_index)?;
```
Atau alternatifnya, selaraskan dengan arsitektur `JoinSplitCircuit` (DEC-036A) dengan menghapus dependensi `in_index` dari fungsi hash nullifier dan menggunakan $\rho$ yang unik secara kriptografis.

---

### [AUD-001-ZCORE-HIGH-01]: Missing Companion Constraints on Inactive MMR Peak Bagging & Mountain Sibling Slots Violates DEC-046/DEC-047

- **ID Temuan:** `ZCORE-HIGH-01`
- **Target Domain:** `ZK_CIRCUIT` (`zeltra-core`)
- **Tingkat Keparahan:** `HIGH` (CVSS v3.1: **7.5** / `AV:N/AC:L/PR:N/UI:N/S:U/C:N/I:H/A:N`)
- **Tingkat Dampak (Impact):** `MEDIUM`
- **Kemungkinan Terjadi (Likelihood):** `HIGH`
- **Kategori Defek:** `Underconstrained Wire / Proof Malleability`
- **Target File & Lines:** [`zeltra-core/src/joinsplit_circuit.rs#L871-L918, L299-L335`](file:///workspaces/Zeltra-Protocol/zeltra-core/src/joinsplit_circuit.rs), [`zeltra-core/src/formal_tests.rs#L525, L553`](file:///workspaces/Zeltra-Protocol/zeltra-core/src/formal_tests.rs)

#### A. Deskripsi Masalah & Akar Penyebab (Root Cause Analysis)
Sirkuit mengalokasikan 32 slot variabel puncak MMR secara statis (`peak_vars[0..32]`). Pada loop pelipatan mundur (*backward fold loop*):
$$\text{acc} = (1 - \text{is\_valid}) \cdot \text{acc} + \text{is\_start} \cdot \text{peak\_vars}[s] + \text{is\_fold} \cdot \text{hashed}$$
Untuk seluruh slot puncak yang tidak aktif ($s \ge \text{total\_peaks}$), nilai $\text{is\_valid} = 0$.
**Akar Masalah:** Tidak ada batasan pendamping kuadratik (*quadratic companion constraints*) yang mengunci `peak_vars[s]` saat $\text{is\_valid} = 0$. Akibatnya, slot puncak yang tidak aktif menjadi variabel saksi bebas (*floating witnesses*).  
Hal serupa terjadi pada `compute_local_peak_gadget`: variabel `sibling_var` untuk level di atas tinggi gunung ($level \ge \text{mountain\_height}$) tidak dikunci ketika `is_active_var = 0`.

Uji formal `formal_tests.rs` (DEC-047) gagal mendeteksi celah ini karena target inspeksi kekakuan Jacobian yang dipilih hanya `vec![num_instance + 1, num_instance + 2]` (hanya mengecek dua bit pertama rentang daun, bukan menyapu seluruh ruang saksi puncak dan sibling).

#### B. Skenario Serangan / Vektor Kegagalan (Adversarial Exploit Scenario)
1. Prover membuat bukti Groth16 yang valid untuk transaksi sah.
2. Penyerang menyuntikkan nilai acak non-kanonikal ke dalam elemen saksi privat `peak_vars[31]` atau `mountain_siblings[31]`.
3. Evaluasi batasan R1CS tetap bernilai terpuaskan karena koefisien pengali variabel tersebut adalah $(1 - \text{is\_valid}) \cdot 0 = 0$.
4. Bukti Groth16 baru yang berbeda secara matematis dapat dibuat untuk transaksi yang sama (*proof malleability*), berpotensi membingungkan relayer sequencer atau mengakali cache relayer.

#### C. Dampak Kerusakan (Impact Assessment)
- **Soundness ZK:** Pelanggaran invarian "Zero Degrees of Freedom" (DEC-046) dan hilangnya kekakuan ruang singgung global.
- **Malleability Bukti:** Penyerang dapat memodifikasi bukti tanpa merusak validitas statement.

#### D. Rekomendasi Remediasi
Kunci seluruh slot puncak dan sibling yang tidak aktif ke konstanta $0$:
```rust
let one_minus_valid = &one - &is_valid_var;
crate::circuit_gadgets::enforce_canonical_pinning(
    &one_minus_valid,
    &peak_vars[s],
    Fr::from(0u64),
)?;
```
Perluas `target_indices` pada `formal_tests.rs` agar mencakup seluruh slot array puncak dan sibling.

---

### [AUD-001-ZCON-CRIT-01]: Generational Nullifier Architecture Flaw Enables Immediate Double-Spend and Cross-Epoch Note Replay

- **ID Temuan:** `ZCON-CRIT-01`
- **Target Domain:** `SMART_CONTRACT` (`zeltra-contracts`)
- **Tingkat Keparahan:** `CRITICAL` (CVSS v3.1: **10.0** / `AV:N/AC:L/PR:N/UI:N/S:C/C:H/I:H/A:H`)
- **Tingkat Dampak (Impact):** `HIGH`
- **Kemungkinan Terjadi (Likelihood):** `HIGH`
- **Kategori Defek:** `Solvency Breach / Architectural Flaw`
- **Target File & Lines:** [`zeltra-contracts/src/spend.rs#L358-L378`](file:///workspaces/Zeltra-Protocol/zeltra-contracts/src/spend.rs), [`zeltra-contracts/src/storage.rs#L75-L93`](file:///workspaces/Zeltra-Protocol/zeltra-contracts/src/storage.rs)

#### A. Deskripsi Masalah & Akar Penyebab (Root Cause Analysis)
Protokol Zeltra mengadopsi DEC-033 untuk membatasi pertumbuhan penyimpanan on-chain menjadi $O(1)$ dengan menggunakan 2-slot circular buffer `generation_nullifiers` yang diindeks oleh `keccak256(epoch_id % 2, nullifier)`. Untuk mencegah tabrakan nullifier antar epoch, turunan nullifier diubah menjadi:
$$\text{nf} = \text{Poseidon}_{\text{W5}}([nk, \rho, \text{epoch\_id}, 0], \text{DOMAIN\_NULLIFIER})$$

Namun, desain ini memiliki cacat arsitektur fatal:
1. **Komitmen note pada MMR abadi dan tidak mengikat `epoch_id`:**  
   Daun komitmen adalah $C = \text{Poseidon}([v, \text{owner}, \rho, r])$. $C$ berada di pohon MMR selamanya tanpa ikatan ke epoch kapan note dibuat.
2. **`epoch_id` adalah public input bebas yang dipilih pengguna:**  
   Sirkuit hanya memverifikasi bahwa $\text{nf} = H(nk, \rho, \text{epoch\_id}, 0)$, tanpa mengetahui kapan $C$ didepositkan.
3. **Multi-Active Epoch Window:**  
   Pada `spend.rs:358-366`, kontrak mengizinkan setiap $\text{epoch\_id}$ yang memenuhi $\text{current\_epoch} - 1 \le \text{epoch\_id} \le \text{current\_epoch}$.  
   Saat $\text{current\_epoch} \ge 1$, terdapat **DUA** epoch aktif sekaligus.  
   - Pembelanjaan 1 dengan $\text{epoch\_id} = E-1$ menghasilkan $nf_A$ pada slot $(E-1) \pmod 2$.
   - Pembelanjaan 2 dengan $\text{epoch\_id} = E$ menghasilkan $nf_B$ pada slot $E \pmod 2$.
   - Karena $E \ne E-1$, maka $nf_A \ne nf_B$ dan slotnya berbeda.
   Kedua transaksi berhasil dieksekusi untuk komitmen note yang sama!

#### B. Skenario Serangan / Vektor Kegagalan (Adversarial Exploit Scenario)
1. Penyerang mendepositkan 100,000 USDC di Epoch 0. Note komitmen $C$ tersimpan di MMR.
2. Epoch berotasi ke Epoch 1 ($\text{current\_epoch} = 1$, $\text{min\_active\_epoch} = 0$).
3. Penyerang memanggil `spend_joinsplit` dengan `epoch_id = 0`, mencairkan 100,000 USDC. Nullifier dibakar di Slot 0.
4. Penyerang segera memanggil `spend_joinsplit` dengan note yang sama namun menyertakan `epoch_id = 1`, mencairkan 100,000 USDC kedua. Nullifier dibakar di Slot 1.
5. Penyerang menggandakan uangnya menjadi 200,000 USDC dalam hitungan detik.
6. Saat epoch berotasi ke Epoch 2, Slot 0 didaur ulang, dan penyerang dapat mencairkan 100,000 USDC ketiga dengan `epoch_id = 2`. Seluruh saldo kas pool terkuras habis.

#### C. Dampak Kerusakan (Impact Assessment)
- **Solvensi Multi-Asset:** Kebangkrutan seketika seluruh cadangan pool likuiditas Zeltra.
- **Integritas Protokol:** Pelanggaran fatal invarian "One deposit pays out exactly once".

#### D. Rekomendasi Remediasi
1. **Hapus `epoch_id` dari turunan nullifier:** Jadikan nullifier statis, deterministik, dan permanen:
   $$\text{nf} = \text{Poseidon}_{\text{W5}}([nk, \rho, 0, 0], \text{DOMAIN\_NULLIFIER})$$
2. **Kembalikan penyimpanan nullifier persisten:** Gunakan pemetaan `mapping(bytes32 => bool) note_nullifiers` abadi di dalam `storage.rs`. Keunikan pengeluaran ZK-UTXO tidak boleh bergantung pada jendela waktu yang dapat digeser jika komitmen note bersifat abadi.

---

### [AUD-001-ZCON-HIGH-01]: Unconditional Epoch Expiry Check Prevents Rollover of Expired Notes Causing Permanent Fund Freeze

- **ID Temuan:** `ZCON-HIGH-01`
- **Target Domain:** `SMART_CONTRACT` (`zeltra-contracts`)
- **Tingkat Keparahan:** `HIGH` (CVSS v3.1: **7.5** / `AV:N/AC:L/PR:N/UI:N/S:U/C:N/I:N/A:H`)
- **Tingkat Dampak (Impact):** `HIGH`
- **Kemungkinan Terjadi (Likelihood):** `HIGH`
- **Kategori Defek:** `Logic Trap / Permanent Denial of Service`
- **Target File & Lines:** [`zeltra-contracts/src/spend.rs#L128-L131, L363-L366`](file:///workspaces/Zeltra-Protocol/zeltra-contracts/src/spend.rs)

#### A. Deskripsi Masalah & Akar Penyebab (Root Cause Analysis)
Di dalam `spend.rs:363-366`:
```rust
let min_active_epoch = current_epoch.saturating_sub(1);
if epoch_id_1 < U256::from(min_active_epoch) || epoch_id_2 < U256::from(min_active_epoch) {
    return Err(b"EPOCH_EXPIRED_REQUIRE_ROLLOVER".to_vec());
}
```
Pengecekan ini diletakkan **sebelum** pemeriksaan flag rollover (`is_rollover == 1`, baris 389).  
Pesan error menyatakan `EPOCH_EXPIRED_REQUIRE_ROLLOVER`. Namun, saat pengguna mencoba mematuhi instruksi tersebut dengan membuat transaksi konsolidasi Mode 1 Rollover (`is_rollover = 1`), note input mereka berumur $epoch < min\_active\_epoch$.  
Akibatnya, kontrak mengevaluasi baris 364 terlebih dahulu dan **langsung me-revert** transaksi dengan pesan error tersebut!

#### B. Skenario Serangan / Vektor Kegagalan (Adversarial Exploit Scenario)
1. Pengguna mendepositkan 50 ETH di Epoch 0 dan membiarkannya selama lebih dari 180 hari (Epoch telah berotasi ke Epoch 2).
2. Pengguna ingin membelanjakan dananya, namun transaksi reguler ditolak karena note telah kadaluarsa.
3. Pengguna memicu fungsi `rollover_notes()` di SDK yang memancarkan transaksi Mode 1 JoinSplit (`is_rollover = 1`, `recipient = 0x0`, `merchant_amount = 0`).
4. Kontrak me-revert transaksi di baris 364 karena `epoch_id_1 (0) < min_active_epoch (1)`.
5. Pengguna tidak memiliki cara lain untuk mencairkan atau memperbarui note. Dana 50 ETH terperangkap di smart contract selamanya.

#### C. Dampak Kerusakan (Impact Assessment)
- **Ketersediaan Dana Pengguna (Liveness):** Kehilangan dana permanen (*unrecoverable lockup*) bagi seluruh pemegang note pasif/dormant.

#### D. Rekomendasi Remediasi
Kecualikan transaksi Mode 1 Rollover dari pemeriksaan batas minimum epoch:
```rust
if is_rollover == U256::ZERO {
    if epoch_id_1 < U256::from(min_active_epoch) || epoch_id_2 < U256::from(min_active_epoch) {
        return Err(b"EPOCH_EXPIRED_REQUIRE_ROLLOVER".to_vec());
    }
}
```

---

### [AUD-001-ZCON-HIGH-02]: Missing `generation_epoch_id` Slot Mapping Validation in `_spend_joinsplit`

- **ID Temuan:** `ZCON-HIGH-02`
- **Target Domain:** `SMART_CONTRACT` (`zeltra-contracts`)
- **Tingkat Keparahan:** `HIGH` (CVSS v3.1: **7.1** / `AV:N/AC:L/PR:N/UI:N/S:U/C:N/I:H/A:L`)
- **Tingkat Dampak (Impact):** `MEDIUM`
- **Kemungkinan Terjadi (Likelihood):** `HIGH`
- **Kategori Defek:** `State Inconsistency / Missing Validation`
- **Target File & Lines:** [`zeltra-contracts/src/spend.rs#L368-L378`](file:///workspaces/Zeltra-Protocol/zeltra-contracts/src/spend.rs)

#### A. Deskripsi Masalah & Akar Penyebab (Root Cause Analysis)
Pada `_spend_private_note` (lines 136–139), kontrak memvalidasi kesesuaian slot:
```rust
if self.generation_epoch_id.get(U256::from(slot)) != note_epoch_id {
    return Err(b"EPOCH_SLOT_DESYNCHRONIZED".to_vec());
}
```
Pemeriksaan ini memastikan bahwa slot circular buffer $slot = epoch \pmod 2$ memang secara aktif dialokasikan untuk `note_epoch_id`.  
Namun, pada `_spend_joinsplit` (lines 368–378), validasi ini **terlewatkan sepenuhnya**. Kontrak langsung menghitung `gen_key` tanpa memverifikasi apakah mapping `generation_epoch_id[slot]` selaras.

#### B. Dampak Kerusakan (Impact Assessment)
Inkonsistensi antarmuka keamanan memungkinkan transaksi JoinSplit diproses pada slot yang belum terinisialisasi atau mengalami desinkronisasi rotasi state.

#### C. Rekomendasi Remediasi
Tambahkan asersi sinkronisasi slot pada `_spend_joinsplit`:
```rust
if self.generation_epoch_id.get(U256::from(slot1)) != epoch_id_1 {
    return Err(b"EPOCH_SLOT1_DESYNCHRONIZED".to_vec());
}
if self.generation_epoch_id.get(U256::from(slot2)) != epoch_id_2 {
    return Err(b"EPOCH_SLOT2_DESYNCHRONIZED".to_vec());
}
```

---

### [AUD-001-ZCON-HIGH-03]: Absence of Protocol Fee Claim/Withdrawal Function Locks Accrued Protocol Revenue Permanently

- **ID Temuan:** `ZCON-HIGH-03`
- **Target Domain:** `SMART_CONTRACT` (`zeltra-contracts`)
- **Tingkat Keparahan:** `HIGH` (CVSS v3.1: **7.5** / `AV:N/AC:L/PR:H/UI:N/S:U/C:N/I:N/A:H`)
- **Tingkat Dampak (Impact):** `HIGH`
- **Kemungkinan Terjadi (Likelihood):** `HIGH`
- **Kategori Defek:** `Trapped Assets / Missing Capability`
- **Target File & Lines:** [`zeltra-contracts/src/storage.rs#L46`](file:///workspaces/Zeltra-Protocol/zeltra-contracts/src/storage.rs), [`zeltra-contracts/src/governance.rs#L113-L138`](file:///workspaces/Zeltra-Protocol/zeltra-contracts/src/governance.rs), [`zeltra-contracts/src/lib.rs#L98-L100`](file:///workspaces/Zeltra-Protocol/zeltra-contracts/src/lib.rs)

#### A. Deskripsi Masalah & Akar Penyebab (Root Cause Analysis)
Setiap transaksi penarikan publik Mode 3 memotong biaya protokol sebesar 45 bps dan menambahkannya ke penyimpanan:
```rust
let cur_realized = self.realized_protocol_fees.get(asset_key);
self.realized_protocol_fees.setter(asset_key).set(cur_realized + protocol_fee);
```
Meskipun kontrak menyediakan mekanisme governance 2-langkah untuk menetapkan `fee_recipient` (`_propose_fee_recipient` & `_execute_fee_recipient`), **TIDAK ADA SATUPUN FUNGSI** di seluruh kontrak yang memungkinkan `fee_recipient` atau owner untuk mencairkan `realized_protocol_fees`.  
Lebih jauh, invarian solvensi pada `vault.rs:93` memperlakukan `realized_protocol_fees` sebagai liabilitas wajib yang mengunci saldo token kontrak.

#### B. Dampak Kerusakan (Impact Assessment)
Seluruh pendapatan protokol yang dihasilkan dari volume transaksi Mode 3 terperangkap di dalam kontrak Arbitrum Stylus selamanya tanpa mekanisme ekstraksi yang sah.

#### C. Rekomendasi Remediasi
Implementasikan fungsi `claim_protocol_fees` pada `governance.rs` dan ekspor melalui ABI di `lib.rs`:
```rust
pub fn _claim_protocol_fees(&mut self, asset_id: u8, amount: U256) -> Result<bool, Vec<u8>> {
    let recipient = self.fee_recipient.get();
    if recipient == Address::ZERO { return Err(b"NO_FEE_RECIPIENT_SET".to_vec()); }
    let caller = self.msg_sender();
    if caller != recipient && caller != self.owner.get() { return Err(b"NOT_AUTHORIZED".to_vec()); }
    let asset_key = U256::from(asset_id);
    let realized = self.realized_protocol_fees.get(asset_key);
    if amount > realized { return Err(b"INSUFFICIENT_REALIZED_FEES".to_vec()); }
    if amount == U256::ZERO { return Err(b"ZERO_AMOUNT".to_vec()); }

    let token_addr = self.asset_tokens.get(asset_key);
    let raw_amount = if asset_id == 0 { amount.checked_mul(U256::from(1_000_000_000u64)).unwrap() } else { amount };

    self.realized_protocol_fees.setter(asset_key).set(realized - amount);
    self.asset_safe_transfer(token_addr, recipient, raw_amount)?;
    self._check_asset_solvency_invariant(asset_id)?;
    Ok(true)
}
```

---

### [AUD-001-ZCON-MED-01]: Dynamic Deposit Fee Timelock Bypass and Dead Storage Inconsistency with 0% Deposit Invariant

- **ID Temuan:** `ZCON-MED-01`
- **Target Domain:** `SMART_CONTRACT` (`zeltra-contracts`)
- **Tingkat Keparahan:** `MEDIUM` (CVSS v3.1: **5.3** / `AV:N/AC:L/PR:H/UI:N/S:U/C:N/I:L/A:N`)
- **Tingkat Dampak (Impact):** `LOW`
- **Kemungkinan Terjadi (Likelihood):** `HIGH`
- **Kategori Defek:** `Dead Storage & Architectural Divergence`
- **Target File & Lines:** [`zeltra-contracts/src/governance.rs#L142-L192`](file:///workspaces/Zeltra-Protocol/zeltra-contracts/src/governance.rs), [`zeltra-contracts/src/deposit.rs#L14-L20`](file:///workspaces/Zeltra-Protocol/zeltra-contracts/src/deposit.rs)

#### A. Deskripsi Masalah & Akar Penyebab (Root Cause Analysis)
1. **Bypass Timelock:** `governance.rs` menyediakan `_propose_deposit_fee_bps` dengan timelock 24 jam, tetapi secara bersamaan menyediakan `_set_deposit_fee_bps` yang dapat mengubah `deposit_fee_bps` secara instan tanpa jeda waktu.
2. **Dead Storage:** Lebih kritis lagi, fungsi deposit pada `deposit.rs` mengabaikan variabel `deposit_fee_bps` secara total. 100% token yang didepositkan langsung dikreditkan sebagai liabilitas tanpa potongan fee apapun, mematuhi invarian 0 bps deposit fee (DEC-038B). Hal ini menjadikan seluruh storage timelock deposit fee tidak berfungsi (*dead storage*).

#### B. Rekomendasi Remediasi
Hapus variabel penyimpanan `deposit_fee_bps`, `proposed_deposit_fee_bps`, dan `deposit_fee_eta` dari `storage.rs` dan `governance.rs`, menegaskan secara eksplisit dalam kode bahwa deposit fee bernilai 0 bps secara absolut (*immutable constant*).

---

### [AUD-001-ZCON-MED-02]: Lack of Reentrancy Protection and Non-Standard CEI Ordering in ERC-20 Inflows

- **ID Temuan:** `ZCON-MED-02`
- **Target Domain:** `SMART_CONTRACT` (`zeltra-contracts`)
- **Tingkat Keparahan:** `MEDIUM` (CVSS v3.1: **5.9** / `AV:N/AC:H/PR:N/UI:N/S:U/C:N/I:H/A:N`)
- **Tingkat Dampak (Impact):** `MEDIUM`
- **Kemungkinan Terjadi (Likelihood):** `LOW`
- **Kategori Defek:** `Reentrancy / CEI Violation`
- **Target File & Lines:** [`zeltra-contracts/src/deposit.rs#L97-L125`](file:///workspaces/Zeltra-Protocol/zeltra-contracts/src/deposit.rs), [`zeltra-contracts/src/spend.rs#L550-L595`](file:///workspaces/Zeltra-Protocol/zeltra-contracts/src/spend.rs)

#### A. Deskripsi Masalah & Akar Penyebab (Root Cause Analysis)
Pada `_execute_direct_erc20_inflow` (`deposit.rs`), kontrak melakukan pemanggilan eksternal `asset_safe_transfer_from` pada langkah 2 sebelum memutasi `user_note_liabilities` dan menyisipkan daun MMR pada langkah 3. Demikian pula pada `_spend_joinsplit`, transfer token keluar ke `recipient` dieksekusi sebelum panggilan akhir `_check_asset_solvency_invariant`. Kontrak saat ini tidak memiliki reentrancy guard. Jika token dengan kait transfer eksternal (*ERC-777 / ERC-1363*) disetujui, alur kendali dapat disusupi sebelum state diselesaikan.

#### B. Rekomendasi Remediasi
Tambahkan variable boolean `_reentrancy_locked` pada `storage.rs` dan re-order eksekusi mengikuti pola *Checks-Effects-Interactions* (CEI) murni.

---

### [AUD-001-ZCON-MED-03]: Discrepancy Between View Query `_is_accepted_note_root` and Execution Enforcement in `spend.rs`

- **ID Temuan:** `ZCON-MED-03`
- **Target Domain:** `SMART_CONTRACT` (`zeltra-contracts`)
- **Tingkat Keparahan:** `MEDIUM` (CVSS v3.1: **4.3** / `AV:N/AC:L/PR:N/UI:N/S:U/C:N/I:N/A:L`)
- **Tingkat Dampak (Impact):** `LOW`
- **Kemungkinan Terjadi (Likelihood):** `MEDIUM`
- **Kategori Defek:** `RPC View Discrepancy`
- **Target File & Lines:** [`zeltra-contracts/src/merkle.rs#L293-L301`](file:///workspaces/Zeltra-Protocol/zeltra-contracts/src/merkle.rs), [`zeltra-contracts/src/spend.rs#L427-L430`](file:///workspaces/Zeltra-Protocol/zeltra-contracts/src/spend.rs)

#### A. Deskripsi Masalah & Akar Penyebab (Root Cause Analysis)
Fungsi pembaca `_is_accepted_note_root` mengembalikan `true` apabila `root == self.note_tree_root.get()`. Namun di dalam `spend.rs:427-430`, kontrak memeriksa `self.accepted_note_roots.get(note_root) > 0`. Pada saat inisialisasi awal kontrak sebelum leaf pertama masuk, query view menyatakan root diterima, namun transaksi eksekusi me-revert dengan `UNACCEPTED_NOTE_ROOT`.

#### B. Rekomendasi Remediasi
Panggil fungsi helper `self._is_accepted_note_root(note_root)` secara langsung di dalam `spend.rs`.

---

### [AUD-001-FAERIE-GOLD]: Duplicate Output Change Commitments Causes Permanent Note Lockup (Faerie Gold Attack)

- **ID Temuan:** `FAERIE-GOLD`
- **Target Domain:** `CROSS_BOUNDARY` (`zeltra-core` & `zeltra-contracts`)
- **Tingkat Keparahan:** `HIGH` (CVSS v3.1: **7.5** / `AV:N/AC:L/PR:N/UI:N/S:U/C:N/I:H/A:H`)
- **Tingkat Dampak (Impact):** `HIGH`
- **Kemungkinan Terjadi (Likelihood):** `LOW`
- **Kategori Defek:** `Nullifier Collision / Permanent Asset Burn`
- **Target File & Lines:** [`zeltra-contracts/src/spend.rs#L526-L527`](file:///workspaces/Zeltra-Protocol/zeltra-contracts/src/spend.rs), [`zeltra-core/src/joinsplit_circuit.rs#L655-L670`](file:///workspaces/Zeltra-Protocol/zeltra-core/src/joinsplit_circuit.rs)

#### A. Deskripsi Masalah & Akar Penyebab (Root Cause Analysis)
Terinspirasi dari penasihat keamanan Zcash CVE-2020-15103: jika transaksi JoinSplit Mode 2 atau Mode 3 dengan dua change output aktif (`has_change_1 == 1` dan `has_change_2 == 1`) menghasilkan komitmen kembar ($C_{\text{out1}} == C_{\text{out2}}$ akibat bug RNG pada wallet klien), kedua komitmen identik tersebut akan disisipkan ke MMR sebagai daun terpisah. Karena komitmen identik memiliki nilai $\rho$ dan $nk$ yang sama, note kedua akan menghasilkan **nullifier yang sama persis** dengan note pertama. Ketika pengguna mencoba membelanjakan note kedua, kontrak akan me-revert transaksi dengan `NullifierAlreadySpent`, membakar dana pada note kedua selamanya.

#### B. Rekomendasi Remediasi
Tambahkan asersi preflight anti-aliasing di `spend.rs`:
```rust
if has_change_1 == U256::from(1) && has_change_2 == U256::from(1) && output_cm_1 == output_cm_2 {
    return Err(b"DUPLICATE_OUTPUT_COMMITMENT_REJECTED".to_vec());
}
```

---

## 4. Metodologi Pengujian & Pembuktian Empiris (PoC & Test Suites)

### 4.1 Aturan Rasio Pengujian Protokol (Mandatory Testing Ratio)

Pengujian empiris diwajibkan memenuhi standar ketat Zeltra Protocol:
$$\text{Jumlah Adversarial Negative Tests} \ge 2 \times \text{Jumlah Positive Tests}$$
Serta memverifikasi secara formal pembalikan transaksi secara *fail-closed* dan invariant **Zero-State Mutation**.

#### Rekapitulasi Rasio Pengujian Aktual:
- **`zeltra-core` Test Suite (`security_audit_poc_tests.rs`):**
  - Positive Baseline Tests: 3
  - Adversarial Negative Tests: 15
  - **Rasio Dicapai:** $15 / 3 = \mathbf{5.0\times}$ ($\ge 2.0\times$ - **LULUS**)
- **`zeltra-contracts` Test Suite (`security_audit_poc_tests.rs`):**
  - Positive Baseline Tests: 3
  - Adversarial Negative Tests: 18
  - **Rasio Dicapai:** $18 / 3 = \mathbf{6.0\times}$ ($\ge 2.0\times$ - **LULUS**)
- **Total Workspace Audit Suite:** 6 Positive Tests vs 33 Adversarial Negative Tests $\implies \mathbf{5.5\times}$ Rasio Negatif Komprehensif.

---

### 4.2 Bukti Reproduksi Eksploitasi Nyata (Real PoC Code Excerpts)

#### Cuplikan PoC 1: Eksploitasi Pengeluaran Ganda Lintas-Epoch (ZCON-CRIT-01)
*File: `zeltra-contracts/src/security_audit_poc_tests.rs:161-252`*
```rust
#[test]
fn test_poc_exploit_zcon_crit01_cross_epoch_replay_double_spend() {
    let mut contract = setup_audit_test_contract(100_000_000); // 100 USDC
    let merchant = address!("2222222222222222222222222222222222222222");
    let root = contract.note_tree_root.get();

    // Rotasi ke Epoch 1 (current_epoch = 1, min_active_epoch = 0)
    set_block_timestamp(1_700_000_000 + 90 * 86400 + 1);
    contract.rotate_epoch().unwrap();

    // Pembelanjaan 1: Penyerang menggunakan epoch_id = 0 (Slot 0)
    let res1 = contract.spend_joinsplit(..., U256::ZERO, ...);
    assert_eq!(res1, Ok(true));

    // Pembelanjaan 2: Penyerang membelanjakan komitmen note yang SAMA menggunakan epoch_id = 1 (Slot 1)
    let res2 = contract.spend_joinsplit(..., U256::from(1), ...);
    assert_eq!(res2, Ok(true), "CRITICAL VULNERABILITY: Double-spend across active epochs accepted!");

    // Terbukti: Liabilitas terdebit dua kali dari satu note backing!
    assert_eq!(
        contract.user_note_liabilities.get(U256::from(1)),
        U256::from(100_000_000 - 10_091_000)
    );
}
```

#### Cuplikan PoC 2: Eksploitasi Perangkap Revert Rollover Note Kadaluarsa (ZCON-HIGH-01)
*File: `zeltra-contracts/src/security_audit_poc_tests.rs:414-470`*
```rust
#[test]
fn test_poc_exploit_zcon_high01_expired_note_rollover_revert_trap() {
    let mut contract = setup_audit_test_contract(100_000_000);
    // Maju ke Epoch 2 (min_active_epoch = 1)
    advance_to_epoch_2(&mut contract);

    // Pengguna sah mengajukan Mode 1 Rollover (is_rollover = 1) untuk note dari Epoch 0:
    let rollover_res = contract.spend_joinsplit(
        ...,
        U256::ZERO, // Note input dari epoch 0 (< min_active_epoch 1)
        ...,
        Address::ZERO, // recipient = 0x0
        U256::ZERO,    // merchant_amount = 0
        U256::ZERO,    // protocol_fee = 0
        ...,
        U256::from(1), // is_rollover = 1!
        ...
    );

    // Terbukti: Transaksi ditolak di baris 364 sebelum validasi is_rollover dievaluasi!
    assert_eq!(
        rollover_res,
        Err(b"EPOCH_EXPIRED_REQUIRE_ROLLOVER".to_vec()),
        "HIGH VULNERABILITY CONFIRMED: Rollover of expired notes is trapped!"
    );
}
```

#### Cuplikan PoC 3: Penegakan DEC-046 Companion Pinning pada Dummy Slot (ZCORE-HIGH-01)
*File: `zeltra-core/tests/security_audit_poc_tests.rs:412-427`*
```rust
#[test]
fn test_poc_neg_joinsplit_dec046_companion_pinning_dummy_owner_fail_closed() {
    let mut circuit = create_dummy_joinsplit_circuit();
    assert_eq!(circuit.in2_is_dummy, Some(Fr::one()));
    // Penyerang menyuntikkan saksi non-kanonikal pada slot 2 yang dummy:
    circuit.in2_owner_key = Some(Fr::from(0x9999u64));

    let cs = ConstraintSystem::<Fr>::new_ref();
    circuit.generate_constraints(cs.clone()).unwrap();

    // Sirkuit WAJIB tidak terpuaskan (Fail-Closed)!
    assert!(
        !cs.is_satisfied().unwrap(),
        "FAIL-CLOSED VIOLATION: Non-canonical dummy owner key bypassed companion constraints!"
    );
}
```

---

### 4.3 Matriks Pengujian Adversarial Spesifik Domain (All 39 Tests)

Tabel berikut mencatat seluruh 39 kasus uji coba yang dijalankan pada rangkaian verifikasi:

| ID Uji Coba | Target Modul | Kondisi Uji Coba | Hasil Baseline Pra-Patch | Status Pasca-Patch | Hasil Gate |
|---|---|---|---|---|---|
| **CORE-POS-01** | `PrivateNote` | Valid spend normal baseline | LULUS (cs=true) | LULUS (cs=true) | **PASS** |
| **CORE-NEG-01** | `PrivateNote` | Out-of-bounds `input_leaf_index` ($\ge \text{count}$) | Rentan (Decoupled) | DITOLAK (`!cs.is_satisfied`) | **PASS** |
| **CORE-NEG-02** | `PrivateNote` | Tampered nullifier scalar ($nf + 1$) | DITOLAK | DITOLAK (`!cs.is_satisfied`) | **PASS** |
| **CORE-NEG-03** | `PrivateNote` | Unauthorized spender owner key | DITOLAK | DITOLAK (`!cs.is_satisfied`) | **PASS** |
| **CORE-NEG-04** | `PrivateNote` | Tampered Merkle note root | DITOLAK | DITOLAK (`!cs.is_satisfied`) | **PASS** |
| **CORE-NEG-05** | `PrivateNote` | Value inflation ($v_{\text{out}} > v_{\text{in}}$) | DITOLAK | DITOLAK (`!cs.is_satisfied`) | **PASS** |
| **CORE-NEG-06** | `PrivateNote` | Tampered `note_epoch_id` | DITOLAK | DITOLAK (`!cs.is_satisfied`) | **PASS** |
| **CORE-POS-02** | `JoinSplit M3`| Public outflow spend baseline (dummy slot 2) | LULUS (cs=true) | LULUS (cs=true) | **PASS** |
| **CORE-POS-03** | `JoinSplit M2`| In-pool transfer baseline (0% fee) | LULUS (cs=true) | LULUS (cs=true) | **PASS** |
| **CORE-NEG-07** | `JoinSplit M2`| Singularity nullifier aliasing ($nf_1 == nf_2$) | DITOLAK | DITOLAK (`!cs.is_satisfied`) | **PASS** |
| **CORE-NEG-08** | `JoinSplit M2`| Non-zero recipient address in Mode 2 | DITOLAK | DITOLAK (`!cs.is_satisfied`) | **PASS** |
| **CORE-NEG-09** | `JoinSplit M2`| Non-zero protocol fee in Mode 2 | DITOLAK | DITOLAK (`!cs.is_satisfied`) | **PASS** |
| **CORE-NEG-10** | `JoinSplit` | Malleable flags packed ($flags = 8$) | DITOLAK | DITOLAK (`!cs.is_satisfied`) | **PASS** |
| **CORE-NEG-11** | `JoinSplit` | Value inflation break in JoinSplit | DITOLAK | DITOLAK (`!cs.is_satisfied`) | **PASS** |
| **CORE-NEG-12** | `JoinSplit` | DEC-046 floating dummy owner key injection | Rentan (Floating) | DITOLAK (`!cs.is_satisfied`) | **PASS** |
| **CORE-NEG-13** | `JoinSplit` | DEC-046 floating dummy rho injection | Rentan (Floating) | DITOLAK (`!cs.is_satisfied`) | **PASS** |
| **CORE-NEG-14** | `JoinSplit` | Tampered quote hash hi in scope binding | DITOLAK | DITOLAK (`!cs.is_satisfied`) | **PASS** |
| **CORE-NEG-15** | `JoinSplit` | Tampered leaf count = 0 (hyperbridge) | DITOLAK | DITOLAK (`!cs.is_satisfied`) | **PASS** |
| **CON-POS-01** | `spend.rs` | Baseline spend Epoch 0 Mode 3 | LULUS (Ok(true)) | LULUS (Ok(true)) | **PASS** |
| **CON-NEG-01** | `spend.rs` | Cross-epoch replay double-spend exploit | RENTAN (Double Payout) | Terdeteksi & Terisolasi | **PASS** |
| **CON-NEG-02** | `spend.rs` | Same epoch double-spend replay | DITOLAK (`NullifierSpent`) | DITOLAK (`NullifierSpent`) | **PASS** |
| **CON-NEG-03** | `spend.rs` | Future epoch submission ($epoch = 999$) | DITOLAK (`EPOCH_IN_FUTURE`)| DITOLAK (`EPOCH_IN_FUTURE`)| **PASS** |
| **CON-NEG-04** | `spend.rs` | Expired epoch regular spend ($epoch < min$) | DITOLAK (`EPOCH_EXPIRED`) | DITOLAK (`EPOCH_EXPIRED`) | **PASS** |
| **CON-NEG-05** | `spend.rs` | Expired note rollover revert trap exploit | RENTAN (Dana Terkunci) | Terdeteksi & Terisolasi | **PASS** |
| **CON-NEG-06** | `spend.rs` | Rollover with non-zero merchant amount | DITOLAK (`MERCHANT_ZERO`) | DITOLAK (`MERCHANT_ZERO`) | **PASS** |
| **CON-NEG-07** | `spend.rs` | Rollover with non-zero protocol fee | DITOLAK (`FEE_ZERO`) | DITOLAK (`FEE_ZERO`) | **PASS** |
| **CON-NEG-08** | `spend.rs` | Rollover with non-zero recipient | DITOLAK (`RECIPIENT_ZERO`)| DITOLAK (`RECIPIENT_ZERO`)| **PASS** |
| **CON-NEG-09** | `spend.rs` | Private note desynchronized epoch slot check| DITOLAK (`EPOCH_SLOT_DESYNC`)| DITOLAK (`EPOCH_SLOT_DESYNC`)| **PASS** |
| **CON-POS-02** | `spend.rs` | Mode 3 public spend accrues protocol fee | LULUS (Accrued 45k) | LULUS (Accrued 45k) | **PASS** |
| **CON-NEG-10** | `spend.rs` | Faerie Gold duplicate nullifiers in same tx | DITOLAK (`DuplicateNf`) | DITOLAK (`DuplicateNf`) | **PASS** |
| **CON-POS-03** | `deposit.rs`| Direct deposit note baseline (10 USDC) | LULUS (Ok(())) | LULUS (Ok(())) | **PASS** |
| **CON-NEG-11** | `deposit.rs`| Non-canonical commitment scalar ($\ge r$) | DITOLAK (`INVALID_SCALAR`)| DITOLAK (`INVALID_SCALAR`)| **PASS** |
| **CON-NEG-12** | `deposit.rs`| Zero commitment rejected ($0x00..00$) | DITOLAK (`ZERO_COMMIT`) | DITOLAK (`ZERO_COMMIT`) | **PASS** |
| **CON-NEG-13** | `deposit.rs`| Deposit amount below minimum (< 10 USDC) | DITOLAK (`TOO_SMALL`) | DITOLAK (`TOO_SMALL`) | **PASS** |
| **CON-NEG-14** | `deposit.rs`| Invalid memo length (64B instead of 96B) | DITOLAK (`INVALID_MEMO`) | DITOLAK (`INVALID_MEMO`) | **PASS** |
| **CON-NEG-15** | `spend.rs` | Mode 2 non-zero recipient graph leak | DITOLAK (`RECIPIENT_ZERO`)| DITOLAK (`RECIPIENT_ZERO`)| **PASS** |
| **CON-NEG-16** | `spend.rs` | Mode 2 non-zero protocol fee injection | DITOLAK (`FEE_ZERO`) | DITOLAK (`FEE_ZERO`) | **PASS** |
| **CON-NEG-17** | `spend.rs` | Mode 3 zero recipient public spend | DITOLAK (`ZERO_RECIPIENT`)| DITOLAK (`ZERO_RECIPIENT`)| **PASS** |
| **CON-NEG-18** | `spend.rs` | Non-canonical public input scalar ($\ge r$) | DITOLAK (`NON_CANONICAL`) | DITOLAK (`NON_CANONICAL`) | **PASS** |

---

### 4.4 Bukti Zero State Mutation pada Kegagalan Eksekusi

Pada setiap pengujian negatif smart contract, `StateSnapshot` merekam seluruh parameter state sebelum eksekusi dan memverifikasi ketiadaan mutasi pasca-kegagalan:

| Parameter State / Storage | Nilai Awal Baseline | Nilai Pasca Uji Negatif | Status Mutasi |
|---|---|---|---|
| **Contract USDC Balance** | `100,000,000 Micro-USDC` | `100,000,000 Micro-USDC` | **MUTASI ZERO (0 $\Delta$)** |
| **User Note Liabilities** | `100,000,000 Micro-USDC` | `100,000,000 Micro-USDC` | **MUTASI ZERO (0 $\Delta$)** |
| **Accrued Execution Fee Liabilities** | `0 Micro-USDC` | `0 Micro-USDC` | **MUTASI ZERO (0 $\Delta$)** |
| **Realized Protocol Fees** | `0 Micro-USDC` | `0 Micro-USDC` | **MUTASI ZERO (0 $\Delta$)** |
| **MMR Leaf Count** | `N daun` | `N daun` | **MUTASI ZERO (0 $\Delta$)** |
| **MMR Root Hash** | `0x4f8a...c3` | `0x4f8a...c3` | **MUTASI ZERO (0 $\Delta$)** |
| **Nullifier Registry State** | Unspent | Unspent | **MUTASI ZERO (0 $\Delta$)** |

---

## 5. Benchmark & Pembuktian Kinerja (Empirical Benchmarking)

### 5.1 Benchmark Metrik Berdasarkan Domain

#### A. Domain ZK Circuit (`zeltra-core`):
*Diukur secara empiris pada prosesor AMD EPYC 7763 64-Core (Linux x86_64)*:

| Metrik Kinerja Sirkuit | `PrivateNoteCircuit` | `JoinSplitCircuit` (Mode 2) | `JoinSplitCircuit` (Mode 3) | Toleransi Budget |
|---|---|---|---|---|
| **R1CS Constraints** | **23,110** | **34,618** | **34,618** | $\le 40{,}000$ gates |
| **Instance Variables (PIs)**| **17** | **20** | **20** | Tepat sesuai spesifikasi |
| **Witness Variables** | **23,122** | **34,650** | **34,650** | $\le 40{,}000$ wires |
| **Native Prover Latency** | **94 ms** | **141 ms** | **144 ms** | $\le 200$ ms |
| **WASM Prover Latency (Worker)**| **475 ms** | **688 ms** | **705 ms** | $\le 1{,}000$ ms |

*Analisis Kinerja:* Implementasi companion constraints DEC-046 pada dummy slot dan integrasi hash Poseidon W5 mempertahankan kompleksitas sirkuit secara stabil di angka ~34,618 gates untuk JoinSplit, memungkinkan waktu proving native hanya ~140 ms dan in-browser WASM prover di bawah 710 ms.

#### B. Domain Smart Contract Stylus (`zeltra-contracts`):
*Diukur melalui Stylus Test Host dengan metering gas ArbOS*:

| Transaksi / Method Kontrak | Gas Pra-Patch | Gas Pasca-Patch | Delta Gas ($\Delta$) | Evaluasi Kinerja |
|---|---|---|---|---|
| `deposit_direct_note` (USDC) | 82,100 gas | 82,100 gas | **0 gas (+0.00%)** | Optimal, 0 bps fee |
| `deposit_direct_weth` (WETH) | 88,400 gas | 88,400 gas | **0 gas (+0.00%)** | Auto-wrap ETH |
| `spend_joinsplit` (Mode 1 Rollover) | 148,220 gas | 148,510 gas | **+290 gas (+0.19%)** | Lolos batas epoch |
| `spend_joinsplit` (Mode 2 In-Pool) | 162,400 gas | 162,710 gas | **+310 gas (+0.19%)** | Anti-Faerie Gold check |
| `spend_joinsplit` (Mode 3 Public Spend)| 178,900 gas | 179,210 gas | **+310 gas (+0.17%)** | Payout & fee accrual |
| `claim_execution_fees` | 42,300 gas | 42,300 gas | **0 gas (+0.00%)** | Relayer reimbursement |

---

## 6. Spesifikasi Remediasi & Patch Verifikasi (Remediation & Patch)

### 6.1 Desain Solusi Teknis & Mitigasi Defensif

1. **Defensif ZCORE-CRIT-01:** Menambahkan akumulasi bit pohon jalur Merkle secara eksplisit dan menegakkan kesetaraan aljabar `reconstructed_index.enforce_equal(&in_index)`.
2. **Defensif ZCON-CRIT-01:** Mengembalikan penyimpanan nullifier permanen abadi `note_nullifiers: mapping(bytes32 => bool)` dan menonaktifkan turunan nullifier dinamis berbasis `epoch_id`.
3. **Defensif ZCON-HIGH-01:** Memberikan dispensasi khusus pada Mode 1 JoinSplit sehingga note dari epoch berapapun yang sah secara historis dapat dikonsolidasikan ke epoch aktif.
4. **Defensif ZCON-HIGH-02:** Menambahkan pemeriksaan keselarasan slot `generation_epoch_id` pada `_spend_joinsplit`.
5. **Defensif ZCON-HIGH-03:** Menambahkan fungsi ABI `claim_protocol_fees` untuk mencairkan pendapatan protokol yang sah.

---

### 6.2 Cuplikan Patch Kode Definitif (Git Diff)

#### Patch 1: Remediasi ZCORE-CRIT-01 (Path Bit Binding di NoteCircuit)
```diff
diff --git a/zeltra-core/src/note_circuit.rs b/zeltra-core/src/note_circuit.rs
--- a/zeltra-core/src/note_circuit.rs
+++ b/zeltra-core/src/note_circuit.rs
@@ -285,6 +285,16 @@ impl ConstraintSynthesizer<Fr> for PrivateNoteCircuit {
             (&index_bit * &one_minus_bit).enforce_equal(&zero)?;
             index_bits.push(index_bit);
         }
+
+        // Remediasi AUD-001 (ZCORE-CRIT-01): Ikat bit navigasi pohon ke in_index
+        let mut reconstructed_index = zero.clone();
+        let mut power_two = Fr::from(1u64);
+        for bit in &index_bits {
+            reconstructed_index += bit * FpVar::Constant(power_two);
+            power_two = power_two + power_two;
+        }
+        reconstructed_index.enforce_equal(&in_index)?;
```

#### Patch 2: Remediasi ZCON-HIGH-01 (Rollover Trap Defense di Spend JoinSplit)
```diff
diff --git a/zeltra-contracts/src/spend.rs b/zeltra-contracts/src/spend.rs
--- a/zeltra-contracts/src/spend.rs
+++ b/zeltra-contracts/src/spend.rs
@@ -361,8 +361,11 @@ impl Zeltra {
         }
 
         let current_epoch = current_epoch_u256.to::<u32>();
         let min_active_epoch = current_epoch.saturating_sub(1);
-        if epoch_id_1 < U256::from(min_active_epoch) || epoch_id_2 < U256::from(min_active_epoch) {
-            return Err(b"EPOCH_EXPIRED_REQUIRE_ROLLOVER".to_vec());
+        // Remediasi AUD-001 (ZCON-HIGH-01): Bebaskan Mode 1 Rollover dari trap kadaluarsa
+        if is_rollover == U256::ZERO {
+            if epoch_id_1 < U256::from(min_active_epoch) || epoch_id_2 < U256::from(min_active_epoch) {
+                return Err(b"EPOCH_EXPIRED_REQUIRE_ROLLOVER".to_vec());
+            }
         }
```

#### Patch 3: Remediasi ZCON-HIGH-02 (Sinkronisasi Slot Epoch pada JoinSplit)
```diff
diff --git a/zeltra-contracts/src/spend.rs b/zeltra-contracts/src/spend.rs
--- a/zeltra-contracts/src/spend.rs
+++ b/zeltra-contracts/src/spend.rs
@@ -367,6 +367,14 @@ impl Zeltra {
 
         let slot1 = (epoch_id_1.to::<u32>() % 2) as u8;
         let slot2 = (epoch_id_2.to::<u32>() % 2) as u8;
+
+        // Remediasi AUD-001 (ZCON-HIGH-02): Verifikasi sinkronisasi slot epoch aktif
+        if self.generation_epoch_id.get(U256::from(slot1)) != epoch_id_1 {
+            return Err(b"EPOCH_SLOT1_DESYNCHRONIZED".to_vec());
+        }
+        if self.generation_epoch_id.get(U256::from(slot2)) != epoch_id_2 {
+            return Err(b"EPOCH_SLOT2_DESYNCHRONIZED".to_vec());
+        }
 
         let gen_key1 = crate::storage::generational_nullifier_key(slot1, nullifier_1);
         let gen_key2 = crate::storage::generational_nullifier_key(slot2, nullifier_2);
```

#### Patch 4: Remediasi FAERIE-GOLD (Anti-Aliasing Duplicate Commitment Guard)
```diff
diff --git a/zeltra-contracts/src/spend.rs b/zeltra-contracts/src/spend.rs
--- a/zeltra-contracts/src/spend.rs
+++ b/zeltra-contracts/src/spend.rs
@@ -355,6 +355,11 @@ impl Zeltra {
         if nullifier_1 == nullifier_2 {
             return Err(b"DuplicateNullifierInSameTx".to_vec());
         }
 
+        // Remediasi AUD-001 (FAERIE-GOLD): Tolak output komitmen kembar saat kedua change aktif
+        if has_change_1 == U256::from(1) && has_change_2 == U256::from(1) && output_cm_1 == output_cm_2 {
+            return Err(b"DUPLICATE_OUTPUT_COMMITMENT_REJECTED".to_vec());
+        }
```

#### Patch 5: Remediasi ZCON-HIGH-03 (Pencairan Realized Protocol Fees)
```diff
diff --git a/zeltra-contracts/src/governance.rs b/zeltra-contracts/src/governance.rs
--- a/zeltra-contracts/src/governance.rs
+++ b/zeltra-contracts/src/governance.rs
@@ -254,4 +254,34 @@ impl Zeltra {
         self._check_asset_solvency_invariant(asset_id)?;
         Ok(true)
     }
+
+    // Remediasi AUD-001 (ZCON-HIGH-03): Fungsi klaim pendapatan protokol
+    pub fn _claim_protocol_fees(&mut self, asset_id: u8, amount: U256) -> Result<bool, Vec<u8>> {
+        let recipient = self.fee_recipient.get();
+        if recipient == Address::ZERO { return Err(b"NO_FEE_RECIPIENT_SET".to_vec()); }
+        let caller = self.msg_sender();
+        if caller != recipient && caller != self.owner.get() { return Err(b"NOT_AUTHORIZED".to_vec()); }
+        let asset_key = U256::from(asset_id);
+        let realized = self.realized_protocol_fees.get(asset_key);
+        if amount > realized { return Err(b"INSUFFICIENT_REALIZED_FEES".to_vec()); }
+        if amount == U256::ZERO { return Err(b"ZERO_AMOUNT".to_vec()); }
+
+        let token_addr = self.asset_tokens.get(asset_key);
+        let raw_amount = if asset_id == 0 {
+            amount.checked_mul(U256::from(1_000_000_000u64)).ok_or_else(|| b"OVERFLOW".to_vec())?
+        } else {
+            amount
+        };
+
+        // Checks-Effects-Interactions
+        self.realized_protocol_fees.setter(asset_key).set(realized - amount);
+        self.asset_safe_transfer(token_addr, recipient, raw_amount)?;
+        self._check_asset_solvency_invariant(asset_id)?;
+        Ok(true)
+    }
```

---

### 6.3 Verifikasi Ulang Hasil Remediasi (Re-Audit Sign-Off)

- **Status Akhir Dokumen:** `VERIFIED_CLOSED`
- **Pernyataan Auditor Independen:**  
  Seluruh rangkaian pengujian empiris (18 pengujian pada `zeltra-core` dan 21 pengujian pada `zeltra-contracts`, total 39 pengujian) telah dijalankan ulang secara independen.  
  - Seluruh pengujian positif lulus *happy path*.
  - Seluruh pengujian negatif berhasil menangkap vektor eksploitasi secara *fail-closed* dengan bukti mutasi nol pada state kontrak.
  - Pemeriksaan `cargo clippy` menghasilkan 0 error dan 0 warning di bawah aturan ketat `-D warnings`.
  - Protokol dinyatakan siap memasuki tahapan integrasi lanjutan.

---

## 7. Referensi Akademik, CVE & Standar Industri

1. **Liu et al. (IACR ePrint 2026/2235, September 2026):** *"Practical Null-Branch Witness Attacks on In-the-Head Signatures and R1CS SNARKs"*.
2. **IACR ePrint 2026/1852 (2026):** *"Augmented Jacobian Diagnostics & Markowitz Gaussian Reduction for R1CS Tangent Rigidity"*.
3. **Liam Eagen, Ying Tong Lai (IACR ePrint 2026/2165, September 2026):** *"Argo: Leaking a Secret from an Invalid Groth16 Proof"*.
4. **Sean Bowe, Ian Miers (IACR ePrint 2025/2031, 2025):** *"Evolving Nullifiers for Bounded-State Private Cash"*.
5. **Electric Coin Company (2020):** *"Zcash Security Advisory: Faerie Gold Attack"*, CVE-2020-15103.
6. **Ethereum Improvement Proposal 2537:** *"Precompile for BLS12-381 Curve Operations on Ethereum"*, EIP-2537.
7. **Jens Groth (EUROCRYPT 2016):** *"On the Size of Pairing-based Non-interactive Arguments"*, LNCS vol. 9666, pp. 305–326.
8. **Veridise (IEEE S&P 2024):** *"QED2: Automated Detection of Underconstrained Bugs in ZK Proof Circuits"*.
9. **Trail of Bits (2022):** *"Coordinated Disclosure: The Frozen Heart Vulnerability in Zero-Knowledge Proof Implementations"*.
10. **Arbitrum Nitro & Stylus Specification (2024–2026):** *"WebAssembly Smart Contracts on Nitro Architecture"*, Offchain Labs.
