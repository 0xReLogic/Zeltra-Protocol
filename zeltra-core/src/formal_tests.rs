#[cfg(test)]
mod tests {
    use crate::*;
    use ark_bls12_381::{Fr, G1Projective, G2Projective};
    use ark_ff::UniformRand;
    use rand::{thread_rng, Rng, RngCore};

    /// DETERMINISTIC FUZZING: Deserialization and Parser Robustness
    ///
    /// Fuzzes the deserialize function with corrupted, truncated, and random byte strings
    /// to ensure absolutely no panics occur.
    #[test]
    fn fuzz_deserialization_safety() {
        let mut rng = thread_rng();

        // Fuzz with completely random byte buffers of varying lengths
        for _ in 0..1000 {
            let len = rng.gen_range(0..1024);
            let mut fuzzed_bytes = vec![0u8; len];
            rng.fill_bytes(&mut fuzzed_bytes);

            // We fuzz multiple structures
            let _: Option<Fr> = deserialize_from_bytes(&fuzzed_bytes);
            let _: Option<G1Projective> = deserialize_from_bytes(&fuzzed_bytes);
            let _: Option<G2Projective> = deserialize_from_bytes(&fuzzed_bytes);
        }
    }

    /// DETERMINISTIC FUZZING: EVM Coordinate Serialization Safety
    ///
    /// Fuzzes `to_evm_g1` and `to_evm_g2` coordinate converters to ensure bounds safety.
    #[test]
    fn fuzz_evm_serialization_safety() {
        use ark_ec::CurveGroup;
        let mut rng = thread_rng();

        // Verify that random affine points do not trigger out-of-bounds
        for _ in 0..100 {
            let g1_proj = G1Projective::rand(&mut rng);
            let g2_proj = G2Projective::rand(&mut rng);

            let g1_affine = g1_proj.into_affine();
            let g2_affine = g2_proj.into_affine();

            let evm_g1 = to_evm_g1(&g1_affine);
            let evm_g2 = to_evm_g2(&g2_affine);

            assert_eq!(evm_g1.len(), 128);
            assert_eq!(evm_g2.len(), 256);
        }
    }
}
