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

## 4. Jacobian Diagnostics for Under-Constrained ZK Circuits (IACR ePrint 2026/1852)
* **File Jurnal:** [`jurnal/reference/Jacobian-Diagnostics-Under-Constrained-ZK-Circuits.md`](file:///workspaces/Zeltra-Protocol/jurnal/reference/Jacobian-Diagnostics-Under-Constrained-ZK-Circuits.md)
* **Penulis:** Vijay Singh (September 2026)
* **Problem Kritis:**
  Sirkuit aritmatika ZK yang *under-constrained* adalah sumber kegagalan soundness paling fatal di Web3. Penyerang dapat menetapkan saksi dengan nilai palsu (misal jumlah uang atau penerima fiktif) sambil tetap menghasilkan proof Groth16 yang valid.
* **Inovasi Paper:**
  Diagnostik aljabar berbasis linear algebra matriks Jacobian eksak. Mampu mengklasifikasikan sirkuit R1CS hingga 66.000 constraint dalam waktu $\le 1$ detik di hardware standar. Memverifikasi secara deterministik apakah sebuah variabel target memiliki derajat kebebasan tak terduga (*infinitesimal freedom / non-uniqueness certificate*).
* **Implementasi di Zeltra Core:**
  * Komponen: Test suite diagnostik di [`zeltra-core/src/note_circuit.rs`](file:///workspaces/Zeltra-Protocol/zeltra-core/src/note_circuit.rs) dan [`zeltra-core/src/joinsplit_circuit.rs`](file:///workspaces/Zeltra-Protocol/zeltra-core/src/joinsplit_circuit.rs).
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
  * Komponen: Universal JoinSplit circuit di [`zeltra-core/src/joinsplit_circuit.rs`](file:///workspaces/Zeltra-Protocol/zeltra-core/src/joinsplit_circuit.rs) (DEC-036A) dengan slot dummy canonical dan 448 range gates.
  * Memungkinkan penyisipan dummy slot konstan bernilai nol/acak untuk memutus korelasi grafik saldo secara statistik dan menjamin privasi jangka panjang bagi AI Agent dan retail.

---

## 6. Constant-Time Conditions for Left-to-Right Scalar Multiplication (IACR ePrint 2026/1881)
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

## 7. Committing AEAD: Partitioning Oracle Attack Defense for Encrypted Memos (IACR ePrint 2026/240)
* **File Jurnal / ePrint:** IACR ePrint 2026/240 (Oktober 2026)
* **Penulis:** Kazuhiko Minematsu and Akiko Inoue
* **Problem Kritis:**
  Pada skema AEAD konvensional (seperti ChaCha20Poly1305 naif), sebuah ciphertext dapat valid didekripsi oleh dua kunci berbeda menjadi dua plaintext berbeda. Kerentanan ini (*non-committing AEAD flaw*) memungkinkan penyerang mengeksekusi *partitioning oracle attacks* pada protokol berbasis UTXO/memo: adversary menyuntikkan memo crafted on-chain yang membingungkan scanner wallet penerima dan dapat memicu desinkronisasi saldo atau deanonymization.
* **Inovasi Paper:**
  Mengevaluasi dan memperbaiki transformasi generik Committing AEAD (seperti CTX Chan-Rogaway dan Chen-Karadžić) yang rentan terhadap instansiasi hash praktis. Paper ini merumuskan transformasi committing AEAD yang terbukti kokoh di bawah *standard model* dengan modifikasi algoritmik minimal tanpa mengorbankan performa enkripsi/dekripsi simetris.
* **Implementasi di Zeltra Core & SDK:**
  * Komponen: [`zeltra-core/src/note.rs`](file:///workspaces/Zeltra-Protocol/zeltra-core/src/note.rs) (`NotePlaintext::encrypt_memo` & `decrypt_memo`) dan trial decryption scanner di [`zeltra-sdk/src/wallet/note_wallet.rs`](file:///workspaces/Zeltra-Protocol/zeltra-sdk/src/wallet/note_wallet.rs).
  * Menjamin 96-byte encrypted memo yang dipancarkan lewat event `NoteCommitmentAppended` on-chain mengikat komitmen ciphertext secara matematis (*binding ciphertext to key & plaintext*), membendung partitioning attacks dan menjamin keaslian data scanning note wallet 100%.

---

## 8. Fast Polynomial Arithmetic in the Exponent without Roots of Unity (IACR ePrint 2026/2413)
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

## 9. The Geometry of Witness-Update Tradeoffs in Additive Positive Accumulators (IACR ePrint 2026/2362)
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

## 10. Cryptanalysis & Security Margins of Poseidon via Subspace Restriction (IACR ePrint 2026/1692)
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

## 11. Jacobian Diagnostics for Under-Constrained Zero-Knowledge Circuits (IACR ePrint 2026/1852)
* **File Jurnal / ePrint:** IACR ePrint 2026/1852 (Oktober 2026)
* **Penulis:** Vijay Singh
* **Problem Kritis:**
  *Under-constrained arithmetic circuits* adalah salah satu celah keamanan paling mematikan dalam protokol zero-knowledge. Jika suatu wire saksi (*witness wire*) atau variabel perantara dalam sirkuit R1CS tidak terikat secara ketat (*under-constrained*), seorang prover jahat dapat menetapkan nilai alternatif tanpa melanggar persamaan verifikasi (misalnya menduplikasi proof, memanipulasi nilai kembalian uang *change*, atau mengubah flag multi-aset). Pengujian fuzzer acak sering kali gagal mendeteksi celah ini pada sirkuit berukuran puluhan ribu gerbang.
* **Inovasi Paper:**
  Memperkenalkan metodologi diagnostik aljabar berbasis aljabar linier matriks Jacobian eksak. Dengan menggabungkan matriks Jacobian dari seluruh *constraint* R1CS dan menetapkan koordinat input publik, *kernel* dari Jacobian tersebut merepresentasikan *Zariski tangent space*. Jika terdapat arah pergerakan dalam kernel (*tangent dimension $> 0$*), hal tersebut membuktikan secara matematis keberadaan derajat kebebasan infinitesimal (*infinitesimal freedom / non-uniqueness certificate*). Alat ini memverifikasi sirkuit hingga 66.000 constraints hanya dalam hitungan detik.

---

## 12. Accelerated Endomorphism & Side-Channel Regular Scalar Multiplication on BLS12-381 (IACR ePrint 2024/1906)
* **File Jurnal / ePrint:** IACR ePrint 2024/1906 (September 2026 update)
* **Penulis:** Guangwu Xu, Wei Yu, Ke Han, Pengfei Lu
* **Problem Kritis:**
  Perhitungan perkalian skalar $[k]P$ pada kurva eliptik pairing-friendly (terutama $G_1$ dari kurva BLS12-381) adalah operasi komputasi terberat dalam pembuktian Groth16, evaluasi kombinasi linier input publik, serta derivasi kunci publik note ephemeral di [`zeltra-core/`](file:///workspaces/Zeltra-Protocol/zeltra-core/). Algoritma GLV standar membutuhkan tabel prekomputasi besar dan rentan terhadap serangan analisis saluran samping (*side-channel timing attacks*) jika pengolahan bit skalar bocor saat eksekusi.
* **Inovasi Paper:**
  Memperluas algoritma window $\tau$-NAF Solinas ke famili kurva $y^2 = x^3 + b$ di atas $\mathbb{F}_p$ dengan $p \equiv 1 \pmod 3$, yang secara spesifik mencakup kurva **BLS12-381** ($b = 4$):
  1. Memanfaatkan endomorfisme $\tau = [\omega]$ dalam gelanggang bilangan bulat Eisenstein $\mathbb{Z}[\omega]$, di mana pemetaan $\tau$ dalam koordinat proyektif Jacobian hanya membutuhkan **1 perkalian medan ($1M$)**.
  2. Menghasilkan formula point-tripling baru hanya dengan $6M + 2S$ (mengungguli rekor sebelumnya $7M + 2S$).
  3. Memanfaatkan simetri lipat-enam (*sixfold symmetry*) di bawah grup unit $\mathbb{Z}[\omega]^\times$, memangkas memori tabel prekomputasi hingga **5/6 bagian (penghematan 83%)**.
  4. Merumuskan varian reguler (*regular window $\tau$-NAF*) yang kebal terhadap serangan side-channel, sekaligus meningkatkan efisiensi sebesar **18.2%–20.5%** dibanding metode GLV tercanggih saat ini.

---

## 13. Defeating Null-Branch Witness Attacks in Conditional ZK-UTXO Circuits (IACR ePrint 2026/2235)
* **File Jurnal / ePrint:** IACR ePrint 2026/2235 (September 2026)
* **Penulis:** Guoxiao Liu, Zhuoqing Peng, Guowei Liu, Keting Jia, Xiaoyun Wang, Zongyue Wang, Junfeng Fan
* **Problem Kritis:**
  Banyak sirkuit ZK modern yang memuat percabangan kondisional (seperti apakah suatu slot input bersifat aktif atau *dummy*, apakah ada *change note*, atau apakah rollover aktif). Paper ini mengungkap celah fatal bernama **Null-Branch Witness Attack**: ketika sebuah perkalian constraint dikalikan dengan faktor nol (misal $(1 - \text{is\_active}) \cdot \dots$ atau $\text{has\_change} \cdot \dots$), intermediate wire di cabang yang tidak aktif sering kali dibiarkan bebas tanpa ikatan (*unconstrained zero factor*). Akibatnya, prover jahat dapat mengisi nilai acak yang valid secara matematis pada wire tersebut untuk memalsukan bukti (*public-key-only forgery*) tanpa melanggar *soundness* sirkuit tingkat tinggi.
* **Inovasi Paper:**
  Menganalisis kesenjangan mendasar antara *One-Way Function (OWF) Hardness* dan *Proof-System Soundness*, membuktikan bahwa verifier menerima bukti palsu karena relasi arithmetisasi sirkuit membiarkan cabang null memiliki ruang solusi bebas. Paper ini merumuskan teknik *Quadratic Encoding Soundness* yang secara ketat mengunci seluruh wire pada cabang non-aktif ke konstanta deterministik kanonikal.
* **Implementasi di Zeltra Core (Hardening Aljabar — DEC-046):**
  * Komponen: Sirkuit JoinSplit di [`zeltra-core/src/joinsplit_circuit.rs`](file:///workspaces/Zeltra-Protocol/zeltra-core/src/joinsplit_circuit.rs) (`DEC-036A`), gadget bit-packing di [`zeltra-core/src/circuit_gadgets.rs`](file:///workspaces/Zeltra-Protocol/zeltra-core/src/circuit_gadgets.rs), dan PrivateNoteCircuit di [`zeltra-core/src/note_circuit.rs`](file:///workspaces/Zeltra-Protocol/zeltra-core/src/note_circuit.rs).
  * Memberikan justifikasi kriptografis krusial untuk arsitektur Zeltra:
    - Ketika slot kedua adalah dummy (`is_dummy == true`), komitmen dan nullifier dummy **wajib di-pin ke konstanta nol kanonikal** ($C = 0, \text{nullifier} = 0, \text{asset\_id} = 0$) melalui constraint kuadratik ketat, bukan sekadar membatalkan penjumlahan nilai uang.
    - Pada percabangan `has_change_1` dan `has_change_2`, jika note kembalian tidak ada, nilai amount dan blinding factor dikunci ke nilai kanonikal agar penyerang tidak dapat mengeksploitasi *null-branch witness degrees of freedom*.
    - Menjamin anti-aliasing inverse `diff_inv` di-pin ke nol kanonikal saat `is_dummy == 1` via gerbang pendamping kuadratik (*Proposition 5 Zero-Safe Inverse*).

---

## 14. Dixon Resultants Algebraic Cryptanalysis & Parameter Hardening for Poseidon-W5 (IACR ePrint 2026/1969)
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

## 15. Forkcipher-Based Committing Security & Context-Binding AEAD (IACR ePrint 2026/2110)
* **File Jurnal / ePrint:** IACR ePrint 2026/2110 (September 2026)
* **Penulis:** Elena Andreeva, Maria Eichlseder, Simon Gerhalter, Marcel Nageler, Oliver Christoph Popa, Andreas Weninger
* **Problem Kritis:**
  Banyak skema Authenticated Encryption (AE/AEAD) konvensional tidak memberikan jaminan *context-committing security*—yaitu mengikat ciphertext secara kriptografis unik ke konteks enkripsinya. Ketiadaan jaminan ini membuka celah keamanan di mana ciphertext valid dapat dipindahkan atau didekripsi dalam konteks domain yang berbeda tanpa terdeteksi (*ciphertext recycling / multi-context malleability*), seperti menukar memo antar-transaksi atau antar-epoch.
* **Inovasi Paper:**
  Mengembangkan committing AE dari properti resistensi tabrakan eksplisit pada *forkcipher*. Mengonstruksi FHashN dan skema AEaH-2K yang menggabungkan AE aman dengan variable-input-length collision-resistant PRF. Memperkenalkan mode AEAD berbasis forkcipher tunggal yang mencapai kerahasiaan, keaslian (*authenticity*), dan keamanan komitmen CMT-4 dalam model standar terbukti.
* **Implementasi di Zeltra Core:**
  * Komponen: Enkripsi memo 96-byte di [`zeltra-core/src/note.rs`](file:///workspaces/Zeltra-Protocol/zeltra-core/src/note.rs) (`DEC-037`), ChaCha20-Poly1305 AEAD, dan domain tagging Poseidon.
  * Menjadi fondasi pengikatan konteks (*context-committing binding*) pada enkripsi memo: ciphertext memo 96-byte diikat langsung dengan komitmen note ($C$) dan epoch ID ($E$) sebagai *Associated Data (AAD)*, memastikan memo terenkripsi tidak dapat dipalsukan, ditukar, atau di-replay ke transaksi koin lain (*CMT-4 security level*).

---

## 16. Top Gun: Degree Annihilation Attacks on Poseidon & Security Margins (IACR ePrint 2026/1254)
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

## 17. Slipway: Accessing Finite Subspace Trails & Full-Round Absorption in Poseidon (IACR ePrint 2026/1579)
* **File Jurnal / ePrint:** IACR ePrint 2026/1579 (September 2026)
* **Penulis:** Giuseppe Vitto
* **Problem Kritis:**
  Analisis ketahanan Poseidon terhadap *finite subspace trails* (jejak sub-ruang hingga) pada putaran parsial. Jika suatu arah konstan melintasi beberapa putaran parsial berturut-turut, S-box tidak menambah derajat aljabar. Paper ini mengungkap fenomena *full-round absorption*: jika matriks linier MDS dan konstanta putaran memiliki relasi aljabar tersembunyi, penyerang dapat mengonstruksi keluarga input khusus yang melintasi 4 putaran penuh awal tanpa mengalami pertumbuhan derajat aljabar, membuka akses langsung ke subspace trail dan menghasilkan serangan CICO-2/CICO-3 penuh pada instansiasi Poseidon tertentu.
* **Inovasi Paper:**
  Mendemonstrasikan bahwa matriks MDS yang memenuhi kriteria uji standar desainer Poseidon pun masih bisa memiliki sifat *reachability* ke subspace trail jika terdapat relasi aljabar dengan putaran penuh prefix. Merumuskan batas matematis pada dimensi trail dan citra matriks (*matrix-image bound*) untuk mendeteksi serta mencegah fenomena full-round absorption.
* **Implementasi di Zeltra Core:**
  * Komponen: Generator matriks Cauchy MDS di [`zeltra-core/src/poseidon.rs`](file:///workspaces/Zeltra-Protocol/zeltra-core/src/poseidon.rs) dan generator Grain-128 LFSR (`DEC-021`).
  * Menjadi panduan verifikasi formal untuk matriks MDS Poseidon Zeltra: memastikan matriks MDS $5 \times 5$ (W5) dan $3 \times 3$ (W3) di-generate murni dari ekspansi Grain-128 LFSR yang independen secara statistik dari round constants, memutus total kemungkinan *full-round absorption* dan menjamin tidak ada subspace trail yang dapat diakses dari input manapun.
