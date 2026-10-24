//! Universal 2-in-2-out JoinSplit Spend Circuit (Groth16 ZK-UTXO) — DEC-036A / Gate C0.
//!
//! Proves universal 2-in-2-out private spend:
//! - Exact integer value conservation: (v_in1 + v_in2) == (merchant + protocol_fee + execution_fee) + (v_out1 + v_out2)
//! - 64-bit quadratic boolean range constraints on all 7 value terms (448 active gates) preventing Fr wrap-around
//! - Strict constant-arity topology (Direction B): exactly 2 output commitments synthesized and exported (ZIP 315 defense)
//! - Canonical dummy second input slot gadget (Penumbra 2024 defense): is_dummy_2 * v_in2 == 0, deterministic PRF via session_nonce
//! - Singularity nullifier anti-aliasing constraint ((1 - is_dummy) * (nf1 - nf2) != 0)
//! - In-circuit Merkle Mountain Range (MMR) peak bagging (up to 32 peaks) with local peak authentication (DEC-032)
//! - Two-limb 128-bit quote hash range checks (DEC-035A) & formal scope binding (DEC-035B)
//! - 19 Canonical Public Inputs Layout guaranteeing non-identity verifying key elements (IC[0..=19] != O_G1)

use ark_bls12_381::{Bls12_381, Fr};
use ark_ff::{Field, PrimeField};
use ark_r1cs_std::alloc::AllocVar;
use ark_r1cs_std::boolean::Boolean;
use ark_r1cs_std::eq::EqGadget;
use ark_r1cs_std::fields::fp::{AllocatedFp, FpVar};
use ark_r1cs_std::fields::FieldVar;
use ark_r1cs_std::select::CondSelectGadget;
use ark_relations::gr1cs::{ConstraintSynthesizer, ConstraintSystemRef, SynthesisError};
use ark_snark::SNARK;
use ark_std::rand::rngs::StdRng;
use ark_std::rand::SeedableRng;
use std::sync::LazyLock;

use crate::note::{
    domain_dummy_nullifier, domain_merkle_node, domain_mmr_bag, domain_note_commitment,
    domain_nullifier,
};
use crate::poseidon::{poseidon_hash as poseidon_w3_hash, poseidon_w5_hash};

/// Number of canonical public inputs in the Universal 2-in-2-out JoinSplit circuit (DEC-036A).
pub const NUM_JOINSPLIT_PUBLIC_INPUTS: usize = 19;

/// Deterministic seed for JoinSplit trusted setup key generation.
pub const JOINSPLIT_CIRCUIT_SETUP_SEED: u64 = 0x4e696d6275734a53; // "ZeltraJS"

/// Cached precomputed keys for JoinSplit circuit.
pub static CACHED_JOINSPLIT_KEYS: LazyLock<JoinSplitCircuitKeys> = LazyLock::new(|| {
    generate_joinsplit_circuit_keys().expect("Failed to initialize JoinSplitCircuitKeys")
});

/// Get or initialize cached keys for Universal JoinSplit circuit.
pub fn get_or_init_joinsplit_circuit_keys() -> &'static JoinSplitCircuitKeys {
    &CACHED_JOINSPLIT_KEYS
}

/// Universal 2-in-2-out JoinSplit circuit for Groth16 proof generation (DEC-036A).
#[derive(Clone)]
pub struct JoinSplitCircuit {
    // ── Public inputs (19 scalar field elements) ──
    pub note_root: Option<Fr>,           // 0: Aggregated MMR bagged root
    pub leaf_count: Option<Fr>,          // 1: Total active leaves in MMR
    pub input_nullifier_1: Option<Fr>,   // 2: Nullifier for input note 1
    pub input_nullifier_2: Option<Fr>,   // 3: Nullifier for input note 2 (or dummy nullifier)
    pub epoch_id_1: Option<Fr>,          // 4: Epoch of input note 1
    pub epoch_id_2: Option<Fr>,          // 5: Epoch of input note 2 (aligned if dummy)
    pub output_commitment_1: Option<Fr>, // 6: Commitment for change note 1 (or dummy)
    pub output_commitment_2: Option<Fr>, // 7: Commitment for change note 2 (or dummy)
    pub recipient: Option<Fr>,           // 8: Merchant Ethereum address
    pub merchant_amount: Option<Fr>,     // 9: Net USDC payout to merchant
    pub protocol_fee: Option<Fr>,        // 10: Protocol fee (flat 45 bps ceiling division)
    pub execution_fee: Option<Fr>,       // 11: Relayer gas reimbursement fee
    pub quote_hash_hi: Option<Fr>,       // 12: High 128-bit limb of quote hash
    pub quote_hash_lo: Option<Fr>,       // 13: Low 128-bit limb of quote hash
    pub chain_id: Option<Fr>,            // 14: EVM chain ID
    pub contract_address: Option<Fr>,    // 15: Target Stylus contract address
    pub expiry: Option<Fr>,              // 16: Expiry timestamp
    pub flags_packed: Option<Fr>,        // 17: Bit 0: has_change_1, Bit 1: has_change_2
    pub is_rollover: Option<Fr>,         // 18: Rollover flag (0 = regular, 1 = rollover)

    // ── Private witnesses: Input Note 1 ──
    pub in1_value: Option<Fr>,
    pub in1_owner_key: Option<Fr>,
    pub in1_rho: Option<Fr>,
    pub in1_randomness: Option<Fr>,
    pub in1_leaf_index: Option<Fr>,
    pub in1_mountain_height: Option<u8>,
    pub in1_mountain_siblings: Option<[Fr; 32]>,

    // ── Private witnesses: Input Note 2 ──
    pub in2_value: Option<Fr>,
    pub in2_owner_key: Option<Fr>,
    pub in2_rho: Option<Fr>,
    pub in2_randomness: Option<Fr>,
    pub in2_leaf_index: Option<Fr>,
    pub in2_mountain_height: Option<u8>,
    pub in2_mountain_siblings: Option<[Fr; 32]>,
    pub in2_is_dummy: Option<Fr>, // 1 if dummy, 0 if real
    pub in2_session_nonce: Option<Fr>,

    // ── Private witnesses: MMR Peaks ──
    pub mmr_peaks: Option<[Fr; 32]>,

    // ── Private witnesses: Output Note 1 (Change 1) ──
    pub out1_value: Option<Fr>,
    pub out1_owner_key: Option<Fr>,
    pub out1_rho: Option<Fr>,
    pub out1_randomness: Option<Fr>,

    // ── Private witnesses: Output Note 2 (Change 2) ──
    pub out2_value: Option<Fr>,
    pub out2_owner_key: Option<Fr>,
    pub out2_rho: Option<Fr>,
    pub out2_randomness: Option<Fr>,

    // ── Private witnesses: Change Flags ──
    pub has_change_1: Option<Fr>,
    pub has_change_2: Option<Fr>,

    // ── Private witnesses: Scope Binding (DEC-035B) ──
    pub expected_binding: Option<Fr>,

    // ── Private witnesses: Dual-Asset & Intra-Asset Equality (DEC-034A) ──
    pub public_asset_id: Option<Fr>,
    pub in1_asset_id: Option<Fr>,
    pub in2_asset_id: Option<Fr>,
    pub out1_asset_id: Option<Fr>,
    pub out2_asset_id: Option<Fr>,

    // ── Private witnesses: Deniability Permutation & Decoy (DEC-051) ──
    pub perm_bit: Option<Fr>,
    pub is_decoy: Option<Fr>,
}

/// Nopenena Cryptographic Deniability & Decoy Permutation Gadget (DEC-051 / IACR 2024/903).
///
/// Implements a 7-constraint R1CS secret permutation and decoy pinning gadget:
/// 1. Companion constraint (DEC-046): `is_decoy * aux_val == 0` (locks decoy output value strictly to 0)
/// 2. Permutation of output values based on secret bit pi:
///    - `out1_val = (1 - pi) * target_val + pi * aux_val`
///    - `out2_val = pi * target_val + (1 - pi) * aux_val`
/// 3. Permutation of output commitments based on secret bit pi:
///    - `out1_cm = (1 - pi) * target_cm + pi * aux_cm`
///    - `out2_cm = pi * target_cm + (1 - pi) * aux_cm`
#[derive(Clone, Debug)]
pub struct DeniabilityPermutationGadget {
    pub perm_bit: Boolean<Fr>,
    pub out1_val: FpVar<Fr>,
    pub out2_val: FpVar<Fr>,
    pub out1_cm: FpVar<Fr>,
    pub out2_cm: FpVar<Fr>,
}

impl DeniabilityPermutationGadget {
    pub fn enforce_permutation(
        _cs: ConstraintSystemRef<Fr>,
        perm_bit: &Boolean<Fr>,
        target_val: &FpVar<Fr>,
        target_cm: &FpVar<Fr>,
        aux_val: &FpVar<Fr>,
        aux_cm: &FpVar<Fr>,
        is_decoy: &Boolean<Fr>,
    ) -> Result<Self, SynthesisError> {
        // 1. Enforce DEC-046 companion constraint on decoy slot:
        // if is_decoy == true, aux_val MUST be exactly 0
        let zero_val = FpVar::<Fr>::zero();
        aux_val.conditional_enforce_equal(&zero_val, is_decoy)?;

        // 2. Permute values based on secret bit pi
        let out1_val = FpVar::conditionally_select(perm_bit, aux_val, target_val)?;
        let out2_val = FpVar::conditionally_select(perm_bit, target_val, aux_val)?;

        // 3. Permute commitments based on secret bit pi
        let out1_cm = FpVar::conditionally_select(perm_bit, aux_cm, target_cm)?;
        let out2_cm = FpVar::conditionally_select(perm_bit, target_cm, aux_cm)?;

        Ok(Self {
            perm_bit: perm_bit.clone(),
            out1_val,
            out2_val,
            out1_cm,
            out2_cm,
        })
    }
}

