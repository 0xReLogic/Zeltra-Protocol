# Future Crypto Upgrades Roadmap for Nimbus

Date: 2026-06-07

## Overview

Based on 2026 state-of-the-art cryptographic research, this document outlines potential upgrades to Nimbus cryptography layer. These are NOT required for MVP, but should be considered for future roadmap to maintain competitive advantage and gas efficiency.

---

## Priority 1: FROST (Flexible Round-Optimized Schnorr Threshold)

### Status
- **Standard:** IETF RFC 9591 (Draft 2025, stabilization ongoing)
- **Maturity:** Draft standard, limited Rust ecosystem
- **Security:** Less audited than BLS12-381, still in standardization

### Benefits over Current BLS Threshold
```rust
// Current (BLS Threshold):
- Verification: Pairing via EIP-2537 precompile
- Gas cost: ~100-200k per signature
- Rounds: 1 round (simpler)

// FROST (Schnorr Threshold):
- Verification: Schnorr (lighter, no pairing)
- Gas cost: ~30-50k per signature (60-70% savings!)
- Rounds: 2-3 rounds (slightly more complex)
```

### Implementation Challenges
- Rust ecosystem still limited (frost-rs exists but less mature)
- Draft standard (not finalized RFC yet)
- Less security audit history
- Implementation complexity higher than BLS

### Migration Requirements
```
Before migration:
1. FROST RFC finalization (expected 2026-2027)
2. Rust ecosystem maturity (frost-rs stable)
3. Security audit complete
4. Battle-tested in production by other protocols

Migration steps:
1. Research phase (1-2 months)
   - Study FROST specification
   - Evaluate frost-rs implementation
   - Security audit review

2. PoC phase (2-3 months)
   - Implement FROST in nimbus-core
   - Benchmark gas cost
   - Compare security model

3. Integration phase (3-4 months)
   - Update nimbus-node for FROST signing
   - Update nimbus-contracts for Schnorr verification
   - Update guardian coordination logic

4. Testing phase (2-3 months)
   - Unit tests
   - Integration tests
   - Security audit
   - Testnet deployment

Total timeline: 8-12 months
```

### Risks
- **High:** Draft standard may change
- **Medium:** Security concerns (less audited)
- **Low:** Breaking changes to existing architecture

### Recommendation
**WAIT for stabilization** (1-2 years from now)
- Monitor FROST RFC finalization
- Monitor frost-rs maturity
- Monitor production adoption by other protocols
- **Do not implement for MVP**

---

## Priority 2: ZK Halo2/STARKs for Batch Aggregation

### Status
- **Standard:** Halo2 (Aztec), STARKs (StarkNet, Polygon zkEVM)
- **Maturity:** Production in Aztec, StarkNet, Polygon zkEVM
- **Security:** Battle-tested, multiple audits

### Benefits over Current Approach
```rust
// Current (BLS Threshold):
- Single verification: ~100-200k gas
- Batch verification: 100 signatures = 200 signatures × 100k = 20M gas

// ZK Halo2/STARKs:
- Batch aggregation: 1000 proofs = cost of 1 proof verification
- No trusted setup (unlike Groth16)
- Quantum-resistant (STARKs)

Use case: Batch spend operations
- Kalau banyak user spend sekaligus
- ZK aggregation jauh lebih efisien
```

### Implementation Approach (Hybrid)
```rust
// Keep BLS for single verification (simple, efficient)
// Add ZK aggregation layer for batch operations

Architecture:
├── Single spend: BLS verification (current)
│   └── Gas: ~100-200k per signature
│
└── Batch spend: ZK aggregation (new)
    ├── Prover: Generate ZK proof for batch
    ├── Verifier: Verify 1 proof for 1000 signatures
    └── Gas: ~300-400k for 1000 signatures
```

### Implementation Requirements
```
Phase 1: Research (1-2 months)
- Study Halo2/STARKs specifications
- Evaluate Halo2 Rust implementation
- Design hybrid architecture

Phase 2: ZK Integration Layer (3-4 months)
- Implement Halo2 prover in nimbus-core
- Implement ZK verification in nimbus-contracts
- Add ZK aggregation endpoints to nimbus-node

Phase 3: Hybrid Routing (2-3 months)
- Logic to choose BLS vs ZK based on batch size
- Queue management for batch operations
- Smart routing algorithm

Phase 4: Testing (3-4 months)
- Unit tests
- Integration tests
- Gas benchmarking
- Testnet deployment
- Gradual rollout

Total timeline: 9-13 months
```

### Benefits
- **Massive gas savings** for batch operations
- **Quantum-resistant** (STARKs)
- **No trusted setup** (unlike Groth16)
- **Keep BLS** for simple operations (best of both worlds)

### Risks
- **High:** Complex implementation (hybrid architecture)
- **Medium:** Prover computation heavy (client-side)
- **Low:** Breaking changes to architecture

### Recommendation
**Consider for future roadmap (1-2 years)**
- Priority lower than FROST (FROST gives broader benefits)
- Implement AFTER FROST stabilization
- Use case: Institutional batch operations, high-frequency trading

---

