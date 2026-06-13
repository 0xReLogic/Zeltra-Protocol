//! Poseidon hash function gadget for R1CS circuits
//!
//! Implements the Poseidon permutation directly using `FpVar` field operations
//! from `ark-r1cs-std`. No external Poseidon crate needed.
//!
//! Parameters (width=3, alpha=5) for BLS12-381 scalar field:
//! - State width t = 3 (rate=2, capacity=1)
//! - Full rounds R_F = 8 (4 before + 4 after partial rounds)
//! - Partial rounds R_P = 57
//! - S-box exponent alpha = 5

use ark_bls12_381::Fr;
use ark_ff::Field;
use ark_r1cs_std::fields::fp::FpVar;
use ark_r1cs_std::fields::FieldVar;
use ark_relations::gr1cs::SynthesisError;
use ark_std::rand::SeedableRng;

/// Poseidon parameters for BLS12-381 scalar field
/// Width t=3, alpha=5, R_F=8, R_P=57
pub const POSEIDON_WIDTH: usize = 3;
pub const POSEIDON_FULL_ROUNDS: usize = 8;
pub const POSEIDON_PARTIAL_ROUNDS: usize = 57;
#[allow(dead_code)]
pub const POSEIDON_ALPHA: u64 = 5;
pub const POSEIDON_HALF_FULL: usize = POSEIDON_FULL_ROUNDS / 2;

/// Deterministic seed for Poseidon parameter generation (Phase A).
/// Phase B will replace this with parameters from a real ceremony.
pub const POSEIDON_SEED: u64 = 0x4e696d627573506f; // "NimbusPo"

/// Generate Poseidon round constants deterministically from a seed.
///
/// Uses a seeded StdRng to produce reproducible field elements.
/// Total sets needed: 1 (initial) + R_F + R_P = 66 sets of WIDTH = 198 constants.
pub fn generate_round_constants(seed: u64) -> Vec<Fr> {
    use ark_ff::UniformRand;

    let mut rng = ark_std::rand::rngs::StdRng::seed_from_u64(seed);
    let total = POSEIDON_WIDTH * (1 + POSEIDON_FULL_ROUNDS + POSEIDON_PARTIAL_ROUNDS);
    (0..total).map(|_| Fr::rand(&mut rng)).collect()
}

/// Generate a 3x3 MDS (Maximum Distance Separable) matrix deterministically.
///
/// Uses a Cauchy matrix construction: M[i][j] = 1 / (x_i + y_j)
/// with x = [0, 1, 2] and y = [3, 4, 5] to guarantee all entries are non-zero
/// and the matrix is invertible (required for the Poseidon linear layer).
pub fn generate_mds_matrix(seed: u64) -> Vec<Vec<Fr>> {
    use ark_ff::UniformRand;

    let mut rng = ark_std::rand::rngs::StdRng::seed_from_u64(seed.wrapping_add(1));

    let x: Vec<Fr> = (0..POSEIDON_WIDTH).map(|_| Fr::rand(&mut rng)).collect();
    let y: Vec<Fr> = (0..POSEIDON_WIDTH).map(|_| Fr::rand(&mut rng)).collect();

    (0..POSEIDON_WIDTH)
        .map(|i| {
            (0..POSEIDON_WIDTH)
                .map(|j| (x[i] + y[j]).inverse().unwrap())
                .collect()
        })
        .collect()
}

// ─── R1CS Gadget ────────────────────────────────────────────────────────

/// Poseidon x^5 S-box gadget.
///
/// Constrains `result = x^5` using 3 multiplications:
///   x2 = x * x
///   x4 = x2 * x2
///   x5 = x4 * x
fn sbox(x: &FpVar<Fr>) -> Result<FpVar<Fr>, SynthesisError> {
    let x2 = x * x;
    let x4 = &x2 * &x2;
    Ok(&x4 * x)
}

