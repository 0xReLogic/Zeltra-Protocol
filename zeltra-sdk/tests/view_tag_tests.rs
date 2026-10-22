//! Comprehensive Unit & Soundness Tests for DEC-045C 1-Byte View Tag Memo Protocol.
//!
//! Enforces:
//! - Positive Tests: C5.1 (Fast Scan 1000 Memos), C5.2 (Statistical Uniformity Chi-Square), C5.3 (Legacy Fallback)
//! - Negative Tests (>2x ratio, state invariance):
//!   C5.4 (Ciphertext Tamper MAC Failure), C5.5 (View Tag Tamper Pre-Filter Skip),
//!   C5.6 (Invalid Memo Length Boundary), C5.7 (Wrong IVK Failure),
//!   C5.8 (Tampered Ephemeral PK), C5.9 (Dummy Zero Note Skip),
//!   C5.10 (Non-Canonical Scalar Modulus Rejection).

use ark_bls12_381::Fr;
use ark_ff::UniformRand;
use rand::rngs::OsRng;
use rand::RngCore;

use zeltra_core::{
    create_legacy_dec037_memo, derive_owner_key, derive_view_tag,
    encrypt_committing_note_memo, encrypt_note_memo_deterministic, fr_to_be_bytes, note_commitment,
    x25519_dalek, ENCRYPTED_MEMO_SIZE, INFO_MEMO_CIPHER_V1,
};
use zeltra_sdk::wallet::note_wallet::{NoteEvent, PrivateNoteWallet};


// ═══════════════════════════════════════════════════════════════════════════
// POSITIVE TESTS (3 Tests)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_view_tag_derivation_and_fast_scan_success() {
    let mut rng = OsRng;
    let seed = [42u8; 32];
    let mut wallet = PrivateNoteWallet::new(&seed);
    let zsa = wallet.shielded_address().unwrap();
    let ivk = wallet.incoming_viewing_key().unwrap();
    let sk = wallet.spending_key().unwrap();
    let owner_key = derive_owner_key(sk);

    let value = 50_000_000u64; // 50 USDC
    let rho = Fr::rand(&mut rng);
    let asset_id = 1u64;

    // Target note for wallet
    let (target_memo, target_rand) =
        encrypt_note_memo_deterministic(value, rho, asset_id, &zsa.incoming_viewing_public_key)
            .expect("encryption success");
    let target_cm = note_commitment(value, asset_id as u8, owner_key, rho, target_rand);
    let target_cm_hex = format!("0x{}", hex::encode(fr_to_be_bytes(&target_cm)));

    let mut events = Vec::with_capacity(1000);
    let mut prefilter_discard_count = 0usize;

    // Generate 999 noise memos
    for i in 0..999 {
        let mut random_memo = vec![0u8; ENCRYPTED_MEMO_SIZE];
        rng.fill_bytes(&mut random_memo);

        // Check if prefilter drops it
        let eph_pk: [u8; 32] = random_memo[0..32].try_into().unwrap();
        let shared = ivk.diffie_hellman(&eph_pk);
        let expected_tag = derive_view_tag(&shared);
        if random_memo[32] != expected_tag {
            prefilter_discard_count += 1;
        }

        events.push(NoteEvent {
            leaf_index: i as u64,
            commitment_hex: format!("0x{:064x}", i),
            new_mmr_root_hex: "0xroot".to_string(),
            leaf_count: i as u64 + 1,
            encrypted_memo: random_memo,
            block_number: Some(100 + i as u64),
        });
    }

    // Insert target note at position 500
    events.insert(
        500,
        NoteEvent {
            leaf_index: 500,
            commitment_hex: target_cm_hex.clone(),
            new_mmr_root_hex: "0xroot".to_string(),
            leaf_count: 501,
            encrypted_memo: target_memo,
            block_number: Some(600),
        },
    );

    // Fast-scan pre-filter should discard ~99.6% of non-owned memos
    assert!(
        prefilter_discard_count >= 980,
        "Expected at least 980 memos discarded at pre-filter, got {prefilter_discard_count}"
    );

    let res = wallet.scan_memos_fast(&events).expect("fast scan succeeds");
    assert_eq!(res.recovered_notes.len(), 1, "Must find exactly 1 target note");
    assert_eq!(res.total_unspent_balance, value);
    assert_eq!(res.recovered_notes[0].commitment_hex, target_cm_hex);
    assert_eq!(wallet.get_balance(asset_id as u8).unwrap(), value);
}

