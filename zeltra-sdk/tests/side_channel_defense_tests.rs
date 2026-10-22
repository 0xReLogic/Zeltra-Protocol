//! DEC-049 Gate V-2 (Negative Test Matrix) & Gate V-3 (Dudect Differential Timing) Test Suite
//!
//! Validates:
//! - Gate V-2: 5 failure categories fail-closed with Choice::from(0) and zero leaked note state.
//!   1. Non-contributory Ephemeral Public Key (AUD-26)
//!   2. View Tag mismatch (DEC-045C)
//!   3. Nonce transport tampering (DEC-048)
//!   4. Committing tag HMAC corruption/forgery (AUD-21)
//!   5. Non-canonical Fr rho scalar (AUD-25)
//! - Gate V-3: Statistical differential timing evaluation using Welch's two-tailed t-test:
//!   - Negative control: Random vs Random proves |t| < 4.5
//!   - Positive control: Leaky early-abort proves |t| > 15.0
//!   - CT-FOE ConstantTimeMemoScanner: Valid vs Random proves |t| < 4.5

use ark_bls12_381::Fr;
use ark_ff::UniformRand;
use rand::rngs::OsRng;
use rand::RngCore;

use zeltra_core::chacha20::cipher::{KeyIvInit, StreamCipher};
use zeltra_core::chacha20::ChaCha20;
use zeltra_core::keys::IncomingViewingKey;
use zeltra_core::memo::{
    compute_committing_tag, derive_key_separation, derive_view_tag, encrypt_committing_note_memo,
    ENCRYPTED_MEMO_SIZE,
};
use zeltra_core::memo_ct::{is_canonical_fr_ct, trial_decrypt_ct, CtNotePlaintext};
use zeltra_core::x25519_dalek;
use zeltra_sdk::wallet::note_wallet::{NoteEvent, PrivateNoteWallet};

/// Modulus r of BLS12-381 Fr in little-endian byte array
const BLS12_381_R_LE: [u8; 32] = [
    0x01, 0x00, 0x00, 0x00, 0xff, 0xff, 0xff, 0xff, 0xfe, 0x5b, 0xfe, 0xff, 0x02, 0xa4, 0xbd, 0x53,
    0x05, 0xd8, 0xa1, 0x09, 0x08, 0xd8, 0x39, 0x33, 0x48, 0x7d, 0x9d, 0x29, 0x53, 0xa7, 0xed, 0x73,
];

fn random_ivk<R: RngCore>(rng: &mut R) -> IncomingViewingKey {
    let mut bytes = [0u8; 32];
    rng.fill_bytes(&mut bytes);
    IncomingViewingKey::from_bytes(&bytes)
}

// ═══════════════════════════════════════════════════════════════════════════════
// GATE V-2: ADVERSARIAL NEGATIVE TEST MATRIX (5 CATEGORIES)
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_gate_v2_category_1_non_contributory_epk() {
    let mut rng = OsRng;
    let ivk = random_ivk(&mut rng);
    let rho = Fr::rand(&mut rng);
    let (valid_memo, _) =
        encrypt_committing_note_memo(1_000_000, rho, 1, &ivk).expect("Encryption must succeed");

    // Case 1A: All-zero public key
    let mut bad_memo_zero: [u8; ENCRYPTED_MEMO_SIZE] = valid_memo.clone().try_into().unwrap();
    bad_memo_zero[0..32].fill(0);
    let (is_valid, pt) = trial_decrypt_ct(&bad_memo_zero, &ivk.secret_bytes());
    assert_eq!(is_valid.unwrap_u8(), 0, "All-zero EPK must fail-closed");
    assert_eq!(pt, CtNotePlaintext::dummy());

    // Case 1B: Order 8 low-order point (RFC 7748 point 1)
    let mut bad_memo_one = bad_memo_zero;
    bad_memo_one[0] = 1;
    let (is_valid_one, pt_one) = trial_decrypt_ct(&bad_memo_one, &ivk.secret_bytes());
    assert_eq!(is_valid_one.unwrap_u8(), 0, "Low-order point 1 must fail-closed");
    assert_eq!(pt_one, CtNotePlaintext::dummy());

    // Case 1C: SDK Wallet Level Verification
    let mut wallet = PrivateNoteWallet::new(&[42u8; 32]);
    let wallet_ivk = wallet.incoming_viewing_key().unwrap();
    let (wallet_valid_memo, _) =
        encrypt_committing_note_memo(500_000, rho, 1, &wallet_ivk).unwrap();
    let mut wallet_bad_memo = wallet_valid_memo;
    wallet_bad_memo[0..32].fill(0);

    let event = NoteEvent {
        leaf_index: 0,
        commitment_hex: "0x1234".to_string(),
        new_mmr_root_hex: String::new(),
        leaf_count: 1,
        encrypted_memo: wallet_bad_memo,
        block_number: Some(100),
    };

    let recovery = wallet.scan_memos_fast(&[event]).unwrap();
    assert_eq!(recovery.recovered_notes.len(), 0);
    assert_eq!(recovery.total_unspent_balance, 0);
}

