use std::time::Instant;
use ark_bls12_381::{Fr, G2Projective};
use ark_ec::PrimeGroup;
use ark_ff::UniformRand;
use ark_relations::gr1cs::{ConstraintSynthesizer, ConstraintSystem};
use rand::thread_rng;

use nimbus_core::{
    client_blind, client_unmask, verify_unmasked, sign_share, aggregate_shares,
    split_secret_key, IssuerSecretKey, IssuerPublicKey, MaskingKey,
    create_dummy_circuit, extract_public_inputs, generate_note_proof, verify_note_proof,
    get_or_init_note_circuit_keys,
    note_commitment,
};

fn main() {
    println!("================================================================================");
    println!("     AUDIT BENCHMARK RESMI: GROTH16 VS THRESHOLD BLS (REAL HARDWARE RUN)");
    println!("================================================================================\n");

    // 1. INFO ENVIRONMENT
    let cpu_info = std::fs::read_to_string("/proc/cpuinfo").unwrap_or_default();
    let model_line = cpu_info.lines().find(|l| l.starts_with("model name")).unwrap_or("model name: Unknown");
    let cores = cpu_info.lines().filter(|l| l.starts_with("processor")).count();
    println!("[HARDWARE CPU]");
    println!("  • {}", model_line.trim());
    println!("  • Total Logical Core: {}", cores);
    println!("  • Rust Target: Release Profile (opt-level = 3, LTO/codegen default)");
    println!();

    // 2. CONSTRAINT SYSTEM AUDIT GROTH16
    let cs = ConstraintSystem::<Fr>::new_ref();
    create_dummy_circuit().generate_constraints(cs.clone()).expect("Constraint generation failed");
    let num_constraints = cs.num_constraints();
    let num_instance = cs.num_instance_variables();
    let num_witness = cs.num_witness_variables();
    let is_satisfied = cs.is_satisfied().unwrap();

    println!("[METRIK SIRKUIT R1CS PRODUKSI (PrivateNoteCircuit)]");
    println!("  • Jumlah R1CS Constraints : {}", num_constraints);
    println!("  • Public Inputs (Instance): {}", num_instance);
    println!("  • Private Inputs (Witness): {}", num_witness);
    println!("  • Constraint Satisfied    : {}", is_satisfied);
    assert!(is_satisfied, "Sirkuit harus satisfied!");
    println!();

    // 3. LOAD KEYS
    print!("[*] Memuat Proving Key & Verifying Key... ");
    let t_load = Instant::now();
    let keys = get_or_init_note_circuit_keys();
    println!("Done ({:.2?})\n", t_load.elapsed());

    // 4. BENCHMARK GROTH16 PROVE TIAP RUN (NO FLUFF)
    println!("--------------------------------------------------------------------------------");
    println!("PENGUKURAN RIIL: GROTH16 PROVER (Waktu Tiap Sample Individu)");
    println!("--------------------------------------------------------------------------------");
    let mut prove_durations = Vec::new();
    let runs = 5;

    for i in 1..=runs {
        let circ = create_dummy_circuit();
        let t0 = Instant::now();
        let proof = generate_note_proof(circ, &keys.proving_key).expect("Proving failed");
        let elapsed = t0.elapsed();
        prove_durations.push(elapsed);

        // Langsung validasi keabsahan proof
        let pub_inputs = extract_public_inputs(&create_dummy_circuit());
        let valid = verify_note_proof(&proof, &keys.verifying_key, &pub_inputs);
        println!("  [Sample #{}] Durasi: {:>8.4?} | Proof Valid: {}", i, elapsed, valid);
        assert!(valid, "Proof harus valid matematis!");
    }

    let min_prove = prove_durations.iter().min().unwrap();
    let max_prove = prove_durations.iter().max().unwrap();
    let total_prove: std::time::Duration = prove_durations.iter().sum();
    let avg_prove = total_prove / (runs as u32);
    println!("\n  >>> HASIL GROTH16 PROVE:");
    println!("      • Min : {:.4?}", min_prove);
    println!("      • Max : {:.4?}", max_prove);
    println!("      • Mean: {:.4?}", avg_prove);
    println!();

    // 5. BENCHMARK GROTH16 VERIFY
    println!("--------------------------------------------------------------------------------");
    println!("PENGUKURAN RIIL: GROTH16 VERIFIER (CPU arkworks MSM + 4-Pairing)");
    println!("--------------------------------------------------------------------------------");
    let proof = generate_note_proof(create_dummy_circuit(), &keys.proving_key).unwrap();
    let pub_inputs = extract_public_inputs(&create_dummy_circuit());

    let v_runs = 50;
    let t0_v = Instant::now();
    for _ in 0..v_runs {
        let ok = verify_note_proof(&proof, &keys.verifying_key, &pub_inputs);
        assert!(ok);
    }
    let avg_verify = t0_v.elapsed() / v_runs;
    println!("  >>> HASIL GROTH16 VERIFY ({} runs average): {:.4?}", v_runs, avg_verify);
    println!();

    // 6. BENCHMARK POSEIDON COMMITMENT (DEPOSIT DIRECT MMR)
    println!("--------------------------------------------------------------------------------");
    println!("PENGUKURAN RIIL: POSEIDON NOTE COMMITMENT (Komputasi Klien Saat Deposit)");
    println!("--------------------------------------------------------------------------------");
    let p_runs = 1000;
    let mut rng = thread_rng();
    let sk = Fr::rand(&mut rng);
    let rho = Fr::rand(&mut rng);
    let r = Fr::rand(&mut rng);

    let t0_p = Instant::now();
    for _ in 0..p_runs {
        let _ = note_commitment(100_000_000, sk, rho, r);
    }
    let avg_poseidon = t0_p.elapsed() / p_runs;
    println!("  >>> HASIL POSEIDON COMMITMENT ({} runs average): {:.4?}", p_runs, avg_poseidon);
    println!();

    // 7. BENCHMARK THRESHOLD BLS INFLOW
    println!("--------------------------------------------------------------------------------");
    println!("PENGUKURAN RIIL: THRESHOLD BLS BDHKE (3-of-5 Quorum)");
    println!("--------------------------------------------------------------------------------");
    let issuer_sk = IssuerSecretKey(Fr::rand(&mut rng));
    let issuer_pk = IssuerPublicKey(G2Projective::generator() * issuer_sk.0);
    let shares = split_secret_key(&issuer_sk, 3, 5, &mut rng);
    let k_scalar = Fr::rand(&mut rng);
    let k = MaskingKey(k_scalar);
    let msg = b"sample_session_deposit_message";

    let bls_runs = 200;

    // A. Blind
    let t0 = Instant::now();
    for _ in 0..bls_runs {
        let _ = client_blind(msg, &mut rng);
    }
    let avg_blind = t0.elapsed() / bls_runs;

    // B. Sign Share
    let (blinded, r_factor) = client_blind(msg, &mut rng);
    let t0 = Instant::now();
    for _ in 0..bls_runs {
        let _ = sign_share(&shares[0].1, &blinded, &k.0);
    }
    let avg_sign_share = t0.elapsed() / bls_runs;

    // C. Aggregate
    let p_sigs = vec![
        (shares[0].0, sign_share(&shares[0].1, &blinded, &k.0)),
        (shares[1].0, sign_share(&shares[1].1, &blinded, &k.0)),
        (shares[2].0, sign_share(&shares[2].1, &blinded, &k.0)),
    ];
    let t0 = Instant::now();
    for _ in 0..bls_runs {
        let _ = aggregate_shares(&p_sigs).unwrap();
    }
    let avg_agg = t0.elapsed() / bls_runs;

    // D. Unmask
    let masked = aggregate_shares(&p_sigs).unwrap();
    let t0 = Instant::now();
    for _ in 0..bls_runs {
        let _ = client_unmask(&masked, &r_factor, &k).unwrap();
    }
    let avg_unmask = t0.elapsed() / bls_runs;

    // E. Verify Unmasked (2 Pairings)
    let unmasked = client_unmask(&masked, &r_factor, &k).unwrap();
    let t0 = Instant::now();
    for _ in 0..bls_runs {
        let ok = verify_unmasked(msg, &unmasked, &issuer_pk);
        assert!(ok);
    }
    let avg_bls_verify = t0.elapsed() / bls_runs;

    println!("  >>> KOMPONEN OPERASI BLS ({} runs average):", bls_runs);
    println!("      • Client Blind (Hash to G1 + Mul) : {:.4?}", avg_blind);
    println!("      • 1 Guardian Partial Sign         : {:.4?}", avg_sign_share);
    println!("      • Lagrange Aggregation (3 shares) : {:.4?}", avg_agg);
    println!("      • Client Unmasking                : {:.4?}", avg_unmask);
    println!("      • BLS Signature Verify (2 Pairing): {:.4?}", avg_bls_verify);
    println!("      • Total Crypto Komputasi BLS      : {:.4?}", avg_blind + (avg_sign_share * 3) + avg_agg + avg_unmask + avg_bls_verify);
    println!();

    println!("================================================================================");
    println!("DATA SUMMARY RESMI (BENAR-BENAR TERVERIFIKASI DI HARDWARE LINUX)");
    println!("================================================================================");
    println!("1. DI SISI PENGGUNA (CLIENT):");
    println!("   • Saat Deposit Groth16 Direct MMR : {:.4?} (Poseidon Hash murni)", avg_poseidon);
    println!("   • Saat Deposit BLS               : {:.4?} (Blind + Unmask, belum termasuk network)", avg_blind + avg_unmask);
    println!("   • Saat Belanja Groth16 (Prove)   : {:.4?} (Sirkuit {} Constraints)", avg_prove, num_constraints);
    println!();
    println!("2. DI SISI VERIFIKASI (VERIFIER):");
    println!("   • Verifikasi Groth16 (4 Pairings): {:.4?}", avg_verify);
    println!("   • Verifikasi BLS (2 Pairings)    : {:.4?}", avg_bls_verify);
    println!("================================================================================");
}
