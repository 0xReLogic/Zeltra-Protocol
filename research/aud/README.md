# Security & Cryptographic Audit Reports (`research/aud/`)

Folder ini didedikasikan untuk menyimpan seluruh laporan audit keamanan resmi (*Security & Cryptographic Audit Reports*), temuan kerentanan (*Vulnerability Findings*), dan hasil verifikasi formal pihak ketiga untuk Zeltra Protocol.

---

##  Indeks Laporan Audit (AUD)

| ID | Modul / Scope Target | Tanggal | Auditor / Tooling | Status Resolusi | Dokumen |
|---|---|---|---|---|---|
| *Pending* | Smart Contract Stylus (`zeltra-contracts`), Circuits (`zeltra-core`), Relayer (`zeltra-node`) | *Scheduled Q4 2026* | Tier-1 ZK & Smart Contract Auditor | *In Backlog* | - |

---

## Standar Format Temuan Audit (AUD Finding)
Setiap temuan audit yang dicatat di folder ini menggunakan format:
1. **Summary & Severity:** (Critical, High, Medium, Low, Informational)
2. **Affected Code / Gadget:** File, baris, dan komponen terkait.
3. **Attack Vector / Proof of Concept:** Penjelasan celah atau vektor eksploitasi.
4. **Impact:** Dampak terhadap solvensi atau privasi protokol.
5. **Mitigation & Fix Commit:** Solusi teknis dan nomor commit perbaikan.
