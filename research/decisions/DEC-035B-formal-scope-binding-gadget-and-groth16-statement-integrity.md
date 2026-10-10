# DEC-035B: Formal Scope Binding Gadget & Groth16 Statement Integrity

- **Status:** APPROVED AS ARCHITECTURAL SUB-SPECIFICATION (Child of [`DEC-035`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-035-cryptographic-canonicality-public-input-binding-and-mmr.md))
- **Author:** Zeltra Architecture & Cryptography Team
- **Date:** 2026-10-10
- **Applies to:**
  - `nimbus-core/src/note_circuit.rs` (`PrivateNoteCircuit` constraint system, replacement of dead `_binding` assignment)
  - `nimbus-core/src/joinsplit_circuit.rs` (JoinSplit circuit scope binding gadget parity)
  - `nimbus-contracts/src/groth16_note_verifier.rs` (Verifier constraint matrix consistency & public inputs check)
  - `nimbus-node/src/handlers/spend.rs` (Relayer preflight statement checks and quote equality)
- **Parent & Related DECs:**
  - [`DEC-035`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-035-cryptographic-canonicality-public-input-binding-and-mmr.md) (Master Zero-Tech-Debt Blueprint)
  - [`DEC-035A`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-035A-two-limb-quote-hash-representation-and-range-constraints.md) (Two-Limb Quote Hash Specification)
  - [`DEC-016B`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-016B-mvp-circuit-shortcuts.md) (MVP Circuit Shortcuts & Scope Binding)
  - [`DEC-022`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-022-defense-against-proof-settlement-mismatch-boundary-gaps.md) (Proof-Settlement Mismatch Defense)

---

## 1. Executive Summary & Problem Statement

