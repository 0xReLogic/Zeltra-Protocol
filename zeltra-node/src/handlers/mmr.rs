//! Handlers for MMR Client Sync Endpoints (DEC-035C)
//!
//! - `GET /api/v1/mmr/proof/{leaf_index}`: Authenticated MMR inclusion proof and peak bagging siblings.
//! - `GET /api/v1/mmr/tip`: Lightweight current tree state and tip accumulator.
//!
//! Enforces strict Hyperbridge (2026) out-of-bounds upper bound checks (INV-4)
//! and strict zero-knowledge privacy boundaries (INV-3: only public integer index accepted).

use crate::dto::{MmrErrorResponse, MmrProofResponse, MmrTipResponse, NoteSyncItem, NoteSyncQuery};
use crate::state::AppState;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};

pub use handle_mmr_proof as get_mmr_proof;
pub use handle_notes_sync as get_bulk_leaves;

/// Retrieve cryptographic MMR inclusion proof for a given leaf index (DEC-035C & DEC-045B)
///
/// Serving latency < 100 µs via reader lock on in-memory FlatMerkleMountainRange (INV-45B-1).
pub async fn handle_mmr_proof(
    State(state): State<AppState>,
    Path(leaf_index): Path<u64>,
) -> Response {
    // 1. Ekstraksi proof langsung dari in-memory state via read-lock (Zero Disk I/O & Zero Poseidon Hash)
    let (proof, leaf_count, bagged_root, commitment_hex, block_number) = {
        let tree = state.in_memory_mmr.read().await;
        let count = tree.leaf_count();

        // 2. Strict Upper Bound Check (DEC-035C INV-4 / Hyperbridge Exploit Defense)
        // Leaf index must be strictly less than leaf_count
        if count == 0 || leaf_index >= count {
            return (
                StatusCode::NOT_FOUND,
                Json(MmrErrorResponse {
                    code: "LeafIndexOutOfBounds".to_string(),
                    message: format!(
                        "leaf_index {} exceeds current leaf_count {}",
                        leaf_index, count
                    ),
                }),
            )
                .into_response();
        }

        let p = match tree.generate_proof(leaf_index as usize) {
            Ok(proof) => proof,
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(MmrErrorResponse {
                        code: "ProofGenerationError".to_string(),
                        message: e.to_string(),
                    }),
                )
                    .into_response();
            }
        };

        let root_hex = format!(
            "0x{}",
            hex::encode(zeltra_core::fr_to_be_bytes(&tree.get_root()))
        );
        let commitment_fr = tree.get_leaf(leaf_index as usize).unwrap_or_default();
        let cm_hex = format!(
            "0x{}",
            hex::encode(zeltra_core::fr_to_be_bytes(&commitment_fr))
        );

        let blk = {
            let blocks = state.in_memory_leaf_blocks.read().await;
            blocks.get(leaf_index as usize).copied().unwrap_or(0)
        };

        (p, count, root_hex, cm_hex, blk)
        // Read-lock otomatis lepas di sini (< 5 µs - INV-45B-1)
    };

    // 3. Format DTO respons JSON
    let mountain_siblings: Vec<String> = proof
        .mountain_siblings
        .iter()
        .map(|s| format!("0x{}", hex::encode(zeltra_core::fr_to_be_bytes(s))))
        .collect();

    let peak_bagging_siblings: Vec<String> = proof
        .peak_bagging_siblings
        .iter()
        .map(|s| format!("0x{}", hex::encode(zeltra_core::fr_to_be_bytes(s))))
        .collect();

    (
        StatusCode::OK,
        Json(MmrProofResponse {
            leaf_index,
            leaf_count,
            commitment: commitment_hex,
            mountain_height: proof.mountain_height,
            mountain_siblings,
            peak_bagging_siblings,
            bagged_root,
            block_number,
        }),
    )
        .into_response()
}

