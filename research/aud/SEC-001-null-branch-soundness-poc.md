# SEC-001: Null-Branch Witness Soundness Breakdown in Universal JoinSplit Circuit

> **Panduan Dokumen Audit Independen:**  
> Laporan audit keamanan ini mendokumentasikan investigasi dan pembuktian empiris atas celah kerentanan *Null-Branch Witness Attack* pada sirkuit `JoinSplitCircuit` di `zeltra-core`.  
> Status temuan ini adalah **VERIFIED_CLOSED** setelah implementasi mitigasi *Quadratic Encoding Soundness* dan *Augmented Jacobian Rigidity*.

---

## Metadata Laporan

- **Identifier Dokumen:** `SEC-001`
- **Status Resolusi:** `VERIFIED_CLOSED`
- **Tingkat Keparahan (Severity):** `CRITICAL` (CVSS v3.1: **9.8** / `CVSS:3.1/AV:N/AC:L/PR:N/UI:N/S:U/C:H/I:H/A:H`)
- **Target Domain Arsitektur:**
  - [x] **`ZK_CIRCUIT` (`zeltra-core`)** — Sirkuit R1CS, Aljabar Kurva BLS12-381, JoinSplit Circuit, Jacobian Diagnostics
  - [ ] **`SMART_CONTRACT` (`zeltra-contracts`)**
  - [ ] **`RELAYER_NODE` (`zeltra-node`)**
  - [ ] **`CLIENT_SDK` (`zeltra-sdk`)**
  - [ ] **`CLI_TOOLING` (`zeltra-cli`)**
  - [ ] **`FULL_STACK_E2E`**
- **Tipe Audit / Archetype:**
  - `ZK_SOUNDNESS_BREAK` (Underconstrained dummy input branch & anti-aliasing inverse degree of freedom)
  - `SECURITY_VULNERABILITY` (Pemalsuan bukti Groth16 tanpa kepemilikan note kedua)
- **Cakupan Modul / Target Crates & Components:**
  - `zeltra-core/src/joinsplit_circuit.rs` (Universal JoinSplit 2-in-2-out circuit)
  - `zeltra-core/src/circuit_gadgets.rs` (Companion quadratic constraints & zero-safe inverse gadgets)
- **Target Commit SHA (Vulnerable):** `Pre-DEC-046 Baseline`
- **Remediation Commit SHA (Fixed / Verified):** `Post-DEC-046 & DEC-047 Hardening`
- **Auditor / Tim Penilai:** Internal Cryptography Research Team
- **Tanggal Evaluasi:** 2026-10-20
- **Dasar Ilmiah, Standar & Referensi Primer:**
  - IACR ePrint 2026/2235: *"Practical Null-Branch Witness Attacks on In-the-Head Signatures"* (Liu et al., Sept 2026)
  - IACR ePrint 2026/1852: *"Augmented Jacobian Diagnostics & Markowitz Gaussian Reduction for R1CS"*
  - DEC-046: *"Null-Branch Witness Attack Defense & Quadratic Encoding Soundness"*
  - DEC-047: *"Jacobian Diagnostics and Tangent Space Rigidity Auditing"*

---

## 1. Executive Summary & Audit Scope

### 1.1 Ringkasan Eksekutif
Sirkuit universal `JoinSplitCircuit` (2-in-2-out, DEC-036A) menggunakan topologi aritas tetap untuk menyembunyikan pola transaksi pengguna (*graph metadata privacy*). Ketika pengguna hanya membelanjakan 1 note, slot input kedua dialokasikan sebagai *dummy slot* (`in2_is_dummy = 1`).

Sebelum perbaikan, penonaktifan cabang dummy dilakukan dengan teknik standar industri yaitu mengalikan batasan dengan faktor $(1 - \text{is\_dummy})$:
$$(1 - \text{is\_dummy}) \cdot (\text{computed\_peak}_2 - \text{selected\_peak}_2) = 0$$
$$(1 - \text{is\_dummy}) \cdot (\text{nullifier}_1 - \text{nullifier}_2) \cdot \text{diff\_inv} = (1 - \text{is\_dummy})$$

