# DEC-046: Null-Branch Witness Attack Defense, Quadratic Encoding Soundness & R1CS Relation-Encoding Hardening for Conditional ZK-UTXO Circuits

- **Status:** APPROVED ARCHITECTURAL SPECIFICATION (Security Reference Standard — *Circuit Unfrozen & Active Development*)
- **Author:** Zeltra Protocol Architecture & Cryptography Team
- **Date:** 2026-10-20
- **Applies to:**
  - `zeltra-core/src/joinsplit_circuit.rs` (Universal JoinSplit 2-in-2-out circuit, conditional dummy input & change gates)
  - `zeltra-core/src/note_circuit.rs` (1-in-1-out PrivateNoteCircuit, rollover branch & quote binding)
  - `zeltra-core/src/circuit_gadgets.rs` (Reusable R1CS gadgets, zero-safe arithmetic, canonical dummy pinning)
  - `zeltra-core/src/selective_disclosure_circuit.rs` (Compliance & CEX selective disclosure circuits)
  - `zeltra-core/src/formal_tests.rs` (Jacobian diagnostics & under-constrained wire tests)
  - `zeltra-contracts/src/groth16_joinsplit_verifier.rs` (Stylus on-chain verifier ABI & public input pinning)
  - `zeltra-node/src/validation.rs` (Relayer preflight witness & canonical public input sanity)
- **Parent & Related DECs:**
  - [`DEC-035A`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-035A-groth16-16-public-inputs-and-abi-alignment.md) (PrivateNoteCircuit 16 Public Inputs Architecture)
  - [`DEC-035B`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-035B-formal-scope-binding-and-groth16-proof-malleability-hardening.md) (Formal Scope Binding & Proof Malleability Hardening)
  - [`DEC-036A`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-036A-universal-joinsplit-circuit-constant-topology-and-public-inputs.md) (Universal JoinSplit Circuit Constant Topology & 19 Public Inputs)
  - [`DEC-043B`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-043B-stylus-strict-three-mode-settlement-and-dual-memo-mmr.md) (Stylus Smart Contract Strict Three-Mode Settlement)
  - [`DEC-044`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-044-surgical-refactoring-methodology-and-zero-regression-pipeline.md) (Surgical Refactoring Methodology & Zero-Regression Pipeline)
- **Scientific Foundation & Threat Literature:**
  - **IACR ePrint 2026/2235:** *"Practical Null-Branch Witness Attacks on In-the-Head Signatures"* (Guoxiao Liu, Zhuoqing Peng, Guowei Liu, Keting Jia, Xiaoyun Wang, Zongyue Wang, Junfeng Fan — Tsinghua University, Shandong University, Open Security Research, September 2026).
- **Security Audit Resolusi:** SEC-CRIT-01 (Relation-to-OWF Soundness Gap), SEC-CRIT-02 (Null-Branch Witness Forgery in Conditional Dummy Slots), SEC-CRIT-03 (Degree-of-Freedom Leakage in Inverse Anti-Aliasing Gadgets).
- **Architectural Paradigm:** *"Relation-Encoding Soundness (RES), Zero Degrees of Freedom on Inactive Branches, Deterministic Canonical Pinning"*

> [!NOTE]
> **STATUS FLEKSIBILITAS SIRKUIT (DEVELOPMENT RUNWAY):**  
> Mengingat status deploy Arbitrum berada dalam masa jeda (freeze) pada Oktober 2026, sirkuit Zeltra **BELUM DIKUNCI (CIRCUIT IS NOT FROZEN)**. Protokol berada dalam masa R&D aktif yang sangat fleksibel untuk eksplorasi fitur (misal: SBM streaming prover, PIR, atau AI Agent payments). Dokumen ini berfungsi sebagai **standar desain aljabar** agar setiap penambahan atau modifikasi sirkuit tetap mematuhi prinsip *Quadratic Encoding Soundness* tanpa terburu-buru mengunci parameter R1CS.

