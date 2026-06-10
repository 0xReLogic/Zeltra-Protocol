use nimbus_core::{
    client_blind, client_unmask, get_alpha_neg_evm, get_hm_evm, get_pk_iss_evm,
    issuer_sign_blinded, verify_unmasked, IssuerSecretKey, to_evm_g2,
};
use rand::{rngs::StdRng, SeedableRng};
use sha3::{Digest, Keccak256};
use std::collections::HashMap;
use ark_ec::CurveGroup;

fn to_evm_scalar(scalar: &ark_bls12_381::Fr) -> [u8; 32] {
    let mut buf = vec![];
    ark_serialize::CanonicalSerialize::serialize_uncompressed(scalar, &mut buf).unwrap();
    let mut evm_buf = [0u8; 32];
    for j in 0..32 {
        evm_buf[j] = buf[31 - j];
    }
    evm_buf
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let invalid = args.iter().any(|arg| arg == "--invalid");
    let structured = args.iter().any(|arg| arg == "--spend-contract");
    let opts = parse_opts(&args);
    let amount = parse_u64_opt(&opts, "amount").unwrap_or(5_000_000u64);
    let message = if structured {
        compute_contract_spend_hash(&opts, amount)
    } else {
        b"nimbus-bls-known-answer-vector-v1".to_vec()
    };
    let mut rng = StdRng::from_seed([0x42; 32]);

    let sk_iss = IssuerSecretKey::generate(&mut rng);
    let pk_iss = sk_iss.public_key();
    let (blinded, blinding_factor) = client_blind(&message, &mut rng);
    let (masked_sig, masking_key, com_k) = issuer_sign_blinded(&sk_iss, &blinded, &mut rng);
    let signature = client_unmask(&masked_sig, &blinding_factor, &masking_key)
        .expect("deterministic masking factors must be invertible");

    assert!(verify_unmasked(&message, &signature, &pk_iss));

    let mut alpha_neg = get_alpha_neg_evm(&signature);
    let hm = get_hm_evm(&message);
    let pk_iss_evm = get_pk_iss_evm(&pk_iss);
    let nullifier = Keccak256::digest(&hm);
    let k_evm = to_evm_scalar(&masking_key.0);
    let com_k_evm = to_evm_g2(&com_k.0.into_affine());

    if invalid {
        alpha_neg[127] ^= 0x01;
    }

    println!(
        "mode: {}",
        if invalid {
            "invalid-alpha"
        } else if structured {
            "contract-spend"
        } else {
            "valid"
        }
    );
    println!("message_hex: 0x{}", hex::encode(&message));
    println!("amount: {}", amount);
    if structured {
        let intent_hash = opts
            .get("intent-hash")
            .map(|value| decode_fixed_hex(value, 32, "intent-hash"))
            .unwrap_or_else(|| recipient_intent_hash(required_opt(&opts, "recipient")));
        println!("chain_id: {}", required_opt(&opts, "chain-id"));
        println!("contract: {}", required_opt(&opts, "contract"));
        println!(
            "recipient: {}",
            opts.get("recipient")
                .map(String::as_str)
                .unwrap_or("0x0000000000000000000000000000000000000000")
        );
        println!(
            "recipient_or_intent_hash_hex: 0x{}",
            hex::encode(intent_hash)
        );
        println!("expiry: {}", parse_u64_opt(&opts, "expiry").unwrap_or(0));
        println!("nonce_hex: {}", required_opt(&opts, "nonce"));
    }
    println!("nullifier_hex: 0x{}", hex::encode(nullifier));
    println!("alpha_neg_hex: 0x{}", hex::encode(alpha_neg));
    println!("hm_hex: 0x{}", hex::encode(hm));
    println!("pk_iss_hex: 0x{}", hex::encode(pk_iss_evm));
    println!("k_hex: 0x{}", hex::encode(k_evm));
    println!("com_k_hex: 0x{}", hex::encode(com_k_evm));
}

fn parse_opts(args: &[String]) -> HashMap<String, String> {
    let mut opts = HashMap::new();
    let mut i = 1;
    while i < args.len() {
        if let Some(key) = args[i].strip_prefix("--") {
            if key != "invalid" && key != "spend-contract" {
                let value = args
                    .get(i + 1)
                    .unwrap_or_else(|| panic!("missing value for --{}", key));
                opts.insert(key.to_string(), value.to_string());
                i += 2;
                continue;
            }
        }
        i += 1;
    }
    opts
}

fn required_opt<'a>(opts: &'a HashMap<String, String>, key: &str) -> &'a str {
    opts.get(key)
        .map(String::as_str)
        .unwrap_or_else(|| panic!("missing required --{}", key))
}

fn parse_u64_opt(opts: &HashMap<String, String>, key: &str) -> Option<u64> {
    opts.get(key).map(|value| {
        value
            .parse::<u64>()
            .unwrap_or_else(|_| panic!("invalid --{}", key))
    })
}

fn compute_contract_spend_hash(opts: &HashMap<String, String>, amount: u64) -> Vec<u8> {
    let chain_id = parse_u64_opt(opts, "chain-id").expect("missing required --chain-id");
    let contract = decode_fixed_hex(required_opt(opts, "contract"), 20, "contract");
    let intent_hash = opts
        .get("intent-hash")
        .map(|value| decode_fixed_hex(value, 32, "intent-hash"))
        .unwrap_or_else(|| recipient_intent_hash(required_opt(opts, "recipient")));
    let expiry = parse_u64_opt(opts, "expiry").unwrap_or(0);
    let nonce = decode_fixed_hex(required_opt(opts, "nonce"), 32, "nonce");

    let mut msg = Vec::with_capacity(5 + 32 + 20 + 32 + 32 + 32 + 32);
    msg.extend_from_slice(b"SPEND");
    msg.extend_from_slice(&u64_to_be_32(chain_id));
    msg.extend_from_slice(&contract);
    msg.extend_from_slice(&u64_to_be_32(amount));
    msg.extend_from_slice(&intent_hash);
    msg.extend_from_slice(&u64_to_be_32(expiry));
    msg.extend_from_slice(&nonce);
    Keccak256::digest(&msg).to_vec()
}

fn u64_to_be_32(value: u64) -> [u8; 32] {
    let mut out = [0u8; 32];
    out[24..].copy_from_slice(&value.to_be_bytes());
    out
}

fn decode_fixed_hex(value: &str, len: usize, label: &str) -> Vec<u8> {
    let trimmed = value.strip_prefix("0x").unwrap_or(value);
    let bytes = hex::decode(trimmed).unwrap_or_else(|_| panic!("invalid hex for --{}", label));
    if bytes.len() != len {
        panic!(
            "invalid --{} length: expected {} bytes, got {}",
            label,
            len,
            bytes.len()
        );
    }
    bytes
}

fn recipient_intent_hash(recipient: &str) -> Vec<u8> {
    let address = decode_fixed_hex(recipient, 20, "recipient");
    let mut out = vec![0u8; 32];
    out[12..].copy_from_slice(&address);
    out
}
