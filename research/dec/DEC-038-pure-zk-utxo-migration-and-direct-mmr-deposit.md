# DEC-038: Full Zero-Legacy Elimination of Threshold BLS & Pure ZK-UTXO Architecture (Direct MMR Inflow)

- **Status:** ACCEPTED (Definitive Architectural Pivot & Zero-Legacy Elimination Mandate)
- **Author:** Zeltra Architecture & Cryptography Team
- **Date:** 2026-10-12
- **Child Sub-Specifications:**
  - [`DEC-038A`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-038A-zero-legacy-purge-and-threshold-bls-elimination.md) — Zero-Legacy Purge, Complete BDHKE / Threshold BLS Elimination, and Multi-Guardian Cluster Decommissioning
  - [`DEC-038B`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-038B-contract-direct-mmr-deposit-and-storage-cleanup.md) — Stylus Smart Contract Direct MMR Inflow, Storage Layout Purification, and Absolute Solvency Invariant
  - [`DEC-038C`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-038C-relayer-sequencer-sdk-direct-deposit-and-cli.md) — Single Sequencer Relayer Architecture, Client SDK Direct Inflow Lifecycle, and Pure ZK CLI Tooling
- **Related DECs:**
  - [`DEC-016`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-016-private-note-change-ledger.md) (Private Note Change Ledger Concept)
  - [`DEC-032`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-032-merkle-mountain-range-commitment-accumulator.md) (Merkle Mountain Range Commitment Accumulator)
  - [`DEC-033`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-033-epoch-windowed-nullifier-pruning-in-flight-rollover.md) (Epoch-Windowed Nullifier Registry & In-Flight Rollover)
  - [`DEC-035`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-035-cryptographic-canonicality-public-input-binding-and-mmr.md) (Cryptographic Canonicality Master Blueprint)
  - [`DEC-036`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-036-universal-2-in-2-out-joinsplit-full-stack-architecture.md) (Universal 2-in-2-out JoinSplit Architecture)
  - [`DEC-037`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-037-note-encryption-memo-and-viewing-key-recovery.md) (Note Encryption Memo & Viewing Key Recovery)

---

## 1. Executive Summary & Kebijakan "Zero-Legacy" (Hapus Tanpa Jejak)

Zeltra Protocol secara resmi menetapkan kebijakan **Zero-Legacy**:
> **"Tidak ada wrapper `#[deprecated]`, tidak ada kode warisan (legacy shim), dan tidak ada tabel database usang yang dibiarkan menggantung. Seluruh rel kriptografi BDHKE / Threshold BLS dan sistem multi-Guardian dihapus secara total dari codebase."**

### Mengapa Keputusan Ini Diambil?
1. **Hasil Audit Benchmark Nyata (AMD EPYC 7763, Linux Container):**
   - **Deposit Pure ZK-UTXO:** Klien hanya menghitung hash Poseidon selama **$74.7\ \mu\text{s}$ ($0.074\text{ ms}$)**. **Nol ZK proof saat deposit**.
   - **Deposit BLS Lama:** Membutuhkan **$944.7\ \mu\text{s}$** kriptografi murni **ditambah $300 - 800\text{ ms}$ latensi jaringan RPC** p2p ke klaster Guardian.
   - **Spend Groth16:** Membutuhkan **$3.21\text{ detik}$** (atau $\sim 1.2 - 1.8\text{ detik}$ di perangkat modern multi-thread) untuk menghasilkan ZK proof dengan **fitur uang kembalian privat atomik ($10 \rightarrow \$3\text{ merchant} + \$7\text{ change})$**.
   - **Kelemahan Fatal BLS:** Tidak mendukung uang kembalian (*voucher fragmentation / voucher hell*).
2. **Eliminasi Kompleksitas Ekstrem:**
   Mempertahankan rel BLS memaksa kita memelihara 5 server Guardian, Vault KMS, mesh Tailscale, registrasi Proof of Possession (RFC 9380), dan smart contract Staking & Slashing $100k. Dengan menghapusnya, protokol menjadi **100% berdaulat (sovereign), trustless, dan matematis murni**.

---

## 2. Inventaris Lengkap: Kode & Komponen yang Dihapus Tanpa Jejak

Berikut adalah pemetaan menyeluruh dari setiap baris, struct, fungsi, dan tabel yang akan dihapus total:

