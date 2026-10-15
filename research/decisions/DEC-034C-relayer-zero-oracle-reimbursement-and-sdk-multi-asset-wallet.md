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

### A. Endpoint Kuotasi Fee Dual-Asset (`/api/v1/quote`) & Volatility Protection
Relayer menyediakan kuotasi fee eksekusi gas bertanda tangan EIP-712 dengan batas masa berlaku ketat:

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
    pub expiry: u64,                // Maksimum 20 detik (anti-volatility free option)
    pub relayer_signature: String,  // EIP-712 Signature
}
```

#### Parameter Perlindungan Anti-Arbitrase & Lonjakan Gas (Gas-Spike Guard):
1. **Short Expiry Duration:** Masa berlaku quote USDC dibatasi maksimal **20 detik** (~80 blok Arbitrum) untuk mencegah pengguna mengeksploitasi quote lama saat gas Arbitrum melonjak.
2. **Dynamic Gas Spike Guard:** Jika base fee L2 saat broadcast melonjak $> 1.5\times$ dari fee pada quote tertandatangani, relayer secara otomatis menolak transaksi tanpa penalti SLA untuk mencegah *drainage of relayer operational capital*.

#### Pengikatan `asset_id` ke dalam Preimage EIP-712 `quote_hash`:
TypeHash EIP-712 untuk kuotasi relayer **WAJIB** menyertakan `asset_id` di dalam struktur yang ditandatangani dan di-hash menjadi `quote_hash`:

```solidity
bytes32 constant EXECUTION_QUOTE_TYPEHASH = keccak256(
    "ExecutionQuote(uint8 assetId,address targetContract,uint64 baseGasFee,uint64 relayerMarkupFee,uint64 totalExecutionFee,uint64 expiry,uint256 chainId)"
);
```

Dengan demikian, `quote_hash_hi` dan `quote_hash_lo` pada Public Inputs sirkuit secara kriptografis mengikat `asset_id` dari quote tersebut, mencegah manipulasi silang metadata aset pada payload JSON request tanpa membatalkan tanda tangan EIP-712 relayer.

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

### A. Penyimpanan Note Terpartisi & Trial Decryption (Big-Endian DEC-037)
Untuk mencegah kontaminasi saldo, `PrivateNoteWallet` mengisolasi note berdasarkan `asset_id`.

```rust
// zeltra-sdk/src/wallet/note_wallet.rs
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PrivateNoteV1 {
    pub value: u64,        // Gwei (jika asset_id == 0) atau Micro-USDC (jika asset_id == 1)
    pub asset_id: u8,      // 0: WETH, 1: USDC (WAJIB disimpan untuk perhitungan value_packed!)
    pub owner_key: Fr,     // Public owner key (bukan raw spending_key)
    pub rho: Fr,
    pub randomness: Fr,
}

impl PrivateNoteV1 {
    /// Menghitung value_packed kanonikal sesuai DEC-034A: value + asset_id * 2^64
    pub fn value_packed(&self) -> Fr {
        let two_pow_64 = Fr::from(1u128 << 64);
        Fr::from(self.value) + (Fr::from(self.asset_id as u64) * two_pow_64)
    }

    /// Menghitung komitmen leaf Poseidon-W5
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

    /// Deserialisasi memo on-chain kanonikal Big-Endian (DEC-037)
    pub fn process_decrypted_plaintext(
        &mut self,
        plaintext_bytes: &[u8; 48],
        shared_secret: &[u8; 32],
    ) -> Result<(), WalletError> {
        let value = u64::from_be_bytes(plaintext_bytes[0..8].try_into().unwrap());
        let rho = crate::from_evm_scalar(&plaintext_bytes[8..40].try_into().unwrap())
            .ok_or(WalletError::NonCanonicalRhoScalar)?;
        let asset_id_u64 = u64::from_be_bytes(plaintext_bytes[40..48].try_into().unwrap());

        if asset_id_u64 > 1 {
            return Err(WalletError::UnsupportedAssetId(asset_id_u64 as u8));
        }
        let asset_id = asset_id_u64 as u8;

        // Randomness diturunkan dari shared_secret ECDH & rho (sesuai DEC-037)
        let randomness = derive_note_randomness(shared_secret, &rho);
        let owner_key = self.incoming_viewing_key.owner_key();
        let note = PrivateNoteV1 {
            value,
            asset_id,
            owner_key,
            rho,
            randomness,
        };

        if asset_id == 0 {
            self.weth_notes.push(note);
        } else {
            self.usdc_notes.push(note);
        }

        Ok(())
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

* **Partisi Total:**
  - Saat membayar merchant USDC: Solver hanya memeriksa dan memilih note dari `usdc_notes`.
  - Saat mentransfer dana WETH: Solver hanya memilih note dari `weth_notes`.
  - **Zero Mixed-Asset Transactions:** Sistem menolak pemilihan note campuran (1 WETH + 1 USDC) di tingkat SDK sebelum perakitan sirkuit ZK.
* **Perhitungan Fee Ceil Division (Anti-Dust Truncation):**
  Untuk mencegah pembulatan ke bawah yang merugikan solvabilitas protokol, protocol fee 45 bps dihitung dengan pembagian ke atas (*ceiling division*):
  $$\text{protocol\_fee} = \left\lceil \frac{\text{merchant\_amount} \times 45}{10000} \right\rceil = \frac{\text{merchant\_amount} \times 45 + 9999}{10000}$$

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
2. `test_relayer_quote_expiry_strict_window`: Permintaan spend dengan quote yang telah berumur lebih dari 20 detik ditolak seketika oleh relayer.
3. `test_knapsack_rejects_mixed_assets`: Upaya memilih koin gabungan WETH dan USDC ditolak secara instan oleh solver coin selection.
4. `test_relayer_rejects_cross_asset_quote_tampering`: Permintaan spend note WETH menggunakan quote USDC ditolak pada preflight check (`CrossAssetFeeMismatch`).
5. `test_sdk_trial_decryption_partitions_notes_correctly`: Deserialisasi memo log on-chain secara otomatis mengelompokkan note masuk ke sub-wallet yang sesuai via format kanonikal Big-Endian.
6. `test_knapsack_ceiling_fee_calculation`: Pembayaran dengan pecahan kecil (baik WETH maupun USDC) membuktikan nilai fee protokol tidak pernah terpotong ke nol.
7. `test_eip712_quote_hash_binds_asset_id`: Memastikan manipulasi `asset_id` pada permintaan spend membatalkan verifikasi tanda tangan EIP-712 relayer dan quote hash sirkuit.

