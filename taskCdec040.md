# TASK C (DEC-040C): CEX Selective Disclosure Verifier Engine & Compliance CLI Tooling (`zeltra-cli` & `zeltra-sdk`)

> **Scope:** `zeltra-cli/` (Clap Subcommands & Handlers) & `zeltra-sdk/` (Standalone Verifier Engine)  
> **Spesifikasi Referensi:** [`research/decisions/DEC-040C-cex-verifier-engine-and-compliance-cli.md`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-040C-cex-verifier-engine-and-compliance-cli.md) & [`research/decisions/DEC-040-viewing-key-export-suite-and-cex-selective-disclosure.md`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-040-viewing-key-export-suite-and-cex-selective-disclosure.md)  
> **Prinsip:** *"Stateless Verifier Autonomy, Strict Verifier Binding, Cryptographic Proof of Innocence"*  
> **Status:** **PENDING IMPLEMENTATION**

---

## 1. Latar Belakang & Tujuan Arsitektur

Menyediakan perkakas baris perintah (`zeltra-cli`) dan mesin verifikasi independen (*Standalone Verifier Engine*) yang memungkinkan staf kepatuhan bursa (seperti Binance, Coinbase, Kraken) atau auditor eksternal untuk memvalidasi keaslian berkas bukti sumber dana (`zeltra_cex_proof_v1.json`) secara deterministik tanpa perlu memiliki dompet Zeltra, tanpa rahasia privat, hanya dengan koneksi RPC ke Arbitrum.

---

## 2. Checklist Implementasi Rinci

### A. Standalone CEX Verifier Engine (`zeltra-sdk/src/compliance/verifier.rs`)
- [ ] **Implementasi Algoritma Verifikasi 5 Tahap:**
  - **Tahap 1: Validasi Format & Re-komputasi Komitmen:**
    - Validasi `asset_id <= 1` dan `raw_circuit_value <= u64::MAX`.
    - Hitung `value_packed = value + asset_id * 2^64` (DEC-034A).
    - Hitung $C' = \text{Poseidon\_W5}([value\_packed, owner\_key, rho, randomness])$.
    - Pastikan $C' == C$ (Kembalikan `CommitmentPreimageMismatch` jika beda).
  - **Tahap 2: Verifikasi Jalur Merkle Mountain Range (MMR, DEC-032):**
    - Hitung puncak sub-tree dari $C$ dan `sibling_path` berdasarkan `leaf_index`.
    - Lakukan *peak bagging* terhadap `mmr_peaks` untuk mendapatkan `computed_root`.
    - Pastikan `computed_root == mmr_root` (Kembalikan `InvalidMmrInclusionProof` jika beda).
    - Query RPC kontrak Stylus `is_accepted_note_root(mmr_root)` untuk memastikan root sah on-chain.
  - **Tahap 3: Verifikasi Verifier Binding & Kedaluwarsa:**
    - Pastikan `expected_verifier_id == verifier_binding.verifier_id` (Kembalikan `VerifierIdMismatch`).
    - Pastikan waktu saat ini $\le \text{expires\_at}$ (Kembalikan `ProofExpired`).
    - Hitung ulang $\text{Challenge} = \text{Blake2s-256}(\dots)$ dan pastikan cocok dengan `challenge_hash`.
  - **Tahap 4: Verifikasi Bukti Groth16 NIZK (SelectiveDisclosureCircuit):**
    - Dekomposisi challenge menjadi 128-bit `challenge_hi` dan `challenge_lo`.
    - Verifikasi bukti Groth16 terhadap Public Inputs `[owner_key, rho, nullifier, challenge_hi, challenge_lo]`.
  - **Tahap 5: Verifikasi Status On-Chain Transaksi Pencairan:**
    - Query RPC kontrak Stylus `is_nullifier_spent(nullifier)` untuk memastikan status spent.
    - Query RPC tx receipt untuk memastikan `spent_tx_hash` mentransfer aset `asset_id` ke `payout_recipient`.

### B. Parser Perintah Clap (`zeltra-cli/src/cli.rs`)
- [ ] **Subcommand `export-audit`:**
  - `format`: String (`json` atau `csv`, default: `json`).
  - `asset`: String (`all`, `usdc`, atau `weth`, default: `all`, short flag `-A`).
  - `from`: Option<String> (Format: `YYYY-MM-DD`).
  - `to`: Option<String> (Format: `YYYY-MM-DD`).
  - `out`: String (Path berkas output).
