//! Formal Soundness & Boundary Hardening Test Suite for DEC-041 In-Browser WASM Prover
//!
//! Implements required testing matrix from DEC-041 Section 9:
//! - Positive:
//!   * `test_wasm_private_note_prover_produces_valid_stylus_proof`
//!   * `test_wasm_joinsplit_prover_produces_valid_stylus_proof`
//!   * `test_wasm_selective_disclosure_prover_latency`
//! - Negative:
//!   * `test_wasm_prover_rejects_non_canonical_scalar_without_panic`
//!   * `test_wasm_prover_rejects_unbalanced_utxo_witness`
//!   * `test_wasm_prover_handles_corrupted_proving_key`

use ark_bls12_381::Fr;
use ark_serialize::CanonicalSerialize;
use rand::rngs::StdRng;
use rand::SeedableRng;
use std::time::Instant;

use zeltra_core::{
    decompose_challenge_to_limbs, derive_joinsplit_nullifier, derive_nullifier_key, from_evm_proof,
    from_evm_scalar, get_or_init_joinsplit_circuit_keys, get_or_init_note_circuit_keys,
    note_commitment, selective_disclosure_circuit::setup_disclosure_circuit_params,
    verify_disclosure_proof_unprepared, verify_joinsplit_proof, verify_note_proof,
    MerkleMountainRange,
};

use zeltra_sdk::{
    prove_joinsplit_internal, prove_private_note_internal, prove_selective_disclosure_internal,
    scalar_to_hex, JoinSplitWitnessInput, PrivateNoteWitnessInput, SelectiveDisclosureWitnessInput,
};

// ═══════════════════════════════════════════════════════════════════════════
// Test Fixtures
// ═══════════════════════════════════════════════════════════════════════════

fn create_valid_private_note_witness() -> (PrivateNoteWitnessInput, Vec<u8>) {
    let keys = get_or_init_note_circuit_keys();
    let mut pk_bytes = Vec::new();
    keys.proving_key
        .serialize_compressed(&mut pk_bytes)
        .unwrap();

    let sk = Fr::from(42u64);
    let in_val = 99_800_000u64;
    let aid = 1u8; // USDC
    let in_rho = Fr::from(1u64);
    let in_rand = Fr::from(2u64);

    let in_cm = note_commitment(in_val, aid, sk, in_rho, in_rand);

    let mut mmr = MerkleMountainRange::new();
    let (leaf_idx, root) = mmr.append(in_cm);
    let proof = mmr.generate_proof(leaf_idx);

    let mut m_sibs = Vec::new();
    for s in proof.mountain_siblings.iter() {
        m_sibs.push(scalar_to_hex(s));
    }

    let mut p_sibs = Vec::new();
    for s in proof.peak_bagging_siblings.iter() {
        p_sibs.push(scalar_to_hex(s));
    }

    let merch = 5_000_000u64;
    let pfee = 12_500u64;
    let efee = 23_000u64;
    let change = in_val - merch - pfee - efee;

    let ch_rho = Fr::from(3u64);
    let ch_rand = Fr::from(4u64);

    let witness = PrivateNoteWitnessInput {
        note_root_hex: scalar_to_hex(&root),
        leaf_count: mmr.leaf_count as u64,
        note_epoch_id: Some(0),
        recipient_hex: "0x0000000000000000000000000000000000000064".to_string(), // address 100
        merchant_amount: merch,
        protocol_fee: pfee,
        execution_fee: efee,
        quote_hash_hi_hex: scalar_to_hex(&Fr::from(100u64)),
        quote_hash_lo_hex: scalar_to_hex(&Fr::from(200u64)),
        chain_id: 421614,
        contract_address_hex: "0x000000000000000000000000000000000000012c".to_string(), // address 300
        expiry: 0,
        has_change: true,
        flags_packed: None,
        is_rollover: Some(false),
        asset_id: aid,

        input_value: in_val,
        input_spending_key_hex: scalar_to_hex(&sk),
        input_rho_hex: scalar_to_hex(&in_rho),
        input_randomness_hex: scalar_to_hex(&in_rand),
        input_leaf_index: leaf_idx as u64,
        mountain_height: proof.mountain_height as u8,
        mountain_siblings_hex: m_sibs,
        peak_bagging_siblings_hex: p_sibs,

        change_value: Some(change),
        change_owner_key_hex: Some(scalar_to_hex(&sk)),
        change_rho_hex: Some(scalar_to_hex(&ch_rho)),
        change_randomness_hex: Some(scalar_to_hex(&ch_rand)),
        expected_binding_hex: None,
    };

    (witness, pk_bytes)
}