```text
┌────────────────────────────────────────────────────────────────────────────────────────┐
│                        INVENTARIS ELIMINASI ZERO-LEGACY DEC-038                        │
├─────────────────────┬──────────────────────────────────────────────────────────────────┤
│ Crate / Modul       │ Elemen yang Dihapus Total Tanpa Jejak                            │
├─────────────────────┼──────────────────────────────────────────────────────────────────┤
│ zeltra-contracts    │ • Storage: session_client, session_amount, session_resolved,    │
│                     │   session_timestamp, session_commitment_hash, session_exists,    │
│                     │   session_note_commitment, trusted_issuer_keys,                  │
│                     │   refundable_deposit_liability, nullifiers (BLS legacy).         │
│                     │ • Fungsi: _deposit, _deposit_with_commitment, _reveal_mask_key,  │
│                     │   _claim_refund, verify_bls_spend, verify_bls_spend_batch,       │
│                     │   _spend, _batch_spend.                                          │
│                     │ • Precompiles: BLS12_G2_MSM (0x0e), BLS12_G2_GENERATOR_EVM.      │
│                     │ • Entrypoints lib.rs: deposit, deposit_with_commitment,          │
│                     │   reveal_mask_key, reveal_mask_key_with_memo, claim_refund,      │
│                     │   spend, batch_spend.                                            │
├─────────────────────┼──────────────────────────────────────────────────────────────────┤
│ zeltra-core         │ • File HAPUS: src/blind_sign.rs                                  │
│                     │ • File HAPUS: src/threshold.rs                                   │
│                     │ • Types Hapus: BlindedMessage, BlindingFactor, IssuerPublicKey,  │
│                     │   IssuerSecretKey, MaskedBlindSignature, MaskingKey,             │
│                     │   MaskingKeyCommitment, PartialBlindSignature, UnmaskedSignature. │
│                     │ • Re-exports lib.rs: semua simbol blind_sign & threshold.        │
├─────────────────────┼──────────────────────────────────────────────────────────────────┤
│ zeltra-node         │ • File HAPUS: src/handlers/threshold.rs                          │
│                     │ • File HAPUS: src/deposit_indexer.rs                             │
│                     │ • File HAPUS: src/handlers/deposit.rs                            │
│                     │ • File HAPUS: src/circuit_breaker.rs                             │
│                     │ • Database Hapus: Tabel sessions, deposit_indexer_state,         │
│                     │   stalled_sessions, guardian_shares.                             │
│                     │ • Structs Hapus: StalledSessionInfo, SessionForReveal.           │
│                     │ • Config Hapus: GUARDIAN_URLS, VAULT_ISSUER_KEY, THRESHOLD_QUORUM│
├─────────────────────┼──────────────────────────────────────────────────────────────────┤
│ zeltra-sdk          │ • File HAPUS: src/blind_wasm.rs, src/threshold_wasm.rs           │
│                     │ • Hapus: Semua logika unmasking BDHKE, voucher splitting, dan    │
│                     │   polling tanda tangan Guardian di note_wallet.rs.               │
├─────────────────────┼──────────────────────────────────────────────────────────────────┤
│ zeltra-cli          │ • File HAPUS: src/commands/blind.rs, src/commands/threshold.rs   │
│                     │ • Subcommands Hapus: SplitKey, Aggregate, Blind, Sign,           │
│                     │   VerifyMasked, Unmask, VerifyUnmasked dari cli.rs & main.rs.    │
│                     │ • Fitur Baru: CLI Wallet murni ZK (GenerateWallet,               │
│                     │   CreateDepositNote, InspectMmr).                                │
├─────────────────────┼──────────────────────────────────────────────────────────────────┤
│ scripts / infra     │ • Hapus: scripts/start_cluster.sh (1 leader + 4 guardian + Vault)│
│                     │ • Ganti dengan: scripts/start_relayer.sh (single sequencer node).│
└─────────────────────┴──────────────────────────────────────────────────────────────────┘
```

---

## 3. Spesifikasi Arsitektur Baru: 100% Pure ZK-UTXO

Setelah pembersihan total, Zeltra Protocol hanya beroperasi di atas satu rel tunggal:

```text
========================================================================================
ALUR PRODUKSI ZELTRA: PURE ZK-UTXO (DIRECT MMR INFLOW + GROTH16 OUTFLOW)
========================================================================================

[ 1. DEPOSIT INFLOW - USER KE KONTRAK ]
User (HP/Browser)
  1. Generate random: rho ∈ Fr, r = derive_note_randomness(sk, rho)
  2. Hitung commitment: cm = Poseidon(amount, sk, rho, r)            [74 µs]
  3. Enkripsi memo: enc_memo = ChaCha20Poly1305(amount, rho, asset)   [15 µs]
  4. Panggil transaksi:
     Stylus.deposit_direct_note(amount, cm, enc_memo)
          │
          ▼ (Arbitrum Nitro - 250 ms)
     • Transfer USDC dari user ke kontrak
     • user_note_liability += amount
     • _mmr_insert(cm, enc_memo)  ──► Update note_tree_root & mmr_peaks
     • Emit NoteCommitmentAppended(leaf_index, cm, new_root, enc_memo)
     [STATUS: Note Langsung Sah & UNSPENT, 0 Panggilan Relayer, 0 Guardian]

[ 2. SPEND OUTFLOW - USER KE RELAYER KE KONTRAK ]
User (HP/Browser)
  1. Pilih Note dari dompet lokal (Knapsack Coin Selection)
  2. Buat Groth16 ZK Proof lokal via WASM (PrivateNoteCircuit / JoinSplit) [1.5 - 3s]
  3. Kirim proof & public inputs ke Relayer (POST /api/v1/spend-private-note)
          │
          ▼
Relayer (Stateless Sequencer)
  1. Pre-flight check skalar (< r) & semantik domain
  2. Cek nullifier lokal & on-chain
  3. Siarkan tx ke Stylus contract
          │
          ▼ (Arbitrum Nitro)
Stylus.spend_private_note(...)
  1. Verifikasi Groth16 via EIP-2537 precompile (0x0c MSM + 0x0f 4-Pairing) [1.5 ms]
  2. Bakar nullifier (note_nullifiers[nf] = true)
  3. Sisipkan Change Note ke MMR (_mmr_insert)
  4. Transfer USDC ke Merchant & Relayer fee reimbursement
  5. user_note_liability berkurang sesuai nominal belanja
```

---

## 4. Rincian Teknis Per Komponen

### A. Smart Contract (`zeltra-contracts`)

#### 1. Pembersihan Storage (`storage.rs`):
Struktur storage dibersihkan dari 11 field peninggalan BLS:
```rust
sol_storage! {
    #[entrypoint]
    pub struct Zeltra {
        // --- Core Governance & Security ---
        address owner;
        bool paused;
        address stablecoin;
        address fee_recipient;
        address execution_fee_recipient;

        // --- Principal Tracking & Solvency ---
        uint256 total_deposited_principal;
        uint256 user_note_liability;
        uint256 accrued_execution_fee_liability;
        uint256 realized_protocol_fees;

        // --- Merkle Mountain Range (MMR, DEC-032) ---
        uint256 mmr_leaf_count;
        bytes32 note_tree_root;
        mapping(uint256 => bytes32) mmr_peaks;
        mapping(bytes32 => uint256) accepted_note_roots;
        mapping(uint256 => bytes32) root_history;
        uint256 root_history_index;

        // --- ZK-UTXO Nullifiers (Anti Double-Spend) ---
        mapping(bytes32 => bool) note_nullifiers;
        
        // --- Generational Epoch Pruning (DEC-033) ---
        uint32 current_epoch;
        uint64 epoch_start_timestamp;
        mapping(uint8 => uint32) generation_epoch_id;
        mapping((uint8, bytes32) => bool) generation_nullifiers;

        // --- CCIP & Pricing Configuration ---
        address ccip_router;
        mapping(bytes32 => bool) ccip_processed_messages;
        mapping(bytes32 => bool) ccip_allowed_senders;
    }
}
```

