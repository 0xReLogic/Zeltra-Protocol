# Accelerated Endomorphism & Regular Scalar Multiplication on BLS12-381 (IACR ePrint 2024/1906)

- **Authors:** Guangwu Xu, Wei Yu, Ke Han, Pengfei Lu
- **Date:** September 2026 (Updated)
- **ePrint ID:** 2024/1906
- **PDF Artifact:** [`jurnal/pdf/2024-1906.pdf`](../../jurnal/pdf/2024-1906.pdf)
- **Status di Zeltra:** **PARTIALLY ADOPTED / SURGICAL ADOPTION (DEC-050)**

## Ringkasan Keputusan Zeltra (Prinsip ATMI)

1. **Yang Diadopsi (Emas Aljabar - Proposition 3.1):**
   - Formula evaluasi cepat koordinat Jacobian proyektif untuk endomorfisme $\tau P = P - \omega P$ ($3S + 3M$, **37,02% lebih cepat**).
   - Formula point tripling cepat $3P = (1 - \omega^2)(\tau P)$ ($4S + 6M$, **16,71% lebih cepat** dari standard double-and-add).
   - Diimplementasikan di [`zeltra-core/src/curve_opt.rs`](../../zeltra-core/src/curve_opt.rs) dan disahkan lewat [`DEC-050`](../../research/dec/DEC-050-fast-jacobian-tripling-and-eisenstein-endomorphism-bls12-381.md).

2. **Yang Ditolak / Diarsipkan (Window tau-NAF Penuh):**
   - Aljabar gelanggang bilangan bulat Eisenstein $\mathbb{Z}[\omega]$ (~1.200 baris kode).
   - **Alasan Penolakan:** Prover Groth16 Zeltra (34.571 gates, ~5,4s) menggunakan Multi-Scalar Multiplication (MSM), bukan single scalar mult $[k]P$. Penghematan single scalar mult dari NAF hanyalah ~36 mikrodetik (0,0006% dari total transaksi), sehingga menulis ribuan baris kode NAF adalah *over-engineering* yang tidak sebanding dengan beban perawatannya.
