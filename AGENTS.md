# AGENTS.md

## What This Is

**Zeltra Protocol** (codebase transition name: Nimbus) = **"The Stripe of Web3 with Absolute Privacy."** A high-throughput, shielded payment and settlement rail on Arbitrum Stylus for AI Agents, retail, whales, B2B corporates, and global merchants.

Full vision manifesto: [`VISION.md`](file:///workspaces/Zeltra-Protocol/VISION.md).

Product Mantra: **Privacy 9.5/10, Product 10/10.** Apple Pay / QRIS speed with real-cash mental accounting ($10 - $3 = $7 change via ZK-UTXO Private Notes). Engineering hell stays in the background; user experience stays effortless.

Zero-friction inflow (0% deposit fee), volume-over-TVL economic velocity, and absolute mathematical solvency: **one deposit pays out exactly once, no more, no less.**

## Repo Layout

```
nimbus-contracts/  — Stylus WASM smart contract (deposit, spend, batch, ZK private note Groth16, LeanIMT tree)
nimbus-core/       — Crypto primitives (BLS12-381, Groth16 PrivateNoteCircuit, Poseidon Grain-128, fees, accounting)
nimbus-node/       — Relayer node (leader, guardian, spend queue, ZK note ingress & direct settlement, API)
nimbus-sdk/        — Client SDK (PrivateNoteWallet, coin selection, local Groth16 prover, encrypted store)
nimbus-cli/        — CLI tool
scripts/           — Test scripts & cluster automation
research/decisions/ — Design decisions (DEC-001 s/d DEC-026)
docs/              — Modular chapter-based docs
todo.md            — Master active checklist (pending tasks only)
```

Stack: Rust (Stylus SDK 0.10.7, Alloy 2.0, Arkworks 0.6.0 `ark-bls12-381` & `ark-groth16`), Axum 0.8, SQLite with SQLCipher AES-256 (`bundled-sqlcipher`), OpenBao/Vault KMS.

## Core Business Flow

### 1. Inflow (Deposit USDC → Private Credential / Note)
```
User deposits USDC → contract records liability & emits DepositFee event (session_id, com_k_hash, client)
  ↓
Relayer deposit indexer verifies on-chain event with safe-block reorg protection (DEC-018)
  ↓
Leader + guardians do threshold BLS blind signing → masked signature + commitment
  ↓
Deposit confirmed on-chain → leader verifies k * pk_iss == com_k → releases masking key k
  ↓
User unmasks → gets final credential / initial Private Note
```

### 2. Outflow (ZK-UTXO Spend & Change Note)
```
User selects Note UTXO via PrivateNoteWallet (SDK, DEC-024)
  ↓
User generates local Groth16 proof (PrivateNoteCircuit: value conservation, Poseidon nullifier, Merkle path)
  ↓
User submits proof & 12 public inputs to Relayer (/api/v1/spend-private-note, DEC-025)
  ↓
Relayer validates scalars (< r), binds semantics (DEC-022), checks nullifiers locally & on-chain
  ↓
Relayer broadcasts spend_private_note(...) to Stylus contract
  ↓
Contract verifies Groth16 via EIP-2537 precompiles (0x0c MSM + 0x0f Pairing)
  ↓
Contract marks note_nullifier spent, inserts change note to LeanIMT tree (depth 20), pays recipient USDC
  ↓
Multi-liability reduced, exact value conservation maintained
```

Refund path: if quorum fails or k never released → 24h timelock → user claims refund → liability reduced.

**Terminal states:** SPENT or REFUNDED. Never both.

## Invariants — Break These and It's a Critical Bug

**Accounting & Value Conservation:**
- `contract USDC balance >= total_liabilities (user_note_liability + refundable_deposit_liability + accrued_execution_fee_liability)` — checked after every state change, reverts on insolvency
- `gross_deposit = deposit_fee + net_liability` (deposit fee = 0 bps immutable)
- `Input Note = Payout + Protocol Fee + Execution Fee + Change Note` (ZK-UTXO value conservation)
- One session adds liability exactly once (duplicate session_id rejected)
- One credential/note reduces liability exactly once (nullifier prevents double spend)

**Deposit/Reveal/Refund:**
- `reveal()` does NOT transfer collateral — it only marks session resolved after verifying `k * pk_iss == com_k`
- Commitment is bound at deposit time — reveal compares against stored commitment, not caller-supplied
- Relayer enforces fail-closed cryptographic verification `k * pk_iss == com_k` before releasing `k` (DEC-018)
- Refund only after 24h timelock, only by depositor, only if session not resolved
- `SPENT XOR REFUNDED` — a session cannot be both

**Spend (Legacy BLS & ZK-UTXO Private Note):**
- Legacy BLS pairing check: `e(-alpha, G2) * e(H(m), pk_iss) == 1`
- ZK-UTXO Private Note Groth16 check via EIP-2537 (`0x0c` MSM + `0x0f` 4-pairing):
  `e(-A, B) * e(alpha, beta) * e(L, gamma) * e(C, delta) == 1`
- Semantic public input binding (DEC-022): exact match of recipient, amount, fees, quote hash, chain_id, contract_address, expiry, has_change
- Double-spend protection: siloed nullifier (`note_nullifiers[input_nullifier] == true`)
- Fee rounding: ceiling division, always favors solvency

**CCIP:**
- Router must be non-zero, source chain + sender allowlisted, message ID replay-protected
- Source confirmation ≠ destination confirmation

## Behavior Rules

### Before Coding

1. **Read `todo.md`** — check item status before touching it
2. **Check `research/decisions/`** — there might be a relevant decision
3. **Navigasi Kode & Arsitektur (CodeGraph & Graphify):**
   - **Arsitektur Makro & Hubungan Antar-Modul (Graphify):** Gunakan `graphify query "<pertanyaan>"`, `graphify path "<A>" "<B>"`, atau `graphify explain "<Simbol>"`. JANGAN membaca langsung file `graphify-out/graph.json` atau seluruh `GRAPH_REPORT.md` ke context window (sangat boros token).
   - **Call Path & Verbatim Code (CodeGraph):** Gunakan MCP tool `codegraph_explore` (atau shell `codegraph explore "<query>"`) untuk melihat implementasi aktual, signature baris, dan dynamic hops antar fungsi.
   - **Maintenance:** Setelah melakukan modifikasi file Rust, jalankan `graphify update .` untuk menjaga graf tetap sinkron tanpa memicu LLM cost.
4. **Prinsip ATMI (Amati, Tiru, Modifikasi, Inovasi)** — Belajar dan serap pola unggul dari paper kriptografi teruji dan repo battle-tested (seperti Zcash, Aztec Barretenberg, Railgun, zk-kit, zk-sunade, Wasabi). Amati polanya, tiru fondasinya, modifikasi agar cocok dengan Stylus/BLS12-381/ZK-UTXO Zeltra, dan inovasikan keunggulan baru (privacy 9.5/10, produk 10/10).
5. **Research gate for critical changes** (financial logic, crypto, CCIP, custody, storage layout):
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
| Spend Protocol Outflow | 45 bps (0.45% Flat - DEC-028) | `spend.rs` + `fees.rs` |
| Relayer Execution | Gas reimbursement + 15% markup | `quote.rs` + `fees.rs` |

Under DEC-028, holding-time fee discounts are abolished to eliminate root age gaming and maintain 100% predictable cash accounting ($10 - $3 = $7). Protocol spend fee is unified to a flat 45 bps across all spends.

## Key Files & Docs

- [`todo.md`](file:///workspaces/Zeltra-Protocol/todo.md) — Master active backlog & test status
- [`VISION.md`](file:///workspaces/Zeltra-Protocol/VISION.md) & [`docs/bisnis.md`](file:///workspaces/Zeltra-Protocol/docs/bisnis.md) — Product vision, business model, & fee architecture
- [`docs/`](file:///workspaces/Zeltra-Protocol/docs/) — Chapter-based modular docs (`core/`, `contract/`, `relayer/`, `sdk/`, `cli/`)
- [`research/decisions/`](file:///workspaces/Zeltra-Protocol/research/decisions/) — Architecture ADRs (DEC-001 through DEC-029)

## Environment, Deployment & Cluster

- **Runtime Config:** `nimbus-node/.env.test` is the single source of truth (RPC URLs, keys, contract addresses, DB cipher keys, Vault/KMS settings). Always run `source nimbus-node/.env.test` before executing nodes or deployment tools.
- **Stylus Contract Deployment:** `cargo stylus check` / `cargo stylus deploy` from `nimbus-contracts/`.
- **Guardian Cluster Automation:**
  ```bash
  bash scripts/start_cluster.sh {start|status|stop}  # 1 leader + 4 guardians + Vault (3-of-5 threshold)
  ```

Threshold 3/5. Auto-loads `.env.test` for RPC/contract/key. Health check timeout ≥15s.
