# GEMINI.md — Nimbus Protocol Context & Working Guide

Dokumen ini adalah memori konteks persisten untuk agen AI saat bekerja di repository **Zeltra Protocol (Nimbus)**. Jika terjadi kompresi token context window atau sesi baru dimulai, jadikan dokumen ini sebagai kompas utama.

---

## 1. Identitas & Misi Proyek

* **Nama:** Nimbus Protocol (Zeltra Protocol)
* **Tujuan:** Shielded private payment system di **Arbitrum Stylus (Rust WASM)** dengan jaminan matematis: *Satu deposit membayar tepat satu kali, tidak ada sisa uang yang hilang, dan tidak ada aset tanpa backing.*
* **Stack Utama:** Rust (Stylus SDK 0.10.7, Alloy 2.0, Arkworks 0.6.0 `ark-bls12-381` & `ark-groth16`), Axum 0.8, SQLite dengan SQLCipher AES-256 (`bundled-sqlcipher`), OpenBao/Vault KMS.

---

## 2. Status Terkini & Keputusan Arsitektur

### A. Yang Sudah Selesai & Teruji di Arbitrum Sepolia (Phase 1 Baseline)
1. **Smart Contract Stylus (`nimbus-contracts`):** Verifikasi pairing BLS12-381 EIP-2537 (`0x0f`), 13/13 testnet negative tests pass. Rekonstruksi pesan on-chain `NIMBUS_SPEND_V1`.
2. **Threshold Cluster (`nimbus-node`):** 1 Leader + 4 Guardian (3-of-5 threshold) terhubung via Tailscale private mesh dengan pelepasan atomik masking key $k$.
3. **Settlement & Batching:** Persistent queue SQLite dengan leasing worker, retry exponential backoff, dan entrypoint `batch_spend()` (2-8 item) terdeploy on-chain.
4. **CCIP & Finality:** Tracking pesan CCIP dan refund timelock 24 jam.

### B. Arsitektur ZK-UTXO & Gate D (Private Note Balance)
* **Jalur B (ZK-UTXO Model):** Sesuai `DEC-016`, `DEC-016A`, dan `DEC-016B`.
* **Gate C0 Security Repair Selesai:**
  - Bit Merkle path terikat ke `input_leaf_index` (20 bits decomposition gadget).
  - Boolean constraint pada `has_change` dan zero-change integrity.
  - 64-bit integer range constraints mencegah overflow / modular wrap-around di $\mathbb{F}_r$.
* **Gate D Merkle Tree Terintegrasi:** State LeanIMT Merkle tree tingkat 20 terdeploy on-chain di smart contract Stylus, dengan history bounded accepted-root.

### C. Relayer Hardening (DEC-017 & DEC-018)
* **DEC-017 Receipt Finality & Nonce Lock:** Thread-safe atomic nonce manager, mempool watchdog, dan automatic +15% gas-bump transaction replacement.
* **DEC-018 On-Chain Deposit Indexer & Cryptographic Reveal:**
  - Event listener on-chain `DepositFee` dengan `com_k_hash` terindeks.
  - Reorg-safe indexing window dengan ambang batas konfirmasi finalitas dan checkpoint persistent di tabel `indexer_state`.
  - Verifikasi kriptografis kurva eliptis $k \cdot \text{pk}_{\text{iss}} == \text{com}_k$ enforced *fail-closed* sebelum rilis kunci $k$.

---

## 3. Prioritas Aktif Selanjutnya

Mengacu pada master checklist [`todo.md`](file:///workspaces/Zeltra-Protocol/todo.md):

1. **Input & Safety Validation (`nimbus-node`):**
   * Validasi recipient address EVM valid, batas min/max amount, dan chain selector allowlist.
   * Rekonsiliasi status database dengan nullifier contract setelah node restart.
2. **Batch & Fee Metrics (`nimbus-node`):**
   * Tambahkan batch profitability metrics ke health endpoint & DB (`batch_count`, `total_batch_margin_usdc`, `avg_batch_size`).
3. **Cross-Chain CCIP Testing & Verification (Phase 4):**
   * Testnet E2E Hard-Test di Arbitrum Sepolia (source broadcast $\rightarrow$ router $\rightarrow$ destination execution $\rightarrow$ destination confirmation).
   * Uji jalur kegagalan destination failure (refund 24h & double payout rejection).


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

1. **Gunakan CodeGraph MCP Terlebih Dahulu (Mandatory):** Repository ini diindeks dengan CodeGraph (`.codegraph/`). Panggil MCP `codegraph_explore` (atau shell `codegraph explore "<query>"`) **SEBELUM** grep, find, atau manual read file. CodeGraph mengembalikan verbatim source ber-line number, call paths, dan blast radius lengkap dalam 1 kali round-trip hemat token.
2. **Cek Source Code Riil Terlebih Dahulu:** Jangan berasumsi atau hanya membaca ringkasan. Buka dan baca kode Rust aslinya sebelum mengubah atau mendokumentasikan.
3. **Hindari Birokrasi Berlebihan:** Jangan membuat puluhan file markdown baru atau checklist ribet yang mendistraksi developer. Jaga dokumentasi tetap terpusat pada bab modular yang sudah ada.
4. **Zero Breaking Change untuk Public API:** Semua fungsi yang diexport di `lib.rs` harus tetap kompatibel dengan crate lain di workspace.
5. **Keamanan Kriptografi & Finansial:**
   * Jangan pernah membypass verifikasi kriptografi di production path.
   * `fail-closed` jika rahasia/kunci tidak tersedia (dilarang fallback ke mock key di mode non-test).
   * Uji selalu kondisi negatif (tes gagal harus membuktikan state on-chain/DB tidak berubah).

---

## 6. Tooling & MCP Capabilities

* **CodeGraph MCP (`codegraph_explore`) [MANDATORY FIRST STEP]:** Tool utama pencarian dan pemetaan kode. Panggil untuk memahami flow antar fungsi, caller hierarchy, dan blast radius sebelum melakukan edit.
* **Web Search & Fetching:** Tersedia MCP `parallel-search` (`web_search`, `web_fetch`) dan native `search_web`/`read_url_content` untuk mencari paper ZK, auditing report, atau referensi kriptografi terkini.
* **Browser Agent:** Tersedia `browser_subagent` jika butuh navigasi interaktif atau visual inspection.
* **Environment:** Codespace Linux dengan Git, Curl, dan tool eksekusi shell. Selalu verifikasi sintaks dan tipe secara ketat pada crate `nimbus-core` dan `nimbus-contracts`.
