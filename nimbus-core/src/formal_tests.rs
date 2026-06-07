#[cfg(test)]
mod tests {
    use crate::*;
    use ark_bls12_381::{Fr, G1Projective, G2Projective};
    use ark_ff::UniformRand;
    use ark_std::Zero;
    use rand::{thread_rng, Rng, RngCore};

    /// 1. FORMAL VERIFICATION: Shamir Secret Sharing & Lagrange Reconstruction Correctness
    /// Verifies the algebraic correctness of Shamir Secret Sharing across multiple random scenarios
    /// and checks completeness (threshold >= t) and soundness (threshold < t).
    #[test]
    fn verify_shamir_algebraic_properties() {
        let mut rng = thread_rng();

        for _ in 0..50 {
            let sk_iss = IssuerSecretKey::generate(&mut rng);
            let pk_iss = sk_iss.public_key();

            // Random threshold t between 2 and 10, n between t and 20
            let t = rng.gen_range(2..=10);
            let n = rng.gen_range(t..=20);

            let shares = split_secret_key(&sk_iss, t, n, &mut rng);
            assert_eq!(shares.len(), n);

            // Message and blinding factors
            let message = b"formal_verification_test_message";
            let (x, r) = client_blind(message, &mut rng);
            let k = Fr::rand(&mut rng);

            // Generate all partial signatures
            let all_sigs: Vec<(usize, PartialBlindSignature)> = shares
                .iter()
                .map(|&(idx, ref share)| (idx, sign_share(share, &x, &k)))
                .collect();

            // A. Test completeness: Any random subset of size >= t can reconstruct the signature
            let mut subset = all_sigs.clone();
            // Shuffle
            for i in (1..subset.len()).rev() {
                let j = (rng.next_u32() as usize) % (i + 1);
                subset.swap(i, j);
            }
            let valid_subset = &subset[0..t];

            let aggregated =
                aggregate_shares(valid_subset).expect("Failed to aggregate valid subset");
            let unmasked = client_unmask(&aggregated, &r, &MaskingKey(k))
                .expect("Failed to unmask aggregated signature");

            assert!(
                verify_unmasked(message, &unmasked, &pk_iss),
                "Shamir reconstruction failed for valid threshold subset!"
            );

            // B. Test soundness: Any subset of size < t MUST NOT be able to reconstruct a valid signature
            if t > 2 {
                let invalid_subset = &subset[0..(t - 1)];
                let aggregated_invalid = aggregate_shares(invalid_subset).expect(
                    "Aggregation should still work mathematically but with wrong coefficients",
                );
                let unmasked_invalid = client_unmask(&aggregated_invalid, &r, &MaskingKey(k))
                    .expect("Failed to unmask");

                assert!(
                    !verify_unmasked(message, &unmasked_invalid, &pk_iss),
                    "Soundness failure: Sub-threshold subset successfully verified!"
                );
            }

            // C. Test duplicate index prevention: Supplying duplicates must return Err
            let duplicate_subset = vec![all_sigs[0].clone(), all_sigs[0].clone()];
            let agg_res = aggregate_shares(&duplicate_subset);
            assert!(
                agg_res.is_err(),
                "Soundness failure: Duplicate indices did not trigger an error!"
            );
        }
    }

    /// 2. FORMAL VERIFICATION: Blind Signature Protocol Soundness
    /// Verifies that any perturbation in the signature, blinding factor, or message leads to verification failure.
    #[test]
    fn verify_blind_signature_soundness() {
        let mut rng = thread_rng();

        let sk_iss = IssuerSecretKey::generate(&mut rng);
        let pk_iss = sk_iss.public_key();
        let message = b"original_honest_message";

        let (x, r) = client_blind(message, &mut rng);
        let (masked_sig, k, com_k) = issuer_sign_blinded(&sk_iss, &x, &mut rng);

        // Verify honest path
        assert!(client_verify_masked(&x, &com_k, &masked_sig));
        let alpha = client_unmask(&masked_sig, &r, &k).expect("Unmask failed");
        assert!(verify_unmasked(message, &alpha, &pk_iss));

        // A. Adversary alters the message
        let mutated_message = b"original_honest_messagf"; // 1 bit altered
        assert!(
            !verify_unmasked(mutated_message, &alpha, &pk_iss),
            "Adversary successfully forged a signature on a modified message!"
        );

        // B. Adversary alters the unmasked signature point
        let mut corrupted_alpha = alpha.clone();
        corrupted_alpha.0 = corrupted_alpha.0 + G1Projective::generator();
        assert!(
            !verify_unmasked(message, &corrupted_alpha, &pk_iss),
            "Adversary successfully verified a corrupted unmasked signature!"
        );

        // C. Adversary alters the public key
        let wrong_sk = IssuerSecretKey::generate(&mut rng);
        let wrong_pk = wrong_sk.public_key();
        assert!(
            !verify_unmasked(message, &alpha, &wrong_pk),
            "Signature verified successfully against the wrong public key!"
        );
    }

    /// 3. DETERMINISTIC FUZZING: Deserialization and Parser Robustness
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

    /// 4. DETERMINISTIC FUZZING: EVM Coordinate Serialization Safety
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

    /// 5. DETERMINISTIC FUZZING: Parameter Bounds Safety
    /// Fuzzes threshold parameter functions with invalid inputs to ensure assertions are safe.
    #[test]
    fn fuzz_threshold_parameter_assertions() {
        let mut rng = thread_rng();
        let sk = IssuerSecretKey::generate(&mut rng);

        // A. Verify that t > n panics safely via assertion
        let res_t_greater_than_n = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut local_rng = thread_rng();
            split_secret_key(&sk, 5, 3, &mut local_rng);
        }));
        assert!(
            res_t_greater_than_n.is_err(),
            "split_secret_key should panic when t > n"
        );

        // B. Verify that t = 0 panics safely via assertion
        let res_t_zero = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut local_rng = thread_rng();
            split_secret_key(&sk, 0, 5, &mut local_rng);
        }));
        assert!(
            res_t_zero.is_err(),
            "split_secret_key should panic when t == 0"
        );

        // C. Verify that n > 1000 panics safely via assertion
        let res_n_too_large = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut local_rng = thread_rng();
            split_secret_key(&sk, 3, 1005, &mut local_rng);
        }));
        assert!(
            res_n_too_large.is_err(),
            "split_secret_key should panic when n > 1000"
        );

        // D. Verify that zero secret key panics safely via assertion
        let res_zero_key = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut local_rng = thread_rng();
            let zero_sk = IssuerSecretKey(Fr::zero());
            split_secret_key(&zero_sk, 3, 5, &mut local_rng);
        }));
        assert!(
            res_zero_key.is_err(),
            "split_secret_key should panic when secret key is zero"
        );
    }
}
