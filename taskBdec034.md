# TASK B (DEC-034B): Smart Contract Stylus Arbitrum (`zeltra-contracts`)

> **Scope:** `zeltra-contracts/` (Arbitrum Stylus WASM, EIP-2537 Precompiles, Solvency Invariants)  
> **Spesifikasi Referensi:** [`research/decisions/DEC-034B-stylus-siloed-solvency-payable-weth-wrap-and-multi-asset-settlement.md`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-034B-stylus-siloed-solvency-payable-weth-wrap-and-multi-asset-settlement.md) & [`research/decisions/DEC-034-dual-asset-architecture-usdc-weth-sovereign-commerce-rails.md`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-034-dual-asset-architecture-usdc-weth-sovereign-commerce-rails.md)  
> **Prinsip:** *"Siloed Per-Asset Accounting, Frictionless Payable Auto-Wrap, Immutable Token Resolution, Strict CEI"*

---

## 1. Latar Belakang & Tujuan Arsitektur
Mengimplementasikan akuntansi multi-aset terlindung pada smart contract Arbitrum Stylus Zeltra:
1. **Nol Caller Token Injection:** Pemanggil `spend_*` dilarang menginjeksi alamat token; kontrak me-resolve alamat token terdaftar dari storage immutable berdasarkan `asset_id` yang dibuktikan oleh sirkuit Groth16.
2. **Zero Dual-Accounting Native ETH vs WETH:** Saldo cadangan dinormalisasi 100% ke WETH9 kanonikal. Deposit native ETH langsung dibungkus (*auto-wrapped*) pada instruksi pertama fungsi payable.
3. **Failure Containment Theorem:** Pembekuan kontrak USDC oleh Circle tidak boleh memengaruhi operasional kolam WETH.

---

## 2. Checklist Implementasi Rinci

### A. Tata Letak Penyimpanan Stylus (`zeltra-contracts/src/storage.rs`)
- [ ] **Pendaftaran Token Resmi:**
  - Tambahkan `asset_tokens: mapping(uint256 => address)` (0: WETH9, 1: USDC).
  - Tambahkan `weth9_address: address`.
