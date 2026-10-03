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
1. **Gas Reimbursement:** Estimasi biaya gas on-chain aktual dari RPC node Arbitrum yang dikonversi ke unit stablecoin USDC.
2. **Markup Relayer:** Tambahan komisi operasional flat **15%** (`DEFAULT_RELAYER_MARKUP_BPS = 1500`) di atas biaya gas.
3. **Diskon Waktu Simpan (*Holding Time*):** Node memeriksa timestamp `association_root` di cache memori. Jika dana sudah di-hold $\ge 7$ hari $\to$ fee turun ke 0.20%; jika $\ge 30$ hari $\to$ fee turun ke 0.10%.

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