fn create_valid_joinsplit_witness() -> (JoinSplitWitnessInput, Vec<u8>) {
    let keys = get_or_init_joinsplit_circuit_keys();
    let mut pk_bytes = Vec::new();
    keys.proving_key
        .serialize_compressed(&mut pk_bytes)
        .unwrap();

    let sk = Fr::from(100u64);
    let rho1 = Fr::from(200u64);
    let rand1 = Fr::from(300u64);
    let v_in1 = 10_000_000u64;
    let aid = 1u8;

    let cm1 = note_commitment(v_in1, aid, sk, rho1, rand1);
    let mut mmr = MerkleMountainRange::new();
    let (leaf_idx1, root) = mmr.append(cm1);
    let proof1 = mmr.generate_proof(leaf_idx1);

    let mut in1_sibs = Vec::new();
    for sib in proof1.mountain_siblings.iter() {
        in1_sibs.push(scalar_to_hex(sib));
    }

    let mut peaks = Vec::new();
    for p in mmr.get_peaks().iter() {
        peaks.push(scalar_to_hex(p));
    }

    let merch = 9_900_000u64;
    let pfee = (merch * 45).div_ceil(10_000); // 44_550
    let efee = v_in1 - merch - pfee; // 55_450

    let witness = JoinSplitWitnessInput {
        note_root_hex: scalar_to_hex(&root),
        leaf_count: mmr.leaf_count() as u64,
        recipient_hex: "0x0000000000000000000000000000000000001234".to_string(),
        merchant_amount: merch,
        protocol_fee: pfee,
        execution_fee: efee,
        quote_hash_hi_hex: scalar_to_hex(&Fr::from(0x1111u64)),
        quote_hash_lo_hex: scalar_to_hex(&Fr::from(0x2222u64)),
        chain_id: 421614,
        contract_address_hex: "0x0000000000000000000000000000000000005678".to_string(),
        expiry: 2000,
        flags_packed: None,
        is_rollover: Some(false),
        asset_id: aid,

        in1_value: v_in1,
        in1_spending_key_hex: scalar_to_hex(&sk),
        in1_rho_hex: scalar_to_hex(&rho1),
        in1_randomness_hex: scalar_to_hex(&rand1),
        in1_leaf_index: leaf_idx1 as u64,
        in1_mountain_height: proof1.mountain_height as u8,
        in1_mountain_siblings_hex: in1_sibs,
        in1_epoch_id: Some(10),

        in2_is_dummy: true,
        in2_value: None,
        in2_spending_key_hex: None,
        in2_rho_hex: None,
        in2_randomness_hex: None,
        in2_leaf_index: None,
        in2_mountain_height: None,
        in2_mountain_siblings_hex: None,
        in2_session_nonce_hex: Some(scalar_to_hex(&Fr::from(999u64))),
        in2_epoch_id: None,

        mmr_peaks_hex: peaks,

        out1_value: 0,
        out1_owner_key_hex: scalar_to_hex(&sk),
        out1_rho_hex: scalar_to_hex(&Fr::from(401u64)),
        out1_randomness_hex: scalar_to_hex(&Fr::from(402u64)),

        out2_value: 0,
        out2_owner_key_hex: scalar_to_hex(&sk),
        out2_rho_hex: scalar_to_hex(&Fr::from(501u64)),
        out2_randomness_hex: scalar_to_hex(&Fr::from(502u64)),

        has_change_1: false,
        has_change_2: false,
        expected_binding_hex: None,
    };

    (witness, pk_bytes)
}

fn create_valid_disclosure_witness() -> (
    SelectiveDisclosureWitnessInput,
    Vec<u8>,
    ark_groth16::VerifyingKey<ark_bls12_381::Bls12_381>,
) {
    let mut rng = StdRng::seed_from_u64(0xdec041);
    let (pk, vk) = setup_disclosure_circuit_params(&mut rng);
    let mut pk_bytes = Vec::new();
    pk.serialize_compressed(&mut pk_bytes).unwrap();

    let sk = Fr::from(1337u64);
    let epoch = 1u32;
    let owner_key = Fr::from(99999u64);
    let rho = Fr::from(42u64);

    let nk = derive_nullifier_key(sk);
    let nf = derive_joinsplit_nullifier(nk, rho, epoch as u64);

    let challenge = [0x5au8; 32];
    let (c_hi, c_lo) = decompose_challenge_to_limbs(&challenge);

    let witness = SelectiveDisclosureWitnessInput {
        spending_key_hex: scalar_to_hex(&sk),
        epoch_id: epoch,
        owner_key_hex: scalar_to_hex(&owner_key),
        rho_hex: scalar_to_hex(&rho),
        nullifier_hex: scalar_to_hex(&nf),
        challenge_hex: None,
        challenge_hi_hex: Some(scalar_to_hex(&c_hi)),
        challenge_lo_hex: Some(scalar_to_hex(&c_lo)),
    };

    (witness, pk_bytes, vk)
}

