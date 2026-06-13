# Governance & Pause

**Priority:** Tier 3 (Bisa Nanti)
**Status:** Not Started (0/10 complete)

## Context
Contract punya pause/timelock tapi belum di-test thoroughly. Harus prove: non-owner reject, timelock enforce, pause behavior correct.

## Test Scenarios
- [ ] Non-owner gagal propose/execute
- [ ] Owner propose lalu execute sebelum ETA gagal
- [ ] Execute setelah ETA berhasil
- [ ] Proposal overwrite/cancel policy diuji
- [ ] Ownership transfer dua tahap diuji
- [ ] Pending owner salah ditolak
- [ ] Pause memblokir semua state-changing user path yang seharusnya diblokir
- [ ] Emergency refund behavior saat pause ditentukan dan diuji
- [ ] Router, pool, token, dan fee recipient zero-address handling diuji
- [ ] Event governance tersedia untuk monitoring

## Test Plan
1. Setup: deploy contract, set owner
2. Test non-owner propose/execute → expect revert
3. Test owner propose → execute sebelum ETA → expect revert
4. Test owner propose → wait ETA → execute → expect success
5. Test pause → user call state-changing function → expect revert
6. Test pause → user call refund → expect success (emergency)
7. Test zero-address validation (router/pool/token/fee_recipient)

## Acceptance Criteria
- [ ] All 10 test scenarios pass
- [ ] Timelock enforce (ETA check)
- [ ] Pause behavior correct (block user, allow refund)
- [ ] Zero-address validation works
- [ ] Governance events emitted
- [ ] Test report generated (JSON + Markdown)

## Reference
- `todo.md` section: HT-11 Governance dan Pause
- `nimbus-contracts/src/lib.rs`: propose_owner, execute_owner, pause, unpause
- `docs/mainnet_readiness_todo.md` section 1.4: Kebijakan Pause & Timelock
