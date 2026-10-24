# DEC-043C: Relayer Dual-Memo Ingress, Client Note Wallet & Trial Decryption Recovery

- **Status:** APPROVED AS ARCHITECTURAL SUB-SPECIFICATION (Child of [`DEC-043`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-043-shielded-to-shielded-in-pool-transfer-and-merchant-masking.md))
- **Author:** Zeltra Protocol Architecture & Cryptography Team
- **Date:** 2026-10-16
- **Applies to:**
  - `zeltra-node/src/dto.rs` (Request & Response DTOs, 192-char hex memo fields)
  - `zeltra-node/src/handlers/spend.rs` (3-mode preflight validation, anti-griefing calldata checks)
  - `zeltra-node/src/evm_client.rs` (Alloy contract invocation with dual 96B memos)
  - `zeltra-sdk/src/wallet/note_wallet.rs` (`prepare_shielded_transfer_proof`, `recover_notes_batch`, Knapsack coin selection)
- **Parent & Related DECs:**
  - [`DEC-043`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-043-shielded-to-shielded-in-pool-transfer-and-merchant-masking.md) (Master In-Pool Transfer Blueprint)
  - [`DEC-043A`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-043A-zsa-key-derivation-and-deterministic-ecdh-memo.md) (ZSA Key Derivation & Deterministic ECDH Randomness)
  - [`DEC-043B`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-043B-stylus-strict-three-mode-settlement-and-dual-memo-mmr.md) (Stylus Smart Contract Strict Three-Mode Settlement)
  - [`DEC-024`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-024-client-private-wallet-state-crash-safety-coin-selection.md) (Client Wallet State & 2PC Crash-Safety)
  - [`DEC-030`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-030-multi-utxo-joinsplit-coin-selection-zeroize-and-aead-backup.md) (Tiered Knapsack Coin Selection)
  - [`DEC-035C`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-035C-relayer-mmr-indexer-worker-and-client-sync-architecture.md) (Relayer MMR Indexer & Client REST Sync)
  - [`DEC-037`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-037-note-encryption-memo-and-viewing-key-recovery.md) (Viewing Key Recovery Protocol)
- **Security Audit Resolusi:** AUD-1 (ZSA End-to-End Handling), AUD-2 (Deterministic Randomness in Prover & Recovery), AUD-3 (Sender IVK Public Key), AUD-6 (Client Gating of Zero-Payout), AUD-7 (Dummy Memo Encryption & Skipping).
- **Architectural Paradigm:** *"Fail-Closed Relayer Preflight, Zero-Leak Client Proving, Seamless Merchant Synchronization"*

---

## 1. Executive Summary & Alur Komunikasi End-to-End

Arsitektur In-Pool Transfer menghubungkan tiga entitas independen: **Client SDK Pengirim (Alice)**, **Stateless Relayer Sequencer (`zeltra-node`)**, dan **Client SDK Penerima (Bob/Merchant)**:

```text
┌────────────────────────────────────────────────────────────────────────────────────────┐
│               DEC-043C END-TO-END DATAFLOW & INTEGRATION ARCHITECTURE                   │
└────────────────────────────────────────────────────────────────────────────────────────┘

 [Sender SDK: Alice]
  • Receives Bob's ZSA (64B Bech32m: zs1zeltra...)
  • Selects unspent notes via Knapsack (v_in1 + v_in2 >= v_pay + v_exec)
  • Derives deterministic r_1 via X25519 ECDH with pk_ivk_bob
  • Generates cm_1 = note_commitment(v_pay, asset_id, owner_key_bob, rho_1, r_1)
  • Encrypts Memo 1 (96B) and Memo 2 (96B)
  • Proves JoinSplitCircuit (19 PIs: merchant=0, fee=0, rec=0x0)
  • Assembles JoinSplitSpendRequest DTO
         │
         ▼ HTTP POST /api/v1/spend-joinsplit
 ┌───────────────────────────────────────────────────────────────────────────┐
 │ Relayer Sequencer (zeltra-node)                                           │
 │ • Preflight Check 1: encrypted_memo_1 & encrypted_memo_2 == 192 hex chars │
 │ • Preflight Check 2: 3-Mode Mutually Exclusive State Machine Validation    │
 │ • Preflight Check 3: Local Groth16 Preflight Verify (DEC-026 C-01)        │
 │ • Database Ingress: Transaction Idempotency Queue                         │
 │ • Broadcast to Stylus Contract via Alloy (evm_client.rs)                  │
 └───────────────────────────────────────────────────────────────────────────┘
         │
         ▼ Stylus L2 Execution (_spend_joinsplit)
  Contract emits NoteCommitmentAppended(cm_1, memo_1) & (cm_2, memo_2)
         │
         ▼ Indexed by mmr_indexer worker
 [Recipient SDK: Bob]
  • Calls recover_notes_batch()
  • Decrypts Memo 1 with ivk_bob -> extracts (v_pay, rho_1, asset_id)
  • Filters out dummy notes (if value == 0 -> SKIP)
  • Reconstructs r_1 = derive_ecdh_note_randomness(shared_secret, rho_1)
  • Reconstructs cm_1 -> matches MMR leaf!
  • Marks note unspent in Bob's wallet -> Balance successfully credited!
```

