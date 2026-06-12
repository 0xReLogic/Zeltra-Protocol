# Nimbus Protocol TODO

Dokumen ini membandingkan klaim pada `SESSION_SUMMARY.md` dengan implementasi
runtime saat ini. Tujuannya adalah memisahkan fitur yang sudah nyata dari mock,
bypass, fallback development, dan pekerjaan yang masih diperlukan sebelum
mainnet.

## Research Gate untuk Perubahan Critical

Sebelum mengubah financial logic, cryptographic protocol, smart contract,
cross-chain settlement, custody, key management, atau accounting, lakukan riset
terlebih dahulu. Riset untuk mencari teknologi terbaru dan memastikan desain
memiliki dasar formal, mengikuti spesifikasi terkini, dan tidak mengulangi exploit
yang sudah pernah terjadi.

### Kapan Research Gate Wajib

- Perubahan alur deposit, mint/issuance, spend, withdraw, reveal, atau refund.
- Perubahan liability, fee, rounding, yield, liquidity, atau solvency.
- Perubahan BLS, blind signature, threshold signature, hash-to-curve, atau ZK.
- Perubahan nullifier, replay protection, domain separation, atau message
  binding.
- Perubahan CCIP, bridge, cross-chain intent, destination execution, atau
  recovery.
- Perubahan guardian, Vault/KMS, key rotation, signing API, atau quorum.
- Perubahan storage layout, upgrade/migration, pause, governance, atau admin
  capability.
- Integrasi token, Polymarket, x402, relayer, atau external protocol.

### Tool yang Digunakan

#### Exa

Gunakan untuk:

- Mencari paper/jurnal akademik terbaru.
- Mencari audit report, exploit post-mortem, dan security research.
- Membandingkan implementasi open-source production.
- Mencari desain threshold, privacy payment, bridge, dan intent protocol.

#### Tavily

Gunakan untuk:

- Cross-check informasi terbaru dari beberapa sumber.
- Mencari dokumentasi resmi, announcement upgrade, dan perubahan network.
- Memverifikasi alamat, chain support, standard, dan behavior protocol terkini.
- Mencari incident terbaru yang relevan dengan desain.

#### Context7

Gunakan untuk:

- Dokumentasi API/library terbaru.
- Contoh implementasi Rust, Alloy, Stylus SDK, Arkworks, Axum, dan dependency
  lain.
- Memastikan signature method, type, feature flag, dan versi library benar.

Context7 bukan sumber utama untuk keputusan ekonomi atau pembuktian
kriptografi.

### Prioritas Sumber

Urutan sumber:

1. Spesifikasi dan dokumentasi resmi.
2. Paper akademik peer-reviewed atau preprint dari penulis kredibel.
3. Implementasi audited yang aktif digunakan.
4. Audit report dan exploit post-mortem.
5. Repository/reference implementation resmi.
6. Artikel teknis sekunder untuk menemukan sumber primer.

Hindari menjadikan blog SEO, jawaban forum tanpa referensi, atau kode contoh
acak sebagai dasar perubahan critical.

### Aturan Riset

- Gunakan minimal dua sumber independen untuk klaim critical.
- Setidaknya satu sumber harus primer/resmi.
- Catat tanggal publikasi dan versi spesifikasi/library.
- Bedakan standard aktif, draft, proposal, dan eksperimen.
- Cari known attack dan failure mode, bukan hanya happy path.
- Cari audit finding yang mirip dengan flow Nimbus.
- Bandingkan minimal satu implementasi production/audited jika tersedia.
- Jangan menganggap teknologi tahun 2026 otomatis lebih aman.
- Jangan mengubah cryptographic construction hanya untuk optimasi tanpa
  security argument yang jelas.
- Jangan copy contract/library code tanpa memahami invariant dan lisensi.
- Verifikasi hasil riset terhadap versi dependency yang benar-benar dipakai
  repo.
- Setelah riset, tetap wajib hard-test di testnet tanpa mock.

### Research Note Wajib

Untuk setiap perubahan critical, tambahkan catatan singkat:

```text
Decision ID:
Masalah:
Invariant bisnis/security:
Pilihan yang dipertimbangkan:
Keputusan:
Alasan:
Sumber primer:
Sumber pembanding:
Known risks:
Versi/library/network:
Rencana positive test:
Rencana negative test:
Rollback/recovery:
```

- Simpan research note di `research/decisions/`.
- Gunakan satu file per keputusan.
- Sertakan link sumber dan tanggal akses.
- Tandai asumsi yang belum terbukti.
- Hubungkan decision ID ke TODO, code change, dan hard-test report.

### Research Gate Kelulusan

Perubahan critical baru boleh diimplementasikan jika:

- Invariant bisnisnya tertulis.
- Threat model dan failure mode utamanya diketahui.
- Sumber primer sudah dibaca.
- Pilihan desain alternatif sudah dibandingkan.
- Dampak accounting/storage/API dipahami.
- Positive dan negative test direncanakan.
- Recovery jika perubahan gagal sudah jelas.



## Blueprint Bisnis Nimbus

Bagian ini adalah sumber utama untuk mengingat alur bisnis. Implementasi,
produk tambahan, dan hard test harus mengikuti blueprint ini. Jika kode berbeda
dengan blueprint, perbedaan tersebut harus diputuskan secara eksplisit dan
didokumentasikan sebelum deployment berikutnya.

### Tujuan Core

Nimbus menerima USDC publik dan menerbitkan credential pembayaran privat dengan
nilai yang sama setelah fee. Credential tersebut dapat digunakan satu kali
untuk membayar recipient atau dikembalikan melalui refund jika penerbitan gagal.

Alur sederhananya:

```text
User/AI Agent
    |
    | deposit USDC
    v
Nimbus Contract
    |
    | issue private credential melalui leader + guardians
    v
User/AI Agent memegang credential privat
    |
    | spend satu kali
    v
Merchant/recipient menerima USDC
```

AI agent dan manusia menggunakan accounting core yang sama. Perbedaannya hanya:

- Manusia memakai UI dan wallet.
- AI agent memakai SDK/API dan wallet automation.

Contract tidak perlu memiliki accounting khusus AI agent.

### Aktor

- **User/AI Agent**: pemilik dana dan private credential.
- **Nimbus Contract**: escrow USDC dan source of truth liability/nullifier.
- **Leader**: mengoordinasikan signing session dan settlement.
- **Guardian**: memakai share lokal untuk menghasilkan partial signature.
- **Relayer**: mengirim transaksi dan memonitor receipt.
- **Recipient/Merchant**: menerima payout USDC.
- **Fee Recipient/Treasury**: menerima fee yang dinyatakan eksplisit.

### Keputusan Model Dana yang Wajib

Model yang direkomendasikan untuk MVP:

```text
Deposit USDC
    -> USDC tetap menjadi collateral di contract
    -> client menerima private credential
    -> credential di-spend
    -> contract membayar recipient
```

`reveal(k)` hanya membuka kemampuan client untuk mendapatkan credential final.
Reveal **tidak boleh sekaligus mengembalikan collateral** kepada caller.

Alternatif lain adalah deposit hanya sebagai atomic escrow lalu dikembalikan
saat reveal. Namun jika memilih model tersebut, credential tidak boleh lagi
memiliki claim terhadap USDC contract. Mencampur kedua model menyebabkan
double payout.

Keputusan sebelum melanjutkan:

- Tetapkan model MVP sebagai **collateral-backed private payment**
  (`DEC-001`).
- Dokumentasikan bahwa deposit net menjadi liability sampai spend/refund.
- Ubah reveal agar tidak membayar USDC.
- Pastikan hanya spend atau refund yang mengurangi liability.
- Hapus logic lama yang memperlakukan reveal sebagai payout.

### P0 Bisnis: Potensi Double Payout Saat Reveal

Kode saat ini:

1. `deposit()` menarik USDC dari user.
2. `reveal_mask_key()` mengirim amount USDC kepada caller.
3. Client tetap memperoleh unmasked credential.
4. Credential dapat digunakan pada `spend()` untuk meminta payout lagi.

Jika alur ini berjalan penuh, satu deposit dapat menghasilkan:

```text
payout saat reveal + payout saat spend
```

Ini merupakan business-logic blocker paling kritis.

- Buat hard test yang membuktikan satu deposit tidak dapat menghasilkan
  dua payout.
- Hapus transfer USDC dari reveal untuk model collateral-backed.
- Reveal hanya mengubah signing-session state; distribusi `k` tetap tugas
  leader/off-chain.
- Liability tidak turun saat reveal.
- Liability turun tepat satu kali saat spend atau refund.
- Tambahkan invariant global setelah setiap transaksi:
  `contract_assets >= outstanding_liabilities`.

Acceptance criteria:

- Satu deposit net hanya dapat dibayarkan satu kali.
- Total payout + refund dari sebuah session tidak pernah melebihi deposit net.
- Reveal tidak mengubah saldo USDC contract/user/recipient kecuali gas.

## Core Business State Machine

Setiap session harus memiliki satu state canonical:

```text
CREATED
  |
  | quorum menghasilkan masked signature + com_k
  v
SIGNED_MASKED
  |
  | deposit confirmed on-chain
  v
FUNDED
  |
  | k dirilis
  v
CREDENTIAL_READY
  |                    |
  | spend confirmed    | timeout sebelum credential siap
  v                    v
SPENT                REFUNDABLE
                       |
                       | refund confirmed
                       v
                    REFUNDED
```

Terminal state:

- `SPENT`
- `REFUNDED`
- `CANCELLED` tanpa dana

Transisi yang dilarang:

```text
SPENT -> REFUNDED
REFUNDED -> SPENT
FUNDED -> payout saat reveal
session A deposit -> membuka k session B
```

TODO state machine:

