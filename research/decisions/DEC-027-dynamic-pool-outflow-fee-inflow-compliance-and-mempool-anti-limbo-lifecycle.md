# DEC-027: Predictable Outflow Economic Model, Inflow Compliance Gate, and Relayer Anti-Limbo Mempool Lifecycle

- **Status:** Proposed & Accepted (Architecture Core Decision)
- **Author:** Zeltra Protocol Architecture Team
- **Date:** 2026-10-05 (Updated 2026-10-07)
- **Impact Areas:** `nimbus-core` (fee policy & solvency invariants), `nimbus-node` (deposit ingress screening, dynamic mempool lease watchdog), `nimbus-contracts` (deposit refund invariant guarantees), `research/economics`
- **Architectural Paradigm:** *"Solvency by Mathematics, Predictability for Users, Security at Ingress, Self-Custody at Mempool"*

---

## 1. Konteks & Evaluasi Kritis (Audit Panel Evaluation)

Evaluasi independen terhadap Zeltra Protocol menyoroti tiga tantangan struktural yang berpotensi mengancam keberlanjutan ekonomi, kepatuhan hukum, dan pengalaman pengguna:

1. **Free-Parking Risk vs. TVL Anonymity Shield:**
   Protokol mematok biaya deposit permanen 0 bps dan hanya menarik biaya saat pengeluaran (outflow 40–45 bps). Muncul kekhawatiran bahwa jika paus kripto atau agen AI menyimpan jutaan USDC secara pasif (brankas dingin on-chain), relayer tetap menanggung biaya operasional (indexing, Merkle tree state, RPC node) tanpa pemasukan. Usulan reviewer luar untuk memarkir dana di protokol pinjaman pihak ketiga (misal: Aave/Compound) **ditolak mentah-mentah** karena memperkenalkan *composability risk* dan ancaman insolvensi eksternal yang melanggar invariant `Assets >= Liabilities`.
2. **Kelemahan Penyaringan Penerima (Recipient-Only Filter Flaw):**
   Menyaring alamat sanksi OFAC murni di sisi penerima (*spend recipient*) adalah celah fatal: peretas tidak pernah mengirim dana curian ke alamat yang sudah masuk daftar hitam (seperti Tornado Router atau alamat Lazarus); mereka selalu menarik dana ke alamat baru (*fresh wallet*). Jika penyaringan hanya ada di pintu keluar, dana hasil eksploitasi dapat masuk secara bebas mencemari *shielded pool*.
3. **Mempool Stalling & Hilangnya Kedaulatan Pengguna (Mempool Limbo):**
   Jika lonjakan gas jaringan menyebabkan transaksi spend macet di mempool melebihi batas `max_gas_bumps` (3x), relayer menghentikan upaya (*sleep*). Karena nullifier dikunci di database relayer lokal untuk mencegah *double-spending*, dana pengguna terjebak dalam status `failed settlement` tanpa kepastian, melanggar prinsip *strict self-custody*.

Keputusan arsitektur ini merumuskan mitigasi terintegrasi untuk persoalan di atas dengan mengutamakan kenyamanan pengguna, prediktabilitas agen AI, dan ketahanan kriptografi tingkat institusi.

---

## 2. Predictable Flat Fee Rails & Penolakan Model Dynamic Fee (Economic Predictability over Surge Volatility)

### A. Mengapa Model Dynamic Fee / Kinked Curve Ditolak ("Haram" bagi Pengguna Kripto & AI Agent)
Meskipun model biaya dinamis berbasis utilisasi (*kinked utilization curve* ala Aave/Uniswap) tampak menarik secara teoretis, analisis mendalam terhadap perilaku pasar kripto (*crypto-native psychology*) dan otomatisasi agen AI membuktikan bahwa kurva dinamis pada rel privasi menghasilkan efek destruktif:

