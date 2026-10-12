# TASK-DEC-038C: Single Sequencer Relayer, Client SDK Direct Inflow, & Pure ZK CLI Execution Checklist

> **Spesifikasi Arsitektur Referensi:**
> - Master Blueprint: [`DEC-038`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-038-pure-zk-utxo-migration-and-direct-mmr-deposit.md)
> - Sub-Spesifikasi Formal: [`DEC-038C`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-038C-relayer-sequencer-sdk-direct-deposit-and-cli.md)

---

## Mandat Utama
**Stateless Sequencer & Self-Sovereign Inflow:**
- Relayer node (`nimbus-node`) beroperasi sebagai *Stateless Single Sequencer*: hanya memproses quote gas EIP-712, verifikasi bukti spend Groth16, dan melayani sinkronisasi bukti MMR.
- Client SDK (`nimbus-sdk`) menyiapkan deposit mandiri via `prepare_direct_deposit` ($0.089\text{ ms}$ komputasi lokal, $0$ panggilan ke relayer).
- CLI Tooling (`nimbus-cli`) menyediakan perkakas diagnostik dompet privat dan inspeksi pohon MMR.

---

## Checklist Eksekusi Step-by-Step

### Step 1: Konfigurasi Router API Relayer di Axum 0.8 (`nimbus-node`)
- [ ] Buka `nimbus-node/src/main.rs` dan daftarkan route murni sequencer:
  ```rust
  let app = Router::new()
      .route("/health", get(handlers::health::health_check))
      .route("/api/v1/quote", post(handlers::quote::create_quote))
      .route("/api/v1/spend-private-note", post(handlers::spend::spend_private_note))
      .route("/api/v1/spend-joinsplit", post(handlers::spend::spend_joinsplit))
      .route("/api/v1/mmr/proof/:leaf_index", get(handlers::mmr::get_mmr_proof))
      .route("/api/v1/mmr/leaves", get(handlers::mmr::get_bulk_leaves));
  ```
- [ ] Hapus seluruh rute threshold dari kode produksi.

---

### Step 2: Finalisasi Skema Database SQLite SQLCipher (`database.rs`)
- [ ] Pastikan skema basis data bersih hanya memelihara 5 tabel esensial:
  1. `mmr_leaves`: Daun komitmen on-chain dari event `NoteCommitmentAppended`.
  2. `indexer_state`: Checkpoint blok sinkronisasi indexer.
  3. `spend_queue`: Antrean spend privat yang diasuransikan lease worker.
  4. `spend_batches`: Riwayat batching transaksi on-chain.
  5. `nullifier_cache`: Cache nullifier lokal untuk preflight double-spend check cepat.
  6. `relayer_transactions`: Manajemen nonce atomic dan status transaksi EVM.
- [ ] Pastikan konfigurasi WAL aktif:
  ```sql
  PRAGMA journal_mode = WAL;
  PRAGMA synchronous = NORMAL;
  ```

---

### Step 3: Implementasi SDK Direct Deposit Inflow (`nimbus-sdk`)
- [ ] Buka `nimbus-sdk/src/wallet/note_wallet.rs` dan tambahkan `prepare_direct_deposit`:
  ```rust
  pub fn prepare_direct_deposit(
      &mut self,
      amount: u64,
      recipient_key: Option<Fr>,
  ) -> Result<DirectDepositPayload, WalletError> {
      let spending_key = recipient_key.unwrap_or(self.spending_key);
      let rho = Fr::rand(&mut thread_rng());
      let r = derive_note_randomness(spending_key, rho);
      let cm = note_commitment(amount, spending_key, rho, r);
      let memo = encrypt_note_memo(amount, rho, 1 /* USDC */, &self.viewing_key)?;

      self.stage_pending_deposit_note(cm, amount, rho, r)?;

      Ok(DirectDepositPayload {
          amount,
          commitment_bytes: fr_to_be_bytes(&cm),
          encrypted_memo: memo,
      })
  }
  ```
- [ ] Tambahkan handler konfirmasi event `on_commitment_appended` untuk memutasi status note dari `PendingDeposit` ke `Unspent` (2PC State Invariant).

---

### Step 4: Ephemeral Web Worker Prover & Memory Guard
- [ ] Terapkan pola *Ephemeral Web Worker* untuk prover browser:
  - Worker hanya di-spawn saat pembuktian spend Groth16.
  - Setelah proof didapat via `postMessage`, panggil `worker.terminate()` untuk membebaskan 100% memori linear WASM kembali ke browser.
  - Alur deposit tidak memicu Web Worker sama sekali ($< 100\text{ KB}$ RAM overhead).

---

### Step 5: Modernisasi Subcommand CLI (`nimbus-cli`)
- [ ] Implementasikan subcommand baru di `nimbus-cli/src/commands/`:
  - `GenerateWallet`: Membuat spending key, full viewing key, dan incoming viewing key baru.
  - `CreateDepositNote`: Menghasilkan komitmen Poseidon dan encrypted memo untuk transaksi deposit manual.
  - `InspectMmr`: Menampilkan status puncak MMR (`mmr_peaks`) dan note root terkini.
  - `VerifyProof`: Memverifikasi bukti Groth16 lokal menggunakan Arkworks verifier.
- [ ] Daftarkan subcommand tersebut di `cli.rs` dan `main.rs`.

---

### Step 6: Test Suite Off-Chain ($2\times$ Negative Safety)
- [ ] `POS-1`: `prepare_direct_deposit` menghasilkan komitmen dan memo sah secara instan.
- [ ] `POS-2`: Sinkronisasi event `NoteCommitmentAppended` memutasi status note lokal ke `Unspent`.
- [ ] `NEG-1`: Relayer offline/mati saat user menyetor $\rightarrow$ transaksi deposit on-chain tetap berhasil 100%.
- [ ] `NEG-2`: Event log berisi memo rusak/palsu $\rightarrow$ SDK abaikan secara aman (`Ok(None)`) tanpa panik.
- [ ] `NEG-3`: Pengguna mengirim proof double-spend $\rightarrow$ Relayer tolak di layer preflight (`HTTP 409 Conflict`).
- [ ] `NEG-4`: Pengguna mengirim skalar non-kanonikal $\rightarrow$ Relayer tolak di preflight check (`HTTP 400 Bad Request`).
- [ ] `NEG-5`: Browser crash saat proses deposit $\rightarrow$ SDK pulihkan status note saat restart via 2PC reconciliation.
- [ ] `NEG-6`: Transaksi relayer pending di mempool $\rightarrow$ Watchdog gas-bump menggantikan transaksi secara atomik (+15%).

---

## Verifikasi & Perintah Uji
```bash
cargo check -p nimbus-node
cargo test -p nimbus-node --lib -- --nocapture
cargo test -p nimbus-sdk --lib -- --nocapture
cargo test -p nimbus-cli -- --nocapture
```
