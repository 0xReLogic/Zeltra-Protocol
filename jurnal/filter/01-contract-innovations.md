# Inovasi Jurnal untuk Smart Contract (Stylus WASM — `zeltra-contracts`)

Dokumen ini memetakan paper terpilih untuk arsitektur smart contract Zeltra ([`zeltra-contracts`](file:///workspaces/Zeltra-Protocol/zeltra-contracts/)). Smart contract Stylus menuntut efisiensi WASM, biaya komputasi rendah, dan storage invariant ketat (`contract USDC balance >= liabilities`).

---

## 1. Rate-Limiting Nullifiers for Gasless Sequencer Admission (IACR ePrint 2025)
* **File Jurnal:** [`jurnal/Rate-Limiting-Nullifiers-Gasless-Sequencer.md`](file:///workspaces/Zeltra-Protocol/jurnal/Rate-Limiting-Nullifiers-Gasless-Sequencer.md)
* **Problem di Sistem Klasik:**
  Endpoint relayer gasless/meta-transaction rentan terhadap serangan spam DoS dan drained gas reserves karena siapa pun bisa mengirim request spend palsu tanpa modal.
* **Inovasi Paper:**
  Rate-Limiting Nullifiers (RLN) berbasis Shamir Secret Sharing di finite field. Pengguna dapat mengeksekusi 1 transaksi per epoch secara gratis dan anonim. Jika mencoba mengirim 2 transaksi pada epoch yang sama, secret key pengguna secara otomatis terbongkar (terbuka di public) dan deposit/stakenya disita on-chain sebagai slashing.
* **Implementasi di Zeltra Stylus:**
  * Komponen: Kontrak pendaftaran relayer & staking pool di Arbitrum Sepolia.
  * Perlindungan otomatis terhadap relayer `zeltra-node`: relayer tidak akan rugi gas karena transaksi spam ganda dapat langsung di-slash di kontrak Stylus.

---

## 2. FlatMerkle: In-Contract Arbitrum Accumulator & Gas-Cost Optimization (IACR ePrint 2026/2468)
* **File Jurnal / ePrint:** IACR ePrint 2026/2468 (Oktober 2026)
* **Penulis:** Sergio Demian Lerner
* **Problem di Sistem Klasik:**
  Pembaruan pohon Merkle biner konvensional di smart contract L2 sering kali memakan biaya gas storage write (`SSTORE`) yang membengkak per transaksi deposit. Selain itu, verifikasi bukti inklusi membutuhkan transmisi rantai node saudara ($O(\log N)$ hashes) yang membebani calldata.
* **Inovasi Paper:**
  Merumuskan *FlatMerkle*, struktur akumulator berbasis re-injeksi checkpoint status internal fungsi hash secara periodik ke dalam linear hash stream. Menghasilkan digest tunggal tanpa format komitmen yang terpisah, sambil mempertahankan bukti inklusi kompak $O(\log N)$ dan pembuktian malformasi logaritmik. Paper ini menyajikan analisis komparasi gas in-contract nyata secara komparatif di **Arbitrum Nitro, Ethereum, dan Rootstock**, menyingkap bagaimana arsitektur kompresi calldata dan eksekusi WASM di Arbitrum mengubah efisiensi pemrosesan state on-chain.
* **Implementasi di Zeltra Stylus:**
  * Komponen: [`zeltra-contracts/src/merkle.rs`](file:///workspaces/Zeltra-Protocol/zeltra-contracts/src/merkle.rs) (Merkle Mountain Range & ring buffer storage).
  * Menjadi landasan optimasi arsitektur Merkle Mountain Range (MMR) Stylus WASM Zeltra: memanfaatkan keunggulan eksekusi komputasi Arbitrum Nitro untuk meminimalkan write storage slot, menjaga konsumsi gas deposit langsung (`deposit_direct_note`) tetap flat dan ultra-murah.

---

## 3. zk-Rollup Verification on BLS12-381 (IACR ePrint 2025/1390)
* **File Jurnal:** [`jurnal/zk-Rollup-Verification-BLS12-381.md`](file:///workspaces/Zeltra-Protocol/jurnal/zk-Rollup-Verification-BLS12-381.md)
* **Problem di Sistem Klasik:**
  Verifikasi pairing BLS12-381 via EIP-2537 (`0x0f`) dan Groth16 membutuhkan parsing byte yang presisi. Salah endianness atau titik tak termalformasi (subgroup check) menyebabkan revert mahal atau celah silent falsification.
* **Inovasi Paper:**
  Formulasi representasi koordinat terkompresi dan batching pre-evaluasi pairing multi-skalar untuk memangkas konsumsi gas on-chain hingga 35%.
* **Implementasi di Zeltra Stylus:**
  * Optimasi langsung fungsi verifikasi Groth16 di [`zeltra-contracts/src/groth16_note_verifier.rs`](file:///workspaces/Zeltra-Protocol/zeltra-contracts/src/groth16_note_verifier.rs) dan [`groth16_joinsplit_verifier.rs`](file:///workspaces/Zeltra-Protocol/zeltra-contracts/src/groth16_joinsplit_verifier.rs), mengagregasi verifikasi BLS EIP-2537 (`0x0c` MSM + `0x0f` 4-pairing check) dalam panggilan precompile terpadu.

---

## 4. ZK Email (NFA-based Regex): Identity & Recovery Engine (IACR ePrint 2026)
* **File Jurnal:** [`jurnal/ZK-Proofs-Regex-Email-Verification.md`](file:///workspaces/Zeltra-Protocol/jurnal/ZK-Proofs-Regex-Email-Verification.md)
* **Problem di Sistem Klasik:**
  Verifikasi bukti kepemilikan email DKIM secara on-chain di EVM/Stylus konvensional membutuhkan evaluasi string regex yang memakan jutaan gas dan puluhan ribu constraint sirkuit jika menggunakan automata DFA naif.
* **Inovasi Paper:**
  Penggunaan $\varepsilon$-free Non-deterministic Finite Automata (NFA) dengan model *off-circuit matching* dan *in-circuit path verification*. Pembuktian regex header DKIM email dipadatkan secara dramatis sehingga verifikasinya murah dan efisien dieksekusi native di WASM Arbitrum Stylus.
* **Implementasi di Zeltra Stylus:**
  * **Social Recovery:** Pengguna dapat memulihkan akses kunci note atau mereset viewing key menggunakan bukti DKIM email terverifikasi on-chain tanpa pihak ketiga tepercaya.
  * **Enterprise Whitelisting:** Pembuktian mandat zero-knowledge dari domain korporat (misal `@perusahaan.com`) langsung diverifikasi di kontrak Stylus untuk memenuhi syarat kepatuhan B2B tanpa mengungkap alamat email spesifik karyawan.

---

## 5. Faster Incremental Authenticated Dictionaries: Removing the Query Bottleneck (IACR ePrint 2026/2098)
* **File Jurnal / ePrint:** IACR ePrint 2026/2098 (September 2026)
* **Penulis:** Michael A. Bender, John Kuszmaul, William Kuszmaul
* **Problem di Sistem Klasik:**
  Batasan teoritis klasik pada struktur data kamus terautentikasi (*authenticated dictionary*) melarang konstruksi berefisiensi tinggi jika mendukung penambahan (*insertions*) dan penghapusan (*deletions*) sekaligus. Pada sistem akumulator komitmen koin ZK-UTXO yang murni *insertion-only* (append-only), pohon Merkle biner konvensional menghadapi hambatan performa query pengambilan data (*retrieval query bottleneck*) dan ukuran bukti inklusi logaritmik $O(\log N)$ yang membebani komputasi on-chain.
* **Inovasi Paper:**
  Membuktikan secara definitif bahwa kamus terautentikasi inkremental yang murni *insertion-only* dapat secara tegas mendominasi (*strictly dominate*) pohon Merkle klasik. Memperkenalkan *exponential Verkle forest*, struktur data yang mendukung:
  1. Kueri pencarian (*retrieval queries*) dengan waktu dan ukuran bukti $O(\log \log N)$.
  2. Penyisipan daun (*insertions*) dengan waktu dan ukuran bukti $O(\log N)$, serta batas waktu *amortized* $O(1)$.
  3. Versi *full-query* yang mendukung kueri inklusi dan eksklusi sekaligus dengan waktu pencarian $O(\log \log N)$ optimal untuk solusi berwaktu konstruksi linear.
* **Implementasi di Zeltra Stylus:**
  * Komponen: [`zeltra-contracts/src/merkle.rs`](file:///workspaces/Zeltra-Protocol/zeltra-contracts/src/merkle.rs) (Akumulator Merkle Mountain Range & query traversal).
  * Menjadi justifikasi akademis kuat bagi arsitektur akumulator komitmen Zeltra: membuktikan bahwa akumulator append-only tanpa delesi memiliki batas efisiensi asimtotik yang jauh melampaui pohon statis biasa, memperkuat desain MMR Zeltra dengan amortized $O(1)$ hashing dan membuka potensi kompresi kueri pembuktian keanggotaan sub-logaritmik di masa depan.

---

## 6. DelegProof: Machine-Checked Security Analysis of EIP-7702 Delegation (IACR ePrint 2026/2060)
* **File Jurnal / ePrint:** IACR ePrint 2026/2060 (September 2026)
* **Penulis:** Rong Qian, Yu Cheng, Lingyu Gao, Yuchang Zhang, Zengli Guo
* **Problem Kritis:**
  EIP-7702 (diaktifkan pada Ethereum/Arbitrum via upgrade Pectra) memungkinkan akun EOA biasa mendelegasikan eksekusi transaksi ke kode smart contract dengan otorisasi tanda tangan tunggal. Di lapangan nyata, 63% delegasi teramati mengarah ke kontrak berbahaya (\$2.36M kerugian riil), rentan terhadap *cross-chain replay* (chainId-0), *front-run initialization*, kebingungan penyimpanan (*storage confusion*) saat re-delegasi, dan serangan substitusi tanda tangan ERC-1271 pada kontrak penerima.
* **Inovasi Paper:**
  Menyajikan *DelegProof*, analisis formal simbolis pertama yang diverifikasi secara mesin (*machine-checked* via Tamarin prover) untuk semantik otorisasi EIP-7702 yang dikomposisikan dengan pipeline ERC-4337 EntryPoint. Paper ini membuktikan bahwa:
  1. Melarang otorisasi dengan `chainId = 0` mengeliminasi serangan *cross-chain replay* secara mutlak.
  2. Gerbang inisialisasi yang terikat akun (*account-bound initialization gate*) memulihkan integritas otorisasi inisialisasi.
  3. Penggunaan slot penyimpanan bernamespace (*namespaced storage slots*) membuat kelas serangan *storage confusion* secara struktural mustahil terjadi (*structurally inexpressible*).
* **Implementasi di Zeltra Stylus:**
  * Komponen: [`zeltra-contracts/src/storage.rs`](file:///workspaces/Zeltra-Protocol/zeltra-contracts/src/storage.rs) (Stylus `sol_storage!` layout), [`zeltra-contracts/src/spend.rs`](file:///workspaces/Zeltra-Protocol/zeltra-contracts/src/spend.rs), dan [`zeltra-contracts/src/lib.rs`](file:///workspaces/Zeltra-Protocol/zeltra-contracts/src/lib.rs).
  * Memberikan validasi keamanan formal bagi smart contract Stylus Zeltra:
    - Penegakan ketat binding domain `chain_id` pada seluruh verifikasi Groth16 dan quote eksekusi ([`DEC-035B`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-035B-formal-scope-binding-and-groth16-proof-malleability-hardening.md)), menutup celah replay lintas chain secara mutlak.
    - Struktur penyimpanan `sol_storage!` Zeltra mengadopsi namespaced layout terisolasi untuk data multi-aset, riwayat MMR, dan generational nullifiers, menjamin kekebalan penuh terhadap *storage confusion* jika kontrak Zeltra dipanggil oleh akun EOA yang didelegasikan via EIP-7702 atau ERC-4337 smart account.

