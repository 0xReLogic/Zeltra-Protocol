# DEC-031: Relayer Three-Way Reconciliation, Privacy-Preserving Solvency Observability, and Strict Fail-Closed Quote Domain Binding

- **Status:** Proposed & Accepted (Architecture Core Decision)
- **Author:** Zeltra Protocol Architecture & Security Team
- **Date:** 2026-10-08
- **Applies to:** `nimbus-node/src/execution_fee_claimer.rs`, `nimbus-node/src/handlers/quote.rs`, `nimbus-node/src/handlers/health.rs`, `nimbus-node/src/handlers/spend.rs`, `nimbus-node/src/evm_client.rs`, `nimbus-node/src/dto.rs`, `nimbus-node/src/database.rs`
- **Related DECs:** DEC-016 (ZK-UTXO Change Ledger), DEC-017 (Receipt Finality & Nonce Tracking), DEC-018 (On-Chain Indexer), DEC-020 (Batch Profitability), DEC-022 (Proof Settlement Mismatch), DEC-025 (Relayer ZK Note Spend Settlement), DEC-028 (Public Input Domain Hardening)
- **Architectural Paradigm:** *"Triangulated Solvency, Fail-Closed Domain Isolation, Zero User-Data Leakage Observability"*
- **Guiding Principle:** *"A relayer must never claim a single base unit of fees that is not triply verified across on-chain contract state, local database accruals, and confirmed transaction receipts; it must never issue an unverified quote as OK; and it must prove aggregate solvency without exposing user identity or transaction graph."*

---

## 1. Executive Summary & Kajian Postmortem (2024–2026)

Menindaklanjuti penyelesaian Phase 1 Gate F (Direct Settlement Pipeline ZK-UTXO di [`DEC-025`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-025-relayer-zk-note-spend-settlement-atomic-batching-and-reconciliation.md)) dan implementasi pengerasan domain pada [`DEC-028`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-028-zk-public-input-domain-hardening-flat-fee-unification-and-compliance-scoping.md), kami melakukan evaluasi forensik mendalam terhadap insiden eksploitasi, laporan audit, dan postmortem kegagalan rekonsiliasi bridge/relayer sepanjang 2024 hingga awal 2026.

### A. Insiden Polymarket Settlement Desync (19 Februari 2026)
- **Pola Kegagalan:** Terjadinya *synchronization gap* antara status eksekusi off-chain (acknowledgement di tingkat API order-matching) dan status finalitas settlement on-chain (transaksi Polygon).
- **Akar Masalah:** Komponen off-chain menandai order sebagai selesai dan memperhitungkan fee sebelum transaksi on-chain memperoleh konfirmasi finalitas melalui transaction receipt. Saat transaksi on-chain mengalami revert atau gagal akibat nonce race, sistem off-chain mengalami *state drift* yang memicu ketidakseimbangan modal (directional exposure & phantom balances).
- **Pelajaran untuk Zeltra:** Relayer tidak boleh mengakui akrual fee atau mengajukan klaim eksekusi fee sebelum tanda terima transaksi (*transaction receipt*) terkonfirmasi sukses (`status == 1`) di rantai Arbitrum.

### B. Laporan OpenZeppelin 2024–2026: Curve ETH/OETH Fee Drift & Paymaster Sponsor Desync
- **Pola Kegagalan:** Fungsi penarikan admin fee (`withdraw_admin_fees`) dan paymaster gas sponsorship menghitung hak klaim dari selisih tracking internal off-chain/in-memory terhadap saldo on-chain tanpa melakukan rekonsiliasi tiga arah.
- **Akar Masalah:** Terjadinya drift kumulatif akibat rounding error dan transaksi revert yang tetap tercatat di counter lokal. Ketika hak klaim ditarik melampaui cadangan likuiditas riil, kontrak memicu invariant violation (`contract_assets < total_liabilities`) atau revert karena dana tidak mencukupi (`INSUFFICIENT_ACCUMULATED_FEES`).
- **Pelajaran untuk Zeltra:** Klaim biaya eksekusi relayer harus direkonsiliasi secara fail-closed terhadap saldo akrual on-chain aktual (`contract.accrued_execution_fee_liability`). Jumlah klaim harus merupakan fungsi `min(db_verified_receipts, contract_accrued)` dan tidak boleh pernah melebihi salah satunya.

