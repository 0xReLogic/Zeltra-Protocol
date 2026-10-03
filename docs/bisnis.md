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

### 2.1 Netralitas Kepatuhan & Verifiable Provenance (Association Sets)

Nimbus dirancang dengan arsitektur **pembuktian selektif mandiri (self-provable innocence)**:
1. **Non-Custodial Absolut:** Protokol, smart contract, relayer, dan pengembang **tidak pernah memegang private spend key atau viewing key pengguna**. Tidak ada master key atau backdoor untuk membuka identitas secara sepihak.
2. **Beban Pembuktian di Tangan Pengguna:** Pengguna jujur memiliki kemampuan matematis untuk membuktikan bahwa deposit mereka berasal dari himpunan bersih (*Association Set*) yang diterbitkan oleh pihak ketiga independen (*Association Set Provider / ASP*), tanpa membocorkan riwayat dompet pribadi mereka ke publik.
3. **Penyaringan Otomatis di Pintu Masuk:** Aktor jahat atau dana hasil eksploitasi tidak akan dapat membuktikan keanggotaan dalam *clean association set*. Akibatnya, dana mereka akan otomatis ditolak oleh exchange atau merchant yang mewajibkan verifikasi kepatuhan.
4. **Netralitas Infrastruktur:** Nimbus adalah rel pembayaran publik yang netral. Kewajiban pemilihan kebijakan verifikasi dan penegakan hukum berada di pihak verifier (exchange/merchant), bukan di tangan protokol.

## 3. Pihak dalam Jaringan

| Pihak | Peran |
|---|---|
| Pengguna | Memiliki dan membelanjakan dana dari privacy pool |
| AI agent | Berbelanja memakai mandat pemilik melalui Nimbus SDK |
| Merchant | API provider, dApp, marketplace, game, atau penerima pembayaran |
| Relayer | Membayar gas native terlebih dahulu dan mengirim transaksi |
| Guardian | Menjalankan threshold signing dan menjaga availability |
| Association Set Provider (ASP) | Pihak independen yang menerbitkan daftar deposit bersih (Merkle root) |
| Protocol treasury | Membiayai pengembangan, RPC, domain, audit, monitoring, dan bug bounty |

Merchant bukan hanya toko retail. Dalam konteks Nimbus, merchant adalah pihak mana
pun yang menerima pembayaran melalui protokol.

## 4. Alur Pembayaran

### 4.1 Shield (Deposit ke Private Note)

Pengguna memasukkan stablecoin (USDC) ke Nimbus Privacy Pool. Setelah proses verifikasi
deposit dan issuance selesai, pengguna memperoleh **Initial Private Note commitment** yang
tercatat di dalam on-chain Merkle tree dan dapat dibelanjakan berkali-kali tanpa perlu
melakukan deposit ulang untuk setiap pembayaran.

Deposit/shield dikenai fee `0,20%`. Nilai bersih setelah fee menjadi private principal
pengguna yang dienkripsi dan diikat ke spending key privat milik pengguna atau agent.

### 4.2 Private Spend & Change Output (UTXO Model)

Pengguna atau AI agent membuat ZK payment payload melalui Nimbus SDK. Relayer memverifikasi
proof dan quote, membayar gas native di muka, dan mengeksekusi settlement on-chain.
Merchant menerima nominal invoice penuh tanpa potongan.

Pada transaksi partial spend (misal: saldo $100, bayar invoice $5):
1. **Input Note** lama dikonsumsi dan menghasilkan nullifier unik on-chain (mencegah double spend).
2. **Merchant Payout** dibayarkan penuh ke alamat recipient.
3. **Protocol & Execution Fee** dipotong sesuai quote terverifikasi.
4. **Change Note (Kembalian)** secara otomatis dibuat dan di-mint kembali ke Merkle tree
   secara privat untuk spending key pengirim via ZK-SNARK proof (Value Conservation).

```text
Input Note Value = Merchant Payout + Protocol Fee + Execution Fee + Output Change Value
```

Dengan model ini:
* Tidak ada sisa dana yang terkunci (*no dead/locked funds*).
* Pengguna tidak perlu menghitung gas manual sebelum deposit.
* Sisa kembalian langsung menjadi unspent note baru yang siap dibelanjakan lagi kapan saja.

### 4.3 Unshield / Withdrawal (Konsolidasi)

Pengguna dapat mengeluarkan saldo dari privacy pool menuju alamat publik kapan saja.
Untuk menjaga kesederhanaan arsitektur dan efisiensi WASM di Arbitrum Stylus, tidak ada fungsi
`withdraw()` terpisah. Penarikan saldo dilakukan dengan mengeksekusi **Private Spend**
ke alamat dompet publik milik pengguna sendiri sebagai recipient. Sisa saldo (jika ada) tetap
kembali sebagai change note privat.

### 4.4 Emergency Exit / Ragequit (Penarikan Darurat Non-Privat)

