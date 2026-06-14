# DEC-016: Private Note Ledger dan Change Output

Date: 2026-06-14
Status: Proposed - blocks mainnet and supersedes the reusable-voucher assumption
Frozen spec: DEC-016A (research/decisions/DEC-016A-private-note-spec-freeze.md)

## Masalah

Kontrak Nimbus saat ini menyimpan deposit sebagai liability global
`total_deposited_principal`, lalu membolehkan satu BLS credential membayar satu
nominal yang sudah terikat pada message. Model ini belum mempunyai representasi
on-chain yang membuktikan hak pengguna atas sisa deposit setelah partial spend.

Contoh:

```text
gross deposit         = 100.000000 USDC
deposit fee 0.20%     =   0.200000 USDC
net private value     =  99.800000 USDC

merchant payout       =   5.000000 USDC
protocol fee          =   0.012500 USDC
execution fee         =   0.023000 USDC
remaining user value  =  94.764500 USDC
```

Setelah credential lama dipakai, nilai `94.764500` masih berada dalam pool,
tetapi kontrak tidak mempunyai commitment atau ownership proof yang membuat
pengguna dapat membelanjakan atau menarik nilai tersebut. Dana ada secara fisik,
namun hak klaimnya tidak direpresentasikan dengan aman.

Masalah kedua adalah accounting execution fee. Jalur spend saat ini menambah
`accumulated_execution_fees`, tetapi hanya mengurangi principal sebesar merchant
payout dan protocol fee. Claim execution fee kemudian mengeluarkan aset tanpa
mengurangi user liability. Ini dapat membuat:

```text
contract assets < user liabilities + accrued relayer liabilities
```

Masalah ketiga adalah signed quote masih opsional pada node dan tidak diverifikasi
sepenuhnya oleh kontrak. Jalur batch juga tidak membawa execution fee per item.

## Product Invariant

Nimbus adalah private payment account, bukan voucher yang menyita remainder.

Setiap unit stablecoin setelah fee harus selalu berada tepat di salah satu state:

1. user-owned unspent private note;
2. merchant payout yang sudah diselesaikan;
3. protocol fee yang sudah diakui;
4. accrued execution fee milik relayer;
5. refundable unresolved deposit.

Tidak boleh ada dana tanpa pemilik atau liability tanpa aset pendukung.

## Security Invariants

### Value conservation

Untuk setiap private spend:

```text
sum(input_note_values)
  = merchant_payout
  + protocol_fee
  + execution_fee
  + sum(output_change_values)
```

Semua operasi menggunakan integer base units stablecoin dan checked arithmetic.
Tidak ada toleransi pembulatan tersembunyi.

### Note lifecycle

- Setiap input note hanya dapat dikonsumsi sekali.
- Konsumsi note menghasilkan nullifier yang unik dan unlinkable tanpa owner key.
- Partial spend wajib menghasilkan change note baru.
- Full-balance spend menghasilkan change nol dan tidak membuat zero-value note.
- Output note memakai owner key, randomness, dan commitment baru.
- Contract tidak menyimpan plaintext owner atau plaintext change value.

### Ownership

- Prover harus membuktikan mengetahui spending/nullifier key input note.
- Prover harus membuktikan input commitment merupakan anggota note tree.
- Output change commitment harus terikat ke owner key yang dipilih prover.
- Relayer, guardian, dan contract tidak boleh dapat mengambil change note.

### Accounting

Definisi liability dipisahkan:

```text
user_note_liability
accrued_execution_fee_liability
refundable_deposit_liability
```

Invariant solvency:

```text
contract USDC balance
  >= user_note_liability
   + accrued_execution_fee_liability
   + refundable_deposit_liability
```

Saat spend diterima, user liability turun sebesar:

```text
merchant payout + protocol fee + execution fee
```

Execution fee tetap berada di kontrak sebagai relayer liability sampai diklaim.
Saat claim, aset dan relayer liability turun dengan nominal yang sama.

## Pilihan yang Dipertimbangkan

### 1. Credential BLS reusable

Ditolak. Memakai credential atau nullifier yang sama lebih dari sekali merusak
replay protection dan membuat transaksi dapat ditautkan. Menambah counter publik
tidak menyelesaikan ownership maupun privacy.

### 2. Auto-split fixed denomination vouchers

Ditolak sebagai arsitektur utama. Denomination mengurangi remainder rata-rata,
tetapi tidak memberikan exact change, memperbesar jumlah credential, dan tetap
membutuhkan mekanisme aman untuk mengikat setiap voucher ke collateral.

### 3. Refund remainder berdasarkan session ID

