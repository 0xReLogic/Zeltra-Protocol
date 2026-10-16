# TASK B (DEC-040B): Client SDK Compliance Engine, B2B Double-Entry Audit Trail & CSV Sanitization (`zeltra-sdk`)

> **Scope:** `zeltra-sdk/` (Client Wallet State, Compliance Module, Report Formatter)  
> **Spesifikasi Referensi:** [`research/decisions/DEC-040B-client-sdk-compliance-and-double-entry-audit-trail.md`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-040B-client-sdk-compliance-and-double-entry-audit-trail.md) & [`research/decisions/DEC-040-viewing-key-export-suite-and-cex-selective-disclosure.md`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-040-viewing-key-export-suite-and-cex-selective-disclosure.md)  
> **Prinsip:** *"Double-Entry UTXO Balance, Anti-CSV Injection, Zero Secret Leakage"*  
> **Status:** **PENDING IMPLEMENTATION**

---

## 1. Latar Belakang & Tujuan Arsitektur

Menyediakan modul kepatuhan enterprise pada `zeltra-sdk` untuk menyusun laporan audit pembukuan berpasangan (*double-entry*) bagi bisnis B2B dan AI Agent Fleet, pembuatan berkas bukti sumber dana untuk CEX, pertahanan terhadap injeksi formula spreadsheet (CWE-1236), dan pendelegasian sesi audit baca-saja (*read-only auditor session*) melalui `FullViewingKey` (Tier 3).

---

## 2. Checklist Implementasi Rinci

### A. Modul Kepatuhan & Model Data (`zeltra-sdk/src/compliance/models.rs`)
- [ ] **Definisi Model Laporan Audit B2B:**
  - `pub struct AuditReport`:
    - `pub schema_version: String` ("zeltra.compliance.audit.v1")
    - `pub generated_at: u64`
    - `pub organization_name: String`
    - `pub report_period: PeriodRange`
    - `pub summary_by_asset: HashMap<String, AssetSummary>`
    - `pub records: Vec<AuditRecord>`
  - `pub struct AssetSummary`:
    - `pub asset_id: u8`, `pub symbol: String`, `pub circuit_decimals: u8`
    - `pub opening_balance: String`, `pub total_inflow_deposit: String`
    - `pub total_merchant_payout: String`, `pub total_protocol_fees: String`
    - `pub total_relayer_execution_fees: String`, `pub closing_unspent_balance: String`
  - `pub struct AuditRecord`:
    - `pub record_id: u64`, `pub timestamp: u64`, `pub date_iso: String`, `pub tx_hash: String`
    - `pub direction: RecordDirection` (`InflowDeposit`, `OutflowSpend`, `InPoolConsolidation`)
    - `pub asset_id: u8`, `pub symbol: String`
    - `pub input_notes_total: String`, `pub deposit_amount: String`, `pub merchant_payout: String`
    - `pub protocol_fee: String`, `pub relayer_execution_fee: String`, `pub change_amount: String`
    - `pub created_commitments: Vec<String>`, `pub spent_nullifiers: Vec<String>`
    - `pub recipient: Option<String>`, `pub status: String`
- [ ] **Definisi Filter Audit (`AuditFilter`):**
  - `asset_id: Option<u8>`, `from_timestamp: Option<u64>`, `to_timestamp: Option<u64>`, `include_spent: bool`.

### B. Validasi Konservasi UTXO & Rekonsiliasi Neraca (`zeltra-sdk/src/compliance/mod.rs`)
- [ ] **Pengecekan Konservasi Per-Record (`validate_utxo_conservation`):**
  - Untuk setiap record `OutflowSpend`:
    $$\text{input\_notes\_total} == \text{merchant\_payout} + \text{protocol\_fee} + \text{relayer\_execution\_fee} + \text{change\_amount}$$
  - Kembalikan `ComplianceError::AccountingReconciliationMismatch` jika ada selisih sekecil 1 unit terendah.
- [ ] **Pengecekan Neraca Saldo Periodik (`validate_balance_sheet_reconciliation`):**
  - Untuk setiap aset (`WETH` dan `USDC`):
    $$\text{OpeningBalance} + \text{TotalInflow} - (\text{TotalMerchantPayout} + \text{TotalProtocolFees} + \text{TotalExecutionFees}) == \text{ClosingBalance}$$
  - Pastikan `ClosingBalance` sama persis dengan saldo aktual note yang berstatus `UNSPENT` pada database lokal wallet.

