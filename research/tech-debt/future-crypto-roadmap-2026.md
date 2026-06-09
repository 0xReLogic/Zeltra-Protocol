# Future Crypto Upgrades Roadmap for Nimbus

Date: 2026-06-07

## Overview

Based on 2026 state-of-the-art cryptographic research, this document outlines potential upgrades to Nimbus cryptography layer. These are NOT required for MVP, but should be considered for future roadmap to maintain competitive advantage and gas efficiency.

---

## Priority 1: FROST (Flexible Round-Optimized Schnorr Threshold)

### Status
- **Standard:** IETF RFC 9591 (Draft 2025), FROST2 (2026 SOTA with static corruption handling)
- **Maturity:** Draft standard, limited Rust ecosystem (frost-rs exists but not production-ready)
- **Adoption:** Standard for institutional wallets 2026

### Benefits over Current BLS Threshold
```
BLS Threshold (current):
- Verification via EIP-2537 pairing precompile
- Gas: 37,700 + 32,600 * k
- 1 round

FROST2 (proposed):
- Schnorr verification (no pairing needed)
- Gas: ~30-50k per signature (60-70% savings)
- 2 rounds (reducible to 1 with pre-processing)
- Handles static corruption vulnerabilities without performance cost
```

### Implementation Challenges
- Rust ecosystem still limited (frost-rs not stable)
- Draft standard (not finalized RFC)
- Less security audit history
- Higher implementation complexity than BLS

### Migration Path
```
Prerequisites:
1. FROST RFC finalization (expected 2026-2027)
2. frost-rs stable release
3. Security audit complete
4. Battle-tested in production by other protocols

Phases:
1. Research (1-2 months) - Spec study, frost-rs evaluation
2. PoC (2-3 months) - Implement in nimbus-core, benchmark gas
3. Integration (3-4 months) - Update node, contracts, guardian logic
4. Testing (2-3 months) - Tests, audit, testnet deployment

Total: 8-12 months
```

### Risks
- **High:** Draft standard may change
- **Medium:** Less audited than BLS
- **Low:** Breaking changes to architecture

### Verdict
**WAIT for stabilization (1-2 years).** Do not implement for MVP. Begin evaluation 2027-2028, migrate 2028-2029 if production-ready. Expected benefit: 60-70% gas savings.

---

## Priority 1b: dWallet Labs (Ika Network) - 2PC-MPC Threshold

### Status
- **Standard:** 2PC-MPC (2-Party ECDSA) for native multi-chain signatures
- **Maturity:** Production in Ika Network, REFHE research published at Eurocrypt 2026
- **Adoption:** Sui, Aptos networks

### What it does
```
- Zero-trust threshold signature (no single party sees full key)
- Native multi-chain signature generation (asynchronous, non-blocking)
- No client-independent presigns required
- REFHE (Ring-Enhanced Fully Homomorphic Encryption) integration for privacy
```

### Relevance to Nimbus
- More advanced security model than BLS (zero-trust + FHE)
- Better for cross-chain (native multi-chain signatures)
- Different architecture (2-party vs multi-party) — needs compatibility evaluation
- Still research phase, limited to Sui/Aptos initially

### Verdict
**RESEARCH for future roadmap.** More advanced than FROST but still research phase. Priority: MEDIUM — evaluate when production-ready.

---

## Priority 2: ZK Halo2/STARKs for Batch Aggregation

### Status
- **Standard:** Halo2 (Aztec), STARKs (StarkNet, Polygon zkEVM)
- **Maturity:** Production in Aztec, StarkNet, Polygon zkEVM
- **Security:** Battle-tested, multiple audits

### Benefits over Current Approach
```
BLS Threshold (current):
- Single verification: ~100-200k gas
- Batch 100 signatures: ~20M gas

Nova Folding Schemes (SOTA 2026):
- 48-240x speedup for large batches (up to 2^11 signatures)
- Memory usage: < 1 GB
- Gas: O(1) constant per batch

Halo2 (PLONKish):
- No trusted setup (unlike Groth16)
- Quantum-resistant (STARKs variant)
```

