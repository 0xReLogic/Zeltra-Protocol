# DEC-XXX: [Judul Keputusan Arsitektur]

- **Status:** PROPOSED | APPROVED ARCHITECTURAL SPECIFICATION
- **Tipe DEC:** FITUR BARU | OPTIMASI PERFORMA | CIRCUIT HARDENING | SECURITY FIX / AUDIT
- **Author:** [Nama / Tim Pengembang]
- **Date:** YYYY-MM-DD
- **Target Crates & Components:**
  - `zeltra-core/src/...` ([Deskripsi komponen / sirkuit / primitif])
  - `zeltra-sdk/src/...` ([Deskripsi wallet / prover WASM / compliance])
  - `zeltra-node/src/...` ([Deskripsi relayer / indexer / database])
  - `zeltra-contracts/src/...` ([Deskripsi smart contract Stylus / storage])
- **Parent & Related DECs:**
  - [`DEC-YYY: Nama Dokumen`](DEC-YYY-nama-file.md)
- **Security Audit Targets:** (Wajib jika Tipe = Security Fix / Audit, misal `AUD-XX` atau `SEC-CRIT-XX`)
- **Scientific Foundation & Primary Citations:** [Paper IACR / RFC / EIP / Standardisasi]

---

## 1. Executive Summary & Problem Formulation

### A. Konteks & Problem Statement
- **Jika Fitur Baru:** Apa kapabilitas baru yang ingin ditambahkan (misal: AI Agent x402, Direct Stealth Payment)? Mengapa sistem lama belum bisa mengakomodasinya?
- **Jika Optimasi:** Di mana bottleneck terjadi (misal: RAM boros di mobile, scanning lambat, gas mahal)? Sertakan metrik sebelum optimasi.
- **Jika Circuit / Security Hardening:** Celah apa yang teridentifikasi (misal: under-constrained wire, partitioning oracle, non-contributory point, DPA timing)?

### B. Threat Model & Financial Failure Scenario (Jika Relevan)
Jelaskan dampak buruk jika keputusan ini tidak diterapkan:
- Apakah dana bisa terkunci permanen (*silent fund loss*)?
- Apakah ada derajat kebebasan liar (*unconstrained witness degrees of freedom*)?
- Apakah ada kebocoran metadata atau pelacakan identitas pengirim/penerima?

---

## 2. Invariants & Batasan Non-Negotiable

Daftar aturan mutlak yang **TIDAK BOLEH DILANGGAR** oleh keputusan ini:
1. **Multi-Asset Solvency Invariant:** `contract token balance >= total liabilities` (USDC & WETH selalu solvent, tidak ada fraksional).
2. **Circuit Soundness (Jika menyentuh R1CS / ZK):**
   - *Zero Degrees of Freedom (DEC-046):* Inactive branches / dummy slots wajib di-pinning secara deterministik.
   - *Jacobian Rigidity (DEC-047):* Wajib lolos verifikasi rank augmented Jacobian ($r_T = 0$) pada matriks saksi adversarial.
3. **Wire & Interface Budget:** Panjang calldata/memo tetap presisi (misal: tepat 96 byte, log event format tidak merusak scanner).
4. **Zero-Knowledge Privacy:** Tidak membocorkan IP, nilai nominal, atau grafik relasi transaksi on-chain.

---

## 3. Analisis Pilihan & Trade-Off Matrix

Tabel komparasi berbagai alternatif solusi yang sempat dipertimbangkan:

| Alternatif Pendekatan | Kelebihan | Kelemahan / Trade-Off Teknis | Keputusan Evaluasi |
|---|---|---|---|
| **Opsi A: [Pendekatan Konvensional]** | Implementasi cepat | Boros RAM / Rentan celah soundness | DITOLAK |
| **Opsi B: [Pendekatan Baru yang Dipilih]** | Matematis terbukti aman / Efisien | Butuh penyesuaian serializer / R1CS | **DIPILIH & DIRATIFIKASI** |

---

## 4. Spesifikasi Teknis & Detail Implementasi

*(Pilih sub-bagian yang relevan sesuai tipe DEC)*

### A. Konstruksi Matematis & Aljabar (Wajib untuk Circuit / Crypto)
Tuliskan relasi R1CS, formula hash Poseidon, kurva BLS12-381 / X25519, atau pembuktian formal teorema:
$$\text{Tag} = \text{Poseidon}(\text{secret}_{\text{sender}}, \text{secret}_{\text{recipient}})$$

### B. Arsitektur Data, Wire Layout & State Machine (Wajib untuk Fitur / Optimasi)
Gambarkan diagram alur data, transisi state, atau tata letak bit/byte wire format:

```text
┌────────────────────────────────────────────────────────────────────────────────────────┐
│                        DIAGRAM ALUR / STRUKTUR DATA (ASCII)                            │
└────────────────────────────────────────────────────────────────────────────────────────┘
```

### C. Implementasi Kode & Algoritma (Rust Reference)
Sertakan cuplikan fungsi inti pada crate terkait yang menunjukkan mekanisme fail-closed:

```rust
// Contoh implementasi referensi
pub fn execute_routine() {
    // Fail-closed invariant check
}
```

## 5. Rencana Verifikasi Empiris & Test Gates

- **Aturan Rasio Pengujian (Mandatory Protocol Standard):**
  - **Setiap 1 pengujian positif (happy path) WAJIB didampingi minimal 2x lebih banyak pengujian negatif (adversarial / negative tests)**:
    $$\text{Jumlah Negative Tests} \ge 2 \times \text{Jumlah Positive Tests}$$
  - Seluruh pengujian negatif wajib membuktikan secara formal bahwa **state sistem sama sekali tidak berubah (revert / fail-closed / zero state change)** saat eksekusi gagal.
- **Gate 1 (Positive / KAT):** Pengujian skenario normal & Known-Answer Test (vektor deterministik lulus 100%).
- **Gate 2 (Negative & Boundary Adversarial — Min 2x):** Pengujian fail-closed terhadap input cacat, tampered ciphertext, forged proof, replay nullifier, manipulasi flag non-kanonikal, dan overflow field scalar ($\ge r$).
- **Gate 3 (Benchmark / Formal Matrix):** 
  - *Jika Circuit:* Lolos evaluasi Jacobian rank pada 6-family witness sweep (`formal_tests.rs`).
  - *Jika Optimasi:* Tolok ukur benchmark performa (misal: penggunaan RAM turun $O(N) \to O(\sqrt{N})$, scanning speedup >10x).
  - *Jika Side-Channel:* Uji statistik waktu-konstan (`dudect` $|t| < 4.5$).
- **Gate 4 (E2E Integration Lifecycle):** Verifikasi alur penuh lintas crate (`core` -> `contracts` -> `node` -> `sdk` -> `cli`) bebas regresi.

---

## 6. Referensi & Sitasi Akademik (Wajib Bagian Terakhir)

Daftar sitasi literatur ilmiah, paper riset, EIP, atau standar kriptografi primer yang menjadi rujukan:

1. **[Penulis / Peneliti] ([Tahun / Bulan]):** *"[Judul Paper Penelitian Kriptografi]"*, [Nama Jurnal / Konferensi / IACR ePrint YYYY/XXXX], [Link DOI atau URL ePrint].
2. **[Standardisasi Resmi] ([Tahun]):** *"[Standar Industri, misal: NIST FIPS 203 / RFC 8439 / EIP-2537]"*, [URL Resmi].
3. **DEC-YYY (Zeltra Architecture):** [`DEC-YYY-nama-file.md`](DEC-YYY-nama-file.md) — [Konteks hubungan dengan DEC ini].