- [ ] **Subcommand `export-cex-proof`:**
  - `commitment`: String (Heksadesimal 32-byte komitmen note).
  - `verifier`: String (Identifier verifikator, misal: `"BINANCE_KYC_DESK"`).
  - `account_uid`: String (Identifier akun pengguna di bursa, misal: `"USER-98124712"`).
  - `spent_tx`: String (Hash transaksi pengeluaran on-chain).
  - `recipient`: String (Alamat deposit pengguna di bursa).
  - `out`: String (Path berkas output).
- [ ] **Subcommand `verify-cex-proof`:**
  - `file`: String (Path berkas `zeltra_cex_proof_v1.json`).
  - `expected_verifier`: String (Identifier bursa pemeriksa).
  - `rpc`: Option<String> (URL RPC Arbitrum Sepolia/One).
- [ ] **Subcommand `export-viewing-key`:**
  - `key_type`: String (`ivk`, `ovk`, atau `fvk`, default: `fvk`, short flag `-t`).
  - `out`: String (Path berkas output).

### C. Handler Perintah CLI (`zeltra-cli/src/commands/compliance.rs`)
- [ ] `pub async fn handle_export_audit(...) -> Result<(), CliError>`:
  - Buka wallet terenkripsi lokal, kueri riwayat note, hasilkan `AuditReport`, format ke JSON/CSV, simpan ke file dengan permission `0600`.
- [ ] `pub async fn handle_export_cex_proof(...) -> Result<(), CliError>`:
  - Buka wallet lokal, ambil preimage komitmen, generate ZK proof via `SelectiveDisclosureCircuit`, hitung binding challenge, simpan ke file.
- [ ] `pub async fn handle_verify_cex_proof(...) -> Result<(), CliError>`:
  - Parse berkas JSON, hubungkan ke RPC, jalankan `CexProofVerifier::verify`, cetak ringkasan verifikasi warna-warni yang jelas ke console.
- [ ] `pub async fn handle_export_viewing_key(...) -> Result<(), CliError>`:
  - Ekspor kunci sesuai tipe yang diminta ke file teks hex.

### D. Keamanan & Sanitasi Lingkungan Eksekusi
- [ ] Pastikan tidak ada kunci privat atau spending key yang bocor ke stdout/stderr.
- [ ] File output dibuat dengan izin akses ketat (`0600`).
- [ ] Fail-closed pada kegagalan koneksi RPC.

---

## 3. Matriks Pengujian Unit & Integrasi (`zeltra-cli/tests/` & Python Suite)

### Positive Tests:
- [ ] `test_cli_export_audit_json_roundtrip`: Perintah CLI `export-audit -f json` menghasilkan berkas JSON yang valid dan dapat di-parse kembali.
- [ ] `test_cli_export_audit_csv_roundtrip`: Perintah CLI `export-audit -f csv` menghasilkan berkas CSV dengan sanitasi formula injection.
- [ ] `test_cli_export_and_verify_cex_proof_roundtrip`: Alur lengkap end-to-end: ekspor bukti CEX lalu verifikasi dengan `verify-cex-proof` mengembalikan status `SUCCESS`.
- [ ] `test_cli_export_viewing_key_fvk`: Perintah `export-viewing-key -t fvk` menghasilkan file kunci FVK yang dapat diimpor kembali.

### Negative Tests (Security & Integrity Guardrails):
- [ ] `test_verify_cex_proof_fails_on_tampered_json`: Mengubah angka nominal di file JSON menyebabkan verifikasi gagal pada tahap re-komputasi komitmen.
- [ ] `test_verify_cex_proof_fails_on_wrong_verifier`: Verifikasi gagal jika `--expected-verifier` tidak cocok dengan yang tertera di bukti.
- [ ] `test_verify_cex_proof_fails_on_expired_timestamp`: Bukti dengan timestamp yang sudah kedaluwarsa ditolak seketika.
- [ ] `test_verify_cex_proof_fails_on_invalid_mmr`: Komitmen yang tidak ada di pohon MMR on-chain ditolak.

---

## 4. Strict Engineering Guardrails (Zero-Slop Policy)

1. **NO PLACEHOLDERS:** Dilarang menggunakan `todo!()`, `unimplemented!()`, dummy byte arrays, atau mock signatures di jalur production.
2. **COMPILE-CLEAN:** Wajib lulus `cargo check -p zeltra-cli` dan `cargo test -p zeltra-cli` dengan 0 error dan 0 warning.
3. **FORMAT & CLIPPY:** Wajib bersih `cargo fmt --check` dan `cargo clippy -p zeltra-cli -- -D warnings`.