#### 2. Entrypoint Tunggal Penyetoran (`deposit.rs`):
```rust
impl Zeltra {
    pub fn deposit_direct_note(
        &mut self,
        amount: U256,
        note_commitment: FixedBytes<32>,
        encrypted_memo: Bytes,
    ) -> Result<(), Vec<u8>> {
        self.check_not_paused()?;

        let min_amount = U256::from(10_000_000); // 10 USDC
        if amount < min_amount {
            return Err(b"AMOUNT_TOO_SMALL".to_vec());
        }

        if crate::types::from_evm_scalar(&note_commitment.0).is_none() {
            return Err(b"INVALID_NOTE_COMMITMENT_SCALAR".to_vec());
        }

        if encrypted_memo.len() < 64 || encrypted_memo.len() > 256 {
            return Err(b"INVALID_MEMO_LENGTH".to_vec());
        }

        let client = self.msg_sender();

        #[cfg(not(test))]
        {
            let stablecoin_address = self.stablecoin.get();
            let erc20 = IErc20::new(stablecoin_address);
            let this_address = self.env_contract_address();
            let host = Self::runtime_host();

            let success = erc20.transfer_from(
                &host,
                Call::new_mutating(self),
                client,
                this_address,
                amount,
            )?;
            if !success {
                return Err(b"TRANSFER_FROM_FAILED".to_vec());
            }
        }

        let principal = self.total_deposited_principal.get();
        self.total_deposited_principal.set(
            principal.checked_add(amount).ok_or(b"OVERFLOW".to_vec())?
        );

        let user_liab = self.user_note_liability.get();
        self.user_note_liability.set(
            user_liab.checked_add(amount).ok_or(b"OVERFLOW".to_vec())?
        );

        // Langsung append komitmen ke MMR on-chain
        self._mmr_insert(note_commitment, encrypted_memo.to_vec())?;

        // Penegasan matematis solvensi mutlak
        self.check_liability_invariant()?;

        Ok(())
    }
}
```

#### 3. Invariant Solvensi yang Disederhanakan:
$$ \text{Contract USDC Balance} \ge \text{user\_note\_liability} + \text{accrued\_execution\_fee\_liability} + \text{realized\_protocol\_fees} $$
Tidak ada lagi `refundable_deposit_liability` karena tidak ada lagi sesi deposit menggantung!

---

### B. Primitives Kriptografi (`zeltra-core`)

1. **Penghapusan File:** Hapus fisik file `src/blind_sign.rs` dan `src/threshold.rs`.
2. **Pembersihan `types.rs`:** Hapus semua struct pembungkus BLS point/scalar (`BlindedMessage`, `IssuerPublicKey`, dsb).
3. **Core API Murni:**
   - Poseidon Hash & Grain-128 LFSR.
   - Merkle Mountain Range (MMR) & Bagging.
   - `PrivateNoteCircuit` (16 public inputs, Groth16).
   - `JoinSplitCircuit` (19 public inputs, Groth16).
   - Note AEAD ChaCha20-Poly1305 & Viewing Keys (DEC-037).
   - Fee calculations (Flat 45 bps).

---

### C. Relayer Node & Database (`zeltra-node`)

1. **Pembersihan Database SQLite (`database.rs`):**
   - Hapus tabel:
     * `DROP TABLE IF EXISTS sessions;`
     * `DROP TABLE IF EXISTS deposit_indexer_state;`
     * `DROP TABLE IF EXISTS stalled_sessions;`
     * `DROP TABLE IF EXISTS guardian_shares;`
   - Tabel yang dipertahankan murni:
     * `mmr_leaves`: Menyimpan event on-chain `NoteCommitmentAppended`.
     * `indexer_state`: Menyimpan checkpoint blok terakhir untuk MMR indexer.
     * `spend_queue` & `spend_batches`: Antrean transaksi spend ZK.
     * `nullifier_cache`: Cache nullifier unspent/spent lokal.
     * `relayer_transactions`: Manajemen nonce dan status tx EVM.
2. **Pembersihan Handlers & Routes (`main.rs`):**
   - Hapus route `/api/v1/blind-sign`, `/api/v1/sign-share`, `/api/v1/reveal`, `/api/v1/threshold-status`.
   - Hapus thread worker `deposit_indexer.rs`.
   - Node bertindak sebagai **Single High-Performance Sequencer**:
     * `GET /api/v1/mmr/proof/:leaf_index` $\rightarrow$ Menyajikan bukti MMR instan ke client SDK.
     * `POST /api/v1/quote` $\rightarrow$ Menyajikan signed quote biaya eksekusi gas.
     * `POST /api/v1/spend-private-note` $\rightarrow$ Menerima spend note Groth16.
     * `POST /api/v1/spend-joinsplit` $\rightarrow$ Menerima multi-UTXO joinsplit Groth16.

---

### D. Client SDK (`zeltra-sdk`)

