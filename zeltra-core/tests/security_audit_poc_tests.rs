//! Empirical Proof-of-Concept (PoC) & Security Audit Test Suite: `zeltra-core`
//!
//! Evaluates and verifies:
//! - ZCORE-CRIT-01: Decoupled leaf index bounds, nullifier key integrity, and fail-closed defenses in `PrivateNoteCircuit`
//! - ZCORE-HIGH-01 & ZCORE-MED-01: Companion constraints, dummy slot pinning, and anti-aliasing in `JoinSplitCircuit`
//! - Mandatory Testing Ratio: At least 2x Adversarial Negative Tests for every Positive Test
//! - Fail-Closed Invariant: All negative tests assert `!cs.is_satisfied().unwrap()`
//! - Empirical Benchmarks: Records R1CS constraint counts, instance variables, and witness counts

use ark_bls12_381::Fr;
use ark_ff::{One, Zero};
use ark_relations::gr1cs::{ConstraintSynthesizer, ConstraintSystem};
use zeltra_core::joinsplit_circuit::{create_dummy_joinsplit_circuit, JoinSplitCircuit};
use zeltra_core::note::{
    derive_joinsplit_nullifier, derive_nullifier_key, note_commitment, MerkleMountainRange,
};
use zeltra_core::note_circuit::create_dummy_circuit;

// ═══════════════════════════════════════════════════════════════════════
// Helper: Construct Valid 2-in-2-out Mode 2 In-Pool Transfer Circuit
// ═══════════════════════════════════════════════════════════════════════

