# DEC-043B: Stylus Smart Contract Strict Three-Mode Settlement, Calldata Guards & Dual-Memo MMR Ingestion

- **Status:** APPROVED AS ARCHITECTURAL SUB-SPECIFICATION (Child of [`DEC-043`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-043-shielded-to-shielded-in-pool-transfer-and-merchant-masking.md))
- **Author:** Zeltra Protocol Architecture & Cryptography Team
- **Date:** 2026-10-16
- **Applies to:**
  - `zeltra-contracts/src/lib.rs` (External ABI entrypoints, dual memo calldata parameters)
  - `zeltra-contracts/src/spend.rs` (JoinSplit 3-mode state machine, calldata validation, ERC-20 bypass, MMR insertion)
  - `zeltra-contracts/src/events.rs` (Event logging NoteCommitmentAppended with encrypted memo)
  - `zeltra-contracts/src/storage.rs` (Solvency storage layout invariants)
- **Parent & Related DECs:**
  - [`DEC-043`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-043-shielded-to-shielded-in-pool-transfer-and-merchant-masking.md) (Master In-Pool Transfer Blueprint)
  - [`DEC-032`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-032-merkle-mountain-range-commitment-accumulator.md) (Merkle Mountain Range Commitment Accumulator)
  - [`DEC-034B`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-034B-stylus-siloed-solvency-payable-weth-wrap-and-multi-asset-settlement.md) (Stylus Siloed Solvency & Payable Auto-Wrap)
  - [`DEC-036C`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-036C-relayer-ingress-guard-and-stylus-joinsplit-settlement.md) (Universal JoinSplit Stylus Settlement)
  - [`DEC-038B`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-038B-contract-direct-mmr-deposit-and-storage-cleanup.md) (Contract Direct MMR Inflow Protocol)
- **Security Audit Resolusi:** AUD-4 (Silent Fund Burn / Loss Bug), AUD-5 (Relayer L1 Gas Griefing & Missing Memo Drop), AUD-6 (Client Gating of Zero Payout).
- **Architectural Paradigm:** *"Strict State Segregation, Anti-Silent-Burn, Fixed-Length Calldata Verification, Perfect Solvency Conservation"*

---

## 1. Executive Summary & Dilema Eksekusi On-Chain

Dalam smart contract Arbitrum Stylus WASM (`zeltra-contracts`), fungsi `spend_joinsplit` bertanggung jawab memvalidasi bukti Groth16, membakar nullifier, menyisipkan daun baru ke Merkle Mountain Range (MMR), dan memproses penyelesaian finansial.

Ketika menambahkan kapabilitas In-Pool Transfer (di mana dana tidak ditarik keluar dari brankas kontrak), muncul dua kerentanan tingkat tinggi:

### A. Bahaya Silent Fund Burn (AUD-4)
Pada desain awal, transaksi dianggap sebagai pembelanjaan reguler jika `is_rollover == 0`. Jika pengguna mengirim transaksi dengan `merchant_amount > 0` namun menyetel alamat penerima `recipient == Address::ZERO`, kontrak mendebit `user_note_liabilities` sebesar total nilai, tetapi pada blok interaksi:
```rust
if recipient != Address::ZERO && merchant_amount > U256::ZERO {
    self.asset_safe_transfer(target_token, recipient, raw_payout)?;
}
```
Transfer ERC-20 dilewati begitu saja karena `recipient == Address::ZERO`! Akibatnya, liabilitas pengguna terdebit, tetapi tidak ada seorang pun yang menerima token ERC-20 tersebut. Dana pengguna hangus (*burned*) secara diam-diam di brankas kontrak.

