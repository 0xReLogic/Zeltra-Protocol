# Task A: Implementasi Fondasi Constant-Time Scanner & Branchless Primitives (DEC-049 Fase 1)

> **Fokus Sesi 1:** Implementasi level core, tipe data branchless, deserialisasi waktu-konstan, dan zero-leakage primitives di `zeltra-core`.  
> **Status:** Pending Execution  
> **Referensi Arsitektur:** [`research/decisions/DEC-049-leakage-resilient-decapsulation-and-constant-time-flag-defense.md`](research/decisions/DEC-049-leakage-resilient-decapsulation-and-constant-time-flag-defense.md)

---

##  Target Utama
1. Mengimplementasikan modul `zeltra-core/src/memo_ct.rs` berbasis spesifikasi DEC-049.
2. Memastikan seluruh tipe data (`CtNotePlaintext`) mengimplementasikan `Copy, Clone, Debug, Zeroize, ConditionallySelectable`.
3. Membangun evaluasi kanonikalitas medan skalar BLS12-381 `Fr` secara bebas-cabang (`is_canonical_fr_ct`).
4. Menghubungkan modul baru ke `zeltra-core/src/lib.rs` dan `zeltra-core/src/memo.rs`.

---

##  Checklist Pekerjaan

- [ ] **A1: Pembuatan Struktur Data `CtNotePlaintext`**
  - Implementasi struct dengan limb representation (`rho_limbs: [u64; 4]`).
  - Implementasi trait `subtle::ConditionallySelectable` berbasis `u64::conditional_select`.
  - Implementasi method konversi aman `to_canonical_note(&self) -> Option<NotePlaintext>`.
  - Uji zeroize saat drop.

- [ ] **A2: Implementasi Fungsi Primitif Branchless**
  - Implementasi `derive_view_tag_ct`: HKDF-Expand berbasis SHA-256 waktu-konstan.
  - Implementasi `derive_key_separation_ct`: G(S) -> (K_enc, K_cmt, N_7).
  - Implementasi `compute_committing_tag_ct`: Truncated HMAC-SHA256 16-byte.
  - Implementasi `is_canonical_fr_ct`: Perbandingan leksikografis branchless 4 limb terhadap modulus `FrConfig::MODULUS`.

- [ ] **A3: Fungsi Utama `trial_decrypt_ct`**
  - Integrasi X25519 dalek constant-time Diffie-Hellman + non-contributory check.
  - Dekripsi in-place ChaCha20 atas 40-byte payload tanpa early-return.
  - Akumulasi bendera validitas (`Choice`) via bitwise bitmask:
    `is_contributory & tag_match & hkdf_ok & nonce_match & tag_auth_valid & rho_canonical`.
  - Sanitasi eksplisit seluruh buffer sensitif (`epk`, `nonce`, `raw_plaintext`, `shared_secret_bytes`).

- [ ] **A4: Unit Test Fondasi & Kompilasi**
  - Buat unit test dasar di `zeltra-core/tests/memo_ct_tests.rs`.
  - Validasi kompilasi: `cargo check -p zeltra-core` & `cargo test -p zeltra-core --test memo_ct_tests`.

---

##  Kriteria Sukses Sesi 1
- `zeltra-core` terkompilasi bersih tanpa warning atau error trait.
- Tidak ada percabangan bersyarat (`if`) pada path pengolahan rahasia memo.
- Unit test membuktikan plaintext valid berhasil diekstrak dan ciphertext acak menghasilkan `Choice::from(0)` dengan data ter-dummy secara aman.
