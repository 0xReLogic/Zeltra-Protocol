# Master Blueprint & Laporan Audit Inovasi Nimbus Protocol

**Status:** Dokumen Konsolidasi Definitif (100% Parameter-Aligned)  
**Fondasi Inti:** BLS12-381 + ZK-UTXO + Arbitrum Stylus (Rust) + Stateless Guardian 3-of-5  
**Papers Discan:** 458 | **Approved:** 14 | **Rejected:** 4

---

## I. FONDASI ARSITEKTUR RESMI NIMBUS PROTOCOL

| Parameter | Kepatuhan Wajib |
|---|---|
| **Kurva Kriptografi** | BLS12-381 Pairing-Friendly Curve (bukan BN254, bukan Curve25519) |
| **Model Akuntansi** | ZK-UTXO Model (Note Commitment + Nullifier) |
| **Peran Guardian** | Murni Stateless Threshold Blind Signer 3-of-5 |
| **Blind Scope** | Guardian lihat blinded message & masking commitment; buta nominal, identitas, recipient |
| **Kontrak Stylus** | 5 fungsi: Vault, Merkle Tree Registry, Nullifier Registry, Pairing Verifier, Liability Tracking |
| **Target Pasar** | Enterprise/B2B ($10M+), Retail Konsumen ($10-$500), AI Agents ($0.05) |
| **Model Gasless** | Relayer menalangi gas, tarik execution_fee (gas + 15% markup) |
| **Penyimpanan Note** | 100% Client-Side, zeroized on drop |
| **Filosofi Kepatuhan** | Netral secara protokol (policy-agnostic) |
| **Siklus Transaksi** | 6 Tahap: Deposit → Blind Sign → Reveal k → Unmask → Spend → Settlement |

---

## II. MATRIKS KONSOLIDASI AUDIT INOVASI

| No | Inovasi & Paper | Status | Dampak Arsitektur |
|---|---|---|---|
| #1 | Budgeted Threshold Signatures (2026/2176) | **APPROVED** | Velocity/rate limit pada layer signing Guardian tanpa rusak blind |
| #2 | zkAgent / DeepProve (2026/199) | **PIVOT** | ZK inferensi LLM penuh dibuang; adopsi zkTLS untuk verifikasi faktur API |

| #4 | ZK Email NFA Regex (2026) | **APPROVED** | Social Recovery & Otorisasi Domain B2B via sirkuit NFA efisien |

| #6 | Improved Issuer-Hiding BBS+ (2026/555) | **APPROVED** | Kredensial anonim multi-penerbit $O(1)$ on-chain dengan Accountable Issuer Hiding |
| #7 | NI-DKG on Blockchain (2026/552) | **APPROVED** | Perekrutan/rotasi kuorum Guardian on-chain tanpa fase sengketa |

| #10 | Coral-CFG Proofs (2025/1420) | **APPROVED** | Parser ZK untuk struktur JSON/API B2B di level SDK klien |



| #17 | Device-Binding Anonymous Credentials (2026/965) | **APPROVED** | Credential terikat ke secure hardware element device; anti-malware/credential theft |


---

## III. CETAK BIRU INOVASI YANG DISETUJUI

### #1 — Budgeted Threshold Signatures (BTS): Velocity Limiting Layer

**Paper:** `Budgeted-Threshold-Signatures.md` (2026/2176)  
**Status:** APPROVED — Prioritas Utama

**Realitas Teknis:**
Guardian Nimbus tidak boleh lihat nominal saldo. BTS difungsikan untuk **velocity/rate limiting**, bukan value limiting.

**Mekanisme:**
- Pengguna request signature dengan tag kuota epoch ("Epoch 24H, Tiket ke-N")
- Guardian cluster (3-of-5) reject jika exceed quota
- Transaksi di-batch ($k=20$) oleh Relayer

**Alokasi Komponen:** `nimbus-node/src/guardian/`, `nimbus-core/src/bls.rs`

---

### #2 — zkTLS Invoice Attestation (Pivot dari zkAgent)

**Paper:** `zkAgent-Verifiable-LLM-Agent-Execution-One-Shot-Transcript-Proofs.md` (2026/199)  
**Status:** PIVOT — Efisiensi Komputasi

**Realitas Teknis:**
ZK inferensi LLM penuh pada transaksi mikro ($0.05) tidak feasible. Modul ini memverifikasi keaslian faktur dari web.

**Mekanisme:**
- Agent/ERP tarik faktur dari endpoint merchant (Stripe, AWS Billing)
- Modul zkTLS di SDK bukti autentik dari domain merchant
- Kredensial ZK-UTXO diikat ke komitmen faktur

**Alokasi Komponen:** `nimbus-sdk/src/agent.rs`, `nimbus-sdk/src/compliance/`

---

### #4 — ZK Email (NFA-based Regex): Identity & Recovery Engine

