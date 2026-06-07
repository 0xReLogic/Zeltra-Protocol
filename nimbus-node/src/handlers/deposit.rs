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

    let com_k_hex = payload.com_k.trim_start_matches("0x").to_string();
    
    // First try confirming the existing signing session
    let response = match state.db.confirm_deposit(&payload.session_id, payload.amount, &com_k_hex).await {
        Ok(true) => {
            println!("RELAYER: Deposit confirmed and registered for Session ID: {}", payload.session_id);
            DepositResponse {
                status: "SUCCESS".to_string(),
                message: format!("Deposit confirmed for session {}", payload.session_id),
            }
        }
        Ok(false) => {
            // Fallback: If no matching signing session is found (e.g. legacy/testing path), call insert_session and then confirm it
            let client_address = format!("0x{:040x}", rand::random::<u128>()); // Mock client address
            match state.db.insert_session(&payload.session_id, &com_k_hex, payload.amount, &client_address).await {
                Ok(true) => {
                    let _ = state.db.confirm_deposit(&payload.session_id, payload.amount, &com_k_hex).await;
                    println!("RELAYER: Fallback session registered and confirmed for Session ID: {}", payload.session_id);
                    DepositResponse {
                        status: "SUCCESS".to_string(),
                        message: format!("Escrow registered for session {}", payload.session_id),
                    }
                }
                Ok(false) => {
                    println!("RELAYER: Duplicate session ID attempted: {}", payload.session_id);
                    DepositResponse {
                        status: "ERROR".to_string(),
                        message: "Session ID already exists".to_string(),
                    }
                }
                Err(e) => {
                    eprintln!("RELAYER ERROR: Database insert fallback failed: {}", e);
                    DepositResponse {
                        status: "ERROR".to_string(),
                        message: "Database error".to_string(),
                    }
                }
            }
        }
        Err(e) => {
            eprintln!("RELAYER ERROR: Database confirm deposit failed: {}", e);
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
    let session_id = payload.session_id.clone();
    match state.db.resolve_session_release(&session_id).await {
        Ok(Some(masking_key_hex)) => {
            println!("RELAYER: Masking key revealed for Session ID: {}", session_id);
            
            // Zeroize the masking key after revealing (DEC-011)
            // Spawn background task to avoid delaying the response
            let state_clone = state.clone();
            let session_id_clone = session_id.clone();
            tokio::spawn(async move {
                match state_clone.db.zeroize_session_masking_key(&session_id_clone).await {
                    Ok(true) => println!("RELAYER: Masking key zeroized for Session ID: {}", session_id_clone),
                    Ok(false) => println!("RELAYER: Session already zeroized or not found: {}", session_id_clone),
                    Err(e) => eprintln!("RELAYER ERROR: Failed to zeroize masking key for Session ID {}: {}", session_id_clone, e),
                }
            });
            
            Json(RevealResponse {
                status: "SUCCESS".to_string(),
                valid: true,
                message: "Masking key verified and released. Escrow resolved.".to_string(),
                masking_key_hex: Some(masking_key_hex),
            })
        }
        Ok(None) => {
            Json(RevealResponse {
                status: "ERROR".to_string(),
                valid: false,
                message: "Session ID not found, not confirmed, or already resolved".to_string(),
                masking_key_hex: None,
            })
        }
        Err(e) => {
            eprintln!("RELAYER ERROR: Database resolve release failed: {}", e);
            Json(RevealResponse {
                status: "ERROR".to_string(),
                valid: false,
                message: "Database error".to_string(),
                masking_key_hex: None,
            })
        }
    }
}