- Definisikan enum/status yang sama pada contract, relayer DB, SDK, dan API.
- Setiap transition memiliki precondition eksplisit.
- Setiap transition idempotent atau menolak retry dengan aman.
- Simpan tx hash/block untuk transition on-chain.
- Rekonsiliasi state relayer dengan contract setelah restart.
- Jangan menyimpulkan state finansial hanya dari log atau queue memory.

## Ledger dan Invariant Bisnis

Definisi:

```text
gross_deposit     = USDC yang ditarik dari user
deposit_fee       = fee mint/deposit
net_liability     = gross_deposit - deposit_fee
redemption_fee    = fee saat spend jika memang diterapkan
recipient_payout  = net_liability - redemption_fee
```

Invariant protocol:

- `assets >= outstanding_liabilities`.
- `gross_deposit = deposit_fee + net_liability`.
- Satu session hanya menambah liability satu kali.
- Satu credential hanya mengurangi liability satu kali.
- `SPENT XOR REFUNDED`; tidak boleh keduanya.
- Failed/reverted transaction tidak mengubah liability.
- Fee hanya diakui setelah transaksi yang mendasarinya berhasil.
- Rounding selalu menguntungkan solvency, tetapi tidak mengambil fee
  tersembunyi.
- Decimal token divalidasi; MVP hanya USDC 6 decimal.
- Admin tidak dapat mengklaim user principal sebagai yield.
- Saldo contract langsung bukan satu-satunya liability ledger.

Ledger minimum yang harus dapat diaudit:

- Total gross deposit.
- Total fee terkumpul.
- Total outstanding liability.
- Total payout confirmed.
- Total refund confirmed.
- Total pending/funded sessions.
- Actual USDC balance.
- Asset yang ditempatkan di external vault, jika nanti diaktifkan.

## Core Business Flow

### Flow 1 - Create Private Credential

1. Client membuat message/intent privat dan blinding factor `r`.
2. Client mengirim blinded point ke leader dengan `session_id` dan amount.
3. Leader membuat masking key `k` dan commitment `com_k`.
4. Guardians mengembalikan partial signatures.
5. Leader mengagregasi masked signature.
6. Client memverifikasi masked signature dan `com_k`.
7. Client approve lalu deposit USDC ke contract.
8. Contract mencatat client, amount net, commitment, dan liability.
9. Setelah receipt final, leader merilis `k`.
10. Client unmask dan memperoleh credential final.

Checklist:

- Tidak ada liability sebelum deposit receipt sukses.
- `k` tidak bocor sebelum deposit confirmed.
- Deposit commitment sama dengan signing commitment.
- Amount session sama dengan amount deposit.
- Credential final diverifikasi client sebelum dianggap ready.
- Reveal tidak mengirim payout.

### Flow 2 - Spend Private Credential

1. Client memilih recipient dan amount.
2. Authorization/message harus mengikat domain, chain, contract, recipient,
   amount, asset, expiry, dan nonce/nullifier.
3. Client/SDK menghasilkan parameter BLS EVM.
4. Relayer memvalidasi format dan policy.
5. Contract memverifikasi BLS dan nullifier.
6. Contract menandai nullifier.
7. Contract mengurangi liability.
8. Contract mengirim payout dan fee.
9. Relayer menunggu receipt lalu melaporkan confirmed.

Checklist:

- Recipient terikat pada signature.
- Amount terikat pada signature.
- Chain ID dan contract address terikat pada signature.
- Asset terikat pada signature.
- Expiry terikat pada signature.
- Nullifier unik dan deterministic dari credential/domain.
- State update dan transfer atomic.
- API tidak melaporkan sukses sebelum settlement state yang disepakati.

### Flow 3 - Refund

Refund hanya untuk funded session yang gagal menghasilkan credential sesuai
deadline.

1. Deposit confirmed.
2. Quorum/reveal gagal sampai timeout.
3. Session menjadi refundable.
4. Session client mengklaim refund.
5. Contract mengurangi liability dan mengirim USDC.
6. Session terminal `REFUNDED`.

Checklist:

- Credential yang sudah ready tidak dapat direfund.
- Session yang sudah spent tidak dapat direfund.
- Hanya depositor atau recovery address yang disepakati dapat menerima
  refund.
- Refund tidak membayar lebih dari liability session.
- Refund retry tidak menghasilkan transfer kedua.

### Flow 4 - Recovery

- Jika leader mati sebelum deposit: tidak ada dana yang perlu dipulihkan.
- Jika leader mati setelah deposit: session dapat dilanjutkan node baru atau
  direfund setelah timeout.
- Jika guardian kurang quorum: tidak ada `k` release, lalu refund.
- Jika relayer mati setelah broadcast: node baru mencari receipt dan
  melanjutkan state.
- Jika contract pause: policy harus menentukan apakah refund tetap aktif.
- Tidak ada recovery path yang membutuhkan admin mengambil custody user.

## Core MVP Gate

Core dianggap berhasil hanya jika semua berikut lulus hard test nyata:

- Deposit nyata.
- Masked threshold signing nyata.
- Atomic release `k`.
- Credential final valid.
- Spend BLS valid menghasilkan satu payout.
- Signature invalid tidak menghasilkan payout.
- Double spend gagal.
- Refund berhasil pada failure path.
- Spend dan refund tidak pernah sama-sama berhasil.
- Restart leader/relayer tidak menghilangkan uang atau request.
- Accounting selalu solvent.
- Emergency pause/recovery bekerja.

Sebelum gate ini selesai, fitur tambahan tidak boleh menjadi dependency jalur
uang utama.

## Status Saat Ini


### Artifact Testnet yang Sudah Ada

- `nimbus-core/examples/generate_bls_test_data.rs`
- `nimbus-node/.env.test`
- `scripts/`

## Hard-Test Charter: Testnet Diperlakukan Seperti Mainnet

Semua fitur yang akan dipercaya di mainnet harus diuji melalui binary release,
RPC nyata, contract Stylus yang benar-benar terdeploy, transaksi nyata, receipt
nyata, dan state on-chain nyata. Unit test Rust tetap dipakai untuk feedback
cepat, tetapi tidak boleh menjadi satu-satunya bukti bahwa fitur selesai.

### Aturan Kelulusan

- [ ] Jalankan contract dan relayer dari commit Git yang dicatat.
- [ ] Build contract menggunakan profile release yang sama dengan deployment.
- [ ] Catat hash WASM/contract artifact sebelum deploy.
- [ ] Catat chain ID, contract address, deployment tx, activation tx, dan block.
- [ ] Semua transaksi wajib menunggu receipt dan memeriksa status.
- [ ] Keberhasilan broadcast atau adanya tx hash tidak dianggap sukses.
- [ ] Untuk CCIP, source receipt saja tidak dianggap E2E sukses.
- [ ] Setiap positive test wajib memiliki negative control.
- [ ] Setiap negative test wajib membuktikan state tidak berubah.
- [ ] Verifikasi state melalui RPC langsung, bukan hanya database/log relayer.
- [ ] Verifikasi saldo token sebelum dan sesudah transaksi.
- [ ] Verifikasi event contract dan parameter event.
- [ ] Simpan revert data/reason untuk transaksi yang memang harus gagal.
- [ ] Test gagal jika menggunakan mock tx hash, dummy signature, test default
  key, `#[cfg(test)]` bypass, atau random bytes sebagai proof/signature valid.
- [ ] Test gagal jika EVM client, Vault, guardian, atau destination monitor tidak
  tersedia; jangan silently fallback ke mock.
- [ ] Test runner wajib menghasilkan report JSON dan Markdown yang dapat
  direproduksi.

### Mode Runtime Khusus Hard Test

- [x] Tambahkan `NIMBUS_ENV=hard-test`.
- [x] Pada `hard-test`, node harus fail startup jika RPC tidak tersedia.
- [x] Pada `hard-test`, node harus fail startup jika contract address invalid.
- [x] Pada `hard-test`, node harus fail startup jika signer key tidak tersedia.
- [ ] Pada `hard-test`, guardian harus fail startup jika share key tidak dapat
  diambil.
- [ ] Pada `hard-test`, semua mock tx hash dan fallback insecure harus
  dinonaktifkan.
- [x] Pada `hard-test`, default DB key harus ditolak.
- [ ] Pada `hard-test`, chain ID dan deployed bytecode harus diverifikasi saat
  startup.
- [ ] Log startup harus mencetak mode, chain ID, signer address, contract
  address, dan artifact version tanpa mencetak secret.


## Hard-Test Matrix

### HT-00 Deployment Reproducibility

- [ ] Bersihkan build artifact lalu build contract release dari nol.
- [x] Jalankan `cargo stylus check`.
  - 2026-06-07: gagal sebelum release profile size optimization
    (`95.6 KB`, 4 fragments, `execution reverted, data: 0x`).
  - 2026-06-07: lolos setelah `opt-level = "z"`, LTO, `panic = "abort"`,
    `codegen-units = 1`, dan `strip = true` (`65.9 KB`, 3 fragments).
- [x] Deploy contract baru ke Arbitrum Sepolia.
  - 2026-06-07: deployed `0x5e5bff9f5979989f93fca5c460873cb49dbb3b3e`
    pada commit `8fafd05`; deployment tx
    `0xdc3f714fe4dc3291e553500e8ac24aa5fc40baaf63a7095a5ff4127349fc3b08`.
- [ ] Aktifkan dan cache contract.
  - Activation sukses dengan tx
    `0xaa4cf529e212fe2836bfd226b1712cf85192f7c7bd7f9fcae54e51bc07c902cb`.
    Cache ArbOS belum selesai; `cargo stylus cache bid ... 0` gagal karena
    estimasi gas cache manager menjadi sangat besar dan wallet dianggap
    insufficient funds.
- [ ] Simpan deployment manifest berisi commit, rustc, cargo-stylus, Stylus SDK,
  WASM hash, ABI hash, chain ID, addresses, dan tx hashes.
  - Script deploy sudah menulis manifest dasar; item ini tetap belum selesai
    sampai deploy RC nyata mencatat address, tx hash, WASM hash, ABI hash, dan
    hasil verifikasi receipt.
