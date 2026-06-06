# DEC-002: BLS Spend Verification Boundaries

Date: 2026-06-06

## Masalah

Jalur `spend` menerima komponen BLS tetapi tidak memanggil pairing precompile.
Selain itu, caller dapat memilih issuer public key dan nullifier secara bebas.
Mengaktifkan pairing saja tidak cukup: attacker dapat menandatangani dengan key
sendiri atau memakai satu credential berulang kali dengan nullifier berbeda.

## Invariant bisnis/security

- Hanya issuer key yang disetujui governance dapat mengotorisasi spend.
- Signature harus lolos pairing EIP-2537 sebelum state berubah.
- Point infinity dan encoding dengan panjang salah harus ditolak.
- Satu `H(m)` hanya mempunyai satu nullifier.
- Signature gagal tidak boleh mengubah nullifier atau principal.
- Spend tidak boleh melebihi outstanding principal.

## Keputusan

1. Owner mendaftarkan hash exact EIP-2537 encoding dari issuer public key.
2. Spend menolak key yang tidak terdaftar.
3. Nullifier wajib sama dengan `keccak256(hm_bytes)`.
4. Contract memverifikasi:
   `e(-alpha, G2_generator) * e(H(m), pk_iss) == 1`.
5. Principal subtraction menggunakan checked arithmetic dan gagal jika kurang.
6. SDK menghasilkan nullifier dengan aturan yang sama.

## Batas keamanan yang belum selesai

`H(m)` belum dapat direkonstruksi contract dari parameter transaksi. Akibatnya,
signature belum membuktikan bahwa issuer menyetujui `amount`, `recipient`,
`chain_id`, contract address, expiry, atau action. Pairing dan replay protection
sudah aktif, tetapi mainnet tetap diblokir sampai canonical spend message dan
domain separation diimplementasikan end-to-end.

Canonical message yang harus diteliti:

```text
NIMBUS_SPEND_V1 ||
chain_id ||
contract_address ||
action ||
amount ||
recipient_or_intent_hash ||
expiry ||
credential_nonce
```

Contract harus dapat membuktikan bahwa `H(m)` berasal dari message tersebut;
tidak cukup menerima `hm_bytes` dari caller.

## Sumber primer

- EIP-2537 pairing ABI, encoding, subgroup checks, and error behavior:
  https://eips.ethereum.org/EIPS/eip-2537
- Ethereum execution-spec tests for EIP-2537:
  https://github.com/ethereum/execution-spec-tests/tree/master/tests/prague/eip2537_bls_12_381_precompiles
- RFC 9380 hash-to-curve:
  https://www.rfc-editor.org/rfc/rfc9380.html

Accessed: 2026-06-06.

## Versi

- `stylus-sdk = 0.6.x`
- `ark-bls12-381 = 0.5.x` in `nimbus-core`
- Target pairing precompile: `0x0f`

## Test

Host regression tests cover malformed length, infinity, untrusted key, false
pairing output, nullifier mismatch, replay, and insufficient principal. The host
still mocks precompile execution. A Stylus testnet test with official valid and
invalid EIP-2537 vectors remains mandatory.

## Rollback/recovery

The trusted-key mapping is append-only storage. A fresh deployment must register
the active issuer key before spend is enabled. Revocation immediately blocks new
spends using that key; operational key rotation and grace-period behavior remain
to be specified.