### Implementation Approach (Hybrid)
```
Keep BLS for single verification (simple, efficient).
Add ZK aggregation layer for batch operations.

Architecture:
- Single spend: BLS verification (current, ~100-200k gas)
- Batch spend: ZK aggregation (~300-400k gas for 1000 signatures)

Phases:
1. Research (1-2 months) - Spec study, design hybrid architecture
2. ZK Integration Layer (3-4 months) - Prover in nimbus-core, verifier in contracts
3. Hybrid Routing (2-3 months) - BLS vs ZK routing based on batch size
4. Testing (3-4 months) - Benchmarking, testnet, gradual rollout

Total: 9-13 months
```

### Risks
- **High:** Complex hybrid architecture
- **Medium:** Prover computation heavy (client-side)
- **Low:** Breaking changes

### Verdict
**Consider for future roadmap (2029-2030).** Priority lower than FROST. Implement after FROST stabilization. Use case: institutional batch operations, high-frequency trading.

---

## Priority 3: PQC On-Chain (Post-Quantum Cryptography)

### Status
- **Standard:** WOTS (Winternitz One-Time Signatures), STARK-based signatures
- **Maturity:** Active research, limited production
- **Integration:** ERC-4337 (Account Abstraction)
- **Quantum Threat:** Real but not imminent

### PQC Options
```
Option 1: WOTS-based threshold
- One-time signature (needs key rotation)
- No EIP precompile yet

Option 2: STARK-based threshold
- Heavy prover (client-side), efficient verifier (on-chain)
- Growing ecosystem (Risc Zero, SP1)

Option 3: Hybrid (BLS + PQC fallback)
- Keep BLS for current, add PQC as fallback
- Gradual migration path
```

### Implementation Challenges
- No PQC EIP precompile yet
- WOTS requires complex key rotation
- STARK prover is computationally heavy
- Limited Rust ecosystem
- Full protocol redesign needed

### Verdict
**Monitor for long-term roadmap (3-5 years).** Quantum threat real but not imminent. PQC on-chain verification still immature. Research 2026-2027, evaluate 2028-2030, migrate 2030+ only if quantum threat becomes imminent and on-chain verification is feasible.

---

## Priority 5: Threshold Signing Bridges

### Status
- **Requirement:** Modern cross-chain bridges mandate threshold cryptography (2026 standard)
- **Standard:** FROST or 2PC-MPC based
- **Use case:** Anti-hacking bridge protection via distributed trust

### How it differs from traditional bridges
```
Traditional Bridge:
- Private key on single server = single point of failure
- High hacking risk

Threshold Signing Bridge (SOTA 2026):
- Keys distributed across validator network
- Dynamic validator participation
- No single point of failure
```

### Relevance to Nimbus
- Nimbus currently uses CCIP integration (Chainlink) for cross-chain
- Future cross-chain features will require threshold signing bridges
- CCIP vs Wormhole comparison (2026): CCIP offers Privacy-Preserving Bilateral Networks (institutional), Wormhole offers Zero-Knowledge Light Clients (decentralized)

### Verdict
**MANDATORY for future cross-chain features.** Priority: HIGH when implementing cross-chain. Research 2026-2027, implement alongside cross-chain features.

---

## Priority 6: Cross-Chain Privacy & Identity Protocols

### Status
- **Standard:** LACChain ID Framework (zkVoter, ZK-proof cross-chain)
- **Use case:** Private credential issuance across chains without revealing underlying data

### Relevance to Nimbus
- Nimbus currently handles credential issuance on single chain
- Future: cross-chain credential issuance via ZK-proof between chains

### Verdict
**Consider for identity roadmap.** Priority: LOW (identity-specific feature). Research 2027-2028.

---

## Priority 7: MPC Garbled Circuits (Social Recovery)

### Status
- **Standard:** Yao's Garbled Circuits + Oblivious Transfer
- **Maturity:** Research phase, some institutional DeFi production use

### Comparison with Current Approach
```
Shamir's SSS (current):
+ Simple, non-interactive, efficient, mature
- Less flexible for recovery scenarios

MPC Garbled Circuits:
+ Better for social recovery (friends/validators reconstruct without seeing key)
+ Institutional custody friendly
- Complex, interactive (higher latency), limited ecosystem
```

### Recommended Approach
Keep Shamir's SSS for threshold signing. Add MPC Garbled Circuits for social recovery only (hybrid).