### C. Kerentanan EIP-712 Domain Downgrade & Unbound Quote Replay (2026)
- **Pola Kegagalan:** Quote endpoint mengembalikan respon berstatus `status: "OK"` meskipun hash EIP-712 (`domain_separator` dan `struct_hash`) tidak dapat dibuat (karena RPC atau klien blockchain tidak aktif).
- **Akar Masalah:** Klien menerima quote yang tampak valid secara sintaksis, namun transaksi yang disusun tidak terikat pada domain `chain_id` dan `contract_address` target. Ini membuka ruang serangan *cross-chain replay* atau kebingungan tanda tangan (*signature confusion*).
- **Pelajaran untuk Zeltra:** Endpoint quote wajib *fail-closed*. Jika domain EIP-712 tidak dapat dihitung atau klien blockchain tidak terkonfigurasi, relayer **DILARANG** mengembalikan `status: "OK"`. Status yang dikembalikan wajib `status: "ERROR"` dengan pesan kegagalan domain yang eksplisit.

---

## 2. Tiga Pilar Arsitektur DEC-031

```text
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                                   DEC-031 ARCHITECTURE                                  │
├───────────────────────────────┬─────────────────────────────┬───────────────────────────┤
│    Pilar 1: Fail-Closed       │   Pilar 2: Three-Way        │    Pilar 3: Privacy       │
│    Quote Domain Binding       │   Reconciliation Engine     │    Solvency Observability │
├───────────────────────────────┼─────────────────────────────┼───────────────────────────┤
│ • Hapus status "OK" fallback  │ • 1. Kontrak On-Chain       │ • Endpoint /health        │
│ • Validasi EIP-712 domain     │ • 2. Database SQLite        │ • Invariant Assets >= Liab│
│ • Domain separator & struct   │ • 3. Confirmed Receipts     │ • Zero User Data Leak     │
│   hash wajib ada              │ • min(db_fee, contract_fee) │ • Aggregated metrics only │
└───────────────────────────────┴─────────────────────────────┴───────────────────────────┘
```

---

## 3. Pilar 1: Penghapusan Fallback Quote Tanpa Signing Domain

### A. Masalah Status Quo
Pada `nimbus-node/src/handlers/quote.rs`, ketika `state.evm_client` bernilai `None` atau gagal memanggil `client.chain_id().await`, kode mengeksekusi `fallback_quote_response` yang mengembalikan:
```json
{
  "status": "OK",
  "domain_separator": null,
  "struct_hash": null,
  "message": "Quote generated (EIP-712 hashes unavailable)"
}
```
Ini melanggar prinsip *fail-closed* dan membuka celah di mana klien mengira quote dapat digunakan, padahal sirkuit ZK-UTXO (DEC-028 Fix R3 & R4) dan relayer ingress mewajibkan pembuktian domain target yang valid.

### B. Solusi & Spesifikasi
1. **Pemusnahan Fallback "OK":** Struktur `FallbackQuoteParams` dan fungsi `fallback_quote_response` dihapus sepenuhnya dari kode produksi.
2. **Kueri Domain Fallback Graceful:** Jika klien EVM belum siap, relayer memeriksa konfigurasi environment (`NIMBUS_CHAIN_ID` dan `NIMBUS_CONTRACT_ADDRESS`). Jika parameter domain valid tersedia, relayer menghitung `domain_separator` dan `struct_hash` resmi.
3. **Fail-Closed Rejection:** Jika domain EIP-712 tetap tidak dapat dihitung, handler mengembalikan `error_response(...)` dengan:
   - `status: "ERROR"`
   - `domain_separator: None`, `struct_hash: None`
   - `message: "EIP-712 signing domain unavailable: target chain and contract domain required"`

---