In zero-knowledge proof systems based on Groth16 ([Groth, EUROCRYPT 2016](https://eprint.iacr.org/2016/260)), verification security relies on the absolute coupling between public inputs and the Quadratic Arithmetic Program (QAP) constraint matrices $A, B, C$. The joint audit across Claude (CTO), ChatGPT (Security Reviewer), and Gemini (Repository Investigator) uncovered two critical vulnerabilities in Zeltra's earlier circuit and relayer ingress implementations:

1. **The Dangling `_binding` Defect (Dead Wire Assignment):**
   In `nimbus-core/src/note_circuit.rs`, the circuit witness generation routine executed:
   ```rust
   let _binding = poseidon_w3_hash(&quote_hash_var, &scope_hash, ...);
   ```
   Because `_binding` was prefixed with an underscore and never constrained against any public input wire, linear combination, or downstream gate, it was compiled out or remained an unconstrained intermediate computation.
2. **The Statement Binding Vulnerability (Identity Point Degradation):**
   If an engineer naively deleted `_binding` without an active replacement gadget, six contextual public inputs:
   $$\text{recipient}, \quad \text{chain\_id}, \quad \text{contract\_address}, \quad \text{expiry}, \quad \text{quote\_hash\_hi}, \quad \text{quote\_hash\_lo}$$
   would completely drop out of the R1CS constraint matrix rows!
   In Groth16, if a public input wire $k$ has zero entries across matrices $A, B, C$, its verifying key generator element collapses to the **identity point (point at infinity)**:
   $$IC[k] = \mathcal{O}_{G_1}$$
   When $IC[k] = \mathcal{O}_{G_1}$, any scalar scalar multiplier satisfies $x_k \cdot \mathcal{O}_{G_1} = \mathcal{O}_{G_1}$. An attacker or malicious relayer could freely replace the `recipient` or `quote_hash` with arbitrary values, and the Groth16 pairing verification would still succeed!
3. **Missing Relayer Ingress Equality Guard:**
   In `nimbus-node/src/handlers/spend.rs`, the relayer verified that the client had signed a valid `ExecutionQuote`, but failed to assert bit-exact equality between the reconstructed quote hash and the submitted public inputs `public_inputs[9]` and `public_inputs[10]`. This allowed a client to submit a valid signature for Quote A (e.g. paying high fees) while executing a proof bound to Quote B.

[`DEC-035`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-035-cryptographic-canonicality-public-input-binding-and-mmr.md) mandates establishing a **Formal Scope Binding Gadget** that tightly couples all transaction context into active R1CS constraints, eliminating dead wires and guaranteeing non-trivial $IC[i] \ne \mathcal{O}_{G_1}$ across all public inputs.

---

## 2. Post-Mortem & Vulnerability Archaeology

```
      THE UNCONSTRAINED PUBLIC INPUT VULNERABILITY (GROTH16)
┌────────────────────────────────────────────────────────────────────────┐
│  Public Input x_k declared in Circuit, but unconstrained in R1CS       │
│  ==> Polynomials u_k(x) = 0, v_k(x) = 0, w_k(x) = 0                     │
│  ==> Verifying Key Element: IC[k] = O_G1 (Point at Infinity)           │
├────────────────────────────────────────────────────────────────────────┤
│  Groth16 Verifier MSM: vk_x = IC[0] + Σ x_i * IC[i]                    │
│  Attacker substitutes x_k with x_k' (Attacker's address as Recipient): │
│  x_k' * IC[k] = x_k' * O_G1 = O_G1                                     │
│  ==> vk_x is UNCHANGED! Pairing Check PASSES!                          │
│  ==> ATTACKER STEALS FUNDS VIA PUBLIC INPUT TAMPERING!                 │
└────────────────────────────────────────────────────────────────────────┘
```

### A. Tornado Cash Recipient & Relayer Substitution Defense (MixBytes & Beosin Audits)
- **Vulnerability Context:** When Tornado Cash was designed, the withdrawal circuit needed to prove knowledge of a note commitment without revealing the secret depositor. However, the transaction also required specifying a `recipient` and `relayer` fee to execute the withdrawal on Ethereum.
- **Root Cause Identified:** As documented in the MixBytes security audit and Beosin research ([MixBytes Blog, 2020](https://mixbytes.io/blog/audit-zk-application-example-tornado-cash); [Beosin Research, 2023](https://beosin.com/resources/exploring-tornado-cash-in-depth-to-reveal-malleability-attac)), if external transaction parameters (`recipient`, `relayer`, `fee`, `refund`) were merely declared as circuit inputs without participating in quadratic constraints, an MEV bot or malicious relayer could intercept the transaction in the Ethereum mempool, replace the `recipient` address with the attacker's address, and steal the withdrawal.
- **The Tornado Cash Workaround:** In Circom, Tornado Cash added explicit squaring constraints:
  ```circom
  signal recipientSquare;
  recipientSquare <== recipient * recipient;
  ```
  This forced `recipient` into matrix $A$ and $B$, ensuring that tampering with `recipient` altered the proof statement.
- **Limitation & Zeltra Improvement:** Simple dummy squaring (`x * x`) guarantees $IC[i] \ne \mathcal{O}$, but provides **zero cryptographic binding** between the variables (e.g. `recipient` is not cryptographically entangled with `chain_id` or `expiry`). Zeltra implements a multi-variate **Poseidon Scope Sponge** that cryptographically binds the entire settlement context into a unified commitment.

### B. The "Frozen Heart" Vulnerability & Missing Statement Binding (Trail of Bits, 2022)
- **Vulnerability Context:** In 2022, Trail of Bits uncovered the "Frozen Heart" vulnerability across multiple zero-knowledge proof implementations (including Bulletproofs, Plonk, and STARKs).
- **Root Cause:** In the Fiat-Shamir transformation, implementations failed to include the public statement and instance inputs in the challenge generation hash transcript. Attackers could generate a valid proof for a generic statement and then bind it retroactively to arbitrary public inputs, completely breaking proof soundness.
- **Relevance to Zeltra:** In Groth16, statement binding is enforced through the linear combination of verifying key points $IC$. If public inputs can be decoupled from the circuit's active constraint graph, the protocol becomes vulnerable to statement-malleability attacks analogous to Frozen Heart.

### C. Groth16 Generator Architecture & Bellman/Arkworks Soundness
- **Vulnerability Context:** In the official Rust `groth16` generator ([docs.rs/groth16](https://docs.rs/groth16/latest/src/groth16/generator.rs.html)), the generator explicitly iterates over all query elements $L_i$ and asserts:
  ```rust
  for e in l.iter() {
      if e.is_identity().into() {
          return Err(SynthesisError::UnconstrainedVariable);
      }
  }
  ```
- **Root Cause:** A public input that evaluates to the identity point in $G_1$ represents a catastrophic design error. It proves that the variable has zero weight in the R1CS constraint matrix, allowing the verifier to accept arbitrary inputs.
- **Relevance to Zeltra:** Dead assignments like `let _binding = ...` silently bypass naive linter checks. Zeltra mandates active circuit equality assertions and automated verifying key non-identity tests (`test_verifying_key_non_identity`) across all 17 elements of $IC$.

---

## 3. Mathematical Foundations of Statement Binding

### A. The Groth16 Verifying Equation & Public Inputs MSM
In Groth16 ([Groth, 2016](https://eprint.iacr.org/2016/260)), given public inputs $(x_1, \dots, x_{\ell}) \in \mathbb{F}_r^{\ell}$ and proof $\pi = (A \in G_1, B \in G_2, C \in G_1)$, the verifier checks:
$$e(A, B) = e(\alpha, \beta) + e\left( IC[0] + \sum_{i=1}^{\ell} x_i \cdot IC[i], \gamma \right) + e(C, \delta)$$

During trusted setup with toxic waste $(\alpha, \beta, \gamma, \delta, x)$, the verifying key elements for public inputs are defined by:
$$IC[i] = \left( \frac{\beta u_i(x) + \alpha v_i(x) + w_i(x)}{\gamma} \right) \cdot G_1 \quad \forall i \in \{0, \dots, \ell\}$$

Where:
- $u_i(x) = \sum_{j=1}^{m} A_{j, i} \cdot \ell_j(x)$
- $v_i(x) = \sum_{j=1}^{m} B_{j, i} \cdot \ell_j(x)$
- $w_i(x) = \sum_{j=1}^{m} C_{j, i} \cdot \ell_j(x)$

#### Proof of Vulnerability when Variable is Unconstrained:
Suppose wire $k \in \{1, \dots, \ell\}$ does not appear in any R1CS constraint. Then:
$$A_{j, k} = 0, \quad B_{j, k} = 0, \quad C_{j, k} = 0 \quad \forall j \in \{1, \dots, m\}$$
Consequently:
$$u_k(x) = 0, \quad v_k(x) = 0, \quad w_k(x) = 0 \implies IC[k] = 0 \cdot G_1 = \mathcal{O}_{G_1}$$

In the verifier's multi-scalar multiplication (MSM):
$$\sum_{i=1}^{\ell} x_i \cdot IC[i] = \left( \sum_{i \ne k} x_i \cdot IC[i] \right) + x_k \cdot \mathcal{O}_{G_1} = \sum_{i \ne k} x_i \cdot IC[i]$$
Because $x_k \cdot \mathcal{O}_{G_1} = \mathcal{O}_{G_1}$ for **all possible values** of $x_k \in \mathbb{F}_r$, the value of $x_k$ is completely unconstrained by the pairing equation!

---

## 4. Invariants

- **INV-1 (Non-Trivial Verifying Key Elements):** For every public input wire $i \in [0, 16]$:
  $$IC[i] \ne \mathcal{O}_{G_1}$$
  No public input may exist as an unconstrained or non-binding dummy wire.
- **INV-2 (Deterministic Scope Commitment):** All contextual metadata parameters (`recipient`, `chain_id`, `contract_address`, `expiry`, `quote_hash_hi`, `quote_hash_lo`) MUST be non-malleably committed inside the proof witness through non-linear Poseidon permutations.
- **INV-3 (Relayer Ingress Completeness):** The relayer ingress handler MUST enforce bit-exact equality between client-submitted quote parameters and public inputs before queueing or broadcast.
- **INV-4 (Domain Separation Soundness):** Scope hash and binding commitment computations MUST utilize cryptographically distinct Poseidon domain tags (`DOMAIN_SCOPE_V1` and `DOMAIN_BINDING_V1`) to prevent cross-sponge collision attacks.

---

## 5. Technical Specification

```
                         THE FORMAL SCOPE BINDING GADGET
┌────────────────────────────────────────────────────────────────────────┐
│ Stage 1: Contextual Scope Hash                                         │
│                                                                        │
│  recipient ──────┐                                                     │
│  chain_id ───────┼─► Poseidon_w5(..., DOMAIN_SCOPE_V1) ─► scope_hash   │
│  contract_addr ──┤                                          │          │
│  expiry ─────────┘                                          │          │
├─────────────────────────────────────────────────────────────┼──────────┤
│ Stage 2: Binding Commitment & R1CS Enforcement              │          │
│                                                             ▼          │
│  quote_hash_hi ──┐                                                     │
│  quote_hash_lo ──┼─► Poseidon_w5(..., DOMAIN_BINDING_V1) ─► binding_cm │
│  scope_hash ─────┤                                          │          │
│  zero ───────────┘                                          │          │
│                                                             ▼          │
│                     Enforce R1CS Constraint:                           │
│                     binding_cm * 1 == expected_binding_var             │
│                     (All 6 variables participate in A, B, C)           │
└────────────────────────────────────────────────────────────────────────┘
```

### A. Contextual Scope Hash Gadget
To bind the environmental execution parameters and prevent replay across chains, contracts, or expired blocks:
$$\text{scope\_hash} = \text{Poseidon}_{w5}([\text{recipient}, \text{chain\_id}, \text{contract\_address}, \text{expiry}], \text{DOMAIN\_SCOPE\_V1})$$

Where:
- $\text{recipient}$: The 20-byte merchant Ethereum address, converted to an $\mathbb{F}_r$ element.
- $\text{chain\_id}$: Target EVM chain ID (e.g. `421614` for Arbitrum Sepolia).
- $\text{contract\_address}$: Deployed Stylus smart contract address.
- $\text{expiry}$: Unix timestamp after which the execution quote is invalid.
- $\text{DOMAIN\_SCOPE\_V1} = \texttt{0x5a454c5452415f53434f50455f5631}$ ("ZELTRA_SCOPE_V1").

### B. Formal Binding Commitment Gadget
The binding commitment wire $\text{binding\_cm}$ unites the two-limb quote hash (DEC-035A) with the contextual scope:
$$\text{binding\_cm} = \text{Poseidon}_{w5}([quote\_hash\_hi\_var, quote\_hash\_lo\_var, scope\_hash, zero], \text{DOMAIN\_BINDING\_V1})$$

Where:
- $quote\_hash\_hi\_var$: High 128-bit limb of EIP-712 quote hash (range checked in DEC-035A).
- $quote\_hash\_lo\_var$: Low 128-bit limb of EIP-712 quote hash (range checked in DEC-035A).
- $\text{DOMAIN\_BINDING\_V1} = \texttt{0x5a454c5452415f42494e44494e475f5631}$ ("ZELTRA_BINDING_V1").

### C. Active R1CS Enforcement in `nimbus-core`
In `nimbus-core/src/note_circuit.rs`:
```rust
// 1. Allocate contextual public inputs
let recipient_var = cs.new_input_variable(|| Ok(self.recipient))?;
let chain_id_var = cs.new_input_variable(|| Ok(self.chain_id))?;
let contract_address_var = cs.new_input_variable(|| Ok(self.contract_address))?;
let expiry_var = cs.new_input_variable(|| Ok(self.expiry))?;
let quote_hi_var = cs.new_input_variable(|| Ok(self.quote_hash_hi))?;
let quote_lo_var = cs.new_input_variable(|| Ok(self.quote_hash_lo))?;

// 2. Synthesize Scope Hash via Poseidon Grain-128
let scope_hash = poseidon_w5(
    cs.namespace(|| "scope_hash"),
    &[recipient_var, chain_id_var, contract_address_var, expiry_var],
    DOMAIN_SCOPE_V1,
)?;

// 3. Synthesize Formal Binding Commitment
let binding_cm = poseidon_w5(
    cs.namespace(|| "binding_cm"),
    &[quote_hi_var, quote_lo_var, scope_hash, zero_var],
    DOMAIN_BINDING_V1,
)?;

// 4. Enforce rank-1 active constraint against computed witness
let expected_binding_var = cs.new_witness_variable(|| Ok(self.expected_binding))?;
cs.enforce(
    || "enforce_binding_integrity",
    |lc| lc + binding_cm.get_variable(),
    |lc| lc + CS::one(),
    |lc| lc + expected_binding_var,
);
```

Because each input passes through the non-linear Poseidon S-boxes ($x^5$) and MDS matrix multiplications, every contextual variable occupies multiple non-zero rows in matrices $A, B, C$, mathematically guaranteeing that:
$$IC[i] \ne \mathcal{O}_{G_1} \quad \forall i \in \{5, 9, 10, 11, 12, 13\}$$

---

## 6. Canonical 16 Public Inputs Layout

The table below details the complete, canonical 16 Groth16 public inputs:

```
Index  Wire Name              Constraint Status & Binding Mechanism
---------------------------------------------------------------------------------------------
0      note_root              Semantically bound to MMR bagged peak
1      leaf_count             Semantically bound to MMR mountain height / bagging loop
2      input_nullifier        Semantically bound to nullifier PRF: Poseidon(secret, epoch)
3      note_epoch_id          Semantically bound to nullifier PRF domain tag
4      output_commitment      Semantically bound to change note Poseidon commitment
5      recipient              Bound via Scope Hash (Poseidon_w5) & Binding Gadget
6      merchant_amount        Semantically bound via u64 range & value conservation
7      protocol_fee           Semantically bound via u64 range & value conservation
8      execution_fee          Semantically bound via u64 range & value conservation
9      quote_hash_hi          Bound via u128 range check (DEC-035A) & Binding Gadget
10     quote_hash_lo          Bound via u128 range check (DEC-035A) & Binding Gadget
11     chain_id               Bound via Scope Hash (Poseidon_w5) & Binding Gadget
12     contract_address       Bound via Scope Hash (Poseidon_w5) & Binding Gadget
13     expiry                 Bound via Scope Hash (Poseidon_w5) & Binding Gadget
14     has_change             Semantically bound via boolean & zero-change check
15     is_rollover            Semantically bound via rollover branch zero-checks
```

---

## 7. Relayer Ingress Equality Guard (`nimbus-node`)

In `nimbus-node/src/handlers/spend.rs`, add the strict preflight equality assertion to prevent quote-swapping and griefing attacks:

```rust
// 1. Reconstruct expected EIP-712 signing digest
let expected_hash_bytes = compute_quote_hash(&quote, chain_id, contract_addr);
let (expected_hi, expected_lo) = split_quote_hash_to_limbs(&expected_hash_bytes);

// 2. Validate bit-exact equality with public inputs
let input_quote_hi = public_inputs[9];
let input_quote_lo = public_inputs[10];

if input_quote_hi != expected_hi || input_quote_lo != expected_lo {
    tracing::warn!(
        target: "relayer::spend",
        "Public inputs quote_hash mismatch: expected ({:?}, {:?}), got ({:?}, {:?})",
        expected_hi, expected_lo, input_quote_hi, input_quote_lo
    );
    return (
        StatusCode::BAD_REQUEST,
        Json(SpendResponse::rejected(
            "MismatchedQuoteHash",
            "Public inputs quote_hash limbs do not match verified ExecutionQuote",
        )),
    );
}
```

---

## 8. Threat Modeling & Security Analysis

```
┌───────────────────────────────────────┬────────────────────────────────────────────────────────┐
│ Threat Vector                         │ Architectural Mitigation / Mathematical Defense        │
├───────────────────────────────────────┼────────────────────────────────────────────────────────┤
│ 1. Recipient Substitution Attack      │ recipient enters Poseidon Scope Hash. Modification     │
│    (MEV bot redirects merchant payout)│ alters binding_cm, causing Groth16 pairing revert.     │
├───────────────────────────────────────┼────────────────────────────────────────────────────────┤
│ 2. Cross-Chain Replay Attack          │ chain_id is committed in Scope Hash. Proof generated   │
│    (Replaying Arbitrum Sepolia on L1) │ for Arbitrum Sepolia fails verification on any other id│
├───────────────────────────────────────┼────────────────────────────────────────────────────────┤
│ 3. Dead Wire Identity Degradation     │ Active R1CS constraint binding_cm * 1 == expected.     │
│    (IC[i] collapses to O_G1)          │ All 17 elements of IC are non-identity (IC[i] != O).   │
├───────────────────────────────────────┼────────────────────────────────────────────────────────┤
│ 4. Relayer Ingress Quote Desync       │ Bit-exact assertion in spend handler:                  │
│    (Client signs Quote A, proves B)   │ asserts public_inputs[9..11] == split(quote_hash).     │
└───────────────────────────────────────┴────────────────────────────────────────────────────────┘
```

---

## 9. Testing & Verification Matrix

### A. R1CS Constraint Soundness Tests (`nimbus-core`)
1. **Public Input Tampering Tests:**
   - Tamper `recipient` in public inputs without updating proof: assert `verify_proof() == false`.
   - Tamper `chain_id` or `contract_address`: assert `verify_proof() == false`.
   - Tamper `expiry`: assert `verify_proof() == false`.
   - Tamper `quote_hash_hi` or `quote_hash_lo`: assert `verify_proof() == false`.
2. **Automated Verifying Key Sanity Test (`test_verifying_key_non_identity`):**
   - Iterate over all 17 points of $IC \in G_1$:
     ```rust
     for (i, ic_point) in vk.ic.iter().enumerate() {
         assert!(
             !ic_point.is_zero(),
             "Verifying key element IC[{}] is identity point (point at infinity)!",
             i
         );
     }
     ```

### B. Relayer Ingress Negative Tests (`nimbus-node`)
1. **Mismatched Quote Rejection:**
   - Client signs valid quote with gas limit $G_1$, but submits proof generated with gas limit $G_2$.
   - Assert relayer immediately returns `400 Bad Request` with `MismatchedQuoteHash`.
   - Prove SQLite database state and spend queue remain completely unmodified.

---

## 10. References & Citations

1. **Groth, Jens.** (2016). *On the Size of Pairing-based Non-interactive Arguments.* In: Advances in Cryptology – EUROCRYPT 2016, Lecture Notes in Computer Science, vol 9666, pp. 305–326. Springer. [https://eprint.iacr.org/2016/260](https://eprint.iacr.org/2016/260)
2. **MixBytes Security Team.** (2020). *Audit of a ZK Application on Example: Tornado Cash.* MixBytes Security Blog. [https://mixbytes.io/blog/audit-zk-application-example-tornado-cash](https://mixbytes.io/blog/audit-zk-application-example-tornado-cash)
3. **Beosin Security Team.** (2023). *Exploring Tornado Cash In-Depth to Reveal Malleability Attacks in ZKP Projects.* Beosin Research. [https://beosin.com/resources/exploring-tornado-cash-in-depth-to-reveal-malleability-attac](https://beosin.com/resources/exploring-tornado-cash-in-depth-to-reveal-malleability-attac)
4. **Trail of Bits.** (2022). *Coordinated Disclosure: The "Frozen Heart" Vulnerability in Fiat-Shamir Implementations across Zero-Knowledge Proof Systems (Bulletproofs, Plonk, STARKs).* Trail of Bits Security Blog (Jim Miller). [https://blog.trailofbits.com/2022/04/13/part-1-coordinated-disclosure-of-vulnerabilities-affecting-fiat-shamir-implementations/](https://blog.trailofbits.com/2022/04/13/part-1-coordinated-disclosure-of-vulnerabilities-affecting-fiat-shamir-implementations/)
5. **Grassesi, Lorenzo; Khovratovich, Dmitry; Rechberger, Christian; Roy, Arnab; & Schofnegger, Markus.** (2021). *Poseidon: A New Hash Function for Zero-Knowledge Proof Systems.* USENIX Security Symposium 2021. [https://eprint.iacr.org/2019/458](https://eprint.iacr.org/2019/458)
6. **Arkworks Community.** (2024). *ark-groth16: Rust Implementation of Groth16 Prover and Verifier with Constraint System Synthesis.* [https://github.com/arkworks-rs/groth16](https://github.com/arkworks-rs/groth16)
7. **Bowe, Sean; Gabizon, Ariel; & Miers, Ian.** (2017). *Scalable Multi-party Computation for zk-SNARK Parameters in the Random Beacon Model.* IACR Cryptology ePrint Archive, Report 2017/1050. [https://eprint.iacr.org/2017/1050](https://eprint.iacr.org/2017/1050)
8. **Maller, Mary; Bowe, Sean; Chiesa, Alessandro; & Meiklejohn, Sarah.** (2019). *Sonic: Zero-Knowledge SNARKs from Linear-Size Universal and Updatable Structured Reference Strings.* ACM CCS 2019. [https://eprint.iacr.org/2019/099](https://eprint.iacr.org/2019/099)
9. **Ethereum Foundation.** (2018). *EIP-712: Typed structured data hashing and signing.* Ethereum Improvement Proposals. [https://eips.ethereum.org/EIPS/eip-712](https://eips.ethereum.org/EIPS/eip-712)
10. **Ethereum Foundation.** (2020). *EIP-2537: Precompiled contracts for BLS12-381 curve operations on Ethereum.* Ethereum Improvement Proposals. [https://eips.ethereum.org/EIPS/eip-2537](https://eips.ethereum.org/EIPS/eip-2537)
