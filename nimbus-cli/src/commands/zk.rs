//! ZK-UTXO CLI commands (DEC-038A / DEC-038C)

use nimbus_core::note::{
    derive_note_randomness, derive_nullifier_key, derive_spending_key, derive_viewing_keys,
    encrypt_note_memo, note_commitment, MerkleMountainRange,
};
use nimbus_core::{from_evm_scalar, to_evm_scalar, Fr, UniformRand};
use rand::Rng;

/// Generate a new random spending secret, full viewing key, and incoming viewing key (DEC-038C)
pub fn generate_wallet<R: Rng>(out_file: Option<&str>, rng: &mut R) {
    let mut seed = [0u8; 32];
    rng.fill(&mut seed);
    let spending_key = derive_spending_key(&seed);
    let nullifier_key = derive_nullifier_key(spending_key);
    let (ivk, ovk) = derive_viewing_keys(&seed);
    let ivpk = ivk.public_key();

    let seed_hex = format!("0x{}", hex::encode(seed));
    let sk_hex = format!("0x{}", hex::encode(to_evm_scalar(&spending_key)));
    let nk_hex = format!("0x{}", hex::encode(to_evm_scalar(&nullifier_key)));
    let ivk_hex = format!("0x{}", hex::encode(ivk.secret_bytes()));
    let ovk_hex = format!("0x{}", hex::encode(ovk.as_bytes()));
    let ivpk_hex = format!("0x{}", hex::encode(ivpk));

    println!("NEW ZK-UTXO WALLET GENERATED (DEC-038C)");
    println!("------------------------------------------------------------");
    println!("Master Seed (32 bytes)             : {}", seed_hex);
    println!("Spending Key (Master Secret, sk)   : {}", sk_hex);
    println!("Nullifier Secret Key (nk)          : {}", nk_hex);
    println!("Incoming Viewing Key (ivk)         : {}", ivk_hex);
    println!("Outcoming Viewing Key (ovk / fvk)  : {}", ovk_hex);
    println!("Incoming Viewing Public Key (ivpk) : {}", ivpk_hex);
    println!("------------------------------------------------------------");

    if let Some(path) = out_file {
        let json = serde_json::json!({
            "master_seed": seed_hex,
            "spending_key": sk_hex,
            "nullifier_key": nk_hex,
            "incoming_viewing_key": ivk_hex,
            "outcoming_viewing_key": ovk_hex,
            "incoming_viewing_pubkey": ivpk_hex,
        });
        if let Err(e) = std::fs::write(path, serde_json::to_string_pretty(&json).unwrap()) {
            eprintln!("Failed to write wallet file to {}: {}", path, e);
        } else {
            println!("Wallet credentials saved to {}", path);
        }
    }
}

/// Create a private note and note commitment with encrypted memo for direct deposit (DEC-038C)
pub fn create_deposit_note<R: Rng>(amount: u64, spending_key_hex: Option<&str>, rng: &mut R) {
    let (spending_key, seed) = match spending_key_hex {
        Some(s) => {
            let clean = s.trim_start_matches("0x");
            let bytes = hex::decode(clean).expect("Invalid spending key hex");
            assert_eq!(bytes.len(), 32, "Spending key must be 32 bytes");
            let mut arr = [0u8; 32];
            arr.copy_from_slice(&bytes);
            let sk = from_evm_scalar(&arr).expect("Non-canonical field scalar for spending key");
            (sk, arr)
        }
        None => {
            let mut seed = [0u8; 32];
            rng.fill(&mut seed);
            let sk = derive_spending_key(&seed);
            (sk, seed)
        }
    };

    let rho = Fr::rand(rng);
    let randomness = derive_note_randomness(spending_key, rho);
    let commitment = note_commitment(amount, spending_key, rho, randomness);
    let (ivk, _) = derive_viewing_keys(&seed);
    let enc_memo = encrypt_note_memo(amount, rho, 1 /* USDC */, &ivk);

    let cm_bytes = to_evm_scalar(&commitment);
    let rho_bytes = to_evm_scalar(&rho);
    let r_bytes = to_evm_scalar(&randomness);

    println!("NEW DIRECT DEPOSIT NOTE GENERATED (DEC-038C)");
    println!("------------------------------------------------------------");
    println!("Amount (micro-USDC)         : {}", amount);
    println!("Note Commitment (cm)        : 0x{}", hex::encode(cm_bytes));
    println!("Note Nullifier Seed (rho)   : 0x{}", hex::encode(rho_bytes));
    println!("Commitment Randomness (r)   : 0x{}", hex::encode(r_bytes));
    println!("Encrypted Memo (96 bytes)   : 0x{}", hex::encode(&enc_memo));
    println!("------------------------------------------------------------");
}

