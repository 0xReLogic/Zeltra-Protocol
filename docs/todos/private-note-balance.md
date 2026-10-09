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

User dapat deposit nominal bebas, melakukan pembayaran berkali-kali, dan selalu
memiliki hak atas seluruh sisa dana.

```text
net deposit
  = merchant payouts
  + protocol fees
  + execution fees
  + current unspent private notes
  + valid refunds
```

Tidak ada fungsi contract `withdraw()` terpisah. Withdrawal adalah `spend()` ke
wallet publik milik user.

---

## Gap Fatal yang Telah Dituntaskan (Sprint DEC-016 s/d DEC-031)

Seluruh 7 gap fatal legacy berikut telah **dituntaskan 100%**:
- **Solusi Compliance:** `ComplianceCircuit` unconstrained telah di-deprecate dan digantikan oleh Receiver-Enforced ASP & Layer-1/HTTP OFAC screening ([`DEC-026`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-026-receiver-enforced-compliance-zero-cost-sanctions-filtering-relayer-protection.md)).
- **Solusi Principal & Fees:** Execution fee mengurangi user principal secara tepat dan solven lewat multi-liability accounting ([`DEC-019`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-019-merkle-tree-sizing-and-solvency-invariants.md), Gate B & D).
- **Solusi Quote Enforce:** Signed quote diwajibkan *fail-closed* di endpoint spend ([`DEC-025`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-025-relayer-zk-note-spend-settlement-atomic-batching-and-reconciliation.md)).
- **Solusi Domain Binding:** Validasi domain EIP-712 terikat ketat ke contract address & chain ID ([`DEC-031`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-031-relayer-reconciliation-solvency-observability-quote-domain-binding.md)).
- **Solusi Batch Execution Fee:** Akumulasi execution fee per item terintegrasi di `batch_spend()` & single settlement.
- **Solusi Reconcile:** Relayer Three-Way Reconciliation mencatat exact `execution_fee` on-chain vs DB vs receipts ([`DEC-031`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-031-relayer-reconciliation-solvency-observability-quote-domain-binding.md)).
- **Solusi UTXO Fragmentation & Consolidation:** Universal 2-in-2-out JoinSplit Groth16 circuit, Tiered Stochastic Knapsack coin selection, dan in-pool autonomous consolidation ([`DEC-030`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-030-multi-utxo-joinsplit-coin-selection-zeroize-and-aead-backup.md)).

---

## Gate A - Bekukan Model dan Encoding

> **Status Gate A Primitives:** Dasar encoding dan model Rust telah disepakati.

- [ ] Tambahkan known-answer vectors dari minimal dua implementasi independen (bukan dua wrapper atas implementasi Rust yang sama) untuk:
  - note commitment;
  - nullifier;
  - parent hash;
  - Merkle root/path;
  - exact value conservation;
  - chain/contract domain separation.
- [ ] Independent design review: tidak ada jalur mint liability dari BLS dan ZK secara bersamaan.

---

## Gate B - Perbaiki Accounting Sebelum Note Circuit

> **Status Gate B:** **SELESAI (100%)** — Storage multi-liability (`user_note_liability`, `refundable_deposit_liability`, `accrued_execution_fee_liability`), checks-effects-interactions, solvency check on-chain (`assets >= total_liabilities`), dan event logging telah diimplementasikan dan diverifikasi dengan 45/45 test di Stylus contract dan 23/23 test di core accounting.

---

## Gate C - Implementasi Private Note Circuit

### Keys and artifacts (Pending)
- [ ] Version circuit ID dan verifying key ID.
- [ ] Jangan generate proving key saat runtime (bundle static precompiled artifacts).
- [ ] Tambahkan hash artifact ke manifest rilis.
- [ ] Rencanakan MPC ceremony setelah circuit freeze (MVP: single-party seed per DEC-016B).
- [ ] Independent circuit security review sebelum trusted setup production.

---

## Gate C0 - Security Repair Sebelum Integrasi Contract

