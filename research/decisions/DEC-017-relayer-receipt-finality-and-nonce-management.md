# DEC-017: Relayer Receipt Finality, Nonce Management, and Transaction Replacement

## Status

Accepted for implementation on 2026-10-03.

## Masalah

Implementasi relayer sebelumnya memiliki celah operasional pada pengiriman transaksi:
1. **Nonce Collision antar Worker:** Fungsi `send_tx_with_fallback()` mengambil nonce secara ad-hoc via `provider.get_transaction_count()`. Jika beberapa worker mengeksekusi settlement secara paralel, mereka memperoleh nilai nonce yang identik, memicu error `nonce too low` atau `nonce already known`.
2. **Transaksi Macet di Mempool (Stuck Transactions):** Tidak ada mekanisme batas waktu (timeout) pada receipt polling. Jika gas price melonjak drastis, transaksi relayer tersangkut di mempool dan menahan seluruh pipeline pengiriman berikutnya karena sifat sekuensial nonce EVM (terutama pada Nitro sequencer yang hanya memiliki short-lived retry buffer).
3. **Ketiadaan Konfirmasi Kedalaman Block (Reorg Protection):** Receipt yang baru saja masuk block langsung dianggap final tanpa menunggu ambang batas konfirmasi ($N$ blocks), rentan terhadap soft-reorg pada L2 atau reorg L1.
4. **Ketiadaan Visibilitas Status Transaksi:** Klien SDK dan pengguna tidak memiliki endpoint standar untuk memeriksa status lifecycle settlement (`pending`, `confirmed`, `failed`), receipt block number, gas used, dan error revert on-chain.

## Invariant Bisnis dan Security

1. **Atomic Nonce Allocation:** Relayer mengalokasikan nonce secara terkoordinasi (thread-safe / DB-synchronized) agar tidak pernah terjadi tabrakan nonce antar worker atau pembuatan lubang nonce (nonce gaps).
2. **EIP-1559 Compliant Replacement:** Jika transaksi tidak masuk ke dalam block dalam kurun waktu timeout (default 30 detik), sistem secara otomatis membuat transaksi pengganti (*replacement transaction*) dengan **nonce yang sama persis** dan menaikkan parameter biaya gas:
   $$\text{maxFeePerGas}_{\text{new}} \ge \lfloor \text{maxFeePerGas}_{\text{old}} \times 1.15 \rfloor$$
   $$\text{maxPriorityFeePerGas}_{\text{new}} \ge \lfloor \text{maxPriorityFeePerGas}_{\text{old}} \times 1.15 \rfloor$$
3. **Chain-Specific Confirmation Threshold:** Transaksi dinyatakan `confirmed` hanya jika:
   $$\text{current\_block} - \text{receipt\_block} + 1 \ge \text{confirmation\_threshold}$$
   (Nilai default: 1 untuk Arbitrum fast-finality, dapat dikonfigurasi via `NIMBUS_CONFIRMATION_THRESHOLD` hingga 12 untuk Ethereum L1).
4. **Idempotent Terminal Status & Audit Trail:** Seluruh status pengiriman dan penggantian dicatat dalam database relayer untuk diaudit, dan diekspos melalui `GET /api/tx-status?tx_hash=...`.

## Pilihan yang Dipertimbangkan

1. **Optimistic In-Memory Nonce Counter murni:**
   - *Kelemahan:* Reset counter saat restart menyebabkan desinkronisasi terhadap state on-chain, memicu error `nonce too low`.
2. **Hybrid Remote Sync + In-Memory Mutex Lock:**
   - *Kelebihan:* Mengambil baseline nonce dari RPC saat inisialisasi / startup, lalu mengontrol alokasi berikutnya via atomic Mutex di relayer. Jika terjadi kegagalan atau error `nonce too low`, secara otomatis resinkronisasi dengan RPC pending transaction count.
   - *Keputusan:* Menggunakan Hybrid Remote Sync dengan thread-safe lock dan pelacakan transaksi di database SQLite.

## Detail Desain

### 1. Atomic Nonce Lock & Replacement Loop
```text
[Worker Claim Item]
       |
[Acquire Nonce Mutex] -> Ambil confirmed/pending nonce dari RPC atau lokal counter
       |
[Sign & Broadcast Tx #1] -> Catat tx_hash, nonce, gas params, timestamp
       |
[Wait for Receipt with Timeout (30s)]
  ├── Receipt Diterima -> Verifikasi Confirmation Threshold (N blocks) -> Status: Confirmed
  └── Timeout Expired -> Broadcast Tx Replacement (Same Nonce, +15% Gas) -> Ulangi polling
```

### 2. Status Lifecycle di Database & API
Tabel transaksi / antrian menyimpan:
* `tx_hash`: Hash transaksi aktif terbaru.
* `status`: `pending` (termasuk broadcasting & replacement), `confirmed`, `failed`.
* `block_number`: Block di mana receipt terinklusi.
* `gas_used`: Jumlah gas yang terkonsumsi.
* `confirmations`: Jumlah konfirmasi block saat ini.
* `error_reason`: Pesan revert atau kegagalan jika status `failed`.

## Sumber Primer & Referensi

1. Arbitrum Nitro Architecture — Nonce Management:
   https://docs.arbitrum.io/arbitrum-essentials/arbitrum-vs-ethereum/nonce-management
2. EIP-1559: Fee market change for ETH 1.0 chain:
   https://eips.ethereum.org/EIPS/eip-1559
3. OpenZeppelin Relayer EVM Integration & Transaction Replacement:
   https://docs.openzeppelin.com/relayer/1.1.x/evm
