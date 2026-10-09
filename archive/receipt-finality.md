# Receipt & Finality

**Priority:** Tier 1 (Safety + Revenue)
**Status:** Completed (6/6 complete)

## Context
Relayer broadcast tx tapi ga handle edge cases: replacement tx, nonce conflict, API status exposure.

## Current State
- [x] Tunggu receipt dan cek status (bukan cuma broadcast)
- [x] Simpan revert data/reason untuk transaksi yang harus gagal
- [x] Verifikasi saldo token sebelum dan sesudah transaksi
- [x] Terapkan confirmation threshold sesuai chain
- [x] Tangani replacement transaction dan nonce conflict
- [x] Ekspos status transaksi melalui endpoint API

## Requirements
1. Confirmation threshold: tunggu N block setelah receipt (prevent reorg)
   - Arbitrum: 1 block (fast finality)
   - Ethereum L1: 12 blocks (~2.5 menit)
2. Replacement tx: kalau tx stuck di mempool > 30s, resend dengan gas +15% (EIP-1559 compliant)
   - Track nonce di DB
   - Detect nonce conflict (tx rejected karena nonce udah dipake)
3. API endpoint: `GET /api/tx-status?tx_hash=...`
   - Return: pending/confirmed/failed
   - Include: block number, gas used, error reason (kalau failed), confirmations

## Acceptance Criteria
- [x] Confirmation threshold configurable per chain
- [x] Replacement tx otomatis kalau stuck
- [x] API expose tx status
- [x] No nonce conflict (detect dan handle)
- [x] `cargo test` + `cargo clippy` clean


## Reference
- `todo.md` section: P1 - Receipt dan Finality
- `nimbus-node/src/evm_client.rs`: tx broadcast dan receipt handling
