# DEC-016A: Private Note V1 — Frozen Specification

Date: 2026-06-14
Status: Frozen (Gate A deliverable)
Supersedes: Candidate fields in DEC-016 §"Bentuk Note V1"
Decision basis: Zcash Protocol Spec (NU6.1), ZIP 212, Privacy Boost LeanIMT,
Penumbra Poseidon paramgen, EIP-5988 security analysis, RAILGUN UTXO docs.

---

## 1. PrivateNoteV1

```text
PrivateNoteV1 {
    value       : u64          // stablecoin amount in base units (6 decimals)
    owner_key   : Fr           // owner spending public key (BLS12-381 scalar)
    rho         : Fr           // unique note identifier / nullifier randomness
    randomness  : Fr           // commitment trapdoor (hiding property)
}
```

All `Fr` values are elements of the BLS12-381 scalar field:
`p = 0x73eda753299d7d483339d80809a1d80553bda402fffe5bfeffffffff00000001`

`value` is a 64-bit unsigned integer. Range: `[0, 2^64 - 1]`.
Max value in USDC: `2^64 / 10^6 ≈ 1.84 × 10^13` USDC (far exceeds any realistic amount).

---

## 2. Domain Separators

All domain tags are distinct, fixed `Fr` constants derived deterministically:

```text
DOMAIN_NOTE_COMMITMENT_V1 = Fr::from(
    keccak256("nimbus.note.commitment.v1") mod p
)

DOMAIN_NULLIFIER_V1 = Fr::from(
    keccak256("nimbus.note.nullifier.v1") mod p
)

DOMAIN_MERKLE_NODE_V1 = Fr::from(
    keccak256("nimbus.merkle.node.v1") mod p
)

DOMAIN_INITIAL_NOTE_V1 = Fr::from(
    keccak256("nimbus.note.initial.v1") mod p
)
```

These are computed at compile time. No runtime randomness in domain tags.

---

## 3. Poseidon Configuration

### 3a. Parameters (shared across all widths)

| Parameter | Value | Rationale |
|---|---|---|
| Field | BLS12-381 scalar field Fr | Matches existing Groth16 curve |
| S-box exponent α | 5 | gcd(5, p-1) = 1; smallest valid odd exponent |
| Full rounds R_F | 8 (4+4) | 128-bit security at all widths ≥ 3 |
| Capacity | 1 | Standard for collision-resistant hashing |

### 3b. Width instances

| Width t | Rate r | Use case |
|---|---|---|
| 3 | 2 | Nullifier derivation (2 inputs), legacy 2:1 hash |
| 5 | 4 | Note commitment (4 inputs), Merkle tree node hash |

### 3c. Round constants generation

For each width `t`, generate round constants deterministically:

```text
seed_t = keccak256("nimbus.poseidon.rc.v1" || le_bytes(t))
rng = StdRng::seed_from_u64(seed_t as u64)
for i in 0..(R_F + R_P) * t:
    rc[i] = Fr::rand(&mut rng)
```

Each width has its own independent set of round constants.

### 3d. MDS matrix generation

For each width `t`, generate the MDS matrix using the **standard Cauchy construction**:

```text
// Use small deterministic values, NOT random
for i in 0..t:
    x[i] = Fr::from(i + 1)       // x = [1, 2, 3, ..., t]
    y[i] = Fr::from(t + i + 1)   // y = [t+1, t+2, ..., 2t]

for i in 0..t:
    for j in 0..t:
        M[i][j] = 1 / (x[i] + y[j])
```

This is the canonical Cauchy matrix from the Poseidon paper. It is guaranteed
to be MDS because `x[i] + y[j]` are all distinct and non-zero for these choices.

**SECURITY NOTE:** The current codebase uses `Fr::rand` for MDS entries, which
produces a random Cauchy matrix. The new implementation MUST use the deterministic
small-value Cauchy construction above. Random MDS entries make it impossible to
reproduce parameters independently.

### 3e. Sponge initialization

State is initialized with capacity element set to a domain tag:

```text
state = [input_0, input_1, ..., input_{r-1}, domain_tag]
```

This provides domain separation at the sponge level in addition to explicit
domain tags in the inputs.

---

## 4. Note Commitment

