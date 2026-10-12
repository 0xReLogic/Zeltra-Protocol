//! ZK-UTXO CLI commands (DEC-038A)

use nimbus_core::note::{
    derive_nullifier_key, derive_spending_key, MerkleMountainRange, PrivateNoteV1,
};
use nimbus_core::{from_evm_scalar, to_evm_scalar, Fr, UniformRand};
use rand::Rng;

/// Generate a new random spending secret and viewing key for ZK-UTXO wallet
pub fn generate_wallet<R: Rng>(rng: &mut R) {
    let mut seed = [0u8; 32];
    rng.fill(&mut seed);
    let spending_key = derive_spending_key(&seed);
    let nullifier_key = derive_nullifier_key(spending_key);

    let spending_key_bytes = to_evm_scalar(&spending_key);
    let nullifier_key_bytes = to_evm_scalar(&nullifier_key);

    println!("NEW ZK-UTXO WALLET GENERATED (DEC-038A)");
    println!("------------------------------------------------------------");
    println!("Master Seed (32 bytes)      :\n0x{}", hex::encode(seed));
    println!(
        "\nSpending Key (Master Secret) :\n0x{}",
        hex::encode(spending_key_bytes)
    );
    println!(
        "\nNullifier Secret Key        :\n0x{}",
        hex::encode(nullifier_key_bytes)
    );
    println!("------------------------------------------------------------");
}

/// Create a private note and note commitment for direct deposit (DEC-038B)
pub fn create_deposit_note<R: Rng>(amount: u64, rng: &mut R) {
    let mut seed = [0u8; 32];
    rng.fill(&mut seed);
    let spending_key = derive_spending_key(&seed);
    let rho = Fr::rand(rng);
    let randomness = Fr::rand(rng);

    let note = PrivateNoteV1::new(amount, spending_key, rho, randomness);
    let commitment = note.commitment();
    let cm_bytes = to_evm_scalar(&commitment);
    let rho_bytes = to_evm_scalar(&note.rho);
    let r_bytes = to_evm_scalar(&note.randomness);

    println!("NEW DIRECT DEPOSIT NOTE GENERATED (DEC-038B)");
    println!("------------------------------------------------------------");
    println!("Amount (micro-USDC)         : {}", amount);
    println!("Note Commitment (cm)        :\n0x{}", hex::encode(cm_bytes));
    println!(
        "\nNote Nullifier Seed (rho)   :\n0x{}",
        hex::encode(rho_bytes)
    );
    println!(
        "\nCommitment Trapdoor (r)     :\n0x{}",
        hex::encode(r_bytes)
    );
    println!("------------------------------------------------------------");
}

/// Inspect Merkle Mountain Range peaks and root for a set of note commitments
pub fn inspect_mmr(commitments_hex: &str) {
    let hex_items: Vec<&str> = commitments_hex.split(',').map(|s| s.trim()).collect();
    println!("MERKLE MOUNTAIN RANGE INSPECTOR (DEC-032 / DEC-038A)");
    println!("------------------------------------------------------------");
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
    println!("Final MMR Bagged Root:\n0x{}", hex::encode(root_bytes));
    println!("Active MMR Peaks: {}", mmr.get_peaks().len());
    println!("------------------------------------------------------------");
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::thread_rng;

    #[test]
    fn test_generate_wallet() {
        let mut rng = thread_rng();
        generate_wallet(&mut rng);
    }

    #[test]
    fn test_create_deposit_note() {
        let mut rng = thread_rng();
        create_deposit_note(5_000_000, &mut rng);
    }

    #[test]
    fn test_inspect_mmr() {
        let cm1 = "0x0000000000000000000000000000000000000000000000000000000000000001";
        let cm2 = "0x0000000000000000000000000000000000000000000000000000000000000002";
        let list = format!("{},{}", cm1, cm2);
        inspect_mmr(&list);
    }
}