Jika pengguna mengalami kendala teknis pada cluster relayer, atau jika transaksi mereka ditolak oleh kebijakan ASP merchant tertentu, pengguna **tidak akan pernah kehilangan hak atas dananya**. 
Pengguna dapat memanfaatkan jalur `claim_refund()` setelah timelock 24 jam untuk menarik 100% principal deposit mereka kembali ke alamat dompet penyetor asli secara transparan on-chain. Ini memberikan jaminan bahwa dana pengguna tidak dapat tertahan selamanya di dalam protokol.

## 5. Model Biaya Target (Zero-Deposit & Fixed Mathematical Constants)

Untuk meminimalkan celah eksploitasi dan serangan governance DAO (seperti insiden flashloan/governance takeover The DAO 2016 atau Tornado Cash 2023), Nimbus mengadopsi **konstanta matematis protokol yang terjamin (*hardcoded immutable constants*)**:

| Aktivitas | Biaya target | Penerima | Keterangan |
|---|---:|---|---|
| **Deposit / Shield** | **0,00% (0 bps)** | N/A | **Zero-Friction Inflow**. 100% dana masuk ke saldo note, refund darurat kembali 100% utuh tanpa potongan. |
| **Private Spend (< 30 hari)** | **0,45% (45 bps)** | Protocol treasury | Mengkompensasi 0% fee deposit dengan biaya transaksi wajar saat belanja/transfer. |
| **Private Spend (≥ 30 hari)** | **0,40% (40 bps)** | Protocol treasury | **Diskon 5 bps** bagi penyimpan saldo jangka panjang (insentif likuiditas pool). |
| **Execution Fee** | Gas aktual + 15% markup | Relayer | Penggantian gas L2 Arbitrum (~$0,02) + markup margin operasional node relayer. |
| **Fast / Cross-Chain Settlement** | Quote dinamis | Relayer/LP, jaringan, treasury | Ditetapkan per rute CCIP. |
| **SDK & Integrasi** | Gratis | N/A | Open-source client libraries. |

### 5.1 Network Fee

Biaya pengguna terdiri dari protocol fee dan fixed execution quote:

```text
Network Fee =
    protocol fee 0,45% dari nominal transaksi (default) / 0,40% (hold >= 30 hari)
  + gas reimbursement
  + relayer markup
```

Contoh ilustratif (dengan default spend fee 0,45%):

```text
Harga merchant / payout           100,0000 USDC
Protocol fee 0,45%                  0,4500 USDC
Gas quote                           0,0200 USDC
Relayer markup 15% gas              0,0030 USDC
                                  -------------
Total debit pengguna              100,4730 USDC
Merchant menerima                 100,0000 USDC
```

Relayer membayar gas dalam ETH terlebih dahulu. Execution quote ditetapkan
sebelum user menandatangani dan terdiri dari estimasi gas dalam stablecoin plus
markup relayer (misal 15% dari gas quote). Jika beberapa transaksi berhasil
dibatch dengan biaya aktual lebih rendah, selisihnya menjadi margin efisiensi relayer.

### 5.2 Execution Quote dan Margin Batch

Execution quote bukan klaim penggantian gas aktual setelah transaksi. User
mengetahui total biaya sebelum tanda tangan. Relayer boleh menggabungkan 2 sampai
8 same-chain spend dan menyimpan selisih efisiensi setelah gas dibayar.

Karena itu:

- quote harus cukup menutup jalur transaksi single;
- batch menambah waktu tunggu normal maksimal sekitar 1 detik dan hard timeout
  sekitar 2 detik sebelum broadcast;
- cross-chain dan transaksi dengan deadline dekat tidak dipaksa menunggu batch;
- pendapatan protokol berasal dari transaksi spend `0,45%` (atau `0,40%` untuk hold ≥ 30 hari);
- pendapatan relayer berasal dari reimbursement gas, markup gas 15%, dan margin batch;
- biaya harus ditampilkan sebagai satu quote transparan sebelum pengguna menandatangani.

Payload memuat `max_fee` dan expiry agar relayer tidak dapat menaikkan biaya setelah persetujuan pengguna.

## 6. AI Agent Spending Wallet dan SDK

Nimbus tidak membuat atau mengoperasikan bot AI pengguna. Developer memasang
Nimbus SDK ke dalam autonomous agent mereka agar agent dapat melakukan pembayaran API
atau layanan secara privat dan self-custodial.

```text
Pemilik mendanai Initial Private Note untuk Agent
    |
    v
Nimbus SDK (Client-Side Note & Spending Key Manager)
    |
    v
AI agent menerima HTTP 402 atau payment request
    |
    | SDK membuat Groth16 ZK proof (Spend Note -> Pay Merchant + Change Note)
    v
Nimbus Relayer melakukan instant settlement on-chain
    |
    +---> API provider menerima USDC
    |
    +---> Change Note (kembalian) kembali ke SDK agent untuk request berikutnya
```

