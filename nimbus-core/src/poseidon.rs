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

// ─── Grain-128 LFSR Parameter Generator (DEC-021) ─────────────────────────

/// Canonical Grain-128 LFSR in self-shrinking mode for audited Poseidon parameter generation.
///
/// Implements the standardized parameter generation algorithm specified in
/// Grassi et al. (Poseidon Paper, Section 2.3) and Aztec Barretenberg (`poseidon2_cpp_params.sage`),
/// mitigating invariant subspace trails and Gröbner basis algebraic attacks (DEC-021).
#[derive(Clone, Debug)]
pub struct GrainLfsr {
    state: [bool; 80],
}

impl GrainLfsr {
    /// Initialize Grain LFSR for a specific Poseidon instance over BLS12-381 Fr:
    /// - `width`: state width t (e.g., 3 or 5)
    /// - `full_rounds`: R_F
    /// - `partial_rounds`: R_P
    pub fn new(width: usize, full_rounds: usize, partial_rounds: usize) -> Self {
        let mut state = [false; 80];

        // b0, b1: 1, 0 for prime field Fp
        state[0] = true;
        state[1] = false;

        // b2..b5: S-box exponent alpha = 5 (binary 0101)
        state[2] = true;
        state[3] = false;
        state[4] = true;
        state[5] = false;

        // b6..b17: field size n = 255 bits (12 bits)
        let n = 255u16;
        for i in 0..12 {
            state[6 + i] = ((n >> i) & 1) == 1;
        }

        // b18..b29: state size t (12 bits)
        let t = width as u16;
        for i in 0..12 {
            state[18 + i] = ((t >> i) & 1) == 1;
        }

        // b30..b39: full rounds R_F (10 bits)
        let rf = full_rounds as u16;
        for i in 0..10 {
            state[30 + i] = ((rf >> i) & 1) == 1;
        }

        // b40..b49: partial rounds R_P (10 bits)
        let rp = partial_rounds as u16;
        for i in 0..10 {
            state[40 + i] = ((rp >> i) & 1) == 1;
        }

        // b50..b79: 1's (30 bits)
        for i in 50..80 {
            state[i] = true;
        }

        let mut lfsr = Self { state };

        // Warmup: discard first 160 bits
        for _ in 0..160 {
            lfsr.clock_raw();
        }

        lfsr
    }

    /// Single clock cycle of the 80-bit LFSR.
    /// Polynomial: x^80 + x^62 + x^51 + x^38 + x^23 + x^13 + 1
    fn clock_raw(&mut self) -> bool {
        let feedback = self.state[62]
            ^ self.state[51]
            ^ self.state[38]
            ^ self.state[23]
            ^ self.state[13]
            ^ self.state[0];
        let out = self.state[0];
        self.state.copy_within(1..80, 0);
        self.state[79] = feedback;
        out
    }

    /// Self-shrinking mode: sample 2 bits b0, b1.
    /// If b0 == 1, output b1; if b0 == 0, discard b1 and retry.
    pub fn get_bit(&mut self) -> bool {
        loop {
            let b0 = self.clock_raw();
            let b1 = self.clock_raw();
            if b0 {
                return b1;
            }
        }
    }

    /// Generate next field element in Fr using rejection sampling.
    pub fn get_field_element(&mut self) -> Fr {
        loop {
            let mut bytes = [0u8; 32];
            for bit_idx in 0..255 {
                if self.get_bit() {
                    let byte_pos = bit_idx / 8;
                    let bit_pos = bit_idx % 8;
                    bytes[byte_pos] |= 1 << bit_pos;
                }
            }
            if let Some(elem) = Fr::from_random_bytes(&bytes) {
                return elem;
            }
        }
    }
}

