//! Private Note Spend Circuit (Groth16 ZK-UTXO) — DEC-016A / Gate C0.
//! Proves 1-in 1-out private spend: Merkle membership, nullifier derivation,
//! 64-bit integer range safety, boolean has_change, and exact value conservation.

use ark_bls12_381::{Bls12_381, Fr};
use ark_ff::{Field, PrimeField};
use ark_r1cs_std::fields::fp::{AllocatedFp, FpVar};
use ark_relations::gr1cs::{ConstraintSynthesizer, ConstraintSystemRef, SynthesisError};
use ark_snark::SNARK;
use ark_std::rand::rngs::StdRng;
use ark_std::rand::SeedableRng;

use crate::note::MERKLE_TREE_DEPTH;
use crate::poseidon::{poseidon_hash as poseidon_w3_hash, poseidon_w5_hash};

/// Number of public inputs in the PrivateNoteCircuit.
/// MVP: 12 inputs (no reserved nullifier/output slots).
pub const NUM_PUBLIC_INPUTS: usize = 12;

/// Private note spend circuit for Groth16 proof generation.
///
/// This circuit is independent from `ComplianceCircuit` — it uses different
/// Poseidon widths, different nullifier derivation, and different public inputs.
pub struct PrivateNoteCircuit {
    // ── Public inputs ──
    pub note_root: Option<Fr>,
    pub input_nullifier: Option<Fr>,
    pub output_commitment: Option<Fr>,
    pub recipient: Option<Fr>,
    pub merchant_amount: Option<Fr>,
    pub protocol_fee: Option<Fr>,
    pub execution_fee: Option<Fr>,
    pub quote_hash: Option<Fr>,
    pub chain_id: Option<Fr>,
    pub contract_address: Option<Fr>,
    pub expiry: Option<Fr>,
    pub has_change: Option<Fr>,

    // ── Private witnesses: input note ──
    pub input_value: Option<Fr>,
    pub input_owner_key: Option<Fr>,
    pub input_rho: Option<Fr>,
    pub input_randomness: Option<Fr>,
    pub input_leaf_index: Option<Fr>,
    pub input_merkle_path: Option<[Fr; MERKLE_TREE_DEPTH]>,