- [ ] **Multi-Liability Terisolasi Per Asset ID (Unifikasi Nama Kanonikal - Patch Audit #8):**
  - `user_note_liabilities: mapping(uint256 => uint256)`
  - `accrued_execution_fee_liabilities: mapping(uint256 => uint256)` (**HAPUS** duplikasi `accumulated_execution_fees`!)
  - `realized_protocol_fees: mapping(uint256 => uint256)`
  - `total_deposited_principals: mapping(uint256 => uint256)`

### B. Protokol Inflow Direct MMR Deposit (`zeltra-contracts/src/deposit.rs`)
- [ ] **1. Direct USDC Deposit (`deposit_direct_note`):**
  - Input: `amount_micro: U256`, `note_commitment: FixedBytes<32>`, `encrypted_memo: Bytes`.
  - Cek minimum 10 USDC (`10_000_000 micro-units`) dan `amount_micro <= u64::MAX`.
  - Teruskan ke `_execute_direct_erc20_inflow(1, amount_micro, amount_micro, ...)`.
- [ ] **2. Direct WETH ERC-20 Deposit (`deposit_direct_weth`):**
  - Input: `amount_wei: U256`, `note_commitment: FixedBytes<32>`, `encrypted_memo: Bytes`.
  - Cek minimum 0.005 WETH ($5 \times 10^{15}$ Wei = $5{,}000{,}000$ Gwei).
  - Validasi presisi Gwei: `amount_wei % 1_000_000_000 == 0`.
  - Hitung `amount_gwei = amount_wei / 1_000_000_000`. Cek `amount_gwei <= u64::MAX`.
  - **PENTING (Patch Audit #3):** Oper `amount_wei` sebagai `raw_erc20_amount` dan `amount_gwei` sebagai `norm_liability_amount`!
- [ ] **3. Helper Inflow ERC-20 (`_execute_direct_erc20_inflow`) — Strict CEI:**
  - Signature:
    ```rust
    fn _execute_direct_erc20_inflow(
        &mut self,
        asset_id: u8,
        raw_erc20_amount: U256,
        norm_liability_amount: U256,
        note_commitment: FixedBytes<32>,
        encrypted_memo: Bytes,
    ) -> Result<(), Vec<u8>>
    ```
  - **CHECKS:**
    - `encrypted_memo.len() == 96`.
    - `note_commitment != ZERO` & kanonikal scalar (`from_evm_scalar < r`).
    - Resolve `token_addr = self.asset_tokens.get(asset_id) != Address::ZERO`.
    - *(Catatan Pengikatan Komitmen - Patch Audit #4):* Pastikan validasi bahwa `value_packed` di dalam commitment terikat pada `norm_liability_amount` dan `asset_id`.
  - **EFFECTS:**
    - Tambah `user_note_liabilities[asset_id] += norm_liability_amount`.
    - Tambah `total_deposited_principals[asset_id] += norm_liability_amount`.
    - Sisipkan leaf ke MMR: `self._mmr_insert(note_commitment, encrypted_memo.to_vec())`.
  - **INTERACTIONS:**
    - Tarik token: `self.safe_transfer_from(token_addr, sender, this_contract, raw_erc20_amount)`.
  - **SOLVENCY CHECK:**
    - Jalankan `self.check_asset_solvency_invariant(asset_id)`.
- [ ] **4. Direct Native ETH Auto-Wrap Deposit (`deposit_direct_native_eth`) — #[payable]:**
  - Baca `amount_wei = self.msg_value()`.
  - Cek min 0.005 ETH & habis dibagi $10^9$ Wei.
  - Catat liabilitas internal dalam Gwei (`amount_wei / 10^9`).
  - Sisipkan leaf ke MMR (`_mmr_insert`).
  - Interaksi eksternal: Panggil `IWeth9::deposit{value: amount_wei}()` ke alamat `weth9_address`.
  - Jalankan `self.check_asset_solvency_invariant(0)`.

### C. Eksekusi Spend & Penyaluran Payout (`zeltra-contracts/src/spend.rs`)
- [ ] **1. Ekstraksi Kanonikal & Pemeriksaan Batas Bit Ketat (Patch Audit #2):**
  - **Pada Private Note Spend (16-PI, Slot 15):**
    - `is_rollover = flags_packed & 0x01`
    - `asset_id = (flags_packed >> 1) & 0x01`
    - **Strict Bounds Check:** `flags_packed <= 3` (revert `FLAGS_PACKED_MALLEABLE_BITS` jika $> 3$).
  - **Pada JoinSplit Spend (19-PI, Slot 17):**
    - `has_change_1 = flags_packed & 0x01`
    - `has_change_2 = (flags_packed >> 1) & 0x01`
    - `asset_id = (flags_packed >> 2) & 0x01`
    - **Strict Bounds Check:** `flags_packed <= 7` (revert `FLAGS_PACKED_MALLEABLE_BITS` jika $> 7$).
- [ ] **2. Lookup Token & Penyaluran Dana Berdasarkan Skala:**
  - Ambil alamat token dari storage: `self.asset_tokens.get(asset_id)`.
  - Jika $\text{asset\_id} == 1$ (USDC): Payout `merchant_amount` dalam mikro-unit.
  - Jika $\text{asset\_id} == 0$ (WETH): Payout `merchant_amount * 10^9` Wei via `IWeth9::transfer()`.
- [ ] **3. Mutasi Liabilitas & Fee Akrual (Unifikasi Storage):**
  - Kurangi `user_note_liabilities[asset_id] -= total_debit`.
  - Tambah `accrued_execution_fee_liabilities[asset_id] += execution_fee` (disimpan dalam basis unit sirkuit: Gwei atau Micro).
  - Tambah `realized_protocol_fees[asset_id] += protocol_fee`.
- [ ] **4. Klaim Fee Eksekusi Relayer (`claim_execution_fees`):**
  - Input: `asset_id: u8`, `amount_in_circuit_units: U256`.
  - Validasi pemanggil adalah `execution_fee_recipient`.
  - Jika WETH: transfer `amount_in_circuit_units * 10^9` Wei via `IWeth9::transfer()`.
  - Jika USDC: transfer `amount_in_circuit_units` micro-units via `IERC20::transfer()`.
  - Kurangi `accrued_execution_fee_liabilities[asset_id] -= amount_in_circuit_units`.

### D. Solvensi Matematika Terisolasi (`zeltra-contracts/src/vault.rs` / `lib.rs`)
- [ ] Implementasikan `check_asset_solvency_invariant(asset_id: u8)`:
  $$\text{Balance}(a) \ge \text{user\_note\_liabilities}(a) + \text{accrued\_execution\_fee\_liabilities}(a) + \text{realized\_protocol\_fees}(a)$$
  - Jika $a = 1$ (USDC): `Balance` dari `IERC20(usdc).balanceOf(this)`.
  - Jika $a = 0$ (WETH): `Balance` dari `IWeth9(weth9).balanceOf(this) / 10^9` (integer floor). Saldo native ETH mentah diabaikan demi mencegah manipulasi inflasi cadangan via `selfdestruct`.

---

## 3. Matriks Pengujian Wajib (Unit & Integration Tests)
Jalankan dan pastikan 100% lulus:
1. `test_weth_payable_wrap_success_cei_order`: Pengguna mengirim native ETH, efek internal tercatat sebelum interaksi WETH9, saldo WETH9 kontrak bertambah, komitmen masuk ke MMR, dan liabilitas WETH terupdate.
2. `test_unaligned_gwei_wei_rejection`: Pengiriman nilai ETH yang tidak habis dibagi $10^9$ wei memicu revert `ETH_VALUE_NOT_GWEI_ALIGNED`.
3. `test_caller_token_injection_impossible`: Pembuktian bahwa pemanggil fungsi spend tidak dapat mengarahkan transfer ke token lain di luar registry internal.
4. `test_weth_relayer_fee_claim_scaling_exactness`: Klaim fee WETH berdenominasi Gwei mentransfer Wei dalam jumlah tepat ($10^9 \times \text{fee}$) ke relayer tanpa pemotongan presisi.
5. `test_flags_packed_malleable_bits_rejection`: Pengiriman `flags_packed > 3` pada 16-PI atau $> 7$ pada 19-PI ditolak on-chain (`FLAGS_PACKED_MALLEABLE_BITS`).
6. `test_usdc_revert_leaves_weth_solvent`: Simulasi mock revert pada transfer USDC membuktikan kolam WETH tetap dapat memproses transfer tanpa gagal solvensi.

---

## 4. Definisi Selesai (Definition of Done)
- `cargo stylus check` dari `zeltra-contracts/` lulus tanpa error ABI/storage.
- Seluruh pengujian kontrak lulus: `cargo test -p zeltra-contracts`.
- Tidak ada hardcoded address atau credential.
