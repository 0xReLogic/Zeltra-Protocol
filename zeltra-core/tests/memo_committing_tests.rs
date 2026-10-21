//! DEC-048 Committing AEAD & Payload Hardening Test Suite
//!
//! Validates:
//! 1. AUD-25 (Value Truncation): Full 64-bit NotePlaintext serialization (v3) via Field-Bit Embedding
//! 2. AUD-26 (Non-Contributory Point): Rejection of all-zero and low-order X25519 public keys
//! 3. EtH-KS Committing AEAD roundtrips across all financial boundary values
//! 4. 128 bit flips on committing tag (100% rejection rate)
//! 5. Ciphertext, nonce, view tag, and epk tampering rejection (Verify-then-Decrypt fail-closed)
//! 6. Multi-recipient partitioning oracle resistance across random IVKs
//! 7. Backward compatibility with legacy DEC-045C and DEC-037 memos

use ark_bls12_381::Fr;
use ark_ff::UniformRand;
use rand::rngs::OsRng;
use rand::RngCore;
use zeltra_core::keys::IncomingViewingKey;
use zeltra_core::memo::{
    compute_committing_tag, create_legacy_dec037_memo, decrypt_committing_note_memo,
    decrypt_committing_note_memo_payload, decrypt_committing_note_memo_plaintext,
    decrypt_note_memo_deterministic, derive_key_separation, encrypt_committing_note_memo,
    encrypt_note_memo_deterministic, verify_committing_tag_ct, CommittingMemoError, NotePlaintext,
    ENCRYPTED_MEMO_SIZE,
};

fn random_ivk<R: RngCore>(rng: &mut R) -> IncomingViewingKey {
    let mut bytes = [0u8; 32];
    rng.fill_bytes(&mut bytes);
    IncomingViewingKey::from_bytes(&bytes)
}

/// BLS12-381 Fr scalar field modulus r in big-endian hex:
/// r = 0x73eda753299d7d483339d80809a1d80553bda402fffe5bfeffffffff00000001
const BLS12_381_R_LE: [u8; 32] = [
    0x01, 0x00, 0x00, 0x00, 0xff, 0xff, 0xff, 0xff, 0xfe, 0x5b, 0xfe, 0xff, 0x02, 0xa4, 0xbd, 0x53,
    0x05, 0xd8, 0xa1, 0x09, 0x08, 0xd8, 0x39, 0x33, 0x48, 0x7d, 0x9d, 0x29, 0x53, 0xa7, 0xed, 0x73,
];

#[test]
fn test_memo_aud25_note_plaintext_v3_boundary_values_and_field_bit_embedding() {
    let mut rng = OsRng;
    let rho = Fr::rand(&mut rng);

    // Boundary test values:
    // 0, 1, 2^31 - 1 (2,147,483,647), 2^31 (2,147,483,648),
    // 5,000 USDC (5,000,000,000 micro-USDC), 10 Billion USDC (10^16 micro-USDC), u64::MAX
    let boundary_values: [u64; 7] = [
        0,
        1,
        0x7fff_ffff,            // 2^31 - 1: historical 31-bit truncation boundary
        0x8000_0000,            // 2^31: previously overflowed/truncated
        5_000_000_000,          // 5,000 USDC: previously truncated to 705.03 USDC
        10_000_000_000_000_000, // 10 Billion USDC
        u64::MAX,               // 18.44 Quintillion units
    ];

    for &val in &boundary_values {
        for asset_id in [0u64, 1u64] {
            let pt = NotePlaintext {
                value: val,
                rho,
                asset_id,
            };

            let bytes = pt.to_bytes_v3();
            assert_eq!(bytes.len(), NotePlaintext::SERIALIZED_SIZE_V3);
            assert_eq!(bytes.len(), 40);

            // Verify little-endian value encoding in bytes [0..8]
            let val_recovered = u64::from_le_bytes(bytes[0..8].try_into().unwrap());
            assert_eq!(val_recovered, val, "Value must not be truncated!");

            // Verify asset_id bit 255 in byte 39 (byte 31 of rho)
            let asset_bit = (bytes[39] >> 7) & 1;
            assert_eq!(asset_bit, asset_id as u8, "Asset bit embedding mismatch!");

            // Verify deserialization
            let pt_recovered = NotePlaintext::from_bytes_v3(&bytes)
                .expect("Valid serialized v3 bytes must deserialize");
            assert_eq!(pt_recovered.value, val);
            assert_eq!(pt_recovered.rho, rho);
            assert_eq!(pt_recovered.asset_id, asset_id);
        }
    }
}

