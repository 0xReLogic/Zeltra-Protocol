# HT-06: Relayer Queue, Crash, dan Recovery

**Priority:** Tier 2 (Mainnet Safety)
**Status:** Not Started (0/16 complete)

## Context
Queue persistent tapi belum di-test scenario crash/restart. Harus prove queue survive dan no double broadcast.

## Test Scenarios
- [ ] Submit spend lalu kill node sebelum worker mengambil queue
- [ ] Restart dan pastikan request tetap diproses
- [ ] Kill node setelah nullifier reserved tetapi sebelum broadcast
- [ ] Kill node setelah broadcast tetapi sebelum receipt
- [ ] Restart dan rekonsiliasi tx hash/nonce tanpa double broadcast
- [ ] RPC timeout tidak menghapus queue
- [ ] RPC returns error tidak menandai nullifier confirmed
- [ ] On-chain revert me-release atau menandai terminal failure sesuai policy
- [ ] Dua worker bersamaan tidak memproses item yang sama
- [ ] Submit nullifier sama secara concurrent; hanya satu settlement terjadi
- [ ] Database locked/busy tidak menghilangkan request
- [ ] Disk full simulation menghasilkan fail closed
- [ ] Corrupt DB diuji dengan restore dari backup
- [ ] Retry memiliki batas dan dead-letter state
- [ ] Queue order/shuffle tidak melanggar deadline
- [ ] Expired request tidak pernah dibroadcast

## Test Plan
1. Setup: start node, submit 10 spend requests
2. Kill node di berbagai stage (reserved/broadcasting/submitted)
3. Restart node
4. Verify: semua request eventually processed, no double broadcast
5. Test concurrent workers (spawn 2 worker threads)
6. Test RPC failure (block RPC port)
7. Test disk full (fill disk sampai 100%)

## Acceptance Criteria
- [ ] All 16 test scenarios pass
- [ ] No stuck queue item (semua eventually reach terminal state)
- [ ] No double broadcast (idempotency enforce)
- [ ] No data loss (queue survive crash)
- [ ] Test report generated (JSON + Markdown)

## Reference
- `todo.md` section: HT-06 Relayer Queue, Crash, dan Recovery
- `nimbus-node/src/handlers/spend.rs`: batch processing worker
- `nimbus-node/src/database.rs`: spend_queue table
