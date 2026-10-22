//! DEC-049 Constant-Time Memo Scanner (CT-FOE) Test Suite
//!
//! Validates:
//! 1. A1: CtNotePlaintext limb representation, ConditionallySelectable, and zeroization
//! 2. A2: Branchless primitives: derive_view_tag_ct, derive_key_separation_ct, compute_committing_tag_ct, is_canonical_fr_ct
//! 3. A3: Constant-time trial_decrypt_ct:
//!    - Exact parity with DEC-048 canonical encryption
//!    - Financial boundaries (0, 1, 5000 USDC, 10B USDC, u64::MAX) and multi-asset (WETH & USDC)
//!    - Non-contributory public key defense (all-zero and low-order points)
//!    - Fail-closed dummy return on wrong IVK, tampered view tag, nonce, ciphertext, or tag
//!    - Branchless non-canonical scalar rejection in encrypted payload
//!    - Random noise resistance

use ark_bls12_381::{Fr, FrConfig};
use ark_ff::{MontConfig, PrimeField, UniformRand};
use rand::rngs::OsRng;
use rand::RngCore;
use subtle::{Choice, ConditionallySelectable};
use zeroize::Zeroize;

use zeltra_core::keys::IncomingViewingKey;
use zeltra_core::memo::{
    compute_committing_tag, derive_key_separation, derive_view_tag, encrypt_committing_note_memo,
    NotePlaintext, ENCRYPTED_MEMO_SIZE,
};
use zeltra_core::memo_ct::{
    compute_committing_tag_ct, derive_key_separation_ct, derive_view_tag_ct, is_canonical_fr_ct,
    trial_decrypt_ct, trial_decrypt_ct_ivk, ConstantTimeMemoScanner, CtNotePlaintext,
};

/// BLS12-381 Fr scalar field modulus r in little-endian byte array:
/// r = 0x73eda753299d7d483339d80809a1d80553bda402fffe5bfeffffffff00000001
const BLS12_381_R_LE: [u8; 32] = [
    0x01, 0x00, 0x00, 0x00, 0xff, 0xff, 0xff, 0xff, 0xfe, 0x5b, 0xfe, 0xff, 0x02, 0xa4, 0xbd, 0x53,
    0x05, 0xd8, 0xa1, 0x09, 0x08, 0xd8, 0x39, 0x33, 0x48, 0x7d, 0x9d, 0x29, 0x53, 0xa7, 0xed, 0x73,
];

fn random_ivk<R: RngCore>(rng: &mut R) -> IncomingViewingKey {
    let mut bytes = [0u8; 32];
    rng.fill_bytes(&mut bytes);
    IncomingViewingKey::from_bytes(&bytes)
}

#[test]
fn test_a1_ct_note_plaintext_conditionally_selectable() {
    let dummy = CtNotePlaintext::dummy();
    assert_eq!(dummy.value, 0);
    assert_eq!(dummy.asset_id, 0);
    assert_eq!(dummy.rho_limbs, [0u64; 4]);

    let candidate = CtNotePlaintext {
        value: 5_000_000_000,
        asset_id: 1,
        rho_limbs: [0x1111, 0x2222, 0x3333, 0x4444],
    };

    // When choice is 0, select first (dummy)
    let selected_0 = CtNotePlaintext::conditional_select(&dummy, &candidate, Choice::from(0));
    assert_eq!(selected_0, dummy);
    assert_eq!(selected_0.value, 0);
    assert_eq!(selected_0.asset_id, 0);
    assert_eq!(selected_0.rho_limbs, [0u64; 4]);

    // When choice is 1, select second (candidate)
    let selected_1 = CtNotePlaintext::conditional_select(&dummy, &candidate, Choice::from(1));
    assert_eq!(selected_1, candidate);
    assert_eq!(selected_1.value, 5_000_000_000);
    assert_eq!(selected_1.asset_id, 1);
    assert_eq!(selected_1.rho_limbs, [0x1111, 0x2222, 0x3333, 0x4444]);
}

