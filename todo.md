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
- **In-Memory Incremental MMR Engine & Proof Acceleration (DEC-045):** Flat Post-Order MMR Node Array (`nodes: Vec<Fr>`) formula Peter Todd di `zeltra-core`, $O(\log N)$ proof generation tanpa poseidon hash (<20 µs), concurrent thread-safe accumulator `Arc<RwLock<FlatMerkleMountainRange>>` di `zeltra-node`, dan 1-byte HKDF View Tag protocol di memo 96-byte `zeltra-sdk` untuk 23.6x scanning speedup.
- **Stateless Sequencer Relayer (DEC-038C):** Single sequencer daemon (`scripts/start_relayer.sh`), dynamic MMR proof sync, fail-closed preflight validation, dan dual-layer nullifier double-spend prevention.
- **Circuit Hardening & Null-Branch Defense (DEC-046 — IACR 2026/2235):** Gerbang pendamping kuadratik kanonikal deterministik anti-null branch pada dummy input slot 2 (`in2_is_dummy * (owner, rho, r, index) == 0`), inactive change outputs 1 & 2 (`(1 - has_change) * (owner, rho, r) == 0`), dan zero-safe anti-aliasing inverse (`enforce_zero_safe_diff_inv`). 16 negative tests membuktikan saksi tampered tertolak mutlak.
- **Augmented Jacobian Diagnostics & Rigidity Auditing (DEC-047 — IACR 2026/1852):** Engine evaluasi $M(a) = \begin{pmatrix} J(a) \\ E_F \end{pmatrix}$, eliminasi Gauss sparse Markowitz over $\mathbb{F}_p$, row-space test $e_j^T \in \text{rowspan}(M(a))$, dan sintesis sertifikat garis afin Proposisi 27. Matriks 6 keluarga saksi adversarial (FAM-1 s/d FAM-6) 100% rigid ($r_T = 0$), terverifikasi di Rust (`formal_tests.rs`, 26/26 passed) dan harness independen Python (`test_dec046_dec047_jacobian_soundness.py`, 100% verified).
- **Committing AEAD & Partitioning Oracle Defense (DEC-048 — IACR 2026/240):** Generic transform `EtH-KS` (Minematsu & Inoue) dengan Key Separation RCR-PRG $G(S) \to (K_{\text{enc}}, K_{\text{cmt}}, N_7)$ dan CR-PRF $H(K_{\text{cmt}}, N_7 \parallel A \parallel C)$. Mengeliminasi kerentanan partitioning oracle (`AUD-21`), desinkronisasi saldo (`AUD-23`), dan multi-recipient equivocation (`AUD-24`) pada batas calldata tepat 96 byte dan View Tag 1-byte. Terverifikasi 11/11 suite pengujian adversarial Python dan 61/61 test lifecycle multi-asset.
- **Leakage-Resilient Decapsulation & CT-FOE Defense (DEC-049 — Pay, Peters & Standaert 2026):** Terverifikasi penuh (*IMPLEMENTED & VERIFIED*). Evaluasi bendera validitas note bebas-cabang (`ConstantTimeMemoScanner` CT-FOE via `subtle::Choice`), sanitasi canonical scalar branchless over $\mathbb{F}_r$, mitigasi Flag DPA (`AUD-25`), mitigasi Guessing DPA & non-contributory point defense (`AUD-26`), dan penyelarasan standar FIPS 203 implicit rejection (`AUD-27`). Terverifikasi 100% pada 5 kategori matriks uji adversarial (Gate V-2), evaluasi statistik diferensial Dudect Welch t-test $|t| < 4.5$ tanpa false positive (Gate V-3), dan inspeksi objdump assembly linear bebas lompatan rahasia (Gate V-4).
- **Zero-Custody Regulatory Safe Harbor & Direct Self-Broadcast (DEC-054):** Terverifikasi penuh (*IMPLEMENTED & VERIFIED*). Jalur self-broadcast unhosted wallet direct EVM (`SelfBroadcastTransactionPayload`, `prepare_self_broadcast_joinsplit`, `prepare_self_broadcast_shielded_transfer`, `encode_joinsplit_calldata`) dengan `execution_fee = 0` murni independen dari relayer (mitigasi 18 U.S.C. § 1960). Eliminasi mutlak istilah komersial broker (`markup` 100% purged dari codebase Rust & EIP-712 typed data menjadi `executionOverheadFee`). Terverifikasi 5/5 test gates integrasi (`self_broadcast_tests.rs`) dan zero warning across workspace.
- **Cross-Chain Inbound Deposit & MMR Minting (DEC-055 — Chainlink CCIP v2):** Terverifikasi penuh (*IMPLEMENTED & VERIFIED*). Pipeline deposit cross-chain programmable via Router CCIP ke Arbitrum Stylus receiver (`ccip_receive`), validasi canonical scalar (`from_evm_scalar < r`), min amount, emisi `CCIPDepositExecuted` & `NoteCommitmentAppended`, dan penyisipan atomik ke MMR.
- **Cross-Chain Spend Sanitation & Unified Lifecycle Monitor (DEC-056 — Chainlink CCIP v2):** Terverifikasi penuh (*IMPLEMENTED & VERIFIED*). Eliminasi dead inverted `_ccip_receive` di Stylus `spend.rs`, penutupan double-spend bypass di `batch_processor.rs` (enforce Groth16 proof & nullifier burn on-chain sebelum CCIP dispatch), sanitasi EVM client (`tokenAmounts`, `isChainSupported`, approval), client API CCIP v2 (`GET /v2/messages/{id}`), unified daemon lifecycle monitor (`ccip_monitor.rs`), database tabel `ccip_inbound_deposits`, dan endpoint REST `/api/v1/ccip/deposit/:id`, `/api/v1/ccip/spend/:id`, `/api/v1/ccip/status/:id`. 122 node tests passed, 85 contract tests passed, 26 Jacobian formal tests passed.