    // ── Private witnesses: change note ──
    pub change_value: Option<Fr>,
    pub change_owner_key: Option<Fr>,
    pub change_rho: Option<Fr>,
    pub change_randomness: Option<Fr>,
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

/// Enforce that `val` is in [0, 2^64) by decomposing into 64 boolean bits.
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

impl ConstraintSynthesizer<Fr> for PrivateNoteCircuit {
    fn generate_constraints(self, cs: ConstraintSystemRef<Fr>) -> Result<(), SynthesisError> {
        use ark_r1cs_std::eq::EqGadget;

        // Precomputed Poseidon parameters (cached globally in static LazyLock)
        let (w3_rc, w3_mds) = &*crate::poseidon::CACHED_W3_PARAMS;
        let (w5_rc, w5_mds) = &*crate::poseidon::CACHED_W5_PARAMS;

        // Domain tags
        let domain_note = crate::note::domain_note_commitment();
        let domain_nullifier = crate::note::domain_nullifier();
        let domain_merkle = crate::note::domain_merkle_node();

        // ═══════════════════════════════════════════════════════════════
        // 1. Allocate all public inputs
        // ═══════════════════════════════════════════════════════════════

        let note_root_var = public_input(&cs, self.note_root)?;
        let nullifier_var = public_input(&cs, self.input_nullifier)?;
        let output_cm_var = public_input(&cs, self.output_commitment)?;
        let recipient_var = public_input(&cs, self.recipient)?;
        let merchant_var = public_input(&cs, self.merchant_amount)?;
        let protocol_fee_var = public_input(&cs, self.protocol_fee)?;
        let exec_fee_var = public_input(&cs, self.execution_fee)?;
        let quote_hash_var = public_input(&cs, self.quote_hash)?;
        let chain_id_var = public_input(&cs, self.chain_id)?;
        let contract_addr_var = public_input(&cs, self.contract_address)?;
        let expiry_var = public_input(&cs, self.expiry)?;
        let has_change_var = public_input(&cs, self.has_change)?;

        // ═══════════════════════════════════════════════════════════════
        // 2. Allocate all private witnesses
        // ═══════════════════════════════════════════════════════════════

        let in_value = private_witness(&cs, self.input_value)?;
        let in_owner = private_witness(&cs, self.input_owner_key)?;
        let in_rho = private_witness(&cs, self.input_rho)?;
        let in_rand = private_witness(&cs, self.input_randomness)?;
        let in_index = private_witness(&cs, self.input_leaf_index)?;

        let merkle_path: Vec<FpVar<Fr>> = match &self.input_merkle_path {
            Some(path) => path
                .iter()
                .map(|&v| private_witness(&cs, Some(v)))
                .collect::<Result<Vec<_>, _>>()?,
            None => (0..MERKLE_TREE_DEPTH)
                .map(|_| private_witness(&cs, None::<Fr>))
                .collect::<Result<Vec<_>, _>>()?,
        };

        let ch_value = private_witness(&cs, self.change_value)?;
        let ch_owner = private_witness(&cs, self.change_owner_key)?;
        let ch_rho = private_witness(&cs, self.change_rho)?;
        let ch_rand = private_witness(&cs, self.change_randomness)?;

        // ═══════════════════════════════════════════════════════════════
        // 3. Compute input note commitment (used for Merkle AND nullifier)
        //    cm = Poseidon_W5(value, owner_key, rho, randomness; domain_note)
        // ═══════════════════════════════════════════════════════════════

        let in_commitment = poseidon_w5_hash(
            &[
                in_value.clone(),
                in_owner.clone(),
                in_rho.clone(),
                in_rand.clone(),
            ],
            domain_note,
            w5_rc,
            w5_mds,
        )?;

        // ═══════════════════════════════════════════════════════════════
        // 4. Verify Merkle membership
        //    Walk the path from leaf to root, hashing at each level.
        //    Note: `index` is consumed in the loop; `in_index` is preserved.
        // ═══════════════════════════════════════════════════════════════

        let mut current = in_commitment.clone();
        let zero = FpVar::Constant(Fr::from(0u64));
        let one = FpVar::Constant(Fr::from(1u64));

        let mut reconstructed_index = zero.clone();
        let mut two_power = Fr::from(1u64);

        for (level, sibling) in merkle_path.iter().enumerate().take(MERKLE_TREE_DEPTH) {
            // Allocate index_bit as witness
            let index_bit = private_witness(
                &cs,
                self.input_leaf_index.map(|idx| {
                    let level_shift = level as u64;
                    Fr::from((idx.into_bigint().as_ref()[0] >> level_shift) & 1u64)
                }),
            )?;

            // Enforce boolean: index_bit * (1 - index_bit) == 0
            let one_minus_bit = &one - &index_bit;
            let bit_check = &index_bit * &one_minus_bit;
            bit_check.enforce_equal(&zero)?;

            // Accumulate reconstructed index from the 20 direction bits
            reconstructed_index += &index_bit * FpVar::Constant(two_power);
            two_power = two_power + two_power;

            // Conditional swap based on index bit
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

        // Gate C0 Fix 1: Enforce reconstructed_index == in_index.
        // Binds path direction strictly to leaf index, prevents index substitution & double spending.
        reconstructed_index.enforce_equal(&in_index)?;

        // Constrain: computed root == public note_root
        current.enforce_equal(&note_root_var)?;

        // ═══════════════════════════════════════════════════════════════
        // 5. Derive nullifier key + compute nullifier
        //    nk = Poseidon_W3(owner_key, domain_nullifier)
        //    nf = Poseidon_W5(nk, commitment, leaf_index, 0; domain_nullifier)
        // ═══════════════════════════════════════════════════════════════

        let nullifier_key =
            poseidon_w3_hash(&in_owner, &FpVar::Constant(domain_nullifier), w3_rc, w3_mds)?;

        let computed_nf = poseidon_w5_hash(
            &[nullifier_key, in_commitment, in_index, zero.clone()],
            domain_nullifier,
            w5_rc,
            w5_mds,
        )?;

        computed_nf.enforce_equal(&nullifier_var)?;

        // ═══════════════════════════════════════════════════════════════
        // 7. Compute change note commitment & enforce has_change rules
        //    change_cm = Poseidon_W5(ch_value, ch_owner, ch_rho, ch_rand; domain_note)
        // ═══════════════════════════════════════════════════════════════

        let change_cm = poseidon_w5_hash(
            &[ch_value.clone(), ch_owner, ch_rho, ch_rand],
            domain_note,
            w5_rc,
            w5_mds,
        )?;

        // Gate C0 Fix 2:
        // a. Boolean constrain has_change: has_change * (1 - has_change) == 0
        let one_minus_has_change = &one - &has_change_var;
        let has_change_bool = &has_change_var * &one_minus_has_change;
        has_change_bool.enforce_equal(&zero)?;

        // b. Zero-change enforcement: if has_change == 0 => change_value must be 0
        let ch_value_zero_check = &ch_value * &one_minus_has_change;
        ch_value_zero_check.enforce_equal(&zero)?;

        // c. Output commitment: output_cm == has_change * change_cm
        let expected_output = &has_change_var * &change_cm;
        expected_output.enforce_equal(&output_cm_var)?;

        // ═══════════════════════════════════════════════════════════════
        // 8. 64-bit integer range safety and exact value conservation
        // ═══════════════════════════════════════════════════════════════

        // Gate C0 Fix 3: Range constrain all 5 amounts to [0, 2^64)
        enforce_u64_range(&cs, &in_value, self.input_value)?;
        enforce_u64_range(&cs, &merchant_var, self.merchant_amount)?;
        enforce_u64_range(&cs, &protocol_fee_var, self.protocol_fee)?;
        enforce_u64_range(&cs, &exec_fee_var, self.execution_fee)?;
        enforce_u64_range(&cs, &ch_value, self.change_value)?;

        // Value conservation: input_value == merchant + protocol_fee + execution_fee + change_value
        let total_debits = &merchant_var + &protocol_fee_var + &exec_fee_var + &ch_value;
        in_value.enforce_equal(&total_debits)?;

        // ═══════════════════════════════════════════════════════════════
        // 9. Bind remaining public inputs to constraints
        //    These must appear in at least one constraint to prevent
        //    the prover from substituting arbitrary values.
        // ═══════════════════════════════════════════════════════════════

        // ═══════════════════════════════════════════════════════════════
        // 9. Canonical Payment & Domain Binding (Gate C0)
        // ═══════════════════════════════════════════════════════════════
        // Bind recipient, chain_id, contract_address, expiry as independent
        // inputs into Poseidon_W5 (no linear addition or parameter swapping possible).
        let domain_binding = crate::note::domain_quote_binding();
        let scope_hash = poseidon_w5_hash(
            &[
                recipient_var.clone(),
                chain_id_var.clone(),
                contract_addr_var.clone(),
                expiry_var.clone(),
            ],
            domain_binding,
            w5_rc,
            w5_mds,
        )?;

        // Non-linearly combine quote_hash and scope_hash
        let _binding = poseidon_w3_hash(&quote_hash_var, &scope_hash, w3_rc, w3_mds)?;

        // ═══════════════════════════════════════════════════════════════
        // 10. Input value > 0 (non-zero)
        //     We enforce this by computing the inverse and checking it exists.
        // ═══════════════════════════════════════════════════════════════

        // For non-zero check: allocate inverse of in_value, constrain in_value * inv == 1
        // This only works if in_value != 0 (otherwise no inverse exists)
        let in_value_inv = private_witness(
            &cs,
            self.input_value
                .map(|v| v.inverse().unwrap_or(Fr::from(0u64))),
        )?;
        let product = &in_value * &in_value_inv;
        product.enforce_equal(&FpVar::Constant(Fr::from(1u64)))?;

        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Key generation, proof generation, and verification
// ═══════════════════════════════════════════════════════════════════════════

/// Proving and verifying keys for the PrivateNoteCircuit.
pub struct NoteCircuitKeys {
    pub proving_key: ark_groth16::ProvingKey<Bls12_381>,
    pub verifying_key: ark_groth16::VerifyingKey<Bls12_381>,
}

/// Deterministic seed for PrivateNoteCircuit trusted setup.
/// Distinct from ComplianceCircuit seed.
pub const NOTE_CIRCUIT_SETUP_SEED: u64 = 0x4e696d6275734e43; // "NimbusNC"

/// Generate proving and verifying keys for the PrivateNoteCircuit.
pub fn generate_note_circuit_keys() -> Result<NoteCircuitKeys, SynthesisError> {
    use ark_groth16::Groth16;

    let mut rng = StdRng::seed_from_u64(NOTE_CIRCUIT_SETUP_SEED);

    // Create a dummy circuit with valid values for key generation
    let dummy = create_dummy_circuit();

    let (pk, _) = Groth16::<Bls12_381>::circuit_specific_setup(dummy, &mut rng)?;
    let vk = pk.vk.clone();

    Ok(NoteCircuitKeys {
        proving_key: pk,
        verifying_key: vk,
    })
}

/// Serialize the PrivateNoteCircuit VerifyingKey into EVM precompile format:
/// - alpha_g1: [u8; 128]
/// - beta_g2:  [u8; 256]
/// - gamma_g2: [u8; 256]
/// - delta_g2: [u8; 256]
/// - ic[13]:   [[u8; 128]; 13]
#[allow(clippy::type_complexity)]
pub fn generate_note_circuit_evm_vk(
    vk: &ark_groth16::VerifyingKey<Bls12_381>,
) -> ([u8; 128], [u8; 256], [u8; 256], [u8; 256], [[u8; 128]; 13]) {
    use crate::evm::{to_evm_g1, to_evm_g2};

    fn to_g1_array(v: &[u8]) -> [u8; 128] {
        let mut a = [0u8; 128];
        let len = v.len().min(128);
        a[..len].copy_from_slice(&v[..len]);
        a
    }

    fn to_g2_array(v: &[u8]) -> [u8; 256] {
        let mut a = [0u8; 256];
        let len = v.len().min(256);
        a[..len].copy_from_slice(&v[..len]);
        a
    }

    let alpha_g1 = to_g1_array(&to_evm_g1(&vk.alpha_g1));
    let beta_g2 = to_g2_array(&to_evm_g2(&vk.beta_g2));
    let gamma_g2 = to_g2_array(&to_evm_g2(&vk.gamma_g2));
    let delta_g2 = to_g2_array(&to_evm_g2(&vk.delta_g2));

    let mut ic = [[0u8; 128]; 13];
    for (i, ic_point) in vk.gamma_abc_g1.iter().take(13).enumerate() {
        ic[i] = to_g1_array(&to_evm_g1(ic_point));
    }

    (alpha_g1, beta_g2, gamma_g2, delta_g2, ic)
}

static CACHED_KEYS: std::sync::OnceLock<NoteCircuitKeys> = std::sync::OnceLock::new();

/// Retrieve or lazily initialize cached keys for development, testing, and relayer verification.
pub fn get_or_init_note_circuit_keys() -> &'static NoteCircuitKeys {
    CACHED_KEYS.get_or_init(|| {
        generate_note_circuit_keys().expect("Failed to initialize development note circuit keys")
    })
}

/// Create a dummy circuit with valid values for setup and testing.
pub fn create_dummy_circuit() -> PrivateNoteCircuit {
    use crate::note::{
        compute_empty_hashes, derive_nullifier as dn, derive_nullifier_key as dnk,
        note_commitment as nc,
    };

    let spending_key = Fr::from(42u64);
    let nk = dnk(spending_key);

    let in_value: u64 = 99_800_000;
    let in_rho = Fr::from(1u64);
    let in_rand = Fr::from(2u64);
    let in_index: u64 = 0;

    let in_cm = nc(in_value, spending_key, in_rho, in_rand);

    let empty = compute_empty_hashes();
    let siblings: [Fr; MERKLE_TREE_DEPTH] = std::array::from_fn(|i| empty[i]);
    let root = crate::note::compute_merkle_root(in_cm, in_index, &siblings);

    let nf = dn(nk, in_cm, in_index);

    let merchant: u64 = 5_000_000;
    let pfee: u64 = 12_500;
    let efee: u64 = 23_000;
    let change: u64 = in_value - merchant - pfee - efee;

    let ch_rho = Fr::from(3u64);
    let ch_rand = Fr::from(4u64);
    let ch_cm = nc(change, spending_key, ch_rho, ch_rand);

    PrivateNoteCircuit {
        note_root: Some(root),
        input_nullifier: Some(nf),
        output_commitment: Some(ch_cm),
        recipient: Some(Fr::from(100u64)),
        merchant_amount: Some(Fr::from(merchant)),
        protocol_fee: Some(Fr::from(pfee)),
        execution_fee: Some(Fr::from(efee)),
        quote_hash: Some(Fr::from(200u64)),
        chain_id: Some(Fr::from(421614u64)),
        contract_address: Some(Fr::from(300u64)),
        expiry: Some(Fr::from(0u64)),
        has_change: Some(Fr::from(1u64)),

        input_value: Some(Fr::from(in_value)),
        input_owner_key: Some(spending_key),
        input_rho: Some(in_rho),
        input_randomness: Some(in_rand),
        input_leaf_index: Some(Fr::from(in_index)),
        input_merkle_path: Some(siblings),

        change_value: Some(Fr::from(change)),
        change_owner_key: Some(spending_key),
        change_rho: Some(ch_rho),
        change_randomness: Some(ch_rand),
    }
}

/// Generate a private note spend proof.
pub fn generate_note_proof(
    circuit: PrivateNoteCircuit,
    pk: &ark_groth16::ProvingKey<Bls12_381>,
) -> Result<ark_groth16::Proof<Bls12_381>, SynthesisError> {
    use ark_groth16::Groth16;

    let mut rng = StdRng::from_entropy();
    Groth16::<Bls12_381>::prove(pk, circuit, &mut rng)
}

/// Verify a private note spend proof.
pub fn verify_note_proof(
    proof: &ark_groth16::Proof<Bls12_381>,
    vk: &ark_groth16::VerifyingKey<Bls12_381>,
    public_inputs: &[Fr],
) -> bool {
    use ark_groth16::{prepare_verifying_key, Groth16};

    assert_eq!(
        public_inputs.len(),
        NUM_PUBLIC_INPUTS,
        "Expected {} public inputs, got {}",
        NUM_PUBLIC_INPUTS,
        public_inputs.len()
    );

    let pvk = prepare_verifying_key(vk);
    Groth16::<Bls12_381>::verify_with_processed_vk(&pvk, public_inputs, proof).unwrap_or(false)
}

static PREPARED_NOTE_VK: std::sync::OnceLock<ark_groth16::PreparedVerifyingKey<Bls12_381>> =
    std::sync::OnceLock::new();

/// Returns the prepared verifying key for PrivateNoteCircuit, cached across invocations.
pub fn get_prepared_note_vk() -> &'static ark_groth16::PreparedVerifyingKey<Bls12_381> {
    PREPARED_NOTE_VK.get_or_init(|| {
        let keys = get_or_init_note_circuit_keys();
        ark_groth16::prepare_verifying_key(&keys.verifying_key)
    })
}

/// Verify an EVM-encoded Groth16 proof for PrivateNoteCircuit.
/// Preflight check for relayer nodes to defend against gas-griefing attacks (DEC-026 C-01).
pub fn verify_evm_note_proof(
    proof_a_neg: &[u8; 128],
    proof_b: &[u8; 256],
    proof_c: &[u8; 128],
    public_inputs: &[Fr; 12],
) -> bool {
    let Some(proof) = crate::evm::from_evm_proof(proof_a_neg, proof_b, proof_c) else {
        return false;
    };
    let pvk = get_prepared_note_vk();
    ark_groth16::Groth16::<Bls12_381>::verify_with_processed_vk(pvk, public_inputs, &proof)
        .unwrap_or(false)
}

/// Extract public inputs from a dummy circuit (for testing).
pub fn extract_public_inputs(circuit: &PrivateNoteCircuit) -> Vec<Fr> {
    vec![
        circuit.note_root.unwrap(),
        circuit.input_nullifier.unwrap(),
        circuit.output_commitment.unwrap(),
        circuit.recipient.unwrap(),
        circuit.merchant_amount.unwrap(),
        circuit.protocol_fee.unwrap(),
        circuit.execution_fee.unwrap(),
        circuit.quote_hash.unwrap(),
        circuit.chain_id.unwrap(),
        circuit.contract_address.unwrap(),
        circuit.expiry.unwrap(),
        circuit.has_change.unwrap(),
    ]
}

// ═══════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::note::compute_merkle_root;

