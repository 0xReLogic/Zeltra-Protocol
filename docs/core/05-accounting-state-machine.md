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
| **Deposit** | `assets += gross` | `refundable_liability += net`<br>`protocol_fees += fee` | Fee 0.20% dipotong di muka; sisa net siap di-mint. |
| **Resolve (Reveal)** | *Tidak berubah* | `refundable_liability -= net`<br>`user_note_liability += net` | Hak refund hangus; note aktif menjadi saldo privat. |
| **Refund** | `assets -= net` | `refundable_liability -= net` | Dilakukan oleh depositor jika timeout 24 jam lewat. |
| **Private Spend** | `assets -= payout` | `user_note_liability -= (payout + fee + exec)`<br>`protocol_fees += fee`<br>`relayer_liability += exec` | Liabilitas user berkurang sebesar total debit invoice. |
| **Claim Relayer Fee** | `assets -= amount` | `relayer_liability -= amount` | Relayer mencairkan komisi gas yang sudah terkumpul. |
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
2. **Besaran Tarif Fee:**
   * **Deposit Fee:** `20 bps` (0.20%).
   * **Private Spend Fee (Default):** `25 bps` (0.25%).
   * **Holding Time Discount (≥ 7 Hari):** Diskon ke `20 bps` (0.20%).
   * **Whale Holding Time Discount (≥ 30 Hari):** Diskon ke `10 bps` (0.10%).
3. **Holding Time Proof:**
   Dihitung secara anonim dan gas-efisien di smart contract on-chain menggunakan timestamp registrasi root:
   `mapping(bytes32 => uint256) clean_association_roots`.