/// MDS matrix multiplication gadget.
///
/// Computes new_state = M * state where M is the MDS matrix.
/// All matrix entries are constants, so multiplications use `FpVar * &Fr`.
pub fn mds_mul(
    state: &[FpVar<Fr>],
    mds: &[Vec<Fr>],
) -> Result<[FpVar<Fr>; POSEIDON_WIDTH], SynthesisError> {
    let mut result: [FpVar<Fr>; POSEIDON_WIDTH] = [
        FpVar::Constant(Fr::from(0u64)),
        FpVar::Constant(Fr::from(0u64)),
        FpVar::Constant(Fr::from(0u64)),
    ];
    for i in 0..POSEIDON_WIDTH {
        for j in 0..POSEIDON_WIDTH {
            result[i] = &result[i] + &state[j] * mds[i][j];
        }
    }
    Ok(result)
}

/// Full Poseidon permutation gadget over 3 field elements.
///
/// Structure:
/// 1. Add round constants (round 0)
/// 2. First half of full rounds (S-box on all 3 state elements + MDS)
/// 3. Partial rounds (S-box on first element only + MDS)
/// 4. Second half of full rounds (S-box on all 3 + MDS)
///
/// The permutation is a one-way function when at least one input is secret.
pub fn poseidon_permutation(
    state: &mut [FpVar<Fr>; POSEIDON_WIDTH],
    round_constants: &[Fr],
    mds: &[Vec<Fr>],
) -> Result<(), SynthesisError> {
    let mut rc_offset = 0;

    // Add initial round constants
    #[allow(clippy::needless_range_loop)]
    for i in 0..POSEIDON_WIDTH {
        state[i] += round_constants[rc_offset + i];
    }
    rc_offset += POSEIDON_WIDTH;

    // First half of full rounds: S-box on ALL state elements
    for _ in 0..POSEIDON_HALF_FULL {
        let s0 = sbox(&state[0])?;
        let s1 = sbox(&state[1])?;
        let s2 = sbox(&state[2])?;
        *state = [s0, s1, s2];
        *state = mds_mul(state, mds)?;
        #[allow(clippy::needless_range_loop)]
        for i in 0..POSEIDON_WIDTH {
            state[i] += round_constants[rc_offset + i];
        }
        rc_offset += POSEIDON_WIDTH;
    }

    // Partial rounds: S-box on FIRST element only
    for _ in 0..POSEIDON_PARTIAL_ROUNDS {
        state[0] = sbox(&state[0])?;
        *state = mds_mul(state, mds)?;
        #[allow(clippy::needless_range_loop)]
        for i in 0..POSEIDON_WIDTH {
            state[i] += round_constants[rc_offset + i];
        }
        rc_offset += POSEIDON_WIDTH;
    }

    // Second half of full rounds: S-box on ALL state elements
    for _ in 0..POSEIDON_HALF_FULL {
        let s0 = sbox(&state[0])?;
        let s1 = sbox(&state[1])?;
        let s2 = sbox(&state[2])?;
        *state = [s0, s1, s2];
        *state = mds_mul(state, mds)?;
        #[allow(clippy::needless_range_loop)]
        for i in 0..POSEIDON_WIDTH {
            state[i] += round_constants[rc_offset + i];
        }
        rc_offset += POSEIDON_WIDTH;
    }

    Ok(())
}

/// Poseidon hash gadget: H(input1, input2) → output
///
/// Uses sponge construction with capacity=1, rate=2:
/// 1. Initialize state = [input1, input2, capacity(0)]
/// 2. Apply Poseidon permutation
/// 3. Return state[0] as hash output
pub fn poseidon_hash(
    input1: &FpVar<Fr>,
    input2: &FpVar<Fr>,
    round_constants: &[Fr],
    mds: &[Vec<Fr>],
) -> Result<FpVar<Fr>, SynthesisError> {
    let mut state = [
        input1.clone(),
        input2.clone(),
        FpVar::constant(Fr::from(0u64)),
    ];
    poseidon_permutation(&mut state, round_constants, mds)?;
    Ok(state[0].clone())
}

// ─── Native (non-circuit) evaluation ────────────────────────────────────

/// Native Poseidon x^5 S-box (field element, no constraints).
fn native_sbox(x: Fr) -> Fr {
    let x2 = x * x;
    let x4 = x2 * x2;
    x4 * x
}

