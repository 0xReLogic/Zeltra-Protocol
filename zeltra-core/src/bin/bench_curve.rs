use std::time::Instant;
use ark_bls12_381::{Fr, G1Affine, G1Projective};
use ark_ec::{CurveGroup, VariableBaseMSM};
use ark_ff::{AdditiveGroup, UniformRand};

fn main() {
    let mut rng = ark_std::rand::thread_rng();
    println!("================================================================================");
    println!("ZELTRA BLS12-381 CURVE ARITHMETIC BENCHMARK (PAPER IACR 2024/1906 CONTEXT)");
    println!("================================================================================");

    // 1. Single Point Doubling (2P) in Jacobian / Projective
    {
        let p = G1Projective::rand(&mut rng);
        let mut cur = p;
        let iters = 100_000;
        let start = Instant::now();
        for _ in 0..iters {
            cur.double_in_place();
        }
        let elapsed = start.elapsed();
        let avg_ns = elapsed.as_nanos() as f64 / iters as f64;
        let ops_per_sec = iters as f64 / elapsed.as_secs_f64();
        println!(
            "{:<35} | Total: {:<8.2} ms | Avg: {:<8.2} ns ({:<6.3} us) | {:<10.0} ops/s",
            "G1 Point Doubling (2P)", elapsed.as_secs_f64() * 1000.0, avg_ns, avg_ns / 1000.0, ops_per_sec
        );
    }

    // 2. Single Point Addition (P + Q) in Jacobian / Projective
    {
        let p = G1Projective::rand(&mut rng);
        let q = G1Projective::rand(&mut rng);
        let mut cur = p;
        let iters = 100_000;
        let start = Instant::now();
        for _ in 0..iters {
            cur += q;
        }
        let elapsed = start.elapsed();
        let avg_ns = elapsed.as_nanos() as f64 / iters as f64;
        let ops_per_sec = iters as f64 / elapsed.as_secs_f64();
        println!(
            "{:<35} | Total: {:<8.2} ms | Avg: {:<8.2} ns ({:<6.3} us) | {:<10.0} ops/s",
            "G1 Point Addition (P + Q)", elapsed.as_secs_f64() * 1000.0, avg_ns, avg_ns / 1000.0, ops_per_sec
        );
    }

    // 2B. HEAD-TO-HEAD: Point Tripling (3P) — Standard Double-and-Add vs Fast Jacobian (4S + 6M)
    {
        use zeltra_core::curve_opt::fast_triple_p;
        let p = G1Projective::rand(&mut rng);
        let iters = 100_000;

        // Standard Arkworks Tripling: p.double() + p
        let start_std = Instant::now();
        let mut sink_std = p;
        for _ in 0..iters {
            sink_std = sink_std.double() + p;
        }
        let elapsed_std = start_std.elapsed();
        let avg_ns_std = elapsed_std.as_nanos() as f64 / iters as f64;
        let ops_std = iters as f64 / elapsed_std.as_secs_f64();

        // Fast Jacobian Tripling (Proposition 3.1): fast_triple_p(&p)
        let start_fast = Instant::now();
        let mut sink_fast = p;
        for _ in 0..iters {
            sink_fast = fast_triple_p(&sink_fast);
        }
        let elapsed_fast = start_fast.elapsed();
        let avg_ns_fast = elapsed_fast.as_nanos() as f64 / iters as f64;
        let ops_fast = iters as f64 / elapsed_fast.as_secs_f64();

        let speedup = (avg_ns_std - avg_ns_fast) / avg_ns_std * 100.0;
        println!("--------------------------------------------------------------------------------");
        println!(
            "{:<35} | Total: {:<8.2} ms | Avg: {:<8.2} ns ({:<6.3} us) | {:<10.0} ops/s",
            "Standard 3P (2P + P in Arkworks)", elapsed_std.as_secs_f64() * 1000.0, avg_ns_std, avg_ns_std / 1000.0, ops_std
        );
        println!(
            "{:<35} | Total: {:<8.2} ms | Avg: {:<8.2} ns ({:<6.3} us) | {:<10.0} ops/s",
            "FAST 3P (DEC-050: 4S + 6M)", elapsed_fast.as_secs_f64() * 1000.0, avg_ns_fast, avg_ns_fast / 1000.0, ops_fast
        );
        println!(">>> Point Tripling Speedup: {:.2}% faster!", speedup);
        println!("--------------------------------------------------------------------------------");
    }

    // 2C. HEAD-TO-HEAD: Endomorphism tau P = P - phi(P) vs Fast Jacobian (3S + 3M)
    {
        use zeltra_core::curve_opt::{fast_phi_p, fast_tau_p};
        let p = G1Projective::rand(&mut rng);
        let iters = 100_000;

        // Standard Arkworks tau P: p - phi(p)
        let start_std = Instant::now();
        let mut sink_std = p;
        for _ in 0..iters {
            let phi = fast_phi_p(&sink_std);
            sink_std = sink_std - phi;
        }
        let elapsed_std = start_std.elapsed();
        let avg_ns_std = elapsed_std.as_nanos() as f64 / iters as f64;
        let ops_std = iters as f64 / elapsed_std.as_secs_f64();

        // Fast Jacobian tau P (Proposition 3.1): fast_tau_p(&p)
        let start_fast = Instant::now();
        let mut sink_fast = p;
        for _ in 0..iters {
            sink_fast = fast_tau_p(&sink_fast);
        }
        let elapsed_fast = start_fast.elapsed();
        let avg_ns_fast = elapsed_fast.as_nanos() as f64 / iters as f64;
        let ops_fast = iters as f64 / elapsed_fast.as_secs_f64();

        let speedup = (avg_ns_std - avg_ns_fast) / avg_ns_std * 100.0;
        println!(
            "{:<35} | Total: {:<8.2} ms | Avg: {:<8.2} ns ({:<6.3} us) | {:<10.0} ops/s",
            "Standard tau P (P - phi(P))", elapsed_std.as_secs_f64() * 1000.0, avg_ns_std, avg_ns_std / 1000.0, ops_std
        );
        println!(
            "{:<35} | Total: {:<8.2} ms | Avg: {:<8.2} ns ({:<6.3} us) | {:<10.0} ops/s",
            "FAST tau P (DEC-050: 3S + 3M)", elapsed_fast.as_secs_f64() * 1000.0, avg_ns_fast, avg_ns_fast / 1000.0, ops_fast
        );
        println!(">>> Endomorphism tau P Speedup: {:.2}% faster!", speedup);
        println!("--------------------------------------------------------------------------------");
    }

    // 3. Single Scalar Multiplication [k]P (255-bit scalar Fr)
    {
        let p = G1Projective::rand(&mut rng);
        let k = Fr::rand(&mut rng);
        let iters = 5_000;
        let start = Instant::now();
        for _ in 0..iters {
            let _ = p * k;
        }
        let elapsed = start.elapsed();
        let avg_us = elapsed.as_micros() as f64 / iters as f64;
        let avg_ms = avg_us / 1000.0;
        let ops_per_sec = iters as f64 / elapsed.as_secs_f64();
        println!(
            "{:<35} | Total: {:<8.2} ms | Avg: {:<8.2} us ({:<6.3} ms) | {:<10.0} ops/s",
            "G1 Single Scalar Mult [k]P", elapsed.as_secs_f64() * 1000.0, avg_us, avg_ms, ops_per_sec
        );
    }

    // 4. MSM of size 20 (JoinSplit Public Inputs Linear Combination in Groth16 Verify)
    {
        let bases: Vec<G1Affine> = (0..20).map(|_| G1Projective::rand(&mut rng).into_affine()).collect();
        let scalars: Vec<Fr> = (0..20).map(|_| Fr::rand(&mut rng)).collect();
        let iters = 2_000;
        let start = Instant::now();
        for _ in 0..iters {
            let _ = G1Projective::msm(&bases, &scalars).unwrap();
        }
        let elapsed = start.elapsed();
        let avg_us = elapsed.as_micros() as f64 / iters as f64;
        let avg_ms = avg_us / 1000.0;
        let ops_per_sec = iters as f64 / elapsed.as_secs_f64();
        println!(
            "{:<35} | Total: {:<8.2} ms | Avg: {:<8.2} us ({:<6.3} ms) | {:<10.0} ops/s",
            "G1 MSM Size 20 (Groth16 Verifier L)", elapsed.as_secs_f64() * 1000.0, avg_us, avg_ms, ops_per_sec
        );
    }

    // 5. MSM of size 34,571 (Full JoinSplit Circuit Constraints - Groth16 Prover)
    {
        const CIRCUIT_MSM_SIZE: usize = 34571;
        println!("--------------------------------------------------------------------------------");
        println!("Preparing MSM of size {} (identical to JoinSplit Groth16 witness)...", CIRCUIT_MSM_SIZE);
        let bases: Vec<G1Affine> = (0..CIRCUIT_MSM_SIZE).map(|_| G1Projective::rand(&mut rng).into_affine()).collect();
        let scalars: Vec<Fr> = (0..CIRCUIT_MSM_SIZE).map(|_| Fr::rand(&mut rng)).collect();
        let iters = 5;
        let start = Instant::now();
        for _ in 0..iters {
            let _ = G1Projective::msm(&bases, &scalars).unwrap();
        }
        let elapsed = start.elapsed();
        let avg_ms = elapsed.as_secs_f64() * 1000.0 / iters as f64;
        let ops_per_sec = iters as f64 / elapsed.as_secs_f64();
        println!(
            "{:<35} | Total: {:<8.2} ms | Avg: {:<8.2} ms ({:<6.2} s)  | {:<10.2} ops/s",
            "G1 MSM Size 34,571 (Prover Pippenger)", elapsed.as_secs_f64() * 1000.0, avg_ms, avg_ms / 1000.0, ops_per_sec
        );
    }

    println!("================================================================================");
}
