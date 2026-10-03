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

### B. Keputusan Desain: Jalur B (ZK-UTXO Private Note Balance)
* Masalah sistem voucher lama (1 deposit = 1 spend, uang kembalian hangus di kontrak) diselesaikan melalui **Jalur B (ZK-UTXO Model)** mengacu pada `DEC-016` & `DEC-016A`.
* Setiap transaksi *partial spend* mengkonsumsi input note dan otomatis men-generate **Change Note baru** ke Merkle tree atas spending key privat user via ZK-SNARK Groth16.
* **Hukum Kekekalan Nilai (Value Conservation):**
  $$\text{Input Note} = \text{Payout} + \text{Protocol Fee} + \text{Execution Fee} + \text{Change Note}$$

---

## 3. Prioritas Aktif Sekarang: Gate C0 Security Repair

Lokasi target: [`nimbus-core/src/note_circuit.rs`](file:///workspaces/Zeltra-Protocol/nimbus-core/src/note_circuit.rs)  
Referensi audit: [`docs/todos/private-note-balance.md`](file:///workspaces/Zeltra-Protocol/docs/todos/private-note-balance.md) & [`DEC-016B`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-016B-mvp-circuit-shortcuts.md)

Audit menemukan 3 blocker kritis pada sirkuit Groth16 prototype yang **wajib diperbaiki sebelum Gate D (integrasi contract)**:

1. **[P0] Ikat Bit Merkle Path ke `input_leaf_index` di Circuit:**
   * Dekomposisi `leaf_index` menjadi tepat 20 boolean bits di circuit.
   * Enforce $\text{leaf\_index} == \sum_{i=0}^{19} \text{bit}_i \cdot 2^i$ dan $< 2^{20}$.
   * Gunakan bit-bit tersebut untuk menentukan arah hashing kiri/kanan pada Merkle path (mencegah double-spending via pergantian leaf index).
2. **[P0] Boolean Constraint pada `has_change`:**
   * Enforce constraint boolean: $\text{has\_change} \times (1 - \text{has\_change}) == 0$.
   * Enforce jika `has_change == 0` maka $\text{change\_value} == 0$ dan output commitment nol.
3. **[P0] Integer Range Safety (64-bit Constraints):**
   * Range constrain input value, payout, protocol fee, execution fee, dan change value ke $[0, 2^{64})$ menggunakan bit-decomposition gadget.
   * Mencegah eksploitasi pencetakan uang melalui *modular wrap-around* pada scalar field $\mathbb{F}_r$.

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

1. **Cek Source Code Riil Terlebih Dahulu:** Jangan berasumsi atau hanya membaca ringkasan. Buka dan baca kode Rust aslinya sebelum mengubah atau mendokumentasikan.
2. **Hindari Birokrasi Berlebihan:** Jangan membuat puluhan file markdown baru atau checklist ribet yang mendistraksi developer. Jaga dokumentasi tetap terpusat pada bab modular yang sudah ada.
3. **Zero Breaking Change untuk Public API:** Semua fungsi yang diexport di `lib.rs` harus tetap kompatibel dengan crate lain di workspace.
4. **Keamanan Kriptografi & Finansial:**
   * Jangan pernah membypass verifikasi kriptografi di production path.
   * `fail-closed` jika rahasia/kunci tidak tersedia (dilarang fallback ke mock key di mode non-test).
   * Uji selalu kondisi negatif (tes gagal harus membuktikan state on-chain/DB tidak berubah).

---

## 6. Tooling & MCP Capabilities

* **Web Search & Fetching:** Tersedia MCP `parallel-search` (`web_search`, `web_fetch`) dan native `search_web`/`read_url_content` untuk mencari paper ZK, auditing report, atau referensi kriptografi terkini.
* **Browser Agent:** Tersedia `browser_subagent` jika butuh navigasi interaktif atau visual inspection.
* **Environment:** Codespace Linux dengan Git, Curl, dan tool eksekusi shell. Selalu verifikasi sintaks dan tipe secara ketat pada crate `nimbus-core` dan `nimbus-contracts`.
