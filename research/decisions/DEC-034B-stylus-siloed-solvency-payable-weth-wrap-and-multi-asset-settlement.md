# DEC-034B: Stylus Smart Contract Siloed Solvency, Payable Native ETH Auto-Wrap, and Multi-Asset Settlement Execution

- **Status:** APPROVED AS ARCHITECTURAL SUB-SPECIFICATION (Child of [`DEC-034`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-034-dual-asset-architecture-usdc-weth-sovereign-commerce-rails.md))
- **Author:** Zeltra Protocol Architecture & Smart Contracts Team
- **Date:** 2026-10-13
- **Applies to:**
  - `zeltra-contracts/src/storage.rs` (Siloed per-asset liabilities and token address mapping)
  - `zeltra-contracts/src/deposit.rs` (Direct ERC-20 deposit and payable native ETH auto-wrap)
  - `zeltra-contracts/src/spend.rs` (Asset-isolated payout execution and zero-caller address resolution)
  - `zeltra-contracts/src/lib.rs` (Entrypoints, asset configuration, and invariant assertions)
  - `zeltra-contracts/src/helpers.rs` (Multi-asset ERC-20 transfer helpers and WETH conversion)
- **Parent & Related DECs:**
  - [`DEC-034`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-034-dual-asset-architecture-usdc-weth-sovereign-commerce-rails.md) (Master Dual-Asset Architecture Blueprint)
  - [`DEC-034A`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-034A-canonical-field-bit-packing-and-intra-asset-circuit-constraints.md) (Canonical Field Bit-Packing & Intra-Asset Circuit Soundness)
  - [`DEC-034C`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-034C-relayer-zero-oracle-reimbursement-and-sdk-multi-asset-wallet.md) (Relayer Zero-Oracle Routing & Multi-Asset Client SDK)
  - [`DEC-009`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-009-liability-invariant-check.md) (Mathematical Solvency Invariant Check)
  - [`DEC-022`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-022-defense-against-proof-settlement-mismatch-boundary-gaps.md) (Semantic Public Input Binding)
  - [`DEC-038B`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-038B-contract-direct-mmr-deposit-and-storage-cleanup.md) (Stylus Contract Direct MMR Deposit Architecture)
- **Architectural Paradigm:** *"Siloed Per-Asset Accounting, Frictionless Payable Auto-Wrap, Immutable Token Resolution"*

---

## 1. Executive Summary & Pertahanan Terhadap Exploit 2024–2026

Implementasi multi-aset pada smart contract Arbitrum Stylus Zeltra dirancang dengan pertahanan ketat terhadap kegagalan historis:

1. **Pertahanan Terhadap TrustedVolumes Exploit (Mei 2026, $5.87M Lost):**
   Pada kasus TrustedVolumes, proxy kontrak menerima alamat token tujuan dari caller. Penyerang mengesahkan hak pada satu token berharga rendah dan menarik WETH/USDC.
   - **Solusi Zeltra:** **Nol Caller Token Injection**. Fungsi `spend_private_note` dan `spend_joinsplit` sama sekali tidak menerima parameter alamat token dari pemanggil. Kontrak Stylus secara eksklusif mengekstrak `asset_id` (0 atau 1) dari `flags_packed` public input Groth16, lalu me-lookup alamat token resmi dari internal immutable storage.
2. **Pertahanan Terhadap Conic Finance Reentrancy (CertiK 2023–2024):**
   Conic mengalami kerugian 1.727 ETH akibat ambiguitas antara native ETH (`0xeee`) dan WETH.
   - **Solusi Zeltra:** **Zero Dual-Accounting**. Seluruh saldo cadangan internal Zeltra untuk native currency 100% dinormalisasi ke alamat **WETH9 kanonikal**. Deposit ETH langsung dibungkus menjadi WETH9 pada instruksi pertama fungsi payable sebelum mencatat liabilitas atau menyentuh accumulator MMR.
3. **Pertahanan Terhadap Pembekuan Circle (Anti-Freeze Isolation):**
   Jika kontrak USDC Circle (`FiatTokenV2`) mem-blacklist alamat Zeltra, seluruh transfer ERC-20 USDC akan revert. Namun, karena kolam WETH memiliki mapping liabilitas dan saldo terisolasi, **kolam WETH tetap dapat beroperasi normal 100% tanpa hambatan**.

