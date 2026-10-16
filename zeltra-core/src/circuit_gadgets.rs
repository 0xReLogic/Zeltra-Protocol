//! R1CS Circuit Gadgets for Dual-Asset Bit-Packing and Flag Packing (DEC-034A).
//!
//! Provides mathematically constrained gadgets:
//! - `enforce_packed_value_and_asset`: binds `value_packed == value + asset_id * 2^64` with boolean asset_id and 64-bit value range
//! - `enforce_private_note_flags_packed`: binds Slot 15 `flags_packed == is_rollover + 2 * asset_id`
//! - `enforce_joinsplit_flags_packed`: binds Slot 17 `flags_packed == has_change_1 + 2 * has_change_2 + 4 * asset_id`

use ark_bls12_381::Fr;
use ark_ff::PrimeField;
use ark_r1cs_std::eq::EqGadget;
use ark_r1cs_std::fields::fp::{AllocatedFp, FpVar};
use ark_r1cs_std::fields::FieldVar;
use ark_relations::gr1cs::{ConstraintSystemRef, SynthesisError};

/// Helper: allocate a private witness variable and wrap as FpVar.
pub fn private_witness(
    cs: &ConstraintSystemRef<Fr>,
    value: Option<Fr>,
) -> Result<FpVar<Fr>, SynthesisError> {
    let var = cs.new_witness_variable(|| value.ok_or(SynthesisError::AssignmentMissing))?;
    Ok(FpVar::Var(AllocatedFp::new(value, var, cs.clone())))
}

/// Helper: allocate a public input variable and wrap as FpVar.
pub fn public_input(
    cs: &ConstraintSystemRef<Fr>,
    value: Option<Fr>,
) -> Result<FpVar<Fr>, SynthesisError> {
    let var = cs.new_input_variable(|| value.ok_or(SynthesisError::AssignmentMissing))?;
    Ok(FpVar::Var(AllocatedFp::new(value, var, cs.clone())))
}