Ketika $\text{is\_dummy} = 1$, faktor $(1 - \text{is\_dummy}) = 0$, sehingga persamaan tereduksi menjadi $0 \cdot (\dots) = 0$. Hal ini menyebabkan variabel saksi privat `in2_owner`, `in2_rho`, `in2_rand`, `in2_leaf_index`, dan `diff_inv` **kehilangan seluruh kendala aljabar (*floating degrees of freedom*)**. Penyerang dapat menyuntikkan nilai saksi acak (*equivalent witness*) dan memanipulasi bukti verifikasi.

### 1.2 Cakupan Kode yang Diaudit

| Target File & Path | Modul / Sub-Komponen | Fungsi & Tanggung Jawab Keamanan |
|---|---|---|
| [`zeltra-core/src/joinsplit_circuit.rs`](file:///workspaces/Zeltra-Protocol/zeltra-core/src/joinsplit_circuit.rs) | Phase 2 Dummy Slot & Anti-Aliasing | Menjamin isolasi slot dummy input 2 dan keunikan nullifier. |
| [`zeltra-core/src/circuit_gadgets.rs`](file:///workspaces/Zeltra-Protocol/zeltra-core/src/circuit_gadgets.rs) | Companion Gadgets & Zero-Safe Inversion | Penegakan konstanta deterministik pada cabang non-aktif. |

---

## 2. Investigasi Teknis & Mitigasi Defensif

### 2.1 Analisis Aljabar Celah
Sebagaimana dibuktikan dalam literatur IACR ePrint 2026/2235, Groth16 hanya mengevaluasi relasi aljabar $R_C(\mathbf{x}_{\text{pub}}, W) = 0$ dan tidak memahami semantik tingkat tinggi. Pengali nol meruntuhkan matriks batasan menjadi tautologi $0 = 0$, mengekspos ruang singgung bebas $\dim(\ker(M(a))) > 0$.

### 2.2 Desain Solusi Defensif (Quadratic Companion Constraints)
Seluruh variabel saksi pada cabang non-aktif diikat secara deterministik menggunakan batasan pendamping kuadratis (*Quadratic Companion Constraints*):
$$\text{condition} \cdot (\text{variable} - \text{canonical\_constant}) = 0$$

- Saat $\text{is\_dummy} = 1$: `in2_owner`, `in2_rho`, `in2_rand`, `in2_index`, dan `diff_inv` dipaksa bernilai kanonikal (0).
- Saat $\text{is\_dummy} = 0$: Batasan bernilai $0 \cdot (\dots) = 0$ (terpenuhi secara trivial dan tidak menghambat eksekusi riil).

---

## 3. Verifikasi Empiris & Benchmarking

### 3.1 Test Suite & PoC
Pengujian formal dijalankan melalui `zeltra-core/src/formal_tests.rs`:
1. `test_dec046_companion_pinning_rejects_unpinned_dummy`: Percobaan menyuntikkan saksi non-nol pada dummy branch langsung ditolak.
2. `test_dec047_augmented_jacobian_soundness`: Evaluasi matriks augmented Jacobian $M(a) = \begin{pmatrix} J(a) \\ E_F \end{pmatrix}$ membuktikan bahwa dimensi ruang singgung target finansial $r_{T_{\text{financial}}} = 0$ di seluruh 6 keluarga saksi adversarial (*Witness-Locality Sweep*, IACR 2026/1852).

---

## 4. Status Penutupan (Re-Audit Sign-Off)

- **Status Akhir Dokumen:** `VERIFIED_CLOSED`
- **Pernyataan Auditor:** Seluruh derajat kebebasan liar pada `JoinSplitCircuit` telah ditutup deterministik dengan companion constraints dan divalidasi oleh engine Jacobian rank reduction.
