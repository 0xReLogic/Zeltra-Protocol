# DEC-035B: Formal Scope Binding Gadget & Groth16 Statement Integrity

- **Status:** APPROVED AS ARCHITECTURAL SUB-SPECIFICATION (Child of [`DEC-035`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-035-cryptographic-canonicality-public-input-binding-and-mmr.md))
- **Author:** Zeltra Architecture & Cryptography Team
- **Date:** 2026-10-10
- **Applies to:**
  - `nimbus-core/src/note_circuit.rs` (`PrivateNoteCircuit` constraint system, replacement of dead `_binding` assignment)
  - `nimbus-core/src/joinsplit_circuit.rs` (JoinSplit circuit scope binding gadget parity)
  - `nimbus-contracts/src/groth16_note_verifier.rs` (Verifier constraint matrix consistency)
  - `nimbus-node/src/handlers/spend.rs` (Relayer preflight statement checks and quote equality)
- **Parent & Related DECs:**
  - [`DEC-035`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-035-cryptographic-canonicality-public-input-binding-and-mmr.md) (Master Zero-Tech-Debt Blueprint)
  - [`DEC-035A`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-035A-two-limb-quote-hash-representation-and-range-constraints.md) (Two-Limb Quote Hash Specification)
  - [`DEC-016B`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-016B-mvp-circuit-shortcuts.md) (MVP Circuit Shortcuts & Scope Binding)
  - [`DEC-022`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-022-defense-against-proof-settlement-mismatch-boundary-gaps.md) (Proof-Settlement Mismatch Defense)

---

## 1. Problem Statement & Motivation

During the collaborative security audit across Claude, ChatGPT, and Gemini:
1. **The Dangling `_binding` Defect:**
   In `nimbus-core/src/note_circuit.rs`, the circuit computed:
   ```rust
   let _binding = poseidon_w3_hash(&quote_hash_var, &scope_hash, ...);
   ```
   Because `_binding` was prefixed with an underscore and never constrained against any public input wire, it remained dead variable code.
2. **The Statement Binding Vulnerability:**
   ChatGPT and Claude correctly warned that naively deleting `_binding` without an active gadget would cause public inputs (recipient, chain_id, contract_address, expiry, quote_hash) to drop out of R1CS matrix $A$, resulting in verifying key elements $IC[i] = \mathcal{O}$ (identity point). An unconstrained public input can be modified arbitrarily by a malicious relayer or MEV searcher without invalidating the Groth16 pairing!
3. **Missing Relayer Ingress Equality Check:**
   In `nimbus-node/src/handlers/spend.rs`, the relayer verified signatures against the quote, but failed to assert:
   $$\text{parsed\_inputs}[9 \dots 10] == \text{compute\_quote\_hash}(\text{quote})$$
   allowing mismatched proofs to pass ingress preflight.

[`DEC-035`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-035-cryptographic-canonicality-public-input-binding-and-mmr.md) mandates establishing a **Formal Scope Binding Gadget** that tightly couples all contextual transaction metadata into active R1CS constraints, eliminating dead code and guaranteeing non-trivial $IC[i]$ elements.

---

## 2. Invariants

- **INV-1 (Non-Trivial Verifying Key Elements):** For every public input wire $i \in [0, 15]$:
  $$IC[i] \ne \mathcal{O}_{G_1}$$
  No public input may exist as an unconstrained or non-binding dummy wire.
- **INV-2 (Deterministic Scope Commitment):** All contextual metadata parameters (`recipient`, `chain_id`, `contract_address`, `expiry`, `quote_hash_hi`, `quote_hash_lo`) MUST be non-malleably committed inside the proof witness.
- **INV-3 (Relayer Ingress Completeness):** The relayer ingress handler MUST enforce bit-exact equality between client-submitted quote parameters and public inputs before queueing or broadcast.

---

## 3. Specification

