# Inovasi Jurnal untuk Kriptografi Inti (`zeltra-core`)

Dokumen ini memetakan paper terpilih untuk lapisan kriptografi fundamental Zeltra ([`zeltra-core`](file:///workspaces/Zeltra-Protocol/zeltra-core/)), mencakup sirkuit ZK-UTXO (Arkworks `ark-groth16`), BLS12-381, hashing Poseidon, dan skema blind signature.

---

## 1. Argo: Leaking a Secret from an Invalid Groth16 Proof (ACM CCS / IACR 2024–2025)
* **File Jurnal:** [`jurnal/Argo-Leaking-Secret-Invalid-Groth16.md`](file:///workspaces/Zeltra-Protocol/jurnal/Argo-Leaking-Secret-Invalid-Groth16.md)
* **Problem Kritis:**
  Banyak protokol ZK mengabaikan sanitasi titik kurva Groth16 publik ($A \in G_1, B \in G_2, C \in G_1$). Paper *Argo* mendemonstrasikan serangan di mana prover jahat atau relayer manipulatif dapat mengekstraksi saksi rahasia (*witness leakage*) melalui pembuktian dengan titik kurva titik tak valid (invalid curve attack / small subgroup confinement).
* **Inovasi & Mitigasi:**
  Validasi subgroup wajib (`into_affine()`, non-infinity check, dan subgroup membership check) sebelum mengevaluasi pairing.
* **Implementasi di Zeltra Core:**
  * Komponen: [`zeltra-core/src/circuit.rs`](file:///workspaces/Zeltra-Protocol/zeltra-core/src/circuit.rs) dan deserializer Groth16.
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
  * Komponen: [`zeltra-core/src/note.rs`](file:///workspaces/Zeltra-Protocol/zeltra-core/src/note.rs) (Merkle Mountain Range & Poseidon-W5 bagging).
  * Memastikan struktur pohon komitmen MMR dan hash nullifier bersifat *Strongly History-Independent*: pihak luar yang mengaudit state ledger hanya melihat status validitas saat ini tanpa dapat merekonstruksi urutan transaksi masa lalu antar pengguna.

---

## 4. Constant-Time Conditions for Left-to-Right Scalar Multiplication (IACR ePrint 2026/1881)
* **File Jurnal / ePrint:** IACR ePrint 2026/1881 (Oktober 2026)
* **Penulis:** Sergey Agievich
* **Problem Kritis:**
  Perkalian skalar kurva eliptik pada kurva BLS12-381 ($G_1, G_2$) dan X25519 (saat derivasi spending key, nullifier secret key derivation `owner_key * G`, atau uncompressed point scalar mult) sering kali memiliki titik cabang eksepsional (exception handling saat point addition/doubling degenerate cases). Jika eksekusi tidak strictly constant-time, serangan side-channel timing attack dapat merekonstruksi bit-bit skalar rahasia pengguna dari variasi latensi instruksi.
* **Inovasi Paper:**
  Menetapkan kondisi aljabar deterministik pada digit skalar pemrosesan left-to-right (MSB to LSB) sehingga tidak ada titik eksepsi sama sekali selama intermediate doublings and additions (analisis varian Comb: SAB-Set dan LSB-Set). Menjamin eksekusi strictly constant-time tanpa cabang kondisional pengecualian dan tanpa overhead komputasi.
* **Implementasi di Zeltra Core:**
  * Komponen: [`zeltra-core/src/evm.rs`](file:///workspaces/Zeltra-Protocol/zeltra-core/src/evm.rs) dan [`zeltra-core/src/note.rs`](file:///workspaces/Zeltra-Protocol/zeltra-core/src/note.rs).
  * Mengeliminasi titik cabang kondisional pada proses derivasi nullifier dan kalkulasi skalar BLS12-381, menjamin proses komputasi kriptografi inti di `zeltra-core` dan preflight relayer kebal terhadap side-channel timing analysis.

---

## 5. Fast Polynomial Arithmetic in the Exponent without Roots of Unity (IACR ePrint 2026/2413)
* **File Jurnal / ePrint:** IACR ePrint 2026/2413 (Oktober 2026)
* **Penulis:** Diane Ducrocq and Pierrick Gaudry
* **Problem di Sistem Klasik:**
  Operasi perkalian polinomial berderajat tinggi di dalam eksponen kurva eliptik ($g^{P(x)}$) adalah pondasi penting dalam evaluasi QAP Groth16 dan skema komitmen polinomial KZG. Teknik Number Theoretic Transform (NTT) konvensional menuntut keberadaan *roots of unity* (akar kesatuan modulo $q$) pada domain evaluasi. Jika derajat sirkuit atau field grup tidak memiliki akar kesatuan 2-adik yang cukup, prover terpaksa menggunakan perkalian naif kuadratik $O(N^2)$ yang sangat lambat.
* **Inovasi Paper:**
  Merumuskan algoritma perkalian polinomial di dalam eksponen berbasis adaptasi Schönhage-Strassen yang mencapai kompleksitas kuasi-linear $O(N \log N)$ **tanpa mensyaratkan keberadaan akar kesatuan sama sekali** pada field grup kurva.
* **Implementasi di Zeltra Core:**
  * Komponen: Evaluator QAP di [`zeltra-core/src/note_circuit.rs`](file:///workspaces/Zeltra-Protocol/zeltra-core/src/note_circuit.rs) dan [`zeltra-core/src/joinsplit_circuit.rs`](file:///workspaces/Zeltra-Protocol/zeltra-core/src/joinsplit_circuit.rs).
  * Menghilangkan restriksi ukuran sirkuit berbasis pangkat dua sempurna ($2^k$), memberikan fleksibilitas aljabar penuh saat mengagregasi batched commitments dan QAP polynomial evaluations pada kurva BLS12-381.

---

## 6. The Geometry of Witness-Update Tradeoffs in Additive Positive Accumulators (IACR ePrint 2026/2362)
* **File Jurnal / ePrint:** IACR ePrint 2026/2362 (Oktober 2026)
* **Penulis:** Wei Qi
* **Problem Kritis:**
  Pada akumulator set aditif (*additive positive accumulators*) seperti Merkle Mountain Range (MMR) yang digunakan oleh Zeltra ([`zeltra-contracts/src/merkle.rs`](file:///workspaces/Zeltra-Protocol/zeltra-contracts/src/merkle.rs)), setiap penambahan $N$ note baru menuntut penyegaran bukti keanggotaan (*membership witness / Merkle proof updates*). Jika client atau indexer menyegarkan witness secara naif setiap kali ada deposit baru, beban komputasi dan transfer bandwidth melonjak secara eksponensial.
* **Inovasi Paper:**
  Secara fundamental memetakan geometri ruang vektor pembaruan saksi (*witness-update vector tradeoff*) pada pohon biner berurutan (*inorder binary trees / Merkle forests*). Paper ini membuktikan batas batas bawah koordinat (*coordinatewise lower boundary*) dan merumuskan distribusi pohon optimal yang meminimalkan total pembaruan saksi secara global, membuktikan bahwa akumulator berbasis hutan Merkle (seperti MMR) dapat mencapai amortisasi pembaruan saksi yang optimal jika traversal merger diatur secara strictly monotonic.
* **Implementasi di Zeltra Core, Contract & Relayer:**
  * Komponen: Akumulator MMR di [`zeltra-core/src/`](file:///workspaces/Zeltra-Protocol/zeltra-core/) dan sinkronisasi MMR di [`zeltra-node/src/handlers/mmr.rs`](file:///workspaces/Zeltra-Protocol/zeltra-node/src/handlers/mmr.rs).
  * Memberikan model matematika presisi untuk penjadwalan caching bukti MMR di client SDK: bukti daun di-refresh dengan jadwal amortisasi optimal $O(\log N)$ tanpa pemborosan komputasi traversal hashing ulang yang redundan.
---

## 7. Cryptanalysis & Security Margins of Poseidon via Subspace Restriction (IACR ePrint 2026/1692)
* **File Jurnal / ePrint:** IACR ePrint 2026/1692 (Oktober 2026)
* **Penulis:** Amit Singh Bhati, Sundas Tariq, Tomer Ashur
* **Problem Kritis:**
  Fungsi hash ramah-ZK *Poseidon* merupakan tulang punggung komputasi komitmen UTXO note dan akumulator MMR di Zeltra ([`zeltra-core/src/poseidon.rs`](file:///workspaces/Zeltra-Protocol/zeltra-core/src/poseidon.rs) dan [`zeltra-contracts/src/poseidon_w5_constants.rs`](file:///workspaces/Zeltra-Protocol/zeltra-contracts/src/poseidon_w5_constants.rs)). Jika struktur ronde penuh ($R_F$) atau ronde parsial ($R_P$) rentan terhadap pembatalan aljabar (*algebraic cancellation*), penyerang berpotensi memecahkan problem *Constrained-Input Constrained-Output (CICO)* untuk memalsukan komitmen note atau membalikkan hash nullifier.
* **Inovasi Paper:**
  Merumuskan kerangka kerja matematika terpadu *Smart Subspace Restriction* yang memanfaatkan *Generalized S-Box Skipping (GSR)* dan *$d$-wise collapsing*. Teknik ini memproyeksikan variabel state ke dalam *nullspace* dari submatriks invers MDS, sehingga membatalkan non-linearitas S-box putaran awal dan putaran parsial. Paper ini berhasil mengekstrak basis Gröbner pada instance tereduksi (seperti Poseidon KoalaBear 31-bit hingga 20 dari 31 ronde).
* **Implementasi & Validasi di Zeltra Core:**
  * Komponen: Generator konstanta dan parameter round di [`zeltra-core/src/poseidon.rs`](file:///workspaces/Zeltra-Protocol/zeltra-core/src/poseidon.rs) dan kepatuhan [`DEC-021`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-021-poseidon-grain128-lfsr-round-constants-audit.md).
  * Memberikan validasi kriptanalisis formal: Zeltra mengimplementasikan Poseidon-W3 dan Poseidon-W5 di atas medan skalar besar BLS12-381 $Fr$ (255-bit) dengan $R_F = 8$ dan $R_P \ge 57$, jauh melampaui ambang batas penetrasi subspace attack (yang hanya efektif pada medan kecil $\le 64$-bit atau varian round-reduced). Temuan ini memperkuat audit keamanan bahwa margin keamanan 128-bit Zeltra terhadap serangan basis Gröbner dan CICO tetap kokoh tak tertembus.

---

## 8. Dixon Resultants Algebraic Cryptanalysis & Parameter Hardening for Poseidon-W5 (IACR ePrint 2026/1969)
* **File Jurnal / ePrint:** IACR ePrint 2026/1969 (September 2026)
* **Penulis:** Haohai Suo, Jiamin Cui
* **Problem Kritis:**
  Primitif *Arithmetization-Oriented (AO)* seperti Poseidon width-5 di [`zeltra-core/src/poseidon.rs`](file:///workspaces/Zeltra-Protocol/zeltra-core/src/poseidon.rs) dan [`zeltra-contracts/src/poseidon_w5_constants.rs`](file:///workspaces/Zeltra-Protocol/zeltra-contracts/src/poseidon_w5_constants.rs) dirancang dengan derajat polinomial rendah (S-Box $x^5$) agar ramah gerbang sirkuit ZK. Namun, sistem polinomial berderajat rendah rentan terhadap serangan aljabar multi-variabel (*algebraic attacks*) menggunakan basis Gröbner atau eliminasi resultan. Pendekatan Gröbner konvensional menuntut ideal nol-dimensi, sementara resultan Sylvester hanya mampu mengeliminasi satu variabel per langkah, membatasi analisis keamanan formal terhadap konvergensi serangan aljabar.
* **Inovasi Paper:**
  Mengevaluasi ulang **Dixon Resultants** sebagai metode fleksibel untuk mengeliminasi banyak variabel secara simultan pada primitif AO:
  1. Membuktikan batas atas ukuran matriks Dixon melalui perhitungan lintasan kisi (*lattice-path counting*).
  2. Mengimplementasikan solver berkecepatan tinggi *DRSolve* dan merumuskan 3 strategi eliminasi (direct, iterative, dan hybrid reduction).
  3. Menguji dan memvalidasi ketahanan parameter putaran Poseidon terhadap reduksi kompleksitas eliminasi aljabar.
* **Implementasi di Zeltra Core & Kontrak:**
  * Komponen: [`zeltra-core/src/poseidon.rs`](file:///workspaces/Zeltra-Protocol/zeltra-core/src/poseidon.rs), [`zeltra-contracts/src/poseidon_w5_constants.rs`](file:///workspaces/Zeltra-Protocol/zeltra-contracts/src/poseidon_w5_constants.rs), dan generator konstanta Grain-128 LFSR (`DEC-021`).
  * Memberikan landasan verifikasi matematis krusial untuk ketahanan parameter aljabar:
    - Membuktikan bahwa jumlah putaran penuh (*Full Rounds* $R_F = 8$) dan putaran parsial (*Partial Rounds* $R_P = 56$ atau 60) pada konfigurasi Poseidon-W5 Zeltra memberikan margin keamanan yang kokoh terhadap serangan eliminasi resultan Dixon deterministik.
    - Menjamin bahwa komitmen note ($C = \text{Poseidon}(v, \text{owner\_key}, \rho, r, \text{asset\_id})$) dan nullifier generator tidak dapat diinversi secara aljabar melalui sistem eliminasi polinomial berderajat rendah.

---

## 9. Top Gun: Degree Annihilation Attacks on Poseidon & Security Margins (IACR ePrint 2026/1254)
* **File Jurnal / ePrint:** IACR ePrint 2026/1254 (September 2026)
* **Penulis:** Antonio Sanso, Giuseppe Vitto
* **Problem Kritis:**
  Poseidon adalah permutasi arithmetization-oriented yang paling banyak dipakai pada sistem ZK modern. Paper ini memperkenalkan *Degree Annihilation*, teknik serangan aljabar yang memperluas pembatalan koefisien (*coefficient cancellation*) pada serangan round-skipping. Dengan memaksakan kendala aljabar yang membuat suku derajat dominan lenyap (*degree terms vanish*), penyerang dapat menurunkan derajat efektif sistem polinomial secara drastis dan meluncurkan serangan CICO-2 (*Constrained-Input Constrained-Output*) pada varian Poseidon berputaran tereduksi.
* **Inovasi Paper:**
  Merumuskan kerangka kerja *multivariate degree annihilation* menggunakan teknik eliminasi, resultan, dan basis Gröbner untuk membatalkan kontribusi derajat putaran parsial secara berturut-turut. Menunjukkan bahwa pada S-box kubik dengan 6 putaran penuh, membuat input putaran parsial pertama menjadi affine memangkas batas derajat akhir hingga faktor tiga.
* **Implementasi di Zeltra Core & Kontrak:**
  * Komponen: Parameter putaran Poseidon di [`zeltra-core/src/poseidon.rs`](file:///workspaces/Zeltra-Protocol/zeltra-core/src/poseidon.rs), [`zeltra-contracts/src/poseidon_w5_constants.rs`](file:///workspaces/Zeltra-Protocol/zeltra-contracts/src/poseidon_w5_constants.rs), dan generator konstanta Grain-128 LFSR (`DEC-021`).
  * Memvalidasi margin keamanan Zeltra: membuktikan bahwa konfigurasi Poseidon Width-5 Zeltra (menggunakan S-box pangkat lima $x^5$, 8 putaran penuh $R_F = 8$, dan 60 putaran parsial $R_P = 60$) memiliki margin derajat aljabar yang eksponensial di atas ambang batas *degree annihilation*, memastikan serangan CICO-2 atau pembatalan derajat aljabar tidak dapat menembus komitmen koin Zeltra.

---

## 10. Slipway: Accessing Finite Subspace Trails & Full-Round Absorption in Poseidon (IACR ePrint 2026/1579)
* **File Jurnal / ePrint:** IACR ePrint 2026/1579 (September 2026)
* **Penulis:** Giuseppe Vitto
* **Problem Kritis:**
  Analisis ketahanan Poseidon terhadap *finite subspace trails* (jejak sub-ruang hingga) pada putaran parsial. Jika suatu arah konstan melintasi beberapa putaran parsial berturut-turut, S-box tidak menambah derajat aljabar. Paper ini mengungkap fenomena *full-round absorption*: jika matriks linier MDS dan konstanta putaran memiliki relasi aljabar tersembunyi, penyerang dapat mengonstruksi keluarga input khusus yang melintasi 4 putaran penuh awal tanpa mengalami pertumbuhan derajat aljabar, membuka akses langsung ke subspace trail dan menghasilkan serangan CICO-2/CICO-3 penuh pada instansiasi Poseidon tertentu.
* **Inovasi Paper:**
  Mendemonstrasikan bahwa matriks MDS yang memenuhi kriteria uji standar desainer Poseidon pun masih bisa memiliki sifat *reachability* ke subspace trail jika terdapat relasi aljabar dengan putaran penuh prefix. Merumuskan batas matematis pada dimensi trail dan citra matriks (*matrix-image bound*) untuk mendeteksi serta mencegah fenomena full-round absorption.
* **Implementasi di Zeltra Core:**
  * Komponen: Generator matriks Cauchy MDS di [`zeltra-core/src/poseidon.rs`](file:///workspaces/Zeltra-Protocol/zeltra-core/src/poseidon.rs) dan generator Grain-128 LFSR (`DEC-021`).
  * Menjadi panduan verifikasi formal untuk matriks MDS Poseidon Zeltra: memastikan matriks MDS $5 \times 5$ (W5) dan $3 \times 3$ (W3) di-generate murni dari ekspansi Grain-128 LFSR yang independen secara statistik dari round constants, memutus total kemungkinan *full-round absorption* dan menjamin tidak ada subspace trail yang dapat diakses dari input manapun.


