# Inovasi Jurnal untuk Smart Contract (Stylus WASM — `nimbus-contracts`)

Dokumen ini memetakan paper terpilih untuk arsitektur smart contract Nimbus ([`nimbus-contracts`](file:///workspaces/Zeltra-Protocol/nimbus-contracts/)). Smart contract Stylus menuntut efisiensi WASM, biaya komputasi rendah, dan storage invariant ketat (`contract USDC balance >= liabilities`).

---

## 1. A Note on Notes: Evolving Nullifiers (IACR ePrint 2025/2031)
* **File Jurnal:** [`jurnal/Note-on-Notes-Scalable-Anonymous-Payments.md`](file:///workspaces/Zeltra-Protocol/jurnal/Note-on-Notes-Scalable-Anonymous-Payments.md)
* **Penulis:** Sean Bowe, Ian Miers (Arsitek Utama Zerocash & Zcash)
* **Problem di Sistem Klasik:**
  Mengecek keanggotaan nullifier di dataset raksasa membutuhkan akses storage acak (SSTORE / SLOAD) berulang kali yang sangat mahal di smart contract layer.
* **Inovasi Paper:**
  *Evolving Nullifiers*: Nullifier ditransformasikan secara homomorfik menggunakan pseudo-random ratchet seiring bertambahnya epoch, memungkinkan *Oblivious Synchronization* dan verifikasi state ringkas.
* **Implementasi di Nimbus Stylus:**
  * Kontrak Stylus hanya perlu menyimpan hash accumulator aktif dari epoch saat ini.
  * User yang membelanjakan note lama wajib menyertakan bukti migrasi epoch (rollover). Storage on-chain tetap konstan $O(1)$ selamanya.

---

## 2. Rate-Limiting Nullifiers for Gasless Sequencer Admission (IACR ePrint 2025)
* **File Jurnal:** [`jurnal/Rate-Limiting-Nullifiers-Gasless-Sequencer.md`](file:///workspaces/Zeltra-Protocol/jurnal/Rate-Limiting-Nullifiers-Gasless-Sequencer.md)
* **Problem di Sistem Klasik:**
  Endpoint relayer gasless/meta-transaction rentan terhadap serangan spam DoS dan drained gas reserves karena siapa pun bisa mengirim request spend palsu tanpa modal.
* **Inovasi Paper:**
  Rate-Limiting Nullifiers (RLN) berbasis Shamir Secret Sharing di finite field. Pengguna dapat mengeksekusi 1 transaksi per epoch secara gratis dan anonim. Jika mencoba mengirim 2 transaksi pada epoch yang sama, secret key pengguna secara otomatis terbongkar (terbuka di public) dan deposit/stakenya disita on-chain sebagai slashing.
* **Implementasi di Nimbus Stylus:**
  * Komponen: Kontrak pendaftaran relayer & staking pool di Arbitrum Sepolia.
  * Perlindungan otomatis terhadap relayer `nimbus-node`: relayer tidak akan rugi gas karena transaksi spam ganda dapat langsung di-slash di kontrak Stylus.

---

## 3. zk-Rollup Verification on BLS12-381 (IACR ePrint 2025/1390)
* **File Jurnal:** [`jurnal/zk-Rollup-Verification-BLS12-381.md`](file:///workspaces/Zeltra-Protocol/jurnal/zk-Rollup-Verification-BLS12-381.md)
* **Problem di Sistem Klasik:**
  Verifikasi pairing BLS12-381 via EIP-2537 (`0x0f`) dan Groth16 membutuhkan parsing byte yang presisi. Salah endianness atau titik tak termalformasi (subgroup check) menyebabkan revert mahal atau celah silent falsification.
* **Inovasi Paper:**
  Formulasi representasi koordinat terkompresi dan batching pre-evaluasi pairing multi-skalar untuk memangkas konsumsi gas on-chain hingga 35%.
* **Implementasi di Nimbus Stylus:**
  * Optimasi langsung fungsi `batch_spend()` di [`nimbus-contracts/src/spend.rs`](file:///workspaces/Zeltra-Protocol/nimbus-contracts/src/spend.rs), mengagregasi 2–8 verifikasi BLS EIP-2537 dalam satu panggilan precompile terpadu.

---

## 4. Blind Spots in Blind Signatures: Chaumian Ecash System-Level Security (IACR ePrint 2026/2174)
* **File Jurnal:** [`jurnal/Blind-Spots-Chaumian-Ecash.md`](file:///workspaces/Zeltra-Protocol/jurnal/Blind-Spots-Chaumian-Ecash.md)
* **Penulis:** Huaifeng Chen, Yuchang Zhang, Yu Cheng
* **Problem di Sistem Klasik:**
  Sistem Chaumian ecash seperti Cashu tumbuh dari satu blind signature sederhana menjadi 31 dokumen spesifikasi, 58 mint, dan 7 wallet yang saling interoperabel — namun analisis keamanan selalu terbatas pada core blind signature saja. Insiden cross-mint theft 2026 terbukti bukan dari kelemahan kriptografi primitif, melainkan dari komposisi: keyset rotation bersamaan wallet recovery dan multi-mint identity.
* **Inovasi Paper:**
  Analisis keamanan level sistem pertama untuk Chaumian ecash. Mengidentifikasi tiga kelas serangan komposisional: (1) *keyset rotation race* — window di mana token lama masih valid tapi key baru sudah aktif, memungkinkan double-issuance; (2) *wallet recovery oracle* — endpoint backup memungkinkan enumerasi denominasi token yang dimiliki korban; (3) *cross-mint identity linkage* — pola deposit/withdraw lintas mint dapat dikaitkan via timing correlation.
* **Implementasi di Nimbus Stylus:**
  * Nimbus adalah Chaumian ecash di atas Arbitrum — semua attack surface yang ditemukan paper ini identik dengan Nimbus. Tiga mitigasi langsung:
  * **Keyset rotation:** Kontrak `nimbus-contracts/src/storage.rs` harus enforce atomic key transition — `trusted_issuer_keys` lama di-disable dalam satu transaksi yang sama dengan aktivasi key baru, tanpa window overlap.
  * **Denomination fingerprinting:** Fungsi `deposit()` dan `spend()` harus enforce denomination bucketing (misal: hanya 10, 100, 1000 USDC) agar amount tidak bisa digunakan sebagai identifier.
  * **Cross-session timing:** Settlement batcher di `batcher.rs` harus menambahkan random delay (1–5 blok) sebelum on-chain submission untuk memutus timing correlation antara deposit event dan spend event.

---

## 5. ammBoost: State Growth Control for AMMs (IACR ePrint 2024/1021)
* **File Jurnal / PDF:** [`jurnal/pdf/2024-1021.pdf`](file:///workspaces/Zeltra-Protocol/jurnal/pdf/2024-1021.pdf)
* **Penulis:** Nicolas Michel, Mohamed E. Najd, Ghada Almashaqbeh (2024/2025)
* **Problem di Sistem Klasik:**
  Pertumbuhan pohon Merkle dan state accumulator on-chain yang tak terbatas menyebabkan pembengkakan storage kontrak dan degradasi performa sinkronisasi node.
* **Inovasi Paper:**
  Mekanisme *bounded historical root window* dan *cryptographic state pruning* terverifikasi: membatasi kedalaman riwayat root yang disimpan on-chain tanpa merusak verifikasi bukti keanggotaan Merkle.
* **Implementasi di Nimbus Stylus:**
  * Komponen: [`nimbus-contracts/src/storage.rs`](file:///workspaces/Zeltra-Protocol/nimbus-contracts/src/storage.rs) (circular buffer `accepted_note_roots` dan LeanIMT Merkle tree).
  * Menjaga ukuran storage Stylus WASM tetap terkendali dan hemat gas, mencegah memory bloat pada validator Arbitrum.

---

## 6. ZK Email (NFA-based Regex): Identity & Recovery Engine (IACR ePrint 2026)
* **File Jurnal:** [`jurnal/ZK-Proofs-Regex-Email-Verification.md`](file:///workspaces/Zeltra-Protocol/jurnal/ZK-Proofs-Regex-Email-Verification.md)
* **Problem di Sistem Klasik:**
  Verifikasi bukti kepemilikan email DKIM secara on-chain di EVM/Stylus konvensional membutuhkan evaluasi string regex yang memakan jutaan gas dan puluhan ribu constraint sirkuit jika menggunakan automata DFA naif.
* **Inovasi Paper:**
  Penggunaan $\varepsilon$-free Non-deterministic Finite Automata (NFA) dengan model *off-circuit matching* dan *in-circuit path verification*. Pembuktian regex header DKIM email dipadatkan secara dramatis sehingga verifikasinya murah dan efisien dieksekusi native di WASM Arbitrum Stylus.
* **Implementasi di Nimbus Stylus:**
  * Komponen: [`nimbus-contracts/src/verifier/`](file:///workspaces/Zeltra-Protocol/nimbus-contracts/src/) dan modul pemulihan dompet.
  * **Social Recovery:** Pengguna dapat memulihkan akses kunci note atau mereset viewing key menggunakan bukti DKIM email terverifikasi on-chain tanpa pihak ketiga tepercaya.
  * **Enterprise Whitelisting:** Pembuktian mandat zero-knowledge dari domain korporat (misal `@perusahaan.com`) langsung diverifikasi di kontrak Stylus untuk memenuhi syarat kepatuhan B2B tanpa mengungkap alamat email spesifik karyawan.

---

## 7. NI-DKG on Blockchain: Zero-Complaint Guardian Setup (IACR ePrint 2026/552)
* **File Jurnal:** [`jurnal/NI-DKG-Non-Interactive-Distributed-Key-Generation-Blockchain-Zero-Knowledge-Proofs.md`](file:///workspaces/Zeltra-Protocol/jurnal/NI-DKG-Non-Interactive-Distributed-Key-Generation-Blockchain-Zero-Knowledge-Proofs.md)
* **Problem di Sistem Klasik:**
  Protokol Distributed Key Generation (DKG) interaktif tradisional membutuhkan fase sanggahan (*complaint phase*) multi-ronde. Jika terjadi network partition atau serangan sensor transaksi di L2, fase sanggahan bisa macet (*griefing delay*), menyebabkan kegagalan rotasi kunci kuorum Guardian.
* **Inovasi Paper:**
  Non-Interactive DKG (NI-DKG) berbasis blockchain dengan zero-knowledge proofs. Setiap node kandidat mengunggah komitmen koefisien polinomial Feldman beserta bukti zk-SNARK validitas pembagian kunci di muka. Smart contract secara deterministik memvalidasi keabsahan data tanpa memerlukan fase komplain interaktif antar node.
* **Implementasi di Nimbus Stylus:**
  * Komponen: Kontrak pendaftaran dan rotasi kuorum Guardian di [`nimbus-contracts/src/storage.rs`](file:///workspaces/Zeltra-Protocol/nimbus-contracts/src/storage.rs).
  * Mengotomatisasi rotasi kuorum threshold Guardian (misal ekspansi dari 3-of-5 ke 5-of-7) secara langsung on-chain di Stylus tanpa risiko macet atau sensor gas.


