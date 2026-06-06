use nimbus_core::{
    client_blind, client_unmask, get_alpha_neg_evm, get_hm_evm, get_pk_iss_evm,
    issuer_sign_blinded, verify_unmasked, IssuerSecretKey,
};
use rand::{rngs::StdRng, SeedableRng};
use sha3::{Digest, Keccak256};

fn main() {
    let invalid = std::env::args().any(|arg| arg == "--invalid");
    let message = b"nimbus-bls-known-answer-vector-v1";
    let amount = 5_000_000u64;
    let mut rng = StdRng::from_seed([0x42; 32]);

    let sk_iss = IssuerSecretKey::generate(&mut rng);
    let pk_iss = sk_iss.public_key();
    let (blinded, blinding_factor) = client_blind(message, &mut rng);
    let (masked_sig, masking_key, _) =
        issuer_sign_blinded(&sk_iss, &blinded, &mut rng);
    let signature = client_unmask(&masked_sig, &blinding_factor, &masking_key)
        .expect("deterministic masking factors must be invertible");

    assert!(verify_unmasked(message, &signature, &pk_iss));

    let mut alpha_neg = get_alpha_neg_evm(&signature);
    let hm = get_hm_evm(message);
    let pk_iss_evm = get_pk_iss_evm(&pk_iss);
    let nullifier = Keccak256::digest(&hm);

    if invalid {
        alpha_neg[127] ^= 0x01;
    }

    println!("mode: {}", if invalid { "invalid-alpha" } else { "valid" });
    println!("message_hex: 0x{}", hex::encode(message));
    println!("amount: {}", amount);
    println!("nullifier_hex: 0x{}", hex::encode(nullifier));
    println!("alpha_neg_hex: 0x{}", hex::encode(alpha_neg));
    println!("hm_hex: 0x{}", hex::encode(hm));
    println!("pk_iss_hex: 0x{}", hex::encode(pk_iss_evm));
}