#[test]
fn test_a1_ct_note_plaintext_zeroize() {
    let mut pt = CtNotePlaintext {
        value: 42_000_000,
        asset_id: 1,
        rho_limbs: [0xaaaa_bbbb, 0xcccc_dddd, 0xeeee_ffff, 0x1234_5678],
    };

    pt.zeroize();
    assert_eq!(pt.value, 0);
    assert_eq!(pt.asset_id, 0);
    assert_eq!(pt.rho_limbs, [0u64; 4]);

    // Test zeroization with zeroize::Zeroizing wrapper
    {
        let wrapped = zeroize::Zeroizing::new(CtNotePlaintext {
            value: 999,
            asset_id: 0,
            rho_limbs: [1, 2, 3, 4],
        });
        assert_eq!(wrapped.value, 999);
    } // dropped and zeroized safely
}

#[test]
fn test_a1_ct_note_plaintext_to_canonical_note_and_conversions() {
    let mut rng = OsRng;
    let rho = Fr::rand(&mut rng);
    let original = NotePlaintext {
        value: 12_345_678,
        rho,
        asset_id: 1,
    };

    // Conversion From NotePlaintext to CtNotePlaintext
    let ct_pt = CtNotePlaintext::from(original);
    assert_eq!(ct_pt.value, 12_345_678);
    assert_eq!(ct_pt.asset_id, 1);
    assert_eq!(ct_pt.rho_limbs, rho.into_bigint().0);

    // Conversion back to NotePlaintext
    let recovered = ct_pt
        .to_canonical_note()
        .expect("Valid Fr limbs must convert to NotePlaintext");
    assert_eq!(recovered.value, original.value);
    assert_eq!(recovered.asset_id, original.asset_id);
    assert_eq!(recovered.rho, original.rho);

    // Test rejection on invalid asset_id
    let mut invalid_asset = ct_pt;
    invalid_asset.asset_id = 2;
    assert!(invalid_asset.to_canonical_note().is_none());

    invalid_asset.asset_id = u64::MAX;
    assert!(invalid_asset.to_canonical_note().is_none());

    // Test rejection on non-canonical rho_limbs (>= r)
    let mut invalid_rho = ct_pt;
    invalid_rho.rho_limbs = FrConfig::MODULUS.0;
    assert!(invalid_rho.to_canonical_note().is_none());

    // Test PrivateNoteV1 reconstruction
    let owner_key = Fr::rand(&mut rng);
    let private_note = ct_pt
        .to_note(owner_key)
        .expect("Valid note must produce PrivateNoteV1");
    assert_eq!(private_note.value, 12_345_678);
    assert_eq!(private_note.asset_id, 1);
    assert_eq!(private_note.owner_key, owner_key);
    assert_eq!(private_note.rho, rho);
}

