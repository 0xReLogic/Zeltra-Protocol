# Nimbus Smart Contract

Dokumen ini menjelaskan desain, alur dana, state, API, dan status implementasi
smart contract Nimbus. Targetnya adalah pembaca dapat memahami perilaku kontrak
tanpa harus membaca source code.

> **Status:** testnet/development. Pairing BLS pada jalur `spend` sudah aktif
> di kode, dan direct recipient binding sudah diterapkan. Hard-test 2026-06-07
> di Arbitrum Sepolia masih gagal pada `BLS12_PAIRING_CHECK` karena target chain
> belum mengaktifkan/mendukung EIP-2537. Verifying key ZK masih mock.

## 1. Gambaran Sistem

Nimbus menggunakan model **collateral-backed private payment**:

```text
Client deposit stablecoin
    -> contract menyimpan collateral
    -> issuer/guardian menerbitkan private credential
    -> masking key dibuktikan melalui reveal
    -> client membuka credential
    -> credential digunakan untuk spend
    -> contract membayar recipient
```

Collateral hanya boleh keluar melalui salah satu jalur terminal:

1. `claimRefund`: deposit belum berhasil diterbitkan dan timelock berakhir.
2. `spend`: credential valid dibelanjakan.

`revealMaskKey` tidak membayar collateral. Reveal hanya menyelesaikan issuance,
menutup hak refund, dan membuat credential dapat dibuka oleh client. Aturan ini
mencegah satu deposit dibayar saat reveal lalu dibayar kembali saat spend.

### Invariant Dana

- Satu `session_id` hanya dapat digunakan sekali.
- Satu deposit hanya boleh menghasilkan satu pelepasan collateral.
- Deposit bersih tetap menjadi liability sampai refund atau spend.
- Reveal tidak mengurangi `total_deposited_principal`.
- Refund dan spend mengurangi principal tepat satu kali.
- Commitment reveal harus sama dengan commitment saat deposit.
- Nullifier spend hanya dapat digunakan sekali.
- **Invariant check otomatis**: setiap state-changing transaction memverifikasi
  `contract_assets >= outstanding_liabilities` dan revert jika violated.

Keputusan lifecycle ini dicatat dalam
[`DEC-001`](../research/decisions/DEC-001-deposit-reveal-collateral-lifecycle.md).
Invariant check otomatis dicatat dalam
[`DEC-009`](../research/decisions/DEC-009-liability-invariant-check.md).

## 2. Struktur Modul

```text
nimbus-contracts/src/
├── lib.rs          Public ABI, initialization, admin, dan delegate method
├── storage.rs      Layout storage on-chain
├── deposit.rs      Deposit, reveal masking key, dan refund
├── spend.rs        Spend, Polymarket intent, fallback, dan CCIP receiver
├── verification.rs Groth16 dan compliance verification
├── vault.rs        Accounting, reserve allocation, dan liquidity buffer
├── interfaces.rs   Interface ERC-20, Aave, RWA, dan Conditional Tokens
├── constants.rs    Alamat precompile EIP-2537
├── helpers.rs      Authorization, pause, sender, dan timestamp
└── types.rs        Konversi BLS12-381 ke format EVM
```

Kontrak menggunakan pola satu `Nimbus` storage object dengan implementasi yang
dipisahkan per domain. Method Rust `snake_case` diekspor oleh Stylus sebagai
method ABI `camelCase`.

## 3. State Utama

### Session Issuance

| State | Fungsi |
|---|---|
| `session_client[sid]` | Pemilik deposit dan satu-satunya pihak yang dapat refund |
| `session_amount[sid]` | Deposit bersih setelah fee |
| `session_timestamp[sid]` | Awal timelock refund |
| `session_resolved[sid]` | Issuance selesai atau refund sudah dilakukan |
| `session_commitment_hash[sid]` | Hash commitment `com_k` dari deposit |
| `session_exists[sid]` | Mencegah overwrite dan penggunaan ulang `sid` |

`session_resolved = true` mempunyai dua kemungkinan terminal:

- reveal valid: refund ditutup dan collateral tetap menjadi backing credential;
- refund valid: collateral dikembalikan dan sesi tidak dapat di-reveal.

### Payment dan Compliance

