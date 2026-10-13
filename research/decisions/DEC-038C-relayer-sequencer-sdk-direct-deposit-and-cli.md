# DEC-038C: Single Sequencer Relayer Architecture, Client SDK Direct Inflow Lifecycle, and Pure ZK CLI Tooling

- **Status:** APPROVED AS ARCHITECTURAL SUB-SPECIFICATION (Child of [`DEC-038`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-038-pure-zk-utxo-migration-and-direct-mmr-deposit.md))
- **Author:** Zeltra Architecture & Cryptography Team
- **Date:** 2026-10-12
- **Applies to:**
  - `zeltra-node/src/database.rs` (Purge of legacy session tables & SQLite schema finalization)
  - `zeltra-node/src/handlers/` (Elimination of `threshold.rs`, `deposit.rs`, `circuit_breaker.rs`; pure ZK endpoints)
  - `zeltra-node/src/deposit_indexer.rs` (Replaced by MMR event listener worker)
  - `zeltra-sdk/src/wallet/note_wallet.rs` (`prepare_direct_deposit`, direct note staging, unspent state machine)
  - `zeltra-sdk/src/blind_wasm.rs`, `threshold_wasm.rs` (Complete deletion of legacy WASM bindings)
  - `zeltra-cli/src/commands/` (Purge of BLS commands; pure ZK wallet and MMR diagnostic CLI)
- **Parent & Related DECs:**
  - [`DEC-038`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-038-pure-zk-utxo-migration-and-direct-mmr-deposit.md) (Master Pure ZK-UTXO Architecture Blueprint)
  - [`DEC-038A`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-038A-zero-legacy-purge-and-threshold-bls-elimination.md) (Zero-Legacy Purge & Threshold BLS Elimination Protocol)
  - [`DEC-038B`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-038B-contract-direct-mmr-deposit-and-storage-cleanup.md) (Stylus Smart Contract Direct MMR Inflow & Storage Purification)
  - [`DEC-017`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-017-relayer-receipt-finality-and-nonce-management.md) (Relayer Receipt Finality & Nonce Management)
  - [`DEC-024`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-024-client-private-wallet-state-crash-safety-coin-selection.md) (Client Private Wallet State & Crash Safety)
  - [`DEC-025`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-025-relayer-zk-note-spend-settlement-atomic-batching-and-reconciliation.md) (Relayer ZK Note Spend Settlement Pipeline)
  - [`DEC-031`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-031-relayer-reconciliation-solvency-observability-quote-domain-binding.md) (Relayer Reconciliation & Solvency Observability)
  - [`DEC-035C`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-035C-relayer-mmr-indexer-worker-and-client-sync-architecture.md) (Relayer MMR Indexer Worker & Client Sync Architecture)
  - [`DEC-037`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-037-note-encryption-memo-and-viewing-key-recovery.md) (Note Encryption Memo & Viewing Key Recovery)

---

## 1. Executive Summary & Problem Statement

Pada paradigma transisi sebelumnya, lapisan off-chain Zeltra Protocol memikul beban komputasi dan koordinasi yang sangat terfragmentasi:
1. **Relayer Sebagai Koordinator Multi-Guardian:** Relayer bertindak sebagai pemimpin klaster (*Leader*) yang wajib mengoordinasikan 4 Guardian independen via jaringan mesh Tailscale, melakukan agregasi tanda tangan Shamir/BLS, dan mengelola instance HashiCorp Vault / OpenBao KMS.
2. **Database Overloaded dengan Sesi Asinkronus:** Basis data SQLite SQLCipher terbebani oleh tabel-tabel pemantau sesi (`sessions`, `stalled_sessions`, `guardian_shares`), menciptakan risiko korupsi kunci dan kegagalan leasing worker saat terjadi lonjakan beban.
3. **Ketergantungan Total Pengguna pada Relayer Saat Menyetor:** Client SDK (`zeltra-sdk`) tidak dapat menyetor dana secara mandiri; pengguna harus meminta izin relayer, memanggil endpoint blind-sign, dan menunggu pelepasan masking key $k$. Jika relayer mengalami gangguan (*downtime*), aliran dana masuk (*inflow*) terhenti total.

