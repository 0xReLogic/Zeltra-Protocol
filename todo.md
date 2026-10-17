# Zeltra Protocol TODO (Active Backlog)

> **Prinsip Eksekusi (Anti-Burnout / Single-Pass Testnet):**  
> Seluruh task yang sudah selesai (`[x]`) telah diarsipkan agar dokumen ini murni berisi **task pending (`[ ]`)**.  
> **Urutan Kerja:** Selesaikan freeze sirkuit, audit independen, dan MPC Trusted Setup terlebih dahulu. Pengujian on-chain Arbitrum Sepolia dijalankan **sekali di akhir (Fase 4 / Gate G)** sebelum peluncuran mainnet.

---

## Ringkasan Pondasi yang Telah Selesai & Terverifikasi (100% Lolos)

- **Pure ZK-UTXO Architecture (DEC-038):** Arsitektur 100% murni ZK-UTXO tanpa pihak ketiga/guardian/threshold blind-signing.
- **Dual Sovereign Rails (USDC & WETH — DEC-034):** Canonical field bit-packing (`asset_id`), payable native ETH wrap on-contract, multi-liability siloed solvency, dan zero-oracle gas reimbursement. (162 unit tests passed, 46/46 Python real-case passed).
- **Direct MMR Inflow (DEC-038B):** Deposit langsung via `deposit_direct_note` menyisipkan note commitment ke Merkle Mountain Range (`_mmr_insert`), emisi log AEAD memo 96-byte (`NoteCommitmentAppended`), 0 bps deposit fee permanen, dan mutlak solvent.
- **Shielded-to-Shielded In-Pool Transfer (DEC-043):** Strict 3-Mode JoinSplit state machine. Mode 2 mentransfer dana langsung antar alamat ZSA Bech32m (`zs1...`) dengan 0 bps protocol fee, dual 96B memos di MMR on-chain, dan Merchant Masking (44/44 Python test suite passed).
- **Viewing Key Export Suite & CEX Selective Disclosure (DEC-040):** 4-tier key hierarchy (`sk`, `nk`, `fvk`, `ivk`/`ovk`), `SelectiveDisclosureCircuit` Groth16 dengan Verifier Binding bursa (Binance/Coinbase), dan engine verifier mandiri (`zeltra-sdk/src/compliance/verifier.rs`).
- **In-Browser WebAssembly SDK Prover (DEC-041):** Runner prover Groth16 langsung di browser via Web Worker dengan siklus isolasi memori sekali pakai (`worker.terminate()`) anti-OOM linear.
- **Pure ZK-UTXO HTTP 402 AI Agent Facilitator (DEC-042):** Mikropembayaran AI Agent sub-sen murni Groth16 1-in-1-out, dynamic dust threshold tracking, dan universal merchant payout on-chain.
- **Stateless Sequencer Relayer (DEC-038C):** Single sequencer daemon (`scripts/start_relayer.sh`), dynamic MMR proof sync, fail-closed preflight validation, dan dual-layer nullifier double-spend prevention.

---

## 1. Kriptografi, Circuit Freeze & MPC Trusted Setup (Fase 2)
Ref: [`docs/roadmap.md`](file:///workspaces/Zeltra-Protocol/docs/roadmap.md) (Fase 2) & [`research/decisions/DEC-035`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-035-cryptographic-canonicality-public-input-binding-and-mmr.md)

- [ ] **Circuit ABI & Constraints Freeze:**
  - Kunci mati parameter R1CS dan public-input ABI pada 16 PI (`PrivateNoteCircuit`) dan 19 PI (`JoinSplitCircuit`).
  - Finalisasi parameter Two-Limb quote hash, Formal Scope Binding Gadget, dan 448 quadratic range gates.
- [ ] **Proving/Verifying Keys Management:**
  - Simpan proving/verifying key resmi sebagai artifact versioned; pastikan test runner menggunakan cached dev keys secara terisolasi.
- [ ] **MPC Ceremony (Phase 2 Trusted Setup):**
  - Jalankan multi-party computation ceremony setelah seluruh constraint sirkuit dibekukan dan direview.
  - Masukkan final verifying keys resmi ke smart contract Stylus.
- [ ] **Independent Security Review:**
  - Audit independen untuk smart contract Stylus, verifier EIP-2537, dan sirkuit Groth16 Arkworks (Audit Tier-1).

---

## 2. Keamanan & Sanitasi Kredensial (Fase 3)

- [ ] **Credential Rotation & Git Cleanup:**
  - Rotasi seluruh secret/private key dev historis dan pembersihan git commit history sebelum peluncuran testnet publik.

---

## 3. Unified Hard-Test Arbitrum Sepolia (Fase 4 / Gate G)
Ref: Checklist lengkap di [`docs/todos/testnet-gate-g-checklist.md`](file:///workspaces/Zeltra-Protocol/docs/todos/testnet-gate-g-checklist.md) & [`docs/roadmap.md`](file:///workspaces/Zeltra-Protocol/docs/roadmap.md) (Fase 4)

> ⚠️ **Catatan:** Deploy kontrak Stylus dan relayer node **SEKALI SAJA** dengan verifying key resmi hasil MPC setelah Tahap 1–2 selesai.

- [ ] **Matrix On-Chain Full Lifecycle (USDC & WETH):**
  - Deposit direct (0 bps fee) $\rightarrow$ in-pool transfer $\rightarrow$ partial spend $\rightarrow$ change note $\rightarrow$ second spend $\rightarrow$ withdraw-all.
  - Verifikasi saldo kontrak reconcile exact terhadap total liabilities.
  - Relayer mengklaim akumulasi execution fee tepat; klaim kedua gagal tanpa state change.
- [ ] **Crash Recovery, Boundary & Negative Scenarios:**
  - Uji penolakan double-spend nullifier replay on-chain.
  - Uji penolakan pemalsuan nominal change note atau wrong owner key.
  - Uji non-aliasing nullifier (`nullifier_1 == nullifier_2`) dan strict 3-mode calldata guards.
  - Uji mati paksa relayer node di setiap boundary (sebelum enqueue, setelah reserve note, setelah proof generation, sebelum broadcast, sebelum local commit).
  - Reorg dan replacement transaction tidak menyebabkan note ganda.
  - Concurrent spend pada note yang sama: hanya satu yang berhasil.
- [ ] **Cross-Chain CCIP Testnet Flow:**
  - Uji alur lengkap: source broadcast $\rightarrow$ CCIP router $\rightarrow$ destination execution $\rightarrow$ destination confirmation.
  - Uji penolakan replay `message_id` dan sender yang tidak ter-allowlist.

---

## 4. Mainnet Arbitrum One Launch & Go-Live (Fase 5)
Ref: [`docs/roadmap.md`](file:///workspaces/Zeltra-Protocol/docs/roadmap.md) (Fase 5) & [`docs/bisnis.md`](file:///workspaces/Zeltra-Protocol/docs/bisnis.md)

- [ ] Verifikasi invariant solvensi final: `contract assets >= all liabilities` (siloed multi-asset).
- [ ] Deployment smart contract resmi di Arbitrum One.
- [ ] Program Bug Bounty Publik.
- [ ] Peluncuran Tiga Pilar Komersial: Shielded Checkout Widget, HTTP 402 Facilitator, dan Sovereign Shielded RWA Staking Gateway.
