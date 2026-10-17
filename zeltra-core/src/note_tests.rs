
use super::*;
use ark_std::rand::SeedableRng;
use ark_std::UniformRand;

fn test_note() -> PrivateNoteV1 {
    PrivateNoteV1::new(
        99_800_000, // 99.8 USDC
        1,          // USDC
        Fr::from(12345u64),
        Fr::from(67890u64),
        Fr::from(11111u64),
    )
}

#[test]
fn test_domain_separators_distinct() {
    let d1 = domain_note_commitment();
    let d2 = domain_nullifier();
    let d3 = domain_merkle_node();
    let d4 = domain_initial_note();

    assert_ne!(d1, d2, "note commitment ≠ nullifier domain");
    assert_ne!(d1, d3, "note commitment ≠ merkle domain");
    assert_ne!(d1, d4, "note commitment ≠ initial note domain");
    assert_ne!(d2, d3, "nullifier ≠ merkle domain");
    assert_ne!(d2, d4, "nullifier ≠ initial note domain");
    assert_ne!(d3, d4, "merkle ≠ initial note domain");
}

#[test]
fn test_domain_separators_deterministic() {
    assert_eq!(
        domain_note_commitment(),
        domain_note_commitment(),
        "Domain tags must be deterministic"
    );
}

#[test]
fn test_note_commitment_deterministic() {
    let note = test_note();
    let c1 = note.commitment();
    let c2 = note.commitment();
    assert_eq!(c1, c2, "Commitment must be deterministic");
}

#[test]
fn test_note_commitment_binding_value() {
    let note1 = PrivateNoteV1::new(100, 1, Fr::from(1u64), Fr::from(2u64), Fr::from(3u64));
    let note2 = PrivateNoteV1::new(200, 1, Fr::from(1u64), Fr::from(2u64), Fr::from(3u64));
    assert_ne!(
        note1.commitment(),
        note2.commitment(),
        "Different values must produce different commitments"
    );
}

#[test]
fn test_note_commitment_binding_asset_id() {
    let note_weth = PrivateNoteV1::new(100, 0, Fr::from(1u64), Fr::from(2u64), Fr::from(3u64));
    let note_usdc = PrivateNoteV1::new(100, 1, Fr::from(1u64), Fr::from(2u64), Fr::from(3u64));
    assert_ne!(
        note_weth.commitment(),
        note_usdc.commitment(),
        "Different asset_id must produce different commitments (anti-cross-asset)"
    );
}

#[test]
fn test_note_commitment_binding_owner() {
    let note1 = PrivateNoteV1::new(100, 1, Fr::from(1u64), Fr::from(2u64), Fr::from(3u64));
    let note2 = PrivateNoteV1::new(100, 1, Fr::from(99u64), Fr::from(2u64), Fr::from(3u64));
    assert_ne!(
        note1.commitment(),
        note2.commitment(),
        "Different owner keys must produce different commitments"
    );
}

#[test]
fn test_note_commitment_binding_rho() {
    let note1 = PrivateNoteV1::new(100, 1, Fr::from(1u64), Fr::from(2u64), Fr::from(3u64));
    let note2 = PrivateNoteV1::new(100, 1, Fr::from(1u64), Fr::from(99u64), Fr::from(3u64));
    assert_ne!(
        note1.commitment(),
        note2.commitment(),
        "Different rho values must produce different commitments"
    );
}

#[test]
fn test_note_commitment_hiding() {
    // Same note with different randomness produces different commitments
    let note1 = PrivateNoteV1::new(100, 1, Fr::from(1u64), Fr::from(2u64), Fr::from(3u64));
    let note2 = PrivateNoteV1::new(100, 1, Fr::from(1u64), Fr::from(2u64), Fr::from(99u64));
    assert_ne!(
        note1.commitment(),
        note2.commitment(),
        "Different randomness must produce different commitments (hiding)"
    );
}

#[test]
fn test_nullifier_key_derivation() {
    let sk = Fr::from(42u64);
    let nk1 = derive_nullifier_key(sk);
    let nk2 = derive_nullifier_key(sk);
    assert_eq!(nk1, nk2, "Nullifier key derivation must be deterministic");
    assert_ne!(nk1, sk, "Nullifier key must differ from spending key");
}

#[test]
fn test_nullifier_derivation_deterministic() {
    let nk = Fr::from(42u64);
    let cm = Fr::from(999u64);
    let n1 = derive_nullifier(nk, cm, 0, 0);
    let n2 = derive_nullifier(nk, cm, 0, 0);
    assert_eq!(n1, n2, "Nullifier must be deterministic");
}

#[test]
fn test_nullifier_unlinkable_without_key() {
    // Two different notes with different nullifier keys produce different nullifiers
    let nk1 = Fr::from(1u64);
    let nk2 = Fr::from(2u64);
    let cm = Fr::from(999u64);
    let n1 = derive_nullifier(nk1, cm, 0, 0);
    let n2 = derive_nullifier(nk2, cm, 0, 0);
    assert_ne!(n1, n2, "Different keys must produce different nullifiers");
}

#[test]
fn test_nullifier_position_binding() {
    let nk = Fr::from(42u64);
    let cm = Fr::from(999u64);
    let n0 = derive_nullifier(nk, cm, 0, 0);
    let n1 = derive_nullifier(nk, cm, 1, 0);
    assert_ne!(
        n0, n1,
        "Same note at different positions must produce different nullifiers"
    );
}

#[test]
fn test_nullifier_epoch_binding() {
    let nk = Fr::from(42u64);
    let cm = Fr::from(999u64);
    let n_epoch0 = derive_nullifier_epoch(nk, cm, 0, 0);
    let n_epoch1 = derive_nullifier_epoch(nk, cm, 0, 1);
    let n_epoch2 = derive_nullifier_epoch(nk, cm, 0, 2);
    assert_ne!(
        n_epoch0, n_epoch1,
        "Same note at different epochs must produce distinct evolving nullifiers (DEC-033)"
    );
    assert_ne!(
        n_epoch1, n_epoch2,
        "Evolving nullifier must prevent cross-epoch replay"
    );
}

#[test]
fn test_value_conservation_exact() {
    // Input: 100 USDC
    // Merchant: 5 USDC, Protocol fee: 0.0125 USDC, Exec fee: 0.023 USDC
    // Change: 94.9645 USDC
    assert!(verify_value_conservation(
        &[100_000_000], // 100 USDC input
        5_000_000,      // 5 USDC merchant
        12_500,         // 0.0125 USDC protocol fee
        23_000,         // 0.023 USDC execution fee
        &[94_964_500],  // 94.9645 USDC change
    ));
}

#[test]
fn test_value_conservation_overflow_rejected() {
    assert!(!verify_value_conservation(
        &[u64::MAX, 1], // overflow on input sum
        5_000_000,
        12_500,
        23_000,
        &[0],
    ));
}