## 4. Pilar 2: Mesin Rekonsiliasi Tiga Arah (Three-Way Reconciliation)

### A. Segitiga Kebenaran Finansial
Sebelum relayer mencairkan execution fee melalui `claim_execution_fees(amount)`, worker wajib mencocokkan tiga sumber data:
1. **On-Chain Contract State:** Membaca `accrued_execution_fee_liability()` dan `get_accumulated_fees()` langsung dari smart contract Stylus.
2. **Local Database Accrual:** Menghitung total execution fee yang tercatat pada transaksi spend (baik multi-item batch di `spend_batches` maupun single spend ZK-UTXO).
3. **Confirmed Transaction Receipts:** Memvalidasi bahwa setiap hash transaksi yang diklaim telah memiliki receipt on-chain dengan `status == 1` (sukses) dan block confirmation yang cukup.

### B. Aturan Klaim & Konservasi Solvensi
- **Formula Klaim:**
  $$\text{claim\_amount} = \min(\text{verified\_receipt\_fees}, \text{contract\_accrued\_liability})$$
- **Proteksi Revert:** Jika $\text{contract\_accrued\_liability} == 0$ atau $\text{claim\_amount} == 0$, relayer menahan transaksi klaim untuk mencegah kegagalan revert `INSUFFICIENT_ACCUMULATED_FEES`.
- **Deteksi Drift:** Jika terdapat deviasi antara DB dan kontrak:
  $$\Delta_{\text{drift}} = |\text{contract\_accrued} - \text{db\_unclaimed}|$$
  Sistem memicu peringatan diagnostik dan mempublikasikannya ke metrik observabilitas tanpa menghentikan liveness sistem yang solvent.
- **Pencatatan Single Spends:** Setiap ZK-UTXO spend individual yang berhasil dikonfirmasi di on-chain dicatat ke tabel metrik eksekusi (`spend_batches` dengan `item_count = 1`), menjamin simetri 100% antara spend individual dan batching.

---

## 5. Pilar 3: Observabilitas Solvensi Tanpa Kebocoran Privasi (Zero-Leakage Observability)

### A. Prinsip Kerahasiaan Pengguna
Dalam sistem pembayaran privat ZK-UTXO, transparansi solvensi protokol adalah keharusan, namun privasi pengguna tidak boleh dikompromikan:
- **DILARANG KERAS mengekspos:** Nullifier, note commitment, session ID, alamat penerima, nilai individual note, atau hash data transaksi privat.
- **HANYA mengekspos:** Metrik agregat protokol tingkat makro yang membuktikan kecukupan agunan (*mathematical solvency*).

### B. Skema Metrik Solvensi pada `/health`
Payload endpoint `/health` diperluas dengan field `solvency_metrics`:
```json
{
  "status": "OK",
  "queued_transactions": 0,
  "processed_nullifiers": 142,
  "relayer_wallet_balance_eth": 2.45,
  "relayer_accumulated_profit_usdc": 18.50,
  "solvency_metrics": {
    "solvency_status": "SOLVENT",
    "contract_balance_usdc": 100000000,
    "total_liabilities_usdc": 99500000,
    "user_note_liability_usdc": 95000000,
    "refundable_deposit_liability_usdc": 4000000,
    "contract_accrued_execution_fee_usdc": 500000,
    "db_unclaimed_execution_fee_usdc": 500000,
    "reconciliation_drift_usdc": 0,
    "is_solvent": true,
    "is_accounting_matched": true
  }
}
```

### C. Klasifikasi Status Solvensi
1. `SOLVENT`: `contract_balance >= total_liabilities` DAN `reconciliation_drift == 0`.
2. `MISMATCH_WARNING`: `contract_balance >= total_liabilities` TETAPI `reconciliation_drift != 0`.
3. `INSOLVENT_ALERT`: `contract_balance < total_liabilities` (Pelanggaran Invariant Kritis).
4. `DEGRADED`: RPC blockchain tidak dapat dijangkau untuk membaca saldo atau liabilities.

---

## 6. Matrix Pengujian & Kriteria Selesai