/// Enforce that `val` is in [0, 2^64) by decomposing into 64 boolean bits.
pub fn enforce_u64_range(
    cs: &ConstraintSystemRef<Fr>,
    val: &FpVar<Fr>,
    val_witness: Option<Fr>,
) -> Result<(), SynthesisError> {
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

/// Enforce that `val` is in [0, 2^128) by decomposing into 128 boolean bits (DEC-035A).
pub fn enforce_u128_range(
    cs: &ConstraintSystemRef<Fr>,
    val: &FpVar<Fr>,
    val_witness: Option<Fr>,
) -> Result<(), SynthesisError> {
    let zero = FpVar::Constant(Fr::from(0u64));
    let one = FpVar::Constant(Fr::from(1u64));
    let mut reconstructed = zero.clone();
    let mut two_power = Fr::from(1u64);

    let bigint_opt = val_witness.map(|w| w.into_bigint());

    for bit_idx in 0..128 {
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

/// Enforces the dual-asset bit-packing relationship in R1CS (DEC-034A §3):
/// 1. Strict boolean check on asset_id: asset_id * (asset_id - 1) == 0
/// 2. Strict 64-bit range decomposition on value: value in [0, 2^64)
/// 3. Linear combination constraint: value_packed == value + asset_id * 2^64
pub fn enforce_packed_value_and_asset(
    cs: &ConstraintSystemRef<Fr>,
    value_packed_var: &FpVar<Fr>,
    value_var: &FpVar<Fr>,
    asset_id_var: &FpVar<Fr>,
    value_witness: Option<u64>,
    _asset_id_witness: Option<u8>,
) -> Result<(), SynthesisError> {
    // 1. Strict boolean check: asset_id * (asset_id - 1) == 0
    asset_id_var.enforce_equal(&asset_id_var.square()?)?;

    // 2. Strict 64-bit integer range decomposition on value
    enforce_u64_range(cs, value_var, value_witness.map(Fr::from))?;

    // 3. Linear combination constraint (Zero non-linear multiplication overhead!):
    // value_packed == value + asset_id * 2^64
    let two_power_64 = FpVar::Constant(Fr::from(1u128 << 64));
    let reconstructed = value_var + (asset_id_var * &two_power_64);
    reconstructed.enforce_equal(value_packed_var)?;

    Ok(())
}

/// Enforces Private Note flags_packed (Slot 15, 16-PI) (DEC-034A §4.D):
/// flags_packed == is_rollover + 2 * asset_id
pub fn enforce_private_note_flags_packed(
    flags_packed_pi: &FpVar<Fr>,
    is_rollover_var: &FpVar<Fr>,
    asset_id_var: &FpVar<Fr>,
) -> Result<(), SynthesisError> {
    // Both variables must be strictly boolean (0 or 1)
    is_rollover_var.enforce_equal(&is_rollover_var.square()?)?;
    asset_id_var.enforce_equal(&asset_id_var.square()?)?;

    // Linear combination: is_rollover + 2 * asset_id
    let two = FpVar::Constant(Fr::from(2u64));
    let expected_flags = is_rollover_var + (asset_id_var * &two);
    flags_packed_pi.enforce_equal(&expected_flags)?;

    Ok(())
}

/// Enforces Universal JoinSplit flags_packed (Slot 17, 19-PI) (DEC-034A §4.D):
/// flags_packed == has_change_1 + 2 * has_change_2 + 4 * asset_id
pub fn enforce_joinsplit_flags_packed(
    flags_packed_pi: &FpVar<Fr>,
    has_change_1_var: &FpVar<Fr>,
    has_change_2_var: &FpVar<Fr>,
    asset_id_var: &FpVar<Fr>,
) -> Result<(), SynthesisError> {
    // All three variables must be strictly boolean (0 or 1)
    has_change_1_var.enforce_equal(&has_change_1_var.square()?)?;
    has_change_2_var.enforce_equal(&has_change_2_var.square()?)?;
    asset_id_var.enforce_equal(&asset_id_var.square()?)?;

    let two = FpVar::Constant(Fr::from(2u64));
    let four = FpVar::Constant(Fr::from(4u64));
    let expected_flags = has_change_1_var + (has_change_2_var * &two) + (asset_id_var * &four);
    flags_packed_pi.enforce_equal(&expected_flags)?;

    Ok(())
}

/// Enforces range constraints on challenge limbs (DEC-035A, DEC-035B, DEC-040A).
/// Constrains challenge_hi and challenge_lo to 128-bit integers via boolean bit decomposition,
/// ensuring both wires are actively constrained in R1CS and never pruned by the optimizer.
pub fn enforce_dummy_range_binding(
    cs: ConstraintSystemRef<Fr>,
    challenge_hi: &FpVar<Fr>,
    challenge_lo: &FpVar<Fr>,
) -> Result<(), SynthesisError> {
    use ark_r1cs_std::GR1CSVar;
    let hi_witness = challenge_hi.value().ok();
    let lo_witness = challenge_lo.value().ok();
    enforce_u128_range(&cs, challenge_hi, hi_witness)?;
    enforce_u128_range(&cs, challenge_lo, lo_witness)?;
    Ok(())
}

/// Width-3 Poseidon sponge gadget for key derivation (DEC-040A).
/// Applies Poseidon permutation to state [inputs[0], domain, inputs[1]].
pub fn poseidon_w3_gadget(
    _cs: ConstraintSystemRef<Fr>,
    inputs: &[FpVar<Fr>],
    domain: FpVar<Fr>,
) -> Result<FpVar<Fr>, SynthesisError> {
    assert_eq!(
        inputs.len(),
        2,
        "poseidon_w3_gadget expects 2 inputs [sk, 0]"
    );
    let (w3_rc, w3_mds) = &*crate::poseidon::CACHED_W3_PARAMS;
    let mut state = [inputs[0].clone(), domain, inputs[1].clone()];
    crate::poseidon::poseidon_permutation(&mut state, w3_rc, w3_mds)?;
    Ok(state[0].clone())
}

/// Width-5 Poseidon hash gadget for nullifier and note derivation (DEC-040A).
/// State layout: [inputs[0], inputs[1], inputs[2], inputs[3], domain].
pub fn poseidon_w5_gadget(
    _cs: ConstraintSystemRef<Fr>,
    inputs: &[FpVar<Fr>],
    domain: FpVar<Fr>,
) -> Result<FpVar<Fr>, SynthesisError> {
    use ark_r1cs_std::GR1CSVar;
    assert_eq!(
        inputs.len(),
        4,
        "poseidon_w5_gadget expects 4 inputs [nk, rho, epoch, 0]"
    );
    let (w5_rc, w5_mds) = &*crate::poseidon::CACHED_W5_PARAMS;
    let domain_val = domain
        .value()
        .unwrap_or_else(|_| crate::note::domain_nullifier());
    let arr = [
        inputs[0].clone(),
        inputs[1].clone(),
        inputs[2].clone(),
        inputs[3].clone(),
    ];
    crate::poseidon::poseidon_w5_hash(&arr, domain_val, w5_rc, w5_mds)
}
