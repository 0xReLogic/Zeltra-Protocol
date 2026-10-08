use nimbus_core::*;
use rand::thread_rng;
use std::time::Instant;

const ITERATIONS: u32 = 1000;

fn main() {
    let mut rng = thread_rng();

    println!("NIMBUS CRYPTOGRAPHIC BENCHMARK");
    println!("================================================================================");
    println!("Running benchmarks with {} iterations...", ITERATIONS);
    println!("================================================================================");
    println!(
        "{:<30} | {:<12} | {:<12} | {:<12}",
        "Operation", "Total (ms)", "Avg (us)", "Ops/sec"
    );
    println!("--------------------------------------------------------------------------------");

    // 1. Client Blind
    {
        let start = Instant::now();
        for _ in 0..ITERATIONS {
            let message = b"ephemeral_public_key_benchmarking_1234567890";
            let _ = client_blind(message, &mut rng);
        }
        let elapsed = start.elapsed();
        let total_ms = elapsed.as_millis() as f64;
        let avg_us = elapsed.as_micros() as f64 / ITERATIONS as f64;
        let ops_per_sec = ITERATIONS as f64 / elapsed.as_secs_f64();
        println!(
            "{:<30} | {:<12.2} | {:<12.2} | {:<12.0}",
            "Client Blind (G1)", total_ms, avg_us, ops_per_sec
        );
    }

    // 2. Issuer Sign Blinded
    {
        let sk_iss = IssuerSecretKey::generate(&mut rng);
        let message = b"ephemeral_public_key_benchmarking_1234567890";
        let (x, _) = client_blind(message, &mut rng);

        let start = Instant::now();
        for _ in 0..ITERATIONS {
            let _ = issuer_sign_blinded(&sk_iss, &x, &mut rng);
        }
        let elapsed = start.elapsed();
        let total_ms = elapsed.as_millis() as f64;
        let avg_us = elapsed.as_micros() as f64 / ITERATIONS as f64;
        let ops_per_sec = ITERATIONS as f64 / elapsed.as_secs_f64();
        println!(
            "{:<30} | {:<12.2} | {:<12.2} | {:<12.0}",
            "Issuer Sign Blinded (G2 MSM)", total_ms, avg_us, ops_per_sec
        );
    }

    // 3. Client Verify Masked
    {
        let sk_iss = IssuerSecretKey::generate(&mut rng);
        let message = b"ephemeral_public_key_benchmarking_1234567890";
        let (x, _) = client_blind(message, &mut rng);
        let (masked_sig, _, com_k) = issuer_sign_blinded(&sk_iss, &x, &mut rng);

        let start = Instant::now();
        for _ in 0..ITERATIONS {
            let _ = client_verify_masked(&x, &com_k, &masked_sig);
        }
        let elapsed = start.elapsed();
        let total_ms = elapsed.as_millis() as f64;
        let avg_us = elapsed.as_micros() as f64 / ITERATIONS as f64;
        let ops_per_sec = ITERATIONS as f64 / elapsed.as_secs_f64();
        println!(
            "{:<30} | {:<12.2} | {:<12.2} | {:<12.0}",
            "Client Verify Masked (Pairing)", total_ms, avg_us, ops_per_sec
        );
    }

    // 4. Client Unmask
    {
        let sk_iss = IssuerSecretKey::generate(&mut rng);
        let message = b"ephemeral_public_key_benchmarking_1234567890";
        let (x, r) = client_blind(message, &mut rng);
        let (masked_sig, k, _) = issuer_sign_blinded(&sk_iss, &x, &mut rng);

        let start = Instant::now();
        for _ in 0..ITERATIONS {
            let _ = client_unmask(&masked_sig, &r, &k);
        }
        let elapsed = start.elapsed();
        let total_ms = elapsed.as_millis() as f64;
        let avg_us = elapsed.as_micros() as f64 / ITERATIONS as f64;
        let ops_per_sec = ITERATIONS as f64 / elapsed.as_secs_f64();
        println!(
            "{:<30} | {:<12.2} | {:<12.2} | {:<12.0}",
            "Client Unmask (Fr Inv)", total_ms, avg_us, ops_per_sec
        );
    }

    // 5. Verify Unmasked
    {
        let sk_iss = IssuerSecretKey::generate(&mut rng);
        let pk_iss = sk_iss.public_key();
        let message = b"ephemeral_public_key_benchmarking_1234567890";
        let (x, r) = client_blind(message, &mut rng);
        let (masked_sig, k, _) = issuer_sign_blinded(&sk_iss, &x, &mut rng);
        let alpha = client_unmask(&masked_sig, &r, &k).unwrap();

        let start = Instant::now();
        for _ in 0..ITERATIONS {
            let _ = verify_unmasked(message, &alpha, &pk_iss);
        }
        let elapsed = start.elapsed();
        let total_ms = elapsed.as_millis() as f64;
        let avg_us = elapsed.as_micros() as f64 / ITERATIONS as f64;
        let ops_per_sec = ITERATIONS as f64 / elapsed.as_secs_f64();
        println!(
            "{:<30} | {:<12.2} | {:<12.2} | {:<12.0}",
            "Verify Unmasked (Pairing)", total_ms, avg_us, ops_per_sec
        );
    }

    // 6. Poseidon W3 (Nullifier / Rate=2)
    {
        use ark_bls12_381::Fr;
        let s = Fr::from(123456789u64);
        let r = Fr::from(987654321u64);
        let start = Instant::now();
        for _ in 0..ITERATIONS {
            let _ = compute_nullifier(s, r);
        }
        let elapsed = start.elapsed();
        let total_ms = elapsed.as_millis() as f64;
        let avg_us = elapsed.as_micros() as f64 / ITERATIONS as f64;
        let ops_per_sec = ITERATIONS as f64 / elapsed.as_secs_f64();
        println!(
            "{:<30} | {:<12.2} | {:<12.2} | {:<12.0}",
            "Poseidon W3 Permutation", total_ms, avg_us, ops_per_sec
        );
    }

    // 7. Poseidon W5 (Note Commitment / Merkle Node)
    {
        use ark_bls12_381::Fr;
        let inputs = [
            Fr::from(100_000_000u64),
            Fr::from(222222222u64),
            Fr::from(333333333u64),
            Fr::from(444444444u64),
        ];
        let domain = domain_note_commitment();
        let start = Instant::now();
        for _ in 0..ITERATIONS {
            let _ = native_poseidon_w5(&inputs, domain);
        }
        let elapsed = start.elapsed();
        let total_ms = elapsed.as_millis() as f64;
        let avg_us = elapsed.as_micros() as f64 / ITERATIONS as f64;
        let ops_per_sec = ITERATIONS as f64 / elapsed.as_secs_f64();
        println!(
            "{:<30} | {:<12.2} | {:<12.2} | {:<12.0}",
            "Poseidon W5 (Commitment)", total_ms, avg_us, ops_per_sec
        );
    }

    // 8. Merkle Tree Root (Depth 20)
    {
        use ark_bls12_381::Fr;
        let leaf = Fr::from(999999999u64);
        let siblings = [Fr::from(42u64); MERKLE_TREE_DEPTH];
        let start = Instant::now();
        for i in 0..ITERATIONS {
            let _ = compute_merkle_root(leaf, i as u64, &siblings);
        }
        let elapsed = start.elapsed();
        let total_ms = elapsed.as_millis() as f64;
        let avg_us = elapsed.as_micros() as f64 / ITERATIONS as f64;
        let ops_per_sec = ITERATIONS as f64 / elapsed.as_secs_f64();
        println!(
            "{:<30} | {:<12.2} | {:<12.2} | {:<12.0}",
            "Merkle Root (Depth 20)", total_ms, avg_us, ops_per_sec
        );
    }

    // 9. Groth16 2-in-2-out JoinSplit Circuit Prover & Verifier
    {
        println!(
            "--------------------------------------------------------------------------------"
        );
        println!("Benchmarking Groth16 Universal 2-in-2-out JoinSplit Circuit...");
        let setup_start = Instant::now();
        let keys = generate_joinsplit_circuit_keys().expect("joinsplit circuit setup");
        let setup_ms = setup_start.elapsed().as_millis();
        println!("JoinSplit Setup Time: {} ms", setup_ms);

        let circuit = create_dummy_joinsplit_circuit();
        let pub_inputs = extract_joinsplit_public_inputs(&circuit);

        const ZK_ITERATIONS: u32 = 10;
        let mut proof_sample = None;

        let prove_start = Instant::now();
        for _ in 0..ZK_ITERATIONS {
            let p = generate_joinsplit_proof(circuit.clone(), &keys.proving_key)
                .expect("joinsplit prove failed");
            proof_sample = Some(p);
        }
        let prove_elapsed = prove_start.elapsed();
        let prove_avg_ms = prove_elapsed.as_secs_f64() * 1000.0 / ZK_ITERATIONS as f64;
        let prove_ops_per_sec = ZK_ITERATIONS as f64 / prove_elapsed.as_secs_f64();
        println!(
            "{:<30} | {:<12.2} | {:<12.2} | {:<12.2}",
            "JoinSplit Groth16 Prove (2-in-2-out)",
            prove_elapsed.as_millis() as f64,
            prove_avg_ms,
            prove_ops_per_sec
        );

        let proof = proof_sample.unwrap();
        const VERIFY_ITERATIONS: u32 = 100;
        let verify_start = Instant::now();
        for _ in 0..VERIFY_ITERATIONS {
            let valid = verify_joinsplit_proof(&keys.verifying_key, &proof, &pub_inputs)
                .expect("joinsplit verify failed");
            assert!(valid);
        }
        let verify_elapsed = verify_start.elapsed();
        let verify_avg_ms = verify_elapsed.as_secs_f64() * 1000.0 / VERIFY_ITERATIONS as f64;
        let verify_ops_per_sec = VERIFY_ITERATIONS as f64 / verify_elapsed.as_secs_f64();
        println!(
            "{:<30} | {:<12.2} | {:<12.2} | {:<12.2}",
            "JoinSplit Groth16 Verify",
            verify_elapsed.as_millis() as f64,
            verify_avg_ms,
            verify_ops_per_sec
        );
    }

    println!("================================================================================");
}