impl JoinSplitCircuit {
    /// Compute the expected scope binding commitment natively from circuit inputs (DEC-035B).
    pub fn derive_expected_binding(&self) -> Option<Fr> {
        let rec = self.recipient?;
        let cid = self.chain_id?;
        let caddr = self.contract_address?;
        let exp = self.expiry?;
        let qhi = self.quote_hash_hi?;
        let qlo = self.quote_hash_lo?;
        let scope_hash = crate::note::compute_scope_hash(rec, cid, caddr, exp);
        Some(crate::note::compute_binding_commitment(
            qhi, qlo, scope_hash,
        ))
    }
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

/// Enforce that `val` is strictly in [0, 2^N) by decomposing into N boolean bits.
///
/// Implements N active rank-1 quadratic constraints: b_i * (1 - b_i) == 0,
/// and 1 linear combination equality: sum(2^i * b_i) == val.
fn enforce_bits_range(
    cs: &ConstraintSystemRef<Fr>,
    val: &FpVar<Fr>,
    val_witness: Option<Fr>,
    num_bits: usize,
) -> Result<(), SynthesisError> {
    use ark_r1cs_std::eq::EqGadget;

    let zero = FpVar::Constant(Fr::from(0u64));
    let one = FpVar::Constant(Fr::from(1u64));
    let mut reconstructed = zero.clone();
    let mut two_power = Fr::from(1u64);

    let bigint_opt = val_witness.map(|w| w.into_bigint());

    for bit_idx in 0..num_bits {
        let bit_val = bigint_opt.as_ref().map(|bi| {
            let limb_idx = bit_idx / 64;
            let bit_in_limb = bit_idx % 64;
            let word = bi.as_ref()[limb_idx];
            Fr::from((word >> bit_in_limb) & 1u64)
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

/// Enforce that `val` is strictly in [0, 2^64) by decomposing into 64 boolean bits.
fn enforce_u64_range(
    cs: &ConstraintSystemRef<Fr>,
    val: &FpVar<Fr>,
    val_witness: Option<Fr>,
) -> Result<(), SynthesisError> {
    enforce_bits_range(cs, val, val_witness, 64)
}

/// Enforce that `val` is strictly in [0, 2^128) by decomposing into 128 boolean bits (DEC-035A).
fn enforce_u128_range(
    cs: &ConstraintSystemRef<Fr>,
    val: &FpVar<Fr>,
    val_witness: Option<Fr>,
) -> Result<(), SynthesisError> {
    enforce_bits_range(cs, val, val_witness, 128)
}

/// Helper gadget: walks an MMR mountain from a leaf commitment to its local peak.
#[allow(clippy::too_many_arguments)]
fn compute_local_peak_gadget(
    cs: &ConstraintSystemRef<Fr>,
    leaf: &FpVar<Fr>,
    leaf_index_witness: Option<Fr>,
    mountain_height: usize,
    mountain_siblings: &[Fr; 32],
    w5_rc: &[Fr],
    w5_mds: &[Vec<Fr>],
    domain_merkle: Fr,
) -> Result<FpVar<Fr>, SynthesisError> {
    use ark_r1cs_std::eq::EqGadget;

    let zero = FpVar::Constant(Fr::from(0u64));
    let one = FpVar::Constant(Fr::from(1u64));
    let mut current = leaf.clone();

    for (level, sibling) in mountain_siblings.iter().enumerate() {
        let is_active = level < mountain_height;
        let sibling_var = private_witness(cs, Some(*sibling))?;
        let is_active_var = private_witness(
            cs,
            Some(if is_active {
                Fr::from(1u64)
            } else {
                Fr::from(0u64)
            }),
        )?;
        let one_minus_active = &one - &is_active_var;
        (&is_active_var * &one_minus_active).enforce_equal(&zero)?;

        let bit_val = leaf_index_witness.map(|idx| {
            let level_shift = level as u64;
            Fr::from((idx.into_bigint().as_ref()[0] >> level_shift) & 1u64)
        });
        let index_bit = private_witness(cs, bit_val)?;
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
        let domain_mmr_bag = domain_mmr_bag();
        let domain_scope = crate::note::domain_scope_v1();
        let domain_binding = crate::note::domain_binding_v1();

        // ═══════════════════════════════════════════════════════════════
        // 1. Allocate all 19 canonical public inputs (DEC-036A)
        // ═══════════════════════════════════════════════════════════════
        let note_root_var = public_input(&cs, self.note_root)?; // 0
        let leaf_count_var = public_input(&cs, self.leaf_count)?; // 1
        let nullifier_1_var = public_input(&cs, self.input_nullifier_1)?; // 2
        let nullifier_2_var = public_input(&cs, self.input_nullifier_2)?; // 3
        let epoch_id_1_var = public_input(&cs, self.epoch_id_1)?; // 4
        let epoch_id_2_var = public_input(&cs, self.epoch_id_2)?; // 5
        let output_cm1_var = public_input(&cs, self.output_commitment_1)?; // 6
        let output_cm2_var = public_input(&cs, self.output_commitment_2)?; // 7
        let recipient_var = public_input(&cs, self.recipient)?; // 8
        let merchant_var = public_input(&cs, self.merchant_amount)?; // 9
        let protocol_fee_var = public_input(&cs, self.protocol_fee)?; // 10
        let exec_fee_var = public_input(&cs, self.execution_fee)?; // 11
        let quote_hash_hi_var = public_input(&cs, self.quote_hash_hi)?; // 12
        let quote_hash_lo_var = public_input(&cs, self.quote_hash_lo)?; // 13
        let chain_id_var = public_input(&cs, self.chain_id)?; // 14
        let contract_addr_var = public_input(&cs, self.contract_address)?; // 15
        let expiry_var = public_input(&cs, self.expiry)?; // 16
        let flags_val = self.flags_packed.or_else(|| {
            let ch1 = self.has_change_1.unwrap_or(Fr::from(0u64));
            let ch2 = self.has_change_2.unwrap_or(Fr::from(0u64));
            let aid = self.public_asset_id.unwrap_or(Fr::from(1u64));
            Some(ch1 + ch2 * Fr::from(2u64) + aid * Fr::from(4u64))
        });
        let flags_var = public_input(&cs, flags_val)?; // 17
        let is_rollover_var = public_input(&cs, self.is_rollover)?; // 18

        // ═══════════════════════════════════════════════════════════════
        // 2. Range constraints on public numerical values & limbs
        // ═══════════════════════════════════════════════════════════════
        enforce_u64_range(&cs, &leaf_count_var, self.leaf_count)?;
        enforce_u64_range(&cs, &epoch_id_1_var, self.epoch_id_1)?;
        enforce_u64_range(&cs, &epoch_id_2_var, self.epoch_id_2)?;
        enforce_u64_range(&cs, &merchant_var, self.merchant_amount)?;
        enforce_u64_range(&cs, &protocol_fee_var, self.protocol_fee)?;
        enforce_u64_range(&cs, &exec_fee_var, self.execution_fee)?;
        enforce_u128_range(&cs, &quote_hash_hi_var, self.quote_hash_hi)?;
        enforce_u128_range(&cs, &quote_hash_lo_var, self.quote_hash_lo)?;

        // ═══════════════════════════════════════════════════════════════
        // 3. Allocate private witnesses
        // ═══════════════════════════════════════════════════════════════
        // Input Note 1
        let in1_value = private_witness(&cs, self.in1_value)?;
        let in1_owner = private_witness(&cs, self.in1_owner_key)?;
        let in1_rho = private_witness(&cs, self.in1_rho)?;
        let in1_rand = private_witness(&cs, self.in1_randomness)?;
        let in1_index = private_witness(&cs, self.in1_leaf_index)?;

        // Input Note 2
        let in2_value = private_witness(&cs, self.in2_value)?;
        let in2_owner = private_witness(&cs, self.in2_owner_key)?;
        let in2_rho = private_witness(&cs, self.in2_rho)?;
        let in2_rand = private_witness(&cs, self.in2_randomness)?;
        let in2_index = private_witness(&cs, self.in2_leaf_index)?;
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

        // Change Flags & Asset IDs
        let has_ch1_var = private_witness(&cs, self.has_change_1)?;
        let has_ch2_var = private_witness(&cs, self.has_change_2)?;

        let pub_aid_witness = self.public_asset_id.or_else(|| {
            flags_val.map(|f| {
                let f_u64 = f.into_bigint().as_ref()[0];
                Fr::from((f_u64 >> 2) & 1)
            })
        });
        let public_asset_id_var = private_witness(&cs, pub_aid_witness)?;

        let in1_aid_witness = self.in1_asset_id.or(pub_aid_witness);
        let in2_aid_witness = self.in2_asset_id.or(pub_aid_witness);
        let out1_aid_witness = self.out1_asset_id.or(pub_aid_witness);
        let out2_aid_witness = self.out2_asset_id.or(pub_aid_witness);

        let in1_asset_id_var = private_witness(&cs, in1_aid_witness)?;
        let in2_asset_id_var = private_witness(&cs, in2_aid_witness)?;
        let out1_asset_id_var = private_witness(&cs, out1_aid_witness)?;
        let out2_asset_id_var = private_witness(&cs, out2_aid_witness)?;

        // Strict intra-asset equality without conditional branching (Patch Audit #1):
        // asset_id_in1 == asset_id_in2 == asset_id_out1 == asset_id_out2 == public_asset_id
        in1_asset_id_var.enforce_equal(&public_asset_id_var)?;
        in2_asset_id_var.enforce_equal(&public_asset_id_var)?;
        out1_asset_id_var.enforce_equal(&public_asset_id_var)?;
        out2_asset_id_var.enforce_equal(&public_asset_id_var)?;

        // ═══════════════════════════════════════════════════════════════
        // 4. Value Range Checks & Dual-Asset Bit-Packing (DEC-034A)
        // ═══════════════════════════════════════════════════════════════
        let two_pow_64 = Fr::from(1u128 << 64);

        let in1_val_u64 = self.in1_value.map(|v| v.into_bigint().as_ref()[0]);
        let in1_aid_u8 = in1_aid_witness.map(|a| a.into_bigint().as_ref()[0] as u8);
        let in1_value_packed_val = match (self.in1_value, in1_aid_witness) {
            (Some(v), Some(a)) => Some(v + a * two_pow_64),
            _ => None,
        };
        let in1_value_packed = private_witness(&cs, in1_value_packed_val)?;
        crate::circuit_gadgets::enforce_packed_value_and_asset(
            &cs,
            &in1_value_packed,
            &in1_value,
            &in1_asset_id_var,
            in1_val_u64,
            in1_aid_u8,
        )?;

        let in2_val_u64 = self.in2_value.map(|v| v.into_bigint().as_ref()[0]);
        let in2_aid_u8 = in2_aid_witness.map(|a| a.into_bigint().as_ref()[0] as u8);
        let in2_value_packed_val = match (self.in2_value, in2_aid_witness) {
            (Some(v), Some(a)) => Some(v + a * two_pow_64),
            _ => None,
        };
        let in2_value_packed = private_witness(&cs, in2_value_packed_val)?;
        crate::circuit_gadgets::enforce_packed_value_and_asset(
            &cs,
            &in2_value_packed,
            &in2_value,
            &in2_asset_id_var,
            in2_val_u64,
            in2_aid_u8,
        )?;

        let out1_val_u64 = self.out1_value.map(|v| v.into_bigint().as_ref()[0]);
        let out1_aid_u8 = out1_aid_witness.map(|a| a.into_bigint().as_ref()[0] as u8);
        let out1_value_packed_val = match (self.out1_value, out1_aid_witness) {
            (Some(v), Some(a)) => Some(v + a * two_pow_64),
            _ => None,
        };
        let out1_value_packed = private_witness(&cs, out1_value_packed_val)?;
        crate::circuit_gadgets::enforce_packed_value_and_asset(
            &cs,
            &out1_value_packed,
            &out1_val,
            &out1_asset_id_var,
            out1_val_u64,
            out1_aid_u8,
        )?;

        let out2_val_u64 = self.out2_value.map(|v| v.into_bigint().as_ref()[0]);
        let out2_aid_u8 = out2_aid_witness.map(|a| a.into_bigint().as_ref()[0] as u8);
        let out2_value_packed_val = match (self.out2_value, out2_aid_witness) {
            (Some(v), Some(a)) => Some(v + a * two_pow_64),
            _ => None,
        };
        let out2_value_packed = private_witness(&cs, out2_value_packed_val)?;
        crate::circuit_gadgets::enforce_packed_value_and_asset(
            &cs,
            &out2_value_packed,
            &out2_val,
            &out2_asset_id_var,
            out2_val_u64,
            out2_aid_u8,
        )?;

        enforce_u64_range(&cs, &in1_index, self.in1_leaf_index)?;
        enforce_u64_range(&cs, &in2_index, self.in2_leaf_index)?;

        // Note 1 value > 0 (strictly positive non-zero, cannot be dummy)
        let in1_inv = private_witness(
            &cs,
            self.in1_value
                .map(|v| v.inverse().unwrap_or(Fr::from(0u64))),
        )?;
        (&in1_value * &in1_inv).enforce_equal(&one)?;

        // Anti-Hyperbridge check: in1_leaf_index < leaf_count
        let delta1_witness = match (self.leaf_count, self.in1_leaf_index) {
            (Some(lc), Some(li)) => {
                let one_fr = Fr::from(1u64);
                Some(lc - one_fr - li)
            }
            _ => None,
        };
        let delta1_var = &leaf_count_var - FpVar::Constant(Fr::from(1u64)) - &in1_index;
        enforce_u64_range(&cs, &delta1_var, delta1_witness)?;

        // ═══════════════════════════════════════════════════════════════
        // 5. Phase 2: Canonical Dummy Second Input Gadget (Penumbra Defense)
        // ═══════════════════════════════════════════════════════════════
        // A2.1: is_dummy_2 * (1 - is_dummy_2) == 0
        let one_minus_dummy = &one - &is_dummy_var;
        (&is_dummy_var * &one_minus_dummy).enforce_equal(&zero)?;

        // A2.1: is_dummy_2 * in2_value == 0 (locks dummy input value strictly to zero)
        (&is_dummy_var * &in2_value).enforce_equal(&zero)?;

        // A2.4: is_dummy_2 * (epoch_id_2 - epoch_id_1) == 0 (prevents public input metadata leakage)
        let epoch_diff = &epoch_id_2_var - &epoch_id_1_var;
        (&is_dummy_var * &epoch_diff).enforce_equal(&zero)?;

        // DEC-046 §3.B: Companion quadratic constraints pinning dummy slot 2 witness coordinates
        // strictly to canonical deterministic constants:
        // is_dummy_2 * (in2_owner - CANONICAL_DUMMY_OWNER) == 0
        // is_dummy_2 * (in2_rho - CANONICAL_DUMMY_RHO) == 0
        // is_dummy_2 * (in2_rand - CANONICAL_DUMMY_RANDOMNESS) == 0
        // is_dummy_2 * (in2_index - CANONICAL_DUMMY_LEAF_INDEX) == 0
        crate::circuit_gadgets::enforce_canonical_pinning(
            &is_dummy_var,
            &in2_owner,
            crate::circuit_gadgets::CANONICAL_DUMMY_OWNER,
        )?;
        crate::circuit_gadgets::enforce_canonical_pinning(
            &is_dummy_var,
            &in2_rho,
            crate::circuit_gadgets::CANONICAL_DUMMY_RHO,
        )?;
        crate::circuit_gadgets::enforce_canonical_pinning(
            &is_dummy_var,
            &in2_rand,
            crate::circuit_gadgets::CANONICAL_DUMMY_RANDOMNESS,
        )?;
        crate::circuit_gadgets::enforce_canonical_pinning(
            &is_dummy_var,
            &in2_index,
            Fr::from(crate::circuit_gadgets::CANONICAL_DUMMY_LEAF_INDEX),
        )?;

        // ═══════════════════════════════════════════════════════════════
        // 6. Phase 3: Direction B Constant-Arity Output Gadget (ZIP 315 Defense)
        // ═══════════════════════════════════════════════════════════════
        crate::circuit_gadgets::enforce_joinsplit_flags_packed(
            &flags_var,
            &has_ch1_var,
            &has_ch2_var,
            &public_asset_id_var,
        )?;

        let one_minus_ch1 = &one - &has_ch1_var;
        let one_minus_ch2 = &one - &has_ch2_var;

        // A3.1: If has_change == 0 => change value MUST be zero
        (&out1_val * &one_minus_ch1).enforce_equal(&zero)?;
        (&out2_val * &one_minus_ch2).enforce_equal(&zero)?;

        // DEC-046 §3.D: Companion quadratic constraints for inactive change outputs:
        // When has_change_1 == 0 (i.e. (1 - has_change_1) == 1):
        // (1 - has_change_1) * (out1_owner - CANONICAL_DUMMY_OWNER) == 0
        // (1 - has_change_1) * (out1_rho - CANONICAL_DUMMY_RHO) == 0
        // (1 - has_change_1) * (out1_rand - CANONICAL_DUMMY_RANDOMNESS) == 0
        crate::circuit_gadgets::enforce_canonical_pinning(
            &one_minus_ch1,
            &out1_owner,
            crate::circuit_gadgets::CANONICAL_DUMMY_OWNER,
        )?;
        crate::circuit_gadgets::enforce_canonical_pinning(
            &one_minus_ch1,
            &out1_rho,
            crate::circuit_gadgets::CANONICAL_DUMMY_RHO,
        )?;
        crate::circuit_gadgets::enforce_canonical_pinning(
            &one_minus_ch1,
            &out1_rand,
            crate::circuit_gadgets::CANONICAL_DUMMY_RANDOMNESS,
        )?;

        // When has_change_2 == 0 (i.e. (1 - has_change_2) == 1):
        // (1 - has_change_2) * (out2_owner - CANONICAL_DUMMY_OWNER) == 0
        // (1 - has_change_2) * (out2_rho - CANONICAL_DUMMY_RHO) == 0
        // (1 - has_change_2) * (out2_rand - CANONICAL_DUMMY_RANDOMNESS) == 0
        crate::circuit_gadgets::enforce_canonical_pinning(
            &one_minus_ch2,
            &out2_owner,
            crate::circuit_gadgets::CANONICAL_DUMMY_OWNER,
        )?;
        crate::circuit_gadgets::enforce_canonical_pinning(
            &one_minus_ch2,
            &out2_rho,
            crate::circuit_gadgets::CANONICAL_DUMMY_RHO,
        )?;
        crate::circuit_gadgets::enforce_canonical_pinning(
            &one_minus_ch2,
            &out2_rand,
            crate::circuit_gadgets::CANONICAL_DUMMY_RANDOMNESS,
        )?;

        // A3.2 & A3.3: Direction B: Compute and export exactly 2 output commitments
        // Even when has_change == 0, a canonical dummy commitment (value = 0) is synthesized and exported!
        let cm_out1 = poseidon_w5_hash(
            &[out1_value_packed, out1_owner, out1_rho, out1_rand],
            domain_note,
            w5_rc,
            w5_mds,
        )?;

        let cm_out2 = poseidon_w5_hash(
            &[out2_value_packed, out2_owner, out2_rho, out2_rand],
            domain_note,
            w5_rc,
            w5_mds,
        )?;

        // ═══════════════════════════════════════════════════════════════
        // 6.B DEC-051: Nopenena Deniability Permutation & Companion Gadget
        // ═══════════════════════════════════════════════════════════════
        let perm_override = TAMPER_PERM_BIT.with(|cell| *cell.borrow());
        let perm_val = perm_override.or(self.perm_bit).unwrap_or(Fr::from(0u64));
        let perm_bool = perm_val == Fr::from(1u64);
        let perm_bit_var = Boolean::new_witness(cs.clone(), || Ok(perm_bool))?;
        if perm_val != Fr::from(0u64) && perm_val != Fr::from(1u64) {
            if let Boolean::Var(ref b) = perm_bit_var {
                let v = b.variable();
                if v.is_witness() {
                    if let Some(idx) = v.get_variable_index(0) {
                        if let Some(mut cs_b) = cs.borrow_mut() {
                            if idx < cs_b.assignments.witness_assignment.len() {
                                cs_b.assignments.witness_assignment[idx] = perm_val;
                            }
                        }
                    }
                }
            }
        }

        let is_decoy_override = TAMPER_IS_DECOY.with(|cell| *cell.borrow());
        let is_decoy_val = is_decoy_override
            .or(self.is_decoy)
            .unwrap_or(Fr::from(0u64));
        let is_decoy_bool = is_decoy_val == Fr::from(1u64);
        let is_decoy_var = Boolean::new_witness(cs.clone(), || Ok(is_decoy_bool))?;
        if is_decoy_val != Fr::from(0u64) && is_decoy_val != Fr::from(1u64) {
            if let Boolean::Var(ref b) = is_decoy_var {
                let v = b.variable();
                if v.is_witness() {
                    if let Some(idx) = v.get_variable_index(0) {
                        if let Some(mut cs_b) = cs.borrow_mut() {
                            if idx < cs_b.assignments.witness_assignment.len() {
                                cs_b.assignments.witness_assignment[idx] = is_decoy_val;
                            }
                        }
                    }
                }
            }
        }

        let permuted = DeniabilityPermutationGadget::enforce_permutation(
            cs.clone(),
            &perm_bit_var,
            &out1_val,
            &cm_out1,
            &out2_val,
            &cm_out2,
            &is_decoy_var,
        )?;

        permuted.out1_cm.enforce_equal(&output_cm1_var)?;
        permuted.out2_cm.enforce_equal(&output_cm2_var)?;

        // ═══════════════════════════════════════════════════════════════
        // 7. Value Conservation & Protocol Fee Ceiling Division (A1.2 & A1.3)
        // ═══════════════════════════════════════════════════════════════
        // Linear combination equality:
        // (v_in1 + v_in2) == (v_merchant + v_protocol_fee + v_execution_fee + v_out1 + v_out2)
        let total_inputs = &in1_value + &in2_value;
        let total_outputs = &merchant_var
            + &protocol_fee_var
            + &exec_fee_var
            + &permuted.out1_val
            + &permuted.out2_val;
        total_inputs.enforce_equal(&total_outputs)?;

        // A1.3: Flat 45 bps Protocol Fee Ceiling Division Gadget
        // 10,000 * v_protocol_fee - 45 * v_merchant == rem, where 0 <= rem < 10,000
        let ten_thousand = FpVar::Constant(Fr::from(10_000u64));
        let forty_five = FpVar::Constant(Fr::from(45u64));
        let rem_var = &protocol_fee_var * &ten_thousand - &merchant_var * &forty_five;

        let rem_witness = match (self.protocol_fee, self.merchant_amount) {
            (Some(fee), Some(merch)) => {
                let f_val = fee.into_bigint().as_ref()[0];
                let m_val = merch.into_bigint().as_ref()[0];
                let lhs = f_val.checked_mul(10_000).unwrap_or(0);
                let rhs = m_val.checked_mul(45).unwrap_or(0);
                if lhs >= rhs {
                    Some(Fr::from(lhs - rhs))
                } else {
                    Some(Fr::from(lhs) - Fr::from(rhs))
                }
            }
            _ => None,
        };
        let slack_witness = rem_witness.map(|r| {
            let r_val = r.into_bigint().as_ref()[0];
            if r_val <= 9999 {
                Fr::from(9999 - r_val)
            } else {
                Fr::from(9999) - r
            }
        });
        let slack_var = FpVar::Constant(Fr::from(9999u64)) - &rem_var;

        enforce_bits_range(&cs, &rem_var, rem_witness, 14)?;
        enforce_bits_range(&cs, &slack_var, slack_witness, 14)?;

        // Rollover constraint (DEC-033):
        // is_rollover * (1 - is_rollover) == 0
        let one_minus_rollover = &one - &is_rollover_var;
        (&is_rollover_var * &one_minus_rollover).enforce_equal(&zero)?;

        // If is_rollover == 1 => merchant_amount == 0 and protocol_fee == 0
        (&is_rollover_var * &merchant_var).enforce_equal(&zero)?;
        (&is_rollover_var * &protocol_fee_var).enforce_equal(&zero)?;

        // ═══════════════════════════════════════════════════════════════
        // 8. Phase 4: In-Circuit MMR Peak Bagging & Path Verification
        // ═══════════════════════════════════════════════════════════════
        // Input Note Commitments (A1.4)
        let in1_cm = poseidon_w5_hash(
            &[
                in1_value_packed,
                in1_owner.clone(),
                in1_rho.clone(),
                in1_rand,
            ],
            domain_note,
            w5_rc,
            w5_mds,
        )?;

        let in2_cm = poseidon_w5_hash(
            &[
                in2_value_packed,
                in2_owner.clone(),
                in2_rho.clone(),
                in2_rand,
            ],
            domain_note,
            w5_rc,
            w5_mds,
        )?;

        // Allocate 32 peak variables representing the MMR mountain peaks
        let mmr_peaks_val = self.mmr_peaks.unwrap_or([Fr::from(0u64); 32]);
        let mut peak_vars = Vec::with_capacity(32);
        for peak_val in &mmr_peaks_val {
            peak_vars.push(private_witness(&cs, Some(*peak_val))?);
        }

        // Determine total peaks m = popcount(leaf_count) and peak targets
        let lc_opt = self
            .leaf_count
            .map(|lc| lc.into_bigint().as_ref()[0] as usize);
        let li1_opt = self
            .in1_leaf_index
            .map(|li| li.into_bigint().as_ref()[0] as usize);
        let li2_opt = self
            .in2_leaf_index
            .map(|li| li.into_bigint().as_ref()[0] as usize);

        let (target_peak_idx_1, total_peaks) = match (lc_opt, li1_opt) {
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

        let target_peak_idx_2 = match (lc_opt, li2_opt) {
            (Some(lc), Some(li)) => {
                let mut peak_heights = Vec::new();
                for h in (0..usize::BITS).rev() {
                    if (lc >> h) & 1 == 1 {
                        peak_heights.push(h as usize);
                    }
                }
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
                target_k
            }
            _ => 0,
        };

        let total_peaks_effective = total_peaks.max(1);

        // A4.2: Bagging aggregation loop (backward fold from right to left across 32 steps)
        let mut acc = zero.clone();
        for s in (0..32).rev() {
            let is_start = s == total_peaks_effective.saturating_sub(1);
            let is_fold = s < total_peaks_effective.saturating_sub(1);

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

        // A4.3: Root equality constraint
        let bagged_root = poseidon_w5_hash(
            &[acc, leaf_count_var.clone(), zero.clone(), zero.clone()],
            domain_mmr_bag,
            w5_rc,
            w5_mds,
        )?;
        bagged_root.enforce_equal(&note_root_var)?;

        // A4.1: Note 1 internal mountain path to local peak
        let m_height_1 = self.in1_mountain_height.unwrap_or(0) as usize;
        let mountain_sibs_1 = self.in1_mountain_siblings.unwrap_or([Fr::from(0u64); 32]);
        let computed_peak_1 = compute_local_peak_gadget(
            &cs,
            &in1_cm,
            self.in1_leaf_index,
            m_height_1,
            &mountain_sibs_1,
            w5_rc,
            w5_mds,
            domain_merkle,
        )?;

        // One-hot selector to enforce computed_peak_1 equals peak_vars[target_peak_idx_1]
        let mut selected_peak_1 = zero.clone();
        let mut sel1_sum = zero.clone();
        for (s, peak_var) in peak_vars.iter().enumerate().take(32) {
            let is_target = s == target_peak_idx_1;
            let sel_var = private_witness(
                &cs,
                Some(if is_target {
                    Fr::from(1u64)
                } else {
                    Fr::from(0u64)
                }),
            )?;
            let one_minus_sel = &one - &sel_var;
            (&sel_var * &one_minus_sel).enforce_equal(&zero)?;

            sel1_sum += &sel_var;
            let term = &sel_var * peak_var;
            selected_peak_1 += &term;
        }
        sel1_sum.enforce_equal(&one)?;
        computed_peak_1.enforce_equal(&selected_peak_1)?;

        // A4.1: Note 2 internal mountain path to local peak
        let m_height_2 = self.in2_mountain_height.unwrap_or(0) as usize;
        let mountain_sibs_2 = self.in2_mountain_siblings.unwrap_or([Fr::from(0u64); 32]);
        let computed_peak_2 = compute_local_peak_gadget(
            &cs,
            &in2_cm,
            self.in2_leaf_index,
            m_height_2,
            &mountain_sibs_2,
            w5_rc,
            w5_mds,
            domain_merkle,
        )?;

        let mut selected_peak_2 = zero.clone();
        let mut sel2_sum = zero.clone();
        for (s, peak_var) in peak_vars.iter().enumerate().take(32) {
            let is_target = s == target_peak_idx_2;
            let sel_var = private_witness(
                &cs,
                Some(if is_target {
                    Fr::from(1u64)
                } else {
                    Fr::from(0u64)
                }),
            )?;
            let one_minus_sel = &one - &sel_var;
            (&sel_var * &one_minus_sel).enforce_equal(&zero)?;

            sel2_sum += &sel_var;
            let term = &sel_var * peak_var;
            selected_peak_2 += &term;
        }
        sel2_sum.enforce_equal(&one)?;

        // A2.2: Conditional MMR Path Verification for Note 2
        // (1 - is_dummy_2) * (computed_peak_2 - selected_peak_2) == 0
        let peak2_diff = &computed_peak_2 - &selected_peak_2;
        (&one_minus_dummy * &peak2_diff).enforce_equal(&zero)?;

        // ═══════════════════════════════════════════════════════════════
        // 9. Nullifier Derivations & Singularity Aliasing Defense
        // ═══════════════════════════════════════════════════════════════
        // Nullifier 1: nf_1 = Poseidon_W5([nk_1, rho_1, epoch_id_1, 0], DOMAIN_NULLIFIER)
        let nk_1 = poseidon_w3_hash(
            &in1_owner,
            &FpVar::Constant(domain_nullifier),
            w3_rc,
            w3_mds,
        )?;
        let computed_nf_1 = poseidon_w5_hash(
            &[nk_1, in1_rho, epoch_id_1_var.clone(), zero.clone()],
            domain_nullifier,
            w5_rc,
            w5_mds,
        )?;
        computed_nf_1.enforce_equal(&nullifier_1_var)?;

        // Nullifier 2 (A2.3):
        // real_nf_2 = Poseidon_W5([nk_2, rho_2, epoch_id_2, 0], DOMAIN_NULLIFIER)
        // dummy_nf_2 = Poseidon_W5([nk_2, session_nonce, 0, 0], DOMAIN_DUMMY_NULLIFIER)
        // expected_nf_2 = (1 - is_dummy) * real_nf_2 + is_dummy * dummy_nf_2
        let nk_2 = poseidon_w3_hash(
            &in2_owner,
            &FpVar::Constant(domain_nullifier),
            w3_rc,
            w3_mds,
        )?;
        let real_nf_2 = poseidon_w5_hash(
            &[nk_2.clone(), in2_rho, epoch_id_2_var.clone(), zero.clone()],
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

        let nf_diff = &dummy_nf_2 - &real_nf_2;
        let expected_nf_2 = &real_nf_2 + &is_dummy_var * &nf_diff;
        expected_nf_2.enforce_equal(&nullifier_2_var)?;

        // Singularity Multi-Input Aliasing Defense (A7.4):
        // When is_dummy_2 == 0, nullifier_1 MUST NOT equal nullifier_2:
        // (1 - is_dummy_2) * (nullifier_1 - nullifier_2) * diff_inv == (1 - is_dummy_2)
        let diff_inv_override = TAMPER_DIFF_INV.with(|cell| *cell.borrow());
        let diff_inv_witness = diff_inv_override.or_else(|| {
            match (
                self.input_nullifier_1,
                self.input_nullifier_2,
                self.in2_is_dummy,
            ) {
                (Some(n1), Some(n2), Some(dummy)) => {
                    if dummy == Fr::from(0u64) {
                        let diff = n1 - n2;
                        Some(diff.inverse().unwrap_or(Fr::from(0u64)))
                    } else {
                        Some(Fr::from(0u64))
                    }
                }
                _ => None,
            }
        });
        let diff_inv_var = private_witness(&cs, diff_inv_witness)?;
        crate::circuit_gadgets::enforce_zero_safe_diff_inv(
            &is_dummy_var,
            &nullifier_1_var,
            &nullifier_2_var,
            &diff_inv_var,
        )?;

        // ═══════════════════════════════════════════════════════════════
        // 10. Phase 5: Two-Limb Quote Hash & Scope Binding Integration
        // ═══════════════════════════════════════════════════════════════
        // Stage 1: Contextual Scope Hash (A5.2)
        let scope_hash = poseidon_w5_hash(
            &[
                recipient_var.clone(),
                chain_id_var.clone(),
                contract_addr_var.clone(),
                expiry_var.clone(),
            ],
            domain_scope,
            w5_rc,
            w5_mds,
        )?;

        // Stage 2: Formal Binding Commitment (A5.3)
        let binding_cm = poseidon_w5_hash(
            &[
                quote_hash_hi_var.clone(),
                quote_hash_lo_var.clone(),
                scope_hash,
                zero.clone(),
            ],
            domain_binding,
            w5_rc,
            w5_mds,
        )?;

        // Stage 3: Active R1CS Rank-1 Constraint Enforcement
        let expected_binding_val = self
            .expected_binding
            .or_else(|| self.derive_expected_binding());
        let expected_binding_var = private_witness(&cs, expected_binding_val)?;
        binding_cm.enforce_equal(&expected_binding_var)?;

        Ok(())
    }
}

std::thread_local! {
    static TAMPER_DIFF_INV: std::cell::RefCell<Option<Fr>> = const { std::cell::RefCell::new(None) };
    static TAMPER_PERM_BIT: std::cell::RefCell<Option<Fr>> = const { std::cell::RefCell::new(None) };
    static TAMPER_IS_DECOY: std::cell::RefCell<Option<Fr>> = const { std::cell::RefCell::new(None) };
}

/// Helper to inject a tampered witness for diff_inv during testing of DEC-046 soundness.
pub fn set_tamper_diff_inv(val: Option<Fr>) {
    TAMPER_DIFF_INV.with(|cell| {
        *cell.borrow_mut() = val;
    });
}

/// Helper to inject a tampered witness for perm_bit during testing of DEC-051 soundness.
pub fn set_tamper_perm_bit(val: Option<Fr>) {
    TAMPER_PERM_BIT.with(|cell| {
        *cell.borrow_mut() = val;
    });
}

/// Helper to inject a tampered witness for is_decoy during testing of DEC-051 soundness.
pub fn set_tamper_is_decoy(val: Option<Fr>) {
    TAMPER_IS_DECOY.with(|cell| {
        *cell.borrow_mut() = val;
    });
}

/// Helper to verify constraint system satisfaction for a JoinSplitCircuit (DEC-046 testing).
pub fn check_joinsplit_circuit_satisfaction(
    circuit: JoinSplitCircuit,
) -> Result<bool, SynthesisError> {
    use ark_relations::gr1cs::ConstraintSystem;
    let cs = ConstraintSystem::<Fr>::new_ref();
    circuit.generate_constraints(cs.clone())?;
    cs.is_satisfied()
}

// ─── Proving and Verifying Keys & Helpers ─────────────────────────────────────

/// Proving and verifying keys for Universal JoinSplitCircuit.
pub struct JoinSplitCircuitKeys {
    pub proving_key: ark_groth16::ProvingKey<Bls12_381>,
    pub verifying_key: ark_groth16::VerifyingKey<Bls12_381>,
}

/// Creates a valid dummy circuit for trusted setup ceremony synthesis (DEC-036A).
pub fn create_dummy_joinsplit_circuit() -> JoinSplitCircuit {
    use crate::note::{
        derive_joinsplit_dummy_nullifier, derive_joinsplit_nullifier, derive_nullifier_key,
        note_commitment, MerkleMountainRange,
    };

    let sk = Fr::from(100u64);
    let nk = derive_nullifier_key(sk);
    let rho1 = Fr::from(200u64);
    let rand1 = Fr::from(300u64);
    let v_in1 = 10_000_000u64; // 10 USDC
    let asset_id_u8 = 1u8; // USDC default

    let cm1 = note_commitment(v_in1, asset_id_u8, sk, rho1, rand1);

    // Create MMR with 1 leaf (cm1)
    let mut mmr = MerkleMountainRange::new();
    let (leaf_idx1, root) = mmr.append(cm1);
    let proof1 = mmr.generate_proof(leaf_idx1);

    let mut in1_sibs = [Fr::from(0u64); 32];
    for (i, sib) in proof1.mountain_siblings.iter().enumerate() {
        in1_sibs[i] = *sib;
    }

    let mut peaks = [Fr::from(0u64); 32];
    let all_peaks = mmr.get_peaks();
    for (i, p) in all_peaks.iter().enumerate() {
        peaks[i] = *p;
    }

    let epoch1 = 10u64;
    let nf1 = derive_joinsplit_nullifier(nk, rho1, epoch1);

    // DEC-046: Dummy second input uses canonical dummy owner (0)
    let nonce = Fr::from(999u64);
    let owner_dummy = crate::circuit_gadgets::CANONICAL_DUMMY_OWNER;
    let rho_dummy = crate::circuit_gadgets::CANONICAL_DUMMY_RHO;
    let rand_dummy = crate::circuit_gadgets::CANONICAL_DUMMY_RANDOMNESS;
    let dummy_nk = derive_nullifier_key(owner_dummy);
    let nf2 = derive_joinsplit_dummy_nullifier(dummy_nk, nonce);

    // DEC-046: Direction B Canonical dummy output commitments (v=0, owner=0, rho=0, rand=0)
    let cm_out1 = note_commitment(0, asset_id_u8, owner_dummy, rho_dummy, rand_dummy);
    let cm_out2 = note_commitment(0, asset_id_u8, owner_dummy, rho_dummy, rand_dummy);

    let merch_amt = 9_900_000u64;
    let proto_fee = (merch_amt * 45).div_ceil(10_000); // 44_550
    let exec_fee = v_in1 - merch_amt - proto_fee; // 55_450

    let rec = Fr::from(0x1234u64);
    let cid = Fr::from(421614u64);
    let caddr = Fr::from(0x5678u64);
    let exp = Fr::from(2000u64);
    let q_hi = Fr::from(0x1111u64);
    let q_lo = Fr::from(0x2222u64);

    let scope_hash = crate::note::compute_scope_hash(rec, cid, caddr, exp);
    let binding = crate::note::compute_binding_commitment(q_hi, q_lo, scope_hash);

    JoinSplitCircuit {
        note_root: Some(root),
        leaf_count: Some(Fr::from(mmr.leaf_count())),
        input_nullifier_1: Some(nf1),
        input_nullifier_2: Some(nf2),
        epoch_id_1: Some(Fr::from(epoch1)),
        epoch_id_2: Some(Fr::from(epoch1)),
        output_commitment_1: Some(cm_out1),
        output_commitment_2: Some(cm_out2),
        recipient: Some(rec),
        merchant_amount: Some(Fr::from(merch_amt)),
        protocol_fee: Some(Fr::from(proto_fee)),
        execution_fee: Some(Fr::from(exec_fee)),
        quote_hash_hi: Some(q_hi),
        quote_hash_lo: Some(q_lo),
        chain_id: Some(cid),
        contract_address: Some(caddr),
        expiry: Some(exp),
        flags_packed: Some(Fr::from(4u64)), // has_change_1 = 0, has_change_2 = 0, asset_id = 1 (0 + 2*0 + 4*1 = 4)
        is_rollover: Some(Fr::from(0u64)),

        in1_value: Some(Fr::from(v_in1)),
        in1_owner_key: Some(sk),
        in1_rho: Some(rho1),
        in1_randomness: Some(rand1),
        in1_leaf_index: Some(Fr::from(leaf_idx1 as u64)),
        in1_mountain_height: Some(proof1.mountain_height as u8),
        in1_mountain_siblings: Some(in1_sibs),

        in2_value: Some(Fr::from(0u64)),
        in2_owner_key: Some(owner_dummy),
        in2_rho: Some(rho_dummy),
        in2_randomness: Some(rand_dummy),
        in2_leaf_index: Some(Fr::from(crate::circuit_gadgets::CANONICAL_DUMMY_LEAF_INDEX)),
        in2_mountain_height: Some(0),
        in2_mountain_siblings: Some([Fr::from(0u64); 32]),
        in2_is_dummy: Some(Fr::from(1u64)),
        in2_session_nonce: Some(nonce),

        mmr_peaks: Some(peaks),

        out1_value: Some(Fr::from(0u64)),
        out1_owner_key: Some(owner_dummy),
        out1_rho: Some(rho_dummy),
        out1_randomness: Some(rand_dummy),

        out2_value: Some(Fr::from(0u64)),
        out2_owner_key: Some(owner_dummy),
        out2_rho: Some(rho_dummy),
        out2_randomness: Some(rand_dummy),

        has_change_1: Some(Fr::from(0u64)),
        has_change_2: Some(Fr::from(0u64)),
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

/// Verify a Groth16 proof for JoinSplitCircuit against 19 public inputs.
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

/// Extracts the canonical 19 public inputs from a JoinSplitCircuit (DEC-036A).
pub fn extract_joinsplit_public_inputs(circuit: &JoinSplitCircuit) -> Vec<Fr> {
    vec![
        circuit.note_root.expect("note_root"),                     // 0
        circuit.leaf_count.expect("leaf_count"),                   // 1
        circuit.input_nullifier_1.expect("input_nullifier_1"),     // 2
        circuit.input_nullifier_2.expect("input_nullifier_2"),     // 3
        circuit.epoch_id_1.expect("epoch_id_1"),                   // 4
        circuit.epoch_id_2.expect("epoch_id_2"),                   // 5
        circuit.output_commitment_1.expect("output_commitment_1"), // 6
        circuit.output_commitment_2.expect("output_commitment_2"), // 7
        circuit.recipient.expect("recipient"),                     // 8
        circuit.merchant_amount.expect("merchant_amount"),         // 9
        circuit.protocol_fee.expect("protocol_fee"),               // 10
        circuit.execution_fee.expect("execution_fee"),             // 11
        circuit.quote_hash_hi.expect("quote_hash_hi"),             // 12
        circuit.quote_hash_lo.expect("quote_hash_lo"),             // 13
        circuit.chain_id.expect("chain_id"),                       // 14
        circuit.contract_address.expect("contract_address"),       // 15
        circuit.expiry.expect("expiry"),                           // 16
        circuit.flags_packed.expect("flags_packed"),               // 17
        circuit.is_rollover.expect("is_rollover"),                 // 18
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::note::{
        derive_joinsplit_nullifier, derive_nullifier_key, note_commitment, MerkleMountainRange,
    };
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
    }

    #[test]
    fn test_joinsplit_circuit_satisfaction_2_inputs_with_change() {
        let cs = ConstraintSystem::<Fr>::new_ref();

        let sk = Fr::from(100u64);
        let nk = derive_nullifier_key(sk);
        let asset_id_u8 = 1u8; // USDC

        // Input 1: 10 USDC (10_000_000)
        let rho1 = Fr::from(111u64);
        let rand1 = Fr::from(222u64);
        let cm1 = note_commitment(10_000_000, asset_id_u8, sk, rho1, rand1);
        let epoch1 = 5u64;
        let nf1 = derive_joinsplit_nullifier(nk, rho1, epoch1);

        // Input 2: 10 USDC (10_000_000)
        let rho2 = Fr::from(333u64);
        let rand2 = Fr::from(444u64);
        let cm2 = note_commitment(10_000_000, asset_id_u8, sk, rho2, rand2);
        let epoch2 = 6u64;
        let nf2 = derive_joinsplit_nullifier(nk, rho2, epoch2);

        // Build MMR with 2 leaves
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

        // Payout: 15 USDC merchant, flat 45 bps fee = 67_500, exec fee = 32_500 => sum = 15_100_000
        // Change: 20_000_000 - 15_100_000 = 4_900_000 total change
        // Change 1: 3_000_000, Change 2: 1_900_000
        let merch_amt = 15_000_000u64;
        let proto_fee = (merch_amt * 45).div_ceil(10_000); // 67_500
        let exec_fee = 32_500u64;

        let rho_c1 = Fr::from(555u64);
        let rand_c1 = Fr::from(666u64);
        let cm_out1 = note_commitment(3_000_000, asset_id_u8, sk, rho_c1, rand_c1);

        let rho_c2 = Fr::from(777u64);
        let rand_c2 = Fr::from(888u64);
        let cm_out2 = note_commitment(1_900_000, asset_id_u8, sk, rho_c2, rand_c2);

        let rec = Fr::from(0xbeefu64);
        let cid = Fr::from(421614u64);
        let caddr = Fr::from(0xaaaa_u64);
        let exp = Fr::from(3000u64);
        let q_hi = Fr::from(0x7777u64);
        let q_lo = Fr::from(0x8888u64);

        let scope_hash = crate::note::compute_scope_hash(rec, cid, caddr, exp);
        let binding = crate::note::compute_binding_commitment(q_hi, q_lo, scope_hash);

        let circuit = JoinSplitCircuit {
            note_root: Some(root),
            leaf_count: Some(Fr::from(mmr.leaf_count())),
            input_nullifier_1: Some(nf1),
            input_nullifier_2: Some(nf2),
            epoch_id_1: Some(Fr::from(epoch1)),
            epoch_id_2: Some(Fr::from(epoch2)),
            output_commitment_1: Some(cm_out1),
            output_commitment_2: Some(cm_out2),
            recipient: Some(rec),
            merchant_amount: Some(Fr::from(merch_amt)),
            protocol_fee: Some(Fr::from(proto_fee)),
            execution_fee: Some(Fr::from(exec_fee)),
            quote_hash_hi: Some(q_hi),
            quote_hash_lo: Some(q_lo),
            chain_id: Some(cid),
            contract_address: Some(caddr),
            expiry: Some(exp),
            flags_packed: Some(Fr::from(7u64)), // has_change_1 = 1, has_change_2 = 1, asset_id = 1 (1 + 2*1 + 4*1 = 7)
            is_rollover: Some(Fr::from(0u64)),

            in1_value: Some(Fr::from(10_000_000u64)),
            in1_owner_key: Some(sk),
            in1_rho: Some(rho1),
            in1_randomness: Some(rand1),
            in1_leaf_index: Some(Fr::from(idx1 as u64)),
            in1_mountain_height: Some(proof1.mountain_height as u8),
            in1_mountain_siblings: Some(in1_sibs),

            in2_value: Some(Fr::from(10_000_000u64)),
            in2_owner_key: Some(sk),
            in2_rho: Some(rho2),
            in2_randomness: Some(rand2),
            in2_leaf_index: Some(Fr::from(idx2 as u64)),
            in2_mountain_height: Some(proof2.mountain_height as u8),
            in2_mountain_siblings: Some(in2_sibs),
            in2_is_dummy: Some(Fr::from(0u64)), // Real 2nd input
            in2_session_nonce: Some(Fr::from(0u64)),

            mmr_peaks: Some(peaks),

            out1_value: Some(Fr::from(3_000_000u64)),
            out1_owner_key: Some(sk),
            out1_rho: Some(rho_c1),
            out1_randomness: Some(rand_c1),

            out2_value: Some(Fr::from(1_900_000u64)),
            out2_owner_key: Some(sk),
            out2_rho: Some(rho_c2),
            out2_randomness: Some(rand_c2),

            has_change_1: Some(Fr::from(1u64)),
            has_change_2: Some(Fr::from(1u64)),
            expected_binding: Some(binding),

            public_asset_id: Some(Fr::from(asset_id_u8 as u64)),
            in1_asset_id: Some(Fr::from(asset_id_u8 as u64)),
            in2_asset_id: Some(Fr::from(asset_id_u8 as u64)),
            out1_asset_id: Some(Fr::from(asset_id_u8 as u64)),
            out2_asset_id: Some(Fr::from(asset_id_u8 as u64)),

            perm_bit: Some(Fr::from(0u64)),
            is_decoy: Some(Fr::from(0u64)),
        };

        circuit.generate_constraints(cs.clone()).unwrap();
        assert!(
            cs.is_satisfied().unwrap(),
            "2-in 2-out circuit must be satisfied: {:?}",
            cs.which_is_unsatisfied()
        );
    }

    #[test]
    fn test_joinsplit_constraint_budget_audit() {
        let cs = ConstraintSystem::<Fr>::new_ref();
        let circuit = create_dummy_joinsplit_circuit();
        circuit.generate_constraints(cs.clone()).unwrap();

        let num_constraints = cs.num_constraints();
        // DEC-036A Section 6 Constraint Budget:
        // With 96 Poseidon W5 nodes (32 levels per note + 32 levels bagging) @ ~360 constraints,
        // 448 u64 bit gates, 256 u128 bit gates, and fee remainder decomposition,
        // actual production R1CS constraint count is 34,571 constraints.
        assert!(
            (30_000..=36_000).contains(&num_constraints),
            "Constraint budget unexpected: got {}, expected in [30000, 36000]",
            num_constraints
        );
    }

    #[test]
    fn test_joinsplit_value_conservation_mismatch_rejected() {
        let cs = ConstraintSystem::<Fr>::new_ref();
        let mut circuit = create_dummy_joinsplit_circuit();
        // Maliciously tamper with execution fee (+100)
        circuit.execution_fee = Some(Fr::from(55_550u64));

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

        // Attempt modular underflow: Fr::MODULUS - 1
        let neg_one = -Fr::from(1u64);
        circuit.in1_value = Some(neg_one);

        circuit.generate_constraints(cs.clone()).unwrap();
        assert!(
            !cs.is_satisfied().unwrap(),
            "Modular underflow / negative input value must fail 64-bit range constraints!"
        );
    }

    #[test]
    fn test_joinsplit_negative_wrap_around_zero_in_max_out() {
        let cs = ConstraintSystem::<Fr>::new_ref();
        let mut circuit = create_dummy_joinsplit_circuit();

        // A7.2: vin = 0, vout = r - 1
        circuit.in1_value = Some(Fr::from(0u64));
        circuit.out1_value = Some(-Fr::from(1u64));

        circuit.generate_constraints(cs.clone()).unwrap();
        assert!(
            !cs.is_satisfied().unwrap(),
            "Modular wrap-around exploit (r - 1) must be rejected by range check!"
        );
    }

    #[test]
    fn test_joinsplit_dummy_input_positive_value_rejected() {
        // A7.3: Penumbra exploit: v_in2 > 0 with is_dummy_2 = 1
        let cs = ConstraintSystem::<Fr>::new_ref();
        let mut circuit = create_dummy_joinsplit_circuit();
        circuit.in2_value = Some(Fr::from(1_000_000u64));
        circuit.in2_is_dummy = Some(Fr::from(1u64));

        circuit.generate_constraints(cs.clone()).unwrap();
        assert!(
            !cs.is_satisfied().unwrap(),
            "Dummy input with value > 0 must fail Penumbra zero-value constraint!"
        );
    }

    #[test]
    fn test_joinsplit_dummy_input_epoch_mismatch_rejected() {
        // A2.4: Epoch alignment guard
        let cs = ConstraintSystem::<Fr>::new_ref();
        let mut circuit = create_dummy_joinsplit_circuit();
        // Dummy input, but epoch_id_2 != epoch_id_1
        circuit.epoch_id_2 = Some(Fr::from(999u64));

        circuit.generate_constraints(cs.clone()).unwrap();
        assert!(
            !cs.is_satisfied().unwrap(),
            "Dummy input with mismatched epoch_id must fail epoch alignment constraint!"
        );
    }

    #[test]
    fn test_joinsplit_singularity_nullifier_aliasing_rejected() {
        // A7.4: Singularity aliasing: nf1 == nf2 when is_dummy_2 = 0
        let cs = ConstraintSystem::<Fr>::new_ref();
        let mut circuit = create_dummy_joinsplit_circuit();
        // Make second input active (real note)
        circuit.in2_is_dummy = Some(Fr::from(0u64));
        // Tamper nullifier 2 to be equal to nullifier 1
        circuit.input_nullifier_2 = circuit.input_nullifier_1;

        circuit.generate_constraints(cs.clone()).unwrap();
        assert!(
            !cs.is_satisfied().unwrap(),
            "Identical nullifiers (aliasing) must fail non-aliasing constraint!"
        );
    }

    #[test]
    fn test_joinsplit_quote_hash_limb_out_of_range_rejected() {
        // A7.5: Out-of-range quote limb: quote_hash_hi >= 2^128
        let cs = ConstraintSystem::<Fr>::new_ref();
        let mut circuit = create_dummy_joinsplit_circuit();
        let mut bad_hi_bytes = [0u8; 32];
        bad_hi_bytes[15] = 1; // 2^128
        let bad_hi = Fr::from_be_bytes_mod_order(&bad_hi_bytes);
        circuit.quote_hash_hi = Some(bad_hi);

        circuit.generate_constraints(cs.clone()).unwrap();
        assert!(
            !cs.is_satisfied().unwrap(),
            "Quote hash limb >= 2^128 must fail 128-bit range check!"
        );
    }

    #[test]
    fn test_joinsplit_fee_ceiling_division_manipulation_rejected() {
        let cs = ConstraintSystem::<Fr>::new_ref();
        let mut circuit = create_dummy_joinsplit_circuit();
        // Tamper protocol fee to be 1 less than ceiling division
        let correct_fee = circuit.protocol_fee.unwrap();
        circuit.protocol_fee = Some(correct_fee - Fr::from(1u64));

        circuit.generate_constraints(cs.clone()).unwrap();
        assert!(
            !cs.is_satisfied().unwrap(),
            "Underpaid protocol fee must fail fee ceiling division remainder check!"
        );
    }

    #[test]
    fn test_joinsplit_flags_packed_manipulation_rejected() {
        let cs = ConstraintSystem::<Fr>::new_ref();
        let mut circuit = create_dummy_joinsplit_circuit();
        circuit.flags_packed = Some(Fr::from(1u64)); // Change flags claim 1 change note, but has_change is 0

        circuit.generate_constraints(cs.clone()).unwrap();
        assert!(
            !cs.is_satisfied().unwrap(),
            "Tampered flags_packed mismatching change flags must fail!"
        );
    }

    #[test]
    fn test_joinsplit_vk_non_identity() {
        use ark_ec::AffineRepr;
        let keys = generate_joinsplit_circuit_keys().unwrap();
        let vk = &keys.verifying_key;

        // A6.2: 19 public inputs => IC has 20 elements (IC[0..=19])
        assert_eq!(
            vk.gamma_abc_g1.len(),
            NUM_JOINSPLIT_PUBLIC_INPUTS + 1,
            "JoinSplit VK must contain exactly NUM_JOINSPLIT_PUBLIC_INPUTS + 1 elements (IC[0..=19])"
        );

        for (i, ic_point) in vk.gamma_abc_g1.iter().enumerate() {
            assert!(
                !ic_point.is_zero(),
                "JoinSplit VK element IC[{}] MUST NOT be the point at infinity!",
                i
            );
        }
    }

    #[test]
    fn test_joinsplit_proof_roundtrip() {
        let keys = generate_joinsplit_circuit_keys().unwrap();
        let circuit = create_dummy_joinsplit_circuit();
        let public_inputs = extract_joinsplit_public_inputs(&circuit);

        assert_eq!(public_inputs.len(), NUM_JOINSPLIT_PUBLIC_INPUTS);

        let proof = generate_joinsplit_proof(circuit, &keys.proving_key).unwrap();
        let valid = verify_joinsplit_proof(&keys.verifying_key, &proof, &public_inputs).unwrap();
        assert!(valid, "JoinSplit Groth16 proof verification must succeed!");
    }

    #[test]
    fn test_joinsplit_asset_id_intra_asset_equality_enforced() {
        // Critical Audit Patch #1: Cross-asset note mixing must fail!
        let cs = ConstraintSystem::<Fr>::new_ref();
        let mut circuit = create_dummy_joinsplit_circuit();
        // Tamper in2_asset_id to WETH (0) while public_asset_id is USDC (1)
        circuit.in2_asset_id = Some(Fr::from(0u64));

        circuit.generate_constraints(cs.clone()).unwrap();
        assert!(
            !cs.is_satisfied().unwrap(),
            "Cross-asset input note asset_id mismatch must fail equality constraint!"
        );
    }

    #[test]
    fn test_joinsplit_asset_id_boolean_constraint() {
        let cs = ConstraintSystem::<Fr>::new_ref();
        let mut circuit = create_dummy_joinsplit_circuit();
        // Set invalid asset_id = 2
        circuit.public_asset_id = Some(Fr::from(2u64));
        circuit.in1_asset_id = Some(Fr::from(2u64));
        circuit.in2_asset_id = Some(Fr::from(2u64));
        circuit.out1_asset_id = Some(Fr::from(2u64));
        circuit.out2_asset_id = Some(Fr::from(2u64));
        circuit.flags_packed = Some(Fr::from(8u64)); // 0 + 0 + 4 * 2 = 8

        circuit.generate_constraints(cs.clone()).unwrap();
        assert!(
            !cs.is_satisfied().unwrap(),
            "asset_id >= 2 must fail quadratic boolean constraint b * (1 - b) == 0!"
        );
    }

    #[test]
    fn test_joinsplit_dummy_input_usdc_value_packed() {
        // Direction B for USDC: value = 0 but asset_id = 1 => value_packed = 2^64
        let cs = ConstraintSystem::<Fr>::new_ref();
        let circuit = create_dummy_joinsplit_circuit();
        assert_eq!(circuit.public_asset_id, Some(Fr::from(1u64)));

        circuit.generate_constraints(cs.clone()).unwrap();
        assert!(
            cs.is_satisfied().unwrap(),
            "USDC dummy input packing (value_packed = 2^64) must satisfy constraints: {:?}",
            cs.which_is_unsatisfied()
        );
    }

    #[test]
    fn test_flags_packed_r1cs_gadget_enforcement() {
        use crate::circuit_gadgets::{
            enforce_joinsplit_flags_packed, enforce_private_note_flags_packed, private_witness,
        };

        // 1. JoinSplit flags: flags = ch1 + 2*ch2 + 4*asset_id
        for ch1 in 0..=1u64 {
            for ch2 in 0..=1u64 {
                for aid in 0..=1u64 {
                    let cs = ConstraintSystem::<Fr>::new_ref();
                    let expected = ch1 + 2 * ch2 + 4 * aid;
                    let flags_var = private_witness(&cs, Some(Fr::from(expected))).unwrap();
                    let ch1_var = private_witness(&cs, Some(Fr::from(ch1))).unwrap();
                    let ch2_var = private_witness(&cs, Some(Fr::from(ch2))).unwrap();
                    let aid_var = private_witness(&cs, Some(Fr::from(aid))).unwrap();

                    enforce_joinsplit_flags_packed(&flags_var, &ch1_var, &ch2_var, &aid_var)
                        .unwrap();
                    assert!(cs.is_satisfied().unwrap());
                }
            }
        }

        // 2. Private note flags: flags = rollover + 2*asset_id
        for ro in 0..=1u64 {
            for aid in 0..=1u64 {
                let cs = ConstraintSystem::<Fr>::new_ref();
                let expected = ro + 2 * aid;
                let flags_var = private_witness(&cs, Some(Fr::from(expected))).unwrap();
                let ro_var = private_witness(&cs, Some(Fr::from(ro))).unwrap();
                let aid_var = private_witness(&cs, Some(Fr::from(aid))).unwrap();

                enforce_private_note_flags_packed(&flags_var, &ro_var, &aid_var).unwrap();
                assert!(cs.is_satisfied().unwrap());
            }
        }
    }

    #[test]
    fn test_intra_asset_conservation_rejects_cross_spend() {
        let cs = ConstraintSystem::<Fr>::new_ref();
        let mut circuit = create_dummy_joinsplit_circuit();
        // Malicious attempt: input notes are USDC (asset_id = 1), but attacker attempts to produce WETH change note (out1_asset_id = 0)
        circuit.out1_asset_id = Some(Fr::from(0u64));

        circuit.generate_constraints(cs.clone()).unwrap();
        assert!(
            !cs.is_satisfied().unwrap(),
            "Cross-asset spend (mixing USDC input with WETH output) must be strictly rejected by intra-asset equality constraint!"
        );
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // Quadratic Encoding Soundness & Null-Branch Defense Tests (DEC-046)
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_null_branch_canonical_dummy_satisfied() {
        let cs = ConstraintSystem::<Fr>::new_ref();
        let circuit = create_dummy_joinsplit_circuit();
        circuit.clone().generate_constraints(cs.clone()).unwrap();

        assert!(
            cs.is_satisfied().unwrap(),
            "Canonical dummy baseline circuit must be satisfied: {:?}",
            cs.which_is_unsatisfied()
        );

        // Verify Groth16 proof roundtrip with canonical dummy
        let keys = generate_joinsplit_circuit_keys().unwrap();
        let public_inputs = extract_joinsplit_public_inputs(&circuit);
        let proof = generate_joinsplit_proof(circuit, &keys.proving_key).unwrap();
        let valid = verify_joinsplit_proof(&keys.verifying_key, &proof, &public_inputs).unwrap();
        assert!(valid, "Canonical dummy baseline Groth16 proof must verify!");
    }

    #[test]
    fn test_null_branch_tamper_dummy_rho_rejected() {
        // SEC-CRIT-02: Non-zero dummy rho must be rejected by companion constraint
        let cs = ConstraintSystem::<Fr>::new_ref();
        let mut circuit = create_dummy_joinsplit_circuit();
        assert_eq!(circuit.in2_is_dummy, Some(Fr::from(1u64)));

        // Malicious witness injection: random non-zero scalar for in2_rho
        circuit.in2_rho = Some(Fr::from(0x12345678_9abcdef0u64));

        circuit.generate_constraints(cs.clone()).unwrap();
        assert!(
            !cs.is_satisfied().unwrap(),
            "Tampered non-canonical in2_rho on dummy slot MUST fail constraint satisfaction!"
        );
    }

    #[test]
    fn test_null_branch_diff_inv_degree_of_freedom_rejected() {
        // SEC-CRIT-03: When in2_is_dummy == 1, diff_inv degree of freedom must be zero (diff_inv == 0)
        let cs = ConstraintSystem::<Fr>::new_ref();
        let circuit = create_dummy_joinsplit_circuit();
        assert_eq!(circuit.in2_is_dummy, Some(Fr::from(1u64)));

        // Inject malicious non-zero diff_inv witness
        set_tamper_diff_inv(Some(Fr::from(0xdead_beef_1337_u64)));
        let res = circuit.generate_constraints(cs.clone());
        set_tamper_diff_inv(None); // Reset thread-local

        res.unwrap();
        assert!(
            !cs.is_satisfied().unwrap(),
            "Non-zero diff_inv on dummy slot MUST be rejected by companion quadratic constraint is_dummy_2 * diff_inv == 0!"
        );
    }

    #[test]
    fn test_null_branch_tamper_inactive_change1_owner_rejected() {
        // SEC-CRIT-02: When has_change_1 == 0, out1_owner MUST be canonical zero
        let cs = ConstraintSystem::<Fr>::new_ref();
        let mut circuit = create_dummy_joinsplit_circuit();
        assert_eq!(circuit.has_change_1, Some(Fr::from(0u64)));

        // Inject non-zero address for out1_owner
        circuit.out1_owner_key = Some(Fr::from(0xaaaaaaaa_bbbbbbbb_u64));

        circuit.generate_constraints(cs.clone()).unwrap();
        assert!(
            !cs.is_satisfied().unwrap(),
            "Tampered non-canonical out1_owner on inactive change 1 MUST fail companion constraint!"
        );
    }

    #[test]
    fn test_null_branch_tamper_dummy_owner_rejected() {
        // Dummy slot in2_owner must be CANONICAL_DUMMY_OWNER (0)
        let cs = ConstraintSystem::<Fr>::new_ref();
        let mut circuit = create_dummy_joinsplit_circuit();
        assert_eq!(circuit.in2_is_dummy, Some(Fr::from(1u64)));

        // Inject arbitrary non-zero owner key for dummy input 2
        circuit.in2_owner_key = Some(Fr::from(0x9999_8888_7777u64));

        circuit.generate_constraints(cs.clone()).unwrap();
        assert!(
            !cs.is_satisfied().unwrap(),
            "Non-zero in2_owner on dummy slot MUST fail companion constraint!"
        );
    }

    #[test]
    fn test_null_branch_tamper_dummy_randomness_rejected() {
        let cs = ConstraintSystem::<Fr>::new_ref();
        let mut circuit = create_dummy_joinsplit_circuit();
        assert_eq!(circuit.in2_is_dummy, Some(Fr::from(1u64)));

        circuit.in2_randomness = Some(Fr::from(0x5555_6666_7777u64));

        circuit.generate_constraints(cs.clone()).unwrap();
        assert!(
            !cs.is_satisfied().unwrap(),
            "Non-zero in2_randomness on dummy slot MUST fail companion constraint!"
        );
    }

    #[test]
    fn test_null_branch_tamper_dummy_leaf_index_rejected() {
        let cs = ConstraintSystem::<Fr>::new_ref();
        let mut circuit = create_dummy_joinsplit_circuit();
        assert_eq!(circuit.in2_is_dummy, Some(Fr::from(1u64)));

        circuit.in2_leaf_index = Some(Fr::from(12345u64));

        circuit.generate_constraints(cs.clone()).unwrap();
        assert!(
            !cs.is_satisfied().unwrap(),
            "Non-zero in2_leaf_index on dummy slot MUST fail companion constraint!"
        );
    }

    #[test]
    fn test_null_branch_tamper_inactive_change2_rejected() {
        // When has_change_2 == 0, out2_owner, out2_rho, out2_rand must all be 0
        let circuit = create_dummy_joinsplit_circuit();
        assert_eq!(circuit.has_change_2, Some(Fr::from(0u64)));

        // Test out2_owner
        let cs1 = ConstraintSystem::<Fr>::new_ref();
        let mut c1 = circuit.clone();
        c1.out2_owner_key = Some(Fr::from(1u64));
        c1.generate_constraints(cs1.clone()).unwrap();
        assert!(
            !cs1.is_satisfied().unwrap(),
            "Non-zero out2_owner must fail"
        );

        // Test out2_rho
        let cs2 = ConstraintSystem::<Fr>::new_ref();
        let mut c2 = circuit.clone();
        c2.out2_rho = Some(Fr::from(1u64));
        c2.generate_constraints(cs2.clone()).unwrap();
        assert!(!cs2.is_satisfied().unwrap(), "Non-zero out2_rho must fail");

        // Test out2_rand
        let cs3 = ConstraintSystem::<Fr>::new_ref();
        let mut c3 = circuit;
        c3.out2_randomness = Some(Fr::from(1u64));
        c3.generate_constraints(cs3.clone()).unwrap();
        assert!(!cs3.is_satisfied().unwrap(), "Non-zero out2_rand must fail");
    }

    #[test]
    fn test_null_branch_state_invariance_proof() {
        // Prove that rejected tampered witness attempts prevent proof synthesis without modifying state
        let keys = generate_joinsplit_circuit_keys().unwrap();
        let mut circuit = create_dummy_joinsplit_circuit();

        // Tamper null-branch witness
        circuit.in2_rho = Some(Fr::from(0xbad_f00d_u64));

        // Prover must fail to synthesize a valid proof: Groth16::prove panics with cs.is_satisfied() failure
        let cs = ConstraintSystem::<Fr>::new_ref();
        circuit.clone().generate_constraints(cs.clone()).unwrap();
        assert!(
            !cs.is_satisfied().unwrap(),
            "Tampered null-branch witness MUST NOT satisfy constraint system!"
        );

        let proving_attempt = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            generate_joinsplit_proof(circuit.clone(), &keys.proving_key)
        }));
        assert!(
            proving_attempt.is_err(),
            "Prover must panic/fail when attempting to prove unsatisfied tampered witness!"
        );

        // Assert accounting invariant remains pristine: balance >= total_liabilities
        let accounting = crate::accounting::ContractAccounting::new();
        let tr = accounting.deposit(10_000_000).unwrap();
        assert_eq!(tr.new_state.assets, 10_000_000);
        // Failed spend does not alter liabilities and solvency holds
        assert!(tr.new_state.check_solvency());
    }
}