> **Status Gate C0:** **SELESAI (100%)** — Seluruh 3 blocker keamanan P0 (leaf index Merkle binding, 64-bit integer range gadget, boolean constraint `has_change`) dan negative test suite telah lulus 100%. Parameter Poseidon Grain-128 LFSR terimplementasi di [`nimbus-core/src/poseidon.rs`](file:///workspaces/Zeltra-Protocol/nimbus-core/src/poseidon.rs) ([`DEC-021`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-021-audited-poseidon-parameters-grain-lfsr-defense.md)) dan divalidasi dengan implementasi referensi independen.

- [ ] Jalankan MPC ceremony hanya setelah seluruh constraint dan public-input ABI dibekukan serta direview.

---

## Gate D - Contract Private Note Ledger

> **Status Gate D:** **SELESAI (100%)** — Module verifier Groth16 BLS12-381 via EIP-2537 (`0x0c` MSM + `0x0f` Pairing Check) dan entrypoint `spend_private_note(...)` telah diimplementasikan di Stylus (`nimbus-contracts`). Didukung dengan pohon LeanIMT depth 20 (`_merkle_insert`), namespaced note nullifiers, multi-liability invariant, Stylus WASM target check lulus, serta 53/53 tests passing (termasuk 8 positive & negative tests untuk ZK private note spend).

### Referensi Repositori Kloning / ATM (Amati, Tiru, Modifikasi):
1. **[`https://github.com/supernovahs/zk-sunade`](https://github.com/supernovahs/zk-sunade):**
   - *Apa yang diamati & ditiru:* Pola verifier Groth16 di Stylus SDK menggunakan `RawCall::new_static` ke host precompile tanpa memasukkan full arkworks library ke contract WASM. Menghasilkan binary super lean (<25 KB compressed) dan gas ~250k.
   - *Modifikasi untuk Nimbus:* `zk-sunade` menggunakan BN254 (`0x06`, `0x07`, `0x08`). Di Nimbus kita adaptasi ke **EIP-2537 BLS12-381** (`0x0c` MSM dan `0x0f` Pairing Check) yang sudah terbukti lolos 13/13 testnet tests di Sepolia.
2. **[`https://github.com/Railgun-Privacy/contract`](https://github.com/Railgun-Privacy/contract):**
   - *Apa yang diamati & ditiru:* Smart contract Railgun: struktur `Commitments.sol` (accumulator UTXO, nullifier mapping, root history ring buffer), verifier integration, dan token transfer logic.
   - *Modifikasi untuk Nimbus:* Sirkuit Nimbus jauh lebih ramping (fokus 1-in / 2-out: payout + optional change note), menghindari kompleksitas 54 circuit terpisah ala Railgun.
3. **[`https://github.com/zk-kit/zk-kit`](https://github.com/zk-kit/zk-kit):**
   - *Apa yang diamati & ditiru:* Monorepo resmi Privacy & Scaling Explorations (PSE) Ethereum Foundation untuk reusable ZK libraries (`lean-imt.sol`, `imt.sol`, dan parameter hashing Poseidon ter-audit).

### Pipeline Verifikasi Groth16 BLS12-381 On-Chain:
- **Proof format (512 bytes):**
  - $A \in \mathbb{G}_1$ (128 bytes EVM uncompressed: X, Y)
  - $B \in \mathbb{G}_2$ (256 bytes EVM uncompressed: X1, X2, Y1, Y2)
  - $C \in \mathbb{G}_1$ (128 bytes EVM uncompressed: X, Y)
- **Public inputs (12 field elements $\in \mathbb{F}_r$, masing-masing 32 bytes):**
  `[merkle_root, input_nullifier, output_commitment, recipient, merchant_amount, protocol_fee, execution_fee, quote_hash, chain_id, contract_address, expiry, has_change]`
- **Dua Tahap Precompile EIP-2537:**
  1. `0x0c` (`BLS12_G1MSM`): Hitung linear combination $\mathcal{L} = \text{IC}_0 + \sum_{i=1}^{12} x_i \text{IC}_i$ dalam 1 batch call host.
  2. `0x0f` (`BLS12_PAIRING_CHECK`): Evaluasi 4-pairing equation dalam 1 call buffer 1536 bytes:
     $$e(-A, B) \cdot e(\alpha, \beta) \cdot e(\mathcal{L}, \gamma) \cdot e(C, \delta) == 1$$

---

## Gate E - SDK Private Wallet State

> **Status Gate E:** **SELESAI (100%)** — `PrivateNoteWallet` (`nimbus-sdk/src/wallet/note_wallet.rs`) menggantikan voucher exact-amount dengan model saldo terpadu note UTXO. Mendukung encrypted note store (DEC-024), versioned backup dengan HMAC-SHA256, siklus hidup note 4-tahap (`unconfirmed -> unspent -> reserved -> spent`), crash-safe two-phase commit, local Groth16 proof generation (`prepare_spend_proof`), privacy-aware coin selection, multi-tab lease reservation (120s), dan 20/20 test suite pass.

---

## Gate F - Node, Quote, dan Batch

> **Status Gate F:** **SELESAI (100% Full Pipeline & Solvency Observability)** — Sesuai [`DEC-025`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-025-relayer-zk-note-spend-settlement-atomic-batching-and-reconciliation.md) & [`DEC-031`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-031-relayer-reconciliation-solvency-observability-quote-domain-binding.md), relayer telah mengimplementasikan pipeline validasi fail-closed, endpoint ingress `/api/v1/spend-private-note`, pre-flight kanonikalitas 12 skalar, semantic public input binding, dual-layer double-spend guard (`is_nullifier_spent`), single-item & batch settlement dispatcher, three-way reconciliation engine (`reconcile_execution_fees`), pemusnahan total fallback quote berstatus OK tanpa signing domain, zero-leakage solvency observability di endpoint `/health`, pooling TLS client dengan strict timeouts (`http.rs`), periodic rate-limit pruner anti memory-leak, serta dedicated `/live` dan `/ready` health probes (147/147 node tests pass, clippy clean).

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

## Mainnet Terminal Condition

Semua syarat berikut wajib:

- [ ] Gate A-G selesai.
- [ ] `DEC-016` status Accepted.
- [ ] Circuit dan accounting mendapat independent security review.
- [ ] Production proving/verifying artifacts selesai dan diverifikasi.
- [ ] Deposit -> repeated partial spends -> withdraw-all lulus testnet.
- [ ] Contract balance reconcile exact terhadap seluruh liability.
- [ ] Tidak ada legacy user balance yang dimigrasikan tanpa ownership proof.
- [ ] Deployment baru, ABI, contract address, and circuit version terdokumentasi.