fn setup_valid_mode2_inpool_joinsplit() -> JoinSplitCircuit {
    let sk = Fr::from(100u64);
    let nk = derive_nullifier_key(sk);
    let rho1 = Fr::from(201u64);
    let rand1 = Fr::from(301u64);
    let rho2 = Fr::from(202u64);
    let rand2 = Fr::from(302u64);
    let v_in = 10_000_000u64; // 10 USDC each = 20 USDC total
    let asset_id_u8 = 1u8;

    let cm1 = note_commitment(v_in, asset_id_u8, sk, rho1, rand1);
    let cm2 = note_commitment(v_in, asset_id_u8, sk, rho2, rand2);

    let mut mmr = MerkleMountainRange::new();
    let (idx1, _) = mmr.append(cm1);
    let (idx2, root) = mmr.append(cm2);

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

    let epoch1 = 1u64;
    let epoch2 = 1u64;
    let nf1 = derive_joinsplit_nullifier(nk, rho1, epoch1);
    let nf2 = derive_joinsplit_nullifier(nk, rho2, epoch2);

    // Mode 2: In-Pool Transfer (DEC-043B):
    // recipient = 0x0, merchant_amount = 0, protocol_fee = 0
    // Total in = 20_000_000. Total out = Out1 (12_000_000) + Out2 (8_000_000) = 20_000_000
    let out1_val = 12_000_000u64;
    let out2_val = 8_000_000u64;

    let rho_c1 = Fr::from(555u64);
    let rand_c1 = Fr::from(666u64);
    let cm_out1 = note_commitment(out1_val, asset_id_u8, sk, rho_c1, rand_c1);

    let rho_c2 = Fr::from(777u64);
    let rand_c2 = Fr::from(888u64);
    let cm_out2 = note_commitment(out2_val, asset_id_u8, sk, rho_c2, rand_c2);

    let rec = Fr::from(0u64); // Mode 2: Recipient MUST be 0
    let cid = Fr::from(421614u64);
    let caddr = Fr::from(0xaaaa_u64);
    let exp = Fr::from(3000u64);
    let q_hi = Fr::from(0x7777u64);
    let q_lo = Fr::from(0x8888u64);

    let scope_hash = zeltra_core::note::compute_scope_hash(rec, cid, caddr, exp);
    let binding = zeltra_core::note::compute_binding_commitment(q_hi, q_lo, scope_hash);

    JoinSplitCircuit {
        note_root: Some(root),
        leaf_count: Some(Fr::from(mmr.leaf_count())),
        input_nullifier_1: Some(nf1),
        input_nullifier_2: Some(nf2),
        epoch_id_1: Some(Fr::from(epoch1)),
        epoch_id_2: Some(Fr::from(epoch2)),
        output_commitment_1: Some(cm_out1),
        output_commitment_2: Some(cm_out2),
        recipient: Some(rec),
        merchant_amount: Some(Fr::from(0u64)),
        protocol_fee: Some(Fr::from(0u64)),
        execution_fee: Some(Fr::from(0u64)),
        quote_hash_hi: Some(q_hi),
        quote_hash_lo: Some(q_lo),
        chain_id: Some(cid),
        contract_address: Some(caddr),
        expiry: Some(exp),
        // flags: has_change_1 = 1, has_change_2 = 1, asset_id = 1 (1 + 2 + 4 = 7)
        flags_packed: Some(Fr::from(7u64)),
        is_rollover: Some(Fr::from(0u64)),

        in1_value: Some(Fr::from(v_in)),
        in1_owner_key: Some(sk),
        in1_rho: Some(rho1),
        in1_randomness: Some(rand1),
        in1_leaf_index: Some(Fr::from(idx1 as u64)),
        in1_mountain_height: Some(proof1.mountain_height as u8),
        in1_mountain_siblings: Some(in1_sibs),

        in2_value: Some(Fr::from(v_in)),
        in2_owner_key: Some(sk),
        in2_rho: Some(rho2),
        in2_randomness: Some(rand2),
        in2_leaf_index: Some(Fr::from(idx2 as u64)),
        in2_mountain_height: Some(proof2.mountain_height as u8),
        in2_mountain_siblings: Some(in2_sibs),
        in2_is_dummy: Some(Fr::from(0u64)),
        in2_session_nonce: Some(Fr::from(0u64)),

        mmr_peaks: Some(peaks),

        out1_value: Some(Fr::from(out1_val)),
        out1_owner_key: Some(sk),
        out1_rho: Some(rho_c1),
        out1_randomness: Some(rand_c1),
        has_change_1: Some(Fr::from(1u64)),

        out2_value: Some(Fr::from(out2_val)),
        out2_owner_key: Some(sk),
        out2_rho: Some(rho_c2),
        out2_randomness: Some(rand_c2),
        has_change_2: Some(Fr::from(1u64)),

        expected_binding: Some(binding),

        public_asset_id: Some(Fr::from(asset_id_u8 as u64)),
        in1_asset_id: Some(Fr::from(asset_id_u8 as u64)),
        in2_asset_id: Some(Fr::from(asset_id_u8 as u64)),
        out1_asset_id: Some(Fr::from(asset_id_u8 as u64)),
        out2_asset_id: Some(Fr::from(asset_id_u8 as u64)),

        perm_bit: Some(Fr::from(0u64)),
        is_decoy: Some(Fr::from(0u64)),
    }
}

// ═══════════════════════════════════════════════════════════════════════
// CATEGORY 1: PrivateNoteCircuit (ZCORE-CRIT-01 & Defenses)
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_poc_pos_private_note_baseline() {
    let circuit = create_dummy_circuit();
    let cs = ConstraintSystem::<Fr>::new_ref();
    circuit
        .generate_constraints(cs.clone())
        .expect("PrivateNoteCircuit constraint generation must succeed");

    assert!(
        cs.is_satisfied().unwrap(),
        "Baseline honest PrivateNoteCircuit MUST be satisfied!"
    );

    let num_constraints = cs.num_constraints();
    let num_instance = cs.num_instance_variables();
    let num_witness = cs.num_witness_variables();

    println!(
        "EMPIRICAL_BENCHMARK: PrivateNoteCircuit Baseline: constraints={}, instance_vars={}, witness_vars={}",
        num_constraints, num_instance, num_witness
    );

    assert!(
        num_constraints > 0 && num_constraints < 30_000,
        "PrivateNoteCircuit constraint budget check"
    );
}

