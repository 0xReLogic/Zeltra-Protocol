# DEC-034C: Relayer Zero-Oracle Gas Reimbursement, Multi-Asset Client Wallet, and CLI Tooling Specification

- **Status:** APPROVED AS ARCHITECTURAL SUB-SPECIFICATION (Child of [`DEC-034`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-034-dual-asset-architecture-usdc-weth-sovereign-commerce-rails.md))
- **Author:** Zeltra Protocol Architecture, Relayer & SDK Infrastructure Team
- **Date:** 2026-10-13
- **Applies to:**
  - `zeltra-node/src/handlers/quote.rs` (Dual-asset signed EIP-712 gas fee quotes)
  - `zeltra-node/src/handlers/spend.rs` (Zero-oracle spend ingress and dual-asset settlement routing)
  - `zeltra-node/src/validation.rs` (Preflight `asset_id` consistency checks and anti-drain filters)
  - `zeltra-sdk/src/wallet/note_wallet.rs` (Partitioned multi-asset private note store and balance queries)
  - `zeltra-sdk/src/coin_selection.rs` (Stochastic Knapsack partitioned by `asset_id`)
  - `zeltra-cli/src/commands/zk.rs` (CLI commands for dual-asset deposit, spend, and viewing key export)
- **Parent & Related DECs:**
  - [`DEC-034`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-034-dual-asset-architecture-usdc-weth-sovereign-commerce-rails.md) (Master Dual-Asset Architecture Blueprint)
  - [`DEC-034A`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-034A-canonical-field-bit-packing-and-intra-asset-circuit-constraints.md) (Canonical Field Bit-Packing & Intra-Asset Circuit Soundness)
  - [`DEC-034B`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-034B-stylus-siloed-solvency-payable-weth-wrap-and-multi-asset-settlement.md) (Stylus Smart Contract Siloed Solvency & Payable Auto-Wrap)
  - [`DEC-026`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-026-receiver-enforced-compliance-zero-cost-sanctions-filtering-relayer-protection.md) (Receiver-Enforced Compliance & Relayer Protection)
  - [`DEC-031`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-031-relayer-reconciliation-solvency-observability-quote-domain-binding.md) (Relayer Solvency Observability & Signed Quote Binding)
  - [`DEC-036B`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-036B-client-sdk-knapsack-coin-selection-consolidation-and-bulk-sync.md) (Client SDK Knapsack Coin Selection & 2PC Safe Store)
  - [`DEC-038C`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-038C-relayer-sequencer-sdk-direct-deposit-and-cli.md) (Stateless Relayer Sequencer & Direct Deposit SDK)
- **Architectural Paradigm:** *"Zero-Oracle Economic Immunity, Strictly Partitioned Coin Selection, Transparent Multi-Asset UX"*

---

## 1. Executive Summary & Masalah Ketergantungan Price Oracle

Dalam sistem pembayaran multi-aset tradisional, relayer sering kali terpaksa mengintegrasikan **Price Oracle eksternal (Chainlink / Pyth / Uniswap TWAP)** untuk mengonversi biaya gas L2 (yang berbasis ETH) ke dalam token yang dibelanjakan oleh pengguna.

### Risiko Fatal Penggunaan Oracle:
1. **Manipulasi Flash Loan & MEV Sandwich:** Penyerang dapat memanipulasi pool likuiditas DEX untuk membuat relayer mengutip fee gas yang terlalu murah, mengakibatkan *toxic debt* dan kerugian modal bagi operator relayer.
2. **Latensi & Oracle Staleness:** Jika data feed oracle terhenti atau mengalami *staleness* saat volatilitas pasar tinggi, relayer terpaksa mematikan layanan (*service outage*).
3. **Risiko Inventaris Token Illiquid:** Jika relayer menerima pembayaran gas dalam puluhan jenis token, relayer menanggung risiko depresiasi nilai sebelum sempat menukarkannya kembali ke ETH.

**Solusi Zero-Oracle Zeltra (DEC-034C):**
Zeltra membatasi ekosistem **hanya pada WETH dan USDC**, yang merupakan mata uang gas dan likuiditas dasar Arbitrum:
* **Pada Belanja WETH:** Fee eksekusi dipotong langsung dalam **WETH**. Karena WETH bernilai 1:1 terhadap native ETH gas Arbitrum, relayer **tidak membutuhkan price oracle sama sekali**!
* **Pada Belanja USDC:** Fee eksekusi dikuotasi dalam **USDC** menggunakan harga gas L2 riil yang dipatok langsung oleh relayer via tanda tangan EIP-712 yang mengikat ([`DEC-031`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-031-relayer-reconciliation-solvency-observability-quote-domain-binding.md)).

---

## 2. Arsitektur Relayer Node (`zeltra-node`)

### A. Endpoint Kuotasi Fee Dual-Asset (`/api/v1/quote`)
Relayer menyediakan kuotasi fee eksekusi gas bertanda tangan EIP-712:

```rust
// zeltra-node/src/dto.rs
#[derive(Debug, Deserialize, Serialize)]
pub struct ExecutionQuoteRequest {
    pub asset_id: u8,               // 0: WETH, 1: USDC
    pub target_contract: String,
    pub estimated_gas_limit: u64,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ExecutionQuoteResponse {
    pub asset_id: u8,
    pub base_gas_fee: u64,
    pub relayer_markup_fee: u64,    // Flat 15% markup (DEC-031)
    pub total_execution_fee: u64,   // Satuan Gwei (WETH) atau Mikro (USDC)
    pub quote_hash: String,
    pub expiry: u64,
    pub relayer_signature: String,  // EIP-712 Signature
}
```

