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

use crate::poseidon::{poseidon_hash as poseidon_w3_hash, poseidon_w5_hash};

/// Number of public inputs in the PrivateNoteCircuit (DEC-032: 13 inputs).
pub const NUM_PUBLIC_INPUTS: usize = 13;

/// Private note spend circuit for Groth16 proof generation.
///
/// This circuit is independent from `ComplianceCircuit` — it uses different
/// Poseidon widths, different nullifier derivation, and different public inputs.
pub struct PrivateNoteCircuit {
    // ── Public inputs (13 scalar field elements) ──
    pub note_root: Option<Fr>,
    pub leaf_count: Option<Fr>,
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

    // MMR Internal Mountain Path (maksimal tinggi 32)
    pub mountain_height: Option<u8>,
    pub mountain_siblings: Option<[Fr; 32]>,

    // MMR Bagging Siblings (peak-peak lain yang membentuk root)
    pub peak_bagging_siblings: Option<[Fr; 32]>,
    pub peak_bagging_count: Option<u8>,

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
        // 1. Allocate all public inputs (13 scalar field elements)
        // ═══════════════════════════════════════════════════════════════

        let note_root_var = public_input(&cs, self.note_root)?;
        let leaf_count_var = public_input(&cs, self.leaf_count)?;
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
        // 4. Verify MMR membership, mountain path and peak bagging (DEC-032)
        // ═══════════════════════════════════════════════════════════════

        // Range constrain leaf_count and input_leaf_index to [0, 2^64)
        enforce_u64_range(&cs, &in_index, self.input_leaf_index)?;
        enforce_u64_range(&cs, &leaf_count_var, self.leaf_count)?;

        // DEC-032 Section 4.1: Anti-Hyperbridge strictly upper bound check: input_leaf_index < leaf_count
        // Enforces delta = leaf_count - 1 - input_leaf_index in [0, 2^64)
        let delta_witness = match (self.leaf_count, self.input_leaf_index) {
            (Some(lc), Some(li)) => {
                let one = Fr::from(1u64);
                Some(lc - one - li)
            }
            _ => None,
        };
        let delta_var = &leaf_count_var - FpVar::Constant(Fr::from(1u64)) - &in_index;
        enforce_u64_range(&cs, &delta_var, delta_witness)?;

        let domain_mmr_bag = crate::note::domain_mmr_bag();
        let zero = FpVar::Constant(Fr::from(0u64));
        let one = FpVar::Constant(Fr::from(1u64));

        let m_height = self.mountain_height.unwrap_or(0) as usize;
        let mountain_sibs = self.mountain_siblings.unwrap_or([Fr::from(0u64); 32]);
        let mut current = in_commitment.clone();

        // 4a. Walk internal mountain from leaf to computed peak
        for level in 0..32 {
            let is_active = level < m_height;
            let sibling_var = private_witness(&cs, Some(mountain_sibs[level]))?;
            let is_active_var = private_witness(
                &cs,
                Some(if is_active {
                    Fr::from(1u64)
                } else {
                    Fr::from(0u64)
                }),
            )?;
            let one_minus_active = &one - &is_active_var;
            (&is_active_var * &one_minus_active).enforce_equal(&zero)?;

            let bit_val = self.input_leaf_index.map(|idx| {
                let level_shift = level as u64;
                Fr::from((idx.into_bigint().as_ref()[0] >> level_shift) & 1u64)
            });
            let index_bit = private_witness(&cs, bit_val)?;
            let one_minus_bit = &one - &index_bit;
            (&index_bit * &one_minus_bit).enforce_equal(&zero)?;

            let diff_cs = &sibling_var - &current;
            let diff_sc = &current - &sibling_var;
            let left = &current + &index_bit * &diff_cs;
            let right = &sibling_var + &index_bit * &diff_sc;

            let hashed = poseidon_w5_hash(
                &[left, right, zero.clone(), zero.clone()],
                domain_merkle,
                w5_rc,
                w5_mds,
            )?;

            // current = current + is_active * (hashed - current)
            current = &current + &is_active_var * (&hashed - &current);
        }
        let computed_peak = current;

        // 4b. Reconstruct active peaks and canonical backward bagging fold
        let lc_opt = self
            .leaf_count
            .map(|lc| lc.into_bigint().as_ref()[0] as usize);
        let li_opt = self
            .input_leaf_index
            .map(|li| li.into_bigint().as_ref()[0] as usize);

