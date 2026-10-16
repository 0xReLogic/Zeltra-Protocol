# TASK B (DEC-043B): Stylus Smart Contract Strict Three-Mode Settlement & Dual-Memo MMR (`zeltra-contracts`)

> **Scope:** `zeltra-contracts/` (Arbitrum Stylus WASM, EIP-2537 Precompiles, MMR Accumulator, CEI & Invariant Checks)  
> **Spesifikasi Referensi:** [`research/decisions/DEC-043B-stylus-strict-three-mode-settlement-and-dual-memo-mmr.md`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-043B-stylus-strict-three-mode-settlement-and-dual-memo-mmr.md) & [`research/decisions/DEC-043-shielded-to-shielded-in-pool-transfer-and-merchant-masking.md`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-043-shielded-to-shielded-in-pool-transfer-and-merchant-masking.md)  
> **Prinsip:** *"Strict State Segregation, Anti-Silent-Burn, Fixed-Length Calldata Verification, Perfect Solvency Conservation"*  
> **Status:** **COMPLETED** (75/75 Tests Passed, 100% Soundness, Clean Clippy & Fmt)

---

## 1. Latar Belakang & Tujuan Arsitektur

Menerapkan penyelesaian on-chain untuk shielded-to-shielded in-pool transfer di smart contract Stylus WASM:
1. **Strict 3-Mode Mutually Exclusive State Machine:**
   Memisahkan secara kaku antara Mode 1 (Epoch Rollover), Mode 2 (In-Pool Transfer), dan Mode 3 (Public Outflow Spend) untuk mencegah bahaya *Silent Fund Burn* di mana liabilitas pengguna terdebit tanpa ada token ERC-20 yang ditransfer keluar (Resolusi AUD-4).
2. **Fixed-Length 96-Byte Calldata Guard:**
   Memastikan parameter calldata `encrypted_memo_1` dan `encrypted_memo_2` tepat berukuran 96 byte, melindungi relayer dari *L1 Data Gas Griefing* dan mencegah *Silent Memo Drop* (Resolusi AUD-5).
3. **Dual-Memo MMR Ingestion & ERC-20 Bypass:**
   Menyisipkan kedua komitmen output ke Merkle Mountain Range (`_mmr_insert`) bersama memo AEAD 96 byte, dan melewati (*bypass*) interaksi eksternal transfer token ERC-20 pada Mode 2.

---

## 2. Checklist Implementasi Rinci

### A. Modifikasi Entrypoint Kontrak (`zeltra-contracts/src/lib.rs`)
- [x] **Pembaruan Interface `spend_joinsplit`:**
  - Tambahkan parameter `encrypted_memo_1: Bytes, encrypted_memo_2: Bytes` ke entrypoint publik `spend_joinsplit`.
  - Teruskan kedua memo ke pemanggilan fungsi internal `self._spend_joinsplit(...)`.

### B. Implementasi Strict 3-Mode State Machine (`zeltra-contracts/src/spend.rs`)
- [x] **Pemisahan Mode Transaksi:**
  - **Mode 1 (Rollover):** `is_rollover == 1`. Wajib `merchant_amount == 0`, `protocol_fee == 0`, `recipient == Address::ZERO`. Revert jika salah satu dilanggar.
  - **Mode 2 (In-Pool Transfer):** `is_rollover == 0 && merchant_amount == 0`. Wajib `has_change_1 == 1`, `protocol_fee == 0`, `recipient == Address::ZERO`. Revert `b"IN_POOL_TRANSFER_REQUIRES_OUTPUT_1"`, `b"PROTOCOL_FEE_MUST_BE_ZERO_FOR_IN_POOL_TRANSFER"`, atau `b"RECIPIENT_MUST_BE_ZERO_FOR_IN_POOL_TRANSFER"`.
  - **Mode 3 (Public Outflow Spend):** `is_rollover == 0 && merchant_amount > 0`. Wajib `recipient != Address::ZERO`. Revert `b"ZERO_RECIPIENT_ON_PUBLIC_SPEND"` jika `recipient == Address::ZERO` (Anti-Silent-Burn!).
- [x] **Validasi Panjang Memo Tepat 96 Byte:**
  - Tegakkan guard:
    ```rust
    if encrypted_memo_1.len() != 96 || encrypted_memo_2.len() != 96 {
        return Err(b"INVALID_MEMO_LENGTH".to_vec());
    }
    ```
- [x] **Penyisipan Dual Memo ke MMR:**
  - Gantikan `Vec::new()` pada baris 565–566 dengan memo riil:
    ```rust
    self._mmr_insert(output_cm_1, encrypted_memo_1.to_vec())?;
    self._mmr_insert(output_cm_2, encrypted_memo_2.to_vec())?;
    ```
- [x] **Bypass Transfer ERC-20:**
  - Pastikan blok interaksi transfer token ERC-20 hanya dieksekusi pada Mode 3:
    ```rust
    if is_rollover == U256::ZERO && merchant_amount > U256::ZERO && recipient != Address::ZERO {
        // execute asset_safe_transfer
    }
    ```

---

## 3. Matriks Pengujian Unit & Soundness (`zeltra-contracts/src/spend.rs`)

Sesuai aturan engineering Zeltra: **1 Positive Test + Minimal 2x Negative Tests**:

### Positive Tests:
- [x] `test_spend_joinsplit_in_pool_usdc_success`: Eksekusi Mode 2 untuk USDC dengan dual memo 96 byte. MMR bertambah 2 daun, nullifier terbakar, saldo kas kontrak utuh, liabilitas terdebit tepat sebesar execution fee, dan invariant solvensi terpenuhi.
- [x] `test_spend_joinsplit_in_pool_weth_success`: Eksekusi Mode 2 untuk WETH dengan dual memo 96 byte.

### Negative Tests (Minimal 2x Positive = Minimal 4 Tests):
- [x] `test_in_pool_rejects_non_zero_recipient`: Mode 2 me-revert jika menyertakan alamat penerima non-zero.
- [x] `test_in_pool_rejects_non_zero_protocol_fee`: Mode 2 me-revert jika `protocol_fee > 0`.
- [x] `test_in_pool_rejects_missing_output_1`: Mode 2 me-revert jika `has_change_1 == 0`.
- [x] `test_public_spend_rejects_zero_recipient`: Mode 3 (`merchant_amount > 0`) me-revert jika `recipient == Address::ZERO` (mencegah silent fund burn).
- [x] `test_spend_joinsplit_rejects_invalid_memo_length`: Me-revert jika memo 1 atau memo 2 tidak berukuran tepat 96 byte.

---

## 4. Strict Engineering Guardrails (Zero-Slop Policy)

1. **NO PLACEHOLDERS:** Dilarang menggunakan `todo!()`, `unimplemented!()`, dummy byte arrays, atau mock proofs di jalur production.
2. **COMPILE-CLEAN:** Wajib lulus `cargo check -p zeltra-contracts` dan `cargo test -p zeltra-contracts` dengan 0 error dan 0 warning.
3. **FORMAT & CLIPPY:** Wajib bersih `cargo fmt --check` dan `cargo clippy -p zeltra-contracts -- -D warnings`.
