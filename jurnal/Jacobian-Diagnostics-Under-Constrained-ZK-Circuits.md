**2026/1852**
(PDF)
Last updated: 2026-09-01

**Jacobian Diagnostics for Under-Constrained Zero-Knowledge Circuits**
Vijay Singh

**Category:** Implementation

Under-constrained arithmetic circuits are a recurring source of soundness failures in zero-knowledge applications: after fixing the public statement, a malicious prover may be able to assign a security-relevant wire in more than one way while still satisfying the circuit. Existing tools attack this uniqueness question with solver-based checking, direct polynomial solving, abstract interpretation, or fuzzing. We study a complementary algebraic diagnostic based on exact Jacobian linear...

**Relevance to Nimbus:** **HIGH** - Circuit soundness diagnostics untuk nimbus-core ZK circuits, prevents Groth16 attacks