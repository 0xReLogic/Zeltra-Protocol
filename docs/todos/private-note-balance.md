# Private Note Balance dan Change Output

**Priority:** Tier 0 - Mainnet Blocker
**Status:** Gate A/B/C dibuka kembali setelah audit independen; Gate C0 security
repair wajib selesai sebelum Gate D.
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

## Audit Kondisi Sekarang

### Yang dapat dipertahankan

- [x] Threshold BLS blind-signing implementation.
- [x] Deposit confirmation dan reveal/refund lifecycle sebagai fondasi issuance.
- [x] EIP-2537 BLS and Groth16 precompile integration.
- [x] Persistent settlement queue dan receipt tracking.
- [x] Contract nullifier replay protection concept.
- [x] Relayer same-chain batching concept.
- [x] Fee helper integer rounding.
- [x] EIP-712 SDK primitives dan quote domain separation.

Item bertanda selesai di atas berarti komponennya reusable, bukan private balance
sudah selesai.

### Gap fatal yang ditemukan

- [ ] Tidak ada commitment tree untuk private notes.
- [ ] Tidak ada ownership proof atas remainder.
- [ ] Partial spend tidak membuat change note.
- [ ] `ComplianceCircuit` hanya constrain Poseidon nullifier; `root`, `recipient`,
  dan `amount` masih unconstrained.
- [ ] Legacy BLS nullifier berasal dari spend message, bukan note ownership key.
- [ ] Execution fee dicatat tetapi tidak ikut mengurangi user principal.
- [ ] Execution fee claim tidak memeriksa solvency invariant setelah transfer.
- [ ] Signed quote opsional pada endpoint spend.
- [ ] Contract tidak memverifikasi EIP-712 quote signature/quote ID.
- [ ] `batch_spend()` tidak membawa atau mengakumulasi execution fee per item.
- [ ] Node menyimpan `max_execution_fee` sebagai batch revenue, bukan exact
  `execution_fee`.
- [ ] `docs/bisnis.md` menyatakan persistent/pre-staged balance sudah ada,
  padahal belum diimplementasikan.

## Gate A - Bekukan Model dan Encoding

- [x] Review dan accept `DEC-016`.
- [x] Tetapkan scope MVP: public recipient payout + private sender change.
- [x] Tetapkan `PrivateNoteV1` fields.
- [x] Tetapkan domain separators untuk note, nullifier, proof, dan quote.
- [x] Tetapkan field packing, endianness, stablecoin decimals, dan max value.
- [x] Tetapkan Poseidon arity/composition dan parameter source.
- [x] Tetapkan Merkle tree depth, empty leaf, insertion algorithm, dan root history.
- [x] Tetapkan maksimum input/output notes per proof.
- [x] Tetapkan aturan zero-value note dan dust.
- [x] Tetapkan ownership/spending/nullifier key derivation.
- [x] Tetapkan hubungan BLS issuance authorization dengan initial note commitment.
- [x] Tetapkan migration policy deployment legacy.
- [ ] Tambahkan known-answer vectors dari minimal dua implementasi independen
  (bukan dua wrapper atas implementasi Rust yang sama) untuk:
  - note commitment;
  - nullifier;
  - parent hash;
  - Merkle root/path;
  - exact value conservation;
  - chain/contract domain separation.
- [ ] Independent design review: tidak ada jalur mint liability dari BLS dan ZK
  secara bersamaan.

**Gate A selesai jika:** format tidak ambigu dan dua implementasi independen
menghasilkan vector yang sama.

**Status: REOPENED** — format dasar dan implementasi Rust tersedia, tetapi Gate A
belum lulus sampai KAV cocok pada dua implementasi independen dan design review
selesai. Vector yang hanya dihasilkan oleh `nimbus-core` belum memenuhi syarat ini.

## Gate B - Perbaiki Accounting Sebelum Note Circuit

- [ ] Integrasikan storage/accounting berikut ke contract, bukan hanya model
  transisi murni di `nimbus-core`:
  - `user_note_liability`;
  - `refundable_deposit_liability`;
  - `accrued_execution_fee_liability`;
  - realized protocol fees.
