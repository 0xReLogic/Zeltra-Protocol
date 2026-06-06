# DEC-001: Deposit, Reveal, and Collateral Lifecycle

Date: 2026-06-06

## Masalah

`reveal_mask_key` membayar seluruh collateral kepada caller sekaligus menandai
credential selesai diterbitkan. Credential tersebut masih dapat masuk ke jalur
`spend`, sehingga satu deposit dapat menghasilkan dua pelepasan nilai.
Commitment yang diverifikasi saat reveal juga berasal dari caller dan sebelumnya
tidak diikat ke deposit.

## Invariant bisnis/security

- Satu deposit hanya boleh menghasilkan satu terminal pelepasan collateral.
- Reveal hanya menyelesaikan issuance dan menutup hak refund.
- Collateral tetap menjadi backing sampai credential dibelanjakan.
- Commitment reveal harus identik dengan commitment yang dicatat saat deposit.
- `sid` tidak dapat ditimpa atau dipakai ulang.

## Pilihan yang dipertimbangkan

1. Bayar collateral saat reveal dan membuat credential non-redeemable.
2. Tidak membayar saat reveal; collateral keluar hanya lewat spend atau refund.
3. Membuat ledger liability baru dan settlement terpisah.

## Keputusan

Gunakan pilihan 2 untuk MVP. Deposit menyimpan hash commitment dan menolak
duplicate `sid`. Reveal memverifikasi commitment tersimpan, menandai sesi
resolved, dan tidak mengurangi principal atau mentransfer token. Refund hanya
tersedia sebelum reveal. Spend menjadi jalur pelepasan collateral setelah reveal.

## Alasan

Pilihan ini paling kecil perubahan arsitekturnya dan menghasilkan state machine
escrow yang jelas: deposit aktif berakhir melalui refund atau issuance, sementara
collateral hasil issuance tetap menjadi liability sampai spend.

## Sumber primer

- EIP-5528, refundable fungible token and escrow state transitions:
  https://eips.ethereum.org/EIPS/eip-5528
- OpenZeppelin Contracts Security, pull-payment/escrow and reentrancy guidance:
  https://docs.openzeppelin.com/contracts/4.x/api/security
- Stylus SDK Rust documentation, `StorageMap` and VM-backed keccak:
  https://github.com/OffchainLabs/stylus-sdk-rs

## Sumber pembanding

- Freeverse crypto-payments, audited payment/escrow implementation:
  https://github.com/freeverseio/crypto-payments

Accessed: 2026-06-06.

## Known risks

- BLS spend verification is still bypassed elsewhere and remains independently
  critical.
- `session_resolved` currently combines "issuance completed" and "refund closed";
  a future migration should use an explicit enum state.
- Existing deployed sessions do not have the appended commitment fields and
  require migration or a fresh testnet deployment.
- Unit tests mock only the EIP-2537 response; a real Stylus testnet hard test is
  mandatory.

## Versi/library/network

- `stylus-sdk = 0.6.0`
- `alloy-primitives = 0.7.6`
- Target precompile: EIP-2537 G2 MSM at `0x0e`

## Test

Positive:

- Deposit with a 256-byte commitment succeeds.
- Matching reveal resolves the session without reducing principal.
- Refund after reveal fails.

Negative:

- Invalid commitment length fails.
- Duplicate `sid` fails.
- Caller-supplied commitment differing from deposit fails.

## Rollback/recovery

This change appends storage fields but changes session semantics. Deploy a fresh
testnet contract and pause the previous deployment. Do not attempt an in-place
mainnet rollout until migration behavior for existing sessions is specified.
