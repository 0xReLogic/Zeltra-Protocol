# DEC-008: Threshold Ceremony Public Key

Date: 2026-06-07

## Masalah

Secret share Shamir milik satu guardian bukan issuer secret key. Karena itu,
public key yang diturunkan dari share lokal juga bukan issuer public key
gabungan. Jika setiap node memakai public key lokal tersebut, commitment
`com_k = k * pk_iss` berbeda antar-node dan distributed signing gagal.

## Keputusan

- Leader dan seluruh guardian wajib memakai `NIMBUS_ISSUER_PUBLIC_KEY` yang sama,
  yaitu canonical compressed `IssuerPublicKey` hasil ceremony threshold.
- `KeyManager` menyimpan ceremony public key secara terpisah dari secret share.
- Public key tidak berubah saat share di-remask atau di-reload.
- Fallback derivasi dari local signing key hanya diizinkan untuk test atau
  konfigurasi eksplisit `NIMBUS_THRESHOLD=1`.
- Nilai `pk_iss_hex` dari request client bersifat assertion kompatibilitas.
  Nilai yang berbeda atau invalid ditolak dan tidak pernah mengganti key node.
- Ceremony juga menghasilkan public share untuk setiap indeks guardian. Leader
  memuatnya dari `NIMBUS_GUARDIAN_PUBLIC_KEYS`, bukan dari response guardian.
- Partial signature hanya dihitung ke quorum jika pairing berikut valid:
  `e(partial_signature, G2) == e(blinded_message, k * public_share)`.
- Indeks yang tidak ada di registry, encoding signature rusak, atau pairing
  yang gagal ditolak sebelum agregasi.

## Batas

Registry masih berupa konfigurasi startup dan belum memiliki manifest ceremony
bertanda tangan atau key version. Pencegahan mixed-key saat rotasi tetap menjadi
blocker berikutnya.

## Test

- Node dengan share berbeda mengembalikan ceremony public key yang identik.
- Request tanpa `pk_iss_hex` memakai konfigurasi node.
- Request dengan issuer public key berbeda atau encoding invalid ditolak.
- Public share yang tidak terdaftar dan partial signature corrupt ditolak.
- Partial signature valid terhadap public share yang dipin diterima.
