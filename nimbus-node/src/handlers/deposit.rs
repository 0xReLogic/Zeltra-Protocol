//! Deposit and reveal handlers with idempotency support (Finding #16)

use crate::{dto::*, state::AppState};
use axum::Json;

pub async fn handle_deposit(
    axum::extract::State(state): axum::extract::State<AppState>,
    Json(payload): Json<DepositRequest>,
) -> Json<DepositResponse> {
    // --- Idempotency check (Finding #16) ---
    if let Some(ref idem_key) = payload.idempotency_key {
        match state.db.check_idempotency(idem_key, "deposit").await {
            Ok(Some(cached_json)) => {
                if let Ok(cached_resp) = serde_json::from_str::<DepositResponse>(&cached_json) {
                    println!(
                        "IDEMPOTENCY: Returning cached deposit response for key {}...",
                        &idem_key[..8.min(idem_key.len())]
                    );
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

    // Inflow Compliance Gate (DEC-027): Verify depositor address is not sanctioned
    if let Ok(Some(sess)) = state.db.get_session_for_reveal(&payload.session_id).await {
        use std::str::FromStr;
        if let Ok(client_addr) = alloy_primitives::Address::from_str(&sess.client_address) {
            if let crate::validation::ComplianceStatus::Sanctioned(source) =
                crate::validation::check_address_compliance(
                    &client_addr,
                    state.evm_client.as_deref(),
                )
                .await
            {
                eprintln!(
                    "COMPLIANCE ALERT: Deposit rejected for sanctioned depositor {} per {} (DEC-027)",
                    client_addr, source
                );
                return Json(DepositResponse {
                    status: "REJECTED".to_string(),
                    message: format!(
                        "Depositor address {} is restricted under sanctions policy ({})",
                        client_addr, source
                    ),
                });
            }
        }
    }

    // 1. Check if deposit is already confirmed on-chain in DB
    let is_confirmed = state
        .db
        .is_deposit_confirmed(&payload.session_id)
        .await
        .unwrap_or(false);
    let response = if is_confirmed {
        println!(
            "RELAYER: Deposit already confirmed on-chain for Session ID: {}",
            payload.session_id
        );
        DepositResponse {
            status: "SUCCESS".to_string(),
            message: format!("Deposit confirmed for session {}", payload.session_id),
        }
    } else {
        // 2. Attempt on-chain verification (DEC-018)
        let verified = crate::deposit_indexer::verify_session_on_chain(
            &state,
            &payload.session_id,
            payload.tx_hash.as_deref(),
        )
        .await
        .unwrap_or(false);

        if verified {
            println!(
                "RELAYER: Deposit verified on-chain and confirmed for Session ID: {}",
                payload.session_id
            );
            DepositResponse {
                status: "SUCCESS".to_string(),
                message: format!(
                    "Deposit verified on-chain for session {}",
                    payload.session_id
                ),
            }
        } else {
            // Check if signing session exists in database
            match state
                .db
                .validate_signing_session(&payload.session_id, payload.amount, &com_k_hex)
                .await
            {
                Ok(true) => {
                    // In offline dev/test mode without an EVM client, allow local confirmation
                    if state.evm_client.is_none() && crate::config::runtime_mode().is_dev() {
                        let _ = state
                            .db
                            .confirm_deposit(&payload.session_id, payload.amount, &com_k_hex)
                            .await;
                        DepositResponse {
                            status: "SUCCESS".to_string(),
                            message: format!(
                                "Deposit confirmed in dev mode for session {}",
                                payload.session_id
                            ),
                        }
                    } else {
                        // Strict on-chain requirement: session exists, waiting for on-chain block confirmation
                        DepositResponse {
                            status: "PENDING".to_string(),
                            message: format!(
                                "Deposit transaction for session {} is pending on-chain confirmation",
                                payload.session_id
                            ),
                        }
                    }
                }
                _ => DepositResponse {
                    status: "REJECTED".to_string(),
                    message: "Session ID not found or parameter mismatch. Initiate signing ceremony first."
                        .to_string(),
                },
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

    // 1. Fetch session details from database (DEC-018)
    let session = match state.db.get_session_for_reveal(&session_id).await {
        Ok(Some(s)) => s,
        Ok(None) => {
            return Json(RevealResponse {
                status: "ERROR".to_string(),
                valid: false,
                message: "Session ID not found".to_string(),
                masking_key_hex: None,
            });
        }
        Err(e) => {
            eprintln!(
                "RELAYER ERROR: Database get_session_for_reveal failed: {}",
                e
            );
            return Json(RevealResponse {
                status: "ERROR".to_string(),
                valid: false,
                message: "Database error".to_string(),
                masking_key_hex: None,
            });
        }
    };

    // 2. Validate session lifecycle states
    if !session.deposit_confirmed {
        return Json(RevealResponse {
            status: "PENDING".to_string(),
            valid: false,
            message: "Deposit has not been confirmed on-chain".to_string(),
            masking_key_hex: None,
        });
    }

    if session.resolved {
        return Json(RevealResponse {
            status: "ERROR".to_string(),
            valid: false,
            message: "Session has already been resolved".to_string(),
            masking_key_hex: None,
        });
    }

    // Inflow Compliance Gate (DEC-027): Verify client address is not sanctioned before revealing masking key k
    use std::str::FromStr;
    if let Ok(client_addr) = alloy_primitives::Address::from_str(&session.client_address) {
        if let crate::validation::ComplianceStatus::Sanctioned(source) =
            crate::validation::check_address_compliance(&client_addr, state.evm_client.as_deref())
                .await
        {
            eprintln!(
                "CRITICAL COMPLIANCE ALERT: Masking key reveal rejected for sanctioned client {} per {} (DEC-027)",
                client_addr, source
            );
            return Json(RevealResponse {
                status: "REJECTED".to_string(),
                valid: false,
                message: format!(
                    "Client address {} is restricted under sanctions policy ({})",
                    client_addr, source
                ),
                masking_key_hex: None,
            });
        }
    }

    // 3. Cryptographic Verification: com_k == k * pk_iss on G2 (DEC-018)
    let k_bytes = match hex::decode(&session.masking_key_hex) {
        Ok(b) => b,
        Err(_) => {
            return Json(RevealResponse {
                status: "ERROR".to_string(),
                valid: false,
                message: "Corrupted masking key in database".to_string(),
                masking_key_hex: None,
            });
        }
    };
    let k: nimbus_core::MaskingKey = match nimbus_core::deserialize_from_bytes(&k_bytes) {
        Some(key) => key,
        None => {
            return Json(RevealResponse {
                status: "ERROR".to_string(),
                valid: false,
                message: "Invalid masking key scalar encoding".to_string(),
                masking_key_hex: None,
            });
        }
    };

    let com_k_bytes = match hex::decode(&session.com_k_hex) {
        Ok(b) => b,
        Err(_) => {
            return Json(RevealResponse {
                status: "ERROR".to_string(),
                valid: false,
                message: "Corrupted commitment in database".to_string(),
                masking_key_hex: None,
            });
        }
    };
    let com_k: nimbus_core::MaskingKeyCommitment =
        match nimbus_core::deserialize_from_bytes(&com_k_bytes) {
            Some(c) => c,
            None => {
                return Json(RevealResponse {
                    status: "ERROR".to_string(),
                    valid: false,
                    message: "Invalid commitment point encoding".to_string(),
                    masking_key_hex: None,
                });
            }
        };

    let pk_iss = state.key_manager.get_issuer_public_key().await;
    if !nimbus_core::verify_masking_key_commitment(&pk_iss, &k, &com_k) {
        eprintln!(
            "CRITICAL SECURITY ALERT: Masking key cryptographic verification failed for session {}! k * pk_iss != com_k",
            session_id
        );
        return Json(RevealResponse {
            status: "REJECTED".to_string(),
            valid: false,
            message: "Cryptographic verification failed: k * pk_iss does not match stored com_k"
                .to_string(),
            masking_key_hex: None,
        });
    }

    // 4. Mark resolved atomically in database
    match state.db.mark_session_resolved(&session_id).await {
        Ok(true) => {
            println!(
                "RELAYER: Masking key cryptographically verified and revealed for Session ID: {}",
                session_id
            );

            // Zeroize the masking key in database after revealing (DEC-011)
            let state_clone = state.clone();
            let session_id_clone = session_id.clone();
            tokio::spawn(async move {
                match state_clone
                    .db
                    .zeroize_session_masking_key(&session_id_clone)
                    .await
                {
                    Ok(true) => println!(
                        "RELAYER: Masking key zeroized for Session ID: {}",
                        session_id_clone
                    ),
                    Ok(false) => println!(
                        "RELAYER: Session already zeroized or not found: {}",
                        session_id_clone
                    ),
                    Err(e) => eprintln!(
                        "RELAYER ERROR: Failed to zeroize masking key for Session ID {}: {}",
                        session_id_clone, e
                    ),
                }
            });

            Json(RevealResponse {
                status: "SUCCESS".to_string(),
                valid: true,
                message: "Masking key cryptographically verified and released. Escrow resolved."
                    .to_string(),
                masking_key_hex: Some(session.masking_key_hex),
            })
        }
        Ok(false) => Json(RevealResponse {
            status: "ERROR".to_string(),
            valid: false,
            message: "Session already resolved concurrently".to_string(),
            masking_key_hex: None,
        }),
        Err(e) => {
            eprintln!(
                "RELAYER ERROR: Database mark_session_resolved failed: {}",
                e
            );
            Json(RevealResponse {
                status: "ERROR".to_string(),
                valid: false,
                message: "Database error".to_string(),
                masking_key_hex: None,
            })
        }
    }
}

/// Background job to monitor and auto-refund stalled signing sessions (DEC-012 Phase 2)
/// Checks for sessions that are deposit confirmed but not resolved after threshold
pub async fn quorum_failure_monitor(state: AppState) {
    let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(60)); // Check every 60 seconds

    loop {
        interval.tick().await;

        // Check if auto-refund is enabled
        let auto_refund_enabled = std::env::var("NIMBUS_AUTO_REFUND")
            .unwrap_or_else(|_| "false".to_string())
            .parse()
            .unwrap_or(false);

        if !auto_refund_enabled {
            continue; // Skip if auto-refund is disabled
        }

        let threshold_seconds = std::env::var("NIMBUS_SIGNING_STALL_THRESHOLD")
            .unwrap_or_else(|_| "600".to_string())
            .parse()
            .unwrap_or(600); // Default 10 minutes

        match state
            .db
            .get_stalled_signing_sessions(threshold_seconds)
            .await
        {
            Ok(stalled_sessions) => {
                for session in stalled_sessions {
                    // TODO: Phase 2 - Check on-chain state before triggering refund
                    // Currently just log warning
                    println!(
                        "QUORUM MONITOR: Stalled session detected - session_id: {}, amount: {}, client: {}, created_at: {}",
                        session.session_id, session.amount, session.client_address, session.created_at
                    );

                    // Future: Check on-chain state and trigger refund if:
                    // 1. Deposit is confirmed on-chain
                    // 2. Session is not spent/refunded on-chain
                    // 3. Signing has stalled beyond threshold

                    // state.trigger_refund(&session.session_id).await;
                }
            }
            Err(e) => {
                eprintln!("QUORUM MONITOR: Failed to check stalled sessions: {}", e);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::Database;
    use crate::dto::RevealRequest;
    use crate::state::AppState;
    use axum::extract::State;
    use axum::Json;
    use nimbus_core::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_handle_reveal_positive_and_negative_cases() {
        std::env::set_var("NIMBUS_DB_KEY", "test-encryption-key");
        std::env::set_var("NIMBUS_ENV", "test");

        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("reveal_handler.db");
        let db = Database::new(&db_path).await.unwrap();

        let share_sk = Fr::from(100u64);
        let share_index = 1;
        let issuer_sk = IssuerSecretKey(share_sk);
        let issuer_pk = issuer_sk.public_key();

        let state = AppState::new(
            db.clone(),
            share_sk,
            share_index,
            issuer_pk.clone(),
            std::collections::HashMap::new(),
            None,
        )
        .await;

        let k_scalar = Fr::from(42u64);
        let k = MaskingKey(k_scalar);
        let com_k = MaskingKeyCommitment(issuer_pk.0 * k_scalar);

        let k_hex = hex::encode(serialize_to_bytes(&k));
        let com_k_hex = hex::encode(serialize_to_bytes(&com_k));

        let session_valid = "0xsession_valid";
        let amount = 1_000_000u64;
        let client_address = "0x0000000000000000000000000000000000000001";

        db.insert_signing_session(session_valid, &com_k_hex, amount, client_address, &k_hex)
            .await
            .unwrap();

        // 1. Negative: Unconfirmed deposit should return PENDING
        let res_unconfirmed = handle_reveal(
            State(state.clone()),
            Json(RevealRequest {
                session_id: session_valid.to_string(),
            }),
        )
        .await;
        assert_eq!(res_unconfirmed.0.status, "PENDING");
        assert!(!res_unconfirmed.0.valid);
        assert_eq!(res_unconfirmed.0.masking_key_hex, None);

        // Confirm deposit on-chain
        assert!(db
            .confirm_deposit_on_chain(session_valid, amount, client_address, "0xtx1", 100)
            .await
            .unwrap());

        // 2. Positive: Now reveal should succeed and pass cryptographic verification
        let res_valid = handle_reveal(
            State(state.clone()),
            Json(RevealRequest {
                session_id: session_valid.to_string(),
            }),
        )
        .await;
        assert_eq!(res_valid.0.status, "SUCCESS");
        assert!(res_valid.0.valid);
        assert_eq!(res_valid.0.masking_key_hex, Some(k_hex.clone()));

        // Check that session is marked resolved
        let s = db
            .get_session_for_reveal(session_valid)
            .await
            .unwrap()
            .unwrap();
        assert!(s.resolved);

        // 3. Negative: Tampered masking key in DB fails closed
        let session_tampered = "0xsession_tampered";
        let bad_k = MaskingKey(Fr::from(999u64));
        let bad_k_hex = hex::encode(serialize_to_bytes(&bad_k));

        db.insert_signing_session(
            session_tampered,
            &com_k_hex, // valid com_k for k=42, but stored k is 999!
            amount,
            client_address,
            &bad_k_hex,
        )
        .await
        .unwrap();
        assert!(db
            .confirm_deposit_on_chain(session_tampered, amount, client_address, "0xtx2", 101)
            .await
            .unwrap());

        let res_tampered = handle_reveal(
            State(state.clone()),
            Json(RevealRequest {
                session_id: session_tampered.to_string(),
            }),
        )
        .await;
        assert_eq!(res_tampered.0.status, "REJECTED");
        assert!(!res_tampered.0.valid);
        assert_eq!(res_tampered.0.masking_key_hex, None);

        // Verify session remains unresolved!
        let s_tampered = db
            .get_session_for_reveal(session_tampered)
            .await
            .unwrap()
            .unwrap();
        assert!(!s_tampered.resolved);

        // 4. Negative: Non-existent session
        let res_nonexistent = handle_reveal(
            State(state.clone()),
            Json(RevealRequest {
                session_id: "0xnonexistent".to_string(),
            }),
        )
        .await;
        assert_eq!(res_nonexistent.0.status, "ERROR");
        assert!(!res_nonexistent.0.valid);

        // 5. Negative: Sanctioned client address in session is rejected (DEC-027 Inflow Compliance Gate)
        let binding = crate::validation::sanctioned_evm_addresses();
        let sanctioned_addr = binding.iter().next().unwrap().to_string();
        let session_sanctioned = "0xsession_sanctioned";
        db.insert_signing_session(
            session_sanctioned,
            &com_k_hex,
            amount,
            &sanctioned_addr,
            &k_hex,
        )
        .await
        .unwrap();
        assert!(db
            .confirm_deposit_on_chain(
                session_sanctioned,
                amount,
                &sanctioned_addr,
                "0xtx_sanc",
                102
            )
            .await
            .unwrap());

        let res_sanctioned = handle_reveal(
            State(state.clone()),
            Json(RevealRequest {
                session_id: session_sanctioned.to_string(),
            }),
        )
        .await;
        assert_eq!(res_sanctioned.0.status, "REJECTED");
        assert!(!res_sanctioned.0.valid);
        assert_eq!(res_sanctioned.0.masking_key_hex, None);
        assert!(res_sanctioned.0.message.contains("sanctions policy"));
    }
}