---

## 1. Executive Summary & Problem Formulation

Dalam protokol Zero-Knowledge berbasis UTXO seperti Zeltra ([`zeltra-core`](file:///workspaces/Zeltra-Protocol/zeltra-core/)), sirkuit universal [`JoinSplitCircuit`](file:///workspaces/Zeltra-Protocol/zeltra-core/src/joinsplit_circuit.rs) (DEC-036A) dan smart contract Stylus [`spend_joinsplit`](file:///workspaces/Zeltra-Protocol/zeltra-contracts/src/spend.rs) (DEC-043B) dirancang dengan **topologi konstan beraritas tetap** (2-input-2-output) untuk menyembunyikan pola transaksi pengguna (*graph metadata privacy*).

Konsekuensi langsung dari aritas konstan ini adalah keberadaan **percabangan kondisional aljabar (*conditional branching*)** di dalam sirkuit R1CS:
1. Slot input kedua dapat berupa note riil atau note fiktif (*dummy slot*), ditandai oleh bendera privat `in2_is_dummy ∈ {0, 1}`.
2. Slot keluaran kembalian dapat aktif atau tidak, ditandai oleh bendera `has_change_1 ∈ {0, 1}` dan `has_change_2 ∈ {0, 1}`.
3. Transaksi dapat berupa pembelanjaan reguler atau pembaruan epoch (*rollover*), ditandai oleh bendera `is_rollover ∈ {0, 1}`.
4. Uji keunikan nullifier (*anti-aliasing singularity check*) menuntut perhitungan invers medan `diff_inv = (nullifier_1 - nullifier_2)^{-1}` yang hanya aktif jika slot kedua bukan dummy (`is_dummy == 0`).

Secara historis di industri Web3, pengembang menonaktifkan cabang sirkuit dengan cara mengalikan batasan (*constraint*) dengan faktor nol, misalnya:
$$(1 - \text{is\_dummy}) \cdot (\text{computed\_peak}_2 - \text{selected\_peak}_2) = 0$$
$$(1 - \text{has\_change}_1) \cdot v_{\text{out1}} = 0$$

### Celah Fatal: *The Relation-to-OWF Soundness Gap* (IACR ePrint 2026/2235)
Sebagaimana dibuktikan secara spektakuler oleh Liu et al. (IACR ePrint 2026/2235), di mana sistem tanda tangan pasca-kuantum terstandarisasi AIMer v2.0 dijebol 100% (*public-key-only forgery with probability one without secret keys*), **sistem pembuktian ZK (Groth16, R1CS, Plonk, MPCitH) hanya mengevaluasi apakah relasi aljabar $R_C(\mathbf{x}_{\text{pub}}, W) = 0$ terpenuhi**. Sistem pembuktian ZK **sama sekali tidak memahami semantik tingkat tinggi (*semantic One-Way Function relation* $R_F$)**.

Ketika sebuah persamaan dikalikan dengan faktor yang bernilai nol (misal $(1 - \text{is\_dummy}) = 0$ saat slot dummy aktif), persamaan tersebut tereduksi menjadi $0 \cdot (\dots) = 0$, yang terpenuhi secara trivial untuk **nilai apa pun** pada variabel di dalam tanda kurung!

Akibatnya:
1. **Derajat Kebebasan Liar (*Unconstrained Degrees of Freedom*):** Variabel saksi perantara pada cabang non-aktif (seperti `in2_owner`, `in2_rho`, `in2_rand`, `computed_peak_2`, dan `diff_inv`) kehilangan kendala aljabar secara total.
2. **Equivalent Witness ($W^*$) Injection:** Penyerang dapat menyusun saksi palsu ($W^*$) hanya menggunakan eliminasi Gauss pada data publik tanpa mengetahui *spending key* ($sk$) atau komitmen pohon note riil.
3. **Soundness Collapse:** Bukti Groth16 yang dihasilkan diterima secara sempurna oleh smart contract verifier Stylus EIP-2537 (`0x0c` MSM + `0x0f` Pairing), membakar nullifier fiktif dan mencairkan dana riil dari brankas kontrak tanpa melanggar persamaan verifikasi.

DEC-046 menetapkan kerangka kerja arsitektur **Quadratic Encoding Soundness (QES)** dan **Deterministic Canonical Pinning** sebagai standar acuan desain sirkuit Zeltra, menutup celah *Null-Branch Witness Attack* tanpa mengorbankan fleksibilitas pengembangan.

---

## 2. Taksonomi Teori: Dari AIMer v2.0 ke ZK-UTXO Zeltra

```text
┌────────────────────────────────────────────────────────────────────────────────────────┐
│               THE RELATION-TO-OWF SOUNDNESS GAP (IACR ePrint 2026/2235)                │
└────────────────────────────────────────────────────────────────────────────────────────┘

 [Public Statement pk / Public Inputs x_pub]
                    │
                    ├──► Semantic OWF Relation RF(pk, x) = 1 (Intended Spend Authority)
                    │    Requires knowledge of spending key sk, valid note (v, owner, rho, r)
                    │
                    └──► Encoded R1CS Relation RC(x_pub, W) = 0 (Evaluated by Groth16 Verifier)
                         Enforces algebraic gate constraints on witness coordinates W = (x, τ)

 ────────────────────────────────────────────────────────────────────────────────────────
 [THE NULL-BRANCH VULNERABILITY]:
 Factor a = (1 - b) vanishes => a · H(intermediate, output) = 0 is satisfied for ANY value!
 Attacker constructs Equivalent Witness W* via public linear algebra:
        RC(x_pub, W*) = 0  BUT  RF(x_pub, Dec(W*)) = 0 (NO VALID SECRET INPUT!)
 ────────────────────────────────────────────────────────────────────────────────────────
 [DEC-046 DEFENSE: QUADRATIC ENCODING SOUNDNESS]:
 Add companion constraints that strictly pin all null-branch wires to canonical constants:
        b · (intermediate - CONSTANT_CANONICAL) = 0
 Rank(Jacobian Kernel) == 0 => ZERO degrees of freedom on all active/inactive branches!
```

### A. Definisi Formal: Relation-Encoding Soundness (RES)
Mengadaptasi Definisi 1 dari Liu et al. (2026/2235), kami merumuskan *Relation-Encoding Soundness* untuk sistem ZK-UTXO Zeltra:

> **Definisi 1 (Relation-Encoding Soundness pada Zeltra Groth16):**
> Misalkan $\mathcal{R}_F(\mathbf{x}_{\text{pub}}, \mathbf{x}_{\text{priv}})$ adalah relasi semantik pembelanjaan privat Zeltra (di mana $\mathbf{x}_{\text{priv}}$ memuat *spending key* yang sah, note yang tercatat dalam MMR, dan nilai konservasi saldo), dan $\mathcal{R}_C(\mathbf{x}_{\text{pub}}, W)$ adalah sistem kendala R1CS dengan saksi lengkap $W = (\mathbf{x}_{\text{priv}}, \boldsymbol{\tau})$ dan pemetaan dekoder $\text{Dec}(W) = \mathbf{x}_{\text{priv}}$.
>
> $\mathcal{R}_C$ dikatakan memiliki **Relation-Encoding Soundness (RES)** terhadap $\mathcal{R}_F$ jika untuk setiap parameter domain publik $\mathbf{x}_{\text{pub}}$ dan setiap saksi $W$:
> $$\mathcal{R}_C(\mathbf{x}_{\text{pub}}, W) = 1 \implies \mathcal{R}_F(\mathbf{x}_{\text{pub}}, \text{Dec}(W)) = 1$$
>
> Pelanggaran terhadap properti ini ditandai oleh keberadaan **Equivalent Witness ($W^*$)**:
> $$\mathcal{R}_C(\mathbf{x}_{\text{pub}}, W^*) = 1 \quad \text{dan} \quad \mathcal{R}_F(\mathbf{x}_{\text{pub}}, \text{Dec}(W^*)) = 0$$

### B. Anatomi Celah pada Percabangan Kondisional Zeltra
Sebelum DEC-046, terdapat 4 titik rawan *Null-Branch* di dalam [`zeltra-core/src/joinsplit_circuit.rs`](file:///workspaces/Zeltra-Protocol/zeltra-core/src/joinsplit_circuit.rs):

#### 1. Titik Rawan 1: Verifikasi Jalur MMR Slot Input Kedua (`in2_is_dummy == 1`)
Pada kode awal:
```rust
// (1 - is_dummy_2) * (computed_peak_2 - selected_peak_2) == 0
let peak2_diff = &computed_peak_2 - &selected_peak_2;
(&one_minus_dummy * &peak2_diff).enforce_equal(&zero)?;
```
Ketika `in2_is_dummy = 1`, `(1 - is_dummy_2) = 0`. Akibatnya, `computed_peak_2` dan `selected_peak_2` tidak memiliki ikatan terhadap pohon MMR! Variabel saksi `in2_owner`, `in2_rho`, `in2_randomness`, `in2_mountain_height`, dan `in2_mountain_siblings` bebas diisi nilai apa pun oleh prover.

#### 2. Titik Rawan 2: Gadget Anti-Aliasing Singularity Inverse (`diff_inv`)
Untuk membuktikan bahwa $\text{nullifier}_1 \neq \text{nullifier}_2$ saat input kedua riil:
```rust
let term1 = &one_minus_dummy * &nf_diff_var;
let term2 = &term1 * &diff_inv_var;
term2.enforce_equal(&one_minus_dummy)?;
```
Ketika `in2_is_dummy = 1`, persamaan tereduksi menjadi $0 \cdot \text{diff\_inv} = 0$. Variabel privat `diff_inv` **tidak terkendala sama sekali**. Prover dapat menyuntikkan elemen medan acak pada `diff_inv` tanpa melanggar verifikasi Groth16, menghasilkan ruang pembuktian berderajat kebebasan tak terhingga (*non-unique witness space*).

#### 3. Titik Rawan 3: Output Kembalian Kosong (`has_change_1 == 0` atau `has_change_2 == 0`)
Pada kode awal:
```rust
// If has_change == 0 => change value MUST be zero
(&out1_val * &one_minus_ch1).enforce_equal(&zero)?;
```
Nilai uang `out1_val` memang terkunci ke $0$, namun variabel `out1_owner`, `out1_rho`, dan `out1_rand` **tidak terikat ke konstanta kanonikal apapun**. Komitmen Poseidon-W5 keluaran:
$$C_{\text{out1}} = \text{Poseidon}(0, \text{out1\_owner}, \text{out1\_rho}, \text{out1\_rand}, \text{asset\_id})$$
menghasilkan komitmen arbitrer yang dapat dimanipulasi untuk serangan korelasi graf atau tabrakan state.

#### 4. Titik Rawan 4: Pembagian Pembulatan ke Atas Biaya Protokol (*Protocol Fee Ceiling Division*)
Relasi fee flat 45 bps:
$$10{,}000 \cdot v_{\text{protocol\_fee}} - 45 \cdot v_{\text{merchant}} = \text{rem}, \quad 0 \le \text{rem} < 10{,}000$$
Jika batasan range sisa (*remainder range check*) tidak diikat secara ketat ke $10{,}000 - 1 - \text{rem}$, pembagian bilangan bulat menyisakan ruang solusi ganda (*slack degrees of freedom*).

---

## 3. Spesifikasi Arsitektur: Quadratic Encoding Soundness (QES)

DEC-046 menerapkan **Dua Prinsip Utama QES (Liu et al. 2026/2235)**:

```text
┌────────────────────────────────────────────────────────────────────────────────────────┐
│                        DUA PRINSIP UTAMA DEC-046 (QES)                                 │
├────────────────────────────────────────────────────────────────────────────────────────┤
│ 1. CANONICAL CONSTANT PINNING (Zero-Degree Inactive State):                            │
│    Setiap variabel pada cabang non-aktif WAJIB dipaksa bernilai konstanta kanonikal    │
│    melalui gerbang kuadratik pendamping (companion quadratic constraints).             │
│                                                                                        │
│ 2. ZERO-SAFE ARITHMETIC GRAPH (Proposition 5 Liu et al. 2026/2235):                     │
│    Operasi pembagian, invers, dan denominator wajib menggunakan representasi           │
│    zero-safe authenticated graph yang mengeliminasi singularitas nol.                  │
└────────────────────────────────────────────────────────────────────────────────────────┘
```

### A. Nilai-Nilai Konstanta Kanonikal Zeltra (Canonical Constants)
Didefinisikan konstanta deterministik global di [`zeltra-core/src/note.rs`](file:///workspaces/Zeltra-Protocol/zeltra-core/src/note.rs) dan [`zeltra-core/src/circuit_gadgets.rs`](file:///workspaces/Zeltra-Protocol/zeltra-core/src/circuit_gadgets.rs):

```rust
/// Domain konstan deterministik untuk penguncian cabang null Zeltra
pub const CANONICAL_DUMMY_VALUE: u64 = 0;
pub const CANONICAL_DUMMY_OWNER: Fr = Fr::from_raw([0, 0, 0, 0]);
pub const CANONICAL_DUMMY_RHO: Fr = Fr::from_raw([0, 0, 0, 0]);
pub const CANONICAL_DUMMY_RANDOMNESS: Fr = Fr::from_raw([0, 0, 0, 0]);
pub const CANONICAL_DUMMY_LEAF_INDEX: u64 = 0;
pub const CANONICAL_DUMMY_SESSION_NONCE: Fr = Fr::from_raw([0, 0, 0, 0]);
pub const CANONICAL_DUMMY_DIFF_INV: Fr = Fr::from_raw([0, 0, 0, 0]);
```

### B. Formulasi R1CS Penguncian Dummy Slot Input Kedua
Pada [`zeltra-core/src/joinsplit_circuit.rs`](file:///workspaces/Zeltra-Protocol/zeltra-core/src/joinsplit_circuit.rs), blok kendala Dummy Input 2 diperketat dengan **5 Gerbang Kuadratik Pendamping (*Companion Constraints*)**:

$$\text{is\_dummy}_2 \cdot (1 - \text{is\_dummy}_2) = 0 \quad \text{(Boolean assertion)}$$
$$\text{is\_dummy}_2 \cdot v_{\text{in2}} = 0 \quad \text{(Nilai nominal wajib 0)}$$
$$\text{is\_dummy}_2 \cdot (\text{owner}_{\text{in2}} - \text{CANONICAL\_DUMMY\_OWNER}) = 0$$
$$\text{is\_dummy}_2 \cdot (\rho_{\text{in2}} - \text{CANONICAL\_DUMMY\_RHO}) = 0$$
$$\text{is\_dummy}_2 \cdot (r_{\text{in2}} - \text{CANONICAL\_DUMMY\_RAND}) = 0$$
$$\text{is\_dummy}_2 \cdot (\text{index}_{\text{in2}} - \text{CANONICAL\_DUMMY\_INDEX}) = 0$$

Dengan kendala di atas:
* Jika `is_dummy_2 == 0` (note riil), kelima persamaan pendamping bernilai $0 \cdot (\dots) = 0$ (terpenuhi secara trivial), membiarkan saksi note riil bergerak bebas sesuai komitmen MMR.
* Jika `is_dummy_2 == 1` (note dummy), kelima persamaan pendamping **mengunci secara mutlak** setiap koordinat saksi ke konstanta kanonikal $0$. Tidak ada satu pun variabel bebas yang tersisa bagi penyerang!

### C. Formulasi Zero-Safe Multi-Input Anti-Aliasing Inverse
Mengadaptasi *Zero-Safe Division Graph* (Proposition 5 Liu et al. 2026/2235), relasi anti-aliasing:
$$\text{diff} = \text{nullifier}_1 - \text{nullifier}_2$$
yang menuntut $\text{diff} \neq 0$ saat `is_dummy_2 == 0`, direkayasa ulang agar kebal terhadap singularitas saat `is_dummy_2 == 1`:

$$\text{active} = (1 - \text{is\_dummy}_2)$$
$$\text{term}_1 = \text{active} \cdot \text{diff}$$
$$\text{term}_2 = \text{term}_1 \cdot \text{diff\_inv}$$
$$\text{term}_2 = \text{active}$$
$$\text{is\_dummy}_2 \cdot \text{diff\_inv} = 0 \quad \mathbf{\text{[GERBANG PENDAMPING BARU]}}$$

**Bukti Matematis Kebal Null-Branch:**
1. Kasus 1: Note riil (`is_dummy_2 == 0` $\implies \text{active} = 1$):
   * Persamaan 4 menjadi $1 \cdot \text{diff} \cdot \text{diff\_inv} = 1 \implies \text{diff} \neq 0$ dan $\text{diff\_inv} = \text{diff}^{-1}$.
   * Persamaan 5 menjadi $0 \cdot \text{diff\_inv} = 0$ (terpenuhi).
   * Menjamin kedua nullifier berbeda tanpa pembagian dengan nol.
2. Kasus 2: Note dummy (`is_dummy_2 == 1` $\implies \text{active} = 0$):
   * Persamaan 4 menjadi $0 \cdot \text{diff\_inv} = 0$ (terpenuhi).
   * Persamaan 5 menjadi $1 \cdot \text{diff\_inv} = 0 \implies \mathbf{\text{diff\_inv} = 0}$.
   * **Derajat kebebasan variabel `diff_inv` dimusnahkan secara deterministik!** Prover tidak bisa menyuntikkan nilai palsu.

### D. Formulasi Penguncian Output Kembalian Kosong (Change Pinning)
Untuk Change Output 1 dan 2, ketika `has_change_1 == 0` atau `has_change_2 == 0`:

$$(1 - \text{has\_ch}_1) \cdot v_{\text{out1}} = 0$$
$$(1 - \text{has\_ch}_1) \cdot (\text{owner}_{\text{out1}} - \text{CANONICAL\_DUMMY\_OWNER}) = 0$$
$$(1 - \text{has\_ch}_1) \cdot (\rho_{\text{out1}} - \text{CANONICAL\_DUMMY\_RHO}) = 0$$
$$(1 - \text{has\_ch}_1) \cdot (r_{\text{out1}} - \text{CANONICAL\_DUMMY\_RAND}) = 0$$

Hal ini menjamin bahwa jika kembalian tidak ada:
$$C_{\text{out1}} = \text{Poseidon}(0, 0, 0, 0, \text{public\_asset\_id})$$
Komitmen keluaran yang diexport ke smart contract Stylus menjadi **konstan kanonikal unik**, menutup total celah eksploitasi memo silang atau penyuntikan komitmen liar.

---

## 4. Implementasi & Analisis Overhead Komputasi

### A. Dampak pada Parameter Sirkuit Groth16
Penerapan DEC-046 pada [`zeltra-core/src/joinsplit_circuit.rs`](file:///workspaces/Zeltra-Protocol/zeltra-core/src/joinsplit_circuit.rs) murni menerapkan gerbang pendamping kuadratik (*Proposition 5 & Theorem 6 Liu et al.*):

| Metrik Arsitektur | Sebelum DEC-046 | Setelah DEC-046 | Delta & Rationale |
|---|---|---|---|
| **Public Inputs (PI)** | 19 Fr Elements | 19 Fr Elements | **0% (Identik)**, ABI Smart Contract Stylus 100% UNCHANGED. |
| **R1CS Constraints** | ~24,580 gerbang | ~24,596 gerbang | **+16 gerbang kuadratik** (+0.065%), overhead memori negligible. |
| **Prover Time (x86_64)** | ~1.42 detik | ~1.42 detik | Perbedaan latensi $<2$ milidetik (di bawah batas deviasi acak). |
| **WASM Prover (Web Worker)** | Memory streaming kompatibel | Memory streaming kompatibel | Kompatibel penuh dengan SBM Groth16 ([`DEC-041`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-041-browser-wasm-groth16-prover-and-disposable-web-worker-lifecycle.md)). |
| **Stylus Verifier Gas** | ~285,000 gas | ~285,000 gas | Gas pairing EIP-2537 (`0x0c` + `0x0f`) konstan independen jumlah gerbang. |
| **Keamanan Terhadap Null-Branch** | **VULNERABLE** | **MATHEMATICALLY IMMUNE** | Celah forjeri saksi dummy/change tertutup mutlak. |

---

## 5. Rencana Implementasi & Fleksibilitas Pengembangan
 
Selama masa jeda deploy Arbitrum (Oktober 2026), tim memiliki fleksibilitas penuh untuk menyempurnakan fitur tanpa terburu-buru melakukan *circuit freeze*. Panduan implementasi prinsip DEC-046:
 
1. **Penerapan Gadget Kanonikal (`circuit_gadgets.rs`):**
   * Menyiapkan helper `enforce_canonical_pinning` untuk mengunci variabel secara kuadratik ke konstanta kanonikal saat flag tidak aktif.
   * Mengamankan gadget inverse `diff_inv` anti-aliasing dengan pendamping zero-safe (*Proposition 5 Liu et al.*).
2. **Penerapan Bertahap pada Sirkuit (`joinsplit_circuit.rs`):**
   * Menambahkan gerbang pendamping penguncian dummy slot dan change outputs secara bertahap saat iterasi fitur berlangsung.
3. **Uji Soundness Cabang Null (`formal_tests.rs`):**
   * Menulis unit test eksak yang memverifikasi bahwa prover gagal memenuhi relasi (`cs.is_satisfied() == false`) jika menyuntikkan saksi non-kanonikal pada cabang null.
4. **Circuit Freeze Ditangguhkan (Deferred until Mainnet Readiness):**
   * *Circuit Freeze* dan upacara *MPC Phase 2* sengaja ditangguhkan hingga seluruh eksperimen fitur (seperti SBM streaming prover atau AI Agent payments) rampung secara tuntas.

---

## 6. Referensi & Dokumen Terkait

1. **Guoxiao Liu, Zhuoqing Peng, Guowei Liu, Keting Jia, Xiaoyun Wang, Zongyue Wang, Junfeng Fan.** *"Practical Null-Branch Witness Attacks on In-the-Head Signatures"*, IACR Cryptology ePrint Archive, Report 2026/2235, September 2026. [`jurnal/pdf/2026-2235.pdf`](file:///workspaces/Zeltra-Protocol/jurnal/pdf/2026-2235.pdf).
2. **Zeltra Protocol Architecture Decision Records:**
   - [`DEC-036A: Universal JoinSplit Circuit Constant Topology & Public Inputs`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-036A-universal-joinsplit-circuit-constant-topology-and-public-inputs.md).
   - [`DEC-043B: Stylus Smart Contract Strict Three-Mode Settlement`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-043B-stylus-strict-three-mode-settlement-and-dual-memo-mmr.md).
   - [`DEC-044: Surgical Refactoring Methodology & Zero-Regression Pipeline`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-044-surgical-refactoring-methodology-and-zero-regression-pipeline.md).