#[test]
fn test_poc_neg_zcore_crit01_decoupled_leaf_index_out_of_bounds_fail_closed() {
    // Adversarial vector: An attacker attempts to spend a note with leaf_index >= leaf_count
    // (Hyperbridge / tree traversal bypass attack)
    let mut circuit = create_dummy_circuit();
    // In dummy circuit: leaf_count = 1. Attacker sets input_leaf_index = 1 (out of bounds: [0, 1) allowed)
    circuit.input_leaf_index = circuit.leaf_count;

    let cs = ConstraintSystem::<Fr>::new_ref();
    circuit.generate_constraints(cs.clone()).unwrap();

    assert!(
        !cs.is_satisfied().unwrap(),
        "FAIL-CLOSED VIOLATION: PrivateNoteCircuit accepted out-of-bounds leaf index!"
    );
}

#[test]
fn test_poc_neg_zcore_crit01_tampered_nullifier_fail_closed() {
    // Adversarial vector: Attacker mutates the nullifier public input to evade on-chain double-spend checks
    let mut circuit = create_dummy_circuit();
    let original_nf = circuit.input_nullifier.unwrap();
    circuit.input_nullifier = Some(original_nf + Fr::one());

    let cs = ConstraintSystem::<Fr>::new_ref();
    circuit.generate_constraints(cs.clone()).unwrap();

    assert!(
        !cs.is_satisfied().unwrap(),
        "FAIL-CLOSED VIOLATION: Tampered nullifier satisfied PrivateNoteCircuit!"
    );
}

#[test]
fn test_poc_neg_zcore_crit01_unauthorized_spender_owner_key_mismatch_fail_closed() {
    // Adversarial vector: Attacker without knowledge of owner spending key attempts to spend victim note
    let mut circuit = create_dummy_circuit();
    let spoofed_owner = Fr::from(0xbadf00du64);
    circuit.input_owner_key = Some(spoofed_owner);

    let cs = ConstraintSystem::<Fr>::new_ref();
    circuit.generate_constraints(cs.clone()).unwrap();

    assert!(
        !cs.is_satisfied().unwrap(),
        "FAIL-CLOSED VIOLATION: Unauthorized owner key satisfied PrivateNoteCircuit!"
    );
}

#[test]
fn test_poc_neg_zcore_crit01_tampered_merkle_root_fail_closed() {
    // Adversarial vector: Attacker attempts spend against an uncommitted / fabricated root
    let mut circuit = create_dummy_circuit();
    circuit.note_root = Some(circuit.note_root.unwrap() + Fr::one());

    let cs = ConstraintSystem::<Fr>::new_ref();
    circuit.generate_constraints(cs.clone()).unwrap();

    assert!(
        !cs.is_satisfied().unwrap(),
        "FAIL-CLOSED VIOLATION: Tampered Merkle root satisfied PrivateNoteCircuit!"
    );
}

#[test]
fn test_poc_neg_zcore_crit01_value_conservation_violation_fail_closed() {
    // Adversarial vector: Attacker inflates merchant payout beyond note input value
    let mut circuit = create_dummy_circuit();
    let input_val = circuit.input_value.unwrap();
    circuit.merchant_amount = Some(input_val + Fr::from(1_000_000u64));

    let cs = ConstraintSystem::<Fr>::new_ref();
    circuit.generate_constraints(cs.clone()).unwrap();

    assert!(
        !cs.is_satisfied().unwrap(),
        "FAIL-CLOSED VIOLATION: Value inflation satisfied PrivateNoteCircuit!"
    );
}

#[test]
fn test_poc_neg_zcore_crit01_tampered_epoch_fail_closed() {
    // Adversarial vector: Attacker tampers note_epoch_id to manipulate generational nullifier derivation
    let mut circuit = create_dummy_circuit();
    circuit.note_epoch_id = Some(circuit.note_epoch_id.unwrap() + Fr::one());

    let cs = ConstraintSystem::<Fr>::new_ref();
    circuit.generate_constraints(cs.clone()).unwrap();

    assert!(
        !cs.is_satisfied().unwrap(),
        "FAIL-CLOSED VIOLATION: Tampered note_epoch_id satisfied PrivateNoteCircuit!"
    );
}

