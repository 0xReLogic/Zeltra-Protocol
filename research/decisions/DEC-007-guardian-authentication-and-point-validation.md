# DEC-007: Guardian Authentication and Point Validation

Date: 2026-06-07

## Masalah

Sebelumnya, terdapat celah di mana secret share atau parameter arbitrary blind signature dapat dilewati ke guardian tanpa autentikasi memadai, yang memungkinkan penyerang eksternal untuk melompati proteksi KeyManager atau meminta guardian menandatangani blinded point arbitrary secara permissionless. Selain itu, karena penandatanganan terjadi *sebelum* deposit berada on-chain, guardian tidak dapat melakukan validasi keberadaan sesi melalui kueri L2. Hal ini menciptakan risiko replay/extraction attack jika leader yang tidak sah dapat mengirimkan request penandatanganan ke guardian.

## Invariant bisnis/security

- Guardian hanya boleh memproses permintaan tanda tangan dari Leader yang terautentikasi dan terdaftar di dalam allowlist (`NIMBUS_TRUSTED_LEADERS`).
- Permintaan tanda tangan harus terikat dengan parameter sesi yang sah (`session_id`, `blinded_hex`, `k_hex`, `timestamp`, `amount`, `client_address`, `com_k_hex`) untuk mencegah front-running dan perubahan parameter secara jahat.
- Replay protection: Setiap session_id hanya boleh digunakan sekali untuk penandatanganan di setiap guardian untuk mencegah eksploitasi berulang (double signing).
- Cryptographic consistency: Guardian harus memverifikasi bahwa commitment `com_k` yang dikirimkan oleh leader benar-benar cocok dengan hasil perkalian `pk_iss * k` dari masking key `k` yang dikirimkan.
- Pembatasan waktu: Request penandatanganan memiliki expiration time yang ketat (tolerance ±60 detik) untuk mencegah penyerangan ulang request yang lama.

## Keputusan

1. **Application-Layer Leader Authentication**:
   - Leader harus menandatangani payload parameter sesi menggunakan private key EVM miliknya (`sign_leader_payload`).
   - Guardian memverifikasi tanda tangan ini menggunakan algoritma secp256k1 (Alloy `Signature::recover_address_from_msg`).
   - Guardian memastikan bahwa address leader yang terpulih cocok dengan parameter `leader_address` dan terdaftar di dalam environment variable `NIMBUS_TRUSTED_LEADERS` (diperbolehkan default hanya di lingkungan test/dev jika env tersebut tidak diset).

2. **Replay & Double-Sign Protection**:
   - Guardian mencatat setiap sesi penandatanganan di database lokalnya menggunakan `insert_signing_session`.
   - Jika `session_id` yang sama dikirimkan lagi, database akan menolaknya (karena UNIQUE constraint pada primary key `session_id` di SQLite), dan guardian akan menolak permintaan penandatanganan tersebut dengan pesan error duplicate session.

3. **Cryptographic Validation**:
   - Guardian merekonstruksi commitment dengan mengalikan issuer public key (`pk_iss`) dengan masking key `k` yang dikirimkan oleh leader.
   - Guardian mencocokkan hasil rekonstruksi ini dengan `com_k_hex` yang dikirimkan oleh leader. Permintaan akan ditolak jika ada ketidakcocokan, mencegah manipulasi commitment.

4. **Timestamp Verification**:
   - Guardian memverifikasi bahwa `timestamp` yang ditandatangani oleh leader berada dalam rentang toleransi ±60 detik terhadap waktu lokal guardian saat ini (dinonaktifkan jika dalam mode pengujian `cfg!(test)` atau `NIMBUS_ENV=test`).

## Sumber primer

- Nimbus Protocol TODO: `Hilangkan Secret Share Override dari API`
- `nimbus-node/src/handlers/threshold.rs`
- `nimbus-node/src/dto.rs`
- `nimbus-node/src/evm_client.rs`

## Versi

- `nimbus-node = "0.1.0"`

## Test

Unit test `test_secure_sign_share` ditambahkan untuk memverifikasi:
- Tanda tangan leader yang sah diterima oleh guardian dengan sukses.
- Upaya double signing dengan session ID yang sama ditolak (replay protection).
- Modifikasi commitment (`com_k` tidak cocok dengan `pk_iss * k`) dideteksi dan ditolak oleh guardian.
- Tanda tangan yang tidak sah atau rusak ditolak dengan benar.