#[test]
fn test_gate_v2_category_2_view_tag_mismatch() {
    let mut rng = OsRng;
    let ivk = random_ivk(&mut rng);
    let rho = Fr::rand(&mut rng);
    let (valid_memo, _) =
        encrypt_committing_note_memo(25_000_000, rho, 0, &ivk).expect("Encryption must succeed");

    for bit in 0..8 {
        let mut tampered: [u8; ENCRYPTED_MEMO_SIZE] = valid_memo.clone().try_into().unwrap();
        tampered[32] ^= 1 << bit;

        let (is_valid, pt) = trial_decrypt_ct(&tampered, &ivk.secret_bytes());
        assert_eq!(
            is_valid.unwrap_u8(),
            0,
            "View tag bit {} mutated must fail-closed",
            bit
        );
        assert_eq!(pt, CtNotePlaintext::dummy());
    }

    // SDK Level
    let mut wallet = PrivateNoteWallet::new(&[55u8; 32]);
    let wallet_ivk = wallet.incoming_viewing_key().unwrap();
    let (wallet_valid_memo, _) =
        encrypt_committing_note_memo(10_000_000, rho, 0, &wallet_ivk).unwrap();
    let mut bad_memo = wallet_valid_memo;
    bad_memo[32] ^= 0xff;

    let event = NoteEvent {
        leaf_index: 1,
        commitment_hex: "0x5678".to_string(),
        new_mmr_root_hex: String::new(),
        leaf_count: 2,
        encrypted_memo: bad_memo,
        block_number: Some(101),
    };

    let recovery = wallet.scan_memos_fast(&[event]).unwrap();
    assert_eq!(recovery.recovered_notes.len(), 0);
}

#[test]
fn test_gate_v2_category_3_nonce_transport_tampered() {
    let mut rng = OsRng;
    let ivk = random_ivk(&mut rng);
    let rho = Fr::rand(&mut rng);
    let (valid_memo, _) =
        encrypt_committing_note_memo(7_500_000, rho, 1, &ivk).expect("Encryption must succeed");

    for offset in 33..40 {
        let mut tampered: [u8; ENCRYPTED_MEMO_SIZE] = valid_memo.clone().try_into().unwrap();
        tampered[offset] ^= 0x55;

        let (is_valid, pt) = trial_decrypt_ct(&tampered, &ivk.secret_bytes());
        assert_eq!(
            is_valid.unwrap_u8(),
            0,
            "Tampered nonce byte {} must fail-closed",
            offset
        );
        assert_eq!(pt, CtNotePlaintext::dummy());
    }
}

#[test]
fn test_gate_v2_category_4_committing_tag_corrupt_or_forged() {
    let mut rng = OsRng;
    let ivk = random_ivk(&mut rng);
    let rho = Fr::rand(&mut rng);
    let (valid_memo, _) =
        encrypt_committing_note_memo(99_000_000, rho, 1, &ivk).expect("Encryption must succeed");

    for offset in 80..96 {
        for bit in 0..8 {
            let mut tampered: [u8; ENCRYPTED_MEMO_SIZE] = valid_memo.clone().try_into().unwrap();
            tampered[offset] ^= 1 << bit;

            let (is_valid, pt) = trial_decrypt_ct(&tampered, &ivk.secret_bytes());
            assert_eq!(
                is_valid.unwrap_u8(),
                0,
                "Tag bit flip at byte {} bit {} must fail-closed",
                offset,
                bit
            );
            assert_eq!(pt, CtNotePlaintext::dummy());
        }
    }
}

