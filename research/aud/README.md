# Security & Cryptographic Audit Reports (`research/aud/`)

Folder `research/aud/` adalah repositori sentral dan katalog master untuk seluruh laporan audit keamanan resmi (*Security Assessment Reports*), investigasi kerentanan (*Vulnerability Findings*), pembuktian eksploitasi empiris (*Proof-of-Concept / PoC*), serta verifikasi formal untuk **Zeltra Protocol**.

Setiap laporan audit wajib menyertakan **metodologi evaluasi**, **PoC pengujian empiris (rasio $\ge 2\times$ negative tests)**, dan **benchmark komparasi performa**. Format laporan menggunakan template resmi: [`TEMPLATE.md`](TEMPLATE.md).

---

## Master Indeks Laporan Audit (AUD & SEC)

| ID | Dokumen / Rujukan | Domain | Judul Temuan & Ringkasan | Severity | Auditor / Tim Penilai | Status | Tanggal |
|---|---|---|---|---|---|---|---|
| **SEC-001** | [`SEC-001-null-branch-soundness-poc.md`](SEC-001-null-branch-soundness-poc.md) | `zeltra-core` | Null-Branch Floating Witness Attack pada R1CS JoinSplit Circuit | **CRITICAL** | Security Research Team | `VERIFIED_CLOSED` | 2026-10-24 |
| **SEC-002** | [`SEC-002-selective-disclosure-unconstrained-owner-key.md`](SEC-002-selective-disclosure-unconstrained-owner-key.md) | `zeltra-core` | Underconstrained Public Input & Missing Identity Binding in SelectiveDisclosureCircuit | **CRITICAL** | Security Research Team | `VERIFIED_CLOSED` | 2026-10-25 |
| **AUD-001** | *In Preparation* | `Full-Stack` | Full-Scope Protocol Security Audit (Circuits, Stylus Contracts, Relayer, SDK) | *Scheduled* | Tier-1 Security Audit Firm | `SCHEDULED` | *Q4 2026* |

---

## Katalog Audit Berdasarkan Domain Arsitektur

### 1. Sirkuit ZK & Kriptografi (`zeltra-core`)
Fokus: Sinyal tak berkendala (*underconstrained*), manipulasi saksi privat (*witness malleability*), *finite field modular overflow*, kekakuan Jacobian (*tangent rigidity*), dan kendala dekomposisi bit.
- **SEC-001**: Null-Branch Floating Witness Attack & Companion Constraints Verification.
- **SEC-002**: Missing Identity-to-SpendingKey Binding & Underconstrained Public Input in Selective Disclosure.

### 2. Smart Contract Arbitrum Stylus (`zeltra-contracts`)
Fokus: Solvensi multi-asset (*contract balance $\ge$ total liabilities*), integrasi precompile EIP-2537 (`0x0c` MSM & `0x0f` pairing), *re-entrancy*, *storage layout collision*, dan profiling gas Stylus WASM.
- *Laporan mendatang dijadwalkan pada milestone AUD-001.*

### 3. Relayer Sequencer & Ingress Pipeline (`zeltra-node`)
Fokus: *Mempool double-spend race condition*, pemalsuan tanda tangan EIP-712 quote, serangan *griefing* kuota gas, konsistensi transaksi SQLite/SQLCipher WAL, dan proteksi DoS.
- *Laporan mendatang dijadwalkan pada milestone AUD-001.*

### 4. Client SDK & Private Note Wallet (`zeltra-sdk`)
Fokus: De-anonimisasi korelasi pada *knapsack coin selection*, stabilitas alokasi memori in-browser Web Worker WASM (*anti-OOM*), dan efisiensi pemindaian memo via View Tag.
- *Laporan mendatang dijadwalkan pada milestone AUD-001.*

### 5. CLI Tooling & Kepatuhan CEX (`zeltra-cli`)
Fokus: Netralisasi serangan *CSV formula injection* pada laporan audit CEX, verifikasi pembukaan selektif (*selective disclosure*), dan pembersihan kunci di memori (*zeroization*).
- *Laporan mendatang dijadwalkan pada milestone AUD-001.*

---

## Klasifikasi Tingkat Keparahan (Severity Matrix CVSS 3.1)

| Severity | Definisi Teknis & Kriteria Dampak | Target SLA Resolusi |
|---|---|---|
| **CRITICAL** | Pembobolan dana pool langsung, pemalsuan bukti ZK valid dari saksi palsu (*soundness break*), atau pencurian kunci belanja privat pengguna di SDK. | **Immediate Blocker** (Freeze & hotfix sebelum deploy) |
| **HIGH** | Kerusakan soundness bersyarat, potensi dana terkunci permanen (*silent lockup*), race condition mempool double-spend, atau reentrancy pada kontrak. | **Pre-Release Blocker** (Wajib selesai sebelum rilis publik) |
| **MEDIUM** | Underconstrained wire yang terlindung guard sekunder, de-anonimisasi parsial pola koin SDK, atau degradasi performa sinkronisasi MMR yang parah. | **Active Sprint** (Dijadwalkan dalam siklus perbaikan aktif) |
| **LOW** | Sanitasi calldata/input non-kritis, inkonsistensi error handling minor, atau format log event scanner yang kurang optimal. | **Routine Maintenance** (Diperbaiki pada rilis berkala) |
| **INFORMATIONAL** | Optimasi penghematan gas Stylus, pembersihan wire R1CS redundan, atau peningkatan efisiensi kueri database relayer. | **Backlog / Code Quality** (Rekomendasi arsitektur) |

---

## Panduan Menulis Laporan Audit Baru

Setiap laporan temuan kerentanan (`SEC-XXX`) atau laporan audit menyeluruh (`AUD-XXX`) wajib mengikuti format resmi:

1. **Gunakan Template:** Gandakan [`TEMPLATE.md`](TEMPLATE.md) menjadi `research/aud/AUD-XXX-[judul].md` atau `SEC-XXX-[judul].md`.
2. **Sertakan Bukti Pengujian (PoC):** Wajib menyertakan kode eksploitasi mandiri dan rasio pengujian:
   $$\text{Jumlah Adversarial Negative Tests} \ge 2 \times \text{Jumlah Positive Tests}$$
   Serta membuktikan bahwa pada pengujian negatif, transaksi di-revert secara *fail-closed* tanpa ada mutasi state ledger (**Zero-State Mutation**).
3. **Sertakan Benchmark Empiris:** Wajib mengukur metrik sebelum vs sesudah perbaikan (misal: selisih kendala R1CS, profil gas Stylus, throughput RPS relayer, memori WASM, atau uji statistik waktu konstan `dudect` $|t| < 4.5$).
4. **Daftarkan ke Indeks:** Tambahkan entri dokumen baru pada tabel **Master Indeks Laporan Audit** di atas.
