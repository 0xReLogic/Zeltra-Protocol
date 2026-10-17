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

## 4. ammBoost: State Growth Control for Accumulators (IACR ePrint 2024/1021)
* **File Jurnal / PDF:** [`jurnal/pdf/2024-1021.pdf`](file:///workspaces/Zeltra-Protocol/jurnal/pdf/2024-1021.pdf)
* **Penulis:** Nicolas Michel, Mohamed E. Najd, Ghada Almashaqbeh (2024/2025)
* **Problem di Sistem Klasik:**
  Pertumbuhan pohon Merkle dan state accumulator on-chain yang tak terbatas menyebabkan pembengkakan storage kontrak dan degradasi performa sinkronisasi node.
* **Inovasi Paper:**
  Mekanisme *bounded historical root window* dan *cryptographic state pruning* terverifikasi: membatasi kedalaman riwayat root yang disimpan on-chain tanpa merusak verifikasi bukti keanggotaan Merkle.
* **Implementasi di Zeltra Stylus:**
  * Komponen: [`zeltra-contracts/src/merkle.rs`](file:///workspaces/Zeltra-Protocol/zeltra-contracts/src/merkle.rs) (ring buffer `root_history` size 100 dan `accepted_note_roots` mapping).
  * Menjaga ukuran storage Stylus WASM tetap terkendali dan hemat gas, mencegah memory bloat pada validator Arbitrum.

---

## 5. ZK Email (NFA-based Regex): Identity & Recovery Engine (IACR ePrint 2026)
* **File Jurnal:** [`jurnal/ZK-Proofs-Regex-Email-Verification.md`](file:///workspaces/Zeltra-Protocol/jurnal/ZK-Proofs-Regex-Email-Verification.md)
* **Problem di Sistem Klasik:**
  Verifikasi bukti kepemilikan email DKIM secara on-chain di EVM/Stylus konvensional membutuhkan evaluasi string regex yang memakan jutaan gas dan puluhan ribu constraint sirkuit jika menggunakan automata DFA naif.
* **Inovasi Paper:**
  Penggunaan $\varepsilon$-free Non-deterministic Finite Automata (NFA) dengan model *off-circuit matching* dan *in-circuit path verification*. Pembuktian regex header DKIM email dipadatkan secara dramatis sehingga verifikasinya murah dan efisien dieksekusi native di WASM Arbitrum Stylus.
* **Implementasi di Zeltra Stylus:**
  * **Social Recovery:** Pengguna dapat memulihkan akses kunci note atau mereset viewing key menggunakan bukti DKIM email terverifikasi on-chain tanpa pihak ketiga tepercaya.
  * **Enterprise Whitelisting:** Pembuktian mandat zero-knowledge dari domain korporat (misal `@perusahaan.com`) langsung diverifikasi di kontrak Stylus untuk memenuhi syarat kepatuhan B2B tanpa mengungkap alamat email spesifik karyawan.