### B. Bahaya Calldata Gas Griefing & Silent Memo Drop (AUD-5)
Penggunaan tipe dinamis `Bytes` tanpa pemeriksaan panjang calldata memungkinkan penyerang mengirim bukti sah dengan menyisipkan payload memo berukuran puluhan kilobyte. Karena relayer yang menanggung biaya calldata L1 Arbitrum, hal ini membuka celah pengurasan ETH relayer (*L1 Data Gas Griefing*). Sebaliknya, jika klien lupa mengirim memo (`b""`), bukti tetap dieksekusi di kontrak tetapi penerima kehilangan note karena event log tidak memuat ciphertext untuk didekripsi.

---

## 2. Strict 3-Mode Mutually Exclusive State Machine

Untuk mengeliminasi seluruh ambiguitas state dan mencegah silent fund burn, DEC-043B mendefinisikan **tiga mode eksekusi yang saling terpisah secara mutlak** (*mutually exclusive*):

```text
┌────────────────────────────────────────────────────────────────────────────────────────┐
│               STYLUS JOINSPLIT 3-MODE EXECUTION STATE MACHINE                          │
└────────────────────────────────────────────────────────────────────────────────────────┘

 [Incoming Transaction Payload]
              │
              ├──► is_rollover == 1? ──► [MODE 1: Rollover (DEC-033)]
              │                          • merchant_amount MUST == 0
              │                          • protocol_fee MUST == 0
              │                          • recipient MUST == Address::ZERO
              │
              ├──► is_rollover == 0 && merchant_amount == 0? ──► [MODE 2: In-Pool Transfer (DEC-043)]
              │                                                  • has_change_1 MUST == 1
              │                                                  • protocol_fee MUST == 0
              │                                                  • recipient MUST == Address::ZERO
              │                                                  • ERC-20 Transfer Bypassed
              │
              └──► is_rollover == 0 && merchant_amount > 0? ──► [MODE 3: Public Outflow (DEC-036)]
                                                                 • recipient MUST != Address::ZERO
                                                                 • protocol_fee == 45 bps
                                                                 • ERC-20 Transfer Executed
```

### A. Tabel Spesifikasi Guard Tiga Mode
| Variabel / Kondisi | Mode 1 (Epoch Rollover) | Mode 2 (In-Pool Transfer) | Mode 3 (Public Outflow) |
|:---|:---:|:---:|:---:|
| `is_rollover` | `U256::from(1)` | `U256::ZERO` | `U256::ZERO` |
| `merchant_amount` | **Wajib `0`** | **Wajib `0`** | **Wajib `> 0`** |
| `protocol_fee` | **Wajib `0`** | **Wajib `0`** | 45 bps Outflow Fee |
| `recipient` | **Wajib `Address::ZERO`** | **Wajib `Address::ZERO`** | **Dilarang `Address::ZERO`** |
| `has_change_1` | `0` atau `1` | **Wajib `1` (Output 1 Transfer)** | `0` atau `1` |
| `has_change_2` | `0` | `0` (Pas) atau `1` (Kembalian) | `0` |
| **Transfer ERC-20?** | **TIDAK (Bypass)** | **TIDAK (Bypass)** | **YA (`asset_safe_transfer`)** |

---

## 3. Spesifikasi Guard Calldata Memo Tepat 96-Byte

Setiap panggilan `spend_joinsplit` pada kontrak Stylus wajib menyertakan tepat dua parameter memo berukuran tepat **96 byte**:
```rust
// DEC-043B: Strict 96-Byte Calldata Guard (Resolusi Audit #5)
if encrypted_memo_1.len() != 96 || encrypted_memo_2.len() != 96 {
    return Err(b"INVALID_MEMO_LENGTH".to_vec());
}
```

Manfaat Arsitektur:
1. **Perlindungan Biaya Gas Relayer:** Calldata calldata L1 terikat pada ukuran tetap (192 byte total), meniadakan kemungkinan serangan pengurasan gas relayer via *oversized calldata*.
2. **Jaminan Recoverability:** Menjamin setiap event `NoteCommitmentAppended` membawa ciphertext AEAD yang utuh untuk trial decryption di sisi penerima.
3. **Kepatuhan Topologi Konstan (INV-4):** Seluruh transaksi JoinSplit di on-chain memiliki footprint calldata yang seragam.

