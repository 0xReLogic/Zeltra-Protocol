use clap::{Parser, Subcommand};
use nimbus_core::*;
use rand::thread_rng;

#[derive(Parser)]
#[command(name = "nimbus-cli")]
#[command(about = "Nimbus Protocol CLI Wallet & Cryptography Utility", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Generate a new random Issuer keypair (sk and pk)
    GenerateKeys,
    
    /// Split a secret key into n shares with threshold t using Shamir Secret Sharing
    SplitKey {
        /// Secret key in hex
        #[arg(short, long)]
        sk: String,
        
        /// Threshold t
        #[arg(short, long, default_value_t = 3)]
        threshold: usize,
        
        /// Total shares n
        #[arg(short = 'n', long, default_value_t = 5)]
        total: usize,
    },
    
    /// Aggregate partial signatures from a threshold of guardians
    Aggregate {
        /// Comma-separated list of guardian indices (e.g. "1,2,3")
        #[arg(short, long)]
        indices: String,
        
        /// Comma-separated list of partial signatures in hex
        #[arg(short, long)]
        signatures: String,
    },
    
    /// Blind a message (e.g. ephemeral public key) using a random blinding factor
    Blind {
        /// The message to blind (e.g. "ephemeral_public_key_abc123")
        #[arg(short, long)]
        message: String,
    },
    
    /// Sign a blinded message (masked signing) using the issuer secret key
    Sign {
        /// Blinded message in hex
        #[arg(short, long)]
        blinded: String,
        
        /// Issuer secret key in hex
        #[arg(short, long)]
        sk: String,
    },
    
    /// Verify a masked signature (client check)
    VerifyMasked {
        /// Blinded message in hex
        #[arg(short, long)]
        blinded: String,
        
        /// Masking key commitment in hex
        #[arg(short, long)]
        com_k: String,
        
        /// Masked signature in hex
        #[arg(short, long)]
        masked_sig: String,
    },
    
    /// Unmask a masked signature once masking key is revealed on-chain
    Unmask {
        /// Masked signature in hex
        #[arg(short, long)]
        masked_sig: String,
        
        /// Blinding factor in hex
        #[arg(short, long)]
        r: String,
        
        /// Masking key in hex
        #[arg(short, long)]
        k: String,
    },
    
    /// Verify the final unblinded signature
    Verify {
        /// The original message string
        #[arg(short, long)]
        message: String,
        
        /// Unmasked signature in hex
        #[arg(short, long)]
        sig: String,
        
        /// Issuer public key in hex
        #[arg(short, long)]
        pk: String,
    },


}