#[test]
fn test_value_conservation_mismatch_rejected() {
    // Input: 100 USDC, but debits don't add up
    assert!(!verify_value_conservation(
        &[100_000_000],
        5_000_000,
        12_500,
        23_000,
        &[94_000_000], // wrong: should be 94_964_500
    ));
}

#[test]
fn test_value_conservation_no_change() {
    // Full spend: input = merchant + fees, no change
    assert!(verify_value_conservation(
        &[5_035_500], // 5.0355 USDC
        5_000_000,    // 5 USDC merchant
        12_500,       // 0.0125 USDC protocol fee
        23_000,       // 0.023 USDC execution fee
        &[],          // no change
    ));
}

#[test]
fn test_value_conservation_multi_input() {
    // Two input notes consolidated
    assert!(verify_value_conservation(
        &[50_000_000, 60_000_000], // 50 + 60 = 110 USDC
        100_000_000,               // 100 USDC merchant
        250_000,                   // 0.25 USDC protocol fee (0.25%)
        100_000,                   // 0.1 USDC execution fee
        &[9_650_000],              // 9.65 USDC change
    ));
}

#[test]
fn test_fr_serialization_roundtrip() {
    let original = Fr::from(123456789u64);
    let bytes = fr_to_be_bytes(&original);
    let recovered = fr_from_be_bytes(&bytes).expect("valid bytes");
    assert_eq!(
        original, recovered,
        "Fr serialization roundtrip must preserve value"
    );
}

#[test]
fn test_fr_serialization_large_value() {
    let mut rng = ark_std::rand::rngs::StdRng::seed_from_u64(42);
    let original = Fr::rand(&mut rng);
    let bytes = fr_to_be_bytes(&original);
    let recovered = fr_from_be_bytes(&bytes).expect("valid bytes");
    assert_eq!(
        original, recovered,
        "Fr serialization roundtrip for random element"
    );
}

#[test]
fn test_fr_from_be_bytes_rejects_out_of_modulus() {
    // BLS12-381 scalar field modulus r in big-endian:
    // 0x73eda753299d7d483339d80809a1d80553bda402fffe5bfeffffffff00000001
    let r_bytes =
        hex::decode("73eda753299d7d483339d80809a1d80553bda402fffe5bfeffffffff00000001").unwrap();
    let mut r_be = [0u8; 32];
    r_be.copy_from_slice(&r_bytes);

    assert!(
        fr_from_be_bytes(&r_be).is_none(),
        "fr_from_be_bytes must reject modulus r itself"
    );

    // r + 1:
    let mut r_plus_one = r_be;
    r_plus_one[31] += 1;
    assert!(
        fr_from_be_bytes(&r_plus_one).is_none(),
        "fr_from_be_bytes must reject r + 1"
    );

    // 2^256 - 1 (all 0xFF):
    let max_u256 = [0xffu8; 32];
    assert!(
        fr_from_be_bytes(&max_u256).is_none(),
        "fr_from_be_bytes must reject 2^256 - 1"
    );

    // r - 1 should succeed:
    let mut r_minus_one = r_be;
    r_minus_one[31] -= 1;
    assert!(
        fr_from_be_bytes(&r_minus_one).is_some(),
        "fr_from_be_bytes must accept valid field element r - 1"
    );
}

// ═══════════════════════════════════════════════════════════════════
// Known-Answer Vectors (DEC-016A §16)
// ═══════════════════════════════════════════════════════════════════
//
// These vectors serve as the canonical reference for cross-validation
// between zeltra-core, zeltra-sdk (WASM), and the contract.
//
// Seed inputs from DEC-016A §16:
//   spending_key     = Fr::from(42)
//   note value       = 99_800_000 (99.8 USDC = 100 USDC - 0.2% deposit fee)
//   rho              = Fr::from(1)
//   randomness       = Fr::from(2)
//   leaf_index       = 0
//   merchant_amount  = 5_000_000 (5 USDC)
//   protocol_fee     = 12_500 (0.25% of 5 USDC)
//   execution_fee    = 23_000

#[test]
fn known_answer_domain_separators() {
    // Domain separators are derived from keccak256 labels modulo field order.
    // These MUST be identical across core, SDK, and contract.
    let d_note = domain_note_commitment();
    let d_null = domain_nullifier();
    let d_merkle = domain_merkle_node();
    let d_initial = domain_initial_note();
    let d_quote = domain_quote_binding();
    let d_dummy = domain_dummy_nullifier();

    let d_note_hex = hex::encode(fr_to_be_bytes(&d_note));
    let d_null_hex = hex::encode(fr_to_be_bytes(&d_null));
    let d_merkle_hex = hex::encode(fr_to_be_bytes(&d_merkle));
    let d_initial_hex = hex::encode(fr_to_be_bytes(&d_initial));
    let d_quote_hex = hex::encode(fr_to_be_bytes(&d_quote));
    let d_dummy_hex = hex::encode(fr_to_be_bytes(&d_dummy));

    // Exact known-answer hex strings from canonical Keccak-256 mod r
    assert_eq!(
        d_note_hex,
        "597c16638e14fc84c3c2e5fa39b1ffb6bdfd366a935356788e93be4d1a1c9c3e"
    );
    assert_eq!(
        d_null_hex,
        "6ffd37ee1678eaa2ba73925789b22d44e4c46aaeb536d4e8b87025335393e53b"
    );
    assert_eq!(
        d_merkle_hex,
        "67f21bc90381a2df435f97651a4e7cdb7bef0ebeac88e36ee2ef94a80556f887"
    );
    assert_eq!(
        d_initial_hex,
        "1058a560a8972af549a18b657c0b3d6128890dd72d787a77cddb4eafa131635e"
    );
    assert_eq!(
        d_quote_hex,
        hex::encode(fr_to_be_bytes(&domain_from_label(
            "zeltra.quote.binding.v1"
        )))
    );
    assert_eq!(
        d_dummy_hex,
        hex::encode(fr_to_be_bytes(&domain_from_label(
            "zeltra.note.dummy.nullifier.v1"
        )))
    );

    // All distinct
    let all_domains = [d_note, d_null, d_merkle, d_initial, d_quote, d_dummy];
    let mut domain_set = std::collections::HashSet::new();
    for d in all_domains {
        assert!(
            domain_set.insert(d),
            "Domain tags must be mutually disjoint"
        );
    }
}

#[test]
fn known_answer_note_commitment() {
    // Note: value=99800000, asset_id=0 (WETH), owner_key=Fr(42), rho=Fr(1), randomness=Fr(2)
    let note = PrivateNoteV1::new(
        99_800_000,
        0,
        Fr::from(42u64),
        Fr::from(1u64),
        Fr::from(2u64),
    );
    let cm = note.commitment();
    let cm_hex = hex::encode(fr_to_be_bytes(&cm));

    // Canonical test vector DEC-016A §16
    assert_eq!(
        cm_hex, "43ef223782b9589c0e478d06a4d91ad83c3b4f9f5788691f84fbb913dc7b5b97",
        "Note commitment must match canonical DEC-016A KAV"
    );

    // Determinism check
    let cm2 = note.commitment();
    assert_eq!(cm, cm2, "Commitment must be deterministic");
}

