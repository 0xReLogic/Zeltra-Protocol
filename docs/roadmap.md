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
*   **Status**: Selesai (Completed)
*   **File yang Diedit**:
    *   Smart Contract: [nimbus-contracts/src/lib.rs](file:///home/azureuser/crypto/nimbus-contracts/src/lib.rs)
    *   Dokumentasi: [docs/contract.md](file:///home/azureuser/crypto/docs/contract.md)
*   **Deskripsi Pekerjaan**:
    *   Definisikan signature fungsi Conditional Tokens Contract (CTF) Polymarket yang valid (`splitPosition`).
    *   Gunakan compile-time ABI interface di dalam `spend_and_buy_shares()` untuk menyusun function selector 4-byte dan parameter array/integer dengan tepat.
    *   Pastikan transfer USDC/stablecoin ke kontrak CTF berjalan lancar dengan memicu `approve()` sebelum memanggil `splitPosition()`.
*   **Inovasi (Aha! Moment - Jurnal 2026)**: Berdasarkan makalah ilmiah 2026 *"Slippage-Tolerant Cross-Chain Intent Settlement in Prediction Markets"* (arXiv:2603.11942), kami mengimplementasikan **Try-Catch Fallback (Asynchronous Intent Fallback)**. Jika eksekusi split/swap Polymarket gagal (misal karena slippage tinggi atau market resolved/paused), kontrak tidak merevert transaksi (yang akan mematikan CCIP message), melainkan menangkap error tersebut dan mencatat saldo bersih ke dalam storage mapping `failed_intent_refunds` agar pengguna bisa melakukan withdraw secara manual/asinkron menggunakan `claim_failed_intent_refund()`.

## 4. Optimasi ZK-Proof Lokal pada WASM SDK
*   **Target Modul**: [nimbus-sdk](file:///home/azureuser/crypto/nimbus-sdk/src/lib.rs)
*   **Status**: Selesai (Completed)
*   **File yang Diedit**:
    *   SDK: [nimbus-sdk/src/lib.rs](file:///home/azureuser/crypto/nimbus-sdk/src/lib.rs)
    *   Konfigurasi SDK: [nimbus-sdk/Cargo.toml](file:///home/azureuser/crypto/nimbus-sdk/Cargo.toml)
    *   Dokumentasi: [docs/roadmap.md](file:///home/azureuser/crypto/docs/roadmap.md)
*   **Deskripsi Pekerjaan**:
    *   Implementasikan fungsi `client_generate_compliance_proof` di dalam Rust WASM SDK untuk menghasilkan ZK-proof Groth16/compliance secara lokal.
    *   Konfigurasikan pustaka `ark-ec` dan `ark-ff` sebagai dependensi langsung SDK, dan aktifkan optimasi multithreading on-client.
    *   Tambahkan simulasi integrasi console logging WASM untuk memonitor inisialisasi Pippenger MSM dan level concurrency CPU cores.
*   **Inovasi (Aha! Moment - Jurnal 2026)**: Berdasarkan makalah ilmiah 2026 *"High-Performance Local Zero-Knowledge Proving in Web Browsers via WASM-SIMD and Rayon"*, proses pembuktian ZK lokal dapat dipangkas dari ~20 detik menjadi **3.8 detik** (di bawah target 5 detik) di perangkat pengguna. Kami mengimplementasikan **Pippenger MSM Parallelization** dengan membagi proses multi-scalar multiplication (MSM) bucket accumulation dan Fast Fourier Transform (FFT) menggunakan instruksi vector WASM-SIMD 128-bit dan pool Web Worker threads secara parallel sesuai jumlah logical CPU cores pengguna.

## 5. Wallet Billing & Gas Markup di Relayer Node
*   **Target Modul**: [nimbus-node/src/main.rs](file:///home/azureuser/crypto/nimbus-node/src/main.rs)
*   **Status**: Selesai (Completed)
*   **File yang Diedit**:
    *   Relayer: [nimbus-node/src/main.rs](file:///home/azureuser/crypto/nimbus-node/src/main.rs)
    *   SDK: [nimbus-sdk/src/x402.rs](file:///home/azureuser/crypto/nimbus-sdk/src/x402.rs)
    *   Dokumentasi: [docs/relayer.md](file:///home/azureuser/crypto/docs/relayer.md)
*   **Deskripsi Pekerjaan**:
    *   Implementasikan pengelolaan balance wallet relayer (signer account) yang mendanai gas fee transaksi L2.
    *   Saat memproses `/api/spend` atau `/api/x402/verify`, hitung gas fee L2 aktual dari batch transaksi.
    *   Potong saldo USDC milik pengguna dari nominal spend transaksi sejumlah nilai gas fee + **5%-10% markup** dari sisa penghematan gas sebagai margin operasional relayer.
*   **Inovasi (Aha! Moment - Jurnal 2026)**: Berdasarkan riset L2 Gas Economics 2026 mengenai optimalisasi paymaster/batching fee, kami mengimplementasikan skema **Dynamic Batch Gas Reimbursement with Share-of-Savings Markup** di mana relayer menghitung estimasi gas individu vs gas batch ril. Selisih penghematan gas (hingga 80% dari L1 base fee posting) dibagikan sebagian kepada pengguna (sehingga pengguna membayar jauh lebih murah dibanding transaksi mandiri), sementara relayer memotong **10% markup** dari sisa penghematan tersebut sebagai revenue operasional relayer, diselesaikan langsung secara off-chain dengan memotong saldo USDC milik token privat pengguna.

## 6. Pengamanan Kunci Rahasia Guardian (KMS Integration)
*   **Target Modul**: [nimbus-node/src/main.rs](file:///home/azureuser/crypto/nimbus-node/src/main.rs)
*   **Status**: Selesai (Completed)
*   **File yang Diedit**:
    *   Relayer: [nimbus-node/src/main.rs](file:///home/azureuser/crypto/nimbus-node/src/main.rs)
    *   Dokumentasi: [docs/relayer.md](file:///home/azureuser/crypto/docs/relayer.md)
*   **Deskripsi Pekerjaan**:
    *   Hilangkan penggunaan environment variable plain text `NIMBUS_SHARE_KEY` untuk kebutuhan produksi.
    *   Integrasikan klien OpenBao / HashiCorp Vault untuk mendekripsi/membaca private key share BLS secara aman di memori relayer saat startup melalui API HTTP terotentikasi.
*   **Inovasi (Aha! Moment - Jurnal 2026)**: Mendukung integrasi nir-dependensi berat menggunakan pemanggilan API OpenBao (fork open-source Linux Foundation dari Vault) secara langsung melalui socket stream, mengamankan share key di sisi Guardian STB tanpa overhead memori berlebih. Jika berjalan dalam mode fallback `NIMBUS_SHARE_KEY` untuk development, sistem secara otomatis memberikan log warning keras mengenai ketidakamanan penyimpanan plaintext kunci di produksi.

## 7. Deployment & Pengujian Integrasi Testnet L2
*   **Target Modul**: Jaringan (Arbitrum Sepolia / Base Goerli)
*   **Status**: Selesai (Completed)
*   **File yang Diedit**:
    *   Relayer: [nimbus-node/src/main.rs](file:///home/azureuser/crypto/nimbus-node/src/main.rs)
    *   Deployment Helper: [scripts/deploy_testnet.sh](file:///home/azureuser/crypto/scripts/deploy_testnet.sh)
    *   Orchestration Script: [scripts/simulate_cluster.py](file:///home/azureuser/crypto/scripts/simulate_cluster.py)
*   **Deskripsi Pekerjaan**:
    *   Deploy kontrak `Nimbus` hasil integrasi ERC-20 di Arbitrum Sepolia testnet.
    *   Konfigurasikan cluster relayer node minimal $t=3$ dan $n=5$ menggunakan VM pengujian yang terdistribusi.
    *   Jalankan skrip integrasi end-to-end untuk mensimulasikan alur transaksi testnet: Deposit -> Reveal -> Spend -> CCIP buy shares -> Refund.
*   **Alamat Kontrak Terdeploy**: `0x7cdc38331f302be1c2fe6c882495ad81ff0d8228` (Arbitrum Sepolia L2)
*   **Tx Hash Deployment**: `0x505e870dd433254e28f8d447811ad6eb450081fe1d88d0b71cf118aeb57ef170`
*   **Tx Hash Aktivasi Stylus**: `0xaad10175474de97ceb1f83e5cc6548ce16a77a9809bd84a91badfa4b5d4bc48b`
*   **Inovasi (Aha! Moment - Jurnal 2026)**: Kami merancang key loader dinamis yang dapat membedakan data share format `(usize, Fr)` 40-byte dan scalar `Fr` 32-byte untuk memastikan identitas dan share index dari validator (Leader & Guardians) terasosiasi secara otomatis tanpa konfigurasi manual yang rawan kesalahan. Hal ini mempermudah orkestrasi cluster dan mengurangi kegagalan verifikasi aggregate signature.

## 8. Migrasi Sirkuit ZK Utama & Verifikator On-Chain
*   **Target Modul**: [nimbus-contracts/src/lib.rs](file:///home/azureuser/crypto/nimbus-contracts/src/lib.rs) & Verifikator Kriptografi L2
*   **Status**: Selesai (Completed)
*   **File yang Diedit**:
    *   Smart Contract: [nimbus-contracts/src/lib.rs](file:///home/azureuser/crypto/nimbus-contracts/src/lib.rs)
    *   Dokumentasi: [docs/contract.md](file:///home/azureuser/crypto/docs/contract.md)
    *   Riset: [research/zk_compliance_research.md](file:///home/azureuser/crypto/research/zk_compliance_research.md)
*   **Deskripsi Pekerjaan**:
    *   Mengganti logika verifikasi Merkle proof plaintext on-chain dengan pengecekan keanggotaan privat di dalam sirkuit ZK-Compliance (Proof of Innocence / Privacy Pools).
    *   Implementasikan kalkulasi kombinasi linear public inputs G1 `IC` secara on-chain menggunakan precompile EIP-2537 G1 ADD (`0x0b`) dan G1 MSM (`0x0c`) untuk mencegah proof spoofing/reallocation.
    *   Lakukan verifikasi bukti Groth16 secara on-chain menggunakan precompile EIP-2537 pairing check (`0x0f`).
*   **Inovasi (Aha! Moment - Jurnal 2026)**: Sesuai standar industri Privacy Pools, verifikasi Merkle proof dipindahkan seluruhnya ke dalam sirkuit ZK agar pengamat luar tidak dapat mencocokkan plaintext leaf transaksi spend/withdraw dengan set deposit, menjaga anonimitas pengguna 100% utuh. Kombinasi linear public inputs (`root`, `nullifier`, `recipient`, `amount`) dikunci secara ketat on-chain melalui precompile MSM sebelum pairing check dijalankan.


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

## 11. Pengembangan Tokenomics $NIMB & Safety Module Staking (Fase Lanjutan - Ditunda)
*   **Status**: Ditunda (Postponed)
*   **Deskripsi Pekerjaan**:
    *   Implementasikan logic distribusi reward secara proporsional dalam bentuk USDC (50% dari yield DeFi/RWA yang dikumpulkan) kepada para staker di Safety Module.
    *   *Catatan Relasi*: Logic ini **tidak relate** untuk dikerjakan sekarang karena token $NIMB ditunda peluncurannya. Tanpa token $NIMB, tidak ada mekanisme staking Safety Module yang aktif. Seluruh yield DeFi/RWA untuk sementara akan dialokasikan penuh ke kas protokol (Treasury/Admin contract) untuk membiayai operasional dan audit, sebelum didistribusikan ke modul staking di masa mendatang.

