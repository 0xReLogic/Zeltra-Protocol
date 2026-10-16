# TASK C (DEC-043C): Relayer Dual-Memo Ingress, Client Note Wallet & Trial Decryption Recovery (`zeltra-node` & `zeltra-sdk`)

> **Scope:** `zeltra-node/` (Axum 0.8, Ingress Preflight, Alloy Broadcast) & `zeltra-sdk/` (Client Private Note Wallet, Groth16 Prover, Knapsack, Multi-Asset Recovery)  
> **Spesifikasi Referensi:** [`research/decisions/DEC-043C-relayer-dual-memo-ingress-and-sdk-in-pool-wallet.md`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-043C-relayer-dual-memo-ingress-and-sdk-in-pool-wallet.md) & [`research/decisions/DEC-043-shielded-to-shielded-in-pool-transfer-and-merchant-masking.md`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-043-shielded-to-shielded-in-pool-transfer-and-merchant-masking.md)  
> **Prinsip:** *"Fail-Closed Relayer Preflight, Zero-Leak Client Proving, Seamless Merchant Synchronization"*  
> **Status:** **COMPLETED & VERIFIED**

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
- [x] **Request DTO (`zeltra-node/src/dto.rs`):**
  - Tambahkan `encrypted_memo_1: String` dan `encrypted_memo_2: String` pada struct `JoinSplitSpendRequest`.
- [x] **Preflight Handler (`zeltra-node/src/handlers/spend.rs`):**
  - Validasi panjang memo: tolak jika bukan tepat 192 karakter hex.
  - Validasi Strict 3-Mode: tolak jika Mode 2 (In-Pool Transfer) memiliki `recipient != 0x0`, `protocol_fee > 0`, atau `has_change_1 == 0`.
  - Tolak jika Mode 3 (Public Spend) memiliki `recipient == 0x0`.
- [x] **Broadcast Transaksi L2 (`zeltra-node/src/evm_client.rs`):**
  - Konversi hex memo menjadi bytes dan teruskan ke pemanggilan contract `spend_joinsplit`.

### B. Client SDK & Note Wallet (`zeltra-sdk/src/wallet/note_wallet.rs`)
- [x] **Implementasi `prepare_shielded_transfer_proof`:**
  - Validasi `transfer_amount > 0`.
  - Parsing alamat ZSA 64-byte (Hex atau Bech32m).
  - Seleksi koin via `select_notes_knapsack_asset`.
  - Output 1: Enkripsi memo penerima dan derivasi deterministik $r_1$.
  - Output 2: Enkripsi memo kembalian dengan `incoming_viewing_public_key()`. Jika tanpa kembalian, tetap enkripsi nilai 0 dengan kunci ephemeral acak.
  - Generate bukti Groth16 menggunakan `JoinSplitCircuit` (Mode 2: `merchant_amount = 0`, `protocol_fee = 0`, `recipient = Address::ZERO`).
- [x] **Pembaruan `recover_notes_batch`:**
  - Dekripsi memo menggunakan `decrypt_note_memo_deterministic`.
  - Abaikan (*skip*) note yang memiliki `plaintext.value == 0`.
  - Rekonstruksi komitmen daun dan simpan status unspent ke state dompet.

---

## 3. Matriks Pengujian Unit & E2E

Sesuai aturan engineering Zeltra: **1 Positive Test + Minimal 2x Negative Tests**:

### Positive Tests:
- [x] `test_e2e_shielded_transfer_flow`: Alice mentransfer ke Bob. Proof Mode 2 terbentuk dengan dual memo 96B, relayer menerima via preflight, transaksi dieksekusi di kontrak, Bob memanggil `recover_notes_batch()`, saldo Bob bertambah, saldo Alice berkurang tepat `transfer + execution_fee`.
- [x] `test_exact_shielded_transfer_without_change`: Alice membayar pas tanpa kembalian. Output 2 terenkripsi acak, diabaikan oleh scanner, saldo kedua belah pihak tetap akurat.

