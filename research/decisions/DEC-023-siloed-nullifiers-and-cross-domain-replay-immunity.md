# DEC-023: Siloed Nullifier Derivation & Cross-Domain Replay Immunity

**Status:** Accepted  
**Date:** October 2026  
**Applies to:** `nimbus-core/src/note.rs`, `nimbus-core/src/note_circuit.rs`, `nimbus-contracts/src/spend.rs`  
**Related DECs:** DEC-016, DEC-016A, DEC-016B, DEC-021  

---

## 1. Context & Vulnerability Vector

In early privacy protocols (such as early Tornado Cash instances and basic ZK-UTXO designs), nullifiers were computed simply as:
$$\text{nullifier} = \text{Hash}(\text{spending\_key}, \text{commitment\_or\_path})$$

Security reviews and audits between 2024 and 2026 (including Aztec's note architecture analysis and privacy bridge audit reports) identified two major vulnerabilities in naive nullifier derivations:
1. **Cross-Contract Nullifier Collisions & Frontrunning:** If a user uses the same spending key across multiple deployments or testnets, revealing a nullifier on one contract can inadvertently invalidate or frontrun a note on another contract.
2. **View-Key Linkability:** If the nullifier derivation directly exposes the spending key or note preimage upon auditing, external observers can correlate historical transactions and break forward secrecy.

---

## 2. Research & Architectural Reference

### Aztec Protocol's Siloed Nullifier Model (`uint_note.nr`)
Aztec's canonical note standard implements a two-stage derivation:
1. **Nullifier Secret Key Derivation:**
   $$\text{nk} = \text{Poseidon}(\text{sk\_owner}, \text{DOMAIN\_NULLIFIER\_KEY})$$
2. **Position-Bound Nullifier:**
   $$\text{inner\_nullifier} = \text{Poseidon}(\text{nk}, \text{leaf\_index})$$
3. **Siloed Nullifier on Contract Layer:**
   $$\text{siloed\_nullifier} = \text{Poseidon}(\text{contract\_address}, \text{inner\_nullifier})$$

This ensures that:
- Even if a view-key is disclosed, the spending key remains uncompromised.
- Nullifiers are unique per smart contract address and cannot be replayed or collided across contracts or chains.

---

## 3. Decision for Nimbus Protocol

1. **Separation of Spending Key and Nullifier Key:**
   In `nimbus-core/src/note.rs`, `derive_nullifier_key(sk)` derives `nk` using a unique domain separator (`nimbus.nullifier_key.v1`).
2. **Leaf Index Binding (Gate C0 Verified):**
   In `nimbus-core/src/note_circuit.rs`, `nullifier` is strictly constrained to the 20-bit decomposition of `input_leaf_index`. A nullifier cannot be computed or spent without knowledge of its exact Merkle leaf index.
3. **On-Chain Domain Siloing (Anti-Cross-Chain Replay):**
   The `PrivateNoteCircuit` and smart contract `spend_private_note()` explicitly incorporate:
   - `chain_id`: Enforcing that a nullifier spent on Arbitrum Sepolia (421614) cannot be replayed on Arbitrum One (42161).
   - `contract_address`: Siloing the transaction to the specific deployed Stylus contract address.
   - `quote_hash`: Binding the user's signed EIP-712 consent, preventing relayer malleability.

---

## 4. Consequences & Verification

- **Security:** Guarantees cryptographic unlinkability, forward secrecy, and immunity against cross-chain and cross-contract replay.
- **Verification:**
  - `nimbus-core`: `note::tests::test_nullifier_unlinkable_without_key`, `test_domain_separators_distinct`, and `test_gate_c0_tampered_leaf_index_fails_proving`.
  - `nimbus-contracts`: `test_spend_rejects_untrusted_key_and_message_replay` and `test_spend_private_note_double_spend_rejected`.