### C. Generator Format Ekspor (JSON & CSV Sanitized) (`zeltra-sdk/src/compliance/exporter.rs`)
- [ ] **JSON Exporter:** Format JSON terstruktur yang rapi (`serde_json::to_string_pretty`).
- [ ] **CSV Exporter dengan Sanitasi Formula Injection (CWE-1236):**
  - Implementasikan fungsi pembersih `sanitize_csv_cell(&str) -> String`.
  - Jika sel diawali dengan `=`, `+`, `-`, `@`, `\t`, atau `\r`, tambahkan prefix `'` (tanda kutip tunggal).
  - Format angka desimal presisi: tepat 6 desimal untuk USDC, tepat 9 desimal untuk WETH.
  - Header CSV wajib lengkap: `RecordID,Timestamp,DateISO,Direction,AssetID,AssetSymbol,GrossAmount,FeePaid,NetAmount,Recipient,TxHash,Commitment,Nullifier,Status`.

### D. Pembuatan Paket Bukti CEX (`zeltra-sdk/src/compliance/cex_proof.rs`)
- [ ] **Struktur `CexProofPayload`:**
  - `verifier_binding`: `verifier_id`, `account_uid`, `nonce`, `expires_at`, `challenge_hash`.
  - `asset`: `asset_id`, `symbol`, `circuit_unit_name`, `circuit_decimals`, `erc20_decimals`, `raw_circuit_value`, `value_packed_scalar`, `human_value`.
  - `note_cryptography`: `commitment`, `owner_key`, `rho`, `randomness`, `nullifier`, `nullifier_status`, `spent_tx_hash`, `payout_recipient`.
  - `mmr_proof`: `leaf_index`, `leaf_count`, `peak_index`, `sibling_path`, `mmr_peaks`, `mmr_root`.
  - `ownership_zk_proof`: Bukti Groth16 `SelectiveDisclosureCircuit` (5-PI).
- [ ] **Fungsi Wallet (`PrivateNoteWallet`):**
  - `pub fn create_cex_proof(&self, commitment: [u8; 32], binding: VerifierBinding) -> Result<CexProofPayload, WalletError>`.

### E. Integrasi Sesi Auditor Tier 3 (`zeltra-sdk/src/wallet/note_wallet.rs`)
- [ ] Implementasikan `export_full_viewing_key(&self) -> Result<FullViewingKey, WalletError>`.
- [ ] Implementasikan `create_auditor_session(fvk: FullViewingKey) -> Result<ReadOnlyAuditorSession, WalletError>`.
- [ ] Pastikan `ReadOnlyAuditorSession` dapat memindai saldo dan nullifier on-chain, tetapi menolak secara instan semua panggilan pembuatan proof belanja (`WalletError::SpendAuthorityRequired`).

---

## 3. Matriks Pengujian Unit (`zeltra-sdk/tests/`)

### Positive Tests:
- [ ] `test_audit_report_utxo_conservation_and_reconciliation`: Verifikasi persamaan neraca saldo pas 100% pada simulasi 5 deposit dan 3 spend multi-asset.
- [ ] `test_csv_export_properly_formatted`: CSV output memiliki header lengkap dan dapat di-parse kembali tanpa kehilangan presisi angka desimal.
- [ ] `test_cex_proof_generation_valid`: Berhasil meng-generate `CexProofPayload` lengkap dengan MMR inclusion path dan ZK proof.
- [ ] `test_fvk_auditor_session_recovery`: Sesi auditor yang dibuka hanya dengan FVK berhasil mendeteksi transaksi masuk dan keluar serta menghitung saldo dengan benar.

### Negative Tests (Security & Integrity Guardrails):
- [ ] `test_audit_report_fails_on_unbalanced_record`: Jika ada record dengan selisih fee sengaja disuntikkan, ekspor gagal secara *fail-closed*.
- [ ] `test_csv_sanitizes_malicious_formula`: Field nama organisasi `=cmd|' /C calc'!A0` otomatis disanitasi menjadi `'=cmd|' /C calc'!A0`.
- [ ] `test_export_never_leaks_spending_key`: Inspeksi byte memory pada JSON/CSV memastikan tidak ada spending key yang bocor.
- [ ] `test_auditor_session_cannot_spend`: Upaya memanggil fungsi pembuatan spend proof pada `ReadOnlyAuditorSession` gagal dengan error `SpendAuthorityRequired`.

---

## 4. Strict Engineering Guardrails (Zero-Slop Policy)

1. **NO PLACEHOLDERS:** Dilarang menggunakan `todo!()`, `unimplemented!()`, dummy byte arrays, atau mock signatures di jalur production.
2. **COMPILE-CLEAN:** Wajib lulus `cargo check -p zeltra-sdk` dan `cargo test -p zeltra-sdk` dengan 0 error dan 0 warning.
3. **FORMAT & CLIPPY:** Wajib bersih `cargo fmt --check` dan `cargo clippy -p zeltra-sdk -- -D warnings`.
