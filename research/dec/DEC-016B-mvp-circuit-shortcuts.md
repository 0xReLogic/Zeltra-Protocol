# DEC-016B: MVP Circuit Shortcuts — HARUS FIX SEBELUM MAINNET

Date: 2026-06-14
Status: DRAFT — menunggu keputusan founder
Priority: 3 dari 5 issue ini MAINNET BLOCKER

---

## 1. Single Input Note (1 input, 1 change)

**Status:** MVP acceptable, tech debt
**Severity:** Medium
**Fix effort:** Circuit rewrite + new PK/VK

Sekarang: User cuma bisa spend dari 1 note. Kalau punya 3 note kecil ($1, $2, $3)
dan mau bayar $5 → ga bisa, harus spend satu-satu.

Impact:
- UX: User harus manage banyak note manual
- Privacy: Tiap note = 1 on-chain tx = lebih banyak metadata bocor
- Consolidation attack: Combine notes = observer bisa link ke user yang sama

Fix: Expand circuit ke max 4 input notes (DEC-016A spec). Contract + SDK bisa
backward-compatible. Butuh circuit rewrite + new trusted setup.

---

## 2. Public Input Binding Lemah [MAINNET BLOCKER]

**Status:** HARUS FIX
**Severity:** HIGH
**Fix effort:** Medium

Sekarang: recipient, quote_hash, chain_id, contract_address, expiry di-bind lewat:
```
binding = Poseidon_W3(recipient + quote_hash, chain_id + contract_addr + expiry)
```

Ini BUKAN proper binding. Prover bisa swap values tanpa mengubah hash.
Contoh: swap chain_id ↔ contract_address.

Attack vectors:
- Replay attack: proof chain A → replay di chain B
- Contract swap: proof contract X → pakai di contract Y

Fix options:
- A) Bind tiap public input secara independent di circuit
- B) Include semua di quote_hash yang di-sign user (EIP-712)
- C) Gabungan A + B (paling aman)

Recommendation: Option B. User udah sign quote via EIP-712. Quote hash
seharusnya mencakup semua binding fields. Circuit tinggal verify
quote_hash matches signed data.

---

## 3. Single-Party Trusted Setup [MAINNET BLOCKER]

**Status:** HARUS FIX
**Severity:** CRITICAL
**Fix effort:** Besar (MPC ceremony)

Sekarang: `NOTE_CIRCUIT_SETUP_SEED = 0x4e696d6275734e43` hardcoded.
Siapapun yang tau seed ini bisa generate fake proofs → spend tanpa valid note.

Ini = cetak uang gratis. P0 security issue.

Fix: MPC ceremony. Minimal 3 party. Cukup 1 party jujur (destroy their share).
DEC-016A udah mention ini.

Timeline estimate: 2-4 minggu untuk organize + execute ceremony.

---

## 4. has_change Tidak Di-Boolean-Constrain [MAINNET BLOCKER]

**Status:** HARUS FIX
**Severity:** HIGH
**Fix effort:** 1 constraint, 1 line

Sekarang:
```
output_commitment == has_change * change_cm
```

Kalau has_change = 2 (bukan boolean), output = 2 * change_cm.
Prover bisa set has_change = 2 → double change commitment → double spend.

Fix:
```rust
// Tambahin boolean constraint:
let one_minus_hc = FpVar::Constant(Fr::from(1u64)) - &has_change_var;
let hc_check = &has_change_var * &one_minus_hc;
hc_check.enforce_equal(&zero)?;
```

Ini fix paling gampang — 3 lines of code. Tapi kalau lupa, double spend possible.

---

## 5. Zero Change Witnesses di Full Spend

**Status:** Should fix, low priority
**Severity:** Medium
**Fix effort:** Small

Sekarang: Kalau no change, circuit pake change_owner_key = Fr(0), rho = Fr(0),
randomness = Fr(0). Commitment = note_commitment(0, 0, 0, 0) = fixed value.

Kalau attacker bisa bikin note value 0 dengan commitment ini, mereka bisa
double-nullify.

Fix: Explicit reject di circuit kalau change_value == 0 AND has_change == 1.
Atau: pakai random witnesses walaupun no change (commitment jadi unpredictable).

DEC-016A udah bilang zero-value output forbidden. Ini tinggal enforce di circuit.

---

## Keputusan yang Dibutuhkan

| # | Issue | Decision Needed |
|---|---|---|
| 1 | 1-input limit | Accept MVP? Atau fix sekarang? |
| 2 | Weak binding | Fix sebelum Phase 4 (contract) |
| 3 | Single-party setup | Kapan MPC ceremony? |
| 4 | has_change not boolean | Fix sekarang (1 line) |
| 5 | Zero change witnesses | Fix sekarang (small) |

Recommendation: Fix #4 dan #5 sekarang (gampang). Fix #2 sebelum Phase 4.
#3 bisa parallel. #1 bisa post-launch.
