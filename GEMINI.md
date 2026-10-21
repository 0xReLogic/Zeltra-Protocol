# GEMINI.md — Zeltra Protocol Context & Working Guide

Dokumen ini adalah memori konteks persisten untuk agen AI saat bekerja di repository **Zeltra Protocol (Zeltra)**. Jika terjadi kompresi token context window atau sesi baru dimulai, jadikan dokumen ini sebagai kompas utama.

---

## 1. Identitas & Misi Proyek

* **Nama:** **Zeltra Protocol** *(codebase transisi: Zeltra)*.
* **Manifesto Lengkap:** Baca [`VISION.md`](file:///workspaces/Zeltra-Protocol/VISION.md).
* **Visi & Posisi:** **"The Stripe of Web3 with Absolute Privacy."** Rel pembayaran dan settlement universal untuk AI Agent, retail, whale, korporat B2B, dan merchant tanpa diskriminasi.
* **Dual Sovereign Rails:** USDC (Micro-USDC, 6 desimal) dan WETH / Native ETH (Gwei, 9 desimal ter-normalisasi) dengan payable auto-wrapping di kontrak.
* **Mantra Produk:** **Privasi 9.5/10, Produk 10/10.** User experience secepat dan seringan Apple Pay / QRIS dengan mental model kas nyata ($10 bayar $3 kembali $7 lewat ZK-UTXO Private Note, atau transfer langsung antar ZSA tanpa token publik keluar). Beban komputasi kriptografi berat diselesaikan di latar belakang.
* **Filosofi Finansial:**
  * *Zero-Friction Inflow:* Deposit 0 bps (0.00%) permanen untuk USDC & WETH/ETH.
  * *Volume Over TVL & Zero In-Pool Fee:* In-pool shielded transfer **0 bps (0.00%)**, public outflow spend **flat 45 bps (0.45%)**.
  * *Mathematical Solvency:* Invariant mutlak `Balance(asset) >= total_liabilities(asset)` (siloed per-asset solvency, anti-fractional reserve, anti-cross contamination).
  * *Etika Kepatuhan:* *"Permissionless Privacy, Permissioned Acceptance"* via Receiver-Enforced Modular ASP ([`DEC-026`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-026-receiver-enforced-compliance-zero-cost-sanctions-filtering-relayer-protection.md)), OFAC local screening, dan CEX Selective Disclosure ([`DEC-040`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-040-viewing-key-export-suite-and-cex-selective-disclosure.md)).
* **Stack Utama:** Rust (Stylus SDK 0.10.7, Alloy 2.0, Arkworks 0.6.0 `ark-bls12-381` & `ark-groth16`), Axum 0.8, SQLite dengan SQLCipher AES-256 (`bundled-sqlcipher`), OpenBao/Vault KMS.

---

## 2. Status Terkini & Keputusan Arsitektur

### A. Arsitektur Murni ZK-UTXO & Dual-Asset (DEC-034 & DEC-038)
1. **Pemusnahan Total Legacy Threshold BLS (DEC-038A):** Seluruh kode blind signing, guardian network, session storage (11 storage slot kontrak), dan alur refund timelock 24 jam telah dihapus bersih. Arsitektur 100% murni ZK-UTXO.
2. **Direct MMR Deposit (DEC-038B / DEC-034B):** Deposit langsung via `deposit_direct_note` (USDC) atau `deposit_direct_native_eth` (ETH payable auto-wrap) menyisipkan komitmen note ke Merkle Mountain Range (`_mmr_insert`), memancarkan event `NoteCommitmentAppended` berisi 96-byte AEAD memo, dan seketika berstatus unspent tanpa perantara kunci $k$.
3. **Stateless Sequencer Relayer (DEC-038C):** Single stateless sequencer node (`scripts/start_relayer.sh`), murni melayani ingress ZK spend, preflight verification, Zero-Oracle gas reimbursement (Gwei untuk WETH, Micro-USDC untuk USDC), dan dynamic MMR sync.

### B. Pondasi Kriptografi, Sirkuit ZK & Ekosistem Lengkap (DEC-034 s/d DEC-047)
* **Sirkuit Groth16 (zeltra-core):**
  - **1-in-1-out (`PrivateNoteCircuit`):** 16 Public Inputs (DEC-035A), 128-bit Two-Limb quote hash, Formal Scope Binding Gadget (DEC-035B), Poseidon Grain-128 LFSR (DEC-021), 64-bit integer range constraints, dan evolusi nullifier 2-Epoch (DEC-033).
  - **Universal 2-in-2-out (`JoinSplitCircuit`):** 19 Public Inputs (DEC-036A), constant-arity topology, canonical dummy input/output slots, multi-asset bit-packing (`flags_packed`), 448 quadratic range gates, dan penguncian gerbang pendamping kuadratik deterministik anti-null branch (DEC-046).
  - **CEX Selective Disclosure (`SelectiveDisclosureCircuit` - DEC-040A):** Sirkuit kepatuhan Groth16 dengan Verifier Binding (mengikat alamat bursa CEX & challenge nonce) untuk membuktikan asal dana note secara selektif ke Binance/Coinbase tanpa mengekspos kunci belanja ($sk$).
  - **Automated Jacobian Diagnostics (DEC-047):** Mesin verifikasi aljabar diferensial berbasis sparse augmented Jacobian $M(a) = \begin{pmatrix} J(a) \\ E_F \end{pmatrix}$, uji ruang-baris kekakuan target $e_j^T \in \text{rowspan}(M(a))$, sintesis sertifikat garis afin eksak non-uniqueness (Proposisi 27), dan sapuan saksi adversarial multi-cabang (IACR 2026/1852).
* **Akumulator MMR & Akselerasi Proof (DEC-032 & DEC-045):** In-memory dan on-chain Merkle Mountain Range. Flat Post-Order Array (`FlatMerkleMountainRange`) dengan formula Peter Todd, $O(\log N)$ zero-hash proof (<20 µs), concurrent thread-safe accumulator di `zeltra-node`, dan 1-byte HKDF View Tag memo di `zeltra-sdk` (23.6x scanning acceleration).
* **Smart Contract Stylus (zeltra-contracts):**
  - Verifikasi Groth16 BLS12-381 via EIP-2537 (`0x0c` MSM + `0x0f` Pairing Check) di WASM Stylus.
  - **Strict 3-Mode State Machine (DEC-043B):**
    * Mode 1: Mint / Consolidation / Rollover.
    * Mode 2: In-Pool Transfer & Merchant Masking (`recipient == 0x0`, `protocol_fee == 0`, dual 96B memos disisipkan ke MMR on-chain).
    * Mode 3: Public Spend (`recipient != 0x0`, protocol fee flat 45 bps).
  - Invariant solvensi terisolasi multi-aset (`_check_asset_solvency_invariant`).
* **Client SDK & Wallet State (zeltra-sdk):**
  - `PrivateNoteWallet`: Tiered Stochastic Knapsack Coin Selection multi-aset, in-pool shielded transfer (`prepare_shielded_transfer_proof`), View Tag fast-scanner (`recover_notes_batch`), alamat ZSA Bech32m (`zs1...` - DEC-043A), dan 2PC crash-safe storage.
  - **B2B Double-Entry Compliance & CEX Verifier Engine (DEC-040B/C):** Modul `compliance/` untuk ekspor laporan audit pajak dan verifikasi independen oleh bursa.
  - **In-Browser Web Worker Prover (DEC-041):** Runner prover Groth16 langsung di browser via Web Worker dengan isolasi memori sekali pakai (`worker.terminate()`) anti-OOM linear.
  - **Pure ZK-UTXO HTTP 402 Facilitator (DEC-042):** Mikropembayaran AI Agent sub-sen murni ZK-UTXO.

---

## 3. Prioritas Aktif Selanjutnya (Fleksibilitas R&D & Jeda Deploy Arbitrum)

Seluruh task di **Fase 0 (Dual-Asset)** dan **Fase 1 (SDK, Browser Prover & Ekosistem Fitur)** telah **100% SELESAI, DIAUDIT & TERVERIFIKASI**.

> [!NOTE]
> **PENTING UNTUK AGENT (Sirkuit BELUM Dikunci):**  
> Status deployment Arbitrum sedang mengalami jeda/freeze (Oktober 2026), sehingga Zeltra memiliki banyak waktu luang dan fleksibilitas (*flexible runway*). **Sirkuit ZK TIDAK/BELUM DIKUNCI (Circuit is UNLOCKED & FLEXIBLE)**. Agent tidak perlu bersikap kaku atau paranoid soal circuit freeze. Parameter sirkuit, public input, dan R1CS gadgets masih bebas dikembangkan dan dioptimasi (misal: SBM Groth16 streaming di SDK, MinMandate untuk AI Agent, iSimplePIR di Relayer, penguatan aljabar DEC-046, dan audit Jacobian DEC-047).

Mengacu pada master checklist [`todo.md`](file:///workspaces/Zeltra-Protocol/todo.md) dan [`docs/roadmap.md`](file:///workspaces/Zeltra-Protocol/docs/roadmap.md), prioritas aktif saat ini adalah:

1. **Fase 2 — Hardening Kriptografi, Eksplorasi Fitur & Persiapan MPC:**
   * Hardening aljabar sirkuit (DEC-046: penguncian cabang dummy slot & anti-aliasing inverse zero-safe) tanpa terburu-buru mengunci R1CS.
   * Diagnostik otomatis & audit kekakuan tangen sirkuit via rank matriks Jacobian (DEC-047, `formal_tests.rs`, IACR 2026/1852).
   * Eksplorasi inovasi riset unggul (SBM Streaming Prover $O(\sqrt{N})$, iSimplePIR, MinMandate).
   * Persiapan harness upacara MPC Phase 2 Trusted Setup (dijalankan nanti saat siap mainnet).
   * Review keamanan sirkuit independen & smart contract Stylus.
2. **Fase 3 — Keamanan & Sanitasi Kredensial:**
   * Rotasi seluruh secret/private key dev historis dan pembersihan git commit history.
3. **Fase 4 — Unified Hard-Test Arbitrum Sepolia (Gladi Bersih Akhir — Deploy Sekali):**
   * Deploy kontrak Stylus dan relayer node **sekali saja** menggunakan verifying keys resmi hasil MPC setelah Tahap 2 & 3 tuntas.
   * Full testnet lifecycle matrix (deposit direct $\rightarrow$ in-pool transfer $\rightarrow$ change note $\rightarrow$ second spend $\rightarrow$ withdraw-all).
   * Cross-Chain CCIP testnet flow & boundary crash recovery testing.
4. **Fase 5 — Mainnet Arbitrum One Launch & Adopsi Pasar:**
   * Deploy production, bug bounty, dan integrasi merchant checkout widget.

---

## 4. Peta Struktur Repository & Pemetaan Kode Riil

```text
/workspaces/Zeltra-Protocol/
├── zeltra-contracts/         — Smart Contract Arbitrum Stylus WASM (EIP-2537 BLS12-381 Precompiles)
│   ├── src/lib.rs            — Entrypoint #[entrypoint] contract Zeltra:
│   │                           • Inflow: deposit_direct_note, deposit_direct_weth, deposit_direct_native_eth
│   │                           • Outflow/Transfer: spend_joinsplit (Strict 3-Mode), spend_private_note
│   │                           • Solvensi: check_asset_solvency_invariant, check_liability_invariant
│   │                           • Nullifier: is_nullifier_spent, is_note_nullifier_spent, is_generational_nullifier_spent
│   │                           • MMR Tree: note_tree_root, mmr_leaf_count, mmr_peak, is_accepted_note_root, get_root_history
│   │                           • Epoch & Admin: get_current_epoch, rotate_epoch, init, init_multi_asset, pause, unpause
│   ├── src/deposit.rs        — Logika deposit direct note ERC20 & native ETH auto-wrap (_mmr_insert)
│   ├── src/spend.rs          — Logika eksekusi Strict 3-Mode JoinSplit & private spend (Groth16 verify, nullifier burning, dual 96B memo append)
│   ├── src/merkle.rs         — Akumulator on-chain Merkle Mountain Range (_mmr_insert, canonical peak bagging, bounded root history ring)
│   ├── src/storage.rs        — Stylus sol_storage! Zeltra layout (MMR state, accepted roots, nullifiers, epoch slots, multi-asset storage)
│   ├── src/groth16_note_verifier.rs — Verifier Groth16 BLS12-381 untuk PrivateNoteCircuit (16 public inputs) via precompile 0x0c & 0x0f
│   ├── src/groth16_joinsplit_verifier.rs — Verifier Groth16 BLS12-381 untuk JoinSplitCircuit (19 public inputs) via precompile 0x0c & 0x0f
│   ├── src/poseidon_w5_constants.rs — Konstanta round constants & MDS matrix Poseidon width-5 in-contract
│   ├── src/events.rs         — Event NoteCommitmentAppended (memo terenkripsi 96-byte), JoinSplitSpendExecuted, DepositFee, dll.
│   ├── src/verification.rs   — Wrapper pemanggilan host precompile EIP-2537 (call_msm_g1 0x0c, call_pairing 0x0f)
│   ├── src/vault.rs          — Akuntansi solvensi multi-aset terisolasi (_check_asset_solvency_invariant, total_assets)
│   └── src/interfaces.rs, types.rs, constants.rs, helpers.rs — Interface ERC-20 / CCIP & konversi tipe EVM
│
├── zeltra-core/              — Primitif Kriptografi & Sirkuit ZK (Arkworks 0.6.0 ark-bls12-381 / ark-groth16)
│   ├── src/lib.rs            — Re-export utama BLS12-381, Poseidon, EVM, fees, MMR, dan sirkuit
│   ├── src/note_circuit.rs   — PrivateNoteCircuit (1-in-1-out Groth16, 16 public inputs, Two-Limb quote hash, formal scope binding DEC-035B)
│   ├── src/joinsplit_circuit.rs — JoinSplitCircuit (Universal 2-in-2-out Groth16, 19 public inputs, constant topology, 448 range gates)
│   ├── src/selective_disclosure_circuit.rs — SelectiveDisclosureCircuit (CEX compliance Groth16 dengan Verifier Binding, DEC-040A)
│   ├── src/circuit_gadgets.rs — R1CS Gadgets (flags_packed multi-asset, range checks, Poseidon gadgets)
│   ├── src/note.rs           — Primitif note (PrivateNoteV1 dengan asset_id, NotePlaintext, ZSA key derivation, deterministic ECDH, ChaCha20Poly1305)
│   ├── src/poseidon.rs       — Native Poseidon width-3 & width-5, Grain-128 LFSR parameter generator (DEC-021)
│   ├── src/accounting.rs     — State machine ContractAccounting & validasi invariant matematis
│   ├── src/fees.rs           — Kalkulasi fee (0 bps deposit, 0 bps in-pool transfer, 45 bps spend, 15% relayer markup)
│   ├── src/evm.rs            — Serialisasi & formatting EVM untuk G1, G2, skalar, dan proof BLS12-381
│   ├── src/formal_tests.rs   — Test suite formal verifikasi integritas matematis sirkuit
│   └── src/bin/bench.rs      — Binary benchmark mikro-kriptografi
│
├── zeltra-node/              — Stateless Single-Sequencer Relayer Node (Axum 0.8)
│   ├── src/main.rs           — Boot server, router Axum, orkestrator worker background, rate limiter
│   ├── src/bin/inpool_interop.rs — Test harness CLI untuk pengujian independen lintas bahasa (Python suite)
│   ├── src/handlers/
│   │   ├── health.rs         — Endpoint /health, /live, /ready dengan observabilitas solvensi makro
│   │   ├── quote.rs          — Endpoint fee eksekusi tertandatangani EIP-712 (/api/v1/quote) dengan zero-oracle reimbursement
│   │   ├── spend.rs          — Strict 3-Mode spend ingress (/api/v1/spend-joinsplit, /api/v1/spend-private-note) dengan validasi memo 192 hex
│   │   ├── mmr.rs            — Dynamic MMR sync (/api/v1/mmr/proof/:leaf_index, /api/v1/mmr/leaves, /api/v1/mmr/tip, /api/v1/notes/sync)
│   │   ├── tx_status.rs      — Query status transaksi (/api/tx-status)
│   │   └── x402.rs           — Pure ZK-UTXO HTTP 402 Facilitator gateway (/api/x402/verify)
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
│   ├── src/wallet/note_wallet.rs — PrivateNoteWallet: prepare_shielded_transfer_proof, Knapsack multi-asset, Groth16 prover, recover_notes_batch, 2PC store
│   ├── src/compliance/       — Modul audit B2B & verifikasi bursa CEX (exporter.rs, verifier.rs, models.rs)
│   ├── src/coin_selection.rs — Algoritma Stochastic Knapsack Coin Selection multi-aset
│   ├── src/storage.rs        — Store note 2PC crash-safe terenkripsi Argon2id + ChaCha20Poly1305
│   ├── src/client.rs         — HTTP client komunikasi ke relayer node
│   ├── src/eip712.rs         — Penandatangan EIP-712 typed data untuk quote eksekusi
│   ├── src/zk_wasm.rs, fee_wasm.rs — In-browser WASM Groth16 prover runner dengan disposable Web Worker memory lifecycle (DEC-041)
│   ├── src/wasm_types.rs     — DTO serializable untuk interop JS/TS
│   ├── src/x402/             — Pure ZK-UTXO x402 payment client codec & types (DEC-042)
│   └── src/bin/bench_e2e.rs  — Binary benchmark performa end-to-end deposit & spend client
│
├── zeltra-cli/               — Pure ZK CLI Utility
│   ├── src/main.rs, cli.rs   — Parser Clap: GenerateWallet, CreateDepositNote, InspectMmr, VerifyProof, Compliance
│   ├── src/commands/zk.rs    — Implementasi perintah CLI pembuatan wallet, deposit note, inspeksi MMR, dan verifikasi proof
│   └── src/commands/compliance.rs — Ekspor audit compliance, verifikasi bukti CEX independen
│
├── scripts/                  — Script Otomasi, Deployment & Pengujian
│   ├── scripts/start_relayer.sh  — Daemon runner relayer sequencer tunggal (start|status|stop|restart)
│   ├── scripts/deploy_testnet.sh — Deployment smart contract Stylus ke Arbitrum Sepolia
│   ├── scripts/test_real_case_dec043c_inpool_wallet.py — Test suite Python independen 44/44 lolos
│   └── scripts/check_balance.py, ht01_init_verification.py, ht04_spend_negative_tests.py, ht05_refund_tests.py, ht10_signed_quote_tests.py
│
├── research/decisions/       — 71 Dokumen Arsitektur & Keputusan Desain Resmi (DEC-001 s/d DEC-047)
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
* **Groq LPU Acceleration (`qwen/qwen3.8-27b`):** Engine akselerasi inferensi ultra-cepat (~200–400+ tok/s) via OpenAI-compatible endpoint Groq (`https://api.groq.com/openai/v1`). Terpasang untuk semantic batch extraction dan token generation berlatensi rendah.
* **Web Search & Research MCPs (Exa & Tavily Synergy):**
  - **Exa MCP (`web_search_exa`, `web_fetch_exa`):** *Semantic & Deep Technical Search*. Paling bagus saat butuh pemahaman mendalam yang tidak bisa ditemukan lewat keyword biasa: paper kriptografi (2024–2026), audit post-mortem, arsitektur deep tech, dan eksplorasi kode implementasi/crate eksternal. Format query ideal berupa deskripsi halaman ideal, bukan sekadar kata kunci.
  - **Tavily MCP (`tavily_search`, `tavily_extract`, `tavily_crawl`):** *Real-Time Fact, News & Ecosystem Intel*. Paling bagus untuk fact-checking instan, berita ekosistem live, proposal ArbitrumDAO/AIP, status rilis protokol, dan ekstraksi konten halaman spesifik.
  - **Best Practice Integrasi:** Gunakan Tavily untuk membaca gambaran lanskap/status rilis terkini secara cepat, lalu gunakan Exa untuk deep-dive ke detail teknis atau spesifikasi formalnya. Jika kedua endpoint limit, gunakan `parallel-search` (`web_search`, `web_fetch`) sebagai fallback.
* **Browser Agent:** Tersedia `browser_subagent` jika butuh navigasi interaktif atau visual inspection.
* **Environment:** Codespace Linux dengan Git, Curl, dan tool eksekusi shell. Selalu verifikasi sintaks dan tipe secara ketat pada crate `zeltra-core` dan `zeltra-contracts`.