### Negative Tests (Minimal 2x Positive = Minimal 4 Tests):
- [x] `test_prepare_transfer_rejects_zero_amount`: Me-revert jika mencoba mentransfer `amount == 0`.
- [x] `test_prepare_transfer_rejects_insufficient_balance`: Me-revert jika saldo tidak cukup menutupi `transfer + max_execution_fee`.
- [x] `test_relayer_rejects_invalid_memo_hex_length`: Relayer menolak request jika memo bukan 192 hex chars.
- [x] `test_relayer_rejects_non_zero_recipient_in_pool`: Relayer menolak request in-pool jika `recipient != 0x0`.
- [x] `test_prepare_transfer_rejects_corrupted_zsa`: Me-revert jika string ZSA penerima memiliki checksum rusak.

### Real-Case Independent Python Verification Suite (`scripts/test_real_case_dec043c_inpool_wallet.py`):
- [x] **Section 1: BIP-350 Bech32m & ZSA Cross-Verification (7/7 tests pass)**
  * HRP `zs1` canonical decoding, 64-byte payload parsing, BLS12-381 $Fr < r$ scalar validation, checksum corruption rejection, wrong HRP rejection.
- [x] **Section 2: Real-World In-Pool Transfer Alice -> Bob 5.00 USDC (13/13 tests pass)**
  * Alice Groth16 Mode 2 proof synthesis, dual 96B AEAD memos, `recipient=0x0`, `merchant_amount=0`, `protocol_fee=0`, independent Python trial decryption with Bob IVK, Arkworks pairing engine verification, Relayer preflight ingress acceptance, Bob note discovery & balance credit ($5.00), Alice change balance credit ($4.98).
- [x] **Section 3: Exact Payment Without Change (6/6 tests pass)**
  * `has_change_2 == false`, Output 2 padded to 192 hex chars with ephemeral zero-note encryption (AUD-7), Bob recovers Output 1, Alice trial scanner skips zero-note cleanly without state pollution.
- [x] **Section 4: Relayer Ingress Adversary Attack & Threat Matrix (6/6 tests pass)**
  * Fail-closed rejection of truncated memo 1, oversized memo 2, non-zero recipient on Mode 2, non-zero protocol fee on Mode 2, `has_change_1 == false` on Mode 2, zero recipient on Mode 3 (AUD-4 anti-silent-burn).
- [x] **Section 5: Client SDK Negative & Boundary Conditions (3/3 tests pass)**
  * Immediate rejection of zero transfer amount (AUD-6), insufficient balance knapsack rejection, corrupted Bech32m ZSA rejection.
- [x] **Section 6: Eavesdropper & Adversary Tampering Resistance (2/2 tests pass)**
  * Eve trial decryption fails closed with Poly1305 MAC failure, bit-flipped ciphertext authentication failure.
- [x] **Section 7: Dual-Asset In-Pool Transfer WETH Rail (4/4 tests pass)**
  * WETH in-pool transfer on `asset_id = 0` (0.50 WETH), Bob recovery & balance credit, Alice balance credit (1.49995 WETH), zero cross-asset bleed.
- **Hasil Akhir Python Suite:** **44/44 TESTS PASSED (100% SUCCESS)**.

---

## 4. Strict Engineering Guardrails (Zero-Slop Policy)

1. **NO PLACEHOLDERS:** Dilarang menggunakan `todo!()`, `unimplemented!()`, dummy byte arrays, atau mock proofs di jalur production.
2. **COMPILE-CLEAN:** Wajib lulus `cargo check -p zeltra-node` dan `cargo check -p zeltra-sdk` dengan 0 error dan 0 warning.
3. **FORMAT & CLIPPY:** Wajib bersih `cargo fmt --check` dan `cargo clippy -p zeltra-node -p zeltra-sdk -- -D warnings`.
