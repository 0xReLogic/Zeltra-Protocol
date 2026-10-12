# GEMINI.md — Nimbus Protocol Context & Working Guide

Dokumen ini adalah memori konteks persisten untuk agen AI saat bekerja di repository **Zeltra Protocol (Nimbus)**. Jika terjadi kompresi token context window atau sesi baru dimulai, jadikan dokumen ini sebagai kompas utama.

---

## 1. Identitas & Misi Proyek

* **Nama:** **Zeltra Protocol** *(codebase transisi: Nimbus)*.
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
* **Sirkuit Groth16 (nimbus-core):**
  - **1-in-1-out (`PrivateNoteCircuit`):** 16 Public Inputs (DEC-035A), 128-bit Two-Limb quote hash, Formal Scope Binding Gadget (DEC-035B), Poseidon Grain-128 LFSR (DEC-021), 64-bit integer range constraints, dan evolusi nullifier 2-Epoch (DEC-033).
  - **Universal 2-in-2-out (`JoinSplitCircuit`):** 19 Public Inputs (DEC-036A), constant-arity topology, canonical dummy input/output slots, dan 448 quadratic range gates.
* **Akumulator MMR (DEC-032):** In-memory dan on-chain Merkle Mountain Range dengan canonical peak bagging backward fold (`DOMAIN_MMR_BAG`) dan mitigasi Hyperbridge out-of-bounds ($leaf\_index < leaf\_count$).
* **Smart Contract Stylus (nimbus-contracts):**
  - Verifikasi Groth16 BLS12-381 via EIP-2537 (`0x0c` MSM + `0x0f` Pairing Check) di WASM Stylus.
  - State MMR terintegrasi, history accepted roots bounded ring, dan entrypoints `deposit_direct_note`, `spend_private_note`, serta `spend_joinsplit`.
  - Invariant solvensi absolut: `USDC Balance >= user_note_liability + accrued_execution_fee_liability + realized_protocol_fees`.
* **Client SDK & Wallet State (nimbus-sdk):**
  - `PrivateNoteWallet`: Tiered Stochastic Knapsack Coin Selection (DEC-030/036B), in-pool `consolidate_notes()`, 2PC crash-safe note store (DEC-024), Argon2id + ChaCha20Poly1305 encrypted storage, dan zeroize memory safety.
  - Viewing Key Recovery Protocol (DEC-037): Derivasi kunci HKDF (`IncomingViewingKey`, `OutcomingViewingKey`), 48-byte `NotePlaintext`, compact 96-byte AEAD memo, dan pemulihan saldo on-chain (`recover_notes`).
* **Relayer Pipeline (nimbus-node):**
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

## 4. Peta Struktur Repository & Dokumentasi Modular

```text
/workspaces/Zeltra-Protocol/
├── nimbus-contracts/  — Stylus WASM smart contract (lib.rs, storage.rs, spend.rs, deposit.rs)
├── nimbus-core/       — Crypto primitives, BDHKE, Merkle tree, & PrivateNoteCircuit
├── nimbus-node/       — Relayer node (Leader, Guardian, queue, DB SQLCipher, Vault KMS)
├── nimbus-sdk/        — WebAssembly client SDK (JS/TS bindings & EIP-712 quote signer)
├── nimbus-cli/        — CLI utility untuk simulasi kriptografi lokal
├── todo.md            — Master active backlog (hanya task pending [ ])
├── archive/           — Arsip dokumen usang, histori tes lama, & original monolithic docs
│
└── docs/              — Dokumentasi modular berbasis bab (Chapter-Based Docs)
    ├── bisnis.md      — Model bisnis & alur ZK-UTXO yang jujur
    ├── cli/           — 3 chapters (01-overview, 02-commands, 03-e2e-workflow)
    ├── contract/      — 4 chapters (01-overview, 02-storage, 03-deposit-refund, 04-spend-batch)
    ├── core/          — 5 chapters (01-primitives, 02-bdhke, 03-note-utxo, 04-circuit, 05-accounting)
    ├── relayer/       — 5 chapters (01-cluster, 02-database, 03-settlement, 04-kms, 05-quotes)
    ├── sdk/           — 3 chapters (01-wasm, 02-integration, 03-agent-wallet)
    └── todos/         — private-note-balance.md (Gate C0 bible) & receipt-finality.md
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
* **Environment:** Codespace Linux dengan Git, Curl, dan tool eksekusi shell. Selalu verifikasi sintaks dan tipe secara ketat pada crate `nimbus-core` dan `nimbus-contracts`.
