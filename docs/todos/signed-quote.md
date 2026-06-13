# Signed Quote + Batch Claim

**Priority:** Tier 1 (Safety + Revenue)
**Status:** Not Started

## Context
Relayer bayar gas sendiri tapi ga ada reimbursement dari user. Quote sekarang cuma informational, ga binding. Tanpa ini relayer rugi tiap transaksi.

## Requirements

### 1. Signed Quote (User Binding)
- User sign `max_execution_fee` + `quote_expiry` + `quote_id` + chain ID + relayer identity
- Contract verify signature sebelum execute
- Actual debit ≤ signed amount → reject kalau lebih
- Use EIP-712 typed data signing

### 2. Batch Claim (Relayer Revenue)
- Node track `execution_fee` per batch di `spend_batches` table
- Tambah kolom `claimed` (boolean)
- Background worker: `SUM(execution_fee) WHERE claimed = false`
- Klaim saat threshold tercapai (amount/count/time-based)
- Contract: `claimExecutionFee(uint256 amount)` — owner/relayer only
- Update `claimed = true` hanya setelah tx sukses
- Gas overhead: ~65k per claim (negligible kalau di-batch)

## Acceptance Criteria
- [ ] User sign quote sebelum transaksi
- [ ] Contract reject kalau actual fee > signed max
- [ ] Node track execution fee per batch
- [ ] Background worker klaim berkala
- [ ] No double claim (claimed flag)
- [ ] `cargo test` + `cargo clippy` clean

## Reference
- `todo.md` section: P1 - Adaptive Private Spend Batching
- `todo.md` section: P1 - Relayer Execution Fee Batch Claim
- Research: EIP-712 typed data signing pattern
