//! Handlers for MMR Client Sync Endpoints (DEC-035C)
//!
//! - `GET /api/v1/mmr/proof/{leaf_index}`: Authenticated MMR inclusion proof and peak bagging siblings.
//! - `GET /api/v1/mmr/tip`: Lightweight current tree state and tip accumulator.
//!
//! Enforces strict Hyperbridge (2026) out-of-bounds upper bound checks (INV-4)
//! and strict zero-knowledge privacy boundaries (INV-3: only public integer index accepted).

use crate::dto::{MmrErrorResponse, MmrProofResponse, MmrTipResponse};
use crate::state::AppState;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};

/// Retrieve cryptographic MMR inclusion proof for a given leaf index (DEC-035C Section 5.C)
pub async fn handle_mmr_proof(
    State(state): State<AppState>,
    Path(leaf_index): Path<u64>,
) -> Response {
    // INV-3 (Zero-Knowledge Privacy Preservation): Log ONLY the public integer leaf position
    println!(
        "MMR_API: Inclusion proof requested for leaf_index: {}",
        leaf_index
    );

    // 1. Reconstruct current canonical MMR tree from storage
    let tree = match state.db.load_mmr_tree().await {
        Ok(t) => t,
        Err(e) => {
            eprintln!("MMR_API ERROR: Failed to load MMR tree: {:?}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(MmrErrorResponse {
                    code: "InternalStorageError".to_string(),
                    message: "Failed to load MMR tree mirror from database".to_string(),
                }),
            )
                .into_response();
        }
    };

    let leaf_count = tree.leaf_count();

    // 2. Strict Upper Bound Check (DEC-035C INV-4 / Hyperbridge Exploit Defense)
    // Leaf index must be strictly less than leaf_count
    if leaf_count == 0 || leaf_index >= leaf_count {
        return (
            StatusCode::NOT_FOUND,
            Json(MmrErrorResponse {
                code: "LeafIndexOutOfBounds".to_string(),
                message: format!(
                    "leaf_index {} exceeds current leaf_count {}",
                    leaf_index, leaf_count
                ),
            }),
        )
            .into_response();
    }

    // 3. Query leaf metadata (commitment, block_number) from persistent storage
    let leaf_record = match state.db.get_mmr_leaf(leaf_index).await {
        Ok(Some(r)) => r,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(MmrErrorResponse {
                    code: "LeafNotFound".to_string(),
                    message: format!("leaf_index {} record not found in storage", leaf_index),
                }),
            )
                .into_response();
        }
        Err(e) => {
            eprintln!(
                "MMR_API ERROR: Failed to query mmr_leaf {}: {:?}",
                leaf_index, e
            );
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(MmrErrorResponse {
                    code: "InternalStorageError".to_string(),
                    message: "Database query failed".to_string(),
                }),
            )
                .into_response();
        }
    };

    // 4. Generate MMR inclusion proof from canonical in-memory mirror
    let proof = tree.generate_proof(leaf_index as usize);

    // 5. Serialize siblings to 0x-prefixed 32-byte hex strings
    let mountain_siblings: Vec<String> = proof
        .mountain_siblings
        .iter()
        .map(|s| format!("0x{}", hex::encode(nimbus_core::fr_to_be_bytes(s))))
        .collect();

    let peak_bagging_siblings: Vec<String> = proof
        .peak_bagging_siblings
        .iter()
        .map(|s| format!("0x{}", hex::encode(nimbus_core::fr_to_be_bytes(s))))
        .collect();

    let bagged_root = format!(
        "0x{}",
        hex::encode(nimbus_core::fr_to_be_bytes(&tree.bagged_root()))
    );

    (
        StatusCode::OK,
        Json(MmrProofResponse {
            leaf_index,
            leaf_count,
            commitment: leaf_record.commitment,
            mountain_height: proof.mountain_height,
            mountain_siblings,
            peak_bagging_siblings,
            bagged_root,
            block_number: leaf_record.block_number,
        }),
    )
        .into_response()
}

