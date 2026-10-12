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

## 2. Cryptographic Erasure on Public Ledgers (IACR ePrint 2026/1109)
* **File Jurnal:** [`jurnal/Cryptographic-Erasure-Public-Ledgers.md`](file:///workspaces/Zeltra-Protocol/jurnal/Cryptographic-Erasure-Public-Ledgers.md)
* **Problem Fundamental:**
  Pertentangan antara sifat *immutability* blockchain dan hak penghapusan data *Right to be Forgotten* (GDPR Art. 17).
* **Inovasi Paper:**
  *Verifiable Key Destruction Protocol*: Memusnahkan kunci dekripsi (*ephemeral trapdoor*) di dalam sirkuit ZK secara matematis terbukti (*proof of key destruction*). Ciphertext di public ledger terbukti secara kriptografis tidak dapat lagi didekripsi selamanya (bertransformasi menjadi derau entropi murni).
* **Implementasi di Zeltra Core:**
  * Gadget ZK baru di `PrivateNoteCircuit`: `ErasureGadget` yang membuktikan bahwa viewing key atau ephemeral note seed telah dibakar dan dihapus dari memori. Memberikan jaminan keamanan tingkat institusi untuk audit privasi.

---

## 3. Auditable Data Structures: Strong History-Independence (IACR ePrint 2016/755)
* **File Jurnal / PDF:** [`jurnal/pdf/2016-755.pdf`](file:///workspaces/Zeltra-Protocol/jurnal/pdf/2016-755.pdf)
* **Penulis:** Michael T. Goodrich, Evgenios M. Kornaropoulos, Michael Mitzenmacher, Roberto Tamassia
* **Problem di Sistem Klasik:**
  Struktur data pohon kriptografis standar sering kali membocorkan riwayat urutan operasi (*history leakage*). Pola penyisipan daun atau node internal dapat digunakan oleh penyerang untuk merekonstruksi kronologi urutan transaksi historis antar pengguna.
* **Inovasi Paper:**
  Merumuskan standar dan pembuktian formal untuk *Strongly History-Independent (SHI)* auditable data structures. Struktur data SHI menjamin bahwa status kriptografis hanya bergantung secara eksklusif pada himpunan data aktif saat ini, dan independen seragam terhadap urutan penyisipan masa lalu.
* **Implementasi di Zeltra Core:**
  * Komponen: [`nimbus-core/src/note.rs`](file:///workspaces/Zeltra-Protocol/nimbus-core/src/note.rs) (Merkle Mountain Range & Poseidon-W5 bagging).
  * Memastikan struktur pohon komitmen MMR dan hash nullifier bersifat *Strongly History-Independent*: pihak luar yang mengaudit state ledger hanya melihat status validitas saat ini tanpa dapat merekonstruksi urutan transaksi masa lalu antar pengguna.

---

## 4. Jacobian Diagnostics for Under-Constrained ZK Circuits (IACR ePrint 2026/1852)
* **File Jurnal:** [`jurnal/reference/Jacobian-Diagnostics-Under-Constrained-ZK-Circuits.md`](file:///workspaces/Zeltra-Protocol/jurnal/reference/Jacobian-Diagnostics-Under-Constrained-ZK-Circuits.md)
* **Penulis:** Vijay Singh (September 2026)
* **Problem Kritis:**
  Sirkuit aritmatika ZK yang *under-constrained* adalah sumber kegagalan soundness paling fatal di Web3. Penyerang dapat menetapkan saksi dengan nilai palsu (misal jumlah uang atau penerima fiktif) sambil tetap menghasilkan proof Groth16 yang valid.
* **Inovasi Paper:**
  Diagnostik aljabar berbasis linear algebra matriks Jacobian eksak. Mampu mengklasifikasikan sirkuit R1CS hingga 66.000 constraint dalam waktu $\le 1$ detik di hardware standar. Memverifikasi secara deterministik apakah sebuah variabel target memiliki derajat kebebasan tak terduga (*infinitesimal freedom / non-uniqueness certificate*).
* **Implementasi di Zeltra Core:**
  * Komponen: Test suite diagnostik di [`nimbus-core/src/note_circuit.rs`](file:///workspaces/Zeltra-Protocol/nimbus-core/src/note_circuit.rs) dan [`nimbus-core/src/joinsplit_circuit.rs`](file:///workspaces/Zeltra-Protocol/nimbus-core/src/joinsplit_circuit.rs).
  * Menjalankan rank evaluation matriks Jacobian pada constraint system sebelum circuit freeze dan trusted setup ceremony untuk menjamin zero under-constrained wires pada seluruh sirkuit produksi.

---

## 5. Nopenena: Cryptographic Deniability for UTXO Blockchains with Dummy Notes (IACR ePrint 2024/903)
* **File Jurnal:** [`jurnal/honorable/Nopenena-Untraceable-Payments-Small-Decoy-Sets.md`](file:///workspaces/Zeltra-Protocol/jurnal/honorable/Nopenena-Untraceable-Payments-Small-Decoy-Sets.md)
* **Penulis:** Foteini Baldimtsi, Panagiotis Chatzigiannis, Konstantinos Chalkias
* **Problem Kritis:**
  Pada sistem pembayaran privat berbasis UTXO, pengembalian saldo sisa (*Change Note*) secara linier rentan terhadap analisis graf transaksi heuristik (*payment-graph analysis*). Pengamat on-chain dapat menebak nominal change note dan mengidentifikasi kepemilikan note lanjutan, menyebabkan degradasi anonymity set sebesar 40%–59%.
* **Inovasi Paper:**
  Protokol pembayaran rahasia berbasis decoy set kecil yang membuktikan secara matematis sifat *cryptographic deniability*: pengamat luar tidak dapat membedakan mana note pembayaran riil dan mana dummy change note yang disuntikkan secara acak.
* **Implementasi di Zeltra Core:**
  * Komponen: Universal JoinSplit circuit di [`nimbus-core/src/joinsplit_circuit.rs`](file:///workspaces/Zeltra-Protocol/nimbus-core/src/joinsplit_circuit.rs) (DEC-036A) dengan slot dummy canonical dan 448 range gates.
  * Memungkinkan penyisipan dummy slot konstan bernilai nol/acak untuk memutus korelasi grafik saldo secara statistik dan menjamin privasi jangka panjang bagi AI Agent dan retail.