/// Inspect Merkle Mountain Range peaks and root for a set of note commitments (DEC-038C)
pub fn inspect_mmr(commitments_hex: Option<&str>, rpc_url: Option<&str>) {
    println!("MERKLE MOUNTAIN RANGE INSPECTOR (DEC-032 / DEC-038C)");
    println!("------------------------------------------------------------");
    if let Some(commitments) = commitments_hex {
        let hex_items: Vec<&str> = commitments
            .split(',')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();
        println!("Leaf Count: {}", hex_items.len());

        let mut mmr = MerkleMountainRange::new();
        for item in hex_items.iter() {
            let clean_hex = item.trim_start_matches("0x");
            let bytes = hex::decode(clean_hex).expect("Invalid leaf commitment hex");
            assert_eq!(bytes.len(), 32, "Commitment must be 32 bytes");
            let mut arr = [0u8; 32];
            arr.copy_from_slice(&bytes);
            let scalar = from_evm_scalar(&arr).expect("Non-canonical scalar");
            let (leaf_idx, root) = mmr.append_leaf(scalar);
            let root_bytes = to_evm_scalar(&root);
            println!(
                "Leaf #{}: 0x{} => Root: 0x{}",
                leaf_idx,
                clean_hex,
                hex::encode(root_bytes)
            );
        }

        let root_bytes = to_evm_scalar(&mmr.bagged_root());
        println!("------------------------------------------------------------");
        println!("Final MMR Bagged Root: 0x{}", hex::encode(root_bytes));
        println!("Active MMR Peaks: {}", mmr.get_peaks().len());
        for (i, peak) in mmr.get_peaks().iter().enumerate() {
            println!("  Peak #{}: 0x{}", i, hex::encode(to_evm_scalar(peak)));
        }
        println!("------------------------------------------------------------");
    } else if let Some(rpc) = rpc_url {
        println!("Querying MMR tip from RPC: {}", rpc);
        println!("Tip: use endpoint /api/v1/mmr/tip for active state synchronization");
    } else {
        println!("Please provide --commitments <c1,c2,...> or --rpc-url <url>");
    }
}