## NOT Recommended (Do NOT Implement)

### 1. Lattice-Based Cryptography (ML-KEM/ML-DSA)
```
Status: NIST standardized, production-ready

Why NOT for Nimbus:
- On-chain verification: NO EIP precompile for lattice operations
- Gas cost: Millions for single verification (not feasible on L2)
- Off-chain only: Cannot meet Nimbus on-chain verification requirement
- Use case: Post-quantum security, but not feasible for on-chain

Recommendation: Wait for L2 lattice precompile (no roadmap yet)
```

### 2. Fully Homomorphic Encryption (FHE)
```
Status: Available (TFHE, Concrete), but expensive

Why NOT for Nimbus:
- Gas cost: 10-100k per operation, millions for full computation
- Use case: AI compute on encrypted data (not Nimbus scope)
- Nimbus: Simple reveal/spend不需要 FHE
- L2 (Arbitrum): Not feasible for regular usage

Recommendation: Not relevant to Nimbus use case
```

### 3. Isogeny-Based Cryptography
```
Status: Research phase, security concerns

Why NOT for Nimbus:
- History of attacks: SIDH broken 2022, SIKE broken 2022
- Security concerns: Not battle-tested
- Use case: IoT (small keys), not L2 blockchain
- Maturity: Insufficient for production financial protocol

Recommendation: Avoid due to security concerns
```

---

## Migration Timeline

### Phase 0: Current (2026)
```
✅ BLS12-381 threshold signing
✅ Standard Merkle tree
✅ Simple verification

Action: None, current approach still state-of-art
```

### Phase 1: Dependencies Upgrade (2026 Q3)
```
⚠️  Upgrade to latest versions (not algorithm change)
- Stylus 0.6.0 → 0.10.0
- ark-bls12-381 0.5.0 → 0.6.0
- alloy-primitives → 1.6.0
- Tokio, Axum latest

Action: Tech debt management, security patches
```

### Phase 2: Monitor & Research (2026-2027)
```
📊 Monitor FROST stabilization
- IETF RFC finalization
- frost-rs maturity
- Production adoption
- Security audits

📊 Monitor ZK ecosystem
- Halo2 production adoption
- STARKs gas cost optimizations
- EIP precompile support

Action: Research, no implementation
```

### Phase 3: FROST Evaluation (2027-2028)
```
🔍 Evaluate FROST production readiness
- RFC finalization status
- Rust ecosystem maturity
- Security audit results
- Production battle-testing

If ready: Begin PoC implementation
If not ready: Continue monitoring
```

### Phase 4: FROST Migration (2028-2029) [Conditional]
```
🚀 If FROST production-ready:
- Implement FROST in nimbus-core
- Migrate threshold signing from BLS to FROST
- Update nimbus-node guardians
- Update nimbus-contracts verification
- Full testnet deployment
- Gradual mainnet migration

Expected benefit: 60-70% gas savings
```

### Phase 5: ZK Aggregation Layer (2029-2030) [Conditional]
```
🚀 After FROST migration:
- Implement ZK aggregation layer
- Hybrid BLS/ZK architecture
- Batch operation optimization
- Testnet deployment
- Production rollout

Expected benefit: Massive gas savings for batch operations
```

---

## Summary Matrix

| Technology | Priority | Timeline | Benefit | Risk | Recommended |
|------------|----------|----------|---------|------|-------------|
| **FROST** | 1 | 2027-2028 | 70% gas savings | Medium (draft) | WAIT 1-2 years |
| **ZK Halo2/STARKs** | 2 | 2029-2030 | Batch aggregation | High (complex) | Consider future |
| **Lattice** | - | - | Post-quantum | High (not feasible) | NOT feasible |
| **FHE** | - | - | Compute encrypted | High (gas) | NOT relevant |
| **Isogeny** | - | - | Small keys | High (security) | NOT recommended |

---

## Action Items for Today

### Immediate (2026)
1. ✅ Keep current BLS threshold (still state-of-art)
2. ✅ Upgrade dependencies (security patches)
3. ✅ Monitor FROST RFC finalization
4. ✅ Monitor ZK ecosystem developments

### Short-term (6-12 months)
5. 📊 Research FROST implementation
6. 📊 Benchmark FROST vs BLS
7. 📊 Research ZK aggregation use cases

### Long-term (1-2 years)
8. 🚀 Evaluate FROST production readiness
9. 🚀 Consider FROST migration if stable
10. 🚀 Consider ZK aggregation for batch ops

---

## Conclusion

**Current Nimbus tech stack is still state-of-art for 2026**
- BLS12-381 threshold signing remains the gold standard
- No need to replace cryptography for MVP
- Focus on dependency upgrades, not algorithm changes

**Future upgrades are NOT required for MVP**
- FROST: Best candidate for gas savings (60-70%), but wait for stabilization
- ZK Halo2/STARKs: Consider for batch aggregation (long-term roadmap)
- Other trends (Lattice, FHE, Isogeny): Not feasible or relevant

**Strategy:**
- Monitor, research, wait for stabilization
- Do not rush migration (security > novelty)
- Focus on production stability over cutting-edge novelty