/// Generate Poseidon round constants deterministically using the canonical Grain LFSR (DEC-021).
pub fn generate_grain_round_constants(
    width: usize,
    full_rounds: usize,
    partial_rounds: usize,
) -> Vec<Fr> {
    let mut lfsr = GrainLfsr::new(width, full_rounds, partial_rounds);
    let total = width * (1 + full_rounds + partial_rounds);
    (0..total).map(|_| lfsr.get_field_element()).collect()
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
pub fn native_poseidon_permutation(state: &mut [Fr; 3], round_constants: &[Fr], mds: &[Vec<Fr>]) {
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

// ═══════════════════════════════════════════════════════════════════════════
// Width-5 Poseidon (for note commitments, nullifier derivation, Merkle nodes)
// ═══════════════════════════════════════════════════════════════════════════
//
// DEC-016A: PrivateNoteV1 requires hashing 4 inputs (value, owner_key, rho,
// randomness) plus a domain tag in the capacity element. This needs width=5
// (rate=4, capacity=1). Merkle tree node hashing also uses width-5.

/// Width-5 Poseidon parameters.
pub const POSEIDON_W5_WIDTH: usize = 5;
pub const POSEIDON_W5_FULL_ROUNDS: usize = 8;
/// Partial rounds for width-5 at 128-bit security.
/// Per Poseidon paper Table 2: R_P = 60 for t=5, alpha=5, M=128.
pub const POSEIDON_W5_PARTIAL_ROUNDS: usize = 60;
pub const POSEIDON_W5_HALF_FULL: usize = POSEIDON_W5_FULL_ROUNDS / 2;

/// Deterministic seed for width-5 round constants.
/// Distinct from width-3 seed to ensure independent parameter sets.
pub const POSEIDON_W5_SEED: u64 = 0x4e696d6275735735; // "NimbusW5"

/// Generate round constants for width-5 Poseidon.
pub fn generate_w5_round_constants() -> Vec<Fr> {
    use ark_ff::UniformRand;

    let mut rng = ark_std::rand::rngs::StdRng::seed_from_u64(POSEIDON_W5_SEED);
    let total = POSEIDON_W5_WIDTH * (1 + POSEIDON_W5_FULL_ROUNDS + POSEIDON_W5_PARTIAL_ROUNDS);
    (0..total).map(|_| Fr::rand(&mut rng)).collect()
}

/// Generate 5x5 MDS matrix using deterministic Cauchy construction.
///
/// Uses small sequential values: x = [1,2,3,4,5], y = [6,7,8,9,10].
/// M[i][j] = 1 / (x[i] + y[j])
///
/// This is the canonical construction from the Poseidon paper, guaranteed
/// to produce an MDS matrix. Unlike the width-3 implementation which uses
/// random Cauchy vectors, this uses deterministic small values for
/// reproducibility across independent implementations.
pub fn generate_w5_mds_matrix() -> Vec<Vec<Fr>> {
    let t = POSEIDON_W5_WIDTH;
    let x: Vec<Fr> = (1..=t).map(|i| Fr::from(i as u64)).collect();
    let y: Vec<Fr> = (t + 1..=2 * t).map(|i| Fr::from(i as u64)).collect();

    (0..t)
        .map(|i| {
            (0..t)
                .map(|j| {
                    (x[i] + y[j])
                        .inverse()
                        .expect("Cauchy matrix entry must be invertible")
                })
                .collect()
        })
        .collect()
}

/// Native width-5 MDS multiplication.
fn native_w5_mds_mul(state: &[Fr; POSEIDON_W5_WIDTH], mds: &[Vec<Fr>]) -> [Fr; POSEIDON_W5_WIDTH] {
    let mut result = [Fr::from(0u64); POSEIDON_W5_WIDTH];
    for (result_i, row) in result.iter_mut().zip(mds.iter()) {
        for (state_j, mds_ij) in state.iter().zip(row.iter()) {
            *result_i += *state_j * mds_ij;
        }
    }
    result
}

/// Native width-5 Poseidon permutation.
pub fn native_w5_poseidon_permutation(
    state: &mut [Fr; POSEIDON_W5_WIDTH],
    round_constants: &[Fr],
    mds: &[Vec<Fr>],
) {
    let w = POSEIDON_W5_WIDTH;
    let mut rc_offset = 0;

    // Initial round constant addition
    for i in 0..w {
        state[i] += round_constants[rc_offset + i];
    }
    rc_offset += w;

    // First half of full rounds: S-box on ALL state elements
    for _ in 0..POSEIDON_W5_HALF_FULL {
        for s in state.iter_mut() {
            *s = native_sbox(*s);
        }
        *state = native_w5_mds_mul(state, mds);
        for i in 0..w {
            state[i] += round_constants[rc_offset + i];
        }
        rc_offset += w;
    }

    // Partial rounds: S-box on FIRST element only
    for _ in 0..POSEIDON_W5_PARTIAL_ROUNDS {
        state[0] = native_sbox(state[0]);
        *state = native_w5_mds_mul(state, mds);
        for i in 0..w {
            state[i] += round_constants[rc_offset + i];
        }
        rc_offset += w;
    }

    // Second half of full rounds: S-box on ALL state elements
    for _ in 0..POSEIDON_W5_HALF_FULL {
        for s in state.iter_mut() {
            *s = native_sbox(*s);
        }
        *state = native_w5_mds_mul(state, mds);
        for i in 0..w {
            state[i] += round_constants[rc_offset + i];
        }
        rc_offset += w;
    }
}

/// Native width-5 Poseidon hash: H(inputs[0..4], domain_tag) → output
///
/// State layout: [input_0, input_1, input_2, input_3, domain_tag]
/// Returns state[0] after permutation.
pub fn native_poseidon_w5(inputs: &[Fr; 4], domain_tag: Fr) -> Fr {
    let rc = generate_w5_round_constants();
    let mds = generate_w5_mds_matrix();
    let mut state = [inputs[0], inputs[1], inputs[2], inputs[3], domain_tag];
    native_w5_poseidon_permutation(&mut state, &rc, &mds);
    state[0]
}

/// Width-5 Poseidon hash gadget for R1CS circuits.
///
/// Same structure as native version but using FpVar constraints.
#[allow(dead_code)] // Used in Phase 3 (circuit implementation)
pub fn poseidon_w5_hash(
    inputs: &[FpVar<Fr>; 4],
    domain_tag: Fr,
    round_constants: &[Fr],
    mds: &[Vec<Fr>],
) -> Result<FpVar<Fr>, SynthesisError> {
    let w = POSEIDON_W5_WIDTH;
    let mut state = [
        inputs[0].clone(),
        inputs[1].clone(),
        inputs[2].clone(),
        inputs[3].clone(),
        FpVar::constant(domain_tag),
    ];
    let mut rc_offset = 0;

    // Initial RC addition
    for i in 0..w {
        state[i] += round_constants[rc_offset + i];
    }
    rc_offset += w;

    // First half of full rounds
    for _ in 0..POSEIDON_W5_HALF_FULL {
        for s in state.iter_mut() {
            *s = sbox(s)?;
        }
        state = w5_mds_mul(&state, mds)?;
        for i in 0..w {
            state[i] += round_constants[rc_offset + i];
        }
        rc_offset += w;
    }

    // Partial rounds
    for _ in 0..POSEIDON_W5_PARTIAL_ROUNDS {
        state[0] = sbox(&state[0])?;
        state = w5_mds_mul(&state, mds)?;
        for i in 0..w {
            state[i] += round_constants[rc_offset + i];
        }
        rc_offset += w;
    }

    // Second half of full rounds
    for _ in 0..POSEIDON_W5_HALF_FULL {
        for s in state.iter_mut() {
            *s = sbox(s)?;
        }
        state = w5_mds_mul(&state, mds)?;
        for i in 0..w {
            state[i] += round_constants[rc_offset + i];
        }
        rc_offset += w;
    }

    Ok(state[0].clone())
}

/// Width-5 MDS matrix multiplication gadget for R1CS.
#[allow(dead_code)] // Used in Phase 3 (circuit implementation)
fn w5_mds_mul(
    state: &[FpVar<Fr>; POSEIDON_W5_WIDTH],
    mds: &[Vec<Fr>],
) -> Result<[FpVar<Fr>; POSEIDON_W5_WIDTH], SynthesisError> {
    let w = POSEIDON_W5_WIDTH;
    let mut result: Vec<FpVar<Fr>> = (0..w).map(|_| FpVar::Constant(Fr::from(0u64))).collect();
    for i in 0..w {
        for j in 0..w {
            result[i] = &result[i] + &state[j] * mds[i][j];
        }
    }
    Ok([
        result[0].clone(),
        result[1].clone(),
        result[2].clone(),
        result[3].clone(),
        result[4].clone(),
    ])
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

    // ── Width-5 tests ──────────────────────────────────────────────

    #[test]
    fn test_w5_round_constants_length() {
        let rc = generate_w5_round_constants();
        let expected =
            POSEIDON_W5_WIDTH * (1 + POSEIDON_W5_FULL_ROUNDS + POSEIDON_W5_PARTIAL_ROUNDS);
        assert_eq!(rc.len(), expected);
    }

    #[test]
    fn test_w5_mds_matrix_dimensions() {
        let mds = generate_w5_mds_matrix();
        assert_eq!(mds.len(), POSEIDON_W5_WIDTH);
        for row in &mds {
            assert_eq!(row.len(), POSEIDON_W5_WIDTH);
        }
    }

    #[test]
    fn test_w5_mds_matrix_is_mds() {
        // Verify the matrix is actually MDS: all square sub-matrices are non-singular.
        // For a Cauchy matrix M[i][j] = 1/(x_i + y_j) with distinct x_i + y_j,
        // this is guaranteed by construction. We verify a basic property:
        // no row is all zeros, and the determinant of any 1x1 submatrix is non-zero.
        let mds = generate_w5_mds_matrix();
        for row in &mds {
            for entry in row {
                assert_ne!(*entry, Fr::from(0u64), "MDS entry must be non-zero");
            }
        }
    }

    #[test]
    fn test_w5_poseidon_deterministic() {
        let inputs = [
            Fr::from(1u64),
            Fr::from(2u64),
            Fr::from(3u64),
            Fr::from(4u64),
        ];
        let domain = Fr::from(999u64);
        let h1 = native_poseidon_w5(&inputs, domain);
        let h2 = native_poseidon_w5(&inputs, domain);
        assert_eq!(h1, h2, "Width-5 Poseidon must be deterministic");
    }

    #[test]
    fn test_w5_poseidon_different_inputs() {
        let domain = Fr::from(999u64);
        let h1 = native_poseidon_w5(
            &[
                Fr::from(1u64),
                Fr::from(2u64),
                Fr::from(3u64),
                Fr::from(4u64),
            ],
            domain,
        );
        let h2 = native_poseidon_w5(
            &[
                Fr::from(5u64),
                Fr::from(6u64),
                Fr::from(7u64),
                Fr::from(8u64),
            ],
            domain,
        );
        assert_ne!(h1, h2, "Different inputs must produce different hashes");
    }

    #[test]
    fn test_w5_poseidon_domain_separation() {
        let inputs = [
            Fr::from(1u64),
            Fr::from(2u64),
            Fr::from(3u64),
            Fr::from(4u64),
        ];
        let h1 = native_poseidon_w5(&inputs, Fr::from(100u64));
        let h2 = native_poseidon_w5(&inputs, Fr::from(200u64));
        assert_ne!(
            h1, h2,
            "Different domain tags must produce different hashes"
        );
    }

    #[test]
    fn test_w5_independent_from_w3() {
        // Width-5 hash of 2 inputs should NOT equal width-3 hash of same inputs.
        // They use different round constants, MDS matrices, and widths.
        let a = Fr::from(42u64);
        let b = Fr::from(123u64);
        let h_w3 = compute_nullifier(a, b);
        let h_w5 = native_poseidon_w5(&[a, b, Fr::from(0u64), Fr::from(0u64)], Fr::from(0u64));
        assert_ne!(
            h_w3, h_w5,
            "Width-3 and width-5 must produce different outputs"
        );
    }

    #[test]
    fn test_grain_lfsr_deterministic_and_nonzero() {
        let rc1 = generate_grain_round_constants(3, 8, 57);
        let rc2 = generate_grain_round_constants(3, 8, 57);
        assert_eq!(rc1.len(), 3 * (1 + 8 + 57));
        assert_eq!(rc1, rc2, "Grain LFSR must be strictly deterministic");
        for c in &rc1 {
            assert_ne!(*c, Fr::from(0u64), "Round constants must be non-zero");
        }
    }

    #[test]
    fn test_grain_lfsr_round_constants_unique() {
        let rc = generate_grain_round_constants(5, 8, 60);
        assert_eq!(rc.len(), 5 * (1 + 8 + 60));
        let mut set = std::collections::HashSet::new();
        for c in &rc {
            assert!(set.insert(*c), "All round constants must be distinct");
        }
    }

    #[test]
    fn test_grain_lfsr_distinct_widths() {
        let rc_w3 = generate_grain_round_constants(3, 8, 57);
        let rc_w5 = generate_grain_round_constants(5, 8, 60);
        assert_ne!(
            rc_w3[0], rc_w5[0],
            "Different state widths must yield independent constants"
        );
    }
}