#[test]
fn test_memo_aud25_from_bytes_v3_non_canonical_scalar_rejection() {
    // 1. Modulus r itself (should be rejected since valid Fr are < r)
    let mut bytes_r = [0u8; 40];
    bytes_r[0..8].copy_from_slice(&100u64.to_le_bytes());
    bytes_r[8..40].copy_from_slice(&BLS12_381_R_LE);
    // Bit 255 is 0 in r
    assert!(
        NotePlaintext::from_bytes_v3(&bytes_r).is_none(),
        "Scalar equal to modulus r must be rejected"
    );

    // 2. Modulus r with asset_id bit set (bit 255 = 1)
    bytes_r[39] |= 0x80;
    assert!(
        NotePlaintext::from_bytes_v3(&bytes_r).is_none(),
        "Scalar equal to modulus r must be rejected even with asset_id bit"
    );

    // 3. Scalar strictly >= r (e.g. byte 31 set to 0x74 > 0x73)
    let mut bytes_overflow = [0u8; 40];
    bytes_overflow[8..40].copy_from_slice(&BLS12_381_R_LE);
    bytes_overflow[39] = 0x74; // > 0x73eda753...
    assert!(
        NotePlaintext::from_bytes_v3(&bytes_overflow).is_none(),
        "Scalar > r must be rejected"
    );

    // 4. All 0xFF (except bit 255 cleared)
    let mut bytes_all_ff = [0xffu8; 40];
    bytes_all_ff[0..8].copy_from_slice(&1000u64.to_le_bytes());
    assert!(
        NotePlaintext::from_bytes_v3(&bytes_all_ff).is_none(),
        "All 0xFF scalar must be rejected"
    );
}

#[test]
fn test_memo_dec048_committing_memo_roundtrip_all_boundary_values() {
    let mut rng = OsRng;
    let ivk = random_ivk(&mut rng);
    let rho = Fr::rand(&mut rng);

    let test_values: [u64; 6] = [
        0,
        0x7fff_ffff,            // 2^31 - 1
        0x8000_0000,            // 2^31
        5_000_000_000,          // 5,000 USDC
        10_000_000_000_000_000, // 10 Billion USDC
        u64::MAX,
    ];

    for &val in &test_values {
        for asset_id in [0u64, 1u64] {
            let (memo, enc_randomness) = encrypt_committing_note_memo(val, rho, asset_id, &ivk)
                .expect("Encryption should succeed");

            assert_eq!(
                memo.len(),
                ENCRYPTED_MEMO_SIZE,
                "Memo must strictly satisfy 96-byte calldata invariant"
            );

            // Decrypt with recipient IVK
            let (dec_pt, dec_randomness) = decrypt_committing_note_memo(&memo, &ivk)
                .expect("Decryption with recipient IVK must succeed");

            assert_eq!(dec_pt.value, val, "Decrypted value mismatch");
            assert_eq!(dec_pt.rho, rho, "Decrypted rho mismatch");
            assert_eq!(dec_pt.asset_id, asset_id, "Decrypted asset_id mismatch");
            assert_eq!(
                dec_randomness, enc_randomness,
                "Decrypted hiding randomness must match sender derivation"
            );

            // Also verify convenience helper
            let pt_only = decrypt_committing_note_memo_plaintext(&memo, &ivk)
                .expect("Plaintext helper must succeed");
            assert_eq!(pt_only.value, val);

            // Decrypt with wrong IVK must return None
            let wrong_ivk = random_ivk(&mut rng);
            assert!(
                decrypt_committing_note_memo(&memo, &wrong_ivk).is_none(),
                "Decryption with wrong IVK must fail closed"
            );
        }
    }
}

