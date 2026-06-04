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

    /// Generate a random offline identity parameter (scalar I) or slope (scalar a)
    GenerateOfflineParams,

    /// Spend a koin offline by answering the merchant challenge: y = a * x + I (mod p)
    SpendOffline {
        /// Slope scalar (a) in hex
        #[arg(short, long)]
        a: String,

        /// Challenge scalar (x) in hex
        #[arg(short, long)]
        x: String,

        /// Identity scalar (I) in hex
        #[arg(short, long)]
        identity: String,
    },

    /// Reconstruct the identity of a double spender given two different challenges and responses
    Slash {
        /// Challenge 1 (x1) in hex
        #[arg(long)]
        x1: String,

        /// Response 1 (y1) in hex
        #[arg(long)]
        y1: String,

        /// Challenge 2 (x2) in hex
        #[arg(long)]
        x2: String,

        /// Response 2 (y2) in hex
        #[arg(long)]
        y2: String,
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
        
        Commands::GenerateOfflineParams => {
            let mut rng = thread_rng();
            let identity = Fr::rand(&mut rng);
            let a = Fr::rand(&mut rng);
            let x = Fr::rand(&mut rng);
            
            let identity_hex = hex::encode(serialize_to_bytes(&identity));
            let a_hex = hex::encode(serialize_to_bytes(&a));
            let x_hex = hex::encode(serialize_to_bytes(&x));
            
            println!("OFFLINE PARAMS GENERATED");
            println!("------------------------------------------------------------");
            println!("Identity Secret (I)  :\n{}", identity_hex);
            println!("\nSlope Parameter (a)  :\n{}", a_hex);
            println!("\nDefault Challenge (x):\n{}", x_hex);
            println!("------------------------------------------------------------");
        }
        
        Commands::SpendOffline { a, x, identity } => {
            let a_bytes = hex::decode(a).expect("Invalid a hex");
            let x_bytes = hex::decode(x).expect("Invalid x hex");
            let identity_bytes = hex::decode(identity).expect("Invalid identity hex");
            
            let a_scalar: Fr = deserialize_from_bytes(&a_bytes)
                .expect("Failed to deserialize a");
            let x_scalar: Fr = deserialize_from_bytes(&x_bytes)
                .expect("Failed to deserialize x");
            let identity_scalar: Fr = deserialize_from_bytes(&identity_bytes)
                .expect("Failed to deserialize identity");
                
            let y_scalar = generate_offline_response(a_scalar, x_scalar, identity_scalar);
            let y_hex = hex::encode(serialize_to_bytes(&y_scalar));
            
            println!("OFFLINE SPEND PROOF GENERATED");
            println!("------------------------------------------------------------");
            println!("Challenge (x) :\n{}", hex::encode(x_bytes));
            println!("\nResponse (y)  :\n{}", y_hex);
            println!("------------------------------------------------------------");
        }
        
        Commands::Slash { x1, y1, x2, y2 } => {
            let x1_bytes = hex::decode(x1).expect("Invalid x1 hex");
            let y1_bytes = hex::decode(y1).expect("Invalid y1 hex");
            let x2_bytes = hex::decode(x2).expect("Invalid x2 hex");
            let y2_bytes = hex::decode(y2).expect("Invalid y2 hex");
            
            let x1_scalar: Fr = deserialize_from_bytes(&x1_bytes)
                .expect("Failed to deserialize x1");
            let y1_scalar: Fr = deserialize_from_bytes(&y1_bytes)
                .expect("Failed to deserialize y1");
            let x2_scalar: Fr = deserialize_from_bytes(&x2_bytes)
                .expect("Failed to deserialize x2");
            let y2_scalar: Fr = deserialize_from_bytes(&y2_bytes)
                .expect("Failed to deserialize y2");
                
            let proof1 = OfflineSpendProof { x: x1_scalar, y: y1_scalar };
            let proof2 = OfflineSpendProof { x: x2_scalar, y: y2_scalar };
            
            match reconstruct_identity(&proof1, &proof2) {
                Some(identity) => {
                    let identity_hex = hex::encode(serialize_to_bytes(&identity));
                    println!("DOUBLE SPENDER DETECTED & SLASHED");
                    println!("------------------------------------------------------------");
                    println!("Reconstructed Identity (I) :\n{}", identity_hex);
                    println!("------------------------------------------------------------");
                }
                None => {
                    println!("SLASHING FAILED");
                    println!("------------------------------------------------------------");
                    println!("Failed to reconstruct identity. Ensure x1 != x2.");
                    println!("------------------------------------------------------------");
                }
            }
        }
    }
}
