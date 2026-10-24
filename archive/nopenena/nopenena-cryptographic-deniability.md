# Nopenena Untraceable Payments: Defeating Graph Analysis with Small Decoy Sets (IACR ePrint 2024/903)

- **Authors:** Jayamine Alupotha, Mathieu Gestin, Christian Cachin
- **Date:** 2024
- **ePrint ID:** 2024/903
- **PDF Artifact:** [`jurnal/pdf/2024-903.pdf`](../../jurnal/pdf/2024-903.pdf)
- **Status di Zeltra:** **FULLY ADOPTED & RATIFIED (DEC-051)**
- **Dokumen Terkait:**
  - Spesifikasi Arsitektur: [`research/dec/DEC-051-nopenena-cryptographic-deniability-small-decoy-sets-dummy-notes.md`](../../research/dec/DEC-051-nopenena-cryptographic-deniability-small-decoy-sets-dummy-notes.md)
  - Task Tracker: [`archive/tasks/taskdec51.md`](../tasks/taskdec51.md)
  - Benchmark Suite: [`zeltra-core/src/bin/bench_deniability.rs`](../../zeltra-core/src/bin/bench_deniability.rs)
  - Test Suite: [`zeltra-core/tests/deniability_tests.rs`](../../zeltra-core/tests/deniability_tests.rs)

---

## 1. Problem di Sistem Klasik
Pada sistem pembayaran privat berbasis UTXO dengan decoy set kecil (seperti Monero dan Zether), transaksi kembalian (*change note*) pada topologi 1-in-2-out rentan terhadap analisis graf transaksi heuristik (*payment-graph analysis* / DM-decomposition). Pengamat on-chain dapat mengidentifikasi mana note pembayaran dan mana note kembalian, menyebabkan degradasi anonymity set efektif sebesar **40% hingga 59%**.

## 2. Inovasi Paper
Nopenena merumuskan protokol pembayaran privat confidential berbasis decoy set kecil ($N \in [2, 24]$) yang mencapai sifat **Cryptographic Deniability** tanpa trusted setup dan tanpa verifiable shuffling raksasa (80% lebih kecil dari QuisQuis):
1. **Verifiable Rerandomizable Assets**: Setiap aset dapat di-rerandomisasi dengan blinding baru $r'$ tanpa mengetahui private key $k$.
2. **Anonymous Forced Openings**: Membuktikan secara zero-knowledge bahwa komitmen Pedersen $\{C_l\}_{l=0}^n$ membuka *hanya dan seluruh* akun yang mengalami perubahan saldo, sementara akun decoy terbukti tidak berubah nilai tanpa mewajibkan pemilik decoy membuat range proof.

## 3. Implementasi Penuh di Zeltra Protocol (DEC-051)
Zeltra mengadopsi prinsip Nopenena ke dalam arsitektur Pure ZK-UTXO di Arbitrum Stylus:
1. **`DeniabilityPermutationGadget` di `zeltra-core`:** Gerbang rahasia permutasi $\pi \in \{0, 1\}$ yang menukar kawat Output 1 dan Output 2 secara probabilistik ($\text{Pr} = 0.5$).
2. **Deterministic Decoy Companion Constraint (DEC-046):** Mengunci kawat dummy auxiliary secara deterministik: $\text{is\_decoy} \cdot v_{\text{aux}} == 0$ (Zero Degrees of Freedom).
3. **Decoy Dummy Note di `zeltra-sdk`:** Injeksi dummy note bernilai $0$ dengan komitmen Poseidon-W5 seragam dan dual 96B encrypted memo pada exact payment.
4. **Hasil Empiris Benchmark:** Overhead hanya **+7 R1CS constraints**, latensi sintesis **11.85 µs**, Poseidon latency difference **0.686%**, dan Shannon Entropy delta **0.000104 bits** (uniform random murni).