| State | Fungsi |
|---|---|
| `nullifiers[nullifier]` | Replay protection dan pencegahan double-spend |
| `clean_association_roots[root]` | Root compliance yang disetujui owner/oracle |
| `failed_intent_refunds[nullifier]` | Payout yang perlu diklaim setelah intent gagal |

### Configuration dan Accounting

| State | Fungsi |
|---|---|
| `owner`, `pending_owner` | Governance dua langkah |
| `paused` | Emergency stop |
| `stablecoin` | Token collateral ERC-20 |
| `fee_recipient` | Penerima fee dan yield |
| `total_deposited_principal` | Liability principal yang masih outstanding |
| `aave_pool`, `a_token`, `rwa_token` | Integrasi reserve eksternal |
| `ccip_router` | Router yang diizinkan memanggil CCIP receiver |
| `fast_path_phase` | Mode premium dan sumber liquidity fast path |
| `total_lp_liquidity` | Kapasitas LP fase 3 |
| `utilized_lp_liquidity` | Liquidity LP yang sedang digunakan |
| `target_cash_pct` | Target cash reserve: 15%, 30%, atau 45% |

Parameter sensitif seperti fee recipient, fast-path phase, Aave, dan RWA
menggunakan proposal lalu eksekusi setelah timelock 24 jam. Perubahan ownership
menggunakan `proposeOwner` lalu `claimOwnership`.

Field baru wajib ditambahkan di akhir `Nimbus` storage agar layout field lama
tidak bergeser.

## 4. Lifecycle Deposit

### `deposit(sid, comKBytes, amount)`

Deposit membuat session issuance baru.

Validasi:

- kontrak tidak sedang paused;
- `sid` belum pernah digunakan;
- `comKBytes` tepat 256 byte;
- `amount >= 10_000_000` untuk stablecoin 6 desimal, yaitu 10 USDC.

Accounting:

```text
deposit_fee = ceil(amount / 1000)       // 0.1%
net_amount  = amount - deposit_fee
principal   = principal + net_amount
```

Effects:

1. Simpan client, net amount, timestamp, dan status unresolved.
2. Simpan `keccak256(comKBytes)`.
3. Tandai `session_exists`.
4. Tambahkan net amount ke principal.
5. Catat volume epoch.
6. Tarik full amount melalui ERC-20 `transferFrom`.
7. Kirim deposit fee ke `fee_recipient`.
8. Alokasikan reserve sesuai konfigurasi vault.

Client harus melakukan `approve` stablecoin sebelum deposit.

### `revealMaskKey(sid, kBytes, pkIssBytes, comKBytes)`

Reveal membuktikan:

```text
k * pk_iss == com_k
```

Validasi:

- session ada dan belum resolved;
- `kBytes` tepat 32 byte;
- `pkIssBytes` tepat 256 byte;
- `comKBytes` tepat 256 byte;
- hash `comKBytes` sama dengan commitment yang disimpan saat deposit.

Kontrak membentuk payload 288 byte:

```text
pk_iss (256 byte) || k (32 byte)
```

Payload dikirim ke precompile G2 MSM `0x0e`. Jika output sama dengan `com_k`,
session ditandai resolved.

Reveal bersifat permissionless: address mana pun boleh mengirim bukti valid.
Caller tidak menerima collateral dan tidak dapat mengganti commitment. Principal
tetap sama setelah reveal.

### `claimRefund(sid)`

Refund adalah recovery jika issuance tidak selesai.

Syarat:

- session ada dan belum resolved;
- caller sama dengan `session_client`;
- minimal 86.400 detik telah berlalu sejak deposit.

Jika valid:

1. session ditandai resolved;
2. principal dikurangi `session_amount`;
3. contract menyiapkan liquidity;
4. deposit bersih dikirim kembali ke client.

Deposit fee tidak dikembalikan karena `session_amount` menyimpan nilai bersih.
Reveal valid dan refund bersifat saling eksklusif.

## 5. Lifecycle Spend

### `spend(nullifier, alphaNegBytes, pkIssBytes, recipient, amount, recipientOrIntentHash, expiry, nonce)`

Desain verifikasi BLS:

```text
e(-alpha, G2) * e(H(m), pk_iss) == 1
```

Input EVM:

- `alphaNegBytes`: G1, 128 byte;
- `pkIssBytes`: G2, 256 byte;
- `recipient`: address;
- `amount`: uint256;
- `recipientOrIntentHash`: bytes32;
- `expiry`: uint256;
- `nonce`: bytes32;
- pairing payload: 768 byte untuk dua pasangan.

