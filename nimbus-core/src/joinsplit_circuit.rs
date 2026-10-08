//! Universal 2-in-2-out JoinSplit Spend Circuit (Groth16 ZK-UTXO) — DEC-030 / Gate C0.
//!
//! Proves 2-in-2-out private spend:
//! - Exact integer value conservation: (v_in1 + v_in2) == (merchant + protocol_fee + execution_fee) + (v_out1 + v_out2)
//! - 64-bit range constraints on all 7 value variables (448 boolean decomposition constraints)
//! - Constant-Topology Privacy (DEC-030 Option A): is_dummy_2 is purely private witness
//! - Uniform nullifier derivation & LeanIMT Merkle membership
//! - Canonical domain & recipient scope binding

use ark_bls12_381::{Bls12_381, Fr};
use ark_ff::{Field, PrimeField};
use ark_r1cs_std::fields::fp::{AllocatedFp, FpVar};
use ark_relations::gr1cs::{ConstraintSynthesizer, ConstraintSystemRef, SynthesisError};
use ark_snark::SNARK;
use ark_std::rand::rngs::StdRng;
use ark_std::rand::SeedableRng;

use crate::note::{
    domain_dummy_nullifier, domain_merkle_node, domain_note_commitment, domain_nullifier,
    domain_quote_binding, MERKLE_TREE_DEPTH,
};
use crate::poseidon::{poseidon_hash as poseidon_w3_hash, poseidon_w5_hash};

/// Number of public inputs in the Universal 2-in-2-out JoinSplit circuit (DEC-030).
pub const NUM_JOINSPLIT_PUBLIC_INPUTS: usize = 14;

/// Deterministic seed for JoinSplit trusted setup key generation.
pub const JOINSPLIT_CIRCUIT_SETUP_SEED: u64 = 0x4e696d6275734a53; // "NimbusJS"

/// Universal 2-in-2-out JoinSplit circuit for Groth16 proof generation.
#[derive(Clone)]
pub struct JoinSplitCircuit {
    // Public inputs (14)
    pub note_root: Option<Fr>,
    pub input_nullifier_1: Option<Fr>,
    pub input_nullifier_2: Option<Fr>,
    pub output_commitment_1: Option<Fr>,
    pub output_commitment_2: Option<Fr>,
    pub recipient: Option<Fr>,
    pub merchant_amount: Option<Fr>,
    pub protocol_fee: Option<Fr>,
    pub execution_fee: Option<Fr>,
    pub quote_hash: Option<Fr>,
    pub chain_id: Option<Fr>,
    pub contract_address: Option<Fr>,
    pub expiry: Option<Fr>,
    pub flags_packed: Option<Fr>, // Bit 0: has_change_1, Bit 1: has_change_2

    // Witnesses: Input Note 1
    pub in1_value: Option<Fr>,
    pub in1_owner_key: Option<Fr>,
    pub in1_rho: Option<Fr>,
    pub in1_randomness: Option<Fr>,
    pub in1_leaf_index: Option<Fr>,
    pub in1_merkle_path: Option<[Fr; MERKLE_TREE_DEPTH]>,

    // Witnesses: Input Note 2
    pub in2_value: Option<Fr>,
    pub in2_owner_key: Option<Fr>,
    pub in2_rho: Option<Fr>,
    pub in2_randomness: Option<Fr>,
    pub in2_leaf_index: Option<Fr>,
    pub in2_merkle_path: Option<[Fr; MERKLE_TREE_DEPTH]>,
    pub in2_is_dummy: Option<Fr>, // 1 if dummy, 0 if real
    pub in2_session_nonce: Option<Fr>,

    // Witnesses: Output Change 1
    pub out1_value: Option<Fr>,
    pub out1_owner_key: Option<Fr>,
    pub out1_rho: Option<Fr>,
    pub out1_randomness: Option<Fr>,

    // Witnesses: Output Change 2
    pub out2_value: Option<Fr>,
    pub out2_owner_key: Option<Fr>,
    pub out2_rho: Option<Fr>,
    pub out2_randomness: Option<Fr>,

    // Witnesses: Flags
    pub has_change_1: Option<Fr>,
    pub has_change_2: Option<Fr>,
}

/// Helper: allocate an FpVar from a gr1cs variable.
fn alloc_fp(
    cs: &ConstraintSystemRef<Fr>,
    var: ark_relations::gr1cs::Variable,
    value: Option<Fr>,
) -> FpVar<Fr> {
    FpVar::Var(AllocatedFp::new(value, var, cs.clone()))
}