---

## 2. Stylus Storage Architecture (`storage.rs`)

Mengikuti pemusnahan total sesi legacy dan liability refund pada [`DEC-038B`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-038B-contract-direct-mmr-deposit-and-storage-cleanup.md), tata letak penyimpanan Stylus berevolusi menjadi:

```rust
// zeltra-contracts/src/storage.rs
sol_storage! {
    #[entrypoint]
    pub struct Zeltra {
        // --- Tata Kelola & Keamanan ---
        address owner;
        address pending_owner;
        bool paused;
        address fee_recipient;
        address execution_fee_recipient;

        // --- Pendaftaran Token Resmi (DEC-034B Curated Pair) ---
        // mapping asset_id (0: WETH9, 1: USDC) => alamat smart contract ERC-20
        mapping(uint256 => address) asset_tokens;
        address weth9_address;

        // --- Multi-Liability Terisolasi Per Asset ID (DEC-034B & DEC-038B) ---
        // 0: WETH (Gwei), 1: USDC (Micro-units)
        mapping(uint256 => uint256) user_note_liabilities;
        mapping(uint256 => uint256) accrued_execution_fee_liabilities;
        mapping(uint256 => uint256) realized_protocol_fees;
        mapping(uint256 => uint256) accumulated_execution_fees;
        mapping(uint256 => uint256) total_deposited_principals;

        // --- Akumulator Komitmen MMR Terpadu (DEC-032) ---
        uint256 mmr_leaf_count;
        bytes32 note_tree_root;
        mapping(uint256 => bytes32) mmr_peaks;
        mapping(bytes32 => uint256) accepted_note_roots;
        mapping(uint256 => bytes32) root_history;
        uint256 root_history_index;

        // --- Nullifier Registry 2-Epoch (DEC-033) ---
        uint256 current_epoch;
        uint256 epoch_start_timestamp;
        mapping(bytes32 => bool) generation_nullifiers;
        mapping(uint256 => uint256) generation_epoch_id;
        mapping(bytes32 => bool) note_nullifiers;
    }
}
```

---

## 3. Protokol Inflow Direct MMR Deposit (`deposit.rs`)

Smart contract menyediakan tiga pintu gerbang inflow atomik tanpa perantara:

```rust
// zeltra-contracts/src/deposit.rs
impl Zeltra {
    /// 1. Direct USDC Deposit (Asset ID 1) — Standar Komersial
    pub fn deposit_direct_note(
        &mut self,
        amount: U256,
        note_commitment: FixedBytes<32>,
        encrypted_memo: Bytes,
    ) -> Result<(), Vec<u8>> {
        self.check_not_paused()?;
        // Min 10 USDC (10_000_000 micro-units)
        if amount < U256::from(10_000_000) {
            return Err(b"AMOUNT_TOO_SMALL".to_vec());
        }
        self._execute_direct_erc20_inflow(1, amount, note_commitment, encrypted_memo)
    }

    /// 2. Direct WETH ERC-20 Deposit (Asset ID 0) — Standar Sovereign ERC-20
    pub fn deposit_direct_weth(
        &mut self,
        amount_wei: U256,
        note_commitment: FixedBytes<32>,
        encrypted_memo: Bytes,
    ) -> Result<(), Vec<u8>> {
        self.check_not_paused()?;
        // Min 0.005 WETH (5_000_000 Gwei = 5 * 10^15 Wei)
        if amount_wei < U256::from(5_000_000_000_000_000u64) {
            return Err(b"AMOUNT_TOO_SMALL".to_vec());
        }
        // Validasi presisi Gwei (wajib habis dibagi 10^9 Wei)
        if amount_wei % U256::from(1_000_000_000) != U256::ZERO {
            return Err(b"WETH_AMOUNT_NOT_GWEI_ALIGNED".to_vec());
        }
        let gwei_amount = amount_wei / U256::from(1_000_000_000);
        self._execute_direct_erc20_inflow(0, gwei_amount, note_commitment, encrypted_memo)
    }

    /// 3. Direct Native ETH Auto-Wrap Deposit (Payable - Asset ID 0) — Produk 10/10 UX (Strict CEI Compliant)
    #[payable]
    pub fn deposit_direct_native_eth(
        &mut self,
        note_commitment: FixedBytes<32>,
        encrypted_memo: Bytes,
    ) -> Result<(), Vec<u8>> {
        // --- 1. CHECKS ---
        self.check_not_paused()?;
        let amount_wei = self.msg_value();

        if amount_wei < U256::from(5_000_000_000_000_000u64) {
            return Err(b"AMOUNT_TOO_SMALL".to_vec());
        }
        if amount_wei % U256::from(1_000_000_000) != U256::ZERO {
            return Err(b"ETH_VALUE_NOT_GWEI_ALIGNED".to_vec());
        }
        if note_commitment == FixedBytes::<32>::ZERO {
            return Err(b"ZERO_COMMITMENT_REJECTED".to_vec());
        }
        if crate::types::from_evm_scalar(&note_commitment.0).is_none() {
            return Err(b"INVALID_NOTE_COMMITMENT_SCALAR".to_vec());
        }

        // --- 2. EFFECTS (State mutations precede external interactions) ---
        let gwei_amount = amount_wei / U256::from(1_000_000_000);
        let cur_liab = self.user_note_liabilities.get(U256::ZERO);
        self.user_note_liabilities.insert(U256::ZERO, cur_liab + gwei_amount);

        let cur_principal = self.total_deposited_principals.get(U256::ZERO);
        self.total_deposited_principals.insert(U256::ZERO, cur_principal + gwei_amount);

        self._mmr_insert(note_commitment, encrypted_memo.to_vec())?;

        // --- 3. INTERACTIONS (External call to canonical WETH9 wrapper) ---
        let weth_addr = self.weth9_address.get();
        if weth_addr == Address::ZERO {
            return Err(b"WETH9_NOT_CONFIGURED".to_vec());
        }
        let iweth = IWeth9::new(weth_addr);
        let host = Self::runtime_host();
        iweth.deposit(&host, Call::new_mutating(self).value(amount_wei))
            .map_err(|_| b"WETH_WRAP_FAILED".to_vec())?;

        // --- 4. SOLVENCY ASSERTION ---
        self.check_asset_solvency_invariant(0)?;
        Ok(())
    }
}
```

---

## 4. Eksekusi Spend & Penyaluran Payout Terisolasi (`spend.rs`)

Pada proses `spend_private_note` atau `spend_joinsplit`:

1. **Ekstraksi Kanonikal `asset_id` dari Public Inputs:**
   - **Pada Private Note Spend (16-PI):**
     $$\text{is\_rollover} = \text{flags\_packed} \ \& \ 0\text{x01}, \quad \text{asset\_id} = (\text{flags\_packed} \gg 1) \ \& \ 0\text{xFF}$$
   - **Pada JoinSplit Spend (19-PI):**
     $$\text{has\_change\_1} = \text{flags\_packed} \ \& \ 0\text{x01}, \quad \text{has\_change\_2} = (\text{flags\_packed} \gg 1) \ \& \ 0\text{x01}$$
     $$\text{asset\_id} = (\text{flags\_packed} \gg 2) \ \& \ 0\text{xFF}$$
   - **Pencegahan Malleability:** Kontrak menolak jika $\text{flags\_packed} \gg 10 \ne 0$ atau $\text{asset\_id} \ge 2$ (`UNSUPPORTED_ASSET_ID`).

2. **Lookup Alamat Token Kanonikal (Anti-TrustedVolumes Exploit):**
   ```rust
   let target_token = self.asset_tokens.get(U256::from(asset_id));
   if target_token == Address::ZERO {
       return Err(b"ASSET_NOT_CONFIGURED".to_vec());
   }
   ```
3. **Penyaluran Payout Berdasarkan Skala Satuan:**
   - Jika $\text{asset\_id} == 1$ (USDC): Nilai `merchant_amount` langsung ditransfer dalam satuan mikro-unit ($10^6$).
   - Jika $\text{asset\_id} == 0$ (WETH): Nilai `merchant_amount` (dalam Gwei) dikalikan $10^9$ untuk mendapatkan Wei sebelum pemanggilan `transfer()`:
     $$\text{wei\_payout} = \text{merchant\_amount} \times 10^9$$
