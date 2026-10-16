# Zeltra Protocol TODO (Active Backlog)

> **Prinsip Eksekusi (Anti-Burnout / Single-Pass Testnet):**  
> Seluruh task yang sudah selesai (`[x]`) telah diarsipkan agar dokumen ini murni berisi **task pending (`[ ]`)**.  
> **Urutan Kerja:** Selesaikan pengembangan SDK, integrasi lokal, audit, dan MPC Trusted Setup terlebih dahulu. Pengujian on-chain Arbitrum Sepolia dijalankan **sekali di akhir (Tahap 4)** sebelum peluncuran mainnet.

---

## Ringkasan Pondasi yang Telah Selesai & Terverifikasi (100% Lolos)

- **Pure ZK-UTXO Architecture (DEC-038):** Arsitektur 100% murni ZK-UTXO tanpa pihak ketiga/guardian.
- **Direct MMR Inflow (DEC-038B):** Deposit langsung via `deposit_direct_note` menyisipkan note commitment ke Merkle Mountain Range (`_mmr_insert`), emisi log AEAD memo 96-byte (`NoteCommitmentAppended`), 0 bps deposit fee permanen, dan mutlak solvent.
- **Stateless Sequencer Relayer (DEC-038C):** Single sequencer daemon (`scripts/start_relayer.sh`), dynamic MMR proof sync, fail-closed preflight validation, dan dual-layer nullifier double-spend prevention.
- **Groth16 Verifiers di Stylus:** EIP-2537 (`0x0c` MSM + `0x0f` Pairing Check) untuk `PrivateNoteCircuit` (16 public inputs, Two-Limb quote hash, Formal Scope Binding) dan `Universal JoinSplitCircuit` (19 public inputs, 448 quadratic range gates).
- **Client SDK State & Security:** Tiered Stochastic Knapsack Coin Selection, in-pool `consolidate_notes()`, 2PC crash-safe storage (Argon2id + ChaCha20Poly1305), viewing key trial decryption (`recover_notes`), dan zeroize memory safety.

---

## 0. Implementasi Dual-Asset Architecture (USDC & WETH — DEC-034)
Ref: [`DEC-034`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-034-dual-asset-architecture-usdc-weth-sovereign-commerce-rails.md)

