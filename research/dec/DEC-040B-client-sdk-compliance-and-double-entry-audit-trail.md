# DEC-040B: Client SDK Compliance Engine, B2B Double-Entry Audit Trail & CSV Sanitization

- **Status:** APPROVED AS ARCHITECTURAL SUB-SPECIFICATION (Child of [`DEC-040`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-040-viewing-key-export-suite-and-cex-selective-disclosure.md))
- **Author:** Zeltra Protocol Architecture, SDK & Enterprise Accounting Team
- **Date:** 2026-10-15
- **Applies to:**
  - `zeltra-sdk/src/compliance/mod.rs` (Core compliance module and error definitions)
  - `zeltra-sdk/src/compliance/models.rs` (Data models: `AuditReport`, `AuditFilter`, `AuditRecord`, `AssetSummary`)
  - `zeltra-sdk/src/compliance/exporter.rs` (JSON and sanitized CSV formatters)
  - `zeltra-sdk/src/wallet/note_wallet.rs` (`PrivateNoteWallet` audit trail query methods and FVK delegation)
  - `zeltra-sdk/src/lib.rs` (Module re-exports and feature gating)
- **Parent & Related DECs:**
  - [`DEC-040`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-040-viewing-key-export-suite-and-cex-selective-disclosure.md) (Master Viewing Key Export Suite Specification)
  - [`DEC-040A`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-040A-selective-disclosure-circuit-and-key-separation.md) (Cryptographic Key Separation & Groth16 Circuit)
  - [`DEC-034`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-034-dual-asset-architecture-usdc-weth-sovereign-commerce-rails.md) (Dual-Asset Architecture: USDC Commerce & WETH Sovereign)
  - [`DEC-034A`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-034A-canonical-field-bit-packing-and-intra-asset-circuit-constraints.md) (Canonical Bit-Packing & Unit Normalization)
  - [`DEC-036B`](file:///workspaces/Zeltra-Protocol/research/dec/DEC-036B-client-sdk-knapsack-coin-selection-consolidation-and-bulk-sync.md) (Client SDK 2PC Crash-Safe Note Store)
- **Architectural Paradigm:** *"Double-Entry UTXO Balance, Anti-CSV Injection, Zero Secret Leakage"*

---

## 1. Executive Summary & Dilema Kepatuhan Akuntansi Korporat B2B

Penggunaan rel pembayaran privat oleh AI Agent Fleet, merchant enterprise, dan korporat B2B menuntut kepatuhan mutlak terhadap standar audit keuangan global (seperti IFRS, GAAP, dan SOC 2 Type II) serta peraturan perpajakan (seperti Formulir Pajak Penghasilan Badan & SPT). 

### Tantangan Akuntansi Kas UTXO Privat:
1. **Ketidakseimbangan Neraca Kas (The Balance-Mismatch Trap):**
   Pada model UTXO (seperti Bitcoin atau Zcash), pengeluaran dana melibatkan pemusnahan input note dan penciptaan note kembalian (*change note*). Jika laporan akuntansi hanya mencatat jumlah yang dibayarkan ke merchant tanpa memperhitungkan *input notes consumed*, *change notes created*, dan biaya gas relayer, neraca akhir (*closing balance*) tidak akan pernah cocok dengan audit saldo bank/dompet.
2. **Serangan Formula Spreadsheet (CSV Injection / CWE-1236):**
   Akuntan perusahaan membuka laporan transaksi menggunakan Microsoft Excel, Google Sheets, atau LibreOffice Calc. Jika field teks (seperti nama organisasi, memo transfer, atau label penerima) diawali dengan karakter formula (`=`, `+`, `-`, `@`), file CSV dapat mengeksekusi perintah biner lokal (*arbitrary code execution*) di laptop auditor saat dibuka.
3. **Pemisahan Entitas Audit Internal vs Eksternal:**
   Departemen keuangan internal memerlukan visibilitas audit real-time atas seluruh perputaran kas privat tanpa boleh memiliki kuasa memindahkan atau mencuri dana perusahaan (*separation of custody and audit*).

**Solusi Penetapan DEC-040B:**
1. Mengembangkan mesin audit `zeltra_sdk::compliance` yang menerapkan prinsip pembukuan berpasangan (*double-entry bookkeeping*) untuk seluruh siklus hidup note privat.
2. Menerapkan protokol sanitasi CSV otomatis terhadap vektor eksploitasi CWE-1236.
3. Mengintegrasikan delegasi `FullViewingKey` (Tier 3) ke dalam `PrivateNoteWallet` untuk mode audit baca-saja (*read-only auditor session*).

---

## 2. Struktur Data Model Kepatuhan (`models.rs`)

```rust
// zeltra-sdk/src/compliance/models.rs
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Filter query untuk pelaporan audit terarah.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AuditFilter {
    /// None = Semua aset, Some(0) = WETH, Some(1) = USDC
    pub asset_id: Option<u8>,
    /// Batas awal waktu transaksi (Unix timestamp detik)
    pub from_timestamp: Option<u64>,
    /// Batas akhir waktu transaksi (Unix timestamp detik)
    pub to_timestamp: Option<u64>,
    /// Apakah menyertakan note yang sudah spent
    pub include_spent: bool,
}

/// Struktur data utama laporan audit kepatuhan B2B berstandar IFRS.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuditReport {
    pub schema_version: String, // "zeltra.compliance.audit.v1"
    pub generated_at: u64,
    pub organization_name: String,
    pub report_period: PeriodRange,
    pub summary_by_asset: HashMap<String, AssetSummary>,
    pub records: Vec<AuditRecord>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PeriodRange {
    pub from_timestamp: u64,
    pub to_timestamp: u64,
}

/// Ringkasan neraca saldo per aset untuk rekonsiliasi akuntansi.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AssetSummary {
    pub asset_id: u8,
    pub symbol: String,
    pub circuit_decimals: u8,
    pub opening_balance: String,
    pub total_inflow_deposit: String,
    pub total_merchant_payout: String,
    pub total_protocol_fees: String,
    pub total_relayer_execution_fees: String,
    pub closing_unspent_balance: String,
}

/// Jurnal entri baris transaksi kas privat.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuditRecord {
    pub record_id: u64,
    pub timestamp: u64,
    pub date_iso: String,
    pub tx_hash: String,
    pub direction: RecordDirection,
    pub asset_id: u8,
    pub symbol: String,
    pub input_notes_total: String,
    pub deposit_amount: String,
    pub merchant_payout: String,
    pub protocol_fee: String,
    pub relayer_execution_fee: String,
    pub change_amount: String,
    pub created_commitments: Vec<String>,
    pub spent_nullifiers: Vec<String>,
    pub recipient: Option<String>,
    pub status: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecordDirection {
    InflowDeposit,
    OutflowSpend,
    InPoolConsolidation,
}
```

---

## 3. Persamaan Konservasi UTXO & Validasi Neraca Saldo

Sebelum menyetujui dan menandatangani file laporan audit, mesin SDK wajib menjalankan dua uji rekonsiliasi matematis secara *fail-closed*:

### A. Uji Konservasi Transaksi Outflow Spend (Per-Record Check):
Untuk setiap record dengan arah `RecordDirection::OutflowSpend`, total nilai input note yang dikonsumsi wajib terdistribusi sempurna tanpa selisih:
$$\text{input\_notes\_total} == \text{merchant\_payout} + \text{protocol\_fee} + \text{relayer\_execution\_fee} + \text{change\_amount}$$

Jika $\Delta = \text{input\_notes\_total} - (\dots) \neq 0$, fungsi ekspor seketika membatalkan pembuatan laporan dengan error:
```rust
ComplianceError::AccountingReconciliationMismatch {
    record_id: record.record_id,
    expected: input_notes_total,
    actual: sum_of_outputs,
}
```

### B. Uji Persamaan Neraca Saldo Periodik (Asset Balance Sheet Check):
Untuk setiap aset (`WETH` pada `asset_id = 0`, dan `USDC` pada `asset_id = 1`), saldo penutupan (*closing balance*) yang tercantum dalam ringkasan wajib cocok dengan saldo aktual note yang berstatus `UNSPENT` pada database lokal:
$$\text{OpeningBalance} + \text{TotalInflow} - (\text{TotalMerchantPayout} + \text{TotalProtocolFees} + \text{TotalExecutionFees}) == \text{ClosingBalance}$$

Uji ini menjamin bahwa tidak ada note gaib, tidak ada kebocoran fee tersembunyi, dan angka yang disajikan dapat dipertanggungjawabkan di hadapan auditor eksternal.

---

## 4. Perlindungan Terhadap CSV Formula Injection (CWE-1236)

Dalam modul `zeltra-sdk/src/compliance/exporter.rs`, generator CSV menerapkan algoritma sanitasi ketat sebelum menuliskan baris tabel:

```rust
// zeltra-sdk/src/compliance/exporter.rs

/// Membersihkan nilai string dari karakter eksekusi formula spreadsheet.
pub fn sanitize_csv_cell(input: &str) -> String {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    // Karakter pemicu formula berbahaya menurut OWASP & CWE-1236
    let trigger_chars = ['=', '+', '-', '@', '\t', '\r'];
    if trigger_chars.iter().any(|&c| trimmed.starts_with(c)) {
        // Beri prefix tanda kutip tunggal untuk memaksa spreadsheet membacanya sebagai plain text
        format!("'{}", trimmed)
    } else {
        trimmed.to_string()
    }
}
```

Setiap kolom teks, termasuk `organization_name`, label `recipient`, `tx_hash`, dan string status, wajib melewati `sanitize_csv_cell` sebelum digabungkan ke dalam aliran CSV.

---

## 5. Protokol Delegasi Sesi Auditor (Tier 3 FVK)

Untuk mendukung pengawasan internal korporat tanpa hak belanja:

```rust
// zeltra-sdk/src/wallet/note_wallet.rs
impl PrivateNoteWallet {
    /// Mengekspor Full Viewing Key untuk diserahkan ke auditor internal.
    pub fn export_full_viewing_key(&self) -> Result<FullViewingKey, WalletError> {
        let (ivk, ovk) = self.viewing_keys()?;
        let sk = self.spending_key()?;
        let nk = crate::note::derive_nullifier_key(sk);
        Ok(FullViewingKey {
            owner_key: self.incoming_viewing_key()?.owner_key(),
            incoming_viewing_key: ivk.secret_bytes(),
            outcoming_viewing_key: ovk.key_bytes(),
            nullifier_deriving_key: nk,
        })
    }

    /// Membuka sesi dompet khusus auditor (Read-Only).
    pub fn open_auditor_session(fvk: &FullViewingKey) -> ReadOnlyAuditorSession {
        ReadOnlyAuditorSession::new(fvk.clone())
    }
}
```

### Kapabilitas Sesi Auditor (`ReadOnlyAuditorSession`):
- **Bisa:** Memindai event on-chain `NoteCommitmentAppended` dan mendekripsi note masuk menggunakan $ivk$.
- **Bisa:** Memeriksa status pembelanjaan note on-chain melalui kueri RPC `is_nullifier_spent(nf)` menggunakan $nk$.
- **Bisa:** Menghitung saldo neraca kas privat secara akurat.
- **DILARANG:** Memanggil `prepare_spend()`, `sign_quote()`, atau menghasilkan bukti pembelanjaan Groth16 (menghasilkan error `WalletError::SpendAuthorityRequired`).

---

## 6. Format Normalisasi Satuan Nilai

Untuk mencegah kekeliruan perhitungan finansial, nilai dikonversi dan diformat dengan presisi desimal kaku:
1. **USDC (Asset ID 1):**
   - Satuan sirkuit: Micro-USDC ($10^{-6}$).
   - Format CSV/JSON: `50000.000000` (Tepat 6 angka di belakang koma).
2. **WETH (Asset ID 0):**
   - Satuan sirkuit: Gwei ($10^{-9}$ ETH).
   - Format CSV/JSON: `5.000000000` (Tepat 9 angka di belakang koma).
3. **Anti-Mixing:** Seluruh ringkasan total (`summary_by_asset`) dipisahkan ke dalam sub-objek terisolasi. Dilarang menggabungkan kolom angka dari WETH dan USDC.

---

## 7. Matriks Pengujian Unit (`zeltra-sdk/tests/`)

1. `test_audit_report_reconciliation_exact_balance`: Simulasi riwayat akun dengan 5 deposit dan 3 spend multi-asset menghasilkan saldo penutupan yang identik dengan saldo unspent note di database lokal.
2. `test_audit_report_fails_on_unbalanced_record`: Jika data transaksi dimanipulasi sehingga input tidak sama dengan output + fee, ekspor gagal dengan error `AccountingReconciliationMismatch`.
3. `test_csv_sanitizes_formula_injection`: Input teks `=cmd|' /C calc'!A0` otomatis disanitasi menjadi `'=cmd|' /C calc'!A0` di output CSV.
4. `test_export_never_contains_spending_key`: Pengecekan byte memory memastikan tidak ada jejak byte $sk$ pada output JSON atau CSV.
5. `test_auditor_session_read_only_protection`: Memastikan sesi auditor yang dibuka hanya dengan FVK gagal saat mencoba memanggil fungsi pembelanjaan.
