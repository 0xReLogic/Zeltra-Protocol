# DEC-011: Atomic Release dan Key Zeroization

Date: 2026-06-07

## Masalah

### Session Validation

Guardian saat ini menandatangani request signing tanpa memvalidasi bahwa:
- Session terkait masih valid (tidak expired)
- Session belum spent/refunded
- Amount dan commitment matching dengan expected values

Ini dapat menyebabkan guardian menandatangani session yang tidak valid,
membuang computational resources dan berpotensi memicu edge case yang tidak
diprediksi.

### Key Zeroization

Masking key `k` disimpan terenkripsi tetapi tidak di-zeroize setelah:
- Reveal selesai
- Session expired
- Quorum gagal

Ini meningkatkan exposure window di mana `k` dapat diekstrak dari memory jika
server compromised, meskipun dalam bentuk terenkripsi.

## Invariant bisnis/security

- Guardian hanya menandatangani session yang sah dan belum expired
- Guardian hanya menandatangani session yang belum spent/refunded
- Guardian menolak session dengan amount/commitment yang tidak valid
- Masking key `k` di-zeroize dari memory segera setelah reveal selesai
- Masking key `k` di-zeroize setelah session expired atau quorum gagal
- Memory protection menggunakan zeroize dengan volatile writes untuk mencegah
  compiler optimization

## Pilihan yang dipertimbangkan

### Session Validation

1. Tanpa validasi (current) - tandatangani semua request yang lolos application-layer auth
2. Validasi sederhana - hanya cek expiry session
3. Validasi lengkap - cek expiry, status, amount, commitment
4. Validasi dengan state on-chain - query contract untuk status session

### Key Zeroization

1. Tanpa zeroization (current) - biarkan GC handle cleanup
2. Manual zeroize dengan zeroize crate
3. Secure memory allocation dengan mlock/munprotect
4. Encrypted memory with canaries

## Keputusan

### Session Validation

Gunakan pilihan 3 untuk testnet: validasi lengkap (expiry, status, amount, commitment)
berdasarkan state lokal di relayer database.

Untuk production, pertimbangkan pilihan 4 sebagai defense-in-depth.

### Key Zeroization

Gunakan pilihan 2 untuk testnet: zeroize menggunakan `zeroize` crate dengan
volatile writes dan atomic fences.

Untuk production, pertimbangkan pilihan 3 jika threat model memerlukan proteksi
melawan cold boot atau swap attacks.

## Alasan

### Session Validation

- Pilihan 1 tidak memberikan protection dari invalid/abusive requests
- Pilihan 2 tidak cukup - perlu validasi status dan parameter
- Pilihan 3 memberikan validasi komprehensif tanpa on-chain overhead
- Pilihan 4 menambah latency dan complexity untuk production-only value

### Key Zeroization

- Pilihan 1 bergantung pada Rust drop yang tidak dijamin zeroization
- Pilihan 2 memberikan cryptographic hygiene yang baik dengan overhead minimal
- Pilihan 3 memberikan proteksi lebih kuat tetapi memerlukan libsodium dependency
- Pilihan 4 memberikan proteksi maksimal dengan complexity tinggi

`zeroize` crate menggunakan stable Rust primitives (ptr::write_volatile dan atomic
fences) untuk memastikan zeroization tidak di-optimize away oleh compiler.

## Sumber primer

- Zeroize crate documentation: https://docs.rs/zeroize/latest/zeroize
- Secrets crate documentation: https://docs.rs/secrets/latest/secrets
- memsecurity crate documentation: https://crates.io/crates/memsecurity
- RFC 9591 (FROST protocol): https://datatracker.ietf.org/doc/html/rfc9591

## Sumber pembanding

- THORChain TSS ceremony timeout and blame attribution:
  https://dev.thorchain.org/bifrost/tss.html
- Lux Network threshold signing slashing for missed deadlines:
  https://lps.lux.network/docs/lp-5013/
- ICP threshold signatures pre-signature management:
  https://docs.internetcomputer.org/docs/references/t-sigs-how-it-works

Accessed: 2026-06-07.

## Known risks

- Zeroize tidak melindungi dari microarchitectural attacks (Spectre/Meltdown)
- Zeroize tidak menjamin protection dari cold boot jika swap tidak dinonaktifkan
- Session validation berdasarkan state lokal bisa stale jika tidak sinkron dengan
  on-chain state
- Race condition antara validasi dan actual signing

## Versi/library/network

- `zeroize = "1.7"` untuk secure memory zeroization
- Session expiry: 1 jam (3600 detik) dari creation
- State validation: relayer database sebagai source of truth lokal

## Test

Positive:

- Guardian menandatangani session valid dalam expiry window
- Session dengan expiry yang valid diterima
- `k` di-zeroize setelah reveal sukses

Negative:

- Guardian menolak session expired
- Guardian menolak session sudah spent/refunded
- Guardian menolak session dengan amount mismatch
- Guardian menolak session dengan commitment mismatch
- Memory dump setelah reveal tidak mengandung `k` dalam bentuk plaintext

## Rollback/recovery

Jika session validation menyebabkan false positive rejection:
1. Log semua rejection dengan reason yang jelas
2. Dapat dinonaktifkan sementara dengan feature flag
3. Admin dapat override expiry untuk emergency recovery

Jika zeroization menyebabkan performance issue:
1. Dapat dinonaktifkan dengan feature flag untuk profiling
2. Fallback ke manual cleanup tanpa volatile writes

## Implementation

### Session Validation

1. Tambah field `expiry_at` pada signing request DTO
2. Tambah validasi di guardian handler:
   ```rust
   fn validate_signing_session(
       session: &SigningSession,
       request: &SigningRequest
   ) -> Result<(), SessionValidationError> {
       // Check expiry
       if session.expiry_at < Utc::now() {
           return Err(SessionValidationError::Expired);
       }

       // Check status
       if session.status != SessionStatus::Pending {
           return Err(SessionValidationError::InvalidStatus);
       }

       // Check amount
       if session.amount != request.amount {
           return Err(SessionValidationError::AmountMismatch);
       }

       // Check commitment
       if session.commitment != request.commitment {
           return Err(SessionValidationError::CommitmentMismatch);
       }

       Ok(())
   }
   ```

3. Set expiry default ke 1 jam dari session creation
4. Tambah log untuk rejection dengan reason

### Key Zeroization

1. Tambah dependency `zeroize` ke Cargo.toml
2. Implement Zeroize trait untuk masking key storage:
   ```rust
   use zeroize::Zeroize;

   struct MaskingKey {
       k: Option<[u8; 32]>,
   }

   impl Drop for MaskingKey {
       fn drop(&mut self) {
           if let Some(ref mut key) = self.k {
               key.zeroize();
           }
       }
   }
   ```

3. Zeroize setelah reveal sukses:
   ```rust
   fn reveal_masking_key(session_id: &str) -> Result<[u8; 32], Error> {
       let mut key = load_encrypted_key(session_id)?;
       let revealed = decrypt_and_return(&key)?;
       key.zeroize(); // Zeroize immediately after use
       Ok(revealed)
   }
   ```

4. Zeroize pada session expiry cleanup job:
   ```rust
   async fn cleanup_expired_sessions() {
       for session in expired_sessions {
           session.masking_key.zeroize();
           session.status = SessionStatus::Expired;
       }
   }
   ```

## Dependency pada DEC

- DEC-004 (atomic release flow) - lifecycle management untuk masking key
- DEC-007 (secure guardian authentication) - application-layer auth yang sudah ada
- DEC-011 menambah validasi session di atas DEC-007
