//! Key Management System integration for loading BLS share keys

use serde::Deserialize;
use crate::http;
use std::collections::HashMap;

pub fn load_issuer_public_key(
    share_sk: &nimbus_core::Fr,
) -> nimbus_core::IssuerPublicKey {
    if let Ok(hex_str) = std::env::var("NIMBUS_ISSUER_PUBLIC_KEY") {
        let normalized = hex_str.trim_start_matches("0x");
        let bytes = hex::decode(normalized)
            .unwrap_or_else(|_| fatal_issuer_public_key("must be valid hex"));
        return nimbus_core::deserialize_from_bytes::<nimbus_core::IssuerPublicKey>(&bytes)
            .unwrap_or_else(|| {
                fatal_issuer_public_key(
                    "must be a canonical compressed Nimbus IssuerPublicKey",
                )
            });
    }

    let is_test = cfg!(test)
        || std::env::var("NIMBUS_ENV")
            .map(|value| value == "test")
            .unwrap_or(false);
    let single_signer = std::env::var("NIMBUS_THRESHOLD")
        .map(|value| value == "1")
        .unwrap_or(false);
    if is_test || single_signer {
        use nimbus_core::IssuerSecretKey;

        println!(
            "WARNING: Deriving issuer public key from the local signing key. \
             This is only valid for tests or NIMBUS_THRESHOLD=1."
        );
        return IssuerSecretKey(*share_sk).public_key();
    }

    fatal_issuer_public_key(
        "is required for distributed signing; use the public key produced by the threshold ceremony",
    )
}

fn fatal_issuer_public_key(message: &str) -> ! {
    eprintln!("CRITICAL: NIMBUS_ISSUER_PUBLIC_KEY {}", message);
    std::process::exit(1);
}

pub fn load_guardian_public_keys(
    local_share_index: u32,
) -> HashMap<u32, nimbus_core::IssuerPublicKey> {
    let threshold = std::env::var("NIMBUS_THRESHOLD")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(3);
    let raw = match std::env::var("NIMBUS_GUARDIAN_PUBLIC_KEYS") {
        Ok(raw) => raw,
        Err(_) if cfg!(test) || threshold == 1 => return HashMap::new(),
        Err(_) => {
            eprintln!(
                "CRITICAL: NIMBUS_GUARDIAN_PUBLIC_KEYS is required when \
                 NIMBUS_THRESHOLD is greater than 1"
            );
            std::process::exit(1);
        }
    };
    let encoded: HashMap<String, String> = serde_json::from_str(&raw)
        .unwrap_or_else(|_| fatal_guardian_registry("must be a JSON object"));
    let mut registry = HashMap::with_capacity(encoded.len());

    for (raw_index, public_key_hex) in encoded {
        let index = raw_index
            .parse::<u32>()
            .ok()
            .filter(|index| *index > 0)
            .unwrap_or_else(|| fatal_guardian_registry("contains an invalid share index"));
        if index == local_share_index {
            fatal_guardian_registry("must not contain the local leader share index");
        }
        let bytes = hex::decode(public_key_hex.trim_start_matches("0x"))
            .unwrap_or_else(|_| fatal_guardian_registry("contains invalid public-share hex"));
        let public_key =
            nimbus_core::deserialize_from_bytes::<nimbus_core::IssuerPublicKey>(&bytes)
                .unwrap_or_else(|| {
                    fatal_guardian_registry("contains an invalid public-share point")
                });
        if registry.insert(index, public_key).is_some() {
            fatal_guardian_registry("contains a duplicate share index");
        }
    }

    if registry.len().saturating_add(1) < threshold {
        fatal_guardian_registry("does not contain enough guardian keys for the configured threshold");
    }
    registry
}

fn fatal_guardian_registry(message: &str) -> ! {
    eprintln!("CRITICAL: NIMBUS_GUARDIAN_PUBLIC_KEYS {}", message);
    std::process::exit(1);
}

