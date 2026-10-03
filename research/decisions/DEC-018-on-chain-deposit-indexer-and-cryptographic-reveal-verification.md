# DEC-018: On-Chain Deposit Event Indexer and Cryptographic Reveal Verification

## Status

Accepted for implementation on 2026-10-03.

## Masalah

Implementasi alur deposit dan reveal sebelumnya memiliki dua celah kritis yang dapat disalahgunakan:

1. **Injeksi Deposit Palsu via HTTP API (Phantom Deposits):**
   Fungsi `handle_deposit` di `nimbus-node/src/handlers/deposit.rs` mengonfirmasi deposit (`deposit_confirmed = 1`) hanya berdasarkan payload HTTP `POST /api/deposit` tanpa memverifikasi apakah transaksi deposit tersebut benar-benar dieksekusi di smart contract Stylus. Penyerang dapat memanggil endpoint ini secara langsung untuk memicu unmasking token tanpa menyetorkan collateral USDC sepeser pun.
   *Insiden Dunia Nyata:* Celah serupa menjadi penyebab exploit **Across Protocol Solana Bridge (Juli 2026)** di mana software relayer off-chain mempercayai event tanpa validasi program authority sehingga mengalirkan dana jutaan dolar untuk deposit fiktif, serta exploit **Coreum / XRP Ledger (Agustus 2026)** dan **Qubit Finance ($80M, 2022)**.

2. **Ketiadaan Verifikasi Kriptografi pada Reveal:**
   Fungsi `handle_reveal` merilis `masking_key_hex` dari database hanya dengan memeriksa flag status tanpa memverifikasi relasi matematis $k \cdot \text{pk}_{\text{iss}} == \text{com}_k$ di kurva BLS12-381 $\mathbb{G}_2$. Jika data di database terkorupsi atau terjadi race condition, node dapat melepaskan kunci cacat atau mengorbankan status invariant protocol.

---

## Invariant Bisnis & Keamanan

1. **No Phantom Deposits (Hukum Konservasi Collateral):**
   Status `deposit_confirmed` HANYA BOLEH diaktifkan jika terdapat bukti event on-chain `DepositFee` yang valid dari alamat kontrak Stylus resmi:
   $$\text{event DepositFee(bytes32 indexed session\_id, address indexed client, uint256 gross\_amount, uint256 fee, uint256 net\_amount)}$$
2. **Reorg Protection & Confirmation Depth:**
   Event on-chain hanya diindeks jika tinggi blok transaksi telah memenuhi ambang batas finalitas:
   $$\text{current\_block} - \text{event\_block} + 1 \ge \text{confirmation\_threshold}$$
3. **Cryptographic Verification Before Release:**
   Sebelum `resolve_session_release` menandai sesi `resolved = 1` dan melepaskan $k$, node wajib memverifikasi:
   $$\text{com}_k \stackrel{?}{=} k \cdot \text{pk}_{\text{iss}} \quad (\text{pada grup } \mathbb{G}_2 \text{ BLS12-381})$$
   Jika hasil komputasi tidak cocok dengan komitmen yang tercatat saat inisiasi sesi, proses **wajib fail-closed** (kembalikan error, tolak rilis kunci, dan log alert keamanan).
4. **Idempotensi & Tracking Blok Indexer:**
   Indexer menyimpan `last_indexed_block` di database persisten. Setiap event yang diproses bersifat idempoten (re-indexing tidak menduplikasi state atau merusak saldo).

---

## Pilihan Desain & Arsitektur

### 1. Polling Berbasis Block Range vs WebSocket Filter
* **WebSocket:** Cepat, namun rentan putus koneksi diam-diam (*silent drop*) saat load spike atau restart node RPC.
* **Polling HTTP dengan Sliding Window (Dipilih):**
  Relayer menjalankan background worker `deposit_indexer_worker` dengan interval teratur (misal: 3–5 detik). Indexer membaca block range dari $\text{last\_block} + 1$ sampai $\text{latest\_safe\_block}$. Jika terjadi restart, indexer secara mulus melanjutkan dari block terakhir yang tersimpan di DB.

### 2. Pengetatan Handler `POST /api/deposit`
* Endpoint `POST /api/deposit` tidak lagi dapat memasukkan sesi fiktif atau mengubah `deposit_confirmed = 1` secara sembarangan.
* Endpoint ini sekarang berfungsi sebagai **On-Demand Verification Trigger**: jika client memanggil `/api/deposit` dengan `session_id` dan `tx_hash`, node akan segera memeriksa receipt/event dari RPC untuk sesi tersebut secara langsung (tanpa harus menunggu siklus batch worker reguler), namun konfirmasi tetap bersumber 100% dari event on-chain.

---

## Detail Alur Eksekusi

```text
[ User / SDK ]
      |
      | 1. Stylus Contract: deposit_with_commitment(session_id, com_k, amount, note_cm)
      v
[ Arbitrum Stylus ]
      |
      | 2. Emit Event: DepositFee(session_id, client, gross, fee, net)
      v
[ Deposit Indexer Worker / nimbus-node ]
      |
      | 3. Query get_logs(contract_address, DepositFee) up to safe block
      | 4. Cocokkan session_id & net_amount dengan database sessions
      v
[ Database SQLCipher ]
      |
      | 5. Update sessions SET deposit_confirmed = 1, deposit_tx_hash = ?, deposit_block = ?
      v
[ User memanggil POST /api/reveal ]
      |
      | 6. Handler membaca k, com_k, pk_iss
      | 7. Verifikasi Kriptografi: k * pk_iss == stored com_k
      ├── LULUS: Update resolved = 1, rilis k ke user, zeroize k di memori
      └── GAGAL: FAIL-CLOSED (Kembalikan Error, Kunci Dibatalkan)
```

---

## Sumber Primer & Referensi Exploit (2024–2026)

1. **Across Protocol Relayer Incident Post-Mortem (Juli 2026):**
   * *Analisis:* Exploit software relayer off-chain yang mempercayai log transaksi tanpa validasi discriminator dan program ID.
   * *URL:* https://x.com/AcrossProtocol/status/2080722320814121237
2. **Harman Kamboj (Juni 2026) — "Handling chain reorgs without corrupting your indexer":**
   * *Prinsip:* Jangan pernah membiarkan payout/settlement membaca unconfirmed state; terapkan absolute event writes dan pelacakan block hash/number.
   * *URL:* https://hammyasf.github.io/handling-chain-reorgs-indexer.html
3. **Coreum / XRP Ledger Bridge Exploit (Agustus 2026):**
   * *Analisis:* Relayer memproses penarikan riil akibat deposit palsu tanpa rekonsiliasi state token & saldo on-chain.
   * *URL:* https://hacked.slowmist.io/?c=Bridge&page=1
4. **EIP-2537: Precompiles for BLS12-381 curve operations:**
   * Operasi scalar multiplication pada grup $\mathbb{G}_2$ ($k \cdot P_2$).
   * *URL:* https://eips.ethereum.org/EIPS/eip-2537