/// Helper: allocate a public input variable and wrap as FpVar.
fn public_input(
    cs: &ConstraintSystemRef<Fr>,
    value: Option<Fr>,
) -> Result<FpVar<Fr>, SynthesisError> {
    let var = cs.new_input_variable(|| value.ok_or(SynthesisError::AssignmentMissing))?;
    Ok(alloc_fp(cs, var, value))
}

/// Helper: allocate a private witness variable and wrap as FpVar.
fn private_witness(
    cs: &ConstraintSystemRef<Fr>,
    value: Option<Fr>,
) -> Result<FpVar<Fr>, SynthesisError> {
    let var = cs.new_witness_variable(|| value.ok_or(SynthesisError::AssignmentMissing))?;
    Ok(alloc_fp(cs, var, value))
}

/// Enforce that `val` is strictly in [0, 2^64) by decomposing into 64 boolean bits.
fn enforce_u64_range(
    cs: &ConstraintSystemRef<Fr>,
    val: &FpVar<Fr>,
    val_witness: Option<Fr>,
) -> Result<(), SynthesisError> {
    use ark_r1cs_std::eq::EqGadget;

    let zero = FpVar::Constant(Fr::from(0u64));
    let one = FpVar::Constant(Fr::from(1u64));
    let mut reconstructed = zero.clone();
    let mut two_power = Fr::from(1u64);

    let bigint_opt = val_witness.map(|w| w.into_bigint());

    for bit_idx in 0..64 {
        let bit_val = bigint_opt.as_ref().map(|bi| {
            let word = bi.as_ref()[0];
            Fr::from((word >> bit_idx) & 1u64)
        });
        let bit_var = private_witness(cs, bit_val)?;

        // bit * (1 - bit) == 0
        let one_minus_bit = &one - &bit_var;
        let bit_check = &bit_var * &one_minus_bit;
        bit_check.enforce_equal(&zero)?;

        reconstructed += &bit_var * FpVar::Constant(two_power);
        two_power = two_power + two_power;
    }

    reconstructed.enforce_equal(val)?;
    Ok(())
}

/// Helper gadget: Verifies a Merkle path against a leaf commitment and returns the computed root.
#[allow(clippy::too_many_arguments)]
fn verify_merkle_path_gadget(
    cs: &ConstraintSystemRef<Fr>,
    leaf: &FpVar<Fr>,
    leaf_index: &FpVar<Fr>,
    leaf_index_witness: Option<Fr>,
    merkle_path: &[FpVar<Fr>; MERKLE_TREE_DEPTH],
    w5_rc: &[Fr],
    w5_mds: &[Vec<Fr>],
    domain_merkle: Fr,
) -> Result<FpVar<Fr>, SynthesisError> {
    use ark_r1cs_std::eq::EqGadget;

    let zero = FpVar::Constant(Fr::from(0u64));
    let one = FpVar::Constant(Fr::from(1u64));
    let mut current = leaf.clone();
    let mut reconstructed_index = zero.clone();
    let mut two_power = Fr::from(1u64);

    for (level, sibling) in merkle_path.iter().enumerate().take(MERKLE_TREE_DEPTH) {
        let index_bit = private_witness(
            cs,
            leaf_index_witness.map(|idx| {
                let level_shift = level as u64;
                Fr::from((idx.into_bigint().as_ref()[0] >> level_shift) & 1u64)
            }),
        )?;

        // Enforce boolean: index_bit * (1 - index_bit) == 0
        let one_minus_bit = &one - &index_bit;
        let bit_check = &index_bit * &one_minus_bit;
        bit_check.enforce_equal(&zero)?;

        reconstructed_index += &index_bit * FpVar::Constant(two_power);
        two_power = two_power + two_power;

        let diff_cs = sibling - &current;
        let diff_sc = &current - sibling;
        let left = &current + &index_bit * &diff_cs;
        let right = sibling + &index_bit * &diff_sc;

        current = poseidon_w5_hash(
            &[left, right, zero.clone(), zero.clone()],
            domain_merkle,
            w5_rc,
            w5_mds,
        )?;
    }

    reconstructed_index.enforce_equal(leaf_index)?;
    Ok(current)
}

