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
  - [`DEC-016A`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-016A-private-note-spec-freeze.md) (Private Note Specification & Public Inputs)
  - [`DEC-022`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-022-defense-against-proof-settlement-mismatch-boundary-gaps.md) (Proof-Settlement Mismatch Defense)

---

## 1. Problem Statement & Motivation

In [`DEC-035`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-035-cryptographic-canonicality-public-input-binding-and-mmr.md), the consensus between Claude (CTO), ChatGPT (Security Reviewer), and Gemini (Repo Investigator) resolved the fundamental conflict between:
1. Standard EVM Keccak-256 EIP-712 digests, which are 256-bit integers in $[0, 2^{256}-1]$.
2. BLS12-381 scalar field elements $\mathbb{F}_r$, where $r \approx 2^{254.85}$.

If a 256-bit quote hash exceeds $r$, importing it as a single EVM scalar into Groth16 causes immediate deserialization failure (`from_evm_scalar >= r`) or non-canonical alias vulnerabilities.

While rejection sampling (retrying nonce until hash $< r$) was considered as a quick hotfix, the Founder's Zero-Tech-Debt Directive explicitly chose the **Two-Limb 128-bit representation**:
- Zero tech debt: permanently eliminates hash rejection loops.
- Standard compliant: client wallets sign normal EIP-712 Keccak-256 digests.
- Calldata preserved: smart contract external entrypoint keeps `quote_hash: FixedBytes<32>`.

---

## 2. Invariants

- **INV-1 (Entropy & Soundness Preservation):** The decomposition MUST preserve the full 256-bit cryptographic entropy of the EIP-712 signing digest:
  $$\text{quote\_hash} \equiv H_{\text{hi}} \cdot 2^{128} + H_{\text{lo}} \pmod{2^{256}}$$
- **INV-2 (Fail-Closed Field Embedding):** Since $2^{128} \ll r$, both $H_{\text{hi}}$ and $H_{\text{lo}}$ are strictly canonical in $\mathbb{F}_r$ without modular reduction.
- **INV-3 (Range Soundness in R1CS):** The circuit MUST enforce that both $H_{\text{hi}}$ and $H_{\text{lo}}$ reside strictly in $[0, 2^{128}-1]$ via bit-decomposition constraints.

---

## 3. Specification

### A. Decomposition Definition
Given the 32-byte Keccak-256 digest:
$$\text{quote\_hash} = \text{keccak256}(\texttt{"\textbackslash x19\textbackslash x01"} \parallel \text{domainSeparator} \parallel \text{structHash})$$

Split into Big-Endian byte arrays:
- `quote_hash_hi`: first 16 bytes ($b_0 \dots b_{15}$), left-padded with 16 zero bytes into a 32-byte word.
- `quote_hash_lo`: last 16 bytes ($b_{16} \dots b_{31}$), left-padded with 16 zero bytes into a 32-byte word.

Mathematically:
$$H_{\text{hi}} = \sum_{i=0}^{15} b_i \cdot 256^{15-i} \in [0, 2^{128}-1]$$
$$H_{\text{lo}} = \sum_{i=16}^{31} b_i \cdot 256^{31-i} \in [0, 2^{128}-1]$$

### B. Circuit Constraints (`nimbus-core`)
In `nimbus-core/src/note_circuit.rs`:
1. Allocate two public input variables:
   ```rust
   let quote_hash_hi_var = cs.new_input_variable(|| Ok(self.quote_hash_hi))?;
   let quote_hash_lo_var = cs.new_input_variable(|| Ok(self.quote_hash_lo))?;
   ```
2. Implement and enforce 128-bit range checks using bit-decomposition gadget:
   ```rust
   enforce_u128_range(&mut cs, &quote_hash_hi_var)?;
   enforce_u128_range(&mut cs, &quote_lo_var)?;
   ```
   Each variable is constrained to be equal to $\sum_{i=0}^{127} 2^i \cdot b_i$, where each $b_i \in \{0, 1\}$ is constrained by $b_i \cdot (1 - b_i) = 0$.

### C. Smart Contract Unpacking (`nimbus-contracts`)
In `nimbus-contracts/src/spend.rs`:
The external function signature remains:
```rust
pub fn spend_private_note(
    &mut self,
    proof: Bytes,
    // ... other public fields ...
    quote_hash: FixedBytes<32>,
    // ...
)
```
Internally, Stylus splits `quote_hash`:
```rust
let quote_hi_bytes = &quote_hash.as_slice()[0..16];
let quote_lo_bytes = &quote_hash.as_slice()[16..32];

let mut quote_hi = [0u8; 32];
quote_hi[16..32].copy_from_slice(quote_hi_bytes);

let mut quote_lo = [0u8; 32];
quote_lo[16..32].copy_from_slice(quote_lo_bytes);
```
Both are passed as consecutive 32-byte EVM words into the Groth16 public inputs array.

### D. Verifying Key & Public Input Count
- Total public inputs expand from **15** to **16** elements.
- Verifying Key $IC$ elements expand from **16** to **17** points ($IC[0 \dots 16]$ in $G_1$).
- Regenerate development keys using seed `NOTE_CIRCUIT_SETUP_SEED = 0x4e696d6275734e43`.

---

## 4. Test & Verification Plan

1. **Unit Test (`nimbus-core`):**
   - Test canonical decomposition with extreme values ($0$, $2^{128}-1$, $2^{256}-1$).
   - Negative test: supply witness with 129th bit set; verify R1CS constraint satisfaction fails.
2. **Integration Test (`nimbus-sdk` & `nimbus-contracts`):**
   - Sign an EIP-712 quote with known digest.
   - Unpack into limbs, prove Groth16, verify via Stylus `groth16_note_verifier`.
   - Verify that altering 1 bit in either `quote_hash_hi` or `quote_hash_lo` causes Groth16 verification revert.