#[test]
fn test_memo_aud26_all_zero_and_non_contributory_public_key_rejection() {
    let mut rng = OsRng;
    let ivk = random_ivk(&mut rng);
    let rho = Fr::rand(&mut rng);

    // 1. Decryption of memo crafted with all-zero epk must return None
    let (mut memo, _) =
        encrypt_committing_note_memo(1000, rho, 1, &ivk).expect("Encryption should succeed");
    memo[0..32].fill(0u8); // All-zero epk
    let dec_res = decrypt_committing_note_memo(&memo, &ivk);
    assert!(
        dec_res.is_none(),
        "AUD-26: All-zero epk memo must be rejected immediately (fail-closed)"
    );

    // 2. Encryption with all-zero recipient public key must return Err(InvalidPublicKey)
    let zero_pk = [0u8; 32];
    let enc_res = encrypt_committing_note_memo(1000, rho, 1, &zero_pk);
    assert!(
        matches!(enc_res, Err(CommittingMemoError::InvalidPublicKey)),
        "AUD-26: Encryption with all-zero recipient public key must return InvalidPublicKey"
    );
}

#[test]
fn test_memo_aud22_single_bit_tag_tampering_128_bit_flips() {
    let mut rng = OsRng;
    let ivk = random_ivk(&mut rng);
    let rho = Fr::rand(&mut rng);

    let (memo, _) = encrypt_committing_note_memo(5_000_000_000, rho, 1, &ivk)
        .expect("Encryption should succeed");

    // The 16-byte committing tag is at offsets [80..96]
    let tag_start = 80;
    let tag_len = 16;
    let mut rejection_count = 0;

    for byte_offset in 0..tag_len {
        for bit_pos in 0..8 {
            let mut tampered_memo = memo.clone();
            tampered_memo[tag_start + byte_offset] ^= 1 << bit_pos;

            let result = decrypt_committing_note_memo(&tampered_memo, &ivk);
            assert!(
                result.is_none(),
                "Single-bit tag tampering at byte {} bit {} MUST return None",
                byte_offset,
                bit_pos
            );
            rejection_count += 1;
        }
    }

    assert_eq!(
        rejection_count, 128,
        "All 128 bit flips must be tested and rejected (100% rejection rate)"
    );
}

#[test]
fn test_memo_ciphertext_payload_tampering_rejection() {
    let mut rng = OsRng;
    let ivk = random_ivk(&mut rng);
    let rho = Fr::rand(&mut rng);

    let (memo, _) = encrypt_committing_note_memo(5_000_000_000, rho, 1, &ivk)
        .expect("Encryption should succeed");

    // Ciphertext is at offsets [40..80] (40 bytes)
    for byte_idx in 40..80 {
        let mut tampered_memo = memo.clone();
        tampered_memo[byte_idx] ^= 0x01; // flip 1 bit

        let result = decrypt_committing_note_memo(&tampered_memo, &ivk);
        assert!(
            result.is_none(),
            "Ciphertext modification at byte {} must fail tag check",
            byte_idx
        );
    }
}

