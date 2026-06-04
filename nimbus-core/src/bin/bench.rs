use nimbus_core::*;
use std::time::Instant;
use rand::thread_rng;

const ITERATIONS: u32 = 1000;

fn main() {
    let mut rng = thread_rng();

    println!("NIMBUS CRYPTOGRAPHIC BENCHMARK");
    println!("================================================================================");
    println!("Running benchmarks with {} iterations...", ITERATIONS);
    println!("================================================================================");
    println!("{:<30} | {:<12} | {:<12} | {:<12}", "Operation", "Total (ms)", "Avg (us)", "Ops/sec");
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
        println!("{:<30} | {:<12.2} | {:<12.2} | {:<12.0}", "Client Blind (G1)", total_ms, avg_us, ops_per_sec);
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
        println!("{:<30} | {:<12.2} | {:<12.2} | {:<12.0}", "Issuer Sign Blinded (G2 MSM)", total_ms, avg_us, ops_per_sec);
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
        println!("{:<30} | {:<12.2} | {:<12.2} | {:<12.0}", "Client Verify Masked (Pairing)", total_ms, avg_us, ops_per_sec);
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
        println!("{:<30} | {:<12.2} | {:<12.2} | {:<12.0}", "Client Unmask (Fr Inv)", total_ms, avg_us, ops_per_sec);
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
        println!("{:<30} | {:<12.2} | {:<12.2} | {:<12.0}", "Verify Unmasked (Pairing)", total_ms, avg_us, ops_per_sec);
    }

    // 6. Generate Offline Response
    {
        let identity = Fr::rand(&mut rng);
        let a = Fr::rand(&mut rng);
        let x = Fr::rand(&mut rng);

        let start = Instant::now();
        for _ in 0..ITERATIONS {
            let _ = generate_offline_response(a, x, identity);
        }
        let elapsed = start.elapsed();
        let total_ms = elapsed.as_millis() as f64;
        let avg_us = elapsed.as_micros() as f64 / ITERATIONS as f64;
        let ops_per_sec = ITERATIONS as f64 / elapsed.as_secs_f64();
        println!("{:<30} | {:<12.2} | {:<12.2} | {:<12.0}", "Spend Offline (Fr Mul+Add)", total_ms, avg_us, ops_per_sec);
    }

    // 7. Reconstruct Identity
    {
        let identity = Fr::rand(&mut rng);
        let a = Fr::rand(&mut rng);
        
        let x1 = Fr::rand(&mut rng);
        let y1 = generate_offline_response(a, x1, identity);
        let proof1 = OfflineSpendProof { x: x1, y: y1 };

        let mut x2 = Fr::rand(&mut rng);
        while x2 == x1 {
            x2 = Fr::rand(&mut rng);
        }
        let y2 = generate_offline_response(a, x2, identity);
        let proof2 = OfflineSpendProof { x: x2, y: y2 };

        let start = Instant::now();
        for _ in 0..ITERATIONS {
            let _ = reconstruct_identity(&proof1, &proof2);
        }
        let elapsed = start.elapsed();
        let total_ms = elapsed.as_millis() as f64;
        let avg_us = elapsed.as_micros() as f64 / ITERATIONS as f64;
        let ops_per_sec = ITERATIONS as f64 / elapsed.as_secs_f64();
        println!("{:<30} | {:<12.2} | {:<12.2} | {:<12.0}", "Reconstruct Identity (Slash)", total_ms, avg_us, ops_per_sec);
    }

    println!("================================================================================");
}