Ditolak. Setelah spend anonim, kontrak tidak dapat menghubungkan nullifier ke
session depositor tanpa merusak privacy. Binding yang lemah juga dapat membuka
double claim: spend credential lalu claim remainder yang salah.

### 4. Public account balance per address

Ditolak. Implementasinya sederhana tetapi membuat deposit, seluruh spend, dan
withdraw mudah dikorelasikan melalui address dan perubahan balance.

### 5. Private note ledger dengan change output

Diterima. Setiap state privat adalah note sekali pakai. Update dilakukan dengan
menullify input note dan membuat output commitment baru. SDK menampilkan jumlah
notes sebagai satu saldo sehingga user tidak melihat kompleksitas UTXO.

## Keputusan

Nimbus mengadopsi **private note ledger minimal**:

```text
deposit -> mint private note commitment

private spend:
  consume input note(s)
  publish input nullifier(s)
  pay public recipient
  account protocol and execution fees
  create private change note commitment(s)
```

`spend()` tetap menjadi satu-satunya primitive untuk:

- pembayaran merchant;
- transfer ke wallet lain;
- withdrawal/unshield ke wallet user sendiri.

Tidak ditambahkan entrypoint `withdraw()` terpisah. SDK boleh menampilkan
`withdraw_all()` sebagai helper yang memanggil primitive spend dengan wallet user
sebagai recipient.

## Peran BLS Setelah Perubahan

Threshold BLS tidak dibuang. Perannya diubah menjadi authorization pada fase
issuance/shielding, bukan sebagai reusable balance state.

BLS dapat membuktikan bahwa quorum guardian menyetujui issuance note yang:

- berasal dari deposit yang confirmed;
- memakai asset dan chain yang benar;
- tidak melebihi net deposit;
- lolos policy/compliance issuance.

Hak membelanjakan note setelah issuance dibuktikan oleh note ownership proof dan
ZK value-conservation proof. BLS signature exact-spend lama menjadi legacy path
dan tidak boleh menjadi authority untuk private-note spend baru.

Pemisahan ini mempertahankan manfaat blind threshold issuance tanpa memaksa
guardian online untuk menandatangani setiap change.

## Bentuk Note V1

Format final harus dibekukan melalui test vector. Kandidat konseptual:

```text
PrivateNoteV1 {
    version,
    asset_id,
    value,
    owner_spend_key,
    rho,
    randomness,
}

commitment = Poseidon(
    domain_note_v1,
    chain_id,
    contract_address,
    asset_id,
    value,
    owner_spend_key,
    rho,
    randomness
)

nullifier = Poseidon(
    domain_nullifier_v1,
    owner_nullifier_key,
    commitment,
    leaf_index
)
```

Catatan:

- Domain, encoding, field packing, tree depth, and endianness harus normatif.
- `asset_id` wajib ada walaupun MVP hanya USDC, untuk mencegah cross-asset reuse.
- `chain_id` dan contract address mencegah cross-chain/cross-contract replay.
- Nilai note harus range-constrained ke integer USDC yang didukung.
- Nullifier tidak boleh hanya `keccak256(H(m))` seperti spend BLS legacy.

## ZK Statement V1

Public inputs minimum:

```text
note_root
input_nullifiers[]
output_commitments[]
recipient
merchant_amount
protocol_fee
execution_fee
quote_hash
chain_id
contract_address
expiry
```

Private witnesses minimum:

```text
input note preimages
owner spending/nullifier keys
Merkle paths and leaf indices
output change note preimages
```

Circuit wajib membuktikan:

1. setiap input commitment berada dalam accepted root;
2. prover memiliki spending authority untuk setiap input;
3. setiap public nullifier dihitung dari input yang sesuai;
4. semua nilai berada dalam range canonical;
5. setiap output commitment dihitung dari output note yang sesuai;
6. exact value conservation;
7. recipient, fees, quote, chain, contract, dan expiry terikat ke proof;
8. zero-value output tidak diterima;
9. duplicate input dalam proof yang sama ditolak.

Circuit `ComplianceCircuit` saat ini tidak memenuhi statement ini. `root`,
`recipient`, dan `amount` memang public input, tetapi belum diberi constraints.
Circuit tersebut saat ini hanya mengikat:

```text
nullifier = Poseidon(secret, randomness)
```

Karena itu VK Phase A yang sekarang tidak dapat dipakai untuk private note ledger.

## Contract State V1

Storage baru harus append-only dan versioned:

```text
note_tree_root
note_tree_next_index
accepted_note_roots
note_nullifiers
user_note_liability
accrued_execution_fee_liability
```

Keputusan exact incremental Merkle tree implementation belum dibekukan. Root
history harus dibatasi atau dikelola agar proof yang dibuat sebelum satu block
baru tidak langsung invalid.

