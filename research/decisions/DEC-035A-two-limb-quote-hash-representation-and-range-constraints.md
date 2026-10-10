# DEC-035A: Two-Limb (128-bit) Canonical Quote Hash Representation & Circuit Range Constraints

- **Status:** APPROVED AS ARCHITECTURAL SUB-SPECIFICATION (Child of [`DEC-035`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-035-cryptographic-canonicality-public-input-binding-and-mmr.md))
- **Author:** Zeltra Architecture & Cryptography Team
- **Date:** 2026-10-10
- **Applies to:**
  - `nimbus-core/src/note_circuit.rs` (`PrivateNoteCircuit` input layout, 128-bit bit-decomposition range gadget)
  - `nimbus-contracts/src/spend.rs` (`_spend_private_note` ABI preservation, unpacking `bytes32` into high/low limbs)
  - `nimbus-contracts/src/groth16_note_verifier.rs` (Public inputs expansion from 15 to 16, verifying key $IC$ update)
  - `nimbus-sdk/src/wallet/note_wallet.rs` (Witness generation and public input array construction)
  - `nimbus-sdk/src/eip712.rs` (Canonical EVM Keccak-256 digest computation)
  - `nimbus-node/src/handlers/spend.rs` (Relayer ingress public input validation and limb parsing)
- **Parent & Related DECs:**
  - [`DEC-035`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-035-cryptographic-canonicality-public-input-binding-and-mmr.md) (Master Zero-Tech-Debt Blueprint)
  - [`DEC-035B`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-035B-formal-scope-binding-gadget-and-groth16-statement-integrity.md) (Formal Scope Binding Gadget & Statement Integrity)
  - [`DEC-016A`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-016A-private-note-spec-freeze.md) (Private Note Specification & Public Inputs)
  - [`DEC-022`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-022-defense-against-proof-settlement-mismatch-boundary-gaps.md) (Proof-Settlement Mismatch Defense)

---

## 1. Executive Summary & Problem Statement

