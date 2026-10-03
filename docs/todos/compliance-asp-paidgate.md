# Verifiable Provenance, ASP & Institutional PaidGate

**Priority:** Tier 2 (Future Extension / Institutional Compliance)  
**Status:** Backlog (Bebas dikerjakan kapan saja setelah Gate C0 & Core ZK-UTXO stabil)  
**Ref:** [`docs/bisnis.md`](file:///workspaces/Zeltra-Protocol/docs/bisnis.md) (Bab 2.1 & Bab 8) & [`PROJECT_X_PRD_Whitepaper_TechSpec.md`](file:///workspaces/Zeltra-Protocol/PROJECT_X_PRD_Whitepaper_TechSpec.md)

---

## 1. Filosofi & Konteks

1. **Non-Custodial Absolut:** Nimbus tidak memegang kunci privat, spending key, atau viewing key pengguna. Tidak ada master key atau backdoor untuk membuka identitas secara sepihak.
2. **Self-Provable Innocence:** Pengguna jujur memiliki kemampuan membuktikan depositnya berasal dari *clean set* (Association Set) tanpa membuka jejak dompet publiknya.
3. **Penyaringan Otomatis di Pintu Masuk:** Aktor jahat/dana curian tidak akan bisa menghasilkan ZK proof ke *clean association set*, sehingga otomatis tertahan/ditolak di pintu masuk exchange atau merchant yang mewajibkan verifikasi.
4. **Netralitas Protokol:** Nimbus adalah jalan tol matematika netral. Pemilihan daftar acuan kepatuhan adalah tanggung jawab verifier (exchange/merchant), bukan ditentukan secara terpusat oleh protokol.

---

## 2. Rincian Task & Komponen

### A. Association Set Provider (ASP) Registry on-chain
- [ ] Buat kontrak `AspRegistry.sol` atau modul Stylus Rust:
  - Menyimpan mapping root Merkle himpunan deposit bersih per ASP: `mapping(bytes32 asp_id => mapping(bytes32 pool_id => bytes32 root))`.
  - Riwayat root berbatas waktu (`root_history`) agar proof tidak langsung kedaluwarsa saat ASP memperbarui set.
- [ ] Verifier/merchant menetapkan policy ID berisi ASP yang mereka akui.
- [ ] Hubungkan sirkuit ZK dengan pembuktian keanggotaan `label ∈ assoc_root`.

### B. Institutional PaidGate Contract (B2B Enterprise)
- [ ] Buat kontrak gerbang verifikasi berbayar `PaidGate`:
  - Jalur gratis tetap ada via RPC / SDK offline untuk pengguna ritel biasa.
  - Jalur institusi berbayar mengenakan biaya per panggilan atau sistem saldo prabayar (`topUp` per `verifier_id`).
  - Mencatat anti-replay nonce on-chain dan memancarkan event audit `Verified(verifier_id, policy_id, nonce, timestamp)`.
  - Fungsi `sweep()` permissionless untuk meneruskan seluruh fee akumulasi ke Protocol Treasury.

### C. Emergency Exit (Ragequit) SDK Helper
- [ ] Implementasikan helper method `emergencyRefund(session_id)` di Nimbus SDK:
  - Memanggil `claim_refund()` pada smart contract setelah timelock 24 jam.
  - Memberikan jaminan kepada pengguna bahwa jika transaksi privat mereka ditolak oleh policy ASP tertentu, dana 100% dapat ditarik kembali ke alamat penyetor asal secara transparan non-privat tanpa tertahan selamanya.

### D. Selective Source Disclosure (Mode P2 - Opsional)
- [ ] Fitur pengungkapan sumber terarah di SDK:
  - Pengguna dapat mengenkripsi alamat deposit asal ke public key milik verifier tertentu (misal auditor atau compliance officer exchange).
  - Verifier dapat membaca data sumber secara privat tanpa membocorkan alamat pengguna ke publik blockchain.

---

## 3. Acceptance Criteria (Bila Dikerjakan)

- [ ] Kontrak ASP registry bersifat permissionless atau terkurasi transparan tanpa admin key backdoor.
- [ ] Kegagalan verifikasi pada `PaidGate` tidak memotong saldo verifier (anti-griefing).
- [ ] Emergency refund selalu berhasil setelah timelock 24 jam jika kredensial belum berstatus spent.
- [ ] Unit tests dan property tests untuk alur registry, pembayaran fee, dan refund darurat lolos 100%.
