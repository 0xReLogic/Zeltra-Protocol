# Model Bisnis Nimbus Protocol

## 1. Ringkasan

Nimbus adalah **shielded payment pool untuk Web3**. Pengguna memasukkan stablecoin
ke pool Nimbus, lalu manusia, aplikasi, atau AI agent dapat melakukan pembayaran
tanpa membuka hubungan langsung antara alamat sumber dana dan pembayaran tersebut.

Nimbus tidak menjual AI dan bukan bisnis SaaS. Nimbus menyediakan smart contract,
SDK, relayer, dan jaringan guardian sebagai infrastruktur pembayaran privat.

```text
Pemilik dana
    |
    | shield stablecoin
    v
Nimbus Privacy Pool
    |
    | private payment melalui SDK
    v
Merchant / API / dApp
```

SDK diberikan gratis agar wallet, dApp, API provider, dan developer AI dapat
mengintegrasikan Nimbus. Protokol memperoleh pendapatan ketika jaringan benar-benar
digunakan.

## 2. Nilai Utama

Nimbus memberikan:

- pemutusan hubungan publik antara funding wallet dan pembayaran;
- ephemeral identifier untuk mengurangi address clustering;
- private payment bagi manusia dan autonomous agent;
- relayer agar pengguna tidak perlu memegang native gas token;
- threshold guardian untuk signing dan accountable privacy;
- nullifier untuk mencegah double-spending;
- selective disclosure untuk audit atau proses compliance yang sah.

Nimbus tidak menjanjikan anonimitas mutlak. Nominal, waktu, recipient, funding
source, gas payer, dan pola withdrawal masih dapat menimbulkan korelasi. Produk
harus diposisikan sebagai **confidential settlement**, bukan alat untuk menghindari
hukum.

## 3. Pihak dalam Jaringan

| Pihak | Peran |
|---|---|
| Pengguna | Memiliki dan membelanjakan dana dari privacy pool |
| AI agent | Berbelanja memakai mandat pemilik melalui Nimbus SDK |
| Merchant | API provider, dApp, marketplace, game, atau penerima pembayaran |
| Relayer | Membayar gas native terlebih dahulu dan mengirim transaksi |
| Guardian | Menjalankan threshold signing dan menjaga availability |
| Protocol treasury | Membiayai pengembangan, RPC, domain, audit, monitoring, dan bug bounty |

Merchant bukan hanya toko retail. Dalam konteks Nimbus, merchant adalah pihak mana
pun yang menerima pembayaran melalui protokol.

## 4. Alur Pembayaran

### 4.1 Shield

Pengguna memasukkan stablecoin ke Nimbus Privacy Pool. Setelah proses issuance
selesai, pengguna memperoleh kemampuan melakukan private spend tanpa memakai
funding wallet untuk setiap pembelian.

Deposit/shield dikenai fee `0,10%`. Saldo bersih setelah fee menjadi liability
pool dan dapat dipakai sebagai pre-staged balance untuk manusia atau agent.

### 4.2 Private Spend

Pengguna atau agent membuat payment payload melalui SDK. Relayer memverifikasi
payload, membayar gas native, dan mengirim settlement. Merchant menerima nominal
harga yang diminta.

```text
Harga merchant + Network Fee = total yang dibayar pengguna
```

Merchant tidak dipaksa menanggung biaya privasi milik pengirim.

Kontrak saat ini memotong protocol fee dari `amount`. Karena itu, SDK harus
melakukan gross-up saat merchant meminta nominal pasti:

```text
contract amount = harga merchant / (1 - 0,0015)
```

Dengan cara ini, fee tetap berasal dari pengirim dan merchant menerima harga yang
diminta. Pembulatan final harus mengikuti integer stablecoin dan diverifikasi
terhadap quote kontrak.

### 4.3 Unshield

Pengguna dapat mengeluarkan saldo dari privacy pool menuju alamat publik. Proses
ini dikenai fee `0,10%`, simetris dengan deposit. Alasannya: unshield adalah
perpindahan saldo keluar dari pool, bukan private payment ke merchant. Private
spend tetap memakai fee transaksi yang lebih tinggi karena menggunakan jalur
settlement, relayer, quote, dan merchant exact-payout.

## 5. Model Biaya Target

| Aktivitas | Biaya target | Penerima |
|---|---:|---|
| Deposit/shield | 0,10% dari nominal | Protocol treasury |
| Private spend/transaction | 0,15% dari nominal | Protocol treasury |
| Execution fee | Gas quote + markup relayer | Relayer |
| Unshield tanpa pembayaran | 0,10% dari nominal | Protocol treasury |
| Fast/cross-chain settlement | Quote dinamis | Relayer/LP, jaringan, treasury |
| SDK dan integrasi | Gratis | N/A |

Persentase tersebut adalah hipotesis awal yang harus diuji di testnet dan kepada
calon integrator. Governance tidak boleh dapat menaikkannya tanpa batas.

