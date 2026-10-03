# Nimbus Relayer — 03: Settlement Queue & Adaptive Batching

Dokumen ini menjelaskan mesin antrian persisten, mekanisme *leasing worker*, siklus hidup transaksi, dan logika penggabungan batch pada [`nimbus-node/src/handlers/spend.rs`](file:///workspaces/Zeltra-Protocol/nimbus-node/src/handlers/spend.rs) (mengacu pada spesifikasi [`DEC-005`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-005-persistent-spend-settlement-queue.md) & [`DEC-006`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-006-adaptive-private-spend-batching.md)).

---

## 1. Siklus Hidup Transaksi Antrian (`spend_queue`)

Untuk menjamin agar tidak ada transaksi atau dana pengguna yang hilang saat node restart atau crash, antrian disimpan durable di SQLite dengan mesin status berikut:

```text
       [ POST /api/spend ]
                |
                v
            ( queued )
                |
                | Worker mengklaim dengan lease 60 detik (claim_spends)
                v
          ( broadcasting )
                |
                | Transaksi on-chain terkirim ke mempool RPC
                v
           ( submitted )
           /           \
  Receipt Sukses       Receipt Revert / RPC Timeout
         /               \
        v                 v
  ( confirmed )     ( retryable )  <-- Exponential backoff
        |                 |
  Nullifier sah       Retry > 5x
  masuk ke DB             |
                          v
                      ( failed )
```

---

## 2. Mekanisme Klaim Transaksional Ber-Lease (*Worker Lease*)

Untuk mencegah kondisi balapan (*race condition*) di mana dua worker background mengambil dan menyiarkan transaksi yang sama secara bersamaan:

* Saat worker mengambil batch transaksi, worker mengeksekusi:
  ```sql
  UPDATE spend_queue 
  SET status = 'broadcasting', lease_expires_at = ? 
  WHERE id IN (...) AND (status = 'queued' OR (status = 'retryable' AND lease_expires_at < ?));
  ```
* **Pemulihan Otomatis Pasca Crash:** Jika proses node mati di tengah-tengah broadcast, item yang ditinggalkan akan memiliki `lease_expires_at` yang kedaluwarsa. Begitu node menyala kembali, worker berikutnya secara otomatis mengambil alih item tersebut tanpa intervensi manual.

---

## 3. Mesin Adaptive Batching (2 s/d 8 Transaksi)

File referensi: [`nimbus-node/src/handlers/spend.rs`](file:///workspaces/Zeltra-Protocol/nimbus-node/src/handlers/spend.rs)

Jika variabel `NIMBUS_BATCH_ENABLED=true` aktif, relayer tidak mengirimkan transaksi satu per satu:

1. **Jendela Adaptif (*Adaptive Window*):**
   * Target jeda kumpul: **1 detik**.
   * Batas waktu tunggu maksimal (*Hard Timeout*): **2 detik**.
   * Jika antrian mencapai 8 transaksi sebelum 1 detik, transaksi langsung dibroadcast seketika tanpa menunggu.
2. **Pengecualian (*Bypass*):**
   * Transaksi single yang memiliki batas waktu kadaluarsa (*expiry*) mendekati detik-detik akhir akan langsung dikirim melalui jalur `spend()` tunggal tanpa menunggu batching.
3. **Penghematan Biaya:**
   * Menggabungkan 8 transaksi ke dalam satu pemanggilan `batch_spend()` membagi biaya calldata L1 dan setup gas precompile EIP-2537, memangkas biaya gas per transaksi hingga **~35%**.

---

## 4. Pemecahan Batch Gagal (*Failure Unbundling*)

Jika sebuah transaksi batch mengalami revert di on-chain:
* Relayer **tidak membatalkan seluruh isi batch**.
* Relayer secara otomatis **memecah batch (*unbundle*)** dan menandai setiap item kembali ke status `retryable`.
* Pada siklus pengiriman berikutnya, item-item tersebut diuji secara independen sehingga satu transaksi invalid/bermasalah tidak menyandera transaksi valid milik pengguna lain.

---

## 5. Manajemen Nonce & Transaksi Pengganti (*Transaction Replacement*)

Mengacu pada spesifikasi [`DEC-017`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-017-relayer-receipt-finality-and-nonce-management.md), relayer mencegah kegagalan nonce dan transaksi tersangkut di mempool melalui dua mekanisme:

1. **Alokasi Nonce Atomik (*Thread-Safe Lock*):**
   * Di dalam [`nimbus-node/src/evm_client.rs`](file:///workspaces/Zeltra-Protocol/nimbus-node/src/evm_client.rs), pengiriman transaksi diamankan oleh `nonce_lock: Arc<Mutex<Option<u64>>>`.
   * Mencegah dua worker konkuren mengambil nonce yang sama atau melompati nonce (*nonce gap*) yang memicu penolakan sequencer Arbitrum Nitro.
   * **Pemulihan Otomatis (*Auto-Resync*):** Jika RPC merespons error `nonce too low`, `nonce too high`, atau `already known`, relayer seketika melakukan query ulang ke pending count on-chain dan menyiarkan ulang transaksi secara transparan.

2. **Watchdog Mempool & Eskalasi Gas (+15%):**
   * Jika transaksi belum terinklusi dalam blok setelah batas waktu `NIMBUS_TX_TIMEOUT_SECS` (default: 30 detik), sistem secara otomatis membuat transaksi pengganti (*replacement transaction*).
   * **Aturan Penggantian:** Transaksi pengganti menggunakan **nonce yang sama persis** dengan parameter gas dinaikkan minimal **+15%** (`maxFeePerGas` dan `maxPriorityFeePerGas`) sesuai standar EIP-1559.
   * Transaksi pengganti dapat dieskalasi hingga batas `NIMBUS_MAX_GAS_BUMPS` (default: 3 kali).

---

## 6. Ambang Batas Konfirmasi (*Confirmation Threshold*) & Reorg Protection

Untuk melindungi relayer dari risiko chain reorganization:
* Status transaksi tidak ditandai `confirmed` hanya berdasarkan ketersediaan receipt awal.
* Relayer memverifikasi kedalaman blok konfirmasi:
  $$\text{current\_block} - \text{receipt\_block} + 1 \ge \text{confirmation\_threshold}$$
* Nilai ambang batas dapat dikonfigurasi melalui `NIMBUS_CONFIRMATION_THRESHOLD`:
  * **Arbitrum:** 1 blok (fast-finality L2).
  * **Ethereum L1:** 12 blok (~2.5 menit untuk PoS Casper finality).

---

## 7. Endpoint Status Transaksi Publik

Klien SDK, antarmuka pengguna, dan merchant dapat memantau status penyelesaian transaksi secara real-time:

```http
GET /api/tx-status?tx_hash=0x1234...
```

**Respons JSON:**
```json
{
  "tx_hash": "0x1234...",
  "status": "confirmed",
  "block_number": 21854930,
  "gas_used": 142500,
  "effective_gas_price": 20000000,
  "confirmations": 4,
  "error_reason": null
}
```

*Status yang didukung:* `pending` (termasuk broadcasting & replacement), `confirmed`, `failed`, dan `not_found`. Jika transaksi telah masuk blok, counter `confirmations` dihitung secara dinamis terhadap tinggi blok blockchain saat ini.

---

## 8. On-Chain Deposit Indexer & Cryptographic Reveal (DEC-018)

Mengacu pada spesifikasi [`DEC-018`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-018-on-chain-deposit-indexer-and-cryptographic-reveal-verification.md), relayer menghapus ketergantungan pencatatan deposit berbasis client API murni dan menerapkan arsitektur *on-chain event listener* yang kebal terhadap reorg dan phantom deposits:

1. **Smart Contract Event Binding (`DepositFee`):**
   * Smart contract Stylus memancarkan event `DepositFee(session_id, com_k_hash, client, gross, fee, net)`.
   * Komitmen masking key di-hash dengan `keccak256(com_k)` dan ditempatkan sebagai topic indexed ke-2 untuk memungkinkan relayer memvalidasi keaslian komitmen langsung dari log receipt EVM.
2. **Background Indexer Worker (`deposit_indexer_worker`):**
   * Polling berkala terhadap blok on-chain dengan batasan `safe_block = current_block.saturating_sub(confirmation_threshold - 1)`.
   * Menolak memproses blok di ujung rantai (*tip*) untuk mencegah pemalsuan deposit akibat chain reorg.
   * Checkpoint nomor blok terakhir yang berhasil diproses dicatat durable pada tabel `indexer_state`.
3. **Fail-Closed Cryptographic Verification pada Reveal (`/api/reveal`):**
   * Kunci masking $k$ hanya dirilis jika dan hanya jika:
     1. Status sesi deposit sudah berstatus confirmed di on-chain (`deposit_confirmed == true`).
     2. Verifikasi kriptografis kurva eliptis BLS12-381 $\mathbb{G}_2$ valid:
        $$k \cdot \text{pk}_{\text{iss}} == \text{com}_k$$
   * Jika $k$ atau $\text{com}_k$ rusak, dimanipulasi, atau tidak cocok, endpoint seketika mengembalikan respons `REJECTED`, sesi tetap terkunci (`resolved == false`), dan tidak ada rahasia yang bocor ke publik.

---

## 9. Input Sanitization & Post-Restart Nullifier Reconciliation (DEC-019)

Mengacu pada spesifikasi [`DEC-019`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-019-relayer-input-sanitization-safety-limits-and-post-restart-reconciliation.md):

1. **Ingress Input & Financial Safety Bounds (`validation.rs`):**
   * **Recipient Address:** Wajib format alamat EVM 20-byte valid dan secara tegas menolak zero address (`0x000...000`).
   * **Spend Amount Bounds:** Menolak transaksi debu spam di bawah minimum on-chain 5 USDC (`5_000_000` base units) dan transaksi di atas batas circuit breaker 50,000 USDC (`50_000_000_000` base units).
   * **CCIP Destination Allowlist:** Memvalidasi `destination_chain_selector` terhadap rantai terotorisasi (Arbitrum Sepolia, Ethereum Sepolia, Base Sepolia, Optimism Sepolia) dan memverifikasi `destination_contract` non-zero.
2. **On-Chain View Function `isNullifierSpent(bytes32)`:**
   * Smart contract Stylus mengekspos public view function `is_nullifier_spent(nullifier)` untuk inspeksi status langsung via RPC.
3. **Crash Recovery & Post-Restart Reconciliation:**
   * Saat node menyala kembali pasca restart atau crash, relayer memeriksa seluruh transaksi berstatus `queued`, `retryable`, `broadcasting`, atau `submitted` terhadap kontrak on-chain.
   * Jika nullifier terbukti sudah berstatus *spent* on-chain (misal berhasil ditambang sesaat sebelum crash), relayer menandai antrian lokal sebagai `confirmed` dan mencatat nullifier ke tabel lokal tanpa mengirim transaksi ganda ke mempool.
   * Jika transaksi belum masuk on-chain dan lease kedaluwarsa, relayer mereset status menjadi `queued` untuk diproses worker secara aman.

---

## 10. Batch Profitability & Operational Fee Metrics (DEC-020)

Mengacu pada spesifikasi [`DEC-020`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-020-relayer-batch-profitability-and-operational-fee-metrics.md):

1. **Database Aggregates (`get_batch_metrics`):**
   * Menghitung total kumpulan batch (`batch_count`), profit margin bersih akumulatif (`total_batch_margin_usdc`), serta ukuran rata-rata batch (`avg_batch_size`) langsung dari tabel `spend_batches`.
   * Menelusuri total fee eksekusi yang dihasilkan (`total_execution_fees_usdc`), fee yang sudah diklaim on-chain (`claimed_execution_fees_usdc`), dan piutang relayer yang belum ditarik (`unclaimed_execution_fees_usdc`).
2. **Eksposur Telemetri & Monitoring (`GET /api/health`):**
   * Objek `batch_metrics` disertakan pada respons endpoint kesehatan untuk monitoring otomatis (Prometheus / Grafana scraper) guna mendeteksi anomali gas margin dan efisiensi throughput batching secara real-time.




