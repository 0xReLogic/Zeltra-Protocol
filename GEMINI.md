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

### A. Yang Sudah Selesai & Teruji di Arbitrum Sepolia (Phase 1 Baseline)
1. **Smart Contract Stylus (`nimbus-contracts`):** Verifikasi pairing BLS12-381 EIP-2537 (`0x0f`), 13/13 testnet negative tests pass. Rekonstruksi pesan on-chain `NIMBUS_SPEND_V1`.
2. **Threshold Cluster (`nimbus-node`):** 1 Leader + 4 Guardian (3-of-5 threshold) terhubung via Tailscale private mesh dengan pelepasan atomik masking key $k$.
3. **Settlement & Batching:** Persistent queue SQLite dengan leasing worker, retry exponential backoff, dan entrypoint `batch_spend()` (2-8 item) terdeploy on-chain.
4. **CCIP & Finality:** Tracking pesan CCIP dan refund timelock 24 jam.

### B. Arsitektur ZK-UTXO & Gates C0, D, E, F
* **Jalur B (ZK-UTXO Model):** Sesuai `DEC-016`, `DEC-016A`, dan `DEC-016B`.
* **Gate C0 Security Repair Selesai:**
  - Bit Merkle path terikat ke `input_leaf_index` (20 bits decomposition gadget).
  - Boolean constraint pada `has_change` dan zero-change integrity.
  - 64-bit integer range constraints mencegah overflow / modular wrap-around di $\mathbb{F}_r$.
  - Poseidon Grain-128 LFSR parameter generator (`DEC-021`, 108/108 tests pass).
* **Gate D Merkle Tree & Verifier Terintegrasi (Contracts):**
  - Groth16 BLS12-381 verifier EIP-2537 (`0x0c` MSM + `0x0f` Pairing Check) terdeploy di Stylus WASM.
  - State LeanIMT Merkle tree tingkat 20 (`_merkle_insert`) terdeploy on-chain dengan bounded accepted-root history.
  - Entrypoint `spend_private_note(...)` teruji 53/53 tests pass.
* **Gate E SDK Private Note Wallet Selesai:**
  - `PrivateNoteWallet` di `nimbus-sdk` menggantikan voucher exact-amount dengan saldo gabungan Note UTXO.
  - Local proof generator Groth16, crash-safe 2PC note store (DEC-024), HMAC-SHA256 backup, dan stochastic coin selection (20/20 tests pass).
* **Gate F Relayer Settlement Pipeline Selesai (Phase 1):**
  - Endpoint `POST /api/v1/spend-private-note` dengan pre-flight fail-closed check (`from_evm_scalar < r`), semantic input binding (`DEC-022`), dual-layer nullifier double-spend guard (SQLite & on-chain `is_nullifier_spent`), dan single-item direct settlement dispatcher `broadcast_spend_private_note_transaction` (`DEC-025`, 91/91 tests pass).

### C. Relayer Hardening (DEC-017 s/d DEC-025)
* **DEC-017 Receipt Finality & Nonce Lock:** Thread-safe atomic nonce manager, mempool watchdog, dan automatic +15% gas-bump transaction replacement.
* **DEC-018 On-Chain Deposit Indexer & Cryptographic Reveal:**
  - Event listener on-chain `DepositFee` dengan `com_k_hash` terindeks.
  - Reorg-safe indexing window dengan ambang batas konfirmasi finalitas dan checkpoint persistent di tabel `indexer_state`.
  - Verifikasi kriptografis kurva eliptis $k \cdot \text{pk}_{\text{iss}} == \text{com}_k$ enforced *fail-closed* sebelum rilis kunci $k$.
* **DEC-025 Relayer ZK Note Spend Settlement:** Single-item direct settlement, idempotensi 24h, dan proteksi race-condition status nullifier.
* **DEC-031 Relayer Three-Way Reconciliation & Solvency Observability:** Three-way reconciliation engine (`reconcile_execution_fees`) mencocokkan akrual kontrak on-chain, DB SQLite (`spend_batches`), dan confirmed receipts (`status == 1`); pemusnahan quote fallback tanpa signing domain (fail-closed EIP-712 domain binding); serta observabilitas solvensi makro pada `/health` tanpa membuka privasi user.

---

## 3. Prioritas Aktif Selanjutnya

Mengacu pada master checklist [`todo.md`](file:///workspaces/Zeltra-Protocol/todo.md) & [`docs/todos/private-note-balance.md`](file:///workspaces/Zeltra-Protocol/docs/todos/private-note-balance.md):

1. **Hard-Test E2E Arbitrum Sepolia (Gate G):**
   * Deposit $\rightarrow$ partial spend $\rightarrow$ change note $\rightarrow$ second spend $\rightarrow$ withdraw-all di testnet.
   * Crash recovery & idempotency boundary testing.
3. **Cross-Chain CCIP Testing & Verification (Phase 4):**
   * Testnet E2E Hard-Test di Arbitrum Sepolia (source broadcast $\rightarrow$ router $\rightarrow$ destination execution $\rightarrow$ destination confirmation).
   * Uji jalur kegagalan destination failure (refund 24h & double payout rejection).
4. **MPC Ceremony & Circuit Freeze (Gate C):**
   * Freeze circuit constraints & public ABI sebelum production trusted setup ceremony.


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
* **Graphify Knowledge Graph (`graphify-out/`):** Graph navigasi arsitektur, god nodes, dependencies cluster, dan shortest-path tracing antar komponen (`graphify query`, `graphify path`, `graphify explain`, `graphify update .`).
* **Web Search & Research MCPs (Exa & Tavily Synergy):**
  - **Exa MCP (`web_search_exa`, `web_fetch_exa`):** *Semantic & Deep Technical Search*. Paling bagus saat butuh pemahaman mendalam yang tidak bisa ditemukan lewat keyword biasa: paper kriptografi (2024–2026), audit post-mortem, arsitektur deep tech, dan eksplorasi kode implementasi/crate eksternal. Format query ideal berupa deskripsi halaman ideal, bukan sekadar kata kunci.
  - **Tavily MCP (`tavily_search`, `tavily_extract`, `tavily_crawl`):** *Real-Time Fact, News & Ecosystem Intel*. Paling bagus untuk fact-checking instan, berita ekosistem live, proposal ArbitrumDAO/AIP, status rilis protokol, dan ekstraksi konten halaman spesifik.
  - **Best Practice Integrasi:** Gunakan Tavily untuk membaca gambaran lanskap/status rilis terkini secara cepat, lalu gunakan Exa untuk deep-dive ke detail teknis atau spesifikasi formalnya. Jika kedua endpoint limit, gunakan `parallel-search` (`web_search`, `web_fetch`) sebagai fallback.
* **Browser Agent:** Tersedia `browser_subagent` jika butuh navigasi interaktif atau visual inspection.
* **Environment:** Codespace Linux dengan Git, Curl, dan tool eksekusi shell. Selalu verifikasi sintaks dan tipe secara ketat pada crate `nimbus-core` dan `nimbus-contracts`.
