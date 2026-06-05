//! Key Management System integration for loading BLS share keys

use serde::Deserialize;
use crate::http;

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

    println!("WARNING: No share key found via Vault or environment variables. Using default insecure key.");
    (nimbus_core::Fr::from(12345u64), env_index)
}
