# DEC-036B: Client SDK Multi-UTXO Knapsack Coin Selection, In-Pool Progressive Consolidation, and Anti-Snooping MMR Tree Sync

- **Status:** APPROVED AS ARCHITECTURAL SUB-SPECIFICATION (Child of [`DEC-036`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-036-universal-2-in-2-out-joinsplit-full-stack-architecture.md))
- **Author:** Zeltra Architecture & Cryptography Team
- **Date:** 2026-10-10
- **Applies to:**
  - `nimbus-sdk/src/wallet/note_wallet.rs` (`PrivateNoteWallet` state, coin selection, local proof generation, and consolidation)
  - `nimbus-sdk/src/coin_selection.rs` (Stochastic Knapsack multi-note selection algorithm)
  - `nimbus-sdk/src/client.rs` (HTTP client routines for relayer quotes, bulk MMR sync, and spend broadcast)
  - `nimbus-sdk/src/storage.rs` (SQLCipher / IndexedDB crash-safe 2PC note state transitions)
- **Parent & Related DECs:**
  - [`DEC-036`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-036-universal-2-in-2-out-joinsplit-full-stack-architecture.md) (Master JoinSplit Architecture Blueprint)
  - [`DEC-036A`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-036A-universal-2-in-2-out-joinsplit-r1cs-circuit-specification.md) (Universal 2-in-2-out JoinSplit R1CS Circuit & Soundness)
  - [`DEC-036C`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-036C-relayer-ingress-guard-and-stylus-joinsplit-settlement.md) (Relayer Ingress & Stylus Settlement)
  - [`DEC-024`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-024-client-private-wallet-state-crash-safety-coin-selection.md) (Client Private Wallet State & Crash Safety)
  - [`DEC-030`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-030-multi-utxo-joinsplit-coin-selection-zeroize-and-aead-backup.md) (Multi-UTXO JoinSplit Coin Selection & AEAD Backup)
  - [`DEC-032`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-032-merkle-mountain-range-commitment-accumulator.md) (Merkle Mountain Range Commitment Accumulator)
  - [`DEC-035C`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-035C-relayer-mmr-indexer-worker-and-client-sync-architecture.md) (Relayer MMR Indexer Worker & Sync Architecture)

---

## 1. Executive Summary & Problem Statement

Penerapan sirkuit ZK-UTXO JoinSplit 2-in-2-out ([`DEC-036A`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-036A-universal-2-in-2-out-joinsplit-r1cs-circuit-specification.md)) menuntut keandalan dan privasi absolut pada lapisan klien SDK (`nimbus-sdk`). Visi Zeltra Protocol adalah **Privasi 9.5/10, Produk 10/10**: pengguna awam harus menikmati kecepatan dan kemudahan seperti Apple Pay atau QRIS tanpa perlu memahami komputasi kriptografi di balik layar. Namun, implementasi dompet UTXO privat menghadapi jebakan arsitektur yang sering kali merusak privasi atau menyebabkan kehilangan dana (*funds loss*):

1. **Balance Fragmentation Lockout ($N > 2$):** Karena sirkuit JoinSplit dibatasi tepat 2 input demi menjaga waktu pembuktian (< 2.5 detik) dan ukuran constraint (~10.800 R1CS constraints), dompet dengan banyak pecahan saldo mikro ($N > 2$) akan gagal bertransaksi tanpa mekanisme konsolidasi bertahap (*in-pool progressive consolidation*).
2. **Metadata Snooping Threat on Tree Sync (ZIP 314):** Jika klien meminta jalur bukti Merkle/MMR secara spesifik per-daun (`GET /api/v1/mmr/proof/{leaf_index}`), operator relayer dapat memetakan indeks daun yang dimiliki pengguna, menghancurkan himpunan anonimitas (*anonymity set*) dari luar.
3. **Ghost Notes & In-Flight Crash Mismatch:** Jika aplikasi/browser crash tepat saat transaksi dipancarkan, mutasi status note yang tidak atomik dapat menyebabkan note lama terkunci permanen atau change note baru hilang tak tercatat.
4. **Knapsack Fingerprinting & Dust Accumulation:** Algoritma pemilihan koin yang deterministik dapat membocorkan heuristik change note dan menghasilkan serpihan debu (*dust*) yang tidak bernilai ekonomis.

**DEC-036B** menetapkan spesifikasi komprehensif Client SDK untuk memecahkan tantangan-tantangan ini dengan standar **Zero Tech Debt**.