#[test]
fn test_view_tag_uniform_distribution_statistical_entropy() {
    let mut counts = [0usize; 256];
    let num_samples = 10_000usize;

    for _ in 0..num_samples {
        let mut secret = [0u8; 32];
        OsRng.fill_bytes(&mut secret);
        let tag = derive_view_tag(&secret);
        counts[tag as usize] += 1;
    }

    let expected = num_samples as f64 / 256.0; // 39.0625
    let chi_square: f64 = counts
        .iter()
        .map(|&c| {
            let diff = c as f64 - expected;
            (diff * diff) / expected
        })
        .sum();

    // Critical value for Chi-Square distribution with df = 255 at alpha = 0.001 is 331.4
    assert!(
        chi_square < 331.4,
        "Chi-square test failed: statistic {chi_square} >= critical value 331.4 (p <= 0.001)"
    );
}

#[test]
fn test_view_tag_backward_compatible_fallback_scan() {
    let mut rng = OsRng;
    let seed = [77u8; 32];
    let mut wallet = PrivateNoteWallet::new(&seed);
    let zsa = wallet.shielded_address().unwrap();
    let sk = wallet.spending_key().unwrap();
    let owner_key = derive_owner_key(sk);

    // 1. Legacy DEC-037 note (48-byte plaintext)
    let val_legacy = 25_000_000u64; // 25 USDC
    let rho_legacy = Fr::rand(&mut rng);
    let (legacy_memo, rand_legacy) = create_legacy_dec037_memo(
        val_legacy,
        rho_legacy,
        1,
        zsa.incoming_viewing_public_key,
    );
    let cm_legacy = note_commitment(val_legacy, 1, owner_key, rho_legacy, rand_legacy);
    let cm_legacy_hex = format!("0x{}", hex::encode(fr_to_be_bytes(&cm_legacy)));

    // 2. Legacy DEC-045C note (36-byte plaintext with view tag)
    let val_045c = 35_000_000u64; // 35 USDC
    let rho_045c = Fr::rand(&mut rng);
    let (memo_045c, rand_045c) =
        encrypt_note_memo_deterministic(val_045c, rho_045c, 1, &zsa.incoming_viewing_public_key)
            .unwrap();
    let cm_045c = note_commitment(val_045c, 1, owner_key, rho_045c, rand_045c);
    let cm_045c_hex = format!("0x{}", hex::encode(fr_to_be_bytes(&cm_045c)));

    // 3. New DEC-048 note (40-byte committing AEAD EtH-KS memo with view tag)
    let val_048 = 50_000_000u64; // 50 USDC
    let rho_048 = Fr::rand(&mut rng);
    let (memo_048, rand_048) =
        encrypt_committing_note_memo(val_048, rho_048, 1, &zsa.incoming_viewing_public_key)
            .unwrap();
    let cm_048 = note_commitment(val_048, 1, owner_key, rho_048, rand_048);
    let cm_048_hex = format!("0x{}", hex::encode(fr_to_be_bytes(&cm_048)));

    let events = vec![
        NoteEvent {
            leaf_index: 0,
            commitment_hex: cm_legacy_hex.clone(),
            new_mmr_root_hex: "0xroot".to_string(),
            leaf_count: 1,
            encrypted_memo: legacy_memo,
            block_number: Some(10),
        },
        NoteEvent {
            leaf_index: 1,
            commitment_hex: cm_045c_hex.clone(),
            new_mmr_root_hex: "0xroot".to_string(),
            leaf_count: 2,
            encrypted_memo: memo_045c,
            block_number: Some(11),
        },
        NoteEvent {
            leaf_index: 2,
            commitment_hex: cm_048_hex.clone(),
            new_mmr_root_hex: "0xroot".to_string(),
            leaf_count: 3,
            encrypted_memo: memo_048,
            block_number: Some(12),
        },
    ];

    let total_expected = val_legacy + val_045c + val_048;

    let res = wallet
        .scan_memos_fast(&events)
        .expect("mixed batch scan must succeed");
    assert_eq!(
        res.recovered_notes.len(),
        3,
        "All three memo generations (DEC-037, DEC-045C, DEC-048) must be recovered"
    );
    assert_eq!(res.total_unspent_balance, total_expected);
    assert_eq!(wallet.get_balance(1).unwrap(), total_expected);
}