1. **Metode Penyetoran Mandiri (`prepare_direct_deposit`):**
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

       // Simpan langsung note kandidat ke lokal
       self.stage_pending_deposit_note(cm, amount, rho, r)?;

       Ok(DirectDepositPayload {
           amount,
           commitment_bytes: fr_to_be_bytes(&cm),
           encrypted_memo: memo,
       })
   }
   ```
2. **UI/UX Lifecycle:**
   - User klik *"Deposit $100"*.
   - Web3 Wallet (MetaMask/Rabby) memunculkan pop-up transaksi `deposit_direct_note`.
   - Konfirmasi blok Arbitrum (~250 milidetik).
   - SDK menangkap event `NoteCommitmentAppended` $\rightarrow$ saldo $100 langsung berstatus **UNSPENT** dan siap dibelanjakan.

### E. CLI Wallet & Cryptography Utility (`zeltra-cli`)

1. **File yang Dihapus Total:**
   - `src/commands/blind.rs` (seluruh alur blind signing, unblinding, verify masked).
   - `src/commands/threshold.rs` (seluruh alur Shamir split key, partial sign, Lagrange aggregation).
2. **Subcommand yang Dihapus dari `cli.rs` & `main.rs`:**
   - `SplitKey`, `Aggregate`, `Blind`, `Sign`, `VerifyMasked`, `Unmask`, `VerifyUnmasked`.
3. **Subcommand Baru untuk Pengembang & Integrator:**
   - `GenerateWallet`: Membuat wallet privat baru (Spending Key + Viewing Keys).
   - `CreateDepositNote`: Membangkitkan commitment Poseidon dan encrypted memo untuk transaksi deposit di rantai.
   - `VerifyProof`: Memverifikasi Groth16 proof secara lokal di command-line.
   - `InspectMmr`: Memeriksa root MMR dan bagging siblings dari node relayer.

### F. SDK WASM Bindings (`zeltra-sdk`)

1. **File yang Dihapus Total:**
   - `src/blind_wasm.rs`: Seluruh binding WebAssembly untuk BLS client blinding.
   - `src/threshold_wasm.rs`: Seluruh binding WebAssembly untuk threshold aggregation.
2. **WASM Bindings yang Dipertahankan:**
   - `src/zk_wasm.rs`: Groth16 note prover & proof verification di browser.
   - `src/fee_wasm.rs`: Perhitungan fee 45 bps dan quote range.
   - `src/evm_wasm.rs`: Serialisasi skalar EVM canonical (< r).

---

## 5. Rencana Eksekusi Bertahap (Clean-Room Execution Protocol)

Untuk memastikan kompilasi dan pengujian tidak hancur saat pembersihan dilakukan:

```text
┌────────────────────────────────────────────────────────────────────────┐
│                    URUTAN EKSEKUSI PEMBERSIHAN KODE                    │
├─────────┬──────────────────────────────────────────────────────────────┤
│ LANGKAH │ TINDAKAN DETAIL                                              │
├─────────┼──────────────────────────────────────────────────────────────┤
│ FASE 1  │ zeltra-core: Hapus blind_sign.rs, threshold.rs, dan BLS      │
│         │ types di types.rs & lib.rs. Pastikan `cargo test -p          │
│         │ zeltra-core` lolos 100% untuk modul note & circuits.        │
├─────────┼──────────────────────────────────────────────────────────────┤
│ FASE 2  │ zeltra-contracts: Tambah deposit_direct_note, bersihkan      │
│         │ storage.rs dari session_*, hapus _reveal_mask_key,          │
│         │ _claim_refund, dan verify_bls_spend. Jalankan `cargo test`.  │
├─────────┼──────────────────────────────────────────────────────────────┤
│ FASE 3  │ zeltra-node: Hapus handlers/threshold.rs, deposit_indexer.rs,│
│         │ bersihkan schema SQLite, dan sederhanakan routes di main.rs. │
├─────────┼──────────────────────────────────────────────────────────────┤
│ FASE 4  │ zeltra-sdk: Integrasi prepare_direct_deposit dan bersihkan   │
│         │ sisa pemanggilan BDHKE di note_wallet.rs.                    │
├─────────┼──────────────────────────────────────────────────────────────┤
│ FASE 5  │ Scripts & Docs: Hapus scripts/start_cluster.sh, perbarui     │
│         │ dokumentasi modular ke Pure ZK-UTXO.                         │
└─────────┴──────────────────────────────────────────────────────────────┘
```

---

## 6. Kesimpulan & Penegasan Solvensi

Dengan mengeksekusi DEC-038:
1. **Zero Dead Code:** Seluruh kode yang tidak relevan dengan ZK-UTXO dimusnahkan.
2. **Zero Attack Surface dari Guardian:** Risiko serangan kartel, Rogue Key Attack, dan koordinasi off-chain menjadi **NOL**.
3. **Produk 10/10 & Privasi 10/10:** Deposit instan 250 milidetik, biaya gas $<\$0.003$, dan pembelanjaan dengan uang kembalian nyata secepat Apple Pay.
