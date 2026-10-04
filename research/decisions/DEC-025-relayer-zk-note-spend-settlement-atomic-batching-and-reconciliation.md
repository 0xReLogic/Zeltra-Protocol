# DEC-025: Relayer ZK Note Spend Settlement, Atomic Batching, and Accrual Reconciliation

**Status:** Accepted  
**Date:** October 2026  
**Applies to:** `nimbus-node/src/handlers/spend.rs`, `nimbus-node/src/evm_client.rs`, `nimbus-node/src/database.rs`, `nimbus-node/src/main.rs`, `nimbus-node/src/dto.rs`  
**Related DECs:** DEC-016 (ZK-UTXO Model), DEC-016A (Nullifier & Circuit Invariants), DEC-016B (Range Constraints), DEC-017 (Receipt Finality & Nonce Lock), DEC-018 (Deposit Indexer & Cryptographic Reveal), DEC-019 (Input Sanitization & Post-Restart Reconciliation), DEC-020 (Batch Profitability & Operational Fee Metrics), DEC-022 (Defense Against Proof-Settlement Mismatch Boundary Gaps), DEC-023 (Siloed Nullifiers & Replay Immunity), DEC-024 (Client Private Note Wallet State)

---

## 1. Context & Vulnerability Vectors (2025–2026 Post-Mortems & Papers)