**Paper:** `ZK-Proofs-Regex-Email-Verification.md` (2026)  
**Status:** APPROVED — Recovery & Enterprise Whitelisting

**Realitas Teknis:**
Menggunakan $\varepsilon$-free NFA dengan pencocokan off-circuit dan validasi path in-circuit.

**Mekanisme:**
- **Social Recovery:** Bukti kepemilikan email DKIM untuk restore kunci
- **Enterprise Whitelisting:** Bukti mandate dari domain korporat (@perusahaan.com) zero-knowledge
- Verifikasi native di WASM Stylus

**Alokasi Komponen:** `nimbus-sdk/src/recovery.rs`, `nimbus-contracts/src/verifier/`

---

### #6 — Improved Issuer-Hiding BBS+: Multi-Issuer Anonymity

**Paper:** `Improved-Issuer-Hiding-BBS-based-Anonymous-Credentials.md` (2026/555)  
**Status:** APPROVED — Prioritas Privasi

**Realitas Teknis:**
Menghapus kerentanan kolusi issuer di skema lama. BBS+ signed-policy di Algebraic Group Model.

**Mekanisme:**
- **Penyimpanan On-Chain $O(1)$:** Kontrak hanya simpan satu kunci kebijakan induk
- **Accountable Issuer Hiding (AIH):** Identitas issuer dienkripsi ElGamal, dibuka jika fraud terdeteksi
- Kuorum pembuka (threshold governance) bisa eksekusi slashing

**Alokasi Komponen:** `nimbus-core/src/bbs.rs`, `nimbus-contracts/src/spend.rs`

---

### #7 — NI-DKG on Blockchain: Zero-Complaint Guardian Setup

**Paper:** `NI-DKG-Non-Interactive-Distributed-Key-Generation-Blockchain-Zero-Knowledge-Proofs.md` (2026/552)  
**Status:** APPROVED — Guardian Lifecycle

**Realitas Teknis:**
DKG non-interaktif tanpa fase komplain. Mencegah timing griefing dan gas sensor.

**Mekanisme:**
- Node kandidat upload komitmen koefisien + bukti zk-SNARK ke kontrak Stylus
- Bukti ZK membuktikan validitas Feldman di muka
- Kontrak reject invalid data otomatis
- Digunakan untuk rotasi kunci kuorum (3-of-5 → 5-of-7)

**Alokasi Komponen:** `nimbus-contracts/src/staking.rs`, `nimbus-node/src/guardian/dkg.rs`

---

### #10 — Coral-CFG: Structured Business Data Parsing

**Paper:** `Coral-CFG-Proofs.md` (2025/1420)  
**Status:** APPROVED — B2B Compliance

**Realitas Teknis:**
Parser ZK untuk JSON/TOML via pohon biner LCRS dan Segmented Memory (Nebula).

**Mekanisme:**
- 100% di sisi klien (SDK), tidak membebani Guardian
- Parsing field nilai uang, mata uang, status pembayaran dalam 1-3 detik CPU lokal
- Output: komitmen terverifikasi kompatibel dengan ZK-UTXO spend

**Alokasi Komponen:** `nimbus-sdk/src/compliance/cfg.rs`

---



### #17 — Device-Binding Anonymous Credentials: Hardware-Bound Payment

**Paper:** `Device-Binding-Anonymous-Credentials-Legacy-Phones.md` (2026/965)  
**Status:** APPROVED — Komponen Kritis Keamanan Mobile SDK & Retail

**Konteks Arsitektur:**
Paper ini terhubung langsung dengan Inovasi #6 (Improved BBS+) dan menjadi fondasi pengamanan penyimpanan client-side Nimbus di iOS dan Android.

**Masalah Fundamental:**
Prinsip Nimbus menetapkan penyimpanan note ZK-UTXO 100% di sisi klien (zeroized on drop). Ancaman: malware/infostealer dapat menyalin note dari storage perangkat lunak. Solusi: bind ke Secure Enclave / TEE.

**Constraint Eksternal:**
Chip Secure Element di miliaran ponsel terkunci pada ECDSA P-256, tidak mendukung BLS12-381. Paper memecahkan ini dengan memisahkan domain.

**Solusi: PoP-BP (Bulletproofs + T-256)**
- Ukuran bukti: ~1.47 kB (paling kecil)
- Waktu prover (ponsel modern): 208 ms
- Verifier: 35-52 ms
- Kompatibel dengan BLS12-381 + BBS+

**Keamanan:**
- Note yang dicuri dari storage tidak bisa digunakan tanpa Secure Enclave korban
- Relayer ekonomi terlindungi (1.47 kB << batas calldata L2)
- Guardian tetap stateless blind signer

**Rekomendasi:** PoP-BP (utama) atau PoP-PLONK (alternatif jika butuh kurva standar 100%)

**Alokasi Komponen:** `nimbus-sdk/src/credential.rs`, `nimbus-core/src/bls.rs`, `nimbus-contracts/src/spend.rs`

---

