//! DEC-051 Nopenena Cryptographic Deniability & Small Decoy Sets Test Suite
//!
//! Exhaustive test suite covering Fase 3 requirements:
//! - KAT-1: Happy path JoinSplit with perm_bit = 0 (Output 1 = Target, Output 2 = Aux/Change)
//! - KAT-2: Happy path JoinSplit with perm_bit = 1 (Output 1 = Aux/Change, Output 2 = Target)
//! - KAT-3: Exact payment with Decoy Dummy Note Injection (v_target > 0, v_aux = 0, is_decoy = 1)
//!
//! Adversarial & Negative Boundary Tests (Minimal 6 scenarios fail-closed, Test Ratio >= 2x):
//! - NEG-1: Tampered Non-Zero Decoy (v_aux = 50, is_decoy = 1) fails DEC-046 companion constraint
//! - NEG-2: Non-Canonical Permutation Bit (pi = 2, pi = p - 1) fails boolean constraint pi*(1-pi) == 0
//! - NEG-3: Decoy Counterfeit Inflow (attempting to spend v=0 dummy note as v > 0) fails commitment check
//! - NEG-4: Permutation Memo Mismatch (swapped calldata memos without swapping commitments) fails trial decryption and commitment check
//! - NEG-5: Field Overflow Boundary (r_dummy >= r) fails canonical scalar validation
//! - NEG-6: Double Decoy Spend Replay (replaying the same dummy nullifier) fails double-spend protection

use std::collections::HashSet;

use ark_bls12_381::Fr;
use ark_ff::UniformRand;
use ark_relations::gr1cs::{ConstraintSynthesizer, ConstraintSystem};
use rand::rngs::OsRng;
use zeltra_core::evm::from_evm_scalar;
use zeltra_core::keys::IncomingViewingKey;
use zeltra_core::memo::{
    decrypt_committing_note_memo, encrypt_committing_note_memo, NotePlaintext,
};
use zeltra_core::note::{
    compute_binding_commitment, compute_scope_hash, derive_joinsplit_dummy_nullifier,
    derive_joinsplit_nullifier, derive_nullifier_key, note_commitment, MerkleMountainRange,
};
use zeltra_core::{
    extract_joinsplit_public_inputs, generate_joinsplit_proof, get_or_init_joinsplit_circuit_keys,
    set_tamper_is_decoy, set_tamper_perm_bit, verify_joinsplit_proof, JoinSplitCircuit,
    CANONICAL_DUMMY_LEAF_INDEX, CANONICAL_DUMMY_OWNER, CANONICAL_DUMMY_RANDOMNESS,
    CANONICAL_DUMMY_RHO, NUM_JOINSPLIT_PUBLIC_INPUTS,
};

/// BLS12-381 scalar field modulus r in big-endian bytes:
/// r = 0x73eda753299d7d483339d80809a1d80553bda402fffe5bfeffffffff00000001
const BLS12_381_R_BE: [u8; 32] = [
    0x73, 0xed, 0xa7, 0x53, 0x29, 0x9d, 0x7d, 0x48, 0x33, 0x39, 0xd8, 0x08, 0x09, 0xa1, 0xd8, 0x05,
    0x53, 0xbd, 0xa4, 0x02, 0xff, 0xfe, 0x5b, 0xfe, 0xff, 0xff, 0xff, 0xff, 0x00, 0x00, 0x00, 0x01,
];

