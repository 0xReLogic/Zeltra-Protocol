# ARCHITECTURE.md — Zeltra Protocol Code Mapping & Architecture

Dokumen ini memetakan struktur kode riil, crate, dan hirarki direktori di seluruh workspace **Zeltra Protocol**.

---

## 1. Repository Layout & Code Mapping

```text
/workspaces/Zeltra-Protocol/
├── zeltra-contracts/                 — Smart Contract Arbitrum Stylus WASM (EIP-2537 BLS12-381)
│   ├── src/lib.rs                    — Entrypoint #[entrypoint] contract Zeltra:
│   │                                   • Inflow: deposit_direct_note, deposit_direct_weth, deposit_direct_native_eth
│   │                                   • Outflow/Transfer: spend_joinsplit (Strict 3-Mode), spend_private_note
│   │                                   • Solvensi: check_asset_solvency_invariant, check_liability_invariant
│   │                                   • Nullifier & MMR: is_nullifier_spent, note_tree_root, mmr_leaf_count, etc.
│   ├── src/deposit.rs                — Logika deposit direct note ERC20 & native ETH auto-wrap (_mmr_insert)
│   ├── src/spend.rs                  — Logika eksekusi Strict 3-Mode JoinSplit & private spend
│   ├── src/merkle.rs                 — Akumulator on-chain Merkle Mountain Range (_mmr_insert, canonical peak bagging)
│   ├── src/storage.rs                — Stylus sol_storage! Zeltra layout (MMR state, accepted roots, nullifiers, multi-asset)
│   ├── src/groth16_note_verifier.rs  — Verifier Groth16 BLS12-381 untuk PrivateNoteCircuit (16 public inputs) via precompile 0x0c & 0x0f
│   ├── src/groth16_joinsplit_verifier.rs — Verifier Groth16 BLS12-381 untuk JoinSplitCircuit (19 public inputs) via precompile 0x0c & 0x0f
│   ├── src/verification.rs          — Wrapper pemanggilan host precompile EIP-2537 (call_msm_g1 0x0c, call_pairing 0x0f)
│   ├── src/vault.rs                  — Akuntansi solvensi multi-aset terisolasi (_check_asset_solvency_invariant, total_assets)
│   ├── src/views.rs                  — View/getter functions untuk status on-chain
│   ├── src/governance.rs             — Pause, unpause, dan pengaturan parameter darurat
│   ├── src/events.rs                 — Event NoteCommitmentAppended (memo 96B), JoinSplitSpendExecuted, dll.
│   ├── src/poseidon_w5_constants.rs  — Konstanta round constants & MDS matrix Poseidon width-5 in-contract
│   ├── src/interfaces.rs, types.rs, constants.rs, helpers.rs — Interface ERC-20 / CCIP & konversi tipe EVM
│   ├── src/bin/contract_interop.rs   — Binary interop contract CLI
│   └── abi/IZeltra.sol               — Antarmuka Solidity ABI
│
├── zeltra-core/                      — Primitif Kriptografi & Sirkuit ZK (Arkworks 0.6.0 BLS12-381)
│   ├── src/lib.rs                    — Re-export BLS12-381, Poseidon, EVM, fees, MMR, dan sirkuit
│   ├── src/note_circuit.rs           — PrivateNoteCircuit (1-in-1-out Groth16, 16 public inputs, two-limb quote hash, DEC-035B)
│   ├── src/joinsplit_circuit.rs      — JoinSplitCircuit (Universal 2-in-2-out Groth16, 19 public inputs, companion constraints DEC-046)
│   ├── src/selective_disclosure_circuit.rs — SelectiveDisclosureCircuit (CEX compliance Groth16 dengan Verifier Binding, DEC-040A)
│   ├── src/circuit_gadgets.rs        — Reusable R1CS gadgets (flags_packed multi-asset, range checks, Poseidon gadgets)
│   ├── src/note.rs                   — Note primitives (PrivateNoteV1 dengan asset_id, NotePlaintext)
│   ├── src/keys.rs                   — Derivasi kunci ZSA (SpendingKey, NullifierKey, ViewingKey, IncomingViewingKey)
│   ├── src/memo.rs                   — Committing AEAD 96B memo (EtH-KS transform, ChaCha20 + HMAC-SHA256, 1B View Tag, DEC-048)
│   ├── src/memo_ct.rs                — Constant-Time Flag Oblivious Evaluation (CT-FOE), branchless canonical Fr, zeroize (DEC-049)
│   ├── src/merkle.rs                 — Flat Merkle Mountain Range (FlatMerkleMountainRange, Peter Todd indexing)
│   ├── src/poseidon.rs               — Poseidon width-3 & width-5, Grain-128 LFSR parameter generator (DEC-021)
│   ├── src/jacobian.rs               — Mesin kalkulasi Augmented Jacobian M(a) untuk uji kekakuan tangen (DEC-047)
│   ├── src/formal_tests.rs           — Test suite formal Augmented Jacobian & verifikasi rank finansial (DEC-047)
│   ├── src/accounting.rs             — State machine ContractAccounting & validasi invariant matematis
│   ├── src/fees.rs                   — Kalkulasi fee (0 bps deposit, 0 bps transfer, 45 bps spend, relayer markup)
│   ├── src/curve_opt.rs              — Fast Jacobian point tripling 3P (4S+6M) & Eisenstein tau-endomorphism (3S+3M, DEC-050)
│   ├── src/evm.rs                    — Serialisasi & formatting EVM untuk G1, G2, skalar, dan proof BLS12-381
│   ├── src/serialization.rs          — Helper serialisasi data arkworks
│   └── src/bin/                      — Binary CLI / benchmarks (bench.rs, flat_mmr_interop.rs, zsa_interop.rs)
│
├── zeltra-node/                      — Stateless Single-Sequencer Relayer Node (Axum 0.8)
│   ├── src/main.rs                   — Server boot, background worker orchestration, rate limiter
│   ├── src/lib.rs                    — Library exports untuk relayer testing & integration
│   ├── src/router.rs                 — Axum routes definition
│   ├── src/state.rs, config.rs, dto.rs — AppState, network configuration, DTOs
│   ├── src/handlers/                 — HTTP Endpoint Handlers:
│   │   ├── health.rs                 — /health, /live, /ready & metrik makro solvensi
│   │   ├── quote.rs                  — /api/v1/quote (Signed EIP-712 execution fee quote)
│   │   ├── spend.rs                  — /api/v1/spend-joinsplit & /api/v1/spend-private-note
│   │   ├── mmr.rs                    — /api/v1/mmr/proof/:leaf_index, /api/v1/mmr/leaves, /api/v1/notes/sync
│   │   ├── tx_status.rs              — /api/tx-status
│   │   └── x402.rs                   — /api/x402/verify (ZK-UTXO HTTP 402 Facilitator gateway)
│   ├── src/database/                 — Modul Database SQLite + SQLCipher AES-256 (WAL mode):
│   │   ├── mod.rs, types.rs          — Connection pool, migrations, schema definitions
│   │   ├── spends.rs, quotes.rs      — Queue spend ingress & quote storage
│   │   ├── mmr.rs, nullifiers.rs     — Leaf cache & status nullifier on-chain
│   │   ├── relayer_tx.rs, ccip.rs    — Transaksi broadcast & tracking CCIP
│   │   └── idempotency.rs            — Idempotency deduplication guard
│   ├── src/validation/               — Preflight validation (spend_preflight.rs: skalar kanonikal, nullifier check)
│   ├── src/evm_client.rs             — Alloy client untuk broadcast L2 tx, gas bumping, dan receipt tracking
│   ├── src/mmr_indexer.rs            — Background worker pengindeks event NoteCommitmentAppended & memo
│   ├── src/mempool_watchdog.rs       — Background worker pemulih lease stalled & pemantau nullifier
│   ├── src/execution_fee_claimer.rs  — Background worker klaim akumulasi fee dari smart contract
│   ├── src/sanctions_sync.rs         — Background worker sinkronisasi harian daftar OFAC SDN
│   ├── src/ccip_monitor.rs           — Background worker tracking pesan CCIP & handler refund
│   ├── src/batch_processor.rs        — Pemrosesan batch transaksi relayer
│   └── src/bin/                      — Binary CLI (inpool_interop.rs, relayer_test_server.rs, mmr_bench.rs, query_db.rs)
│
├── zeltra-sdk/                       — Client SDK & Private Note Wallet (WASM + Native)
│   ├── src/lib.rs                    — Re-exports, memory zeroization guards (secure_zeroize)
│   ├── src/coin_selection.rs         — Stochastic Knapsack multi-asset coin selection
│   ├── src/storage.rs                — 2PC crash-safe note store terenkripsi Argon2id + ChaCha20Poly1305
│   ├── src/client.rs                 — HTTP client komunikasi ke relayer endpoints
│   ├── src/eip712.rs                 — EIP-712 signer untuk validasi quote relayer
│   ├── src/zk_wasm.rs, fee_wasm.rs   — Prover WASM in-browser runner dengan Web Worker lifecycle (DEC-041)
│   ├── src/wasm_types.rs             — Serializable DTOs untuk bindings JS/TS
│   ├── src/wallet/                   — Core Private Note Wallet:
│   │   ├── mod.rs                    — Re-exports & Wallet interfaces
│   │   └── note_wallet/              — Modular Wallet Implementation:
│   │       ├── mod.rs                — PrivateNoteWallet struct & core state
│   │       ├── deposit.rs            — Alur deposit direct note (USDC & WETH/ETH)
│   │       ├── joinsplit.rs          — In-pool shielded transfer & mode 2/3 proof generation
│   │       ├── spend.rs              — Public spend & outflow generation
│   │       ├── sync.rs               — Sinkronisasi MMR & View Tag fast scanner (recover_notes_batch)
│   │       ├── compliance.rs         — Integrasi pembuatan bukti Selective Disclosure
│   │       ├── keys.rs               — Manajemen kunci wallet & derive child keys
│   │       ├── state_2pc.rs          — Two-phase commit safe state transitions
│   │       └── types.rs              — Tipe data internal note wallet
│   ├── src/compliance/               — Modul Kepatuhan & Audit B2B (DEC-040):
│   │   ├── exporter.rs               — Ekspor laporan audit double-entry (CSV/JSON)
│   │   ├── verifier.rs               — Mesin verifikasi independen CEX (Binance/Coinbase)
│   │   └── models.rs                 — Model data compliance & challenge-response
│   ├── src/x402/                     — Pure ZK-UTXO HTTP 402 Client Protocol (DEC-042):
│   │   ├── codec.rs                  — Header parser & serializer untuk X-402-Invoice / Authorization
│   │   └── types.rs                  — Struct invoice, payment receipt, status
│   ├── js/                           — JavaScript / TypeScript WASM Web Worker Bridge:
│   │   ├── index.ts, index.js        — Entrypoint JS SDK package
│   │   ├── prover_client.ts/.js      — Client wrapper pengelola worker
│   │   └── zeltra_prover_worker.js   — Web Worker background thread (DEC-041)
│   ├── tests/side_channel_defense_tests.rs — Matriks uji adversarial 5 kategori & evaluasi timing Dudect (|t| < 4.5) (DEC-049)
│   └── src/bin/                      — Benchmark & bridge (bench_e2e.rs, prover_worker_bridge.rs, dudect_ct_foe.rs)
│
├── zeltra-cli/                       — Pure ZK CLI Utility Tool
│   ├── src/main.rs, cli.rs           — Parser Clap: GenerateWallet, CreateDepositNote, InspectMmr, VerifyProof, Compliance
│   ├── src/lib.rs                    — Library CLI exports
│   └── src/commands/                 — Subcommand handlers:
│       ├── zk.rs                     — Pembuatan wallet, deposit note, inspeksi MMR, verifikasi proof
│       └── compliance.rs             — Ekspor audit compliance & verifikasi bukti CEX
│
├── scripts/                          — Automation, Testing & Deployment
│   ├── start_relayer.sh              — Daemon runner relayer sequencer tunggal (start|status|stop|restart)
│   ├── deploy_testnet.sh             — Deployment contract Stylus ke Arbitrum Sepolia
│   ├── test_dec048_committing_aead.py — Suite verifikasi adversarial EtH-KS committing AEAD DEC-048 (11/11 passed)
│   ├── test_real_case_dec043c_inpool_wallet.py — Test suite Python independen (61/61 in-pool lifecycle tests)
│   ├── test_dec046_dec047_jacobian_soundness.py — Suite verifikasi formal Jacobian rank DEC-046 & DEC-047
│   └── check_balance.py, ht01_init_verification.py, ht04_spend_negative_tests.py, ht05_refund_tests.py, ht10_signed_quote_tests.py
│
├── research/dec/               — 74 Architecture Decision Records (DEC-001 s/d DEC-050)
├── docs/                             — Documentation (bisnis.md, roadmap.md, X402_INTEGRATION_STRATEGY.md, analisis/)
└── todo.md                           — Master active checklist (pending tasks only)
```

---

## 2. Core Stack

* **Smart Contract:** Rust Stylus SDK 0.10.7, Alloy 2.0, Arbitrum Nitro WASM, EIP-2537 BLS12-381 Precompiles (`0x0c` MSM + `0x0f` 4-pairing).
* **Cryptography & Circuits:** Arkworks 0.6.0 (`ark-bls12-381`, `ark-groth16`, `ark-r1cs-std`), Poseidon hash (width-3 & width-5), Grain-128 LFSR.
* **Relayer Node:** Axum 0.8, Tokio, SQLite + SQLCipher AES-256 (`bundled-sqlcipher`), Alloy EVM client.
* **Client SDK:** Dual Native/WASM, Web Worker disposable lifecycle, Argon2id + ChaCha20Poly1305.
