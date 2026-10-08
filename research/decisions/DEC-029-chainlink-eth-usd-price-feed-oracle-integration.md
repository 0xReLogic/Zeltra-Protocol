# DEC-029: Chainlink Data Feeds (ETH/USD) Price Oracle Integration for Relayer Margin & Solvency Accounting

## Status
Accepted

## Context
During settlement and batching operations, the Nimbus/Zeltra relayer pays native gas in ETH upfront to Arbitrum and reimburses gas in USDC from user execution fees (`max_execution_fee` / `execution_fee`). 

Previously, `nimbus-node/src/handlers/spend.rs` calculated execution gas costs and batch margins using:
```rust
let eth_price = std::env::var("NIMBUS_ETH_PRICE_USDC")
    .ok()
    .and_then(|v| v.parse::<f64>().ok())
    .unwrap_or(3500.0);
```
As reported in GitHub Issue #2, this hardcoded approach had several deficiencies:
1. `NIMBUS_ETH_PRICE_USDC` was absent from `docker-compose.yml`, `.env.test.example`, and centralized configuration structs (`config.rs`).
2. ETH price exhibits significant market volatility (±30–50% over a 3-month period). Using a static fallback (e.g. $3,500) when actual ETH is $2,200 causes relayer to overstate gas cost and reject profitable batches; when actual ETH is $4,500, relayer experiences silent operational losses.
3. No audit trail or real-time oracle reconciliation for relayer accounting and health reporting.

## Decision

We adopt a two-tier hybrid architecture combining **live Chainlink AggregatorV3 price feeds** with **cached TTL fallbacks**:

### 1. Chainlink `AggregatorV3Interface` On-Chain Integration
We query the official Chainlink ETH/USD aggregator proxy on-chain via read-only `eth_call` (zero gas cost):
- **Arbitrum Sepolia (Testnet):** `0xd30e2101a97dcbAeBCBC04F14C3f624E67A35165` (8 decimals, verified: returns active round data).
- **Arbitrum One (Mainnet):** `0x639Fe6ab55C921f74e7fac1ee960C0B6293ba612` (8 decimals).

We add `latestRoundData()` and `decimals()` ABI bindings directly in `nimbus-node/src/evm_client.rs` using Alloy's `sol!` macro:
```solidity
function latestRoundData() external view returns (
    uint80 roundId,
    int256 answer,
    uint256 startedAt,
    uint256 updatedAt,
    uint80 answeredInRound
);
function decimals() external view returns (uint8);
```

### 2. Staleness, Sanity & Fallback Protections (Hard-Audit Tested 2024–2026 Patterns)
To ensure robust relayer execution under extreme network or oracle conditions and eliminate known vulnerabilities (e.g., Code4rena/Sherlock findings regarding stale rounds and unconstrained answers):
- **Positive & Non-Zero:** Answers $\le 0$ are rejected.
- **Round Completeness Check:** Verify `answeredInRound >= roundId` to guarantee the round was not incomplete or aborted mid-computation.
- **Future Timestamp Protection:** Reject `updatedAt > block_timestamp + 300s` to prevent manipulated clock drift anomalies.
- **Staleness Check:** Answers older than `staleness_threshold_secs` (default 3,600s / 1 hour) trigger a warning or fallback.
- **Hard Min/Max Circuit Breaker Bounds:** Enforce $\$100.00 \le \text{ETH} \le \$100,000.00$ to prevent decimal corruption, flash loan price distortions, or extreme misreporting.
- **In-Memory Cache (TTL):** Successful oracle reads are cached for 300s (5 minutes) to avoid hammering the RPC node with redundant calls on high transaction volume.
- **Configurable Fallback:** If the oracle call fails, times out, or returns invalid data, the relayer falls back to `NIMBUS_ETH_PRICE_USDC` env var, and finally to `3500.0`.

### 3. Centralized Configuration (`PricingConfig`)
Add `PricingConfig` to `nimbus-node/src/config.rs`:
- `eth_price_feed_address`: Optional custom aggregator address override via `NIMBUS_CHAINLINK_ETH_FEED`.
- `fallback_eth_price`: Fallback price in USDC via `NIMBUS_ETH_PRICE_USDC`.
- `cache_ttl_secs`: TTL for in-memory price caching (default 300s).
- `staleness_threshold_secs`: Max age of oracle round before declaring stale (default 3600s).

### 4. Background Sync & Observable State
An asynchronous worker periodically refreshes the cached price in `AppState.eth_price_cache`. The active price and oracle status are exposed in the `/health` endpoint to provide full solvency and accounting observability.

## Consequences & Guarantees
- Zero gas spent: Uses read-only `eth_call`.
- Fail-safe: Network partitions or RPC hiccups gracefully degrade to fallback env config without halting relayer settlements.
- Solvency protection: Relayer margin tracking (`margin_usdc`) and batch profitability calculations in `spend_batches` accurately reflect actual market conditions.

---

## References & Audit Citations
1. **Chainlink Data Feeds Documentation:**
   - *Using Data Feeds on EVM Chains:* [https://docs.chain.link/data-feeds/using-data-feeds](https://docs.chain.link/data-feeds/using-data-feeds)
   - *L2 Sequencer Uptime Feeds:* [https://docs.chain.link/data-feeds/l2-sequencer-feeds](https://docs.chain.link/data-feeds/l2-sequencer-feeds)
   - *Arbitrum Price Feed Registry:* Arbitrum Sepolia (`0xd30e2101a97dcbAeBCBC04F14C3f624E67A35165`), Arbitrum One (`0x639Fe6ab55C921f74e7fac1ee960C0B6293ba612`).
2. **DeFi Oracle Security Findings & Exploit Analyses (2024–2026):**
   - *Code4rena M-04 / Issue #69 (Predy Finance, 2024):* Missing round completeness (`answeredInRound >= roundId`) and staleness threshold validation in Chainlink `latestRoundData`.
   - *Code4rena Issue #273 (LoopFi, 2024):* Stale price check absence in oracle adapters risking protocol insolvency during L2 sequencer heartbeat delays.
   - *Sherlock Issue #259 (Sentiment v2, 2024):* Lack of minAnswer/maxAnswer circuit-breaker bounds allowing extreme feeder anomalies or decimal corruption to distort collateral valuation.
   - *Venus Protocol / LUNA Post-Mortem Precedents:* Oracle unbounded returns during extreme market volatility leading to bad debt.