---

## 1. Kriptografi, Circuit Hardening & MPC Setup (Fase 2 — Fleksibilitas R&D Terbuka)
Ref: [`docs/roadmap.md`](file:///workspaces/Zeltra-Protocol/docs/roadmap.md) (Fase 2), [`research/dec/DEC-035`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-035-cryptographic-canonicality-public-input-binding-and-mmr.md), & [`research/dec/DEC-046`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-046-null-branch-witness-attack-defense-and-quadratic-encoding-soundness.md)

> [!NOTE]
> **Status Fleksibilitas:** Mengingat deploy Arbitrum sedang jeda/freeze (Oktober 2026), **sirkuit ZK BELUM DIKUNCI**. Tim bebas mengembangkan fitur inovatif (SBM Prover, MinMandate, iSimplePIR) dan memperkuat aljabar sirkuit tanpa terburu-buru melakukan *circuit freeze*.

- [ ] **Studi Komparasi Desain Privasi & Tooling Ekosistem (Fairblock, ZeroStyl, Renegade):**
  - **Fairblock:** Analisis arsitektur *conditional confidentiality* (FHE/t-IBE/decentralized MPC) untuk inspirasi batasan perlindungan transaksi privat Zeltra.
  - **ZeroStyl:** Evaluasi *tooling ZK di Arbitrum Stylus* untuk optimasi pipeline verifikasi dan interop host WASM.
  - **Renegade:** Analisis strategi produk & UX *dark pool MPC-ZK* dalam mengubah konsep privasi abstrak menjadi produk dengan use case nyata dan daya tarik pasar tinggi.

- [ ] **Circuit ABI & Constraints Freeze (Ditangguhkan hingga R&D Lengkap):**
  - Kunci mati parameter R1CS dan public-input ABI pada 16 PI (`PrivateNoteCircuit`) dan 19 PI (`JoinSplitCircuit`) ketika seluruh fitur siap ke mainnet.
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