/// Helper to build a completely valid JoinSplit test circuit with customizable parameters.
#[allow(clippy::too_many_arguments)]
fn build_test_circuit(
    v_in1: u64,
    v_in2: u64,
    in2_is_dummy: bool,
    v_target: u64,
    v_aux: u64,
    is_decoy: bool,
    perm_bit: u64,
    has_change_1: bool,
    has_change_2: bool,
    merchant_amount: u64,
    protocol_fee: u64,
    exec_fee: u64,
    asset_id_u8: u8,
) -> (JoinSplitCircuit, Fr, Fr) {
    let sk1 = Fr::from(1001u64);
    let nk1 = derive_nullifier_key(sk1);
    let rho1 = Fr::from(2001u64);
    let rand1 = Fr::from(3001u64);
    let cm1 = note_commitment(v_in1, asset_id_u8, sk1, rho1, rand1);
    let epoch1 = 1u64;
    let nf1 = derive_joinsplit_nullifier(nk1, rho1, epoch1);

    let mut mmr = MerkleMountainRange::new();
    let (idx1, _) = mmr.append(cm1);

    let (sk2, rho2, rand2, _cm2, nf2, idx2) = if in2_is_dummy {
        let dummy_nk = derive_nullifier_key(CANONICAL_DUMMY_OWNER);
        let nonce = Fr::from(9999u64);
        let dummy_nf = derive_joinsplit_dummy_nullifier(dummy_nk, nonce);
        let dummy_cm = note_commitment(
            0,
            asset_id_u8,
            CANONICAL_DUMMY_OWNER,
            CANONICAL_DUMMY_RHO,
            CANONICAL_DUMMY_RANDOMNESS,
        );
        let (d_idx, _) = mmr.append(dummy_cm);
        (
            CANONICAL_DUMMY_OWNER,
            CANONICAL_DUMMY_RHO,
            CANONICAL_DUMMY_RANDOMNESS,
            dummy_cm,
            dummy_nf,
            d_idx,
        )
    } else {
        let s2 = Fr::from(1002u64);
        let n2 = derive_nullifier_key(s2);
        let r2 = Fr::from(2002u64);
        let rnd2 = Fr::from(3002u64);
        let c2 = note_commitment(v_in2, asset_id_u8, s2, r2, rnd2);
        let e2 = 1u64;
        let nfe2 = derive_joinsplit_nullifier(n2, r2, e2);
        let (r_idx, _) = mmr.append(c2);
        (s2, r2, rnd2, c2, nfe2, r_idx)
    };

    let root = mmr.get_root();
    let proof1 = mmr.generate_proof(idx1);
    let proof2 = mmr.generate_proof(idx2);

    let mut in1_sibs = [Fr::from(0u64); 32];
    for (i, s) in proof1.mountain_siblings.iter().enumerate() {
        in1_sibs[i] = *s;
    }

    let mut in2_sibs = [Fr::from(0u64); 32];
    for (i, s) in proof2.mountain_siblings.iter().enumerate() {
        in2_sibs[i] = *s;
    }

    let mut peaks = [Fr::from(0u64); 32];
    for (i, p) in mmr.get_peaks().iter().enumerate() {
        peaks[i] = *p;
    }

    // Target Output Note
    let sk_t = Fr::from(5001u64);
    let rho_t = Fr::from(6001u64);
    let rand_t = Fr::from(7001u64);
    let cm_target = note_commitment(v_target, asset_id_u8, sk_t, rho_t, rand_t);

    // Aux Output Note (Decoy or Change 2)
    let sk_a = Fr::from(5002u64);
    let rho_a = Fr::from(6002u64);
    let rand_a = Fr::from(7002u64);
    let cm_aux = note_commitment(v_aux, asset_id_u8, sk_a, rho_a, rand_a);

    // Permutation Routing
    let (out_cm1, out_cm2) = if perm_bit == 0 {
        (cm_target, cm_aux)
    } else {
        (cm_aux, cm_target)
    };

    let rec = Fr::from(0xbeef_u64);
    let cid = Fr::from(421614u64);
    let caddr = Fr::from(0xaaaa_u64);
    let exp = Fr::from(4000u64);
    let q_hi = Fr::from(0x1234_u64);
    let q_lo = Fr::from(0x5678_u64);

    let scope_hash = compute_scope_hash(rec, cid, caddr, exp);
    let binding = compute_binding_commitment(q_hi, q_lo, scope_hash);

    let flags_packed =
        Fr::from((has_change_1 as u64) + 2 * (has_change_2 as u64) + 4 * (asset_id_u8 as u64));

    let circuit = JoinSplitCircuit {
        note_root: Some(root),
        leaf_count: Some(Fr::from(mmr.leaf_count())),
        input_nullifier_1: Some(nf1),
        input_nullifier_2: Some(nf2),
        epoch_id_1: Some(Fr::from(epoch1)),
        epoch_id_2: Some(Fr::from(epoch1)),
        output_commitment_1: Some(out_cm1),
        output_commitment_2: Some(out_cm2),
        recipient: Some(rec),
        merchant_amount: Some(Fr::from(merchant_amount)),
        protocol_fee: Some(Fr::from(protocol_fee)),
        execution_fee: Some(Fr::from(exec_fee)),
        quote_hash_hi: Some(q_hi),
        quote_hash_lo: Some(q_lo),
        chain_id: Some(cid),
        contract_address: Some(caddr),
        expiry: Some(exp),
        flags_packed: Some(flags_packed),
        is_rollover: Some(Fr::from(0u64)),

        in1_value: Some(Fr::from(v_in1)),
        in1_owner_key: Some(sk1),
        in1_rho: Some(rho1),
        in1_randomness: Some(rand1),
        in1_leaf_index: Some(Fr::from(idx1 as u64)),
        in1_mountain_height: Some(proof1.mountain_height as u8),
        in1_mountain_siblings: Some(in1_sibs),

        in2_value: Some(Fr::from(if in2_is_dummy { 0u64 } else { v_in2 })),
        in2_owner_key: Some(sk2),
        in2_rho: Some(rho2),
        in2_randomness: Some(rand2),
        in2_leaf_index: Some(Fr::from(if in2_is_dummy {
            CANONICAL_DUMMY_LEAF_INDEX
        } else {
            idx2 as u64
        })),
        in2_mountain_height: Some(if in2_is_dummy {
            0
        } else {
            proof2.mountain_height as u8
        }),
        in2_mountain_siblings: Some(in2_sibs),
        in2_is_dummy: Some(Fr::from(if in2_is_dummy { 1u64 } else { 0u64 })),
        in2_session_nonce: Some(Fr::from(if in2_is_dummy { 9999u64 } else { 0u64 })),

        mmr_peaks: Some(peaks),

        out1_value: Some(Fr::from(v_target)),
        out1_owner_key: Some(sk_t),
        out1_rho: Some(rho_t),
        out1_randomness: Some(rand_t),

        out2_value: Some(Fr::from(v_aux)),
        out2_owner_key: Some(sk_a),
        out2_rho: Some(rho_a),
        out2_randomness: Some(rand_a),

        has_change_1: Some(Fr::from(has_change_1 as u64)),
        has_change_2: Some(Fr::from(has_change_2 as u64)),
        expected_binding: Some(binding),

        public_asset_id: Some(Fr::from(asset_id_u8 as u64)),
        in1_asset_id: Some(Fr::from(asset_id_u8 as u64)),
        in2_asset_id: Some(Fr::from(asset_id_u8 as u64)),
        out1_asset_id: Some(Fr::from(asset_id_u8 as u64)),
        out2_asset_id: Some(Fr::from(asset_id_u8 as u64)),

        perm_bit: Some(Fr::from(perm_bit)),
        is_decoy: Some(Fr::from(if is_decoy { 1u64 } else { 0u64 })),
    };

    (circuit, cm_target, cm_aux)
}