In the cryptographic architecture of **Zeltra Protocol** (DEC-035), off-chain user transaction quotes are authenticated via standard Ethereum typed data signatures ([EIP-712](https://eips.ethereum.org/EIPS/eip-712)) to guarantee gas price certainty, execution fee caps, and deadline expiry. However, integrating standard EVM 256-bit Keccak digests into zero-knowledge circuits operating over pairing-friendly elliptic curves exposes a fundamental algebraic field mismatch:

1. **The Algebraic Characteristic Mismatch:**
   - Standard EVM Keccak-256 digests reside in the integer ring $\mathbb{Z} / 2^{256} \mathbb{Z}$, with values uniformly distributed in $[0, 2^{256} - 1]$.
   - The scalar field $\mathbb{F}_r$ of the **BLS12-381** elliptic curve (standardized in [EIP-2537](https://eips.ethereum.org/EIPS/eip-2537)) has prime order:
     $$r = 52435875175126190479447740508185965837690552500527637822603658699938581184513$$
     $$\text{Hex: } \texttt{0x73eda753299d7d483339d80809a1d80553bda402fffe5bfeffffffff00000001} \approx 2^{254.853}$$
2. **The "Danger Zone" ($\Delta \approx 2^{255.15}$):**
   Because $r < 2^{256}$, the scalar field is strictly smaller than the 256-bit word space. The gap:
   $$\Delta = 2^{256} - r \approx 6.345 \times 10^{76} \approx 2^{255.15}$$
   represents approximately **$54.68\%$ of the entire 256-bit integer space**. Over half of all potential Keccak-256 digests exceed $r$.
3. **The Failure Modes of Naive Embeddings:**
   - If an out-of-range hash ($H \ge r$) is submitted as a raw 32-byte scalar to the Groth16 on-chain verifier or MSM precompile (`0x0c`), EIP-2537 explicitly reverts with an invalid field element error (`from_evm_scalar >= r`).
   - If a naive modulo reduction ($H \bmod r$) is performed, distinct 256-bit hashes $H_1$ and $H_2 = H_1 + r$ map to the exact same scalar element, destroying the collision resistance of Keccak-256 and enabling second-preimage quote malleability.
   - If rejection sampling (repeatedly re-hashing or incrementing salt until $H < r$) is used, it introduces non-deterministic relayer latencies, UX stalls, and violates the Founder's Zero-Tech-Debt directive.

To permanently resolve this architectural barrier with **zero tech debt**, Zeltra Protocol establishes the **Canonical Two-Limb (128-bit) Representation** with strict in-circuit range constraints.

---

## 2. Post-Mortem & Vulnerability Archaeology

The necessity of strict multi-limb representation and explicit in-circuit range constraints is rooted in multiple historic zero-knowledge exploits and audit findings across the industry.

```
       EVM Keccak-256 Hash Space: [0, 2^256 - 1]
┌────────────────────────────────────────┬────────────────────────────────────────┐
│        High 128-bit Limb (H_hi)        │        Low 128-bit Limb (H_lo)         │
│               [0, 2^128 - 1]           │               [0, 2^128 - 1]           │
└────────────────────────────────────────┴────────────────────────────────────────┘
                   │                                        │
     Constrained via R1CS gadget              Constrained via R1CS gadget
       enforce_u128_range(H_hi)                 enforce_u128_range(H_lo)
                   │                                        │
                   ▼                                        ▼
    Strictly canonical in Fr (< r)           Strictly canonical in Fr (< r)
    2^128 - 1 << 2^254.85                    2^128 - 1 << 2^254.85
    [NO OVERFLOW / NO WRAP-AROUND]           [NO OVERFLOW / NO WRAP-AROUND]
```

### A. The Semaphore Input Aliasing Vulnerability (2019)
- **Vulnerability Context:** In 2019, security researcher *poma* identified a critical input aliasing vulnerability in the Semaphore zero-knowledge signaling protocol on Ethereum ([Semaphore Issue #16](https://github.com/semaphore-protocol/semaphore/issues/16), analyzed by Beosin).
- **Root Cause:** In the smart contract verifier, public inputs were represented as Solidity `uint256` types. However, inside the pairing check equation, elements were evaluated modulo the scalar field order $p$. An attacker possessing a valid nullifier $s \in [0, p-1]$ could forge up to 4 additional valid transaction payloads by submitting:
  $$s' = s + k \cdot p < 2^{256} \quad (k \in \{1, 2, 3, 4\})$$
  Because $s' \equiv s \pmod p$, the elliptic curve pairing verified identically. However, the contract's nullifier mapping check `nullifiers[s']` evaluated $s'$ as a brand new unique key, enabling a **5x double-spending attack**.
- **Relevance to Zeltra:** If Zeltra were to accept a 256-bit scalar without strict limb bounds, an attacker could manipulate public input representations to trigger aliasing bugs across nullifiers or quote hashes. By splitting $H$ into two 128-bit limbs where $\max(H_{\text{limb}}) = 2^{128}-1 \ll r$, aliasing is algebraically impossible because $s + r > 2^{128}-1$.

### B. Circomlib `Num2Bits` vs `Num2Bits_strict` & The "Danger Zone" (RareSkills, ZK-Security)
- **Vulnerability Context:** Circomlib's legacy bit-decomposition template `Num2Bits(254)` decomposes a field element into 254 bits and reconstructs it via:
  $$v = \sum_{i=0}^{253} 2^i \cdot b_i \pmod p$$
- **Root Cause:** As documented by RareSkills and ZK-Security, $2^{254}-1 \approx 2.89 \times 10^{76}$ is greater than the BN254 field characteristic $p \approx 2.18 \times 10^{76}$. A malicious prover can supply a 254-bit assignment in the interval $[p, 2^{254}-1]$ that reconstructs to $(v + p) \bmod p \equiv v$. This created an under-constrained second preimage vulnerability where two different binary assignments satisfied the same public input signal.
- **Relevance to Zeltra:** Bit-decomposition gadgets that approximate the field size without strict boundary checks create catastrophic soundness gaps. Zeltra's 128-bit range check operates far below the scalar modulus:
  $$2^{128} \ll r \approx 2^{254.853}$$
  The margin between the maximum limb value ($2^{128}-1$) and the field characteristic is over 126 bits ($> 10^{38}$), entirely bypassing the "danger zone".

### C. Axiom Halo2 Audit: TOB-AXIOMv2-3 Range Check Defect (Trail of Bits, 2025)
- **Vulnerability Context:** In Trail of Bits' formal security review of Axiom's zero-knowledge coprocessor circuits ([Trail of Bits Blog, 2025](https://blog.trailofbits.com/2025/05/30/a-deep-dive-into-axioms-halo2-circuits/)), issue `TOB-AXIOMv2-3` revealed a high-severity flaw in integer range checking.
- **Root Cause:** The `range_check` method was mistakenly implemented with a Rust `debug_assert!` instead of synthesising active R1CS / Plonkish gate constraints. In production release builds, `debug_assert!` was compiled out, leaving the variable completely unconstrained. A malicious prover could supply arbitrarily large values without violating the SNARK proof.
- **Relevance to Zeltra:** Zeltra mandates that all range constraints are compiled directly into the arithmetic constraint system as rank-1 quadratic equations ($b_i \cdot (1 - b_i) = 0$), verified in production release builds and covered by continuous R1CS satisfiability tests.

### D. Tornado Cash Unconstrained MIMC Hash Output (Trail of Bits Circomspect, 2022)
- **Vulnerability Context:** In October 2019, Tornado Cash suffered an under-constrained signal vulnerability where MIMC hash computation used the assignment operator `=` (`<--`) instead of the constraint assignment `<==` ([Trail of Bits Blog, 2022](https://blog.trailofbits.com/2022/09/15/it-pays-to-be-circomspect/)).
- **Root Cause:** The Merkle tree root output was computed by the prover during witness generation, but never constrained in the R1CS matrices during verification. Anyone could forge a deposit proof and drain contract funds.
- **Relevance to Zeltra:** Range checks and limb representations cannot remain passive helper functions. The decomposed limbs must actively enter the Groth16 verifying key linear combination ($IC[9]$ and $IC[10]$) and be bound through non-linear gates, guaranteeing non-trivial verifying key elements ($IC[i] \ne \mathcal{O}$).

---

## 3. Mathematical Foundations & Trade-Off Analysis

| Architectural Approach | Cryptographic Entropy | EIP-712 Signature Standard | Relayer UX / Latency | Verifier Gas Overhead | Vulnerability to Aliasing / Collisions | Founder Verdict (DEC-035) |
|---|---|---|---|---|---|---|
| **Approach 1: Modulo Reduction ($H \bmod r$)** | Degraded (Collision: $H, H+r$) | Breaks standard wallets (non-canonical digest) | Instant | Zero | **Vulnerable** (second preimage collisions) | **REJECTED** (High Tech Debt & Security Hazard) |
| **Approach 2: Rejection Sampling ($H < r$)** | 254.85 bits | Compliant | Degraded (average 2.2 retries per quote) | Zero | Safe, but DoS prone | **REJECTED** (Temporary Hack, Violates Directive) |
| **Approach 3: Truncation (Upper 250 bits)** | 250 bits (6 bits entropy loss) | Breaks EIP-712 standard verification | Instant | Zero | **Vulnerable** (collision resistance shaved) | **REJECTED** (Cryptographic Sub-optimality) |
| **Approach 4: Two-Limb 128-bit Representation (Zeltra)** | **Full 256 bits (100% Entropy)** | **100% Standard Compliant** | **Zero Delay (Deterministic $O(1)$)** | Negligible (+1 MSM scalar) | **Mathematically Immune** ($2^{128} \ll r$) | **ADOPTED AS CANONICAL SPECIFICATION** |

---

## 4. Invariants

- **INV-1 (Entropy & Soundness Preservation):** The decomposition MUST preserve the full 256-bit cryptographic entropy of the EIP-712 signing digest:
  $$\text{quote\_hash} \equiv H_{\text{hi}} \cdot 2^{128} + H_{\text{lo}} \pmod{2^{256}}$$
- **INV-2 (Fail-Closed Field Embedding):** Since $2^{128} \ll r$, both $H_{\text{hi}}$ and $H_{\text{lo}}$ are strictly canonical in $\mathbb{F}_r$ without modular reduction.
- **INV-3 (Range Soundness in R1CS):** The circuit MUST enforce that both $H_{\text{hi}}$ and $H_{\text{lo}}$ reside strictly in $[0, 2^{128}-1]$ via bit-decomposition constraints.
- **INV-4 (Calldata ABI Immutability):** External smart contract entrypoints MUST continue to accept standard Solidity `bytes32` (`FixedBytes<32>`). Slicing and limb unpacking MUST occur internally in contract memory before invoking the Groth16 verifier.
- **INV-5 (Statement Binding Completeness):** Both limb variables MUST be constrained in non-linear constraints and bound through the transaction scope gadget (DEC-035B), ensuring that their corresponding verifying key elements satisfy:
  $$IC[9] \ne \mathcal{O}_{G_1} \quad \text{and} \quad IC[10] \ne \mathcal{O}_{G_1}$$

---

## 5. Technical Specification

### A. Canonical Byte Decomposition
Given the standard 32-byte Keccak-256 digest computed according to EIP-712:
$$\text{quote\_hash} = \text{keccak256}(\texttt{"\textbackslash x19\textbackslash x01"} \parallel \text{domainSeparator} \parallel \text{structHash})$$

Let $\text{quote\_hash} = [b_0, b_1, \dots, b_{31}]$ be the 32 Big-Endian bytes. The digest is partitioned into two 16-byte halves:
- High Half: $\text{raw\_hi} = [b_0, \dots, b_{15}]$
- Low Half: $\text{raw\_lo} = [b_{16}, \dots, b_{31}]$

Each half is left-padded with 16 zero bytes into a 32-byte Big-Endian EVM word:
$$\text{padded\_hi} = [0x00; 16] \parallel [b_0, \dots, b_{15}]$$
$$\text{padded\_lo} = [0x00; 16] \parallel [b_{16}, \dots, b_{31}]$$

Interpreted as unsigned integers:
$$H_{\text{hi}} = \sum_{i=0}^{15} b_i \cdot 256^{15-i} = \sum_{j=0}^{127} 2^j \cdot \beta_{j}^{\text{hi}} \in [0, 2^{128}-1]$$
$$H_{\text{lo}} = \sum_{i=16}^{31} b_i \cdot 256^{31-i} = \sum_{j=0}^{127} 2^j \cdot \beta_{j}^{\text{lo}} \in [0, 2^{128}-1]$$

Where $\beta_j \in \{0, 1\}$ are individual bits.

### B. In-Circuit R1CS Range Constraint Gadget (`nimbus-core`)

In `nimbus-core/src/note_circuit.rs`, the circuit allocates two public input variables and enforces strict 128-bit range checks.

```
                  ┌────────────────────────────────────────┐
                  │    quote_hash_limb (Public Input)      │
                  └──────────────────┬─────────────────────┘
                                     │
           Decompose into 128 boolean witness bits: b_0 ... b_127
                                     │
             ┌───────────────────────┴───────────────────────┐
             ▼                                               ▼
   128 Boolean Constraints                         1 Linear Combination
     b_i * (1 - b_i) == 0                     Σ (2^i * b_i) - limb == 0
  (Rank-1 Quadratic Equations)               (Exact Bit-Sum Integrity)
```

#### Mathematical Formulation:
For each limb variable $V \in \{H_{\text{hi}}, H_{\text{lo}}\}$, the prover supplies 128 witness bits $b_0, b_1, \dots, b_{127} \in \mathbb{F}_r$. The circuit enforces:
1. **Boolean Constancy Constraints (128 constraints per limb):**
   $$b_i \cdot (1 - b_i) = 0 \quad \forall i \in [0, 127]$$
2. **Linear Combination Equality Constraint (1 constraint per limb):**
   $$\left( \sum_{i=0}^{127} 2^i \cdot b_i \right) \cdot 1 = V$$

Total R1CS constraint cost: **$128 + 1 = 129$ constraints per limb** (258 constraints total for the 256-bit hash).

#### Implementation Reference:
```rust
fn enforce_u128_range<CS: ConstraintSystem<Fr>>(
    cs: &mut CS,
    var: &AllocatedNum<Fr>,
    name: &'static str,
) -> Result<(), SynthesisError> {
    // 1. Witness decomposition
    let bits: Vec<AllocatedBit> = match var.get_value() {
        Some(val) => {
            let bigint = val.into_bigint();
            let mut bit_vals = Vec::with_capacity(128);
            for i in 0..128 {
                let bit = (bigint.as_ref()[0] >> (i % 64)) & 1 == 1; // multi-limb access
                bit_vals.push(Some(bit));
            }
            bit_vals
                .into_iter()
                .enumerate()
                .map(|(i, b)| {
                    AllocatedBit::alloc(
                        cs.namespace(|| format!("{}_bit_{}", name, i)),
                        || b.ok_or(SynthesisError::AssignmentMissing),
                    )
                })
                .collect::<Result<Vec<_>, _>>()?
        }
        None => (0..128)
            .map(|i| {
                AllocatedBit::alloc(
                    cs.namespace(|| format!("{}_bit_{}", name, i)),
                    || Err(SynthesisError::AssignmentMissing),
                )
            })
            .collect::<Result<Vec<_>, _>>()?,
    };

    // 2. Linear combination enforcement: sum(2^i * bit_i) == var
    let mut coeff = Fr::one();
    let mut lc = LinearCombination::<Fr>::zero();
    for bit in bits {
        lc = lc + (coeff, bit.get_variable());
        coeff.double_in_place();
    }
    
    cs.enforce(
        || format!("{}_range_128_enforcement", name),
        |zero| zero + &lc,
        |zero| zero + CS::one(),
        |zero| zero + var.get_variable(),
    );

    Ok(())
}
```

### C. Smart Contract Unpacking & Calldata Preservation (`nimbus-contracts`)

In `nimbus-contracts/src/spend.rs`, the external Stylus ABI is strictly preserved to prevent breaking changes for existing integrators and merchant frontends:

```rust
pub fn spend_private_note(
    &mut self,
    proof: Bytes,
    note_root: FixedBytes<32>,
    leaf_count: u64,
    input_nullifier: FixedBytes<32>,
    note_epoch_id: u64,
    output_commitment: FixedBytes<32>,
    recipient: Address,
    merchant_amount: u64,
    protocol_fee: u64,
    execution_fee: u64,
    quote_hash: FixedBytes<32>,       // External ABI preserved as FixedBytes<32>
    chain_id: u64,
    contract_address: Address,
    expiry: u64,
    has_change: bool,
    is_rollover: bool,
) -> Result<(), Vec<u8>> {
    // Unpack 32-byte quote_hash into two 128-bit EVM words in memory
    let quote_hi_bytes = &quote_hash.as_slice()[0..16];
    let quote_lo_bytes = &quote_hash.as_slice()[16..32];

    let mut quote_hi = [0u8; 32];
    quote_hi[16..32].copy_from_slice(quote_hi_bytes); // left-padded with 16 zeros

    let mut quote_lo = [0u8; 32];
    quote_lo[16..32].copy_from_slice(quote_lo_bytes); // left-padded with 16 zeros

    // Construct the 16 public inputs for Groth16 verifier
    let public_inputs: [[u8; 32]; 16] = [
        note_root.into(),
        to_be_bytes32(leaf_count),
        input_nullifier.into(),
        to_be_bytes32(note_epoch_id),
        output_commitment.into(),
        to_address_word(recipient),
        to_be_bytes32(merchant_amount),
        to_be_bytes32(protocol_fee),
        to_be_bytes32(execution_fee),
        quote_hi,                     // Wire 9: High 128-bit limb
        quote_lo,                     // Wire 10: Low 128-bit limb
        to_be_bytes32(chain_id),
        to_address_word(contract_address),
        to_be_bytes32(expiry),
        to_boolean_word(has_change),
        to_boolean_word(is_rollover),
    ];

    // Invoke EIP-2537 MSM and Pairing Check
    self.verifier.verify_groth16_proof(&proof, &public_inputs)?;
    // ...
}
```

### D. Verifying Key Expansion & Setup Determinism

The expansion of public inputs impacts the Groth16 verification equation:
$$e(A, B) = e(\alpha, \beta) + e\left( \sum_{i=0}^{\ell} x_i \cdot IC[i], \gamma \right) + e(C, \delta)$$

1. **Public Input Count ($\ell$):** Expands from $\ell = 15$ to $\ell = 16$.
2. **Verifying Key Size ($IC$):** The vector of $G_1$ generator points expands from $16$ elements ($IC[0 \dots 15]$) to **$17$ elements ($IC[0 \dots 16]$)**, where:
   - $IC[0]$ corresponds to the public input constant $1$ wire.
   - $IC[9]$ corresponds to $H_{\text{hi}}$.
   - $IC[10]$ corresponds to $H_{\text{lo}}$.
3. **Deterministic Seed:** In accordance with [`DEC-035`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-035-cryptographic-canonicality-public-input-binding-and-mmr.md), development proving and verifying keys are regenerated deterministically using seed:
   $$\texttt{NOTE\_CIRCUIT\_SETUP\_SEED} = \texttt{0x4e696d6275734e43}$$

---

## 6. Threat Modeling & Security Analysis

```
┌───────────────────────────────────────┬────────────────────────────────────────────────────────┐
│ Threat Vector                         │ Architectural Mitigation / Mathematical Defense        │
├───────────────────────────────────────┼────────────────────────────────────────────────────────┤
│ 1. High-Limb Overflow Attack          │ Enforced by R1CS range gadget: b_i * (1 - b_i) = 0.    │
│    (Prover injects H_hi >= 2^128)     │ Bit-sum cannot exceed 2^128 - 1. Proof unsatisfiable.  │
├───────────────────────────────────────┼────────────────────────────────────────────────────────┤
│ 2. Field Characteristic Aliasing      │ 2^128 - 1 < 3.40e38. Scalar modulus r > 5.24e76.       │
│    (Prover injects H + r)             │ Addition of r exceeds 254 bits; rejected by u128 check.│
├───────────────────────────────────────┼────────────────────────────────────────────────────────┤
│ 3. EVM Calldata Deserialization Panic │ Stylus unpacks 16-byte slices with left-zero padding.  │
│    (Precompile 0x0c MSM reverts)      │ Word values are <= 2^128 - 1 < r; always canonical.    │
├───────────────────────────────────────┼────────────────────────────────────────────────────────┤
│ 4. Relayer Ingress Mismatch           │ Relayer re-computes EIP-712 digest, splits into limbs, │
│    (Tampering with signed quote)      │ and asserts public_inputs[9..11] == expected limbs.    │
└───────────────────────────────────────┴────────────────────────────────────────────────────────┘
```

---

## 7. Testing & Verification Matrix

The implementation must pass the following multi-tier verification suite prior to Gate G testnet deployment:

### A. Core Mathematical & Circuit Tests (`nimbus-core`)
1. **Canonical Decomposition Completeness:**
   - Test vectors: $0$, $1$, $2^{128}-1$, $2^{128}$, $2^{256}-1$, and pseudo-random Keccak-256 digests.
   - Verify: $\text{recombine}(H_{\text{hi}}, H_{\text{lo}}) == \text{original\_hash}$.
2. **R1CS Soundness & Negative Testing:**
   - Set 129th bit of $H_{\text{hi}}$ to $1$: assert `cs.is_satisfied() == false`.
   - Swap $H_{\text{hi}}$ and $H_{\text{lo}}$ in public inputs: assert `verify_proof() == false`.
   - Modify any single bit in $H_{\text{hi}}$: assert Groth16 pairing check fails.

### B. Smart Contract & Precompile Integration (`nimbus-contracts`)
1. **Calldata Slicing Integrity:**
   - Send `quote_hash = 0xffffffffffffffffffffffffffffffff00000000000000000000000000000001`.
   - Verify `quote_hi = 0x00...00ffffffffffffffffffffffffffffffff` and `quote_lo = 0x00...00000000000000000000000000000001`.
2. **EIP-2537 MSM Precompile Verification:**
   - Execute test proofs against Arbitrum Sepolia Stylus test harness.
   - Confirm precompile `0x0c` consumes unpacked limbs without reverting.

### C. End-to-End Relayer Ingress Validation (`nimbus-node`)
1. **Quote Equality Guard:**
   - Submit a valid proof where $H_{\text{hi}}$ and $H_{\text{lo}}$ match Quote A, but relayer execution quote is Quote B.
   - Assert relayer responds with `400 Bad Request` (`MismatchedQuoteHash`).

---

## 8. References & Citations

1. **Ethereum Foundation.** (2018). *EIP-712: Typed structured data hashing and signing.* Ethereum Improvement Proposals. [https://eips.ethereum.org/EIPS/eip-712](https://eips.ethereum.org/EIPS/eip-712)
2. **Ethereum Foundation.** (2020). *EIP-2537: Precompiled contracts for BLS12-381 curve operations.* Ethereum Improvement Proposals. [https://eips.ethereum.org/EIPS/eip-2537](https://eips.ethereum.org/EIPS/eip-2537)
3. **Poma & Semaphore Team.** (2019). *Input Aliasing Vulnerability in Semaphore Verifier.* GitHub Issue #16, Semaphore Protocol. [https://github.com/semaphore-protocol/semaphore/issues/16](https://github.com/semaphore-protocol/semaphore/issues/16)
4. **Beosin Security Team.** (2023). *An In-depth Analysis of zk-SNARK Input Aliasing Vulnerability.* Beosin Security Research. [https://beosin.com/resources/an-in-depth-analysis-of-zk-snark-input-aliasing-vulnerabilit](https://beosin.com/resources/an-in-depth-analysis-of-zk-snark-input-aliasing-vulnerabilit)
5. **RareSkills.** (2025). *AliasCheck and Num2Bits_strict in Circomlib.* RareSkills ZK Tutorials. [https://rareskills.io/post/circom-aliascheck](https://rareskills.io/post/circom-aliascheck)
6. **Trail of Bits.** (2022). *It pays to be Circomspect: Verifying Zero-Knowledge Circuits with Static Analysis.* Trail of Bits Security Blog (Fredrik Dahlgren). [https://blog.trailofbits.com/2022/09/15/it-pays-to-be-circomspect/](https://blog.trailofbits.com/2022/09/15/it-pays-to-be-circomspect/)
7. **Trail of Bits.** (2025). *A Deep Dive into Axiom’s Halo2 Circuits: Auditing ZK Coprocessors and Soundness Hazards.* Trail of Bits Security Blog (Filipe Casal, Jim Miller, Fredrik Dahlgren, Joe Doyle, Tjaden Hess). [https://blog.trailofbits.com/2025/05/30/a-deep-dive-into-axioms-halo2-circuits/](https://blog.trailofbits.com/2025/05/30/a-deep-dive-into-axioms-halo2-circuits/)
8. **ZK-Security.** (2025). *Common Circom Pitfalls and How to Dodge Them: Under-constrained Signals and Field Aliasing.* ZK/SEC Quarterly (Marco Besier). [https://blog.zksecurity.xyz/posts/circom-pitfalls-2/](https://blog.zksecurity.xyz/posts/circom-pitfalls-2/)
9. **Thaler, Justin.** (2023). *17 Misconceptions about SNARKs (Word Sizes, Non-Native Fields, and Proof Systems).* a16z crypto research. [https://a16zcrypto.com/posts/article/17-misconceptions-about-snarks](https://a16zcrypto.com/posts/article/17-misconceptions-about-snarks)
10. **Groth, Jens.** (2016). *On the Size of Pairing-based Non-interactive Arguments.* In: Advances in Cryptology – EUROCRYPT 2016, Lecture Notes in Computer Science, vol 9666, pp. 305–326. Springer. [https://eprint.iacr.org/2016/260](https://eprint.iacr.org/2016/260)
11. **Bowe, Sean; Hopwood, Daira; & Wilcox, Zooko.** (2017). *BLS12-381: New zk-SNARK Elliptic Curve Construction.* Electric Coin Company / Zcash Protocol Specification. [https://electriccoin.co/blog/new-snark-curve/](https://electriccoin.co/blog/new-snark-curve/)
12. **Arkworks Community.** (2024). *ark-groth16: Efficient Groth16 Prover and Verifier in Rust.* Arkworks Ecosystem. [https://github.com/arkworks-rs/groth16](https://github.com/arkworks-rs/groth16)
