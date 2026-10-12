# AGENTS.md

## What This Is

**Zeltra Protocol** (codebase transition name: Nimbus) = **"The Stripe of Web3 with Absolute Privacy."** A high-throughput, shielded payment and settlement rail on Arbitrum Stylus for AI Agents, retail, whales, B2B corporates, and global merchants.

Full vision manifesto: [`VISION.md`](file:///workspaces/Zeltra-Protocol/VISION.md).

Product Mantra: **Privacy 9.5/10, Product 10/10.** Apple Pay / QRIS speed with real-cash mental accounting ($10 - $3 = $7 change via ZK-UTXO Private Notes). Engineering hell stays in the background; user experience stays effortless.

Zero-friction inflow (0% deposit fee), volume-over-TVL economic velocity, and absolute mathematical solvency: **one deposit pays out exactly once, no more, no less.**

## Repo Layout

```
nimbus-contracts/  — Stylus WASM smart contract (direct MMR deposit, 1-in-1-out & 2-in-2-out JoinSplit Groth16, Merkle Mountain Range / MMR)
nimbus-core/       — Crypto primitives (BLS12-381, Groth16 PrivateNoteCircuit & JoinSplitCircuit, Poseidon Grain-128, MMR, viewing keys, fees)
nimbus-node/       — Stateless single-sequencer relayer node (ZK note & JoinSplit ingress, preflight verifier, direct EVM settlement, MMR sync API)
nimbus-sdk/        — Client SDK (PrivateNoteWallet, Knapsack coin selection, local Groth16 prover, 2PC encrypted store, viewing key recovery)
nimbus-cli/        — Pure ZK CLI tool (wallet, direct deposit note, MMR inspection, proof verification)
scripts/           — Test scripts & cluster automation (scripts/start_relayer.sh)
research/decisions/ — Design decisions (DEC-001 through DEC-038C)
docs/              — Modular chapter-based docs
todo.md            — Master active checklist (pending tasks only)
```

Stack: Rust (Stylus SDK 0.10.7, Alloy 2.0, Arkworks 0.6.0 `ark-bls12-381` & `ark-groth16`), Axum 0.8, SQLite with SQLCipher AES-256 (`bundled-sqlcipher`), OpenBao/Vault KMS.

## Core Business Flow (Pure ZK-UTXO Architecture — DEC-038)

### 1. Inflow: Zero-Friction Direct MMR Deposit (DEC-038B / DEC-038C)
```
User SDK derives Note (value, owner_key, rho, r) & computes Poseidon-W5 commitment (<15 us)
  ↓
User SDK encrypts 96-byte AEAD ChaCha20Poly1305 memo via Ephemeral X25519 ECDH (DEC-037)
  ↓
User calls deposit_direct_note(amount, note_commitment, encrypted_memo) on Stylus contract
  ↓
Contract validates canonical scalar (< r), min amount, inserts leaf to MMR tree (_mmr_insert),
emits NoteCommitmentAppended event with memo, and updates user_note_liability. (0 bps deposit fee)
  ↓
Note is immediately unspent & active (Zero threshold ceremony, zero masking key release).
```

### 2. Outflow: ZK-UTXO Spend & Change Note (1-in-1-out or 2-in-2-out JoinSplit)
```
User selects Note UTXO(s) via PrivateNoteWallet Knapsack selector (SDK, DEC-024 / DEC-030 / DEC-036B)
  ↓
User generates local Groth16 proof (PrivateNoteCircuit / JoinSplitCircuit: value conservation,
Poseidon nullifiers, MMR membership proof, 64-bit range constraints, formal scope binding DEC-035B)
  ↓
User submits proof & public inputs to Relayer (/api/v1/spend-private-note or /spend-joinsplit)
  ↓
Relayer verifies canonical scalars (< r), checks binding, runs local CPU Groth16 preflight check (DEC-026 C-01),
and verifies dual-layer nullifier protection (SQLite DB & on-chain is_nullifier_spent)
  ↓
Relayer broadcasts spend transaction to Stylus contract
  ↓
Contract verifies Groth16 via EIP-2537 precompiles (0x0c MSM + 0x0f 4-pairing)
  ↓
Contract marks note nullifier(s) spent, inserts change note(s) to MMR tree (_mmr_insert),
pays recipient USDC, and accrues execution fee to relayer.
  ↓
Exact value conservation maintained: Input Note(s) = Payout + Protocol Fee + Execution Fee + Change Note(s)
```

## Invariants — Break These and It's a Critical Bug

**Accounting & Value Conservation (DEC-038B):**
- `contract USDC balance >= total_liabilities (user_note_liability + accrued_execution_fee_liability + realized_protocol_fees)` — checked after every state change, reverts on insolvency
- `gross_deposit = deposit_amount = net_liability` (deposit fee = 0 bps immutable)
- `Input Note(s) = Payout + Protocol Fee + Execution Fee + Change Note(s)` (ZK-UTXO value conservation)
- One commitment adds liability exactly once (inserted into MMR accumulator)
- One nullifier reduces liability exactly once (siloed `note_nullifiers[nullifier] == true` double-spend protection)

**Direct Deposit (DEC-038B):**
- Direct deposit inserts note commitment directly into Merkle Mountain Range (`_mmr_insert`)
- Commitment must be canonical scalar (`from_evm_scalar < r`), min 10 USDC, non-zero
- Emits `NoteCommitmentAppended` with 96-byte AEAD encrypted memo (zero storage bloat, log-only)
- Pure ZK-UTXO: no sessions, no threshold blind signers, no refund timelocks needed

**Spend (ZK-UTXO Private Note & Universal JoinSplit):**
- ZK-UTXO Groth16 check via EIP-2537 (`0x0c` MSM + `0x0f` 4-pairing):
  `e(-A, B) * e(alpha, beta) * e(L, gamma) * e(C, delta) == 1`
- Semantic public input binding (DEC-022 / DEC-035B): exact match of recipient, amount, fees, two-limb quote hash (`quote_hash_hi`, `quote_hash_lo`), chain_id, contract_address, expiry, has_change, and formal scope binding commitment
- Double-spend protection: siloed nullifier (`note_nullifiers[nullifier] == true`)
- Fee policy: 0 bps deposit, flat 45 bps protocol spend fee (DEC-028), gas reimbursement + 15% relayer markup (DEC-031)

**CCIP:**
- Router must be non-zero, source chain + sender allowlisted, message ID replay-protected
- Source confirmation ≠ destination confirmation

## Behavior Rules

### Before Coding

1. **Read `todo.md`** — check item status before touching it
2. **Check `research/decisions/`** — there might be a relevant decision
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
     * *Best for:* Bedah paper kriptografi (2024–2026), post-mortem eksploitasi, riset implementasi lib/crate eksternal, dan dokumen arsitektur panjang. Exa mencari berdasarkan makna semantik ("deskripsikan halaman idealnya"), bukan sekadar keyword, sehingga sangat efektif menemukan bukti matematis atau contoh implementasi kode tersembunyi.
   - **Tavily MCP (`tavily_search`, `tavily_extract`, `tavily_crawl`) — Real-Time Fact, News & Governance:**
     * *Best for:* Pengecekan status live ekosistem (misal: voting AIP ArbitrumDAO, update rilis ArbOS/Stylus, breaking news), fact-checking cepat, atau crawling halaman web tertentu. Tavily dioptimalkan untuk latency rendah dan akurasi fakta real-time.
   - *Sinergi Alur:* Saat menghadapi problem kritis (finansial, kriptografi, upgrade contract), gunakan Tavily untuk memetakan situasi/status terkini, lalu gunakan Exa untuk menyelami referensi teknis dan paper primernya. Catat temuan kunci di `research/decisions/`.
6. **Context Tools (Context-Mode):** Prioritaskan `ctx_execute`, `ctx_search`, dan `ctx_batch_execute` saat inspeksi data/log besar (>200 lines) atau multi-sumber agar raw bytes tidak membebani context window LLM.
7. **Build & Compiler Output (RTK):** Gunakan tooling RTK/tokless untuk memangkas output cargo/compiler dan trace command yang panjang demi efisiensi token dan feedback loop cepat.

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
- [`research/decisions/`](file:///workspaces/Zeltra-Protocol/research/decisions/) — Architecture ADRs (DEC-001 through DEC-038C)

## Environment, Deployment & Cluster

- **Runtime Config:** `nimbus-node/.env.test` is the single source of truth (RPC URLs, keys, contract addresses, DB cipher keys, Vault/KMS settings). Always run `source nimbus-node/.env.test` before executing nodes or deployment tools.
- **Stylus Contract Deployment:** `cargo stylus check` / `cargo stylus deploy` from `nimbus-contracts/`.
- **Relayer Sequencer Automation (DEC-038A):**
  ```bash
  bash scripts/start_relayer.sh {start|status|stop|restart}  # Single sequencer relayer node
  ```

Auto-loads `nimbus-node/.env.test` for RPC/contract/key. Health check endpoint `/health` & `/live`.