// ═══════════════════════════════════════════════════════════════════════
// CATEGORY 2: JoinSplitCircuit (ZCORE-HIGH-01, ZCORE-MED-01, Soundness)
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_poc_pos_joinsplit_mode3_public_outflow_dummy_slot2_baseline() {
    let circuit = create_dummy_joinsplit_circuit();
    let cs = ConstraintSystem::<Fr>::new_ref();
    circuit
        .generate_constraints(cs.clone())
        .expect("JoinSplitCircuit constraint generation must succeed");

    assert!(
        cs.is_satisfied().unwrap(),
        "Baseline honest Mode 3 JoinSplitCircuit MUST be satisfied!"
    );

    let num_constraints = cs.num_constraints();
    let num_instance = cs.num_instance_variables();
    let num_witness = cs.num_witness_variables();

    println!(
        "EMPIRICAL_BENCHMARK: JoinSplitCircuit Mode 3 Baseline: constraints={}, instance_vars={}, witness_vars={}",
        num_constraints, num_instance, num_witness
    );

    assert!(
        num_constraints > 30_000 && num_constraints < 40_000,
        "JoinSplitCircuit constraint budget check (~34,571 gates)"
    );
}

#[test]
fn test_poc_pos_joinsplit_mode2_inpool_transfer_baseline() {
    let circuit = setup_valid_mode2_inpool_joinsplit();
    let cs = ConstraintSystem::<Fr>::new_ref();
    circuit
        .generate_constraints(cs.clone())
        .expect("JoinSplitCircuit Mode 2 constraint generation must succeed");

    assert!(
        cs.is_satisfied().unwrap(),
        "Baseline honest Mode 2 in-pool transfer MUST be satisfied!"
    );

    let num_constraints = cs.num_constraints();
    println!(
        "EMPIRICAL_BENCHMARK: JoinSplitCircuit Mode 2 Baseline: constraints={}",
        num_constraints
    );
}

#[test]
fn test_poc_neg_joinsplit_nullifier_singularity_aliasing_fail_closed() {
    // Adversarial vector: Singularity attack (Zellic 2024 / CVE-2020-15103)
    // Attacker crafts transaction where input_nullifier_1 == input_nullifier_2
    let mut circuit = setup_valid_mode2_inpool_joinsplit();
    circuit.input_nullifier_2 = circuit.input_nullifier_1;

    let cs = ConstraintSystem::<Fr>::new_ref();
    circuit.generate_constraints(cs.clone()).unwrap();

    assert!(
        !cs.is_satisfied().unwrap(),
        "FAIL-CLOSED VIOLATION: Nullifier singularity aliasing satisfied JoinSplitCircuit!"
    );
}

#[test]
fn test_poc_neg_joinsplit_mode2_recipient_graph_leak_fail_closed() {
    // Adversarial vector: In Mode 2 in-pool transfer (0 bps fee), an attacker sets non-zero recipient
    // (violating graph detachment and circular liquidity privacy invariants)
    let mut circuit = setup_valid_mode2_inpool_joinsplit();
    circuit.recipient = Some(Fr::from(0x1337u64));

    let cs = ConstraintSystem::<Fr>::new_ref();
    circuit.generate_constraints(cs.clone()).unwrap();

    assert!(
        !cs.is_satisfied().unwrap(),
        "FAIL-CLOSED VIOLATION: Non-zero recipient accepted in Mode 2 in-pool transfer!"
    );
}

#[test]
fn test_poc_neg_joinsplit_mode2_nonzero_protocol_fee_fail_closed() {
    // Adversarial vector: In Mode 2 in-pool transfer, an attacker injects non-zero protocol_fee
    // (violating 0% in-pool transfer fee invariant)
    let mut circuit = setup_valid_mode2_inpool_joinsplit();
    circuit.protocol_fee = Some(Fr::from(50_000u64));

    let cs = ConstraintSystem::<Fr>::new_ref();
    circuit.generate_constraints(cs.clone()).unwrap();

    assert!(
        !cs.is_satisfied().unwrap(),
        "FAIL-CLOSED VIOLATION: Non-zero protocol fee accepted in Mode 2 in-pool transfer!"
    );
}

