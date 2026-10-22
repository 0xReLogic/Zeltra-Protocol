# Inovasi Jurnal Kriptografi Terverifikasi & Terimplementasi (Arsip)

Dokumen ini mengarsipkan inovasi dan riset jurnal kriptografi dari `jurnal/filter/` yang **telah selesai 100% diimplementasikan, diverifikasi, dan diuji** di dalam codebase Zeltra Protocol.

Dokumentasi arsitektur formal dan keputusan teknis permanen untuk inovasi ini tersimpan di [`research/decisions/`](../../research/decisions/).

---

## 1. Committing AEAD: Partitioning Oracle Attack Defense for Encrypted Memos (IACR ePrint 2026/240)
* **File Jurnal / ePrint:** IACR ePrint 2026/240 (Oktober 2026)
* **Penulis:** Kazuhiko Minematsu and Akiko Inoue
* **Problem Kritis:**
  Pada skema AEAD konvensional (seperti ChaCha20Poly1305 naif), sebuah ciphertext dapat valid didekripsi oleh dua kunci berbeda menjadi dua plaintext berbeda. Kerentanan ini (*non-committing AEAD flaw*) memungkinkan penyerang mengeksekusi *partitioning oracle attacks* pada protokol berbasis UTXO/memo: adversary menyuntikkan memo crafted on-chain yang membingungkan scanner wallet penerima dan dapat memicu desinkronisasi saldo atau deanonymization.
* **Inovasi Paper:**
  Mengevaluasi dan memperbaiki transformasi generik Committing AEAD (seperti CTX Chan-Rogaway dan Chen-Karadžić) yang rentan terhadap instansiasi hash praktis. Paper ini merumuskan transformasi committing AEAD yang terbukti kokoh di bawah *standard model* dengan modifikasi algoritmik minimal tanpa mengorbankan performa enkripsi/dekripsi simetris.
* **Implementasi di Zeltra Core & SDK (SELESAI & TERVERIFIKASI [x] — DEC-048):**
  * Komponen: [`zeltra-core/src/memo.rs`](../../zeltra-core/src/memo.rs) (`encrypt_memo_committing_aead` & `decrypt_memo_committing_aead`), [`zeltra-core/src/note.rs`](../../zeltra-core/src/note.rs), scanner note di [`zeltra-sdk/src/wallet/note_wallet/sync.rs`](../../zeltra-sdk/src/wallet/note_wallet/sync.rs), dan test harness [`scripts/test_dec048_committing_aead.py`](../../scripts/test_dec048_committing_aead.py).
  * Mengadopsi generic transform `EtH-KS` (Minematsu & Inoue, 2026) dengan Key Separation berbasis RCR-PRG $G(S) \to (K_{\text{enc}}, K_{\text{cmt}}, N_7)$ dan CR-PRF $H(K_{\text{cmt}}, N_7 \parallel A \parallel C)$.
  * Menjamin 96-byte encrypted memo yang dipancarkan lewat event `NoteCommitmentAppended` on-chain mengikat komitmen ciphertext secara matematis (*binding ciphertext to key & plaintext*), membendung partitioning attacks (`AUD-21`, `AUD-23`, `AUD-24`) dan menjamin keaslian data scanning note wallet 100% fail-closed.
  * Teruji 100% lolos melintasi 11 suite pengujian adversarial Python dan 61/61 test lifecycle multi-asset di `scripts/test_real_case_dec043c_inpool_wallet.py`.
* **Arsitektur Referensi:** [`DEC-048`](../../research/decisions/DEC-048-committing-aead-partitioning-oracle-defense-and-key-binding-memos.md).

---

## 2. Jacobian Diagnostics for Under-Constrained Zero-Knowledge Circuits (IACR ePrint 2026/1852)
* **File Jurnal / ePrint:** IACR ePrint 2026/1852 (Oktober 2026)
* **Penulis:** Vijay Singh
* **Problem Kritis:**
  *Under-constrained arithmetic circuits* adalah salah satu celah keamanan paling mematikan dalam protokol zero-knowledge. Jika suatu wire saksi (*witness wire*) atau variabel perantara dalam sirkuit R1CS tidak terikat secara ketat (*under-constrained*), seorang prover jahat dapat menetapkan nilai alternatif tanpa melanggar persamaan verifikasi (misalnya menduplikasi proof, memanipulasi nilai kembalian uang *change*, atau mengubah flag multi-aset). Pengujian fuzzer acak sering kali gagal mendeteksi celah ini pada sirkuit berukuran puluhan ribu gerbang.
