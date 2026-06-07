# Roadmap Kesiapan Produksi (Path to Live) Nimbus Protocol

Dokumen ini memetakan seluruh tugas pengembangan, integrasi, dan pengujian yang harus diselesaikan untuk membawa Nimbus Protocol dari status prototipe simulasi saat ini hingga siap dideploy secara aman di mainnet L2.

> Status "selesai" pada catatan historis di bawah bukan pengganti security gate.
> Blocker, hard test, dan acceptance criteria yang berlaku tetap mengacu pada
> [`todo.md`](../todo.md).

---

## Roadmap Berikutnya: Phase 1-4

Bagian ini memuat arah pengembangan setelah blocker core payment ditangani.
Target angka seperti gas, latency, deviasi oracle, dan ukuran payload wajib
dibuktikan dengan benchmark atau hard test sebelum menjadi acceptance criteria.

### Phase 1: Eksekusi Core Protocol dan Baseline Deployment

- [ ] Implementasikan dukungan penuh EIP-2537 BLS12-381 pada seluruh target L2.
- [ ] Verifikasi activation status, alamat precompile, encoding, gas schedule,
  dan error behavior langsung pada setiap target network.
- [ ] Jalankan known-answer vector valid dan invalid untuk G1, G2, MSM, dan
  pairing pada environment Stylus/testnet.
- [ ] Optimalkan verifier Groth16 untuk mengejar target maksimal 100.000 gas.
- [ ] Benchmark biaya Groth16 berdasarkan jumlah public input dan pasangan
  pairing sebelum menetapkan target 100.000 gas sebagai acceptance criteria.
- [ ] Tambahkan gas snapshot dan regression test untuk verifier Groth16.
- [ ] Jadikan Arbitrum One `eip155:42161` sebagai default deployment profile
  yang diuji.
- [ ] Pisahkan konfigurasi mainnet, testnet, local development, dan CI.
- [ ] Tambahkan validasi chain ID agar deployment atau transaksi tidak terkirim
  ke network yang salah.
- [ ] Siapkan deployment script dan struktur konfigurasi untuk ekspansi ke Base
  L2.
- [ ] Jangan aktifkan profile Base sebelum compatibility precompile, Stylus/EVM
  runtime, token, dan bridge path diverifikasi.
- [ ] Audit ulang nullifier database pada handler `x402.rs`.
- [ ] Pastikan reservation nullifier bersifat atomic pada database.
- [ ] Tambahkan unique constraint dan transaction boundary untuk nullifier.
- [ ] Uji concurrent spend pada beban paralel dengan jeda sub-milidetik hingga
  multi-milidetik.
- [ ] Pastikan request gagal, timeout, restart, dan RPC error tidak menciptakan
  double settlement atau nullifier yang salah status.
- [ ] Tetapkan batas ukuran serialization Base64 JSON untuk
  `NimbusPaymentPayload`.
- [ ] Targetkan payload akhir berada pada rentang 1,2-1,5 KB hanya jika seluruh
  field security wajib tetap termuat.
- [ ] Tolak payload oversized dan malformed sebelum decoding kriptografi.
- [ ] Tambahkan benchmark latency verifikasi matematika off-chain.
- [ ] Targetkan latency verifikasi di bawah 5 ms pada hardware reference yang
  terdokumentasi.
- [ ] Catat p50, p95, dan p99; jangan hanya menggunakan hasil satu eksekusi.

### Phase 2: Infrastruktur Terdesentralisasi dan Oracle Network

- [ ] Ganti authorization `check_owner` pada `register_clean_root` dengan
  governance terdesentralisasi.
- [ ] Definisikan proposal, quorum, timelock, emergency pause, dan proses
  pencabutan clean root.
- [ ] Rancang arsitektur Multi-Source Compliance Oracle dari tiga node
  independen.
- [ ] Definisikan format feed, signature, timestamp, sequence number, expiry,
  dan replay protection setiap oracle.
- [ ] Tentukan mekanisme agregasi dan fallback jika satu oracle offline.
- [ ] Implementasikan rejection trigger jika deviasi data oracle melebihi 5%.
- [ ] Definisikan secara formal metrik deviasi, unit, precision, rounding, dan
  perilaku saat tepat di batas 5%.
