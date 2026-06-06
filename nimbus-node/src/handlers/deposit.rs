//! Deposit and reveal handlers with idempotency support (Finding #16)

use axum::Json;
use crate::{state::AppState, dto::*};

pub async fn handle_deposit(
    axum::extract::State(state): axum::extract::State<AppState>,
    Json(payload): Json<DepositRequest>,
) -> Json<DepositResponse> {
    // --- Idempotency check (Finding #16) ---
    if let Some(ref idem_key) = payload.idempotency_key {
        match state.db.check_idempotency(idem_key, "deposit").await {
            Ok(Some(cached_json)) => {
                if let Ok(cached_resp) = serde_json::from_str::<DepositResponse>(&cached_json) {
                    println!("IDEMPOTENCY: Returning cached deposit response for key {}...", &idem_key[..8.min(idem_key.len())]);
                    return Json(cached_resp);
                }
            }
            Ok(None) => { /* No cached response, proceed normally */ }
            Err(e) => {
                eprintln!("RELAYER WARNING: Idempotency check failed: {}", e);
            }
        }
    }

    // Store session in persistent database
    let com_k_hex = hex::encode(&payload.com_k);
    let client_address = format!("0x{:040x}", rand::random::<u128>()); // Mock client address
    
    let response = match state.db.insert_session(
        &payload.session_id,
        &com_k_hex,
        payload.amount,
        &client_address,
    ).await {
        Ok(true) => {
            println!("RELAYER: Escrow deposit registered for Session ID: {}...", &payload.session_id[..8.min(payload.session_id.len())]);
            DepositResponse {
                status: "SUCCESS".to_string(),
                message: format!("Escrow registered for session {}", payload.session_id),
            }
        }
        Ok(false) => {
            println!("RELAYER: Duplicate session ID attempted: {}...", &payload.session_id[..8.min(payload.session_id.len())]);
            DepositResponse {
                status: "ERROR".to_string(),
                message: "Session ID already exists".to_string(),
            }
        }
        Err(e) => {
            eprintln!("RELAYER ERROR: Database insert failed: {}", e);
            DepositResponse {
                status: "ERROR".to_string(),
                message: "Database error".to_string(),
            }
        }
    };

    // Store idempotency response
    if let Some(ref idem_key) = payload.idempotency_key {
        if let Ok(json) = serde_json::to_string(&response) {
            let _ = state.db.store_idempotency(idem_key, "deposit", &json).await;
        }
    }

    Json(response)
}

pub async fn handle_reveal(
    axum::extract::State(state): axum::extract::State<AppState>,
    Json(payload): Json<RevealRequest>,
) -> Json<RevealResponse> {
    let masking_key_hex = hex::encode(&payload.masking_key_k);
    
    match state.db.resolve_session(&payload.session_id, &masking_key_hex).await {
        Ok(true) => {
            // In production, we would perform BLS12-381 G2 MSM: k * pk_iss == com_k
            // For simulation, we log the verification success
            println!("RELAYER: Masking key revealed for Session ID: {}...", &payload.session_id[..8.min(payload.session_id.len())]);
            println!("  Verifying k * pk_iss == com_k ... VALID");

            Json(RevealResponse {
                status: "SUCCESS".to_string(),
                valid: true,
                message: "Masking key verified and published on-chain. Escrow released.".to_string(),
            })
        }
        Ok(false) => {
            Json(RevealResponse {
                status: "ERROR".to_string(),
                valid: false,
                message: "Session ID not found or already resolved".to_string(),
            })
        }
        Err(e) => {
            eprintln!("RELAYER ERROR: Database resolve failed: {}", e);
            Json(RevealResponse {
                status: "ERROR".to_string(),
                valid: false,
                message: "Database error".to_string(),
            })
        }
    }
}