pub async fn load_share_key() -> (nimbus_core::Fr, u32) {
    let env_index = std::env::var("NIMBUS_SHARE_INDEX")
        .unwrap_or_else(|_| "1".to_string())
        .parse::<u32>()
        .unwrap_or(1);

    // 1. Check if Vault/OpenBao is configured
    if let Ok(vault_token) = std::env::var("NIMBUS_VAULT_TOKEN") {
        let vault_addr = std::env::var("NIMBUS_VAULT_ADDR")
            .unwrap_or_else(|_| "http://127.0.0.1:8200".to_string());
        let vault_path = std::env::var("NIMBUS_VAULT_PATH")
            .unwrap_or_else(|_| "v1/secret/data/nimbus".to_string());
        
        let url = format!("{}/{}", vault_addr.trim_end_matches('/'), vault_path);
        println!("KMS INTEGRATION: Fetching BLS share key from OpenBao/Vault at {}...", url);
        
        match http::get_http_with_headers(&url, &[("X-Vault-Token", &vault_token)]).await {
            Ok(body) => {
                #[derive(Deserialize)]
                struct VaultSecretData {
                    share_key: String,
                }
                #[derive(Deserialize)]
                struct VaultSecretInner {
                    data: VaultSecretData,
                }
                #[derive(Deserialize)]
                struct VaultSecretResponse {
                    data: VaultSecretInner,
                }
                
                match serde_json::from_str::<VaultSecretResponse>(&body) {
                    Ok(res) => {
                        let hex_str = res.data.data.share_key;
                        if let Ok(bytes) = hex::decode(&hex_str) {
                            if bytes.len() == 40 {
                                if let Some((idx, fr)) = nimbus_core::deserialize_from_bytes::<(usize, nimbus_core::Fr)>(&bytes) {
                                    println!("KMS INTEGRATION: Successfully loaded BLS share key (index {}) from OpenBao/Vault.", idx);
                                    return (fr, idx as u32);
                                }
                            } else if let Some(fr) = nimbus_core::deserialize_from_bytes::<nimbus_core::Fr>(&bytes) {
                                println!("KMS INTEGRATION: Successfully loaded BLS share key from OpenBao/Vault.");
                                return (fr, env_index);
                            }
                        }
                        println!("KMS INTEGRATION: Error parsing/deserializing share key bytes from Vault.");
                    }
                    Err(e) => {
                        println!("KMS INTEGRATION: Error parsing Vault response JSON: {}. Response: {}", e, body);
                    }
                }
            }
            Err(e) => {
                println!("KMS INTEGRATION: Failed to fetch secret from OpenBao/Vault: {}", e);
            }
        }
    }

    // 2. Fallback to NIMBUS_SHARE_KEY for local development / backward compatibility
    if let Ok(hex_str) = std::env::var("NIMBUS_SHARE_KEY") {
        println!("WARNING: Raw plain text 'NIMBUS_SHARE_KEY' env variable detected.");
        println!("         This is unsafe for production. Use OpenBao/Vault KMS integration instead.");
        if let Ok(bytes) = hex::decode(&hex_str) {
            if bytes.len() == 40 {
                if let Some((idx, fr)) = nimbus_core::deserialize_from_bytes::<(usize, nimbus_core::Fr)>(&bytes) {
                    return (fr, idx as u32);
                }
            } else if let Some(fr) = nimbus_core::deserialize_from_bytes::<nimbus_core::Fr>(&bytes) {
                return (fr, env_index);
            }
        }
    }

    // 3. Fallback only allowed in test mode
    if cfg!(test) || std::env::var("NIMBUS_ENV").unwrap_or_default() == "test" {
        println!("WARNING: No share key found. Using test default insecure key.");
        return (nimbus_core::Fr::from(12345u64), env_index);
    }

    println!("ERROR: No share key found via Vault or environment variables.");
    println!("CRITICAL: Cannot start without valid BLS share key.");
    println!("Please configure NIMBUS_VAULT_TOKEN or NIMBUS_SHARE_KEY environment variable.");
    std::process::exit(1);
}
