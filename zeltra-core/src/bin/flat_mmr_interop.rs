//! Flat MMR Interoperability & Cryptographic Test Harness (DEC-045A).
//!
//! Provides CLI interface for independent cross-language validation and testing:
//! 1. Flat MMR tree building & peak position extraction
//! 2. Zero-hash proof generation
//! 3. Cryptographic proof verification
//! 4. Legacy vs Flat MMR parity verification
//! 5. Micro-benchmarking proof latency before vs after

use ark_bls12_381::Fr;
use std::env;
use std::time::Instant;
use zeltra_core::{
    from_evm_scalar, leaf_index_to_pos, to_evm_scalar, FlatMerkleMountainRange, FlatMmrError,
    MMRProof, MerkleMountainRange,
};

fn fr_to_hex(fr: &Fr) -> String {
    let bytes = to_evm_scalar(fr);
    hex::encode(bytes)
}

fn hex_to_fr(s: &str) -> Option<Fr> {
    let clean = s.strip_prefix("0x").unwrap_or(s);
    let bytes = hex::decode(clean).ok()?;
    if bytes.len() != 32 {
        return None;
    }
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&bytes);
    from_evm_scalar(&arr)
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: flat_mmr_interop <command> [args...]");
        std::process::exit(1);
    }

    let command = &args[1];
    match command.as_str() {
        "build_tree" => {
            if args.len() < 3 {
                eprintln!("Usage: flat_mmr_interop build_tree <leaf_count>");
                std::process::exit(1);
            }
            let leaf_count: usize = match args[2].parse() {
                Ok(n) => n,
                Err(_) => {
                    eprintln!("Invalid leaf_count");
                    std::process::exit(1);
                }
            };

            let mut mmr = FlatMerkleMountainRange::new();
            for i in 1..=leaf_count {
                mmr.append(Fr::from(i as u64));
            }

            let peak_positions_json = format!("{:?}", mmr.peak_positions);
            let root_hex = fr_to_hex(&mmr.get_root());
            let invariant_ok = mmr.verify_array_size_invariant();

            println!(
                r#"{{"leaf_count":{},"nodes_len":{},"peak_positions":{},"root_hex":"{}","array_size_invariant_valid":{}}}"#,
                mmr.leaf_count,
                mmr.nodes.len(),
                peak_positions_json,
                root_hex,
                invariant_ok
            );
        }

        "get_proof" => {
            if args.len() < 4 {
                eprintln!("Usage: flat_mmr_interop get_proof <leaf_count> <leaf_index>");
                std::process::exit(1);
            }
            let leaf_count: usize = args[2].parse().unwrap_or(0);
            let leaf_index: usize = args[3].parse().unwrap_or(0);

            let mut mmr = FlatMerkleMountainRange::new();
            for i in 1..=leaf_count {
                mmr.append(Fr::from(i as u64));
            }

            match mmr.generate_proof(leaf_index) {
                Ok(proof) => {
                    let m_sibs: Vec<String> =
                        proof.mountain_siblings.iter().map(fr_to_hex).collect();
                    let p_sibs: Vec<String> =
                        proof.peak_bagging_siblings.iter().map(fr_to_hex).collect();
                    let root_hex = fr_to_hex(&mmr.get_root());
                    let leaf_pos = leaf_index_to_pos(leaf_index);

                    println!(
                        r#"{{"success":true,"leaf_index":{},"leaf_pos":{},"leaf_count":{},"mountain_height":{},"mountain_siblings":{:?},"peak_bagging_siblings":{:?},"peak_index":{},"root_hex":"{}"}}"#,
                        leaf_index,
                        leaf_pos,
                        leaf_count,
                        proof.mountain_height,
                        m_sibs,
                        p_sibs,
                        proof.peak_index,
                        root_hex
                    );
                }
                Err(err) => {
                    println!(r#"{{"success":false,"error":"{}"}}"#, err);
                }
            }
        }

        "verify_proof" => {
            if args.len() < 8 {
                eprintln!("Usage: flat_mmr_interop verify_proof <leaf_hex> <leaf_index> <leaf_count> <mountain_height> <mountain_sibs_csv> <peak_sibs_csv> <peak_index> <root_hex>");
                std::process::exit(1);
            }
            let leaf = match hex_to_fr(&args[2]) {
                Some(f) => f,
                None => {
                    println!(r#"{{"valid":false,"error":"invalid leaf_hex"}}"#);
                    return;
                }
            };
            let leaf_index: usize = args[3].parse().unwrap_or(0);
            let leaf_count: usize = args[4].parse().unwrap_or(0);
            let mountain_height: usize = args[5].parse().unwrap_or(0);

            let m_sibs_csv = &args[6];
            let mut mountain_siblings = Vec::new();
            if !m_sibs_csv.is_empty() && m_sibs_csv != "none" {
                for hex_str in m_sibs_csv.split(',') {
                    if let Some(f) = hex_to_fr(hex_str.trim()) {
                        mountain_siblings.push(f);
                    }
                }
            }

            let p_sibs_csv = &args[7];
            let mut peak_bagging_siblings = Vec::new();
            if !p_sibs_csv.is_empty() && p_sibs_csv != "none" {
                for hex_str in p_sibs_csv.split(',') {
                    if let Some(f) = hex_to_fr(hex_str.trim()) {
                        peak_bagging_siblings.push(f);
                    }
                }
            }

            let peak_index: usize = if args.len() > 8 {
                args[8].parse().unwrap_or(0)
            } else {
                0
            };
            let root_hex = if args.len() > 9 { &args[9] } else { "" };
            let root = match hex_to_fr(root_hex) {
                Some(r) => r,
                None => {
                    println!(r#"{{"valid":false,"error":"invalid root_hex"}}"#);
                    return;
                }
            };

            let proof = MMRProof {
                mountain_height,
                mountain_siblings,
                peak_bagging_siblings,
                peak_index,
            };

            let valid =
                FlatMerkleMountainRange::verify_proof(leaf, leaf_index, leaf_count, &proof, root);
            println!(r#"{{"valid":{}}}"#, valid);
        }

        "compare_parity" => {
            if args.len() < 3 {
                eprintln!("Usage: flat_mmr_interop compare_parity <leaf_count>");
                std::process::exit(1);
            }
            let leaf_count: usize = args[2].parse().unwrap_or(0);
            let mut flat_mmr = FlatMerkleMountainRange::new();
            let mut legacy_mmr = MerkleMountainRange::new();

            for i in 1..=leaf_count {
                let leaf = Fr::from(i as u64);
                flat_mmr.append(leaf);
                legacy_mmr.append(leaf);
            }

            let flat_root = flat_mmr.get_root();
            let legacy_root = legacy_mmr.get_root();
            let parity = flat_root == legacy_root;

            println!(
                r#"{{"parity":{},"flat_root":"{}","legacy_root":"{}"}}"#,
                parity,
                fr_to_hex(&flat_root),
                fr_to_hex(&legacy_root)
            );
        }

        "inspect_node" => {
            if args.len() < 4 {
                eprintln!("Usage: flat_mmr_interop inspect_node <leaf_count> <pos>");
                std::process::exit(1);
            }
            let leaf_count: usize = args[2].parse().unwrap_or(0);
            let pos: usize = args[3].parse().unwrap_or(0);

            let mut mmr = FlatMerkleMountainRange::new();
            for i in 1..=leaf_count {
                mmr.append(Fr::from(i as u64));
            }

            match mmr.nodes.get(pos) {
                Some(node) => {
                    println!(
                        r#"{{"found":true,"pos":{},"hash_hex":"{}"}}"#,
                        pos,
                        fr_to_hex(node)
                    );
                }
                None => {
                    println!(r#"{{"found":false,"pos":{}}}"#, pos);
                }
            }
        }

        "append_non_canonical" => {
            let mut mmr = FlatMerkleMountainRange::new();
            mmr.append(Fr::from(1u64));
            let len_before = mmr.nodes.len();

            let non_canonical_bytes = [0xffu8; 32];
            let res = mmr.append_from_bytes(&non_canonical_bytes);
            let rejected = res == Err(FlatMmrError::NonCanonicalScalar);
            let len_after = mmr.nodes.len();

            println!(
                r#"{{"rejected":{},"state_unchanged":{}}}"#,
                rejected,
                len_before == len_after
            );
        }

        "benchmark_proof" => {
            if args.len() < 4 {
                eprintln!("Usage: flat_mmr_interop benchmark_proof <leaf_count> <sample_size>");
                std::process::exit(1);
            }
            let leaf_count: usize = args[2].parse().unwrap_or(1000);
            let sample_size: usize = args[3].parse().unwrap_or(50);

            let mut flat_mmr = FlatMerkleMountainRange::new();
            let mut legacy_mmr = MerkleMountainRange::new();

            for i in 1..=leaf_count {
                let leaf = Fr::from(i as u64);
                flat_mmr.append(leaf);
                legacy_mmr.append(leaf);
            }

            // Benchmark Flat MMR (Zero-Hash)
            let start_flat = Instant::now();
            for i in 0..sample_size {
                let target = (i * 37) % leaf_count;
                let _ = flat_mmr.generate_proof(target);
            }
            let flat_duration = start_flat.elapsed();
            let flat_avg_us = (flat_duration.as_nanos() as f64) / (sample_size as f64) / 1000.0;

            // Benchmark Legacy MMR (Re-Hashing)
            let start_legacy = Instant::now();
            for i in 0..sample_size {
                let target = (i * 37) % leaf_count;
                let _ = legacy_mmr.generate_proof(target);
            }
            let legacy_duration = start_legacy.elapsed();
            let legacy_avg_us = (legacy_duration.as_nanos() as f64) / (sample_size as f64) / 1000.0;

            let speedup = legacy_avg_us / flat_avg_us.max(0.001);

            println!(
                r#"{{"leaf_count":{},"sample_size":{},"flat_avg_us":{:.3},"legacy_avg_us":{:.3},"speedup_factor":{:.1}}}"#,
                leaf_count, sample_size, flat_avg_us, legacy_avg_us, speedup
            );
        }

        _ => {
            eprintln!("Unknown command: {}", command);
            std::process::exit(1);
        }
    }
}
