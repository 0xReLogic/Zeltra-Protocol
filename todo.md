# Nimbus Protocol TODO (Active Backlog)

> **Catatan:** File history lengkap sebelum pembersihan tersimpan di [`archive/todo.full-archive.md`](file:///workspaces/Zeltra-Protocol/archive/todo.full-archive.md).
> Seluruh task yang **sudah selesai (`[x]`)** telah diarsipkan agar dokumen ini murni berisi **task pending (`[ ]`)** dan arah kerja yang jelas.

---

## Ringkasan Status Saat Ini (Yang Sudah Berhasil)

Komponen inti berikut **sudah selesai dan terbukti di Arbitrum Sepolia**:
* **Smart Contract Stylus:** EIP-2537 BLS pairing check (`e(-alpha, G2) * e(H(m), pk_iss) == 1`), 13/13 testnet negative tests lolos.
* **BLS Threshold Cluster:** 1 Leader + 4 Guardians (3-of-5 threshold) dengan Tailscale binding dan atomic masking key release.
* **Persistent Queue:** SQLite/SQLCipher persistent spend queue dengan leasing dan exponential backoff.
* **Adaptive Batch Spend:** Entrypoint `batch_spend()` (2-8 item) terdeploy di Stylus dengan EIP-712 quote validation.
* **CCIP Tracking:** Event parsing untuk message ID dan background monitor destination.

---

## 1. Top Priority: Solusi Saldo & Kembalian (The "Voucher" Fix)

Pilih salah satu dari 2 arah berikut sebelum mulai ngoding:
* **Arah A (Jalur Cepat - Ecash / Denominations):** Tetap pakai BLS Blind Signature yang sudah ada, tapi tambahkan mekanisme swap/split pecahan (mirip Cashu/Fedimint). Tidak butuh circuit ZK baru.
* **Arah B (Jalur ZK-UTXO - Gate C0):** Selesaikan circuit Groth16 untuk 1-in 1-out private note dengan change output di bawah ini.

### Gate C0 - Security Repair Private Note Circuit (Jika Memilih Arah B)
Ref: [`docs/todos/private-note-balance.md`](file:///workspaces/Zeltra-Protocol/docs/todos/private-note-balance.md), [`DEC-016A (Frozen Spec)`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-016A-private-note-spec-freeze.md), & [`DEC-016B (Shortcuts Fix)`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-016B-mvp-circuit-shortcuts.md)

- [x] **Bind Merkle path bits ke `input_leaf_index` di circuit**:
  - Dekomposisi indeks menjadi tepat `MERKLE_TREE_DEPTH` boolean bits.
  - Enforce `input_leaf_index == sum(bit_i * 2^i)` dan `< 2^MERKLE_TREE_DEPTH`.
  - Buktikan note yang sama tidak dapat menghasilkan dua nullifier valid hanya dengan mengganti leaf index.
- [x] **Integer value & range safety**:
  - Range constrain input value, merchant payout, protocol fee, execution fee, dan change ke `[0, 2^64)`.
  - Pastikan value conservation adalah integer USDC (cegah modular wrap modulo scalar field Fr).
  - Boolean-constrain `has_change` ke `{0, 1}` (cegah prover nge-scale output commitment).
  - Enforce jika `has_change == 0` maka `change_value == 0` dan output commitment nol.
- [x] **Payment and domain binding**:
  - Definisikan canonical signed quote digest yang mengikat: recipient, merchant amount, protocol fee, execution fee, expiry, chain ID, contract address, asset ID, dan nonce.
  - Enforce digest tersebut di circuit via Poseidon-W5 non-linear lane hashing dan cocokkan dengan quote.
- [ ] **Cryptographic parameters**:
  - Ganti parameter Poseidon width-5 ad-hoc (StdRng) dengan parameter standar/audited.
  - Simpan proving/verifying key sebagai artifact versioned; [x] cache development key pada test runner.

---

## 2. Smart Contract Stylus (`nimbus-contracts`)

- [x] **Multi-Liability Storage Accounting**:
  - Integrasikan storage terpisah: `user_note_liability`, `refundable_deposit_liability`, `accrued_execution_fee_liability`.
  - Saat spend, kurangi user liability sebesar payout + protocol fee + execution fee.
  - Tolak fee claim yang menyentuh backing deposit user/refund.
- [x] **Append-only Note Commitment Tree (Gate D)**:
  - Tambahkan state Merkle tree on-chain dan bounded accepted-root history di Stylus.
  - Initial commitment hanya bisa di-mint dari deposit confirmed sebesar net deposit.
- [ ] **CCIP Contract Hardening**:
  - Wajibkan `ccip_router != Address::ZERO` sebelum menerima message.
  - Tolak semua caller jika router belum dikonfigurasi.
  - Validasi `source_chain_selector` dan bind sender contract yang sah untuk setiap chain.
- [ ] **Fee & Events**:
  - Pastikan refund akibat gagal issuance/settlement tidak dikenai protocol fee.
  - Tambahkan event terpisah untuk deposit fee, protocol fee, execution fee, change commitment, dan fee claim.

---

## 3. Relayer & Node Settlement (`nimbus-node`)

- [ ] **Receipt Finality & Nonce Handling**:
  - Ref: [`docs/todos/receipt-finality.md`](file:///workspaces/Zeltra-Protocol/docs/todos/receipt-finality.md)
  - Terapkan confirmation threshold sesuai chain.
  - Tangani replacement transaction otomatis jika transaksi stuck di mempool.
  - Deteksi dan tangani nonce collision antar worker.
  - Ekspos API status transaksi: `GET /api/tx-status?tx_hash=...` (pending/confirmed/failed).
- [ ] **Deposit Event Indexer On-Chain**:
  - Gantikan pencatatan deposit berbasis API murni dengan event listener on-chain dari smart contract.
  - Verifikasi `k * pk_iss == stored com_k` sebelum resolve session.
- [ ] **Input & Safety Validation**:
  - Validasi recipient address EVM valid, batas min/max amount, dan chain selector allowlist.
  - Rekonsiliasi status database dengan nullifier contract setelah node restart.
- [ ] **Batch & Fee Metrics**:
  - Tambahkan batch profitability metrics ke health endpoint & DB (`batch_count`, `total_batch_margin_usdc`, `avg_batch_size`).

---

## 4. Cross-Chain CCIP Testing & Verification

- [ ] **Testnet E2E Hard-Test (Arbitrum Sepolia)**:
  - Uji alur lengkap: source broadcast $\rightarrow$ router $\rightarrow$ destination execution $\rightarrow$ destination confirmation.
  - Uji jalur kegagalan: destination failure memicu alur refund 24h dan double payout rejection.

---

## 5. Security, Secrets & Infrastructure

- [ ] **Credential Rotation & Git Cleanup**:
  - Rotasi seluruh secret/private key yang pernah ter-commit di masa lalu.
  - Hapus credential lama dari git history.
- [ ] **KMS / Vault TLS Hardening**:
  - Ref: [`docs/todos/kms-tls.md`](file:///workspaces/Zeltra-Protocol/docs/todos/kms-tls.md)
  - Ganti raw TCP HTTP client dengan HTTPS client tervalidasi untuk Vault dan guardian RPC.
  - Verifikasi certificate TLS.
- [ ] **Operasional API**:
  - Terapkan timeout pada guardian, Vault, dan RPC requests.
  - Bersihkan in-memory rate-limit map secara berkala agar tidak memory leak.
  - Tambahkan readiness dan liveness endpoint terpisah.

---

## 6. Integrasi Eksternal (Opsional / Secondary)

- [ ] **x402 Facilitator**:
  - Ref: [`docs/todos/x402.md`](file:///workspaces/Zeltra-Protocol/docs/todos/x402.md)
  - Ganti mock tx hash dengan status settlement nyata.
  - Ganti recipient `"x402-facilitator-pool"` dengan konfigurasi address EVM nyata.
- [ ] **Polymarket Intent Settlement**:
  - Uji alur intent settlement privat dan fallback refund di testnet.

---

## 7. Mainnet Readiness Gate

- [ ] Seluruh invariant akuntansi terbukti solvent: `contract assets >= all liabilities`.
- [ ] Tidak ada fallback mock atau bypass `#[cfg(test)]` pada binary production.
- [ ] Queue dan settlement tahan restart di setiap titik transisi.
- [ ] Audit keamanan eksternal independen untuk smart contract Stylus, relayer node, dan cryptographic circuits.