---

## 2. Spesifikasi Relayer Sequencer (`zeltra-node`)

### A. Data Transfer Objects (`src/dto.rs`)
Menambahkan dua parameter memo wajib berukuran tepat 192 karakter heksadesimal (96 byte):

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JoinSplitSpendRequest {
    pub session_id: String,
    pub input_commitment_1_hex: String,
    pub input_commitment_2_hex: Option<String>,
    pub input_nullifier_1_hex: String,
    pub input_nullifier_2_hex: String,
    pub note_root_hex: String,
    pub leaf_count: u64,
    pub epoch_id_1: u32,
    pub epoch_id_2: u32,
    pub output_commitment_1_hex: String,
    pub output_commitment_2_hex: String,
    pub recipient_hex: String,
    pub merchant_amount: u64,
    pub protocol_fee: u64,
    pub execution_fee: u64,
    pub max_execution_fee: u64,
    pub quote_hash_hex: String,
    pub chain_id: u64,
    pub contract_address_hex: String,
    pub expiry: u64,
    pub flags_packed: u8,
    pub is_rollover: bool,
    pub proof_a_neg_hex: String,
    pub proof_b_hex: String,
    pub proof_c_hex: String,
    pub public_inputs_hex: Vec<String>,
    pub encrypted_memo_1: String, // Wajib 192 hex chars (96 bytes)
    pub encrypted_memo_2: String, // Wajib 192 hex chars (96 bytes)
}
```

### B. Preflight Ingress Guards (`src/handlers/spend.rs`)
Relayer memvalidasi integritas request secara *fail-closed* sebelum mengirim transaksi ke antrean sequencer:

1. **Memo Length Guard (AUD-5):**
   ```rust
   let clean_m1 = payload.encrypted_memo_1.strip_prefix("0x").unwrap_or(&payload.encrypted_memo_1);
   let clean_m2 = payload.encrypted_memo_2.strip_prefix("0x").unwrap_or(&payload.encrypted_memo_2);
   if clean_m1.len() != 192 || clean_m2.len() != 192 {
       return Json(JoinSplitSpendResponse {
           status: "REJECTED".into(),
           message: "encrypted_memo_1 and encrypted_memo_2 must be exactly 96 bytes (192 hex chars)".into(),
           tx_hash: None,
           queue_position: 0,
           estimated_gas_usdc: None,
       });
   }
   ```

2. **Strict 3-Mode State Machine Validation (AUD-4):**
   ```rust
   let is_rollover = payload.is_rollover;
   let has_change_1 = (payload.flags_packed & 1) == 1;
   let is_zero_rec = payload.recipient_hex.trim_start_matches("0x").chars().all(|c| c == '0');

   if is_rollover {
       if payload.merchant_amount != 0 || payload.protocol_fee != 0 || !is_zero_rec {
           return Json(JoinSplitSpendResponse {
               status: "REJECTED".into(),
               message: "Rollover transactions must have merchant_amount=0, protocol_fee=0, recipient=0x0".into(),
               tx_hash: None,
               queue_position: 0,
               estimated_gas_usdc: None,
           });
       }
   } else if payload.merchant_amount == 0 {
       // In-Pool Transfer Mode
       if !has_change_1 || payload.protocol_fee != 0 || !is_zero_rec {
           return Json(JoinSplitSpendResponse {
               status: "REJECTED".into(),
               message: "In-Pool Transfer requires has_change_1=1, protocol_fee=0, and recipient=0x0".into(),
               tx_hash: None,
               queue_position: 0,
               estimated_gas_usdc: None,
           });
       }
   } else {
       // Public Outflow Mode
       if is_zero_rec {
           return Json(JoinSplitSpendResponse {
               status: "REJECTED".into(),
               message: "Public spend cannot have zero recipient address".into(),
               tx_hash: None,
               queue_position: 0,
               estimated_gas_usdc: None,
           });
       }
   }
   ```

### C. Alloy Client Broadcast (`src/evm_client.rs`)
Fungsi `broadcast_spend_joinsplit_transaction` mengonversi hex memo menjadi `Bytes` (tepat 96 byte) dan menyertakannya pada pemanggilan kontrak Stylus `spend_joinsplit(...)`.

---

## 3. Spesifikasi Client SDK & Private Note Wallet (`zeltra-sdk`)

### A. Metode `prepare_shielded_transfer_proof` (`src/wallet/note_wallet.rs`)
Metode ini mengorkestrasi pembuatan bukti ZK transfer privat:

```rust
#[allow(clippy::too_many_arguments)]
pub fn prepare_shielded_transfer_proof(
    &mut self,
    recipient_zsa_bech32m_or_hex: &str,
    transfer_amount: u64,
    asset_id: u8,
    max_execution_fee: u64,
    quote_hash_hex: &str,
    chain_id: u64,
    contract_address_hex: &str,
    expiry: u64,
    current_time_secs: u64,
    joinsplit_pk: &ark_groth16::ProvingKey<Bls12_381>,
) -> Result<JoinSplitSpendProofPayload, WalletError> {
    // 1. Validasi nominal transfer (AUD-6)
    if transfer_amount == 0 {
        return Err(WalletError::CryptoError("Transfer amount must be strictly greater than zero".into()));
    }

    // 2. Parsing ZSA penerima (64B) (AUD-1)
    let recipient_zsa = if recipient_zsa_bech32m_or_hex.starts_with("zs") {
        zeltra_core::ZeltraShieldedAddress::from_bech32m(recipient_zsa_bech32m_or_hex)
            .map_err(|e| WalletError::CryptoError(format!("Invalid Bech32m ZSA: {e}")))?
    } else {
        zeltra_core::ZeltraShieldedAddress::from_hex(recipient_zsa_bech32m_or_hex)
            .map_err(|e| WalletError::CryptoError(format!("Invalid Hex ZSA: {e}")))?
    };

    // 3. Stochastic Knapsack Coin Selection
    let total_required = transfer_amount
        .checked_add(max_execution_fee)
        .ok_or_else(|| WalletError::CryptoError("Transfer amount overflow".into()))?;
    let selected = self.select_notes_knapsack_asset(total_required, asset_id)?;

    // 4. Output 1: Note Penerima dengan Deterministic Randomness (AUD-2)
    let mut rng = rand::rngs::OsRng;
    let rho_1 = Fr::rand(&mut rng);
    let (memo_1, rand_1) = zeltra_core::encrypt_note_memo_deterministic(
        transfer_amount,
        rho_1,
        asset_id as u64,
        &recipient_zsa.incoming_viewing_public_key,
    ).map_err(|e| WalletError::CryptoError(format!("Memo 1 encryption failed: {e}")))?;
    let cm_1 = zeltra_core::note_commitment(
        transfer_amount,
        asset_id,
        recipient_zsa.owner_key,
        rho_1,
        rand_1,
    );

    // 5. Output 2: Note Kembalian Pengirim (AUD-3 & AUD-7)
    let change_amount = selected.change_amount;
    let sender_owner_key = self.owner_key()?;
    let sender_pk_ivk = self.incoming_viewing_public_key()?;

    let (cm_2, memo_2, has_change_2) = if change_amount > 0 {
        let rho_2 = Fr::rand(&mut rng);
        let (memo_2, rand_2) = zeltra_core::encrypt_note_memo_deterministic(
            change_amount,
            rho_2,
            asset_id as u64,
            &sender_pk_ivk,
        ).map_err(|e| WalletError::CryptoError(format!("Memo 2 encryption failed: {e}")))?;
        let c_cm = zeltra_core::note_commitment(
            change_amount,
            asset_id,
            sender_owner_key,
            rho_2,
            rand_2,
        );
        (c_cm, memo_2, true)
    } else {
        // Enkripsi acak untuk dummy memo (AUD-7)
        let rho_2 = Fr::rand(&mut rng);
        let (dummy_memo, rand_2) = zeltra_core::encrypt_note_memo_deterministic(
            0,
            rho_2,
            asset_id as u64,
            &sender_pk_ivk,
        ).map_err(|e| WalletError::CryptoError(format!("Dummy memo encryption failed: {e}")))?;
        let dummy_cm = zeltra_core::note_commitment(
            0,
            asset_id,
            sender_owner_key,
            rho_2,
            rand_2,
        );
        (dummy_cm, dummy_memo, false)
    };

    // 6. Buat Witness & Sirkuit JoinSplit (Mode 2)
    // merchant_amount = 0, protocol_fee = 0, recipient = Address::ZERO
    // has_change_1 = 1, has_change_2 = (change_amount > 0)
    // ...
}
```

### B. Pemulihan Saldo Otomatis di Sisi Merchant (`recover_notes_batch`)
Penerima (merchant) memindai on-chain log `NoteCommitmentAppended`:
1. Mendekripsi memo via `zeltra_core::decrypt_note_memo_deterministic(&memo_bytes, &self.incoming_viewing_key()?)`.
2. Jika gagal atau jika `pt.value == 0`, abaikan (*skip*) dummy note tersebut.
3. Rekonstruksi komitmen daun:
   $$cm = \text{note_commitment}(pt.value, pt.asset\_id \text{ as u8}, \text{self.owner\_key}()?, pt.rho, rand)$$
4. Verifikasi komitmen cocok dengan event log on-chain.
5. Cek status nullifier via view call `is_nullifier_spent`.
6. Simpan note aktif ke dalam `self.notes` dengan status `NoteStatus::Unspent`.

---

## 4. Matriks Pengujian Unit & E2E (`zeltra-node` & `zeltra-sdk`)

Sesuai aturan engineering Zeltra, implementasi DEC-043C wajib disertai **1 Positive Test + Minimal 2x Negative Tests**:

### Positive Tests:
1. `test_e2e_shielded_transfer_flow`: Alice mentransfer 5 USDC ke Bob. Alice men-generate proof Mode 2 dengan dual memo 96B, relayer menerima request via preflight, transaksi dieksekusi di kontrak, Bob memanggil `recover_notes_batch()`, saldo Bob bertambah 5 USDC, saldo Alice berkurang (5 USDC + execution fee).
2. `test_exact_shielded_transfer_without_change`: Alice membayar pas tanpa kembalian (`change_amount == 0`). Output 2 terenkripsi acak, Bob me-recover 5 USDC, Alice me-recover 0 USDC (dummy note diabaikan oleh scanner).

### Negative Tests (Minimal 2x Positive = Minimal 4 Tests):
1. `test_prepare_transfer_rejects_zero_amount`: Me-revert jika pengguna mencoba mentransfer `amount == 0`.
2. `test_prepare_transfer_rejects_insufficient_balance`: Me-revert jika saldo tidak cukup untuk menutupi `transfer_amount + max_execution_fee`.
3. `test_relayer_rejects_invalid_memo_hex_length`: Relayer menolak request dengan status `REJECTED` jika `encrypted_memo_1` berukuran 190 karakter hex (bukan 192).
4. `test_relayer_rejects_non_zero_recipient_in_pool`: Relayer menolak request in-pool jika `recipient_hex != "0x0000...0000"`.
5. `test_prepare_transfer_rejects_corrupted_zsa`: Me-revert jika string ZSA penerima memiliki checksum Bech32m yang salah.