#[test]
fn test_memo_nonce_malleation_rejection() {
    let mut rng = OsRng;
    let ivk = random_ivk(&mut rng);
    let rho = Fr::rand(&mut rng);

    let (memo, _) =
        encrypt_committing_note_memo(1000, rho, 1, &ivk).expect("Encryption should succeed");

    // Nonce is at offsets [33..40] (7 bytes)
    for byte_idx in 33..40 {
        let mut tampered_memo = memo.clone();
        tampered_memo[byte_idx] ^= 0xff;

        let result = decrypt_committing_note_memo(&tampered_memo, &ivk);
        assert!(
            result.is_none(),
            "Nonce modification at byte {} must be rejected",
            byte_idx
        );
    }
}

#[test]
fn test_memo_view_tag_tampering_rejection() {
    let mut rng = OsRng;
    let ivk = random_ivk(&mut rng);
    let rho = Fr::rand(&mut rng);

    let (memo, _) =
        encrypt_committing_note_memo(1000, rho, 1, &ivk).expect("Encryption should succeed");

    let mut tampered_memo = memo.clone();
    tampered_memo[32] ^= 0x55; // View tag at offset 32

    assert!(
        decrypt_committing_note_memo(&tampered_memo, &ivk).is_none(),
        "View tag tampering must be discarded"
    );
}

#[test]
fn test_memo_epk_substitution_rejection() {
    let mut rng = OsRng;
    let ivk = random_ivk(&mut rng);
    let rho = Fr::rand(&mut rng);

    let (memo, _) =
        encrypt_committing_note_memo(1000, rho, 1, &ivk).expect("Encryption should succeed");

    let mut tampered_memo = memo.clone();
    tampered_memo[5] ^= 0xaa; // Epk at offsets [0..32]

    assert!(
        decrypt_committing_note_memo(&tampered_memo, &ivk).is_none(),
        "Epk substitution must fail view tag or tag check"
    );
}

#[test]
fn test_memo_multi_recipient_partitioning_oracle_resistance() {
    let mut rng = OsRng;
    let target_ivk = random_ivk(&mut rng);
    let rho = Fr::rand(&mut rng);

    let (memo, _) = encrypt_committing_note_memo(50_000_000, rho, 1, &target_ivk)
        .expect("Encryption should succeed");

    // Attempt decryption with 50 distinct random IVKs
    for _ in 0..50 {
        let non_target_ivk = random_ivk(&mut rng);
        let result = decrypt_committing_note_memo(&memo, &non_target_ivk);
        assert!(
            result.is_none(),
            "Non-target IVK must NEVER successfully decrypt (Partitioning Oracle defense)"
        );
    }
}

#[test]
fn test_memo_invalid_memo_length_rejection() {
    let mut rng = OsRng;
    let ivk = random_ivk(&mut rng);

    let test_lengths = [0, 31, 32, 95, 97, 128];
    for &len in &test_lengths {
        let dummy = vec![0x42u8; len];
        assert!(
            decrypt_committing_note_memo(&dummy, &ivk).is_none(),
            "Invalid memo length {} must return None",
            len
        );
    }
}

#[test]
fn test_memo_invalid_asset_id_rejection() {
    let mut rng = OsRng;
    let ivk = random_ivk(&mut rng);
    let rho = Fr::rand(&mut rng);

    assert!(matches!(
        encrypt_committing_note_memo(1000, rho, 2, &ivk),
        Err(CommittingMemoError::InvalidAssetId)
    ));
    assert!(matches!(
        encrypt_committing_note_memo(1000, rho, u64::MAX, &ivk),
        Err(CommittingMemoError::InvalidAssetId)
    ));
}