1. **Kehancuran Mental Accounting UX ($10 - $3 = $7):**
   Keunggulan produk utama Zeltra adalah *"Privacy 9.5/10, Product 10/10"* dengan kecepatan Apple Pay / QRIS dan akuntansi mental uang kas nyata. Jika pengguna membayar $3 dari note $10, mereka mengharapkan kembalian uang pas $7 (dikurangi fee tetap yang sudah diketahui di awal). Jika biaya penarikan berfluktuasi antara 35 bps hingga 70 bps tergantung volume penarikan orang lain di blok tersebut, pengguna merasa "dijebak" atau "dipalak" saat jaringan sibuk.
2. **Kegagalan Deterministik pada Agen AI (Budget Allowance Breaking):**
   AI Agent beroperasi otonom menggunakan kuota pengeluaran terikat (*task-scoped allowance* via EIP-7702 / x402). Fluktuasi tarif keluar dinamis memicu kesalahan *underfunded execution* dan kegagalan transaksi sistemik saat lonjakan pasar terjadi.
3. **Eliminasi Vektor Eksploitasi Flash-Loan & Manipulasi Oracle:**
   Sebagaimana terbukti pada post-mortem Euler Finance (2023) dan zkLend (2024), kurva dinamis yang bergantung pada rasio cadangan membuka celah serangan pinjaman kilat (*flash loan fee manipulation*). Dengan menetapkan tarif flat transparan, attack surface manipulasi fee intra-blok terhapus 100%.

### B. Formulasi Tarif Terprediksi (Predictable Transparent Fee Rails)
Zeltra Protocol menetapkan struktur biaya tetap yang transparan dan deterministik:

* **Deposit Inflow:** **0 bps (0.00% Permanen)** — Bebas gesekan, mendorong adopsi modal tanpa hambatan.
* **Spend Outflow Standard (< 30 hari):** **45 bps (0.45% Flat)** — Biaya protokol terprediksi untuk transaksi harian.
* **Spend Outflow Long-Term Hold (≥ 30 hari):** **40 bps (0.40% Flat — Diskon Loyalitas 5 bps)** — Dihitung secara objektif dari timestamp registrasi Merkle root (`clean_association_roots`) di smart contract.
* **Eksekusi Relayer:** **Gas Reimbursement Aktual + 15% Markup Transparan** — Terikat secara kriptografis pada kuotasi EIP-712 dengan batas kedaluwarsa 5–15 menit.

### C. Filosofi Keberlanjutan: Volume over TVL Velocity
Keberlanjutan finansial relayer dan ekosistem dicapai melalui **kecepatan perputaran modal (velocity)** dari pembayaran agen AI dan retail mikro, bukan dari memeras penarikan pengguna dengan *surge penalty* atau memungut biaya diam (*idle fees*). TVL yang tebal berfungsi sebagai *anonymity shield* yang memperluas *privacy set* bagi seluruh peserta ekosistem.

---

## 3. Inflow Compliance Gate: Penyaringan 2-Layer di Gerbang Deposit

### A. Mengapa Deposit Gate Wajib Dilindungi
Peretas yang mengeksploitasi protokol DeFi memindahkan dana dari alamat korban/kontrak yang dieksploitasi dalam hitungan blok. Jika penyaringan kepatuhan hanya diletakkan di pintu penarikan (*spend*), dana kotor sudah telanjur bercampur ke dalam *shielded pool* Zeltra.

### B. Arsitektur Dua Lapis di Inflow (Deposit Ingress)
1. **Lapis 1 — Fast In-Memory Screening (<1 ms, 0 Gas, 0 Biaya):**
   * Sebelum sesi blind signature diproses di relayer/leader, alamat pengirim deposit (`depositor`) diperiksa terhadap set hash in-memory `SANCTIONED_EVM_ADDRESSES` (diperbarui berkala setiap jam 07:00 UTC dari `vile/ofac-sdn-list`).
   * Jika alamat depositor ada dalam daftar SDN, relayer secara *fail-closed* menolak memproses blind signature dan tidak merilis masking key $k$.