// ═══════════════════════════════════════════════════════════════════════════
// NEGATIVE TESTS (7 Tests — >2x Ratio, Proving State Invariance)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_view_tag_rejects_corrupted_ciphertext_mac_failure() {
    let mut rng = OsRng;
    let seed = [11u8; 32];
    let mut wallet = PrivateNoteWallet::new(&seed);
    let zsa = wallet.shielded_address().unwrap();
    let initial_balance = wallet.get_balance(1).unwrap_or(0);

    let (mut memo, _) =
        encrypt_note_memo_deterministic(50_000_000, Fr::rand(&mut rng), 1, &zsa.incoming_viewing_public_key)
            .unwrap();

    // Tamper 1 byte in ciphertext (bytes 44..80) while preserving matching tag at memo[32]
    memo[50] ^= 0x55;

    let event = NoteEvent {
        leaf_index: 0,
        commitment_hex: "0xdeadbeef".to_string(),
        new_mmr_root_hex: "0xroot".to_string(),
        leaf_count: 1,
        encrypted_memo: memo,
        block_number: Some(1),
    };

    let res = wallet.scan_memos_fast(&[event]).expect("scanner does not panic");
    assert_eq!(res.recovered_notes.len(), 0, "MAC failure must drop candidate");
    assert_eq!(
        wallet.get_balance(1).unwrap_or(0),
        initial_balance,
        "Wallet balance must remain strictly unchanged"
    );
}

#[test]
fn test_view_tag_rejects_tampered_view_tag() {
    let mut rng = OsRng;
    let seed = [22u8; 32];
    let mut wallet = PrivateNoteWallet::new(&seed);
    let zsa = wallet.shielded_address().unwrap();
    let initial_balance = wallet.get_balance(1).unwrap_or(0);

    let (mut memo, _) =
        encrypt_note_memo_deterministic(50_000_000, Fr::rand(&mut rng), 1, &zsa.incoming_viewing_public_key)
            .unwrap();

    // Tamper view tag at offset 32
    memo[32] ^= 0x01;

    let event = NoteEvent {
        leaf_index: 0,
        commitment_hex: "0xdeadbeef".to_string(),
        new_mmr_root_hex: "0xroot".to_string(),
        leaf_count: 1,
        encrypted_memo: memo,
        block_number: Some(1),
    };

    let res = wallet.scan_memos_fast(&[event]).expect("scanner does not panic");
    assert_eq!(res.recovered_notes.len(), 0, "Pre-filter tag mismatch must drop note");
    assert_eq!(
        wallet.get_balance(1).unwrap_or(0),
        initial_balance,
        "Wallet balance must remain invariant"
    );
}

#[test]
fn test_view_tag_rejects_invalid_memo_length() {
    let seed = [33u8; 32];
    let mut wallet = PrivateNoteWallet::new(&seed);
    let initial_balance = wallet.get_balance(1).unwrap_or(0);

    let invalid_memos = vec![
        vec![0u8; 0],
        vec![0u8; 95],
        vec![0u8; 97],
        vec![0u8; 128],
    ];

    let events: Vec<NoteEvent> = invalid_memos
        .into_iter()
        .enumerate()
        .map(|(i, m)| NoteEvent {
            leaf_index: i as u64,
            commitment_hex: format!("0x{:064x}", i),
            new_mmr_root_hex: "0xroot".to_string(),
            leaf_count: i as u64 + 1,
            encrypted_memo: m,
            block_number: Some(10),
        })
        .collect();

    let res = wallet.scan_memos_fast(&events).expect("no panic on invalid lengths");
    assert_eq!(res.recovered_notes.len(), 0);
    assert_eq!(wallet.get_balance(1).unwrap_or(0), initial_balance);
}

#[test]
fn test_view_tag_fails_with_wrong_incoming_viewing_key() {
    let mut rng = OsRng;
    let alice_seed = [44u8; 32];
    let bob_seed = [55u8; 32];

    let alice_wallet = PrivateNoteWallet::new(&alice_seed);
    let mut bob_wallet = PrivateNoteWallet::new(&bob_seed);

    let alice_zsa = alice_wallet.shielded_address().unwrap();
    let bob_initial_balance = bob_wallet.get_balance(1).unwrap_or(0);

    // Note encrypted for Alice
    let (memo, _) =
        encrypt_note_memo_deterministic(100_000_000, Fr::rand(&mut rng), 1, &alice_zsa.incoming_viewing_public_key)
            .unwrap();

    let event = NoteEvent {
        leaf_index: 0,
        commitment_hex: "0xalice_note".to_string(),
        new_mmr_root_hex: "0xroot".to_string(),
        leaf_count: 1,
        encrypted_memo: memo,
        block_number: Some(1),
    };

    // Bob scans Alice's memo
    let res = bob_wallet.scan_memos_fast(&[event]).expect("no panic");
    assert_eq!(res.recovered_notes.len(), 0, "Bob cannot recover Alice's note");
    assert_eq!(bob_wallet.get_balance(1).unwrap_or(0), bob_initial_balance);
}

