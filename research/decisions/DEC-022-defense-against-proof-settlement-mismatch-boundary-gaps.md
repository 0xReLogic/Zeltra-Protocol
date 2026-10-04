# DEC-022: Defense Against Proof-Settlement Mismatch & Boundary Gaps (Aztec Connect June 2026 Exploit Post-Mortem)

**Status:** Accepted  
**Date:** October 2026  
**Applies to:** `nimbus-contracts/src/spend.rs`, `nimbus-contracts/src/storage.rs`, `nimbus-node/src/handlers/spend.rs`  
**Related DECs:** DEC-009, DEC-014, DEC-016, DEC-018  

---

## 1. Context & The June 2026 Aztec Connect Exploit

On June 14, 2026, the deprecated `RollupProcessorV3` smart contract of Aztec Connect was exploited for over **$2.1 million** across multiple asset pools (ETH, DAI, wstETH).

### Root Cause Analysis of the Exploit:
According to post-mortem analyses by Halborn, Aztec Labs, and security researchers:
1. **The "Boundary Gap":** There was a architectural decoupling between the number of transactions validated by the zero-knowledge SNARK proof and the number of transactions processed by the Layer-1 execution loop.
2. **Decoupled Parameter Vulnerability:** The contract relied on an explicit, caller-supplied counter parameter (`numRealTxs`) to delimit the transaction loop on L1. Attackers discovered that by crafting proofs where `numRealTxs` did not strictly bind to the verified ZK public input commitments, they could force the contract to execute payouts for non-existent or unbacked transaction slots.
3. **Absence of Real-Time Solvency Gate:** The settlement logic lacked an automated, atomic post-execution solvency check comparing physical contract token reserves against total liabilities. Once the loop processed the unauthorized transactions, unbacked collateral was drained directly from the vault.

---

## 2. Invariants for Nimbus Protocol

To ensure Nimbus is structurally immune to proof-settlement mismatch and boundary gap exploits:

### Invariant 1: 1-to-1 Atomic Public Input Binding (No Decoupled Counters)
In Nimbus smart contract (`nimbus-contracts/src/spend.rs`), there is **NO dynamic loop counter** or unconstrained batch parameter.
- For individual spends (`spend_private_note`): Exactly 1 proof is submitted. The 12 public input scalars evaluated by the EIP-2537 precompiles (`0x0c` MSM + `0x0f` Pairing Check) strictly bind:
  `[merkle_root, nullifier, change_commitment, recipient, merchant_amount, protocol_fee, execution_fee, quote_hash, chain_id, contract_address, expiry, has_change]`
- For batch spends (`batch_spend`): Every item in the batch array has its own dedicated nullifier, recipient, amount, and signature/proof tuple. The contract loops strictly over the validated calldata slice `0..items.len()`. No caller-supplied variable can extend or shrink the iteration range independently of the calldata array.

### Invariant 2: Multi-Liability Post-Execution Solvency Assertion
Unlike traditional bridge contracts that only check individual balances, Nimbus enforces the global solvency invariant **at the end of every state-modifying spend transaction**:
$$\text{USDC.balanceOf}(\text{contract}) \ge \text{user\_note\_liability} + \text{refundable\_liability} + \text{accrued\_fee\_liability}$$
If any rounding anomaly, reentrancy attempt, or settlement mismatch causes total liabilities to exceed physical USDC collateral by even 1 base unit, the entire transaction **reverts immediately** via `self.check_solvency()?`.

### Invariant 3: Reentrancy-Safe Checks-Effects-Interactions
The nullifier is inserted into storage (`note_nullifiers.insert(nullifier, true)`) and user note liability is decremented **BEFORE** any ERC-20 transfer to the recipient is dispatched.

---

## 3. Decision

1. **Strict Public Input Binding:**
   The smart contract reconstructs or extracts public inputs directly from canonical transaction arguments; no decoupled state counter is permitted.
2. **Mandatory Solvency Gate:**
   `check_solvency()` must be invoked at the terminal step of `spend_private_note()`, `batch_spend()`, and `claim_refund()`.
3. **Fail-Closed Relayer Verification:**
   The relayer node (`nimbus-node`) verifies the EIP-712 signed quote and Groth16 proof locally before submitting the transaction to the mempool, rejecting any payload with mismatched public inputs.

---

## 4. Consequences & Verification

- **Security:** Completely closes any "boundary gap" vulnerability akin to the Aztec Connect exploit. Unbacked token payouts are mathematically impossible.
- **Verification:** Unit tests in `nimbus-contracts` (`test_spend_private_note_insufficient_note_liability_rejected`, `test_spend_private_note_double_spend_rejected`, and `test_multi_liability_deposit_reveal_spend_lifecycle`) prove that mismatched liabilities revert without state corruption.
