# Inovasi Jurnal untuk Kriptografi Inti (`nimbus-core`)

Dokumen ini memetakan paper terpilih untuk lapisan kriptografi fundamental Nimbus ([`nimbus-core`](file:///workspaces/Zeltra-Protocol/nimbus-core/)), mencakup sirkuit ZK-UTXO (Arkworks `ark-groth16`), BLS12-381, hashing Poseidon, dan skema blind signature.

---

## 1. Argo: Leaking a Secret from an Invalid Groth16 Proof (ACM CCS / IACR 2024–2025)
* **File Jurnal:** [`jurnal/Argo-Leaking-Secret-Invalid-Groth16.md`](file:///workspaces/Zeltra-Protocol/jurnal/Argo-Leaking-Secret-Invalid-Groth16.md)
* **Problem Kritis:**
  Banyak protokol ZK mengabaikan sanitasi titik kurva Groth16 publik ($A \in G_1, B \in G_2, C \in G_1$). Paper *Argo* mendemonstrasikan serangan di mana prover jahat atau relayer manipulatif dapat mengekstraksi saksi rahasia (*witness leakage*) melalui pembuktian dengan titik kurva titik tak valid (invalid curve attack / small subgroup confinement).
* **Inovasi & Mitigasi:**
  Validasi subgroup wajib (`into_affine()`, non-infinity check, dan subgroup membership check) sebelum mengevaluasi pairing.
* **Implementasi di Nimbus Core:**
  * Komponen: [`nimbus-core/src/circuit.rs`](file:///workspaces/Zeltra-Protocol/nimbus-core/src/circuit.rs) dan deserializer Groth16.
  * Menjamin `proof_a_neg`, `proof_b`, dan `proof_c` lolos sanitasi strict subgroup sebelum dikirimkan ke verifier Stylus contract, mencegah kebocoran secret spend key (`sk_spend`) klien.

---

## 2. Adaptively-Secure Threshold Blind OPRF (IACR ePrint 2025/483)
* **File Jurnal:** [`jurnal/Adaptively-Secure-Threshold-Blind-OPRF.md`](file:///workspaces/Zeltra-Protocol/jurnal/Adaptively-Secure-Threshold-Blind-OPRF.md)
* **Problem di Sistem Klasik:**
  Skema Blind Signature tradisional (seperti blind BDHKE dasar) sering kali hanya aman di bawah model korupsi statis. Jika penyerang dapat mengorup guardian secara adaptif di tengah-tengah ronde penandatanganan, penyerang bisa memalsukan kredensial (unforgeability breach).
* **Inovasi Paper:**
  Protokol Threshold Blind Oblivious PRF (TB-OPRF) round-optimal yang adaptively-secure di grup BLS12-381 tanpa memerlukan interaksi berulang antar guardian (non-interactive share generation).
* **Implementasi di Nimbus Core:**
  * Komponen: [`nimbus-core/src/bdhke.rs`](file:///workspaces/Zeltra-Protocol/nimbus-core/src/bdhke.rs) dan [`nimbus-core/src/bls.rs`](file:///workspaces/Zeltra-Protocol/nimbus-core/src/bls.rs).
  * Menyempurnakan agregasi partial blind signature 3-of-5 threshold di Nimbus menjadi tahan terhadap skenario *adaptive guardian compromise*.

---

## 3. Cryptographic Erasure on Public Ledgers (IACR ePrint 2026/1109)
* **File Jurnal:** [`jurnal/Cryptographic-Erasure-Public-Ledgers.md`](file:///workspaces/Zeltra-Protocol/jurnal/Cryptographic-Erasure-Public-Ledgers.md)
* **Problem Fundamental:**
  Pertentangan antara sifat *immutability* blockchain dan hak penghapusan data *Right to be Forgotten* (GDPR Art. 17).
* **Inovasi Paper:**
  *Verifiable Key Destruction Protocol*: Memusnahkan kunci dekripsi (*ephemeral trapdoor*) di dalam sirkuit ZK secara matematis terbukti (*proof of key destruction*). Ciphertext di public ledger terbukti secara kriptografis tidak dapat lagi didekripsi selamanya (bertransformasi menjadi derau entropi murni).
* **Implementasi di Nimbus Core:**
  * Gadget ZK baru di `PrivateNoteCircuit`: `ErasureGadget` yang membuktikan bahwa viewing key atau ephemeral note seed telah dibakar dan dihapus dari memori. Memberikan jaminan keamanan tingkat institusi untuk audit privasi.

---

## 4. Failure Is Not Silent: Attacks on Blind Signatures (IACR ePrint 2026/09)
* **File Jurnal:** [`jurnal/Failure-Not-Silent-Attacks-Blind-Signatures.md`](file:///workspaces/Zeltra-Protocol/jurnal/Failure-Not-Silent-Attacks-Blind-Signatures.md)
* **Problem Kritis:**
  Banyak skema blind signature yang mengabaikan *signer abort leakage*, di mana signer yang nakal dengan sengaja menyebabkan kegagalan parsial (abort) untuk mendelimitasi dan mengidentifikasi IP atau identitas requestor.
* **Inovasi Paper:**
  Merumuskan standar *Strong Blindness under Abort Attacks* dan teknik zero-knowledge blinding blindsizing untuk mencegah metadata leakage.
* **Implementasi di Nimbus Core:**
  * Memperkuat modul masking key release $k \cdot \text{pk}_{\text{iss}} == \text{com}_k$ (DEC-018) agar kegagalan validasi tidak pernah membocorkan bit rahasia dari unmasking request klien.