#[test]
fn test_poc_neg_joinsplit_malleable_flags_packed_fail_closed() {
    // Adversarial vector: Non-canonical bit manipulation on flags_packed (setting bit 3, flags = 8)
    let mut circuit = create_dummy_joinsplit_circuit();
    circuit.flags_packed = Some(Fr::from(8u64));

    let cs = ConstraintSystem::<Fr>::new_ref();
    circuit.generate_constraints(cs.clone()).unwrap();

    assert!(
        !cs.is_satisfied().unwrap(),
        "FAIL-CLOSED VIOLATION: Malleable flags_packed (flags > 7) satisfied JoinSplitCircuit!"
    );
}

#[test]
fn test_poc_neg_joinsplit_value_inflation_fail_closed() {
    // Adversarial vector: Value conservation break — output sum exceeds input sum
    let mut circuit = create_dummy_joinsplit_circuit();
    let in_val = circuit.in1_value.unwrap();
    circuit.merchant_amount = Some(in_val + Fr::from(10_000_000u64));

    let cs = ConstraintSystem::<Fr>::new_ref();
    circuit.generate_constraints(cs.clone()).unwrap();

    assert!(
        !cs.is_satisfied().unwrap(),
        "FAIL-CLOSED VIOLATION: Inflated value satisfied JoinSplitCircuit!"
    );
}

#[test]
fn test_poc_neg_joinsplit_dec046_companion_pinning_dummy_owner_fail_closed() {
    // Adversarial vector: DEC-046 Violation — Floating witness injection on dummy slot 2
    // In dummy slot 2 (in2_is_dummy = 1), attacker injects non-zero in2_owner_key
    let mut circuit = create_dummy_joinsplit_circuit();
    assert_eq!(circuit.in2_is_dummy, Some(Fr::one()));
    circuit.in2_owner_key = Some(Fr::from(0x9999u64)); // Non-canonical!

    let cs = ConstraintSystem::<Fr>::new_ref();
    circuit.generate_constraints(cs.clone()).unwrap();

    assert!(
        !cs.is_satisfied().unwrap(),
        "FAIL-CLOSED VIOLATION: Non-canonical dummy owner key bypassed DEC-046 companion constraints!"
    );
}

#[test]
fn test_poc_neg_joinsplit_dec046_companion_pinning_dummy_rho_fail_closed() {
    // Adversarial vector: DEC-046 Violation — Inject non-zero in2_rho on dummy slot 2
    let mut circuit = create_dummy_joinsplit_circuit();
    assert_eq!(circuit.in2_is_dummy, Some(Fr::one()));
    circuit.in2_rho = Some(Fr::from(0x8888u64)); // Non-canonical!

    let cs = ConstraintSystem::<Fr>::new_ref();
    circuit.generate_constraints(cs.clone()).unwrap();

    assert!(
        !cs.is_satisfied().unwrap(),
        "FAIL-CLOSED VIOLATION: Non-canonical dummy rho bypassed DEC-046 companion constraints!"
    );
}

#[test]
fn test_poc_neg_joinsplit_scope_binding_quote_hash_tampered_fail_closed() {
    // Adversarial vector: Attacker mutates quote_hash_hi to decouple relayer quote binding
    let mut circuit = create_dummy_joinsplit_circuit();
    circuit.quote_hash_hi = Some(circuit.quote_hash_hi.unwrap() + Fr::one());

    let cs = ConstraintSystem::<Fr>::new_ref();
    circuit.generate_constraints(cs.clone()).unwrap();

    assert!(
        !cs.is_satisfied().unwrap(),
        "FAIL-CLOSED VIOLATION: Tampered quote hash satisfied JoinSplitCircuit scope binding!"
    );
}

#[test]
fn test_poc_neg_joinsplit_anti_hyperbridge_leaf_count_tampered_fail_closed() {
    // Adversarial vector: Tampered leaf count manipulation (hyperbridge attack)
    let mut circuit = create_dummy_joinsplit_circuit();
    // Setting leaf_count to 0 violates strict positive leaf count invariant
    circuit.leaf_count = Some(Fr::zero());

    let cs = ConstraintSystem::<Fr>::new_ref();
    circuit.generate_constraints(cs.clone()).unwrap();

    assert!(
        !cs.is_satisfied().unwrap(),
        "FAIL-CLOSED VIOLATION: Zero leaf_count satisfied JoinSplitCircuit!"
    );
}
