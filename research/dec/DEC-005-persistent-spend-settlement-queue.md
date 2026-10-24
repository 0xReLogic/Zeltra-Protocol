# DEC-005: Persistent Spend Settlement Queue

## Status

Accepted for implementation on 2026-06-07.

## Masalah

Relayer memakai `Vec<SpendRequest>` di memory walaupun tabel `spend_queue`
sudah tersedia. Request hilang saat restart, seluruh batch dihapus setelah satu
putaran, nullifier lokal dicatat sebelum broadcast berhasil, dan receipt hanya
dicetak oleh task yang tidak memperbarui database.

## Invariant Bisnis dan Security

- Request yang sudah diterima tidak hilang karena restart atau RPC failure.
- Satu nullifier hanya memiliki satu settlement aktif.
- Broadcast atau tx hash bukan settlement sukses.
- Nullifier lokal menjadi `confirmed` hanya setelah receipt sukses.
- Receipt revert tidak boleh ditandai sukses dan harus menghasilkan status
  terminal yang dapat diaudit.
- Dua worker tidak boleh memiliki lease item yang sama pada saat bersamaan.

## Pilihan yang Dipertimbangkan

1. Mempertahankan queue memory dan membuat snapshot berkala.
2. Memakai SQLite sebagai source of truth dengan status dan lease.
3. Menambah Redis/SQS/Temporal sebelum MVP.

## Keputusan

Gunakan SQLite sebagai source of truth untuk relayer single-node. Request
disimpan sebagai JSON canonical bersama field lifecycle:

```text
queued -> broadcasting -> submitted -> confirmed
                         \-> queued (retryable pre-submission failure)
                         \-> failed (receipt revert atau retry exhausted)
```

Worker mengambil item memakai transaksi `IMMEDIATE` dan lease. Item tidak
dihapus setelah diproses. Tx hash, block, retry count, error terakhir, dan waktu
perubahan disimpan. Nullifier pada tabel `nullifiers` hanya ditulis dalam
transaksi yang sama dengan perubahan queue menjadi `confirmed`.

## Alasan

SQLite sudah digunakan repo dan menyediakan transaksi atomic serta crash
recovery. Menambah distributed queue sekarang memperbesar operational surface
tanpa menyelesaikan contract/accounting invariant. Status receipt tetap harus
berasal dari chain, bukan log atau keberhasilan RPC submission.

## Sumber Primer

- SQLite, "Atomic Commit In SQLite":
  https://www.sqlite.org/atomiccommit.html
- SQLite, "Write-Ahead Logging":
  https://sqlite.org/wal.html
- EIP-1474, `eth_getTransactionReceipt`:
  https://eips.ethereum.org/EIPS/eip-1474
- Alloy provider documentation, transaction receipt API:
  https://github.com/alloy-rs/alloy

Tanggal akses: 2026-06-07.

## Sumber Pembanding

- OpenZeppelin Relayer, queue backend dan transaction status checker:
  https://github.com/OpenZeppelin/openzeppelin-relayer
- ChainSafe Canton Middleware, persist-before-submit dan reconciliation loop:
  https://github.com/ChainSafe/canton-middleware/blob/mainnet-deploy/docs/relayer-logic.md

## Known Risks

- Crash setelah RPC menerima transaksi tetapi sebelum tx hash tersimpan dapat
  meninggalkan item `broadcasting`. Lease recovery boleh mencoba kembali;
  nullifier contract mencegah payout kedua, tetapi retry dapat membayar gas
  untuk transaksi yang revert.
- Exactly-once broadcast membutuhkan persistent nonce allocation, signed raw
  transaction storage, replacement policy, dan reconciliation by sender+nonce.
  Itu tetap pekerjaan lanjutan sebelum mainnet.
- SQLite single-writer sesuai untuk satu relayer instance, bukan cluster
  distributed ber-throughput tinggi.
- Queue payload lama yang tidak memiliki `request_json` tidak dapat diproses
  otomatis dan ditandai gagal saat migrasi.

## Positive Test

- Enqueue bertahan setelah database dibuka ulang.
- Claim mengubah item menjadi `broadcasting` dan membuat lease.
- Receipt sukses menyimpan tx hash/block serta nullifier confirmed.
- Lease kedaluwarsa membuat item dapat dipulihkan.

## Negative Test

- Nullifier aktif yang sama tidak dapat dienqueue dua kali.
- Dua claim berurutan tidak memperoleh item yang sama.
- Broadcast error mengembalikan item ke queue dengan retry/backoff.
- Receipt revert menghasilkan `failed`, bukan `confirmed`.

## Rollback dan Recovery

Schema migration hanya menambah kolom. Binary lama tetap dapat membaca kolom
lama, tetapi tidak memahami lifecycle baru dan tidak boleh dijalankan setelah
migration. Rollback dilakukan dengan menghentikan worker, menyalin database,
dan memakai binary baru untuk mengekspor item non-terminal sebelum downgrade.