// ═══════════════════════════════════════════════════════════════════════════
// Positive Tests
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_wasm_private_note_prover_produces_valid_stylus_proof() {
    let (witness, pk_bytes) = create_valid_private_note_witness();
    let keys = get_or_init_note_circuit_keys();

    let res = prove_private_note_internal(&witness, &pk_bytes)
        .expect("prove_private_note_internal must succeed");

    // 1. Verify EVM point lengths (EIP-2537: G1 = 128 bytes = 256 hex, G2 = 256 bytes = 512 hex)
    assert_eq!(res.proof_a.len(), 2 + 256); // 0x + 256 hex chars
    assert_eq!(res.proof_b.len(), 2 + 512); // 0x + 512 hex chars
    assert_eq!(res.proof_c.len(), 2 + 256); // 0x + 256 hex chars
    assert_eq!(res.public_inputs.len(), 16);

    // 2. Parse EVM proof points back to Arkworks
    let a_clean = res.proof_a.trim_start_matches("0x");
    let b_clean = res.proof_b.trim_start_matches("0x");
    let c_clean = res.proof_c.trim_start_matches("0x");

    let a_bytes: [u8; 128] = hex::decode(a_clean).unwrap().try_into().unwrap();
    let b_bytes: [u8; 256] = hex::decode(b_clean).unwrap().try_into().unwrap();
    let c_bytes: [u8; 128] = hex::decode(c_clean).unwrap().try_into().unwrap();

    let proof = from_evm_proof(&a_bytes, &b_bytes, &c_bytes).expect("Valid EVM proof decoding");

    // 3. Parse 16 Public Inputs back to Fr scalars
    let mut pis_fr = Vec::new();
    for pi_hex in res.public_inputs.iter() {
        let clean = pi_hex.trim_start_matches("0x");
        let bytes: [u8; 32] = hex::decode(clean).unwrap().try_into().unwrap();
        let scalar = from_evm_scalar(&bytes).expect("Valid EVM scalar");
        pis_fr.push(scalar);
    }

    // 4. Verify against native Arkworks Groth16 Verifier
    let is_valid = verify_note_proof(&proof, &keys.verifying_key, &pis_fr);
    assert!(
        is_valid,
        "WASM generated PrivateNote proof must verify natively with VerifyingKey"
    );
}

#[test]
fn test_wasm_joinsplit_prover_produces_valid_stylus_proof() {
    let (witness, pk_bytes) = create_valid_joinsplit_witness();
    let keys = get_or_init_joinsplit_circuit_keys();

    let res = prove_joinsplit_internal(&witness, &pk_bytes)
        .expect("prove_joinsplit_internal must succeed");

    assert_eq!(res.proof_a.len(), 2 + 256);
    assert_eq!(res.proof_b.len(), 2 + 512);
    assert_eq!(res.proof_c.len(), 2 + 256);
    assert_eq!(res.public_inputs.len(), 19);

    let a_clean = res.proof_a.trim_start_matches("0x");
    let b_clean = res.proof_b.trim_start_matches("0x");
    let c_clean = res.proof_c.trim_start_matches("0x");

    let a_bytes: [u8; 128] = hex::decode(a_clean).unwrap().try_into().unwrap();
    let b_bytes: [u8; 256] = hex::decode(b_clean).unwrap().try_into().unwrap();
    let c_bytes: [u8; 128] = hex::decode(c_clean).unwrap().try_into().unwrap();

    let proof = from_evm_proof(&a_bytes, &b_bytes, &c_bytes).expect("Valid EVM proof decoding");

    let mut pis_fr = Vec::new();
    for pi_hex in res.public_inputs.iter() {
        let clean = pi_hex.trim_start_matches("0x");
        let bytes: [u8; 32] = hex::decode(clean).unwrap().try_into().unwrap();
        let scalar = from_evm_scalar(&bytes).expect("Valid EVM scalar");
        pis_fr.push(scalar);
    }

    let is_valid = verify_joinsplit_proof(&keys.verifying_key, &proof, &pis_fr).unwrap_or(false);
    assert!(
        is_valid,
        "WASM generated JoinSplit proof must satisfy all 19 PIs and 448 range gates"
    );
}