        let (target_peak_idx, total_peaks) = match (lc_opt, li_opt) {
            (Some(lc), Some(li)) => {
                let mut peak_heights = Vec::new();
                for h in (0..usize::BITS).rev() {
                    if (lc >> h) & 1 == 1 {
                        peak_heights.push(h as usize);
                    }
                }
                let m = peak_heights.len();
                let mut current_start = 0;
                let mut target_k = 0;
                for (p_idx, &h) in peak_heights.iter().enumerate() {
                    let mountain_size = 1 << h;
                    if li >= current_start && li < current_start + mountain_size {
                        target_k = p_idx;
                        break;
                    }
                    current_start += mountain_size;
                }
                (target_k, m)
            }
            _ => (0, 1),
        };

        let bagging_sibs = self.peak_bagging_siblings.unwrap_or([Fr::from(0u64); 32]);
        let mut peak_vars = Vec::with_capacity(32);
        for s in 0..32 {
            let p_var = if s == target_peak_idx {
                computed_peak.clone()
            } else {
                let sib_idx = if s < target_peak_idx { s } else { s - 1 };
                let sib_val = if sib_idx < 32 {
                    bagging_sibs[sib_idx]
                } else {
                    Fr::from(0u64)
                };
                private_witness(&cs, Some(sib_val))?
            };
            peak_vars.push(p_var);
        }

        // Backward fold across 32 steps with constant R1CS topology
        let mut acc = zero.clone();
        for s in (0..32).rev() {
            let is_start = s == total_peaks.saturating_sub(1);
            let is_fold = s < total_peaks.saturating_sub(1);

            let is_start_var = private_witness(
                &cs,
                Some(if is_start {
                    Fr::from(1u64)
                } else {
                    Fr::from(0u64)
                }),
            )?;
            let is_fold_var = private_witness(
                &cs,
                Some(if is_fold {
                    Fr::from(1u64)
                } else {
                    Fr::from(0u64)
                }),
            )?;
            let is_valid_var = &is_start_var + &is_fold_var;

            let one_minus_start = &one - &is_start_var;
            (&is_start_var * &one_minus_start).enforce_equal(&zero)?;
            let one_minus_fold = &one - &is_fold_var;
            (&is_fold_var * &one_minus_fold).enforce_equal(&zero)?;
            let one_minus_valid = &one - &is_valid_var;
            (&is_valid_var * &one_minus_valid).enforce_equal(&zero)?;

            let hashed = poseidon_w5_hash(
                &[
                    peak_vars[s].clone(),
                    acc.clone(),
                    zero.clone(),
                    zero.clone(),
                ],
                domain_merkle,
                w5_rc,
                w5_mds,
            )?;

            let inactive_acc = &one_minus_valid * &acc;
            let start_acc = &is_start_var * &peak_vars[s];
            let fold_acc = &is_fold_var * &hashed;
            acc = &inactive_acc + &start_acc + &fold_acc;
        }

