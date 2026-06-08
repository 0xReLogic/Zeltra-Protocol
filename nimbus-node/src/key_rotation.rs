//! Key rotation manager for BLS threshold signature shares
//!
//! Implements automated key rotation by periodically re-fetching
//! the share key from Vault/OpenBao and re-masking it in memory.
//!
//! Based on:
//! - Cycode Secrets Management Best Practices 2026
//! - Proactive refresh for threshold signatures (proactive-refresh)
//! - stxkxs/hsm key lifecycle management patterns

use crate::circuit_breaker::{CircuitBreaker, CircuitBreakerConfig};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

/// Key rotation configuration
#[derive(Debug, Clone)]
pub struct KeyRotationConfig {
    /// How often to check for key updates (default: 1 hour)
    pub rotation_interval: Duration,
    /// Maximum key age before forced rotation (default: 24 hours)
    pub max_key_age: Duration,
    /// Whether rotation is enabled
    pub enabled: bool,
}

impl Default for KeyRotationConfig {
    fn default() -> Self {
        Self {
            rotation_interval: Duration::from_secs(3600), // 1 hour
            max_key_age: Duration::from_secs(86400),      // 24 hours
            enabled: false,                               // Disabled by default, requires Vault
        }
    }
}

/// Versioned key wrapper to track key metadata
#[derive(Clone)]
pub struct VersionedKey {
    /// Masked part 1 of the BLS share key
    pub part1: nimbus_core::Fr,
    /// Masked part 2 of the BLS share key  
    pub part2: nimbus_core::Fr,
    /// Key version (incremented on each rotation)
    pub version: u64,
    /// When this key was loaded
    pub loaded_at: std::time::Instant,
    /// Share index in the threshold scheme
    pub share_index: u32,
}

/// Thread-safe key manager with hot-reload capability
#[derive(Clone)]
pub struct KeyManager {
    current_key: Arc<RwLock<VersionedKey>>,
    issuer_public_key: nimbus_core::IssuerPublicKey,
    config: KeyRotationConfig,
    pub vault_circuit_breaker: CircuitBreaker,
}

impl KeyManager {
    /// Create a new key manager with the initial key
    pub fn new(
        share_sk: nimbus_core::Fr,
        share_index: u32,
        issuer_public_key: nimbus_core::IssuerPublicKey,
        config: KeyRotationConfig,
    ) -> Self {
        use nimbus_core::UniformRand;

        let mut rng = rand::thread_rng();
        let part1 = nimbus_core::Fr::rand(&mut rng);
        let part2 = share_sk - part1;

        let vault_circuit_breaker = CircuitBreaker::new(CircuitBreakerConfig {
            failure_threshold: 3,
            recovery_timeout: Duration::from_secs(60),
            success_threshold: 1,
            name: "vault-kms".to_string(),
        });

        Self {
            current_key: Arc::new(RwLock::new(VersionedKey {
                part1,
                part2,
                version: 1,
                loaded_at: std::time::Instant::now(),
                share_index,
            })),
            issuer_public_key,
            config,
            vault_circuit_breaker,
        }
    }

    /// Sign a blinded message using the current key, with zeroization
    pub async fn sign_share(
        &self,
        blinded: &nimbus_core::BlindedMessage,
        k: &nimbus_core::Fr,
    ) -> nimbus_core::PartialBlindSignature {
        use nimbus_core::sign_share;

        let key = self.current_key.read().await;
        let mut sk = key.part1 + key.part2;
        let sig = sign_share(&sk, blinded, k);

        // Zeroize reconstructed key
        unsafe {
            std::ptr::write_volatile(&mut sk, nimbus_core::Fr::from(0u64));
        }

        sig
    }

    /// Get the aggregate issuer public key produced by the threshold ceremony.
    pub async fn get_issuer_public_key(&self) -> nimbus_core::IssuerPublicKey {
        self.issuer_public_key.clone()
    }

    /// Get current key version and share index
    pub async fn key_info(&self) -> (u64, u32, Duration) {
        let key = self.current_key.read().await;
        (key.version, key.share_index, key.loaded_at.elapsed())
    }

    /// Get share index
    pub async fn share_index(&self) -> u32 {
        self.current_key.read().await.share_index
    }

    /// Rotate the key by re-fetching from Vault/OpenBao
    /// Returns true if key was successfully rotated
    pub async fn rotate(&self) -> bool {
        use nimbus_core::UniformRand;

        // Wrap the Vault fetch in a circuit breaker call
        let new_key = self
            .vault_circuit_breaker
            .call(|| async {
                self.fetch_key_from_vault()
                    .await
                    .ok_or_else(|| "Failed to fetch key from Vault".to_string())
            })
            .await;

        match new_key {
            Ok((new_sk, new_index)) => {
                let (new_part1, new_part2) = {
                    let mut rng = rand::thread_rng();
                    let part1 = nimbus_core::Fr::rand(&mut rng);
                    let part2 = new_sk - part1;
                    (part1, part2)
                };

                let mut key = self.current_key.write().await;
                let old_version = key.version;

                // Zeroize old key parts before overwriting
                unsafe {
                    std::ptr::write_volatile(&mut key.part1, nimbus_core::Fr::from(0u64));
                    std::ptr::write_volatile(&mut key.part2, nimbus_core::Fr::from(0u64));
                }

                key.part1 = new_part1;
                key.part2 = new_part2;
                key.version = old_version + 1;
                key.loaded_at = std::time::Instant::now();
                key.share_index = new_index;

                println!(
                    "KEY-ROTATION: Successfully rotated key v{} -> v{} (index {})",
                    old_version, key.version, new_index
                );
                true
            }
            Err(e) => {
                eprintln!(
                    "KEY-ROTATION: Failed to fetch new key from Vault, keeping current key: {}",
                    e
                );
                false
            }
        }
    }

