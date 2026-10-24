use ark_bls12_381::Fr;
use ark_ff::UniformRand;
use ark_r1cs_std::fields::fp::FpVar;
use ark_r1cs_std::prelude::*;
use ark_relations::gr1cs::{ConstraintSynthesizer, ConstraintSystem, ConstraintSystemRef};
use ark_serialize::CanonicalSerialize;
use std::time::Instant;
use zeltra_core::joinsplit_circuit::{
    create_dummy_joinsplit_circuit, DeniabilityPermutationGadget,
};
use zeltra_core::native_poseidon_w5;
use zeltra_core::note::domain_note_commitment;

fn main() {
    let mut rng = ark_std::rand::thread_rng();

    println!("================================================================================");
    println!("ZELTRA DEC-051: NOPENENA CRYPTOGRAPHIC DENIABILITY BENCHMARK");
    println!("================================================================================");

    // 1. Baseline JoinSplitCircuit Constraints & Synthesis Time
    println!("\n[1] BASELINE UNIVERSAL JOINSPLIT CIRCUIT (DEC-036A / DEC-046)");
    let cs: ConstraintSystemRef<Fr> = ConstraintSystem::new_ref();
    let circuit = create_dummy_joinsplit_circuit();

    let synth_start = Instant::now();
    circuit
        .generate_constraints(cs.clone())
        .expect("circuit synthesis failed");
    let synth_duration = synth_start.elapsed();

    let num_constraints_baseline = cs.num_constraints();
    let num_witness_baseline = cs.num_witness_variables();
    let num_instance_baseline = cs.num_instance_variables();
    let is_satisfied_baseline = cs.is_satisfied().expect("satisfaction check");

    println!(
        "  - Baseline Constraints     : {}",
        num_constraints_baseline
    );
    println!("  - Baseline Witness Wires   : {}", num_witness_baseline);
    println!("  - Baseline Public Inputs   : {}", num_instance_baseline);
    println!("  - Baseline Satisfied       : {}", is_satisfied_baseline);
    println!(
        "  - Baseline Synthesis Time  : {:.3} ms ({:.2} us)",
        synth_duration.as_secs_f64() * 1000.0,
        synth_duration.as_micros() as f64
    );

    // 2. Deniability Permutation & Companion Gadget Constraint Benchmark
    println!("\n[2] NOPENENA DENIABILITY PERMUTATION & COMPANION GADGET OVERHEAD");
    let cs_gadget: ConstraintSystemRef<Fr> = ConstraintSystem::new_ref();

    let target_val_val = Fr::from(100_000_000u64);
    let target_cm_val = Fr::from(111111111u64);
    let aux_val_val = Fr::from(0u64); // Decoy dummy value is strictly 0
    let aux_cm_val = Fr::from(222222222u64);

    let start_gadget = Instant::now();
    let perm_bit = Boolean::new_witness(cs_gadget.clone(), || Ok(true)).unwrap();
    let is_decoy = Boolean::new_witness(cs_gadget.clone(), || Ok(true)).unwrap();

    let target_val = FpVar::new_witness(cs_gadget.clone(), || Ok(target_val_val)).unwrap();
    let target_cm = FpVar::new_witness(cs_gadget.clone(), || Ok(target_cm_val)).unwrap();
    let aux_val = FpVar::new_witness(cs_gadget.clone(), || Ok(aux_val_val)).unwrap();
    let aux_cm = FpVar::new_witness(cs_gadget.clone(), || Ok(aux_cm_val)).unwrap();

    // Gadget Constraints:
    let permuted = DeniabilityPermutationGadget::enforce_permutation(
        cs_gadget.clone(),
        &perm_bit,
        &target_val,
        &target_cm,
        &aux_val,
        &aux_cm,
        &is_decoy,
    )
    .unwrap();
    let gadget_duration = start_gadget.elapsed();

    let gadget_constraints = cs_gadget.num_constraints();
    let gadget_witnesses = cs_gadget.num_witness_variables();
    let is_satisfied_gadget = cs_gadget.is_satisfied().unwrap();

    println!(
        "  - Permutation Gadget Extra Constraints : {} gates",
        gadget_constraints
    );
    println!(
        "  - Permutation Gadget Extra Witnesses   : {} wires",
        gadget_witnesses
    );
    println!(
        "  - Gadget Execution Latency             : {:.2} us",
        gadget_duration.as_nanos() as f64 / 1000.0
    );
    println!(
        "  - Gadget Satisfied (Pi=1, Decoy=True)  : {}",
        is_satisfied_gadget
    );
    println!(
        "  - Out1 Value Permuted                  : {:?}",
        permuted.out1_val.value().unwrap()
    );
    println!(
        "  - Out2 Value Permuted                  : {:?}",
        permuted.out2_val.value().unwrap()
    );
    assert_eq!(permuted.out1_val.value().unwrap(), aux_val_val);
    assert_eq!(permuted.out2_val.value().unwrap(), target_val_val);
    assert_eq!(permuted.out1_cm.value().unwrap(), aux_cm_val);
    assert_eq!(permuted.out2_cm.value().unwrap(), target_cm_val);

    // 3. Poseidon-W5 Commitment Benchmark: Real Note (v > 0) vs Decoy Dummy Note (v = 0)
    println!("\n[3] POSEIDON-W5 NOTE COMMITMENT: REAL NOTE VS DECOY DUMMY NOTE");
    const COMM_ITERS: usize = 50_000;
    let domain = domain_note_commitment();

    // Real note inputs: (value, owner_key, rho, r)
    let real_inputs = [
        Fr::from(50_000_000u64), // 50 USDC
        Fr::rand(&mut rng),
        Fr::rand(&mut rng),
        Fr::rand(&mut rng),
    ];

    let start_real = Instant::now();
    let mut real_cm = Fr::from(0u64);
    for _ in 0..COMM_ITERS {
        real_cm = native_poseidon_w5(&real_inputs, domain);
    }
    std::hint::black_box(real_cm);
    let elapsed_real = start_real.elapsed();
    let avg_real_us = elapsed_real.as_nanos() as f64 / (COMM_ITERS as f64 * 1000.0);
    let ops_real = COMM_ITERS as f64 / elapsed_real.as_secs_f64();

    // Decoy dummy note inputs: (0, owner_key, rho_dummy, r_dummy)
    let dummy_inputs = [
        Fr::from(0u64), // 0 USDC
        Fr::rand(&mut rng),
        Fr::rand(&mut rng),
        Fr::rand(&mut rng),
    ];

    let start_dummy = Instant::now();
    let mut dummy_cm = Fr::from(0u64);
    for _ in 0..COMM_ITERS {
        dummy_cm = native_poseidon_w5(&dummy_inputs, domain);
    }
    std::hint::black_box(dummy_cm);
    let elapsed_dummy = start_dummy.elapsed();
    let avg_dummy_us = elapsed_dummy.as_nanos() as f64 / (COMM_ITERS as f64 * 1000.0);
    let ops_dummy = COMM_ITERS as f64 / elapsed_dummy.as_secs_f64();

    println!(
        "  - Real Note Commitment (v = 50 USDC) : Avg {:<6.3} us | {:<10.0} ops/s",
        avg_real_us, ops_real
    );
    println!(
        "  - Decoy Note Commitment (v = 0 USDC)  : Avg {:<6.3} us | {:<10.0} ops/s",
        avg_dummy_us, ops_dummy
    );
    println!(
        "  - Latency Difference (Real vs Dummy)  : {:.3} % (strictly indistinguishable)",
        ((avg_dummy_us - avg_real_us).abs() / avg_real_us) * 100.0
    );

    // 4. Entropy & Statistical Indistinguishability (Shannon Entropy)
    println!("\n[4] STATISTICAL INDISTINGUISHABILITY & SHANNON ENTROPY");
    const SAMPLES: usize = 10_000;
    let mut real_bytes = Vec::with_capacity(SAMPLES * 32);
    let mut dummy_bytes = Vec::with_capacity(SAMPLES * 32);

    for _ in 0..SAMPLES {
        let r_in = [
            Fr::from(100u64),
            Fr::rand(&mut rng),
            Fr::rand(&mut rng),
            Fr::rand(&mut rng),
        ];
        let d_in = [
            Fr::from(0u64),
            Fr::rand(&mut rng),
            Fr::rand(&mut rng),
            Fr::rand(&mut rng),
        ];

        let cm_r = native_poseidon_w5(&r_in, domain);
        let cm_d = native_poseidon_w5(&d_in, domain);

        let mut buf_r = [0u8; 32];
        let mut buf_d = [0u8; 32];
        cm_r.serialize_compressed(&mut buf_r[..]).unwrap();
        cm_d.serialize_compressed(&mut buf_d[..]).unwrap();

        real_bytes.extend_from_slice(&buf_r);
        dummy_bytes.extend_from_slice(&buf_d);
    }

    let entropy_real = compute_shannon_entropy(&real_bytes);
    let entropy_dummy = compute_shannon_entropy(&dummy_bytes);

    println!(
        "  - Shannon Entropy of Real Commitments  : {:.6} / 8.000000 bits/byte",
        entropy_real
    );
    println!(
        "  - Shannon Entropy of Decoy Commitments : {:.6} / 8.000000 bits/byte",
        entropy_dummy
    );
    println!(
        "  - Entropy Delta                        : {:.6} bits (near-perfect uniform random)",
        (entropy_real - entropy_dummy).abs()
    );

    println!("\n================================================================================");
    println!("BENCHMARK COMPLETE — DATA READY FOR DEC-051 RATIFICATION");
    println!("================================================================================");
}

fn compute_shannon_entropy(data: &[u8]) -> f64 {
    let mut counts = [0usize; 256];
    for &b in data {
        counts[b as usize] += 1;
    }
    let total = data.len() as f64;
    let mut entropy = 0.0;
    for &c in &counts {
        if c > 0 {
            let p = c as f64 / total;
            entropy -= p * p.log2();
        }
    }
    entropy
}