**DEC-038C** menetapkan transformasi menyeluruh pada arsitektur off-chain:
- **Stateless Single Sequencer Relayer (`zeltra-node`):** Relayer sepenuhnya dibebaskan dari urusan penyetoran dana dan koordinasi multi-party. Relayer murni beroperasi sebagai *high-throughput execution sequencer* untuk memvalidasi bukti ZK Groth16, mensponsori gas eksekusi on-chain, dan melayani sinkronisasi data pohon Merkle Mountain Range (MMR).
- **Self-Sovereign Client Inflow (`zeltra-sdk`):** Pengguna dapat menyetor langsung ke smart contract Arbitrum Stylus secara mandiri tanpa memerlukan koneksi jaringan ke relayer. Komputasi lokal hanya memakan waktu **$74.7\ \mu\text{s}$ (Poseidon Hash)** tanpa beban memori WebAssembly.
- **Pure ZK Developer & Operator Tooling (`zeltra-cli`):** Menghapus seluruh subcommand warisan BLS dan melengkapi pengembang dengan perkakas CLI diagnostik untuk membuat dompet privat, menghasilkan komitmen deposit, dan memverifikasi integritas pohon MMR.

---

## 2. Vulnerability Archaeology & Post-Mortem Analysis (2024–2026)

Infrastruktur off-chain pada protokol privasi zero-knowledge (relayer node, database lokal, WebAssembly browser prover, dan sinkronisasi status dompet) memiliki spektrum kerentanan yang kompleks. Audit sistem terdistribusi (Trail of Bits, Nethermind, OpenZeppelin, Zellic 2024–2026) mengungkap bahwa kegagalan off-chain sering kali melumpuhkan protokol bahkan saat kriptografi on-chain terbukti kokoh:

```
          OFF-CHAIN SEQUENCER & CLIENT SDK THREAT ARCHAEOLOGY
┌─────────────────────────────────────┬─────────────────────────────────────┬─────────────────────────────────────┐
│ [Case A: Relayer MEV Censorship]    │ [Case B: Browser WASM Linear OOM]   │ [Case C: SQLite WAL Concurrency]    │
│ MEV-Boost & PBS Audits 2024–2025:   │ Web Worker Memory Traps (2025):     │ SQLCipher Deadlock Post-Mortem:     │
│ Stateful relayers censor deposits   │ Unshrinkable 4GB WASM linear pages  │ Shared un-synchronized DB handles   │
│ & frontrun gas fee margins          │ trigger fatal browser mobile crash  │ corrupt key shares during bursts    │
├─────────────────────────────────────┼─────────────────────────────────────┼─────────────────────────────────────┤
│ [Case D: Wallet 2PC Crash Limbo]    │ [Case E: Mempool Nonce Gap Lockout] │ [Case F: Viewing Key Scanner Panic] │
│ Conduition / Cashu Disclosures:     │ Relayer Nonce Desync (DEC-017):     │ Zcash / Railgun Supply-Chain Audits:│
│ Process crash between staging & tx  │ Dropped L2 transaction halts queue  │ Malformed event memo crashes client │
│ causes permanent balance desync     │ with cascading replacement failure  │ event loop & halts wallet sync      │
└─────────────────────────────────────┴─────────────────────────────────────┴─────────────────────────────────────┘
```

---