#[test]
fn test_view_tag_rejects_tampered_ephemeral_pubkey() {
    let mut rng = OsRng;
    let seed = [66u8; 32];
    let mut wallet = PrivateNoteWallet::new(&seed);
    let zsa = wallet.shielded_address().unwrap();
    let initial_balance = wallet.get_balance(1).unwrap_or(0);

    let (mut memo, _) =
        encrypt_note_memo_deterministic(50_000_000, Fr::rand(&mut rng), 1, &zsa.incoming_viewing_public_key)
            .unwrap();

    // Tamper ephemeral public key bytes 0..32
    memo[0] ^= 0xff;

    let event = NoteEvent {
        leaf_index: 0,
        commitment_hex: "0xcm".to_string(),
        new_mmr_root_hex: "0xroot".to_string(),
        leaf_count: 1,
        encrypted_memo: memo,
        block_number: Some(1),
    };

    let res = wallet.scan_memos_fast(&[event]).expect("no panic");
    assert_eq!(res.recovered_notes.len(), 0);
    assert_eq!(wallet.get_balance(1).unwrap_or(0), initial_balance);
}

#[test]
fn test_view_tag_ignores_dummy_change_note() {
    let mut rng = OsRng;
    let seed = [88u8; 32];
    let mut wallet = PrivateNoteWallet::new(&seed);
    let zsa = wallet.shielded_address().unwrap();
    let initial_balance = wallet.get_balance(1).unwrap_or(0);

    // Dummy change note with value = 0 (Mode 2 exact transfer DEC-043C)
    let (memo, _) =
        encrypt_note_memo_deterministic(0, Fr::rand(&mut rng), 1, &zsa.incoming_viewing_public_key)
            .unwrap();

    let event = NoteEvent {
        leaf_index: 0,
        commitment_hex: "0xdummy_cm".to_string(),
        new_mmr_root_hex: "0xroot".to_string(),
        leaf_count: 1,
        encrypted_memo: memo,
        block_number: Some(1),
    };

    let res = wallet.scan_memos_fast(&[event]).expect("no panic");
    assert_eq!(res.recovered_notes.len(), 0, "Zero-value dummy note must be dropped");
    assert_eq!(
        wallet.get_balance(1).unwrap_or(0),
        initial_balance,
        "Wallet balance must not increase"
    );
}