### B. Preflight Validation Guard (Anti-Drain Filter)
Sebelum membroadcast transaksi ke smart contract Stylus, relayer memeriksa validitas transaksi secara fail-closed:

```rust
// zeltra-node/src/validation.rs
pub fn validate_multi_asset_preflight(
    req: &SpendRequest,
    cached_quote: &SignedQuote,
) -> Result<(), IngressError> {
    // 1. Validasi Canonical Asset ID (0 atau 1)
    if req.asset_id >= 2 {
        return Err(IngressError::UnsupportedAssetId(req.asset_id));
    }

    // 2. Anti-Cross-Asset Fee Drain
    // Quote aset wajib persis sama dengan aset note yang dibelanjakan
    if req.asset_id != cached_quote.asset_id {
        return Err(IngressError::CrossAssetFeeMismatch);
    }

    // 3. Verifikasi Groth16 Proof Lokal (Fail-Closed CPU Preflight DEC-026 C-01)
    verify_groth16_proof_locally(&req.proof, &req.public_inputs)?;

    Ok(())
}
```

---

## 3. Client SDK & Private Note Wallet (`zeltra-sdk`)

### A. Penyimpanan Note Terpartisi (Partitioned UTXO Store)
Untuk mencegah kontaminasi saldo, `PrivateNoteWallet` mengisolasi note berdasarkan `asset_id`:

```rust
// zeltra-sdk/src/wallet/note_wallet.rs
pub struct PrivateNoteWallet {
    // 2PC Encrypted Note Stores terpartisi
    usdc_notes: Vec<PrivateNoteV1>, // asset_id = 1 (6 desimal)
    weth_notes: Vec<PrivateNoteV1>, // asset_id = 0 (9 desimal - Gwei)
    incoming_viewing_key: IncomingViewingKey,
    spending_key: Fr,
}

impl PrivateNoteWallet {
    /// Mengambil saldo total aktif per jenis aset
    pub fn get_balance(&self, asset_id: u8) -> Result<u64, WalletError> {
        match asset_id {
            0 => Ok(self.weth_notes.iter().map(|n| n.value).sum()), // Saldo dalam Gwei
            1 => Ok(self.usdc_notes.iter().map(|n| n.value).sum()), // Saldo dalam Mikro USDC
            _ => Err(WalletError::UnsupportedAssetId(asset_id)),
        }
    }

    /// Helper konversi tampilan ke unit ramah manusia
    pub fn get_human_balance(&self, asset_id: u8) -> Result<String, WalletError> {
        let raw = self.get_balance(asset_id)?;
        match asset_id {
            0 => Ok(format!("{:.6} ETH", raw as f64 / 1_000_000_000.0)),
            1 => Ok(format!("${:.2} USDC", raw as f64 / 1_000_000.0)),
            _ => Err(WalletError::UnsupportedAssetId(asset_id)),
        }
    }
}
```

### B. Algoritma Stochastic Knapsack Coin Selection Terpartisi
Mengikuti invariant strict intra-asset equality, algoritma pemilihan koin Knapsack ([`DEC-036B`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-036B-client-sdk-knapsack-coin-selection-consolidation-and-bulk-sync.md)) berjalan secara terisolasi pada kolam aset yang relevan:

* Saat pengguna membayar merchant USDC: Algoritma hanya memeriksa dan memilih note dari `usdc_notes`.
* Saat pengguna mentransfer dana WETH: Algoritma hanya memilih note dari `weth_notes`.
* **Zero Mixed-Asset Transactions:** Sistem menolak keras pemilihan note campuran (1 WETH + 1 USDC) di tingkat SDK sebelum pembuatan sirkuit ZK dimulai.

---

## 4. Zeltra CLI Tooling (`zeltra-cli`)

Perintah CLI Zeltra diperluas dengan parameter `--asset`:

### A. Membuat Deposit Note
```bash
# 1. Deposit USDC (Standar Komersial)
zeltra-cli create-deposit-note --amount 50.00 --asset usdc --recipient 0x1234...

# 2. Deposit Native ETH (Auto-Wrap Sovereign Rail)
zeltra-cli create-deposit-note --amount 0.05 --asset eth --recipient 0x1234...
```

### B. Membelanjakan Private Note
```bash
# 1. Spend Note USDC
zeltra-cli spend-private-note --amount 25.00 --asset usdc --to 0xMerchant...

# 2. Spend Note WETH
zeltra-cli spend-private-note --amount 0.02 --asset weth --to 0xMachineAgent...
```

### C. Inspeksi Saldo Privat
```bash
zeltra-cli balance
# Output:
# ══════════════════════════════════════════════════════════
# Zeltra Shielded Private Balances:
# • WETH Rail (Sovereign) : 0.030000 ETH (30,000,000 Gwei) [2 unspent notes]
# • USDC Rail (Commerce)  : $25.00 USDC (25,000,000 micro) [1 unspent note]
# ══════════════════════════════════════════════════════════
```

---

## 5. Matriks Pengujian End-to-End

1. `test_relayer_quote_dual_asset`: Kuotasi fee eksekusi WETH diterbitkan dalam satuan Gwei dan USDC dalam mikro-unit dengan tanda tangan EIP-712 yang valid.
2. `test_knapsack_rejects_mixed_assets`: Upaya memilih koin gabungan WETH dan USDC ditolak secara instan oleh solver coin selection.
3. `test_relayer_rejects_cross_asset_quote_tampering`: Permintaan spend note WETH menggunakan quote USDC ditolak pada preflight check (`CrossAssetFeeMismatch`).
4. `test_sdk_trial_decryption_partitions_notes_correctly`: Deserialisasi memo log on-chain secara otomatis mengelompokkan note masuk ke sub-wallet yang sesuai.
