//! Verifying Key Sanity & Statement Binding Assurance (DEC-035B Gate B / Task B2)
//!
//! Asserts that:
//! 1. All verifying key elements IC[i] != O_G1 (Point at infinity)
//! 2. All 6 contextual public inputs occupy non-zero entries in QAP constraint matrices

use ark_bls12_381::Fr;
use ark_ec::AffineRepr;
use ark_ff::Zero;
use ark_relations::gr1cs::{ConstraintSynthesizer, ConstraintSystem};
use nimbus_core::{create_dummy_circuit, get_or_init_note_circuit_keys, NUM_PUBLIC_INPUTS};

#[test]
fn test_verifying_key_non_identity() {
    let keys = get_or_init_note_circuit_keys();
    let vk = &keys.verifying_key;

    assert_eq!(
        vk.gamma_abc_g1.len(),
        NUM_PUBLIC_INPUTS + 1,
        "gamma_abc_g1 must contain exactly NUM_PUBLIC_INPUTS + 1 elements (IC[0..=16])"
    );

    // INV-1: Assert that NO element in IC is the point at infinity (identity point)
    for (i, ic_point) in vk.gamma_abc_g1.iter().enumerate() {
        assert!(
            !ic_point.is_zero(),
            "Verifying key element IC[{}] MUST NOT be the point at infinity (identity point)!",
            i
        );
    }

    // Specifically verify contextual public input wires (1-indexed in gamma_abc_g1):
    // Public input 5 (recipient) -> IC[6]
    // Public input 9 (quote_hash_hi) -> IC[10]
    // Public input 10 (quote_hash_lo) -> IC[11]
    // Public input 11 (chain_id) -> IC[12]
    // Public input 12 (contract_address) -> IC[13]
    // Public input 13 (expiry) -> IC[14]
    let contextual_indices = [
        (6, "recipient"),
        (10, "quote_hash_hi"),
        (11, "quote_hash_lo"),
        (12, "chain_id"),
        (13, "contract_address"),
        (14, "expiry"),
    ];

    for (idx, name) in contextual_indices {
        let pt = &vk.gamma_abc_g1[idx];
        assert!(
            !pt.is_zero(),
            "Contextual public input '{}' at IC[{}] is identity point!",
            name,
            idx
        );
    }
}

#[test]
fn test_qap_density_contextual_inputs() {
    use ark_relations::gr1cs::SynthesisMode;

    let cs = ConstraintSystem::<Fr>::new_ref();
    cs.set_mode(SynthesisMode::Prove {
        construct_matrices: true,
        generate_lc_assignments: true,
    });
    let circuit = create_dummy_circuit();
    circuit
        .generate_constraints(cs.clone())
        .expect("Constraint generation must succeed");

    assert!(
        cs.is_satisfied().unwrap(),
        "Dummy circuit must be satisfied"
    );

    cs.inline_all_lcs();
    let matrices = cs
        .to_matrices()
        .expect("Must extract R1CS constraint matrices");

    // In Arkworks ConstraintSystem:
    // Index 0: Constant ONE
    // Index 1..=NUM_PUBLIC_INPUTS: Public inputs
    // Check all 6 contextual public inputs:
    let target_vars = [
        (6, "recipient"),
        (10, "quote_hash_hi"),
        (11, "quote_hash_lo"),
        (12, "chain_id"),
        (13, "contract_address"),
        (14, "expiry"),
    ];

    for (var_idx, name) in target_vars {
        let mut total_occurrences = 0;

        for matrix_list in matrices.values() {
            for matrix in matrix_list {
                for row in matrix {
                    for (coeff, col) in row {
                        if *col == var_idx && !coeff.is_zero() {
                            total_occurrences += 1;
                        }
                    }
                }
            }
        }

        assert!(
            total_occurrences > 0,
            "Contextual input '{}' (var index {}) has ZERO entries in QAP matrices! QAP density violated!",
            name,
            var_idx
        );
    }
}

#[test]
fn test_recipient_substitution_attack_rejected() {
    // B6.1: Mutate recipient in public inputs without regenerating proof -> MUST fail verification
    let keys = get_or_init_note_circuit_keys();
    let circuit = create_dummy_circuit();
    let mut public_inputs = nimbus_core::extract_public_inputs(&circuit);

    let proof = nimbus_core::generate_note_proof(circuit, &keys.proving_key).unwrap();

    // Attacker substitutes recipient (public input index 5) with attacker address
    public_inputs[5] = Fr::from(0xdeadbeefu64);

    let is_valid = nimbus_core::verify_note_proof(&proof, &keys.verifying_key, &public_inputs);
    assert!(
        !is_valid,
        "Recipient substitution attack MUST be rejected by Groth16 verifier!"
    );
}

#[test]
fn test_quote_hash_substitution_rejected() {
    // B6.2: Mutate quote_hash_hi or quote_hash_lo in public inputs -> MUST fail verification
    let keys = get_or_init_note_circuit_keys();
    let circuit = create_dummy_circuit();
    let public_inputs = nimbus_core::extract_public_inputs(&circuit);

    let proof = nimbus_core::generate_note_proof(circuit, &keys.proving_key).unwrap();

    // Mutate quote_hash_hi (public input index 9)
    let mut pi_mutated_hi = public_inputs.clone();
    pi_mutated_hi[9] += Fr::from(1u64);
    assert!(
        !nimbus_core::verify_note_proof(&proof, &keys.verifying_key, &pi_mutated_hi),
        "Quote hash hi substitution MUST be rejected!"
    );

    // Mutate quote_hash_lo (public input index 10)
    let mut pi_mutated_lo = public_inputs.clone();
    pi_mutated_lo[10] += Fr::from(1u64);
    assert!(
        !nimbus_core::verify_note_proof(&proof, &keys.verifying_key, &pi_mutated_lo),
        "Quote hash lo substitution MUST be rejected!"
    );
}

#[test]
fn test_cross_chain_replay_rejected() {
    // B6.3: Mutate chain_id (e.g. 421614 -> 1) in public inputs -> MUST fail verification
    let keys = get_or_init_note_circuit_keys();
    let circuit = create_dummy_circuit();
    let mut public_inputs = nimbus_core::extract_public_inputs(&circuit);

    let proof = nimbus_core::generate_note_proof(circuit, &keys.proving_key).unwrap();

    // Public input index 11 is chain_id
    assert_eq!(public_inputs[11], Fr::from(421614u64));
    public_inputs[11] = Fr::from(1u64); // Ethereum Mainnet

    let is_valid = nimbus_core::verify_note_proof(&proof, &keys.verifying_key, &public_inputs);
    assert!(
        !is_valid,
        "Cross-chain replay attack (mutated chain_id) MUST be rejected by Groth16 verifier!"
    );
}

#[test]
fn test_tampered_expected_binding_witness_fails_proving() {
    // Malicious witness injection: providing an expected_binding that does not match computed binding_cm
    let cs = ConstraintSystem::<Fr>::new_ref();
    let mut circuit = create_dummy_circuit();
    circuit.expected_binding = Some(Fr::from(0xbadf00du64));

    circuit
        .generate_constraints(cs.clone())
        .expect("Synthesis must run");
    assert!(
        !cs.is_satisfied().unwrap(),
        "Tampered expected_binding witness MUST fail R1CS constraint satisfaction!"
    );
}
