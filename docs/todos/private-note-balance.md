# Private Note Balance dan Change Output

**Priority:** Tier 0 - Mainnet Blocker  
**Catatan:** Seluruh checklist yang sudah selesai (`[x]`) telah dihapus agar dokumen ini murni berfokus pada pekerjaan pending (`[ ]`).  
**Decision:** [`research/decisions/DEC-016-private-note-change-ledger.md`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-016-private-note-change-ledger.md) | **Frozen Spec:** [`research/decisions/DEC-016A-private-note-spec-freeze.md`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-016A-private-note-spec-freeze.md) | **Audit Shortcuts:** [`research/decisions/DEC-016B-mvp-circuit-shortcuts.md`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-016B-mvp-circuit-shortcuts.md)

## Stop Rule

Jangan lanjut mengaktifkan monetisasi, mainnet deployment, execution-fee claim,
atau menambah product flow baru sebelum bagian **Gate A** sampai **Gate D** dalam
file ini selesai.

Testnet deployment lama boleh dipakai untuk regression legacy. Jangan menganggap
hasil legacy sebagai bukti private balance baru sudah aman.

## Tujuan Produk

User dapat deposit nominal bebas via direct MMR deposit (`deposit_direct_note`), melakukan pembayaran berkali-kali (1-in-1-out atau 2-in-2-out JoinSplit), dan selalu memiliki hak atas seluruh sisa dana via change note.

```text
net deposit
  = merchant payouts
  + protocol fees (flat 45 bps)
  + execution fees (gas reimbursement + 15% markup)
  + current unspent private notes
```

Tidak ada fungsi contract `withdraw()` terpisah. Withdrawal adalah `spend()` ke
wallet publik milik user.

---

## Gap Fatal yang Telah Dituntaskan (Sprint DEC-016 s/d DEC-038)

Seluruh gap fatal legacy berikut telah **dituntaskan 100%**:
- **Pemusnahan Legacy Threshold BLS (DEC-038A):** Seluruh kode blind signing, guardian network, session storage (11 storage slot kontrak), dan alur refund timelock 24 jam telah dihapus bersih. Arsitektur kini 100% murni ZK-UTXO.
- **Direct MMR Deposit (DEC-038B/C):** Deposit langsung menyisipkan komitmen note ke Merkle Mountain Range (`_mmr_insert`) dengan 96-byte AEAD encrypted memo (`NoteCommitmentAppended`, DEC-037).
- **Akumulator MMR Skalabel (DEC-032):** Menggantikan batas kaku LeanIMT depth 20 dengan infinite-horizon Merkle Mountain Range (MMR) berbasis canonical peak bagging backward fold (`DOMAIN_MMR_BAG`) dan proteksi anti-Hyperbridge ($leaf\_index < leaf\_count$).
- **Generational Nullifiers & Rollover (DEC-033):** 2-Epoch Generational Window ($E$ dan $E-1$) untuk pencegahan storage bloat nullifier pada Stylus contract.
- **Two-Limb Quote Hash & Scope Binding (DEC-035A/B):** 128-bit limb range checks ($H_{\text{hi}}, H_{\text{lo}} < 2^{128}$) dan Formal Scope Binding Gadget terintegrasi di `PrivateNoteCircuit` (16 public inputs).
- **Universal 2-in-2-out JoinSplit (DEC-036A/B/C):** Topology universal konstan (19 public inputs, 448 quadratic range gates), Knapsack coin selection, dan in-pool autonomous consolidation.
- **Viewing Key Recovery (DEC-037):** Derivasi kunci HKDF (`IncomingViewingKey`, `OutcomingViewingKey`) dan trial decryption saldo dari memo on-chain (`recover_notes`).
- **Solusi Compliance & Sanctions:** Receiver-Enforced ASP & Layer-1/HTTP OFAC screening fail-closed ([`DEC-026`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-026-receiver-enforced-compliance-zero-cost-sanctions-filtering-relayer-protection.md)).
- **Solusi Reconcile & Solvensi:** Relayer Three-Way Reconciliation engine mencocokkan exact fee on-chain vs DB vs receipts ([`DEC-031`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-031-relayer-reconciliation-solvency-observability-quote-domain-binding.md)).

---

## Gate A - Model dan Encoding ZK-UTXO

> **Status Gate A Primitives:** **SELESAI (100%)** — Model Rust, domain separation (Poseidon Grain-128 W3/W5, DEC-021), note commitment, nullifier derivation, dan Merkle Mountain Range (MMR, DEC-032) telah teruji dan diverifikasi secara identik di `zeltra-core` dan `zeltra-contracts`.

---

## Gate B - Accounting & Solvensi Multi-Liability