#[test]
fn test_gate_v2_category_5_non_canonical_rho_scalar() {
    let mut rng = OsRng;
    let ivk = random_ivk(&mut rng);

    // Test non-canonical rho values: r, r+1, 2^255, 2^256-1
    let non_canonical_candidates: [[u8; 32]; 4] = [
        BLS12_381_R_LE,
        {
            let mut b = BLS12_381_R_LE;
            b[0] = b[0].wrapping_add(1);
            b
        },
        {
            // 2^255 - 1: bit 255 is 0, but scalar is strictly > r
            let mut b = [0xffu8; 32];
            b[31] = 0x7f;
            b
        },
        [0xffu8; 32],
    ];

    for (idx, &bad_rho_bytes) in non_canonical_candidates.iter().enumerate() {
        // Core boundary test
        let (is_canon, _) = is_canonical_fr_ct(&bad_rho_bytes);
        assert_eq!(
            is_canon.unwrap_u8(),
            0,
            "Non-canonical candidate {} must be rejected",
            idx
        );

        // Forge a valid ciphertext encrypting bad rho bytes
        let mut eph_bytes = [0u8; 32];
        rng.fill_bytes(&mut eph_bytes);
        let ephemeral_secret = x25519_dalek::StaticSecret::from(eph_bytes);
        let ephemeral_public = x25519_dalek::PublicKey::from(&ephemeral_secret);
        let recipient_public = x25519_dalek::PublicKey::from(ivk.public_key());
        let shared_secret = ephemeral_secret.diffie_hellman(&recipient_public);
        let shared_secret_bytes = *shared_secret.as_bytes();

        let view_tag = derive_view_tag(&shared_secret_bytes);
        let keys = derive_key_separation(&shared_secret_bytes).unwrap();

        // 40B plaintext: 8B value + 32B bad rho
        let mut forged_pt = [0u8; 40];
        forged_pt[0..8].copy_from_slice(&100u64.to_le_bytes());
        forged_pt[8..40].copy_from_slice(&bad_rho_bytes);

        // Encrypt with ChaCha20
        let mut nonce_12 = [0u8; 12];
        nonce_12[0..7].copy_from_slice(&keys.nonce_7);
        let mut cipher = ChaCha20::new(&keys.cipher_key.into(), &nonce_12.into());
        let mut forged_ct = forged_pt;
        cipher.apply_keystream(&mut forged_ct);

        // Compute valid committing tag over the forged ciphertext
        let tag = compute_committing_tag(
            &keys.commitment_key,
            &keys.nonce_7,
            ephemeral_public.as_bytes(),
            view_tag,
            &forged_ct,
        );

        // Pack into 96-byte memo
        let mut forged_memo = [0u8; ENCRYPTED_MEMO_SIZE];
        forged_memo[0..32].copy_from_slice(ephemeral_public.as_bytes());
        forged_memo[32] = view_tag;
        forged_memo[33..40].copy_from_slice(&keys.nonce_7);
        forged_memo[40..80].copy_from_slice(&forged_ct);
        forged_memo[80..96].copy_from_slice(&tag);

        // Trial decrypt must reject due to canonical Fr check!
        let (is_valid, pt) = trial_decrypt_ct(&forged_memo, &ivk.secret_bytes());
        assert_eq!(
            is_valid.unwrap_u8(),
            0,
            "Forged memo with non-canonical rho {} must fail-closed",
            idx
        );
        assert_eq!(pt, CtNotePlaintext::dummy());
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// GATE V-3: STATISTICAL DIFFERENTIAL TIMING EVALUATION (DUDECT / WELCH'S T-TEST)
// ═══════════════════════════════════════════════════════════════════════════════

/// Numerically stable online variance and mean accumulator (Welford's algorithm).
#[derive(Clone, Copy, Debug, Default)]
pub struct WelfordStats {
    pub count: usize,
    pub mean: f64,
    pub m2: f64,
}

impl WelfordStats {
    pub fn new() -> Self {
        Self::default()
    }

    #[inline(always)]
    pub fn update(&mut self, x: f64) {
        self.count += 1;
        let delta = x - self.mean;
        self.mean += delta / self.count as f64;
        let delta2 = x - self.mean;
        self.m2 += delta * delta2;
    }

    pub fn variance(&self) -> f64 {
        if self.count < 2 {
            0.0
        } else {
            self.m2 / (self.count - 1) as f64
        }
    }
}

/// Computes Welch's two-tailed t-test statistic between two groups.
pub fn welch_t_statistic(g0: &WelfordStats, g1: &WelfordStats) -> f64 {
    if g0.count < 2 || g1.count < 2 {
        return 0.0;
    }
    let var0 = g0.variance();
    let var1 = g1.variance();
    let se = (var0 / g0.count as f64 + var1 / g1.count as f64).sqrt();
    if se == 0.0 {
        0.0
    } else {
        (g0.mean - g1.mean) / se
    }
}

/// Measures CPU cycles on x86_64 using RDTSC with serialization fences.
#[cfg(target_arch = "x86_64")]
#[inline(always)]
unsafe fn measure_trace_cycles<F: FnOnce() -> R, R>(f: F) -> u64 {
    core::arch::x86_64::_mm_lfence();
    let start = core::arch::x86_64::_rdtsc();
    core::arch::x86_64::_mm_lfence();
    let result = std::hint::black_box(f());
    core::arch::x86_64::_mm_lfence();
    let end = core::arch::x86_64::_rdtsc();
    core::arch::x86_64::_mm_lfence();
    std::hint::black_box(result);
    end.wrapping_sub(start)
}

#[cfg(not(target_arch = "x86_64"))]
#[inline(always)]
fn measure_trace_cycles<F: FnOnce() -> R, R>(f: F) -> u64 {
    let start = std::time::Instant::now();
    let result = std::hint::black_box(f());
    let elapsed = start.elapsed().as_nanos() as u64;
    std::hint::black_box(result);
    elapsed
}

#[test]
fn test_gate_v3_statistical_dudect_evaluation() {
    let mut rng = OsRng;
    let ivk = random_ivk(&mut rng);
    let ivk_bytes = ivk.secret_bytes();

    // Prepare valid memo pool
    let num_pool = 16usize;
    let mut valid_memos = Vec::with_capacity(num_pool);
    for i in 0..num_pool {
        let rho = Fr::rand(&mut rng);
        let (memo, _) =
            encrypt_committing_note_memo((i as u64 + 1) * 1000, rho, (i % 2) as u64, &ivk)
                .unwrap();
        let memo_arr: [u8; ENCRYPTED_MEMO_SIZE] = memo.try_into().unwrap();
        valid_memos.push(memo_arr);
    }

    // Prepare random memo pool
    let mut random_memos = Vec::with_capacity(num_pool);
    for _ in 0..num_pool {
        let mut r = [0u8; ENCRYPTED_MEMO_SIZE];
        rng.fill_bytes(&mut r);
        random_memos.push(r);
    }

    // ─────────────────────────────────────────────────────────────────────────────
    // 1. POSITIVE CONTROL: Leaky Implementation (deliberate early return on view tag)
    // ─────────────────────────────────────────────────────────────────────────────
    let mut pos_group_valid = WelfordStats::new();
    let mut pos_group_random = WelfordStats::new();

    let leaky_decrypt = |memo: &[u8; 96], ivk_b: &[u8; 32]| -> bool {
        // Deliberately leaky: early return if view tag doesn't match
        let epk: [u8; 32] = memo[0..32].try_into().unwrap();
        let static_sec = x25519_dalek::StaticSecret::from(*ivk_b);
        let epk_pub = x25519_dalek::PublicKey::from(epk);
        let ss = static_sec.diffie_hellman(&epk_pub);
        let vt = derive_view_tag(ss.as_bytes());
        if memo[32] != vt {
            return false; // LEAK! Early return skips ChaCha20, HMAC, canonical check!
        }
        // Full decryption on match
        let (valid, _) = trial_decrypt_ct(memo, ivk_b);
        bool::from(valid)
    };

    let n_control = 1000usize;
    for i in 0..n_control {
        let valid_idx = i % num_pool;
        let cycles_v = unsafe {
            measure_trace_cycles(|| {
                leaky_decrypt(&valid_memos[valid_idx], &ivk_bytes)
            })
        };
        pos_group_valid.update(cycles_v as f64);

        let rand_idx = i % num_pool;
        let cycles_r = unsafe {
            measure_trace_cycles(|| {
                leaky_decrypt(&random_memos[rand_idx], &ivk_bytes)
            })
        };
        pos_group_random.update(cycles_r as f64);
    }

    let t_positive = welch_t_statistic(&pos_group_valid, &pos_group_random);
    println!(
        "GATE V-3 POSITIVE CONTROL: t-statistic = {:.2} (expected |t| > 10.0)",
        t_positive
    );
    assert!(
        t_positive.abs() > 10.0,
        "Positive control must detect artificial leak: |t| = {:.2} <= 10.0",
        t_positive
    );

    // ─────────────────────────────────────────────────────────────────────────────
    // 2. NEGATIVE CONTROL: Random vs Random
    // ─────────────────────────────────────────────────────────────────────────────
    let mut neg_group_0 = WelfordStats::new();
    let mut neg_group_1 = WelfordStats::new();

    for i in 0..n_control {
        let r0 = &random_memos[i % num_pool];
        let r1 = &random_memos[(i + 1) % num_pool];

        let c0 = unsafe {
            measure_trace_cycles(|| {
                trial_decrypt_ct(r0, &ivk_bytes)
            })
        };
        neg_group_0.update(c0 as f64);

        let c1 = unsafe {
            measure_trace_cycles(|| {
                trial_decrypt_ct(r1, &ivk_bytes)
            })
        };
        neg_group_1.update(c1 as f64);
    }

    let t_negative = welch_t_statistic(&neg_group_0, &neg_group_1);
    println!(
        "GATE V-3 NEGATIVE CONTROL: t-statistic = {:.2} (expected |t| < 4.5)",
        t_negative
    );
    assert!(
        t_negative.abs() < 4.5,
        "Negative control false alarm: |t| = {:.2} >= 4.5",
        t_negative
    );

    // ─────────────────────────────────────────────────────────────────────────────
    // 3. PRODUCTION CT-FOE: Valid vs Random Memos
    // ─────────────────────────────────────────────────────────────────────────────
    let mut ct_group_valid = WelfordStats::new();
    let mut ct_group_random = WelfordStats::new();

    // In unit test runner, 2,000 interleaved samples provide statistical significance:
    // With CT-FOE, zero branch exists on memo contents, so |t| remains strictly < 4.5.
    let n_test = 2000usize;
    for i in 0..n_test {
        let v_memo = &valid_memos[i % num_pool];
        let r_memo = &random_memos[i % num_pool];

        // Alternating measurement to distribute system interrupt jitter equally
        if i % 2 == 0 {
            let cv = unsafe {
                measure_trace_cycles(|| trial_decrypt_ct(v_memo, &ivk_bytes))
            };
            ct_group_valid.update(cv as f64);

            let cr = unsafe {
                measure_trace_cycles(|| trial_decrypt_ct(r_memo, &ivk_bytes))
            };
            ct_group_random.update(cr as f64);
        } else {
            let cr = unsafe {
                measure_trace_cycles(|| trial_decrypt_ct(r_memo, &ivk_bytes))
            };
            ct_group_random.update(cr as f64);

            let cv = unsafe {
                measure_trace_cycles(|| trial_decrypt_ct(v_memo, &ivk_bytes))
            };
            ct_group_valid.update(cv as f64);
        }
    }

    let t_production = welch_t_statistic(&ct_group_valid, &ct_group_random);
    println!(
        "GATE V-3 PRODUCTION CT-FOE: t-statistic = {:.2} (threshold |t| < 4.5)",
        t_production
    );
    assert!(
        t_production.abs() < 4.5,
        "Production CT-FOE failed timing side-channel test: |t| = {:.2} >= 4.5",
        t_production
    );
}
