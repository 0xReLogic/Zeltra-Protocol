# DEC-015: CCIP Cross-Chain Message Security Validations

Date: 2026-06-12

## Masalah

Saat menerima pesan lintas rantai melalui Chainlink CCIP, kontrak pintar Nimbus di destination chain menerima data tanpa validasi yang memadai:
1. Alamat CCIP Router tidak diwajibkan (jika bernilai `Address::ZERO`, siapa pun bisa memicu `_ccip_receive`).
2. Tidak ada verifikasi chain asal (`source_chain_selector`) maupun alamat kontrak pengirim asli (`_sender`). Ini membuka celah spoofing di mana siapa pun bisa mengirim pesan palsu dari chain lain.
3. Tidak ada proteksi replay terhadap `message_id`, sehingga pesan CCIP yang sama dapat dieksekusi berkali-kali (double payout).

## Invariant bisnis/security

- **Mandatory Router Verification**: Seluruh eksekusi pesan lintas rantai wajib melalui CCIP Router yang terdaftar (`ccip_router != Address::ZERO`).
- **Sender & Chain Authorization**: Pesan hanya diterima dari kontrak relayer yang sah pada chain asal yang sah (Allowlist terpadu).
- **Message Replay Protection**: CCIP `messageId` harus diverifikasi unik dan disimpan sebagai pencegahan eksekusi ganda (replay protection).
- **Caller Restriction**: Fungsi `_ccip_receive` hanya dapat dipanggil oleh CCIP Router yang terdaftar.

## Pilihan yang dipertimbangkan

1. **Pemeriksaan Minimal (Current)**:
   Hanya mengecek `caller == ccip_router` jika router tidak bernilai nol.
   - *Kelebihan*: Sederhana.
   - *Kekurangan*: Sangat tidak aman, rentan spoofing dan replay.
2. **Pemeriksaan Keamanan Lengkap (Keputusan)**:
   Menambahkan penyimpanan state allowlist sender terpadu (kunci gabungan dari `source_chain_selector` dan `sender`) serta tracking `messageId` yang sudah diproses.
   - *Kelebihan*: Mencegah spoofing dan replay secara absolut.
   - *Kekurangan*: Sedikit peningkatan konsumsi gas saat pemrosesan pesan pertama kali.

## Keputusan

Mengadopsi **Pilihan 2 (Pemeriksaan Keamanan Lengkap)**.
Menambahkan dua bidang state baru di akhir `sol_storage!`:
- `ccip_processed_messages`: `mapping(bytes32 => bool)`
- `ccip_allowed_senders`: `mapping(bytes32 => bool)` (kunci berupa `keccak256(source_chain_selector || sender)`).

Admin pemilik kontrak dapat memicu `setCcipSenderAllowlist` untuk mendaftarkan relayer yang sah pada chain sumber tertentu.

## Alasan

Keamanan lintas rantai membutuhkan kepastian asal-usul pesan (*provenance*) dan jaminan keunikan eksekusi (*idempotency*). Tanpa filter chain asal dan sender allowlist, penyerang dapat meniru (*spoof*) pesan deposit/spend dari chain eksperimental atau chain murah lainnya untuk mencuri dana dari escrow di L2 tujuan.

## Sumber primer

- **Chainlink CCIP Developer Documentation**: https://docs.chain.link/ccip (Panduan praktik terbaik penanganan `ccipReceive`).
- **Chainlink CCIP Security Standards**: Panduan pencegahan replay attack dan spoofing.

## Known risks

- **Admin Key Dependency**: Kesalahan input allowlist oleh admin dapat memblokir aliran lintas rantai.
- **WASM Size Limits**: Penambahan state dan logika baru meningkatkan ukuran bytecode Stylus.
  *Mitigasi*: Logika enkapsulasi dibuat sangat ramping.

## Versi/library/network

- Arbitrum Sepolia (`chain_id = 421614`) & CCIP Router `0x2a9C5afB0d0e4BAb2BCdaE109EC4b0c4Be15a165`.
- Stylus SDK `0.10.7`
- alloy-primitives `1.6.0`

## Rencana positive test

- **PT-01**: Pesan lintas rantai dari sender dan chain yang terdaftar di allowlist berhasil dieksekusi.
- **PT-02**: Message ID berhasil disimpan ke `ccip_processed_messages` setelah transaksi sukses.

## Rencana negative test

- **NT-01**: Eksekusi dibatalkan jika `ccip_router` belum dikonfigurasi (`ccip_router == Address::ZERO`).
- **NT-02**: Eksekusi dibatalkan jika pemanggil bukan router yang sah (`ONLY_CCIP_ROUTER_ALLOWED`).
- **NT-03**: Eksekusi dibatalkan jika chain asal atau sender belum di-allowlist (`CCIP_SENDER_NOT_ALLOWED`).
- **NT-04**: Eksekusi dibatalkan jika message ID yang sama dikirimkan kembali (`CCIP_MESSAGE_ALREADY_PROCESSED`).

## Rollback/recovery

Jika terjadi kendala integrasi, admin pemilik kontrak dapat mencabut allowlist pengirim atau menyetel ulang `ccip_router` ke alamat baru.
