//! Threshold signature handlers for distributed signing
//!
//! Uses circuit breaker pattern (Finding #15) for guardian RPC calls
//! to prevent cascading failures when guardians are unreachable.

use axum::Json;
use crate::{state::AppState, dto::*, http};

pub async fn handle_sign_share(
    axum::extract::State(state): axum::extract::State<AppState>,
    Json(payload): Json<SignShareRequest>,
) -> Json<SignShareResponse> {
    use nimbus_core::*;

    let blinded_bytes = match hex::decode(&payload.blinded_hex) {
        Ok(b) => b,
        Err(e) => return Json(SignShareResponse {
            status: "ERROR".to_string(),
            signature_share_hex: format!("Invalid blinded hex: {}", e),
        }),
    };

    let k_bytes = match hex::decode(&payload.k_hex) {
        Ok(b) => b,
        Err(e) => return Json(SignShareResponse {
            status: "ERROR".to_string(),
            signature_share_hex: format!("Invalid k hex: {}", e),
        }),
    };

    let blinded: BlindedMessage = match deserialize_from_bytes(&blinded_bytes) {
        Some(x) => x,
        None => return Json(SignShareResponse {
            status: "ERROR".to_string(),
            signature_share_hex: "Failed to deserialize blinded message".to_string(),
        }),
    };

    let k: Fr = match deserialize_from_bytes(&k_bytes) {
        Some(val) => val,
        None => return Json(SignShareResponse {
            status: "ERROR".to_string(),
            signature_share_hex: "Failed to deserialize masking key k".to_string(),
        }),
    };

    let sig_share = state.sign_share_masked(&blinded, &k).await;
    let sig_share_hex = hex::encode(serialize_to_bytes(&sig_share));

    Json(SignShareResponse {
        status: "SUCCESS".to_string(),
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
                state.get_issuer_public_key()
            })
        } else {
            state.get_issuer_public_key()
        }
    } else {
        state.get_issuer_public_key()
    };

    let com_k = pk_iss.0 * k;
    let leader_share_sig = state.sign_share_masked(&blinded, &k).await;

    let share_index = state.key_manager.share_index().await;
    let mut partial_signatures = vec![
        PartialSignatureInfo {
            index: share_index,
            signature_hex: hex::encode(serialize_to_bytes(&leader_share_sig)),
        }
    ];

    let client_body = serde_json::to_string(&SignShareRequest {
        blinded_hex: payload.blinded_hex.clone(),
        k_hex: hex::encode(serialize_to_bytes(&k)),
    }).unwrap();

    // Call guardians with circuit breaker (Finding #15)
    for (idx, url) in payload.guardian_urls.iter().enumerate() {
        let target_url = format!("{}/api/sign-share", url.trim_end_matches('/'));
        let body = client_body.clone();
        let url_clone = target_url.clone();
        let cb = state.guardian_circuit_breaker.clone();
        
        match cb.call(|| async {
            http::post_http(&url_clone, &body).await
        }).await {
            Ok(response_body) => {
                if let Ok(res) = serde_json::from_str::<SignShareResponse>(&response_body) {
                    if res.status == "SUCCESS" {
                        partial_signatures.push(PartialSignatureInfo {
                            index: (idx + 2) as u32,
                            signature_hex: res.signature_share_hex,
                        });
                    }
                }
            }
            Err(e) => {
                println!("RELAYER: Error calling Guardian at {}: {}", url, e);
            }
        }
    }

    let com_k_hex = hex::encode(serialize_to_bytes(&MaskingKeyCommitment(com_k)));
    let k_hex = hex::encode(serialize_to_bytes(&MaskingKey(k)));

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