#[test]
fn test_a2_is_canonical_fr_ct_boundaries() {
    let mut rng = OsRng;

    // 1. Zero: canonical
    let zero_bytes = [0u8; 32];
    let (is_canon, limbs) = is_canonical_fr_ct(&zero_bytes);
    assert_eq!(is_canon.unwrap_u8(), 1);
    assert_eq!(limbs, [0u64; 4]);

    // 2. One: canonical
    let mut one_bytes = [0u8; 32];
    one_bytes[0] = 1;
    let (is_canon, limbs) = is_canonical_fr_ct(&one_bytes);
    assert_eq!(is_canon.unwrap_u8(), 1);
    assert_eq!(limbs, [1, 0, 0, 0]);

    // 3. Random canonical Fr elements
    for _ in 0..50 {
        let rand_fr = Fr::rand(&mut rng);
        let rand_limbs = rand_fr.into_bigint().0;
        let mut rand_bytes = [0u8; 32];
        for i in 0..4 {
            rand_bytes[i * 8..(i + 1) * 8].copy_from_slice(&rand_limbs[i].to_le_bytes());
        }
        let (is_canon, limbs) = is_canonical_fr_ct(&rand_bytes);
        assert_eq!(is_canon.unwrap_u8(), 1, "Random Fr must be canonical");
        assert_eq!(limbs, rand_limbs);
    }

    // 4. Modulus r - 1: canonical
    let mut r_minus_1_bytes = BLS12_381_R_LE;
    r_minus_1_bytes[0] -= 1; // 0x01 -> 0x00
    let (is_canon, _) = is_canonical_fr_ct(&r_minus_1_bytes);
    assert_eq!(is_canon.unwrap_u8(), 1, "r - 1 must be canonical");

    // 5. Modulus r itself: NON-canonical
    let (is_canon, _) = is_canonical_fr_ct(&BLS12_381_R_LE);
    assert_eq!(is_canon.unwrap_u8(), 0, "Modulus r must be rejected");

    // 6. Modulus r + 1: NON-canonical
    let mut r_plus_1_bytes = BLS12_381_R_LE;
    r_plus_1_bytes[0] += 1;
    let (is_canon, _) = is_canonical_fr_ct(&r_plus_1_bytes);
    assert_eq!(is_canon.unwrap_u8(), 0, "r + 1 must be rejected");

    // 7. Max 256-bit scalar (all 0xFF): NON-canonical
    let max_u256_bytes = [0xFFu8; 32];
    let (is_canon, _) = is_canonical_fr_ct(&max_u256_bytes);
    assert_eq!(is_canon.unwrap_u8(), 0, "2^256 - 1 must be rejected");

    // 8. 2^255 (highest bit set): NON-canonical since r < 2^255
    let mut two_pow_255_bytes = [0u8; 32];
    two_pow_255_bytes[31] = 0x80;
    let (is_canon, _) = is_canonical_fr_ct(&two_pow_255_bytes);
    assert_eq!(is_canon.unwrap_u8(), 0, "2^255 must be rejected");

    // 9. Match upper limbs with lower limb manipulated:
    // Limb 3 == mod 3, Limb 2 == mod 2, Limb 1 > mod 1
    let mut manipulated_limbs = FrConfig::MODULUS.0;
    manipulated_limbs[1] = manipulated_limbs[1].wrapping_add(1);
    let mut manipulated_bytes = [0u8; 32];
    for i in 0..4 {
        manipulated_bytes[i * 8..(i + 1) * 8].copy_from_slice(&manipulated_limbs[i].to_le_bytes());
    }
    let (is_canon, _) = is_canonical_fr_ct(&manipulated_bytes);
    assert_eq!(is_canon.unwrap_u8(), 0, "Higher limb mismatch must reject");
}

#[test]
fn test_a2_branchless_primitives_exact_parity_with_dec048() {
    let mut rng = OsRng;

    for _ in 0..20 {
        let mut shared_secret = [0u8; 32];
        rng.fill_bytes(&mut shared_secret);

        // 1. View tag parity
        let vt_legacy = derive_view_tag(&shared_secret);
        let vt_ct = derive_view_tag_ct(&shared_secret);
        assert_eq!(vt_ct, vt_legacy, "View tag CT must match DEC-045C/DEC-048");

        // 2. Key separation PRG parity
        let ks_legacy = derive_key_separation(&shared_secret).expect("Valid shared secret");
        let (success, cipher_key, commitment_key, nonce_7) =
            derive_key_separation_ct(&shared_secret);
        assert_eq!(success.unwrap_u8(), 1);
        assert_eq!(cipher_key, ks_legacy.cipher_key);
        assert_eq!(commitment_key, ks_legacy.commitment_key);
        assert_eq!(nonce_7, ks_legacy.nonce_7);

        // 3. Committing tag parity
        let mut epk = [0u8; 32];
        rng.fill_bytes(&mut epk);
        let mut ciphertext = [0u8; 40];
        rng.fill_bytes(&mut ciphertext);

        let tag_legacy = compute_committing_tag(
            &ks_legacy.commitment_key,
            &ks_legacy.nonce_7,
            &epk,
            vt_legacy,
            &ciphertext,
        );
        let tag_ct = compute_committing_tag_ct(&commitment_key, &nonce_7, &epk, vt_ct, &ciphertext);
        assert_eq!(tag_ct, tag_legacy, "Committing tag CT must match DEC-048");
    }
}

