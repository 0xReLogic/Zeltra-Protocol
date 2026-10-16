# TASK C (DEC-034C): Relayer Zero-Oracle, Client SDK Multi-Asset & CLI Tooling (`zeltra-node`, `zeltra-sdk`, `zeltra-cli`)

> **Scope:** `zeltra-node/`, `zeltra-sdk/`, `zeltra-cli/`  
> **Spesifikasi Referensi:** [`research/decisions/DEC-034C-relayer-zero-oracle-reimbursement-and-sdk-multi-asset-wallet.md`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-034C-relayer-zero-oracle-reimbursement-and-sdk-multi-asset-wallet.md) & [`research/decisions/DEC-034-dual-asset-architecture-usdc-weth-sovereign-commerce-rails.md`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-034-dual-asset-architecture-usdc-weth-sovereign-commerce-rails.md)  
> **Prinsip:** *"Zero-Oracle Economic Immunity, Strictly Partitioned Coin Selection, Cryptographic Quote Domain Binding"*

---

## 1. Latar Belakang & Tujuan Arsitektur
1. **Zero-Oracle Gas Reimbursement:** Menghilangkan ketergantungan relayer pada Price Oracle eksternal (Chainlink/Pyth/Uniswap TWAP). Fee belanja WETH dipotong dalam WETH (1:1 native gas), fee belanja USDC dipotong dalam USDC berdasarkan kuotasi L2 terjamin.
2. **EIP-712 Domain & Quote Asset Binding (Patch Audit #7):** Struktur quote relayer wajib mengikat `asset_id` secara kriptografis ke dalam `quote_hash` (yang dibuktikan di public inputs sirkuit) untuk mencegah serangan substitusi kuotasi lintas aset.
3. **Partitioned Client Note Wallet (Patch Audit #5 & #6):** Dompet mengisolasi note USDC dan WETH secara ketat, menggunakan kunci publik `owner_key` (bukan raw `spending_key`), dan menurunkan randomness dari `shared_secret` ECDH + $\rho$ (DEC-037).

---

## 2. Checklist Implementasi Rinci

### A. Relayer Sequencer Node (`zeltra-node`)
- [x] **1. DTO & Kuotasi Fee Dual-Asset (`zeltra-node/src/dto.rs` & `handlers/quote.rs`):**
  - Perbarui `ExecutionQuoteRequest` dan `ExecutionQuoteResponse`:
    ```rust
    pub struct ExecutionQuoteRequest {
        pub asset_id: u8,               // 0: WETH, 1: USDC
        pub target_contract: String,
        pub estimated_gas_limit: u64,
    }

    pub struct ExecutionQuoteResponse {
        pub asset_id: u8,
        pub base_gas_fee: u64,
        pub relayer_markup_fee: u64,    // Flat 15% markup (DEC-031)
        pub total_execution_fee: u64,   // Satuan Gwei (WETH) atau Mikro (USDC)
        pub quote_hash: String,
        pub expiry: u64,                // Maks 20 detik
        pub relayer_signature: String,  // EIP-712 Signature
    }
    ```
  - **TypeHash EIP-712 Wajib Menyertakan `assetId` (Patch Audit #7):**
    ```solidity
    bytes32 constant EXECUTION_QUOTE_TYPEHASH = keccak256(
        "ExecutionQuote(uint8 assetId,address targetContract,uint64 baseGasFee,uint64 relayerMarkupFee,uint64 totalExecutionFee,uint64 expiry,uint256 chainId)"
    );
    ```
  - Pastikan `quote_hash_hi` dan `quote_hash_lo` dihitung dari hash EIP-712 yang memuat `asset_id`.
  - Implementasikan perlindungan anti-arbitrase: `expiry <= 20` detik dan penolakan otomatis jika lonjakan base fee L2 $> 1.5\times$.
- [x] **2. Preflight Validation Guard (`zeltra-node/src/validation.rs`):**
  - Validasi bahwa `req.asset_id <= 1`.
  - Anti-Cross-Asset Fee Drain: Validasi bahwa `req.asset_id == cached_quote.asset_id`.
  - Verifikasi proof Groth16 lokal CPU sebelum broadcast (Fail-Closed anti gas-griefing DEC-026 C-01).
- [x] **3. Routing Ingress Spend (`zeltra-node/src/handlers/spend.rs`):**
  - Menerima payload spend untuk WETH (`asset_id: 0`) atau USDC (`asset_id: 1`).
  - Broadcast transaksi ke kontrak Stylus dan pantau konfirmasi receipt L2.

### B. Client SDK & Private Note Wallet (`zeltra-sdk`)
- [x] **1. Struct `PrivateNoteV1` Terintegrasi `asset_id` (Patch Audit #5):**
  - Lokasi: `zeltra-sdk/src/wallet/note_wallet.rs`
  ```rust
  #[derive(Clone, Debug, Serialize, Deserialize)]
  pub struct PrivateNoteV1 {
      pub value: u64,        // Gwei (jika asset_id == 0) atau Micro-USDC (jika asset_id == 1)
      pub asset_id: u8,      // 0: WETH, 1: USDC
      pub owner_key: Fr,     // Public owner key (bukan raw spending_key)
      pub rho: Fr,
      pub randomness: Fr,
  }

  impl PrivateNoteV1 {
      pub fn value_packed(&self) -> Fr {
          let two_pow_64 = Fr::from(1u128 << 64);
          Fr::from(self.value) + (Fr::from(self.asset_id as u64) * two_pow_64)
      }

      pub fn commitment(&self) -> Fr {
          native_poseidon_w5(
              self.value_packed(),
              self.owner_key,
              self.rho,
              self.randomness,
              DOMAIN_NOTE_COMMITMENT,
          )
      }
  }
  ```
- [x] **2. Penyimpanan Note Terpartisi (`PrivateNoteWallet`):**
  - Pisahkan storage note: `usdc_notes: Vec<PrivateNoteV1>` dan `weth_notes: Vec<PrivateNoteV1>`.
  - Implementasikan `get_balance(asset_id: u8) -> Result<u64, WalletError>`.
  - Implementasikan `get_human_balance(asset_id: u8) -> Result<String, WalletError>`.
- [x] **3. Deserialisasi Memo Kanonikal Big-Endian (Patch Audit #6):**
  - Signature: `process_decrypted_plaintext(&mut self, plaintext_bytes: &[u8; 48], shared_secret: &[u8; 32])`.
  - Baca `value` dari byte 0..8, `rho` dari byte 8..40, dan `asset_id` dari byte 40..48.
  - Validasi `asset_id <= 1`.
  - Turunkan `randomness = derive_note_randomness(shared_secret, &rho)` (sesuai DEC-037).
  - Gunakan `owner_key = self.incoming_viewing_key.owner_key()`.
  - Arahkan simpanan ke `weth_notes` jika `asset_id == 0` atau `usdc_notes` jika `asset_id == 1`.
- [x] **4. Knapsack Coin Selection Terpartisi (`zeltra-sdk/src/coin_selection.rs`):**
  - Partisi total: solver koin hanya memeriksa kolam aset yang relevan.
  - **Zero Mixed-Asset Transactions:** Tolak pemilihan kombinasi note (WETH + USDC) di tingkat SDK.
  - Perhitungan fee protokol flat 45 bps dengan pembagian ke atas (*ceiling division*):
    $$\text{protocol\_fee} = \frac{\text{merchant\_amount} \times 45 + 9999}{10000}$$
- [x] **5. Penandatangan EIP-712 (`zeltra-sdk/src/eip712.rs`):**
  - Perbarui definisi struct `ExecutionQuote` pada macro `sol!` agar memuat field `uint8 assetId`.

### C. Zeltra CLI Utility (`zeltra-cli`)
- [x] **1. Pembaruan Argumen Perintah:**
  - `create-deposit-note --amount <val> --asset [usdc|eth|weth] --recipient <addr>`
  - `spend-private-note --amount <val> --asset [usdc|weth] --to <addr>`
  - `balance`: Menampilkan saldo terlindung terpartisi untuk WETH (Sovereign Rail) dan USDC (Commerce Rail).

---

## 3. Matriks Pengujian Wajib (Unit & Integration Tests)
Jalankan dan pastikan 100% lulus:
1. `test_relayer_quote_dual_asset`: Kuotasi fee eksekusi WETH diterbitkan dalam satuan Gwei dan USDC dalam mikro-unit dengan tanda tangan EIP-712 yang valid. [PASSED]
2. `test_relayer_quote_expiry_strict_window`: Permintaan spend dengan quote yang telah berumur lebih dari 20 detik ditolak seketika oleh relayer. [PASSED]
3. `test_knapsack_rejects_mixed_assets`: Upaya memilih koin gabungan WETH dan USDC ditolak secara instan oleh solver coin selection. [PASSED]
4. `test_relayer_rejects_cross_asset_quote_tampering`: Permintaan spend note WETH menggunakan quote USDC ditolak pada preflight check (`CrossAssetFeeMismatch`). [PASSED]
5. `test_sdk_trial_decryption_partitions_notes_correctly`: Deserialisasi memo log on-chain secara otomatis mengelompokkan note masuk ke sub-wallet yang sesuai via format kanonikal Big-Endian. [PASSED]
6. `test_knapsack_ceiling_fee_calculation`: Pembayaran dengan pecahan kecil (baik WETH maupun USDC) membuktikan nilai fee protokol tidak pernah terpotong ke nol. [PASSED]
7. `test_eip712_quote_hash_binds_asset_id`: Memastikan manipulasi `asset_id` pada permintaan spend membatalkan verifikasi tanda tangan EIP-712 relayer dan quote hash sirkuit. [PASSED]

---

## 4. Definisi Selesai (Definition of Done)
- [x] Seluruh crate terkompilasi bersih: `cargo check -p zeltra-node -p zeltra-sdk -p zeltra-cli`.
- [x] Seluruh unit & integration test lulus: `cargo test -p zeltra-sdk` & `cargo test -p zeltra-node` & `cargo test -p zeltra-cli`.
- [x] `cargo fmt --check` dan `cargo clippy -- -D warnings` 100% bersih.
- [x] Tidak ada kebocoran kunci privat atau credential pada log output.