2. **Lapis 2 — On-Chain Oracle Fallback (Chainalysis Sanctions Oracle `0x40C5...`):**
   * Pemeriksaan dinamis via *free view call* (`eth_call`) terhadap smart contract Oracle Sanctions resmi di Arbitrum.
   * Melindungi relayer terhadap alamat yang baru saja dilaporkan dalam beberapa menit terakhir dan belum tersinkronisasi ke berkas JSON lokal.

### C. Ketiadaan Kebutuhan Sirkuit ZK-Compliance Tambahan pada Inflow
Reviewer mengusulkan penambahan sirkuit ZK-Compliance $N$-derajat di sisi klien. Pendekatan ini ditolak untuk transaksi sehari-hari karena:
* Menghancurkan performa UX (waktu pembuktian di perangkat seluler membengkak dari 1.2 detik menjadi >6 detik).
* Pengguna Zeltra sudah memiliki komitmen kriptografis ZK-UTXO (Groth16). Untuk kebutuhan penarikan ke CEX atau audit pajak, SDK menyediakan fitur *Selective Disclosure on-demand* (menghasilkan bukti kepemilikan dan timestamp deposit) tanpa membebani transaksi privat reguler.

---

## 4. Siklus Hidup Mempool Anti-Limbo (Relayer Dynamic Lease & Smart Contract Invariants)

### A. Klarifikasi Timelock Kontrak: 24 Jam sebagai Worst-Case Disaster Recovery
Timelock 24 jam pada smart contract `deposit.rs` (`NIMBUS_REFUND_DELAY = 86400`) **BUKAN waktu tunggu operasional normal**.
* **Kondisi Normal (Leader + Guardians Aktif):** Proses deposit, quorum blind signature, rilis kunci $k$, dan penerbitan Private Note selesai dalam **1–3 detik** (secepat finalitas blok Arbitrum).
* **Kondisi Gangguan Sementara (Server Mati 15 Menit):** Begitu relayer menyala kembali, `deposit_indexer` mendeteksi sesi tertunda, menyelesaikan threshold signing, dan merilis kunci $k$. Pengguna menerima note dalam hitungan detik setelah node pulih.
* **Fungsi 24 Jam:** Merupakan benteng perlindungan absolut jika kluster guardian kolaps permanen / ditinggalkan dev. Smart contract menjamin pengguna dapat menarik 100% dananya secara sepihak tanpa izin relayer.

### B. Relayer Dynamic Lease Auto-Release (Mempool Watchdog)
Untuk mencegah dana pengguna terkunci dalam status limbo di database relayer akibat mempool macet:

```text
┌─────────────────┐       Broadcast       ┌────────────────────────┐
│  Spend Request  │ ────────────────────► │ Status: BROADCASTING   │
└─────────────────┘                       │ Lease: now + 15 mins   │
                                          └───────────┬────────────┘
                                                      │
                                   ┌──────────────────┴──────────────────┐
                                   │                                     │
                             Mined On-Chain                        Mempool Drop /
                                   │                             Max Bumps Exceeded
                                   ▼                                     │
                        ┌─────────────────────┐                          ▼
                        │  Status: CONFIRMED  │               ┌───────────────────────┐
                        │  Nullifier: SPENT   │               │ Watchdog: is_unspent? │
                        └─────────────────────┘               └──────────┬────────────┘
                                                                         │
                                                              Confirmed Unspent
                                                                         │
                                                                         ▼
                                                              ┌───────────────────────┐
                                                              │ Status: EXPIRED       │
                                                              │ Lease: RELEASED       │
                                                              │ (User Note Re-enabled)│
                                                              └───────────────────────┘
```

1. **Lease Duration Dinamis:**
   Setiap transaksi spend yang diambil oleh worker diberi batas waktu sewa (`lease_until = now + 15–30 menit`), adaptif terhadap volatilitas gas dasar Arbitrum.