---

## 4. Logika Akuntansi Solvensi & CEI (Checks-Effects-Interactions)

### A. Multi-Liability Accounting pada Mode 2 (In-Pool Transfer)
Pada Mode 2, tidak ada nilai pokok yang keluar dari kontrak. Penurunan liabilitas note murni diimbangi oleh penambahan piutang fee eksekusi relayer:
$$\Delta \text{user\_note\_liabilities} = -v_{execution\_fee}$$
$$\Delta \text{accrued\_execution\_fee\_liabilities} = +v_{execution\_fee}$$
$$\Delta \text{realized\_protocol\_fees} = 0$$
$$\Delta \text{Contract Asset Balance} = 0$$

$$\text{Net Change in Total Liabilities} = (-v_{exec} + v_{exec} + 0) = 0$$
$$\Delta \text{Assets} - \Delta \text{Liabilities} = 0 - 0 = 0 \quad \text{(Preservasi Solvensi Absolut)}$$

### B. Urutan Eksekusi Strict CEI di `spend.rs`
1. **CHECKS:**
   - Validasi jeda (`check_not_paused()`).
   - Validasi fee eksekusi (`execution_fee <= max_execution_fee`).
   - Validasi kedaluwarsa kuotasi (`current_time <= expiry`).
   - Validasi keunikan nullifier (`nullifier_1 != nullifier_2`).
   - Validasi status belum terpakai (`!note_nullifiers.get(nf)`).
   - Validasi jendela generasi epoch (DEC-033).
   - **Evaluasi Strict 3-Mode State Machine** (Mode 1 vs Mode 2 vs Mode 3).
   - Validasi calldata memo tepat 96 byte (`len() == 96`).
   - Verifikasi bukti Groth16 via precompile EIP-2537 (`0x0c` MSM + `0x0f` 4-pairing).
2. **EFFECTS:**
   - Pembakaran nullifier 1 & nullifier 2 (`note_nullifiers` & `generation_nullifiers`).
   - Penyisipan daun MMR Output 1 bersama `encrypted_memo_1` (`_mmr_insert`).
   - Penyisipan daun MMR Output 2 bersama `encrypted_memo_2` (`_mmr_insert`).
   - Pembaruan saldo liabilitas per aset (`user_note_liabilities`, `accrued_execution_fee_liabilities`).
