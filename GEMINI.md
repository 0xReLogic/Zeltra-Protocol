# GEMINI.md — Zeltra Protocol Context & Working Guide

Dokumen ini adalah memori konteks persisten untuk agen AI saat bekerja di repository **Zeltra Protocol (Zeltra)**. Jika terjadi kompresi token context window atau sesi baru dimulai, jadikan dokumen ini sebagai kompas utama.

---

## 1. Identitas & Misi Proyek

* **Nama:** **Zeltra Protocol** *(codebase transisi: Zeltra)*.
* **Manifesto Lengkap:** Baca [`VISION.md`](file:///workspaces/Zeltra-Protocol/VISION.md).
* **Visi & Posisi:** **"The Stripe of Web3 with Absolute Privacy."** Rel pembayaran dan settlement universal untuk AI Agent, retail, whale, korporat B2B, dan merchant tanpa diskriminasi.
* **Mantra Produk:** **Privasi 9.5/10, Produk 10/10.** User experience secepat dan seringan Apple Pay / QRIS dengan mental model kas nyata ($10 bayar $3 kembali $7 lewat ZK-UTXO Private Note). Beban komputasi kriptografi berat diselesaikan di belakang layar.
* **Filosofi Finansial:**
  * *Zero-Friction Inflow:* Deposit 0 bps (0.00%) permanen.
  * *Volume Over TVL:* Monetisasi berbasis kecepatan perputaran uang (velocity), bukan mengunci modal diam.
  * *Mathematical Solvency:* Invariant mutlak `Assets >= Liabilities` (anti-fractional reserve).
  * *Etika Kepatuhan:* *"Permissionless Privacy, Permissioned Acceptance"* via Receiver-Enforced Modular ASP & Low-Cost Sanctions Screening ([`DEC-026`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-026-receiver-enforced-compliance-zero-cost-sanctions-filtering-relayer-protection.md)).
* **Stack Utama:** Rust (Stylus SDK 0.10.7, Alloy 2.0, Arkworks 0.6.0 `ark-bls12-381` & `ark-groth16`), Axum 0.8, SQLite dengan SQLCipher AES-256 (`bundled-sqlcipher`), OpenBao/Vault KMS.

---

## 2. Status Terkini & Keputusan Arsitektur

### A. Evolusi Arsitektur: Migrasi Penuh ke Pure ZK-UTXO (DEC-038)
1. **Pemusnahan Total Legacy Threshold BLS (DEC-038A):** Seluruh kode blind signing, guardian network, session storage (11 storage slot kontrak), dan alur refund timelock 24 jam telah dihapus bersih. Arsitektur kini 100% murni ZK-UTXO.
2. **Direct MMR Deposit (DEC-038B):** Deposit langsung via `deposit_direct_note(...)` menyisipkan komitmen note ke Merkle Mountain Range (`_mmr_insert`), memancarkan event `NoteCommitmentAppended` berisi 96-byte AEAD memo, dan seketika berstatus unspent tanpa perantara kunci $k$.
3. **Stateless Sequencer Relayer (DEC-038C):** Cluster 1 leader + 4 guardian digantikan oleh single stateless sequencer node (`scripts/start_relayer.sh`), murni melayani ingress ZK spend, preflight verification, dan MMR dynamic sync.

### B. Pondasi Kriptografi & Sirkuit ZK (Gate C, D, E, F, H, I)
* **Sirkuit Groth16 (zeltra-core):**
  - **1-in-1-out (`PrivateNoteCircuit`):** 16 Public Inputs (DEC-035A), 128-bit Two-Limb quote hash, Formal Scope Binding Gadget (DEC-035B), Poseidon Grain-128 LFSR (DEC-021), 64-bit integer range constraints, dan evolusi nullifier 2-Epoch (DEC-033).
  - **Universal 2-in-2-out (`JoinSplitCircuit`):** 19 Public Inputs (DEC-036A), constant-arity topology, canonical dummy input/output slots, dan 448 quadratic range gates.
* **Akumulator MMR (DEC-032):** In-memory dan on-chain Merkle Mountain Range dengan canonical peak bagging backward fold (`DOMAIN_MMR_BAG`) dan mitigasi Hyperbridge out-of-bounds ($leaf\_index < leaf\_count$).
* **Smart Contract Stylus (zeltra-contracts):**
  - Verifikasi Groth16 BLS12-381 via EIP-2537 (`0x0c` MSM + `0x0f` Pairing Check) di WASM Stylus.
  - State MMR terintegrasi, history accepted roots bounded ring, dan entrypoints `deposit_direct_note`, `spend_private_note`, serta `spend_joinsplit`.
  - Invariant solvensi absolut: `USDC Balance >= user_note_liability + accrued_execution_fee_liability + realized_protocol_fees`.
* **Client SDK & Wallet State (zeltra-sdk):**
  - `PrivateNoteWallet`: Tiered Stochastic Knapsack Coin Selection (DEC-030/036B), in-pool `consolidate_notes()`, 2PC crash-safe note store (DEC-024), Argon2id + ChaCha20Poly1305 encrypted storage, dan zeroize memory safety.
  - Viewing Key Recovery Protocol (DEC-037): Derivasi kunci HKDF (`IncomingViewingKey`, `OutcomingViewingKey`), 48-byte `NotePlaintext`, compact 96-byte AEAD memo, dan pemulihan saldo on-chain (`recover_notes`).
* **Relayer Pipeline (zeltra-node):**
  - Endpoints `/api/v1/spend-private-note`, `/api/v1/spend-joinsplit`, `/api/v1/mmr/proof/:leaf_index`.
  - Fail-closed local Groth16 preflight verification (DEC-026 C-01 anti gas-griefing).
  - Three-Way Reconciliation Engine (`reconcile_execution_fees`) dan solvency observability pada `/health` (DEC-031).

---

## 3. Prioritas Aktif Selanjutnya

Mengacu pada master checklist [`todo.md`](file:///workspaces/Zeltra-Protocol/todo.md):

1. **Hard-Test E2E Arbitrum Sepolia (Gate G / Gate C1 Step 5):**
   * Deposit direct $\rightarrow$ partial spend $\rightarrow$ change note $\rightarrow$ second spend $\rightarrow$ withdraw-all di testnet Arbitrum Sepolia.
   * Crash recovery & idempotency boundary testing.
2. **Cross-Chain CCIP Testing & Verification (Phase 4):**
   * Testnet E2E di Arbitrum Sepolia (source broadcast $\rightarrow$ router $\rightarrow$ destination execution $\rightarrow$ destination confirmation).
   * Uji jalur kegagalan destination failure.
3. **MPC Ceremony & Circuit Freeze (Gate C):**
   * Freeze circuit constraints & public ABI sebelum production trusted setup ceremony.
4. **Credential Rotation & Vault TLS Hardening:**
   * Rotasi secret historis & auto-renew AppRole Vault tokens.


---

## 4. Peta Struktur Repository & Pemetaan Kode Riil

```text
/workspaces/Zeltra-Protocol/
├── zeltra-contracts/         — Smart Contract Arbitrum Stylus WASM (EIP-2537 BLS12-381 Precompiles)
│   ├── src/lib.rs            — Entrypoint #[entrypoint] contract Zeltra:
│   │                           • Inflow: deposit_direct_note, deposit
│   │                           • Outflow: spend_private_note, spend_joinsplit
│   │                           • Solvensi: check_liability_invariant, user_note_liability, accrued_execution_fee_liability, realized_protocol_fees
│   │                           • Nullifier: is_nullifier_spent, is_note_nullifier_spent, are_nullifiers_spent, is_generational_nullifier_spent
│   │                           • MMR Tree: note_tree_root, mmr_leaf_count, mmr_peak, is_accepted_note_root, get_root_history
│   │                           • Epoch & Admin: get_current_epoch, rotate_epoch, init, propose_owner, claim_ownership, pause, unpause
│   ├── src/deposit.rs        — Logika deposit direct note (verifikasi skalar kanonikal from_evm_scalar < r, min amount 10 USDC, _mmr_insert)
│   ├── src/spend.rs          — Logika eksekusi spend private note & JoinSplit (Groth16 verify, nullifier burning, fee accrual, change MMR append)
│   ├── src/merkle.rs         — Akumulator on-chain Merkle Mountain Range (_mmr_insert, canonical peak bagging, bounded root history ring)
│   ├── src/storage.rs        — Stylus sol_storage! Zeltra layout (MMR state, accepted roots, nullifiers, epoch slots, governance)
│   ├── src/groth16_note_verifier.rs — Verifier Groth16 BLS12-381 untuk PrivateNoteCircuit (16 public inputs) via precompile 0x0c & 0x0f
│   ├── src/groth16_joinsplit_verifier.rs — Verifier Groth16 BLS12-381 untuk JoinSplitCircuit (19 public inputs) via precompile 0x0c & 0x0f
│   ├── src/poseidon_w5_constants.rs — Konstanta round constants & MDS matrix Poseidon width-5 in-contract
│   ├── src/events.rs         — Event NoteCommitmentAppended (memo terenkripsi 96-byte), PrivateNoteSpent, DepositFee, dll.
│   ├── src/verification.rs   — Wrapper pemanggilan host precompile EIP-2537 (call_msm_g1 0x0c, call_pairing 0x0f)
│   ├── src/vault.rs          — Akuntansi yield & collateral solvensi (total_assets, claim_accumulated_yield)
│   └── src/interfaces.rs, types.rs, constants.rs, helpers.rs — Interface ERC-20 / CCIP & konversi tipe EVM
│
├── zeltra-core/              — Primitif Kriptografi & Sirkuit ZK (Arkworks 0.6.0 ark-bls12-381 / ark-groth16)
│   ├── src/lib.rs            — Re-export utama BLS12-381, Poseidon, EVM, fees, MMR, dan sirkuit
│   ├── src/note_circuit.rs   — PrivateNoteCircuit (1-in-1-out Groth16, 16 public inputs, Two-Limb quote hash, formal scope binding DEC-035B)
│   ├── src/joinsplit_circuit.rs — JoinSplitCircuit (Universal 2-in-2-out Groth16, 19 public inputs, constant topology, 448 range gates)
│   ├── src/note.rs           — Primitif note (PrivateNoteV1, NotePlaintext, MerkleMountainRange, MMRProof, viewing keys, AEAD memo)
│   ├── src/poseidon.rs       — Native Poseidon width-3 & width-5, Grain-128 LFSR parameter generator (DEC-021)
│   ├── src/accounting.rs     — State machine ContractAccounting & validasi invariant matematis
│   ├── src/fees.rs           — Kalkulasi fee (0 bps deposit, flat 45 bps spend, 15% relayer markup, rollover rules)
│   ├── src/evm.rs            — Serialisasi & formatting EVM untuk G1, G2, skalar, dan proof BLS12-381
│   ├── src/formal_tests.rs   — Test suite formal verifikasi integritas matematis sirkuit
│   └── src/bin/bench.rs      — Binary benchmark mikro-kriptografi
│
├── zeltra-node/              — Stateless Single-Sequencer Relayer Node (Axum 0.8)
│   ├── src/main.rs           — Boot server, router Axum, orkestrator worker background, rate limiter
│   ├── src/handlers/
│   │   ├── health.rs         — Endpoint /health, /live, /ready dengan observabilitas solvensi makro
│   │   ├── quote.rs          — Endpoint penawaran fee eksekusi tertandatangani EIP-712 (/api/v1/quote, /api/quote/private-spend)
│   │   ├── spend.rs          — Ingress spend ZK (/api/v1/spend-private-note, /api/v1/spend-joinsplit)
│   │   ├── mmr.rs            — Dynamic MMR sync (/api/v1/mmr/proof/:leaf_index, /api/v1/mmr/leaves, /api/v1/mmr/tip, /api/v1/notes/sync)
│   │   ├── tx_status.rs      — Query status transaksi (/api/tx-status)
│   │   └── x402.rs           — Gateway HTTP 402 Facilitator (/api/x402/verify)
│   ├── src/database.rs       — SQLite + SQLCipher AES-256 database, WAL mode, queue spend, tabel MMR & idempotency
│   ├── src/evm_client.rs     — Client Alloy untuk broadcast transaksi L2, konfirmasi receipt, nonce lock, dan +15% gas bump
│   ├── src/mmr_indexer.rs    — Background worker mengindeks event on-chain NoteCommitmentAppended & memo
│   ├── src/mempool_watchdog.rs — Background worker pemulihan lease mempool stalled & pengecekan status nullifier on-chain
│   ├── src/sanctions_sync.rs — Background worker sinkronisasi harian daftar sanksi OFAC dari GitHub ke memori
│   ├── src/execution_fee_claimer.rs — Background worker klaim akumulasi execution fee dari kontrak Stylus
│   ├── src/ccip_monitor.rs   — Background worker pemantauan pesan CCIP & handler refund (/api/ccip/refund)
│   ├── src/validation.rs     — Preflight sanitasi (skalar kanonikal < r, nullifier check, verifikasi proof lokal Groth16)
│   └── src/dto.rs, config.rs, state.rs — Data Transfer Objects, konfigurasi (.env.test), shared AppState
│
├── zeltra-sdk/               — Client SDK & Private Note Wallet (WASM + Native)
│   ├── src/lib.rs            — Re-export SDK, memory security (secure_zeroize, Zeroize)
│   ├── src/wallet/note_wallet.rs — PrivateNoteWallet: prepare_direct_deposit, Knapsack coin selection, Groth16 prover, 2PC store
│   ├── src/coin_selection.rs — Algoritma Stochastic Knapsack Coin Selection (1-note & 2-note JoinSplit)
│   ├── src/storage.rs        — Store note 2PC crash-safe terenkripsi Argon2id + ChaCha20Poly1305
│   ├── src/client.rs         — HTTP client komunikasi ke relayer node
│   ├── src/eip712.rs         — Penandatangan EIP-712 typed data untuk quote eksekusi
│   ├── src/zk_wasm.rs, fee_wasm.rs — Binding WASM untuk eksekusi sirkuit & fee di browser klien
│   ├── src/wasm_types.rs     — DTO serializable untuk interop JS/TS
│   ├── src/x402/             — Modul client protokol pembayaran x402 (codec.rs, types.rs)
│   └── src/bin/bench_e2e.rs  — Binary benchmark performa end-to-end deposit & spend client
│
├── zeltra-cli/               — Pure ZK CLI Utility
│   ├── src/main.rs, cli.rs   — Parser Clap: GenerateWallet, CreateDepositNote, InspectMmr, VerifyProof
│   └── src/commands/zk.rs    — Implementasi perintah CLI pembuatan wallet, deposit note, inspeksi MMR, dan verifikasi proof
│
├── scripts/                  — Script Otomasi, Deployment & Pengujian
│   ├── scripts/start_relayer.sh  — Daemon runner relayer sequencer tunggal (start|status|stop|restart)
│   ├── scripts/deploy_testnet.sh — Deployment smart contract Stylus ke Arbitrum Sepolia
│   └── scripts/check_balance.py, ht01_init_verification.py, ht04_spend_negative_tests.py, ht05_refund_tests.py, ht10_signed_quote_tests.py
│
├── research/decisions/       — 50 Dokumen Arsitektur & Keputusan Desain Resmi (DEC-001 s/d DEC-038C)
├── todo.md                   — Master Active Backlog (murni task pending [ ])
└── docs/                     — Dokumentasi Analisis & Panduan Modular (bisnis.md, analisis/, contract/, core/, relayer/, sdk/, cli/)
```

---

## 5. Aturan Kerja Agen (Operating Principles)

1. **Wajib Graphify-First Sebagai Refleks Utama (Mandatory):**
   * **Struktur Makro & Relasi Sistem (Graphify):** Sebelum melakukan grep atau membaca file mentah, SELALU jalankan perintah graphify:
     - `graphify query "<pertanyaan>"`: Mengambil subgraph terfokus dari pertanyaan arsitektur/aliran data.
     - `graphify explain "<Simbol / Node>"`: Mengurai satu node, posisinya di komunitas, serta relasi langsungnya.
     - `graphify path "<A>" "<B>"`: Melacak rantai ketergantungan antar dua komponen/fungsi.
     - *Dilarang keras* membaca file mentah `graphify-out/graph.json` atau seluruh `GRAPH_REPORT.md` ke context window.
   * **Verbatim Code & Call Path (CodeGraph):** Panggil MCP `codegraph_explore` (atau shell `codegraph explore "<query>"`) untuk melihat isi source file lengkap dengan nomor baris dan hierarchy pemanggilan setelah topologinya dipetakan oleh Graphify.
   * **Sinkronisasi Graf:** Setelah selesai mengedit file `.rs`, jalankan `graphify update .` agar graf selalu akurat.
2. **Cek Source Code Riil Terlebih Dahulu:** Jangan berasumsi atau hanya membaca ringkasan. Buka dan baca kode Rust aslinya sebelum mengubah atau mendokumentasikan.
3. **Hindari Birokrasi Berlebihan:** Jangan membuat puluhan file markdown baru atau checklist ribet yang mendistraksi developer. Jaga dokumentasi tetap terpusat pada bab modular yang sudah ada.
4. **Prinsip ATMI (Amati, Tiru, Modifikasi, Inovasi):** Jangan menemukan roda baru dari nol jika sudah ada paper kriptografi teruji atau repo battle-tested (Zcash, Barretenberg, Railgun, zk-kit, zk-sunade, Wasabi). Amati polanya, tiru fondasinya, modifikasi sesuai arsitektur Stylus/BLS12-381/ZK-UTXO Zeltra, dan ciptakan inovasi baru yang membuat produk unggul.
5. **Zero Breaking Change untuk Public API:** Semua fungsi yang diexport di `lib.rs` harus tetap kompatibel dengan crate lain di workspace.
6. **Keamanan Kriptografi & Finansial:**
   * Jangan pernah membypass verifikasi kriptografi di production path.
   * `fail-closed` jika rahasia/kunci tidak tersedia (dilarang fallback ke mock key di mode non-test).
   * Uji selalu kondisi negatif (tes gagal harus membuktikan state on-chain/DB tidak berubah).

---

## 6. Tooling & MCP Capabilities

* **CodeGraph MCP (`codegraph_explore`):** Tool utama penelusuran kode sumber nyata, dynamic-dispatch hops, dan verbatim symbol inspection.
* **Context-Mode MCP (`ctx_execute`, `ctx_search`, `ctx_batch_execute`):** Sandbox-first data processing. Menjalankan filter/analisis kode/log besar (>200 lines) di sandbox tanpa membanjiri konteks token.
* **RTK (Rust Tooling Kit / Tokless CLI):** Kompresi dan pemangkasan output CLI (cargo, compiler, command traces) untuk efisiensi token dan feedback loop cepat.
* **Graphify Knowledge Graph & MCP (`graphify-out/`):** Graph navigasi arsitektur, god nodes, dependencies cluster, dan shortest-path tracing antar komponen (`graphify query`, `graphify path`, `graphify explain`, atau via MCP server `graphify`: `query_graph`, `shortest_path`, `god_nodes`, `get_node`). Auto-update via `graphify update .`.
* **Web Search & Research MCPs (Exa & Tavily Synergy):**
  - **Exa MCP (`web_search_exa`, `web_fetch_exa`):** *Semantic & Deep Technical Search*. Paling bagus saat butuh pemahaman mendalam yang tidak bisa ditemukan lewat keyword biasa: paper kriptografi (2024–2026), audit post-mortem, arsitektur deep tech, dan eksplorasi kode implementasi/crate eksternal. Format query ideal berupa deskripsi halaman ideal, bukan sekadar kata kunci.
  - **Tavily MCP (`tavily_search`, `tavily_extract`, `tavily_crawl`):** *Real-Time Fact, News & Ecosystem Intel*. Paling bagus untuk fact-checking instan, berita ekosistem live, proposal ArbitrumDAO/AIP, status rilis protokol, dan ekstraksi konten halaman spesifik.
  - **Best Practice Integrasi:** Gunakan Tavily untuk membaca gambaran lanskap/status rilis terkini secara cepat, lalu gunakan Exa untuk deep-dive ke detail teknis atau spesifikasi formalnya. Jika kedua endpoint limit, gunakan `parallel-search` (`web_search`, `web_fetch`) sebagai fallback.
* **Browser Agent:** Tersedia `browser_subagent` jika butuh navigasi interaktif atau visual inspection.
* **Environment:** Codespace Linux dengan Git, Curl, dan tool eksekusi shell. Selalu verifikasi sintaks dan tipe secara ketat pada crate `zeltra-core` dan `zeltra-contracts`.
