# DEC-006: Adaptive Private Spend Batching

## Status

Accepted for implementation on 2026-06-07.

## Masalah

Persistent queue saat ini mengambil banyak request sekaligus, tetapi tetap
mengirim satu transaksi EVM untuk setiap spend. Tidak ada penghematan base
transaction overhead dan istilah "batch" pada worker menyesatkan.

Relayer juga memerlukan model biaya yang tetap menutup transaksi ketika hanya
ada satu request, tetapi dapat mengambil margin efisiensi saat beberapa request
berhasil digabungkan.

## Keputusan

- Tambahkan entrypoint kontrak `batch_spend` untuk maksimal 8 same-chain spend.
- Node mengaktifkan batch secara opt-in setelah kontrak baru dideploy; default
  tetap jalur single untuk kompatibilitas kontrak lama.
- Batch bersifat atomic: satu item gagal membuat seluruh transaksi revert.
- Worker menggunakan adaptive micro-batching:
  - batch dikirim segera ketika mencapai 8 item;
  - window normal maksimal 1 detik;
  - hard timeout settlement queue tetap 2 detik;
  - satu item dikirim melalui jalur single;
  - cross-chain dan deadline dekat tidak menunggu batch.
- User menyetujui **fixed execution quote**, bukan klaim actual gas.
- Efisiensi antara total execution quote dan biaya batch aktual menjadi margin
  relayer/protokol sesuai policy.
- Protocol fee 0,15% dan nilai yang diterima merchant tidak berubah karena
  batching.

## Invariant

- Nullifier dan signature setiap item diverifikasi secara independen.
- Tidak ada partial success dalam satu batch.
- Satu tx hash dan receipt berlaku untuk seluruh item batch.
- Item baru ditandai `confirmed` setelah receipt batch sukses.
- Batch size dibatasi untuk mencegah calldata dan gas exhaustion.
- Quote harus memiliki nilai maksimum dan expiry sebelum billing diaktifkan.

## Transparansi Biaya

UI dan SDK harus menyebut biaya sebagai `execution fee` atau `network execution
quote`. Istilah `actual gas reimbursement` tidak boleh digunakan jika user
ditagih berdasarkan fixed quote.

## Rollback

Node dapat menonaktifkan batch melalui konfigurasi dan kembali memanggil
`spend()` per item. Entrypoint `batch_spend` tidak mengubah storage layout.