> **Status Gate B:** **SELESAI (100%)** — Storage akuntansi solvensi (`user_note_liability`, `accrued_execution_fee_liability`, `realized_protocol_fees`), checks-effects-interactions, dan solvency invariant mutlak on-chain (`USDC balance >= total_liabilities`) telah diverifikasi di Stylus contract dan core accounting.

---

## Gate C - Implementasi ZK Circuits (Groth16)

### Keys and artifacts (Pending)
- [ ] Simpan proving/verifying key sebagai artifact versioned; cache development key pada test runner.
- [ ] Rencanakan MPC trusted setup ceremony setelah circuit constraints dan public ABI dibekukan.
- [ ] Independent circuit security review sebelum trusted setup production.

---

## Gate C0 - Security Repair & Range Constraints

> **Status Gate C0:** **SELESAI (100%)** — Range constraints 64-bit integer, boolean constraint `has_change`, two-limb quote hash 128-bit range checks, formal scope binding gadget, dan parameter Poseidon Grain-128 LFSR ([`DEC-021`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-021-audited-poseidon-parameters-grain-lfsr-defense.md)) telah lolos test suite formal.

- [ ] Jalankan MPC ceremony hanya setelah seluruh constraint dan public-input ABI dibekukan serta direview.

---

## Gate D - Stylus Contract Note Ledger & Verifiers

> **Status Gate D:** **SELESAI (100%)** — Verifier Groth16 BLS12-381 via EIP-2537 (`0x0c` MSM + `0x0f` Pairing Check) untuk `PrivateNoteCircuit` (16 public inputs) dan `JoinSplitCircuit` (19 public inputs) telah terpasang di Stylus (`zeltra-contracts`). Didukung dengan on-chain Merkle Mountain Range (`_mmr_insert`), namespaced note nullifiers, generational nullifier slots, dan multi-liability solvency invariant.

### Pipeline Verifikasi Groth16 BLS12-381 On-Chain:
- **Proof format (512 bytes):**
  - $A \in \mathbb{G}_1$ (128 bytes EVM uncompressed: X, Y)
  - $B \in \mathbb{G}_2$ (256 bytes EVM uncompressed: X1, X2, Y1, Y2)
  - $C \in \mathbb{G}_1$ (128 bytes EVM uncompressed: X, Y)
- **Public Inputs:**
  - `PrivateNoteCircuit` (16 scalars): `[note_root, leaf_count, input_nullifier, note_epoch_id, output_commitment, recipient, merchant_amount, protocol_fee, execution_fee, quote_hash_hi, quote_hash_lo, chain_id, contract_address, expiry, has_change, is_rollover]`
  - `JoinSplitCircuit` (19 scalars): `[note_root, leaf_count, input_nullifier_1, input_nullifier_2, epoch_id_1, epoch_id_2, output_commitment_1, output_commitment_2, recipient, merchant_amount, protocol_fee, execution_fee, quote_hash_hi, quote_hash_lo, chain_id, contract_address, expiry, is_dummy_2, session_nonce]`
- **Dua Tahap Precompile EIP-2537:**
  1. `0x0c` (`BLS12_G1MSM`): Hitung linear combination $\mathcal{L} = \text{IC}_0 + \sum_{i=1}^{k} x_i \text{IC}_i$ dalam 1 batch call host.
  2. `0x0f` (`BLS12_PAIRING_CHECK`): Evaluasi 4-pairing equation dalam 1 call buffer 1536 bytes:
     $$e(-A, B) \cdot e(\alpha, \beta) \cdot e(\mathcal{L}, \gamma) \cdot e(C, \delta) == 1$$

---

## Gate E - SDK Private Wallet State

> **Status Gate E:** **SELESAI (100%)** — `PrivateNoteWallet` (`zeltra-sdk/src/wallet/note_wallet.rs`) mendukung direct deposit prep (Poseidon + AEAD memo), encrypted note store (DEC-024), versioned backup dengan HMAC-SHA256, siklus hidup note 4-tahap (`PendingDeposit -> Unspent -> Reserved -> Spent`), crash-safe two-phase commit, local Groth16 proof generation (`prepare_spend_proof`, `prepare_joinsplit_proof`), stochastic knapsack coin selection, autonomous note consolidation (`consolidate_notes`), dan viewing key trial decryption (`recover_notes`).

### Fitur Ekosistem Lanjutan (Fase 3):
- [ ] **Shielded-to-Shielded In-Pool Transfer:** Pengiriman private note langsung antar akun/merchant Zeltra (menggunakan `output_commitment_1` untuk penerima dan `output_commitment_2` untuk change note) tanpa penarikan USDC publik on-chain, menjaga omset merchant 100% terlindungi (Merchant Masking).

---

## Gate F - Node, Quote, dan Settlement

