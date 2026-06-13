# Nullifier Lifecycle Management

**Priority:** Tier 1 (Safety + Revenue)
**Status:** In Progress (4/6 complete)

## Context
Nullifier status di DB bisa out-of-sync sama on-chain setelah restart. Status sekarang cuma `reserved` dan `confirmed`, perlu lebih granular.

## Current State
- [x] Pisahkan reservation nullifier dari confirmed nullifier
- [x] Queue aktif hanya memproses nullifier dengan status `reserved`
- [x] Broadcast hanya mengubah status dari `reserved` menjadi `submitted`
- [x] Receipt sukses mengubah status dari `submitted` menjadi `confirmed`
- [ ] Gunakan status `reserved`, `submitted`, `confirmed`, dan `released`
- [ ] Rekonsiliasi status database dengan nullifier contract setelah restart

## Requirements
1. Tambah status `released` untuk nullifier yang udah di-refund/expired
2. Background worker rekonsiliasi setelah restart:
   - Query nullifier dengan status `submitted`
   - Cek on-chain: apakah tx hash ada di block?
   - Kalau ada → update ke `confirmed`
   - Kalau ga ada (reorg/timeout) → update ke `reserved` (retry)
3. Log reconciliation results

## Acceptance Criteria
- [ ] 4 status lifecycle: reserved → submitted → confirmed → released
- [ ] Reconciliation worker jalan setelah restart
- [ ] No stuck nullifier (semua eventually reach terminal state)
- [ ] `cargo test` + `cargo clippy` clean

## Reference
- `todo.md` section: P1 - Perbaiki Lifecycle Nullifier
- `nimbus-node/src/database.rs`: nullifier status tracking