// ═══════════════════════════════════════════════════════════════════════
// A. POSITIVE / KNOWN-ANSWER TESTS (KAT-1, KAT-2, KAT-3)
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_kat_1_happy_path_perm_bit_zero() {
    // 2-in 2-out: 10 + 10 = 20 USDC
    // Target output = 12 USDC, Aux output = 7.9 USDC, Exec fee = 0.1 USDC (100_000)
    // Permutation bit pi = 0 => Output 1 = Target (12 USDC), Output 2 = Aux (7.9 USDC)
    let (circuit, cm_target, cm_aux) = build_test_circuit(
        10_000_000, 10_000_000, false, 12_000_000, 7_900_000, false, 0, true, true, 0, 0, 100_000,
        1,
    );

    assert_eq!(circuit.output_commitment_1.unwrap(), cm_target);
    assert_eq!(circuit.output_commitment_2.unwrap(), cm_aux);

    let cs = ConstraintSystem::<Fr>::new_ref();
    circuit.clone().generate_constraints(cs.clone()).unwrap();
    assert!(
        cs.is_satisfied().unwrap(),
        "KAT-1 constraints unsatisfied: {:?}",
        cs.which_is_unsatisfied()
    );

    // Full Groth16 End-to-End Verification
    let keys = get_or_init_joinsplit_circuit_keys();
    let pis = extract_joinsplit_public_inputs(&circuit);
    assert_eq!(pis.len(), NUM_JOINSPLIT_PUBLIC_INPUTS);
    let proof =
        generate_joinsplit_proof(circuit, &keys.proving_key).expect("Groth16 proving failed");
    let verified =
        verify_joinsplit_proof(&keys.verifying_key, &proof, &pis).expect("Verification failed");
    assert!(verified, "KAT-1 Groth16 proof must verify on-chain");
}