#[test]
fn test_view_tag_rejects_non_canonical_scalar_during_commitment_reconstruction() {
    let seed = [99u8; 32];
    let mut wallet = PrivateNoteWallet::new(&seed);
    let zsa = wallet.shielded_address().unwrap();
    let initial_balance = wallet.get_balance(1).unwrap_or(0);

    // Build memo payload manually with rho >= r (non-canonical scalar)
    // BLS12-381 r in BE: 0x73eda753299d7d483339d80809a1d80553bda402fffe5bfeffffffff00000001
    let non_canonical_rho = [0xffu8; 32];
    let mut pt_bytes = [0u8; 36];
    let header = 1u32 << 31 | 50_000_000u32; // asset_id 1, value 50M
    pt_bytes[0..4].copy_from_slice(&header.to_be_bytes());
    pt_bytes[4..36].copy_from_slice(&non_canonical_rho);

    let mut eph_bytes = [0u8; 32];
    OsRng.fill_bytes(&mut eph_bytes);
    let ephemeral_secret = x25519_dalek::StaticSecret::from(eph_bytes);
    let ephemeral_public = x25519_dalek::PublicKey::from(&ephemeral_secret);
    let recipient_public = x25519_dalek::PublicKey::from(zsa.incoming_viewing_public_key);

    let shared_secret = ephemeral_secret.diffie_hellman(&recipient_public);
    let shared_secret_bytes = *shared_secret.as_bytes();
    let view_tag = derive_view_tag(&shared_secret_bytes);

    let hk = zeltra_core::hkdf::Hkdf::<sha2::Sha256>::new(
        Some(b"ZELTRA_NOTE_MEMO_KDF_V1"),
        &shared_secret_bytes,
    );
    let mut okm = [0u8; 43];
    hk.expand(INFO_MEMO_CIPHER_V1, &mut okm).unwrap();

    let mut key_bytes = [0u8; 32];
    key_bytes.copy_from_slice(&okm[0..32]);
    let cipher_key = chacha20poly1305::Key::from(key_bytes);

    let mut nonce_11 = [0u8; 11];
    nonce_11.copy_from_slice(&okm[32..43]);
    let mut nonce_12 = [0u8; 12];
    nonce_12[0..11].copy_from_slice(&nonce_11);
    nonce_12[11] = 0u8;
    let nonce = chacha20poly1305::Nonce::from(nonce_12);

    use chacha20poly1305::aead::{Aead, KeyInit};
    let cipher = chacha20poly1305::ChaCha20Poly1305::new(&cipher_key);
    let ciphertext_and_tag = cipher.encrypt(&nonce, pt_bytes.as_ref()).unwrap();

    let mut memo = Vec::with_capacity(ENCRYPTED_MEMO_SIZE);
    memo.extend_from_slice(ephemeral_public.as_bytes());
    memo.push(view_tag);
    memo.extend_from_slice(&nonce_11);
    memo.extend_from_slice(&ciphertext_and_tag[0..36]);
    memo.extend_from_slice(&ciphertext_and_tag[36..52]);

    let event = NoteEvent {
        leaf_index: 0,
        commitment_hex: "0xinvalid_scalar".to_string(),
        new_mmr_root_hex: "0xroot".to_string(),
        leaf_count: 1,
        encrypted_memo: memo,
        block_number: Some(1),
    };

    let res = wallet.scan_memos_fast(&[event]).expect("no panic");
    assert_eq!(
        res.recovered_notes.len(),
        0,
        "Non-canonical scalar must be rejected before commitment reconstruction"
    );
    assert_eq!(wallet.get_balance(1).unwrap_or(0), initial_balance);
}

// ═══════════════════════════════════════════════════════════════════════════
// DEC-048 COMMITTING AEAD & LARGE VALUE TESTS
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_dec048_large_value_5000_usdc_recovered_cleanly() {
    let mut rng = OsRng;
    let seed = [88u8; 32];
    let mut wallet = PrivateNoteWallet::new(&seed);
    let zsa = wallet.shielded_address().unwrap();
    let sk = wallet.spending_key().unwrap();
    let owner_key = derive_owner_key(sk);

    // 5,000 USDC = 5,000,000,000 micro-units (> 2^31 = 2,147,483,648)
    // Previously in DEC-045C 31-bit packing, values > 2^31 were truncated causing commitment desync.
    // In DEC-048 40-byte Field-Bit Embedding, the full u64 is preserved without truncation.
    let value = 5_000_000_000u64;
    let rho = Fr::rand(&mut rng);
    let asset_id = 1u64;

    let (memo, rand) =
        encrypt_committing_note_memo(value, rho, asset_id, &zsa.incoming_viewing_public_key)
            .expect("encryption success");
    assert_eq!(memo.len(), ENCRYPTED_MEMO_SIZE);

    let target_cm = note_commitment(value, asset_id as u8, owner_key, rho, rand);
    let target_cm_hex = format!("0x{}", hex::encode(fr_to_be_bytes(&target_cm)));

    let event = NoteEvent {
        leaf_index: 42,
        commitment_hex: target_cm_hex.clone(),
        new_mmr_root_hex: "0xroot".to_string(),
        leaf_count: 43,
        encrypted_memo: memo.clone(),
        block_number: Some(12345),
    };

    // 1. Test fast scanner
    let res = wallet
        .scan_memos_fast(std::slice::from_ref(&event))
        .expect("fast scan succeeds");
    assert_eq!(
        res.recovered_notes.len(),
        1,
        "5,000 USDC note must be recovered"
    );
    assert_eq!(
        res.recovered_notes[0].value, value,
        "Recovered note value must be exactly 5,000,000,000 micro-units"
    );
    assert_eq!(
        res.recovered_notes[0].commitment_hex, target_cm_hex,
        "Decrypted commitment must match on-chain commitment exactly"
    );
    assert_eq!(res.total_unspent_balance, value);
    assert_eq!(wallet.get_balance(1).unwrap(), value);

    // 2. Test batch nullifier scanner on fresh wallet instance
    let mut wallet2 = PrivateNoteWallet::new(&seed);
    let res_batch = wallet2
        .recover_notes_batch(&[event], |_| Ok(vec![false]))
        .expect("recover_notes_batch succeeds");
    assert_eq!(
        res_batch.recovered_notes.len(),
        1,
        "recover_notes_batch must also recover the note"
    );
    assert_eq!(res_batch.recovered_notes[0].value, value);
    assert_eq!(
        res_batch.recovered_notes[0].commitment_hex, target_cm_hex,
        "recover_notes_batch commitment must match exactly"
    );
    assert_eq!(wallet2.get_balance(1).unwrap(), value);
}