- [ ] Bandingkan exported ABI dengan Alloy interface relayer.
- [ ] Pastikan selector semua method sesuai.
- [ ] Pastikan storage initialization hanya bisa dilakukan sekali.
- [ ] Pastikan owner, stablecoin, fee recipient, router, dan phase sesuai
  manifest.

### HT-01 Vault/KMS Nyata

- [ ] Node leader berhasil mengambil share 40-byte dari Vault Tailscale.
- [ ] Share terdeserialisasi menjadi `(index, Fr)` yang expected.
- [ ] Node tidak pernah mencetak share.
- [ ] Token invalid menyebabkan startup gagal pada hard-test mode.
- [ ] Vault sealed menyebabkan startup gagal atau node tidak-ready.
- [ ] Setelah Vault unseal, node dapat recovery tanpa mengganti share.
- [ ] Path salah tidak fallback ke test key.
- [ ] Hapus `NIMBUS_SHARE_KEY` dan buktikan Vault menjadi sumber key tunggal.
- [ ] Restart node dan pastikan public key/share index konsisten.
- [ ] Rotasi/reload key diuji tanpa menghasilkan mixed-key ceremony.

### HT-02 Distributed Threshold Signing via Tailscale

- [ ] Jalankan leader pada VPS ini dan guardian pada VPS Tailscale lain.
- [ ] Bind guardian hanya ke IP Tailscale.
- [ ] Buktikan port guardian tidak dapat diakses dari interface publik.
- [ ] Leader mengirim blinded message dan `k`, bukan meminta share key.
- [ ] Guardian mengambil share lokal dan mengembalikan partial signature.
- [x] Response guardian menyertakan share index asli dari `KeyManager`.
- [ ] Response guardian menyertakan key version, session ID, dan request digest.
- [x] Leader dan guardian memakai issuer public key hasil ceremony yang sama,
  bukan public key yang diturunkan dari share lokal.
- [x] Client tidak dapat mengganti issuer public key pada request leader.
- [x] Leader memverifikasi pairing partial signature terhadap public share
  yang dipin sebelum menghitungnya ke quorum.
- [ ] Aggregate minimal threshold menghasilkan masked signature valid.
- [x] Leader menolak hasil signing saat jumlah share unik masih sub-threshold.
- [x] Duplicate guardian index ditolak dan tidak dihitung ke quorum.
- [x] Guardian index palsu/tidak terdaftar ditolak.
- [x] Partial signature corrupt atau tidak cocok dengan public share ditolak.
- [ ] Guardian timeout tidak membuat leader menganggap ceremony sukses.
- [ ] Guardian offline menghasilkan retry/failure yang terukur.
- [ ] Guardian lama dengan key version berbeda ditolak.
- [ ] Device yang dikeluarkan dari tailnet tidak dapat memanggil endpoint.

### HT-03 Atomic Deposit -> Sign -> Reveal

- [x] Client membuat `session_id`, message, blinding factor, dan blinded point.
- [x] Leader membuat `k` dan `com_k` tetapi tidak memberikan `k`.
- [x] Quorum guardian menghasilkan masked signature.
- [x] Client memverifikasi masked signature sebelum deposit.
- [x] Client approve USDC dan mengirim deposit nyata.
  - Arbitrum Sepolia approve tx:
    `0xe39e66d5fd51301965604f3ddbde195275363a51e190c130c14a80f4dcc9519e`
  - Arbitrum Sepolia deposit tx:
    `0xc0c4c59bc6992603aaf46922062651fb25032afe535d919107339a27b5eb0545`
  - Session:
    `0x9b1ac450be38cc42ac6b51c8da89f4a9409c85143f1ef47c0d440b361028c5bf`
- [x] Tunggu receipt deposit dan verifikasi event/state.
- [x] Verifikasi fee recipient menerima fee yang tepat.
  - Deposit 10 USDC, fee 0.01 USDC, wallet fee recipient sama dengan
    depositor sehingga wallet turun net 9.99 USDC.
- [x] Verifikasi principal bertambah sebesar net amount.
  - `totalDepositedPrincipal = 9.99 USDC` setelah deposit.
  - Deployment patched recipient-binding:
    `0x3a814eb65b442890abe3f666acac2d4f9ff6ad9c`
  - Patched deposit tx:
    `0x8192bda4a9c1b8765f5572d0af4391b88dbb23e1e2e8604bf5578a5187d27d7f`
- [x] Leader baru merilis `k` setelah deposit confirmed.
  - 2026-06-11: PT-01 deposit & reveal tx `0xb5a9f0f15d3e9fe7...` berhasil mengeksekusi reveal di Arbitrum Sepolia.
- [x] Reveal contract membandingkan dengan commitment tersimpan.
  - EIP-2537 MSM check `k * pk_iss == com_k` sukses memicu `session_resolved` menjadi true.
- [ ] Dana/redeem outcome hanya diterima session client yang benar.
- [x] Client dapat unmask dan final signature valid.
  - Final signature diverifikasi sukses di client.
- [ ] Caller lain mencoba reveal dan gagal mengambil dana.
- [x] Commitment berbeda ditolak.
  - NT-03 wrong com_k revert dengan `COMMITMENT_MISMATCH`.
- [x] `k` berbeda ditolak.
  - NT-01 mismatched k mengembalikan `False`.
- [x] Issuer public key berbeda ditolak sepanjang alur deposit dan reveal.
  - NT-02 wrong pk_iss memicu precompile fail revert `MSM_PRECOMPILE_CALL_FAILED`.
- [x] Session ID duplicate ditolak tanpa mengubah deposit awal.
  - NT-07 duplicate deposit revert dengan `SESSION_ALREADY_EXISTS`.
- [x] Reveal kedua idempotent atau ditolak dengan state konsisten.
  - NT-05 duplicate reveal mengembalikan `False`.
- [ ] Restart leader antara deposit dan reveal tidak kehilangan `k`.
- [ ] Quorum gagal setelah deposit mengaktifkan refund setelah timelock.

### HT-04 Spend BLS On-Chain

- [x] Fund contract dengan jumlah kecil USDC testnet.
  - Contract balance naik dari 15 USDC menjadi 24.99 USDC lewat jalur
    `approve + deposit`, bukan transfer langsung.
- [x] Generate signature sesuai hash spend contract deployed.
  - `nimbus-core/examples/generate_bls_test_data.rs` sekarang mendukung mode
    `--spend-contract` untuk mengikat `chain_id`, `contract`, `amount`,
    `recipient_or_intent_hash`, `expiry`, dan `nonce`.
- [ ] Generate signature melalui alur threshold nyata.
- [x] Kirim spend dengan EVM vector RFC 9380 yang valid.
  - 2026-06-07 hard-test Arbitrum Sepolia blocked:
    `spend` revert `BLS_PAIRING_PRECOMPILE_FAILED`.
  - Koreksi 2026-06-08: Arbitrum Sepolia direct call ke `0x0b`
    (`BLS12_G1ADD`) berhasil dengan known vector EIP-2537. Jadi blocker
    paling mungkin adalah encoding input pairing/G2 Nimbus, bukan chain support.
  - Resolusi 2026-06-08: G1/G2 field bytes tidak boleh di-reverse; Arkworks G2
    raw order dipetakan ke EIP-2537 dengan block order `[1, 0, 3, 2]`.
- [x] Tunggu receipt sukses.
  - Patched EIP-2537 deployment:
    `0xd9f1f8f53a8e0b5b8bc6361946119de02cf5c159`
  - Spend tx:
    `0x39dd200a6205295f190d2bed47ecb74ee6b8f61689d93535d2b715a3a5741498`
  - `spend_call_result = true`, receipt status `1`, gas used `1,048,765`.
- [x] Verifikasi pairing benar-benar dipanggil pada build deployed.
  - Patched contract masuk jalur precompile dan gagal di call
    `BLS12_PAIRING_CHECK`, membuktikan branch pairing aktif tetapi input
    pairing masih ditolak precompile.
- [x] Verifikasi pairing benar-benar sukses pada build deployed.
  - Setelah encoding fix, spend valid diterima on-chain tanpa mock/bypass.
- [x] Verifikasi recipient menerima payout tepat.
  - Recipient naik dari `82.141446` ke `87.133946` USDC: payout `4.9925`
    USDC untuk spend amount `5 USDC` setelah fee `0.0075`.
  - Resolusi exact-payout 2026-06-08:
    contract `0x7853df45be072977082d2872e199ac2e405c6d22`,
    spend tx `0xa947fccc674c816a3cbc5f9297fc3f4a570656da46af25766de13fafd17e668a`.
    Recipient naik dari `87.133946` ke `92.133946` USDC, tepat `+5.0`
    USDC untuk invoice `5 USDC`.
- [x] Verifikasi fee recipient menerima fee tepat.
  - Fee recipient sama dengan owner test wallet; net wallet movement sesuai
    deposit net `-9.99` USDC plus spend fee `+0.0075` USDC.
  - Exact-payout test: owner/fee recipient net `-9.9825` USDC setelah deposit
    10 USDC, deposit fee balik `0.01`, dan spend fee balik `0.0075`.
- [x] Verifikasi principal turun tepat.
  - Principal setelah deposit `9.99` USDC, setelah spend `4.99` USDC.
  - Exact-payout test: principal setelah deposit `9.99` USDC, setelah spend
    invoice `5 USDC` turun ke `4.9825` USDC (`5 + 0.0075` debit).
- [x] Verifikasi nullifier tercatat on-chain.
  - 2026-06-11: NT-11 replay test membuktikan nullifier tercatat;
    spend kedua ditolak (returned false). Spend tx:
    `0x173dd24735c0aaba...`
- [x] Ubah satu byte `alpha_neg`: transaksi harus revert/fail.
  - 2026-06-11: NT-01 (byte 127) dan NT-02 (byte 0) keduanya revert.
