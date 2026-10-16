//! ZSA Interoperability & Cryptographic Test Harness (DEC-043A).
//!
//! Provides CLI interface for cross-language validation and testing:
//! 1. ZSA derivation and parsing (Hex & Bech32m)
//! 2. Deterministic ECDH memo encryption
//! 3. Deterministic ECDH memo decryption

use std::env;
use zeltra_core::{
    decrypt_note_memo_deterministic, derive_master_secret, derive_spending_key,
    derive_viewing_keys_from_secret, encrypt_note_memo_deterministic, fr_from_be_bytes,
    fr_to_be_bytes, note_commitment, IncomingViewingKey, ZeltraShieldedAddress,
};

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: zsa_interop <command> [args...]");
        std::process::exit(1);
    }

    let command = &args[1];
    match command.as_str() {
        "zsa_from_seed" => {
            if args.len() < 3 {
                eprintln!("Usage: zsa_interop zsa_from_seed <seed_hex_or_string>");
                std::process::exit(1);
            }
            let seed_bytes = hex::decode(&args[2]).unwrap_or_else(|_| args[2].as_bytes().to_vec());
            let master = derive_master_secret(&seed_bytes);
            let sk = derive_spending_key(&master);
            let (ivk, _ovk) = derive_viewing_keys_from_secret(&master);
            let zsa = ZeltraShieldedAddress::from_keys(sk, ivk.public_key());

            println!(
                r#"{{"spending_key":"{}","owner_key":"{}","pk_ivk":"{}","ivk_secret":"{}","zsa_hex":"{}","zsa_bech32m":"{}"}}"#,
                hex::encode(fr_to_be_bytes(&sk)),
                hex::encode(fr_to_be_bytes(&zsa.owner_key)),
                hex::encode(zsa.incoming_viewing_public_key),
                hex::encode(ivk.secret_bytes()),
                zsa.to_hex(),
                zsa.to_bech32m().expect("bech32m encode")
            );
        }
        "parse_zsa" => {
            if args.len() < 3 {
                eprintln!("Usage: zsa_interop parse_zsa <zsa_str>");
                std::process::exit(1);
            }
            let zsa_str = &args[2];
            let zsa = if zsa_str.starts_with("zs") {
                ZeltraShieldedAddress::from_bech32m(zsa_str)
            } else {
                ZeltraShieldedAddress::from_hex(zsa_str)
            };

            match zsa {
                Ok(z) => {
                    println!(
                        r#"{{"valid":true,"owner_key":"{}","pk_ivk":"{}","zsa_hex":"{}","zsa_bech32m":"{}"}}"#,
                        hex::encode(fr_to_be_bytes(&z.owner_key)),
                        hex::encode(z.incoming_viewing_public_key),
                        z.to_hex(),
                        z.to_bech32m().unwrap()
                    );
                }
                Err(e) => {
                    println!(r#"{{"valid":false,"error":"{}"}}"#, e);
                }
            }
        }
        "encrypt_memo" => {
            if args.len() < 6 {
                eprintln!("Usage: zsa_interop encrypt_memo <value> <rho_hex> <asset_id> <zsa_str>");
                std::process::exit(1);
            }
            let value: u64 = args[2].parse().expect("valid value u64");
            let rho_bytes = hex::decode(&args[3]).expect("valid rho hex");
            let mut rho_arr = [0u8; 32];
            rho_arr.copy_from_slice(&rho_bytes);
            let rho = fr_from_be_bytes(&rho_arr).expect("canonical rho");
            let asset_id: u64 = args[4].parse().expect("valid asset_id u64");

            let zsa_str = &args[5];
            let zsa: ZeltraShieldedAddress = zsa_str.parse().expect("valid ZSA");

            let (encrypted_memo, randomness) =
                encrypt_note_memo_deterministic(value, rho, asset_id, &zsa)
                    .expect("deterministic encryption");

            let commitment = note_commitment(value, asset_id as u8, zsa.owner_key, rho, randomness);

            println!(
                r#"{{"encrypted_memo":"{}","randomness":"{}","commitment":"{}"}}"#,
                hex::encode(&encrypted_memo),
                hex::encode(fr_to_be_bytes(&randomness)),
                hex::encode(fr_to_be_bytes(&commitment))
            );
        }
        "decrypt_memo" => {
            if args.len() < 4 {
                eprintln!("Usage: zsa_interop decrypt_memo <memo_hex> <ivk_secret_hex>");
                std::process::exit(1);
            }
            let memo_bytes = hex::decode(&args[2]).expect("valid memo hex");
            let ivk_secret_bytes = hex::decode(&args[3]).expect("valid ivk secret hex");
            let mut ivk_arr = [0u8; 32];
            ivk_arr.copy_from_slice(&ivk_secret_bytes);
            let ivk = IncomingViewingKey::from_bytes(&ivk_arr);

            match decrypt_note_memo_deterministic(&memo_bytes, &ivk) {
                Some((pt, randomness)) => {
                    println!(
                        r#"{{"valid":true,"value":{},"rho":"{}","asset_id":{},"randomness":"{}"}}"#,
                        pt.value,
                        hex::encode(fr_to_be_bytes(&pt.rho)),
                        pt.asset_id,
                        hex::encode(fr_to_be_bytes(&randomness))
                    );
                }
                None => {
                    println!(r#"{{"valid":false,"error":"Decryption failed"}}"#);
                }
            }
        }
        _ => {
            eprintln!("Unknown command: {}", command);
            std::process::exit(1);
        }
    }
}