/// Retrieve current MMR tip status and root (DEC-035C & DEC-045B)
///
/// Fast-path: Reads tip state directly from in-memory FlatMerkleMountainRange with read-lock.
pub async fn handle_mmr_tip(State(state): State<AppState>) -> Response {
    let (leaf_count, bagged_root) = {
        let tree = state.in_memory_mmr.read().await;
        let count = tree.leaf_count();
        if count > 0 {
            (
                count,
                format!(
                    "0x{}",
                    hex::encode(zeltra_core::fr_to_be_bytes(&tree.get_root()))
                ),
            )
        } else {
            (
                0,
                "0x0000000000000000000000000000000000000000000000000000000000000000".to_string(),
            )
        }
    };

    let mut last_indexed_block = state
        .last_indexed_block
        .load(std::sync::atomic::Ordering::Relaxed);
    if last_indexed_block == 0 {
        if let Ok(Some(b)) = state.db.get_mmr_last_indexed_block().await {
            last_indexed_block = b;
            state
                .last_indexed_block
                .store(b, std::sync::atomic::Ordering::Relaxed);
        }
    }

    (
        StatusCode::OK,
        Json(MmrTipResponse {
            leaf_count,
            bagged_root,
            last_indexed_block,
        }),
    )
        .into_response()
}