fn main() {
    let cli = Cli::parse();
    let mut rng = thread_rng();

    match cli.command {
        Commands::GenerateKeys => {
            let sk = IssuerSecretKey::generate(&mut rng);
            let pk = sk.public_key();
            
            let sk_hex = hex::encode(serialize_to_bytes(&sk));
            let pk_hex = hex::encode(serialize_to_bytes(&pk));
            
            println!("NEW KEYPAIR GENERATED");
            println!("------------------------------------------------------------");
            println!("Secret Key (sk_iss) :\n{}", sk_hex);
            println!("\nPublic Key (pk_iss) :\n{}", pk_hex);
            println!("------------------------------------------------------------");
        }
        
        Commands::Aggregate { indices, signatures } => {
            let idx_list: Vec<usize> = indices.split(',')
                .map(|s| s.trim().parse::<usize>().expect("Invalid index number"))
                .collect();
            let sig_list: Vec<String> = signatures.split(',')
                .map(|s| s.trim().to_string())
                .collect();
                
            if idx_list.len() != sig_list.len() {
                panic!("Indices and signatures count mismatch");
            }
            
            let mut partial_sigs = vec![];
            for i in 0..idx_list.len() {
                let sig_bytes = hex::decode(&sig_list[i]).expect("Invalid signature hex");
                let sig: PartialBlindSignature = deserialize_from_bytes(&sig_bytes)
                    .expect("Failed to deserialize partial signature");
                partial_sigs.push((idx_list[i], sig));
            }
            
            let aggregated = aggregate_shares(&partial_sigs);
            let aggregated_hex = hex::encode(serialize_to_bytes(&aggregated));
            
            println!("AGGREGATED MASKED SIGNATURE");
            println!("------------------------------------------------------------");
            println!("Masked Signature (sigma_tilde) :\n{}", aggregated_hex);
            println!("------------------------------------------------------------");
        }
        
        Commands::SplitKey { sk, threshold, total } => {
            let sk_bytes = hex::decode(sk).expect("Invalid secret key hex");
            let sk_iss: IssuerSecretKey = deserialize_from_bytes(&sk_bytes)
                .expect("Failed to deserialize secret key");
            let shares = split_secret_key(&sk_iss, threshold, total, &mut rng);
            
            println!("SECRET KEY SPLIT: threshold={}, total={}", threshold, total);
            println!("------------------------------------------------------------");
            for (i, share) in shares.iter().enumerate() {
                let share_hex = hex::encode(serialize_to_bytes(share));
                println!("Share {} (Index {}):\n{}", i + 1, i + 1, share_hex);
            }
            println!("------------------------------------------------------------");
        }
        
        Commands::Blind { message } => {
            let (blinded_msg, r) = client_blind(message.as_bytes(), &mut rng);
            
            let blinded_hex = hex::encode(serialize_to_bytes(&blinded_msg));
            let r_hex = hex::encode(serialize_to_bytes(&r));
            
            println!("MESSAGE BLINDED");
            println!("------------------------------------------------------------");
            println!("Blinded Message (X) :\n{}", blinded_hex);
            println!("\nBlinding Factor (r) :\n{}", r_hex);
            println!("------------------------------------------------------------");
        }
        
        Commands::Sign { blinded, sk } => {
            let blinded_bytes = hex::decode(blinded).expect("Invalid blinded message hex");
            let sk_bytes = hex::decode(sk).expect("Invalid secret key hex");
            
            let blinded_msg: BlindedMessage = deserialize_from_bytes(&blinded_bytes)
                .expect("Failed to deserialize blinded message");
            let sk_iss: IssuerSecretKey = deserialize_from_bytes(&sk_bytes)
                .expect("Failed to deserialize secret key");
                
            let (masked_sig, k, com_k) = issuer_sign_blinded(&sk_iss, &blinded_msg, &mut rng);
            
            let masked_sig_hex = hex::encode(serialize_to_bytes(&masked_sig));
            let k_hex = hex::encode(serialize_to_bytes(&k));
            let com_k_hex = hex::encode(serialize_to_bytes(&com_k));
            
            println!("MASKED SIGNATURE GENERATED BY ISSUER");
            println!("------------------------------------------------------------");
            println!("Masked Signature (sigma_tilde) :\n{}", masked_sig_hex);
            println!("\nMasking Key (k)               :\n{}", k_hex);
            println!("\nCommitment (com_k)            :\n{}", com_k_hex);
            println!("------------------------------------------------------------");
        }
        
        Commands::VerifyMasked { blinded, com_k, masked_sig } => {
            let blinded_bytes = hex::decode(blinded).expect("Invalid blinded message hex");
            let com_k_bytes = hex::decode(com_k).expect("Invalid commitment hex");
            let masked_sig_bytes = hex::decode(masked_sig).expect("Invalid masked signature hex");
            
            let x: BlindedMessage = deserialize_from_bytes(&blinded_bytes)
                .expect("Failed to deserialize blinded message");
            let commitment: MaskingKeyCommitment = deserialize_from_bytes(&com_k_bytes)
                .expect("Failed to deserialize commitment");
            let sig: MaskedBlindSignature = deserialize_from_bytes(&masked_sig_bytes)
                .expect("Failed to deserialize masked signature");
                
            let is_valid = client_verify_masked(&x, &commitment, &sig);
            
            println!("VERIFY MASKED SIGNATURE (OFF-CHAIN)");
            println!("------------------------------------------------------------");
            if is_valid {
                println!("VALID: Masked signature matches blinded message and commitment!");
            } else {
                println!("INVALID: Signature verification failed!");
            }
            println!("------------------------------------------------------------");
        }
        
        Commands::Unmask { masked_sig, r, k } => {
            let sig_bytes = hex::decode(masked_sig).expect("Invalid masked signature hex");
            let r_bytes = hex::decode(r).expect("Invalid blinding factor hex");
            let k_bytes = hex::decode(k).expect("Invalid masking key hex");
            
            let sig: MaskedBlindSignature = deserialize_from_bytes(&sig_bytes)
                .expect("Failed to deserialize masked signature");
            let r_factor: BlindingFactor = deserialize_from_bytes(&r_bytes)
                .expect("Failed to deserialize blinding factor");
            let k_key: MaskingKey = deserialize_from_bytes(&k_bytes)
                .expect("Failed to deserialize masking key");
                
            let unmasked_sig = client_unmask(&sig, &r_factor, &k_key)
                .expect("Failed to compute unmask key inverse");
                
            let unmasked_hex = hex::encode(serialize_to_bytes(&unmasked_sig));
            
            println!("SIGNATURE UNMASKED SUCCESSFULLY");
            println!("------------------------------------------------------------");
            println!("Unmasked Signature (alpha) :\n{}", unmasked_hex);
            println!("------------------------------------------------------------");
        }
        
        Commands::Verify { message, sig, pk } => {
            let sig_bytes = hex::decode(sig).expect("Invalid signature hex");
            let pk_bytes = hex::decode(pk).expect("Invalid public key hex");
            
            let signature: UnmaskedSignature = deserialize_from_bytes(&sig_bytes)
                .expect("Failed to deserialize signature");
            let pk_iss: IssuerPublicKey = deserialize_from_bytes(&pk_bytes)
                .expect("Failed to deserialize public key");
                
            let is_valid = verify_unmasked(message.as_bytes(), &signature, &pk_iss);
            
            println!("VERIFY FINAL UNBLINDED SIGNATURE");
            println!("------------------------------------------------------------");
            if is_valid {
                println!("VALID: The signature is verified and authentic under the Issuer's Public Key!");
            } else {
                println!("INVALID: Signature verification failed!");
            }
            println!("------------------------------------------------------------");
        }
        

    }
}