* **Inovasi Paper:**
  Memperkenalkan metodologi diagnostik aljabar berbasis aljabar linier matriks Jacobian eksak. Dengan menggabungkan matriks Jacobian dari seluruh *constraint* R1CS dan menetapkan koordinat input publik, *kernel* dari Jacobian tersebut merepresentasikan *Zariski tangent space*. Jika terdapat arah pergerakan dalam kernel (*tangent dimension $> 0$*), hal tersebut membuktikan secara matematis keberadaan derajat kebebasan infinitesimal (*infinitesimal freedom / non-uniqueness certificate*). Alat ini memverifikasi sirkuit hingga 66.000 constraints hanya dalam hitungan detik.
* **Implementasi di Zeltra Core (SELESAI & TERVERIFIKASI [x] — DEC-047):**
  * Komponen: [`zeltra-core/src/jacobian.rs`](../../zeltra-core/src/jacobian.rs), [`zeltra-core/src/formal_tests.rs`](../../zeltra-core/src/formal_tests.rs), dan [`scripts/test_dec046_dec047_jacobian_soundness.py`](../../scripts/test_dec046_dec047_jacobian_soundness.py) (`DEC-047`).
  * Mengimplementasikan Sparse Augmented Jacobian $M(a) = \begin{pmatrix} J(a) \\ E_F \end{pmatrix}$, eliminasi Gauss sparse Markowitz over $\mathbb{F}_p$, row-space test $e_j^T \in \text{rowspan}(M(a))$ untuk kekakuan kawat simultan, dan generator sertifikat garis afin Proposisi 27.
  * Teruji 100% lolos melintasi matriks 6 keluarga saksi adversarial (FAM-1 s/d FAM-6) dengan $r_T = 0$ (26/26 formal tests passed).
* **Arsitektur Referensi:** [`DEC-047`](../../research/decisions/DEC-047-jacobian-diagnostics-and-tangent-space-rigidity-auditing-for-underconstrained-circuits.md).

---

## 3. Defeating Null-Branch Witness Attacks in Conditional ZK-UTXO Circuits (IACR ePrint 2026/2235)
* **File Jurnal / ePrint:** IACR ePrint 2026/2235 (September 2026)
* **Penulis:** Guoxiao Liu, Zhuoqing Peng, Guowei Liu, Keting Jia, Xiaoyun Wang, Zongyue Wang, Junfeng Fan
* **Problem Kritis:**
  Banyak sirkuit ZK modern yang memuat percabangan kondisional (seperti apakah suatu slot input bersifat aktif atau *dummy*, apakah ada *change note*, atau apakah rollover aktif). Paper ini mengungkap celah fatal bernama **Null-Branch Witness Attack**: ketika sebuah perkalian constraint dikalikan dengan faktor nol (misal $(1 - \text{is\_active}) \cdot \dots$ atau $\text{has\_change} \cdot \dots$), intermediate wire di cabang yang tidak aktif sering kali dibiarkan bebas tanpa ikatan (*unconstrained zero factor*). Akibatnya, prover jahat dapat mengisi nilai acak yang valid secara matematis pada wire tersebut untuk memalsukan bukti (*public-key-only forgery*) tanpa melanggar *soundness* sirkuit tingkat tinggi.
* **Inovasi Paper:**
  Menganalisis kesenjangan mendasar antara *One-Way Function (OWF) Hardness* dan *Proof-System Soundness*, membuktikan bahwa verifier menerima bukti palsu karena relasi arithmetisasi sirkuit membiarkan cabang null memiliki ruang solusi bebas. Paper ini merumuskan teknik *Quadratic Encoding Soundness* yang secara ketat mengunci seluruh wire pada cabang non-aktif ke konstanta deterministik kanonikal.