Validasi:

- kontrak tidak paused;
- minimum spend 5 USDC;
- nullifier belum digunakan;
- nullifier sama dengan `keccak256(hmBytes)` (di mana `hmBytes` direkonstruksi secara on-chain dari parameter melalui RFC 9380 SSWU);
- issuer public key sudah didaftarkan owner;
- panjang G1/G2 tepat dan bukan point at infinity;
- output pairing tepat 32 byte dan canonical;
- pairing menghasilkan `true`;
- principal mencukupi.

Setelah validasi:

- fee dihitung dengan pembulatan ke atas;
- principal dikurangi sebesar `amount`;
- nullifier ditandai sudah digunakan;
- liquidity disiapkan sebelum transfer;
- payout dikirim ke recipient;
- protocol share dikirim ke `fee_recipient`.

Perhitungan:

```text
base_fee       = ceil(amount * 15 / 10_000)     // 0.15%
premium        = fast_path_premium(amount)
premium_share  = ceil(premium * 20 / 100)
protocol_share = base_fee + premium_share
payout         = amount - protocol_share
```

Issuer key dikelola melalui:

- `registerIssuerKey(pkIssBytes)`;
- `revokeIssuerKey(pkIssBytes)`;
- `isIssuerKeyTrusted(pkIssBytes)`.

Kontrak bersifat fail-closed: jika belum ada issuer key yang didaftarkan, semua
spend ditolak.

> **Critical blocker:** pairing valid belum membuktikan bahwa issuer menyetujui
> `amount`, `recipient`, action, expiry, chain, dan contract tertentu karena
> contract masih menerima `H(m)` dari caller. Canonical spend message dan
> domain separation harus diselesaikan sebelum dana nyata. Lihat
> [`DEC-002`](../research/decisions/DEC-002-bls-spend-verification-boundaries.md).

## 6. Polymarket Intent

### `spendAndBuyShares(...)`

Flow yang dituju:

1. Jalankan spend dan invalidasi nullifier.
2. Hitung payout setelah fee.
3. Approve collateral kepada Conditional Tokens contract.
4. Panggil `splitPosition` untuk mencetak outcome shares.

Jika panggilan target gagal, nominal payout dicatat dalam
`failed_intent_refunds[nullifier]` agar dana dapat dipulihkan secara asynchronous.

API recovery:

- `getFailedIntentRefund(nullifier)` membaca refund yang tersedia.
- `claimFailedIntentRefund(nullifier, recipient)` menghapus record lalu
  mentransfer refund ke recipient.

Pembersihan state dilakukan sebelum external transfer untuk mengurangi risiko
reentrancy. Namun keamanan flow ini tetap bergantung pada verifikasi spend yang
saat ini masih bypass.

## 7. CCIP Receiver

### `ccipReceive(messageId, sourceChainSelector, sender, payload)`

Receiver:

1. Memeriksa kontrak tidak paused.
2. Jika `ccip_router` dikonfigurasi, mensyaratkan caller sama dengan router.
3. Mensyaratkan payload tepat 584 byte (di mana H(m) tidak lagi dikirim secara eksplisit tetapi direkonstruksi on-chain).
4. Mendekode parameter spend dan destination action.
5. Menjalankan `spendAndBuyShares`.

Jika `condition_id == bytes32(0)`, flow digunakan sebagai direct transfer dan
field target berperan sebagai recipient wallet.

Yang belum lengkap untuk mainnet:

- allowlist source chain;
- validasi sender per source chain;
- replay binding terhadap `message_id`;
- hard test dengan CCIP router dan token nyata.

## 8. EIP-2537 BLS12-381 Precompile

Nimbus menargetkan precompile BLS12-381 yang tersedia setelah Ethereum Pectra.

| Address | Operasi | Penggunaan Nimbus |
|---|---|---|
| `0x0b` | G1 ADD | Menjumlahkan public-input commitment |
| `0x0c` | G1 MSM | Linear combination public inputs Groth16 |
| `0x0e` | G2 MSM | Memverifikasi `k * pk_iss == com_k` |
| `0x0f` | Pairing check | Verifikasi BLS spend dan Groth16 |

