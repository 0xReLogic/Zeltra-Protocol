# Nimbus Relayer — 05: Signed Quotes & Klaim Komisi Gas

Dokumen ini menjelaskan mesin estimasi biaya berbasis EIP-712 pada [`nimbus-node/src/handlers/quote.rs`](file:///workspaces/Zeltra-Protocol/nimbus-node/src/handlers/quote.rs) dan background worker penarikan komisi pada [`nimbus-node/src/execution_fee_claimer.rs`](file:///workspaces/Zeltra-Protocol/nimbus-node/src/execution_fee_claimer.rs).

---

## 1. Masalah Ketidakpastian Gas pada Sistem Pembayaran

Tanpa fixed quote di muka, pengguna tidak tahu berapa biaya akhir transaksi mereka, atau transaksi gagal di tengah jalan saat terjadi lonjakan gas (*gas spike*).

Solusi Nimbus: **EIP-712 Signed Execution Quote**:
* Pengguna meminta kuotasi harga pasti sebelum menandatangani transaksi.
* Kuotasi mengikat batas maksimal komisi relayer (`max_execution_fee`) dan masa berlaku kuotasi (`quote_expiry`).
* Relayer menanggung risiko fluktuasi gas di on-chain, dan sebagai kompensasinya mendapatkan margin efisiensi saat transaksi berhasil digabungkan ke dalam batch.

---

## 2. Struktur Komponen Biaya Transaksi

Saat client memanggil endpoint `GET /api/quote/private-spend`:

```text
Total Yang Didebit Dari Saldo Pengguna =
    Nominal Invoice Merchant (Diterima 100% utuh oleh merchant)
  + Biaya Protokol (0.25% default, atau diskon holding time)
  + Biaya Eksekusi Relayer (Reimbursement Gas + Markup Relayer 15%)
```

### Rincian Komponen Kuotasi:
1. **Relayer Execution Gas (`relayer_gas_cost`):** Estimasi biaya gas eksekusi relayer on-chain aktual dari RPC node Arbitrum yang dikonversi ke unit stablecoin USDC (misal ~0.02 USDC).
2. **CCIP Network Fee (`ccip_network_fee`):** Biaya lintas rantai Chainlink CCIP (hanya jika parameter `destination_chain_selector` disertakan, default ~0.80 USDC / 800.000 base units). Nilai ini transparan dipisahkan dari relayer execution gas di response JSON.
3. **Markup Relayer (`relayer_markup`):** Tambahan komisi operasional flat **15%** (`DEFAULT_RELAYER_MARKUP_BPS = 1500`) yang dihitung **HANYA dari `relayer_gas_cost` lokal**, TIDAK PERNAH mem-markup `ccip_network_fee` (karena CCIP adalah fee pass-through murni pihak ketiga Chainlink).
4. **Diskon Waktu Simpan (*Holding Time*):** Node memeriksa timestamp `association_root` di cache memori. Jika dana sudah di-hold $\ge 7$ hari $\to$ fee turun ke 0.20%; jika $\ge 30$ hari $\to$ fee turun ke 0.10%.

### Transparansi UX (Same-Chain vs Cross-Chain):
- **Same-Chain Spend:** `ccip_network_fee = null`, `destination_chain_selector = null`. Pengguna hanya membayar gas eksekusi lokal + markup relayer (15%).
- **Cross-Chain CCIP Spend:** `ccip_network_fee = 800000`, `relayer_gas_cost = 20000`, `destination_chain_selector = <selector>`. UI client menampilkan rincian adil dan transparan:
  * Biaya Jaringan Chainlink CCIP: `$0.80` *(pass-through 100% tanpa markup)*
  * Biaya Gas Relayer Arbitrum: `$0.02`
  * Markup Operasional Relayer (15% dari gas relayer): `$0.003` *(3.000 base units)*
  * Total Biaya Eksekusi (`execution_fee`): `$0.823` *(823.000 base units)*
  * Nilai invariant matematika dan EIP-712 hashing tetap terjaga 100% konsisten.

---

## 3. Worker Pencairan Komisi Gas (`execution_fee_claimer.rs`)

Komisi relayer tidak ditarik satu per satu di setiap transaksi spend karena akan memboroskan gas on-chain:

1. **Akumulasi di Database:** 
   Setiap kali batch transaksi confirmed, kolom `execution_fee` dicatat pada tabel `spend_batches` dengan status `claimed = false`.
2. **Background Worker:** 
   Secara berkala menghitung total komisi yang belum dicairkan:
   ```sql
   SELECT SUM(execution_fee) FROM spend_batches WHERE claimed = false AND status = 'confirmed';
   ```
3. **Pencairan Otomatis On-Chain:** 
   Ketika akumulasi mencapai ambang batas (misal: $\ge 50$ USDC), worker memanggil fungsi smart contract:
   ```rust
   nimbus_contract.claim_execution_fees(amount)
   ```
4. **Update Status Atomic:** 
   Setelah tx receipt berhasil dikonfirmasi di blockchain, seluruh baris batch terkait diperbarui menjadi `claimed = true`.

---

## 4. Integrasi Quote pada ZK Private Note Spend (DEC-022 & DEC-025)

Pada model ZK-UTXO ([`DEC-016`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-016-private-note-change-ledger.md) & [`DEC-025`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-025-relayer-zk-note-spend-settlement-atomic-batching-and-reconciliation.md)), penentuan fee eksekusi relayer terikat secara kriptografis ke dalam sirkuit zero-knowledge `PrivateNoteCircuit`:

1. **Semantic Quote Hash Binding:**
   * Kuotasi biaya eksekusi relayer menghasilkan `quote_hash` (digest EIP-712).
   * Nilai `quote_hash` dipasok sebagai public input ke-7 (`public_inputs[7]`) pada sirkuit Groth16.
   * Node relayer ([`nimbus-node/src/handlers/spend.rs`](file:///workspaces/Zeltra-Protocol/nimbus-node/src/handlers/spend.rs)) memverifikasi bahwa `quote_hash` di request cocok byte-for-byte dengan nilai yang dibuktikan di dalam proof ZK (mitigasi DEC-022).
2. **Replay Protection Quote ID:**
   * Jika signed quote disertakan pada request ZK spend (`quote_id`, `quote_signature`, `user_address`), relayer memverifikasi tanda tangan EIP-712 pengguna dan memastikan `quote_id` belum pernah dipakai sebelumnya via `check_and_insert_quote_id()`.
3. **Penegakan Batas Maksimal Fee (`max_execution_fee`):**
   * Di smart contract Stylus ([`nimbus-contracts/src/spend.rs`](file:///workspaces/Zeltra-Protocol/nimbus-contracts/src/spend.rs)) dan di ingress relayer, berlaku invariant ketat:
     $$\text{execution\_fee} \le \text{max\_execution\_fee}$$
   * Jika relayer mencoba memotong fee di atas kesepakatan kuotasi pengguna, transaksi otomatis revert dengan kode `EXECUTION_FEE_EXCEEDS_MAX`.

---

## 5. Integrasi Oracle Chainlink ETH/USD & Akuntansi Margin Relayer (DEC-029)

Untuk menghitung biaya gas aktual dalam USDC dan melacak margin keuntungan operasional batch secara akurat:
1. **Chainlink Data Feeds (`AggregatorV3Interface`):**
   * Relayer membaca harga ETH/USD secara langsung dari oracle Chainlink on-chain via `eth_call` (0 gas cost):
     - **Arbitrum Sepolia:** `0xd30e2101a97dcbAeBCBC04F14C3f624E67A35165`
     - **Arbitrum One:** `0x639Fe6ab55C921f74e7fac1ee960C0B6293ba612`
   * Fungsi `get_chainlink_eth_price` memvalidasi bahwa nilai harga positif dan tidak kedaluwarsa (`staleness_threshold_secs <= 3600s`).
2. **Fail-Safe Fallback:**
   * Jika panggilan RPC ke oracle gagal atau data stale, relayer secara otomatis menggunakan fallback dari environment variable `NIMBUS_ETH_PRICE_USDC` (default: `$3,500.0`).
3. **Penyimpanan Metrik & Observabilitas:**
   * Kolom `margin_usdc` pada tabel `spend_batches` dihitung berdasarkan harga pasar aktual:
     $$\text{margin\_usdc} = \text{total\_revenue\_usdc} - (\text{total\_cost\_eth} \times \text{eth\_price})$$
   * Nilai harga aktif juga diexpose via endpoint `/health` (`eth_price_usdc`) untuk transparansi solvabilitas relayer.
