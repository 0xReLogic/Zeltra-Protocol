# DEC-013: Refund and Timeout Verification

Date: 2026-06-11

## Masalah

Untuk menjaga insolvensi dan memastikan keamanan dana user, contract harus membatasi claim refund agar tidak terjadi double-spending (refund dan spend secara bersamaan) serta memastikan refund mematuhi aturan timelock. 
Secara spesifik:
1. Sesi deposit yang belum di-reveal tidak boleh di-refund sebelum timelock 24 jam terlewati.
2. Hanya account yang melakukan deposit (`session_client`) yang berhak memicu refund.
3. Sesi yang sudah di-reveal (resolved) tidak boleh di-refund.
4. Sesi yang sudah di-refund tidak boleh di-reveal atau di-spend.
5. Refund ganda (double-refund) harus diblokir.

## Invariant bisnis/security

- Dana yang di-refund harus kembali ke `session_client` asli.
- Saldo totalDepositedPrincipal harus berkurang tepat sebesar `net_amount` yang di-refund.
- Refund hanya bisa dilakukan 1 kali per session ID (`sid`).
- Setelah refund berhasil, status sesi berubah menjadi `resolved=true` untuk mencegah reveal/spend/refund berikutnya.

## Keputusan

1. Implementasikan pengujian negatif dan positif secara on-chain (`scripts/ht05_refund_tests.py`) di Arbitrum Sepolia.
2. Gunakan status `session_resolved` untuk mencegah aksi berulang setelah refund.
3. Gunakan checked arithmetic saat mengurangi totalDepositedPrincipal.

## Test

### Negative Tests (NT):
- **NT-01**: Refund sebelum timelock 24 jam berakhir (harus revert `TIMELOCK_NOT_EXPIRED`).
- **NT-02**: Refund session ID yang tidak terdaftar (harus revert `NO_DEPOSIT_FOUND`).
- **NT-03**: Refund dipanggil oleh non-client (harus revert `NOT_SESSION_CLIENT`).
- **NT-04**: Refund untuk sesi yang sudah di-reveal/resolved (harus revert `SESSION_ALREADY_RESOLVED`).

### Positive Tests (PT):
- **PT-01**: Refund berhasil setelah 24 jam timelock terlewati.
- **PT-02**: Cek totalDepositedPrincipal berkurang tepat sebesar net_amount.
- **PT-03**: Cek status sesi berubah menjadi `resolved=true`.
- **PT-04**: Panggil refund kedua kali setelah sukses (harus revert `SESSION_ALREADY_RESOLVED`).
- **PT-05**: Panggil reveal setelah refund sukses (harus revert `SESSION_ALREADY_RESOLVED`).

## Rollback/recovery

State diupdate secara transaksional di EVM. Jika refund transfer gagal, transaksi revert seluruhnya tanpa merubah state escrow.
