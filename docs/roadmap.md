# Roadmap Kesiapan Produksi (Path to Live) Nimbus Protocol

Dokumen ini memetakan seluruh tugas pengembangan, integrasi, dan pengujian yang harus diselesaikan untuk membawa Nimbus Protocol dari status prototipe simulasi saat ini hingga siap dideploy secara aman di mainnet L2.

---

## 1. Integrasi Token ERC-20, Penanganan Fee, & Fast-Path Likuiditas
*   **Target Modul**: [nimbus-contracts/src/lib.rs](file:///home/azureuser/crypto/nimbus-contracts/src/lib.rs)
*   **Status**: Selesai (Completed)
*   **File yang Diedit**:
    *   Smart Contract: [nimbus-contracts/src/lib.rs](file:///home/azureuser/crypto/nimbus-contracts/src/lib.rs)
    *   Dokumentasi: [docs/contract.md](file:///home/azureuser/crypto/docs/contract.md)
*   **Deskripsi Pekerjaan**:
    *   Definisikan interface ERC-20 standar di dalam Stylus smart contract.
    *   Ubah fungsi `deposit()` agar melakukan `transferFrom` USDC dari wallet pengguna ke escrow kontrak.
    *   Implementasikan pemotongan biaya deposit/minting sebesar **0.1%** secara on-chain.
    *   Ubah fungsi `reveal_mask_key()` dan `spend()` agar melakukan `transfer` USDC rill ke alamat tujuan.
    *   Implementasikan pemotongan biaya penarikan/redemption sebesar **0.15%** saat token privat dibelanjakan.
    *   Implementasikan fitur **Fast-Path Liquidity Premium (0.05% - 0.10%)** secara bertahap sesuai 3 fase rilis:
        1.  *Fase 1*: Dinonaktifkan (hanya CCIP lambat untuk menghilangkan modal awal).
        2.  *Fase 2*: Diaktifkan menggunakan modal hasil yield kas Treasury internal secara mandiri.
        3.  *Fase 3*: Membuka pool publik yang dilindungi batas maksimum dinamis (*Dynamic Pool Cap*).
*   **Inovasi (Aha! Moment - Jurnal 2026)**: Berdasarkan makalah ilmiah 2026 *"Exploiting Liquidity Exhaustion Attacks in Intent-Based Cross-Chain Bridges"* (arXiv:2602.17805), kami mengimplementasikan **Dynamic Pool Cap** dan **Congestion-Based Pricing** (linear/dynamic premium scaling berdasarkan tingkat utilitas pool LP) pada Fase 3 untuk memitigasi serangan pengurasan likuiditas solver/LP.

## 2. Integrasi DeFi & RWA Yield (Rasio Brankas Bertingkat 30/50/20)
*   **Target Modul**: [nimbus-contracts/src/lib.rs](file:///home/azureuser/crypto/nimbus-contracts/src/lib.rs)
*   **Status**: Selesai (Completed)
*   **File yang Diedit**:
    *   Smart Contract: [nimbus-contracts/src/lib.rs](file:///home/azureuser/crypto/nimbus-contracts/src/lib.rs)
    *   Dokumentasi: [docs/contract.md](file:///home/azureuser/crypto/docs/contract.md)
*   **Deskripsi Pekerjaan**:
    *   Definisikan interface interaksi kontrak dengan Aave Pool V3 L2 dan tokenized RWA T-Bills (seperti BlackRock BUIDL atau Ondo USDY).
    *   Terapkan pembagian alokasi otomatis:
        *   **30%** tetap disimpan di dalam brankas Nimbus secara liquid untuk penarikan instan.
        *   **50%** di-supply ke Aave untuk menghasilkan APY ~3%-4%.
        *   **20%** di-supply ke Ondo USDY / BlackRock BUIDL untuk APY ~5% dengan keamanan tingkat tinggi.
    *   Pastikan bunga (yield APY) yang terakumulasi dialokasikan secara otomatis ke kas protokol.
*   **Inovasi (Aha! Moment - Jurnal 2026)**: Berdasarkan makalah ilmiah 2026 *"Mitigating Liquidity Shortfalls in Multi-Chain Bridges"*, kami merancang **Cascading Liquidity Buffer** (Tiered Liquidity buffers) untuk menjamin penarikan lancar: Kas (Tier 1) -> Aave Pool V3 (Tier 2) -> RWA T-Bills (Tier 3), dengan pemisahan otomatis 30/50/20 dari principal deposits. Yield bunga yang terkumpul di atas principal dapat ditarik secara terpisah oleh admin.

## 3. Perbaikan Format Panggilan Eksternal Polymarket CTF
*   **Target Modul**: [nimbus-contracts/src/lib.rs](file:///home/azureuser/crypto/nimbus-contracts/src/lib.rs)
*   **Deskripsi Pekerjaan**:
    *   Definisikan signature fungsi Conditional Tokens Contract (CTF) Polymarket yang valid (seperti `splitPosition`).
    *   Gunakan `abi::encode` yang tepat untuk menyusun function selector 4-byte dan argumennya di dalam payload `spend_and_buy_shares()`.
    *   Pastikan transfer USDC/stablecoin ke kontrak CTF berjalan lancar sebelum memicu fungsi beli shares.

## 4. Optimasi ZK-Proof Lokal pada WASM SDK
*   **Target Modul**: [nimbus-sdk](file:///home/azureuser/crypto/nimbus-sdk/src/lib.rs)
*   **Deskripsi Pekerjaan**:
    *   Gunakan kerangka ZK Plonky3 atau Halo2 dengan kompilasi WASM-SIMD untuk optimasi multithreading di sisi klien.
    *   Pastikan proses pembuatan proof berjalan lokal di perangkat pengguna dalam waktu <5 detik tanpa memerlukan server delegated proving berbayar eksternal.

## 5. Wallet Billing & Gas Markup di Relayer Node
*   **Target Modul**: [nimbus-node/src/main.rs](file:///home/azureuser/crypto/nimbus-node/src/main.rs)
*   **Deskripsi Pekerjaan**:
    *   Implementasikan pengelolaan balance wallet relayer (signer account) yang mendanai gas fee transaksi L2.
    *   Saat memproses `/api/spend` atau `/api/x402/verify`, hitung gas fee L2 aktual dari batch transaksi.
    *   Potong saldo USDC milik pengguna dari nominal spend transaksi sejumlah nilai gas fee + **5%-10% markup** dari sisa penghematan gas sebagai margin operasional relayer.

## 6. Pengamanan Kunci Rahasia Guardian (KMS Integration)
*   **Target Modul**: [nimbus-node/src/main.rs](file:///home/azureuser/crypto/nimbus-node/src/main.rs)
*   **Deskripsi Pekerjaan**:
    *   Hilangkan penggunaan environment variable plain text `NIMBUS_SHARE_KEY`.
    *   Integrasikan pustaka klien Google Cloud KMS, AWS KMS, atau HashiCorp Vault untuk mendekripsi private key share BLS secara aman di memori relayer saat startup.

## 7. Deployment & Pengujian Integrasi Testnet L2
*   **Target Modul**: Jaringan (Arbitrum Sepolia / Base Goerli)
*   **Deskripsi Pekerjaan**:
    *   Deploy kontrak `Nimbus` hasil integrasi ERC-20 di Arbitrum Sepolia testnet.
    *   Konfigurasikan cluster relayer node minimal $t=3$ dan $n=5$ menggunakan VM pengujian yang terdistribusi.
    *   Jalankan skrip integrasi end-to-end untuk mensimulasikan alur transaksi testnet: Deposit -> Reveal -> Spend -> CCIP buy shares -> Refund.

## 8. Pengembangan Tokenomics $NIMB & Safety Module Staking (Fase Lanjutan)
*   **Target Modul**: Kontrak Baru (`nimbus-token` & `nimbus-staking`)
*   **Deskripsi Pekerjaan**:
    *   Buat kontrak ERC-20 untuk tata kelola token $NIMB.
    *   Buat kontrak **Safety Module Staking** tempat staker dapat mengunci $NIMB untuk mem-backstop risiko slashing merchant offline maupun exploit teknis.
    *   Implementasikan logic distribusi reward secara proporsional dalam bentuk USDC (40% dari yield DeFi/RWA yang dikumpulkan) kepada para staker di Safety Module.
    *   Implementasikan modul buyback & burn otomatis (40%) dari kas protokol di DEX L2 serta pengiriman 20% biaya ke Treasury.

## 9. Pemeliharaan Modul Offline POS & Slashing (IP B2B Showcase - Cadangan/Lisensi)
*   **Target Modul**: [nimbus-contracts/src/lib.rs](file:///home/azureuser/crypto/nimbus-contracts/src/lib.rs) & [nimbus-node/src/main.rs](file:///home/azureuser/crypto/nimbus-node/src/main.rs)
*   **Deskripsi Pekerjaan**:
    *   Pertahankan fungsionalitas `slash_double_spender` dan endpoint `/api/pos/sync-claims` sebagai inovasi cadangan/modul IP untuk lisensi B2B.
    *   Pastikan pengujian simulasi offline tetap terjangkau dan dapat divalidasi oleh calon mitra ritel besar.

## 10. Audit Keamanan Kriptografi & Kode Kontrak
*   **Target Modul**: Seluruh Repositori
*   **Deskripsi Pekerjaan**:
    *   Lakukan audit pihak ketiga terhadap implementasi precompile EIP-2537 BLS12-381 untuk memastikan tidak ada kerentanan memory leak di Rust Stylus.
    *   Audit sirkuit ZK-Compliance untuk memverifikasi keandalan Proof of Innocence.