    fn setup_valid_circuit() -> PrivateNoteCircuit {
        create_dummy_circuit()
    }

    #[test]
    fn test_note_circuit_valid_proof() {
        let keys = get_or_init_note_circuit_keys();
        let circuit = setup_valid_circuit();
        let public_inputs = extract_public_inputs(&circuit);

        let proof = generate_note_proof(circuit, &keys.proving_key).unwrap();

        let is_valid = verify_note_proof(&proof, &keys.verifying_key, &public_inputs);
        assert!(is_valid, "Valid proof must verify");
    }

    #[test]
    fn test_note_circuit_tampered_nullifier() {
        let keys = get_or_init_note_circuit_keys();
        let circuit = setup_valid_circuit();
        let mut public_inputs = extract_public_inputs(&circuit);

        let proof = generate_note_proof(circuit, &keys.proving_key).unwrap();

        // Tamper with nullifier (public input index 1)
        public_inputs[1] += Fr::from(1u64);

        let is_valid = verify_note_proof(&proof, &keys.verifying_key, &public_inputs);
        assert!(!is_valid, "Tampered nullifier must fail verification");
    }

    #[test]
    fn test_note_circuit_tampered_merchant_amount() {
        let keys = get_or_init_note_circuit_keys();
        let circuit = setup_valid_circuit();
        let mut public_inputs = extract_public_inputs(&circuit);

        let proof = generate_note_proof(circuit, &keys.proving_key).unwrap();

        // Tamper with merchant amount (public input index 4)
        public_inputs[4] += Fr::from(1u64);

        let is_valid = verify_note_proof(&proof, &keys.verifying_key, &public_inputs);
        assert!(!is_valid, "Tampered merchant amount must fail verification");
    }