- [ ] Tambahkan circuit breaker agar data yang menyimpang membekukan action
  terkait tanpa membekukan seluruh protocol.
- [ ] Rancang `SlashingContract` untuk menyita atau membakar stake NIMB atas
  bukti double-signing yang objektif.
- [ ] Definisikan format cryptographic proof yang dapat diverifikasi on-chain.
- [ ] Pastikan bukti yang sama tidak dapat digunakan untuk slash dua kali.
- [ ] Tambahkan batas maksimum slash, withdrawal delay, dan emergency recovery.
- [ ] Rancang hybrid slashing untuk pelanggaran subjektif atau data abu-abu.
- [ ] Implementasikan freeze state dan challenge period sebelum asset disita.
- [ ] Definisikan siapa yang dapat mengajukan challenge dan biaya anti-spam.
- [ ] Hubungkan voting DAO ke challenge period untuk keputusan akhir seizure.
- [ ] Definisikan quorum, voting period, delegation, conflict of interest, dan
  protection terhadap governance capture untuk konsensus token KAWAL.
- [ ] Bangun paket Decentralized Relayer Node untuk operator pihak ketiga.
- [ ] Tambahkan registration, stake, health check, reward, penalty, rotation,
  dan graceful exit untuk relayer.
- [ ] Dokumentasikan instalasi reproducible, minimum hardware, networking,
  observability, dan incident response.

### Phase 3: Advanced Privacy Circuit dan RWA Routing

- [ ] Finalisasi desain Blind Signature Derivation berbasis EIP-2537 untuk
  menghasilkan disposable G1 point yang tidak dapat ditautkan.
- [ ] Tulis security argument untuk unlinkability, unforgeability, replay
  resistance, dan domain separation.
- [ ] Gunakan test vector lintas `nimbus-core`, SDK, contract, dan testnet.
- [ ] Audit kemungkinan rogue-key, small-subgroup, infinity-point, dan
  malformed-encoding attack.
- [ ] Rancang ZK-Ephemeral Circuit Breaker agar Master Identity dapat mencabut
  child key aktif tanpa membuka identitas atau hubungan child lain.
- [ ] Hindari mekanisme self-destruct literal jika dapat menyebabkan kehilangan
  dana; gunakan revoke/freeze state dengan recovery yang dapat diaudit.
- [ ] Definisikan proof ownership, anti-replay, expiry, dan emergency threshold.
- [ ] Implementasikan router evakuasi asset berbasis ZK dari Arbitrum ke Base.
- [ ] Definisikan trigger, authorization proof, destination binding, fee cap,
  timeout, retry, dan replay protection.
- [ ] Pastikan source lock/burn dan destination release tidak menghasilkan
  duplicate balance.
- [ ] Bangun modul Groth16 ZK-KYC pada `verification.rs` untuk validasi RWA.
- [ ] Pisahkan circuit KYC/RWA dari circuit payment agar perubahan compliance
  tidak merusak payment core.
- [ ] Gunakan proving/verifying key production dan public-input schema yang
  versioned.
- [ ] Integrasikan Ethereum Attestation Service untuk credential issuer dan
  source-of-funds.
- [ ] Definisikan schema UID, trusted attester, revocation, expiry, chain, dan
  data minimization.
- [ ] Pastikan raw PII tidak disimpan pada public chain atau relayer.
- [ ] Tegakkan separation of concerns: relayer hanya memproses payment payload
  dan tidak membaca atau mengelola transport data L7.
- [ ] Dokumentasikan trust boundary antara client, reverse proxy, relayer,
  oracle, guardian, contract, dan destination protocol.

### Phase 4: Reverse Proxy, Tools, dan Integrasi Ekosistem

- [ ] Bangun Nimbus API Guard sebagai reverse proxy plug-and-play.
- [ ] Sediakan deployment profile untuk NGINX dan Cloudflare.
- [ ] Petakan request tanpa pembayaran menjadi response HTTP
  `402 Payment Required`.
- [ ] Definisikan format `PAYMENT-REQUIRED`, pricing policy, expiry, resource
  binding, dan retry behavior.
- [ ] Implementasikan validasi server-side untuk field `PAYMENT-SIGNATURE`.
- [ ] Validasi version, network, scheme, amount, resource, expiry, nullifier,
  issuer key, dan cryptographic proof sebelum request diteruskan.