2. **Background Watchdog Task:**
   Worker relayer memeriksa transaksi yang melewati masa sewa (`now > lease_until`) dan belum berstatus `CONFIRMED`:
   - Melakukan query on-chain: `evm.is_nullifier_spent(nullifier)`.
   - Jika terbukti **belum terbelanjakan on-chain**, relayer mengeksekusi `reset_unspent_lease(id)` di SQLite.
   - Status diubah menjadi `FAILED_EXPIRED`.
3. **Restorasi Kedaulatan Pengguna (Self-Custody Restored):**
   Dompet SDK pengguna mendeteksi status kegagalan mempool, menandai note lokal sebagai `UNSPENT`, dan memungkinkan pengguna mencoba kembali (*retry*) dengan estimasi gas baru atau merutekan transaksi melalui relayer lain. Tidak ada intervensi manual; tidak ada dana yang tersandera.

---

## 5. Red-Team Threat Modeling & Analisis Kegagalan (Post-Mortem Hardening)

Berdasarkan studi empiris atas insiden keamanan DeFi dan audit Account Abstraction (2023–2026), berikut adalah pemetaan kegagalan nyata (*exploit post-mortems*) dan mitigasi arsitektur DEC-027:

### A. 🔴 Vektor 1: Risiko Volatilitas Fee & Manipulasi Kurva (Alasan Penolakan Model Dinamik)
* **Preseden Nyata (Post-Mortem):**
  Insiden peretasan likuiditas (Euler Finance 2023, zkLend 2024) membuktikan bahwa kurva utilisasi instan yang bergantung pada rasio cadangan spot (*spot balance*) rentan dimanipulasi dalam satu blok (*single-block flash loan attack*).
* **Mitigasi Zeltra:**
  1. Protokol menolak formula dinamis yang mudah dimanipulasi dan menerapkan **Flat Predictable Fee Rails** (45 bps standard / 40 bps diskon 30 hari).
  2. Biaya eksekusi relayer dikunci secara kriptografis melalui skema EIP-712 Quote Binding (DEC-020).
  3. **Invariant I1:** *Outflow fee rate is predictable, deterministic, and impervious to intra-block liquidity manipulation.*

### B. 🔴 Vektor 2: Desinkronisasi Mempool & False-Expiry Double-Spend Race (Studi Kasus: ERC-4337 & ERC-7562 Mempool Griefing Audits)
* **Preseden Nyata (Post-Mortem):**
  Audit keamanan *Account Abstraction Alt-Mempool* (OpenZeppelin, Alchemy, TrustSec 2024–2025; standar ERC-7562) mengungkapkan celah kritis pada relayer/bundler: jika status transaksi off-chain (lease lock) kedaluwarsa dan dilepaskan sementara transaksi yang macet di mempool akhirnya diikutsertakan (*late inclusion*), terjadi desinkronisasi fatal antara basis data off-chain dan kontrak on-chain, memicu *race condition* dan pengeluaran ganda (*double-spend race*).
* **Mitigasi Zeltra:**
  1. **Pre-Reset On-Chain State Verification (Strict Simulation Rule):** Mengikuti doktrin ERC-7562, watchdog relayer **WAJIB** mengeksekusi query view `is_nullifier_spent(nullifier)` ke smart contract Stylus via `eth_call` SEBELUM memanggil `reset_unspent_lease()`.
  2. **Atomic State Transition:** Jika ternyata di on-chain nullifier telah berstatus `SPENT` (transaksi mempool ternyata tembus di detik-detik akhir), watchdog langsung memperbarui database lokal menjadi `CONFIRMED` dan **DILARANG KERAS** melepaskan lease.
  3. **On-Chain Terminal Defense:** Smart contract Stylus memegang kedaulatan mutlak anti-double-spend: `if note_nullifiers[input_nullifier] { revert NullifierAlreadySpent(); }`. Jika ada relayer kedua yang mencoba mem-broadcast spend kedua, smart contract akan me-revert transaksi secara atomik.
  4. **Invariant I2:** *A lease may NEVER be reset if the nullifier is confirmed on-chain. Storage on-chain is the single source of truth (`SPENT XOR EXPIRED`).*