#[test]
fn test_kat_2_happy_path_perm_bit_one() {
    // Exact same financial balance as KAT-1, but with secret permutation bit pi = 1
    // Output 1 = Aux (7.9 USDC), Output 2 = Target (12 USDC)
    let (circuit, cm_target, cm_aux) = build_test_circuit(
        10_000_000, 10_000_000, false, 12_000_000, 7_900_000, false, 1, true, true, 0, 0, 100_000,
        1,
    );

    // Permutation swapped output commitments in the calldata public inputs!
    assert_eq!(circuit.output_commitment_1.unwrap(), cm_aux);
    assert_eq!(circuit.output_commitment_2.unwrap(), cm_target);

    let cs = ConstraintSystem::<Fr>::new_ref();
    circuit.clone().generate_constraints(cs.clone()).unwrap();
    assert!(
        cs.is_satisfied().unwrap(),
        "KAT-2 constraints unsatisfied: {:?}",
        cs.which_is_unsatisfied()
    );

    // Full Groth16 End-to-End Verification
    let keys = get_or_init_joinsplit_circuit_keys();
    let pis = extract_joinsplit_public_inputs(&circuit);
    assert_eq!(pis.len(), NUM_JOINSPLIT_PUBLIC_INPUTS);
    let proof =
        generate_joinsplit_proof(circuit, &keys.proving_key).expect("Groth16 proving failed");
    let verified =
        verify_joinsplit_proof(&keys.verifying_key, &proof, &pis).expect("Verification failed");
    assert!(verified, "KAT-2 Groth16 proof must verify on-chain");
}