Gas schedule EIP-2537 mendefinisikan pairing check berdasarkan jumlah pasangan.
Estimasi yang digunakan desain awal Nimbus adalah:

```text
gas = 37.700 + (32.600 * jumlah_pasangan)
```

Dengan dua pasangan, estimasinya 102.900 gas. Angka ini bukan jaminan biaya
akhir transaksi karena target L2 dapat mempunyai activation status, pricing,
dan overhead Stylus yang berbeda.

Encoding point mengikuti format EIP-2537, bukan compressed serialization
Arkworks. Helper di `types.rs` mengubah field element ke representasi EVM:

- G1: 128 byte;
- G2: 256 byte;
- scalar: 32 byte big-endian.

Jangan mengasumsikan semua L2 mendukung precompile pada address dan behavior
yang sama. Deployment harus diuji langsung pada target chain.

Catatan hard-test 2026-06-07:

- Contract Stylus patched berhasil deploy dan activate di Arbitrum Sepolia:
  `0x3a814eb65b442890abe3f666acac2d4f9ff6ad9c`.
- Jalur `spend` mencapai `BLS12_PAIRING_CHECK`, tetapi call revert
  `BLS_PAIRING_PRECOMPILE_FAILED`.
- Rilis mainnet di Arbitrum tidak boleh mengandalkan EIP-2537 sampai support
  chain dikonfirmasi ulang atau desain fallback dipilih.

## 9. ZK Compliance

Flow compliance yang dituju:

1. Owner/oracle mendaftarkan clean association root.
2. Client membuat Groth16 proof off-chain.
3. Contract memastikan root terdaftar.
4. Contract mengikat root, nullifier, recipient, dan amount sebagai public
   inputs.
5. G1 MSM `0x0c` dan G1 ADD `0x0b` menghitung linear combination.
6. Pairing precompile `0x0f` memverifikasi proof.

API:

- `registerCleanRoot(root)`;
- `verifyGroth16Proof(...)`;
- `verifyCompliance(root, nullifier, recipient, amount, proofA, proofB, proofC)`.

> **Critical blocker:** verifying key saat ini dibentuk dari generator untuk
> development, bukan hasil trusted setup/circuit production. Compliance belum
> memberikan security guarantee sampai circuit, proving key, verifying key,
> public-input ordering, dan test vector production dibekukan dan diaudit.

## 10. Vault dan Liquidity

### Reserve Allocation

Target cash default adalah 30%. Bagian non-cash dibagi dengan rasio 5:2 antara
Aave dan RWA.

| Kondisi moving average | Cash | Aave | RWA |
|---|---:|---:|---:|
| Di bawah 10.000 USDC/epoch | 15% | ~60.7% | ~24.3% |
| Normal | 30% | 50% | 20% |
| Di atas 100.000 USDC/epoch | 45% | ~39.3% | ~15.7% |

Volume dihitung per epoch 24 jam menggunakan moving average hingga tujuh epoch.
Deposit dan spend menambah volume. Public monitoring tersedia melalui:

- `targetCashPct`;
- `currentEpochId`;
- `currentEpochVolume`;
- `historicalEpochVolume`.

### Cascading Liquidity Buffer

Saat refund, spend, atau yield withdrawal memerlukan stablecoin:

1. gunakan cash contract;
2. tarik shortfall dari Aave;
3. redeem shortfall dari RWA.

Jika seluruh tier tidak cukup, operasi gagal. `IRwaToken.redeem` menggunakan
`min_receive = shortfall`.

### Yield

```text
total_assets = cash + aToken balance + RWA token balance
yield        = max(total_assets - principal, 0)
```

`claimAccumulatedYield` hanya dapat dipanggil owner dan mengirim yield ke
`fee_recipient`. Principal user tidak boleh diklaim sebagai yield.

Integrasi Aave/RWA masih memerlukan hard test terhadap contract dan token nyata,
termasuk decimal, withdrawal delay, slippage, pause, dan insolvency behavior.

### External Interfaces

Kontrak berinteraksi dengan reserve provider melalui ABI berikut:

```solidity
interface IAavePool {
    function supply(
        address asset,
        uint256 amount,
        address onBehalfOf,
        uint16 referralCode
    ) external;

    function withdraw(
        address asset,
        uint256 amount,
        address to
    ) external returns (uint256);
}

interface IRwaToken {
    function deposit(uint256 amount) external returns (uint256);
    function redeem(
        uint256 amount,
        uint256 minReceive
    ) external returns (uint256);
}
```

