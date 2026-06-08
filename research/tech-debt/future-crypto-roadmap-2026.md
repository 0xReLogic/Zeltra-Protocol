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

## Priority 3: PQC on-chain (Post-Quantum Cryptography)

### Status
- **Standard:** WOTS (Winternitz One-Time Signatures), STARK-based signatures
- **Maturity:** Active research, limited production
- **Integration:** ERC-4337 (Account Abstraction)
- **Quantum Threat:** Real but not imminent (Google progress = NOT urgent)

### Current Nimbus: BLS12-381
```rust
// Current: BLS12-381 threshold signing
- Vulnerability: Quantum attack possible
- Timeline: Quantum computers still years away
- Priority: MEDIUM (future-proofing, not immediate)
```

### PQC Options
```rust
// Option 1: WOTS-based threshold
- One-time signature (needs rotation)
- More complex key management
- Ecosystem: Limited Rust implementation
- On-chain verification: No EIP precompile (yet)

// Option 2: STARK-based threshold
- Prover: Heavy computation (client-side)
- Verifier: Efficient on-chain
- Ecosystem: Growing (Risc Zero, SP1)
- Implementation: Complex

// Option 3: Hybrid (BLS + PQC fallback)
- Keep BLS for current
- Add PQC as fallback option
- Gradual migration
```

### Implementation Challenges
- WOTS one-time signature: Complex rotation needed
- STARK prover: Heavy client-side computation
- On-chain verification: No PQC EIP precompile yet
- Ecosystem: Limited Rust implementation
- Migration: Full protocol redesign

### Timeline
```
Research phase: 2026-2027
- Monitor quantum computer progress
- Research WOTS/STARK implementations
- Evaluate migration complexity

Evaluation phase: 2028-2030
- Assess quantum threat urgency
- Evaluate PQC EIP precompile availability
- Assess Rust ecosystem maturity

Migration phase: 2030+ (conditional)
- Only if quantum threat becomes imminent
- Only if PQC verification on-chain feasible
- Only if ecosystem mature
```

### Verdict
**Monitor for long-term roadmap (3-5 years)**
- Quantum threat real but NOT imminent
- PQC on-chain verification still immature
- WOTS one-time = complex rotation
- **Priority:** MEDIUM (monitor, no implementation now)

---

## Priority 4: MPC Garbled Circuits (Social Recovery)

### Status
- **Standard:** Yao's Garbled Circuits + Oblivious Transfer
- **Maturity:** Research phase, some production in institutional DeFi
- **Use case:** Social recovery wallets, institutional custody

### Current Nimbus: Shamir's Secret Sharing
```rust
// Current: Shamir's Secret Sharing (SSS)
- Threshold signing (t-of-n)
- Non-interactive
- Efficient implementation
- Guardians hold shares

// Use case: Threshold signing
- Distributed trust
- No single point failure
```

### MPC Garbled Circuits Alternative
```rust
// MPC: Multi-Party Computation
- Interactive computation
- Yao's garbled circuits
- Oblivious transfer
- Social recovery use case

// Benefit:
- Better for social recovery
- Friends/validators reconstruct key
- No single person sees private key
```

### Comparison
```
Shamir's SSS (Current):
✅ Simple, non-interactive
✅ Efficient
✅ Mature implementation
❌ Less flexible for recovery scenarios

MPC Garbled Circuits:
✅ Better for social recovery
✅ Institutional custody friendly
✅ Advanced security model
❌ Complex implementation
❌ Interactive (higher latency)
❌ Limited ecosystem
```

### Use Case for Nimbus
```rust
// Social Recovery Feature (optional):
- If user loses key
- Friends/validators reconstruct without seeing key
- Better UX for retail users

// Alternative:
- Keep Shamir's SSS for threshold signing
- Add MPC Garbled Circuits for recovery only
- Hybrid approach
```

### Implementation Requirements
```
Research phase: 1-2 months
- Study Yao's garbled circuits
- Study Oblivious Transfer protocols
- Evaluate Rust implementations
- Design social recovery flow

PoC phase: 3-4 months
- Implement MPC in nimbus-core
- Add social recovery endpoints
- Benchmark performance

Integration phase: 2-3 months
- Update nimbus-node for MPC recovery
- Add recovery workflow
- Add key rotation logic

Testing phase: 2-3 months
- Unit tests
- Integration tests
- Security audit
- Testnet deployment

Total timeline: 8-12 months
```

### Verdict
**Consider as optional feature add (not core replacement)**
- Shamir's SSS still good for threshold signing
- MPC Garbled Circuits better for social recovery
- Use case: Institutional custody, retail recovery
- **Priority:** LOW (feature add, not core requirement)

---

## NOT Relevant to Nimbus (Protocol-Level or Wrong Use Case)