> **Status Gate F:** **SELESAI (100% Full Pipeline & Solvency Observability)** — Stateless sequencer relayer (`zeltra-node`) dengan validasi fail-closed, endpoint `/api/v1/spend-private-note`, `/api/v1/spend-joinsplit`, MMR dynamic sync (`/api/v1/mmr/proof/:leaf_index`), preflight Groth16 verifier lokal (DEC-026 C-01), dual-layer double-spend guard (`is_nullifier_spent`), direct settlement dispatcher, three-way reconciliation engine (`reconcile_execution_fees`), zero-leakage solvency observability di endpoint `/health`, mempool lease watchdog worker, MMR event indexer worker, serta dedicated `/live` dan `/ready` health probes.

---

## Gate G - Hard Test Arbitrum Sepolia

- [ ] Deposit 5 USDC menghasilkan net note exact 5 USDC (0 bps / 0.00% deposit fee).
- [ ] Spend 1 USDC membayar merchant tepat 1 USDC.
- [ ] Protocol fee dan execution fee cocok dengan signed quote.
- [ ] Change note sama dengan input (5 USDC) dikurangi seluruh debit (1 USDC merchant + 45 bps protocol fee + execution fee).
- [ ] Spend kedua memakai change note (~3.9 USDC), bukan input lama.
- [ ] Replay input pertama gagal dan seluruh state/balance tidak berubah.
- [ ] Withdraw-all via spend ke wallet owner mengembalikan seluruh sisa setelah fee.
- [ ] Final user note liability nol.
- [ ] Tidak ada orphan USDC di luar recognized liabilities/fees.
- [ ] Relayer claim exact execution fee.
- [ ] Claim kedua gagal tanpa state change.
- [ ] Batch 2/4/8 mempertahankan exact conservation setiap user.
- [ ] Satu proof invalid dalam batch tidak menghasilkan payout/change/nullifier.
- [ ] Crash pada setiap boundary:
  - sebelum enqueue;
  - setelah reserve note;
  - setelah proof generation;
  - setelah broadcast;
  - setelah receipt;
  - sebelum local output-note commit.
- [ ] Reorg dan replacement transaction tidak menyebabkan note ganda.
- [ ] Wrong owner key gagal.
- [ ] Wrong Merkle path/root gagal.
- [ ] Wrong recipient, amount, fee, quote, chain, atau contract gagal.
- [ ] Forged change lebih besar/kecil satu base unit gagal.
- [ ] Concurrent spend note yang sama: hanya satu berhasil.

---

## Mainnet Terminal Condition & Master Roadmap Alignment

Semua syarat berikut wajib sebelum rilis production:

### 1. Gate A-G & Testnet Arbitrum Sepolia
- [ ] Gate A-G selesai dan diverifikasi di testnet Arbitrum Sepolia.
- [ ] `DEC-016` status Accepted.
- [ ] Deposit -> repeated partial spends -> withdraw-all lulus testnet.
- [ ] Contract balance reconcile exact terhadap seluruh liability.
- [ ] Cross-chain CCIP testnet flow (source -> router -> destination -> confirmation).
- [ ] Testing batas idempotensi & crash-recovery relayer node.

### 2. Kriptografi & Trusted Setup (Gate C)
- [ ] Freeze ABI sirkuit 16 PIs & 19 PIs beserta batasan constraints.
- [ ] Production proving/verifying artifacts (keys) selesai dan versioned.
- [ ] Persiapan dan pelaksanaan Trusted Setup Ceremony (MPC Phase 2).
- [ ] Circuit dan accounting mendapat independent security review (Audit Tier-1: Zellic / Trail of Bits).

### 3. Ekosistem SDK, Kepatuhan & AI Commerce (Fase 3 Blueprint)
- [ ] **Shielded-to-Shielded In-Pool Transfer:** Pengiriman private note langsung antar akun/merchant Zeltra (menggunakan `output_commitment_1` untuk penerima dan `output_commitment_2` untuk change note) tanpa penarikan USDC publik on-chain (Merchant Masking).
- [ ] **Viewing Key Export Suite:** Tooling UI/CLI bagi akuntan B2B untuk export laporan audit SPT/pajak berbasis `ovk`.
- [ ] **SDK Helper CEX:** Helper otomatis "Export Source of Funds" buat bypass freeze bursa (Binance/Coinbase).
- [ ] **In-Browser WebAssembly SDK Prover:** Runner prover Groth16 langsung di browser/mobile via WASM web worker tanpa backend prover.
- [ ] **Integrasi x402 Facilitator:** Jalur streaming payment mikro HTTP 402 untuk AI Agent.

### 4. Mainnet Go-Live & Pilot (Fase 4 Blueprint)
- [ ] Program Bug Bounty Publik di Arbitrum One.
- [ ] Pilot Payroll B2B & Kemitraan Merchant Commerce.
- [ ] Deployment baru, ABI, contract address, dan circuit version terdokumentasi.
- [ ] Tidak ada legacy user balance yang dimigrasikan tanpa ownership proof.