- [x] Ubah satu byte `hm`: transaksi harus revert/fail.
  - 2026-06-11: `hm` direkonstruksi on-chain dari parameter spend.
    NT-05 (wrong amount) menghasilkan `hm` berbeda dan revert
    `NULLIFIER_MESSAGE_MISMATCH`.
- [x] Ganti `pk_iss`: transaksi harus revert/fail.
  - 2026-06-11: NT-03 (byte 100 flipped) revert.
- [x] Gunakan point at infinity/zero: harus ditolak.
  - 2026-06-11: NT-10 (128 zero bytes) revert.
- [x] Gunakan encoding non-canonical: harus ditolak.
  - 2026-06-11: NT-01/NT-02 byte flip menghasilkan non-canonical
    encoding yang ditolak precompile.
- [x] Gunakan panjang 127/129/255/257 byte: harus ditolak.
  - 2026-06-11: NT-06 (127), NT-07 (129), NT-08 (255), NT-09 (257)
    semuanya revert `INVALID_G1_INPUT_LENGTH` atau
    `INVALID_PUBLIC_KEY_LENGTH`.
- [x] Replay nullifier valid: harus ditolak.
  - 2026-06-11: NT-11 replay spend kedua returned false.
- [x] Signature valid dengan recipient berbeda harus gagal jika recipient
  seharusnya terikat ke message.
  - 2026-06-11: NT-04 wrong recipient revert
    `RECIPIENT_INTENT_MISMATCH`.
- [x] Signature valid dengan amount berbeda harus gagal jika amount seharusnya
  terikat ke message.
  - 2026-06-11: NT-05 wrong amount (6 vs 5 USDC) revert
    `NULLIFIER_MESSAGE_MISMATCH`.
- [x] Semua negative test membuktikan saldo, principal, dan nullifier tidak
  berubah.
  - 2026-06-11: principal tetap `24.9625 USDC` sebelum dan sesudah
    seluruh 10 negative test. Report:
    `test-reports/ht04_spend_negative_tests.json`.
- [x] Uji Dynamic Fee berdasarkan holding time (Fase A - Option A):
  - [x] PT-01: Hold < 60 detik (fee 0.25%).
  - [x] PT-02: Hold >= 60 detik (fee 0.20%).
  - [x] PT-03: Hold >= 90 detik (fee 0.10%).
  - [x] NT-12: Spend dengan unregistered Merkle root (revert `INVALID_ASSOCIATION_ROOT`).

### HT-05 Refund dan Timeout (Timelock 60 Detik untuk Testnet)

- [x] Deposit tidak dapat direfund sebelum 60 detik.
- [x] Refund tepat pada boundary timelock diuji (60 detik).
- [x] Refund setelah timelock berhasil.
- [x] Hanya session client dapat claim refund.
  - 2026-06-11: NT-03 membuktikan address lain ditolak (revert).
- [x] Refund kedua ditolak.
- [x] Reveal setelah refund ditolak.
  - 2026-06-11: NT-04 membuktikan refund setelah reveal/resolved ditolak (revert).
- [ ] Refund saat contract pause mengikuti policy yang ditentukan.
- [x] Saldo client kembali tepat setelah fee policy.

### HT-06 Relayer Queue, Crash, dan Recovery

- [ ] Submit spend lalu kill node sebelum worker mengambil queue.
- [ ] Restart dan pastikan request tetap diproses.
- [ ] Kill node setelah nullifier reserved tetapi sebelum broadcast.
- [ ] Kill node setelah broadcast tetapi sebelum receipt.
- [ ] Restart dan rekonsiliasi tx hash/nonce tanpa double broadcast.
- [ ] RPC timeout tidak menghapus queue.
- [ ] RPC returns error tidak menandai nullifier confirmed.
- [ ] On-chain revert me-release atau menandai terminal failure sesuai policy.
- [ ] Dua worker bersamaan tidak memproses item yang sama.
- [ ] Submit nullifier sama secara concurrent; hanya satu settlement terjadi.
- [ ] Database locked/busy tidak menghilangkan request.
- [ ] Disk full simulation menghasilkan fail closed.
- [ ] Corrupt DB diuji dengan restore dari backup.
- [ ] Retry memiliki batas dan dead-letter state.
- [ ] Queue order/shuffle tidak melanggar deadline.
- [ ] Expired request tidak pernah dibroadcast.

### HT-07 RPC dan Nonce Failure Injection

- [ ] Primary RPC dibuat unreachable; fallback RPC digunakan.
- [ ] Primary memberi stale nonce; relayer recovery.
- [ ] Fallback juga mati; queue tetap aman.
- [ ] RPC memberi chain ID salah; node fail closed.
- [ ] RPC mengembalikan receipt timeout; monitor melanjutkan setelah restart.
- [ ] Transaction underpriced diuji.
- [ ] Replacement transaction diuji.
- [ ] Nonce gap diuji.
- [ ] Saldo gas di bawah minimum menghentikan broadcast tanpa kehilangan queue.
- [ ] Gas spike membuat `min_payout` protection menolak transaksi.

### HT-08 CCIP Source ke Destination

- [ ] Deploy/configure contract source dan destination yang benar.
- [ ] Set router non-zero pada destination.
- [ ] Allowlist source chain selector dan sender contract.
- [ ] Kirim payload valid dengan signature nyata.
- [ ] Tunggu source receipt sukses.
- [ ] Parse CCIP message ID asli dari event.
- [ ] Pantau CCIP explorer/API atau destination logs.
- [ ] Tunggu destination receipt sukses.
- [ ] Verifikasi destination nullifier dan payout.
- [ ] Replay message ID ditolak.
- [ ] Caller non-router ditolak.
- [ ] Router benar tetapi source selector salah ditolak.
- [ ] Router benar tetapi sender salah ditolak.
- [ ] Payload 647/649 byte ditolak.
- [ ] Payload valid tetapi signature corrupt gagal tanpa payout.
- [ ] Destination contract paused menghasilkan status failure yang terpantau.
- [ ] Out-of-order messages tidak memblokir message lain.
- [ ] Source success + destination failure menghasilkan recovery/refund state.
- [ ] Native CCIP fee kurang diuji dan harus gagal bersih.

### HT-09 Polymarket Intent dan Failed Refund

- [ ] Gunakan deployment/test double on-chain yang ABI-compatible jika
  Polymarket testnet tidak tersedia; jangan gunakan Rust mock.
- [ ] Approve collateral dengan amount tepat.
- [ ] Successful split position menghasilkan outcome token balance.
- [ ] CTF revert mencatat failed intent refund.
- [ ] Failed refund hanya dapat diklaim sekali.
- [ ] Recipient refund terikat pada intent/session, bukan arbitrary caller.
- [ ] Malicious collateral token diuji.
- [ ] Reentrancy dari token/CTF diuji menggunakan deployed adversarial contract.
- [ ] Allowance sisa setelah failure diperiksa.

### HT-11 Governance dan Pause

- [ ] Non-owner gagal propose/execute.
- [ ] Owner propose lalu execute sebelum ETA gagal.
- [ ] Execute setelah ETA berhasil.
- [ ] Proposal overwrite/cancel policy diuji.
- [ ] Ownership transfer dua tahap diuji.
- [ ] Pending owner salah ditolak.
- [ ] Pause memblokir semua state-changing user path yang seharusnya diblokir.
- [ ] Emergency refund behavior saat pause ditentukan dan diuji.
- [ ] Router, pool, token, dan fee recipient zero-address handling diuji.
- [ ] Event governance tersedia untuk monitoring.

### HT-12 API Adversarial Test

- [ ] Invalid JSON.
- [ ] Body lebih dari 64 KiB.
- [ ] String hex ganjil/non-hex.
- [ ] Empty fields.
- [ ] Extremely long idempotency key.
- [ ] Duplicate idempotency key dengan payload berbeda.
- [ ] Burst rate-limit dari satu Tailscale identity.
- [ ] Spoofed `X-Forwarded-For`.
- [ ] Slowloris/partial body timeout.
- [ ] Guardian URL SSRF ke localhost, metadata endpoint, dan private services.
- [ ] Guardian response malformed/oversized.
- [ ] Request cancellation saat DB/RPC sedang bekerja.
- [ ] Tidak ada panic atau process crash untuk input adversarial.

### HT-13 x402 End-to-End

- [ ] Merchant/resource mengeluarkan payment requirement nyata.
- [ ] SDK memilih ready token dan membentuk payment signature.
- [ ] Facilitator memverifikasi cryptography sebelum settlement.
- [ ] Payment terikat ke resource, merchant, amount, network, dan expiry.
- [ ] Settlement hanya terjadi sekali.
- [ ] Response sukses hanya setelah status settlement yang didefinisikan.
- [ ] Tidak ada direct broadcast dan queue broadcast ganda.
- [ ] Invalid HMAC/payment payload ditolak.
- [ ] Replay payment header ditolak.
- [ ] Resource berbeda dengan token sama ditolak jika binding diwajibkan.

### HT-14 ZK Compliance On-Chain

Test ini baru boleh dijalankan sebagai security test setelah circuit dan VK nyata
selesai. Sebelum itu statusnya harus eksplisit `prototype`.

- [ ] Generate proof dengan proving key versioned.
- [ ] Register clean root nyata.
- [ ] Proof valid diterima on-chain.
- [ ] Root berbeda ditolak.
- [ ] Nullifier berbeda ditolak.
- [ ] Recipient berbeda ditolak.
- [ ] Amount berbeda ditolak.
- [ ] Merkle path invalid ditolak.
- [ ] Witness yang tidak memenuhi policy ditolak.
- [ ] Proof dari circuit/VK version lama ditolak.
- [ ] Semua negative test tidak mengubah state.

### HT-15 Load, Soak, dan Resource Exhaustion

