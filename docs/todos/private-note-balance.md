# Private Note Balance dan Change Output

**Priority:** Tier 0 - Mainnet Blocker  
**Catatan:** Dokumen ini murni berfokus pada pekerjaan pending (`[ ]`). Seluruh task yang telah selesai diarsipkan.  
**Decision:** [`research/decisions/DEC-016-private-note-change-ledger.md`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-016-private-note-change-ledger.md) | **Frozen Spec:** [`research/decisions/DEC-016A-private-note-spec-freeze.md`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-016A-private-note-spec-freeze.md)

---

## Prinsip Alur Kerja (Testnet di Akhir)

> 💡 **PENTING (Zero Gas Waste):**  
> Jangan deploy kontrak ke Arbitrum Sepolia berulang kali. Selesaikan Gate C (Circuit Freeze & MPC) dan Gate E (Fitur Lanjutan SDK) secara **lokal/off-chain**.  
> **Gate G (Hard Test Sepolia)** dijalankan **SEKALI SAJA** sebagai gladi bersih gabungan bersama relayer dan verifying key resmi hasil MPC sebelum rilis Mainnet.

---

## Status Gate Inti yang Telah Selesai (100% Lolos)

- **Gate A (Model & Encoding ZK-UTXO):** SELESAI — Poseidon Grain-128 W3/W5 (DEC-021), note commitment, nullifier derivation, dan Merkle Mountain Range (MMR, DEC-032).
- **Gate B (Accounting & Solvensi):** SELESAI — Multi-liability accounting (`user_note_liability`, `accrued_execution_fee_liability`, `realized_protocol_fees`), checks-effects-interactions, dan solvency invariant mutlak on-chain (`assets >= liabilities`).
- **Gate C0 (Security Repair & Range Constraints):** SELESAI — 64-bit integer range constraints, Two-Limb quote hash 128-bit range checks, dan formal scope binding gadget terintegrasi di `PrivateNoteCircuit` (16 PIs) & `JoinSplitCircuit` (19 PIs).
- **Gate D (Stylus Contract Note Ledger):** SELESAI — Verifier Groth16 EIP-2537 (`0x0c` MSM + `0x0f` Pairing Check), direct MMR insertion (`_mmr_insert`), bounded root history ring, dan dual-epoch nullifier mapping.
- **Gate E Core (SDK Wallet State):** SELESAI — `PrivateNoteWallet` dengan Tiered Stochastic Knapsack coin selection, in-pool note consolidation, 2PC crash-safe storage terenkripsi Argon2id + ChaCha20Poly1305, dan viewing key recovery (`recover_notes`).
- **Gate F (Relayer Node):** SELESAI — Stateless sequencer relayer dengan fail-closed Groth16 preflight check (DEC-026 C-01), dual-layer nullifier double-spend prevention, dynamic MMR proof sync, dan three-way reconciliation engine.

---