#[test]
fn test_memo_decrypt_committing_note_memo_payload_helper() {
    let mut rng = OsRng;
    let ivk = random_ivk(&mut rng);
    let rho = Fr::rand(&mut rng);

    let (memo, _) =
        encrypt_committing_note_memo(123456, rho, 0, &ivk).expect("Encryption should succeed");

    let epk_bytes: [u8; 32] = memo[0..32].try_into().unwrap();
    let epk = x25519_dalek::PublicKey::from(epk_bytes);
    let static_secret = x25519_dalek::StaticSecret::from(ivk.secret_bytes());
    let shared_secret = static_secret.diffie_hellman(&epk);

    let (pt, _r) = decrypt_committing_note_memo_payload(&memo, shared_secret.as_bytes())
        .expect("Payload decryption with precomputed shared secret must succeed");

    assert_eq!(pt.value, 123456);
    assert_eq!(pt.rho, rho);
    assert_eq!(pt.asset_id, 0);

    // Mismatched tag
    let mut tampered = memo.clone();
    tampered[80] ^= 0x01;
    assert!(decrypt_committing_note_memo_payload(&tampered, shared_secret.as_bytes()).is_none());
}

#[test]
fn test_memo_backward_compatibility_dual_path_scanner() {
    let mut rng = OsRng;
    let ivk = random_ivk(&mut rng);
    let rho = Fr::rand(&mut rng);

    // 1. Committing DEC-048 memo decrypted via decrypt_note_memo_deterministic
    let (committing_memo, committing_r) =
        encrypt_committing_note_memo(777_000, rho, 1, &ivk).expect("Encryption should succeed");
    let (pt_committing, r_committing) = decrypt_note_memo_deterministic(&committing_memo, &ivk)
        .expect("Committing memo must be decrypted by primary path");
    assert_eq!(pt_committing.value, 777_000);
    assert_eq!(r_committing, committing_r);

    // 2. Legacy DEC-045C memo decrypted via decrypt_note_memo_deterministic
    let (legacy_45c_memo, legacy_45c_r) = encrypt_note_memo_deterministic(888_000, rho, 0, &ivk)
        .expect("Legacy encryption should succeed");
    let (pt_legacy_45c, r_legacy_45c) = decrypt_note_memo_deterministic(&legacy_45c_memo, &ivk)
        .expect("Legacy DEC-045C memo must be decrypted by fallback path");
    assert_eq!(pt_legacy_45c.value, 888_000);
    assert_eq!(r_legacy_45c, legacy_45c_r);

    // 3. Legacy DEC-037 memo decrypted via decrypt_note_memo_deterministic
    let (legacy_37_memo, legacy_37_r) =
        create_legacy_dec037_memo(999_000, rho, 1, ivk.public_key());
    let (pt_legacy_37, r_legacy_37) = decrypt_note_memo_deterministic(&legacy_37_memo, &ivk)
        .expect("Legacy DEC-037 memo must be decrypted by legacy fallback path");
    assert_eq!(pt_legacy_37.value, 999_000);
    assert_eq!(r_legacy_37, legacy_37_r);
}

#[test]
fn test_memo_key_separation_prg_and_tag_verification() {
    let mut rng = OsRng;
    let mut shared_secret = [0u8; 32];
    rng.fill_bytes(&mut shared_secret);

    // 1. Key separation PRG determinism
    let keys1 = derive_key_separation(&shared_secret).expect("PRG derivation must succeed");
    let keys2 = derive_key_separation(&shared_secret).expect("PRG derivation must succeed");
    assert_eq!(keys1.cipher_key, keys2.cipher_key);
    assert_eq!(keys1.commitment_key, keys2.commitment_key);
    assert_eq!(keys1.nonce_7, keys2.nonce_7);

    // 2. Tag computation and constant-time verification
    let mut epk = [0u8; 32];
    rng.fill_bytes(&mut epk);
    let view_tag = 0x7a;
    let mut ciphertext = [0u8; 40];
    rng.fill_bytes(&mut ciphertext);

    let tag = compute_committing_tag(
        &keys1.commitment_key,
        &keys1.nonce_7,
        &epk,
        view_tag,
        &ciphertext,
    );

    assert!(verify_committing_tag_ct(&tag, &tag));

    let mut tampered_tag = tag;
    tampered_tag[0] ^= 0x01;
    assert!(!verify_committing_tag_ct(&tag, &tampered_tag));
}
