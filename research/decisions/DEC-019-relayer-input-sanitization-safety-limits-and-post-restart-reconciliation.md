# DEC-019: Relayer Input Sanitization, Financial Safety Limits, and Post-Restart Nullifier Reconciliation

## Status
Accepted

## Date
2026-10-03

## Context
In shielded payment systems (such as Tornado Cash, Railgun, and Zeltra/Nimbus), the relayer serves as the bridge between off-chain clients generating zero-knowledge/BLS proofs and the on-chain Stylus contract. Relayers face specific security vulnerabilities:
1. **Invalid/Malformed Inputs & Stuck Funds:** If a user submits an unvalidated recipient (e.g., zero address `0x000...000`, truncated hex, or malformed bytes), the relayer pays execution gas only for the transaction to revert on-chain, or worse, permanently burn collateral to the zero address.
2. **Dust Spam & Mempool Denial of Service (DoS):** Attackers can flood the relayer queue with micro-spends (e.g. 1 wei or $0.000001 USDC), exhausting database connections, mempool bandwidth, and queue worker resources.
3. **Uncapped Single-Transaction Exposure:** In the absence of transaction ceiling limits, an abnormal or compromised transaction could drain a significant portion of pool liquidity in a single shot.
4. **CCIP Routing Misdirection:** Unchecked destination chain selectors could route cross-chain intents to unmonitored or adversarial chains, resulting in lost funds.
5. **Crash-Recovery / Restart Revert Loops:** When a relayer crashes or restarts while a spend transaction is in-flight (or mined during relayer downtime), the relayer database might still record the spend as `queued` or `broadcasting`. Upon reboot, if the relayer blindly re-broadcasts this transaction without checking on-chain nullifier status, the transaction will revert (`DOUBLE_SPEND`), wasting relayer gas and potentially stalling queue processing.

## Precedents and Research (Audits & Post-Mortems 2024-2026)
- **OpenZeppelin BridgeV2 Audit (Nov 2025):** Identified insufficient input validation where zero-address destinations were accepted by relayers, resulting in irreversible burning of bridged tokens.
- **ChainSecurity Circle EVM Bridge Review (2025):** Recommended explicit boundary assertions (min/max bounds) on relayer ingress before proof verification or queue insertion.
- **Tornado Cash & Railgun Crash Recovery Pattern:** Relayers verify the on-chain Merkle root / nullifier status before submitting transactions to avoid submitting known reverts.

## Decision

### 1. Smart Contract View Function (`is_nullifier_spent`)
Add a public view function to [`nimbus-contracts/src/lib.rs`](file:///workspaces/Zeltra-Protocol/nimbus-contracts/src/lib.rs):
```rust
/// Returns whether a nullifier has already been spent on-chain.
pub fn is_nullifier_spent(&self, nullifier: FixedBytes<32>) -> Result<bool, Vec<u8>> {
    Ok(self.nullifiers.get(nullifier))
}
```
This enables EVM clients and relayer indexers to verify nullifier status directly over RPC without relying solely on local cache or re-simulating the entire BLS verification.

### 2. Strict Recipient EVM Address Validation
In [`nimbus-node/src/validation.rs`](file:///workspaces/Zeltra-Protocol/nimbus-node/src/validation.rs) (applied at ingress in `handle_spend`):
- Recipient must be a valid 20-byte EVM address (hex format with `0x` prefix).
- Recipient must NOT be `Address::ZERO` (`0x0000000000000000000000000000000000000000`).
- If cross-chain params are specified, `destination_contract` must also parse to a valid non-zero EVM address.

### 3. Financial Safety Limits (Min / Max USDC Bounds)
- **Minimum Spend Amount:** Minimum threshold of 5 USDC (`5_000_000` units, matching on-chain `AMOUNT_TOO_SMALL` invariant). Any spend below this amount is rejected immediately at ingress with HTTP 400.
- **Maximum Spend Amount (Circuit Breaker):** Default ceiling of 50,000 USDC (`50_000_000_000` units), configurable via `NIMBUS_MAX_SPEND_USDC`. Any spend exceeding this limit is rejected immediately at ingress.
- Zero amount (`amount == 0`) is explicitly rejected.

### 4. CCIP Destination Chain Selector Allowlist
For cross-chain payments (`CrossChainParams`), `destination_chain_selector` must belong to an allowlisted set of active CCIP testnet/mainnet selectors:
- Arbitrum Sepolia: `3478487238524512106`
- Ethereum Sepolia: `16015286601757825753`
- Base Sepolia: `10344971235874465080`
- Optimism Sepolia: `5224473277236331295`
Configurable via environment variable `NIMBUS_ALLOWED_CHAIN_SELECTORS` (comma-separated list).

### 5. Post-Restart Nullifier Reconciliation
During relayer startup in [`main.rs`](file:///workspaces/Zeltra-Protocol/nimbus-node/src/main.rs):
1. Query local database for any spends in `queued`, `leased`, or `broadcasting` state.
2. For each spend, query the on-chain contract via `evm_client.is_nullifier_spent(nullifier)`.
3. If on-chain returns `true`:
   - Mark the spend as `confirmed` in `spend_queue`.
   - Record the nullifier in the local `nullifiers` table.
   - Log an informative audit message: `RECONCILIATION: Nullifier 0x... was already spent on-chain. Marked local spend as confirmed.`
4. If on-chain returns `false`:
   - If the spend lease is expired, reset status to `queued` with clear lease lock so queue workers can process it normally.
5. Startup continues only after reconciliation finishes, guaranteeing no duplicate transaction broadcast or revert loop.

## Invariants Maintained
- **Solvency & Safety:** No unbacked or zero-address spends are queued.
- **DoS Resistance:** Dust amounts (< 5 USDC) rejected before database writes or queue lock contention.
- **Fail-Closed & Replay Protection:** In-flight transactions mined during restart are recognized immediately without re-spending gas.