### 5.1 Network Fee

Biaya pengguna terdiri dari protocol fee dan fixed execution quote:

```text
Network Fee =
    protocol fee 0,15% dari nominal transaksi
  + gas reimbursement
  + relayer markup
```

Contoh ilustratif, bukan harga tetap:

```text
Harga merchant                  100,0000 USDC
Gross-up protocol fee            ~0,1502 USDC
Gas quote                         0,0200 USDC
Relayer markup 15% gas            0,0030 USDC
                                -------------
Total pengguna                 ~100,1732 USDC
Merchant menerima              100,0000 USDC
```

Relayer membayar gas dalam ETH terlebih dahulu. Execution quote ditetapkan
sebelum user menandatangani dan terdiri dari estimasi gas dalam stablecoin plus
markup relayer, misalnya 15% dari gas quote. Jika beberapa transaksi berhasil
dibatch dengan biaya aktual lebih rendah, selisihnya menjadi margin efisiensi
relayer/protokol.

Protocol fee `0,15%` masuk ke treasury. Treasury kemudian membiayai guardian,
domain, RPC, audit, pengembangan, monitoring, dan cadangan keamanan. User tidak
ditagih guardian fee atau hosting fee sebagai komponen terpisah.

### 5.2 Execution Quote dan Margin Batch

Execution quote bukan klaim penggantian gas aktual setelah transaksi. User
mengetahui total biaya sebelum tanda tangan. Relayer boleh menggabungkan 2 sampai
8 same-chain spend dan menyimpan selisih efisiensi setelah gas dibayar.

Karena itu:

- quote harus cukup menutup jalur transaksi single;
- batch menambah waktu tunggu normal maksimal sekitar 1 detik dan hard timeout
  sekitar 2 detik sebelum broadcast;
- cross-chain dan transaksi dengan deadline dekat tidak dipaksa menunggu batch;
- pendapatan protokol berasal dari fee deposit `0,10%`, withdraw `0,10%`, dan
  transaksi `0,15%`;
- pendapatan relayer berasal dari reimbursement gas, markup gas, dan margin batch;
- biaya harus ditampilkan sebagai satu quote sebelum pengguna menandatangani.

Payload harus memuat `max_fee` dan expiry agar relayer atau governance tidak dapat
menaikkan biaya setelah persetujuan pengguna.

## 6. AI Agent Spending Wallet dan SDK

Nimbus tidak membuat atau mengoperasikan AI milik pengguna. Developer memasang
Nimbus SDK ke agent mereka agar agent dapat membayar merchant atau API secara
privat.

```text
Pemilik deposit ke Nimbus Privacy Pool
    |
    | menetapkan Agent Spending Wallet di Layer 7
    v
Pre-staged state: balance, allowance, policy, nonce
    |
    v
AI agent menerima HTTP 402 atau payment request
    |
    | SDK membuat ephemeral payment payload
    v
Nimbus melakukan instant private settlement
    |
    v
API provider menerima stablecoin
```

Agent Spending Wallet bukan wallet kustodian baru dan bukan AI milik Nimbus.
Ini adalah state otorisasi Layer 7 di atas liability pool: owner sudah menaruh
saldo, lalu agent diberi policy terbatas untuk membelanjakan saldo tersebut.
Karena saldo sudah pre-staged, agent tidak perlu melakukan deposit baru untuk
setiap API call. Settlement bisa langsung dibuat, diquote, ditandatangani, dan
masuk queue relayer.

Mandat atau pre-staged state agent minimal harus mencakup:

- batas saldo;
- maksimum per transaksi;
- batas pengeluaran harian;
- merchant atau kategori yang diizinkan;
- expiry;
- nonce dan nullifier;
- maksimum Network Fee.
- status pause/revoke dari owner.

Model ini menjadi alasan ekonomi agar user menyimpan working balance di pool
tanpa dipaksa lock. User tetap dapat withdraw saldo yang tidak terpakai dengan
fee unshield `0,10%`, tetapi agent hanya bisa spend sesuai policy yang sudah
ditandatangani owner.

Nanopayment tidak boleh mengirim satu transaksi on-chain untuk setiap API call.
Saldo atau authorization kecil dikumpulkan dan diselesaikan secara batch agar
biaya jaringan tidak lebih besar daripada nilai pembelian.

## 7. Dynamic Vault: 30/50/20

Rasio `30/50/20` mengatur collateral pengguna, bukan pembagian Network Fee:

| Alokasi normal | Porsi | Fungsi |
|---|---:|---|
| Kas likuid | 30% | Memenuhi spend dan withdrawal |
| Aave | 50% | Menghasilkan yield likuid |
| RWA | 20% | Diversifikasi yield dan reserve |