#[test]
fn test_a3_trial_decrypt_ct_financial_boundaries_and_assets() {
    let mut rng = OsRng;
    let ivk = random_ivk(&mut rng);

    let boundary_values: [u64; 6] = [
        0,
        1,
        2_147_483_647,          // 2^31 - 1
        5_000_000_000,          // 5,000 USDC
        10_000_000_000_000_000, // 10 Billion USDC
        u64::MAX,               // 18.44 Quintillion
    ];

    for &value in &boundary_values {
        for asset_id in [0u64, 1u64] {
            let rho = Fr::rand(&mut rng);
            let (memo_bytes, _expected_randomness) =
                encrypt_committing_note_memo(value, rho, asset_id, &ivk)
                    .expect("Encryption must succeed");

            let memo_arr: [u8; ENCRYPTED_MEMO_SIZE] = memo_bytes.try_into().unwrap();

            // Decrypt with trial_decrypt_ct
            let (is_valid, ct_note) = trial_decrypt_ct(&memo_arr, &ivk.secret_bytes());
            assert_eq!(
                is_valid.unwrap_u8(),
                1,
                "Valid memo must be accepted by trial_decrypt_ct"
            );
            assert_eq!(ct_note.value, value);
            assert_eq!(ct_note.asset_id, asset_id);
            assert_eq!(ct_note.rho_limbs, rho.into_bigint().0);

            // Reconstruct canonical note
            let note = ct_note
                .to_canonical_note()
                .expect("Valid decrypted note must convert to NotePlaintext");
            assert_eq!(note.value, value);
            assert_eq!(note.asset_id, asset_id);
            assert_eq!(note.rho, rho);

            // Verify with helper method trial_decrypt_ct_ivk and ConstantTimeMemoScanner
            let (is_valid_ivk, ct_note_ivk) = trial_decrypt_ct_ivk(&memo_arr, &ivk);
            assert_eq!(is_valid_ivk.unwrap_u8(), 1);
            assert_eq!(ct_note_ivk, ct_note);

            let (is_valid_scanner, ct_note_scanner) =
                ConstantTimeMemoScanner::trial_decrypt_ct(&memo_arr, &ivk.secret_bytes());
            assert_eq!(is_valid_scanner.unwrap_u8(), 1);
            assert_eq!(ct_note_scanner, ct_note);

            // Verify PrivateNoteV1 derivation
            let owner_key = Fr::rand(&mut rng);
            let p_note = ct_note.to_note(owner_key).unwrap();
            let expected_note = note.to_note(owner_key);
            assert_eq!(p_note.commitment(), expected_note.commitment());
        }
    }
}

#[test]
fn test_a3_trial_decrypt_ct_wrong_recipient_ivk() {
    let mut rng = OsRng;
    let alice_ivk = random_ivk(&mut rng);
    let bob_ivk = random_ivk(&mut rng);

    let rho = Fr::rand(&mut rng);
    let (memo_bytes, _) = encrypt_committing_note_memo(1_000_000, rho, 1, &alice_ivk)
        .expect("Encryption must succeed");
    let memo_arr: [u8; ENCRYPTED_MEMO_SIZE] = memo_bytes.try_into().unwrap();

    // Bob attempts to trial decrypt Alice's memo
    let (is_valid, ct_note) = trial_decrypt_ct(&memo_arr, &bob_ivk.secret_bytes());
    assert_eq!(
        is_valid.unwrap_u8(),
        0,
        "Trial decrypt under wrong IVK must fail-closed"
    );
    assert_eq!(
        ct_note,
        CtNotePlaintext::dummy(),
        "Must return dummy plaintext on failure"
    );
}