- [x] Definisikan state transition table untuk deposit, refund, spend, fee accrue,
  fee claim, dan failed settlement.
- [ ] Saat spend contract, kurangi user liability sebesar payout + protocol fee +
  execution fee.
- [ ] Saat execution fee accrue di contract, tambah relayer liability dengan nominal sama.
- [ ] Saat claim di contract, kurangi contract assets dan relayer liability dengan nominal sama.
- [ ] Cek invariant on-chain setelah execution fee claim.
- [ ] Tolak claim on-chain yang menyentuh backing user/refund.
- [x] Gunakan exact `execution_fee`, bukan `max_execution_fee`, pada DB accounting.
- [x] Hilangkan floating-point dari accounting ekonomi dan margin keputusan.
- [ ] Tambahkan event terpisah untuk deposit fee, protocol fee, execution fee,
  change commitment, dan fee claim.
- [x] Buat property tests seluruh urutan state transition.
- [x] Tambahkan adversarial tests:
  - claim fee sebelum cukup accrual;
  - claim dua kali;
  - spend dan claim dalam block berdekatan;
  - rounding satu base unit;
  - fee overflow/underflow;
  - payout sukses tetapi fee transfer gagal.

**Gate B selesai jika:** untuk setiap generated transition sequence,
`assets >= all liabilities` dan tidak ada nilai yang hilang atau tercetak.

**Status: REOPENED** — 23 tests membuktikan model `ContractAccounting`, bukan
implementasi contract. Gate B baru selesai setelah storage dan seluruh entrypoint
contract memakai model liability terpisah serta invariant diuji pada contract.

## Gate C - Implementasi Private Note Circuit

### Core primitives

- [x] Implementasikan `PrivateNoteV1` dan canonical serialization di `nimbus-core`.
- [x] Implementasikan domain-separated note commitment.
- [x] Implementasikan owner-bound nullifier.
- [x] Implementasikan Poseidon incremental Merkle tree helper.
- [ ] Implementasikan note range and canonicality validation.
- [ ] Zeroize note secrets dan owner spending keys setelah penggunaan.

### Circuit (MVP: 1 input, 1 change output)

- [x] Buat circuit baru; jangan memperluas `ComplianceCircuit` lama secara diam-diam.
- [x] Constraint membership setiap input commitment.
- [x] Constraint owner spending authority.
- [x] Constraint nullifier derivation.
- [x] Constraint output commitment derivation.
- [x] Constraint exact value conservation.
- [x] Bind merchant payout.
- [x] Bind protocol fee.
- [x] Bind execution fee dan signed quote hash.
- [ ] Bind recipient, expiry, chain ID, contract address, asset ID, dan version (MVP: weak binding via Poseidon_W3, lihat DEC-016B).
- [ ] Range constrain seluruh amount sebelum arithmetic field.
- [ ] Tolak duplicate input note dalam transaksi yang sama (MVP: 1 input only).
- [ ] Tolak zero-value output (MVP: has_change not boolean-constrained, lihat DEC-016B).
- [ ] Tambahkan maximum input/output bounds (MVP: 1-in 1-out).
- [x] Audit semua public input benar-benar dipakai dalam constraint.

### Keys and artifacts

- [ ] Version circuit ID dan verifying key ID.
- [x] Generate development proving/verifying key sebagai artifact reproducible.
- [ ] Jangan generate proving key saat runtime.
- [ ] Tambahkan hash artifact ke manifest.
- [ ] Rencanakan MPC ceremony setelah circuit freeze, bukan sebelumnya (MVP: single-party seed, lihat DEC-016B).
- [ ] Independent circuit review sebelum trusted setup production.

**Gate C selesai jika:** proof palsu untuk membership, ownership, value, fee,
recipient, atau domain selalu gagal dan tidak mengubah state.

**MVP Status:** Prototype proof round-trip berjalan, tetapi Gate C belum selesai.
Audit menemukan blocker tambahan di luar DEC-016B. Selesaikan Gate C0 berikut
sebelum mulai Gate D.

## Gate C0 - Security Repair Sebelum Integrasi Contract