Model aktual bersifat dinamis. Target kas dapat bergerak antara 15% dan 45%
berdasarkan kebutuhan likuiditas. Sisa dana non-kas dibagi dengan rasio 5:2 antara
DeFi dan RWA, sehingga kondisi normal menghasilkan 30/50/20.

```text
Principal pengguna  -> Dynamic Vault
Network Fee         -> operator dan treasury
```

Principal, yield, dan pendapatan operator harus dicatat terpisah. Yield hanya
pendapatan tambahan dan tidak boleh digunakan untuk menutupi unit economics
relayer yang negatif. Sebelum mainnet, penggunaan collateral di DeFi/RWA harus
melewati audit solvency, liquidity stress test, dan kajian hukum.

## 8. Pendapatan dan Biaya Operasional

### Protocol treasury menerima

- deposit/shield fee `0,10%`;
- private spend fee `0,15%`;
- withdraw/unshield fee `0,10%`;
- bagian yang transparan dari fast/cross-chain settlement;
- yield vault jika model tersebut telah diaudit dan diizinkan.

### Protocol treasury membayar

- pengembangan kontrak, node, dan SDK;
- RPC dan data availability;
- domain, hosting, database, dan monitoring;
- audit keamanan dan bug bounty;
- incident reserve;
- subsidi testnet atau akuisisi integrator yang memiliki batas anggaran.

### Relayer menerima

- reimbursement gas yang sudah diquote;
- markup relayer atas gas, misalnya 15% dari gas quote;
- margin efisiensi batch setelah biaya gas aktual dibayar;
- premium modal dan risiko untuk fast/cross-chain settlement.

### Guardian menerima

- reward dari anggaran protocol treasury berdasarkan pekerjaan signing yang valid;
- reward hanya setelah memenuhi aturan availability dan tidak melakukan
  double-signing.

Pada testnet, seluruh operator dapat dimiliki Nimbus. Pembukuan tetap harus
memisahkan pendapatan relayer, guardian, dan treasury agar biaya jaringan nyata
dapat diukur sebelum operator eksternal dibuka.

## 9. Unit Economics

Contoh tahunan:

```text
Deposit volume     100.000.000 USDC x 0,10% = 100.000 USDC
Spend volume        80.000.000 USDC x 0,15% = 120.000 USDC
Withdraw volume     20.000.000 USDC x 0,10% =  20.000 USDC
                                             ------------
Gross protocol revenue                       240.000 USDC
```

Bagian execution quote yang mengganti gas bukan revenue bersih. Markup gas dan
selisih setelah gas aktual dibayar adalah margin relayer/protokol. Reward
guardian dibayar dari anggaran treasury sampai tersedia mekanisme operator
network yang teruji.

Metrik utama:

- shielded volume;
- private settlement volume;
- active wallets dan active agents;
- jumlah integrasi SDK;
- biaya gas aktual per settlement;
- pendapatan bersih per transaksi;
- uptime relayer dan guardian;
- collateral utilization dan kemampuan memenuhi withdrawal.

## 10. Strategi Pertumbuhan

1. Buktikan private settlement end-to-end di testnet tanpa mock.
2. Integrasikan SDK ke wallet, dApp, dan API provider sebagai distribution channel.
3. Gunakan Agent Spending Wallet sebagai alasan user menyimpan working balance
   untuk instant private settlement.
4. Buka relayer dan guardian eksternal setelah reward serta slashing teruji.
5. Aktifkan fast path dan cross-chain hanya setelah quote, reimbursement, dan
   accounting aman.
6. Tunda token, buyback, dan distribusi staking sampai protokol memiliki penggunaan
   serta pendapatan nyata.

## 11. Batas Implementasi Saat Ini

Dokumen ini menjelaskan **model bisnis target**, bukan seluruh perilaku kontrak
yang sudah aktif.

Implementasi saat ini sudah:

- memotong `0,1%` saat deposit;
- memotong `0,15%` saat spend/redemption.
- memiliki helper fee terpusat di `nimbus-core` dan SDK untuk menghitung
  gross-up private spend agar merchant menerima nominal exact;
- memiliki helper quote execution fee untuk memisahkan gas reimbursement dan
  markup relayer.

Implementasi saat ini masih:

- belum memisahkan private spend fee `0,15%` dan dedicated withdraw/unshield fee
  `0,10%` pada contract, SDK, dan quote API;
- sudah memiliki entrypoint dan broadcaster batch untuk 2 sampai 8 same-chain
  spend, tetapi penghematan gas belum dibenchmark di testnet;
- belum menagih fixed execution quote kepada pengguna;
- belum memiliki signed `max_execution_fee` dan quote expiry;
- belum membuktikan settlement batch nanopayment dengan beban produksi.

Sebelum mainnet, implementasi fee harus direfaktor menjadi policy terpusat,
memiliki batas maksimum permanen, timelock, event perubahan, signed quote, dan
accounting terpisah untuk principal, operator, serta treasury.
