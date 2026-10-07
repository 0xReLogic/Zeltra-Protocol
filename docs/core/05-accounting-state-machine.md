# Nimbus Core — 05: Multi-Liability Accounting & Solvabilitas

Dokumen ini menjelaskan model akuntansi formal pada [`nimbus-core/src/accounting.rs`](file:///workspaces/Zeltra-Protocol/nimbus-core/src/accounting.rs) dan kebijakan kalkulasi fee pada [`nimbus-core/src/fees.rs`](file:///workspaces/Zeltra-Protocol/nimbus-core/src/fees.rs) (mengacu pada spesifikasi [`DEC-016A Gate B`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-016A-private-note-spec-freeze.md)).

---

## 1. Masalah pada Model Akuntansi Lama

Pada arsitektur voucher lama, smart contract hanya mencatat satu angka global: `total_deposited_principal`.
* **Kelemahan Fatal:** Biaya execution fee relayer dicatat sebagai akumulasi, tetapi tidak mengurangi liabilitas user. Ketika relayer mencairkan klaim biaya (*fee claim*), aset kas kontrak berkurang tanpa pengurangan liabilitas yang setara $\to$ **Potensi Insolvabilitas (Aset < Liabilitas)**.

---

## 2. Struktur Multi-Liability (`ContractAccounting`)

Untuk menjamin keamanan keuangan setingkat perbankan (*full reserve*), liabilitas kontrak dipecah menjadi 3 pos terpisah yang independen:

```rust
pub struct ContractAccounting {
    pub assets: u64,                              // Total saldo fisik USDC di kontrak
    pub user_note_liability: u64,                 // Liabilitas atas unspent private notes milik user
    pub refundable_deposit_liability: u64,        // Liabilitas atas deposit yang belum di-reveal / bisa di-refund
    pub accrued_execution_fee_liability: u64,     // Liabilitas atas komisi gas yang menjadi hak relayer
    pub realized_protocol_fees: u64,              // Pendapatan bersih milik treasury protokol
}
```

### Invariant Solvabilitas Utama:
Pada setiap akhir eksekusi transisi state, kontrak **wajib membuktikan**:

$$\text{assets} \ge \text{user\_note\_liability} + \text{refundable\_deposit\_liability} + \text{accrued\_execution\_fee\_liability}$$

Jika kondisi ini dilanggar bahkan hanya selisih 1 unit terkecil (1 *base unit* USDC = $0.000001), transaksi **wajib revert** seketika (*fail-closed*).

---

## 3. Matriks Transisi State (*State Transitions*)

| Transisi | Perubahan Aset Kas | Perubahan Liabilitas | Keterangan |
|---|---|---|---|
| **Deposit** | `assets += gross` | `refundable_liability += net` | Fee deposit 0% (0 bps); 100% nominal menjadi net refundable deposit. |
| **Resolve (Reveal)** | *Tidak berubah* | `refundable_liability -= net`<br>`user_note_liability += net` | Hak refund hangus; note aktif dimint ke Merkle tree on-chain. |
| **Refund** | `assets -= net` | `refundable_liability -= net` | Dilakukan oleh depositor jika timeout 24 jam lewat (100% principal kembali). |
| **Private Spend** | `assets -= payout` | `user_note_liability -= (payout + fee + exec)`<br>`protocol_fees += fee`<br>`relayer_liability += exec` | Liabilitas user berkurang sebesar total debit invoice. |
| **Claim Relayer Fee** | `assets -= amount` | `relayer_liability -= amount` | Relayer mencairkan komisi gas yang sudah terkumpul (cek solvabilitas). |
| **Withdraw Treasury** | `assets -= amount` | `realized_protocol_fees -= amount` | Protokol menarik profit operasional. |

---

## 4. Kebijakan Biaya & Pembulatan Integer (`fees.rs`)

File referensi: [`nimbus-core/src/fees.rs`](file:///workspaces/Zeltra-Protocol/nimbus-core/src/fees.rs)

1. **Aturan Pembulatan ke Atas (*Ceiling Division*):**
   Seluruh penghitungan fee protokol menggunakan pembagian pembulatan ke atas (*ceiling division*) agar tidak pernah ada fraksi desimal yang merugikan kas kontrak:
   ```rust
   pub fn ceil_div(a: u64, b: u64) -> u64 {
       (a + b - 1) / b
   }
   ```
2. **Besaran Tarif Fee (Immutable Constants — DEC-028):**
   * **Deposit Fee:** `0 bps` (0.00% - Zero-Friction Inflow).
   * **Private Spend Fee (Outflow):** `45 bps` (0.45% Flat permanen per DEC-028).
   * **Diskon Holding 30 Hari Dihapus:** Dieliminasi untuk mencegah eksploitasi arbitrase umur root dan menjaga keselarasan filosofi *Volume Over TVL*.
   * **Relayer Execution Fee:** Estimasi gas L2 aktual + markup `15%` (default `1.500 bps`).
3. **Akuntansi Terprediksi & Proteksi Anti-Exploit:**
   Menghindari kompleksitas DAO governance setter untuk fee demi menutup celah eksploitasi flashloan voting atau governance hijacking. Aturan fee bersifat deterministik, matematis, dan 100% konsisten ($10 bayar $3 kembali $7).