#[test]
fn test_a3_trial_decrypt_ct_non_contributory_public_key() {
    let mut rng = OsRng;
    let ivk = random_ivk(&mut rng);
    let rho = Fr::rand(&mut rng);
    let (mut memo_bytes, _) =
        encrypt_committing_note_memo(500, rho, 0, &ivk).expect("Encryption must succeed");

    // Overwrite EPK with all-zero point (AUD-26 non-contributory point)
    memo_bytes[0..32].fill(0);
    let memo_arr: [u8; ENCRYPTED_MEMO_SIZE] = memo_bytes.try_into().unwrap();

    let (is_valid, ct_note) = trial_decrypt_ct(&memo_arr, &ivk.secret_bytes());
    assert_eq!(
        is_valid.unwrap_u8(),
        0,
        "All-zero public key must be rejected by non-contributory check"
    );
    assert_eq!(ct_note, CtNotePlaintext::dummy());

    // Overwrite EPK with low-order point 1 (order 8 point in RFC 7748)
    let mut memo_bytes_2 = memo_arr;
    memo_bytes_2[0..32].fill(0);
    memo_bytes_2[0] = 1;
    let (is_valid_2, ct_note_2) = trial_decrypt_ct(&memo_bytes_2, &ivk.secret_bytes());
    assert_eq!(
        is_valid_2.unwrap_u8(),
        0,
        "Low-order point must be rejected"
    );
    assert_eq!(ct_note_2, CtNotePlaintext::dummy());
}

#[test]
fn test_a3_trial_decrypt_ct_tampered_fields_fail_closed() {
    let mut rng = OsRng;
    let ivk = random_ivk(&mut rng);
    let rho = Fr::rand(&mut rng);
    let (valid_memo, _) =
        encrypt_committing_note_memo(10_000, rho, 1, &ivk).expect("Encryption must succeed");

    // 1. Tamper view tag (byte 32)
    let mut memo_bad_vt: [u8; 96] = valid_memo.clone().try_into().unwrap();
    memo_bad_vt[32] ^= 0x01;
    let (is_valid, ct_note) = trial_decrypt_ct(&memo_bad_vt, &ivk.secret_bytes());
    assert_eq!(
        is_valid.unwrap_u8(),
        0,
        "Tampered view tag must be rejected"
    );
    assert_eq!(ct_note, CtNotePlaintext::dummy());

    // 2. Tamper nonce (bytes 33..40)
    for offset in 33..40 {
        let mut memo_bad_nonce: [u8; 96] = valid_memo.clone().try_into().unwrap();
        memo_bad_nonce[offset] ^= 0x40;
        let (is_valid, ct_note) = trial_decrypt_ct(&memo_bad_nonce, &ivk.secret_bytes());
        assert_eq!(is_valid.unwrap_u8(), 0, "Tampered nonce must be rejected");
        assert_eq!(ct_note, CtNotePlaintext::dummy());
    }

    // 3. Tamper ciphertext (bytes 40..80)
    for offset in [40, 50, 65, 79] {
        let mut memo_bad_ct: [u8; 96] = valid_memo.clone().try_into().unwrap();
        memo_bad_ct[offset] ^= 0x80;
        let (is_valid, ct_note) = trial_decrypt_ct(&memo_bad_ct, &ivk.secret_bytes());
        assert_eq!(
            is_valid.unwrap_u8(),
            0,
            "Tampered ciphertext must be rejected"
        );
        assert_eq!(ct_note, CtNotePlaintext::dummy());
    }

    // 4. Tamper committing tag (bytes 80..96)
    for offset in [80, 85, 90, 95] {
        let mut memo_bad_tag: [u8; 96] = valid_memo.clone().try_into().unwrap();
        memo_bad_tag[offset] ^= 0x01;
        let (is_valid, ct_note) = trial_decrypt_ct(&memo_bad_tag, &ivk.secret_bytes());
        assert_eq!(is_valid.unwrap_u8(), 0, "Tampered tag must be rejected");
        assert_eq!(ct_note, CtNotePlaintext::dummy());
    }
}

#[test]
fn test_a3_trial_decrypt_ct_random_noise_fuzzing() {
    let mut rng = OsRng;
    let ivk = random_ivk(&mut rng);

    // Test 100 completely random 96-byte buffers
    for _ in 0..100 {
        let mut random_memo = [0u8; ENCRYPTED_MEMO_SIZE];
        rng.fill_bytes(&mut random_memo);

        let (is_valid, ct_note) = trial_decrypt_ct(&random_memo, &ivk.secret_bytes());
        assert_eq!(
            is_valid.unwrap_u8(),
            0,
            "Random noise must always fail-closed"
        );
        assert_eq!(
            ct_note,
            CtNotePlaintext::dummy(),
            "Random noise must produce dummy note"
        );
    }
}
