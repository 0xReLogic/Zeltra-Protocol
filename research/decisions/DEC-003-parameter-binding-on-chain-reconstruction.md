# DEC-003: Parameter Binding and On-Chain Hash Reconstruction

Date: 2026-06-07

## Masalah

DEC-002 mengidentifikasi celah keamanan kritis di mana `hm_bytes` ($H(m)$) dikirim langsung oleh pemanggil tanpa pembuktian. Hal ini memungkinkan Relayer jahat untuk mengambil signature yang sah dan menggunakannya kembali dengan parameter transaksi yang berbeda (misalnya, mengubah `recipient` atau `amount`).

Jalur verification harus membuktikan secara kriptografis bahwa signature yang ditandatangani oleh Issuer mengikat detail transaksi spesifik tersebut secara permanen.

## Invariant bisnis/security

- Signature hanya berlaku untuk kontrak Nimbus spesifik di rantai (chain) tujuan tertentu (Domain Separation).
- Signature hanya berlaku untuk `amount`, `recipient`, `expiry`, dan `nonce` yang disetujui.
- Tidak boleh ada bypass verifikasi dengan mengirimkan data `hm_bytes` secara langsung.
- Perubahan apa pun pada parameter spend wajib menggagalkan proses verifikasi BLS.

## Keputusan

1. **Format Canonical Message (`NIMBUS_SPEND_V1`)**:
   Pesan spend dikonstruksi secara byte-packed:
   ```text
   "SPEND" (5 bytes) ||
   chain_id (32 bytes BE) ||
   contract_address (20 bytes) ||
   amount (32 bytes BE) ||
   recipient_or_intent_hash (32 bytes) ||
   expiry (32 bytes BE) ||
   nonce (32 bytes)
   ```
   
2. **On-Chain Hash Reconstruction**:
   Kontrak tidak lagi menerima parameter `hm_bytes` dari pemanggil API. Sebaliknya:
   - Kontrak mengambil parameter spend (`recipient`, `amount`, `recipient_or_intent_hash`, `expiry`, `nonce`).
   - Kontrak menghitung Keccak256 hash dari format pesan canonical di atas.
   - Hasil hash dipetakan ke titik kurva G1 ($H(m)$) menggunakan algoritma **RFC 9380 WBMap/SSWU** dengan domain separation tag:
     `BLS_SIG_BLS12381G1_XMD:SHA-256_SSWU_RO_NUL_`
   - Kontrak memanggil precompile EIP-2537 `BLS12_PAIRING_CHECK` (`0x0f`) untuk memvalidasi signature.

3. **CCIP Payload Adjustment**:
   Payload Chainlink CCIP direduksi dari 648 byte menjadi **584 byte** dengan membuang parameter `hm_bytes` (128 byte) dan menambahkan parameter domain & transaction protection (`expiry` 32 byte, `nonce` 32 byte).

4. **SDK & Node Broadcaster Alignment**:
   - `nimbus-sdk` menghasilkan `PAYMENT-SIGNATURE` yang memuat parameter `recipient_or_intent_hash_hex`, `expiry`, dan `nonce_hex` secara opsional agar kompatibel dengan flow refilling non-interaktif.
   - `nimbus-node` membaca parameter tersebut dan meneruskannya secara transparan ke pemanggilan on-chain.

## Sumber primer

- RFC 9380 (Hash-to-Curve): https://www.rfc-editor.org/rfc/rfc9380.html
- EIP-2537 Precompiles: https://eips.ethereum.org/EIPS/eip-2537

## Versi

- `sha2 = { version = "0.10.8", default-features = false }` (no_std)
- `nimbus_contracts = "0.1.0"`
- `nimbus-sdk = "0.1.0"`
- `nimbus-node = "0.1.0"`

## Test

Semua test suite (21 test kontrak dan 28 test core/SDK/node) memverifikasi invariant:
- Perubahan parameter (`amount`, `nonce`, `expiry`, `recipient`) dengan signature lama akan menghasilkan kegagalan pairing atau `NULLIFIER_MESSAGE_MISMATCH`.
- Replay attack pada nullifier yang sama langsung ditolak.