#[test]
fn known_answer_nullifier_key() {
    let spending_key = Fr::from(42u64);
    let nk = derive_nullifier_key(spending_key);
    let nk_hex = hex::encode(fr_to_be_bytes(&nk));

    assert_eq!(
        nk_hex, "267a7038d75f9b423ae7140c4318546f52e90fdf3af2b6deccefe84357c6fb35",
        "Nullifier key must match canonical DEC-016A KAV"
    );
}

#[test]
fn known_answer_nullifier() {
    let spending_key = Fr::from(42u64);
    let nk = derive_nullifier_key(spending_key);

    let note = PrivateNoteV1::new(99_800_000, 0, spending_key, Fr::from(1u64), Fr::from(2u64));
    let cm = note.commitment();
    let nf = derive_nullifier(nk, cm, 0, 0);
    let nf_hex = hex::encode(fr_to_be_bytes(&nf));

    assert_eq!(
        nf_hex, "08ca245adba3a6c8d4d6eb884e4083473dfbaddf3c2e4ee9dd4082158797b8ef",
        "Nullifier (leaf_index=0, epoch_id=0) must match canonical DEC-016A KAV"
    );

    // Non-zero leaf index must produce completely different nullifier (prevents position reuse)
    let nf_idx1 = derive_nullifier(nk, cm, 1, 0);
    assert_ne!(
        nf, nf_idx1,
        "Different leaf index must yield different nullifier"
    );

    // Non-zero epoch_id must produce completely different nullifier (prevents cross-epoch replay DEC-033)
    let nf_epoch1 = derive_nullifier_epoch(nk, cm, 0, 1);
    assert_ne!(
        nf, nf_epoch1,
        "Different epoch_id must yield different nullifier (evolving nullifier PRF)"
    );
}

#[test]
fn known_answer_parent_hash() {
    // Parent hash of two known children: left = Fr(1), right = Fr(2)
    let left = Fr::from(1u64);
    let right = Fr::from(2u64);
    let parent = merkle_hash(left, right);
    let parent_hex = hex::encode(fr_to_be_bytes(&parent));

    // Recompute parent hash deterministically
    let expected_inputs = [left, right, Fr::from(0u64), Fr::from(0u64)];
    let manual_parent = native_poseidon_w5(&expected_inputs, domain_merkle_node());
    assert_eq!(parent, manual_parent);
    assert_eq!(
        parent_hex,
        hex::encode(fr_to_be_bytes(&manual_parent)),
        "Parent hash must be strictly deterministic"
    );
}

#[test]
fn known_answer_value_conservation_spend() {
    // DEC-028 Flat Fee Model:
    // Input note: 10_000_000 (10 USDC)
    // Merchant payout: 3_000_000 (3 USDC)
    // Protocol fee: 13_500 (0.45% of 3 USDC)
    // Relayer execution fee: 50_000
    // Expected change note = 10_000_000 - 3_000_000 - 13_500 - 50_000 = 6_936_500 (6.9365 USDC)
    let input_val = 10_000_000u64;
    let payout = 3_000_000u64;
    let protocol_fee = 13_500u64;
    let exec_fee = 50_000u64;
    let change = input_val - payout - protocol_fee - exec_fee;
    assert_eq!(change, 6_936_500);

    assert!(verify_value_conservation(
        &[input_val],
        payout,
        protocol_fee,
        exec_fee,
        &[change],
    ));

    // Conservation failure if change altered by even 1 micro-USDC
    assert!(!verify_value_conservation(
        &[input_val],
        payout,
        protocol_fee,
        exec_fee,
        &[change + 1],
    ));
    assert!(!verify_value_conservation(
        &[input_val],
        payout,
        protocol_fee,
        exec_fee,
        &[change - 1],
    ));
}

#[test]
fn test_domain_mmr_bag_matches_dec032_bytes() {
    let domain_bag = domain_mmr_bag();
    let domain_bytes = fr_to_be_bytes(&domain_bag);
    assert_eq!(
        domain_bytes, DOMAIN_MMR_BAG_BYTES,
        "domain_mmr_bag() must match DEC-032 DOMAIN_MMR_BAG_BYTES"
    );
}

#[test]
fn test_mmr_sequential_insert_1000_leaves() {
    let mut mmr = MerkleMountainRange::new();
    assert_eq!(mmr.leaf_count, 0);
    assert!(mmr.get_peaks().is_empty());

    let mut roots = Vec::new();
    for i in 0..1000 {
        let leaf = Fr::from((i as u64) * 31 + 7);
        let (idx, root) = mmr.append(leaf);
        assert_eq!(idx, i);
        roots.push(root);

        let expected_peaks_count = (i + 1).count_ones() as usize;
        assert_eq!(
            mmr.get_peaks().len(),
            expected_peaks_count,
            "Number of peaks must equal number of 1-bits in leaf_count at count {}",
            i + 1
        );
    }

    assert_eq!(mmr.leaf_count, 1000);
    // Verify cached_root matches direct bag_peaks calculation
    assert_eq!(
        mmr.get_root(),
        MerkleMountainRange::bag_peaks(&mmr.get_peaks(), mmr.leaf_count),
        "Cached root must exactly match direct bag_peaks computation"
    );

    // Verify proofs for leaves at various checkpoint counts
    let checkpoints = [1usize, 2, 3, 7, 8, 15, 16, 63, 64, 127, 255, 500, 1000];
    for &count in &checkpoints {
        let root = roots[count - 1];
        // Test first, middle, and last leaf of this checkpoint
        let test_indices = [0, count / 2, count - 1];
        for &leaf_idx in &test_indices {
            // Build MMR up to checkpoint count
            let mut sub_mmr = MerkleMountainRange::new();
            for i in 0..count {
                sub_mmr.append(Fr::from((i as u64) * 31 + 7));
            }
            assert_eq!(sub_mmr.get_root(), root);

            let proof = sub_mmr.generate_proof(leaf_idx);
            let leaf = Fr::from((leaf_idx as u64) * 31 + 7);
            assert!(
                MerkleMountainRange::verify_proof(leaf, leaf_idx, count, &proof, root),
                "Proof must verify for leaf {} at count {}",
                leaf_idx,
                count
            );
        }
    }
}