---

## 2. Vulnerability Archaeology & Post-Mortem Analysis

Audit keamanan industri (ZK-Security, Least Authority, Trail of Bits) dan penelitian privasi dompet (Zcash, Wasabi, OXT Research) menunjukkan bahwa celah privasi terbesar sering kali bukan berada pada matematika sirkuit, melainkan pada lapisan **interaksi dompet klien dan server indexer**:

```
                              CLIENT WALLET THREAT ARCHAEOLOGY
┌────────────────────────────────────────────────────────────────────────────────────────┐
│ [Case A: Zcash ZIP 314 Snooping]  [Case B: Shielder State Desync] [Case C: Wasabi Dust]│
│ Lightwalletd recovers entire tx   ZK-Security 2025: Unsafe        OXT Research 2020:   │
│ graph via point memo/proof query  storage interface orphans notes Knapsack heuristics │
│ -> Anonymity set completely broken during client browser crashes   leak change clustering│
├───────────────────────────────────┼───────────────────────────────┼────────────────────┤
│ [Case D: ZIP 315 Anchor Reorgs]   [Case E: Tezos MASP Extension]  [Case F: ZK-PIR Cost]│
│ Revealed nullifiers on reorg link Least Authority 2024: Dynamic   Bandwidth blowup of  │
│ invalidated tx to future spends   denominations risk asset leaks  3MB per query breaks │
│ without reorg-safe rollback window in multi-UTXO balance math      mobile consumer UX   │
└───────────────────────────────────┴───────────────────────────────┴────────────────────┘
```