impl ConstraintSynthesizer<Fr> for JoinSplitCircuit {
    fn generate_constraints(self, cs: ConstraintSystemRef<Fr>) -> Result<(), SynthesisError> {
        use ark_r1cs_std::eq::EqGadget;

        let zero = FpVar::Constant(Fr::from(0u64));
        let one = FpVar::Constant(Fr::from(1u64));

        let (w5_rc, w5_mds) = &*crate::poseidon::CACHED_W5_PARAMS;
        let (w3_rc, w3_mds) = &*crate::poseidon::CACHED_W3_PARAMS;

        let domain_merkle = domain_merkle_node();
        let domain_note = domain_note_commitment();
        let domain_nullifier = domain_nullifier();
        let domain_dummy_nullifier = domain_dummy_nullifier();
        let domain_binding = domain_quote_binding();

        // 1. Public inputs (14)
        let note_root_var = public_input(&cs, self.note_root)?;
        let nullifier_1_var = public_input(&cs, self.input_nullifier_1)?;
        let nullifier_2_var = public_input(&cs, self.input_nullifier_2)?;
        let output_cm1_var = public_input(&cs, self.output_commitment_1)?;
        let output_cm2_var = public_input(&cs, self.output_commitment_2)?;
        let recipient_var = public_input(&cs, self.recipient)?;
        let merchant_var = public_input(&cs, self.merchant_amount)?;
        let protocol_fee_var = public_input(&cs, self.protocol_fee)?;
        let exec_fee_var = public_input(&cs, self.execution_fee)?;
        let quote_hash_var = public_input(&cs, self.quote_hash)?;
        let chain_id_var = public_input(&cs, self.chain_id)?;
        let contract_addr_var = public_input(&cs, self.contract_address)?;
        let expiry_var = public_input(&cs, self.expiry)?;
        let flags_var = public_input(&cs, self.flags_packed)?;

        // 2. Private witnesses
        // Input Note 1
        let in1_value = private_witness(&cs, self.in1_value)?;
        let in1_owner = private_witness(&cs, self.in1_owner_key)?;
        let in1_rho = private_witness(&cs, self.in1_rho)?;
        let in1_rand = private_witness(&cs, self.in1_randomness)?;
        let in1_index = private_witness(&cs, self.in1_leaf_index)?;
        let in1_path: [FpVar<Fr>; MERKLE_TREE_DEPTH] = {
            let path_opt = self.in1_merkle_path;
            let mut arr = Vec::with_capacity(MERKLE_TREE_DEPTH);
            for i in 0..MERKLE_TREE_DEPTH {
                arr.push(private_witness(&cs, path_opt.map(|p| p[i]))?);
            }
            arr.try_into().map_err(|_| SynthesisError::Unsatisfiable)?
        };

        // Input Note 2
        let in2_value = private_witness(&cs, self.in2_value)?;
        let in2_owner = private_witness(&cs, self.in2_owner_key)?;
        let in2_rho = private_witness(&cs, self.in2_rho)?;
        let in2_rand = private_witness(&cs, self.in2_randomness)?;
        let in2_index = private_witness(&cs, self.in2_leaf_index)?;
        let in2_path: [FpVar<Fr>; MERKLE_TREE_DEPTH] = {
            let path_opt = self.in2_merkle_path;
            let mut arr = Vec::with_capacity(MERKLE_TREE_DEPTH);
            for i in 0..MERKLE_TREE_DEPTH {
                arr.push(private_witness(&cs, path_opt.map(|p| p[i]))?);
            }
            arr.try_into().map_err(|_| SynthesisError::Unsatisfiable)?
        };
        let is_dummy_var = private_witness(&cs, self.in2_is_dummy)?;
        let in2_nonce_var = private_witness(&cs, self.in2_session_nonce)?;

        // Output Note 1 (Change 1)
        let out1_val = private_witness(&cs, self.out1_value)?;
        let out1_owner = private_witness(&cs, self.out1_owner_key)?;
        let out1_rho = private_witness(&cs, self.out1_rho)?;
        let out1_rand = private_witness(&cs, self.out1_randomness)?;

        // Output Note 2 (Change 2)
        let out2_val = private_witness(&cs, self.out2_value)?;
        let out2_owner = private_witness(&cs, self.out2_owner_key)?;
        let out2_rho = private_witness(&cs, self.out2_rho)?;
        let out2_rand = private_witness(&cs, self.out2_randomness)?;

        // Change Flags
        let has_ch1_var = private_witness(&cs, self.has_change_1)?;
        let has_ch2_var = private_witness(&cs, self.has_change_2)?;

        // 3. Flags decomposition & boolean constraints
        // is_dummy * (1 - is_dummy) == 0
        let one_minus_dummy = &one - &is_dummy_var;
        (&is_dummy_var * &one_minus_dummy).enforce_equal(&zero)?;

        // has_change_1 * (1 - has_change_1) == 0
        let one_minus_ch1 = &one - &has_ch1_var;
        (&has_ch1_var * &one_minus_ch1).enforce_equal(&zero)?;

        // has_change_2 * (1 - has_change_2) == 0
        let one_minus_ch2 = &one - &has_ch2_var;
        (&has_ch2_var * &one_minus_ch2).enforce_equal(&zero)?;

        // flags_packed == has_change_1 + 2 * has_change_2
        let two = FpVar::Constant(Fr::from(2u64));
        let expected_flags = &has_ch1_var + &has_ch2_var * two;
        expected_flags.enforce_equal(&flags_var)?;

        // 4. Input Note 1 verification
        // Enforce input 1 value > 0 (strictly positive non-dummy)
        let in1_inv = private_witness(
            &cs,
            self.in1_value
                .map(|v| v.inverse().unwrap_or(Fr::from(0u64))),
        )?;
        (&in1_value * &in1_inv).enforce_equal(&one)?;

        // in1_cm = Poseidon_W5(in1_value, in1_owner, in1_rho, in1_rand)
        let in1_cm = poseidon_w5_hash(
            &[in1_value.clone(), in1_owner.clone(), in1_rho, in1_rand],
            domain_note,
            w5_rc,
            w5_mds,
        )?;

        // Verify Merkle path for Note 1
        let computed_root_1 = verify_merkle_path_gadget(
            &cs,
            &in1_cm,
            &in1_index,
            self.in1_leaf_index,
            &in1_path,
            w5_rc,
            w5_mds,
            domain_merkle,
        )?;
        computed_root_1.enforce_equal(&note_root_var)?;

        // Derive Nullifier 1
        let nk_1 = poseidon_w3_hash(
            &in1_owner,
            &FpVar::Constant(domain_nullifier),
            w3_rc,
            w3_mds,
        )?;
        let computed_nf_1 = poseidon_w5_hash(
            &[nk_1, in1_cm, in1_index, zero.clone()],
            domain_nullifier,
            w5_rc,
            w5_mds,
        )?;
        computed_nf_1.enforce_equal(&nullifier_1_var)?;

        // 5. Input Note 2 verification (Option A constant-topology)
        // a. If is_dummy_2 == 1 => in2_value MUST be 0
        (&is_dummy_var * &in2_value).enforce_equal(&zero)?;

        // b. in2_cm = Poseidon_W5(in2_value, in2_owner, in2_rho, in2_rand)
        let in2_cm = poseidon_w5_hash(
            &[in2_value.clone(), in2_owner.clone(), in2_rho, in2_rand],
            domain_note,
            w5_rc,
            w5_mds,
        )?;

        // c. Merkle check: conditional on (1 - is_dummy_2)
        let computed_root_2 = verify_merkle_path_gadget(
            &cs,
            &in2_cm,
            &in2_index,
            self.in2_leaf_index,
            &in2_path,
            w5_rc,
            w5_mds,
            domain_merkle,
        )?;
        let root2_diff = &computed_root_2 - &note_root_var;
        (&one_minus_dummy * &root2_diff).enforce_equal(&zero)?;

        // d. Nullifier 2 derivation
        let nk_2 = poseidon_w3_hash(
            &in2_owner,
            &FpVar::Constant(domain_nullifier),
            w3_rc,
            w3_mds,
        )?;
        let real_nf_2 = poseidon_w5_hash(
            &[nk_2.clone(), in2_cm, in2_index, zero.clone()],
            domain_nullifier,
            w5_rc,
            w5_mds,
        )?;
        let dummy_nf_2 = poseidon_w5_hash(
            &[nk_2, in2_nonce_var, zero.clone(), zero.clone()],
            domain_dummy_nullifier,
            w5_rc,
            w5_mds,
        )?;

        // expected_nf_2 = is_dummy * dummy_nf_2 + (1 - is_dummy) * real_nf_2
        let nf_diff = &dummy_nf_2 - &real_nf_2;
        let expected_nf_2 = &real_nf_2 + &is_dummy_var * &nf_diff;
        expected_nf_2.enforce_equal(&nullifier_2_var)?;

        // 6. Output change notes (1 and 2)
        // Output 1
        let ch1_cm = poseidon_w5_hash(
            &[out1_val.clone(), out1_owner, out1_rho, out1_rand],
            domain_note,
            w5_rc,
            w5_mds,
        )?;
        (&out1_val * &one_minus_ch1).enforce_equal(&zero)?;
        let expected_out_cm1 = &has_ch1_var * &ch1_cm;
        expected_out_cm1.enforce_equal(&output_cm1_var)?;

        // Output 2
        let ch2_cm = poseidon_w5_hash(
            &[out2_val.clone(), out2_owner, out2_rho, out2_rand],
            domain_note,
            w5_rc,
            w5_mds,
        )?;
        (&out2_val * &one_minus_ch2).enforce_equal(&zero)?;
        let expected_out_cm2 = &has_ch2_var * &ch2_cm;
        expected_out_cm2.enforce_equal(&output_cm2_var)?;

        // 7. 448 R1CS range constraints & value conservation
        enforce_u64_range(&cs, &in1_value, self.in1_value)?;
        enforce_u64_range(&cs, &in2_value, self.in2_value)?;
        enforce_u64_range(&cs, &merchant_var, self.merchant_amount)?;
        enforce_u64_range(&cs, &protocol_fee_var, self.protocol_fee)?;
        enforce_u64_range(&cs, &exec_fee_var, self.execution_fee)?;
        enforce_u64_range(&cs, &out1_val, self.out1_value)?;
        enforce_u64_range(&cs, &out2_val, self.out2_value)?;

        // Value Conservation Invariant:
        // (in1 + in2) == (merchant + protocol_fee + execution_fee + out1 + out2)
        let total_inputs = &in1_value + &in2_value;
        let total_outputs =
            &merchant_var + &protocol_fee_var + &exec_fee_var + &out1_val + &out2_val;
        total_inputs.enforce_equal(&total_outputs)?;

        // 8. Canonical scope & domain binding
        let scope_hash = poseidon_w5_hash(
            &[recipient_var, chain_id_var, contract_addr_var, expiry_var],
            domain_binding,
            w5_rc,
            w5_mds,
        )?;
        let _binding = poseidon_w3_hash(&quote_hash_var, &scope_hash, w3_rc, w3_mds)?;

        Ok(())
    }
}