While **Gate D** ([`nimbus-contracts/src/spend.rs`](file:///workspaces/Zeltra-Protocol/nimbus-contracts/src/spend.rs)) enforces on-chain zero-knowledge verification via EIP-2537 precompiles and **Gate E** ([`nimbus-sdk/src/wallet/note_wallet.rs`](file:///workspaces/Zeltra-Protocol/nimbus-sdk/src/wallet/note_wallet.rs)) manages client UTXO lifecycle and local proof generation, **Gate F (Relayer ZK Note Spend Settlement)** is the critical operational boundary where untrusted client zero-knowledge proofs are received, verified, queued, and dispatched on-chain.

Recent major exploits across decentralized privacy and settlement systems highlight the failure modes this design must mathematically eliminate:

1. **The Aztec Connect Boundary Gap Exploit (June 2026 — $2.28M Drain):**
   - *Reference:* [Rekt News: Aztec Connect Exploit (June 2026)](https://rekt.news/?tag=BTC) & [`research/decisions/DEC-022-defense-against-proof-settlement-mismatch-boundary-gaps.md`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-022-defense-against-proof-settlement-mismatch-boundary-gaps.md).
   - *Exploit Mechanism:* Aztec Connect suffered a critical boundary mismatch where the transaction set processed by the ZK proof circuit diverged from the transaction parameters decoded and settled by the smart contract. Attackers exploited this parameter discrepancy to mint unbacked rollup balances and drain liquid collateral.
   - *Nimbus Mandate:* As formalized in [`DEC-022`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-022-defense-against-proof-settlement-mismatch-boundary-gaps.md) and [`DEC-023`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-023-siloed-nullifiers-and-cross-domain-replay-immunity.md), every parameter passed to `spend_private_note(...)` must be cryptographically locked to the proof's public inputs. The relayer node must perform **strict pre-flight semantic consistency checks** before placing the spend into its transaction queue.
2. **The Payy Network Range-Proof Cache Collision (September 2026 — $320M Breach):**
   - *Reference:* [Avoid.net Payy Network Security Report (September 2026)](https://www.avoid.net/).
   - *Exploit Mechanism:* A stablecoin payment ZK-rollup on Ethereum allowed unbacked note redemptions due to integer wraparound in finite field arithmetic ($\mathbb{F}_r$) coupled with improper range-proof verification at the relayer ingestion layer.
   - *Nimbus Mandate:* In addition to on-chain checks ([`DEC-016B`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-016B-mvp-circuit-shortcuts.md)), the relayer must verify that all amounts ($\text{merchant\_amount}, \text{protocol\_fee}, \text{execution\_fee}$) satisfy strict 64-bit bounds and exact sum conservation ($\text{input\_amount} = \text{payout} + \text{protocol\_fee} + \text{execution\_fee} + \text{change\_amount}$) before spending relayer gas.
3. **Mempool Front-Running & Amount Correlation Attacks (WabiSabi IACR 2021/206):**
   - *Reference:* [`jurnal/building-blocks/WabiSabi-Centrally-Coordinated-CoinJoins.md`](file:///workspaces/Zeltra-Protocol/jurnal/building-blocks/WabiSabi-Centrally-Coordinated-CoinJoins.md) & Full Paper PDF [`jurnal/pdf/2021-206.pdf`](file:///workspaces/Zeltra-Protocol/jurnal/pdf/2021-206.pdf).
   - In private UTXO systems, naive public batching allows external adversaries to link spenders with change notes by correlating input and change amounts across block transactions.
   - *Nimbus Mandate:* The relayer batching mechanism must enforce discrete fee and amount structures, disallowing arbitrary fractional fee leakage as outlined in [`jurnal/filter/03-relayer-innovations.md`](file:///workspaces/Zeltra-Protocol/jurnal/filter/03-relayer-innovations.md#3-wabisabi-variable-amount-coinjoin-decomposition-iacr-eprint-2021206).
4. **Controlled Oversight & Auditing Budget Enforcement (AuditPay IACR 2026/05):**
   - *Reference:* [`jurnal/honorable/AuditPay-Anonymous-Payments-Controlled-Oversight.md`](file:///workspaces/Zeltra-Protocol/jurnal/honorable/AuditPay-Anonymous-Payments-Controlled-Oversight.md) & [`jurnal/filter/03-relayer-innovations.md`](file:///workspaces/Zeltra-Protocol/jurnal/filter/03-relayer-innovations.md#1-auditpay-anonymous-payments-with-controlled-oversight-iacr-eprint-202605).
   - *Concept:* Enforce fail-closed quote and query budgets so relayer nodes can mathematically prove adherence to non-surveillance bounds while satisfying commercial quote signatures.
5. **Relayer Crash Desynchronization & "Ghost Queue" Deadlocks:**
   - *Reference:* [`research/decisions/DEC-019-relayer-input-sanitization-safety-limits-and-post-restart-reconciliation.md`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-019-relayer-input-sanitization-safety-limits-and-post-restart-reconciliation.md).
   - If a relayer node crashes while a transaction is in-flight (`broadcasting`), upon reboot it must not re-submit a transaction that was already mined on-chain (wasting gas and causing on-chain reverts), nor should it abandon transactions that were never mined.
   - *Nimbus Mandate:* The relayer requires **atomic two-phase leasing** in SQLite and **post-restart contract nullifier reconciliation** (extending DEC-019 to ZK private note nullifiers).

---

## 2. Decision & Architecture for Gate F (`nimbus-node`)

To achieve fail-closed settlement and zero-loss operational robustness, `nimbus-node` implements the following architecture:

```
[ Client SDK (PrivateNoteWallet) ]
               │
               │ POST /api/v1/spend-private-note
               │ (SpendProofPayload + Signed EIP-712 Quote)
               ▼
[ Nimbus Node: Ingress Handler ]
  ├─ 1. Idempotency Check (Cached Response)
  ├─ 2. Canonical Field Scalar & Non-Zero Proof Checks (< Fr modulus)
  ├─ 3. Fail-Closed Solvency Conservation: Input == Payout + ProtocolFee + ExecFee + Change
  ├─ 4. EIP-712 Signed Quote Verification (Fee <= MaxFee, Expiry, Nonce)
  ├─ 5. Double-Spend Pre-Check (Local SQLite + On-Chain note_nullifiers)
  └─ 6. Atomic SQLite Enqueue (spend_queue: status='queued')
               │
               ▼
[ Background Settlement Worker ]
  ├─ 1. Atomic Lease Claim (lease_until = now + 120s)
  ├─ 2. Nonce Lock & Gas Price Fetching (+15% bump on replacement)
  ├─ 3. Broadcast to Arbitrum Sepolia (spend_private_note)
  └─ 4. Receipt Finality Watchdog (Safe block confirmations)
               │
               ▼
[ On-Chain Confirmation & Accrual ]
  ├─ Status -> 'confirmed', record tx_hash & block_number
  ├─ Update local execution_fee accrual
  └─ Index Change Note Commitment for Client Oblivious Discovery
```

---

### A. Ingress API Endpoint: `/api/v1/spend-private-note`

The relayer exposes a dedicated endpoint accepting `PrivateNoteSpendRequest`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivateNoteSpendRequest {
    pub note_root: String,            // 0x-prefixed 32-byte hex
    pub input_nullifier: String,      // 0x-prefixed 32-byte hex
    pub output_commitment: String,    // 0x-prefixed 32-byte hex (0x0 if has_change == 0)
    pub recipient: String,            // 0x-prefixed EVM address (20 bytes)
    pub merchant_amount: u64,         // USDC base units (6 decimals)
    pub protocol_fee: u64,            // Protocol fee in USDC base units
    pub execution_fee: u64,           // Agreed relayer execution fee
    pub max_execution_fee: u64,       // Max execution fee signed in quote
    pub quote_hash: String,           // EIP-712 quote digest
    pub quote_signature: String,      // Relayer's signed quote
    pub user_address: String,         // Spender address who requested the quote
    pub expiry: u64,                  // Timestamp cutoff
    pub has_change: u64,              // 0 or 1
    pub proof_a_neg: String,          // 0x-prefixed 128-byte hex (G1 affine)
    pub proof_b: String,              // 0x-prefixed 256-byte hex (G2 affine)
    pub proof_c: String,              // 0x-prefixed 128-byte hex (G1 affine)
    pub public_inputs: Vec<String>,   // 12 public input scalars in hex
    pub idempotency_key: Option<String>,
}
```

#### Pre-Flight Invariant Verification (Fail-Closed):
1. **Idempotency:** If `idempotency_key` is present and previously resolved, return cached response immediately.
2. **Canonical Scalar Check (Z-SCAPE & zkBSA):**
   - *References:* [`jurnal/honorable/Z-SCAPE-Asset-Protection-Entropy-Failure.md`](file:///workspaces/Zeltra-Protocol/jurnal/honorable/Z-SCAPE-Asset-Protection-Entropy-Failure.md) ([`PDF`](file:///workspaces/Zeltra-Protocol/jurnal/pdf/2026-1621.pdf)) & [`jurnal/honorable/zkBSA-Auditable-Compliant-Stealth-Addresses-Blockchains.md`](file:///workspaces/Zeltra-Protocol/jurnal/honorable/zkBSA-Auditable-Compliant-Stealth-Addresses-Blockchains.md) ([`PDF`](file:///workspaces/Zeltra-Protocol/jurnal/pdf/2026-513.pdf)).
   - All 12 public inputs must be $< r$ where $r$ is the BLS12-381 scalar field modulus (`0x73eda753299d7d483339d80809a1d80553bda402fffe5bfeffffffff00000001`). Non-canonical inputs are rejected immediately with `INVALID_PUBLIC_INPUT_SCALAR`.
3. **Semantic Public Input Binding (DEC-022):**
   - *Reference:* [`research/decisions/DEC-022-defense-against-proof-settlement-mismatch-boundary-gaps.md`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-022-defense-against-proof-settlement-mismatch-boundary-gaps.md).
   - `public_inputs[0] == note_root`
   - `public_inputs[1] == input_nullifier`
   - `public_inputs[2] == output_commitment`
   - `public_inputs[3] == recipient`
   - `public_inputs[4] == merchant_amount`
   - `public_inputs[5] == protocol_fee`
   - `public_inputs[6] == execution_fee`
   - `public_inputs[7] == quote_hash`
   - `public_inputs[8] == expiry`
   - `public_inputs[9] == has_change`
4. **Execution Fee Enforcement:** `execution_fee <= max_execution_fee`. If `execution_fee > max_execution_fee`, revert with `FEE_EXCEEDS_MAX`.
5. **Double-Spend Guard:**
   - Query SQLite `is_nullifier_spent(input_nullifier)`. If true $\rightarrow$ reject with `NULLIFIER_ALREADY_SPENT`.
   - Query on-chain Stylus contract `is_nullifier_spent(input_nullifier)`. If true $\rightarrow$ reject with `ON_CHAIN_NULLIFIER_ALREADY_SPENT`.

---

### B. Persistent Queue & Two-Phase Crash-Safe Leasing

To prevent double-spending under concurrent loads or process restarts:
1. When a spend passes validation, it is inserted into SQLite table `spend_queue` with:
   - `spend_type = 'private_note'`
   - `status = 'queued'`
   - `nullifier = input_nullifier`
   - `available_at = unix_timestamp()`
2. Table `nullifiers` is atomically populated with `(nullifier, spend_id, 'reserved')` inside the same database transaction. Any concurrent request attempting to spend the same nullifier fails SQLite unique constraint with `SQLITE_CONSTRAINT_UNIQUE`.
3. The background worker claims spends using immediate transactions:
   $$\text{UPDATE spend\_queue SET status='broadcasting', lease\_until=now+120s WHERE id IN (claimable\_ids)}$$

---

### C. Settlement Dispatcher via `EvmClient`

The relayer constructs the ABI-encoded transaction for the Stylus contract entrypoint:

```rust
// nimbus-contracts/src/lib.rs: spend_private_note(...)
pub fn spend_private_note(
    &mut self,
    note_root: FixedBytes<32>,
    input_nullifier: FixedBytes<32>,
    output_commitment: FixedBytes<32>,
    recipient: Address,
    merchant_amount: U256,
    protocol_fee: U256,
    execution_fee: U256,
    max_execution_fee: U256,
    quote_hash: FixedBytes<32>,
    expiry: U256,
    has_change: U256,
    proof_a_neg: Bytes,
    proof_b: Bytes,
    proof_c: Bytes,
) -> Result<bool, Vec<u8>>;
```

#### Dispatch Rules:
1. Gas estimate check + minimum balance check ($> 0.005$ ETH for relayer account).
2. Broadcast using thread-safe nonce manager with mempool watchdog (DEC-017).
3. If transaction is unmined after 45 seconds, trigger automatic +15% gas bump transaction replacement with the exact same nonce and payload.

---

### D. Post-Restart Reconciliation Worker (Crash Safety)

Upon node startup (`main.rs`), before accepting incoming HTTP requests:
1. Query SQLite for all spends where `status IN ('broadcasting', 'submitted')`.
2. For each in-flight spend:
   - Call Stylus contract `is_nullifier_spent(spend.nullifier)`.
   - **Case A (Nullifier is TRUE on-chain):** The transaction succeeded before the crash. Update DB status $\rightarrow$ `confirmed`, record confirmed block and receipt, and increment accrued execution fee.
   - **Case B (Nullifier is FALSE on-chain):**
     - If the transaction is still present in mempool $\rightarrow$ re-attach watchdog.
     - If the transaction is dropped/missing $\rightarrow$ reset status $\rightarrow$ `retryable` with `available_at = now`.

---

## 3. Implementation Plan across Nimbus Workspace

1. **`nimbus-node/src/dto.rs`**: Define `PrivateNoteSpendRequest` and `PrivateNoteSpendResponse`.
2. **`nimbus-node/src/evm_client.rs`**: Add `spend_private_note(...)` method constructing Alloy transaction call.
3. **`nimbus-node/src/database.rs`**:
   - Add `spend_type` and `proof_payload_json` columns to `spend_queue`.
   - Add nullifier lookup for `note_nullifiers`.
   - Update `reconcile_spend_as_confirmed` to support private note spends.
4. **`nimbus-node/src/handlers/spend.rs`**: Implement `handle_private_note_spend` handler and update background batch worker to dispatch private note spends.
5. **`nimbus-node/src/main.rs`**: Register route `/api/v1/spend-private-note` and include private note spends in post-restart reconciliation loop.

---

## 4. Verification & Testing Matrix

| Test Case | Scenario | Expected Outcome |
|---|---|---|
| **POS-01** | Valid Groth16 proof with change note (`has_change = 1`) | Enqueued $\rightarrow$ broadcast $\rightarrow$ mined on Stylus $\rightarrow$ confirmed in DB. |
| **POS-02** | Valid Groth16 proof without change (`has_change = 0`) | Enqueued $\rightarrow$ broadcast $\rightarrow$ mined with zero change commitment. |
| **NEG-01** | Non-canonical scalar in public inputs ($x \ge r$) | Rejection at ingress: `INVALID_PUBLIC_INPUT_SCALAR`. |
| **NEG-02** | Parameter mismatch between body and public inputs | Rejection at ingress: `PROOF_PUBLIC_INPUT_MISMATCH`. |
| **NEG-03** | Replayed nullifier (already spent in DB) | Rejection at ingress: `NULLIFIER_ALREADY_SPENT`. |
| **NEG-04** | Replayed nullifier (already spent on-chain) | Rejection at ingress: `ON_CHAIN_NULLIFIER_ALREADY_SPENT`. |
| **NEG-05** | Execution fee exceeds signed quote max | Rejection at ingress: `EXECUTION_FEE_EXCEEDS_MAX`. |
| **REC-01** | Relayer killed during broadcast, nullifier spent on-chain | Post-restart reconciliation marks spend `confirmed` without re-broadcast. |
| **REC-02** | Relayer killed during broadcast, tx dropped from mempool | Post-restart reconciliation resets spend to `retryable`. |