### A. Case A: Relayer Censorship & MEV Frontrunning (MEV-Boost / PBS Audits, 2024–2025)
- **Konteks & Sitasi:** Riset Nethermind dan Flashbots mengenai arsitektur Proposer-Builder Separation ([Nethermind Research, 2024](https://nethermind.io/)) menyoroti kerapuhan relayer tersentralisasi yang memiliki hak prerogatif untuk menyaring atau menolak transaksi pengguna secara diskriminatif.
- **Root Cause:**
  Pada sistem di mana relayer bertindak sebagai gatekeeper penyetoran (seperti model Zeltra Phase 1), relayer dapat:
  1. Melakukan sensor selektif terhadap alamat penyetor tertentu (*inflow censorship*).
  2. Memeras pengguna dengan menolak merilis masking key $k$ kecuali pengguna membayar biaya tambahan.
- **Relevansi & Mitigasi Zeltra (DEC-038C):**
  DEC-038C **memisahkan alur deposit dari relayer secara total (*complete decoupling*)**. Penyetoran adalah transaksi mandiri antara pengguna dan smart contract on-chain. Relayer **tidak memiliki kemampuan sensor terhadap deposit pengguna**. Pada alur spend (outflow), pengguna terlindungi oleh bukti Groth16 zero-knowledge dan EIP-712 quote domain binding (DEC-031), sehingga relayer tidak dapat mengubah penerima pembayaran atau biaya fee.

---

### B. Case B: Browser WebAssembly Linear Memory Traps & OOM Crashes (Arkworks & SnarkJS, 2024–2025)
- **Konteks & Sitasi:** Investigasi eksekusi ZK browser pada Web Worker ([WebAssembly Memory Limitations Research, 2024–2025](https://developer.mozilla.org/)) mengungkap bahwa komputasi SNARK di perangkat mobile sering mengalami *Out-Of-Memory (OOM)* fatal.
- **Root Cause:**
  WebAssembly dibatasi oleh ruang alamat linear 32-bit (`memory32`, maksimal 4GB virtual, namun browser mobile sering me-revert saat penggunaan mencapai 1.5–2GB). Jika pengguna diwajibkan menghasilkan ZK proof saat pertama kali menyetor uang (*deposit proving*), browser ponsel pengguna akan hang atau mengalami crash, merusak adopsi retail. Selain itu, WebAssembly tidak dapat mengembalikan memori yang telah dialokasikan kembali ke sistem operasi tanpa me-reset runtime.
- **Relevansi & Mitigasi Zeltra (DEC-038C):**
  Zeltra menegakkan prinsip **Zero ZK Proof Saat Penyetoran**:
  - Deposit hanya menghitung 1 kali hash Poseidon dan 1 enkripsi memo AEAD ($74.7\ \mu\text{s} + 15\ \mu\text{s} = 0.089\text{ ms}$).
  - Komputasi ini membutuhkan memori $< 100\text{ KB}$, berjalan mulus di browser ponsel entry-level tanpa risiko OOM.
  - Untuk pembelanjaan (spend), SDK menerapkan pola **Ephemeral Web Worker Lifecycle**: worker hanya dibuat saat membangkitkan proof spend Groth16 dan langsung dimatikan secara total (`worker.terminate()`) setelah proof selesai, membebaskan 100% memori ke browser.

---

### C. Case C: SQLite / SQLCipher Concurrency Corruption & WAL Starvation
- **Konteks & Sitasi:** Dokumentasi teknis Zetetic SQLCipher dan audit post-mortem database tertanam ([Zetetic SQLCipher Architecture, 2024–2025](https://www.zetetic.net/sqlcipher/)) menekankan pentingnya isolasi koneksi basis data terenkripsi.
- **Root Cause:**
  Pada sistem multi-guardian lama, koneksi SQLCipher dibagi antar-thread worker yang menangani polling RPC, verifikasi tanda tangan, dan pelepasan kunci secara paralel. Saat beban transaksi melonjak, thread saling mengunci (*deadlock*), memicu WAL checkpoint starvation dan korupsi file basis data.
- **Relevansi & Mitigasi Zeltra (DEC-038C):**
  1. Seluruh 4 tabel warisan (`sessions`, `deposit_indexer_state`, `stalled_sessions`, `guardian_shares`) dihapus total.
  2. Database relayer difokuskan murni pada antrean transaksi (`spend_queue`, `spend_batches`, `nullifier_cache`, `mmr_leaves`).
  3. Konfigurasi SQLite dioptimalkan dengan `PRAGMA journal_mode = WAL;`, `PRAGMA synchronous = NORMAL;`, dan connection pool terisolasi via `bb8` / `r2d2` dengan leasing atomic.

---

### D. Case D: Client Note Wallet Crash Desynchronization & 2PC State Safety
- **Konteks & Sitasi:** Riset dompet privat (DEC-024) dan audit kerentanan Cashu (2025) menunjukkan risiko kehilangan saldo jika aplikasi dompet mengalami crash di tengah proses transaksi.
- **Root Cause:**
  Jika dompet menandai note baru sebagai "UNSPENT" sebelum transaksi on-chain terkonfirmasi, atau jika dompet menghapus note sebelum transaksi spend selesai ditambang, pengguna akan kehilangan catatan saldo atau mencoba membelanjakan note yang belum sah di rantai.
- **Relevansi & Mitigasi Zeltra (DEC-038C):**
  SDK mengimplementasikan **Two-Phase Commit (2PC) State Machine**:
  ```text
  [Candidate Note Generated] ──► Status: PendingDeposit (disimpan di DB lokal)
                                       │
                                       ▼ (Deteksi Event NoteCommitmentAppended)
                               Status: Unspent (siap dibelanjakan ZK)
  ```
  Jika aplikasi browser ditutup sebelum blok Arbitrum masuk, saat dibuka kembali, SDK memindai event on-chain berdasarkan komitmen yang tersimpan dan secara atomik memutasi statusnya ke `Unspent`. Tidak ada dana yang hilang atau tercecer.

---

### E. Case E: Relayer Nonce Desynchronization & Mempool Limbo (DEC-017)
- **Konteks & Sitasi:** Cacat klasik relayer EVM saat transaksi macet di mempool akibat lonjakan harga gas basefee L2.
- **Root Cause:**
  Jika transaksi relayer dengan nonce $N$ tidak ditambang karena gas limit rendah, seluruh transaksi berikutnya ($N+1, N+2$) tertahan di antrean mempool, membekukan seluruh settlement ZK pengguna.
- **Relevansi & Mitigasi Zeltra (DEC-038C):**
  Relayer mempertahankan arsitektur DEC-017: manajemen atomic nonce manager berbasis memori, watchdog pemantau tanda terima blok, dan mekanisme penggantian transaksi otomatis (*gas-bump +15% transaction replacement*) jika transaksi belum terkonfirmasi dalam 3 blok.

---

### F. Case F: Viewing Key / Encrypted Memo Scanning Panics (AEAD Defense)
- **Konteks & Sitasi:** Audit audit Zcash / Railgun (2024–2025) mengenai pemindaian catatan terenkripsi (*note decryption scanning*) pada node sinkronisasi klien.
- **Root Cause:**
  Penyerang dapat mengirimkan event deposit on-chain dengan memo acak yang sengaja direkayasa untuk memicu kegagalan panik (*panic / unwrap error*) pada fungsi deserialisasi klien. Jika parsing error menghentikan loop sinkronisasi, dompet pengguna gagal mendeteksi saldo barunya.
- **Relevansi & Mitigasi Zeltra (DEC-038C & DEC-037):**
  Parsing memo menggunakan ChaCha20-Poly1305 terotentikasi. Jika tag MAC Poly1305 gagal diverifikasi (menandakan memo milik pengguna lain atau rusak), SDK memperlakukannya sebagai operasi `Ok(None)` yang diabaikan secara aman tanpa menginterupsi antrean sinkronisasi.

---

## 3. Invariants Arsitektur Off-Chain & Client SDK

Sistem off-chain Zeltra wajib memenuhi invarian-invarian berikut secara mutlak:

- **INV-REL-1 (Deposit Decoupling & Sovereign Inflow):**
  Penyetoran dana oleh pengguna tidak boleh bergantung pada ketersediaan API HTTP atau persetujuan node Relayer. Pengguna berinteraksi langsung dengan smart contract Arbitrum Stylus.
- **INV-REL-2 (Stateless Relayer Ingress & Fail-Closed Preflight):**
  Relayer wajib memverifikasi validitas bukti Groth16, batasan skalar kanonikal ($x < r$), status nullifier lokal, dan validitas quote EIP-712 sebelum menyiarkan transaksi ke mempool blockchain.
- **INV-REL-3 (Crash-Safe 2PC Note Lifecycle):**
  Dompet klien wajib menyimpan kandidat note ke dalam penyimpanan lokal terenkripsi berstatus `PendingDeposit` sebelum memicu transaksi Web3 wallet. Mutasi ke status `Unspent` hanya terjadi setelah konfirmasi event on-chain `NoteCommitmentAppended`.
- **INV-REL-4 (WASM Memory Isolation & Ephemeral Lifetime):**
  Modul WebAssembly pada browser klien wajib membatasi alokasi memori linear. Worker prover Groth16 wajib berumur pendek (*ephemeral*) dan dimusnahkan segera setelah pembuktian selesai untuk mencegah memory leaks.
- **INV-REL-5 (SQLCipher Clean State & WAL Checkpointing):**
  Database relayer tidak boleh menyimpan tabel atau field peninggalan BLS. Operasi I/O wajib menggunakan mode Write-Ahead Logging (`WAL`) dengan transaksi ACID terisolasi.
- **INV-REL-6 (Fail-Closed EIP-712 Quote Binding):**
  Setiap quote biaya gas eksekusi relayer wajib terikat ke domain EIP-712 dengan nonce unik, batas kedaluwarsa waktu (*expiry timestamp*), dan tanda tangan kriptografis secp256k1 milik relayer (DEC-031).

---

## 4. Transformasi Relayer Node (`zeltra-node`)

### A. Migrasi Skema Basis Data SQLite (`database.rs`)
Seluruh skema warisan multi-guardian dimusnahkan secara permanen:

```sql
-- MIGRASI ZERO-LEGACY DEC-038C: Hapus Tabel Warisan
DROP TABLE IF EXISTS sessions;
DROP TABLE IF EXISTS deposit_indexer_state;
DROP TABLE IF EXISTS stalled_sessions;
DROP TABLE IF EXISTS guardian_shares;
```

#### Skema Ramping Produksi Relayer (SQLCipher AES-256):
```sql
-- 1. Tabel Daun MMR On-Chain (DEC-035C)
CREATE TABLE IF NOT EXISTS mmr_leaves (
    leaf_index INTEGER PRIMARY KEY,
    commitment BLOB NOT NULL,
    encrypted_memo BLOB NOT NULL,
    block_number INTEGER NOT NULL,
    tx_hash BLOB NOT NULL
);

-- 2. Checkpoint Sinkronisasi Indexer
CREATE TABLE IF NOT EXISTS indexer_state (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    last_indexed_block INTEGER NOT NULL,
    last_updated_at INTEGER NOT NULL
);

-- 3. Antrean Transaksi Spend Privat (DEC-025)
CREATE TABLE IF NOT EXISTS spend_queue (
    id TEXT PRIMARY KEY,
    circuit_type TEXT NOT NULL, -- 'private_note' atau 'joinsplit'
    proof_bytes BLOB NOT NULL,
    public_inputs_bytes BLOB NOT NULL,
    nullifiers_json TEXT NOT NULL,
    status TEXT NOT NULL,       -- 'PENDING', 'LEASED', 'CONFIRMED', 'FAILED'
    lease_until INTEGER,
    created_at INTEGER NOT NULL
);

-- 4. Cache Nullifier Cepat (Anti-Spam)
CREATE TABLE IF NOT EXISTS nullifier_cache (
    nullifier BLOB PRIMARY KEY,
    spent_at_block INTEGER,
    first_seen_at INTEGER NOT NULL
);

-- 5. Transaksi EVM & Nonce Manager (DEC-017)
CREATE TABLE IF NOT EXISTS relayer_transactions (
    tx_hash BLOB PRIMARY KEY,
    nonce INTEGER NOT NULL,
    gas_price_gwei REAL NOT NULL,
    status INTEGER NOT NULL,     -- 0: Pending, 1: Success, 2: Replaced, 3: Reverted
    created_at INTEGER NOT NULL
);
```

### B. Router REST API Produksi di Axum 0.8 (`main.rs`)
Menghapus seluruh route threshold signing dan hanya mengekspos endpoint esensial ZK Sequencer:

```rust
pub fn create_relayer_router(state: Arc<AppState>) -> Router {
    Router::new()
        // 1. Solvensi Makro & Health Check (DEC-031)
        .route("/health", get(handlers::health::health_check))
        
        // 2. Gas Execution Quote EIP-712 (DEC-035)
        .route("/api/v1/quote", post(handlers::quote::create_quote))
        
        // 3. ZK Spend Ingress Endpoints (DEC-025 / DEC-036C)
        .route("/api/v1/spend-private-note", post(handlers::spend::spend_private_note))
        .route("/api/v1/spend-joinsplit", post(handlers::spend::spend_joinsplit))
        
        // 4. MMR Tree Sync & Bukti Merkle Off-Chain (DEC-035C)
        .route("/api/v1/mmr/proof/:leaf_index", get(handlers::mmr::get_mmr_proof))
        .route("/api/v1/mmr/leaves", get(handlers::mmr::get_bulk_leaves))
        .with_state(state)
}
```

---

## 5. Client SDK Direct Deposit Lifecycle (`zeltra-sdk`)

### A. Alur Kerja Komputasi Klien

```text
┌────────────────────────────────────────────────────────────────────────────────────────┐
│                      ALUR PENYETORAN MANDIRI (CLIENT SDK)                              │
├────────────────────────────────────────────────────────────────────────────────────────┤
│ 1. GENERATE COMMITMENT & MEMO (Lokal di Browser / Mobile - 0.089 ms)                   │
│    • spending_key (sk) diambil dari secure storage / mnemonic.                        │
│    • rho ∈ Fr dibangkitkan secara acak kriptografis (thread_rng).                      │
│    • r = derive_note_randomness(sk, rho).                                              │
│    • cm = Poseidon(amount, sk, rho, r)                    [74.7 µs]                    │
│    • enc_memo = ChaCha20Poly1305(amount, rho, asset, vk)  [15.0 µs]                    │
│                                                                                        │
│ 2. STAGING KE WALLET DB LOKAL                                                          │
│    • Simpan kandidat note dengan status NoteStatus::PendingDeposit.                    │
│                                                                                        │
│ 3. TRANSAKSI ON-CHAIN WEB3 WALLET (MetaMask / Rabby)                                   │
│    • User menyetujui pemanggilan: Stylus.deposit_direct_note(amount, cm, enc_memo)     │
│    • Konfirmasi blok Arbitrum Nitro (~250 ms).                                         │
│                                                                                        │
│ 4. SINKRONISASI OTOMATIS (EVENT LISTENER)                                              │
│    • SDK mendeteksi event NoteCommitmentAppended(leaf_index, cm, root, memo).          │
│    • Status note diperbarui menjadi NoteStatus::Unspent secara instan.                 │
│    • DANA SIAP DIBELANJAKAN (ZK GROTH16 SPEND).                                        │
└────────────────────────────────────────────────────────────────────────────────────────┘
```

### B. Implementasi di `note_wallet.rs`
```rust
impl PrivateNoteWallet {
    /// Menyiapkan payload untuk transaksi on-chain deposit_direct_note.
    /// Operasi ini 100% lokal, cepat (0.089 ms), dan tanpa dependensi jaringan relayer.
    pub fn prepare_direct_deposit(
        &mut self,
        amount: u64,
        recipient_key: Option<Fr>,
    ) -> Result<DirectDepositPayload, WalletError> {
        let spending_key = recipient_key.unwrap_or(self.spending_key);
        let rho = Fr::rand(&mut thread_rng());
        let r = derive_note_randomness(spending_key, rho);
        let cm = note_commitment(amount, spending_key, rho, r);

        // Enkripsi memo untuk viewing key pemilik (DEC-037)
        let memo = encrypt_note_memo(amount, rho, 1 /* USDC */, &self.viewing_key)?;

        // Simpan langsung note kandidat ke lokal dalam status PENDING (2PC Staging)
        self.stage_pending_deposit_note(cm, amount, rho, r)?;

        Ok(DirectDepositPayload {
            amount,
            commitment_bytes: fr_to_be_bytes(&cm),
            encrypted_memo: memo,
        })
    }

    /// Menghandle event konfirmasi on-chain NoteCommitmentAppended.
    /// Memutasi status note kandidat dari PendingDeposit menjadi Unspent.
    pub fn on_commitment_appended(
        &mut self,
        leaf_index: u64,
        commitment: [u8; 32],
        _root: [u8; 32],
    ) -> Result<bool, WalletError> {
        if let Some(note) = self.find_pending_note_mut(&commitment) {
            note.leaf_index = Some(leaf_index);
            note.status = NoteStatus::Unspent;
            self.persist_note_store()?;
            return Ok(true);
        }
        Ok(false)
    }
}
```

### C. Pembersihan Binding WebAssembly (`zeltra-sdk`)
File binding yang dihapus total:
- `src/blind_wasm.rs`: Seluruh ekspor binding unmasking BDHKE.
- `src/threshold_wasm.rs`: Seluruh ekspor binding rekonstruksi Lagrange.

Modul WASM yang dipertahankan & dioptimalkan untuk browser:
- `src/zk_wasm.rs`: Prover Groth16 terisolasi untuk Web Worker.
- `src/fee_wasm.rs`: Perhitungan flat fee 45 bps dan estimasi gas.
- `src/evm_wasm.rs`: Sanitasi skalar kanonikal ($x < r$).

---

## 6. Modernisasi CLI Tooling (`zeltra-cli`)

Perkakas baris perintah `zeltra-cli` disederhanakan dari utilitas pengujian multi-party menjadi perkakas pengembang dan operator ZK-UTXO:

### A. Subcommand yang Dihapus Total:
- `SplitKey`, `Aggregate`, `Blind`, `Sign`, `VerifyMasked`, `Unmask`, `VerifyUnmasked`.

### B. Subcommand Baru yang Didukung:
1. **`GenerateWallet`**:
   Membangkitkan pasangan kunci privat baru: Spending Key ($sk$), Full Viewing Key ($fvk$), dan Incoming Viewing Key ($ivk$).
   ```bash
   zeltra-cli generate-wallet --out-file my_wallet.json
   ```
2. **`CreateDepositNote`**:
   Menghitung komitmen Poseidon dan encrypted memo untuk disematkan dalam transaksi manual on-chain.
   ```bash
   zeltra-cli create-deposit-note --amount 100000000 --spending-key 0x123...
   # Output:
   # Amount: 100 USDC (100000000 micro-units)
   # Commitment: 0x24a7bc9...
   # Encrypted Memo: 0x89ef01... (128 bytes)
   ```
3. **`InspectMmr`**:
   Melakukan query status puncak MMR (`mmr_peaks`) dan note root terkini dari node relayer atau smart contract on-chain.
   ```bash
   zeltra-cli inspect-mmr --rpc-url https://sepolia-rollup.arbitrum.io/rpc
   ```
4. **`VerifyProof`**:
   Memverifikasi keabsahan bukti Groth16 secara lokal menggunakan Arkworks verifier sebelum disiarkan ke mempool.
   ```bash
   zeltra-cli verify-proof --circuit joinsplit --proof-file proof.bin --inputs-file public_inputs.json
   ```

---

## 7. Matriks Efisiensi, UX, dan Latensi Benchmark

Perbandingan komprehensif antara sistem BLS lama dan sistem Pure ZK-UTXO:

| Parameter Evaluasi | Sistem BLS Lama (Phase 1) | Pure ZK-UTXO (DEC-038C) | Peningkatan |
|---|---|---|---|
| **Dependensi Penyetoran** | Relayer + 5 Guardian Online | **0 Dependensi (Smart Contract Mandiri)** | **100% Sovereign Inflow** |
| **Waktu Komputasi Deposit Klien** | $944.7\ \mu\text{s}$ (Operasi Kurva EC) | **$74.7\ \mu\text{s}$ (Poseidon Hash)** | **12.6x Lebih Cepat** |
| **Latensi Jaringan Deposit** | $300 - 800\text{ ms}$ (RPC klaster) | **$0\text{ ms}$ (Nol panggilan API relayer)** | **Zero Network Overhead** |
| **Beban Memori Klien Saat Deposit** | $> 25\text{ MB}$ (State & Buffer) | **$< 100\text{ KB}$ (Hash Lokal Ringan)** | **Bebas Crash OOM di Mobile** |
| **Kesiapan Saldo Belanja** | Tergantung transaksi `reveal` kedua | **Langsung setelah konfirmasi 1 blok Nitro** | **Instant Finality (~250 ms)** |
| **Dukungan Uang Kembalian** | Tidak Ada (Voucher Fragmentation) | **Penuh (Private Note JoinSplit/Change)** | **Pengalaman Uang Nyata ($10-$3=$7)** |
| **Kompleksitas Infrastruktur** | 5 Server + Vault + Tailscale | **1 Single Sequencer Node** | **-80% Biaya Server & Infra** |

---

## 8. Matriks Pengujian Negatif & Positive Safety ($2\times$ Negative Safety)

Setiap pengujian integrasi off-chain wajib memverifikasi ketahanan kegagalan sistem dan pembuktian state invariance:

| Kode Uji | Skenario Pengujian | Input Parameter | Hasil yang Diharapkan | Penegasan State Invariance |
|---|---|---|---|---|
| **POS-1** | Eksekusi `prepare_direct_deposit` lokal | `amount: 100 USDC`, `sk: valid_fr` | Payload siap kirim, note tercatat `PendingDeposit` | Basis data dompet menyimpan komitmen sah secara lokal |
| **POS-2** | Deteksi event on-chain oleh SDK | Event `NoteCommitmentAppended` sah | Status bermutasi ke `Unspent`, saldo dompet terbarui | Saldo note siap dikonsumsi dalam coin selection spend |
| **NEG-1** | Relayer offline saat deposit user | HTTP connection error ke relayer | Transaksi on-chain tetap berhasil 100% | Penyetoran **TIDAK TERGANTUNG** pada status relayer |
| **NEG-2** | Event log mengandung memo rusak / palsu | Memo acak atau MAC tag tidak cocok | SDK abaikan memo via `Ok(None)` tanpa panic | Loop sinkronisasi dompet **TIDAK TERHENTI / TIDAK CRASH** |
| **NEG-3** | Pengguna mengirim spend proof double-spend | `nullifier` telah ada di `nullifier_cache` | Relayer tolak seketika (HTTP 409 Conflict) | Transaksi tidak disiarkan, kuota gas relayer tidak terbuang |
| **NEG-4** | Pengguna mengirim skalar non-kanonikal | Public inputs skalar $\ge r$ | Relayer tolak pada preflight check (HTTP 400) | Tidak ada transaksi yang disiarkan ke blockchain |
| **NEG-5** | Crash aplikasi di tengah deposit | Browser crash sebelum event terbaca | Saat restart, SDK mendeteksi event tertinggal | Status note berhasil pulih ke `Unspent` (2PC Recovery) |
| **NEG-6** | Nonce L2 tertahan di mempool relayer | Transaksi pending $> 3$ blok | Atomic nonce manager memicu gas-bump +15% | Antrean relayer tidak deadlock, transaksi digantikan sah |

---

## 9. Kesimpulan & Penegasan Arsitektur

Dengan diterapkannya spesifikasi DEC-038C:
1. **Relayer Menjadi Stateless & Tahan Banting:** Relayer tidak lagi memegang tanggung jawab kustodian atau koordinasi multi-party saat penyetoran dana. Kegagalan atau restart relayer tidak pernah mengancam deposit pengguna.
2. **Kedaulatan Pengguna 100%:** Pengguna merasakan pengalaman deposit yang identik dengan interaksi Web3 standar, namun dengan privasi zero-knowledge mutlak saat berbelanja.
3. **Efisiensi Memori Klien Maksimal:** Ketiadaan ZK prover saat penyetoran menjamin aplikasi web dan mobile berjalan ringan, bebas crash OOM, dan siap diadopsi oleh jutaan pengguna retail.

---

## 10. References & Citations

1. **Grass, J.; & Zeltra Architecture Team.** (2026). *DEC-024: Client Private Wallet State, Crash-Safety, and Coin Selection.* Zeltra Protocol Architectural Decisions. [DEC-024](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-024-client-private-wallet-state-crash-safety-coin-selection.md)
2. **Zeltra Architecture Team.** (2026). *DEC-025: Relayer ZK Note Spend Settlement Pipeline and Atomic Batching.* Zeltra Protocol Architectural Decisions. [DEC-025](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-025-relayer-zk-note-spend-settlement-atomic-batching-and-reconciliation.md)
3. **Zeltra Architecture Team.** (2026). *DEC-031: Relayer Reconciliation, Solvency Observability, and EIP-712 Quote Domain Binding.* Zeltra Protocol Architectural Decisions. [DEC-031](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-031-relayer-reconciliation-solvency-observability-quote-domain-binding.md)
4. **Zeltra Architecture Team.** (2026). *DEC-035C: Relayer MMR Indexer Worker and Client Synchronization Architecture.* Zeltra Protocol Architectural Decisions. [DEC-035C](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-035C-relayer-mmr-indexer-worker-and-client-sync-architecture.md)
5. **Zeltra Architecture Team.** (2026). *DEC-037: Note Encryption Memo and Viewing Key Recovery Architecture via ChaCha20-Poly1305.* Zeltra Protocol Architectural Decisions. [DEC-037](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-037-note-encryption-memo-and-viewing-key-recovery.md)
6. **Zeltra Architecture Team.** (2026). *DEC-038: Full Zero-Legacy Elimination of Threshold BLS & Pure ZK-UTXO Architecture (Direct MMR Inflow).* Zeltra Protocol Architectural Decisions. [DEC-038](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-038-pure-zk-utxo-migration-and-direct-mmr-deposit.md)
7. **Zeltra Architecture Team.** (2026). *DEC-038A: Zero-Legacy Purge, Complete BDHKE / Threshold BLS Elimination, and Multi-Guardian Cluster Decommissioning.* Zeltra Protocol Architectural Decisions. [DEC-038A](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-038A-zero-legacy-purge-and-threshold-bls-elimination.md)
8. **Zeltra Architecture Team.** (2026). *DEC-038B: Stylus Smart Contract Direct MMR Inflow, Storage Layout Purification, and Absolute Solvency Invariant.* Zeltra Protocol Architectural Decisions. [DEC-038B](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-038B-contract-direct-mmr-deposit-and-storage-cleanup.md)
9. **Nethermind Security & Flashbots.** (2024). *MEV-Boost and Proposer-Builder Separation Security Assessments: Censorship Hazards, Latency Games, and Relayer Centralization.* Nethermind Publications. [https://nethermind.io/](https://nethermind.io/)
10. **Mozilla Developer Network (MDN).** (2024–2025). *WebAssembly Memory Specification and Web Worker Lifecycle Management.* MDN Web Docs. [https://developer.mozilla.org/en-US/docs/WebAssembly/JavaScript_interface/Memory](https://developer.mozilla.org/en-US/docs/WebAssembly/JavaScript_interface/Memory)
11. **Zetetic LLC.** (2024–2025). *SQLCipher Architecture and Security Design: Transparent AES-256 Database Encryption, Write-Ahead Logging (WAL), and Concurrency Locks.* Zetetic Publications. [https://www.zetetic.net/sqlcipher/](https://www.zetetic.net/sqlcipher/)
12. **Tokio Project & Axum Team.** (2024–2026). *Axum 0.8: Ergonomic and High-Performance Web Framework for Rust.* Tokio Organization. [https://github.com/tokio-rs/axum](https://github.com/tokio-rs/axum)
13. **Arkworks Community.** (2024). *ark-groth16 & ark-bls12-381: Zero-Knowledge SNARK Ecosystem in Rust.* Arkworks Organization. [https://github.com/arkworks-rs](https://github.com/arkworks-rs)
