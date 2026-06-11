# Session Summary: HT-04 & HT-05 On-Chain Verification

Date: 2026-06-11
Conversation ID: `e191545a-2d3b-494a-bdb2-bbb8121943ca`

## 1. Pekerjaan yang Selesai

### A. Pengujian Spend BLS On-Chain (HT-04)
- Menjalankan script pengujian [ht04_spend_negative_tests.py](file:///home/azureuser/crypto/scripts/ht04_spend_negative_tests.py) di Arbitrum Sepolia.
- Hasil: **13/13 Test Cases Lolos (100% Pass)**.
  - Memverifikasi penolakan (revert) pada signature rusak, issuer salah, recipient salah, amount salah, input length salah, dan replay nullifier.
  - Menyimpan laporan resmi di [ht04_spend_negative_tests.json](file:///home/azureuser/crypto/test-reports/ht04_spend_negative_tests.json).
- Memperbarui checklist di [todo.md](file:///home/azureuser/crypto/todo.md).
- Memperbarui dokumentasi keputusan [DEC-002-bls-spend-verification-boundaries.md](file:///home/azureuser/crypto/research/decisions/DEC-002-bls-spend-verification-boundaries.md) dengan menyematkan referensi laporan tes HT-04.

### B. Pengujian Refund & Timeout On-Chain (HT-05)
- Membuat dokumen keputusan baru [DEC-013-refund-and-timeout-verification.md](file:///home/azureuser/crypto/research/decisions/DEC-013-refund-and-timeout-verification.md) untuk memetakan spesifikasi dan invariant keamanan refund.
- Menulis script pengujian on-chain [ht05_refund_tests.py](file:///home/azureuser/crypto/scripts/ht05_refund_tests.py).
- Hasil eksekusi di Arbitrum Sepolia: **4/4 Negative Tests Lolos (100% Pass)**:
  - **NT-01**: Refund sebelum timelock 24 jam (revert) -> ✅ PASS
  - **NT-02**: Refund session ID tidak terdaftar (revert) -> ✅ PASS
  - **NT-03**: Refund dipanggil oleh non-client (revert) -> ✅ PASS
  - **NT-04**: Refund setelah sesi di-reveal/resolved (revert) -> ✅ PASS
  - Note: Kasus positif (PT-01..05) dilewati di public testnet karena tidak bisa memajukan waktu block (butuh menunggu timelock 24 jam real-time). Namun, semua kasus positif ini sudah terverifikasi 100% lewat unit test lokal.
- Menyimpan laporan di [ht05_refund_tests.json](file:///home/azureuser/crypto/test-reports/ht05_refund_tests.json).
- Memperbarui checklist HT-05 di [todo.md](file:///home/azureuser/crypto/todo.md).

### C. Git Commit & Push
- Melakukan commit dan push ke repo utama:
  `4f35f58 (origin/main) feat: add HT-05 on-chain refund and timeout tests & DEC-013 design doc`

---

## 2. Tanya Jawab Teknis & Konsep Produk
Menjawab pertanyaan krusial investor/user terkait arsitektur Nimbus:
- **ZK-Compliance vs Tornado Cash**: Menjelaskan bahwa Nimbus memakai model hybrid (Fast settlement via BLS Blind Signature + Compliance via Groth16 Privacy Pools).
- **Pengelolaan Blacklist**: Menjelaskan transisi dari Owner/Admin di MVP ke DAO/KAWAL governance + Multi-Source Compliance Oracle di Phase 2.
- **Double-Spending & Gas**: Menjelaskan pencegahan double-spend 100% on-chain lewat nullifiers database di contract, serta efisiensi gas fee menggunakan **Adaptive Micro-Batching** (menggabungkan hingga 8 transaksi off-chain ke satu call `batch_spend`).
- **MEV Front-running**: Menjelaskan bahwa payload diikat secara kriptografis ke signature BLS (termasuk `recipient` dan `amount`), sehingga transaksi tidak bisa dibajak di mempool.
- **Produk & Token Utility**: Menjelaskan portofolio produk (AI Agent Spending Wallet, POS, Yield Vaults) serta utilitas token `NIMB` (Staking, Slashing, Fee Discount, DAO governance).

## 3. Sinkronisasi Model Bisnis, Pembersihan Todo, & Flow 5 (Withdraw / Unshield)

### A. Penambahan Spesifikasi Flow 5 (Withdraw / Unshield)
- Menambahkan **Flow 5 - Withdraw / Unshield** ke dalam [todo.md](file:///home/azureuser/crypto/todo.md#L413) di bawah bagian *Core Business Flow* dengan rincian model biaya dinamis:
  - **Standard Fee**: Flat 0.10% unshield fee.
  - **Withdrawal Queue (Lock-up Period)**: Penarikan >5% TVL harus masuk antrean 24-48 jam.
  - **Emergency Dynamic Fee (Penalty)**: Dynamic fee tambahan untuk penarikan instan saat kas likuid berada di batas minimal (15%) guna menutup slippage/gas penarikan paksa dari Aave/Ondo.

### B. Pembersihan Redundansi todo.md & Sinkronisasi Kode
- Menghapus bagian **Peta Produk di Atas Core** dan **Urutan Produk** dari [todo.md](file:///home/azureuser/crypto/todo.md) karena sudah tercakup di [docs/bisnis.md](file:///home/azureuser/crypto/docs/bisnis.md) dan [docs/roadmap.md](file:///home/azureuser/crypto/docs/roadmap.md). Ini menghemat sekitar 120 baris berkas todo untuk optimalisasi konteks AI.
- Menghapus **Catatan** warning lama tentang `generate_bls_test_data.rs` karena generator data tes, format RFC 9380 hash-to-curve, negasi alpha, dan limit minimum 5 USDC kini sudah terimplementasi secara valid di code.
- Memverifikasi implementasi fee deposit (0.10%) dan spend (0.15%) di level smart contract (`deposit.rs` & `spend.rs`) serta menganalisis batasan `MAX_BATCH_SIZE: usize = 8` (kaitannya dengan EVM Block Gas Limit dan batas heap memori WASM/Stylus).