        // Final Bagging Poseidon hash: Poseidon_W5([acc, leaf_count, 0, 0], domain_mmr_bag)
        let bagged_root = poseidon_w5_hash(
            &[acc, leaf_count_var.clone(), zero.clone(), zero.clone()],
            domain_mmr_bag,
            w5_rc,
            w5_mds,
        )?;
        bagged_root.enforce_equal(&note_root_var)?;

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
) -> ([u8; 128], [u8; 256], [u8; 256], [u8; 256], [[u8; 128]; 14]) {
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

    let mut ic = [[0u8; 128]; 14];
    for (i, ic_point) in vk.gamma_abc_g1.iter().take(14).enumerate() {
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
        derive_nullifier as dn, derive_nullifier_key as dnk, note_commitment as nc,
        MerkleMountainRange,
    };

    let spending_key = Fr::from(42u64);
    let nk = dnk(spending_key);

    let in_value: u64 = 99_800_000;
    let in_rho = Fr::from(1u64);
    let in_rand = Fr::from(2u64);

    let in_cm = nc(in_value, spending_key, in_rho, in_rand);

    let mut mmr = MerkleMountainRange::new();
    let (leaf_idx, root) = mmr.append(in_cm);
    let proof = mmr.generate_proof(leaf_idx);

    let mut m_sibs = [Fr::from(0u64); 32];
    for (i, s) in proof.mountain_siblings.iter().enumerate() {
        m_sibs[i] = *s;
    }
    let mut p_sibs = [Fr::from(0u64); 32];
    for (i, s) in proof.peak_bagging_siblings.iter().enumerate() {
        p_sibs[i] = *s;
    }

    let nf = dn(nk, in_cm, leaf_idx as u64);

    let merchant: u64 = 5_000_000;
    let pfee: u64 = 12_500;
    let efee: u64 = 23_000;
    let change: u64 = in_value - merchant - pfee - efee;

    let ch_rho = Fr::from(3u64);
    let ch_rand = Fr::from(4u64);
    let ch_cm = nc(change, spending_key, ch_rho, ch_rand);

    PrivateNoteCircuit {
        note_root: Some(root),
        leaf_count: Some(Fr::from(mmr.leaf_count as u64)),
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
        input_leaf_index: Some(Fr::from(leaf_idx as u64)),
        mountain_height: Some(proof.mountain_height as u8),
        mountain_siblings: Some(m_sibs),
        peak_bagging_siblings: Some(p_sibs),
        peak_bagging_count: Some(proof.peak_bagging_siblings.len() as u8),

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
    public_inputs: &[Fr; 13],
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
        circuit.leaf_count.unwrap(),
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

        // Tamper with nullifier (public input index 2)
        public_inputs[2] += Fr::from(1u64);

        let is_valid = verify_note_proof(&proof, &keys.verifying_key, &public_inputs);
        assert!(!is_valid, "Tampered nullifier must fail verification");
    }

    #[test]
    fn test_note_circuit_tampered_merchant_amount() {
        let keys = get_or_init_note_circuit_keys();
        let circuit = setup_valid_circuit();
        let mut public_inputs = extract_public_inputs(&circuit);

        let proof = generate_note_proof(circuit, &keys.proving_key).unwrap();

        // Tamper with merchant amount (public input index 5)
        public_inputs[5] += Fr::from(1u64);

        let is_valid = verify_note_proof(&proof, &keys.verifying_key, &public_inputs);
        assert!(!is_valid, "Tampered merchant amount must fail verification");
    }

    #[test]
    fn test_note_circuit_tampered_recipient() {
        let keys = get_or_init_note_circuit_keys();
        let circuit = setup_valid_circuit();
        let mut public_inputs = extract_public_inputs(&circuit);

        let proof = generate_note_proof(circuit, &keys.proving_key).unwrap();

        // Tamper with recipient (public input index 4)
        public_inputs[4] += Fr::from(1u64);

        let is_valid = verify_note_proof(&proof, &keys.verifying_key, &public_inputs);
        assert!(!is_valid, "Tampered recipient must fail verification");
    }

    #[test]
    fn test_note_circuit_tampered_root() {
        let keys = get_or_init_note_circuit_keys();
        let circuit = setup_valid_circuit();
        let mut public_inputs = extract_public_inputs(&circuit);

        let proof = generate_note_proof(circuit, &keys.proving_key).unwrap();

        // Tamper with root (public input index 0)
        public_inputs[0] += Fr::from(1u64);

        let is_valid = verify_note_proof(&proof, &keys.verifying_key, &public_inputs);
        assert!(!is_valid, "Tampered root must fail verification");
    }

    #[test]
    fn test_note_circuit_tampered_leaf_count() {
        let keys = get_or_init_note_circuit_keys();
        let circuit = setup_valid_circuit();
        let mut public_inputs = extract_public_inputs(&circuit);

        let proof = generate_note_proof(circuit, &keys.proving_key).unwrap();

        // Tamper with leaf_count (public input index 1)
        public_inputs[1] += Fr::from(1u64);

        let is_valid = verify_note_proof(&proof, &keys.verifying_key, &public_inputs);
        assert!(!is_valid, "Tampered leaf_count must fail verification");
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
    fn test_note_circuit_wrong_mountain_path_fails() {
        use crate::note::{
            derive_nullifier as dn, derive_nullifier_key as dnk, note_commitment as nc,
            MerkleMountainRange,
        };

        let keys = get_or_init_note_circuit_keys();
        let spending_key = Fr::from(42u64);
        let nk = dnk(spending_key);

        let in_value: u64 = 99_800_000;
        let in_rho = Fr::from(1u64);
        let in_rand = Fr::from(2u64);
        let in_cm = nc(in_value, spending_key, in_rho, in_rand);

        // Build MMR with 2 leaves so mountain_height = 1 and mountain_siblings has 1 active element
        let mut mmr = MerkleMountainRange::new();
        let (leaf_idx, _) = mmr.append(in_cm);
        let other_cm = nc(
            10_000_000,
            Fr::from(99u64),
            Fr::from(88u64),
            Fr::from(77u64),
        );
        let (_, root) = mmr.append(other_cm);

        let proof = mmr.generate_proof(leaf_idx);
        assert_eq!(proof.mountain_height, 1);

        let mut m_sibs = [Fr::from(0u64); 32];
        for (i, s) in proof.mountain_siblings.iter().enumerate() {
            m_sibs[i] = *s;
        }
        let mut p_sibs = [Fr::from(0u64); 32];
        for (i, s) in proof.peak_bagging_siblings.iter().enumerate() {
            p_sibs[i] = *s;
        }

        let nf = dn(nk, in_cm, leaf_idx as u64);
        let merchant: u64 = 5_000_000;
        let pfee: u64 = 12_500;
        let efee: u64 = 23_000;
        let change: u64 = in_value - merchant - pfee - efee;
        let ch_rho = Fr::from(3u64);
        let ch_rand = Fr::from(4u64);
        let ch_cm = nc(change, spending_key, ch_rho, ch_rand);

        // Corrupt active mountain sibling!
        m_sibs[0] += Fr::from(1u64);

        let circuit = PrivateNoteCircuit {
            note_root: Some(root),
            leaf_count: Some(Fr::from(mmr.leaf_count as u64)),
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
            input_leaf_index: Some(Fr::from(leaf_idx as u64)),
            mountain_height: Some(proof.mountain_height as u8),
            mountain_siblings: Some(m_sibs),
            peak_bagging_siblings: Some(p_sibs),
            peak_bagging_count: Some(proof.peak_bagging_siblings.len() as u8),

            change_value: Some(Fr::from(change)),
            change_owner_key: Some(spending_key),
            change_rho: Some(ch_rho),
            change_randomness: Some(ch_rand),
        };

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            generate_note_proof(circuit, &keys.proving_key)
        }));
        assert!(
            result.is_err() || result.unwrap().is_err(),
            "Wrong mountain path must fail proof generation (unsatisfied constraint)"
        );
    }

    #[test]
    fn test_note_circuit_tampered_peak_siblings_fails() {
        use crate::note::{
            derive_nullifier as dn, derive_nullifier_key as dnk, note_commitment as nc,
            MerkleMountainRange,
        };

        let keys = get_or_init_note_circuit_keys();
        let spending_key = Fr::from(42u64);
        let nk = dnk(spending_key);

        let in_value: u64 = 99_800_000;
        let in_rho = Fr::from(1u64);
        let in_rand = Fr::from(2u64);
        let in_cm = nc(in_value, spending_key, in_rho, in_rand);

        // Build MMR with 3 leaves so peak count = 2 (peaks at height 1 and height 0)
        let mut mmr = MerkleMountainRange::new();
        let (leaf_idx, _) = mmr.append(in_cm);
        mmr.append(nc(
            10_000_000,
            Fr::from(99u64),
            Fr::from(88u64),
            Fr::from(77u64),
        ));
        let (_, root) = mmr.append(nc(
            20_000_000,
            Fr::from(66u64),
            Fr::from(55u64),
            Fr::from(44u64),
        ));

        let proof = mmr.generate_proof(leaf_idx);
        assert_eq!(proof.peak_bagging_siblings.len(), 1);

        let mut m_sibs = [Fr::from(0u64); 32];
        for (i, s) in proof.mountain_siblings.iter().enumerate() {
            m_sibs[i] = *s;
        }
        let mut p_sibs = [Fr::from(0u64); 32];
        for (i, s) in proof.peak_bagging_siblings.iter().enumerate() {
            p_sibs[i] = *s;
        }

        let nf = dn(nk, in_cm, leaf_idx as u64);
        let merchant: u64 = 5_000_000;
        let pfee: u64 = 12_500;
        let efee: u64 = 23_000;
        let change: u64 = in_value - merchant - pfee - efee;
        let ch_rho = Fr::from(3u64);
        let ch_rand = Fr::from(4u64);
        let ch_cm = nc(change, spending_key, ch_rho, ch_rand);

        // Corrupt bagging peak sibling!
        p_sibs[0] += Fr::from(1u64);

        let circuit = PrivateNoteCircuit {
            note_root: Some(root),
            leaf_count: Some(Fr::from(mmr.leaf_count as u64)),
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
            input_leaf_index: Some(Fr::from(leaf_idx as u64)),
            mountain_height: Some(proof.mountain_height as u8),
            mountain_siblings: Some(m_sibs),
            peak_bagging_siblings: Some(p_sibs),
            peak_bagging_count: Some(proof.peak_bagging_siblings.len() as u8),

            change_value: Some(Fr::from(change)),
            change_owner_key: Some(spending_key),
            change_rho: Some(ch_rho),
            change_randomness: Some(ch_rand),
        };

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            generate_note_proof(circuit, &keys.proving_key)
        }));
        assert!(
            result.is_err() || result.unwrap().is_err(),
            "Tampered peak bagging sibling must fail proof generation (unsatisfied constraint)"
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
            derive_nullifier as dn, derive_nullifier_key as dnk, note_commitment as nc,
            MerkleMountainRange,
        };

        let keys = get_or_init_note_circuit_keys();
        let spending_key = Fr::from(42u64);
        let nk = dnk(spending_key);

        let in_value: u64 = 5_035_500; // 5 USDC + fees
        let in_rho = Fr::from(10u64);
        let in_rand = Fr::from(20u64);
        let in_cm = nc(in_value, spending_key, in_rho, in_rand);

        let mut mmr = MerkleMountainRange::new();
        let (leaf_idx, root) = mmr.append(in_cm);
        let proof = mmr.generate_proof(leaf_idx);

        let mut m_sibs = [Fr::from(0u64); 32];
        for (i, s) in proof.mountain_siblings.iter().enumerate() {
            m_sibs[i] = *s;
        }
        let mut p_sibs = [Fr::from(0u64); 32];
        for (i, s) in proof.peak_bagging_siblings.iter().enumerate() {
            p_sibs[i] = *s;
        }

        let nf = dn(nk, in_cm, leaf_idx as u64);

        let merchant: u64 = 5_000_000;
        let pfee: u64 = 12_500;
        let efee: u64 = 23_000;
        // Full spend: no change
        assert_eq!(in_value, merchant + pfee + efee);

        let circuit = PrivateNoteCircuit {
            note_root: Some(root),
            leaf_count: Some(Fr::from(mmr.leaf_count as u64)),
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
            input_leaf_index: Some(Fr::from(leaf_idx as u64)),
            mountain_height: Some(proof.mountain_height as u8),
            mountain_siblings: Some(m_sibs),
            peak_bagging_siblings: Some(p_sibs),
            peak_bagging_count: Some(proof.peak_bagging_siblings.len() as u8),

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

        // Tamper leaf index from 0 to 1 while leaving leaf_count = 1
        // This violates delta = leaf_count - 1 - input_leaf_index = 1 - 1 - 1 = -1 (underflow)
        circuit.input_leaf_index = Some(Fr::from(1u64));

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            generate_note_proof(circuit, &keys.proving_key)
        }));
        assert!(
            result.is_err() || result.unwrap().is_err(),
            "Tampered leaf index >= leaf_count must fail proof generation (anti-hyperbridge)"
        );
    }

    #[test]
    fn test_note_circuit_hyperbridge_out_of_bounds_leaf_index_fails_proving() {
        let keys = get_or_init_note_circuit_keys();
        let mut circuit = setup_valid_circuit();

        // Force leaf_index = 100 on leaf_count = 1
        circuit.input_leaf_index = Some(Fr::from(100u64));

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            generate_note_proof(circuit, &keys.proving_key)
        }));
        assert!(
            result.is_err() || result.unwrap().is_err(),
            "Out of bounds leaf index must fail 64-bit range constraint on delta"
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

        // Swap chain_id (index 9) and contract_address (index 10)
        public_inputs.swap(9, 10);

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
        eprintln!("// Circuit: PrivateNoteCircuit (13 public inputs) over BLS12-381");
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
        eprintln!("pub const NOTE_VK_IC: [[u8; 128]; 14] = [");
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
        let mut public_inputs = [Fr::default(); 13];
        public_inputs.copy_from_slice(&pis[..13]);

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
        tampered_pis[5] += Fr::from(100u64);
        assert!(!verify_evm_note_proof(&a_neg, &b, &c, &tampered_pis));

        // 3. Fake proof points rejected
        let fake_a = [0x11u8; 128];
        assert!(!verify_evm_note_proof(&fake_a, &b, &c, &public_inputs));
    }
}