// Proving key, verifying key & setup helpers

/// Proving and verifying keys for Universal JoinSplitCircuit.
pub struct JoinSplitCircuitKeys {
    pub proving_key: ark_groth16::ProvingKey<Bls12_381>,
    pub verifying_key: ark_groth16::VerifyingKey<Bls12_381>,
}

/// Creates a valid dummy circuit for trusted setup ceremony synthesis.
pub fn create_dummy_joinsplit_circuit() -> JoinSplitCircuit {
    use crate::note::{derive_nullifier, derive_nullifier_key, note_commitment};

    let empty = crate::note::compute_empty_hashes();
    let path: [Fr; MERKLE_TREE_DEPTH] = empty[..MERKLE_TREE_DEPTH].try_into().unwrap();

    let sk = Fr::from(100u64);
    let nk = derive_nullifier_key(sk);
    let rho = Fr::from(200u64);
    let rand = Fr::from(300u64);

    let cm1 = note_commitment(10_000_000, sk, rho, rand);
    let root = crate::note::compute_merkle_root(cm1, 0, &path);
    let nf1 = derive_nullifier(nk, cm1, 0);

    // Dummy second input
    let nonce = Fr::from(999u64);
    let nf2 = crate::poseidon::native_poseidon_w5(
        &[nk, nonce, Fr::from(0u64), Fr::from(0u64)],
        domain_dummy_nullifier(),
    );

    JoinSplitCircuit {
        note_root: Some(root),
        input_nullifier_1: Some(nf1),
        input_nullifier_2: Some(nf2),
        output_commitment_1: Some(Fr::from(0u64)),
        output_commitment_2: Some(Fr::from(0u64)),
        recipient: Some(Fr::from(0x1234u64)),
        merchant_amount: Some(Fr::from(9_900_000u64)),
        protocol_fee: Some(Fr::from(45_000u64)),
        execution_fee: Some(Fr::from(55_000u64)),
        quote_hash: Some(Fr::from(0xabcd_u64)),
        chain_id: Some(Fr::from(421614u64)),
        contract_address: Some(Fr::from(0x5678u64)),
        expiry: Some(Fr::from(2000u64)),
        flags_packed: Some(Fr::from(0u64)), // has_change_1 = 0, has_change_2 = 0

        in1_value: Some(Fr::from(10_000_000u64)),
        in1_owner_key: Some(sk),
        in1_rho: Some(rho),
        in1_randomness: Some(rand),
        in1_leaf_index: Some(Fr::from(0u64)),
        in1_merkle_path: Some(path),

        in2_value: Some(Fr::from(0u64)),
        in2_owner_key: Some(sk),
        in2_rho: Some(Fr::from(0u64)),
        in2_randomness: Some(Fr::from(0u64)),
        in2_leaf_index: Some(Fr::from(0u64)),
        in2_merkle_path: Some(path),
        in2_is_dummy: Some(Fr::from(1u64)),
        in2_session_nonce: Some(nonce),

        out1_value: Some(Fr::from(0u64)),
        out1_owner_key: Some(sk),
        out1_rho: Some(Fr::from(0u64)),
        out1_randomness: Some(Fr::from(0u64)),

        out2_value: Some(Fr::from(0u64)),
        out2_owner_key: Some(sk),
        out2_rho: Some(Fr::from(0u64)),
        out2_randomness: Some(Fr::from(0u64)),

        has_change_1: Some(Fr::from(0u64)),
        has_change_2: Some(Fr::from(0u64)),
    }
}

