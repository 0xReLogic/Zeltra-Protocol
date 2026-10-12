# TASK-DEC-038A: Zero-Legacy Purge & Threshold BLS Elimination Execution Checklist

> **Spesifikasi Arsitektur Referensi:**
> - Master Blueprint: [`DEC-038`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-038-pure-zk-utxo-migration-and-direct-mmr-deposit.md)
> - Sub-Spesifikasi Formal: [`DEC-038A`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-038A-zero-legacy-purge-and-threshold-bls-elimination.md)

---

## Mandat Utama
**Kebijakan Zero-Legacy (Hapus Tanpa Jejak):**
- Dilarang keras menggunakan wrapper `#[deprecated]`, dead code, atau legacy shim.
- Seluruh kode, tipe, fungsi, storage slots, rute API, dan tabel basis data peninggalan BDHKE / Threshold BLS dimusnahkan secara fisik dari repositori.

---

## Checklist Eksekusi 5-Fase

### Fase 1: Pembersihan Fondasi Kriptografi (`nimbus-core`)
- [x] Hapus fisik file `nimbus-core/src/blind_sign.rs`.
- [x] Hapus fisik file `nimbus-core/src/threshold.rs`.
- [x] Bersihkan struct/types warisan BLS di `nimbus-core/src/types.rs`:
  - `BlindedMessage`, `BlindingFactor`, `IssuerPublicKey`, `IssuerSecretKey`
  - `MaskedBlindSignature`, `MaskingKey`, `MaskingKeyCommitment`, `PartialBlindSignature`, `UnmaskedSignature`
- [x] Bersihkan re-exports dan deklarasi modul di `nimbus-core/src/lib.rs` (buang `pub mod blind_sign;`, `pub mod threshold;`).
- [x] **Gate Verifikasi Fase 1:**
  ```bash
  cargo check -p nimbus-core
  cargo test -p nimbus-core --lib -- --nocapture
  ```
  *Target: 100% lulus untuk Poseidon Grain-128, MMR, PrivateNoteCircuit, dan JoinSplitCircuit.*

---

### Fase 2: Pembersihan Smart Contract Stylus (`nimbus-contracts`)
- [x] Bersihkan 11 storage slots sesi lama di `nimbus-contracts/src/storage.rs`:
  - `session_client`, `session_amount`, `session_resolved`, `session_timestamp`
  - `session_commitment_hash`, `session_exists`, `session_note_commitment`
  - `trusted_issuer_keys`, `refundable_deposit_liability`, `nullifiers` (legacy BLS), `guardian_registry`
- [x] Hapus konstanta precompile `0x0e` (BLS12_G2_MSM) dan generator EVM G2 dari kontrak.
- [x] Hapus fungsi warisan deposit & reveal di `nimbus-contracts/src/deposit.rs`:
  - `_deposit`, `_deposit_with_commitment`, `_reveal_mask_key`, `_claim_refund`
- [x] Hapus fungsi verifikasi BLS spend di `nimbus-contracts/src/spend.rs`:
  - `verify_bls_spend`, `verify_bls_spend_batch`, `_spend`, `_batch_spend`
- [x] Bersihkan entrypoint publik di `nimbus-contracts/src/lib.rs`:
  - Hapus `deposit`, `deposit_with_commitment`, `reveal_mask_key`, `reveal_mask_key_with_memo`, `claim_refund`, `spend`, `batch_spend`.
- [x] **Gate Verifikasi Fase 2:**
  ```bash
  cargo check -p nimbus-contracts
  cargo test -p nimbus-contracts --lib -- --nocapture
  cargo stylus check -p nimbus-contracts
  ```
  *Target: Kompilasi Stylus WASM bersih tanpa dependensi precompile 0x0e.*

---

### Fase 3: Pembersihan Relayer Sequencer Node (`nimbus-node`)
- [x] Hapus file handler warisan:
  - `nimbus-node/src/handlers/threshold.rs`
  - `nimbus-node/src/handlers/deposit.rs`
  - `nimbus-node/src/circuit_breaker.rs`
  - `nimbus-node/src/deposit_indexer.rs` (digantikan murni oleh event listener MMR)
- [x] Jalankan migrasi basis data SQLite SQLCipher di `nimbus-node/src/database.rs`:
  ```sql
  DROP TABLE IF EXISTS sessions;
  DROP TABLE IF EXISTS deposit_indexer_state;
  DROP TABLE IF EXISTS stalled_sessions;
  DROP TABLE IF EXISTS guardian_shares;
  ```
- [x] Bersihkan konfigurasi environment dari `nimbus-node/.env.test`:
  - Hapus `GUARDIAN_URLS`, `VAULT_ISSUER_KEY`, `THRESHOLD_QUORUM`.
- [x] Hapus rute API threshold dari `nimbus-node/src/main.rs`:
  - `/api/v1/blind-sign`, `/api/v1/sign-share`, `/api/v1/reveal`, `/api/v1/threshold-status`.
- [x] **Gate Verifikasi Fase 3:**
  ```bash
  cargo check -p nimbus-node
  cargo test -p nimbus-node --lib -- --nocapture
  ```
  *Target: Endpoint ZK spend dan quote EIP-712 berjalan normal.*

---

### Fase 4: Pembersihan Client SDK & CLI (`nimbus-sdk` & `nimbus-cli`)
- [x] Hapus file WebAssembly warisan di `nimbus-sdk`:
  - `nimbus-sdk/src/blind_wasm.rs`
  - `nimbus-sdk/src/threshold_wasm.rs`
- [x] Bersihkan logika unmasking BDHKE dan polling Guardian di `nimbus-sdk/src/wallet/note_wallet.rs`.
- [x] Hapus file subcommand BLS di `nimbus-cli`:
  - `nimbus-cli/src/commands/blind.rs`
  - `nimbus-cli/src/commands/threshold.rs`
- [x] Hapus subcommands `SplitKey`, `Aggregate`, `Blind`, `Sign`, `VerifyMasked`, `Unmask`, `VerifyUnmasked` dari `cli.rs` & `main.rs`.
- [x] **Gate Verifikasi Fase 4:**
  ```bash
  cargo test -p nimbus-sdk --lib -- --nocapture
  cargo test -p nimbus-cli -- --nocapture
  ```
  *Target: 100% pass pada pengujian SDK wallet ZK dan CLI.*

---

### Fase 5: Decommissioning Infrastruktur & Scripts
- [x] Hapus skrip orkestrasi 5-node: `scripts/start_cluster.sh`.
- [x] Buat skrip deployment sequencer mandiri: `scripts/start_relayer.sh`.
- [x] Bersihkan konfigurasi Vault KMS dan Tailscale mesh dari CI/CD dan workflow pengujian.
- [x] **Gate Verifikasi Fase 5:**
  ```bash
  bash scripts/start_relayer.sh start
  curl -s http://127.0.0.1:3000/health | grep '"status":"healthy"'
  bash scripts/start_relayer.sh stop
  ```

---

## Kriteria Penerimaan (Acceptance Criteria)
1. **Zero Dead Code:** `git grep -i "blind_sign"` dan `git grep -i "threshold"` di production path mengembalikan 0 hasil relevan.
2. **Kompilasi Hijau:** `cargo test --workspace` dan `cargo clippy --all-targets -- -D warnings` lolos tanpa peringatan.
3. **Ukuran Binary:** Penurunan jejak biner kontrak Stylus WASM dan pembebasan resource server relayer.
