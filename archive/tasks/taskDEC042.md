# Task DEC-042: Privacy-Preserving HTTP 402 AI Agent Facilitator & Pure ZK-UTXO Micropayments

**Status:** COMPLETED & AUDITED (PR #27)  
**Date:** 2026-10-16  
**Reference ADR:** [`research/decisions/DEC-042-privacy-preserving-x402-facilitator-and-pure-zk-utxo-micropayments.md`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-042-privacy-preserving-x402-facilitator-and-pure-zk-utxo-micropayments.md)  
**Security Level:** CRITICAL (M2M Solvency, Anti-Double-Spend, Replay Immunity, Zero-Gas-Griefing)

---

## Ringkasan Eksekusi

Modernisasi menyeluruh modul `x402` di `zeltra-sdk` dan `zeltra-node`:
1. **Pemusnahan Total Komponen Legacy BLS:** Menghapus seluruh parameter `alpha_neg_hex`, `hm_hex`, `pk_iss_hex`, dan fungsi on-chain mati `spendCall` / `batchSpendCall`.
2. **Adopsi Pure ZK-UTXO Wire-Format:** Mengintegrasikan `PrivateNoteSpendRequest` (16 Public Inputs, Groth16 Proof BLS12-381) ke dalam `X402PaymentSignature` header dengan dukungan backward-compatible alias.
3. **Dynamic Economic Dust Threshold:** Mengimplementasikan threshold dinamis di `coin_selection.rs` mengikuti `execution_fee` aktual ($[0.01, 0.10]$ USDC clamp) sesuai standar Marginal Cost to Spend Bitcoin Core & Zcash ZIP 317.
4. **Fail-Closed Facilitator Verification Pipeline:** Mengintegrasikan validasi skalar kanonikal, dual-layer nullifier check, dan local CPU Groth16 preflight check (DEC-026 C-01) pada endpoint `POST /api/x402/verify`.
5. **Universal Merchant Payout:** Settlement on-chain menyalurkan pembayaran bersih (USDC/WETH) ke alamat EVM publik merchant tanpa potongan liabilitas ganda.

---

## Checklist Task

### Fase 1: Client SDK Modernization (`zeltra-sdk/src/x402/`)
- [x] Update `zeltra-sdk/src/x402/types.rs`:
  - Ganti `ZeltraPaymentPayload` dengan `X402ZeltraPayment` (berisi field ZK-UTXO kanonikal: `asset_id`, `note_root`, `leaf_count`, `input_nullifier`, `output_commitment`, `recipient`, `merchant_amount`, `protocol_fee`, `execution_fee`, `proof_a_neg`, `proof_b`, `proof_c`, `public_inputs`, `change_encrypted_memo`).
  - Selaraskan `X402PaymentSignature` dengan versi x402 v2.
- [x] Update `zeltra-sdk/src/x402/codec.rs`:
  - Hapus fungsi usang `build_zeltra_payment_signature` dan `build_zeltra_payment_signature_for_merchant_amount`.
  - Tambahkan builder modern `build_x402_zk_payment_signature(...)`.
- [x] Update `zeltra-sdk/src/x402/mod.rs`:
  - Perbarui unit test roundtrip base64 encode/decode untuk payload ZK-UTXO baru.
- [x] Dynamic Economic Dust Threshold (`zeltra-sdk/src/coin_selection.rs`):
  - Implementasikan `compute_dynamic_dust_threshold(execution_fee, asset_id)` pada single-note dan joinsplit knapsack paths.

### Fase 2: Relayer Facilitator Modernization (`zeltra-node/`)
- [x] Update `zeltra-node/src/dto.rs`:
  - Selaraskan `X402ZeltraPayment` dengan versi `zeltra-sdk`.
  - Tambahkan implementasi `From<X402ZeltraPayment> for PrivateNoteSpendRequest`.
- [x] Update `zeltra-node/src/handlers/x402.rs`:
  - Validasi protokol version (`x402Version == 2`).
  - Validasi skalar kanonikal (`< r`).
  - Cek nullifier double-spend di persistent database (`state.db.is_nullifier_spent`).
  - Validasi kecukupan nominal dan alamat recipient.
  - Fail-closed Groth16 preflight check (`validate_private_note_spend_preflight`).
  - Enqueue spend request sebagai ZK private note (`state.db.enqueue_spend`).
- [x] Purge Dead Legacy Code:
  - Hapus `function spend(...)` dan `function batchSpend(...)` dari blok `sol!` di `zeltra-node/src/evm_client.rs`.
  - Hapus implementasi dead `broadcast_spend_transaction(...)` dan `broadcast_spend_batch(...)` di `zeltra-node/src/evm_client.rs`.
  - Bersihkan pemanggilan dead batch di `zeltra-node/src/handlers/spend.rs`.

### Fase 3: Testing & Soundness
- [x] Tambahkan suite pengujian komprehensif di `zeltra-node/tests/x402_facilitator_tests.rs`:
  - Positive tests: E2E valid Groth16 payment verification & queue persistence.
  - Negative tests (rasio $\ge 2\times$): Unsupported version, non-canonical scalar, duplicate nullifier, corrupted curve points, recipient mismatch, underpayment.
- [x] Jalankan seluruh workspace tests:
  - `cargo check --workspace`
  - `cargo test -p zeltra-sdk --lib x402`
  - `cargo test -p zeltra-node --test x402_facilitator_tests`
  - `cargo fmt --check`
  - `cargo clippy --workspace -- -D warnings`

### Fase 4: Dokumentasi, Knowledge Graph & PR
- [x] Update `todo.md` & `docs/todos/private-note-balance.md`.
- [x] Jalankan `graphify update .`.
- [x] Arsipkan task ke `archive/tasks/taskDEC042.md`.
- [x] Commit, push, dan buka Pull Request upstream.
