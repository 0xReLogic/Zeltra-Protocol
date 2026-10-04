# DEC-026: Receiver-Enforced Modular Compliance, Low-Cost Public Sanctions Screening, and Relayer Exposure Mitigation

- **Status:** Proposed & Accepted (Red-Team Hardened)
- **Author:** Zeltra Protocol Architecture Team
- **Date:** 2026-10-04
- **Impact Areas:** `nimbus-node` (ingress validation, preflight local verification, gas-griefing defense), `nimbus-contracts` (modular association roots, time-bound policy records), `nimbus-sdk` (client ZK proof of membership), `research/compliance`
- **Architectural Paradigm:** *"Permissionless Privacy, Permissioned Acceptance"*
- **Guiding Epistemological Principle:** *"A valid ZK proof proves exactly the statement encoded by the circuit. It does not prove that the policy root is honest, current, correctly issued, or legally sufficient."*

---

## 1. Konteks & Masalah Regulasi (Regulatory Reality & Legal Facts)

### A. Pelajaran Nyata Kasus Hukum Web3 (Tornado Cash & Samourai Wallet)
Pada kasus penegakan hukum federal AS (*United States v. Roman Storm*, SDNY 2025 dan vonis para pendiri Samourai Wallet 2025):
1. Tuntutan pidana federal dapat muncul dari teori konspirasi pengoperasian bisnis pengiriman uang tanpa izin (*conspiracy to operate an unlicensed money transmitting business* di bawah **18 U.S.C. § 1960** yang dikaitkan dengan pasal konspirasi federal seperti **18 U.S.C. § 371**), serta konspirasi pencucian uang:
   - Mengoperasikan kolam anonimitas monolitik (*all-or-nothing pool*) tanpa mekanisme bagi pengguna untuk membuktikan pemisahan dari aktor siber berbahaya (misal: Lazarus Group).
   - Mengambil keuntungan operasional (*fee capture*) dan merelay transaksi yang patut diduga berasal dari tindak pidana.
2. Batasan Hukum Realistis:
   - **Tidak ada kode perangkat lunak yang memberikan kekebalan hukum otomatis (*no automatic legal immunity*).** Tuntutan hukum bersifat sangat spesifik terhadap fakta operasional (*fact-specific*).
   - Tujuan desain arsitektur Zeltra adalah **mengurangi eksposur kustodian dan pengetahuan transaksi privat (*reduces custodial and plaintext private financial state exposure*)**.
3. Keterbatasan Komersial Kepatuhan Terpusat:
   - Melanggan oracle kepatuhan komersial terpusat (Chainalysis, TRM Labs) menelan biaya $30.000 – $100.000+/tahun, memicu ketergantungan terpusat (*vendor lock-in*) yang tidak sesuai dengan fase bootstrapping independen.

---

## 2. Posisi Novelty: Komposisi Arsitektur (Architectural Composition)

Zeltra secara sadar dan transparan menyatakan posisinya sebagai **Inovasi Komposisi Arsitektur (ATMI: Amati, Tiru, Modifikasi, Inovasi)**, bukan penemu primitif ZK baru dari nol:

* **Preseden Kunci yang Diamati:**
  1. *Privacy Pools (Vitalik Buterin et al., 2023/2024):* Membangun paradigma teoretis *Association Sets*. Zeltra mengeksplorasi model eksekusi *receiver-native* untuk menerapkan kebijakan penerimaan heterogen pada saat transaksi.
  2. *RAILGUN Private Proofs of Innocence (POI):* Pembuktian ZK terhadap dataset alamat buruk menggunakan multi-list providers.
* **Diferensiasi Zeltra:**
  - Mengawinkan model ZK-UTXO ($10 - $3 = $7 change note) dengan **Arbitrum Stylus (Rust WASM)** via precompile EIP-2537 (`0x0c` MSM + `0x0f` Pairing).
  - Mendelegasikan kebijakan kepatuhan ke tingkat pihak penerima (*receiver-specified policy*), memisahkan tata kelola protokol dari penetapan moralitas dana.

---

## 3. Doktrin Desain: Tiga Lapisan Kebijakan Kepatuhan (The Three-Layer Decoupled Model)

Kepatuhan dipisahkan secara tegas menjadi tiga lapisan independen:

```text
┌────────────────────────────────────────────────────────────────────────┐
│ LAYER 1: PROTOCOL VALIDITY (Smart Contract Stylus)                     │
│ "Apakah transaksi valid secara kriptografis & solven?"                │
│ - Nilai terkonservasi (Input = Payout + Protocol Fee + Exec + Change)  │
│ - Nullifier belum pernah dibelanjakan (double-spend protection)        │
│ - Input note ada dalam accepted LeanIMT tree Zeltra                    │
└───────────────────────────────────┬────────────────────────────────────┘
                                    │
┌───────────────────────────────────▼────────────────────────────────────┐
│ LAYER 2: RECEIVER ACCEPTANCE POLICY (Payment / Invoice Context)        │
│ "Apakah dana memenuhi standar kepatuhan yang diminta oleh penerima?"   │
│ - Merchant A (Bank/CEX): Mensyaratkan ZK membership pada Clean Root A  │
│ - Merchant B (AI Agent/P2P): Mensyaratkan baseline validitas tanpa ASP │
│ - Target Semantic Binding: policy_root diikat ke (receiver, quote)     │
└───────────────────────────────────┬────────────────────────────────────┘
                                    │
┌───────────────────────────────────▼────────────────────────────────────┐
│ LAYER 3: RELAYER OPERATIONAL POLICY (Ingress Filter & Dispatcher)       │
│ "Apakah transaksi ini aman dieksekusi oleh operator infrastruktur?"    │
│ - Screening sanksi publik aktif (OFAC SDN List di layer HTTP)          │
│ - Perlindungan non-kustodial (Relayer tidak memegang spend key/preimage│
│ - Mengurangi eksposur operasional relayer di bawah 18 U.S.C. § 1960    │
└────────────────────────────────────────────────────────────────────────┘
```

---

## 4. Red-Team Threat Modeling & Mitigasi Operasional (Post-Mortem Hardening)

Berdasarkan tinjauan red-team Tier-1, berikut mitigasi atas celah-celah kritis:

### A. 🔴 Mitigasi C-01: Relayer Gas-Griefing Defense
* **Ancaman:** Penyerang mengirim input publik yang tampak valid dengan **fake/corrupted Groth16 proof**. Jika relayer langsung mem-broadcast transaksi ke blockchain, kontrak Stylus akan me-revert eksekusi, tetapi relayer tetap membayar gas. Penyerang dapat menguras treasury relayer tanpa komputasi ZK yang valid.
* **Patch Arsitektur di `nimbus-node`:**
  1. **Local Groth16 Preflight Verification:** Sebelum transaksi diserahkan ke mempool atau keluar biaya gas 1 wei pun, relayer memverifikasi bukti Groth16 secara lokal di CPU node menggunakan `ark-groth16` dan `NOTE_VK` (~2-3 ms). Transaksi dengan proof tidak valid ditolak di layer HTTP (`400 Bad Request`).
  2. **Dry-Run RPC Simulation (`eth_call`):** Menjalankan simulasi view call sebelum broadcast aktual untuk memastikan status kontrak tidak revert.
  3. **Admission Control:** Pembatasan laju (*rate limiting*) berbasis IP, kuota per-client, dan circuit-breaker batas maksimal gas harian.
* **Invariant I1:** *No invalid or unverified proof may cause the relayer to broadcast or spend gas.*

### B. 🔴 Mitigasi H-02: Stale Clean Root & TOCTOU Protection
* **Ancaman:** Snapshot dataset sanksi bersifat dinamis. Jika alamat baru masuk daftar sanksi setelah sebuah root diterbitkan, pengguna berbahaya dapat menggunakan root lama yang belum kedaluwarsa untuk membelanjakan dana (*stale certificate hopping*).
* **Patch Arsitektur:**
  - Root compliance tidak berupa skalar 32-byte abadi, melainkan membawa metadata waktu kedaluwarsa:
    $$\text{PolicyRootRecord} = \{\text{root}, \text{issuer\_id}, \text{version}, \text{valid\_from}, \text{expires\_at}, \text{signature}\}$$
  - Kontrak atau penerima secara tegas menolak spend jika `block.timestamp > expires_at`. Freshness requirement diatur sesuai toleransi penerima.
* **Invariant I2:** *An expired or revoked policy root cannot satisfy a spend transaction.*

### C. 🔴 Klarifikasi H-04: Membership Statement vs Recursive Lineage
* **Klarifikasi Ilmiah:** Statement sirkuit Zeltra saat ini membuktikan bahwa note yang dibelanjakan adalah **anggota sah dari himpunan komitmen aktif (*membership in accepted set*)**, bukan bukti silsilah rekursif penuh (*recursive provenance lineage*) seperti pada recursive SNARKs Railgun.
* Dokumen dan whitepaper tidak boleh mencampuradukkan *snapshot membership* dengan *complete origin lineage*. Lineage rekursif multi-hop adalah domain riset sirkuit masa depan.
* **Invariant I4:** *Membership statement strictly verifies the circuit-defined relation without conflating set membership with recursive lineage.*

### D. 🟠 Mitigasi H-06: Context-Binding & Replay Immunity
* Untuk mencegah penggunaan kembali bukti kebijakan pada transaksi atau chain berbeda, semantic binding (DEC-022) mengikat:
  $$\text{policy\_digest} = \mathcal{H}(\text{chain\_id}, \text{contract\_address}, \text{recipient}, \text{merchant\_domain}, \text{quote\_hash}, \text{policy\_root}, \text{expiry})$$
* **Invariant I3:** *A proof for payment context A cannot be replayed under payment context B or on a different chain.*