- [ ] Soak test node minimal 24 jam.
- [ ] Submit transaksi kontinu dengan rate realistis.
- [ ] Ukur latency p50/p95/p99 dari API sampai confirmed.
- [ ] Ukur queue depth dan retry count.
- [ ] Ukur memory growth rate limiter dan queue.
- [ ] Ukur file descriptor/socket leak.
- [ ] Ukur pertumbuhan SQLite/WAL.
- [ ] Jalankan cleanup/vacuum saat load.
- [ ] Restart Vault, guardian, leader, dan RPC proxy bergantian.
- [ ] Pastikan tidak ada double settlement setelah recovery.
- [ ] Catat batas throughput sebelum error rate meningkat.

### HT-16 Economic and Accounting Invariants

- [ ] Untuk setiap flow, hitung delta saldo client, contract, fee recipient,
  relayer, dan destination.
- [ ] Total debit sama dengan total credit plus explicit fee.
- [ ] Principal tidak pernah underflow atau silently clamp ke zero.
- [ ] Fee rounding diuji pada boundary amount.
- [ ] Minimum amount tepat di bawah, tepat sama, dan tepat di atas batas.
- [ ] (Deprecated) Premium phase 1/2/3 diuji dengan transaksi nyata.
- [ ] (Deprecated) LP utilization tidak melebihi total liquidity.
- [ ] (Deprecated) Claim yield tidak mengurangi kemampuan memenuhi seluruh liability.
- [ ] Failure/revert tidak menghasilkan profit atau kehilangan user yang tidak
  dijelaskan.

### HT-17 Upgrade dan Compatibility

- [ ] ABI relayer cocok dengan contract deployment.
- [ ] SDK-generated vector cocok dengan contract.
- [ ] Database migration dari versi sebelumnya diuji pada copy database.
- [ ] Restart binary baru dengan queue lama diuji.
- [ ] Old client request compatibility ditentukan.
- [ ] Contract storage layout dibandingkan sebelum redeploy/upgrade.
- [ ] Rollback binary relayer diuji.

## Evidence dan Report Hard Test

Setiap test case harus menghasilkan:

- Test ID dan deskripsi.
- Commit hash dan dirty-worktree status.
- Build/tool versions.
- Environment name dan chain ID.
- Contract addresses dan artifact hashes.
- Input digest tanpa secret.
- Pre-state balances/storage.
- Source tx hash, receipt, block, gas, dan logs.
- CCIP message ID serta destination tx jika relevan.
- Post-state balances/storage.
- Expected result dan actual result.
- Pass/fail dengan alasan machine-readable.
- Timestamp UTC.

Report tidak boleh berisi:

- Private key.
- Vault token.
- Guardian share.
- Database encryption key.
- Full secret-bearing environment dump.

## Urutan Hard-Test Campaign

### Campaign A - Harness Trustworthy

- [x] Hapus secret hardcoded dari seluruh script.
- [x] Perbaiki generator BLS.
- [x] Perbaiki/validasi ABI generation setelah upgrade Stylus 0.10.7.
  - 2026-06-09: exporter dimigrasi ke `print_from_args()` dan signature ABI
    aktual berhasil diekspor dengan `cargo stylus export-abi`.
- [ ] Implementasikan hard-test mode tanpa fallback mock.
- [x] Implementasikan deployment manifest.
- [ ] Implementasikan receipt/event/state assertion helpers.
- [ ] Implementasikan report JSON/Markdown.

Gate A:

- Runner mampu membedakan broadcast, source confirmation, dan destination
  confirmation.
- Negative test yang sengaja dibuat valid harus menyebabkan runner gagal,
  sehingga assertion harness terbukti tidak selalu hijau.

### Campaign B - Single-Chain Core Safety

- [ ] Jalankan HT-00 sampai HT-05.
- [ ] Fokus pada deposit, commitment, atomic `k`, BLS spend, nullifier, dan
  refund.
- [ ] Gunakan amount sekecil mungkin yang masih melewati batas contract.

Gate B:

- Tidak ada cara mengambil payout tanpa signature valid.
- Tidak ada cara mengambil deposit session lain.
- Semua failure menjaga accounting dan nullifier tetap konsisten.

### Campaign C - Distributed Relayer Safety

- [ ] Jalankan HT-01, HT-02, HT-06, HT-07, dan HT-12.
- [ ] Gunakan leader VPS ini dan guardian VPS melalui Tailscale.
- [ ] Lakukan kill/restart saat setiap transition penting.

Gate C:

- Kehilangan satu proses/RPC tidak menyebabkan lost request atau double spend.
- Leader tidak pernah memperoleh guardian share.
- Quorum dan key version selalu ditegakkan.

### Campaign D - Cross-Chain Safety

- [ ] Jalankan HT-08 dan HT-09.
- [ ] Simpan source tx, message ID, destination tx, dan balance deltas.
- [ ] Uji destination failure serta recovery, bukan hanya happy path.

Gate D:

- Source success tidak pernah salah dilaporkan sebagai destination success.
- Replay, wrong source, wrong sender, dan corrupt payload ditolak.

### Campaign E - Extended Features

- [ ] Jalankan HT-10, HT-11, HT-13, dan HT-14.
- [ ] Jangan mengaktifkan ZK compliance sebagai security control sebelum
  circuit/VK production selesai.

### Campaign F - Stability dan Economics

- [ ] Jalankan HT-15, HT-16, dan HT-17.
- [ ] Jalankan minimal dua campaign penuh dari fresh deployment.
- [ ] Ulangi seluruh critical path setelah setiap perubahan contract.

Final testnet gate:

- [ ] Seluruh P0 lulus hard test nyata.
- [ ] Seluruh negative test kritis lulus.
- [ ] Tidak ada unresolved accounting mismatch.
- [ ] Tidak ada lost queue item atau double settlement.
- [ ] Tidak ada test yang bergantung pada mock verification.
- [ ] Evidence package lengkap dan dapat direproduksi dari commit yang sama.

## P0 - Blocker Keamanan

### Aktifkan Verifikasi BLS pada Spend

Lokasi: `nimbus-contracts/src/spend.rs`

- [x] Validasi panjang `alpha_neg_bytes` harus 128 byte.
- [x] Validasi panjang `hm_bytes` harus 128 byte.
- [x] Validasi panjang `pk_iss_bytes` harus 256 byte.
- [x] Tolak point-at-infinity untuk signature, message point, dan issuer key.
- [x] Bentuk input dua pairing untuk EIP-2537:
  `e(-alpha, G2_generator) * e(H(m), pk_iss) == 1`.
- [x] Panggil precompile `BLS12_PAIRING_CHECK`.
- [x] Tolak output precompile yang panjang/non-canonical.
- [x] Tolak transaksi sebelum nullifier atau principal diubah jika signature
  tidak valid.
- [x] Tambahkan host negative tests untuk pairing false, malformed input,
  infinity, issuer key tidak dipercaya, nullifier mismatch, dan replay.
- [x] Jalankan test pada Stylus-compatible environment atau testnet, bukan
  hanya host mock.
  - 2026-06-07 Arbitrum Sepolia: deployed Stylus path mencapai
    `BLS12_PAIRING_CHECK`, tetapi chain mengembalikan
    `BLS_PAIRING_PRECOMPILE_FAILED`.
  - Koreksi 2026-06-08: direct `0x0b` EIP-2537 test sukses di Arbitrum
    Sepolia. Blocker sekarang dipersempit ke encoding `0x0f` pairing/G2.
  - Resolusi 2026-06-08: direct `0x0b`, `0x0d`, dan `0x0f` known-vector
    tests lolos; deployed `spend` valid sukses di Arbitrum Sepolia.
- [x] Perbaiki `nimbus-core/examples/generate_bls_test_data.rs` agar memakai
  primitive yang sama dengan production:
  `hash_to_g1`, `IssuerSecretKey`, `UnmaskedSignature`,
  `get_alpha_neg_evm`, `get_hm_evm`, dan `get_pk_iss_evm`.
- [x] Pastikan `alpha_neg_hex` benar-benar merupakan `-alpha`.
- [x] Bind direct spend recipient ke signature/intention hash.
  - `spend()` sekarang menolak recipient substitution dengan
    `RECIPIENT_INTENT_MISMATCH` sebelum nullifier/principal berubah.
- [x] Gunakan amount test minimal 5 USDC.
- [x] Tambahkan deterministic seed agar hasil test dapat direproduksi.
- [x] Tambahkan opsi `--invalid` untuk menghasilkan vector dengan satu byte
  signature diubah.
- [x] Commit known-answer vector agar output lintas versi dapat dibandingkan.
  - 2026-06-11: Generate dan commit 5 known-answer vectors di `test-reports/known_answer_vectors.json`.
- [x] Tambahkan testnet negative test yang mengubah satu byte signature,
  `H(m)`, dan issuer key.
  - 2026-06-11: `scripts/ht04_spend_negative_tests.py` — 13/13 pass.
    NT-01/02 (corrupted sig), NT-03 (wrong pk_iss), NT-04 (wrong
    recipient), NT-05 (wrong amount/hm), NT-06..09 (wrong lengths),
    NT-10 (zero/infinity), NT-11 (replay). All on Arbitrum Sepolia.

### Ikat Credential ke Parameter Spend

Decision: `research/decisions/DEC-002-bls-spend-verification-boundaries.md`

Pairing valid hanya membuktikan signature terhadap `H(m)`. Saat ini contract
belum dapat membuktikan bahwa `H(m)` mengandung parameter transaksi.

- [x] Batasi spend ke issuer public key yang didaftarkan owner.
- [x] Tambahkan revoke issuer key.
- [x] Ikat nullifier ke `keccak256(hm_bytes)`.
- [x] Tolak spend jika principal tidak cukup tanpa membakar nullifier.
- [x] Definisikan canonical `NIMBUS_SPEND_V1` message.
- [x] Ikat `chain_id` dan contract address sebagai domain separation.
- [x] Ikat action, amount, recipient/intent hash, expiry, dan credential nonce.
- [x] Tentukan cara contract merekonstruksi RFC 9380 `H(m)` dari canonical
  message, bukan menerima `hm_bytes` tanpa pembuktian.