ERC-20 digunakan untuk `transfer`, `transferFrom`, `approve`, dan `balanceOf`.
Conditional Tokens digunakan untuk `splitPosition`.

## 11. Fast-Path Premium

| Phase | Model |
|---|---|
| 1 | Standard path, premium 0 |
| 2 | Treasury-funded, premium tetap 0.05% |
| 3 | Public LP, dynamic cap dan premium 0.05%-0.15% |

Pada phase 3:

```text
new_utilized = utilized_lp_liquidity + amount
```

Transaksi ditolak jika `new_utilized > total_lp_liquidity`. Premium bertambah
sesuai utilization. Sebesar 20% dari premium masuk protocol share.

Phase 2 dan 3 tidak boleh diaktifkan sebelum treasury/LP benar-benar tersedia
dan accounting utilization terhubung ke lifecycle settlement.

## 12. Admin dan Emergency Control

API utama:

- `init(owner, stablecoin, feeRecipient)`;
- `pause()` dan `unpause()`;
- `proposeOwner()` dan `claimOwnership()`;
- `proposeFeeRecipient()` dan `executeFeeRecipient()`;
- `proposeFastPathPhase()` dan `executeFastPathPhase()`;
- `proposeAaveParams()` dan `executeAaveParams()`;
- `proposeRwaToken()` dan `executeRwaToken()`;
- `registerIssuerKey()`, `revokeIssuerKey()`, dan `isIssuerKeyTrusted()`;
- `setCcipRouter()`;
- `setLpLiquidity()`.

Perubahan parameter finansial memakai timelock 24 jam, kecuali
`setCcipRouter` dan `setLpLiquidity` yang saat ini langsung berlaku setelah
dipanggil owner. Keduanya perlu dipertimbangkan untuk timelock sebelum mainnet.

## 13. Build dan Deployment

Package dan library menggunakan nama `nimbus_contracts`. Underscore menghindari
mismatch nama artifact `cdylib` pada versi `cargo-stylus` yang sebelumnya
mencari nama package dengan tanda hubung.

Validasi minimum:

```bash
RUSTC_WRAPPER= cargo test --workspace
RUSTC_WRAPPER= cargo check -p nimbus_contracts --lib
cargo stylus check
```

Perubahan storage untuk commitment bersifat append-only, tetapi deployment lama
tidak memiliki data `session_exists`, `session_commitment_hash`, dan
`trusted_issuer_keys`. Gunakan deployment testnet baru atau migration plan yang
eksplisit. Deployment baru harus mendaftarkan issuer key sebelum hard test spend.

## 14. Status Keamanan

### Sudah diimplementasikan

- pause guard pada financial flow;
- owner initialization;
- two-step ownership;
- timelock beberapa parameter admin;
- commitment binding deposit dan reveal;
- duplicate session protection;
- refund timelock dan client authorization;
- reveal tanpa payout;
- pairing BLS EIP-2537 pada spend;
- trusted issuer key allowlist dan revocation;
- nullifier terikat ke `keccak256(H(m))`;
- malformed input dan point-at-infinity rejection;
- checked principal subtraction;
- unit/regression test lifecycle deposit dan reveal;
- ikat credential ke amount, recipient/action, expiry, chain, dan contract (selesai secara end-to-end pada kontrak, SDK, dan relayer).

### Blocker sebelum dana nyata

1. Ganti mock compliance VK dengan artifact circuit production.
3. Hard test deposit, reveal, refund, dan spend pada chain/harness yang
   benar-benar mendukung EIP-2537.
4. Uji precompile EIP-2537 dengan test vector valid dan invalid di target
   supported chain; Arbitrum Sepolia saat hard-test 2026-06-07 belum lolos.
5. Audit liability, fee, rounding, dan solvency pada seluruh state transition.
6. Lengkapi authentication dan replay protection CCIP.
7. Uji Aave, RWA, dan Polymarket tanpa mock.
8. Tambahkan event untuk seluruh perubahan state finansial.
9. Tentukan migration dan emergency recovery sebelum deployment immutable.

Daftar pengujian dan pekerjaan lengkap berada di [`todo.md`](../todo.md).
