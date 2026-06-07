# DEC-004: Atomic Masking Key Release

Date: 2026-06-07

## Masalah

Sebelumnya, endpoint `/api/leader/sign` langsung mengembalikan masking key `k_hex` bersama `com_k` dan partial signatures. Hal ini memungkinkan client untuk melakukan unmask signature secara langsung tanpa melakukan deposit atau menunggu on-chain settlement terkonfirmasi. Hal ini merusak *fair exchange* (pertukaran yang adil) yang dijamin oleh alur protokol.

## Invariant bisnis/security

- Masking key `k` tidak boleh dibocorkan ke client sebelum deposit terkonfirmasi di blockchain.
- Rilis masking key `k` harus terikat secara ketat dengan `session_id`, `amount`, dan `com_k` dari deposit.
- Sesi yang gagal didepositkan atau kedaluwarsa (expired) tidak boleh membocorkan `k`.
- Akses `/api/reveal` harus aman, idempotent, dan hanya merilis `k` jika status deposit di database telah divalidasi dan ditandai terkonfirmasi (`deposit_confirmed = 1`).

## Keputusan

1. **Omit `k` dari Signing Response**:
   Endpoint `/api/leader/sign` direfaktorisasi untuk menghasilkan `k` secara random di RAM, berkoordinasi dengan guardian, menyimpannya di SQLite database persisten relayer, dan hanya mengembalikan `com_k_hex` dan `partial_signatures`. Kunci `k` tidak lagi terekspos pada respons awal.

2. **Atomic State Transition**:
   - **Tahap 1: Sign Request**: Client memanggil `/api/leader/sign`. Relayer mencatat session di database dengan status `deposit_confirmed = 0`, `resolved = 0`, dan menyimpan `k_hex`.
   - **Tahap 2: Deposit Confirmation**: Client melakukan deposit on-chain. Endpoint `/api/deposit` memverifikasi kecocokan `session_id`, `amount`, dan `com_k`. Jika cocok, status diubah menjadi `deposit_confirmed = 1`.
   - **Tahap 3: Reveal Request**: Client memanggil `/api/reveal` dengan mengirimkan `session_id`. Relayer memeriksa apakah `deposit_confirmed = 1` dan `resolved = 0`. Jika ya, relayer mengubah `resolved = 1` secara atomik dan merilis `k_hex` kepada client.

3. **Keamanan Storage**:
   Kunci masking `k` disimpan dalam SQLite database yang dilindungi oleh enkripsi SQLCipher (`NIMBUS_DB_KEY`) dan dilindungi dengan izin file 600 di tingkat sistem operasi.

## Sumber primer

- Nimbus Protocol TODO: `Terapkan Atomic Release untuk Masking Key k`
- DEC-001: Deposit-Reveal Collateral Lifecycle

## Versi

- `nimbus-node = "0.1.0"`

## Test

Unit test `test_atomic_release_session_lifecycle` ditambahkan untuk memverifikasi:
- Masking key `k` tidak dapat dibaca oleh reveal sebelum deposit terkonfirmasi.
- Pencocokan parameter deposit yang ketat (amount salah atau com_k salah akan menolak konfirmasi deposit).
- Setelah deposit terkonfirmasi, reveal pertama mengembalikan `k` dengan benar dan menandai session sebagai resolved.
- Pemanggilan reveal berikutnya ditolak untuk mencegah kebocoran ganda (idempotent rejection).