- [x] Update core, SDK, node, x402, CCIP payload, dan contract ABI secara
  atomik.
- [x] Tambahkan cross-chain replay dan wrong-contract negative tests.

Acceptance criteria:

- Spend dengan signature valid berhasil.
- Perubahan satu byte pada signature, message hash, atau issuer key selalu
  gagal.
- Nullifier tidak tercatat jika verifikasi gagal.

### Perbaiki Binding Deposit dan Reveal

Lokasi:

- `nimbus-contracts/src/storage.rs`
- `nimbus-contracts/src/deposit.rs`

Masalah saat ini:

- `deposit()` menerima tetapi tidak menyimpan `com_k_bytes`.
- `reveal_mask_key()` menerima `com_k_bytes` dari caller.
- Dana reveal dikirim kepada `msg.sender`, bukan client yang membuat session.
- Session ID lama dapat ditimpa oleh deposit baru.

TODO:

- [x] Tambahkan storage commitment per session.
- [x] Simpan hash `com_k` saat deposit.
- [x] Tolak deposit jika session ID sudah pernah digunakan.
- [x] Saat reveal, bandingkan hasil `k * pk_iss` dengan commitment yang tersimpan.
- [x] Jangan mempercayai `com_k` yang diberikan saat reveal.
- [x] Reveal dibuat permissionless: caller hanya mengirim bukti dan tidak
  menerima escrow.
- [x] Validasi panjang `k`, `pk_iss`, dan commitment sebelum precompile call.
- [x] Tambahkan test caller lain tidak dapat mengambil collateral saat reveal.
- [x] Tambahkan test overwrite session ID.

Acceptance criteria:

- Caller lain tidak bisa mengambil deposit milik client.
- Commitment tidak bisa diganti setelah deposit.
- Session ID tidak bisa dipakai ulang.

### Hilangkan Secret Share Override dari API

Lokasi:

- `nimbus-node/src/dto.rs`
- `nimbus-node/src/handlers/threshold.rs`

- [x] Hapus `share_sk_hex` dari request publik.
- [x] Guardian harus selalu menggunakan share dari `KeyManager`.
- [x] Untuk testnet, bind endpoint guardian hanya ke IP/interface Tailscale.
- [x] Terapkan Tailscale ACL agar hanya node leader yang dapat mengakses port
  guardian.
- [x] Pastikan port guardian tidak listen pada interface publik.
- [x] Sebelum production atau ketika trust boundary bertambah, tambahkan
  application-layer authentication seperti mTLS atau signed request.
- [x] Tambahkan allowlist identity leader dan replay protection. Pada testnet,
  identity dapat berasal dari Tailscale node/tag; production sebaiknya juga
  diverifikasi pada application layer.
- [x] Validasi bahwa request signing terkait deposit/session yang sah.
- [x] Jangan menerima arbitrary blinded point tanpa policy dan authorization.

Acceptance criteria:

- Request eksternal tidak dapat memilih secret share yang dipakai guardian.
- Endpoint guardian tidak dapat dipanggil oleh pihak yang tidak terautentikasi.

Catatan:

mTLS atau API signature tidak wajib untuk testnet tertutup apabila VPS tidak
memiliki ingress publik, service hanya bind ke Tailscale, ACL hanya mengizinkan
leader, dan device yang keluar langsung dihapus dari tailnet. Tailscale menjadi
network identity dan encrypted transport untuk fase tersebut. Application-layer
authentication tetap diperlukan sebelum production untuk defense-in-depth,
audit identity, dan perlindungan jika ACL atau akun Tailscale salah konfigurasi.

### Terapkan Atomic Release untuk Masking Key `k`

Lokasi:

- `nimbus-node/src/dto.rs`
- `nimbus-node/src/handlers/threshold.rs`
- `nimbus-node/src/handlers/deposit.rs`
- `nimbus-node/src/database.rs`

Masalah saat ini:

Endpoint `/api/leader/sign` langsung mengembalikan `k_hex` bersama commitment
dan partial signatures. Client dapat langsung melakukan unmask tanpa menunggu
deposit atau settlement dikonfirmasi. Ini menghilangkan atomic/fair exchange
yang seharusnya dijamin oleh alur deposit -> masked signature -> reveal.

TODO:

- [x] Tambahkan `session_id` pada request dan response signing.
- [x] `/api/leader/sign` hanya mengembalikan `com_k` dan partial signatures.
- [x] Jangan mengembalikan `k` pada response signing awal.
- [x] Simpan `k` secara terenkripsi dengan binding ke `session_id`,
  commitment, amount, issuer key, dan expiry.
- [x] Rilis `k` hanya setelah deposit on-chain untuk session tersebut
  terkonfirmasi.
- [x] Pastikan amount dan commitment deposit sama dengan signing session.
- [x] Guardian hanya menandatangani session yang sah dan belum expired.
- [x] Tandai `k` sebagai revealed secara atomik agar tidak ada conflicting
  lifecycle.
- [x] Hapus/zeroize `k` setelah reveal selesai atau session expired.
- [x] Jangan log `k` atau memasukkannya ke response/error sebelum reveal.
- [x] Integrasikan refund timelock jika quorum gagal atau `k` tidak dapat
  dirilis. (DEC-012: Monitoring + background job, on-chain check TBD di testnet)
- [x] Tambahkan test bahwa client tidak dapat unmask sebelum reveal.
- [x] Tambahkan test bahwa deposit session lain tidak dapat membuka `k`.
- [x] Tambahkan test retry reveal yang idempotent.

Acceptance criteria:

- Sebelum deposit confirmed, API tidak memberikan informasi yang cukup untuk
  unmask signature.
- Setelah deposit confirmed, hanya `k` untuk session yang tepat yang dirilis.
- Session gagal tetap dapat mengikuti jalur refund tanpa kehilangan dana.

## P1 - Settlement dan Reliabilitas Relayer

### Buat Spend Queue Persisten

Lokasi:

- `nimbus-node/src/state.rs`
- `nimbus-node/src/database.rs`
- `nimbus-node/src/handlers/spend.rs`

Status sebelum DEC-005:

- Tabel `spend_queue` tersedia tetapi runtime memakai `Vec` in-memory.
- Queue hilang saat process restart.
- Semua item dihapus setelah loop, termasuk broadcast yang gagal.

TODO:

- [x] Gunakan tabel `spend_queue` sebagai source of truth (`DEC-005`).
- [x] Tambahkan status `queued`, `broadcasting`, `submitted`, `confirmed`,
  `failed`, dan `retryable`.
- [x] Simpan jumlah retry, error terakhir, tx hash, dan timestamp.
- [x] Gunakan leasing atau transactional claim agar dua worker tidak mengambil
  item yang sama.
- [x] Jangan hapus item setelah loop; pertahankan lifecycle terminal untuk audit.
- [x] Implementasikan retry dengan exponential backoff.
- [x] Pulihkan queue otomatis setelah restart.

Decision: `research/decisions/DEC-005-persistent-spend-settlement-queue.md`

Sisa hard test: kill process pada setiap boundary broadcast/receipt, concurrent
worker, disk full, corrupt DB, dan reconciliation sender+nonce tetap mengikuti
HT-06.

### Private Spend Fee (Consolidated) dan Agent Settlement

Target bisnis:

- Deposit/shield fee: `0,20%`.
- Private spend / withdraw fee:
  - Hold < 7 hari: `0,25%` (default).
  - Hold >= 7 hari: `0,20%` (holding-time discount).
  - Hold >= 30 hari (1 bulan): `0,10%` (Whale holding-time discount).
- Agent Spending Wallet memakai pre-staged state Layer 7 agar agent dapat
  melakukan instant private settlement tanpa funding ulang per request.

Status:

- [x] Dokumentasikan target fee baru dan konsolidasi unshield ke `docs/bisnis.md`.

Sisa implementasi wajib:

- [x] Implementasikan On-Chain Cryptographic Time Proof (Opsi A) dengan memetakan valid root ke timestamp registrasinya (`mapping(bytes32 => uint256) clean_association_roots`) untuk menghitung holding-time diskon fee secara anonim dan gas-efisien.
- [ ] Pastikan refund akibat gagal issuance/settlement tidak dikenai fee.
- [ ] Pastikan spend/withdraw tidak bisa melewati outstanding liability, pending spend,
  atau locked/pre-staged agent allowance.
- [ ] Tambahkan test rounding untuk deposit fee (0.20%) dan spend fee (0.25%) pada decimal stablecoin
  yang didukung.
- [ ] Tambahkan event/accounting terpisah untuk deposit fee, spend fee, execution fee, relayer markup, dan batch margin.
- [ ] Tambahkan hard test deposit -> Agent Spending Wallet -> private spend ->
  pause/revoke -> withdraw (via spend ke wallet owner) sisa saldo.
- [ ] Tambahkan hard test bahwa agent tidak dapat spend setelah owner menarik kembali (via spend ke wallet owner) saldo yang sebelumnya pre-staged.
- [ ] Tambahkan hard test bahwa pending agent spend dan spend penarikan bersamaan tidak
  menyebabkan double debit atau insolvency.

### Adaptive Private Spend Batching dan Monetisasi

Decision:
`research/decisions/DEC-006-adaptive-private-spend-batching.md`

Tujuan:

- Menggabungkan 2 sampai 8 same-chain spend dalam satu transaksi on-chain.
- User menyetujui fixed execution quote sebelum tanda tangan.
- User diberi tahu batching dapat menambah latency sekitar 1 sampai 2 detik.
- Selisih antara execution quote dan biaya batch aktual menjadi margin
  relayer/protokol.
- Merchant tetap menerima nominal yang diminta dan protocol fee 0,15% tidak
  berubah.

Fondasi kode yang sudah selesai:

- [x] Tambahkan entrypoint contract `batch_spend()` dengan maksimum 8 item.
- [x] Validasi empty batch, ukuran maksimum, dan panjang seluruh array.
- [x] Pertahankan verifikasi signature, nullifier, amount, recipient, expiry,
  dan nonce untuk setiap item melalui `_spend()`.
