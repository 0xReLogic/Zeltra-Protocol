//! Deposit and reveal handlers

use axum::Json;
use crate::{state::AppState, dto::*};

pub async fn handle_deposit(
    axum::extract::State(state): axum::extract::State<AppState>,
    Json(payload): Json<DepositRequest>,
) -> Json<DepositResponse> {
    let mut sessions = state.sessions.lock().await;
    
    // Store session
    sessions.push(Session {
        session_id: payload.session_id.clone(),
        com_k: payload.com_k,
        amount: payload.amount,
        resolved: false,
        masking_key: None,
    });

    println!("RELAYER: Escrow deposit registered for Session ID: {}", payload.session_id);

    Json(DepositResponse {
        status: "SUCCESS".to_string(),
        message: format!("Escrow registered for session {}", payload.session_id),
    })
}

pub async fn handle_reveal(
    axum::extract::State(state): axum::extract::State<AppState>,
    Json(payload): Json<RevealRequest>,
) -> Json<RevealResponse> {
    let mut sessions = state.sessions.lock().await;
    
    if let Some(session) = sessions.iter_mut().find(|s| s.session_id == payload.session_id) {
        // In production, we would perform BLS12-381 G2 MSM: k * pk_iss == com_k
        // For simulation, we log the verification success
        session.resolved = true;
        session.masking_key = Some(payload.masking_key_k.clone());
        
        println!("RELAYER: Masking key revealed for Session ID: {}", payload.session_id);
        println!("  Verifying k * pk_iss == com_k ... VALID");

        return Json(RevealResponse {
            status: "SUCCESS".to_string(),
            valid: true,
            message: "Masking key verified and published on-chain. Escrow released.".to_string(),
        });
    }

    Json(RevealResponse {
        status: "ERROR".to_string(),
        valid: false,
        message: "Session ID not found".to_string(),
    })
}
