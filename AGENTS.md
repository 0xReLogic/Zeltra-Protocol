# AGENTS.md

## What This Is

**Zeltra Protocol** (codebase transition name: Zeltra) = **"The Stripe of Web3 with Absolute Privacy."** A high-throughput, shielded payment and settlement rail on Arbitrum Stylus for AI Agents, retail, whales, B2B corporates, and global merchants.

Full vision manifesto: [`VISION.md`](file:///workspaces/Zeltra-Protocol/VISION.md).

Product Mantra: **Privacy 9.5/10, Product 10/10.** Apple Pay / QRIS speed with real-cash mental accounting ($10 - $3 = $7 change via ZK-UTXO Private Notes, or direct peer-to-peer in-pool transfer). Engineering hell stays in the background; user experience stays effortless.

Dual sovereign rails (USDC & WETH / Native ETH), zero-friction inflow (0% deposit fee), volume-over-TVL economic velocity (0% in-pool transfer fee), and absolute mathematical solvency: **one deposit pays out exactly once, no more, no less.**

## Repo Layout & Code Mapping

```
zeltra-contracts/  — Stylus WASM Smart Contract (Arbitrum Nitro / EIP-2537)
  ├── src/lib.rs                   — Entrypoint contract Zeltra (deposit_direct_note, deposit_direct_weth, deposit_direct_native_eth, spend_joinsplit, etc.)
  ├── src/deposit.rs               — Direct MMR note deposit logic for ERC20 & native ETH payable auto-wrap (_mmr_insert)
  ├── src/spend.rs                 — Strict 3-Mode JoinSplit execution logic (Groth16 verify, nullifier burning, dual 96B memos append, fee transfers)
  ├── src/merkle.rs                — On-chain Merkle Mountain Range (MMR) accumulator (_mmr_insert, canonical peak bagging)
  ├── src/storage.rs               — Stylus sol_storage! Zeltra layout (MMR state, accepted roots, nullifiers, epoch slots, multi-asset storage)
  ├── src/groth16_note_verifier.rs — 1-in-1-out Groth16 BLS12-381 verifier over EIP-2537 (16 public inputs)
  ├── src/groth16_joinsplit_verifier.rs — Universal 2-in-2-out JoinSplit Groth16 verifier over EIP-2537 (19 public inputs)
  ├── src/poseidon_w5_constants.rs — Precomputed Poseidon width-5 round constants & MDS matrix for on-contract hashing
  ├── src/events.rs                — Contract events (NoteCommitmentAppended with encrypted memo, JoinSplitSpendExecuted, etc.)
  ├── src/verification.rs          — EIP-2537 host precompile wrappers (0x0c MSM + 0x0f 4-pairing)
  ├── src/vault.rs                 — Siloed multi-asset solvency accounting (_check_asset_solvency_invariant, total_assets)
  └── src/interfaces.rs, types.rs, constants.rs, helpers.rs — ERC20 & CCIP interfaces, EVM byte conversions

zeltra-core/       — Cryptography Primitives & ZK Circuits
  ├── src/lib.rs                   — Re-exports Arkworks BLS12-381, Poseidon, EVM, fees, MMR, and circuits
  ├── src/note_circuit.rs          — 1-in-1-out PrivateNoteCircuit (16 public inputs, Two-Limb quote hash, formal scope binding)
  ├── src/joinsplit_circuit.rs     — Universal 2-in-2-out JoinSplitCircuit (19 public inputs, constant-arity, 448 range gates)
  ├── src/selective_disclosure_circuit.rs — CEX compliance SelectiveDisclosureCircuit (Groth16 verifier binding, DEC-040A)
  ├── src/circuit_gadgets.rs       — Reusable R1CS gadgets (flags_packed packing, range checks, Poseidon gadgets)
  ├── src/note.rs                  — Note primitives (PrivateNoteV1 with asset_id, ZSA key derivation, deterministic ECDH, ChaCha20Poly1305)
  ├── src/poseidon.rs              — Poseidon width-3 & width-5, audited Grain-128 LFSR round constants generator (DEC-021)
  ├── src/accounting.rs            — Solvency state machine (ContractAccounting) with mathematical invariant checks
  ├── src/fees.rs                  — Fee calculations (0 bps deposit, 0 bps in-pool transfer, 45 bps public spend fee, relayer markup)
  ├── src/evm.rs                   — EVM byte & point formatting for BLS12-381 G1, G2, scalars, and proofs
  ├── src/formal_tests.rs          — Mathematical invariant verification tests
  └── src/bin/bench.rs             — Cryptographic micro-benchmarks

zeltra-node/       — Stateless Single-Sequencer Relayer Node (Axum 0.8)
  ├── src/main.rs                  — Server boot, Axum router, background worker orchestration, rate limiter
  ├── src/bin/inpool_interop.rs    — CLI test harness for cross-language verification & Python suite
  ├── src/handlers/
  │   ├── health.rs                — Probes (/health, /live, /ready) with macro solvency metrics
  │   ├── quote.rs                 — Signed execution fee quotes (/api/v1/quote, /api/quote/private-spend)
  │   ├── spend.rs                 — Strict 3-Mode spend ingress (/api/v1/spend-joinsplit, /api/v1/spend-private-note) with 192 hex char memo guards
  │   ├── mmr.rs                   — MMR sync (/api/v1/mmr/proof/:leaf_index, /api/v1/mmr/leaves, /api/v1/mmr/tip, /api/v1/notes/sync)
  │   ├── tx_status.rs             — Transaction status lookup (/api/tx-status)
  │   └── x402.rs                  — Pure ZK-UTXO HTTP 402 Facilitator endpoint (/api/x402/verify)
  ├── src/database.rs              — SQLite + SQLCipher AES-256 database, WAL mode, transaction idempotency
  ├── src/evm_client.rs            — Alloy EVM client for transaction broadcasting, receipt confirmation, gas bumping
  ├── src/mmr_indexer.rs           — Background worker indexing on-chain NoteCommitmentAppended events & memos
  ├── src/mempool_watchdog.rs      — Background worker reclaiming stalled leases & checking on-chain nullifiers
  ├── src/sanctions_sync.rs        — Background worker syncing OFAC sanctions list daily from GitHub
  ├── src/execution_fee_claimer.rs — Background worker claiming accrued execution fees from Stylus contract
  ├── src/ccip_monitor.rs          — Background worker tracking CCIP messages & handling refunds (/api/ccip/refund)
  ├── src/validation.rs            — Preflight validation (canonical scalars, nullifiers, binding, Groth16 check)
  └── src/dto.rs, config.rs, state.rs — DTOs, network config (.env.test), shared AppState

zeltra-sdk/        — Client SDK & Private Note Wallet
  ├── src/lib.rs                   — Re-exports, memory security (secure_zeroize, zeroize)
  ├── src/wallet/note_wallet.rs    — PrivateNoteWallet: prepare_shielded_transfer_proof, Knapsack selector, Groth16 prover, recover_notes_batch, 2PC store
  ├── src/compliance/              — B2B double-entry compliance audit & CEX verification (exporter.rs, verifier.rs, models.rs)
  ├── src/coin_selection.rs        — Tiered stochastic knapsack coin selection algorithm (multi-asset partitioned)
  ├── src/storage.rs               — 2PC crash-safe note store with Argon2id + ChaCha20Poly1305 encryption
  ├── src/client.rs                — HTTP client communicating with relayer endpoints
  ├── src/eip712.rs                — EIP-712 typed data signer for execution quotes
  ├── src/zk_wasm.rs, fee_wasm.rs  — In-browser WASM Groth16 prover with disposable Web Worker memory lifecycle (DEC-041)
  ├── src/wasm_types.rs            — Serializable DTOs for JS/TS bindings
  ├── src/x402/                    — Pure ZK-UTXO x402 payment protocol client codec and types (DEC-042)
  └── src/bin/bench_e2e.rs         — End-to-end deposit & spend benchmark binary

zeltra-cli/        — Pure ZK CLI Tool
  ├── src/main.rs, cli.rs          — Clap subcommands: GenerateWallet, CreateDepositNote, InspectMmr, VerifyProof, Compliance
  ├── src/commands/zk.rs           — Key generation, direct deposit note creation, MMR inspection, proof verification
  └── src/commands/compliance.rs   — CEX verification, compliance audit export, proof verification

scripts/           — Automation & Testing Scripts
  ├── scripts/start_relayer.sh     — Single sequencer relayer daemon runner (start|status|stop|restart)
  ├── scripts/deploy_testnet.sh    — Stylus contract deployer to Arbitrum Sepolia
  ├── scripts/test_real_case_dec043c_inpool_wallet.py — Independent 44/44 Python test harness for in-pool transfer
  └── scripts/check_balance.py, ht01_init_verification.py, ht04_spend_negative_tests.py, ht05_refund_tests.py, ht10_signed_quote_tests.py

research/decisions/ — 71 Architecture Decision Records (DEC-001 through DEC-047)
docs/              — Chapter-based modular docs (bisnis.md, analisis/, contract/, core/, relayer/, sdk/, cli/, todos/)
todo.md            — Master active checklist (pending tasks only)
```

Stack: Rust (Stylus SDK 0.10.7, Alloy 2.0, Arkworks 0.6.0 `ark-bls12-381` & `ark-groth16`), Axum 0.8, SQLite with SQLCipher AES-256 (`bundled-sqlcipher`), OpenBao/Vault KMS.

## Core Business Flows (Pure ZK-UTXO Architecture — DEC-038)

### 1. Inflow: Zero-Friction Direct MMR Deposit (USDC & WETH / Native ETH - DEC-038B / DEC-034B)
```
User SDK derives Note (value, owner_key, rho, r, asset_id) & computes Poseidon-W5 commitment (<15 us)
  ↓
User SDK encrypts 96-byte AEAD ChaCha20Poly1305 memo via Ephemeral X25519 ECDH (DEC-037)
  ↓
User calls deposit_direct_note (USDC) or deposit_direct_native_eth (auto-wrap ETH -> WETH) on Stylus contract
  ↓
Contract validates canonical scalar (< r), min amount, inserts leaf to MMR tree (_mmr_insert),
emits NoteCommitmentAppended event with memo, and updates user_note_liabilities[asset_id]. (0 bps deposit fee)
  ↓
Note is immediately unspent & active (Zero threshold ceremony, zero masking key release).
```

### 2. Shielded-to-Shielded In-Pool Transfer & Merchant Masking (Mode 2 JoinSplit - DEC-043)
```
Sender parses Recipient ZSA (BIP-350 Bech32m 'zs1...') -> extracts owner_key & pk_ivk
  ↓
Sender prepares JoinSplit Mode 2 proof: transfer_amount to Recipient, change back to Sender
  - Output 1: Recipient commitment & 96-byte memo (encrypted via deterministic ECDH X25519)
  - Output 2: Sender change commitment & 96-byte memo (or randomized dummy memo if exact payment)
  - Groth16 constraints: merchant_amount = 0, protocol_fee = 0, recipient = Address::ZERO (0 bps protocol fee)
  ↓
Sender submits proof & dual 96-byte memos to Relayer (/api/v1/spend-joinsplit)
  ↓
Relayer verifies canonical scalars (< r), Strict 3-Mode invariants, and local Groth16 preflight
  ↓
Relayer broadcasts transaction to Stylus contract -> Contract verifies Groth16 via EIP-2537
  ↓
Contract burns input nullifiers, appends Output 1 & Output 2 to MMR tree with encrypted memos.
Zero public token outflow from contract! Perfect merchant masking.
  ↓
Recipient runs recover_notes_batch trial decryption -> discovers Output 1, reconstructs r, credits balance.
```

### 3. Outflow: ZK-UTXO Public Spend & Change Note (Mode 3 JoinSplit or 1-in-1-out)
```
User selects Note UTXO(s) via PrivateNoteWallet Knapsack selector (SDK, DEC-024 / DEC-030 / DEC-036B)
  ↓
User generates local Groth16 proof: recipient != Address::ZERO, protocol_fee = 45 bps
  ↓
User submits proof & public inputs to Relayer (/api/v1/spend-private-note or /spend-joinsplit)
  ↓
Relayer verifies preflight & broadcasts spend transaction to Stylus contract
  ↓
Contract verifies Groth16 via EIP-2537 precompiles (0x0c MSM + 0x0f 4-pairing)
  ↓
Contract marks note nullifier(s) spent, inserts change note(s) to MMR tree (_mmr_insert),
pays recipient token on-chain, and accrues execution fee to relayer.
  ↓
Exact value conservation maintained: Input Note(s) = Payout + Protocol Fee (45 bps) + Execution Fee + Change Note(s)
```

### 4. Institutional Compliance & CEX Selective Disclosure (DEC-040)
```
User needs to deposit to Centralized Exchange (Binance/Coinbase) or provide B2B audit trail
  ↓
4-Tier Key Hierarchy: Spending Key (sk) -> Nullifier Key (nk) -> Full Viewing Key (fvk) -> IVK/OVK
  ↓
User generates SelectiveDisclosureCircuit Groth16 proof with Verifier Binding (CEX address + nonce)
  ↓
CEX verifies proof via CEX Verifier Engine: proves legitimate origin & note ownership
WITHOUT exposing spending authority (sk), unspent notes, or total account balance.
```

## Invariants — Break These and It's a Critical Bug

**Accounting & Value Conservation (DEC-034B / DEC-038B):**
- Siloed Multi-Asset Solvency: `contract Token balance >= total_liabilities (user_note_liabilities[a] + accrued_execution_fee_liabilities[a] + realized_protocol_fees[a])` for each asset (USDC & WETH) — checked after every state change, reverts on insolvency.
- `gross_deposit = deposit_amount = net_liability` (deposit fee = 0 bps immutable).
- `Input Note(s) = Payout + Protocol Fee + Execution Fee + Change Note(s)` (ZK-UTXO value conservation).
- One commitment adds liability exactly once (inserted into MMR accumulator).
- One nullifier reduces liability exactly once (siloed `note_nullifiers[nullifier] == true` double-spend protection).

**Strict 3-Mode JoinSplit Rules (DEC-043B):**
- Mode 1 (Mint / Consolidation): `recipient == Address::ZERO`, `merchant_amount == 0`, `protocol_fee == 0`.
- Mode 2 (In-Pool Transfer): `recipient == Address::ZERO`, `merchant_amount == 0`, `protocol_fee == 0`, `has_change_1 == true`. Dual 96-byte memos mandatory.
- Mode 3 (Public Spend): `recipient != Address::ZERO`, `protocol_fee == 45 bps`. Calldata memo 1 & memo 2 must be exactly 96 bytes each.

**Direct Deposit (DEC-038B / DEC-034B):**
- Inserts note commitment directly into Merkle Mountain Range (`_mmr_insert`).
- Commitment must be canonical scalar (`from_evm_scalar < r`), min amount (10 USDC or 0.005 WETH), non-zero.
- Emits `NoteCommitmentAppended` with 96-byte AEAD encrypted memo (zero storage bloat, log-only).
- Pure ZK-UTXO: no sessions, no threshold blind signers, no refund timelocks needed.

**Spend (ZK-UTXO Private Note & Universal JoinSplit):**
- ZK-UTXO Groth16 check via EIP-2537 (`0x0c` MSM + `0x0f` 4-pairing):
  `e(-A, B) * e(alpha, beta) * e(L, gamma) * e(C, delta) == 1`.
- Semantic public input binding (DEC-022 / DEC-035B / DEC-036A): exact match of root, nullifiers, output commitments, recipient, amounts, fees, two-limb quote hash, chain_id, contract_address, expiry, flags_packed, and is_rollover.
- Double-spend protection: generational nullifier & siloed note nullifiers (`note_nullifiers[nullifier] == true`).

**CCIP:**
- Router must be non-zero, source chain + sender allowlisted, message ID replay-protected.
- Source confirmation ≠ destination confirmation.

## Behavior Rules

### Before Coding

1. **Read `todo.md`** — check item status before touching it.
2. **Check `research/decisions/`** — 71 ADRs available for reference (DEC-001 through DEC-047).
3. **Navigasi Kode & Arsitektur Wajib Graphify-First:**
   - **Refleks Pertama (Graphify):** Sebelum melakukan grep atau membaca file mentah, SELALU jalankan:
     * `graphify query "<pertanyaan arsitektur atau integrasi>"` (atau via MCP `query_graph`) untuk melihat subgraph relasi antar file/fungsi.
     * `graphify explain "<Simbol / Struct / Fungsi>"` (atau via MCP `get_node`) untuk melihat posisi node, degree, dan relasi langsungnya.
     * `graphify path "<A>" "<B>"` (atau via MCP `shortest_path`) untuk melacak rantai pemanggilan antar dua komponen.
     * *Pantangan:* JANGAN membaca langsung file `graphify-out/graph.json` atau seluruh `GRAPH_REPORT.md` ke context window (sangat boros token).
   - **Call Path & Verbatim Code (CodeGraph):** Panggil MCP tool `codegraph_explore` (atau shell `codegraph explore "<query>"`) untuk melihat isi baris implementasi dan dynamic hops setelah lokasinya dipetakan oleh Graphify.
   - **Maintenance:** Setelah melakukan modifikasi file Rust, jalankan `graphify update .` untuk menjaga graf tetap sinkron tanpa memicu LLM cost.
4. **Prinsip ATMI (Amati, Tiru, Modifikasi, Inovasi)** — Belajar dan serap pola unggul dari paper kriptografi teruji dan repo battle-tested (seperti Zcash, Aztec Barretenberg, Railgun, zk-kit, zk-sunade, Wasabi). Amati polanya, tiru fondasinya, modifikasi agar cocok dengan Stylus/BLS12-381/ZK-UTXO Zeltra, dan inovasikan keunggulan baru (privacy 9.5/10, produk 10/10).
5. **Research Gate & External Intel (Exa & Tavily):**
   Gunakan kedua tool pencarian ini secara kontekstual dan saling melengkapi sesuai kekuatannya:
   - **Exa MCP (`web_search_exa`, `web_fetch_exa`) — Deep Technical & Code Archaeology:**
     * *Best for:* Bedah paper kriptografi (2024–2026), post-mortem eksploitasi, riset implementasi lib/crate eksternal, dan dokumen arsitektur panjang. Exa mencari berdasarkan makna semantik ("deskripsikan halaman idealnya"), bukan sekadar keyword.
   - **Tavily MCP (`tavily_search`, `tavily_extract`, `tavily_crawl`) — Real-Time Fact, News & Governance:**
     * *Best for:* Pengecekan status live ekosistem (misal: voting AIP ArbitrumDAO, update rilis ArbOS/Stylus, breaking news), fact-checking cepat, atau crawling halaman web tertentu.
   - *Sinergi Alur:* Saat menghadapi problem kritis (finansial, kriptografi, upgrade contract), gunakan Tavily untuk memetakan situasi/status terkini, lalu gunakan Exa untuk menyelami referensi teknis dan paper primernya. Catat temuan kunci di `research/decisions/`.
6. **Context Tools (Context-Mode):** Prioritaskan `ctx_execute`, `ctx_search`, dan `ctx_batch_execute` saat inspeksi data/log besar (>200 lines) atau multi-sumber agar raw bytes tidak membebani context window LLM.
7. **Build & Compiler Output (RTK):** Gunakan tooling RTK/tokless untuk memangkas output cargo/compiler dan trace command yang panjang demi efisiensi token dan feedback loop cepat.
8. **Groq LPU Acceleration (`qwen/qwen3.8-27b`):** Engine komputasi inferensi ultra-cepat (~200–400+ tok/s) via endpoint OpenAI-compatible (`OPENAI_BASE_URL="https://api.groq.com/openai/v1"`). Digunakan untuk semantic extraction Graphify tanpa delay dan benchmarking cepat.

### While Coding

- **Circuit Status (FLEXIBLE & UNLOCKED):** Deploy Arbitrum sedang dalam masa pause/freeze (Oktober 2026), sehingga sirkuit Zeltra **BELUM DIKUNCI (CIRCUIT IS NOT FROZEN)**. Tim memiliki fleksibilitas penuh untuk menyempurnakan sirkuit, menambah gadget, dan mengubah public inputs tanpa batasan freeze prematur.
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

| Operation / Flow | Rate | Where | Rationale |
|---|---|---|---|
| Inflow (Deposit) | 0 bps (0.00%) | `deposit.rs` + `fees.rs` | Zero-friction liquidity entry (DEC-038B / DEC-034B) |
| In-Pool Transfer (Mode 2) | 0 bps (0.00%) | `spend.rs` + `fees.rs` | Circular liquidity velocity inside pool (DEC-043B) |
| Public Outflow Spend (Mode 3) | 45 bps (0.45% Flat) | `spend.rs` + `fees.rs` | Protocol monetization upon withdrawal (DEC-028) |
| Relayer Execution | Gas reimbursement + 15% markup | `quote.rs` + `fees.rs` | Zero-oracle gas reimbursement in native units (DEC-031 / DEC-034C) |

## Key Files & Docs

- [`todo.md`](file:///workspaces/Zeltra-Protocol/todo.md) — Master active backlog & test status
- [`VISION.md`](file:///workspaces/Zeltra-Protocol/VISION.md) & [`docs/bisnis.md`](file:///workspaces/Zeltra-Protocol/docs/bisnis.md) — Product vision, business model, & fee architecture
- [`docs/`](file:///workspaces/Zeltra-Protocol/docs/) — Chapter-based modular docs (`core/`, `contract/`, `relayer/`, `sdk/`, `cli/`)
- [`research/decisions/`](file:///workspaces/Zeltra-Protocol/research/decisions/) — 71 Architecture ADRs (DEC-001 through DEC-047)

## Environment, Deployment & Cluster

- **Runtime Config:** `zeltra-node/.env.test` is the single source of truth (RPC URLs, keys, contract addresses, DB cipher keys, Vault/KMS settings). Always run `source zeltra-node/.env.test` before executing nodes or deployment tools.
- **Stylus Contract Deployment:** `cargo stylus check` / `cargo stylus deploy` from `zeltra-contracts/`.
- **Relayer Sequencer Automation (DEC-038A):**
  ```bash
  bash scripts/start_relayer.sh {start|status|stop|restart}  # Single sequencer relayer node
  ```

Auto-loads `zeltra-node/.env.test` for RPC/contract/key. Health check endpoint `/health` & `/live`.

