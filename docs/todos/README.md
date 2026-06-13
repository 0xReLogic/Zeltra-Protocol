# Nimbus Protocol - Task Breakdown

Individual todo files untuk AI agents. Setiap file fokus ke satu task/feature dengan context, requirements, dan acceptance criteria yang jelas.

## Priority Tiers

### Tier 1: Safety + Revenue (Critical)
**Must have before mainnet. Directly impact user safety and relayer revenue.**

1. **[signed-quote.md](signed-quote.md)** - Signed Quote + Batch Claim
   - User binding (EIP-712 signature)
   - Relayer revenue (execution fee claim)
   - Status: Not Started

2. **[nullifier-lifecycle.md](nullifier-lifecycle.md)** - Nullifier Lifecycle Management
   - 4 status: reserved → submitted → confirmed → released
   - Reconciliation worker setelah restart
   - Status: In Progress (4/6 complete)

3. **[receipt-finality.md](receipt-finality.md)** - Receipt & Finality
   - Confirmation threshold (prevent reorg)
   - Replacement tx (gas bump)
   - API status exposure
   - Status: In Progress (3/6 complete)

### Tier 2: Mainnet Safety
**Must have before mainnet. Prevent critical failures.**

4. **[ccip-contract.md](ccip-contract.md)** - CCIP Contract Security (On-chain)
   - Router validation, allowlist, sender decode
   - Payload audit
   - Status: In Progress (1/7 complete)

5. **[ht06-queue-recovery.md](ht06-queue-recovery.md)** - HT-06: Queue Crash Recovery
   - 16 test scenarios
   - Prove queue survive crash, no double broadcast
   - Status: Not Started (0/16 complete)

6. **[ht07-rpc-failure.md](ht07-rpc-failure.md)** - HT-07: RPC & Nonce Failure Injection
   - 10 test scenarios
   - Fallback RPC, replacement tx, low balance handling
   - Status: Not Started (0/10 complete)

### Tier 3: Bisa Nanti
**Important but can be deferred. Nice to have for production polish.**

7. **[zk-phase-b.md](zk-phase-b.md)** - ZK Compliance Circuit (Fase B)
   - Merkle membership, range check, canonicality
   - Production trusted setup ceremony
   - Status: Not Started (0/5 complete)

8. **[x402.md](x402.md)** - x402 Protocol Implementation
   - Fix mock/invalid values
   - Payment verification, idempotency
   - Status: Not Started (0/8 complete)

9. **[kms-tls.md](kms-tls.md)** - KMS & TLS Hardening
   - HTTPS client, Vault token rotation
   - Key versioning, zeroization
   - Status: Not Started (0/7 complete)

10. **[governance.md](governance.md)** - Governance & Pause
    - 10 test scenarios
    - Timelock, pause behavior, zero-address validation
    - Status: Not Started (0/10 complete)

## Overall Progress
- **Tier 1:** 7/16 items complete (44%)
- **Tier 2:** 1/33 items complete (3%)
- **Tier 3:** 0/38 items complete (0%)
- **Total:** 8/87 items complete (9%)

## Recommended Execution Order

### Phase 1: Revenue Engine (Week 1)
1. `signed-quote.md` - Get relayer revenue working
2. `nullifier-lifecycle.md` - Fix stuck nullifiers
3. `receipt-finality.md` - Handle edge cases

### Phase 2: Mainnet Safety (Week 2-3)
4. `ccip-contract.md` - CCIP on-chain validation
5. `ht06-queue-recovery.md` - Queue crash tests
6. `ht07-rpc-failure.md` - RPC failure tests

### Phase 3: Production Polish (Week 4+)
7. `zk-phase-b.md` - ZK enhancements
8. `x402.md` - x402 fixes
9. `kms-tls.md` - Security hardening
10. `governance.md` - Governance tests

## How to Use

1. **AI Agent:** Read the specific todo file untuk task yang mau dikerjain
2. **Director (Qwen):** Craft detailed prompt dengan context dari todo file
3. **Executor Agent:** Implementasi sesuai prompt, update status di todo file
4. **Review:** Verify acceptance criteria, update status, commit

## Notes

- Setiap file standalone (ga perlu baca todo.md lagi)
- Status tracking manual (update setelah task selesai)
- Reference ke todo.md dan research decisions ada di setiap file
- Focus pada acceptance criteria (definition of done)
