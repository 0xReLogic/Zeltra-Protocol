# Inovasi Jurnal untuk Smart Contract (Stylus WASM — `nimbus-contracts`)

Dokumen ini memetakan paper terpilih untuk arsitektur smart contract Nimbus ([`nimbus-contracts`](file:///workspaces/Zeltra-Protocol/nimbus-contracts/)). Smart contract Stylus menuntut efisiensi WASM, biaya komputasi rendah, dan storage invariant ketat (`contract USDC balance >= liabilities`).

---

## 1. Bonsai: Scalable Private Payments (IACR ePrint 2026/1987)
* **File Jurnal:** [`jurnal/Bonsai-Scalable-Private-Payments.md`](file:///workspaces/Zeltra-Protocol/jurnal/Bonsai-Scalable-Private-Payments.md)
* **Problem di Sistem Klasik:**
  Di Tornado Cash & Zcash, mapping nullifier bertambah secara monotonik selamanya ($O(N)$ state bloat). Kontrak tidak pernah bisa menghapus nullifier lama karena takut diserang double-spend. Pada EVM/Stylus, biaya penyimpanan gas meledak seiring berjalannya tahun.
* **Inovasi Paper:**
  Bonsai mengeliminasi kebutuhan validator menyimpan seluruh riwayat nullifier historis dengan menyusun komitmen akun berbasis *algebraic witness* dan *hierarchical state pruning*. State yang disimpan validator terikat secara ringkas, sementara beban bukti eksistensi/non-keanggotaan dipindahkan ke prover client.
* **Implementasi di Nimbus Stylus:**
  * Komponen: [`nimbus-contracts/src/storage.rs`](file:///workspaces/Zeltra-Protocol/nimbus-contracts/src/storage.rs) dan [`nimbus-contracts/src/spend.rs`](file:///workspaces/Zeltra-Protocol/nimbus-contracts/src/spend.rs).
  * Mekanisme: Integrasi dengan circular buffer `accepted_note_roots` yang sudah ada di Nimbus. State nullifier dapat dipartisi per epoch rolling (misal 30 hari). Nullifier di luar batas aktif dapat dibersihkan tanpa mengorbankan insolvensi berkat verifikasi range epoch on-chain.

---

## 2. A Note on Notes: Evolving Nullifiers (IACR ePrint 2025/2031)
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

## 3. Rate-Limiting Nullifiers for Gasless Sequencer Admission (IACR ePrint 2025)
* **File Jurnal:** [`jurnal/Rate-Limiting-Nullifiers-Gasless-Sequencer.md`](file:///workspaces/Zeltra-Protocol/jurnal/Rate-Limiting-Nullifiers-Gasless-Sequencer.md)
* **Problem di Sistem Klasik:**
  Endpoint relayer gasless/meta-transaction rentan terhadap serangan spam DoS dan drained gas reserves karena siapa pun bisa mengirim request spend palsu tanpa modal.
* **Inovasi Paper:**
  Rate-Limiting Nullifiers (RLN) berbasis Shamir Secret Sharing di finite field. Pengguna dapat mengeksekusi 1 transaksi per epoch secara gratis dan anonim. Jika mencoba mengirim 2 transaksi pada epoch yang sama, secret key pengguna secara otomatis terbongkar (terbuka di public) dan deposit/stakenya disita on-chain sebagai slashing.
* **Implementasi di Nimbus Stylus:**
  * Komponen: Kontrak pendaftaran relayer & staking pool di Arbitrum Sepolia.
  * Perlindungan otomatis terhadap relayer `nimbus-node`: relayer tidak akan rugi gas karena transaksi spam ganda dapat langsung di-slash di kontrak Stylus.

---

## 4. zk-Rollup Verification on BLS12-381 (IACR ePrint 2025/1390)
* **File Jurnal:** [`jurnal/zk-Rollup-Verification-BLS12-381.md`](file:///workspaces/Zeltra-Protocol/jurnal/zk-Rollup-Verification-BLS12-381.md)
* **Problem di Sistem Klasik:**
  Verifikasi pairing BLS12-381 via EIP-2537 (`0x0f`) dan Groth16 membutuhkan parsing byte yang presisi. Salah endianness atau titik tak termalformasi (subgroup check) menyebabkan revert mahal atau celah silent falsification.
* **Inovasi Paper:**
  Formulasi representasi koordinat terkompresi dan batching pre-evaluasi pairing multi-skalar untuk memangkas konsumsi gas on-chain hingga 35%.
* **Implementasi di Nimbus Stylus:**
  * Optimasi langsung fungsi `batch_spend()` di [`nimbus-contracts/src/spend.rs`](file:///workspaces/Zeltra-Protocol/nimbus-contracts/src/spend.rs), mengagregasi 2–8 verifikasi BLS EIP-2537 dalam satu panggilan precompile terpadu.
