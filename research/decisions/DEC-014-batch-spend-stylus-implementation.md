# DEC-014: Gas Optimization via BLS12-381 Batch Signature Verification

Date: 2026-06-12

## Masalah

Saat melakukan eksekusi spend dalam jumlah banyak (batch), memanggil fungsi `_spend` secara individual di dalam loop akan memicu pemanggilan precompile pairing check EIP-2537 (`BLS12_PAIRING_CHECK` pada alamat `0x0f`) sebanyak $N$ kali. Setiap pemanggilan precompile memiliki flat base gas cost overhead yang signifikan, sehingga pemrosesan loop individual ini tidak efisien dan membuang banyak gas.

## Invariant bisnis/security

- **Double-Spend Prevention**: Nullifier untuk setiap spend item harus unik dan belum pernah digunakan.
- **Signature Validity**: Setiap transaksi spend harus memiliki signature BLS12-381 yang valid dari issuer tepercaya.
- **Asset Conservation**: Total didebit tidak boleh melebihi saldo deposit principal (`total_deposited_principal`).
- **Atomicity**: Transaksi batch harus sepenuhnya sukses atau sepenuhnya gagal (revert) secara keseluruhan. Tidak boleh ada partial execution/payout.

## Pilihan yang dipertimbangkan

1. **Jalur Individual Loop (Current)**:
   Melakukan iterasi di smart contract dan memanggil precompile `0x0f` satu per satu untuk setiap item spend.
   - *Kelebihan*: Logika sederhana.
   - *Kekurangan*: Sangat boros gas karena membayar base call cost precompile $N$ kali.
2. **Consolidated Batch Pairing Call (Keputusan)**:
   Membangun satu payload byte gabungan dari $2N$ pairing points (untuk $N$ signature, masing-masing terdiri dari 2 pairing pairs: $(-\alpha_i, G_2)$ dan $(H(m_i), pk_{iss\_i})$) lalu memanggil precompile `0x0f` sekali dengan input data berukuran $768 \times N$ bytes.
   - *Kelebihan*: Menghemat gas overhead secara signifikan dengan menggabungkan semua pairing check ke dalam satu pemanggilan precompile.
   - *Kekurangan*: Kompleksitas encoding data bertambah di sisi contract.

## Keputusan

Mengadopsi **Pilihan 2 (Consolidated Batch Pairing Call)**. 
Jalur fungsi `batch_spend` akan mengumpulkan seluruh data signature dan kunci publik, merekonstruksi hash pesan $H(m_i)$ secara on-chain untuk masing-masing item, melakukan sanity check panjang input, lalu menggabungkan semua poin ke dalam satu buffer memory `input_bytes` berukuran $768 \times N$ bytes untuk dipasok ke precompile `0x0f` dalam satu static call tunggal.

## Alasan

Formula kalkulasi gas precompile pairing check EIP-2537:
$$\text{Gas} = 37,700 + 32,600 \times k$$
Di mana $k$ adalah jumlah pairing pairs. Untuk setiap signature BLS12-381, kita melakukan 2 pairing ($k=2$).

- **Analisis Gas Loop Individual**:
  $$\text{Gas}_{\text{loop}} = N \times (37,700 + 32,600 \times 2) = N \times 102,900 \text{ gas}$$
- **Analisis Gas Consolidated Batch**:
  $$\text{Gas}_{\text{batch}} = 37,700 + 32,600 \times 2N = 37,700 + 65,200 \times N \text{ gas}$$
- **Penghematan Bersih**:
  $$\Delta \text{Gas} = (N - 1) \times 37,700 \text{ gas}$$

Untuk ukuran batch $N = 8$, kita menghemat $7 \times 37,700 = 263,900$ gas hanya dari overhead pemanggilan precompile.

## Sumber primer

- **EIP-2537 Official Specification**: https://eips.ethereum.org/EIPS/eip-2537 (Menguraikan spesifikasi input precompile `0x0f` berupa rentetan koordinat poin $G_1$ dan $G_2$).
- **Arbitrum ArbOS 50 "Dia" activation**: Mengaktifkan precompile EIP-2537 pada Arbitrum Sepolia & Mainnet.

## Sumber pembanding

- **SSV Network / Charon BLS verification patterns**: Desain agregasi pairing multi-signature.

## Known risks

- **Rogue-key / Cancellation Attack**: 
  Secara teori, penggabungan perkalian pairing tanpa random scaling rentan terhadap cancellation attack jika penyerang dapat menyusun public key secara dinamis.
  *Mitigasi*:
  1. Seluruh public key issuer (`pk_iss`) diverifikasi secara ketat terhadap whitelist `trusted_issuer_keys` yang dikelola oleh governance admin.
  2. Pesan spend di-hash secara kriptografis menggunakan `compute_spend_hash` dengan parameters yang terikat ke chain_id, contract address, dan nonce unik untuk mencegah replay.

## Versi/library/network

- Arbitrum Sepolia (`chain_id = 421614`) & Mainnet.
- Stylus SDK `0.10.7`
- ark-bls12-381 `0.6.0`
- alloy-primitives `1.6.0`

## Rencana positive test

- **PT-01**: Eksekusi batch spend dengan 2, 4, dan 8 item berhasil memindahkan dana secara keseluruhan.
- **PT-02**: Saldo `total_deposited_principal` berkurang tepat sebesar jumlah total nominal payout ditambah protocol fee dari seluruh item.

## Rencana negative test

- **NT-01**: Mengubah satu signature di dalam batch menjadi salah, memastikan precompile `0x0f` gagal dan me-revert transaksi dengan error `BATCH_ITEM_INVALID`.
- **NT-02**: Memasukkan nullifier yang duplikat di dalam batch yang sama atau yang sudah pernah terpakai sebelumnya, transaksi harus revert.
- **NT-03**: Mengirimkan batch dengan ukuran $N > 8$ atau $N = 0$, harus ditolak dengan error `BATCH_TOO_LARGE` / `EMPTY_BATCH`.

## Rollback/recovery

Jika ditemukan kendala kompatibilitas encoding di testnet atau mainnet, relayer node dapat menonaktifkan pengiriman batch secara instan via env configuration `NIMBUS_BATCH_ENABLED=false` untuk kembali ke mode fallback spend individual satu per satu tanpa memerlukan upgrade contract baru.