### P0 - One note pays exactly once

- [x] Bind bit arah Merkle path ke `input_leaf_index` di dalam circuit.
  - Dekomposisi indeks menjadi tepat `MERKLE_TREE_DEPTH` boolean bits.
  - Enforce `input_leaf_index == sum(bit_i * 2^i)`.
  - Enforce indeks `< 2^MERKLE_TREE_DEPTH`.
  - Gunakan bits yang sama untuk conditional left/right Merkle hashing.
- [x] Tambahkan regression test yang mencoba membership path valid dengan
  `input_leaf_index` berbeda.
- [x] Buktikan percobaan tersebut gagal saat proving atau verification.
- [x] Buktikan note yang sama tidak dapat menghasilkan dua nullifier valid hanya
  dengan mengganti leaf index.

### P0 - Integer value safety

- [x] Range constrain input value, merchant payout, protocol fee, execution fee,
  dan change ke `[0, 2^64)`.
- [x] Pastikan value conservation adalah integer USDC, bukan persamaan yang dapat
  wrap modulo scalar field.
- [x] Boolean-constrain `has_change` ke `{0, 1}`.
- [x] Enforce `has_change == 0` berarti `change_value == 0` dan output commitment
  nol.
- [x] Enforce `has_change == 1` berarti change memenuhi dust/minimum-value policy
  dan output commitment cocok.
- [x] Tambahkan negative tests untuk field overflow, modular wrap, non-boolean
  `has_change`, hidden positive change, dan zero-value change note.

### P0 - Payment and domain binding

- [ ] Ganti binding hash sementara yang hanya dihitung tanpa dibandingkan dengan
  nilai authoritative.
- [ ] Definisikan canonical signed quote digest yang mengikat minimal:
  recipient, merchant amount, protocol fee, exact/max execution fee, expiry,
  chain ID, contract address, asset ID, circuit version, dan quote ID/nonce.
- [ ] Enforce digest tersebut di circuit dan cocokkan dengan quote yang
  diverifikasi contract.
- [ ] Tambahkan negative test terpisah untuk setiap field yang ditukar atau
  dimodifikasi.

### P0 - Cryptographic parameters and setup

- [ ] Ganti parameter Poseidon width-5 ad-hoc berbasis `StdRng` dengan parameter
  standar/audited atau prosedur generation resmi yang terdokumentasi.
- [ ] Bekukan seluruh constants, matrix, round schedule, serialization, dan hash
  artifact dalam specification.
- [ ] Validasi hasilnya menggunakan implementasi referensi independen.
- [ ] Larang deterministic public-seed Groth16 setup pada production build.
- [ ] Simpan proving/verifying key sebagai artifact versioned; jangan generate
  saat runtime.
- [ ] Jalankan MPC ceremony hanya setelah seluruh constraint dan public-input ABI
  dibekukan serta direview.

### P1 - Engineering gate

- [ ] Cache development proving/verifying key pada tests agar setup tidak dibuat
  ulang untuk setiap test.
- [ ] `cargo fmt --all -- --check` lulus.
- [ ] `cargo clippy -p nimbus-core --all-targets -- -D warnings` lulus untuk
  perubahan private-note, dan warning legacy dicatat terpisah bila belum dapat
  dibersihkan dalam scope ini.
- [ ] Positive test dan minimal dua negative tests tersedia untuk setiap
  constraint kritis.

**Gate C0 selesai jika:** satu note tidak dapat menghasilkan lebih dari satu
spend valid, semua nilai menggunakan semantics integer bounded, seluruh payment
fields terikat ke signed quote/domain authoritative, dan production setup tidak
dapat direkonstruksi dari seed publik.

## Gate D - Contract Private Note Ledger

- [ ] Fresh deployment design; jangan ubah semantic storage deployment lama.
- [ ] Tambahkan append-only note commitment tree.
- [ ] Tambahkan bounded accepted-root history.
- [ ] Tambahkan namespaced private-note nullifier set.
- [ ] Deposit confirmed hanya dapat mint initial commitment sebesar net deposit.
- [ ] Bind initial note commitment ke deposit session sebelum reveal.
- [ ] Pastikan guardian/BLS quorum tidak dapat mint note tanpa collateral.
- [ ] Tambahkan private-note spend entrypoint dengan:
  - accepted root;
  - input nullifiers;
  - output commitments;
  - public payout;
  - exact fees;
  - quote binding;
  - ZK proof.