### A. Contextual Scope Hash Gadget
The transaction scope bundles all settlement context to prevent cross-chain, cross-contract, and replay attacks:
$$\text{scope\_hash} = \text{Poseidon}_{w5}([\text{recipient}, \text{chain\_id}, \text{contract\_address}, \text{expiry}], \text{DOMAIN\_SCOPE\_V1})$$

### B. Formal Binding Commitment Gadget
Instead of a discarded variable, we construct an explicit binding commitment wire $\text{binding\_cm}$:
$$\text{binding\_cm} = \text{Poseidon}_{w5}([quote\_hash\_hi\_var, quote\_hash\_lo\_var, scope\_hash, zero], \text{DOMAIN\_BINDING\_V1})$$

To formally enforce this in R1CS:
1. We enforce structural non-linear dependencies across all component wires:
   - $quote\_hash\_hi\_var$ and $quote\_hash\_lo\_var$ pass through 128-bit range checks (DEC-035A) and enter the Poseidon S-box ($x^5$).
   - $\text{recipient}$, $\text{chain\_id}$, $\text{contract\_address}$, and $\text{expiry}$ enter the first Poseidon S-box layer ($x^5$) of $\text{scope\_hash}$.
2. Enforce constraint:
   $$\text{binding\_cm} \cdot 1 = \text{expected\_binding\_var}$$
   ensuring that all 6 context variables participate in non-zero rows of matrices $A, B, C$, guaranteeing that every $IC[i] \ne \mathcal{O}$.

### C. Public Input Layout (16 Canonical Inputs)

```text
Index  Wire Name              Constraint Status
---------------------------------------------------------------------------------
0      note_root              Semantically bound to MMR bagged peak
1      leaf_count             Semantically bound to mountain height/bagging
2      input_nullifier        Semantically bound to nullifier PRF
3      note_epoch_id          Semantically bound to nullifier domain tag
4      output_commitment      Semantically bound to change note Poseidon
5      recipient              Bound via Scope Hash & Binding Gadget
6      merchant_amount        Semantically bound via u64 range & value conservation
7      protocol_fee           Semantically bound via u64 range & value conservation
8      execution_fee          Semantically bound via u64 range & value conservation
9      quote_hash_hi          Bound via u128 range check & Binding Gadget
10     quote_hash_lo          Bound via u128 range check & Binding Gadget
11     chain_id               Bound via Scope Hash & Binding Gadget
12     contract_address       Bound via Scope Hash & Binding Gadget
13     expiry                 Bound via Scope Hash & Binding Gadget
14     has_change             Semantically bound via boolean & zero-change check
15     is_rollover            Semantically bound via rollover branch zero-checks
```

### D. Relayer Ingress Equality Guard (`nimbus-node`)
In `nimbus-node/src/handlers/spend.rs`, add the strict equality assertion:
```rust
let expected_hash_bytes = compute_quote_hash(&quote, chain_id, contract_addr);
let (expected_hi, expected_lo) = split_quote_hash_to_limbs(&expected_hash_bytes);

if public_inputs[9] != expected_hi || public_inputs[10] != expected_lo {
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

## 4. Test & Verification Plan

1. **R1CS Soundness Test:**
   - Modify `recipient` in public inputs without changing proof $\rightarrow$ Groth16 verify MUST return `false`.
   - Modify `quote_hash_hi` or `quote_hash_lo` without changing proof $\rightarrow$ Groth16 verify MUST return `false`.
   - Modify `chain_id` or `contract_address` $\rightarrow$ Groth16 verify MUST return `false`.
2. **VK Sanity Check (`test_verifying_key_non_identity`):**
   - Iterate over all 17 elements of $IC$:
   - Assert $IC[i] \ne G_1::identity()$ for all $i \in [0, 16]$.
3. **Relayer Preflight Negative Test:**
   - Submit payload where quote signature is valid for Quote A, but public inputs correspond to Quote B.
   - Relayer MUST return `400 Bad Request` with `MismatchedQuoteHash`.
