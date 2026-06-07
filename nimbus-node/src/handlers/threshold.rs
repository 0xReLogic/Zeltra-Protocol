//! Threshold signature handlers for distributed signing
//!
//! Uses circuit breaker pattern (Finding #15) for guardian RPC calls
//! to prevent cascading failures when guardians are unreachable.

use axum::Json;
use crate::{state::AppState, dto::*, http};
use std::collections::HashSet;

fn append_guardian_share(
    response: SignShareResponse,
    seen_indices: &mut HashSet<u32>,
    partial_signatures: &mut Vec<PartialSignatureInfo>,
) -> Result<(), &'static str> {
    if response.status != "SUCCESS" {
        return Err("guardian rejected signing request");
    }

    let index = response.share_index.ok_or("guardian omitted share index")?;
    if index == 0 {
        return Err("guardian returned invalid share index 0");
    }
    if !seen_indices.insert(index) {
        return Err("guardian returned duplicate share index");
    }

    partial_signatures.push(PartialSignatureInfo {
        index,
        signature_hex: response.signature_share_hex,
    });
    Ok(())
}

fn has_signing_quorum(partial_signatures: &[PartialSignatureInfo], threshold: usize) -> bool {
    threshold > 0 && partial_signatures.len() >= threshold
}

pub async fn handle_sign_share(
    axum::extract::State(state): axum::extract::State<AppState>,
    Json(payload): Json<SignShareRequest>,
) -> Json<SignShareResponse> {
    use nimbus_core::*;
    use std::str::FromStr;

    // 1. Verify the timestamp is within 60 seconds (unless in test mode).
    let is_test_env = cfg!(test) || std::env::var("NIMBUS_ENV").map(|v| v == "test").unwrap_or(false);
    if !is_test_env {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        if payload.timestamp > now + 60 || now > payload.timestamp + 60 {
            return Json(SignShareResponse {
                status: "ERROR".to_string(),
                share_index: None,
                signature_share_hex: "Timestamp expired or out of bounds".to_string(),
            });
        }
    }

    // 2. Recover the leader's address from the signature and payload.
    let msg = format!(
        "{}:{}:{}:{}:{}:{}:{}",
        payload.session_id,
        payload.blinded_hex,
        payload.k_hex,
        payload.timestamp,
        payload.amount,
        payload.client_address,
        payload.com_k_hex
    );
    let sig_bytes = match hex::decode(&payload.signature_hex) {
        Ok(b) => b,
        Err(e) => return Json(SignShareResponse {
            status: "ERROR".to_string(),
            share_index: None,
            signature_share_hex: format!("Invalid signature hex: {}", e),
        }),
    };
    let parsed_sig = match alloy::primitives::Signature::try_from(sig_bytes.as_slice()) {
        Ok(sig) => sig,
        Err(e) => return Json(SignShareResponse {
            status: "ERROR".to_string(),
            share_index: None,
            signature_share_hex: format!("Failed to parse signature: {}", e),
        }),
    };
    let recovered_address = match parsed_sig.recover_address_from_msg(msg.as_bytes()) {
        Ok(addr) => addr,
        Err(e) => return Json(SignShareResponse {
            status: "ERROR".to_string(),
            share_index: None,
            signature_share_hex: format!("Failed to recover address: {}", e),
        }),
    };

    // 3. Verify the recovered address matches leader_address.
    let expected_leader_address = match alloy::primitives::Address::from_str(&payload.leader_address) {
        Ok(addr) => addr,
        Err(e) => return Json(SignShareResponse {
            status: "ERROR".to_string(),
            share_index: None,
            signature_share_hex: format!("Invalid leader address: {}", e),
        }),
    };
    if recovered_address != expected_leader_address {
        return Json(SignShareResponse {
            status: "ERROR".to_string(),
            share_index: None,
            signature_share_hex: "Recovered address does not match leader_address".to_string(),
        });
    }

    // 4. Verify the leader address is in NIMBUS_TRUSTED_LEADERS env (or allow by default in test/dev modes if unset).
    if let Ok(trusted_leaders_str) = std::env::var("NIMBUS_TRUSTED_LEADERS") {
        let trusted_leaders: Vec<alloy::primitives::Address> = trusted_leaders_str
            .split(',')
            .filter_map(|s| alloy::primitives::Address::from_str(s.trim()).ok())
            .collect();
        if !trusted_leaders.contains(&recovered_address) {
            return Json(SignShareResponse {
                status: "ERROR".to_string(),
                share_index: None,
                signature_share_hex: format!("Leader address {} is not trusted", recovered_address),
            });
        }
    } else {
        let is_dev_or_test = cfg!(test) || std::env::var("NIMBUS_ENV").map(|v| v == "test" || v == "dev" || v == "development").unwrap_or(true);
        if !is_dev_or_test {
            return Json(SignShareResponse {
                status: "ERROR".to_string(),
                share_index: None,
                signature_share_hex: "NIMBUS_TRUSTED_LEADERS is not configured in production".to_string(),
            });
        }
    }

    // Decode blinded hex
    let blinded_bytes = match hex::decode(&payload.blinded_hex) {
        Ok(b) => b,
        Err(e) => return Json(SignShareResponse {
            status: "ERROR".to_string(),
            share_index: None,
            signature_share_hex: format!("Invalid blinded hex: {}", e),
        }),
    };

    // Decode k hex
    let k_bytes = match hex::decode(&payload.k_hex) {
        Ok(b) => b,
        Err(e) => return Json(SignShareResponse {
            status: "ERROR".to_string(),
            share_index: None,
            signature_share_hex: format!("Invalid k hex: {}", e),
        }),
    };

    let blinded: BlindedMessage = match deserialize_from_bytes(&blinded_bytes) {
        Some(x) => x,
        None => return Json(SignShareResponse {
            status: "ERROR".to_string(),
            share_index: None,
            signature_share_hex: "Failed to deserialize blinded message".to_string(),
        }),
    };

    let k: Fr = match deserialize_from_bytes(&k_bytes) {
        Some(val) => val,
        None => return Json(SignShareResponse {
            status: "ERROR".to_string(),
            share_index: None,
            signature_share_hex: "Failed to deserialize masking key k".to_string(),
        }),
    };

    // 5. Verify that com_k = pk_iss * k
    let pk_iss = state.key_manager.get_issuer_public_key().await;
    let expected_com_k = pk_iss.0 * k;
    let expected_com_k_bytes = serialize_to_bytes(&MaskingKeyCommitment(expected_com_k));
    let com_k_bytes = match hex::decode(&payload.com_k_hex) {
        Ok(b) => b,
        Err(e) => return Json(SignShareResponse {
            status: "ERROR".to_string(),
            share_index: None,
            signature_share_hex: format!("Invalid com_k hex: {}", e),
        }),
    };
    if com_k_bytes != expected_com_k_bytes {
        return Json(SignShareResponse {
            status: "ERROR".to_string(),
            share_index: None,
            signature_share_hex: "Provided com_k does not match pk_iss * k".to_string(),
        });
    }

    // 6. Call state.db.insert_signing_session to store the parameters. If it returns false, reject with a duplicate session error (replay/extraction protection).
    match state.db.insert_signing_session(
        &payload.session_id,
        &payload.com_k_hex,
        payload.amount,
        &payload.client_address,
        &payload.k_hex,
    ).await {
        Ok(true) => {
            // Success
        }
        Ok(false) => {
            return Json(SignShareResponse {
                status: "ERROR".to_string(),
                share_index: None,
                signature_share_hex: "Session ID already exists or double sign attempt detected".to_string(),
            });
        }
        Err(e) => {
            return Json(SignShareResponse {
                status: "ERROR".to_string(),
                share_index: None,
                signature_share_hex: format!("Database error: {}", e),
            });
        }
    }

    let sig_share = state.sign_share_masked(&blinded, &k).await;
    let sig_share_hex = hex::encode(serialize_to_bytes(&sig_share));
    let share_index = state.key_manager.share_index().await;
    if share_index == 0 {
        return Json(SignShareResponse {
            status: "ERROR".to_string(),
            share_index: None,
            signature_share_hex: "Guardian share index must be non-zero".to_string(),
        });
    }

    Json(SignShareResponse {
        status: "SUCCESS".to_string(),
        share_index: Some(share_index),
        signature_share_hex: sig_share_hex,
    })
}

