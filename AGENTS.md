# AGENTS.md

## What This Is

Nimbus = private payment system on Arbitrum Stylus. User deposits USDC → gets a private credential via BLS threshold signing → spends it once (BLS verification on-chain) → refund if issuance fails.

Everything here protects one thing: **one deposit pays out exactly once, no more, no less.**

## Repo Layout

```
nimbus-contracts/  — Stylus WASM smart contract (deposit, spend, CCIP, ZK verification)
nimbus-core/       — Crypto primitives (BLS, threshold signing, hash-to-curve, ZK circuit, fees)
nimbus-node/       — Relayer node (leader, guardian, spend queue, settlement, API)
nimbus-sdk/        — Client SDK
nimbus-cli/        — CLI tool
scripts/           — Test scripts
research/decisions/ — Design decisions (DEC-001 s/d DEC-018)
docs/              — Technical docs
todo.md            — Master checklist (READ THIS FIRST)
```

Stack: Rust, Stylus SDK 0.10.7, Alloy, Arkworks 0.6.0 (BLS12-381 & Groth16), Axum, SQLCipher AES-256.

## Core Business Flow

```
User deposits USDC → contract records liability & emits DepositFee event (session_id, com_k_hash, client)
  ↓
Relayer deposit indexer verifies on-chain event with safe-block reorg protection
  ↓
Leader + guardians do threshold BLS blind signing → masked signature + commitment
  ↓
Deposit confirmed on-chain → leader verifies k * pk_iss == com_k → releases masking key k
  ↓
User unmasks → gets final credential (BLS signature)
  ↓
User spends → contract verifies BLS pairing + nullifier + Groth16 change note proof
  ↓
Contract pays recipient + mints Change Note to LeanIMT Merkle tree → reduces net liability
```

Refund path: if quorum fails or k never released → 24h timelock → user claims refund → liability reduced.

**Terminal states:** SPENT or REFUNDED. Never both.

## Invariants — Break These and It's a Critical Bug

**Accounting & Value Conservation:**
- `contract USDC balance >= total_deposited_principal` — checked after every state change, reverts on insolvency
- `gross_deposit = deposit_fee + net_liability`
- `Input Note = Payout + Protocol Fee + Execution Fee + Change Note` (ZK-UTXO value conservation)
- One session adds liability exactly once (duplicate session_id rejected)
- One credential reduces liability exactly once (nullifier prevents double spend)

**Deposit/Reveal/Refund:**
- `reveal()` does NOT transfer collateral — it only marks session resolved after verifying `k * pk_iss == com_k`
- Commitment is bound at deposit time — reveal compares against stored commitment, not caller-supplied
- Relayer enforces fail-closed cryptographic verification `k * pk_iss == com_k` before releasing `k` (DEC-018)
- Refund only after 24h timelock, only by depositor, only if session not resolved
- `SPENT XOR REFUNDED` — a session cannot be both

**Spend:**
- BLS pairing check must pass: `e(-alpha, G2) * e(H(m), pk_iss) == 1`
- Spend message is bound: `keccak256("SPEND" || chain_id || contract || amount || recipient || expiry || nonce)`
- Issuer key must be registered by admin (trusted_issuer_keys mapping)
- Fee rounding: ceiling division, always favors solvency

**CCIP:**
- Router must be non-zero, source chain + sender allowlisted, message ID replay-protected
- Source confirmation ≠ destination confirmation

## Behavior Rules

### Before Coding

1. **Read `todo.md`** — check item status before touching it
2. **Check `research/decisions/`** — there might be a relevant decision
3. **Reach for CodeGraph FIRST (Mandatory)** — Repository ini diindeks dengan CodeGraph (`.codegraph/`). SELALU gunakan MCP tool `codegraph_explore` (atau shell `codegraph explore "<query>"`) SEBELUM menggunakan grep, find, atau membaca file saat menelusuri simbol, call path, blast radius, atau arsitektur kode.
4. **Research gate for critical changes** (financial logic, crypto, CCIP, custody, storage layout):
   - Search Exa MCP for recent papers, audit reports, exploit post-mortems (2024-2026)
   - Minimum 2 independent sources, 1 must be primary
   - Write decision note in `research/decisions/`

### While Coding

- `cargo fmt` + `clippy` clean
- Never bypass crypto verification with `#[cfg(test)]` in production paths
- Never fall back to mock/test keys when secrets are unavailable — fail closed
- Never hardcode addresses, keys, or credentials
- Don't change contract storage layout without considering upgrade path
- Don't add features outside task scope

### While Testing

- Positive test + at least 2x more negative tests
- Negative tests must prove state is unchanged on failure
- Critical features need testnet verification (Arbitrum Sepolia) — unit tests aren't enough
- Wait for receipt and check status — broadcast alone doesn't count
- Tests fail if using: mock tx hash, dummy signature, random bytes as proof

### Secrets

- Never log: private keys, Vault tokens, guardian shares, DB encryption keys, sensitive payloads

## Fee Structure

| Fee | Rate | Where |
|---|---|---|
| Deposit | 0 bps (0.00% - Zero-friction inflow) | `deposit.rs` + `fees.rs` |
| Spend default (< 30 days) | 45 bps (0.45%) | `spend.rs` + `fees.rs` |
| Spend hold ≥ 30 days | 40 bps (0.40% - 5 bps discount) | `spend.rs` + `fees.rs` |
| Relayer Execution | Gas reimbursement + 15% markup | `quote.rs` + `fees.rs` |

Holding time is calculated from `clean_association_roots` root registration timestamp. Quote endpoint (`/api/quote/private-spend?association_root=...`) returns the applicable tier and discount to the user before they commit. Root timestamps are cached in node memory after first RPC query.

## Key Files to Read

- `todo.md` — master checklist, hard-test matrix, campaign status
- `docs/bisnis.md` — business blueprint and fee policy
- `docs/mainnet_readiness_todo.md` — mainnet readiness roadmap
- `research/decisions/` — 15 design decisions with rationale
- `SESSION_SUMMARY.md` — what was done in previous sessions (read this when starting fresh)

## Environment & Deployment

**`nimbus-node/.env.test`** is the single source of truth for all runtime config. It contains:
- RPC URLs (Arbitrum Sepolia)
- Relayer private key
- BLS share key + threshold params
- Issuer public key + guardian public keys
- Contract address (deployed Stylus contract)
- DB path + encryption key
- Vault/KMS config

To run the node or deploy with `cargo stylus`, source it first:
```bash
source nimbus-node/.env.test
```


## Guardian Cluster

```bash
bash scripts/start_cluster.sh start    # 1 leader + 4 guardians + Vault
bash scripts/start_cluster.sh status   # check health
bash scripts/start_cluster.sh stop     # teardown
```

Threshold 3/5. Auto-loads `.env.test` for RPC/contract/key. Health check timeout ≥15s.