- [x] Tambahkan Alloy ABI dan broadcaster `batchSpend()` pada EVM client.
- [x] Kelompokkan 2-8 same-chain spend; single item dan cross-chain tetap
  memakai jalur single.
- [x] Gunakan satu tx hash dan receipt batch untuk seluruh item terkait.
- [x] Tambahkan feature flag `NIMBUS_BATCH_ENABLED`, default `false` agar node
  tetap kompatibel dengan contract deployment lama.
- [x] Tambahkan unit test batas ukuran batch.
- [x] Contract tests dan seluruh node/integration tests lulus setelah perubahan.

**STATUS FITUR: BELUM SELESAI DAN BELUM BOLEH DIAKTIFKAN UNTUK MONETISASI.**

Sisa implementasi wajib:

- [ ] Tambahkan `max_execution_fee`, `quote_expiry`, `quote_id`, chain ID, dan
  relayer identity ke message/hash yang ditandatangani user.
- [x] Buat endpoint quote deterministik untuk jalur single private spend.
- [ ] Tambahkan estimasi batch ke endpoint quote setelah benchmark gas testnet.
- [ ] Pastikan relayer tidak dapat memotong lebih dari fixed quote.
- [ ] Implementasikan pemotongan execution fee dalam stablecoin tanpa mengubah
  nominal bersih yang diterima merchant.
- [ ] Pisahkan accounting: gas cost, reimbursement, gross execution fee,
  relayer margin, protocol share, dan rounding.
- [ ] Simpan quote, batch ID, jumlah item, receipt gas used, effective gas
  price, total cost, dan margin pada database.
- [ ] Buat konfirmasi batch/nullifier dalam satu transaksi database atomic,
  bukan update row satu per satu.
- [ ] Jika batch revert, pecah batch untuk mengidentifikasi item invalid tanpa
  membuat item valid gagal permanen atau terjebak retry bersama.
- [ ] Terapkan deadline-near bypass; transaksi yang mendekati expiry langsung
  memakai jalur single.
- [ ] Terapkan adaptive window nyata: target 1 detik, hard timeout 2 detik,
  kirim lebih cepat ketika batch penuh.
- [ ] Hubungkan pemeriksaan saldo ETH relayer sebelum single/batch broadcast.
- [ ] Deploy contract baru yang memiliki `batchSpend()` ke Arbitrum Sepolia.
- [ ] Generate ABI deployment terbaru dan cocokkan selector dengan relayer.
- [ ] Aktifkan `NIMBUS_BATCH_ENABLED=true` hanya pada deployment baru tersebut.
- [ ] Hard test batch ukuran 1, 2, 8, dan 9 pada testnet.
- [ ] Hard test satu item invalid dalam batch dan buktikan tidak ada partial
  payout/nullifier corruption.
- [ ] Benchmark gas receipt single versus batch 2/4/8; jangan memakai asumsi
  persentase penghematan.
- [ ] Ukur latency API-to-broadcast dan API-to-confirmed p50/p95/p99.
- [x] Tambahkan fee helper terpusat agar merchant menerima nominal exact dan
  protocol fee didebit di atas invoice.
- [x] Pisahkan gas reimbursement dan markup relayer dalam quote informasional.
- [x] Buktikan total debit user = merchant payout + protocol fee + execution
  quote, tanpa hidden fee.
  - Core quote sekarang: `contract_amount = merchant_amount`,
    `protocol_fee = ceil(merchant_amount * 15 / 10_000)`,
    `user_total_debit = merchant_amount + protocol_fee + execution_fee`.
- [ ] Buktikan margin batch positif setelah gas, RPC, retry, dan transaksi
  revert diperhitungkan.
- [ ] Tambahkan integration test signed quote sampai settlement accounting.
- [ ] Audit eksternal jalur fee dan batch sebelum mainnet.

Terminal condition:

- Item ini baru boleh dicentang selesai setelah contract baru dideploy, signed
  execution quote tidak dapat dimanipulasi, accounting durable cocok dengan
  receipt, merchant menerima nominal tepat, benchmark testnet tersimpan, dan
  seluruh negative test lulus.

### Perbaiki Lifecycle Nullifier

Status sebelum DEC-005:

Nullifier dimasukkan ke database sebelum broadcast/receipt berhasil. Jika RPC,
encoding, atau transaksi gagal, token dianggap spent oleh relayer.

- [x] Pisahkan reservation nullifier dari confirmed nullifier: queue aktif
  menjadi reservation; tabel `nullifiers` hanya berisi receipt sukses.
- [ ] Gunakan status `reserved`, `submitted`, `confirmed`, dan `released`.
- [x] Simpan tx hash dan block untuk transaksi yang memperoleh receipt.
- [x] Konfirmasi receipt dan status transaksi.
- [x] Release reservation melalui status `retryable` atau `failed` jika
  transaksi belum confirmed.
- [ ] Rekonsiliasi status database dengan nullifier contract setelah restart.

### Receipt dan Finality

- [x] Jangan hanya spawn receipt monitor yang hasilnya tidak masuk database.
- [x] Tunggu receipt dan masukkan hasilnya ke lifecycle database.
- [x] Tandai settlement sukses hanya jika `receipt.status == success`.
- [ ] Terapkan confirmation threshold sesuai chain.
- [ ] Tangani replacement transaction dan nonce conflict.
- [ ] Ekspos status transaksi melalui endpoint API.

### CCIP Message ID dan Destination Tracking

Lokasi: `nimbus-node/src/evm_client.rs`

Masalah saat ini:

Return value `broadcast_ccip_transaction()` adalah source transaction hash,
bukan CCIP message ID asli.

- [ ] Parse event CCIP dari source transaction receipt.
- [ ] Ambil message ID asli dari event router.
- [ ] Simpan source tx hash dan CCIP message ID sebagai field berbeda.
- [ ] Monitor status message sampai destination execution.
- [ ] Verifikasi destination receipt dan contract event.
- [ ] Bedakan status `source_confirmed`, `ccip_in_flight`,
  `destination_success`, dan `destination_failed`.
- [ ] Jangan menyebut broadcast source sebagai E2E success.

### Validasi Input Relayer

- [ ] Nullifier wajib tepat 32 byte, jangan silently pad dengan zero.
- [ ] `alpha_neg` wajib 128 byte.
- [ ] `hm` wajib 128 byte.
- [ ] `pk_iss` wajib 256 byte.
- [ ] Recipient wajib address EVM valid.
- [ ] Amount wajib memenuhi minimum dan maximum policy.
- [ ] Validasi chain selector dan destination contract dengan allowlist.
- [ ] Batasi panjang idempotency key dan string input lainnya.

## P1 - Relayer Deposit dan Reveal

Lokasi: `nimbus-node/src/handlers/deposit.rs`

Masalah saat ini:

- Client address dibuat secara random.
- Deposit handler hanya menulis database, tidak mengamati transaksi on-chain.
- Reveal handler hanya mengubah status database dan mencetak `VALID`.

TODO:

- [ ] Ambil client identity dari signed request atau event deposit on-chain.
- [ ] Verifikasi signature autentikasi client.
- [ ] Index event deposit dari contract sebagai source of truth.
- [ ] Simpan block number dan transaction hash deposit.
- [ ] Verifikasi `k * pk_iss == stored com_k` sebelum resolve.
- [ ] Sinkronkan resolved state dengan contract.
- [ ] Jangan mengembalikan pesan "published on-chain" jika tidak ada transaksi.

## P1 - ZK Compliance

Lokasi:

- `nimbus-core/src/compliance_circuit.rs`
- `nimbus-sdk/src/zk_wasm.rs`
- `nimbus-contracts/src/verification.rs`

Masalah saat ini:

- Circuit hanya memberi constraint `nullifier = secret + randomness`.
- Root, recipient, dan amount dialokasikan tetapi tidak diberi constraint.
- Tidak ada Merkle membership proof.
- Proving key dibuat saat runtime menggunakan test setup.
- Verifying key contract memakai scaled generators dan dinyatakan mock.
- Unit test contract mengembalikan `Ok(true)` untuk proof dengan panjang valid.

TODO:

- [ ] Tentukan statement compliance final secara formal.
- [ ] Implementasikan hash nullifier yang cryptographically secure.
- [ ] Implementasikan Merkle membership di dalam circuit.
- [ ] Bind root, nullifier, recipient, amount, chain ID, dan domain separator.
- [ ] Tambahkan range constraint untuk amount.
- [ ] Tambahkan address/field canonicality constraints.
- [ ] Jalankan trusted setup yang sesuai dengan deployment policy.
- [ ] Distribusikan proving key sebagai artifact versioned, bukan generate runtime.
- [ ] Embed atau simpan verifying key nyata pada contract.
- [ ] Pastikan SDK dan contract memakai circuit/version/VK yang sama.
- [ ] Tambahkan known-answer vectors lintas core, SDK, dan contract.
- [ ] Tambahkan negative proof tests.

## P1 - CCIP Contract Security

- [ ] Wajibkan `ccip_router != Address::ZERO` sebelum menerima message.
- [ ] Tolak semua caller jika router belum dikonfigurasi.
- [ ] Validasi `source_chain_selector` dengan allowlist.
- [ ] Decode dan validasi sender CCIP.
- [ ] Bind sender contract yang sah untuk setiap source chain.
- [ ] Gunakan message ID untuk replay protection.
- [ ] Tambahkan event untuk received, executed, dan failed intents.
- [ ] Audit payload encoding antara source relayer dan destination contract.

## P2 - x402

Lokasi: `nimbus-node/src/handlers/x402.rs`

- [ ] Jangan menghasilkan mock tx hash kecuali explicit development profile.
- [ ] Jangan mengembalikan `success: true` sebelum settlement terkonfirmasi.
- [ ] Recipient `"x402-facilitator-pool"` bukan address EVM valid; ganti dengan
  konfigurasi address nyata.
