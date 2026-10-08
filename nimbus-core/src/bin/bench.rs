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

    println!("================================================================================");
}
