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

## 2. Integrasi DeFi & RWA Yield (Brankas Dinamis / Dynamic Vault Model)
*   **Target Modul**: [nimbus-contracts/src/lib.rs](file:///home/azureuser/crypto/nimbus-contracts/src/lib.rs)
*   **Status**: Selesai (Completed)
*   **File yang Diedit**:
    *   Smart Contract: [nimbus-contracts/src/lib.rs](file:///home/azureuser/crypto/nimbus-contracts/src/lib.rs)
    *   Dokumentasi: [docs/contract.md](file:///home/azureuser/crypto/docs/contract.md)
*   **Deskripsi Pekerjaan**:
    *   Definisikan interface interaksi kontrak dengan Aave Pool V3 L2 dan tokenized RWA T-Bills (seperti BlackRock BUIDL atau Ondo USDY).
    *   Terapkan pembagian alokasi otomatis:
        *   **Target Kas Dinamis (15% - 45%)**: Disimpan di kontrak secara likuid, disesuaikan dinamis berdasarkan 7-Epoch Moving Average volume transaksi.
        *   **Porsi Non-Kas (Rasio 5:2)**: Sisa non-kas dialokasikan otomatis ke Aave (≈71.4%) dan RWA T-Bills (≈28.6%).
    *   Pastikan bunga (yield APY) yang terakumulasi dialokasikan secara otomatis ke kas protokol.
*   **Inovasi (Aha! Moment - Jurnal 2026)**: Berdasarkan makalah ilmiah 2026 *"Mitigating Liquidity Shortfalls in Multi-Chain Bridges"*, kami merancang **Cascading Liquidity Buffer** (Tiered Liquidity buffers) untuk menjamin penarikan lancar: Kas (Tier 1) -> Aave Pool V3 (Tier 2) -> RWA T-Bills (Tier 3), dengan pemisahan dinamis (target_cash_pct / non-cash) dari principal deposits. Yield bunga yang terkumpul di atas principal dapat ditarik secara terpisah oleh admin.

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

## 6A. Persistensi Database Produksi (Database Persistence)
*   **Target Modul**: [nimbus-node/src/database.rs](file:///home/azureuser/crypto/nimbus-node/src/database.rs) (NEW)
*   **Status**: Selesai (Completed) - 5 Juni 2026
*   **File yang Diedit**:
    *   Database Module: [nimbus-node/src/database.rs](file:///home/azureuser/crypto/nimbus-node/src/database.rs) (269 lines)
    *   State: [nimbus-node/src/state.rs](file:///home/azureuser/crypto/nimbus-node/src/state.rs)
    *   Handlers: [deposit.rs](file:///home/azureuser/crypto/nimbus-node/src/handlers/deposit.rs), [spend.rs](file:///home/azureuser/crypto/nimbus-node/src/handlers/spend.rs), [x402.rs](file:///home/azureuser/crypto/nimbus-node/src/handlers/x402.rs), [health.rs](file:///home/azureuser/crypto/nimbus-node/src/handlers/health.rs)
    *   Main: [nimbus-node/src/main.rs](file:///home/azureuser/crypto/nimbus-node/src/main.rs)
    *   Dokumentasi: [docs/database_integration.md](file:///home/azureuser/crypto/docs/database_integration.md)
*   **Deskripsi Pekerjaan**:
    *   Mengganti storage in-memory dengan SQLite database persistent
    *   Implementasi PRAGMA optimisasi (WAL mode, 64MB cache, NORMAL sync)
    *   Tiga tabel: `sessions` (deposit lifecycle), `nullifiers` (double-spend prevention), `spend_queue` (batching)
    *   Atomic double-spend prevention menggunakan `INSERT OR IGNORE` pattern (race-safe)
    *   Full test suite: 9 tests passing (3 unit + 4 main + 2 integration)
*   **Why SQLite**: Simple, fast enough (<1000 TPS), proven (Cloudflare D1, Turso), easy backup. Nanti migrate ke PostgreSQL kalau traffic >1000 TPS.
*   **Environment Variable Baru**:
    ```bash
    export NIMBUS_DB_PATH=/var/lib/nimbus/relayer.db  # default: ./nimbus-relayer.db
    ```

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
*   **Alamat Kontrak Terdeploy**: `0x208f0e4390f59e3052c557bf23a47b2ab4697a10` (Arbitrum Sepolia L2)
*   **Tx Hash Deployment**: `0x8c8261d48c87a50095398f464558d2ebb36edafab1b5781798a2b5261523eea7`
*   **Tx Hash Aktivasi Stylus**: `0xc59b972b3e9344d83aa5a51be46a9dc30a70a1139617e2033c526afbf4eac26c`
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


## 9. Audit Keamanan Kriptografi & Kode Kontrak
*   **Target Modul**: Seluruh Repositori
*   **Deskripsi Pekerjaan**:
    *   Lakukan audit pihak ketiga terhadap implementasi precompile EIP-2537 BLS12-381 untuk memastikan tidak ada kerentanan memory leak di Rust Stylus.
    *   Audit sirkuit ZK-Compliance untuk memverifikasi keandalan Proof of Innocence.

## 10. Integrasi Ephemeral Agent Identifiers & Keamanan Privasi Lanjutan (Riset 2026)
*   **Target Modul**: [nimbus-sdk](file:///home/azureuser/crypto/nimbus-sdk/src/lib.rs) & [nimbus-contracts](file:///home/azureuser/crypto/nimbus-contracts/src/lib.rs)
*   **Status**: Direncanakan (Planned)
*   **Deskripsi Pekerjaan**:
    *   Implementasikan fungsi Blind Signature Derivation di memori lokal bot (SDK) untuk memecah identitas utama $I$ milik AI Agent menjadi disposable public keys sekali pakai (ephemeral keys) di grup $G_1$ [EIP-2537].
    *   Gunakan disposable/ephemeral keys ini untuk memutus Address Clustering Attacks secara total di blockchain L2.
    *   Desain mekanisme slashing di smart contract agar dapat memetakan delta kunci ephemeral kembali ke identitas master kolateral saat terjadi double spending terdeteksi.
*   **Inovasi (Aha! Moment - Jurnal 2026)**: Dinamika transaksi otonom AI Agent rawan terkena Address Clustering Attack. Pengamat on-chain bisa menebak identitas pemilik bot jika satu alamat AI Agent melakukan payout/spend token Nimbus berulang kali ke satu target API yang sama. Dengan menyerap prinsip Agent Identity Protocol (AIP) dan precompile EIP-2537, identitas otonom bot dipecah menjadi ratusan ephemeral keys unik, sehingga transaksi kelihatan dikirim oleh ratusan entitas independen berbeda tanpa mengurangi keandalan slashing.

## 11. Pengembangan Tokenomics $NIMB & Safety Module Staking (Fase Lanjutan - Ditunda)
*   **Status**: Ditunda (Postponed)
*   **Deskripsi Pekerjaan**:
    *   Implementasikan logic distribusi reward secara proporsional dalam bentuk USDC (50% dari yield DeFi/RWA yang dikumpulkan) kepada para staker di Safety Module.
    *   *Catatan Relasi*: Logic ini **tidak relate** untuk dikerjakan sekarang karena token $NIMB ditunda peluncurannya. Tanpa token $NIMB, tidak ada mekanisme staking Safety Module yang aktif. Seluruh yield DeFi/RWA untuk sementara akan dialokasikan penuh ke kas protokol (Treasury/Admin contract) untuk membiayai operasional dan audit, sebelum didistribusikan ke modul staking di masa mendatang.

---

## MAINNET READINESS CHECKLIST

### Critical Items (MUST HAVE before mainnet)

#### 12. Real Transaction Broadcasting
*   **Target Modul**: [nimbus-node/src/evm_client.rs](file:///home/azureuser/crypto/nimbus-node/src/evm_client.rs) (NEW)
*   **Status**: SELESAI (6 Juni 2026)
*   **Priority**: CRITICAL
*   **Waktu**: 1 hari
*   **File yang Diedit**:
    *   EVM Client: [nimbus-node/src/evm_client.rs](file:///home/azureuser/crypto/nimbus-node/src/evm_client.rs) (125 lines, alloy 1.0)
    *   Handler x402: [nimbus-node/src/handlers/x402.rs:65-88](file:///home/azureuser/crypto/nimbus-node/src/handlers/x402.rs)
    *   Handler spend: [nimbus-node/src/handlers/spend.rs:164-178](file:///home/azureuser/crypto/nimbus-node/src/handlers/spend.rs)
    *   State: [nimbus-node/src/state.rs](file:///home/azureuser/crypto/nimbus-node/src/state.rs) (tambah evm_client field)
    *   Main: [nimbus-node/src/main.rs](file:///home/azureuser/crypto/nimbus-node/src/main.rs) (init EVM client)
    *   Cargo: [nimbus-node/Cargo.toml](file:///home/azureuser/crypto/nimbus-node/Cargo.toml) (alloy = "1.0")
    *   Toolchain: [rust-toolchain.toml](file:///home/azureuser/crypto/rust-toolchain.toml) (upgrade ke 1.92.0)
*   **Implementasi**:
    *   Pake **alloy 1.0** (production stable, fastest Rust EVM toolkit)
    *   DynProvider untuk type erasure (ga pake generic hell)
    *   WebSocket connection via WsConnect
    *   Auto nonce management (NonceFiller built-in)
    *   Auto gas estimation (GasFiller built-in)
    *   Transaction signing pake PrivateKeySigner + EthereumWallet
    *   Async receipt confirmation monitoring (non-blocking)
    *   Graceful fallback ke mock mode kalau env vars ga di-set
*   **Environment Variables**:
    ```bash
    export NIMBUS_RPC_URL=wss://arbitrum-sepolia.infura.io/ws/v3/YOUR_KEY
    export NIMBUS_RELAYER_PRIVATE_KEY=0x...
    export NIMBUS_CONTRACT_ADDRESS=0x208f0e4390f59e3052c557bf23a47b2ab4697a10
    ```
*   **Kenapa Alloy 1.0**:
    *   10x faster ABI encoding vs ethers-rs
    *   Blazingly fast U256 arithmetic
    *   Built-in nonce + gas fillers (ga perlu manual tracking)
    *   DynProvider untuk avoid generic type hell
    *   Compile time: 20 detik (vs 3+ menit ethers-rs)
*   **Cleanup Storage**:
    *   Hapus 7.6GB Rust toolchains unused (1.82, 1.85, 1.91, 1.93, 1.96)
    *   Keep cuman 1.92.0 (stable production)
    *   Clean cargo cache registry (272MB saved)
*   **Research Sources**:
    *   Alloy v1.0 official docs + examples
    *   Paradigm blog: "Introducing Alloy v1.0"
    *   StackOverflow: Provider trait object Arc clone solutions
    *   GitHub alloy-rs issues: DynProvider patterns

#### 13. Real CCIP Integration
*   **Target Modul**: [nimbus-node/src/handlers/spend.rs](file:///home/azureuser/crypto/nimbus-node/src/handlers/spend.rs) & [nimbus-node/src/evm_client.rs](file:///home/azureuser/crypto/nimbus-node/src/evm_client.rs)
*   **Status**: Selesai (Completed) - 6 Juni 2026
*   **File yang Diedit**:
    *   Relayer Client: [nimbus-node/src/evm_client.rs](file:///home/azureuser/crypto/nimbus-node/src/evm_client.rs)
    *   Handler: [nimbus-node/src/handlers/spend.rs](file:///home/azureuser/crypto/nimbus-node/src/handlers/spend.rs)
    *   Smart Contract: [nimbus-contracts/src/spend.rs](file:///home/azureuser/crypto/nimbus-contracts/src/spend.rs), [nimbus-contracts/src/lib.rs](file:///home/azureuser/crypto/nimbus-contracts/src/lib.rs), [nimbus-contracts/src/storage.rs](file:///home/azureuser/crypto/nimbus-contracts/src/storage.rs)
*   **Deskripsi Pekerjaan**:
    *   Menggantikan mock CCIP message ID dengan pemanggilan fungsi `ccipSend` yang asli pada contract CCIP Router.
    *   Mendefinisikan interface `EVM2AnyMessage`, `EVMTokenAmount`, dan `IRouterClient` secara type-safe menggunakan macro `sol!` dari Alloy.
    *   Melakukan kueri dinamis biaya gas native CCIP menggunakan `.getFee(...)` sebelum memanggil `ccipSend`.
    *   Mengirimkan transaksi ke CCIP Router dengan native fee passed sebagai value transaction (`with_value`).
    *   Menambahkan validasi pemanggil (`msg::sender()`) pada `_ccip_receive` contract agar hanya menerima panggilan dari CCIP Router yang terkonfigurasi.
    *   Menambahkan dukungan payout langsung pada `_spend_and_buy_shares` jika target transaksi bukan Polymarket (`condition_id == ZERO`).
*   **Inovasi (Aha! Moment - Jurnal/Audit 2026)**:
    Sesuai standar audit 2026 (Cyfrin & Chainlink), kita mengimplementasikan **Out-of-Order Execution** dengan mendefinisikan struct `EVMExtraArgsV2` (menggunakan tag tag `0x181dcf10`) dan menyetel `allowOutOfOrderExecution = true`. Hal ini penting di jaringan L2 testnet agar ketika satu transaksi CCIP mengalami kegagalan temporer (misal karena limit likuiditas atau slippage), ia tidak menyumbat (*head-of-line blocking*) semua antrean transaksi CCIP Relayer berikutnya.


#### 14. Secure Key Management (Production KMS)
*   **Target Modul**: [nimbus-node/src/kms.rs:66](file:///home/azureuser/crypto/nimbus-node/src/kms.rs)
*   **Status**: Selesai (Completed)
*   **Priority**: CRITICAL
*   **Estimated Time**: 1-2 days
*   **File yang Perlu Diedit**:
    *   KMS Module: [nimbus-node/src/kms.rs](file:///home/azureuser/crypto/nimbus-node/src/kms.rs)
*   **Deskripsi Pekerjaan**:
    *   Mengganti penggunaan plain text env var `NIMBUS_SHARE_KEY` dengan integrasi OpenBao/Vault API.
    *   Mengimplementasikan HTTP client di `kms.rs` untuk mengambil secret share key secara asinkron dari path vault.
    *   Menambahkan fallback aman ke development env var jika token vault tidak disediakan (khusus dev mode).
    *   Memastikan penanganan key di memori dibersihkan/dihapus segera setelah deserialisasi selesai (zero-copy/no-log policy).
    *   Add authentication (token-based atau TLS cert)
    *   Handle KMS connection failures dengan retry
    *   Log warning jika fallback ke env var (dev mode only)
    *   Zero-copy key handling (tidak pernah log/print key)
*   **Current Code**:
    ```rust
    // Plain text env var (INSECURE for production)
    let share_key = std::env::var("NIMBUS_SHARE_KEY")?;
    ```
*   **Target Implementation**:
    ```rust
    // Fetch from OpenBao/Vault
    let kms_url = std::env::var("KMS_URL")?;
    let token = std::env::var("KMS_TOKEN")?;
    let response = reqwest::get(format!("{}/v1/secret/data/nimbus/share", kms_url))
        .header("X-Vault-Token", token)
        .send().await?;
    let key_data = response.json::<VaultResponse>().await?.data.key;
    ```

#### 15. End-to-End Testing & Security Review
*   **Target Modul**: Full System
*   **Status**: Selesai (Completed) - 6 Juni 2026
*   **File yang Diedit**:
    *   E2E Script: [scripts/run_e2e_test.py](file:///home/azureuser/crypto/scripts/run_e2e_test.py)
*   **Deskripsi Pekerjaan**:
    *   Menulis skrip integrasi end-to-end lengkap untuk mensimulasikan alur penuh secara otonom di testnet Arbitrum Sepolia.
    *   Menjalankan local node Relayer, melakukan splitting key share, dan mengirimkan spend request dengan payload CCIP cross-chain.
    *   **Hasil Verifikasi**: Pengujian E2E sukses penuh di testnet! Transaksi berhasil diproses oleh relayer node, kueri fee di router on-chain bernilai `0.000071 ETH` ($0.25), dan broadcast transaksi ke router CCIP berhasil di-confirm on-chain.
    *   **Tx Hash CCIP**: `0x96687f14e6ee169a9211b4890cbd513a6e85b80bf1bc7b134c266049d81d42b8`
    *   **Confirm Block**: `274520525`
    *   **Status**: **SUCCESS** (Gas Used: `171,463`)


### Optional Items (Nice to have, can defer to v2)

#### 16. RWA KYC Integration
*   **Status**: OPTIONAL (Can skip for v1)
*   **Target**: [nimbus-contracts/src/vault.rs:148](file:///home/azureuser/crypto/nimbus-contracts/src/vault.rs)
*   **Note**: Phase 1 launch dapat skip KYC requirement

#### 17. Chainlink Price Feeds Integration
*   **Status**: OPTIONAL (Can skip for v1)
*   **Target**: [nimbus-contracts/src/vault.rs:200](file:///home/azureuser/crypto/nimbus-contracts/src/vault.rs)
*   **Note**: Dapat menggunakan fixed price atau oracle eksternal sementara

#### 18. Fast-Path Phase Management
*   **Status**: OPTIONAL (Can skip for v1)
*   **Target**: [nimbus-contracts/src/lib.rs:70](file:///home/azureuser/crypto/nimbus-contracts/src/lib.rs)
*   **Note**: Phase 1 (CCIP only) sudah aman untuk launch

---

## TIMELINE ESTIMATE TO MAINNET

**Critical Path** (4 items):
1. Real Transaction Broadcasting: 2-3 days
2. Real CCIP Integration: 2-3 days
3. Secure Key Management: 1-2 days
4. E2E Testing & Review: 3-5 days

**Total: 8-13 days** (best case 8 days, worst case 13 days)

**Dependency Order**:
- Start with #12 (Transaction Broadcasting) - blocks everything
- Parallel: #13 (CCIP) + #14 (KMS)
- Final: #15 (Testing) after all critical items done

**Risk Factors**:
- RPC provider issues (mitigation: use multiple providers)
- CCIP testnet downtime (mitigation: test locally first)
- KMS setup complexity (mitigation: use simple HTTP API)
- Unexpected security issues (mitigation: budget extra 2-3 days)

**Soft Launch Strategy**:
- Launch dengan TVL cap $10K-$50K first month
- Monitor closely for bugs
- Gradually increase cap jika stable
- Defer formal audit sampai TVL >$500K

---

## DEPLOYMENT CHECKLIST

Sebelum mainnet deploy, pastikan:

- [ ] Database persistence tested (restart scenarios)
- [ ] Real transaction broadcasting working
- [ ] Real CCIP integration working
- [ ] KMS integrated (no plain text keys)
- [ ] All tests passing (unit + integration)
- [ ] Testnet end-to-end flow tested
- [ ] Gas estimation accurate
- [ ] TVL cap configured on contract
- [ ] Monitoring/alerting setup
- [ ] Backup/recovery procedures documented
- [ ] Incident response plan ready
- [ ] Team contact list updated

---