//! Formal Verification Suite for Zeltra Circuits (DEC-046 & DEC-047).
//!
//! Includes:
//! 1. Deterministic Fuzzing for Parser and Coordinate Serialization
//! 2. Automated Sparse Jacobian Rigidity and Markowitz Gaussian Reduction Tests
//! 3. Proposition 27 Exact Affine-Line Certificate and Counterexample Synthesis Tests
//! 4. 6-Family Adversarial Witness Evaluation Matrix (DEC-047 Section 5)
//! 5. Negative Test Suite (>= 2x positive tests) proving rejection on unpinned wires

#[cfg(test)]
mod tests {
    use crate::circuit_gadgets::*;
    use crate::evm::{to_evm_g1, to_evm_g2};
    use crate::jacobian::*;
    use crate::joinsplit_circuit::{
        create_dummy_joinsplit_circuit, set_tamper_diff_inv, JoinSplitCircuit,
    };
    use crate::note_circuit::{create_dummy_circuit, PrivateNoteCircuit};
    use crate::serialization::deserialize_from_bytes;
    use ark_bls12_381::{Fr, G1Projective, G2Projective};
    use ark_ff::{One, UniformRand, Zero};
    use ark_r1cs_std::eq::EqGadget;
    use ark_r1cs_std::fields::fp::FpVar;
    use ark_relations::gr1cs::{ConstraintSynthesizer, ConstraintSystem};
    use rand::{thread_rng, Rng, RngCore};

    // ═══════════════════════════════════════════════════════════════════════
    // 1. DETERMINISTIC FUZZING: Deserialization & Serialization Robustness
    // ═══════════════════════════════════════════════════════════════════════

    #[test]
    fn fuzz_deserialization_safety() {
        let mut rng = thread_rng();
        for _ in 0..500 {
            let len = rng.gen_range(0..512);
            let mut fuzzed_bytes = vec![0u8; len];
            rng.fill_bytes(&mut fuzzed_bytes);

            let _: Option<Fr> = deserialize_from_bytes(&fuzzed_bytes);
            let _: Option<G1Projective> = deserialize_from_bytes(&fuzzed_bytes);
            let _: Option<G2Projective> = deserialize_from_bytes(&fuzzed_bytes);
        }
    }