```text
note_commitment(note: PrivateNoteV1) -> Fr:
    state = [
        Fr::from(note.value),    // value as field element
        note.owner_key,
        note.rho,
        note.randomness,
        DOMAIN_NOTE_COMMITMENT_V1  // capacity = domain tag
    ]
    return poseidon_permutation_t5(state)[0]
```

Uses width-5 Poseidon with rate 4. The domain tag occupies the capacity element.

Output is `state[0]` after the full permutation.

---

## 5. Nullifier Derivation

```text
derive_nullifier(
    nullifier_key : Fr,    // owner's nullifier derivation key
    commitment    : Fr,    // the note commitment being nullified
    leaf_index    : u64    // position in the Merkle tree
) -> Fr:
    index_fr = Fr::from(leaf_index)
    state = [
        nullifier_key,
        commitment,
        index_fr,
        DOMAIN_NULLIFIER_V1  // capacity = domain tag
    ]
    // Use width-5 Poseidon: absorb 3 inputs + domain, capacity = domain tag
    // Actually: state = [nullifier_key, commitment, index_fr, DOMAIN_NULLIFIER_V1, 0]
    // We need 4 rate elements for width 5. Pad with zero.
    state = [
        nullifier_key,
        commitment,
        index_fr,
        Fr::zero(),           // padding
        DOMAIN_NULLIFIER_V1   // capacity = domain tag
    ]
    return poseidon_permutation_t5(state)[0]
```

The nullifier binds to:
- The owner's nullifier key (proves ownership)
- The commitment (identifies which note)
- The leaf index (prevents commitment reuse at different positions)
- Domain tag (prevents cross-usage)

**Privacy property:** Without knowledge of `nullifier_key`, it is infeasible
to link a nullifier to its corresponding commitment.

---

## 6. Key Derivation

```text
// Master spending key (256-bit random, reduced mod p)
spending_key : Fr  // User's master secret

// Derived keys (deterministic, one-way)
owner_spend_pubkey = spending_key  // In MVP, pubkey IS the key itself
                    // (no separate keypair needed for note ownership)

nullifier_key = poseidon_hash_t3(spending_key, DOMAIN_NULLIFIER_V1)
             = poseidon_permutation_t3([spending_key, DOMAIN_NULLIFIER_V1, Fr::zero()])[0]
```

For MVP, the owner's "spending key" is a single `Fr` scalar. The nullifier
derivation key is derived from it via Poseidon. This is simpler than Zcash
Orchard's diversified key hierarchy but sufficient for Nimbus's use case
(single asset, single user per note).

**Future extension:** If multi-asset or delegated spending is needed, extend to:
```text
spending_key -> derive(spending_key, asset_id) -> per-asset key
```

---

## 7. Incremental Merkle Tree

### 7a. Parameters

| Parameter | Value |
|---|---|
| Hash function | Poseidon width-5 (rate 4) with Merkle domain tag |
| Depth | 20 (supports 2^20 = 1,048,576 leaves) |
| Empty leaf | `Fr::zero()` |
| Leaf value | `note_commitment(note)` |

### 7b. Node hash

```text
merkle_hash(left: Fr, right: Fr) -> Fr:
    state = [
        left,
        right,
        Fr::zero(),           // padding
        Fr::zero(),           // padding
        DOMAIN_MERKLE_NODE_V1 // capacity = domain tag
    ]
    return poseidon_permutation_t5(state)[0]
```

### 7c. Empty subtree hashes (precomputed)

```text
EMPTY_HASH[0] = Fr::zero()  // empty leaf
EMPTY_HASH[i] = merkle_hash(EMPTY_HASH[i-1], EMPTY_HASH[i-1])
                for i in 1..=20
```

These 21 constants are computed once at deployment and stored.

### 7d. On-chain storage (append-only)

The contract stores:
- `note_tree_next_index: u64` — next leaf position
- `note_tree_roots: mapping(u64 => Fr)` — maps block number to root at that block
  (or a bounded ring buffer of recent roots)
- `note_tree_frontier: [Fr; 21]` — the "frontier" (right-most path) for O(log n) append

**Append algorithm (on-chain, O(depth) = O(20)):**
```text
function append_leaf(leaf: Fr):
    index = note_tree_next_index
    current = leaf
    for level in 0..20:
        if index % 2 == 0:
            // Left child: sibling is empty
            frontier[level] = current
            sibling = EMPTY_HASH[level]
        else:
            // Right child: sibling is stored frontier
            sibling = frontier[level]
        current = merkle_hash(current, sibling) if index % 2 == 0
                else merkle_hash(sibling, current)
        index = index / 2
    note_tree_root = current
    note_tree_next_index += 1
```