/// Paginated Bulk Note Sync endpoint for client recovery (DEC-037).
///
/// `GET /api/v1/notes/sync?from_leaf={u64}&limit={u64}`
///
/// Default limit: 1000, max: 5000.
pub async fn handle_notes_sync(
    State(state): State<AppState>,
    Query(query): Query<NoteSyncQuery>,
) -> Response {
    let limit = query.limit.unwrap_or(1000).clamp(1, 5000);
    match state.db.get_notes_memo_chunk(query.from_leaf, limit).await {
        Ok(rows) => {
            let items: Vec<NoteSyncItem> = rows
                .into_iter()
                .map(|r| NoteSyncItem {
                    leaf_index: r.leaf_index,
                    commitment: r.commitment,
                    encrypted_memo: r.encrypted_memo,
                    block_number: r.block_number,
                })
                .collect();
            Json(items).into_response()
        }
        Err(e) => {
            eprintln!("NOTES_SYNC ERROR: Failed to query memo chunk: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(MmrErrorResponse {
                    code: "InternalStorageError".to_string(),
                    message: "Database query failed for notes sync".to_string(),
                }),
            )
                .into_response()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::Database;
    use tempfile::TempDir;

    async fn setup_test_state() -> (AppState, TempDir) {
        std::env::set_var("ZELTRA_DB_KEY", "test-encryption-key");
        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("mmr_api_test.db");
        let db = Database::new(&db_path).await.unwrap();

        let state = AppState::new(db, None).await;

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
        let fr = zeltra_core::Fr::from(777u64);
        let hex_cm = format!("0x{}", hex::encode(zeltra_core::fr_to_be_bytes(&fr)));
        state
            .db
            .insert_mmr_leaf(0, &hex_cm, "0xtx0", 123, &[])
            .await
            .unwrap();

        let mut mmr = zeltra_core::MerkleMountainRange::new();
        mmr.append_leaf(fr);
        state.db.save_mmr_state(&mmr, 123).await.unwrap();
        state.in_memory_mmr.write().await.append(fr);

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
            hex::encode(zeltra_core::fr_to_be_bytes(&mmr.bagged_root()))
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
        let fr0 = zeltra_core::Fr::from(100u64);
        let fr1 = zeltra_core::Fr::from(200u64);
        let fr2 = zeltra_core::Fr::from(300u64);

        let hex0 = format!("0x{}", hex::encode(zeltra_core::fr_to_be_bytes(&fr0)));
        let hex1 = format!("0x{}", hex::encode(zeltra_core::fr_to_be_bytes(&fr1)));
        let hex2 = format!("0x{}", hex::encode(zeltra_core::fr_to_be_bytes(&fr2)));

        state
            .db
            .insert_mmr_leaf(0, &hex0, "0xtx0", 500, &[])
            .await
            .unwrap();
        state
            .db
            .insert_mmr_leaf(1, &hex1, "0xtx1", 501, &[])
            .await
            .unwrap();
        state
            .db
            .insert_mmr_leaf(2, &hex2, "0xtx2", 502, &[])
            .await
            .unwrap();

        let mut mmr = zeltra_core::MerkleMountainRange::new();
        mmr.append_leaf(fr0);
        mmr.append_leaf(fr1);
        mmr.append_leaf(fr2);
        state.db.save_mmr_state(&mmr, 502).await.unwrap();
        {
            let mut tree = state.in_memory_mmr.write().await;
            tree.append(fr0);
            tree.append(fr1);
            tree.append(fr2);
        }

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
        let m_sibs: Vec<zeltra_core::Fr> = proof_resp
            .mountain_siblings
            .iter()
            .map(|h| {
                let bytes: [u8; 32] = hex::decode(h.trim_start_matches("0x"))
                    .unwrap()
                    .try_into()
                    .unwrap();
                zeltra_core::from_evm_scalar(&bytes).unwrap()
            })
            .collect();

        let p_sibs: Vec<zeltra_core::Fr> = proof_resp
            .peak_bagging_siblings
            .iter()
            .map(|h| {
                let bytes: [u8; 32] = hex::decode(h.trim_start_matches("0x"))
                    .unwrap()
                    .try_into()
                    .unwrap();
                zeltra_core::from_evm_scalar(&bytes).unwrap()
            })
            .collect();

        let mmr_proof = zeltra_core::MMRProof {
            mountain_height: proof_resp.mountain_height,
            mountain_siblings: m_sibs,
            peak_bagging_siblings: p_sibs,
            peak_index: 0,
        };

        let is_valid =
            zeltra_core::MerkleMountainRange::verify_proof(fr0, 0, 3, &mmr_proof, mmr.get_root());
        assert!(
            is_valid,
            "Cryptographic MMR inclusion proof must verify true"
        );
    }

    #[tokio::test]
    async fn test_mmr_proof_hyperbridge_out_of_bounds_rejected() {
        let (state, _tmp) = setup_test_state().await;

        // Tree with exactly 2 leaves (leaf_index 0 and 1)
        let fr0 = zeltra_core::Fr::from(10u64);
        let fr1 = zeltra_core::Fr::from(20u64);
        let hex0 = format!("0x{}", hex::encode(zeltra_core::fr_to_be_bytes(&fr0)));
        let hex1 = format!("0x{}", hex::encode(zeltra_core::fr_to_be_bytes(&fr1)));

        state
            .db
            .insert_mmr_leaf(0, &hex0, "0xtx0", 10, &[])
            .await
            .unwrap();
        state
            .db
            .insert_mmr_leaf(1, &hex1, "0xtx1", 11, &[])
            .await
            .unwrap();

        let mut mmr = zeltra_core::MerkleMountainRange::new();
        mmr.append_leaf(fr0);
        mmr.append_leaf(fr1);
        state.db.save_mmr_state(&mmr, 11).await.unwrap();
        {
            let mut tree = state.in_memory_mmr.write().await;
            tree.append(fr0);
            tree.append(fr1);
        }

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

    #[tokio::test]
    async fn test_handle_notes_sync_endpoint() {
        let (state, _tmp) = setup_test_state().await;

        for i in 0..5u64 {
            let cm = format!("0x{:064x}", i + 1);
            let memo = vec![0x77; 96];
            state
                .db
                .insert_mmr_leaf(i, &cm, &format!("0xtx_{i}"), 200 + i, &memo)
                .await
                .unwrap();
        }

        let query = NoteSyncQuery {
            from_leaf: 1,
            limit: Some(3),
        };
        let resp = handle_notes_sync(State(state.clone()), Query(query)).await;
        assert_eq!(resp.status(), StatusCode::OK);

        let body_bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let items: Vec<NoteSyncItem> = serde_json::from_slice(&body_bytes).unwrap();

        assert_eq!(items.len(), 3);
        assert_eq!(items[0].leaf_index, 1);
        assert_eq!(items[1].leaf_index, 2);
        assert_eq!(items[2].leaf_index, 3);
        assert!(items[0].encrypted_memo.starts_with("0x7777"));
    }
}