    #[test]
    fn test_note_circuit_tampered_recipient() {
        let keys = get_or_init_note_circuit_keys();
        let circuit = setup_valid_circuit();
        let mut public_inputs = extract_public_inputs(&circuit);

        let proof = generate_note_proof(circuit, &keys.proving_key).unwrap();

        // Tamper with recipient
        public_inputs[3] += Fr::from(1u64);

        let is_valid = verify_note_proof(&proof, &keys.verifying_key, &public_inputs);
        assert!(!is_valid, "Tampered recipient must fail verification");
    }

    #[test]
    fn test_note_circuit_tampered_root() {
        let keys = get_or_init_note_circuit_keys();
        let circuit = setup_valid_circuit();
        let mut public_inputs = extract_public_inputs(&circuit);

        let proof = generate_note_proof(circuit, &keys.proving_key).unwrap();

        // Tamper with root
        public_inputs[0] += Fr::from(1u64);

        let is_valid = verify_note_proof(&proof, &keys.verifying_key, &public_inputs);
        assert!(!is_valid, "Tampered root must fail verification");
    }

    #[test]
    fn test_note_circuit_wrong_owner_key_fails_proving() {
        let keys = get_or_init_note_circuit_keys();
        let mut circuit = setup_valid_circuit();

        // Use wrong owner key — this should fail during proof generation
        // because the nullifier won't match
        circuit.input_owner_key = Some(Fr::from(999u64));

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            generate_note_proof(circuit, &keys.proving_key)
        }));
        assert!(
            result.is_err() || result.unwrap().is_err(),
            "Wrong owner key must fail proof generation"
        );
    }

    #[test]
    fn test_note_circuit_wrong_merkle_path_fails() {
        let keys = get_or_init_note_circuit_keys();
        let mut circuit = setup_valid_circuit();

        // Corrupt one sibling in the Merkle path
        if let Some(ref mut path) = circuit.input_merkle_path {
            path[5] += Fr::from(1u64);
        }

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            generate_note_proof(circuit, &keys.proving_key)
        }));
        assert!(
            result.is_err() || result.unwrap().is_err(),
            "Wrong Merkle path must fail proof generation"
        );
    }

    #[test]
    fn test_note_circuit_value_conservation_violation_fails() {
        let keys = get_or_init_note_circuit_keys();
        let mut circuit = setup_valid_circuit();

        // Violate value conservation: increase change by 1
        circuit.change_value = circuit.change_value.map(|v| v + Fr::from(1u64));

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            generate_note_proof(circuit, &keys.proving_key)
        }));
        assert!(
            result.is_err() || result.unwrap().is_err(),
            "Value conservation violation must fail proof generation"
        );
    }

    #[test]
    fn test_note_circuit_full_spend_no_change() {
        use crate::note::{
            compute_empty_hashes, derive_nullifier as dn, derive_nullifier_key as dnk,
            note_commitment as nc,
        };

        let keys = get_or_init_note_circuit_keys();
        let spending_key = Fr::from(42u64);
        let nk = dnk(spending_key);

        let in_value: u64 = 5_035_500; // 5 USDC + fees
        let in_rho = Fr::from(10u64);
        let in_rand = Fr::from(20u64);
        let in_index: u64 = 0;
        let in_cm = nc(in_value, spending_key, in_rho, in_rand);

        let empty = compute_empty_hashes();
        let siblings: [Fr; MERKLE_TREE_DEPTH] = std::array::from_fn(|i| empty[i]);
        let root = compute_merkle_root(in_cm, in_index, &siblings);
        let nf = dn(nk, in_cm, in_index);

        let merchant: u64 = 5_000_000;
        let pfee: u64 = 12_500;
        let efee: u64 = 23_000;
        // Full spend: no change
        assert_eq!(in_value, merchant + pfee + efee);

        let circuit = PrivateNoteCircuit {
            note_root: Some(root),
            input_nullifier: Some(nf),
            output_commitment: Some(Fr::from(0u64)), // no change
            recipient: Some(Fr::from(100u64)),
            merchant_amount: Some(Fr::from(merchant)),
            protocol_fee: Some(Fr::from(pfee)),
            execution_fee: Some(Fr::from(efee)),
            quote_hash: Some(Fr::from(200u64)),
            chain_id: Some(Fr::from(421614u64)),
            contract_address: Some(Fr::from(300u64)),
            expiry: Some(Fr::from(0u64)),
            has_change: Some(Fr::from(0u64)), // no change

            input_value: Some(Fr::from(in_value)),
            input_owner_key: Some(spending_key),
            input_rho: Some(in_rho),
            input_randomness: Some(in_rand),
            input_leaf_index: Some(Fr::from(in_index)),
            input_merkle_path: Some(siblings),

            change_value: Some(Fr::from(0u64)),
            change_owner_key: Some(Fr::from(0u64)),
            change_rho: Some(Fr::from(0u64)),
            change_randomness: Some(Fr::from(0u64)),
        };

        let public_inputs = extract_public_inputs(&circuit);
        let proof = generate_note_proof(circuit, &keys.proving_key).unwrap();
        let is_valid = verify_note_proof(&proof, &keys.verifying_key, &public_inputs);
        assert!(is_valid, "Full spend (no change) must verify");
    }

    #[test]
    fn test_gate_c0_tampered_leaf_index_fails_proving() {
        let keys = get_or_init_note_circuit_keys();
        let mut circuit = setup_valid_circuit();

        // Tamper leaf index from 0 to 1 while leaving Merkle path intact
        circuit.input_leaf_index = Some(Fr::from(1u64));

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            generate_note_proof(circuit, &keys.proving_key)
        }));
        assert!(
            result.is_err() || result.unwrap().is_err(),
            "Tampered leaf index must fail proof generation (reconstructed_index != in_index)"
        );
    }

    #[test]
    fn test_gate_c0_non_boolean_has_change_fails_proving() {
        let keys = get_or_init_note_circuit_keys();
        let mut circuit = setup_valid_circuit();

        // Non-boolean has_change = 2 must violate has_change * (1 - has_change) == 0
        circuit.has_change = Some(Fr::from(2u64));

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            generate_note_proof(circuit, &keys.proving_key)
        }));
        assert!(
            result.is_err() || result.unwrap().is_err(),
            "Non-boolean has_change must fail proof generation"
        );
    }

    #[test]
    fn test_gate_c0_zero_change_with_positive_change_value_fails_proving() {
        let keys = get_or_init_note_circuit_keys();
        let mut circuit = setup_valid_circuit();

        // has_change = 0 but change_value > 0 must violate ch_value * (1 - has_change) == 0
        circuit.has_change = Some(Fr::from(0u64));
        circuit.output_commitment = Some(Fr::from(0u64));

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            generate_note_proof(circuit, &keys.proving_key)
        }));
        assert!(
            result.is_err() || result.unwrap().is_err(),
            "Positive change value with has_change=0 must fail proof generation"
        );
    }

    #[test]
    fn test_gate_c0_overflow_amount_fails_proving() {
        let keys = get_or_init_note_circuit_keys();
        let mut circuit = setup_valid_circuit();

        // Amount outside [0, 2^64) must violate 64-bit range decomposition
        circuit.merchant_amount = Some(-Fr::from(1u64));

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            generate_note_proof(circuit, &keys.proving_key)
        }));
        assert!(
            result.is_err() || result.unwrap().is_err(),
            "Field overflow amount (> 2^64 - 1) must fail 64-bit range check"
        );
    }

    #[test]
    fn test_gate_c0_swapped_domain_fields_fails_verification() {
        let keys = get_or_init_note_circuit_keys();
        let circuit = setup_valid_circuit();
        let mut public_inputs = extract_public_inputs(&circuit);

        let proof = generate_note_proof(circuit, &keys.proving_key).unwrap();

        // Swap chain_id (index 8) and contract_address (index 9)
        public_inputs.swap(8, 9);

        let is_valid = verify_note_proof(&proof, &keys.verifying_key, &public_inputs);
        assert!(
            !is_valid,
            "Swapped chain_id and contract_address must fail verification"
        );
    }

    #[test]
    fn print_note_circuit_evm_vk_hex() {
        let keys = get_or_init_note_circuit_keys();
        let (alpha, beta, gamma, delta, ic) = generate_note_circuit_evm_vk(&keys.verifying_key);

        let to_hex =
            |b: &[u8]| -> String { b.iter().map(|x| format!("{x:02x}")).collect::<String>() };

        eprintln!("// --- PrivateNoteCircuit Phase A Trusted Setup VK Constants ---");
        eprintln!("// Seed: 0x{:016X} (NimbusNC)", NOTE_CIRCUIT_SETUP_SEED);
        eprintln!("// Circuit: PrivateNoteCircuit (12 public inputs) over BLS12-381");
        eprintln!();
        eprintln!("// NOTE_VK_ALPHA_G1 (128 bytes)");
        eprintln!(
            "pub const NOTE_VK_ALPHA_G1: [u8; 128] = alloy_primitives::hex!(\"{}\");",
            to_hex(&alpha)
        );
        eprintln!();
        eprintln!("// NOTE_VK_BETA_G2 (256 bytes)");
        eprintln!(
            "pub const NOTE_VK_BETA_G2: [u8; 256] = alloy_primitives::hex!(\"{}\");",
            to_hex(&beta)
        );
        eprintln!();
        eprintln!("// NOTE_VK_GAMMA_G2 (256 bytes)");
        eprintln!(
            "pub const NOTE_VK_GAMMA_G2: [u8; 256] = alloy_primitives::hex!(\"{}\");",
            to_hex(&gamma)
        );
        eprintln!();
        eprintln!("// NOTE_VK_DELTA_G2 (256 bytes)");
        eprintln!(
            "pub const NOTE_VK_DELTA_G2: [u8; 256] = alloy_primitives::hex!(\"{}\");",
            to_hex(&delta)
        );
        eprintln!();
        eprintln!("pub const NOTE_VK_IC: [[u8; 128]; 13] = [");
        for (i, ic_point) in ic.iter().enumerate() {
            eprintln!("    // IC[{}]", i);
            eprintln!("    alloy_primitives::hex!(\"{}\"),", to_hex(ic_point));
        }
        eprintln!("];");
    }

    #[test]
    fn test_verify_evm_note_proof_preflight() {
        use crate::evm::{to_evm_g1, to_evm_g2};

        let keys = get_or_init_note_circuit_keys();
        let circuit = setup_valid_circuit();
        let pis = extract_public_inputs(&circuit);
        let mut public_inputs = [Fr::default(); 12];
        public_inputs.copy_from_slice(&pis[..12]);

        let proof = generate_note_proof(circuit, &keys.proving_key).unwrap();

        // Convert proof to EVM format
        let mut a_neg = [0u8; 128];
        a_neg.copy_from_slice(&to_evm_g1(&-proof.a));

        let mut b = [0u8; 256];
        b.copy_from_slice(&to_evm_g2(&proof.b));

        let mut c = [0u8; 128];
        c.copy_from_slice(&to_evm_g1(&proof.c));

        // 1. Valid EVM proof verifies
        assert!(verify_evm_note_proof(&a_neg, &b, &c, &public_inputs));

        // 2. Tampered public input rejected
        let mut tampered_pis = public_inputs;
        tampered_pis[4] += Fr::from(100u64);
        assert!(!verify_evm_note_proof(&a_neg, &b, &c, &tampered_pis));

        // 3. Fake proof points rejected
        let fake_a = [0x11u8; 128];
        assert!(!verify_evm_note_proof(&fake_a, &b, &c, &public_inputs));
    }
}