4. **Mutasi Liabilitas Per-Asset:**
   ```rust
   let cur_user_liab = self.user_note_liabilities.get(U256::from(asset_id));
   self.user_note_liabilities.insert(U256::from(asset_id), cur_user_liab - total_debit);
   ```
5. **Akumulasi & Penarikan Execution Fee Relayer Simetris:**
   - Akumulasi fee (`accumulated_execution_fees[asset_id]`) **selalu disimpan dalam basis unit sirkuit** (Gwei untuk WETH, mikro untuk USDC).
   - Saat operator relayer mengeklaim fee via `claim_execution_fees(asset_id, amount_in_circuit_units)`:
     - Jika $\text{asset\_id} == 0$ (WETH): Kontrak mentransfer `amount_in_circuit_units * 10^9` Wei via `IWeth9::transfer()`.
     - Jika $\text{asset\_id} == 1$ (USDC): Kontrak mentransfer `amount_in_circuit_units` micro-units via `IERC20::transfer()`.

---

## 5. Invariant Solvensi Terisolasi Mutlak (Mathematical Proof)

Sebelum dan sesudah setiap mutasi state pada smart contract, invariant solvensi diperiksa per aset:

$$\forall a \in \{0, 1\}: \quad \text{Balance}(a) \ge \text{UserNoteLiabilities}(a) + \text{AccruedExecutionFees}(a) + \text{RealizedProtocolFees}(a)$$

Di mana:
* Untuk $a = 1$ (USDC): Saldo diukur dari `IERC20(usdc).balanceOf(address(this))`.
* Untuk $a = 0$ (WETH): Saldo diukur dari `IERC20(weth9).balanceOf(address(this)) / 10^9` (integer floor). Saldo native ETH mentah diabaikan demi mencegah manipulasi inflasi cadangan via `selfdestruct`.

### Teorema Isolasi Kegagalan (Failure Containment Theorem):
Jika terjadi kondisi di mana penerbit token USDC (Circle) membekukan alamat smart contract Zeltra:
1. Setiap panggilan `spend_private_note` dengan `asset_id = 1` akan memicu revert pada eksekusi `transferFrom` USDC.
2. **Kondisi Kolam WETH (`asset_id = 0`):**
   Karena saldo WETH disimpan dalam kontrak WETH9 (yang tidak memiliki fungsi `blacklist` atau `owner`), dan liabilitas WETH disimpan pada storage slot terpisah (`asset_id = 0`), **seluruh transaksi deposit, verifikasi ZK, dan spend WETH akan berjalan 100% normal tanpa terpengaruh sedikit pun**.

---

## 6. Matriks Pengujian Formal Smart Contract

1. `test_weth_payable_wrap_success_cei_order`: Pengguna mengirim native ETH, efek internal tercatat sebelum interaksi eksternal WETH9, saldo WETH9 kontrak bertambah, komitmen masuk ke MMR, dan liabilitas WETH terupdate.
2. `test_unaligned_gwei_wei_rejection`: Pengiriman nilai ETH yang tidak habis dibagi $10^9$ wei memicu revert `ETH_VALUE_NOT_GWEI_ALIGNED`.
3. `test_caller_token_injection_impossible`: Pembuktian bahwa pemanggil fungsi spend tidak dapat mengarahkan transfer ke token lain di luar registry internal.
4. `test_weth_relayer_fee_claim_scaling_exactness`: Klaim fee WETH berdenominasi Gwei mentransfer Wei dalam jumlah tepat ($10^9 \times \text{fee}$) ke relayer tanpa pemotongan presisi.
5. `test_flags_packed_malleable_bits_rejection`: Pengiriman `flags_packed` dengan bit kotor di atas bit alokasi ditolak on-chain (`FLAGS_PACKED_MALLEABLE_BITS`).
6. `test_usdc_revert_leaves_weth_solvent`: Simulasi mock revert pada transfer USDC membuktikan kolam WETH tetap dapat memproses transfer tanpa gagal solvensi.