    /// Re-mask the current key without fetching a new one.
    /// This provides defense-in-depth by changing the split periodically.
    pub async fn re_mask(&self) {
        use nimbus_core::UniformRand;

        let mut key = self.current_key.write().await;
        let sk = key.part1 + key.part2;

        let mut rng = rand::thread_rng();
        let new_part1 = nimbus_core::Fr::rand(&mut rng);
        let new_part2 = sk - new_part1;

        // Zeroize old parts
        unsafe {
            std::ptr::write_volatile(&mut key.part1, nimbus_core::Fr::from(0u64));
            std::ptr::write_volatile(&mut key.part2, nimbus_core::Fr::from(0u64));
        }

        key.part1 = new_part1;
        key.part2 = new_part2;
    }

    /// Fetch key from Vault/OpenBao (mirrors kms.rs logic)
    async fn fetch_key_from_vault(&self) -> Option<(nimbus_core::Fr, u32)> {
        let env_index = std::env::var("NIMBUS_SHARE_INDEX")
            .unwrap_or_else(|_| "1".to_string())
            .parse::<u32>()
            .unwrap_or(1);

        let vault_token = std::env::var("NIMBUS_VAULT_TOKEN").ok()?;
        let vault_addr = std::env::var("NIMBUS_VAULT_ADDR")
            .unwrap_or_else(|_| "http://127.0.0.1:8200".to_string());
        let vault_path = std::env::var("NIMBUS_VAULT_PATH")
            .unwrap_or_else(|_| "v1/secret/data/nimbus".to_string());

        let url = format!("{}/{}", vault_addr.trim_end_matches('/'), vault_path);

        match crate::http::get_http_with_headers(&url, &[("X-Vault-Token", &vault_token)]).await {
            Ok(body) => {
                #[derive(serde::Deserialize)]
                struct VaultSecretData {
                    share_key: String,
                }
                #[derive(serde::Deserialize)]
                struct VaultSecretInner {
                    data: VaultSecretData,
                }
                #[derive(serde::Deserialize)]
                struct VaultSecretResponse {
                    data: VaultSecretInner,
                }

                if let Ok(res) = serde_json::from_str::<VaultSecretResponse>(&body) {
                    if let Ok(bytes) = hex::decode(&res.data.data.share_key) {
                        if bytes.len() == 40 {
                            if let Some((idx, fr)) = nimbus_core::deserialize_from_bytes::<(
                                usize,
                                nimbus_core::Fr,
                            )>(&bytes)
                            {
                                return Some((fr, idx as u32));
                            }
                        } else if let Some(fr) =
                            nimbus_core::deserialize_from_bytes::<nimbus_core::Fr>(&bytes)
                        {
                            return Some((fr, env_index));
                        }
                    }
                }
                None
            }
            Err(_) => None,
        }
    }

    /// Spawn the background rotation task.
    /// This should be called once at startup.
    pub fn spawn_rotation_task(self) -> Option<tokio::task::JoinHandle<()>> {
        if !self.config.enabled {
            println!("KEY-ROTATION: Disabled (set NIMBUS_KEY_ROTATION=true to enable)");
            // Even when full rotation is disabled, re-mask every hour for defense-in-depth
            let km = self.clone();
            return Some(tokio::spawn(async move {
                loop {
                    tokio::time::sleep(Duration::from_secs(3600)).await;
                    km.re_mask().await;
                    println!("KEY-ROTATION: Re-masked key parts (defense-in-depth)");
                }
            }));
        }

        let km = self.clone();
        let interval = self.config.rotation_interval;
        let max_age = self.config.max_key_age;

        Some(tokio::spawn(async move {
            println!(
                "KEY-ROTATION: Enabled (interval: {}s, max_age: {}s)",
                interval.as_secs(),
                max_age.as_secs()
            );

            loop {
                tokio::time::sleep(interval).await;

                let (version, _index, age) = km.key_info().await;

                if age >= max_age {
                    println!(
                        "KEY-ROTATION: Key v{} exceeded max age ({:.0}s), forcing rotation",
                        version,
                        age.as_secs_f64()
                    );
                    km.rotate().await;
                } else {
                    // Re-mask even if not rotating for defense-in-depth
                    km.re_mask().await;
                }
            }
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nimbus_core::{serialize_to_bytes, split_secret_key, IssuerSecretKey};

    #[tokio::test]
    async fn different_shares_use_the_same_ceremony_public_key() {
        let issuer_secret = IssuerSecretKey(nimbus_core::Fr::from(17u64));
        let issuer_public = issuer_secret.public_key();
        let shares = split_secret_key(&issuer_secret, 2, 3, &mut rand::thread_rng());
        let config = KeyRotationConfig {
            enabled: false,
            ..KeyRotationConfig::default()
        };

        let first = KeyManager::new(
            shares[0].1,
            shares[0].0 as u32,
            issuer_public.clone(),
            config.clone(),
        );
        let second = KeyManager::new(
            shares[1].1,
            shares[1].0 as u32,
            issuer_public.clone(),
            config,
        );

        assert_ne!(shares[0].1, shares[1].1);
        assert_eq!(
            serialize_to_bytes(&first.get_issuer_public_key().await),
            serialize_to_bytes(&issuer_public),
        );
        assert_eq!(
            serialize_to_bytes(&second.get_issuer_public_key().await),
            serialize_to_bytes(&issuer_public),
        );
    }
}