- [ ] Terapkan size limit, timeout, rate limit, idempotency, dan abuse
  protection pada proxy.
- [ ] Bangun Nimbus Stealth SDK untuk Python.
- [ ] Tambahkan adapter untuk LangChain, CrewAI, dan OpenAI Swarm.
- [ ] Pastikan SDK tidak menyimpan secret, blinding factor, atau credential
  plaintext di log.
- [ ] Tambahkan typed error, retry policy, test vector, dan example integration.
- [ ] Bangun Nimbus DevTools web untuk membuat, menguji, dan men-debug raw
  `NimbusPaymentPayload`.
- [ ] Tambahkan decoder, validator, size inspector, network selector, dan
  simulasi failure tanpa mengekspos secret.
- [ ] Bangun Viewing Key Portal untuk enterprise.
- [ ] Terapkan role-based access, audit log, key rotation, session timeout, dan
  least-privilege decryption.
- [ ] Pastikan viewing key tidak pernah dikirim ke frontend atau disimpan dalam
  browser storage.
- [ ] Tambahkan export/report yang meminimalkan metadata dan dapat diaudit.

---

## 1. Integrasi Token ERC-20, Penanganan Fee, & Fast-Path Likuiditas
*   **Target Modul**: [nimbus-contracts/src/lib.rs](file:///home/azureuser/crypto/nimbus-contracts/src/lib.rs)
*   **Status**: Sebagian diimplementasikan, belum lulus hard test
*   **File yang Diedit**:
    *   Smart Contract: [nimbus-contracts/src/lib.rs](file:///home/azureuser/crypto/nimbus-contracts/src/lib.rs)
    *   Dokumentasi: [docs/contract.md](file:///home/azureuser/crypto/docs/contract.md)
*   **Deskripsi Pekerjaan**:
    *   Definisikan interface ERC-20 standar di dalam Stylus smart contract.
    *   Ubah fungsi `deposit()` agar melakukan `transferFrom` USDC dari wallet pengguna ke escrow kontrak.
    *   Implementasikan pemotongan biaya deposit/minting sebesar **0.1%** secara on-chain.
    *   `reveal_mask_key()` hanya menyelesaikan issuance dan tidak mentransfer
        collateral. Transfer collateral hanya boleh terjadi melalui refund atau
        spend.
    *   Implementasikan pemotongan biaya penarikan/redemption sebesar **0.15%** saat token privat dibelanjakan.
    *   Implementasikan fitur **Fast-Path Liquidity Premium (0.05% - 0.10%)** secara bertahap sesuai 3 fase rilis:
        1.  *Fase 1*: Dinonaktifkan (hanya CCIP lambat untuk menghilangkan modal awal).
        2.  *Fase 2*: Diaktifkan menggunakan modal hasil yield kas Treasury internal secara mandiri.
        3.  *Fase 3*: Membuka pool publik yang dilindungi batas maksimum dinamis (*Dynamic Pool Cap*).
*   **Batas verifikasi**: Perhitungan Dynamic Pool Cap dan Congestion-Based
    Pricing tersedia di kode, tetapi accounting utilization dan liquidity
    settlement belum divalidasi end-to-end dengan pool nyata.

## 2. Integrasi DeFi & RWA Yield (Brankas Dinamis / Dynamic Vault Model)
*   **Target Modul**: [nimbus-contracts/src/lib.rs](file:///home/azureuser/crypto/nimbus-contracts/src/lib.rs)
*   **Status**: Prototype, belum terverifikasi dengan Aave/RWA nyata
*   **File yang Diedit**:
    *   Smart Contract: [nimbus-contracts/src/lib.rs](file:///home/azureuser/crypto/nimbus-contracts/src/lib.rs)
    *   Dokumentasi: [docs/contract.md](file:///home/azureuser/crypto/docs/contract.md)
*   **Deskripsi Pekerjaan**:
    *   Definisikan interface interaksi kontrak dengan Aave Pool V3 L2 dan tokenized RWA T-Bills (seperti BlackRock BUIDL atau Ondo USDY).
    *   Terapkan pembagian alokasi otomatis:
        *   **Target Kas Dinamis (15% - 45%)**: Disimpan di kontrak secara likuid, disesuaikan dinamis berdasarkan 7-Epoch Moving Average volume transaksi.
        *   **Porsi Non-Kas (Rasio 5:2)**: Sisa non-kas dialokasikan otomatis ke Aave (≈71.4%) dan RWA T-Bills (≈28.6%).
    *   Pastikan bunga (yield APY) yang terakumulasi dialokasikan secara otomatis ke kas protokol.
*   **Batas verifikasi**: Cascading Liquidity Buffer tersedia sebagai desain
    dan implementasi awal. Decimal, asset conversion, slippage, withdrawal
    delay, insolvency, serta perilaku Aave/RWA belum diuji pada protocol nyata.

## 3. Perbaikan Format Panggilan Eksternal Polymarket CTF
*   **Target Modul**: [nimbus-contracts/src/lib.rs](file:///home/azureuser/crypto/nimbus-contracts/src/lib.rs)
*   **Status**: ABI dan fallback tersedia, integrasi nyata belum terverifikasi
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
*   **Status**: Prototype; benchmark browser reproducible belum tersedia
*   **File yang Diedit**:
    *   SDK: [nimbus-sdk/src/lib.rs](file:///home/azureuser/crypto/nimbus-sdk/src/lib.rs)
    *   Konfigurasi SDK: [nimbus-sdk/Cargo.toml](file:///home/azureuser/crypto/nimbus-sdk/Cargo.toml)
    *   Dokumentasi: [docs/roadmap.md](file:///home/azureuser/crypto/docs/roadmap.md)
*   **Deskripsi Pekerjaan**:
    *   Implementasikan fungsi `client_generate_compliance_proof` di dalam Rust WASM SDK untuk menghasilkan ZK-proof Groth16/compliance secara lokal.
    *   Konfigurasikan pustaka `ark-ec` dan `ark-ff` sebagai dependensi langsung SDK, dan aktifkan optimasi multithreading on-client.
    *   Tambahkan simulasi integrasi console logging WASM untuk memonitor inisialisasi Pippenger MSM dan level concurrency CPU cores.
*   **Target performa**: Latency 3,8 detik belum dianggap tercapai sampai ada
    benchmark browser yang reproducible, hardware reference, konfigurasi WASM,
    serta hasil p50/p95/p99. Klaim SIMD, Rayon, dan Web Worker harus dibuktikan
    dari build artifact dan profiling runtime.

## 5. Wallet Billing & Gas Markup di Relayer Node
*   **Target Modul**: [nimbus-node/src/main.rs](file:///home/azureuser/crypto/nimbus-node/src/main.rs)
*   **Status**: Implementasi awal, accounting dan settlement belum diaudit
*   **File yang Diedit**:
    *   Relayer: [nimbus-node/src/main.rs](file:///home/azureuser/crypto/nimbus-node/src/main.rs)
    *   SDK: [nimbus-sdk/src/x402.rs](file:///home/azureuser/crypto/nimbus-sdk/src/x402.rs)
    *   Dokumentasi: [docs/relayer.md](file:///home/azureuser/crypto/docs/relayer.md)
*   **Deskripsi Pekerjaan**:
    *   Implementasikan pengelolaan balance wallet relayer (signer account) yang mendanai gas fee transaksi L2.
    *   Saat memproses `/api/spend` atau `/api/x402/verify`, hitung gas fee L2 aktual dari batch transaksi.
    *   Potong saldo USDC milik pengguna dari nominal spend transaksi sejumlah nilai gas fee + **5%-10% markup** dari sisa penghematan gas sebagai margin operasional relayer.
*   **Batas verifikasi**: Perhitungan gas dan markup belum boleh dianggap
    settlement finansial production sampai receipt, actual gas cost, rounding,
    retry, dan reconciliation diuji end-to-end.

## 6. Pengamanan Kunci Rahasia Guardian (KMS Integration)
*   **Target Modul**: [nimbus-node/src/main.rs](file:///home/azureuser/crypto/nimbus-node/src/main.rs)
*   **Status**: Integrasi Vault tersedia; lifecycle key production belum selesai
*   **File yang Diedit**:
    *   Relayer: [nimbus-node/src/main.rs](file:///home/azureuser/crypto/nimbus-node/src/main.rs)
    *   Dokumentasi: [docs/relayer.md](file:///home/azureuser/crypto/docs/relayer.md)
*   **Deskripsi Pekerjaan**:
    *   Hilangkan penggunaan environment variable plain text `NIMBUS_SHARE_KEY` untuk kebutuhan produksi.
    *   Integrasikan klien OpenBao / HashiCorp Vault untuk mendekripsi/membaca private key share BLS secara aman di memori relayer saat startup melalui API HTTP terotentikasi.
*   **Batas verifikasi**: Fallback `NIMBUS_SHARE_KEY` masih tersedia untuk
    development. Production masih membutuhkan authentication policy, TLS/mTLS,
    rotation, revocation, audit log, fail-closed mode, dan hard test multi-node.

## 6A. Persistensi Database Produksi (Database Persistence)
*   **Target Modul**: [nimbus-node/src/database.rs](file:///home/azureuser/crypto/nimbus-node/src/database.rs) (NEW)
*   **Status**: Persistent queue DEC-005 selesai; hard recovery masih terbuka
*   **File yang Diedit**:
    *   Database Module: [nimbus-node/src/database.rs](file:///home/azureuser/crypto/nimbus-node/src/database.rs) (269 lines)
    *   State: [nimbus-node/src/state.rs](file:///home/azureuser/crypto/nimbus-node/src/state.rs)
    *   Handlers: [deposit.rs](file:///home/azureuser/crypto/nimbus-node/src/handlers/deposit.rs), [spend.rs](file:///home/azureuser/crypto/nimbus-node/src/handlers/spend.rs), [x402.rs](file:///home/azureuser/crypto/nimbus-node/src/handlers/x402.rs), [health.rs](file:///home/azureuser/crypto/nimbus-node/src/handlers/health.rs)
    *   Main: [nimbus-node/src/main.rs](file:///home/azureuser/crypto/nimbus-node/src/main.rs)
    *   Dokumentasi: [docs/database.md](file:///home/azureuser/crypto/docs/database.md)
*   **Deskripsi Pekerjaan**:
    *   Mengganti storage in-memory dengan SQLite database persistent
    *   Implementasi PRAGMA optimisasi (WAL mode, 64MB cache, NORMAL sync)
    *   Empat tabel: `sessions`, `nullifiers`, `spend_queue`, dan
        `idempotency_cache`.
    *   `spend_queue` menjadi source of truth dengan status, lease, retry,
        tx hash, block number, dan error terakhir.
    *   Nullifier confirmed hanya ditulis setelah receipt sukses.
    *   Test persistence, duplicate reservation, retry, dan concurrent worker
        claim sudah lulus.
*   **Batas kapasitas**: Klaim throughput belum ditetapkan sampai benchmark
    concurrent writer, WAL checkpoint, disk failure, backup, dan restore
    selesai.
*   **Environment Variable Baru**:
    ```bash
    export NIMBUS_DB_PATH=/var/lib/nimbus/relayer.db  # default: ./nimbus-relayer.db
    ```

## 7. Deployment & Pengujian Integrasi Testnet L2
*   **Target Modul**: Jaringan (Arbitrum Sepolia / Base Goerli)
*   **Status**: Deployment historis selesai; hard test terbaru belum selesai
*   **File yang Diedit**:
    *   Relayer: [nimbus-node/src/main.rs](file:///home/azureuser/crypto/nimbus-node/src/main.rs)
    *   Deployment Helper: [scripts/deploy_testnet.sh](file:///home/azureuser/crypto/scripts/deploy_testnet.sh)
    *   Orchestration Script: [scripts/simulate_cluster.py](file:///home/azureuser/crypto/scripts/simulate_cluster.py)
*   **Deskripsi Pekerjaan**:
    *   Deploy kontrak `Nimbus` hasil integrasi ERC-20 di Arbitrum Sepolia testnet.
    *   Konfigurasikan cluster relayer node minimal $t=3$ dan $n=5$ menggunakan VM pengujian yang terdistribusi.
    *   Skrip integrasi pernah dijalankan, tetapi belum membuktikan seluruh
        lifecycle Deposit -> Reveal -> Spend -> destination CCIP -> Refund
        menggunakan contract terbaru tanpa mock.
*   **Alamat Kontrak Terdeploy**: `0x208f0e4390f59e3052c557bf23a47b2ab4697a10` (Arbitrum Sepolia L2)
*   **Tx Hash Deployment**: `0x8c8261d48c87a50095398f464558d2ebb36edafab1b5781798a2b5261523eea7`
*   **Tx Hash Aktivasi Stylus**: `0xc59b972b3e9344d83aa5a51be46a9dc30a70a1139617e2033c526afbf4eac26c`
*   **Inovasi (Aha! Moment - Jurnal 2026)**: Kami merancang key loader dinamis yang dapat membedakan data share format `(usize, Fr)` 40-byte dan scalar `Fr` 32-byte untuk memastikan identitas dan share index dari validator (Leader & Guardians) terasosiasi secara otomatis tanpa konfigurasi manual yang rawan kesalahan. Hal ini mempermudah orkestrasi cluster dan mengurangi kegagalan verifikasi aggregate signature.

## 8. Migrasi Sirkuit ZK Utama & Verifikator On-Chain
*   **Target Modul**: [nimbus-contracts/src/lib.rs](file:///home/azureuser/crypto/nimbus-contracts/src/lib.rs) & Verifikator Kriptografi L2
*   **Status**: Prototype; verifying key production belum tersedia
*   **File yang Diedit**:
    *   Smart Contract: [nimbus-contracts/src/lib.rs](file:///home/azureuser/crypto/nimbus-contracts/src/lib.rs)
    *   Dokumentasi: [docs/contract.md](file:///home/azureuser/crypto/docs/contract.md)
    *   Riset: [research/zk_compliance_research.md](file:///home/azureuser/crypto/research/zk_compliance_research.md)
*   **Deskripsi Pekerjaan**:
    *   Mengganti logika verifikasi Merkle proof plaintext on-chain dengan pengecekan keanggotaan privat di dalam sirkuit ZK-Compliance (Proof of Innocence / Privacy Pools).
    *   Implementasikan kalkulasi kombinasi linear public inputs G1 `IC` secara on-chain menggunakan precompile EIP-2537 G1 ADD (`0x0b`) dan G1 MSM (`0x0c`) untuk mencegah proof spoofing/reallocation.
    *   Lakukan verifikasi bukti Groth16 secara on-chain menggunakan precompile EIP-2537 pairing check (`0x0f`).
*   **Batas verifikasi**: Contract masih menggunakan verifying key development
    berbasis scaled generator. Modul ini belum memberikan jaminan compliance
    atau privasi sampai circuit, trusted setup, proving key, verifying key, dan
    public-input ordering production dibekukan serta diaudit.


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
    *   Gunakan disposable/ephemeral keys untuk mengurangi risiko Address
        Clustering Attacks di blockchain L2.
    *   Desain mekanisme slashing di smart contract agar dapat memetakan delta kunci ephemeral kembali ke identitas master kolateral saat terjadi double spending terdeteksi.
*   **Batas privasi**: Ephemeral key tidak menjamin unlinkability. Korelasi
    nominal, waktu, recipient, funding source, gas payer, dan withdrawal pattern
    tetap harus dianalisis.

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
*   **Status**: Persistent settlement dasar selesai; nonce/finality belum selesai
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
    *   Menggunakan Alloy 1.x untuk interaksi EVM
    *   DynProvider untuk type erasure (ga pake generic hell)
    *   WebSocket connection via WsConnect
    *   Provider menangani submission transaksi; persistent nonce management
        tetap belum selesai
    *   Gas price dan gas limit dikonfigurasi pada client
    *   Transaction signing pake PrivateKeySigner + EthereumWallet
    *   Worker menunggu receipt dan menyimpan hasil ke persistent queue.
    *   Runtime worker tidak membuat mock tx hash ketika EVM client unavailable;
        item menjadi `retryable`.
*   **Environment Variables**:
    ```bash
    export NIMBUS_RPC_URL=wss://arbitrum-sepolia.infura.io/ws/v3/YOUR_KEY
    export NIMBUS_RELAYER_PRIVATE_KEY=0x...
    export NIMBUS_CONTRACT_ADDRESS=0x208f0e4390f59e3052c557bf23a47b2ab4697a10
    ```
*   **Batas verifikasi**:
    *   Broadcast transaction sudah tersedia.
    *   Receipt parsing, durable tx state, retry/backoff, dan lease recovery
        tersedia melalui DEC-005.
    *   Confirmation depth, reorg, persistent nonce, replacement transaction,
        dan sender+nonce reconciliation masih harus diselesaikan.
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
*   **Status**: Source transaction tersedia; destination execution belum terbukti
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
*   **Batas verifikasi**:
    *   Source transaction hash bukan CCIP message ID.
    *   Event message ID, source/destination sender binding, destination receipt,
        payout, dan failure recovery belum diverifikasi end-to-end.


#### 14. Secure Key Management (Production KMS)
*   **Target Modul**: [nimbus-node/src/kms.rs:66](file:///home/azureuser/crypto/nimbus-node/src/kms.rs)
*   **Status**: Sebagian; belum memenuhi lifecycle key production
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
*   **Development fallback**:
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
*   **Status**: Belum selesai
*   **File yang Diedit**:
    *   E2E Script: [scripts/run_e2e_test.py](file:///home/azureuser/crypto/scripts/run_e2e_test.py)
*   **Deskripsi Pekerjaan**:
    *   Menulis skrip integrasi end-to-end lengkap untuk mensimulasikan alur penuh secara otonom di testnet Arbitrum Sepolia.
    *   Menjalankan local node Relayer, melakukan splitting key share, dan mengirimkan spend request dengan payload CCIP cross-chain.
    *   **Hasil yang sudah terbukti**: Relayer memproses request, membaca fee
        router sebesar `0.000071 ETH`, dan source transaction berhasil
        dikonfirmasi on-chain.
    *   **Tx Hash CCIP**: `0x96687f14e6ee169a9211b4890cbd513a6e85b80bf1bc7b134c266049d81d42b8`
    *   **Confirm Block**: `274520525`
    *   **Batas bukti**: Gas used `171,463` dan receipt source tidak membuktikan
        CCIP destination success, valid BLS spend, payout, atau recovery.


### Optional Items (Nice to have, can defer to v2)

#### 16. RWA KYC Integration
*   **Status**: Dapat ditunda hanya jika jalur RWA dinonaktifkan
*   **Target**: [nimbus-contracts/src/vault.rs:148](file:///home/azureuser/crypto/nimbus-contracts/src/vault.rs)
*   **Note**: Phase 1 launch dapat skip KYC requirement

#### 17. Chainlink Price Feeds Integration
*   **Status**: Dapat ditunda hanya jika tidak ada accounting berbasis harga
*   **Target**: [nimbus-contracts/src/vault.rs:200](file:///home/azureuser/crypto/nimbus-contracts/src/vault.rs)
*   **Note**: Dapat menggunakan fixed price atau oracle eksternal sementara

#### 18. Fast-Path Phase Management
*   **Status**: Phase 2/3 wajib tetap nonaktif sampai accounting teruji
*   **Target**: [nimbus-contracts/src/lib.rs:70](file:///home/azureuser/crypto/nimbus-contracts/src/lib.rs)
*   **Note**: Phase 1 mengurangi kompleksitas liquidity, tetapi belum dianggap
    aman untuk launch sebelum seluruh P0 dan hard test selesai.

---

## TIMELINE MENUJU MAINNET

Belum ada estimasi tanggal atau durasi yang dapat dipertanggungjawabkan. Jadwal
baru boleh dibuat setelah:

- seluruh P0 di [`todo.md`](../todo.md) selesai;
- hard test tanpa mock menghasilkan evidence lengkap;
- destination CCIP dan settlement recovery terverifikasi;
- compliance/ZK yang diaktifkan menggunakan artifact production;
- external security review selesai;
- seluruh temuan critical/high ditutup atau diterima secara formal.

Dependency order:

1. Selesaikan invariant core payment dan canonical parameter binding.
2. Selesaikan persistent nonce, replacement transaction, finality, dan
   restart reconciliation untuk settlement DEC-005.
3. Verifikasi destination CCIP dan recovery.
4. Harden KMS, guardian API, database, dan operational controls.
5. Jalankan hard-test campaign berulang dari fresh deployment.
6. Lakukan external audit sebelum menerima dana publik.

Soft launch hanya boleh dilakukan setelah gate tersebut lulus. TVL cap adalah
damage limiter, bukan pengganti audit atau correctness.

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
