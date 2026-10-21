# Task A: Core Cryptography & Payload Hardening (DEC-048)

> **Session Scope:** `zeltra-core` (Zero breaking changes to outer crates, pure core cryptographic hardening)  
> **Parent ADR:** [`research/decisions/DEC-048-committing-aead-partitioning-oracle-defense-and-key-binding-memos.md`](research/decisions/DEC-048-committing-aead-partitioning-oracle-defense-and-key-binding-memos.md)  
> **Target Files:**
> - `zeltra-core/src/memo.rs`
> - `zeltra-core/src/note.rs`
> - `zeltra-core/tests/memo_committing_tests.rs`

---

## 1. Objectives for Session A

1. **Eliminate 31-Bit Value Truncation (AUD-25):**
   - Replace 36-byte payload with **40-byte full `u64` payload** using **Field-Bit Embedding**:
     - `value`: full 8 bytes (`u64`, 0 up to $18.4 \times 10^{18}$ units; supports any deposit from $0.000001 up to billions of USDC/WETH).
     - `rho`: 32 bytes (BLS12-381 Fr scalar).
     - `asset_id`: embedded into bit 255 of $\rho$ (which is always 0 since Fr modulus $r < 2^{255}$).
   - Adjust transport nonce from 11 bytes to **7 bytes (56 bits)**.
   - Maintain the strict 96-byte calldata invariant:
     $$32\text{B (epk)} + 1\text{B (view\_tag)} + 7\text{B (nonce)} + 40\text{B (ciphertext)} + 16\text{B (tag)} = \mathbf{96\text{ bytes}}.$$

2. **Patch X25519 Non-Contributory Point Vulnerability (AUD-26):**
   - In `decrypt_committing_note_memo`, enforce:
     ```rust
     let shared_secret = static_secret.diffie_hellman(&epk);
     if shared_secret.was_non_contributory() {
         return None; // Fail-closed rejection of malicious low-order points
     }
     ```

3. **Implement EtH-KS Committing AEAD Routines:**
   - Key Separation PRG: $G(S) \to (K_{\text{enc}}^{32\text{B}}, K_{\text{cmt}}^{32\text{B}}, N_7^{7\text{B}})$.
   - Verify-then-Decrypt invariant: verify truncated HMAC-SHA256 tag in constant time *before* passing bytes to ChaCha20.
   - Constant-time comparison using `subtle::ConstantTimeEq`.
   - Automatic memory zeroization via `zeroize::Zeroize`.

---

## 2. Step-by-Step Implementation Checklist

- [ ] **Step A1: Update `NotePlaintext` serialization in `zeltra-core/src/memo.rs`:**
  - Implement `to_bytes_v3(&self) -> [u8; 40]` and `from_bytes_v3(bytes: &[u8; 40]) -> Option<Self>`.
  - Ensure canonical BLS12-381 scalar validation ($< r$) after extracting `asset_id`.
- [ ] **Step A2: Update Key Separation PRG in `zeltra-core/src/memo.rs`:**
  - Derive $K_{\text{enc}}$ (32B), $K_{\text{cmt}}$ (32B), and $N_7$ (7B) via `HKDF-Expand` (71 bytes total OKM).
- [ ] **Step A3: Implement `encrypt_committing_note_memo` & `decrypt_committing_note_memo`:**
  - Wire format: $32 + 1 + 7 + 40 + 16 = 96$ bytes.
  - Add `shared_secret.was_non_contributory()` rejection.
- [ ] **Step A4: Unit Tests in `zeltra-core`:**
  - Test boundary values: 0, $2^{31}-1$, $2^{31}$, 5,000 USDC ($5 \times 10^9$), 10 Billion USDC, and `u64::MAX`.
  - Test all-zero X25519 public key rejection.
  - Test 128 bit flips on committing tag (100% rejection).

---

## 3. Definition of Done for Session A
- `cargo test -p zeltra-core memo` passes 100%.
- No compiler warnings or clippy errors.
- Zero breaking changes to legacy `to_bytes()` / `to_bytes_v2()` fallback methods.