#[test]
fn test_dec048_committing_memo_tampered_tag_rejected() {
    let mut rng = OsRng;
    let seed = [33u8; 32];
    let mut wallet = PrivateNoteWallet::new(&seed);
    let zsa = wallet.shielded_address().unwrap();
    let initial_balance = wallet.get_balance(1).unwrap_or(0);

    let (mut memo, _) = encrypt_committing_note_memo(
        50_000_000,
        Fr::rand(&mut rng),
        1,
        &zsa.incoming_viewing_public_key,
    )
    .unwrap();

    // Tamper committing tag (offset 80..96)
    memo[85] ^= 0xaa;

    let event = NoteEvent {
        leaf_index: 0,
        commitment_hex: "0xdeadbeef".to_string(),
        new_mmr_root_hex: "0xroot".to_string(),
        leaf_count: 1,
        encrypted_memo: memo,
        block_number: Some(1),
    };

    let res = wallet.scan_memos_fast(&[event]).expect("no panic");
    assert_eq!(
        res.recovered_notes.len(),
        0,
        "Committing tag mismatch must drop candidate in verify-then-decrypt"
    );
    assert_eq!(
        wallet.get_balance(1).unwrap_or(0),
        initial_balance,
        "Wallet balance must remain unchanged"
    );
}

#[test]
fn test_dec048_committing_memo_tampered_nonce_rejected() {
    let mut rng = OsRng;
    let seed = [44u8; 32];
    let mut wallet = PrivateNoteWallet::new(&seed);
    let zsa = wallet.shielded_address().unwrap();
    let initial_balance = wallet.get_balance(1).unwrap_or(0);

    let (mut memo, _) = encrypt_committing_note_memo(
        50_000_000,
        Fr::rand(&mut rng),
        1,
        &zsa.incoming_viewing_public_key,
    )
    .unwrap();

    // Tamper 7-byte nonce (offset 33..40)
    memo[35] ^= 0xff;

    let event = NoteEvent {
        leaf_index: 0,
        commitment_hex: "0xdeadbeef".to_string(),
        new_mmr_root_hex: "0xroot".to_string(),
        leaf_count: 1,
        encrypted_memo: memo,
        block_number: Some(1),
    };

    let res = wallet.scan_memos_fast(&[event]).expect("no panic");
    assert_eq!(
        res.recovered_notes.len(),
        0,
        "Nonce mismatch must drop candidate"
    );
    assert_eq!(
        wallet.get_balance(1).unwrap_or(0),
        initial_balance,
        "Wallet balance must remain unchanged"
    );
}

#[test]
fn test_dec048_committing_memo_tampered_ciphertext_rejected() {
    let mut rng = OsRng;
    let seed = [55u8; 32];
    let mut wallet = PrivateNoteWallet::new(&seed);
    let zsa = wallet.shielded_address().unwrap();
    let initial_balance = wallet.get_balance(1).unwrap_or(0);

    let (mut memo, _) = encrypt_committing_note_memo(
        50_000_000,
        Fr::rand(&mut rng),
        1,
        &zsa.incoming_viewing_public_key,
    )
    .unwrap();

    // Tamper ciphertext payload (offset 40..80)
    memo[50] ^= 0x42;

    let event = NoteEvent {
        leaf_index: 0,
        commitment_hex: "0xdeadbeef".to_string(),
        new_mmr_root_hex: "0xroot".to_string(),
        leaf_count: 1,
        encrypted_memo: memo,
        block_number: Some(1),
    };

    let res = wallet.scan_memos_fast(&[event]).expect("no panic");
    assert_eq!(
        res.recovered_notes.len(),
        0,
        "Ciphertext tampering must fail committing HMAC-SHA256 tag verification"
    );
    assert_eq!(
        wallet.get_balance(1).unwrap_or(0),
        initial_balance,
        "Wallet balance must remain unchanged"
    );
}
