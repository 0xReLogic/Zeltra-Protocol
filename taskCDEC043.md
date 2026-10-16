# TASK C (DEC-043C): Relayer Dual-Memo Ingress, Client Note Wallet & Trial Decryption Recovery (`zeltra-node` & `zeltra-sdk`)

> **Scope:** `zeltra-node/` (Axum 0.8, Ingress Preflight, Alloy Broadcast) & `zeltra-sdk/` (Client Private Note Wallet, Groth16 Prover, Knapsack, Multi-Asset Recovery)  
> **Spesifikasi Referensi:** [`research/decisions/DEC-043C-relayer-dual-memo-ingress-and-sdk-in-pool-wallet.md`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-043C-relayer-dual-memo-ingress-and-sdk-in-pool-wallet.md) & [`research/decisions/DEC-043-shielded-to-shielded-in-pool-transfer-and-merchant-masking.md`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-043-shielded-to-shielded-in-pool-transfer-and-merchant-masking.md)  
> **Prinsip:** *"Fail-Closed Relayer Preflight, Zero-Leak Client Proving, Seamless Merchant Synchronization"*  
> **Status:** **PENDING IMPLEMENTATION**

---

## 1. Latar Belakang & Tujuan Arsitektur

Menyediakan integrasi menyeluruh (*end-to-end*) antara client SDK pengirim, relayer sequencer, dan client SDK penerima (merchant):
1. **Relayer Preflight Ingress Guards (`zeltra-node`):**
   Memastikan request transaksi in-pool mematuhi Strict 3-Mode State Machine dan mewajibkan calldata `encrypted_memo_1` dan `encrypted_memo_2` tepat berukuran 192 karakter heksadesimal (96 byte) secara fail-closed sebelum masuk ke mempool sequencer (Resolusi AUD-4 & AUD-5).
2. **Client Private Note Wallet Prover (`zeltra-sdk`):**
   Mengimplementasikan `prepare_shielded_transfer_proof` yang menerima ZSA 64-byte penerima, memvalidasi `transfer_amount > 0` (Resolusi AUD-6), memilih note via Knapsack, menurunkan trapdoor $r_1$ secara deterministik via ECDH X25519 (Resolusi AUD-2), mengenkripsi memo kembalian dengan Montgomery public key pengirim (Resolusi AUD-3), dan mengenkripsi dummy memo secara acak pada transfer pas (Resolusi AUD-7).
3. **Multi-Asset Trial Decryption Recovery (`zeltra-sdk`):**
   Memperbarui `recover_notes_batch` agar mendekripsi memo on-chain, mengabaikan dummy note bernilai 0, merekonstruksi trapdoor $r$, dan mengkredit saldo note penerima secara otomatis.

---

## 2. Checklist Implementasi Rinci

### A. Relayer Ingress & Preflight (`zeltra-node`)
- [ ] **Request DTO (`zeltra-node/src/dto.rs`):**
  - Tambahkan `encrypted_memo_1: String` dan `encrypted_memo_2: String` pada struct `JoinSplitSpendRequest`.
- [ ] **Preflight Handler (`zeltra-node/src/handlers/spend.rs`):**
  - Validasi panjang memo: tolak jika bukan tepat 192 karakter hex.
  - Validasi Strict 3-Mode: tolak jika Mode 2 (In-Pool Transfer) memiliki `recipient != 0x0`, `protocol_fee > 0`, atau `has_change_1 == 0`.
  - Tolak jika Mode 3 (Public Spend) memiliki `recipient == 0x0`.
- [ ] **Broadcast Transaksi L2 (`zeltra-node/src/evm_client.rs`):**
  - Konversi hex memo menjadi bytes dan teruskan ke pemanggilan contract `spend_joinsplit`.

### B. Client SDK & Note Wallet (`zeltra-sdk/src/wallet/note_wallet.rs`)
- [ ] **Implementasi `prepare_shielded_transfer_proof`:**
  - Validasi `transfer_amount > 0`.
  - Parsing alamat ZSA 64-byte (Hex atau Bech32m).
  - Seleksi koin via `select_notes_knapsack_asset`.
  - Output 1: Enkripsi memo penerima dan derivasi deterministik $r_1$.
  - Output 2: Enkripsi memo kembalian dengan `incoming_viewing_public_key()`. Jika tanpa kembalian, tetap enkripsi nilai 0 dengan kunci ephemeral acak.
  - Generate bukti Groth16 menggunakan `JoinSplitCircuit` (Mode 2: `merchant_amount = 0`, `protocol_fee = 0`, `recipient = Address::ZERO`).
- [ ] **Pembaruan `recover_notes_batch`:**
  - Dekripsi memo menggunakan `decrypt_note_memo_deterministic`.
  - Abaikan (*skip*) note yang memiliki `plaintext.value == 0`.
  - Rekonstruksi komitmen daun dan simpan status unspent ke state dompet.

---

## 3. Matriks Pengujian Unit & E2E

Sesuai aturan engineering Zeltra: **1 Positive Test + Minimal 2x Negative Tests**:

### Positive Tests:
- [ ] `test_e2e_shielded_transfer_flow`: Alice mentransfer ke Bob. Proof Mode 2 terbentuk dengan dual memo 96B, relayer menerima via preflight, transaksi dieksekusi di kontrak, Bob memanggil `recover_notes_batch()`, saldo Bob bertambah, saldo Alice berkurang tepat `transfer + execution_fee`.
- [ ] `test_exact_shielded_transfer_without_change`: Alice membayar pas tanpa kembalian. Output 2 terenkripsi acak, diabaikan oleh scanner, saldo kedua belah pihak tetap akurat.

### Negative Tests (Minimal 2x Positive = Minimal 4 Tests):
- [ ] `test_prepare_transfer_rejects_zero_amount`: Me-revert jika mencoba mentransfer `amount == 0`.
- [ ] `test_prepare_transfer_rejects_insufficient_balance`: Me-revert jika saldo tidak cukup menutupi `transfer + max_execution_fee`.
- [ ] `test_relayer_rejects_invalid_memo_hex_length`: Relayer menolak request jika memo bukan 192 hex chars.
- [ ] `test_relayer_rejects_non_zero_recipient_in_pool`: Relayer menolak request in-pool jika `recipient != 0x0`.
- [ ] `test_prepare_transfer_rejects_corrupted_zsa`: Me-revert jika string ZSA penerima memiliki checksum rusak.

---

## 4. Strict Engineering Guardrails (Zero-Slop Policy)

1. **NO PLACEHOLDERS:** Dilarang menggunakan `todo!()`, `unimplemented!()`, dummy byte arrays, atau mock proofs di jalur production.
2. **COMPILE-CLEAN:** Wajib lulus `cargo check -p zeltra-node` dan `cargo check -p zeltra-sdk` dengan 0 error dan 0 warning.
3. **FORMAT & CLIPPY:** Wajib bersih `cargo fmt --check` dan `cargo clippy -p zeltra-node -p zeltra-sdk -- -D warnings`.