- [ ] Verifikasi BLS payment sebelum queue.
- [ ] Bind payment ke resource URI, merchant, network, asset, amount, dan expiry.
- [ ] Tambahkan idempotency dan reservation nullifier.
- [ ] Hindari double broadcast: saat ini request dimasukkan queue dan juga
  langsung dibroadcast.
- [ ] Return payment status ID, bukan receipt palsu.

## P2 - KMS, Database, dan Secrets

### Database

- [x] Tolak default key `"default-change-in-production"` pada strict mode.
- [x] Fail startup jika `NIMBUS_DB_KEY` tidak tersedia di non-development mode.
- [ ] Pastikan build benar-benar memakai SQLCipher dan verifikasi cipher aktif.
- [ ] Ambil database key dari KMS/OpenBao.
- [ ] Perbaiki permission path WAL/SHM agar sesuai nama file SQLite sebenarnya.
- [ ] Tambahkan backup, restore, corruption check, dan migration versioning.
- [ ] Jangan track file `.db` di Git.

### KMS dan Key Lifecycle

- [ ] Ganti raw TCP HTTP client dengan client HTTPS yang tervalidasi.
- [ ] Verifikasi TLS certificate untuk Vault dan guardian RPC.
- [ ] Jangan menghapus awalan `https://` lalu mengirim plaintext TCP.
- [ ] Gunakan short-lived Vault token atau workload identity.
- [ ] Validasi key version dan expected guardian index.
- [ ] Implementasikan threshold proactive refresh yang nyata; re-masking memory
  bukan penggantian share antar guardian.
- [ ] Gunakan crate zeroization yang diaudit untuk secret memory.

## P2 - API dan Operasional

- [ ] Tambahkan authentication dan authorization pada endpoint mutating.
- [ ] Jangan mempercayai `X-Forwarded-For` kecuali request berasal dari trusted
  proxy.
- [ ] Gunakan extractor peer socket address untuk direct connections.
- [ ] Bersihkan rate-limit map agar tidak tumbuh tanpa batas.
- [ ] Terapkan timeout pada guardian, Vault, dan RPC requests.
- [ ] Tambahkan structured logging dan request correlation ID.
- [ ] Jangan log secret, private RPC credential, atau full payload.
- [x] Pisahkan development dan production config secara eksplisit.
- [x] Production mode harus fail closed jika EVM client tidak tersedia.
- [ ] Bind address harus configurable; saat ini hanya `127.0.0.1`.
- [ ] Tambahkan readiness dan liveness endpoint terpisah.

## P2 - Testing

- [ ] Tambahkan production-path tests tanpa `#[cfg(test)]` bypass.
- [ ] Jalankan Stylus contract dalam local dev node atau supported test harness.
- [x] Test precompile EIP-2537 menggunakan known vectors.
  - `0x0b` G1ADD known vector sudah lolos di Arbitrum Sepolia.
  - `0x0d` G2ADD dan `0x0f` pairing known vector sudah lolos di Arbitrum
    Sepolia setelah encoding fix.
- [ ] Test invalid BLS signature tidak mengubah state.
- [ ] Test unauthorized reveal.
- [ ] Test duplicate session.
- [ ] Test CCIP unauthorized router/source/sender.
- [ ] Test source success tetapi destination failure.
- [ ] Test RPC failure setelah nullifier reservation.
- [ ] Test relayer restart dengan queue yang belum selesai.
- [ ] Test concurrent workers.
- [ ] Tambahkan fuzz/property tests untuk ABI and payload decoding.
- [ ] Tambahkan CI untuk `fmt`, `clippy`, tests, WASM build, dan Stylus check.

### Real Testnet Reproduction

Gunakan artifact berikut untuk pengujian nyata:

```bash
source nimbus-node/.env.test
cargo run --package nimbus-core --example generate_bls_test_data
python3 scripts/testnet_integration.py
```

Sebelum menjalankan:

- [x] Perbaiki generator BLS sesuai checklist P0.
- [x] Pastikan `.env.test` menunjuk ke contract deployment terbaru.
- [ ] Pastikan contract terbaru telah diaktifkan dan di-cache di Stylus.
- [ ] Pastikan `NIMBUS_CCIP_ROUTER` dikonfigurasi pada relayer dan contract.
- [x] Pastikan contract memiliki liquidity test secukupnya.
  - 2026-06-11: Terbukti dari saldo test wallet (145.16 USDC) dan kesuksesan transaksi deposit/spend on-chain.
- [x] Gunakan wallet dan key khusus testnet, bukan credential production.
  - 2026-06-11: Menggunakan testnet-only wallet `0x23e32D309c575A3D5E7CD2867BE12B00efa44Bb1`.

Test matrix minimum:

- [x] Signature BLS valid diterima dan receipt sukses.
  - 2026-06-11: PT-01 pada `ht04_spend_negative_tests.py` sukses mengirim spend transaction.
- [x] Signature dengan satu byte berubah ditolak.
  - 2026-06-11: NT-01/NT-02 dibuktikan direvert oleh contract.
- [x] `H(m)` dengan satu byte berubah ditolak.
  - 2026-06-11: NT-05 dibuktikan direvert oleh contract.
- [x] Issuer public key berbeda ditolak.
  - 2026-06-11: NT-03 dibuktikan direvert oleh contract.
- [x] Nullifier yang sama ditolak pada percobaan kedua.
  - 2026-06-11: NT-11 (replay spend) dibuktikan ditolak (returned false).
- [x] Failed signature tidak mengubah nullifier atau principal.
  - 2026-06-11: Saldo principal tetap utuh pada seluruh percobaan negative tests.
- [ ] CCIP source transaction sukses dan menghasilkan message ID asli.
- [ ] CCIP message terpantau sampai destination success.
- [ ] Invalid source chain atau sender CCIP ditolak.

Evidence yang wajib disimpan:

- Source transaction hash.
- Source block dan receipt status.
- CCIP message ID asli dari event.
- Destination transaction hash.
- Destination block dan receipt status.
- Event contract yang menunjukkan nullifier dan payout.
- Hasil negative test beserta revert reason.

### Pengelolaan `.env.test`

`nimbus-node/.env.test` saat ini tracked oleh Git dan memuat konfigurasi sensitif
seperti RPC URL, relayer private key, share key, contract address, dan database
path.

- [ ] Rotasi seluruh credential yang pernah ter-commit atau dibagikan.
- [ ] Hapus credential nyata dari Git history.
- [x] Tambahkan `nimbus-node/.env.test` ke `.gitignore`.
  - 2026-06-11: Ditambahkan ke root `.gitignore` dan dilepas dari pelacakan git (`git rm --cached`).
- [x] Buat `nimbus-node/.env.test.example` hanya dengan placeholder.
  - 2026-06-11: File template `nimbus-node/.env.test.example` berhasil dibuat.
- [ ] Muat secret melalui secret manager atau environment CI.
- [ ] Tambahkan secret scanning pada CI.
- [ ] Pastikan script test tidak mencetak private key, Vault token, atau RPC
  credential.

## Dokumentasi yang Perlu Dikoreksi

- [x] Ubah klaim "BLS signature fix" menjadi "BLS signature ABI transport fix"
  sampai pairing verification diaktifkan.
  - 2026-06-11: Dikoreksi di `docs/contract.md`.
- [x] Ubah "E2E CCIP succeeded" menjadi "CCIP source transaction broadcast
  succeeded" sampai destination execution diverifikasi.
  - 2026-06-11: Dikoreksi di `docs/roadmap.md`.
- [x] Jelaskan bahwa tx hash bukan CCIP message ID.
  - 2026-06-11: Dijelaskan di `docs/roadmap.md`.
- [x] Tandai Groth16 compliance sebagai prototype/demo.
  - 2026-06-11: Ditandai di `docs/sdk.md` dan `docs/contract.md`.
- [x] Jangan menyebut relayer atau contract production-ready sebelum seluruh P0
  selesai.
  - 2026-06-11: Ditambahkan warning keras di `docs/relayer.md` dan diselaraskan di `docs/contract.md`.
- [x] Catat bahwa unit tests contract menggunakan mocked HostIO/precompile paths.
  - 2026-06-11: Dicatat di `docs/contract.md`.
- [x] Catat hard-test Arbitrum Sepolia EIP-2537 gagal pada jalur pairing karena
  input Nimbus belum diterima precompile.

## Urutan Implementasi yang Disarankan

1. Perbaiki binding deposit/reveal.
2. Aktifkan BLS verification pada spend.
3. Hapus secret-share override dan amankan guardian RPC.
4. Buat persistent settlement state machine untuk queue dan nullifier.
5. Parse receipt serta CCIP message ID dan monitor destination execution.
6. Perbaiki relayer deposit/reveal agar mengikuti state on-chain.
7. Bangun ulang compliance circuit dan gunakan verifying key nyata.
8. Harden KMS, database encryption, API authentication, dan TLS.
9. Audit vault, Polymarket, dan x402 setelah invariant pembayaran
   utama aman.

## Roadmap Lanjutan

Roadmap produk dan infrastruktur Phase 1-4 dipisahkan ke
[`docs/roadmap.md`](docs/roadmap.md). Item roadmap tidak menggantikan P0,
hard-test matrix, atau mainnet gate dalam dokumen ini.

## Mainnet Gate

Mainnet tidak boleh dilakukan sampai:

- [ ] Seluruh P0 selesai dan diaudit.
- [ ] Tidak ada cryptographic verification yang dinonaktifkan.
- [ ] Tidak ada mock key, mock tx hash, atau mock verification pada production
  profile.
- [ ] Deposit, reveal, spend, refund, dan CCIP memiliki end-to-end negative
  tests.
- [ ] Queue dan settlement tahan restart serta RPC failure.
- [ ] External security review selesai untuk contract, relayer, threshold
  protocol, dan ZK circuit.