### 7e. Root history

The contract maintains a bounded ring buffer of recent roots:

```text
ROOT_HISTORY_SIZE = 100  // configurable

root_history: [Fr; ROOT_HISTORY_SIZE]
root_history_index: u64

function record_root():
    root_history[root_history_index % ROOT_HISTORY_SIZE] = note_tree_root
    root_history_index += 1
```

Proofs can reference any root in the history, accommodating the delay between
proof generation and on-chain submission.

### 7f. Membership proof (off-chain)

```text
struct MerkleProof {
    leaf_index: u64,
    siblings: [Fr; 20],  // sibling hashes at each level
}

function verify_membership(root: Fr, leaf: Fr, proof: MerkleProof) -> bool:
    current = leaf
    index = proof.leaf_index
    for level in 0..20:
        sibling = proof.siblings[level]
        current = if index % 2 == 0:
            merkle_hash(current, sibling)
        else:
            merkle_hash(sibling, current)
        index = index / 2
    return current == root
```

---

## 8. Input/Output Limits

| Parameter | Value | Rationale |
|---|---|---|
| Max input notes per spend | 4 | Covers typical consolidation; keeps circuit size bounded |
| Max output notes per spend | 2 | 1 merchant payout (public) + 1 change note (private) |
| Min note value | 1 (1 micro-USDC) | Prevents dust; 0 is forbidden |
| Max note value | 2^64 - 1 | Field element range; practical limit |
| Dust threshold | 1000 (0.001 USDC) | Notes below this are rejected to prevent dust attacks |

**MVP simplification:** For the first implementation, limit to:
- **1 input note, 1 change output note** (plus public merchant payout)
- Multi-input support added in a future circuit version

---

## 9. Value Conservation

For every private spend transaction:

```text
sum(input_note.value for each input note)
  = merchant_payout_amount
  + protocol_fee
  + execution_fee
  + sum(output_change_note.value for each change note)
```

All arithmetic uses checked `u64` operations. Overflow is a hard error.

The ZK circuit enforces this as an exact equality constraint over `Fr`:
```text
circuit_constraint:
    sum_inputs == merchant_amount + protocol_fee + execution_fee + sum_changes
```

**Critical:** This constraint MUST be in the circuit. The contract does NOT
independently verify value conservation — it trusts the ZK proof.

---

## 10. Zero-Value Note Rules

- **Zero-value output notes are FORBIDDEN.** The circuit rejects any output
  commitment whose value is 0.
- **Full-balance spend:** When `input_value == merchant + fees`, the change
  value is 0. In this case, no change note is created. The circuit must handle
  this case: either `change_value > 0` (create change note) or
  `change_value == 0` (no change commitment, output_commitments[1] = 0).
- **Dust rejection:** Notes with value < dust_threshold (1000 = 0.001 USDC)
  are rejected at creation time.

---

## 11. ZK Statement (Public Inputs)

```text
Public inputs (all Fr elements, serialized as 32-byte big-endian):
  1. note_root              : Fr    // Merkle root (from accepted history)
  2. input_nullifier_0      : Fr    // nullifier of consumed note 0
  3. input_nullifier_1      : Fr    // nullifier of consumed note 1 (or 0 if unused)
  4. input_nullifier_2      : Fr    // (or 0 if unused)
  5. input_nullifier_3      : Fr    // (or 0 if unused)
  6. output_commitment_0    : Fr    // change note commitment (or 0 if no change)
  7. output_commitment_1    : Fr    // (reserved, 0 in MVP)
  8. recipient              : Fr    // merchant address (zero-padded to Fr)
  9. merchant_amount        : Fr    // exact payout in base units
  10. protocol_fee          : Fr    // protocol fee in base units
  11. execution_fee         : Fr    // relayer execution fee in base units
  12. quote_hash            : Fr    // keccak256 of signed quote (binding)
  13. chain_id              : Fr    // EVM chain ID
  14. contract_address      : Fr    // contract address (zero-padded)
  15. expiry                : Fr    // transaction expiry timestamp
  16. num_inputs            : Fr    // number of active input notes (1-4)
  17. has_change            : Fr    // 1 if change note exists, 0 otherwise
```

