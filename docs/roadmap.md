# Roadmap Prioritas Rekayasa Zeltra Protocol (2026)

Dokumen ini adalah kompas urutan pengerjaan tim.

> 💡 **Prinsip Utama Rekayasa (Anti-Burnout / Zero Gas Waste):**  
> **"Local Simulation & Off-Chain Verification First, Single-Pass Testnet Last."**  
> Jangan deploy kontrak ke Arbitrum Sepolia berulang-ulang saat sirkuit, invariant, atau SDK masih terus dimodifikasi. Selesaikan seluruh constraint, verifikasi formal, SDK tooling, dan Trusted Setup secara lokal. Testnet hanya dijalankan **SEKALI SAJA** sebagai gladi bersih sebelum peluncuran mainnet.

---

## Fase 1: SDK, Browser Prover & Ekosistem Fitur [SELESAI 100% & TERVERIFIKASI]
Ref: [`research/decisions/DEC-040`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-040-viewing-key-export-suite-and-cex-selective-disclosure.md), [`DEC-041`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-041-in-browser-wasm-groth16-prover-and-disposable-worker.md), [`DEC-042`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-042-privacy-preserving-x402-facilitator-and-pure-zk-utxo-micropayments.md), [`DEC-043`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-043-shielded-to-shielded-in-pool-transfer-and-merchant-masking.md)

- [x] **Client SDK & Browser Runner (`zeltra-sdk`):**
  - In-Browser WebAssembly SDK Prover: Eksekusi Groth16 client-side non-blocking via Web Worker dengan isolasi memori anti-OOM linear (`worker.terminate()`) (DEC-041).
  - Viewing Key Recovery Suite: Ekstraksi dan dekripsi saldo note via HKDF `ovk`/`ivk` dari memo on-chain, tooling export laporan audit kas & SPT pajak B2B (DEC-037 & DEC-040B).
  - CEX Helper Tooling: Modul "Export Source of Funds" untuk pembuktian selektif asal-usul dana bersih ke bursa terpusat (Binance/Coinbase) dengan Verifier Binding (DEC-040A/C).
- [x] **Shielded-to-Shielded In-Pool Transfer (DEC-043):**
  - Transfer note privat langsung antar akun/merchant Zeltra (`output_commitment_1` penerima, `output_commitment_2` kembalian) tanpa konversi ke USDC publik on-chain (Merchant Masking). 44/44 test Python real-case lolos.
- [x] **HTTP 402 Pure ZK-UTXO Micropayments (x402 Facilitator - DEC-042):**
  - Streaming payment mikro untuk AI Agent via Pure ZK-UTXO Groth16 1-in-1-out, dynamic dust threshold tracking, zero-oracle reimbursement.
- [x] **Cross-Chain CCIP Simulation:**
  - Verifikasi isolasi logic CCIP router, mapping message ID, dan skenario penolakan double-spend secara lokal.

---

## Fase 2: Circuit Freeze, Security Audit & Trusted Setup MPC (FOKUS AKTIF SAAT INI)
Ref: [`research/decisions/DEC-035`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-035-cryptographic-canonicality-public-input-binding-and-mmr.md) & [`DEC-036`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-036-universal-2-in-2-out-joinsplit-full-stack-architecture.md)

*Kunci mati seluruh spesifikasi sebelum menyentuh jaringan publik:*

- [ ] **Circuit Constraints & ABI Freeze:**
  - Kunci constraint R1CS 16 PI (`PrivateNoteCircuit`) dan 19 PI (`JoinSplitCircuit`).
  - Finalisasi parameter Two-Limb quote hash, Formal Scope Binding Gadget, dan 448 quadratic range gates.
- [ ] **Independent Circuit & Contract Audit:**
  - Review formal invariant solvensi matematis (`contract balance >= total_liabilities`).
  - Analisis keamanan curve BLS12-381 precompile EIP-2537 di Stylus WASM.
- [ ] **Production MPC Ceremony (Phase 2 Trusted Setup):**
  - Eksekusi MPC Trusted Setup multi-partai untuk menghasilkan final proving & verifying keys resmi.
  - Simpan verifying keys resmi sebagai artifact versioned ke dalam smart contract Stylus.

---

## Fase 3: Sanitasi Kredensial & Pembersihan Git History

- [ ] **Credential Rotation & History Cleanup:**
  - Rotasi seluruh secret/private key dev historis dan pembersihan git commit history sebelum peluncuran testnet publik.

---

## Fase 4: Unified Hard-Test Arbitrum Sepolia (GLADI BERSIH AKHIR)
Ref: [`docs/todos/testnet-gate-g-checklist.md`](file:///workspaces/Zeltra-Protocol/docs/todos/testnet-gate-g-checklist.md)

*Deploy kontrak dan jalankan relayer node **SEKALI SAJA** dengan verifying key resmi hasil MPC:*

- [ ] **Matrix Validasi End-to-End On-Chain:**
  - Deposit direct (0 bps fee) $\rightarrow$ in-pool transfer $\rightarrow$ partial spend $\rightarrow$ change note $\rightarrow$ second spend $\rightarrow$ withdraw-all.
  - Verifikasi saldo kontrak reconcile exact terhadap total liabilities (USDC & WETH).
- [ ] **Crash Recovery & Idempotency Boundary:**
  - Uji mati paksa relayer node di setiap titik transisi: sebelum broadcast, setelah broadcast, sebelum local output commit.
  - Uji penolakan mutlak input nullifier replay dan proof invalid.
- [ ] **Cross-Chain CCIP Testnet Flow:**
  - Eksekusi source broadcast $\rightarrow$ router CCIP $\rightarrow$ destination execution $\rightarrow$ destination confirmation.

---

## Fase 5: Mainnet Arbitrum One Launch & Adopsi Pasar
Ref: [`docs/bisnis.md`](file:///workspaces/Zeltra-Protocol/docs/bisnis.md)

- [ ] **Deploy Production Arbitrum One:**
  - Deployment Stylus contract resmi, registrasi address ke dokumentasi publik, dan peluncuran Bug Bounty.
- [ ] **Tiga Pilar Produk Komersial:**
  - **Shielded Checkout Widget:** Komponen 1-baris `<ZeltraCheckout />` untuk merchant global.
  - **HTTP 402 Facilitator:** Rel pembayaran native untuk autonomous AI Agents.
  - **Sovereign Shielded RWA Staking Gateway:** Opt-in yield private note (Ondo USDY, BlackRock BUIDL) tanpa bocor ke kompetitor.