Legacy `total_deposited_principal` dan legacy nullifier mapping tidak boleh
diam-diam diberi semantic baru pada deployment yang sudah ada.

## Privacy Boundary

Nimbus membayar recipient EVM publik, sehingga recipient, payout amount, dan
settlement time tetap terlihat. Yang disembunyikan dan diputus korelasinya:

- owner dan nilai change note;
- hubungan deposit dengan spend tertentu;
- hubungan input commitment dengan nullifier;
- hubungan change note dengan spend berikutnya.

Batching membantu timing privacy tetapi bukan pengganti cryptographic unlinkability.
SDK harus menjelaskan bahwa withdrawal ke wallet publik membuka nominal dan
destination wallet.

## Wallet dan Note Discovery

MVP menggunakan encrypted local note store dalam SDK:

- note preimage dan owner keys tidak disimpan oleh relayer;
- store harus terenkripsi dan dapat di-backup;
- SDK menyimpan note state `unconfirmed`, `unspent`, `reserved`, `spent`;
- output change untuk diri sendiri disimpan sebelum broadcast;
- receipt reconciliation menentukan status final;
- crash tidak boleh menghilangkan note change atau membuat input dapat dipakai
  bersamaan oleh dua request.

Encrypted on-chain note delivery untuk transfer private-to-private bukan bagian
MVP pertama. MVP membayar recipient publik dan membuat change hanya untuk sender.

## Deployment dan Migration

Perubahan membutuhkan fresh contract deployment karena:

- storage dan spend ABI berubah;
- circuit/VK berubah;
- semantic nullifier berubah;
- legacy sessions tidak mempunyai note commitment.

Migration policy:

1. hentikan issuance baru pada deployment lama;
2. unresolved legacy deposit tetap dapat refund;
3. resolved legacy credential diselesaikan melalui jalur lama dalam grace period;
4. jangan mengonversi liability lama menjadi note baru tanpa ownership proof;
5. deploy dan hard-test fresh private-note contract di Arbitrum Sepolia;
6. mainnet hanya memakai deployment baru setelah terminal tests lulus.

## Research Basis

### Primary sources

- Zcash Protocol Specification, version published April 8, 2026. Shielded
  transfers reveal input nullifiers and output commitments, and enforce balance
  between input/output value commitments:
  https://zips.z.cash/protocol/protocol.pdf
- ZIP 224, Orchard Shielded Protocol. Defines note structure, nullifiers,
  commitments, and action model:
  https://zips.z.cash/zip-0224
- Aztec official state-management documentation. Private state is updated by
  nullifying an existing note and creating a new note:
  https://docs.aztec.network/developers/docs/foundational-topics/state_management

### Independent implementation reference

- RAILGUN official documentation. UTXO commitments and nullifiers are public
  circuit outputs while note ownership and values remain private:
  https://docs.railgun.org/wiki/learn/using-private-tokens

Accessed: 2026-06-14.

## Known Risks

- New circuit and trusted setup create a new counterfeit-risk boundary.
- Amount and recipient remain public in the current merchant-payment model.
- Local note loss can become fund loss unless backup/recovery is designed.
- Multiple-input note selection can reveal heuristics through timing and count.
- Merkle root history and concurrent spends require careful race handling.
- BLS issuance and ZK note ownership must not both independently mint liability.
- Execution fee conversion from native gas to USDC needs a signed, bounded oracle
  or deterministic quote policy.

## Mandatory Tests

Positive:

- Deposit 100 USDC, spend 5 USDC, spend again, then spend all remaining value to
  owner wallet; final user liability and change equal zero.
- Partial spend creates one valid change note with exact arithmetic.
- Multiple input notes consolidate into one change note.
- Full spend creates no zero-value change.
- Relayer claims execution fee without reducing backing for user notes.

Negative:

- Reuse input nullifier.
- Forge input value.
- Forge change value.
- Omit protocol or execution fee from conservation equation.
- Use commitment not present in accepted root.
- Use another owner's note.
- Replay proof on another chain, contract, quote, recipient, or amount.
- Claim execution fee twice.
- Crash before/after broadcast and prove note state recovers consistently.

## Terminal Condition

DEC-016 can move from `Proposed` to `Accepted` only after:

- note encoding and circuit statement are frozen;
- independent review finds no unconstrained public input;
- known-answer vectors exist for commitment, nullifier, Merkle path, and balance;
- contract and SDK use the same vectors;
- testnet proves deposit -> partial spend -> repeated spend -> withdraw-all;
- accounting reconciliation ends with no orphan value and no insolvency.