---

## 5. Mitigasi Eksposur Operasional Relayer (Relayer Exposure Mitigation)

1. **Non-Custodial "Dumb Pipe":**
   - Relayer tidak pernah menerima, menyimpan, atau memiliki akses ke *spending key*, *note preimage*, atau data identitas pengguna (*no plaintext private financial state access*).
   - Relayer hanya memproses *proof Groth16 matang* yang telah lulus verifikasi preflight lokal (DEC-025).
2. **Low-Cost Sanctions Screening via Public Data:**
   - Relayer menjalankan sinkronisasi berkala terhadap **OFAC Specially Designated Nationals (SDN) List** publik resmi US Treasury dengan verifikasi integritas hash (*dataset SHA-256 integrity pinning*).
   - Jika parameter `recipient` terdaftar dalam data publik sanksi, relayer menolak transaksi di layer HTTP (`403 Forbidden`).
3. **Stateless Compliance Execution:**
   - Tidak ada database PII pengguna yang disimpan di node relayer maupun kontrak on-chain.

---

## 6. Asumsi Ketergantungan Eksternal (Runtime Dependency Notice)

* **Arbitrum Stylus Runtime:**
  Keamanan, performa, dan ketersediaan komputasi smart contract bergantung pada lingkungan host **ArbOS / Nitro Stylus** serta kebijakan tata kelola *Arbitrum Security Council*. Protokol memperlakukan ketersediaan host precompile EIP-2537 (`0x0c` dan `0x0f`) sebagai fakta runtime/deployment testnet yang diverifikasi secara berkelanjutan, bukan asumsi matematis statis.
* **Klaim Kinerja Gas:**
  Klaim kuantitatif efisiensi gas dan latensi tidak dicantumkan sebagai angka absolut sampai tabel *formal benchmark reproducible* (versi ArbOS, ukuran bytecode WASM, breakdown gas verifikasi dan storage) dipublikasikan secara resmi dari testnet Arbitrum Sepolia.

---

## 7. Roadmap Verifikasi Semantik Kepatuhan

1. **Fase 1 (Current Baseline):**
   - Registry `accepted_note_roots` on-chain, screening sanksi publik di relayer ingress, dan verifikasi preflight lokal anti-griefing.
2. **Fase 2 (Receiver-Bound Policy Statement):**
   - Pengikatan sirkuit Groth16 secara kriptografis:
     $$\text{policy\_root} == \mathcal{H}(\text{recipient}, \text{quote\_hash}, \text{merchant\_domain})$$
   - Memungkinkan penerima dana menentukan acceptance policy secara permissionless tanpa gate global administrator.

---

## 8. Referensi Literatur Akademis & Paper Terkait (Citations)

1. **zkAML: Zero-knowledge Anti Money Laundering in Smart Contracts with Whitelist Approach**
   - *Penulis:* Donghwan Oh, Semin Han, Jihye Kim, Hyunok Oh, Jiyeal Chung, Jieun Lee, Hee-jun Yoo, Tae wan Kim (Hanyang Univ, Kookmin Univ, Zkrypto, Bank of Korea).
   - *Publikasi:* IACR Cryptology ePrint Archive, Report **2025/465** (Maret 2025).
   - *Arsip Lokal:* [`jurnal/pdf/2025-465.pdf`](file:///workspaces/Zeltra-Protocol/jurnal/pdf/2025-465.pdf).
2. **Blockchain Privacy and Regulatory Compliance: Towards a Practical Equilibrium**
   - *Penulis:* Vitalik Buterin, Jacob Illum (Chainalysis), Matthias Nadler, Fabian Schär (University of Basel), Ameen Soleimani (Privacy Pools).
   - *Publikasi:* Journal of *Blockchain: Research and Applications*, Vol. 5, Issue 2 (2024).
3. **SyRA: Sybil-Resilient Anonymous Signatures with Applications to Decentralized Identity**
   - *Penulis:* Elizabeth Crites, Aggelos Kiayias, Markulf Kohlweiss, Amirreza Sarencheh (Web3 Foundation, Univ of Edinburgh, IOG).
   - *Publikasi:* IACR Cryptology ePrint Archive, Report **2024/379** (Agustus 2025 rev).
   - *Arsip Lokal:* [`jurnal/pdf/2024-379.pdf`](file:///workspaces/Zeltra-Protocol/jurnal/pdf/2024-379.pdf).
4. **zkKYC in DeFi: An approach for implementing the zkKYC solution concept in Decentralized Finance**
   - *Penulis:* Pieter Pauwels, Joni Pirovich, Peter Braunz, Jack Deeb (BADASL, Mycelium).
   - *Publikasi:* IACR Cryptology ePrint Archive, Report **2022/321** (Maret 2022).
   - *Arsip Lokal:* [`jurnal/pdf/2022-321.pdf`](file:///workspaces/Zeltra-Protocol/jurnal/pdf/2022-321.pdf).