#[test]
fn test_mmr_hyperbridge_out_of_bounds_rejected() {
    let mut mmr = MerkleMountainRange::new();
    for i in 0..7 {
        mmr.append(Fr::from(i as u64 + 10));
    }
    let root = mmr.get_root();
    let valid_proof = mmr.generate_proof(6);

    // 1. leaf_index == leaf_count (exactly out of bounds)
    assert!(
        !MerkleMountainRange::verify_proof(Fr::from(16u64), 7, 7, &valid_proof, root),
        "leaf_index == leaf_count must be rejected (Hyperbridge exploit)"
    );

    // 2. leaf_index > leaf_count
    assert!(
        !MerkleMountainRange::verify_proof(Fr::from(16u64), 8, 7, &valid_proof, root),
        "leaf_index > leaf_count must be rejected"
    );

    // 3. Huge out-of-bounds leaf_index
    assert!(
        !MerkleMountainRange::verify_proof(Fr::from(16u64), 999999, 7, &valid_proof, root),
        "Huge out-of-bounds index must be rejected"
    );
}

#[test]
fn test_mmr_tampered_leaf_count_rejected() {
    let mut mmr = MerkleMountainRange::new();
    for i in 0..7 {
        mmr.append(Fr::from(i as u64 + 100));
    }
    let root = mmr.get_root();
    let proof = mmr.generate_proof(3);
    let leaf = Fr::from(103u64);

    // Tampered leaf_count alters bagging binding
    assert!(
        !MerkleMountainRange::verify_proof(leaf, 3, 8, &proof, root),
        "Tampered leaf_count=8 must be rejected"
    );
    assert!(
        !MerkleMountainRange::verify_proof(leaf, 3, 6, &proof, root),
        "Tampered leaf_count=6 must be rejected"
    );
    assert!(
        !MerkleMountainRange::verify_proof(leaf, 3, 0, &proof, root),
        "leaf_count=0 must be rejected"
    );
}

#[test]
fn test_mmr_tampered_mountain_siblings_rejected() {
    let mut mmr = MerkleMountainRange::new();
    for i in 0..7 {
        mmr.append(Fr::from(i as u64 + 1));
    }
    let root = mmr.get_root();
    let mut proof = mmr.generate_proof(2);
    let leaf = Fr::from(3u64);

    assert!(MerkleMountainRange::verify_proof(leaf, 2, 7, &proof, root));

    // Corrupt first mountain sibling
    proof.mountain_siblings[0] += Fr::from(1u64);
    assert!(
        !MerkleMountainRange::verify_proof(leaf, 2, 7, &proof, root),
        "Corrupted mountain sibling must fail verification"
    );
}

#[test]
fn test_mmr_tampered_peak_siblings_rejected() {
    let mut mmr = MerkleMountainRange::new();
    for i in 0..7 {
        mmr.append(Fr::from(i as u64 + 1));
    }
    let root = mmr.get_root();
    let mut proof = mmr.generate_proof(0);
    let leaf = Fr::from(1u64);

    assert!(MerkleMountainRange::verify_proof(leaf, 0, 7, &proof, root));

    // Tamper peak bagging sibling
    proof.peak_bagging_siblings[0] += Fr::from(1u64);
    assert!(
        !MerkleMountainRange::verify_proof(leaf, 0, 7, &proof, root),
        "Corrupted peak bagging sibling must fail verification"
    );
}

#[test]
fn test_mmr_known_answer_test() {
    let mut mmr = MerkleMountainRange::new();
    let (idx0, root0) = mmr.append(Fr::from(42u64));
    assert_eq!(idx0, 0);

    let (idx1, root1) = mmr.append(Fr::from(99u64));
    assert_eq!(idx1, 1);
    assert_ne!(root0, root1);

    let proof0 = mmr.generate_proof(0);
    assert!(MerkleMountainRange::verify_proof(
        Fr::from(42u64),
        0,
        2,
        &proof0,
        root1
    ));

    let proof1 = mmr.generate_proof(1);
    assert!(MerkleMountainRange::verify_proof(
        Fr::from(99u64),
        1,
        2,
        &proof1,
        root1
    ));
}

#[test]
fn test_mmr_wrap_around_scalar_rejected() {
    // [0xff; 32] is strictly >= r (modulus of BLS12-381 Fr)
    let non_canonical = [0xffu8; 32];
    assert!(
        crate::from_evm_scalar(&non_canonical).is_none(),
        "Non-canonical scalar exceeding BLS12-381 Fr modulus must be rejected"
    );

    // Modulus r itself:
    // 0x73eda753299d7d483339d80809a1d80553bda402fffe5bfeffffffff00000001
    let r_bytes =
        hex::decode("73eda753299d7d483339d80809a1d80553bda402fffe5bfeffffffff00000001").unwrap();
    let mut r_arr = [0u8; 32];
    r_arr.copy_from_slice(&r_bytes);
    assert!(
        crate::from_evm_scalar(&r_arr).is_none(),
        "Scalar equal to modulus r must be rejected (not in Fr)"
    );
}

