# Checklist Eksekusi DEC-052: Argo Groth16 Invalidity Encoding & Strict Curve Point Sanitation

Status: **COMPLETED / FULLY VERIFIED**  
Dokumen Referensi:
- DEC File: WAJIB DI BACA FULL [`research/dec/DEC-052-argo-groth16-invalidity-encoding-and-curve-point-sanitation.md`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-052-argo-groth16-invalidity-encoding-and-curve-point-sanitation.md)
- Paper: [`jurnal/pdf/2026-2165.pdf`](file:///workspaces/Zeltra-Protocol/jurnal/pdf/2026-2165.pdf) (IACR ePrint 2026/2165)
- Verification Test: [`scripts/test_dec052_argo_verification.py`](file:///workspaces/Zeltra-Protocol/scripts/test_dec052_argo_verification.py)

---

## 1. Audit & Fondasi Kriptografi (`zeltra-core`)

- [x] **1.1. Modul Sanitasi Titik Kurva EIP-2537 (`zeltra-core/src/evm.rs`)**
  - [x] Implementasikan fungsi verifikasi deterministik titik $\mathbb{G}_1$ affine:
    - [x] Coordinate canonical bound: $x, y < q$
    - [x] On-curve validation: $y^2 \equiv x^3 + 4 \pmod q$ (untuk BLS12-381)
    - [x] Point at infinity check: reject jika $(0, 0)$ atau bit infinity aktif
    - [x] Strict subgroup check: $[r]P = \mathcal{O}$ via fast endomorphism $\psi(P)$
  - [x] Implementasikan fungsi verifikasi deterministik titik $\mathbb{G}_2$ affine:
    - [x] Coordinate canonical bounds dalam $\mathbb{F}_{q^2}$
    - [x] On-curve validation: $y^2 \equiv x^3 + 4(1 + i) \pmod q$
    - [x] Non-infinity check
    - [x] $\mathbb{G}_2$ subgroup check via Frobenius endomorphism $\psi(Q)$
  - [x] Tambahkan unit test di `zeltra-core` untuk invalid points, twist points, dan points of small order (orders 3, 7, 13).

- [x] **1.2. Aljabar Endomorfisme Eisenstein (`zeltra-core/src/curve_opt.rs`)**
  - [x] Konstanta akar kubik primitif $\omega$ pada field skalar $\mathbb{F}_r$
  - [x] Definisi himpunan endomorfisme $S = \{0, \pm 1, \pm \omega, \pm \omega^2\}$ ($|S| = 7$)
  - [x] Evaluasi konstan-waktu untuk $\phi \mapsto E(\pi)^\phi$ tanpa rekursi eksponensiasi lambat.

---

## 2. Stylus Smart Contract Ingress Gating (`zeltra-contracts`)

- [x] **2.1. Sanitasi Calldata Proof di Host Verifier (`zeltra-contracts/src/groth16_note_verifier.rs` & `groth16_joinsplit_verifier.rs`)**
  - [x] Terapkan pre-filter sebelum calldata dikirim ke EIP-2537 precompile `0x0c` (MSM) dan `0x0f` (Pairing).
  - [x] Revert dengan error eksplisit `CurvePointNotOnCurve`, `PointAtInfinityNotAllowed`, `SubgroupCheckFailed`, atau `NonCanonicalFieldElement`.
  - [x] Pastikan gas overhead seminimal mungkin (< 2.500 gas di Stylus WASM).

---

## 3. Relayer Sequencer Defense (`zeltra-node`)

- [x] **3.1. Preflight Ingress Filtering (`zeltra-node/src/handlers/spend.rs` & `zeltra-node/src/ingress.rs`)**
  - [x] Tolak transaksi malformed / adversarial proof sebelum masuk ke mempool dan sebelum relayer membayar gas transaksi Arbitrum L2.
  - [x] Logging telemetri jika terdeteksi percobaan *invalid curve attack* atau *subgroup injection*.

---

## 4. Client SDK & Optimistic Fraud Defense (`zeltra-sdk`)

- [x] **4.1. Fast Client-side Sanitizer (`zeltra-sdk/src/note_wallet/`)**
  - [x] Validasi otomatis bukti Groth16 sebelum dikirim ke relayer (fail-fast di browser / node).
- [x] **4.2. Argo Randomized Encoding Fraud Slashing Codec (DEC-052 / Section 8.1)**
  - [x] Proof invalidity encoding generator untuk verifikasi optimistic / cross-chain challenge.
  - [x] O(1) secret key extraction decoder untuk instan fraud slashing.

---

## 5. End-to-End Testing & Formal Verification

- [x] Jalankan regression test suite: `python3 scripts/test_dec052_argo_verification.py`
- [x] Jalankan formal tests zeltra-core: `cargo test -p zeltra-core`
- [x] Jalankan stylus contract check: `cargo test -p zeltra-contracts`
- [x] Verifikasi invariant zero-degrees-of-freedom (DEC-046) & Jacobian soundness (DEC-047) tetap lolos 100%.