### Migration Path
```
1. Research (1-2 months) - Garbled circuits, OT protocols, Rust evaluation
2. PoC (3-4 months) - Implement in nimbus-core, benchmark
3. Integration (2-3 months) - Recovery endpoints, key rotation
4. Testing (2-3 months) - Audit, testnet

Total: 8-12 months
```

### Verdict
**Optional feature add (not core replacement).** Shamir's SSS remains good for threshold signing. Priority: LOW.

---

## NOT Recommended

### Lattice-Based Cryptography (ML-KEM/ML-DSA)
- NIST standardized, but NO EIP precompile for on-chain verification
- Gas: millions for single verification (not feasible on L2)
- **Wait for L2 lattice precompile (no roadmap yet)**

### Fully Homomorphic Encryption (FHE)
- Available (TFHE, Concrete) but gas cost prohibitive (10-100k per operation)
- Use case is AI compute on encrypted data, not signing
- **Not relevant to Nimbus**

### Isogeny-Based Cryptography
- SIDH broken 2022, SIKE broken 2022
- Not battle-tested, insufficient for production financial protocol
- **Avoid due to security concerns**

---

## NOT Relevant (Protocol-Level or Wrong Use Case)

### Folding Schemes (Nova, Sangria, Vega)
- Use case: zk-Rollups, not signing protocols
- Nimbus doesn't need recursive proof aggregation
- BLS threshold already efficient at 100-200k gas

### Verkle Trees & Vector Commitments
- Ethereum protocol-level migration (Prague/Electra)
- Arbitrum will adopt when Ethereum adopts
- **Nimbus benefits automatically, no code change needed**

### Sumcheck Protocol & zkVM (Risc Zero, SP1)
- Use case: complex computation (games, AI, DeFi)
- Nimbus signing logic is simple, no zkVM needed

---

## Summary Matrix

| Technology | Priority | Timeline | Benefit | Risk | Verdict |
|---|---|---|---|---|---|
| FROST2 | 1 | 2027-2028 | 70% gas savings | Medium (draft) | WAIT 1-2 years |
| dWallet 2PC-MPC | 1b | Research | Zero-trust + multi-chain | High (research) | Research when ready |
| ZK Halo2/STARKs | 2 | 2029-2030 | Batch aggregation 48-240x | High (complex) | Consider future |
| PQC on-chain | 3 | 2030+ | Quantum-proof | High (immature) | Monitor 3-5 years |
| Threshold Bridges | 5 | With cross-chain | Anti-hacking bridges | Low | MANDATORY |
| Cross-Chain Identity | 6 | 2027-2028 | Credential cross-chain | Low | Consider roadmap |
| MPC Garbled Circuits | 7 | Future | Social recovery | Medium (complex) | Optional feature |
| Lattice / FHE / Isogeny | - | - | Various | High | NOT feasible/relevant |

---

## Action Items

### Immediate (2026)
1. Keep current BLS threshold (still state-of-art)
2. Upgrade dependencies (security patches) -- DONE 2026-06-08
3. Monitor FROST2 RFC finalization
4. Monitor ZK ecosystem and PQC developments
5. Research dWallet Labs 2PC-MPC

### Short-term (6-12 months)
6. Research FROST2 implementation and benchmark vs BLS
7. Research ZK aggregation use cases (Nova 48-240x speedup)
8. Research PQC migration path (WOTS/STARK)
9. Evaluate threshold signing bridge options (FROST vs 2PC-MPC)
10. Evaluate CCIP vs Wormhole privacy landscape

### Medium-term (1-3 years)
11. Evaluate FROST2 production readiness, consider migration if stable
12. Consider ZK aggregation for batch operations
13. Research cross-chain identity (LACChain ID Framework)
14. Evaluate quantum threat urgency

### Long-term (3-5 years)
15. Evaluate PQC threshold signing if quantum threat becomes imminent
16. Consider MPC Garbled Circuits for social recovery (optional)
17. Implement threshold signing bridges for cross-chain features

---

## Strategy

Current Nimbus tech stack (BLS12-381 threshold signing) remains state-of-art for 2026. No algorithm changes needed for MVP.

Key principles:
- Monitor, research, wait for stabilization
- Security over novelty -- do not rush migration
- Focus on production stability
- Distinguish application-level (Nimbus) vs protocol-level (Ethereum) upgrades