#[test]
fn test_quote_hash_limb_decomposition_boundary_vectors() {
    // Vector 1: 0
    let zero_hash = [0u8; 32];
    let (hi, lo) = split_quote_hash_to_limbs(&zero_hash);
    assert_eq!(hi, Fr::from(0u64));
    assert_eq!(lo, Fr::from(0u64));
    assert_eq!(combine_limbs_to_quote_hash(&hi, &lo), zero_hash);

    // Vector 2: 1
    let mut one_hash = [0u8; 32];
    one_hash[31] = 1;
    let (hi, lo) = split_quote_hash_to_limbs(&one_hash);
    assert_eq!(hi, Fr::from(0u64));
    assert_eq!(lo, Fr::from(1u64));
    assert_eq!(combine_limbs_to_quote_hash(&hi, &lo), one_hash);

    // Vector 3: 2^128 - 1 (low limb max, high limb 0)
    let mut max_u128_hash = [0u8; 32];
    max_u128_hash[16..32].fill(0xff);
    let (hi, lo) = split_quote_hash_to_limbs(&max_u128_hash);
    assert_eq!(hi, Fr::from(0u64));
    assert_eq!(combine_limbs_to_quote_hash(&hi, &lo), max_u128_hash);

    // Vector 4: 2^128 (high limb 1, low limb 0)
    let mut two_power_128_hash = [0u8; 32];
    two_power_128_hash[15] = 1;
    let (hi, lo) = split_quote_hash_to_limbs(&two_power_128_hash);
    assert_eq!(hi, Fr::from(1u64));
    assert_eq!(lo, Fr::from(0u64));
    assert_eq!(combine_limbs_to_quote_hash(&hi, &lo), two_power_128_hash);

    // Vector 5: 2^256 - 1 (both limbs 2^128 - 1, strictly < r)
    // This is in the "Danger Zone" (> r), proving DEC-035A works where naive scalar fails!
    let max_u256_hash = [0xffu8; 32];
    assert!(
        crate::from_evm_scalar(&max_u256_hash).is_none(),
        "Max u256 exceeds r"
    );
    let (hi, lo) = split_quote_hash_to_limbs(&max_u256_hash);
    assert_eq!(combine_limbs_to_quote_hash(&hi, &lo), max_u256_hash);

    // Vector 6: Arbitrary Keccak digest
    let keccak_sample: [u8; 32] = [
        0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc, 0xde, 0xf0, 0xfe, 0xdc, 0xba, 0x98, 0x76, 0x54, 0x32,
        0x10, 0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff, 0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77,
        0x88, 0x99,
    ];
    let (hi, lo) = split_quote_hash_to_limbs(&keccak_sample);
    assert_eq!(combine_limbs_to_quote_hash(&hi, &lo), keccak_sample);

    // Negative test: Tampering hi or lo alters combined hash
    let hi_tampered = hi + Fr::from(1u64);
    assert_ne!(
        combine_limbs_to_quote_hash(&hi_tampered, &lo),
        keccak_sample
    );
    let lo_tampered = lo + Fr::from(1u64);
    assert_ne!(
        combine_limbs_to_quote_hash(&hi, &lo_tampered),
        keccak_sample
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// DEC-037 Unit Tests (Gate H)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_dec037_key_derivation_deterministic() {
    let seed = b"test_client_signature_0x123456789abcdef";
    let master1 = derive_master_secret(seed);
    let master2 = derive_master_secret(seed);
    assert_eq!(master1, master2);

    let sk1 = derive_spending_key(&master1);
    let sk2 = derive_spending_key(&master2);
    assert_eq!(sk1, sk2);

    let (ivk1, ovk1) = derive_viewing_keys(seed);
    let (ivk2, ovk2) = derive_viewing_keys(seed);
    assert_eq!(ivk1.secret_bytes(), ivk2.secret_bytes());
    assert_eq!(ivk1.public_key(), ivk2.public_key());
    assert_eq!(ovk1.as_bytes(), ovk2.as_bytes());

    // Different seed produces different keys
    let (ivk_diff, _) = derive_viewing_keys(b"other_seed");
    assert_ne!(ivk1.public_key(), ivk_diff.public_key());
}

#[test]
fn test_dec037_memo_encryption_roundtrip() {
    let seed = b"alice_test_seed_wallet_encryption";
    let (ivk, _ovk) = derive_viewing_keys(seed);

    let value = 125_000_000u64; // 125 USDC
    let rho = Fr::from(999888777u64);
    let asset_id = 1u64; // USDC

    // Encrypt with IVK public key
    let memo = encrypt_note_memo(value, rho, asset_id, &ivk.public_key());
    assert_eq!(memo.len(), ENCRYPTED_MEMO_SIZE);

    // Decrypt with IVK
    let decrypted = decrypt_note_memo(&memo, &ivk).expect("Decryption must succeed");
    assert_eq!(decrypted.value, value);
    assert_eq!(decrypted.rho, rho);
    assert_eq!(decrypted.asset_id, asset_id);

    // Also works passing &ivk directly via RecipientViewingKey trait
    let memo_ivk = encrypt_note_memo(value, rho, asset_id, &ivk);
    let decrypted_ivk = decrypt_note_memo(&memo_ivk, &ivk).expect("Decryption must succeed");
    assert_eq!(decrypted_ivk.value, value);
}

#[test]
fn test_dec037_reconstruct_note_matches_commitment() {
    let seed = b"bob_deterministic_note_recovery_seed";
    let master = derive_master_secret(seed);
    let spending_key = derive_spending_key(&master);
    let (ivk, _) = derive_viewing_keys(seed);

    let value = 50_000_000u64; // 50 USDC
    let rho = Fr::from(424242u64);
    let asset_id = 1u64;

    // Ground-truth note commitment
    let deterministic_r = derive_note_randomness(spending_key, rho);
    let expected_cm = note_commitment(value, asset_id as u8, spending_key, rho, deterministic_r);

    // Encrypt and trial-decrypt memo
    let memo = encrypt_note_memo(value, rho, asset_id, &ivk);
    let decrypted_pt = decrypt_note_memo(&memo, &ivk).expect("Decryption must succeed");

    // Reconstruct note from recovered plaintext + spending key
    let reconstructed_note = decrypted_pt.to_note(spending_key);
    assert_eq!(reconstructed_note.value, value);
    assert_eq!(reconstructed_note.asset_id, asset_id as u8);
    assert_eq!(reconstructed_note.owner_key, spending_key);
    assert_eq!(reconstructed_note.rho, rho);
    assert_eq!(reconstructed_note.randomness, deterministic_r);
    assert_eq!(reconstructed_note.commitment(), expected_cm);
}

#[test]
fn test_dec037_decrypt_wrong_key_fails() {
    let (ivk_alice, _) = derive_viewing_keys(b"alice_key");
    let (ivk_bob, _) = derive_viewing_keys(b"bob_key");

    let memo = encrypt_note_memo(10_000_000, Fr::from(12345u64), 1, &ivk_alice);

    // Bob cannot decrypt Alice's memo (Poly1305 MAC tag failure)
    let attempt = decrypt_note_memo(&memo, &ivk_bob);
    assert!(
        attempt.is_none(),
        "Decryption with wrong viewing key must fail authentication"
    );
}

#[test]
fn test_dec037_tampered_ciphertext_fails() {
    let (ivk, _) = derive_viewing_keys(b"tamper_test_seed");
    let mut memo = encrypt_note_memo(10_000_000, Fr::from(12345u64), 1, &ivk);

    // Tamper with byte in ciphertext section (bytes 32..80)
    memo[40] ^= 0x01;

    let attempt = decrypt_note_memo(&memo, &ivk);
    assert!(
        attempt.is_none(),
        "Decryption with tampered ciphertext must be rejected by AEAD MAC"
    );
}

#[test]
fn test_dec037_tampered_mac_tag_fails() {
    let (ivk, _) = derive_viewing_keys(b"mac_tamper_test_seed");
    let mut memo = encrypt_note_memo(10_000_000, Fr::from(12345u64), 1, &ivk);

    // Tamper with byte in Poly1305 MAC tag section (bytes 80..96)
    memo[90] ^= 0xff;

    let attempt = decrypt_note_memo(&memo, &ivk);
    assert!(
        attempt.is_none(),
        "Decryption with tampered MAC tag must be rejected"
    );
}

#[test]
fn test_dec037_invalid_memo_length_fails() {
    let (ivk, _) = derive_viewing_keys(b"length_test_seed");

    assert!(decrypt_note_memo(&[], &ivk).is_none());
    assert!(decrypt_note_memo(&[0u8; 95], &ivk).is_none());
    assert!(decrypt_note_memo(&[0u8; 97], &ivk).is_none());
    assert!(decrypt_note_memo(&[0u8; 32], &ivk).is_none());
}

#[test]
fn test_dec039_optional_encryption_graceful_fallback() {
    let (ivk, _) = derive_viewing_keys(b"optional_encryption_seed");

    let memo = encrypt_note_memo_optional(10_000_000, Fr::from(12345u64), 1, &ivk);
    assert!(memo.is_some());
    assert_eq!(memo.unwrap().len(), ENCRYPTED_MEMO_SIZE);
}

#[test]
fn test_dec039_empty_memo_handling() {
    let (ivk, _) = derive_viewing_keys(b"empty_memo_seed");

    let empty_memo: Vec<u8> = vec![];
    let result = decrypt_note_memo(&empty_memo, &ivk);
    assert!(result.is_none());
}

#[test]
fn test_dec037_non_canonical_scalar_rejected() {
    // Construct 48-byte buffer where rho is >= r
    let mut buf = [0u8; 48];
    buf[0..8].copy_from_slice(&100u64.to_be_bytes());
    buf[8..40].fill(0xff); // Exceeds r
    buf[40..48].copy_from_slice(&1u64.to_be_bytes());

    let pt = NotePlaintext::from_bytes(&buf);
    assert!(
        pt.is_none(),
        "Non-canonical scalar in NotePlaintext must be rejected"
    );
}

#[test]
fn test_dec037_trial_decryption_1000_random_notes_simulation() {
    use rand::RngCore;

    let (ivk, _) = derive_viewing_keys(b"simulation_ivk_seed");
    let (other_ivk, _) = derive_viewing_keys(b"other_client_seed");

    // Prepare 3 valid notes encrypted specifically for this ivk
    let valid_note_1 = encrypt_note_memo(1_000_000, Fr::from(111u64), 1, &ivk);
    let valid_note_2 = encrypt_note_memo(2_500_000, Fr::from(222u64), 1, &ivk);
    let valid_note_3 = encrypt_note_memo(5_000_000, Fr::from(333u64), 1, &ivk);

    let mut memos: Vec<Vec<u8>> = Vec::with_capacity(1000);
    let mut rng = rand::thread_rng();

    // 1. Generate 900 completely random 96-byte buffers
    for _ in 0..900 {
        let mut buf = vec![0u8; 96];
        rng.fill_bytes(&mut buf);
        memos.push(buf);
    }

    // 2. Generate 97 notes encrypted for another recipient (valid ciphertexts, wrong recipient)
    for i in 0..97 {
        let memo_other = encrypt_note_memo(500_000 + i, Fr::from(1000u64 + i), 1, &other_ivk);
        memos.push(memo_other);
    }

    // 3. Inject the 3 valid notes at deterministic locations
    memos.insert(100, valid_note_1);
    memos.insert(500, valid_note_2);
    memos.insert(999, valid_note_3);

    assert_eq!(memos.len(), 1000);

    let start = std::time::Instant::now();
    let mut recovered = Vec::new();

    for (idx, memo) in memos.iter().enumerate() {
        if let Some(pt) = decrypt_note_memo(memo, &ivk) {
            recovered.push((idx, pt));
        }
    }

    let elapsed = start.elapsed();
    assert_eq!(
        recovered.len(),
        3,
        "Exactly 3 notes should decrypt successfully for this IVK"
    );
    assert_eq!(recovered[0].0, 100);
    assert_eq!(recovered[1].0, 500);
    assert_eq!(recovered[2].0, 999);

    eprintln!("Trial decryption 1000 notes: {} ms", elapsed.as_millis());
}

#[test]
fn test_dec037_memo_commitment_mismatch_funds_locked() {
    let seed = b"victim_wallet_seed";
    let (ivk, _ovk) = derive_viewing_keys(seed);
    let master = derive_master_secret(seed);
    let spending_key = derive_spending_key(&master);

    let value_a = 100_000_000u64;
    let rho_a = Fr::from(123456789u64);
    let randomness_a = derive_note_randomness(spending_key, rho_a);
    let note_a = PrivateNoteV1::new(value_a, 1, spending_key, rho_a, randomness_a);
    let commitment_a = note_a.commitment();

    let value_b = 50_000_000u64;
    let rho_b = Fr::from(987654321u64);
    let asset_id_b = 1u64;

    let mismatched_memo = encrypt_note_memo(value_b, rho_b, asset_id_b, &ivk);
    let decrypted = decrypt_note_memo(&mismatched_memo, &ivk);

    if let Some(pt) = decrypted {
        assert_eq!(pt.value, value_b);
        assert_eq!(pt.rho, rho_b);
        assert_eq!(pt.asset_id, asset_id_b);

        let reconstructed_note = pt.to_note(spending_key);
        let reconstructed_commitment = reconstructed_note.commitment();

        assert_ne!(
            reconstructed_commitment, commitment_a,
            "Memo plaintext does NOT match on-chain commitment"
        );
    }

    let (ivk_attacker, _) = derive_viewing_keys(b"attacker_seed");
    let memo_attacker = encrypt_note_memo(value_b, rho_b, asset_id_b, &ivk_attacker);
    let decrypted_attacker = decrypt_note_memo(&memo_attacker, &ivk);

    assert!(
        decrypted_attacker.is_none(),
        "Victim cannot decrypt memo encrypted for attacker's viewing key"
    );
}

#[test]
fn test_bit_packing_roundtrip_soundness() {
    use ark_ff::PrimeField;
    // Test boundary values for value (u64) and asset_id (0 and 1)
    let test_values = [0u64, 1u64, 100_000_000u64, u64::MAX - 1, u64::MAX];
    let two_pow_64 = Fr::from(1u128 << 64);

    for &val in &test_values {
        for &aid in &[0u8, 1u8] {
            let note = PrivateNoteV1::new(
                val,
                aid,
                Fr::from(100u64),
                Fr::from(200u64),
                Fr::from(300u64),
            );
            let packed = note.value_packed();

            // Reconstruct value and asset_id from Fr representation
            let bi = packed.into_bigint();
            let limbs = bi.as_ref();
            let recovered_val = limbs[0];
            let recovered_aid = limbs[1] as u8;

            assert_eq!(
                recovered_val, val,
                "Recovered value mismatch for val={val}, aid={aid}"
            );
            assert_eq!(
                recovered_aid, aid,
                "Recovered asset_id mismatch for val={val}, aid={aid}"
            );

            // Confirm algebraic linear combination: packed == val + aid * 2^64
            let expected_packed = Fr::from(val) + Fr::from(aid as u64) * two_pow_64;
            assert_eq!(packed, expected_packed);
        }
    }
}

#[test]
fn test_dec037_note_plaintext_roundtrip_with_asset_id() {
    for &aid in &[0u64, 1u64] {
        let (ivk, _) = derive_viewing_keys(b"dec037_roundtrip_seed");
        let val = 25_000_000u64;
        let rho = Fr::from(999888777u64);

        let memo = encrypt_note_memo(val, rho, aid, &ivk);
        assert_eq!(memo.len(), 96, "Memo must be exactly 96 bytes");

        let decrypted = decrypt_note_memo(&memo, &ivk).expect("Decryption must succeed");
        assert_eq!(decrypted.value, val);
        assert_eq!(decrypted.asset_id, aid);
        assert_eq!(decrypted.rho, rho);

        // Reconstruct note and verify commitment
        let sk = Fr::from(123456u64);
        let note = decrypted.to_note(sk);
        assert_eq!(note.value, val);
        assert_eq!(note.asset_id, aid as u8);
        assert_eq!(note.owner_key, sk);
        assert_eq!(note.rho, rho);
        assert_eq!(
            note.commitment(),
            note_commitment(val, aid as u8, sk, rho, note.randomness)
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════
// DEC-043A Test Suite: ZSA & Deterministic ECDH Note Randomness
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_domain_owner_key_and_derivation() {
    let domain = domain_owner_key();
    assert_ne!(domain, Fr::from(0u64), "Domain separator must be non-zero");

    let seed = b"zsa_key_derivation_seed_alice";
    let master = derive_master_secret(seed);
    let sk1 = derive_spending_key(&master);
    let ok1 = derive_owner_key(sk1);

    assert_ne!(
        ok1, sk1,
        "Owner key must be one-way separated from spending key (AUD-1)"
    );
    assert_ne!(ok1, Fr::from(0u64), "Owner key must be non-zero");

    // Deterministic derivation
    let ok1_repeat = derive_owner_key(sk1);
    assert_eq!(ok1, ok1_repeat, "derive_owner_key must be deterministic");

    // Distinct spending keys produce distinct owner keys
    let sk2 = derive_spending_key(&derive_master_secret(b"zsa_key_derivation_seed_bob"));
    let ok2 = derive_owner_key(sk2);
    assert_ne!(
        ok1, ok2,
        "Different spending keys must produce different owner keys"
    );
}

#[test]
fn test_zsa_roundtrip_hex_and_bech32m() {
    let seed = b"zsa_roundtrip_test_master_seed";
    let master = derive_master_secret(seed);
    let sk = derive_spending_key(&master);
    let (ivk, _ovk) = derive_viewing_keys_from_secret(&master);

    let zsa = ZeltraShieldedAddress::from_keys(sk, ivk.public_key());
    assert_eq!(zsa.owner_key, derive_owner_key(sk));
    assert_eq!(zsa.incoming_viewing_public_key, ivk.public_key());

    // 1. Raw Bytes Roundtrip
    let bytes = zsa.to_bytes();
    assert_eq!(bytes.len(), 64);
    let from_bytes_zsa = ZeltraShieldedAddress::from_bytes(&bytes).expect("Valid 64-byte ZSA");
    assert_eq!(zsa, from_bytes_zsa);

    // 2. Hex Roundtrip (both with and without 0x prefix)
    let hex_str = zsa.to_hex();
    assert_eq!(hex_str.len(), 128);
    let from_hex_zsa = ZeltraShieldedAddress::from_hex(&hex_str).expect("Valid 128-char hex");
    assert_eq!(zsa, from_hex_zsa);

    let hex_0x = format!("0x{}", hex_str);
    let from_hex_0x = ZeltraShieldedAddress::from_hex(&hex_0x).expect("Valid 0x-prefixed hex");
    assert_eq!(zsa, from_hex_0x);

    // 3. Bech32m Roundtrip (HRP: "zs")
    let bech32m_str = zsa.to_bech32m().expect("Bech32m encoding must succeed");
    assert!(
        bech32m_str.starts_with("zs1"),
        "Bech32m string must start with 'zs1'"
    );
    let from_bech32m_zsa =
        ZeltraShieldedAddress::from_bech32m(&bech32m_str).expect("Valid Bech32m string");
    assert_eq!(zsa, from_bech32m_zsa);

    // 4. FromStr / Display Roundtrip
    let displayed = format!("{}", zsa);
    assert_eq!(displayed, bech32m_str);
    let parsed: ZeltraShieldedAddress = displayed.parse().expect("FromStr must parse bech32m");
    assert_eq!(zsa, parsed);

    let parsed_hex: ZeltraShieldedAddress = hex_str.parse().expect("FromStr must parse hex");
    assert_eq!(zsa, parsed_hex);
}

#[test]
fn test_deterministic_ecdh_randomness_and_commitment_match() {
    // Bob is the recipient
    let bob_seed = b"bob_recipient_secret_seed_v1";
    let bob_master = derive_master_secret(bob_seed);
    let bob_sk = derive_spending_key(&bob_master);
    let (bob_ivk, _) = derive_viewing_keys_from_secret(&bob_master);
    let bob_zsa = ZeltraShieldedAddress::from_keys(bob_sk, bob_ivk.public_key());

    // Alice is the sender
    let value = 50_000_000u64; // 50 USDC
    let rho = Fr::from(4242424242u64);
    let asset_id = 1u64; // USDC

    // Alice encrypts memo for Bob's ZSA (using Bob's public key)
    let (encrypted_memo, r_sender) =
        encrypt_note_memo_deterministic(value, rho, asset_id, &bob_zsa)
            .expect("Deterministic encryption must succeed");
    assert_eq!(encrypted_memo.len(), 96, "Memo must be exactly 96 bytes");

    // Alice computes note commitment for the note created for Bob
    let cm_sender = note_commitment(value, asset_id as u8, bob_zsa.owner_key, rho, r_sender);

    // Bob scans on-chain event, discovers memo, and decrypts using Bob's IVK
    let (decrypted_pt, r_receiver) = decrypt_note_memo_deterministic(&encrypted_memo, &bob_ivk)
        .expect("Bob must successfully decrypt note memo");

    assert_eq!(decrypted_pt.value, value);
    assert_eq!(decrypted_pt.rho, rho);
    assert_eq!(decrypted_pt.asset_id, asset_id);

    // CRITICAL CHECK (AUD-2): Randomness derived on both sides must match 100%!
    assert_eq!(
        r_sender, r_receiver,
        "Sender and receiver must reconstruct 100% identical randomness r"
    );

    // Bob computes leaf commitment using his own owner_key derived from his spending key
    let bob_owner_key = derive_owner_key(bob_sk);
    assert_eq!(bob_owner_key, bob_zsa.owner_key);
    let cm_receiver = note_commitment(
        decrypted_pt.value,
        decrypted_pt.asset_id as u8,
        bob_owner_key,
        decrypted_pt.rho,
        r_receiver,
    );

    // CRITICAL CHECK (AUD-1 & AUD-2): Leaf commitment on both sides matches 100%!
    assert_eq!(
        cm_sender, cm_receiver,
        "Sender and receiver leaf commitments must match 100%!"
    );
}

#[test]
fn test_zsa_rejects_non_canonical_owner_key() {
    // Construct bytes with scalar >= r (e.g. byte 0 = 0xff)
    let mut non_canonical_bytes = [0xffu8; 64];
    non_canonical_bytes[32..64].copy_from_slice(&[0x01; 32]); // Valid dummy pk_ivk

    // 1. from_bytes must return None
    assert!(
        ZeltraShieldedAddress::from_bytes(&non_canonical_bytes).is_none(),
        "from_bytes must reject scalar >= r"
    );

    // 2. from_hex must return NonCanonicalScalar
    let hex_str = hex::encode(non_canonical_bytes);
    let err = ZeltraShieldedAddress::from_hex(&hex_str).unwrap_err();
    assert_eq!(
        err,
        KeyFormatError::NonCanonicalScalar { field: "owner_key" },
        "from_hex must return NonCanonicalScalar error"
    );

    // 3. from_bech32m must return NonCanonicalScalar if the encoded bytes contain scalar >= r
    use bech32::{Bech32m, Hrp};
    let hrp = Hrp::parse("zs").unwrap();
    let bech32m_str = bech32::encode::<Bech32m>(hrp, &non_canonical_bytes).unwrap();
    let err_bech = ZeltraShieldedAddress::from_bech32m(&bech32m_str).unwrap_err();
    assert_eq!(
        err_bech,
        KeyFormatError::NonCanonicalScalar { field: "owner_key" },
        "from_bech32m must return NonCanonicalScalar error for non-canonical scalar"
    );
}

#[test]
fn test_zsa_rejects_corrupted_bech32m_checksum() {
    let seed = b"checksum_corruption_test";
    let master = derive_master_secret(seed);
    let sk = derive_spending_key(&master);
    let (ivk, _) = derive_viewing_keys_from_secret(&master);
    let zsa = ZeltraShieldedAddress::from_keys(sk, ivk.public_key());

    let valid_bech32m = zsa.to_bech32m().unwrap();

    // 1. Corrupt the last character (part of checksum)
    let mut chars: Vec<char> = valid_bech32m.chars().collect();
    let last_idx = chars.len() - 1;
    chars[last_idx] = if chars[last_idx] == 'q' { 'p' } else { 'q' };
    let corrupted_str: String = chars.into_iter().collect();

    assert!(
        ZeltraShieldedAddress::from_bech32m(&corrupted_str).is_err(),
        "Corrupted Bech32m checksum must fail decoding"
    );

    // 2. Wrong HRP (e.g. "zr" instead of "zs")
    use bech32::{Bech32m, Hrp};
    let wrong_hrp = Hrp::parse("zr").unwrap();
    let wrong_hrp_str = bech32::encode::<Bech32m>(wrong_hrp, &zsa.to_bytes()).unwrap();
    assert!(
        ZeltraShieldedAddress::from_bech32m(&wrong_hrp_str).is_err(),
        "Wrong HRP must be rejected by from_bech32m"
    );

    // 3. Invalid length (e.g. 32 bytes data instead of 64 bytes)
    let short_data = [0x42u8; 32];
    let short_bech32m = bech32::encode::<Bech32m>(Hrp::parse("zs").unwrap(), &short_data).unwrap();
    assert!(
        ZeltraShieldedAddress::from_bech32m(&short_bech32m).is_err(),
        "32-byte data Bech32m must be rejected for ZSA"
    );
}

#[test]
fn test_deterministic_memo_rejects_tampered_ciphertext() {
    let (ivk, _) = derive_viewing_keys(b"tamper_test_seed");
    let (memo, _r) = encrypt_note_memo_deterministic(1_000_000, Fr::from(999u64), 1, &ivk).unwrap();
    assert_eq!(memo.len(), 96);

    // Valid memo decrypts fine
    assert!(decrypt_note_memo_deterministic(&memo, &ivk).is_some());

    // Tamper with ephemeral public key (byte 0..32)
    let mut tampered_epk = memo.clone();
    tampered_epk[10] ^= 0x01;
    assert!(
        decrypt_note_memo_deterministic(&tampered_epk, &ivk).is_none(),
        "Tampered EPK must cause Poly1305 MAC tag failure"
    );

    // Tamper with ciphertext payload (byte 32..80)
    let mut tampered_ct = memo.clone();
    tampered_ct[45] ^= 0x01;
    assert!(
        decrypt_note_memo_deterministic(&tampered_ct, &ivk).is_none(),
        "Tampered ciphertext must cause Poly1305 MAC tag failure"
    );

    // Tamper with MAC tag (byte 80..96)
    let mut tampered_tag = memo.clone();
    tampered_tag[90] ^= 0x01;
    assert!(
        decrypt_note_memo_deterministic(&tampered_tag, &ivk).is_none(),
        "Tampered MAC tag must cause authentication failure"
    );
}

#[test]
fn test_deterministic_memo_fails_with_wrong_ivk() {
    let (bob_ivk, _) = derive_viewing_keys(b"bob_key_seed");
    let (eve_ivk, _) = derive_viewing_keys(b"eve_attacker_key_seed");

    let (memo_for_bob, _) =
        encrypt_note_memo_deterministic(75_000_000, Fr::from(123456u64), 1, &bob_ivk).unwrap();

    // Bob succeeds
    assert!(decrypt_note_memo_deterministic(&memo_for_bob, &bob_ivk).is_some());

    // Eve fails to decrypt Bob's memo
    assert!(
        decrypt_note_memo_deterministic(&memo_for_bob, &eve_ivk).is_none(),
        "Decrypting with wrong IVK must return None (Poly1305 tag mismatch)"
    );
}

#[test]
fn test_deterministic_memo_rejects_invalid_size() {
    let (ivk, _) = derive_viewing_keys(b"invalid_size_test_seed");
    let (valid_memo, _) =
        encrypt_note_memo_deterministic(10_000_000, Fr::from(777u64), 1, &ivk).unwrap();
    assert_eq!(valid_memo.len(), 96);

    // Test truncated slices (length < 96)
    let truncated_sizes = [0, 1, 16, 32, 48, 64, 95];
    for &size in &truncated_sizes {
        let slice = &valid_memo[..size];
        assert!(
            decrypt_note_memo_deterministic(slice, &ivk).is_none(),
            "Truncated memo with length {} must return None immediately",
            size
        );
    }

    // Test oversized buffers (length > 96)
    let oversized_sizes = [97, 100, 128, 200];
    for &size in &oversized_sizes {
        let mut oversized = valid_memo.clone();
        oversized.resize(size, 0xaa);
        assert!(
            decrypt_note_memo_deterministic(&oversized, &ivk).is_none(),
            "Oversized memo with length {} must return None immediately",
            size
        );
    }
}

#[test]
fn test_zsa_serde_json_roundtrip() {
    let seed = b"serde_json_zsa_seed";
    let master = derive_master_secret(seed);
    let sk = derive_spending_key(&master);
    let (ivk, _) = derive_viewing_keys_from_secret(&master);
    let zsa = ZeltraShieldedAddress::from_keys(sk, ivk.public_key());

    let json_str = serde_json::to_string(&zsa).expect("ZSA must serialize to JSON");
    let deserialized: ZeltraShieldedAddress =
        serde_json::from_str(&json_str).expect("ZSA must deserialize from JSON");
    assert_eq!(zsa, deserialized);
}
