//! Key generation and splitting commands

use nimbus_core::*;
use rand::Rng;

pub fn generate_keys<R: Rng>(rng: &mut R) {
    let sk = IssuerSecretKey::generate(rng);
    let pk = sk.public_key();
    
    let sk_hex = hex::encode(serialize_to_bytes(&sk));
    let pk_hex = hex::encode(serialize_to_bytes(&pk));
    
    println!("NEW KEYPAIR GENERATED");
    println!("------------------------------------------------------------");
    println!("Secret Key (sk_iss) :\n{}", sk_hex);
    println!("\nPublic Key (pk_iss) :\n{}", pk_hex);
    println!("------------------------------------------------------------");
}

pub fn split_key<R: Rng>(sk: String, threshold: usize, total: usize, rng: &mut R) {
    let sk_bytes = hex::decode(sk).expect("Invalid secret key hex");
    let sk_iss: IssuerSecretKey = deserialize_from_bytes(&sk_bytes)
        .expect("Failed to deserialize secret key");
    let shares = split_secret_key(&sk_iss, threshold, total, rng);
    
    println!("SECRET KEY SPLIT: threshold={}, total={}", threshold, total);
    println!("------------------------------------------------------------");
    for (i, share) in shares.iter().enumerate() {
        let share_hex = hex::encode(serialize_to_bytes(share));
        let public_share_hex =
            hex::encode(serialize_to_bytes(&public_key_for_share(&share.1)));
        println!("Share {} (Index {}):\n{}", i + 1, i + 1, share_hex);
        println!("Public Share {}:\n{}", i + 1, public_share_hex);
    }
    println!("------------------------------------------------------------");
}