### A. Zcash Light Client Protocol: Transaction Graph Recovery (ZIP 314 & ZIP 307)
- **Konteks Kerentanan:** Dalam analisis formal protokol Zcash light client ([ZIP 314 / Issue #434](https://github.com/zcash/zips/issues/434); [Zcash Community Forum, 2021–2024](https://forum.zcashcommunity.com/t/zip-314-privacy-upgrades-to-the-zcash-light-client-protocol/38868)), ditemukan bahwa server penghubung (`lightwalletd`) dapat memetakan graf transaksi pengguna secara penuh.
- **Root Cause:** Dompet klien meminta data terenkripsi atau bukti pohon secara individual per-transaksi/per-daun yang ingin dibelanjakannya. Dengan mengamati IP address klien, stempel waktu, dan indeks spesifik yang diminta, `lightwalletd` (serta penyedia ISP / penguping pasif) dapat menghubungkan pengirim dengan penerima secara deterministik, mereduksi himpunan anonimitas menjadi 1.
- **Relevansi & Mitigasi Zeltra (DEC-036B):**
  Zeltra **melarang keras kueri bukti per-daun tunggal (`GET /api/v1/mmr/proof/:leaf_index`) pada alur produksi SDK**. Sebagai gantinya, SDK menerapkan **Paginated Bulk Range Sync** (`GET /api/v1/mmr/leaves?from={idx}&limit=1000`). Seluruh klien mengunduh blok daun komitmen seragam (~32 KB per 1.000 komitmen) dan merekonstruksi pohon MMR serta jalur autentikasi puncak secara lokal di memori CPU/WASM. Relayer sama sekali tidak memiliki cara untuk mengetahui indeks daun mana yang menjadi milik klien.

---

### B. Aleph Zero Shielder: State Persistence Desynchronization (ZK-Security Audit, 2025)
- **Konteks Kerentanan:** Dalam audit formal ZK-Security terhadap Aleph Zero Shielder ([ZK-Security Audit Reports, Maret 2025](https://reports.zksecurity.xyz/reports/aleph-zero-shielder/)), diidentifikasi kelemahan kritis pada modul persistensi status dompet (`InjectedStorageInterface`).
- **Root Cause:** Penyimpanan status note privat diserahkan ke antarmuka abstrak tanpa jaminan Two-Phase Commit (2PC). Jika browser tertutup, daya mati, atau koneksi terputus tepat setelah bukti di-generate dan dipancarkan ke RPC sebelum penerimaan event deposit/withdraw, status database lokal terputus dari rantai on-chain. Akibatnya, note lama tetap berstatus aktif (menyebabkan revert `NullifierAlreadySpent` saat dicoba kembali) atau calon change note baru tidak tersimpan, mengunci dana pengguna selamanya (*funds permanently locked in limbo*).
- **Relevansi & Mitigasi Zeltra (DEC-036B):**
  SDK Zeltra menerapkan mesin status **Crash-Safe 2PC Engine** di atas SQLite SQLCipher dengan mode journaling `WAL` (Write-Ahead Logging):
  1. *Phase 1 (Pre-Broadcast Lock):* Note input ditandai `NoteStatus::SpentPending`, dan calon change notes dicatat dengan status `NoteStatus::Unconfirmed` secara atomik dalam satu transaksi SQL.
  2. *Phase 2 (Post-Confirmation Finalization):* Receipt on-chain yang valid (`status == 1`) menaikkan status change note menjadi `NoteStatus::Active` dan mengarsipkan input note sebagai `NoteStatus::SpentConfirmed`. Jika transaksi revert atau kadaluarsa, SDK mengeksekusi rollback deterministik ke status semula.

---

### C. Wasabi Wallet: Knapsack Change Fingerprinting & Toxic Recall (OXT Research, 2020–2026)
- **Konteks Kerentanan:** Analisis OXT Research terhadap Wasabi Wallet ([OXT Research / Samourai Statement](https://medium.com/oxt-research/a-statement-on-two-discovered-vulnerabilities-in-wasabi-wallet-6e11e29a6ea8); [Wasabi Docs on Coins & Dusting](https://docs.wasabiwallet.io/why-wasabi/Coins.html)) mengungkap bahwa algoritma coin selection Knapsack deterministik membuka celah deanonimisasi.
- **Root Cause:**
  1. *Change Clustering:* Jika algoritma pemilihan koin selalu memilih kombinasi matematis terkecil tanpa variansi acak, pengamat on-chain dapat menebak mana output pembayaran dan mana output kembalian (*change output*) berdasarkan pola pecahan angka tak bulat.
  2. *Dust Attacks:* Penyerang mengirimkan sejumlah kecil satoshi debu (*dust coin*) ke alamat yang ada. Dompet yang secara otomatis mengonsolidasi debu ini dengan note privat lain akan mengaitkan seluruh kluster saldo privat ke identitas publik penyerang (*toxic recall*).
- **Relevansi & Mitigasi Zeltra (DEC-036B):**
  1. *Stochastic Knapsack Optimization:* Algoritma pemilihan note menambahkan faktor entropi terkontrol ($\text{entropy\_noise}$) pada fungsi penalti untuk mengacak pemilihan pasangan note yang setara.
  2. *Economic Dust Floor:* Zeltra menetapkan batas debu ekonomis ($\text{min\_dust\_threshold} = 0.10 \text{ USDC}$). Jika sisa kembalian berada di bawah $0.10 USDC, sistem menyerap sisa tersebut ke dalam alokasi eksekusi gas daripada membuat change note kerdil yang membebani pohon komitmen dan membocorkan tautan kluster.

---

### D. Zcash ZIP 315: Anchor Depth, Reorg Invalidation, and Rollback Linkability (ECC, 2021–2024)
- **Konteks Kerentanan:** Standar ZIP 315 ([Zcash ZIP 315](https://zips.z.cash/zip-0315)) meneliti dampak reorganisasi blockchain (*reorgs*) terhadap privasi nullifier.
- **Root Cause:** Jika transaksi membelanjakan note dengan anchor di puncak rantai (tinggi $H$) dan terjadi reorg sedalam $k$ blok, transaksi tersebut invalid on-chain. Namun, nullifier transaksi telah bocor ke mempool dan jaringan p2p. Jika dompet mencoba membelanjakan note itu kembali dalam transaksi baru dengan parameter berbeda, pengamat dapat mengaitkan kedua transaksi tersebut melalui kesamaan nullifier.
- **Relevansi & Mitigasi Zeltra (DEC-036B):**
  SDK Zeltra hanya memilih MMR root yang memiliki kedalaman konfirmasi aman ($\ge 3$ konfirmasi pada Arbitrum Sepolia). Jika transaksi kadaluarsa tanpa pernah masuk blok, nullifier tidak pernah terekspos on-chain, dan note dikembalikan ke status `Active` dengan aman.

---

### E. Least Authority Tezos Multi-Asset Shielded Pool Audit (2020–2024)
- **Konteks Kerentanan:** Audit Least Authority terhadap ekstensi multi-aset Sapling ([Least Authority Tezos Audit Report](https://leastauthority.com/static/publications/LeastAuthority_Tezos_Foundation_Multi_Asset_Shielded_Pool_Audit_Report.pdf)) menggarisbawahi kompleksitas penanganan multi-UTXO dan fragmentasi denominasi.
- **Root Cause:** Memperbesar ariti sirkuit untuk mendukung $N > 2$ input secara langsung melipatgandakan jumlah gerbang R1CS secara drastis (hingga >30.000 constraints), memperlambat eksekusi client-side di browser hingga puluhan detik dan menyebabkan kegagalan out-of-memory pada perangkat mobile.
- **Relevansi & Mitigasi Zeltra (DEC-036B):**
  Zeltra mempertahankan sirkuit JoinSplit pada ukuran optimal ~10.800 constraints ($N=2$), dan memindahkan beban fragmentasi saldo tinggi ($N > 2$) ke arsitektur **In-Pool Progressive Consolidation** (`consolidate_notes`).

---

## 3. Invariants & Guarantees

Modul Client SDK wajib mematuhi invarian operasional berikut:

- **INV-SDK-1 (Zero Metadata Leakage Sync):** Tidak ada kueri keluar dari klien yang memuat indeks daun, hash komitmen spesifik, nullifier, atau alamat publik pemilik selama proses sinkronisasi pohon komitmen MMR.
- **INV-SDK-2 (Two-Phase Commit Database Atomicity):** Transisi status database lokal untuk note yang dikonsumsi dan note baru yang diciptakan wajib dieksekusi secara atomik dalam satu transaksi database SQLite/IndexedDB terenkripsi.
- **INV-SDK-3 (Deterministic Knapsack Soundness):** Algoritma pemilihan note wajib memprioritaskan pemenuhan transaksi menggunakan 1 note (jika saldo cukup), lalu 2 note dengan variansi kembalian minimal, dan menolak pembuatan pecahan debu di bawah ambang batas ekonomis ($< \text{min\_dust\_threshold}$).
- **INV-SDK-4 (Strict Constant-Arity Client Compliance):** Untuk transaksi 1-input, SDK wajib membangkitkan *Canonical Dummy Witness* untuk slot input 2 dan *Canonical Dummy Output Witness* untuk slot output 2 (Direction B), menyamarkan transaksi ke format standar 2-in-2-out.

---

## 4. Multi-UTXO Stochastic Knapsack Coin Selection Architecture

Algoritma pemilihan koin pada `nimbus-sdk/src/coin_selection.rs` beroperasi melalui skema hirarkis 4-tingkat:

```
                       COIN SELECTION DECISION PIPELINE
                                Target Amount: T
                                       │
                                       ▼
                   ┌───────────────────────────────────────┐
                   │ Tier 1: Exact Match Single Note (v=T)?│
                   └───────────────────┬───────────────────┘
                                       │ No
                                       ▼
                   ┌───────────────────────────────────────┐
                   │ Tier 2: Smallest Single Note (v > T)? │
                   └───────────────────┬───────────────────┘
                                       │ No
                                       ▼
                   ┌───────────────────────────────────────┐
                   │ Tier 3: Stochastic Knapsack 2 Notes?  │
                   │        (v1 + v2 >= T, Min Dust)       │
                   └───────────────────┬───────────────────┘
                                       │ No (N >= 3 needed)
                                       ▼
                   ┌───────────────────────────────────────┐
                   │ Tier 4: Trigger In-Pool Consolidation │
                   │  Suggest 'consolidate_notes()' to user│
                   └───────────────────────────────────────┘
```

### Formulasi Matematis Tier 3 (Stochastic Knapsack Optimization):
1. **Target Pengeluaran Total ($T$):**
   $$T = v_{\text{merchant}} + v_{\text{protocol\_fee}} + v_{\text{execution\_fee}}$$
   di mana $v_{\text{protocol\_fee}} = \lceil (v_{\text{merchant}} \times 45) / 10\,000 \rceil$.
2. **Kandidat Pasangan Note:**
   Ambil seluruh pasangan note aktif $(n_i, n_j)$ sedemikian rupa sehingga:
   $$v_i + v_j \ge T$$
3. **Fungsi Penalti Biaya:**
   $$\text{Cost}(i, j) = (v_i + v_j - T) + \alpha \cdot |v_i - v_j| + \beta \cdot \text{AgePenalty}(n_i, n_j) + \epsilon$$
   di mana:
   - $(v_i + v_j - T)$ meminimalkan nilai kembalian berlebih (*excess change*).
   - $\alpha \cdot |v_i - v_j|$ memprioritaskan penyatuan note dengan denominasi pecahan yang sebanding.
   - $\beta \cdot \text{AgePenalty}$ memprioritaskan pembersihan note dari epoch lama ($E-1$).
   - $\epsilon \leftarrow \mathcal{U}(0, \sigma)$ adalah faktor derau stokastik acak untuk mengacaukan analisis heuristik surveillance (mencegah *Knapsack fingerprinting*).
4. **Pencegahan Debu Kembalian (Dust Prevention):**
   Jika sisa kembalian $v_{\text{change}} = (v_i + v_j - T) < \text{min\_dust\_threshold}$ ($100\,000$ unit / $0.10 USDC):
   - Tambahkan sisa tersebut ke komponen $v_{\text{execution\_fee}}$ (sebagai tip/markup relayer).
   - Nonaktifkan pembuatan change note 1 (`has_change_1 = false`), dan alihkan ke mode dummy output. Hal ini mencegah terciptanya note debu yang tidak dapat dibelanjakan di masa depan.

---

## 5. In-Pool Progressive Consolidation Architecture (`consolidate_notes`)

Ketika saldo pengguna terfragmentasi ke dalam $N \ge 3$ note kecil dan tidak ada kombinasi 2 note yang mampu memenuhi target pembayaran $T$, SDK mengaktifkan alur konsolidasi internal:
`pub async fn consolidate_notes(&self, target_amount: Option<u64>) -> Result<ConsolidationSummary, WalletError>`

```
                     IN-POOL CONSOLIDATION LIFECYCLE
                      Initial: [ $2, $2, $2, $2 ] ($8 total)
                              Want to pay: $7
                                     │
                                     ▼
                ┌────────────────────────────────────────┐
                │ Round 1: Consolidate Note 1 & Note 2   │
                │ Inputs:  $2.00 + $2.00 = $4.00         │
                │ Payout:  $0.00 (Merchant)              │
                │ Fee:     $0.02 (Relayer Gas)           │
                │ Output:  $3.98 (Consolidated Change)   │
                └────────────────────┬───────────────────┘
                                     │ Note Pool: [ $3.98, $2.00, $2.00 ]
                                     ▼
                ┌────────────────────────────────────────┐
                │ Round 2: Consolidate Note 3 & Note 4   │
                │ Inputs:  $2.00 + $2.00 = $4.00         │
                │ Payout:  $0.00 (Merchant)              │
                │ Fee:     $0.02 (Relayer Gas)           │
                │ Output:  $3.98 (Consolidated Change)   │
                └────────────────────┬───────────────────┘
                                     │ Note Pool: [ $3.98, $3.98 ]
                                     ▼
                ┌────────────────────────────────────────┐
                │ Final Payment: JoinSplit Spend         │
                │ Inputs:  $3.98 + $3.98 = $7.96         │
                │ Payout:  $7.00 to Merchant             │
                │ Fees:    $0.03 (Protocol) + $0.02 (Gas)│
                │ Output:  $0.91 Change Note             │
                └────────────────────────────────────────┘
```

### Sifat Transaksi Konsolidasi:
- **Zero-Payout Shielded Self-Spend:** Transaksi konsolidasi adalah transaksi JoinSplit standar on-chain dengan $v_{\text{merchant}} = 0$. Kontrak tidak mentransfer ERC-20 keluar pool.
- **Biaya Minimal:** Karena $v_{\text{merchant}} = 0$, protocol fee bernilai **$0.00** (0% dari 0). Pengguna hanya membayar biaya kompensasi gas relayer (`execution_fee` ~0.015 - 0.02 USDC).
- **In-Pool Atomicity:** Dana tidak pernah keluar ke alamat publik Ethereum/Arbitrum; saldo tetap berada 100% di dalam shielded pool Zeltra.
- **UX Warning & Approval:** Klien menghitung jumlah putaran konsolidasi yang dibutuhkan dan total estimasi gas, meminta konfirmasi satu kali dari pengguna sebelum mengeksekusi pipeline.

---

## 6. Anti-Snooping Paginated Bulk MMR Tree Sync Engine

Untuk menjamin kepatuhan mutlak terhadap **INV-SDK-1**, antarmuka sinkronisasi status pohon diimplementasikan sebagai berikut:

```
Client Wallet (WASM / CPU)                                 Relayer Node
     │                                                          │
     │ 1. GET /api/v1/mmr/status                                │
     │─────────────────────────────────────────────────────────>│
     │                                                          │
     │ 2. 200 OK: { latest_leaf_count: 5240, note_root: "0x.." }│
     │<─────────────────────────────────────────────────────────│
     │                                                          │
     │ [Client local checkpoint: synced_leaf_count = 4000]      │
     │ [Delta = 1240 leaves. Requires 2 chunk requests]         │
     │                                                          │
     │ 3. GET /api/v1/mmr/leaves?from=4000&limit=1000           │
     │─────────────────────────────────────────────────────────>│
     │ 4. 200 OK: [1000 leaves, hex encoded, ~32 KB]            │
     │<─────────────────────────────────────────────────────────│
     │                                                          │
     │ 5. GET /api/v1/mmr/leaves?from=5000&limit=1000           │
     │─────────────────────────────────────────────────────────>│
     │ 6. 200 OK: [240 leaves, hex encoded, ~7.7 KB]            │
     │<─────────────────────────────────────────────────────────│
     │                                                          │
     │ 7. Local MMR Incremental Append:                         │
     │    - Append 1240 leaves to local in-memory MMR           │
     │    - Compute peaks P_0..P_m-1, compute bagged root       │
     │    - Assert bagged_root == response.note_root            │
     │                                                          │
     │ 8. Generate Peak Authentication Paths for Note 1 & 2:    │
     │    - Executed 100% in local memory in < 5ms              │
     │    - Relayer has ZERO knowledge of leaf indices!         │
```

### Keunggulan Efisiensi & Bandwidth:
- Setiap komitmen daun adalah 32 bytes skalar. Satu halaman 1.000 daun berukuran **hanya ~32 KB data biner**.
- Payload terkompresi gzip/brotli berukuran $< 28 \text{ KB}$, dapat diunduh dalam $< 50 \text{ ms}$ pada jaringan mobile 4G/5G.
- Endpoint ini sepenuhnya dapat di-cache menggunakan HTTP CDN / reverse proxy (`Cache-Control: public, max-age=31536000, immutable`), mengurangi beban komputasi relayer hingga 98%.

---

## 7. Dual-Witness Generation & Dual-Epoch Synchronization

Ketika pengguna memiliki note yang berasal dari dua epoch registrasi yang berbeda (misal Note 1 dari Epoch $E$ dan Note 2 dari Epoch $E-1$):
1. **Verifikasi Masa Aktif Epoch:**
   SDK memeriksa apakah Epoch $E-1$ masih berada di dalam jendela toleransi rollover aktif (DEC-033). Jika sudah kadaluarsa penuh, note ditolak dan diarahkan ke transaksi rollover terlebih dahulu.
2. **Penyusunan Saksi Saksi (Witness Assignment):**
   ```rust
   let witness_1 = PrivateNoteWitness {
       value: note_1.amount,
       owner: note_1.owner_pk,
       rho: note_1.rho,
       rand: note_1.rand,
       nullifier_key: note_1.nullifier_key,
       epoch_id: note_1.epoch_id, // e.g. E
       merkle_path: local_mmr.get_proof(note_1.leaf_index)?,
   };

   let witness_2 = if is_dummy {
       PrivateNoteWitness::canonical_dummy(epoch_1)
   } else {
       PrivateNoteWitness {
           value: note_2.amount,
           owner: note_2.owner_pk,
           rho: note_2.rho,
           rand: note_2.rand,
           nullifier_key: note_2.nullifier_key,
           epoch_id: note_2.epoch_id, // e.g. E - 1
           merkle_path: local_mmr.get_proof(note_2.leaf_index)?,
       }
   };
   ```
3. **Public Input Synthesis:**
   SDK menyusun array 19 public inputs secara presisi sesuai spesifikasi [`DEC-036A (Bagian 5)`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-036A-universal-2-in-2-out-joinsplit-r1cs-circuit-specification.md#5-canonical-19-public-inputs-specification).

---

## 8. Crash-Safe 2PC Note Lifecycle & AEAD Storage Schema

Database lokal klien (`SQLCipher` terenkripsi AES-256-GCM) menerapkan skema tabel berikut:

```sql
CREATE TABLE private_notes (
    commitment TEXT PRIMARY KEY,
    leaf_index INTEGER,
    value INTEGER NOT NULL,
    owner_pk TEXT NOT NULL,
    rho TEXT NOT NULL,
    rand TEXT NOT NULL,
    nullifier_key TEXT NOT NULL,
    epoch_id INTEGER NOT NULL,
    status TEXT NOT NULL CHECK(status IN ('UNCONFIRMED', 'ACTIVE', 'SPENT_PENDING', 'SPENT_CONFIRMED')),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE TABLE joinsplit_sessions (
    session_id TEXT PRIMARY KEY,
    input_nf_1 TEXT NOT NULL,
    input_nf_2 TEXT NOT NULL,
    output_cm_1 TEXT NOT NULL,
    output_cm_2 TEXT NOT NULL,
    tx_hash TEXT,
    status TEXT NOT NULL CHECK(status IN ('PENDING_BROADCAST', 'BROADCASTED', 'CONFIRMED', 'FAILED_ROLLBACK')),
    created_at INTEGER NOT NULL
);
```

### Protokol Eksekusi Dua Fase (2PC):
1. **Fase 1 (Pre-Broadcast Atomic Lock):**
   ```sql
   BEGIN TRANSACTION;
   UPDATE private_notes SET status = 'SPENT_PENDING' WHERE commitment IN (cm_in1, cm_in2);
   INSERT INTO private_notes (commitment, value, status, ...) VALUES (cm_out1, v_out1, 'UNCONFIRMED', ...);
   INSERT INTO private_notes (commitment, value, status, ...) VALUES (cm_out2, v_out2, 'UNCONFIRMED', ...);
   INSERT INTO joinsplit_sessions (session_id, input_nf_1, input_nf_2, output_cm_1, output_cm_2, status, ...)
   VALUES (sess_id, nf1, nf2, cm_out1, cm_out2, 'PENDING_BROADCAST', ...);
   COMMIT;
   ```
2. **Fase 2A (Post-Confirmation Finalization — on RPC success receipt):**
   ```sql
   BEGIN TRANSACTION;
   UPDATE private_notes SET status = 'SPENT_CONFIRMED' WHERE commitment IN (cm_in1, cm_in2);
   UPDATE private_notes SET status = 'ACTIVE' WHERE commitment IN (cm_out1, cm_out2) AND value > 0;
   UPDATE joinsplit_sessions SET status = 'CONFIRMED', tx_hash = ? WHERE session_id = sess_id;
   COMMIT;
   ```
3. **Fase 2B (Rollback on Revert / Expiry):**
   ```sql
   BEGIN TRANSACTION;
   UPDATE private_notes SET status = 'ACTIVE' WHERE commitment IN (cm_in1, cm_in2);
   DELETE FROM private_notes WHERE commitment IN (cm_out1, cm_out2) AND status = 'UNCONFIRMED';
   UPDATE joinsplit_sessions SET status = 'FAILED_ROLLBACK' WHERE session_id = sess_id;
   COMMIT;
   ```

---

## 9. Acceptance Criteria & Test Matrix

Modul `nimbus-sdk` wajib lulus pengujian unit dan integrasi berikut:

1. **Coin Selection Optimization:**
   - Single note pas: memilih Tier 1, dummy slot 2 diaktifkan.
   - Single note lebih besar: memilih Tier 2, menghitung 1 real change + 1 dummy change.
   - 2 note kombinasi: memilih Tier 3 yang meminimalkan kembalian dan debu.
   - Fragmentasi ekstrem ($N \ge 3$ note kecil): memicu return `FragmentationLockout` yang menyarankan `consolidate_notes`.
2. **In-Pool Consolidation Test:**
   - Mensimulasikan dompet dengan 6 note @$1.50 USDC (total $9.00 USDC).
   - Menjalankan `consolidate_notes` otomatis hingga saldo terkonsolidasi menjadi 2 note.
   - Memastikan seluruh nilai saldo terkonservasi dikurangi gas relayer presisi.
3. **Anti-Snooping Bulk Sync Integrity:**
   - Mengunduh 10.000 komitmen daun menggunakan chunk 1.000 daun secara bertahap.
   - Memvalidasi kesamaan akar puncak lokal dengan on-chain `note_root`.
   - Menguji bahwa waktu generasi path Merkle lokal $< 10 \text{ ms}$.
4. **Crash-Safety Simulation (2PC Rollback):**
   - Mematikan proses tepat setelah broadcast dikirimkan dan me-restart wallet instance.
   - Menguji skenario receipt sukses (transisi status ke `Active`) dan skenario receipt revert (rollback ke status awal tanpa kehilangan dana).

---

## 10. References & Citations

1. **Zcash Community & Electric Coin Company.** (2021). *ZIP 314: Privacy upgrades to the Zcash light client protocol.* Zcash Improvement Proposals / GitHub Issue #434. [https://github.com/zcash/zips/issues/434](https://github.com/zcash/zips/issues/434) & [https://forum.zcashcommunity.com/t/zip-314-privacy-upgrades-to-the-zcash-light-client-protocol/38868](https://forum.zcashcommunity.com/t/zip-314-privacy-upgrades-to-the-zcash-light-client-protocol/38868)
2. **Zcash Community & Electric Coin Company.** (2021). *ZIP 315: Best Practices for Wallet Implementations — Reducing Transaction Linkability and Arity Leakage.* Zcash Improvement Proposals. [https://zips.z.cash/zip-0315](https://zips.z.cash/zip-0315)
3. **ZK-Security.** (2025). *Audit of Aleph Zero Shielder: State Persistence and Synchronization Pitfalls in Shielded Account Notes.* ZK-Security Public Audit Reports. [https://reports.zksecurity.xyz/reports/aleph-zero-shielder/](https://reports.zksecurity.xyz/reports/aleph-zero-shielder/)
4. **OXT Research & Samourai Wallet.** (2020). *A Statement on Two Discovered Vulnerabilities in Wasabi Wallet: Toxic Recall and Change Clustering Heuristics.* OXT Research Publications. [https://medium.com/oxt-research/a-statement-on-two-discovered-vulnerabilities-in-wasabi-wallet-6e11e29a6ea8](https://medium.com/oxt-research/a-statement-on-two-discovered-vulnerabilities-in-wasabi-wallet-6e11e29a6ea8)
5. **zkSNACKs / Wasabi Wallet Community.** (2026). *Wasabi Wallet Documentation: Coins, Dusting Attacks, and Change Output Linkability.* Wasabi Documentation. [https://docs.wasabiwallet.io/why-wasabi/Coins.html](https://docs.wasabiwallet.io/why-wasabi/Coins.html) & [https://docs.wasabiwallet.io/why-wasabi/AddressReuse.html](https://docs.wasabiwallet.io/why-wasabi/AddressReuse.html)
6. **Least Authority.** (2020). *Tezos Foundation Multi-Asset Shielded Pool (MASP) Sapling Extension Audit Report.* Least Authority Security Audits. [https://leastauthority.com/static/publications/LeastAuthority_Tezos_Foundation_Multi_Asset_Shielded_Pool_Audit_Report.pdf](https://leastauthority.com/static/publications/LeastAuthority_Tezos_Foundation_Multi_Asset_Shielded_Pool_Audit_Report.pdf)
7. **Kappos, George; Yousaf, Haaroon; Maller, Mary; & Meiklejohn, Sarah.** (2018). *An Empirical Analysis of Anonymity in Zcash.* In: 27th USENIX Security Symposium (USENIX Security 18), pp. 463–477. [https://www.usenix.org/system/files/conference/usenixsecurity18/sec18-kappos.pdf](https://www.usenix.org/system/files/conference/usenixsecurity18/sec18-kappos.pdf)
8. **Hopwood, Daira; Bowe, Sean; Hornby, Taylor; & Wilcox, Zooko.** (2024). *Zcash Protocol Specification (Version 2024.1.0): Section 4.8.2 Dummy Notes & Section 4.17.2 Spend Statement.* Electric Coin Company. [https://zips.z.cash/protocol/protocol.pdf](https://zips.z.cash/protocol/protocol.pdf)
9. **Arkworks Community.** (2024). *ark-groth16: Efficient Groth16 Prover and Verifier in Rust.* Arkworks Ecosystem. [https://github.com/arkworks-rs/groth16](https://github.com/arkworks-rs/groth16)
10. **Bowe, Sean; Hopwood, Daira; & Wilcox, Zooko.** (2017). *BLS12-381: New zk-SNARK Elliptic Curve Construction.* Electric Coin Company. [https://electriccoin.co/blog/new-snark-curve/](https://electriccoin.co/blog/new-snark-curve/)