#[test]
fn test_wasm_selective_disclosure_prover_latency() {
    let (witness, pk_bytes, vk) = create_valid_disclosure_witness();

    let t0 = Instant::now();
    let res = prove_selective_disclosure_internal(&witness, &pk_bytes)
        .expect("prove_selective_disclosure_internal must succeed");
    let duration = t0.elapsed();

    // DEC-041 SLO: Proving time < 25 ms in release mode; in unoptimized debug test allow headroom
    assert!(
        duration.as_millis() < 2500,
        "Selective disclosure proof must be fast, took {} ms",
        duration.as_millis()
    );

    assert_eq!(res.public_inputs.len(), 5);

    let a_clean = res.proof_a.trim_start_matches("0x");
    let b_clean = res.proof_b.trim_start_matches("0x");
    let c_clean = res.proof_c.trim_start_matches("0x");

    let a_bytes: [u8; 128] = hex::decode(a_clean).unwrap().try_into().unwrap();
    let b_bytes: [u8; 256] = hex::decode(b_clean).unwrap().try_into().unwrap();
    let c_bytes: [u8; 128] = hex::decode(c_clean).unwrap().try_into().unwrap();

    let proof = from_evm_proof(&a_bytes, &b_bytes, &c_bytes).expect("Valid EVM proof decoding");

    let mut pis_fr = Vec::new();
    for pi_hex in res.public_inputs.iter() {
        let clean = pi_hex.trim_start_matches("0x");
        let bytes: [u8; 32] = hex::decode(clean).unwrap().try_into().unwrap();
        let scalar = from_evm_scalar(&bytes).expect("Valid EVM scalar");
        pis_fr.push(scalar);
    }

    let is_valid = verify_disclosure_proof_unprepared(&proof, &pis_fr, &vk).unwrap_or(false);
    assert!(
        is_valid,
        "Selective disclosure proof must verify against verifier challenge"
    );
}

#[test]
fn test_wasm_json_interface_end_to_end() {
    let (witness, pk_bytes) = create_valid_private_note_witness();
    let witness_json = serde_json::to_string(&witness).unwrap();

    let parsed_witness: PrivateNoteWitnessInput = serde_json::from_str(&witness_json)
        .expect("Witness JSON must deserialize into PrivateNoteWitnessInput");

    let result = prove_private_note_internal(&parsed_witness, &pk_bytes)
        .expect("prove_private_note_internal must succeed");

    assert_eq!(result.public_inputs.len(), 16);
    assert!(result.proof_a.starts_with("0x"));

    // Verify WasmProverResult JSON roundtrip
    let res_json = serde_json::to_string(&result).expect("Result must serialize to JSON");
    let roundtrip: zeltra_sdk::WasmProverResult = serde_json::from_str(&res_json)
        .expect("Result JSON must deserialize back to WasmProverResult");
    assert_eq!(result, roundtrip);
}

// ═══════════════════════════════════════════════════════════════════════════
// Negative Tests (Fail-Closed, Zero-Panic, Robust Boundary Hardening)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_wasm_prover_rejects_non_canonical_scalar_without_panic() {
    let (mut witness, pk_bytes) = create_valid_private_note_witness();

    // Scalar exceeding BLS12-381 Fr modulus: all 0xFF bytes
    witness.note_root_hex =
        "0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff".to_string();

    let err = prove_private_note_internal(&witness, &pk_bytes)
        .expect_err("Must reject scalar >= modulus");

    assert_eq!(err.error_type, "InvalidScalarModulus");
    assert!(err.message.contains("note_root"));
}

#[test]
fn test_wasm_prover_rejects_unbalanced_utxo_witness() {
    let (mut witness, pk_bytes) = create_valid_private_note_witness();

    // Unbalanced: input is 99,800,000 but merchant payout is claimed as 1,000,000,000
    witness.merchant_amount = 1_000_000_000;

    let err = prove_private_note_internal(&witness, &pk_bytes)
        .expect_err("Must fail when value conservation is violated");

    assert_eq!(err.error_type, "ConstraintUnsatisfied");
}

#[test]
fn test_wasm_prover_handles_corrupted_proving_key() {
    let (witness, _) = create_valid_private_note_witness();

    // Corrupted arbitrary random bytes
    let corrupted_pk = vec![0xde, 0xad, 0xbe, 0xef, 0x01, 0x02, 0x03, 0x04];

    let err = prove_private_note_internal(&witness, &corrupted_pk)
        .expect_err("Must fail gracefully on corrupted proving key");

    assert_eq!(err.error_type, "DeserializationFailure");
}