1. **Unit & Integration Tests:**
   - Test penolakan quote tanpa domain separator mengembalikan `status: "ERROR"`.
   - Test kalkulasi quote dengan fallback env variables mengembalikan `status: "OK"` lengkap dengan `domain_separator` dan `struct_hash`.
   - Test claim worker three-way reconciliation dengan skenario exact match, DB < Kontrak, DB > Kontrak, dan receipt unconfirmed.
   - Test health check solvency metrics: memverifikasi output bebas dari kebocoran data pengguna (zero-leakage assertion).
2. **Invariants:**
   - Tidak ada quote tanpa signing domain yang berstatus "OK".
   - Tidak ada klaim fee yang melebihi akrual on-chain.
   - `is_solvent == true` menjamin $\text{Assets} \ge \text{Liabilities}$.

---

## 7. Referensi & Sitasi Akademik / Forensik (2024–2026)

1. **Polymarket Settlement Desync Forensics:**
   Chrispy / Spearbit Security. *"Polymarket Feb 19 2026 sync/settlement desync incident context pack: Synchronization gaps between off-chain order execution and on-chain settlement finality"*. GitHub Gist & Incident Postmortem (February 2026).
   URL: https://gist.github.com/chrispyspearbit/e08af8a6434db5e2c743be2f1c219c93
2. **Internal Accounting vs On-Chain Balance Drift:**
   OpenZeppelin Security. *"Web3 Security Auditor's 2024 Rewind: Curve ETH/OETH withdraw_admin_fees discrepancy, Paymaster gas credit drift, and cumulative rounding vulnerabilities"*. OpenZeppelin News & Research (October 2025 – 2026).
   URL: https://www.openzeppelin.com/news/web3-security-auditors-2024-rewind
3. **Triangulated On-Chain / Off-Chain Accounting Reconciliation:**
   Allium Data & Formance Engineering. *"Onchain Data Reconciliation for Accounting: Concepts, Methods, and System Design Across L2 Blockchains"*. Allium Research Blog (February 3, 2026).
   URL: https://www.allium.so/blog/onchain-data-reconciliation-for-accounting-concepts-methods-and-system-design
4. **Stablecoin Solvency & Triple-Source Attestation:**
   Eco Engineering & Support. *"Stablecoin Accounting Reconciliation: Three Data Sources Triangulation (Internal Ledger, Onchain Reality, and Settlement Receipts)"*. Eco Technical Support Documentation (July 2026).
   URL: https://eco.com/support/en/articles/14710441-stablecoin-accounting-reconciliation
5. **EIP-712 Domain Separation & Signature Replay Defense:**
   OpenZeppelin Contracts Documentation & Shattered Security. *"Cryptography and EIP-712: Typed Structured Data Hashing, Domain Separator Binding, and Signature Replay Attack Testing"*. OpenZeppelin v5.x / Foundry Research (2025–2026).
   URL: https://docs.openzeppelin.com/contracts/5.x/api/utils/cryptography
6. **Relayer Operational Health & Balance Observability:**
   OpenZeppelin Defender. *"Relayers Module Architecture: In-Flight Transaction Lifecycle, Group Health Monitoring, and Automated Recovery"*. OpenZeppelin Docs (2025–2026).
   URL: https://docs.openzeppelin.com/defender/module/relayers
7. **L2 Multi-System Settlement Drift:**
   Fortress Accounting. *"On-Chain Reconciliation Services for L2 Blockchains: Bridge, Swap, and Failed Retry Tracking"*. Fortress Technical Papers (August 2026).
   URL: https://fortress-accounting.com/on-chain-reconciliation/
8. **State of Web3 Security & Settlement Vulnerability Trends:**
   Sherlock Web3 Security. *"The Sherlock Web3 Security Report Q1 2026: Logic Errors, Cross-Contract Desync, and Infrastructure Exposure in Modern Protocols"*. Sherlock Research (April 2026).
   URL: https://sherlock.xyz/post/the-sherlock-web3-security-report-q1-2026-every-major-hack-exploit-and-trends