* **Implementasi di Zeltra Core (SELESAI & TERVERIFIKASI [x] — DEC-046):**
  * Komponen: Sirkuit JoinSplit di [`zeltra-core/src/joinsplit_circuit.rs`](../../zeltra-core/src/joinsplit_circuit.rs) (`DEC-036A`), gadget bit-packing di [`zeltra-core/src/circuit_gadgets.rs`](../../zeltra-core/src/circuit_gadgets.rs), dan PrivateNoteCircuit di [`zeltra-core/src/note_circuit.rs`](../../zeltra-core/src/note_circuit.rs).
  * Memberikan justifikasi kriptografis krusial untuk arsitektur Zeltra:
    - Ketika slot kedua adalah dummy (`is_dummy == true`), komitmen dan nullifier dummy **wajib di-pin ke konstanta nol kanonikal** ($C = 0, \text{nullifier} = 0, \text{asset\_id} = 0$) melalui constraint kuadratik ketat, bukan sekadar membatalkan penjumlahan nilai uang.
    - Pada percabangan `has_change_1` dan `has_change_2`, jika note kembalian tidak ada, nilai amount dan blinding factor dikunci ke nilai kanonikal agar penyerang tidak dapat mengeksploitasi *null-branch witness degrees of freedom*.
    - Menjamin anti-aliasing inverse `diff_inv` di-pin ke nol kanonikal saat `is_dummy == 1` via gerbang pendamping kuadratik (*Proposition 5 Zero-Safe Inverse*).
  * Terverifikasi dengan 16 negative tests pada saksi tampered yang tertolak mutlak.
* **Arsitektur Referensi:** [`DEC-046`](../../research/decisions/DEC-046-null-branch-witness-attack-defense-and-quadratic-encoding-soundness.md).

---

## 4. Leakage-Resilient Decapsulation & CT-FOE Defense (IACR CiC Vol. 3, No. 1, 2026)
* **File Jurnal / ePrint:** IACR Communications in Cryptology (Vol. 3, No. 1, Mei 2026)
* **Penulis:** Duyên Pay, Thomas Peters, and François-Xavier Standaert (UCLouvain Crypto Group)
* **Problem Kritis:**
  Serangan *Flag Differential Power Analysis (Flag DPA)* dan *Control-Flow Timing Leakage* pada proses trial-decryption note terenkripsi (memo 96-byte). Pada skema konvensional, keberadaan operator *short-circuit* (`&&`/`||`), penghentian dini (*early return/abort*), dan perbedaan waktu kalkulasi mengekspos sinyal kendali boolean `is_valid` via lonjakan arus/EM dan mikrodetik variasi eksekusi, memungkinkan penyerang mengekstraksi kunci privat viewing key (`IncomingViewingKey` / `ivk`) pengguna.
* **Inovasi Paper:**
  Menganalisis resistensi kebocoran KEM (*leakage-resilient decapsulation*) dan membuktikan bahwa sekadar menghapus FO re-encryption tidak cukup jika evaluasi penanda validitas (*validity flag*) membocorkan informasi melalui saluran samping fisik. Merumuskan evaluasi bendera tanpa kebocoran (*Constant-Time Flag Oblivious Evaluation / CT-FOE*) di mana seluruh penanda validitas dimasker secara bitwise dan operasi dekripsi selalu dieksekusi penuh tanpa *early abort*.
* **Implementasi di Zeltra Core & SDK (SELESAI & TERVERIFIKASI [x] — DEC-049):**
  * Komponen: [`zeltra-core/src/memo_ct.rs`](../../zeltra-core/src/memo_ct.rs) (`ConstantTimeMemoScanner`, `CtNotePlaintext`, `is_canonical_fr_ct`), integrasi scanner klien di [`zeltra-sdk/src/wallet/note_wallet/sync.rs`](../../zeltra-sdk/src/wallet/note_wallet/sync.rs), harness statistik [`zeltra-sdk/src/bin/dudect_ct_foe.rs`](../../zeltra-sdk/src/bin/dudect_ct_foe.rs), dan test suite [`zeltra-sdk/tests/side_channel_defense_tests.rs`](../../zeltra-sdk/tests/side_channel_defense_tests.rs).
  * Mengeliminasi seluruh *control-flow branching* pada trial-decryption memo 96B; akumulasi penanda validitas via bitwise AND `subtle::Choice`.
  * Mempertahankan waktu eksekusi identik melalui substitusi titik dummy generator jika menerima kurva non-contributory/orde rendah, dekripsi ChaCha20 selalu penuh, dan zeroization kondisional branchless.
  * Teruji 100% lolos Gate V-2 (5 kategori adversarial matrix), Gate V-3 evaluasi statistik diferensial Dudect Welch t-test $|t| < 4.5$ tanpa false positive, dan Gate V-4 inspeksi objdump linear bebas instruksi jump rahasia.
* **Arsitektur Referensi:** [`DEC-049`](../../research/decisions/DEC-049-leakage-resilient-decapsulation-and-constant-time-flag-defense.md).
