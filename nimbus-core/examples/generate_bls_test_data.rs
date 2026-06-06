use ark_bls12_381::{Fr, G1Projective, G2Projective};
use ark_ec::{CurveGroup, PrimeGroup};
use ark_ff::{PrimeField, UniformRand};
use ark_serialize::CanonicalSerialize;
use rand::Rng;
use sha2::{Digest, Sha256};

/// Helper to serialize a G1 point to EVM Big-Endian format (128 bytes)
/// EVM G1 point: X (64 bytes, big-endian), Y (64 bytes, big-endian)
/// Arkworks G1Affine uncompressed: X (48 bytes, little-endian), Y (48 bytes, little-endian)
fn g1_to_evm_bytes(point: &G1Projective) -> Vec<u8> {
    let affine = point.into_affine();
    let mut buf = vec![];
    affine.serialize_uncompressed(&mut buf).unwrap();
    let mut evm_buf = vec![0u8; 128];
    // Each coordinate has a 64-byte block in EVM, padded with 16 leading zeros
    for i in 0..2 {
        for j in 0..48 {
            evm_buf[i * 64 + 16 + j] = buf[i * 48 + (47 - j)];
        }
    }
    evm_buf
}

/// Helper to serialize a G2 point to EVM Big-Endian format (256 bytes)
/// EVM G2 point: X1, X0, Y1, Y0 (each 64 bytes, big-endian)
/// Arkworks G2Affine uncompressed: X0, X1, Y0, Y1 (each 48 bytes, little-endian)
fn g2_to_evm_bytes(point: &G2Projective) -> Vec<u8> {
    let affine = point.into_affine();
    let mut buf = vec![];
    affine.serialize_uncompressed(&mut buf).unwrap();
    let mut evm_buf = vec![0u8; 256];
    
    // EVM blocks: block 0 is X1, block 1 is X0, block 2 is Y1, block 3 is Y0
    // Arkworks order of elements: coeff 0 is X0, coeff 1 is X1, coeff 2 is Y0, coeff 3 is Y1
    let src_indices = [1, 0, 3, 2];
    for i in 0..4 {
        let src_idx = src_indices[i];
        for j in 0..48 {
            evm_buf[i * 64 + 16 + j] = buf[src_idx * 48 + (47 - j)];
        }
    }
    evm_buf
}

fn main() {
    let mut rng = rand::thread_rng();

    // Generate a random message
    let message: [u8; 32] = rng.gen();
    
    // Hash message to G1 (H(m))
    let mut hasher = Sha256::new();
    hasher.update(&message);
    let hash = hasher.finalize();
    let scalar = Fr::from_le_bytes_mod_order(&hash);
    let hm = G1Projective::generator() * scalar;

    // Generate a random secret key
    let sk = Fr::rand(&mut rng);
    
    // Derive public key (pk = sk * G2)
    let pk = G2Projective::generator() * sk;

    // Sign the message (alpha = sk * H(m))
    let alpha = hm * sk;

    // Convert to EVM format
    let hm_bytes = g1_to_evm_bytes(&hm);
    let pk_bytes = g2_to_evm_bytes(&pk);
    let alpha_bytes = g1_to_evm_bytes(&alpha);

    // Calculate nullifier (hash of message)
    let nullifier = hash;

    println!("=== BLS Signature Test Data ===");
    println!("\nNullifier (bytes32):");
    println!("nullifier_hex: 0x{}", hex::encode(nullifier));
    
    println!("\n-alpha (G1 point, 128 bytes):");
    println!("alpha_neg_hex: 0x{}", hex::encode(&alpha_bytes));
    
    println!("\nH(m) (G1 point, 128 bytes):");
    println!("hm_hex: 0x{}", hex::encode(&hm_bytes));
    
    println!("\npk_iss (G2 point, 256 bytes):");
    println!("pk_iss_hex: 0x{}", hex::encode(&pk_bytes));
    
    println!("\n=== Curl Command ===");
    println!("curl -X POST http://127.0.0.1:8080/api/spend -H \"Content-Type: application/json\" -d '{{");
    println!("\"nullifier\":\"0x{}\",", hex::encode(nullifier));
    println!("\"sig_hex\":\"0xtest\",");
    println!("\"recipient\":\"0x742d35Cc6634C0532925a3b844Bc9e7595f0bEbb\",");
    println!("\"amount\":1000000,");
    println!("\"alpha_neg_hex\":\"0x{}\",", hex::encode(&alpha_bytes));
    println!("\"hm_hex\":\"0x{}\",", hex::encode(&hm_bytes));
    println!("\"pk_iss_hex\":\"0x{}\"", hex::encode(&pk_bytes));
    println!("}}'");
}