Total: 17 public inputs → requires 18 IC points in Groth16 VK.

**Every public input MUST appear in at least one circuit constraint.**
Unconstrained public inputs are a critical vulnerability (prover can supply
arbitrary values that pass verification).

---

## 12. ZK Private Witnesses

```text
Private witnesses (not visible to contract):
  For each input note i (0..num_inputs):
    - note_value[i]         : Fr
    - owner_key[i]          : Fr    // spending key
    - nullifier_key[i]      : Fr    // derived nullifier key
    - rho[i]                : Fr    // note identifier
    - randomness[i]         : Fr    // commitment trapdoor
    - leaf_index[i]         : Fr    // position in Merkle tree
    - merkle_path[i][0..20] : [Fr; 20]  // sibling hashes

  For each output change note (if has_change):
    - change_value          : Fr
    - change_owner_key      : Fr
    - change_rho            : Fr
    - change_randomness     : Fr
```

---

## 13. Circuit Constraints (summary)

The circuit MUST prove:

1. **Membership:** For each input note, `verify_membership(note_root, commitment_i, merkle_path_i) == true`
2. **Commitment correctness:** `commitment_i == note_commitment(value_i, owner_key_i, rho_i, randomness_i)`
3. **Ownership:** `nullifier_key_i == poseidon(spending_key_i, DOMAIN_NULLIFIER_V1)`
4. **Nullifier correctness:** `nullifier_i == derive_nullifier(nullifier_key_i, commitment_i, leaf_index_i)`
5. **Output commitment correctness:** `output_commitment_0 == note_commitment(change_value, change_owner_key, change_rho, change_randomness)` (if has_change)
6. **Value conservation:** `sum(value_i) == merchant_amount + protocol_fee + execution_fee + change_value`
7. **Range:** All values are in `[0, 2^64)` (range constraint before field arithmetic)
8. **Non-zero inputs:** `value_i > 0` for all active inputs
9. **No duplicate inputs:** `rho_i != rho_j` for i != j
10. **Binding:** All public inputs (recipient, fees, quote_hash, chain_id, contract_address, expiry) are used in constraints

---

## 14. Initial Note (Deposit → Note)

When a deposit is confirmed and revealed:

```text
gross_deposit = amount_transferred
deposit_fee = ceiling_div(gross_deposit * 20, 10000)  // 0.20%
net_value = gross_deposit - deposit_fee

initial_note = PrivateNoteV1 {
    value: net_value,
    owner_key: depositor_spending_key,  // provided during deposit
    rho: poseidon(DOMAIN_INITIAL_NOTE_V1, session_id),  // deterministic from session
    randomness: random Fr,
}

initial_commitment = note_commitment(initial_note)
```

The contract inserts `initial_commitment` into the Merkle tree upon successful
reveal. The depositor receives the note preimage via the reveal response
(encrypted to their key, or returned directly in MVP since the session is
already authenticated).

**BLS role:** Threshold BLS signing authorizes the issuance. The guardians
verify the deposit is confirmed and sign a blinded note commitment. This
prevents the relayer from minting notes without collateral.

---

## 15. Migration from Legacy

### What is preserved
- Deposit/reveal lifecycle (session tracking)
- BLS threshold signing infrastructure
- EIP-2537 precompile integration
- Relayer batching and settlement queue
- Fee calculation helpers (bps, ceiling division)
- EIP-712 signing primitives

### What is replaced
- BLS spend verification → ZK note spend verification
- `total_deposited_principal` → `user_note_liability`
- Legacy nullifier (`keccak256(hm_evm_bytes)`) → Poseidon nullifier
- `clean_association_roots` → `note_tree_root` + root history
- Voucher spend ABI → note spend ABI

### Deployment
- Fresh contract deployment required
- Legacy contract remains for refund-only path
- No automatic migration of balances
- Legacy sessions can still claim refunds via old path

---

## 16. Known-Answer Vector Seeds

For test vector generation, use these deterministic seeds:

```text
SEED_SPENDING_KEY     = keccak256("nimbus.test.spending_key.v1") mod p
SEED_NOTE_RHO         = keccak256("nimbus.test.rho.v1") mod p
SEED_NOTE_RANDOMNESS  = keccak256("nimbus.test.randomness.v1") mod p
SEED_NULLIFIER_KEY    = keccak256("nimbus.test.nullifier_key.v1") mod p
SEED_CHAIN_ID         = 421614  // Arbitrum Sepolia
SEED_CONTRACT_ADDRESS = 0x00000000000000000000000072fa2ccfb2ac20fd98747ac407f8331f0f5fbd52
SEED_ASSET_ID         = 1       // USDC
SEED_VALUE            = 99800000  // 99.8 USDC (100 - 0.2% fee)
SEED_MERCHANT_AMOUNT  = 5000000   // 5 USDC
SEED_PROTOCOL_FEE     = 12500     // 0.0125 USDC (0.25% of 5 USDC)
SEED_EXECUTION_FEE    = 23000     // 0.023 USDC
SEED_LEAF_INDEX       = 0
SEED_RECIPIENT        = 0x00000000000000000000000023e32D309c575A3D5E7CD2867BE12B00efa44Bb1
SEED_QUOTE_HASH       = keccak256("nimbus.test.quote.v1")
SEED_EXPIRY           = 0  // no expiry
```

Vectors MUST be computed independently in:
1. `nimbus-core` (Rust, native + circuit)
2. `nimbus-sdk` (WASM)
3. Reference Python implementation (for cross-validation)

All three MUST produce identical outputs.

---

## 17. Research Basis

### Primary sources

1. **Zcash Protocol Specification** (NU6.1, 2025):
   - Orchard note tuple: `(d, pkd, v, ρ, ψ, rcm)`
   - Nullifier: `DeriveNullifier_nk(ρ, ψ, cm)` using Poseidon
   - Commitment: `NoteCommitOrchard_rcm(repr(gd), repr(pkd), v, ρ, ψ)` using Sinsemilla
   - Source: https://zips.z.cash/protocol/protocol.pdf

2. **ZIP 212** (2020, activated):
   - Note plaintext version 0x02 with `rseed` replacing `rcm`
   - Key derivation from `rseed` for both `esk` and `rcm`
   - Source: https://zips.z.cash/zip-0212

### Independent references

3. **Privacy Boost Protocol** (2026):
   - UTXO model with Poseidon2 hashing
   - LeanIMT (Lean Incremental Merkle Tree) depth 20
   - Multi-tree architecture with rollover
   - Note structure with value, owner, randomness
   - Source: https://docs.privacyboost.io/technical-explainer/protocol

4. **Penumbra Poseidon Parameter Generation** (2024):
   - Systematic parameter selection procedure
   - S-box α selection: try smallest α where gcd(α, p-1) = 1
   - Round numbers from paper + security margin (+2 full, +7.5% partial)
   - MDS matrix: Cauchy construction with small deterministic values
   - Source: https://protocol.penumbra.zone/main/crypto/poseidon/paramgen.html

5. **EIP-5988** (Poseidon precompile proposal, 2024):
   - MDS matrix must be bundled with round constants
   - Cauchy matrix: `M[i][j] = 1 / (x_i + y_j)`
   - Security analysis: matrix and constants must not be picked independently
   - Source: https://eips.ethereum.org/EIPS/eip-5988

6. **RAILGUN documentation** (2025-2026):
   - UTXO commitments and nullifiers as public circuit outputs
   - Private balances via encrypted notes
   - Shield/unshield for public↔private conversion
   - Source: https://docs.railgun.org/wiki/learn/using-private-tokens

### Design differences from Zcash Orchard

Nimbus simplifies several aspects:

| Aspect | Zcash Orchard | Nimbus PrivateNoteV1 |
|---|---|---|
| Note fields | 6 (d, pkd, v, ρ, ψ, rcm) | 4 (value, owner_key, rho, randomness) |
| Commitment hash | Sinsemilla | Poseidon width-5 |
| Nullifier | Group-based PRF + Poseidon | Poseidon width-5 (3 inputs + domain) |
| Key hierarchy | Diversified (d, pkd, ask, nsk, nk) | Single spending_key → derived nullifier_key |
| Tree | Incremental Merkle (Sapling/Orchard) | LeanIMT with Poseidon |
| Recipient | Private (encrypted memo) | Public (EVM address) |
| Amount | Private | Public (merchant payout visible) |

Nimbus intentionally keeps recipient and amount public because the primary
use case is merchant payment where the recipient must know they were paid
and how much. Privacy protects the sender's identity, balance, and
transaction history — not the payment details.
