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

## 4. Failure Is Not Silent: Attacks on Blind Signatures (IACR ePrint 2026/2043)
* **File Jurnal:** [`jurnal/reference/Failure-Is-Not-Silent-Attacks-Blind-Signatures.md`](file:///workspaces/Zeltra-Protocol/jurnal/reference/Failure-Is-Not-Silent-Attacks-Blind-Signatures.md)
* **Penulis:** Maksymilian Gorski, Lucjan Hanzlik (September 2026)
* **Problem Kritis:**
  Banyak skema blind signature dan OPRF yang mengabaikan serangan *selective abort leakage*. Signer yang jahat (misal guardian/relayer nakal) dapat merekayasa respons agar verifikasi unmasking klien gagal (*abort*) jika dan hanya jika pesan rahasia pengguna memenuhi kriteria tertentu. Status abort/sukses menjadi *one-bit testing oracle* yang membocorkan isi pesan rahasia pengguna.
* **Inovasi & Mitigasi Paper:**
  Merumuskan standar *Strong Blindness under Abort Attacks* dan prinsip desain *Publicly Checkable Signature Derivation* (dalam definisi Fischlin-Schröder) serta zero-knowledge proof of honest signer evaluation agar tidak ada kebocoran bit rahasia melalui selective abort.
* **Implementasi di Nimbus Core:**
  * Komponen: [`nimbus-core/src/blind_sign.rs`](file:///workspaces/Zeltra-Protocol/nimbus-core/src/blind_sign.rs) dan modul verifikasi threshold di [`nimbus-node/src/handlers/deposit.rs`](file:///workspaces/Zeltra-Protocol/nimbus-node/src/handlers/deposit.rs).
  * Memperkuat verifikasi kriptografis fail-closed $k \cdot \text{pk}_{\text{iss}} == \text{com}_k$ (DEC-018) dengan pembuktian derivation publik, memastikan kegagalan validasi tidak pernah membocorkan bit rahasia dari kredensial unmasking klien.

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

---

## 7. Improved Issuer-Hiding BBS+: Multi-Issuer Anonymity (IACR ePrint 2026/555)
* **File Jurnal:** [`jurnal/Improved-Issuer-Hiding-BBS-based-Anonymous-Credentials.md`](file:///workspaces/Zeltra-Protocol/jurnal/Improved-Issuer-Hiding-BBS-based-Anonymous-Credentials.md)
* **Problem di Sistem Klasik:**
  Pada sistem kredensial anonim multi-issuer tradisional, verifikasi keanggotaan publik memerlukan penyimpanan daftar seluruh public key issuer on-chain atau ukuran proof membengkak $O(M)$ seiring bertambahnya issuer. Selain itu, ada kerentanan kolusi di mana issuer dapat melacak kapan kredensial digunakan jika tidak ada proteksi identitas penerbit yang terpisah.
* **Inovasi Paper:**
  Skema BBS+ signed-policy di Algebraic Group Model dengan fitur *Accountable Issuer Hiding (AIH)*:
  1. **Penyimpanan On-Chain $O(1)$:** Smart contract hanya menyimpan satu kunci kebijakan induk (*master policy root*), terlepas dari berapapun jumlah issuer terdaftar.
  2. **Akuntabilitas Kriptografis:** Identitas penerbit asli disembunyikan dalam enkripsi ElGamal in-circuit saat presentasi kredensial. Jika terdeteksi kecurangan (fraud/double issuance), kunci dapat dibuka oleh kuorum governance untuk eksekusi slashing.
* **Implementasi di Nimbus Core:**
  * Komponen: [`nimbus-core/src/bbs.rs`](file:///workspaces/Zeltra-Protocol/nimbus-core/src/) dan [`nimbus-core/src/circuit.rs`](file:///workspaces/Zeltra-Protocol/nimbus-core/src/circuit.rs).
  * Memungkinkan penerbitan kredensial KYC/kepatuhan multi-entitas (misal dari berbagai CEX atau entitas perbankan) secara terdesentralisasi tanpa membocorkan identitas issuer spesifik ke block explorer publik, namun tetap dapat diaudit oleh kuorum jika terjadi pelanggaran hukum.

---

## 8. Jacobian Diagnostics for Under-Constrained ZK Circuits (IACR ePrint 2026/1852)
* **File Jurnal:** [`jurnal/reference/Jacobian-Diagnostics-Under-Constrained-ZK-Circuits.md`](file:///workspaces/Zeltra-Protocol/jurnal/reference/Jacobian-Diagnostics-Under-Constrained-ZK-Circuits.md)
* **Penulis:** Vijay Singh (September 2026)
* **Problem Kritis:**
  Sirkuit aritmatika ZK yang *under-constrained* adalah sumber kegagalan soundness paling fatal di Web3 (sebagaimana terlihat pada temuan Issue #4 GitHub di mana public inputs tidak terikat relasi matematika). Penyerang dapat menetapkan saksi dengan nilai palsu (misal jumlah uang atau penerima fiktif) sambil tetap menghasilkan proof Groth16 yang valid.
* **Inovasi Paper:**
  Diagnostik aljabar berbasis linear algebra matriks Jacobian eksak. Mampu mengklasifikasikan sirkuit R1CS hingga 66.000 constraint dalam waktu $\le 1$ detik di hardware standar. Memverifikasi secara deterministik apakah sebuah variabel target memiliki derajat kebebasan tak terduga (*infinitesimal freedom / non-uniqueness certificate*).
* **Implementasi di Nimbus Core:**
  * Komponen: Test suite diagnostik di [`nimbus-core/src/note_circuit.rs`](file:///workspaces/Zeltra-Protocol/nimbus-core/src/note_circuit.rs) dan [`nimbus-core/src/joinsplit_circuit.rs`](file:///workspaces/Zeltra-Protocol/nimbus-core/src/joinsplit_circuit.rs).
  * Menjalankan rank evaluation matriks Jacobian pada constraint system sebelum circuit freeze dan trusted setup ceremony untuk menjamin zero under-constrained wires pada seluruh sirkuit produksi.

---

## 9. Nopenena: Cryptographic Deniability for UTXO Blockchains with Dummy Notes (IACR ePrint 2024/903)
* **File Jurnal:** [`jurnal/honorable/Nopenena-Untraceable-Payments-Small-Decoy-Sets.md`](file:///workspaces/Zeltra-Protocol/jurnal/honorable/Nopenena-Untraceable-Payments-Small-Decoy-Sets.md)
* **Penulis:** Foteini Baldimtsi, Panagiotis Chatzigiannis, Konstantinos Chalkias
* **Problem Kritis:**
  Pada sistem pembayaran privat berbasis UTXO (seperti Zeltra dan Monero), pengembalian saldo sisa (*Change Note*) secara linier rentan terhadap analisis graf transaksi heuristik (*payment-graph analysis*). Pengamat on-chain dapat menebak nominal change note dan mengidentifikasi kepemilikan note lanjutan, menyebabkan degradasi anonymity set sebesar 40%–59%.
* **Inovasi Paper:**
  Protokol pembayaran rahasia berbasis decoy set kecil dengan ukuran proof ~80% lebih ringkas dari QuisQuis, yang membuktikan secara matematis sifat *cryptographic deniability*: pengamat luar tidak dapat membedakan mana note pembayaran riil dan mana dummy change note yang disuntikkan secara acak.
* **Implementasi di Nimbus Core:**
  * Komponen: Modul generator sirkuit di [`nimbus-core/src/note_circuit.rs`](file:///workspaces/Zeltra-Protocol/nimbus-core/src/note_circuit.rs) dan generator dummy note di [`nimbus-sdk/src/wallet/note_wallet.rs`](file:///workspaces/Zeltra-Protocol/nimbus-sdk/src/wallet/note_wallet.rs).
  * Mengintegrasikan injeksi 1 atau 2 decoy dummy note bernilai acak/nol ke dalam komitmen pohon LeanIMT pada alur spend, memutus korelasi grafik saldo secara statistik dan menjamin privasi jangka panjang bagi AI Agent dan retail.



