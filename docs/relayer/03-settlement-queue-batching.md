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
