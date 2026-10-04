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

## Gap Fatal yang Masih Pending

- [ ] `ComplianceCircuit` hanya constrain Poseidon nullifier; `root`, `recipient`, dan `amount` masih unconstrained.
- [ ] Execution fee dicatat tetapi tidak ikut mengurangi user principal.
- [ ] Signed quote opsional pada endpoint spend.
- [ ] Contract tidak memverifikasi EIP-712 quote signature/quote ID.
- [ ] `batch_spend()` tidak membawa atau mengakumulasi execution fee per item.
- [ ] Node menyimpan `max_execution_fee` sebagai batch revenue, bukan exact `execution_fee`.

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

### Core primitives
- [ ] Implementasikan note range and canonicality validation.
- [ ] Zeroize note secrets dan owner spending keys setelah penggunaan.

### Keys and artifacts
- [ ] Version circuit ID dan verifying key ID.
- [ ] Jangan generate proving key saat runtime.
- [ ] Tambahkan hash artifact ke manifest.
- [ ] Rencanakan MPC ceremony setelah circuit freeze, bukan sebelumnya (MVP: single-party seed, lihat DEC-016B).
- [ ] Independent circuit review sebelum trusted setup production.

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

> **Status Gate F Phase 1 (Direct Settlement Pipeline):** **SELESAI (100%)** — Sesuai [`DEC-025`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-025-relayer-zk-note-spend-settlement-atomic-batching-and-reconciliation.md), relayer telah mengimplementasikan pipeline validasi fail-closed, endpoint ingress `/api/v1/spend-private-note`, pre-flight kanonikalitas 12 skalar (`from_evm_scalar`), semantic public input binding (DEC-022), single-item direct settlement dispatcher `EvmClient::broadcast_spend_private_note_transaction`, pencegahan double-spend on-chain (`is_nullifier_spent`) & SQLite, durable enqueue, idempotensi 24h, dan receipt confirmation tracking (91/91 node tests pass).

### Pending Tasks Gate F:
- [ ] Claim worker reconcile contract accrual, DB accrual, dan receipts.
- [ ] Hilangkan fallback quote yang mengembalikan status OK tanpa signing domain.
- [ ] Health endpoint expose solvency/accounting mismatch tanpa membuka user data.

---

## Gate G - Hard Test Arbitrum Sepolia

- [ ] Deposit 100 USDC menghasilkan net note exact setelah 0.20% fee.
- [ ] Spend 5 USDC membayar merchant tepat 5 USDC.
- [ ] Protocol fee dan execution fee cocok dengan signed quote.
- [ ] Change note sama dengan input dikurangi seluruh debit.
- [ ] Spend kedua memakai change note, bukan input lama.
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