## 1. Fitur Ekosistem Lanjutan SDK & B2B (Fokus Aktif Lokal)
Ref: [`docs/roadmap.md`](file:///workspaces/Zeltra-Protocol/docs/roadmap.md) (Fase 1)

- [ ] **Shielded-to-Shielded In-Pool Transfer:**
  - Pengiriman private note langsung antar akun/merchant Zeltra (`output_commitment_1` penerima, `output_commitment_2` kembalian) tanpa penarikan USDC publik on-chain (Merchant Masking).
- [ ] **Viewing Key Export Suite & CEX Selective Disclosure (DEC-040):**
  - Tooling UI/CLI bagi akuntan B2B untuk export laporan audit transaksi kas privat, rekonsiliasi double-entry, dan pembuktian kepemilikan note ke bursa terpusat (Binance/Coinbase) dengan Verifier Binding.
  - Sub-spesifikasi:
    * [`DEC-040A: Cryptographic Key Separation (sk/nk/fvk) & Groth16 Circuit`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-040A-selective-disclosure-circuit-and-key-separation.md) (Task: [`taskAdec040.md`](file:///workspaces/Zeltra-Protocol/taskAdec040.md))
    * [`DEC-040B: Client SDK Compliance & Double-Entry Audit Trail`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-040B-client-sdk-compliance-and-double-entry-audit-trail.md) (Task: [`taskBdec040.md`](file:///workspaces/Zeltra-Protocol/taskBdec040.md))
    * [`DEC-040C: CEX Verifier Engine & Compliance CLI Tooling`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-040C-cex-verifier-engine-and-compliance-cli.md) (Task: [`taskCdec040.md`](file:///workspaces/Zeltra-Protocol/taskCdec040.md))
- [ ] **SDK CEX Helper (DEC-040C):**
  - Helper otomatis "Export Source of Funds" dengan pembuktian selektif & Verifier Binding untuk bypass freeze deposit bursa terpusat (Binance/Coinbase) (Ref: [`DEC-040C`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-040C-cex-verifier-engine-and-compliance-cli.md)).
- [ ] **In-Browser WebAssembly SDK Prover:**
  - Runner prover Groth16 langsung di browser/mobile via Web Worker dengan siklus terminasi memori (`worker.terminate()`) anti-OOM linear.
- [ ] **Integrasi x402 Facilitator:**
  - Jalur streaming payment mikro HTTP 402 untuk AI Agent.

---

## 2. Kriptografi & Trusted Setup (Gate C)
Ref: [`docs/roadmap.md`](file:///workspaces/Zeltra-Protocol/docs/roadmap.md) (Fase 2)

- [ ] Freeze ABI sirkuit 16 PIs & 19 PIs beserta batasan constraints R1CS.
- [ ] Production proving/verifying artifacts (keys) selesai dan versioned.
- [ ] Persiapan dan pelaksanaan Trusted Setup Ceremony (MPC Phase 2).
- [ ] Circuit dan accounting mendapat independent security review (Audit Tier-1: Zellic / Trail of Bits).

---

## 3. Gate G - Unified Hard Test Arbitrum Sepolia (Gladi Bersih Akhir)
Ref: [`docs/roadmap.md`](file:///workspaces/Zeltra-Protocol/docs/roadmap.md) (Fase 3)

*Dilakukan setelah Gate C & Fitur SDK selesai. Deploy SEKALI SAJA ke Sepolia untuk memvalidasi seluruh skenario berikut:*

- [ ] Deposit 5 USDC menghasilkan net note exact 5 USDC (0 bps deposit fee).
- [ ] Spend 1 USDC membayar merchant tepat 1 USDC.
- [ ] Protocol fee (flat 45 bps) dan execution fee (gas reimbursement + 15% markup) cocok dengan signed quote.
- [ ] Change note sama dengan input (5 USDC) dikurangi seluruh debit (1 USDC merchant + 45 bps protocol fee + execution fee).
- [ ] Spend kedua memakai change note (~3.9 USDC), bukan input lama.
- [ ] Replay input pertama gagal (revert) dan seluruh state/balance on-chain tidak berubah.
- [ ] Withdraw-all via spend ke wallet owner mengembalikan seluruh sisa setelah fee.
- [ ] Final user note liability nol dan tidak ada orphan USDC di luar recognized liabilities/fees.
- [ ] Relayer claim exact execution fee; claim kedua gagal tanpa state change.
- [ ] Crash recovery pada setiap boundary relayer (sebelum enqueue, setelah reserve note, setelah proof generation, sebelum broadcast, sebelum local commit).
- [ ] Reorg dan replacement transaction tidak menyebabkan note ganda.
- [ ] Wrong owner key / wrong Merkle path gagal.
- [ ] Forged change lebih besar/kecil satu unit terkecil gagal.
- [ ] Concurrent spend note yang sama: hanya satu yang berhasil.
- [ ] Cross-chain CCIP testnet flow (source $\rightarrow$ router $\rightarrow$ destination $\rightarrow$ confirmation).

---

## 4. Mainnet Go-Live & Pilot (Fase 5)
Ref: [`docs/roadmap.md`](file:///workspaces/Zeltra-Protocol/docs/roadmap.md) (Fase 5)

- [ ] Deployment smart contract resmi di Arbitrum One.
- [ ] Program Bug Bounty Publik di Arbitrum One.
- [ ] Pilot Payroll B2B & Kemitraan Merchant Commerce.