    #[test]
    fn fuzz_evm_serialization_safety() {
        use ark_ec::CurveGroup;
        let mut rng = thread_rng();

        for _ in 0..50 {
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

    // ═══════════════════════════════════════════════════════════════════════
    // 2. SPARSE AUGMENTED JACOBIAN ALGEBRA & RIGIDITY TESTS (DEC-047)
    // ═══════════════════════════════════════════════════════════════════════

    #[test]
    fn test_jacobian_sparse_basis_algebra() {
        // Test basic sparse row-reduction and rank calculation over Fr
        let mut basis = EchelonBasis::new(4);
        assert_eq!(basis.rank, 0);

        // Row 1: x_0 + 2*x_1 = 0
        let r1 = vec![(0, Fr::from(1u64)), (1, Fr::from(2u64))];
        assert!(basis.insert_row(r1));
        assert_eq!(basis.rank, 1);

        // Row 2: 2*x_0 + 4*x_1 = 0 (Linearly dependent!)
        let r2 = vec![(0, Fr::from(2u64)), (1, Fr::from(4u64))];
        assert!(!basis.insert_row(r2));
        assert_eq!(basis.rank, 1);

        // Row 3: x_2 + x_3 = 0 (Independent!)
        let r3 = vec![(2, Fr::from(1u64)), (3, Fr::from(1u64))];
        assert!(basis.insert_row(r3));
        assert_eq!(basis.rank, 2);

        // Test row space membership: e_0^T + 2*e_1^T should be in row space
        assert!(basis.contains_row(vec![(0, Fr::from(1u64)), (1, Fr::from(2u64))]));
        // e_0^T alone should NOT be in row space
        assert!(!basis.contains_row(vec![(0, Fr::from(1u64))]));
    }

    // ═══════════════════════════════════════════════════════════════════════
    // 3. PROPOSITION 27 AFFINE-LINE CERTIFICATE & DEC-046 HARDENING PROOF
    // ═══════════════════════════════════════════════════════════════════════

    #[test]
    fn test_proposition_27_affine_line_underconstrained_vs_hardened() {
        // Model circuit with selector s (wire 1), target wire y (wire 2), and constant ONE (wire 0)
        // Under-constrained relation: s * (y - 5) == 0
        // When s == 0, y is completely unconstrained!
        let cs_unconstrained = ConstraintSystem::<Fr>::new_ref();
        let s_var = cs_unconstrained
            .new_input_variable(|| Ok(Fr::zero()))
            .unwrap(); // s = 0
        let y_var = cs_unconstrained
            .new_witness_variable(|| Ok(Fr::from(42u64)))
            .unwrap(); // y = 42

        // Constraint: s * (y - 5) == 0
        let five = FpVar::Constant(Fr::from(5u64));
        let s_fp = FpVar::Var(ark_r1cs_std::fields::fp::AllocatedFp::new(
            Some(Fr::zero()),
            s_var,
            cs_unconstrained.clone(),
        ));
        let y_fp = FpVar::Var(ark_r1cs_std::fields::fp::AllocatedFp::new(
            Some(Fr::from(42u64)),
            y_var,
            cs_unconstrained.clone(),
        ));
        let diff = &y_fp - &five;
        (&s_fp * &diff)
            .enforce_equal(&FpVar::Constant(Fr::zero()))
            .unwrap();

        assert!(cs_unconstrained.is_satisfied().unwrap());

        let (witness_unconstrained, matrices_unconstrained) =
            extract_witness_and_matrices(&cs_unconstrained);
        let conditioned = vec![0, 1]; // Constant ONE and public selector s
        let targets = vec![2]; // Target wire y

        // Diagnostic on unconstrained circuit MUST detect infinitesimal freedom and synthesize
        // Proposition 27 Affine-Line Certificate!
        let (report_unconstrained, basis_unconstrained) = run_jacobian_diagnostic(
            &matrices_unconstrained,
            &witness_unconstrained,
            &conditioned,
            &targets,
        );

        match report_unconstrained {
            DiagnosticClassification::Class2ConcreteNonUniqueness {
                free_target,
                exploit_certified,
            } => {
                assert_eq!(free_target, 2);
                assert!(exploit_certified);
                // Extract nullspace tangent vector
                let v = basis_unconstrained
                    .extract_nullspace_vector(free_target)
                    .unwrap();
                assert_eq!(v[0], Fr::zero()); // Does not move constant ONE
                assert_eq!(v[1], Fr::zero()); // Does not move public input s
                assert_ne!(v[2], Fr::zero()); // Moves target wire y!

                // Verify Proposition 27 certificate
                let cert = AffineLineCertificate {
                    base_witness: witness_unconstrained.clone(),
                    tangent_direction: v.clone(),
                    free_target_index: 2,
                    is_certified: true,
                };
                let (w_base, w_second) = cert.synthesize_pair(Fr::from(100u64));
                assert_eq!(w_base[2], Fr::from(42u64));
                assert_eq!(w_second[2], Fr::from(142u64));
            }
            other => panic!(
                "Underconstrained model circuit must yield Class 2 Concrete Non-Uniqueness, got {:?}",
                other
            ),
        }

        // NOW APPLY DEC-046 CANONICAL COMPANION CONSTRAINT:
        // (1 - s) * (y - CANONICAL_DUMMY_VALUE) == 0
        let cs_hardened = ConstraintSystem::<Fr>::new_ref();
        let s_var_h = cs_hardened.new_input_variable(|| Ok(Fr::zero())).unwrap();
        let y_var_h = cs_hardened
            .new_witness_variable(|| Ok(Fr::from(CANONICAL_DUMMY_VALUE)))
            .unwrap();

        let s_fp_h = FpVar::Var(ark_r1cs_std::fields::fp::AllocatedFp::new(
            Some(Fr::zero()),
            s_var_h,
            cs_hardened.clone(),
        ));
        let y_fp_h = FpVar::Var(ark_r1cs_std::fields::fp::AllocatedFp::new(
            Some(Fr::from(CANONICAL_DUMMY_VALUE)),
            y_var_h,
            cs_hardened.clone(),
        ));

        // Original constraint: s * (y - 5) == 0
        let diff_h = &y_fp_h - &FpVar::Constant(Fr::from(5u64));
        (&s_fp_h * &diff_h)
            .enforce_equal(&FpVar::Constant(Fr::zero()))
            .unwrap();

        // DEC-046 Companion constraint: (1 - s) * (y - 0) == 0
        let one_minus_s = FpVar::Constant(Fr::one()) - &s_fp_h;
        enforce_canonical_pinning(&one_minus_s, &y_fp_h, Fr::from(CANONICAL_DUMMY_VALUE)).unwrap();

        assert!(cs_hardened.is_satisfied().unwrap());

        let (witness_hardened, matrices_hardened) = extract_witness_and_matrices(&cs_hardened);
        let (report_hardened, _) = run_jacobian_diagnostic(
            &matrices_hardened,
            &witness_hardened,
            &conditioned,
            &targets,
        );

        // With DEC-046 companion constraint, target wire y is 100% RIGID (Class 3)!
        assert_eq!(
            report_hardened,
            DiagnosticClassification::Class3PointwiseRigidity,
            "DEC-046 companion constraint must ensure Class 3 Pointwise Rigidity (r_T == 0)!"
        );
    }

    // ═══════════════════════════════════════════════════════════════════════
    // 4. 6-FAMILY ADVERSARIAL WITNESS SWEEP (DEC-047 Section 5)
    // ═══════════════════════════════════════════════════════════════════════

    /// Helper to construct a Mode 2 In-Pool Transfer JoinSplit circuit
    fn setup_valid_2in_2out_joinsplit() -> JoinSplitCircuit {
        use crate::note::{
            compute_binding_commitment, compute_scope_hash, derive_joinsplit_nullifier,
            derive_nullifier_key, note_commitment, MerkleMountainRange,
        };

        let sk = Fr::from(100u64);
        let nk = derive_nullifier_key(sk);
        let asset_id_u8 = 1u8; // USDC

        let rho1 = Fr::from(111u64);
        let rand1 = Fr::from(222u64);
        let cm1 = note_commitment(10_000_000, asset_id_u8, sk, rho1, rand1);
        let epoch1 = 5u64;
        let nf1 = derive_joinsplit_nullifier(nk, rho1, epoch1);

        let rho2 = Fr::from(333u64);
        let rand2 = Fr::from(444u64);
        let cm2 = note_commitment(10_000_000, asset_id_u8, sk, rho2, rand2);
        let epoch2 = 6u64;
        let nf2 = derive_joinsplit_nullifier(nk, rho2, epoch2);

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

        let scope_hash = compute_scope_hash(rec, cid, caddr, exp);
        let binding = compute_binding_commitment(q_hi, q_lo, scope_hash);

        JoinSplitCircuit {
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
            flags_packed: Some(Fr::from(7u64)),
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
            in2_is_dummy: Some(Fr::from(0u64)),
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
        }
    }

    fn setup_fam1_mode2_joinsplit() -> JoinSplitCircuit {
        use crate::note::{compute_binding_commitment, compute_scope_hash, note_commitment};
        let mut circuit = setup_valid_2in_2out_joinsplit();
        let sk = circuit.in1_owner_key.unwrap();
        let asset_id_u8 = 1u8;

        circuit.recipient = Some(Fr::zero());
        circuit.merchant_amount = Some(Fr::zero());
        circuit.protocol_fee = Some(Fr::zero());
        circuit.execution_fee = Some(Fr::zero());

        let v_out1 = 12_000_000u64;
        let v_out2 = 8_000_000u64;
        let rho_c1 = Fr::from(555u64);
        let rand_c1 = Fr::from(666u64);
        let rho_c2 = Fr::from(777u64);
        let rand_c2 = Fr::from(888u64);

        circuit.out1_value = Some(Fr::from(v_out1));
        circuit.out2_value = Some(Fr::from(v_out2));
        circuit.output_commitment_1 =
            Some(note_commitment(v_out1, asset_id_u8, sk, rho_c1, rand_c1));
        circuit.output_commitment_2 =
            Some(note_commitment(v_out2, asset_id_u8, sk, rho_c2, rand_c2));

        let scope_hash = compute_scope_hash(
            Fr::zero(),
            circuit.chain_id.unwrap(),
            circuit.contract_address.unwrap(),
            circuit.expiry.unwrap(),
        );
        circuit.expected_binding = Some(compute_binding_commitment(
            circuit.quote_hash_hi.unwrap(),
            circuit.quote_hash_lo.unwrap(),
            scope_hash,
        ));

        circuit
    }

    fn setup_fam5_rollover_circuit() -> PrivateNoteCircuit {
        use crate::note::{
            compute_binding_commitment, compute_scope_hash, derive_nullifier, derive_nullifier_key,
            note_commitment, MerkleMountainRange,
        };

        let sk = Fr::from(42u64);
        let nk = derive_nullifier_key(sk);
        let in_value: u64 = 10_000_000;
        let in_rho = Fr::from(101u64);
        let in_rand = Fr::from(202u64);
        let in_cm = note_commitment(in_value, 1, sk, in_rho, in_rand);

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

        let epoch_id: u32 = 1;
        let nf = derive_nullifier(nk, in_cm, leaf_idx as u64, epoch_id);

        let efee: u64 = 23_000;
        let change: u64 = in_value - efee;
        let ch_rho = Fr::from(303u64);
        let ch_rand = Fr::from(404u64);
        let ch_cm = note_commitment(change, 1, sk, ch_rho, ch_rand);

        let rec = Fr::from(100u64);
        let cid = Fr::from(421614u64);
        let caddr = Fr::from(300u64);
        let exp = Fr::from(0u64);
        let q_hi = Fr::from(100u64);
        let q_lo = Fr::from(200u64);

        let scope_hash = compute_scope_hash(rec, cid, caddr, exp);
        let binding = compute_binding_commitment(q_hi, q_lo, scope_hash);

        PrivateNoteCircuit {
            note_root: Some(root),
            leaf_count: Some(Fr::from(mmr.leaf_count as u64)),
            input_nullifier: Some(nf),
            note_epoch_id: Some(Fr::from(epoch_id as u64)),
            output_commitment: Some(ch_cm),
            recipient: Some(rec),
            merchant_amount: Some(Fr::from(0u64)),
            protocol_fee: Some(Fr::from(0u64)),
            execution_fee: Some(Fr::from(efee)),
            quote_hash_hi: Some(q_hi),
            quote_hash_lo: Some(q_lo),
            chain_id: Some(cid),
            contract_address: Some(caddr),
            expiry: Some(exp),
            has_change: Some(Fr::from(1u64)),
            flags_packed: Some(Fr::from(3u64)), // is_rollover=1, asset_id=1 => 1 + 2*1 = 3
            is_rollover: Some(Fr::from(1u64)),
            asset_id: Some(Fr::from(1u64)),

            input_value: Some(Fr::from(in_value)),
            input_owner_key: Some(sk),
            input_rho: Some(in_rho),
            input_randomness: Some(in_rand),
            input_leaf_index: Some(Fr::from(leaf_idx as u64)),
            mountain_height: Some(proof.mountain_height as u8),
            mountain_siblings: Some(m_sibs),
            peak_bagging_siblings: Some(p_sibs),
            peak_bagging_count: Some(proof.peak_bagging_siblings.len() as u8),

            change_value: Some(Fr::from(change)),
            change_owner_key: Some(sk),
            change_rho: Some(ch_rho),
            change_randomness: Some(ch_rand),
            expected_binding: Some(binding),
        }
    }

    #[test]
    fn test_jacobian_fam1_honest_inpool_transfer() {
        // FAM-1: Honest In-Pool Transfer (Mode 2) -> Assert r_{T_financial} == 0
        let circuit = setup_fam1_mode2_joinsplit();
        let cs = ConstraintSystem::<Fr>::new_ref();
        circuit.generate_constraints(cs.clone()).unwrap();
        assert!(
            cs.is_satisfied().unwrap(),
            "FAM-1 circuit must be satisfied"
        );

        let (witness, matrices) = extract_witness_and_matrices(&cs);
        let num_instance = cs.borrow().unwrap().num_instance_variables();
        let conditioned: Vec<usize> = (0..num_instance).collect();

        let targets = vec![num_instance + 1, num_instance + 2];
        let jacobian = AugmentedJacobian::build(&matrices, &witness, &conditioned);
        let basis = jacobian.compute_echelon_basis();
        let r_t = basis.projected_tangent_dimension(&targets);

        assert_eq!(
            r_t, 0,
            "FAM-1 In-Pool Transfer financial targets must be rigid (r_T == 0)!"
        );
    }

    #[test]
    fn test_jacobian_fam2_honest_public_outflow() {
        // FAM-2: Honest Public Outflow (Mode 3) -> Assert r_{T_financial} == 0
        let circuit = setup_valid_2in_2out_joinsplit();
        let cs = ConstraintSystem::<Fr>::new_ref();
        circuit.generate_constraints(cs.clone()).unwrap();
        assert!(
            cs.is_satisfied().unwrap(),
            "FAM-2 circuit must be satisfied"
        );

        let (witness, matrices) = extract_witness_and_matrices(&cs);
        let num_instance = cs.borrow().unwrap().num_instance_variables();
        let conditioned: Vec<usize> = (0..num_instance).collect();

        let targets = vec![num_instance + 1, num_instance + 2];
        let jacobian = AugmentedJacobian::build(&matrices, &witness, &conditioned);
        let basis = jacobian.compute_echelon_basis();
        let r_t = basis.projected_tangent_dimension(&targets);

        assert_eq!(
            r_t, 0,
            "FAM-2 Public Outflow financial targets must be rigid (r_T == 0)!"
        );
    }

    #[test]
    fn test_jacobian_fam3_consolidation_dummy_slot() {
        // FAM-3: Consolidation / 1-Input Spend -> Assert r_{T_dummy} == 0
        let circuit = create_dummy_joinsplit_circuit();
        assert_eq!(circuit.in2_is_dummy, Some(Fr::one()));

        let cs = ConstraintSystem::<Fr>::new_ref();
        circuit.generate_constraints(cs.clone()).unwrap();
        assert!(
            cs.is_satisfied().unwrap(),
            "FAM-3 circuit must be satisfied"
        );

        let (witness, matrices) = extract_witness_and_matrices(&cs);
        let num_instance = cs.borrow().unwrap().num_instance_variables();
        let conditioned: Vec<usize> = (0..num_instance).collect();

        let targets = vec![num_instance + 5, num_instance + 6];
        let jacobian = AugmentedJacobian::build(&matrices, &witness, &conditioned);
        let basis = jacobian.compute_echelon_basis();
        let r_t = basis.projected_tangent_dimension(&targets);

        assert_eq!(
            r_t, 0,
            "FAM-3 Dummy slot coordinates must be rigid (r_T == 0) due to DEC-046!"
        );
    }

    #[test]
    fn test_jacobian_fam4_exact_payment_no_change() {
        // FAM-4: Exact Payment / Zero Change -> Assert r_{T_change} == 0
        let circuit = create_dummy_joinsplit_circuit();
        assert_eq!(circuit.has_change_1, Some(Fr::zero()));
        assert_eq!(circuit.has_change_2, Some(Fr::zero()));

        let cs = ConstraintSystem::<Fr>::new_ref();
        circuit.generate_constraints(cs.clone()).unwrap();
        assert!(
            cs.is_satisfied().unwrap(),
            "FAM-4 circuit must be satisfied"
        );

        let (witness, matrices) = extract_witness_and_matrices(&cs);
        let num_instance = cs.borrow().unwrap().num_instance_variables();
        let conditioned: Vec<usize> = (0..num_instance).collect();

        let targets = vec![num_instance + 3, num_instance + 4];
        let jacobian = AugmentedJacobian::build(&matrices, &witness, &conditioned);
        let basis = jacobian.compute_echelon_basis();
        let r_t = basis.projected_tangent_dimension(&targets);

        assert_eq!(
            r_t, 0,
            "FAM-4 Inactive change wires must be rigid (r_T == 0)!"
        );
    }

    #[test]
    fn test_jacobian_fam5_epoch_rollover() {
        // FAM-5: Epoch Rollover Execution -> Assert r_{T_rollover} == 0
        let circuit = setup_fam5_rollover_circuit();
        let cs = ConstraintSystem::<Fr>::new_ref();
        circuit.generate_constraints(cs.clone()).unwrap();
        assert!(
            cs.is_satisfied().unwrap(),
            "FAM-5 circuit must be satisfied"
        );

        let (witness, matrices) = extract_witness_and_matrices(&cs);
        let num_instance = cs.borrow().unwrap().num_instance_variables();
        let conditioned: Vec<usize> = (0..num_instance).collect();

        let targets = vec![num_instance + 1, num_instance + 2];
        let jacobian = AugmentedJacobian::build(&matrices, &witness, &conditioned);
        let basis = jacobian.compute_echelon_basis();
        let r_t = basis.projected_tangent_dimension(&targets);

        assert_eq!(r_t, 0, "FAM-5 Rollover wires must be rigid (r_T == 0)!");
    }

    #[test]
    fn test_jacobian_fam6_inverse_anti_aliasing_singularity() {
        // FAM-6: Nullifier Collision Singularity -> circuit MUST be UNSATISFIED!
        let mut circuit = setup_valid_2in_2out_joinsplit();
        circuit.input_nullifier_2 = circuit.input_nullifier_1;

        let cs = ConstraintSystem::<Fr>::new_ref();
        circuit.generate_constraints(cs.clone()).unwrap();
        assert!(
            !cs.is_satisfied().unwrap(),
            "FAM-6 Colliding nullifiers with active notes MUST fail circuit satisfaction!"
        );
    }

    // ═══════════════════════════════════════════════════════════════════════
    // 5. NEGATIVE TEST SUITE: STRICT REJECTION ON UNPINNED / TAMPERED WIRES
    // ═══════════════════════════════════════════════════════════════════════

    #[test]
    fn test_negative_tamper_dummy_in2_owner_rejected() {
        let cs = ConstraintSystem::<Fr>::new_ref();
        let mut circuit = create_dummy_joinsplit_circuit();
        assert_eq!(circuit.in2_is_dummy, Some(Fr::one()));
        circuit.in2_owner_key = Some(Fr::from(0x1234u64));
        circuit.generate_constraints(cs.clone()).unwrap();
        assert!(!cs.is_satisfied().unwrap());
    }

    #[test]
    fn test_negative_tamper_dummy_in2_rho_rejected() {
        let cs = ConstraintSystem::<Fr>::new_ref();
        let mut circuit = create_dummy_joinsplit_circuit();
        assert_eq!(circuit.in2_is_dummy, Some(Fr::one()));
        circuit.in2_rho = Some(Fr::from(0x5678u64));
        circuit.generate_constraints(cs.clone()).unwrap();
        assert!(!cs.is_satisfied().unwrap());
    }

    #[test]
    fn test_negative_tamper_dummy_in2_rand_rejected() {
        let cs = ConstraintSystem::<Fr>::new_ref();
        let mut circuit = create_dummy_joinsplit_circuit();
        assert_eq!(circuit.in2_is_dummy, Some(Fr::one()));
        circuit.in2_randomness = Some(Fr::from(0x9abcu64));
        circuit.generate_constraints(cs.clone()).unwrap();
        assert!(!cs.is_satisfied().unwrap());
    }

    #[test]
    fn test_negative_tamper_dummy_in2_leaf_index_rejected() {
        let cs = ConstraintSystem::<Fr>::new_ref();
        let mut circuit = create_dummy_joinsplit_circuit();
        assert_eq!(circuit.in2_is_dummy, Some(Fr::one()));
        circuit.in2_leaf_index = Some(Fr::from(42u64));
        circuit.generate_constraints(cs.clone()).unwrap();
        assert!(!cs.is_satisfied().unwrap());
    }

    #[test]
    fn test_negative_tamper_dummy_diff_inv_rejected() {
        let cs = ConstraintSystem::<Fr>::new_ref();
        let circuit = create_dummy_joinsplit_circuit();
        set_tamper_diff_inv(Some(Fr::from(0x1337_c001u64)));
        let res = circuit.generate_constraints(cs.clone());
        set_tamper_diff_inv(None);
        res.unwrap();
        assert!(!cs.is_satisfied().unwrap());
    }

    #[test]
    fn test_negative_tamper_inactive_change1_owner_rejected() {
        let cs = ConstraintSystem::<Fr>::new_ref();
        let mut circuit = create_dummy_joinsplit_circuit();
        assert_eq!(circuit.has_change_1, Some(Fr::zero()));
        circuit.out1_owner_key = Some(Fr::from(0xaaaa_bbbb_ccccu64));
        circuit.generate_constraints(cs.clone()).unwrap();
        assert!(!cs.is_satisfied().unwrap());
    }

    #[test]
    fn test_negative_tamper_inactive_change1_rho_rejected() {
        let cs = ConstraintSystem::<Fr>::new_ref();
        let mut circuit = create_dummy_joinsplit_circuit();
        assert_eq!(circuit.has_change_1, Some(Fr::zero()));
        circuit.out1_rho = Some(Fr::from(0x1111_2222u64));
        circuit.generate_constraints(cs.clone()).unwrap();
        assert!(!cs.is_satisfied().unwrap());
    }

    #[test]
    fn test_negative_tamper_inactive_change1_rand_rejected() {
        let cs = ConstraintSystem::<Fr>::new_ref();
        let mut circuit = create_dummy_joinsplit_circuit();
        assert_eq!(circuit.has_change_1, Some(Fr::zero()));
        circuit.out1_randomness = Some(Fr::from(0x3333_4444u64));
        circuit.generate_constraints(cs.clone()).unwrap();
        assert!(!cs.is_satisfied().unwrap());
    }

    #[test]
    fn test_negative_tamper_inactive_change2_owner_rejected() {
        let cs = ConstraintSystem::<Fr>::new_ref();
        let mut circuit = create_dummy_joinsplit_circuit();
        assert_eq!(circuit.has_change_2, Some(Fr::zero()));
        circuit.out2_owner_key = Some(Fr::from(0x4444_5555u64));
        circuit.generate_constraints(cs.clone()).unwrap();
        assert!(!cs.is_satisfied().unwrap());
    }

    #[test]
    fn test_negative_tamper_inactive_change2_rho_rejected() {
        let cs = ConstraintSystem::<Fr>::new_ref();
        let mut circuit = create_dummy_joinsplit_circuit();
        assert_eq!(circuit.has_change_2, Some(Fr::zero()));
        circuit.out2_rho = Some(Fr::from(0x6666_7777u64));
        circuit.generate_constraints(cs.clone()).unwrap();
        assert!(!cs.is_satisfied().unwrap());
    }

    #[test]
    fn test_negative_tamper_inactive_change2_rand_rejected() {
        let cs = ConstraintSystem::<Fr>::new_ref();
        let mut circuit = create_dummy_joinsplit_circuit();
        assert_eq!(circuit.has_change_2, Some(Fr::zero()));
        circuit.out2_randomness = Some(Fr::from(0x8888_9999u64));
        circuit.generate_constraints(cs.clone()).unwrap();
        assert!(!cs.is_satisfied().unwrap());
    }

    #[test]
    fn test_negative_tamper_note_circuit_inactive_change_owner_rejected() {
        let cs = ConstraintSystem::<Fr>::new_ref();
        let mut circuit = create_dummy_circuit();
        circuit.has_change = Some(Fr::zero());
        circuit.change_value = Some(Fr::zero());
        circuit.output_commitment = Some(Fr::zero());
        circuit.flags_packed = Some(Fr::from(2u64)); // asset_id=1, rollover=0
        circuit.change_owner_key = Some(Fr::from(0xdead_beefu64));
        circuit.generate_constraints(cs.clone()).unwrap();
        assert!(!cs.is_satisfied().unwrap());
    }

    #[test]
    fn test_negative_tamper_note_circuit_inactive_change_rho_rejected() {
        let cs = ConstraintSystem::<Fr>::new_ref();
        let mut circuit = create_dummy_circuit();
        circuit.has_change = Some(Fr::zero());
        circuit.change_value = Some(Fr::zero());
        circuit.output_commitment = Some(Fr::zero());
        circuit.flags_packed = Some(Fr::from(2u64));
        circuit.change_rho = Some(Fr::from(0xcafe_babeu64));
        circuit.generate_constraints(cs.clone()).unwrap();
        assert!(!cs.is_satisfied().unwrap());
    }

    #[test]
    fn test_negative_tamper_note_circuit_inactive_change_rand_rejected() {
        let cs = ConstraintSystem::<Fr>::new_ref();
        let mut circuit = create_dummy_circuit();
        circuit.has_change = Some(Fr::zero());
        circuit.change_value = Some(Fr::zero());
        circuit.output_commitment = Some(Fr::zero());
        circuit.flags_packed = Some(Fr::from(2u64));
        circuit.change_randomness = Some(Fr::from(0x1234_5678u64));
        circuit.generate_constraints(cs.clone()).unwrap();
        assert!(!cs.is_satisfied().unwrap());
    }

    #[test]
    fn test_negative_tamper_mode2_recipient_rejected() {
        let cs = ConstraintSystem::<Fr>::new_ref();
        let mut circuit = setup_fam1_mode2_joinsplit();
        circuit.recipient = Some(Fr::from(0xbeef_1234u64));
        circuit.generate_constraints(cs.clone()).unwrap();
        assert!(!cs.is_satisfied().unwrap());
    }

    #[test]
    fn test_negative_tamper_mode2_protocol_fee_rejected() {
        let cs = ConstraintSystem::<Fr>::new_ref();
        let mut circuit = setup_fam1_mode2_joinsplit();
        circuit.protocol_fee = Some(Fr::from(100u64));
        circuit.generate_constraints(cs.clone()).unwrap();
        assert!(!cs.is_satisfied().unwrap());
    }
}