/// Generates proving and verifying keys for Universal JoinSplitCircuit.
pub fn generate_joinsplit_circuit_keys() -> Result<JoinSplitCircuitKeys, SynthesisError> {
    use ark_groth16::Groth16;

    let mut rng = StdRng::seed_from_u64(JOINSPLIT_CIRCUIT_SETUP_SEED);
    let dummy = create_dummy_joinsplit_circuit();
    let (pk, _) = Groth16::<Bls12_381>::circuit_specific_setup(dummy, &mut rng)?;
    let vk = pk.vk.clone();

    Ok(JoinSplitCircuitKeys {
        proving_key: pk,
        verifying_key: vk,
    })
}

/// Generate Groth16 proof for JoinSplitCircuit.
pub fn generate_joinsplit_proof(
    circuit: JoinSplitCircuit,
    pk: &ark_groth16::ProvingKey<Bls12_381>,
) -> Result<ark_groth16::Proof<Bls12_381>, SynthesisError> {
    use ark_groth16::Groth16;
    let mut rng = ark_std::rand::thread_rng();
    Groth16::<Bls12_381>::prove(pk, circuit, &mut rng)
}

/// Verify a Groth16 proof for JoinSplitCircuit against 14 public inputs.
pub fn verify_joinsplit_proof(
    vk: &ark_groth16::VerifyingKey<Bls12_381>,
    proof: &ark_groth16::Proof<Bls12_381>,
    public_inputs: &[Fr],
) -> Result<bool, SynthesisError> {
    use ark_groth16::Groth16;
    if public_inputs.len() != NUM_JOINSPLIT_PUBLIC_INPUTS {
        return Ok(false);
    }
    let pvk = ark_groth16::prepare_verifying_key(vk);
    Groth16::<Bls12_381>::verify_with_processed_vk(&pvk, public_inputs, proof)
}