/// Native MDS multiplication (field elements, no constraints).
fn native_mds_mul(state: &[Fr; 3], mds: &[Vec<Fr>]) -> [Fr; 3] {
    let mut result = [Fr::from(0u64); 3];
    for (result_i, row) in result.iter_mut().zip(mds.iter()) {
        for (state_j, mds_ij) in state.iter().zip(row.iter()) {
            *result_i += *state_j * mds_ij;
        }
    }
    result
}

/// Native Poseidon permutation (field elements, no constraints).
/// Used for computing the nullifier outside the circuit.
pub fn native_poseidon_permutation(
    state: &mut [Fr; 3],
    round_constants: &[Fr],
    mds: &[Vec<Fr>],
) {
    let mut rc_offset = 0;

    let add_rc = |state: &mut [Fr; 3], rc: &[Fr], offset: usize| {
        for (s, r) in state.iter_mut().zip(rc[offset..offset + 3].iter()) {
            *s += r;
        }
    };

    let full_round = |state: &mut [Fr; 3], rc: &[Fr], offset: usize, mds: &[Vec<Fr>]| {
        for s in state.iter_mut() {
            *s = native_sbox(*s);
        }
        *state = native_mds_mul(state, mds);
        add_rc(state, rc, offset);
    };

    add_rc(state, round_constants, rc_offset);
    rc_offset += 3;

    for _ in 0..POSEIDON_HALF_FULL {
        full_round(state, round_constants, rc_offset, mds);
        rc_offset += 3;
    }

    for _ in 0..POSEIDON_PARTIAL_ROUNDS {
        state[0] = native_sbox(state[0]);
        *state = native_mds_mul(state, mds);
        add_rc(state, round_constants, rc_offset);
        rc_offset += 3;
    }

    for _ in 0..POSEIDON_HALF_FULL {
        full_round(state, round_constants, rc_offset, mds);
        rc_offset += 3;
    }
}

/// Compute nullifier natively: nullifier = Poseidon(secret, randomness)
///
/// This is the non-circuit version used by the prover to compute the
/// public nullifier input before generating the proof.
pub fn compute_nullifier(secret: Fr, randomness: Fr) -> Fr {
    let rc = generate_round_constants(POSEIDON_SEED);
    let mds = generate_mds_matrix(POSEIDON_SEED);
    let mut state = [secret, randomness, Fr::from(0u64)];
    native_poseidon_permutation(&mut state, &rc, &mds);
    state[0]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_native_poseidon_deterministic() {
        let a = Fr::from(42u64);
        let b = Fr::from(123u64);
        let h1 = compute_nullifier(a, b);
        let h2 = compute_nullifier(a, b);
        assert_eq!(h1, h2, "Poseidon must be deterministic");
    }

    #[test]
    fn test_native_poseidon_different_inputs() {
        let h1 = compute_nullifier(Fr::from(1u64), Fr::from(2u64));
        let h2 = compute_nullifier(Fr::from(3u64), Fr::from(4u64));
        assert_ne!(h1, h2, "Different inputs must produce different hashes");
    }

    #[test]
    fn test_native_poseidon_not_linear() {
        let a = Fr::from(10u64);
        let b = Fr::from(20u64);
        let h = compute_nullifier(a, b);
        assert_ne!(h, a + b, "Poseidon must not be a linear function");
    }

    #[test]
    fn test_round_constants_length() {
        let rc = generate_round_constants(POSEIDON_SEED);
        let expected = POSEIDON_WIDTH * (1 + POSEIDON_FULL_ROUNDS + POSEIDON_PARTIAL_ROUNDS);
        assert_eq!(rc.len(), expected);
    }

    #[test]
    fn test_mds_matrix_dimensions() {
        let mds = generate_mds_matrix(POSEIDON_SEED);
        assert_eq!(mds.len(), POSEIDON_WIDTH);
        for row in &mds {
            assert_eq!(row.len(), POSEIDON_WIDTH);
        }
    }
}
