# ZK Compliance Circuit (Fase B)

**Priority:** Tier 3 (Bisa Nanti)
**Status:** Not Started (0/5 complete)

## Context
Fase A selesai (Poseidon nullifier, VK hardcoded, bug fix). Fase B nambahin Merkle membership, range check, canonicality, domain separator, production trusted setup.

## Current State
- [ ] Implementasikan Merkle membership di dalam circuit
- [ ] Bind root, nullifier, recipient, amount, chain ID, dan domain separator
- [ ] Tambahkan range constraint untuk amount
- [ ] Tambahkan address/field canonicality constraints
- [ ] Distribusikan proving key sebagai artifact versioned, bukan generate runtime (untuk production)

## Requirements

### 1. Merkle Membership
- Circuit verify witness (secret) ada di Merkle tree dengan root tertentu
- Use Poseidon hash untuk Merkle path (efficient di circuit)
- Public input: root (dari contract)
- Private witness: secret + Merkle path

### 2. Domain Separator Binding
- Bind chain ID, contract address, domain separator ke circuit
- Prevent replay attack across chains/contracts
- Public input: chain_id, contract_address

### 3. Range Constraint
- Amount harus > 0 dan < max_uint256
- Use comparison gadgets di circuit

### 4. Canonicality
- Address harus fit di 20 bytes (bukan 32 bytes dengan garbage)
- Field element harus < field modulus

### 5. Production Trusted Setup
- MPC ceremony (multi-party computation)
- Generate proving key + verifying key
- Distribute sebagai artifact (file download)
- Contract load VK dari artifact (bukan hardcoded)

## Acceptance Criteria
- [ ] Merkle membership proof works
- [ ] Domain separator binding enforce
- [ ] Range constraint works
- [ ] Canonicality check works
- [ ] Production trusted setup ceremony completed
- [ ] `cargo test` + `cargo clippy` clean

## Reference
- `todo.md` section: P1 - ZK Compliance Fase B
- `nimbus-core/src/compliance_circuit.rs`: current circuit implementation
- Research: Poseidon Merkle tree, MPC trusted setup
