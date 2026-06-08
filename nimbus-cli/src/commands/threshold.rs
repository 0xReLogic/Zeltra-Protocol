//! Threshold cryptography commands

use nimbus_core::*;

pub fn aggregate(indices: String, signatures: String) {
    let idx_list: Vec<usize> = indices
        .split(',')
        .map(|s| s.trim().parse::<usize>().expect("Invalid index number"))
        .collect();
    let sig_list: Vec<String> = signatures
        .split(',')
        .map(|s| s.trim().to_string())
        .collect();

    if idx_list.len() != sig_list.len() {
        panic!("Indices and signatures count mismatch");
    }

    let mut partial_sigs = vec![];
    for i in 0..idx_list.len() {
        let sig_bytes = hex::decode(&sig_list[i]).expect("Invalid signature hex");
        let sig: PartialBlindSignature =
            deserialize_from_bytes(&sig_bytes).expect("Failed to deserialize partial signature");
        partial_sigs.push((idx_list[i], sig));
    }

    let aggregated = aggregate_shares(&partial_sigs).expect("Failed to aggregate signatures");
    let aggregated_hex = hex::encode(serialize_to_bytes(&aggregated));

    println!("AGGREGATED MASKED SIGNATURE");
    println!("------------------------------------------------------------");
    println!("Masked Signature (sigma_tilde) :\n{}", aggregated_hex);
    println!("------------------------------------------------------------");
}