- [x] **Task A — Kriptografi, Bit-Packing & Sirkuit ZK (`zeltra-core`):** [SELESAI & AUDITED]
  - Detail spesifikasi eksekusi: [`taskAdec034.md`](file:///workspaces/Zeltra-Protocol/taskAdec034.md) & [`DEC-034A`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-034A-canonical-field-bit-packing-and-intra-asset-circuit-constraints.md)
  - `PrivateNoteV1` asset_id, `value_packed = value + asset_id * 2^64`, R1CS gadget `flags_packed`, dummy input & zero-value change note canonical packing ($2^{64}$), intra-asset equality. (162 unit tests passed, Python soundness passed).
- [x] **Task B — Smart Contract Stylus Arbitrum (`zeltra-contracts`):** [SELESAI & AUDITED]
  - Detail spesifikasi eksekusi: [`taskBdec034.md`](file:///workspaces/Zeltra-Protocol/taskBdec034.md) & [`DEC-034B`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-034B-stylus-siloed-solvency-payable-weth-wrap-and-multi-asset-settlement.md)
  - Multi-liability per-asset, `deposit_direct_weth` & `deposit_direct_native_eth` auto-wrap, pemisahan raw vs normalized amount, bit-bounds check `flags_packed`, unifikasi storage relayer fee. (64/64 Rust unit tests passed, wasm32 clean, clippy clean, Python real-case 46/46 passed).
- [x] **Task C — Relayer Zero-Oracle, Client SDK & CLI Tooling (`zeltra-node`, `zeltra-sdk`, `zeltra-cli`):** [SELESAI & AUDITED]
  - Detail spesifikasi eksekusi: [`taskCdec034.md`](file:///workspaces/Zeltra-Protocol/taskCdec034.md) & [`DEC-034C`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-034C-relayer-zero-oracle-reimbursement-and-sdk-multi-asset-wallet.md)
  - EIP-712 quote preimage binding `asset_id`, zero-oracle gas reimbursement (WETH in Gwei, USDC in Micro-USDC), anti-drain preflight validation, partitioned wallet & Knapsack coin selection, canonical 48-byte BE memo trial decryption recovery. (66/66 SDK tests passed, 97/97 node tests passed, 4/4 CLI tests passed, clippy & fmt clean).

---

## 1. SDK, Ekosistem Fitur & Tooling (Fokus Aktif Saat Ini)
Ref: [`docs/roadmap.md`](file:///workspaces/Zeltra-Protocol/docs/roadmap.md) (Fase 1) & [`docs/todos/private-note-balance.md`](file:///workspaces/Zeltra-Protocol/docs/todos/private-note-balance.md)

- [ ] **Shielded-to-Shielded In-Pool Transfer:**
  - Pengiriman private note langsung antar akun/merchant Zeltra (`output_commitment_1` penerima, `output_commitment_2` kembalian) tanpa penarikan USDC publik on-chain (Merchant Masking).
- [x] **Viewing Key Export Suite & CEX Selective Disclosure (DEC-040):** [SELESAI & AUDITED]
  - Tooling UI/CLI bagi akuntan B2B untuk export laporan audit transaksi kas privat, rekonsiliasi double-entry, dan pembuktian kepemilikan note ke bursa terpusat (Binance/Coinbase) dengan Verifier Binding.
  - Sub-spesifikasi:
    * [x] [`DEC-040A: Cryptographic Key Separation (sk/nk/fvk) & Groth16 Circuit`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-040A-selective-disclosure-circuit-and-key-separation.md) (Task: [`archive/tasks/taskAdec040.md`](file:///workspaces/Zeltra-Protocol/archive/tasks/taskAdec040.md)) [SELESAI — PR #23]
    * [x] [`DEC-040B: Client SDK Compliance & Double-Entry Audit Trail`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-040B-client-sdk-compliance-and-double-entry-audit-trail.md) (Task: [`archive/tasks/taskBdec040.md`](file:///workspaces/Zeltra-Protocol/archive/tasks/taskBdec040.md)) [SELESAI — PR #24]
    * [x] [`DEC-040C: CEX Verifier Engine & Compliance CLI Tooling`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-040C-cex-verifier-engine-and-compliance-cli.md) (Task: [`archive/tasks/taskCdec040.md`](file:///workspaces/Zeltra-Protocol/archive/tasks/taskCdec040.md)) [SELESAI — PR #25]
- [x] **SDK CEX Helper (DEC-040C):** [SELESAI & AUDITED]
  - Helper otomatis "Export Source of Funds" dengan bukti kriptografis selektif & Verifier Binding untuk bypass freeze bursa (Binance/Coinbase) (Ref: [`DEC-040C`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-040C-cex-verifier-engine-and-compliance-cli.md)).
- [x] **In-Browser WebAssembly SDK Prover (DEC-041):** [SELESAI & AUDITED]
  - Detail spesifikasi eksekusi: [`archive/tasks/taskDEC041.md`](file:///workspaces/Zeltra-Protocol/archive/tasks/taskDEC041.md) & [`DEC-041`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-041-in-browser-wasm-groth16-prover-and-disposable-worker.md)
  - Runner prover Groth16 langsung di browser/mobile via Web Worker dengan siklus isolasi memori (`worker.terminate()`) anti-OOM linear (1-in-1-out, Universal 2-in-2-out JoinSplit, CEX Selective Disclosure) [SELESAI — PR #26].
- [ ] **HTTP 402 AI Agent Facilitator (x402 Protocol):**
  - Penyelesaian streaming payment mikro untuk AI Agent via Prepaid Note Pool dan settlement batas ambang.

---

## 2. Kriptografi, Circuit Freeze & MPC Trusted Setup
Ref: [`docs/roadmap.md`](file:///workspaces/Zeltra-Protocol/docs/roadmap.md) (Fase 2) & [`research/decisions/DEC-035`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-035-cryptographic-canonicality-public-input-binding-and-mmr.md)

- [ ] **Circuit ABI & Constraints Freeze:**
  - Kunci mati parameter R1CS dan public-input ABI pada 16 PI (`PrivateNoteCircuit`) dan 19 PI (`JoinSplitCircuit`).
- [ ] **Proving/Verifying Keys Management:**
  - Simpan proving/verifying key resmi sebagai artifact versioned; pastikan test runner menggunakan cached dev keys secara terisolasi.
- [ ] **MPC Ceremony (Phase 2 Trusted Setup):**
  - Jalankan multi-party computation ceremony setelah seluruh constraint sirkuit dibekukan dan direview.
- [ ] **Independent Security Review:**
  - Audit independen untuk smart contract Stylus, verifier EIP-2537, dan sirkuit Groth16 Arkworks.

---

## 3. Keamanan & Sanitasi Kredensial

- [ ] **Credential Rotation & Git Cleanup:**
  - Rotasi seluruh secret/private key dev historis dan pembersihan git commit history.

---

## 4. Unified Hard-Test Arbitrum Sepolia (Gladi Bersih Akhir)
Ref: [`docs/roadmap.md`](file:///workspaces/Zeltra-Protocol/docs/roadmap.md) (Fase 3) & Gate G di [`docs/todos/private-note-balance.md`](file:///workspaces/Zeltra-Protocol/docs/todos/private-note-balance.md)

> ⚠️ **Catatan:** Deploy kontrak Stylus dan relayer node **SEKALI SAJA** dengan verifying key resmi hasil MPC setelah Tahap 1–3 selesai.

- [ ] **Matrix On-Chain Full Lifecycle:**
  - Deposit 5 USDC menghasilkan net note exact 5 USDC (0 bps deposit fee).
  - Spend 1 USDC membayar merchant tepat 1 USDC.
  - Protocol fee (flat 45 bps) dan execution fee (gas reimbursement + 15% markup) cocok dengan signed quote.
  - Change note persis sama dengan input minus total debit.
  - Spend kedua memakai change note, bukan input lama.
  - Replay input pertama gagal (revert) dan seluruh state on-chain tidak berubah.
  - Withdraw-all via spend ke wallet owner mengembalikan seluruh sisa setelah fee (user note liability menjadi 0).
  - Relayer mengklaim akumulasi execution fee tepat; klaim kedua gagal tanpa state change.
- [ ] **Crash Recovery & Idempotency Testing:**
  - Uji kegagalan di setiap boundary (sebelum enqueue, setelah reserve note, setelah proof generation, sebelum broadcast, sebelum local commit).
  - Reorg dan replacement transaction tidak menyebabkan note ganda.
  - Concurrent spend pada note yang sama: hanya satu yang berhasil.
- [ ] **Cross-Chain CCIP Testnet Flow:**
  - Uji alur lengkap: source broadcast $\rightarrow$ CCIP router $\rightarrow$ destination execution $\rightarrow$ destination confirmation.
  - Uji penolakan kegagalan destinasi (destination failure).

---

## 5. Mainnet Arbitrum One Launch & Go-Live
Ref: [`docs/roadmap.md`](file:///workspaces/Zeltra-Protocol/docs/roadmap.md) (Fase 5) & [`docs/bisnis.md`](file:///workspaces/Zeltra-Protocol/docs/bisnis.md)

- [ ] Verifikasi invariant solvensi final: `contract assets >= all liabilities`.
- [ ] Deployment smart contract resmi di Arbitrum One.
- [ ] Program Bug Bounty Publik.
- [ ] Peluncuran Tiga Pilar Komersial: Shielded Checkout Widget, HTTP 402 Facilitator, dan Sovereign Shielded RWA Staking Gateway.
