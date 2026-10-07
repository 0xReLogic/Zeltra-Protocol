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

---

## 5. CUBE: Partially Blind BBS Signatures for Unlinkable Decentralized Identity (IACR ePrint 2026/920)
* **File Jurnal:** [`jurnal/CUBE-Partially-Blind-BBS-Signatures-Unlinkable-Decentralized-Identity.md`](file:///workspaces/Zeltra-Protocol/jurnal/CUBE-Partially-Blind-BBS-Signatures-Unlinkable-Decentralized-Identity.md)
* **Penulis:** Guohao Lai
* **Problem Kritis:**
  Blueprint Nimbus Fase 3 merencanakan BBS+ selective disclosure untuk CEX compliance. Namun implementasi naif BBS+ memiliki celah tersembunyi: guardian cluster yang menerbitkan kredensial dapat menyimpan log internal issuance request dan mengkorelasikan sesi deposit dengan sesi spend — bahkan tanpa membaca konten kredensial. Ini adalah *issuance linkability*, serangan yang tidak ditangani oleh paper Threshold BBS+ standar.
* **Inovasi Paper:**
  CUBE (Credential Unlinkability using Blinded Elements) memperkenalkan *partially blind issuance*: sebagian atribut diblind oleh user sebelum dikirim ke issuer (guardian cluster), sehingga issuer tidak pernah melihat atribut sensitif saat issuance. Hasilnya: issuer tidak bisa membuat link antara sesi issuance dengan sesi verifikasi di CEX, bahkan dengan kolusi seluruh guardian.
* **Implementasi di Nimbus Core:**
  * Komponen utama: [`nimbus-core/src/bbs_credential.rs`](file:///workspaces/Zeltra-Protocol/nimbus-core/src/bbs_credential.rs) (baru) dan [`nimbus-node/src/guardian/credential_issuer.rs`](file:///workspaces/Zeltra-Protocol/nimbus-node/src/guardian/credential_issuer.rs) (baru).
  * Alur baru: User memblind atribut `session_id` dan `deposit_address` sebelum mengirim issuance request ke guardian cluster. Guardian hanya menandatangani blinded commitment — tidak pernah melihat nilai asli. User unmask setelah threshold signature terkumpul.
  * Dampak keamanan: Menghapus asumsi "guardian tidak kolusi" dari threat model Nimbus. Bahkan jika semua 5 guardian berkolusi, mereka tidak dapat menghubungkan credential yang diterbitkan ke transaksi deposit spesifik.

---

## 6. Auditable Data Structures: Strong History-Independence (IACR ePrint 2016/755)
* **File Jurnal / PDF:** [`jurnal/pdf/2016-755.pdf`](file:///workspaces/Zeltra-Protocol/jurnal/pdf/2016-755.pdf)
* **Penulis:** Michael T. Goodrich, Evgenios M. Kornaropoulos, Michael Mitzenmacher, Roberto Tamassia
* **Problem di Sistem Klasik:**
  Struktur data pohon kriptografis standar (seperti Merkle trees, skip lists, dynamic dictionaries) sering kali membocorkan riwayat urutan operasi (*history leakage*). Pola penyisipan daun atau node internal dapat digunakan oleh penyerang untuk merekonstruksi kronologi urutan transaksi historis antar pengguna.
* **Inovasi Paper:**
  Merumuskan standar dan pembuktian formal untuk *Strongly History-Independent (SHI)* auditable data structures. Struktur data SHI menjamin bahwa representasi memori dan status kriptografis hanya bergantung secara eksklusif pada himpunan data aktif saat ini, dan secara independen seragam terhadap urutan penyisipan atau penghapusan masa lalu.
* **Implementasi di Nimbus Core:**
  * Komponen: [`nimbus-core/src/merkle.rs`](file:///workspaces/Zeltra-Protocol/nimbus-core/src/merkle.rs) dan pemetaan nullifier ZK-UTXO.
  * Memastikan struktur pohon komitmen LeanIMT dan hash nullifier bersifat *Strongly History-Independent*: pihak luar yang mengaudit state ledger hanya melihat status validitas saat ini tanpa dapat merekonstruksi urutan transaksi masa lalu antar pengguna.