pub async fn handle_leader_sign(
    axum::extract::State(state): axum::extract::State<AppState>,
    Json(payload): Json<LeaderSignRequest>,
) -> Json<LeaderSignResponse> {
    use nimbus_core::*;

    let blinded_bytes = match hex::decode(&payload.blinded_hex) {
        Ok(b) => b,
        Err(e) => return Json(LeaderSignResponse {
            status: format!("ERROR: Invalid blinded hex: {}", e),
            session_id: payload.session_id,
            com_k_hex: String::new(),
            partial_signatures: vec![],
        }),
    };

    let blinded: BlindedMessage = match deserialize_from_bytes(&blinded_bytes) {
        Some(x) => x,
        None => return Json(LeaderSignResponse {
            status: "ERROR: Failed to deserialize blinded message".to_string(),
            session_id: payload.session_id,
            com_k_hex: String::new(),
            partial_signatures: vec![],
        }),
    };

    let k = Fr::rand(&mut rand::thread_rng());

    let pk_iss = if let Some(pk_hex) = &payload.pk_iss_hex {
        if let Ok(bytes) = hex::decode(pk_hex) {
            deserialize_from_bytes(&bytes).unwrap_or_else(|| {
                // We reconstruct synchronously or block_on if inside closure
                let rt = tokio::runtime::Handle::current();
                rt.block_on(state.key_manager.get_issuer_public_key())
            })
        } else {
            state.key_manager.get_issuer_public_key().await
        }
    } else {
        state.key_manager.get_issuer_public_key().await
    };

    let com_k = pk_iss.0 * k;
    let leader_share_sig = state.sign_share_masked(&blinded, &k).await;

    let share_index = state.key_manager.share_index().await;
    if share_index == 0 {
        return Json(LeaderSignResponse {
            status: "ERROR: Leader share index must be non-zero".to_string(),
            session_id: payload.session_id,
            com_k_hex: String::new(),
            partial_signatures: vec![],
        });
    }
    let mut partial_signatures = vec![
        PartialSignatureInfo {
            index: share_index,
            signature_hex: hex::encode(serialize_to_bytes(&leader_share_sig)),
        }
    ];

    let com_k_hex = hex::encode(serialize_to_bytes(&MaskingKeyCommitment(com_k)));
    let k_hex = hex::encode(serialize_to_bytes(&MaskingKey(k)));

    let leader_signer = if let Some(ref client) = state.evm_client {
        client.signer.clone()
    } else {
        use alloy::signers::local::PrivateKeySigner;
        use std::str::FromStr;
        if let Ok(pk_str) = std::env::var("NIMBUS_RELAYER_PRIVATE_KEY") {
            PrivateKeySigner::from_str(&pk_str).unwrap_or_else(|_| {
                PrivateKeySigner::from_str("0000000000000000000000000000000000000000000000000000000000000001").unwrap()
            })
        } else {
            PrivateKeySigner::from_str("0000000000000000000000000000000000000000000000000000000000000001").unwrap()
        }
    };
    let leader_address = format!("0x{:x}", leader_signer.address());

    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    use alloy::signers::Signer;
    let message = format!(
        "{}:{}:{}:{}:{}:{}:{}",
        payload.session_id,
        payload.blinded_hex,
        k_hex,
        timestamp,
        payload.amount,
        payload.client_address,
        com_k_hex
    );
    let signature_hex = match leader_signer.sign_message(message.as_bytes()).await {
        Ok(sig) => hex::encode(sig.as_bytes()),
        Err(e) => return Json(LeaderSignResponse {
            status: format!("ERROR: Failed to sign leader payload: {}", e),
            session_id: payload.session_id,
            com_k_hex: String::new(),
            partial_signatures: vec![],
        }),
    };

    let client_body = serde_json::to_string(&SignShareRequest {
        session_id: payload.session_id.clone(),
        amount: payload.amount,
        client_address: payload.client_address.clone(),
        com_k_hex: com_k_hex.clone(),
        blinded_hex: payload.blinded_hex.clone(),
        k_hex: k_hex.clone(),
        leader_address,
        timestamp,
        signature_hex,
    }).unwrap();

    // Call guardians with circuit breaker (Finding #15)
    let required_threshold = std::env::var("NIMBUS_THRESHOLD")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(3);
    let mut seen_indices = HashSet::new();
    seen_indices.insert(share_index);

    for url in &payload.guardian_urls {
        let target_url = format!("{}/api/sign-share", url.trim_end_matches('/'));
        let body = client_body.clone();
        let url_clone = target_url.clone();
        let cb = state.guardian_circuit_breaker.clone();
        
        match cb.call(|| async {
            http::post_http(&url_clone, &body).await
        }).await {
            Ok(response_body) => {
                if let Ok(res) = serde_json::from_str::<SignShareResponse>(&response_body) {
                    if let Err(error) =
                        append_guardian_share(res, &mut seen_indices, &mut partial_signatures)
                    {
                        println!("RELAYER: Guardian {} rejected: {}", url, error);
                    }
                } else {
                    println!("RELAYER: Guardian {} returned malformed JSON", url);
                }
            }
            Err(e) => {
                println!("RELAYER: Error calling Guardian at {}: {}", url, e);
            }
        }
    }

    if !has_signing_quorum(&partial_signatures, required_threshold) {
        return Json(LeaderSignResponse {
            status: format!(
                "ERROR: Guardian quorum unavailable: got {} unique shares, require {}",
                partial_signatures.len(),
                required_threshold
            ),
            session_id: payload.session_id,
            com_k_hex: String::new(),
            partial_signatures: vec![],
        });
    }

    // Insert signing session into database
    match state.db.insert_signing_session(
        &payload.session_id,
        &com_k_hex,
        payload.amount,
        &payload.client_address,
        &k_hex,
    ).await {
        Ok(true) => {
            println!("RELAYER: Signing session registered for Session ID: {}", payload.session_id);
            Json(LeaderSignResponse {
                status: "SUCCESS".to_string(),
                session_id: payload.session_id,
                com_k_hex,
                partial_signatures,
            })
        }
        Ok(false) => {
            Json(LeaderSignResponse {
                status: "ERROR: Session ID already exists".to_string(),
                session_id: payload.session_id,
                com_k_hex: String::new(),
                partial_signatures: vec![],
            })
        }
        Err(e) => {
            Json(LeaderSignResponse {
                status: format!("ERROR: Database error: {}", e),
                session_id: payload.session_id,
                com_k_hex: String::new(),
                partial_signatures: vec![],
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn response(share_index: Option<u32>) -> SignShareResponse {
        SignShareResponse {
            status: "SUCCESS".to_string(),
            share_index,
            signature_share_hex: "partial-signature".to_string(),
        }
    }

    #[test]
    fn guardian_share_requires_unique_non_zero_index() {
        let mut seen_indices = HashSet::from([1]);
        let mut partial_signatures = vec![PartialSignatureInfo {
            index: 1,
            signature_hex: "leader-signature".to_string(),
        }];

        assert!(append_guardian_share(
            response(Some(2)),
            &mut seen_indices,
            &mut partial_signatures,
        )
        .is_ok());
        assert_eq!(partial_signatures.len(), 2);

        assert!(append_guardian_share(
            response(Some(2)),
            &mut seen_indices,
            &mut partial_signatures,
        )
        .is_err());
        assert!(append_guardian_share(
            response(Some(0)),
            &mut seen_indices,
            &mut partial_signatures,
        )
        .is_err());
        assert!(append_guardian_share(
            response(None),
            &mut seen_indices,
            &mut partial_signatures,
        )
        .is_err());
        assert_eq!(partial_signatures.len(), 2);
    }

    #[test]
    fn signing_quorum_rejects_sub_threshold_share_set() {
        let partial_signatures = vec![
            PartialSignatureInfo {
                index: 1,
                signature_hex: "leader-signature".to_string(),
            },
            PartialSignatureInfo {
                index: 2,
                signature_hex: "guardian-signature".to_string(),
            },
        ];

        assert!(!has_signing_quorum(&partial_signatures, 3));
        assert!(has_signing_quorum(&partial_signatures, 2));
        assert!(!has_signing_quorum(&partial_signatures, 0));
    }
}