/// Verify a Groth16 proof locally using Arkworks verifier (DEC-038C)
pub fn verify_proof(
    circuit: &str,
    proof_a_neg: Option<&str>,
    proof_b: Option<&str>,
    proof_c: Option<&str>,
    public_inputs: Option<&str>,
    proof_file: Option<&str>,
    inputs_file: Option<&str>,
) -> bool {
    println!("GROTH16 PROOF VERIFIER (DEC-038C)");
    println!("------------------------------------------------------------");
    println!("Circuit Type: {}", circuit);

    let (a_neg_hex, b_hex, c_hex, pi_list): (String, String, String, Vec<String>) =
        if let Some(file_path) = proof_file {
            let content = std::fs::read_to_string(file_path).expect("Failed to read proof file");
            let v: serde_json::Value =
                serde_json::from_str(&content).expect("Invalid JSON proof file");
            let a = v["proof_a_neg"]
                .as_str()
                .or_else(|| v["proof_a"].as_str())
                .expect("Missing proof_a_neg in JSON")
                .to_string();
            let b = v["proof_b"]
                .as_str()
                .expect("Missing proof_b in JSON")
                .to_string();
            let c = v["proof_c"]
                .as_str()
                .expect("Missing proof_c in JSON")
                .to_string();
            let pis = if let Some(arr) = v["public_inputs"].as_array() {
                arr.iter()
                    .map(|item| item.as_str().unwrap_or("").to_string())
                    .collect()
            } else if let Some(inp_path) = inputs_file {
                let inp_content =
                    std::fs::read_to_string(inp_path).expect("Failed to read inputs file");
                serde_json::from_str(&inp_content).expect("Invalid inputs JSON")
            } else {
                vec![]
            };
            (a, b, c, pis)
        } else {
            let a = proof_a_neg
                .expect("Must provide --proof-a-neg or --proof-file")
                .to_string();
            let b = proof_b.expect("Must provide --proof-b").to_string();
            let c = proof_c.expect("Must provide --proof-c").to_string();
            let pis = if let Some(raw) = public_inputs {
                raw.split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect()
            } else if let Some(inp_path) = inputs_file {
                let inp_content =
                    std::fs::read_to_string(inp_path).expect("Failed to read inputs file");
                serde_json::from_str(&inp_content).expect("Invalid inputs JSON")
            } else {
                vec![]
            };
            (a, b, c, pis)
        };

    let a_bytes = hex::decode(a_neg_hex.trim_start_matches("0x")).expect("Invalid proof_a_neg hex");
    let b_bytes = hex::decode(b_hex.trim_start_matches("0x")).expect("Invalid proof_b hex");
    let c_bytes = hex::decode(c_hex.trim_start_matches("0x")).expect("Invalid proof_c hex");

    assert_eq!(
        a_bytes.len(),
        128,
        "proof_a_neg must be 128 bytes EVM format"
    );
    assert_eq!(b_bytes.len(), 256, "proof_b must be 256 bytes EVM format");
    assert_eq!(c_bytes.len(), 128, "proof_c must be 128 bytes EVM format");

    let mut a_arr = [0u8; 128];
    a_arr.copy_from_slice(&a_bytes);
    let mut b_arr = [0u8; 256];
    b_arr.copy_from_slice(&b_bytes);
    let mut c_arr = [0u8; 128];
    c_arr.copy_from_slice(&c_bytes);

    let proof = nimbus_core::from_evm_proof(&a_arr, &b_arr, &c_arr)
        .expect("Failed to deserialize EVM proof points into Arkworks Proof");

    let parsed_pis: Vec<Fr> = pi_list
        .iter()
        .map(|pi_str| {
            let b = hex::decode(pi_str.trim_start_matches("0x"))
                .expect("Invalid public input hex string");
            assert_eq!(b.len(), 32, "Each public input scalar must be 32 bytes");
            let mut arr = [0u8; 32];
            arr.copy_from_slice(&b);
            from_evm_scalar(&arr).expect("Non-canonical public input scalar (< r)")
        })
        .collect();

    let is_valid = match circuit {
        "joinsplit" => {
            let keys = nimbus_core::get_or_init_joinsplit_circuit_keys();
            nimbus_core::verify_joinsplit_proof(&keys.verifying_key, &proof, &parsed_pis)
                .unwrap_or(false)
        }
        "private_note" => {
            let keys = nimbus_core::get_or_init_note_circuit_keys();
            nimbus_core::verify_note_proof(&proof, &keys.verifying_key, &parsed_pis)
        }
        _ => {
            eprintln!(
                "Unknown circuit type: {}. Use 'joinsplit' or 'private_note'",
                circuit
            );
            false
        }
    };

    println!("Public Inputs Count: {}", parsed_pis.len());
    println!(
        "Verification Result: {}",
        if is_valid {
            "VALID (Proof verified successfully!)"
        } else {
            "INVALID (Verification failed!)"
        }
    );
    println!("------------------------------------------------------------");
    is_valid
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::thread_rng;

    #[test]
    fn test_generate_wallet() {
        let mut rng = thread_rng();
        generate_wallet(None, &mut rng);
    }

    #[test]
    fn test_create_deposit_note() {
        let mut rng = thread_rng();
        create_deposit_note(5_000_000, None, &mut rng);
    }

    #[test]
    fn test_inspect_mmr() {
        let cm1 = "0x0000000000000000000000000000000000000000000000000000000000000001";
        let cm2 = "0x0000000000000000000000000000000000000000000000000000000000000002";
        let list = format!("{},{}", cm1, cm2);
        inspect_mmr(Some(&list), None);
    }
}