### C. 🔴 Vektor 3: Penetrasi Deposit via Kontrak Proksi & DoS Oracle RPC (Studi Kasus: Tornado Cash Router Bypass & Frontend Injection)
* **Preseden Nyata (Post-Mortem):**
  Kasus pengabaian sanksi Tornado Cash (2022–2024) menunjukkan bahwa penyaringan sanksi yang hanya diletakkan di sisi frontend atau di gerbang penarikan sangat mudah diakali: peretas langsung berinteraksi dengan kontrak perantara (*unverified proxy/wrapper*) atau menargetkan dompet baru. Selain itu, serangan banjir deposit mikro dapat melumpuhkan kuota API compliance oracle (*rate-limit exhaustion DoS*).
* **Mitigasi Zeltra:**
  1. **Fail-Closed Compliance Policy:** Jika Oracle RPC mengalami kegagalan, timeout, atau kehabisan kuota, relayer menahan rilis kunci unmasking $k$ (status `PENDING_COMPLIANCE`). Sistem **TIDAK PERNAH** melepaskan kunci dalam kondisi *fail-open*.
  2. **Dual-Address Checking:** Pengecekan sanksi memeriksa alamat `msg.sender` sekaligus parameter `depositor` asal yang terikat pada event on-chain `DepositFee`.
  3. **Anti-Spam Economic Bound:** Smart contract Stylus menegakkan deposit minimum sebesar 5 USDC (`MIN_SPEND_AMOUNT_USDC`), menggugurkan serangan banjir transaksi mikro berbiaya nol.
  4. **Invariant I3:** *No deposit may be resolved or unmasked without passing fail-closed compliance verification.*

---

## 6. Matriks Dampak & Rencana Implementasi

| Komponen | Perubahan Arsitektur | File Terkait |
| :--- | :--- | :--- |
| **`nimbus-core`** | Pemeliharaan invariant formula flat predictable fee (45 bps standard / 40 bps diskon 30 hari) | `fees.rs` |
| **`nimbus-node`** | Penambahan 2-layer sanctions screening pada ingress `handle_deposit` / reveal | `handlers/deposit.rs` |
| **`nimbus-node`** | Implementasi berkala `mempool_lease_watchdog` dengan verifikasi `is_nullifier_spent` sebelum unspent lease | `database.rs`, `main.rs` |
| **`nimbus-contracts`**| Mempertahankan `NIMBUS_REFUND_DELAY = 86400` sebagai *fail-closed invariant* tanpa penambahan kode berisiko | `deposit.rs` |
| **`nimbus-sdk`** | Sinkronisasi status mempool `UNSPENT` pada dompet pengguna saat lease kedaluwarsa | `note_wallet.rs` |

---

## 7. Referensi Bibliografi & Studi Kasus Terkait

1. **Euler Finance & zkLend Flash-Loan Utilization Curve Exploits: Incident Analysis & Post-Mortem Reports** — BlockSec, Cyfrin, Sherlock (2023–2024). [Dasar penolakan dynamic pool surge curves pada privacy rails].
2. **ERC-4337 & ERC-7562: Account Abstraction Alt-Mempool Validation and Griefing Defense Audits** — OpenZeppelin, Alchemy, TrustSec (2023–2025). [Dasar desain leasing 15 menit dan unspent watchdog].
3. **Tornado Cash Frontend Compromise & Router Bypass Post-Mortem Analysis** — OpenZeppelin, CertiK, SlowMist (2022–2024). [Dasar fail-closed 2-layer sanctions screening].
4. **Blockchain Privacy and Regulatory Compliance: Towards a Practical Equilibrium** — Vitalik Buterin, Jacob Illum, Matthias Nadler, Fabian Schär, Ameen Soleimani. *Blockchain: Research and Applications*, Vol. 5, Issue 2 (2024).
5. **DEC-026: Receiver-Enforced Modular Compliance and Relayer Exposure Mitigation** — Zeltra Protocol Architecture Team (Oktober 2026).


