//! Threshold signature handlers for distributed signing

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

    let share_sk = if let Some(override_hex) = &payload.share_sk_hex {
        if let Ok(bytes) = hex::decode(override_hex) {
            deserialize_from_bytes(&bytes).unwrap_or(*state.share_sk)
        } else {
            *state.share_sk
        }
    } else {
        *state.share_sk
    };

    let sig_share = sign_share(&share_sk, &blinded, &k);
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
            com_k_hex: String::new(),
            k_hex: String::new(),
            partial_signatures: vec![],
        }),
    };

    let blinded: BlindedMessage = match deserialize_from_bytes(&blinded_bytes) {
        Some(x) => x,
        None => return Json(LeaderSignResponse {
            status: "ERROR: Failed to deserialize blinded message".to_string(),
            com_k_hex: String::new(),
            k_hex: String::new(),
            partial_signatures: vec![],
        }),
    };

    let k = Fr::rand(&mut rand::thread_rng());

    let pk_iss = if let Some(pk_hex) = &payload.pk_iss_hex {
        if let Ok(bytes) = hex::decode(pk_hex) {
            deserialize_from_bytes(&bytes).unwrap_or_else(|| {
                let sk_iss = IssuerSecretKey(*state.share_sk);
                sk_iss.public_key()
            })
        } else {
            let sk_iss = IssuerSecretKey(*state.share_sk);
            sk_iss.public_key()
        }
    } else {
        let sk_iss = IssuerSecretKey(*state.share_sk);
        sk_iss.public_key()
    };

    let com_k = pk_iss.0 * k;
    let leader_share_sig = sign_share(&state.share_sk, &blinded, &k);

    let mut partial_signatures = vec![
        PartialSignatureInfo {
            index: state.share_index,
            signature_hex: hex::encode(serialize_to_bytes(&leader_share_sig)),
        }
    ];

    let client_body = serde_json::to_string(&SignShareRequest {
        blinded_hex: payload.blinded_hex.clone(),
        k_hex: hex::encode(serialize_to_bytes(&k)),
        share_sk_hex: None,
    }).unwrap();

    for (idx, url) in payload.guardian_urls.iter().enumerate() {
        let target_url = format!("{}/api/sign-share", url.trim_end_matches('/'));
        match http::post_http(&target_url, &client_body).await {
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

    Json(LeaderSignResponse {
        status: "SUCCESS".to_string(),
        com_k_hex: hex::encode(serialize_to_bytes(&MaskingKeyCommitment(com_k))),
        k_hex: hex::encode(serialize_to_bytes(&MaskingKey(k))),
        partial_signatures,
    })
}