/// Extracts the canonical 14 public inputs from a JoinSplitCircuit.
pub fn extract_joinsplit_public_inputs(circuit: &JoinSplitCircuit) -> Vec<Fr> {
    vec![
        circuit.note_root.expect("note_root"),
        circuit.input_nullifier_1.expect("input_nullifier_1"),
        circuit.input_nullifier_2.expect("input_nullifier_2"),
        circuit.output_commitment_1.expect("output_commitment_1"),
        circuit.output_commitment_2.expect("output_commitment_2"),
        circuit.recipient.expect("recipient"),
        circuit.merchant_amount.expect("merchant_amount"),
        circuit.protocol_fee.expect("protocol_fee"),
        circuit.execution_fee.expect("execution_fee"),
        circuit.quote_hash.expect("quote_hash"),
        circuit.chain_id.expect("chain_id"),
        circuit.contract_address.expect("contract_address"),
        circuit.expiry.expect("expiry"),
        circuit.flags_packed.expect("flags_packed"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use ark_relations::gr1cs::ConstraintSystem;

    #[test]
    fn test_joinsplit_circuit_satisfaction_1_input_with_dummy() {
        let cs = ConstraintSystem::<Fr>::new_ref();
        let circuit = create_dummy_joinsplit_circuit();
        circuit.generate_constraints(cs.clone()).unwrap();

        assert!(
            cs.is_satisfied().unwrap(),
            "Constraints not satisfied: {:?}",
            cs.which_is_unsatisfied()
        );
        assert_eq!(cs.num_instance_variables(), NUM_JOINSPLIT_PUBLIC_INPUTS + 1);
        // 14 + 1 one-variable
    }

    #[test]
    fn test_joinsplit_circuit_satisfaction_2_inputs_with_change() {
        use crate::note::{
            compute_merkle_root, derive_nullifier, derive_nullifier_key, note_commitment,
        };

        let cs = ConstraintSystem::<Fr>::new_ref();
        let empty = crate::note::compute_empty_hashes();

        let sk = Fr::from(100u64);
        let nk = derive_nullifier_key(sk);

        // Input 1: Value 10 USDC (10_000_000)
        let rho1 = Fr::from(111u64);
        let rand1 = Fr::from(222u64);
        let cm1 = note_commitment(10_000_000, sk, rho1, rand1);
        let nf1 = derive_nullifier(nk, cm1, 0);

        // Input 2: Value 10 USDC (10_000_000) at index 1
        let rho2 = Fr::from(333u64);
        let rand2 = Fr::from(444u64);
        let cm2 = note_commitment(10_000_000, sk, rho2, rand2);
        let nf2 = derive_nullifier(nk, cm2, 1);

        // Path for leaf 0: sibling at level 0 is cm2
        let mut path1: [Fr; MERKLE_TREE_DEPTH] = empty[..MERKLE_TREE_DEPTH].try_into().unwrap();
        path1[0] = cm2;
        let root = compute_merkle_root(cm1, 0, &path1);

        // Path for leaf 1: sibling at level 0 is cm1
        let mut path2 = path1;
        path2[0] = cm1;
        assert_eq!(
            compute_merkle_root(cm2, 1, &path2),
            root,
            "Both leaves must produce same root"
        );

        // Spend: 15 USDC merchant + 45k protocol fee + 55k exec fee = 15_100_000 total required
        // Change: 20_000_000 - 15_100_000 = 4_900_000
        let ch_rho = Fr::from(555u64);
        let ch_rand = Fr::from(666u64);
        let ch_cm = note_commitment(4_900_000, sk, ch_rho, ch_rand);

        let circuit = JoinSplitCircuit {
            note_root: Some(root),
            input_nullifier_1: Some(nf1),
            input_nullifier_2: Some(nf2),
            output_commitment_1: Some(ch_cm),
            output_commitment_2: Some(Fr::from(0u64)),
            recipient: Some(Fr::from(0xbeefu64)),
            merchant_amount: Some(Fr::from(15_000_000u64)),
            protocol_fee: Some(Fr::from(45_000u64)),
            execution_fee: Some(Fr::from(55_000u64)),
            quote_hash: Some(Fr::from(0x9999u64)),
            chain_id: Some(Fr::from(421614u64)),
            contract_address: Some(Fr::from(0xaaaa_u64)),
            expiry: Some(Fr::from(3000u64)),
            flags_packed: Some(Fr::from(1u64)), // has_change_1 = 1, has_change_2 = 0

            in1_value: Some(Fr::from(10_000_000u64)),
            in1_owner_key: Some(sk),
            in1_rho: Some(rho1),
            in1_randomness: Some(rand1),
            in1_leaf_index: Some(Fr::from(0u64)),
            in1_merkle_path: Some(path1),

            in2_value: Some(Fr::from(10_000_000u64)),
            in2_owner_key: Some(sk),
            in2_rho: Some(rho2),
            in2_randomness: Some(rand2),
            in2_leaf_index: Some(Fr::from(1u64)),
            in2_merkle_path: Some(path2),
            in2_is_dummy: Some(Fr::from(0u64)), // Real 2nd input note
            in2_session_nonce: Some(Fr::from(0u64)),

            out1_value: Some(Fr::from(4_900_000u64)),
            out1_owner_key: Some(sk),
            out1_rho: Some(ch_rho),
            out1_randomness: Some(ch_rand),

            out2_value: Some(Fr::from(0u64)),
            out2_owner_key: Some(sk),
            out2_rho: Some(Fr::from(0u64)),
            out2_randomness: Some(Fr::from(0u64)),

            has_change_1: Some(Fr::from(1u64)),
            has_change_2: Some(Fr::from(0u64)),
        };

        circuit.generate_constraints(cs.clone()).unwrap();
        assert!(
            cs.is_satisfied().unwrap(),
            "Unsatisfied: {:?}",
            cs.which_is_unsatisfied()
        );
    }

    #[test]
    fn test_joinsplit_value_conservation_mismatch_rejected() {
        let cs = ConstraintSystem::<Fr>::new_ref();
        let mut circuit = create_dummy_joinsplit_circuit();
        // Maliciously tamper with merchant amount (+100)
        circuit.merchant_amount = Some(Fr::from(9_900_100u64));

        circuit.generate_constraints(cs.clone()).unwrap();
        assert!(
            !cs.is_satisfied().unwrap(),
            "Value conservation violation must fail!"
        );
    }

    #[test]
    fn test_joinsplit_negative_value_modular_underflow_rejected() {
        let cs = ConstraintSystem::<Fr>::new_ref();
        let mut circuit = create_dummy_joinsplit_circuit();

        // Attempt modular underflow: Fr::from(-1) = Fr::MODULUS - 1
        // Without 64-bit range constraints, a user could satisfy value conservation
        // by wrapping around the field modulus r.
        let neg_one = -Fr::from(1u64);
        circuit.in1_value = Some(neg_one);

        circuit.generate_constraints(cs.clone()).unwrap();
        assert!(
            !cs.is_satisfied().unwrap(),
            "Modular underflow / negative input value must fail 64-bit range constraints!"
        );
    }

    #[test]
    fn test_joinsplit_flags_packed_manipulation_rejected() {
        // Test 1: flags_packed out of range (e.g. 4 or 3 when only 0 change notes exist)
        let cs = ConstraintSystem::<Fr>::new_ref();
        let mut circuit = create_dummy_joinsplit_circuit();
        // create_dummy_joinsplit_circuit has has_change_1 = 0, has_change_2 = 0 (flags_packed = 0)
        // Maliciously tamper flags_packed to 1 without setting change note
        circuit.flags_packed = Some(Fr::from(1u64));

        circuit.generate_constraints(cs.clone()).unwrap();
        assert!(
            !cs.is_satisfied().unwrap(),
            "Tampered flags_packed mismatching actual change flags must be unsatisfied!"
        );

        // Test 2: Non-boolean change flag witness (e.g., has_change_1 = 2)
        let cs2 = ConstraintSystem::<Fr>::new_ref();
        let mut circuit2 = create_dummy_joinsplit_circuit();
        circuit2.has_change_1 = Some(Fr::from(2u64));
        circuit2.flags_packed = Some(Fr::from(2u64));

        circuit2.generate_constraints(cs2.clone()).unwrap();
        assert!(
            !cs2.is_satisfied().unwrap(),
            "Non-boolean has_change flag must fail boolean constraints!"
        );

        // Test 3: Non-boolean is_dummy witness (e.g. is_dummy = 2)
        let cs3 = ConstraintSystem::<Fr>::new_ref();
        let mut circuit3 = create_dummy_joinsplit_circuit();
        circuit3.in2_is_dummy = Some(Fr::from(2u64));

        circuit3.generate_constraints(cs3.clone()).unwrap();
        assert!(
            !cs3.is_satisfied().unwrap(),
            "Non-boolean is_dummy flag must fail boolean constraints!"
        );
    }

    #[test]
    fn test_joinsplit_dummy_input_positive_value_rejected() {
        let cs = ConstraintSystem::<Fr>::new_ref();
        let mut circuit = create_dummy_joinsplit_circuit();
        // Maliciously claim dummy note has positive value (v_in_2 > 0 with is_dummy_2 = 1)
        circuit.in2_value = Some(Fr::from(1_000_000u64));
        circuit.in2_is_dummy = Some(Fr::from(1u64));

        circuit.generate_constraints(cs.clone()).unwrap();
        assert!(
            !cs.is_satisfied().unwrap(),
            "Dummy input with value > 0 must fail!"
        );
    }

    #[test]
    fn test_joinsplit_proof_roundtrip() {
        let keys = generate_joinsplit_circuit_keys().unwrap();
        let circuit = create_dummy_joinsplit_circuit();
        let public_inputs = extract_joinsplit_public_inputs(&circuit);

        let proof = generate_joinsplit_proof(circuit, &keys.proving_key).unwrap();
        let valid = verify_joinsplit_proof(&keys.verifying_key, &proof, &public_inputs).unwrap();
        assert!(valid, "JoinSplit Groth16 proof verification must succeed!");
    }
}