### 1. Folding Schemes (Nova, Sangria, Vega)
```
Status: Production in ZK rollups

What it does: Fold multiple R1CS instances into one
Use case: zk-Rollups, cross-chain bridges, identity verification

Why NOT for Nimbus:
- Nimbus doesn't need recursive proof aggregation
- BLS threshold already efficient (100-200k gas)
- Use case: ZK rollups, bridges, identity systems
- Priority: LOW (not applicable to signing protocol)

Recommendation: Not applicable to Nimbus use case
```

### 2. Verkle Trees & Vector Commitments
```
Status: Ethereum protocol-level migration (Pur Prague/Electra)

What it does:
- Vector Commitments (Bandersnatch curve)
- Smaller witness size (puluhan kali)
- Statelessness (no terabytes needed)

Why NOT for Nimbus:
- This is PROTOCOL-LEVEL upgrade (Ethereum L1)
- NOT application-level (Nimbus is smart contract)
- Arbitrum will adopt when Ethereum adopts
- Nimbus benefits automatically (no code change)

Recommendation: Passive benefit (no action needed)
```

### 3. Sumcheck Protocol & zkVM (Risc Zero, SP1)
```
Status: Production in Risc Zero, SP1, Aztec

What it does: Rust/C++ → ZK, complex computation on-chain
Use case: Games, AI, complex DeFi logic

Why NOT for Nimbus:
- Nimbus signing logic simple (no zkVM needed)
- No complex computation needed
- Use case: Games, AI, complex DeFi
- Priority: LOW (not applicable)

Recommendation: Not applicable to current Nimbus use case
```

---

## Summary Matrix

| Technology | Priority | Timeline | Benefit | Risk | Recommended |
|------------|----------|----------|---------|------|-------------|
| **FROST** | 1 | 2027-2028 | 70% gas savings | Medium (draft) | WAIT 1-2 years |
| **ZK Halo2/STARKs** | 2 | 2029-2030 | Batch aggregation | High (complex) | Consider future |
| **PQC on-chain** | 3 | 2030+ | Quantum-proof | High (immature) | Monitor 3-5 years |
| **MPC Garbled Circuits** | 4 | Future | Social recovery | Medium (complex) | Optional feature |
| **Lattice** | - | - | Post-quantum | High (not feasible) | NOT feasible |
| **FHE** | - | - | Compute encrypted | High (gas) | NOT relevant |
| **Isogeny** | - | - | Small keys | High (security) | NOT recommended |
| **Folding Schemes** | - | - | Zero proving | N/A | Not applicable |
| **Verkle Trees** | - | - | Statelessness | N/A | Passive benefit |
| **Sumcheck/zkVM** | - | - | Complex compute | N/A | Not applicable |

---

## Action Items for Today

### Immediate (2026)
1. ✅ Keep current BLS threshold (still state-of-art)
2. ✅ Upgrade dependencies (security patches)
3. ✅ Monitor FROST RFC finalization
4. ✅ Monitor ZK ecosystem developments
5. ✅ Monitor PQC developments (quantum threat)

### Short-term (6-12 months)
6. 📊 Research FROST implementation
7. 📊 Benchmark FROST vs BLS
8. 📊 Research ZK aggregation use cases
9. 📊 Research PQC migration path (WOTS/STARK)
10. 📊 Monitor Ethereum Verkle migration (passive)

### Medium-term (1-3 years)
11. 🚀 Evaluate FROST production readiness
12. 🚀 Consider FROST migration if stable
13. 🚀 Consider ZK aggregation for batch ops
14. 🚀 Evaluate quantum threat urgency

### Long-term (3-5 years)
15. 🚀 Evaluate PQC threshold signing (if quantum imminent)
16. 🚀 Consider MPC Garbled Circuits for social recovery (optional)

---

## Conclusion

**Current Nimbus tech stack is still state-of-art for 2026**
- BLS12-381 threshold signing remains the gold standard
- No need to replace cryptography for MVP
- Focus on dependency upgrades, not algorithm changes

**Future upgrades are NOT required for MVP**
- FROST: Best candidate for gas savings (60-70%), but wait for stabilization
- ZK Halo2/STARKs: Consider for batch aggregation (long-term roadmap)
- PQC on-chain: Monitor for quantum threat (3-5 years timeline)
- MPC Garbled Circuits: Optional feature for social recovery
- Other trends (Lattice, FHE, Isogeny, Folding, Verkle, zkVM): Not feasible or relevant

**Protocol-level upgrades (passive benefits)**
- Verkle Trees: Ethereum migration → Arbitrum adoption → automatic benefit
- Folding Schemes: Not applicable to signing protocol
- Sumcheck/zkVM: Not applicable to simple signing logic

**Strategy:**
- Monitor, research, wait for stabilization
- Do not rush migration (security > novelty)
- Focus on production stability over cutting-edge novelty
- Differentiate between application-level (Nimbus) vs protocol-level (Ethereum) upgrades