3. **INTERACTIONS:**
   - Transfer ERC-20 **hanya dieksekusi** pada Mode 3 (`is_rollover == 0 && merchant_amount > 0 && recipient != Address::ZERO`).
   - Pemancaran event: `PrivateNoteSpend`, `NoteCommitmentAppended` (#1 dan #2), `ExecutionFee`.
4. **INVARIANT ASSERTION:**
   - Pemanggilan `_check_asset_solvency_invariant(asset_id)` di akhir fungsi. Jika terjadi selisih 1 wei saja, seluruh transaksi di-revert seketika.

---

## 5. Implementasi Rust Smart Contract (`zeltra-contracts/src/spend.rs`)

```rust
// ── 1. CHECKS: 3-Mode Segregation & Calldata Length Guards ──
if is_rollover == U256::from(1) {
    // Mode 1: Rollover
    if merchant_amount != U256::ZERO {
        return Err(b"MERCHANT_AMOUNT_MUST_BE_ZERO_IN_ROLLOVER".to_vec());
    }
    if protocol_fee != U256::ZERO {
        return Err(b"PROTOCOL_FEE_MUST_BE_ZERO_IN_ROLLOVER".to_vec());
    }
    if recipient != Address::ZERO {
        return Err(b"RECIPIENT_MUST_BE_ZERO_IN_ROLLOVER".to_vec());
    }
} else if merchant_amount == U256::ZERO {
    // Mode 2: In-Pool Transfer (DEC-043)
    if has_change_1 == U256::ZERO {
        return Err(b"IN_POOL_TRANSFER_REQUIRES_OUTPUT_1".to_vec());
    }
    if protocol_fee != U256::ZERO {
        return Err(b"PROTOCOL_FEE_MUST_BE_ZERO_FOR_IN_POOL_TRANSFER".to_vec());
    }
    if recipient != Address::ZERO {
        return Err(b"RECIPIENT_MUST_BE_ZERO_FOR_IN_POOL_TRANSFER".to_vec());
    }
} else {
    // Mode 3: Public Outflow Spend (Anti-Silent-Burn!)
    if recipient == Address::ZERO {
        return Err(b"ZERO_RECIPIENT_ON_PUBLIC_SPEND".to_vec());
    }
}

// Calldata Length Guard (AUD-5)
if encrypted_memo_1.len() != 96 || encrypted_memo_2.len() != 96 {
    return Err(b"INVALID_MEMO_LENGTH".to_vec());
}

// ── 2. EFFECTS: MMR Appends with Real Memos ──
self._mmr_insert(output_cm_1, encrypted_memo_1.to_vec())?;
self._mmr_insert(output_cm_2, encrypted_memo_2.to_vec())?;

// ── 3. INTERACTIONS: Conditional Token Transfer ──
if is_rollover == U256::ZERO && merchant_amount > U256::ZERO && recipient != Address::ZERO {
    self.ensure_asset_liquidity(asset_id, merchant_amount)?;
    let raw_payout = if asset_id == 0 {
        merchant_amount
            .checked_mul(U256::from(1_000_000_000u64))
            .ok_or_else(|| b"OVERFLOW".to_vec())?
    } else {
        merchant_amount
    };
    self.asset_safe_transfer(target_token, recipient, raw_payout)?;
}
```

---

## 6. Matriks Pengujian Unit & Soundness (`zeltra-contracts`)

Sesuai aturan engineering Zeltra, implementasi DEC-043B wajib disertai **1 Positive Test + Minimal 2x Negative Tests**:

### Positive Tests:
1. `test_spend_joinsplit_in_pool_usdc_success`: Eksekusi Mode 2 untuk USDC dengan dual 96B memo. Berhasil menyisipkan 2 daun ke MMR, membakar 2 nullifier, saldo ERC-20 kontrak tetap utuh, liabilitas note terdebit tepat sebesar execution fee, dan invariant solvensi terpenuhi.
2. `test_spend_joinsplit_in_pool_weth_success`: Eksekusi Mode 2 untuk WETH dengan dual 96B memo.

### Negative Tests (Minimal 2x Positive = Minimal 4 Tests):
1. `test_in_pool_rejects_non_zero_recipient`: Mode 2 me-revert dengan `b"RECIPIENT_MUST_BE_ZERO_FOR_IN_POOL_TRANSFER"` jika pengirim menyertakan alamat penerima non-zero.
2. `test_in_pool_rejects_non_zero_protocol_fee`: Mode 2 me-revert dengan `b"PROTOCOL_FEE_MUST_BE_ZERO_FOR_IN_POOL_TRANSFER"` jika `protocol_fee > 0`.
3. `test_in_pool_rejects_missing_output_1`: Mode 2 me-revert dengan `b"IN_POOL_TRANSFER_REQUIRES_OUTPUT_1"` jika `has_change_1 == 0`.
4. `test_public_spend_rejects_zero_recipient`: Mode 3 (`merchant_amount > 0`) me-revert dengan `b"ZERO_RECIPIENT_ON_PUBLIC_SPEND"` jika `recipient == Address::ZERO` (mencegah silent fund burn).
5. `test_spend_joinsplit_rejects_invalid_memo_length`: Me-revert dengan `b"INVALID_MEMO_LENGTH"` jika memo 1 atau memo 2 tidak berukuran tepat 96 byte (misal 95 byte atau 120 byte).
