# Sluice: Prove-Phase Bounded-Memory Groth16 via Read-Write Streaming (IACR ePrint 2026/1758)

- **Authors:** Kyeongtae Lee, Jihye Kim, Hyunok Oh
- **Date:** October 2026
- **ePrint ID:** 2026/1758
- **PDF Artifact:** [`jurnal/pdf/2026-1758.pdf`](file:///workspaces/Zeltra-Protocol/jurnal/pdf/2026-1758.pdf)
- **Status di Zeltra:** **ARCHIVED / DEFERRED**
  * Alasan: Zeltra JoinSplit Circuit hanya memiliki 34.571 constraints dengan penggunaan RAM native ~246 MiB (jauh di bawah limit browser 1.5–2.0 GB). Mengorbankan latensi proving dari ~4.97s menjadi ~11–15s demi memangkas RAM bertentangan dengan prinsip kecepatan "Apple Pay / QRIS" Zeltra.
  * Dokumen Rujukan Teknis: [`archive/sbm/DEC-050-streaming-bounded-memory-groth16-prover-and-sluice-sbm-architecture.md`](file:///workspaces/Zeltra-Protocol/archive/sbm/DEC-050-streaming-bounded-memory-groth16-prover-and-sluice-sbm-architecture.md) & [`archive/sbm/TASK-DEC-050.md`](file:///workspaces/Zeltra-Protocol/archive/sbm/TASK-DEC-050.md).

## Problem di Sistem Klasik
Membuktikan sirkuit ZK Groth16 membutuhkan alokasi memori kerja acak $O(N)$ yang besar. Pada sirkuit besar (>8M constraints), proses ini memicu Web Worker crash akibat Out-of-Memory (OOM) pada perangkat dengan RAM terbatas.

## Inovasi Paper
Memperkenalkan *Read-Write Streaming Groth16 Prover* (Sluice) yang memangkas kebutuhan working memory dari $O(N)$ menjadi $O(\log N)$ (atau $O(\sqrt{N})$) melalui:
1. Split-Butterfly-Merge (SBM) untuk NTT radix-2 DIT sekuensial.
2. Twiddle factor on-the-fly tanpa tabel memori.
3. Chunked Pippenger MSM accumulator.
4. Interface Groth16 tetap identik 3 elemen grup dan 3 pairing verification tanpa mengubah verifier on-chain.