/// Retrieve current MMR tip status and root (DEC-035C Section 5.C)
pub async fn handle_mmr_tip(State(state): State<AppState>) -> Response {
    let state_opt = match state.db.get_mmr_state().await {
        Ok(s) => s,
        Err(e) => {
            eprintln!("MMR_API ERROR: Failed to get MMR state: {:?}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(MmrErrorResponse {
                    code: "InternalStorageError".to_string(),
                    message: "Failed to read MMR state".to_string(),
                }),
            )
                .into_response();
        }
    };

    match state_opt {
        Some(s) => (
            StatusCode::OK,
            Json(MmrTipResponse {
                leaf_count: s.leaf_count,
                bagged_root: s.bagged_root,
                last_indexed_block: s.last_indexed_block,
            }),
        )
            .into_response(),
        None => (
            StatusCode::OK,
            Json(MmrTipResponse {
                leaf_count: 0,
                bagged_root: "0x0000000000000000000000000000000000000000000000000000000000000000"
                    .to_string(),
                last_indexed_block: 0,
            }),
        )
            .into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::Database;
    use tempfile::TempDir;

    async fn setup_test_state() -> (AppState, TempDir) {
        std::env::set_var("NIMBUS_DB_KEY", "test-encryption-key");
        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("mmr_api_test.db");
        let db = Database::new(&db_path).await.unwrap();

        let state = AppState::new(
            db,
            nimbus_core::Fr::from(1u64),
            1,
            nimbus_core::IssuerPublicKey(nimbus_core::G2Projective::default()),
            std::collections::HashMap::new(),
            None,
        )
        .await;

        (state, tmp)
    }

    #[tokio::test]
    async fn test_mmr_tip_empty_and_populated() {
        let (state, _tmp) = setup_test_state().await;

        // 1. Tip on empty tree
        let resp = handle_mmr_tip(State(state.clone())).await;
        assert_eq!(resp.status(), StatusCode::OK);
        let body_bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let tip: MmrTipResponse = serde_json::from_slice(&body_bytes).unwrap();
        assert_eq!(tip.leaf_count, 0);

        // 2. Insert leaf and checkpoint state
        let fr = nimbus_core::Fr::from(777u64);
        let hex_cm = format!("0x{}", hex::encode(nimbus_core::fr_to_be_bytes(&fr)));
        state
            .db
            .insert_mmr_leaf(0, &hex_cm, "0xtx0", 123)
            .await
            .unwrap();

        let mut mmr = nimbus_core::MerkleMountainRange::new();
        mmr.append_leaf(fr);
        state.db.save_mmr_state(&mmr, 123).await.unwrap();

        let resp2 = handle_mmr_tip(State(state.clone())).await;
        assert_eq!(resp2.status(), StatusCode::OK);
        let body_bytes2 = axum::body::to_bytes(resp2.into_body(), usize::MAX)
            .await
            .unwrap();
        let tip2: MmrTipResponse = serde_json::from_slice(&body_bytes2).unwrap();
        assert_eq!(tip2.leaf_count, 1);
        assert_eq!(tip2.last_indexed_block, 123);
        let expected_root = format!(
            "0x{}",
            hex::encode(nimbus_core::fr_to_be_bytes(&mmr.bagged_root()))
        );
        assert_eq!(
            tip2.bagged_root.to_lowercase(),
            expected_root.to_lowercase()
        );
    }

    #[tokio::test]
    async fn test_mmr_proof_valid_generation_and_verification() {
        let (state, _tmp) = setup_test_state().await;

        // Insert 3 leaves
        let fr0 = nimbus_core::Fr::from(100u64);
        let fr1 = nimbus_core::Fr::from(200u64);
        let fr2 = nimbus_core::Fr::from(300u64);

        let hex0 = format!("0x{}", hex::encode(nimbus_core::fr_to_be_bytes(&fr0)));
        let hex1 = format!("0x{}", hex::encode(nimbus_core::fr_to_be_bytes(&fr1)));
        let hex2 = format!("0x{}", hex::encode(nimbus_core::fr_to_be_bytes(&fr2)));

        state
            .db
            .insert_mmr_leaf(0, &hex0, "0xtx0", 500)
            .await
            .unwrap();
        state
            .db
            .insert_mmr_leaf(1, &hex1, "0xtx1", 501)
            .await
            .unwrap();
        state
            .db
            .insert_mmr_leaf(2, &hex2, "0xtx2", 502)
            .await
            .unwrap();

        let mut mmr = nimbus_core::MerkleMountainRange::new();
        mmr.append_leaf(fr0);
        mmr.append_leaf(fr1);
        mmr.append_leaf(fr2);
        state.db.save_mmr_state(&mmr, 502).await.unwrap();

        // Query leaf 0 proof
        let resp = handle_mmr_proof(State(state.clone()), Path(0)).await;
        assert_eq!(resp.status(), StatusCode::OK);
        let body_bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let proof_resp: MmrProofResponse = serde_json::from_slice(&body_bytes).unwrap();

        assert_eq!(proof_resp.leaf_index, 0);
        assert_eq!(proof_resp.leaf_count, 3);
        assert_eq!(proof_resp.mountain_height, 1);
        assert_eq!(proof_resp.mountain_siblings.len(), 1);
        assert_eq!(proof_resp.peak_bagging_siblings.len(), 1);

        // Verify cryptographic validity of proof using MerkleMountainRange::verify_proof
        let m_sibs: Vec<nimbus_core::Fr> = proof_resp
            .mountain_siblings
            .iter()
            .map(|h| {
                let bytes: [u8; 32] = hex::decode(h.trim_start_matches("0x"))
                    .unwrap()
                    .try_into()
                    .unwrap();
                nimbus_core::from_evm_scalar(&bytes).unwrap()
            })
            .collect();

        let p_sibs: Vec<nimbus_core::Fr> = proof_resp
            .peak_bagging_siblings
            .iter()
            .map(|h| {
                let bytes: [u8; 32] = hex::decode(h.trim_start_matches("0x"))
                    .unwrap()
                    .try_into()
                    .unwrap();
                nimbus_core::from_evm_scalar(&bytes).unwrap()
            })
            .collect();

        let mmr_proof = nimbus_core::MMRProof {
            mountain_height: proof_resp.mountain_height,
            mountain_siblings: m_sibs,
            peak_bagging_siblings: p_sibs,
            peak_index: 0,
        };

        let is_valid =
            nimbus_core::MerkleMountainRange::verify_proof(fr0, 0, 3, &mmr_proof, mmr.get_root());
        assert!(
            is_valid,
            "Cryptographic MMR inclusion proof must verify true"
        );
    }

    #[tokio::test]
    async fn test_mmr_proof_hyperbridge_out_of_bounds_rejected() {
        let (state, _tmp) = setup_test_state().await;

        // Tree with exactly 2 leaves (leaf_index 0 and 1)
        let fr0 = nimbus_core::Fr::from(10u64);
        let fr1 = nimbus_core::Fr::from(20u64);
        let hex0 = format!("0x{}", hex::encode(nimbus_core::fr_to_be_bytes(&fr0)));
        let hex1 = format!("0x{}", hex::encode(nimbus_core::fr_to_be_bytes(&fr1)));

        state
            .db
            .insert_mmr_leaf(0, &hex0, "0xtx0", 10)
            .await
            .unwrap();
        state
            .db
            .insert_mmr_leaf(1, &hex1, "0xtx1", 11)
            .await
            .unwrap();

        let mut mmr = nimbus_core::MerkleMountainRange::new();
        mmr.append_leaf(fr0);
        mmr.append_leaf(fr1);
        state.db.save_mmr_state(&mmr, 11).await.unwrap();

        // Hyperbridge Test: Query leaf_index = 2 on a 2-leaf tree (leaf_count = 2)
        // Strictly must return 404 Not Found with LeafIndexOutOfBounds
        let resp = handle_mmr_proof(State(state.clone()), Path(2)).await;
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
        let body_bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let err: MmrErrorResponse = serde_json::from_slice(&body_bytes).unwrap();
        assert_eq!(err.code, "LeafIndexOutOfBounds");

        // Extreme Hyperbridge Test: Query leaf_index = 999
        let resp2 = handle_mmr_proof(State(state.clone()), Path(999)).await;
        assert_eq!(resp2.status(), StatusCode::NOT_FOUND);
        let body_bytes2 = axum::body::to_bytes(resp2.into_body(), usize::MAX)
            .await
            .unwrap();
        let err2: MmrErrorResponse = serde_json::from_slice(&body_bytes2).unwrap();
        assert_eq!(err2.code, "LeafIndexOutOfBounds");
    }
}
