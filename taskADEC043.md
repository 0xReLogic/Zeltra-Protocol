# TASK A (DEC-043A): Zeltra Shielded Address (ZSA), Key Derivation & Deterministic ECDH Randomness (`zeltra-core`)

> **Scope:** `zeltra-core/` (Arkworks BLS12-381, Poseidon-W3, Poseidon-W5, X25519 ECDH, ChaCha20Poly1305, Bech32m)  
> **Spesifikasi Referensi:** [`research/decisions/DEC-043A-zsa-key-derivation-and-deterministic-ecdh-memo.md`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-043A-zsa-key-derivation-and-deterministic-ecdh-memo.md) & [`research/decisions/DEC-043-shielded-to-shielded-in-pool-transfer-and-merchant-masking.md`](file:///workspaces/Zeltra-Protocol/research/decisions/DEC-043-shielded-to-shielded-in-pool-transfer-and-merchant-masking.md)  
> **Prinsip:** *"Zero Spend Authority Leakage, Pure-Field Commitment Parity, Deterministic In-Band Randomness"*  
> **Status:** **PENDING IMPLEMENTATION**

---

## 1. Latar Belakang & Tujuan Arsitektur

Menyediakan fondasi primitif kriptografi untuk transfer shielded-to-shielded in-pool tanpa penarikan publik (Merchant Masking):
1. **Zeltra Shielded Address (ZSA 64-Byte):**
   Memisahkan kunci penerima menjadi `(owner_key_fr: Fr, incoming_viewing_public_key: [u8; 32])` sehingga pengirim dapat memasukkan `owner_key_fr` ke dalam komitmen Poseidon tanpa mengetahui spending key rahasia ($sk$) milik penerima, dan tanpa terjadi ketidakcocokan kurva aljabar pada saat pembelanjaan note (Resolusi AUD-1).
2. **Deterministic ECDH Note Randomness:**
   Menurunkan trapdoor hiding randomness $r$ secara deterministik dari shared secret Diffie-Hellman $S = \text{X25519}(esk, pk\_ivk)$ dan pengacak $\rho$, sehingga penerima yang mendekripsi memo 48-byte dapat merekonstruksi $r$ dan komitmen daun $cm$ yang 100% identik dengan yang dibuat pengirim (Resolusi AUD-2).
3. **Public Key Memo Encryption & Anti-Leak Dummy:**
   Memastikan enkripsi memo kembalian pengirim menggunakan public key X25519 (Resolusi AUD-3), dan enkripsi memo dummy pada exact payment menghasilkan ciphertext pseudo-random 96 byte (Resolusi AUD-7).

---

## 2. Checklist Implementasi Rinci

### A. Derivasi Kunci Pemilik Satu Arah (`zeltra-core/src/note.rs`)
- [ ] **Domain Separator & Helper Function:**
  - Tambahkan domain separator:
    ```rust
    pub fn domain_owner_key() -> Fr {
        domain_from_label("zeltra.owner.key.v1")
    }
    ```
  - Implementasikan derivasi satu arah:
    ```rust
    pub fn derive_owner_key(spending_key: Fr) -> Fr {
        crate::poseidon::native_poseidon_w3(
            spending_key,
            Fr::from(0u64),
            domain_owner_key(),
        )
    }
    ```

### B. Struktur `ZeltraShieldedAddress` (`zeltra-core/src/note.rs`)
- [ ] **Definisi Struct & Serialisasi (Hex & Bech32m):**
  - Definisikan struct:
    ```rust
    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub struct ZeltraShieldedAddress {
        pub owner_key: Fr,
        pub incoming_viewing_public_key: [u8; 32],
    }
    ```
  - Implementasikan method:
    - `pub fn to_bytes(&self) -> [u8; 64]`
    - `pub fn from_bytes(bytes: &[u8; 64]) -> Option<Self>`
    - `pub fn to_hex(&self) -> String`
    - `pub fn from_hex(hex_str: &str) -> Result<Self, KeyFormatError>`
    - `pub fn to_bech32m(&self) -> Result<String, bech32::Error>` (HRP: `"zs"`)
    - `pub fn from_bech32m(s: &str) -> Result<Self, KeyFormatError>`

### C. Helper Enkripsi Memo Deterministik (`zeltra-core/src/note.rs`)
- [ ] **Derivasi Randomness ECDH:**
  ```rust
  pub fn derive_ecdh_note_randomness(shared_secret: &[u8; 32], rho: &Fr) -> Fr
  ```
- [ ] **Enkripsi Memo Deterministik:**
  ```rust
  pub fn encrypt_note_memo_deterministic<R: RecipientViewingKey>(
      value: u64,
      rho: Fr,
      asset_id: u64,
      recipient: &R,
  ) -> Result<(Vec<u8>, Fr), KeyFormatError>
  ```
- [ ] **Dekripsi Memo Deterministik:**
  ```rust
  pub fn decrypt_note_memo_deterministic(
      encrypted_memo: &[u8],
      ivk: &IncomingViewingKey,
  ) -> Option<(NotePlaintext, Fr)>
  ```

### D. Re-ekspor & Modul (`zeltra-core/src/lib.rs`)
- [ ] Re-ekspor: `ZeltraShieldedAddress`, `derive_owner_key`, `derive_ecdh_note_randomness`, `encrypt_note_memo_deterministic`, `decrypt_note_memo_deterministic`.

---

## 3. Matriks Pengujian Unit & Soundness (`zeltra-core/tests/`)

Sesuai aturan engineering Zeltra: **1 Positive Test + Minimal 2x Negative Tests**:

### Positive Tests:
- [ ] `test_zsa_roundtrip_hex_and_bech32m`: Serialisasi dan deserialisasi ZSA melalui Hex dan Bech32m menghasilkan tuple yang identik.
- [ ] `test_deterministic_ecdh_randomness_and_commitment_match`: Pengirim dan penerima menghasilkan $r$ dan komitmen daun $cm$ yang cocok 100%.

### Negative Tests (Minimal 2x Positive = Minimal 4 Tests):
- [ ] `test_zsa_rejects_non_canonical_owner_key`: Parsing ZSA dengan skalar $owner\_key \ge r$ gagal dengan `NonCanonicalScalar`.
- [ ] `test_zsa_rejects_corrupted_bech32m_checksum`: Mengubah 1 karakter Bech32m menggagalkan verifikasi checksum.
- [ ] `test_deterministic_memo_rejects_tampered_ciphertext`: Mengubah 1 bit pada ciphertext menggagalkan otentikasi tag Poly1305.
- [ ] `test_deterministic_memo_fails_with_wrong_ivk`: Dekripsi dengan IVK yang salah mengembalikan `None`.
- [ ] `test_deterministic_memo_rejects_invalid_size`: Memo selain 96 byte ditolak seketika.

---

## 4. Strict Engineering Guardrails (Zero-Slop Policy)

1. **NO PLACEHOLDERS:** Dilarang menggunakan `todo!()`, `unimplemented!()`, dummy byte arrays, atau mock proofs di jalur production.
2. **COMPILE-CLEAN:** Wajib lulus `cargo check -p zeltra-core` dan `cargo test -p zeltra-core` dengan 0 error dan 0 warning.
3. **FORMAT & CLIPPY:** Wajib bersih `cargo fmt --check` dan `cargo clippy -p zeltra-core -- -D warnings`.
