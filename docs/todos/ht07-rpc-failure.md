# HT-07: RPC dan Nonce Failure Injection

**Priority:** Tier 2 (Mainnet Safety)
**Status:** Not Started (0/10 complete)

## Context
Relayer harus resilient terhadap RPC failure dan nonce issues. Test scenario: RPC timeout, stale nonce, fallback RPC, replacement tx.

## Test Scenarios
- [ ] Primary RPC dibuat unreachable; fallback RPC digunakan
- [ ] Primary memberi stale nonce; relayer recovery
- [ ] Fallback juga mati; queue tetap aman
- [ ] RPC memberi chain ID salah; node fail closed
- [ ] RPC mengembalikan receipt timeout; monitor melanjutkan setelah restart
- [ ] Transaction underpriced diuji
- [ ] Replacement transaction diuji
- [ ] Nonce gap diuji
- [ ] Saldo gas di bawah minimum menghentikan broadcast tanpa kehilangan queue
- [ ] Gas spike membuat `min_payout` protection menolak transaksi

## Test Plan
1. Setup: start node dengan primary + fallback RPC
2. Block primary RPC (iptables/firewall)
3. Verify fallback digunakan
4. Block fallback juga
5. Verify queue safe (no broadcast, no data loss)
6. Test stale nonce (manipulate DB nonce counter)
7. Test replacement tx (submit tx dengan gas rendah, then resend dengan gas tinggi)
8. Test low balance (drain relayer wallet)

## Acceptance Criteria
- [ ] All 10 test scenarios pass
- [ ] Fallback RPC digunakan saat primary fail
- [ ] Queue safe saat semua RPC down
- [ ] Replacement tx works (gas bump)
- [ ] Low balance stop broadcast (no stuck tx)
- [ ] Test report generated (JSON + Markdown)

## Reference
- `todo.md` section: HT-07 RPC dan Nonce Failure Injection
- `nimbus-node/src/evm_client.rs`: RPC client dengan fallback
- `nimbus-node/src/handlers/spend.rs`: batch worker dengan retry logic
