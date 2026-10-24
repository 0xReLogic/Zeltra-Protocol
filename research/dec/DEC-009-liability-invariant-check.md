# DEC-009: Liability Invariant Check

Date: 2026-06-07

## Masalah

Contract tidak memiliki verifikasi otomatis untuk memastikan aset contract selalu cukup
untuk menutup liability yang outstanding. Tanpa invariant check, ada potensi state
corruption di mana contract balance kurang dari total principal yang harus dibayar,
yang bisa menyebabkan insolvency tidak terdeteksi sampai gagal saat withdrawal.

## Invariant bisnis/security

- Contract USDC balance harus selalu >= total_deposited_principal (outstanding liabilities)
- Invariant ini harus dicek setelah setiap state-changing transaction: deposit, spend, dan refund
- Jika invariant violated, transaction harus revert dengan error yang jelas
- Ini adalah defense-in-layer di atas manual principal accounting

## Pilihan yang dipertimbangkan

1. Hanya rely pada manual accounting (total_deposited_principal) tanpa balance check
2. Cek invariant hanya di specific functions (misalnya hanya spend)
3. Cek invariant setelah setiap state-changing operation (deposit, spend, refund)
4. Cek invariant di separate public function yang dipanggil secara periodik

## Keputusan

Gunakan pilihan 3: Cek invariant `contract_assets >= outstanding_liabilities`
setelah setiap state-changing operation (deposit, spend, refund). Function check
dibuat sebagai private method `check_liability_invariant()` yang dipanggil
internal setelah effects dan interactions.

## Alasan

- Pilihan 1 tidak memiliki defense-in-layer dan rentan terhadap accounting bugs
- Pilihan 2 masih ada gap untuk functions lain yang tidak di-check
- Pilihan 3 memberikan coverage paling komprehensif dengan overhead minimal
- Pilihan 4 bergantung pada eksternal caller dan bisa terlewat

Invariant check ini menggunakan `balanceOf` call ke stablecoin contract untuk
verifikasi actual on-chain balance, bukan hanya local accounting.

## Sumber primer

- OpenZeppelin Contracts Security, invariants and access control:
  https://docs.openzeppelin.com/contracts/4.x/security
- Stylus SDK Rust documentation, StaticCallContext and TopLevelStorage:
  https://github.com/OffchainLabs/stylus-sdk-rs
- ERC-20 standard, balanceOf function:
  https://eips.ethereum.org/EIPS/eip-20

## Sumber pembanding

- Centrifuge Protocol accounting.sol double-entry bookkeeping with unlock/lock:
  https://github.com/centrifuge/protocol/blob/main/src/core/hub/Accounting.sol
- Lido Finance stETH accounting with invariant checks:
  https://github.com/lidofinance/core/blob/v3.0.2/contracts/0.8.9/Accounting.sol

Accessed: 2026-06-07.

## Known risks

- Balance check menambah gas cost untuk setiap transaction (perkiraan +2000-5000 gas)
- Di test environment, balance check di-skip dengan `#[cfg(not(test))]` karena
  ERC20 mock tidak memiliki actual balance
- Stablecoin contract harus reliable dan return correct balance; jika contract
  bermasalah, invariant check bisa false positive

## Versi/library/network

- `stylus-sdk = 0.6.0`
- `alloy-primitives = 0.7.6`
- Target contract: USDC (or compatible ERC-20 stablecoin)

## Test

Positive:

- Deposit dengan sufficient balance passes invariant check
- Spend dengan sufficient balance passes invariant check
- Refund dengan sufficient balance passes invariant check

Negative:

- Transaction yang menyebabkan contract balance < principal harus revert
  (hard to test unit tanpa actual ERC20 implementation)

## Rollback/recovery

Jika invariant check menyebabkan issue pada production:
1. Bukan breaking change storage layout, hanya logic addition
2. Bisa di-disable sementara dengan menghapus `#[cfg(not(test))]` dan
   selalu return Ok()
3. Root cause investigation diperlukan jika invariant violated

## Implementation

- Added `check_liability_invariant()` private function in lib.rs
- Called in `_deposit()` after allocating reserves
- Called in `_claim_refund()` after refund transfer
- Called in `_spend()` after fee and payout transfers (both test and prod branches)