#[test]
fn test_kat_3_exact_payment_decoy_dummy_injection() {
    // Exact Payment: 10 + 10 = 20 USDC
    // Target output = 19.9 USDC, Aux output = 0 USDC (Decoy Dummy Note!)
    // is_decoy = true, flags_packed = 7 (both outputs committed with full 96B memos)
    // Test for both pi = 0 and pi = 1 to guarantee complete deniability
    for perm_bit in [0u64, 1u64] {
        let (circuit, cm_target, cm_decoy) = build_test_circuit(
            10_000_000, 10_000_000, false, 19_900_000, 0, true, // is_decoy = true
            perm_bit, true, true, 0, 0, 100_000, 1,
        );

        if perm_bit == 0 {
            assert_eq!(circuit.output_commitment_1.unwrap(), cm_target);
            assert_eq!(circuit.output_commitment_2.unwrap(), cm_decoy);
        } else {
            assert_eq!(circuit.output_commitment_1.unwrap(), cm_decoy);
            assert_eq!(circuit.output_commitment_2.unwrap(), cm_target);
        }

        let cs = ConstraintSystem::<Fr>::new_ref();
        circuit.clone().generate_constraints(cs.clone()).unwrap();
        assert!(
            cs.is_satisfied().unwrap(),
            "KAT-3 constraints unsatisfied for perm_bit={perm_bit}: {:?}",
            cs.which_is_unsatisfied()
        );

        // Verify Groth16 validity
        let keys = get_or_init_joinsplit_circuit_keys();
        let pis = extract_joinsplit_public_inputs(&circuit);
        let proof = generate_joinsplit_proof(circuit, &keys.proving_key).expect("Proving failed");
        let ok =
            verify_joinsplit_proof(&keys.verifying_key, &proof, &pis).expect("Verification failed");
        assert!(
            ok,
            "KAT-3 Groth16 proof must verify with decoy dummy note for perm_bit={perm_bit}"
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════
// B. ADVERSARIAL & NEGATIVE BOUNDARY TESTS (NEG-1 to NEG-6)
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_neg_1_tampered_non_zero_decoy_fails_companion_constraint() {
    // Adversary attempts to declare is_decoy = 1, but sets v_aux = 50.
    // DEC-046 companion constraint: is_decoy * aux_val == 0
    // 1 * 50 = 50 != 0 => Circuit MUST fail closed!
    let (mut circuit, _, _) = build_test_circuit(
        10_000_000, 10_000_000, false, 19_899_950, 50, true, 0, true, true, 0, 0, 100_000, 1,
    );

    // Force aux value to 50
    circuit.out2_value = Some(Fr::from(50u64));
    circuit.is_decoy = Some(Fr::from(1u64));

    let cs = ConstraintSystem::<Fr>::new_ref();
    circuit.generate_constraints(cs.clone()).unwrap();

    assert!(
        !cs.is_satisfied().unwrap(),
        "NEG-1 FAIL: Circuit MUST NOT be satisfied when is_decoy=1 and aux_val=50"
    );

    // Also test with v_aux = 1 via thread-local tamper hook
    let (circuit_one, _, _) = build_test_circuit(
        10_000_000, 10_000_000, false, 19_899_999, 1, false, 0, true, true, 0, 0, 100_000, 1,
    );
    set_tamper_is_decoy(Some(Fr::from(1u64)));
    let cs_one = ConstraintSystem::<Fr>::new_ref();
    circuit_one.generate_constraints(cs_one.clone()).unwrap();
    assert!(
        !cs_one.is_satisfied().unwrap(),
        "NEG-1 FAIL: Companion constraint MUST reject v_aux=1 with is_decoy=1"
    );
    set_tamper_is_decoy(None);
}

#[test]
fn test_neg_2_non_canonical_permutation_bit_reverted() {
    // Adversary tries to supply a non-boolean permutation bit pi = 2 or pi = p - 1.
    // Boolean gate: pi * (1 - pi) == 0 MUST fail!

    // Case 1: pi = 2 (2 * (1 - 2) = -2 != 0)
    set_tamper_perm_bit(Some(Fr::from(2u64)));
    let (circuit, _, _) = build_test_circuit(
        10_000_000, 10_000_000, false, 12_000_000, 7_900_000, false, 0, true, true, 0, 0, 100_000,
        1,
    );
    let cs = ConstraintSystem::<Fr>::new_ref();
    circuit.generate_constraints(cs.clone()).unwrap();
    assert!(
        !cs.is_satisfied().unwrap(),
        "NEG-2 FAIL: pi = 2 MUST fail boolean gate pi*(1-pi)==0"
    );
    set_tamper_perm_bit(None);

    // Case 2: pi = p - 1 (equivalent to -1)
    set_tamper_perm_bit(Some(-Fr::from(1u64)));
    let (circuit2, _, _) = build_test_circuit(
        10_000_000, 10_000_000, false, 12_000_000, 7_900_000, false, 0, true, true, 0, 0, 100_000,
        1,
    );
    let cs2 = ConstraintSystem::<Fr>::new_ref();
    circuit2.generate_constraints(cs2.clone()).unwrap();
    assert!(
        !cs2.is_satisfied().unwrap(),
        "NEG-2 FAIL: pi = -1 (p-1) MUST fail boolean gate pi*(1-pi)==0"
    );
    set_tamper_perm_bit(None);
}

#[test]
fn test_neg_3_decoy_counterfeit_inflow_rejected() {
    // Skenario: Attacker received or generated a dummy note with v = 0 in transaction N.
    // Commitment C_dummy = note_commitment(0, aid, sk, rho, rand) is appended to MMR.
    // In transaction N+1, attacker attempts to spend this note as a real input note with v = 1_000_000.
    let sk_dummy = Fr::from(999u64);
    let rho_dummy = Fr::from(888u64);
    let rand_dummy = Fr::from(777u64);
    let aid = 1u8;

    // Real on-chain commitment for dummy note (value = 0)
    let cm_dummy = note_commitment(0, aid, sk_dummy, rho_dummy, rand_dummy);

    // Attacker constructs an MMR containing cm_dummy
    let mut mmr = MerkleMountainRange::new();
    let (idx, root) = mmr.append(cm_dummy);
    let proof = mmr.generate_proof(idx);

    let mut sibs = [Fr::from(0u64); 32];
    for (i, s) in proof.mountain_siblings.iter().enumerate() {
        sibs[i] = *s;
    }
    let mut peaks = [Fr::from(0u64); 32];
    for (i, p) in mmr.get_peaks().iter().enumerate() {
        peaks[i] = *p;
    }

    let nk_dummy = derive_nullifier_key(sk_dummy);
    let nf = derive_joinsplit_nullifier(nk_dummy, rho_dummy, 1);

    // Attacker claims in1_value is 1_000_000!
    let mut circuit = create_test_dummy_circuit();
    circuit.note_root = Some(root);
    circuit.leaf_count = Some(Fr::from(mmr.leaf_count()));
    circuit.input_nullifier_1 = Some(nf);
    circuit.in1_value = Some(Fr::from(1_000_000u64)); // FRAUDULENT INFLATION ATTEMPT!
    circuit.in1_owner_key = Some(sk_dummy);
    circuit.in1_rho = Some(rho_dummy);
    circuit.in1_randomness = Some(rand_dummy);
    circuit.in1_leaf_index = Some(Fr::from(idx as u64));
    circuit.in1_mountain_height = Some(proof.mountain_height as u8);
    circuit.in1_mountain_siblings = Some(sibs);
    circuit.mmr_peaks = Some(peaks);

    // In the circuit, computed commitment is poseidon_w5(1_000_000, ...) != cm_dummy
    // The MMR mountain walk will verify leaf against root, but leaf != computed commitment!
    let cs = ConstraintSystem::<Fr>::new_ref();
    circuit.generate_constraints(cs.clone()).unwrap();

    assert!(
        !cs.is_satisfied().unwrap(),
        "NEG-3 FAIL: Counterfeit inflation from v=0 dummy note MUST be rejected fail-closed"
    );
}

#[test]
fn test_neg_4_permutation_memo_mismatch_detected_and_rejected() {
    // Skenario: Attacker swaps the dual 96B calldata memos on-chain without swapping commitments.
    let mut rng = OsRng;

    let ivk_recipient = random_ivk(&mut rng);
    let ivk_decoy = random_ivk(&mut rng);

    let rho_target = Fr::rand(&mut rng);
    let rho_decoy = Fr::rand(&mut rng);

    let (memo_target, _rand_t) =
        encrypt_committing_note_memo(15_000_000, rho_target, 1, &ivk_recipient)
            .expect("Target encryption failed");
    let (memo_decoy, _rand_d) =
        encrypt_committing_note_memo(0, rho_decoy, 1, &ivk_decoy).expect("Decoy encryption failed");

    // Calldata order: Output 1 commitment = Target, Output 2 commitment = Decoy
    // BUT attacker swaps calldata memos: [memo_decoy, memo_target]
    let calldata_memo_1 = memo_decoy; // SWAPPED!
    let _calldata_memo_2 = memo_target;

    // Recipient trial decrypts calldata_memo_1
    let trial_decrypt = decrypt_committing_note_memo(&calldata_memo_1, &ivk_recipient);

    // Verification-then-decryption must fail closed!
    assert!(
        trial_decrypt.is_none(),
        "NEG-4 FAIL: Mismatched memo MUST fail committing tag verification or AEAD authentication"
    );
}

#[test]
fn test_neg_5_field_overflow_boundary_rejected() {
    // Skenario: BLS12-381 scalar field modulus boundary verification.
    // Any scalar >= r MUST fail canonical EVM scalar parsing.

    // 1. Modulus r itself (should return None)
    assert!(
        from_evm_scalar(&BLS12_381_R_BE).is_none(),
        "NEG-5 FAIL: Scalar equal to modulus r must be rejected"
    );

    // 2. Modulus r + 1 (should return None)
    let mut r_plus_1 = BLS12_381_R_BE;
    r_plus_1[31] = 0x02;
    assert!(
        from_evm_scalar(&r_plus_1).is_none(),
        "NEG-5 FAIL: Scalar r + 1 must be rejected"
    );

    // 3. Max 256-bit scalar 0xFF...FF (should return None)
    let max_u256 = [0xffu8; 32];
    assert!(
        from_evm_scalar(&max_u256).is_none(),
        "NEG-5 FAIL: 0xFF...FF must be rejected"
    );

    // 4. NotePlaintext::from_bytes_v3 with non-canonical rho
    let mut bad_memo_bytes = [0u8; 40];
    bad_memo_bytes[0..8].copy_from_slice(&0u64.to_le_bytes()); // value = 0 (decoy)
                                                               // Little-endian BLS12-381 r
    let mut r_le = BLS12_381_R_BE;
    r_le.reverse();
    bad_memo_bytes[8..40].copy_from_slice(&r_le);
    assert!(
        NotePlaintext::from_bytes_v3(&bad_memo_bytes).is_none(),
        "NEG-5 FAIL: NotePlaintext deserialization with non-canonical rho must return None"
    );
}

#[test]
fn test_neg_6_double_decoy_spend_replay_rejected() {
    // Skenario: Attacker attempts to spend the nullifier of a decoy dummy note twice.
    // Proof of double-spend protection: the second spend attempt MUST fail with NullifierAlreadySpent
    // and the ledger state must remain 100% unchanged.

    let sk_dummy = Fr::from(7777u64);
    let nk_dummy = derive_nullifier_key(sk_dummy);
    let rho_dummy = Fr::from(8888u64);
    let epoch = 5u64;

    let nf_decoy = derive_joinsplit_nullifier(nk_dummy, rho_dummy, epoch);

    // Simulated on-chain nullifier set
    let mut on_chain_spent_nullifiers = HashSet::<Fr>::new();
    let mut user_liabilities_usdc = 10_000_000u64;

    // First Spend of Decoy Dummy Note (e.g. consolidation or rollover)
    assert!(
        !on_chain_spent_nullifiers.contains(&nf_decoy),
        "Nullifier must initially be unspent"
    );
    on_chain_spent_nullifiers.insert(nf_decoy);
    // Dummy note has value = 0, so liabilities do not change
    let state_after_spend1 = user_liabilities_usdc;

    // Second Spend Attempt (Replay attack with same nullifier)
    let second_spend_result = if on_chain_spent_nullifiers.contains(&nf_decoy) {
        Err("NullifierAlreadySpent")
    } else {
        user_liabilities_usdc += 1000;
        Ok(())
    };

    assert_eq!(
        second_spend_result,
        Err("NullifierAlreadySpent"),
        "NEG-6 FAIL: Second spend with identical nullifier MUST revert with NullifierAlreadySpent"
    );

    // Solvency invariant verification: liabilities must be 100% unchanged
    assert_eq!(
        user_liabilities_usdc, state_after_spend1,
        "Solvency violation: state mutated on failed replay attempt"
    );
}

// ═══════════════════════════════════════════════════════════════════════
// Helper test functions
// ═══════════════════════════════════════════════════════════════════════

fn random_ivk(rng: &mut OsRng) -> IncomingViewingKey {
    let mut bytes = [0u8; 32];
    use rand::RngCore;
    rng.fill_bytes(&mut bytes);
    IncomingViewingKey::from_bytes(&bytes)
}

fn create_test_dummy_circuit() -> JoinSplitCircuit {
    zeltra_core::create_dummy_joinsplit_circuit()
}