- [ ] Terapkan checks-effects-interactions.
- [ ] Insert nullifiers dan output commitments secara atomic.
- [ ] Transfer merchant exact payout.
- [ ] Accrue execution fee tanpa mengambil user change.
- [ ] Terapkan invariant aset terhadap semua liability.
- [ ] Full-balance spend tidak membuat zero note.
- [ ] Self-recipient spend berfungsi sebagai withdrawal.
- [ ] Pause policy tetap mengizinkan recovery/refund yang sudah valid.
- [ ] Emit event yang cukup untuk tree sync tanpa membuka change plaintext.
- [ ] Ukur Stylus WASM size dan gas untuk 1-in/1-out serta multi-input.

**Gate D selesai jika:** contract unit/property tests lulus dan ABI dibekukan untuk
SDK integration.

## Gate E - SDK Private Wallet State

- [ ] Ganti `AgentTokenPool` exact-amount voucher selection dengan note selection.
- [ ] Tampilkan jumlah notes sebagai satu saldo, bukan daftar voucher.
- [ ] Encrypted local note store.
- [ ] Backup/recovery format dengan version dan checksum.
- [ ] Note lifecycle: `unconfirmed -> unspent -> reserved -> spent`.
- [ ] Persist change note sebelum broadcast secara crash-safe.
- [ ] Roll back reservation hanya setelah chain reconciliation membuktikan spend gagal.
- [ ] Buat proof locally; jangan kirim note preimage/key ke relayer.
- [ ] Implementasikan coin selection dengan privacy-aware consolidation.
- [ ] Implementasikan:
  - `deposit(amount)`;
  - `pay(recipient, amount)`;
  - `send_to_wallet(recipient, amount)`;
  - `withdraw_all(owner_wallet)`.
- [ ] Quote UI selalu menampilkan merchant payout, protocol fee, maximum/exact
  execution fee, total debit, dan resulting balance.
- [ ] Peringatkan bahwa withdrawal ke public wallet membuka recipient dan amount.
- [ ] Multiple tabs/processes tidak boleh reserve note yang sama.

## Gate F - Node, Quote, dan Batch

- [ ] Signed quote wajib; field parsial atau quote kosong harus fail closed.
- [ ] Quote bind exact merchant amount, max execution fee, expiry, quote ID,
  relayer, chain, contract, dan circuit version.
- [ ] Tentukan apakah quote signature diverifikasi on-chain atau diikat ke ZK proof.
- [ ] Contract tidak boleh percaya `execution_fee <= max` tanpa bukti user consent.
- [ ] Batch ABI membawa exact execution fee dan quote binding per item.
- [ ] Batch tidak boleh menghilangkan output commitment/change.
- [ ] Satu item invalid membuat seluruh on-chain batch revert tanpa partial state.
- [ ] DB menyimpan input nullifiers dan output commitments secara atomic.
- [ ] Receipt confirmation mengubah input `reserved -> spent` dan output
  `unconfirmed -> unspent`.
- [ ] Reorg mengembalikan note state secara konsisten.
- [ ] Claim worker reconcile contract accrual, DB accrual, dan receipts.
- [ ] Hilangkan fallback quote yang mengembalikan status OK tanpa signing domain.
- [ ] Health endpoint expose solvency/accounting mismatch tanpa membuka user data.

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

## Referensi

- Zcash Protocol Specification:
  https://zips.z.cash/protocol/protocol.pdf
- ZIP 224 Orchard:
  https://zips.z.cash/zip-0224
- Aztec private state and notes:
  https://docs.aztec.network/developers/docs/foundational-topics/state_management
- RAILGUN private UTXO model:
  https://docs.railgun.org/wiki/learn/using-private-tokens
- Nimbus decision:
  `research/decisions/DEC-016-private-note-change-ledger.md`