Agent Spending Wallet **bukan akun kustodian di server relayer** dan bukan saldo Layer 7 fiktif.
Ini adalah pengelolaan cryptographic private notes secara mandiri oleh Nimbus SDK:
1. Pemilik mendelegasikan spending key terisolasi atau mentransfer note dengan nominal tertentu ke agent.
2. Agent memegang spending key dan unspent notes secara lokal.
3. Karena sisa pembayaran selalu kembali sebagai change note ke Merkle tree, agent dapat melakukan pembayaran API berulang kali secara instan tanpa perlu bantuan funding wallet manusia setiap kali ada invoice.

Untuk mencegah pengeluaran berlebihan oleh AI yang error/halusinasi, SDK menyediakan policy guard:
- batas nominal maksimum per transaksi;
- batas pengeluaran harian/per jam;
- allowlist domain/merchant penerima;
- expiry dan kill-switch lokal;
- limit toleransi execution fee.

Nanopayment mikro dapat dikumpulkan oleh relayer dan diselesaikan secara batch (2-8 transaksi)
agar biaya gas on-chain tetap jauh lebih kecil daripada nilai pembayaran.

## 7. Liquid Reserve (Full Reserve) Model

Untuk meminimalkan risiko keamanan smart contract, menghemat batas ukuran byte WASM di Arbitrum Stylus, dan meniadakan risiko Bank Run (kebangkrutan kas akibat penarikan massal), Nimbus mengadopsi **100% Liquid Reserve (Full Reserve)**. 

Seluruh collateral stablecoin milik pengguna disimpan dalam kas likuid aktif langsung di dalam kontrak utama. Model dynamic vault dengan alokasi 30/50/20 via Aave V3 dan Ondo RWA telah didelegasikan ke fase pengembangan modular selanjutnya. Jika dibutuhkan yield native di masa depan, fitur tersebut akan dibangun secara modular sebagai wrapper terpisah (misalnya adaptor ERC-4626) daripada digabungkan langsung ke dalam core privacy pool.

## 8. Pendapatan dan Biaya Operasional

### Protocol treasury menerima

- deposit/shield fee `0,20%`;
- private spend/transaction fee `0,25%` (dapat didiskon ke `0,20%` jika hold >= 7 hari; atau `0,10%` jika hold >= 30 hari);
- bagian yang transparan dari fast/cross-chain settlement;
- **B2B Institutional PaidGate (opsional):** Biaya query verifikasi on-chain per panggilan atau langganan SLA bagi institusi (exchange, bank, atau fintech berlisensi) yang memerlukan on-chain audit log dan manajemen nonce otomatis;

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
Deposit volume     100.000.000 USDC x 0,20% = 200.000 USDC
Spend volume       100.000.000 USDC x 0,25% = 250.000 USDC
                                             ------------
Gross protocol revenue                       450.000 USDC
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

## 11. Realitas Status Implementasi Saat Ini

Dokumen ini menjelaskan **model bisnis target dan roadmap arsitektur**. Status teknis riil di codebase saat ini terbagi menjadi dua fase:

### Yang Sudah Selesai & Teruji di Arbitrum Sepolia (Phase 1 Baseline):
* **Smart Contract Stylus:** Verifikasi pairing BLS12-381 via EIP-2537 (`0x0f`), 13/13 testnet negative tests lolos.
* **Jaringan Threshold:** 1 Leader + 4 Guardian (3-of-5 threshold) dengan rilis atomik masking key `k` setelah deposit confirmed.
* **Settlement Engine:** SQLite/SQLCipher persistent queue dengan auto-retry, status machine durable, dan leasing worker.
* **Batch Spend:** Entrypoint Stylus `batch_spend()` (2-8 item) terdeploy dengan EIP-712 execution quote validation. Gas benchmark riil tersimpan di `docs/gas_latency_benchmark.md`.
* **Fee Structure Awal:** Helper fee terpusat di `nimbus-core` dan SDK untuk mencegah invoice short-pay.

### Yang Sedang Berjalan (Phase 2 - Jalur B: ZK-UTXO Note Balance):
* **Gate C0 Security Repair (`nimbus-core`):** Pengerjaan sirkuit Groth16 Arkworks untuk private note dengan change output:
  - Enforce pembuktian Merkle path direction bits terikat ke `input_leaf_index` di sirkuit (mencegah double-spend).
  - Enforce range constraint 64-bit untuk seluruh nilai nominal (mencegah modular wrap-around field scalar).
  - Enforce boolean constraint pada `has_change` (mencegah pemalsuan output commitment).
* **Gate D (Stylus Note Ledger):** Integrasi on-chain Merkle tree append-only dan multi-liability accounting (`user_note_liability`, `refundable_deposit_liability`, `accrued_execution_fee_liability`) di smart contract Stylus setelah Gate C0 lulus.
* **Tokenomics:** Seluruh desain token $NIMB dibekukan. Protokol beroperasi 100% menggunakan collateral USDC murni (Full Reserve).
