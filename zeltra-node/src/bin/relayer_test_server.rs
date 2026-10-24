//! DEC-045B Production-Simulation Relayer Server for Concurrency & Integration Auditing

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::Json;
use serde::{Deserialize, Serialize};
use std::env;
use std::path::PathBuf;
use tokio::net::TcpListener;

use zeltra_core::{fr_to_be_bytes, Fr};
use zeltra_node::create_relayer_router;
use zeltra_node::database::{Database, MmrLeafRecord};
use zeltra_node::state::AppState;

#[derive(Deserialize)]
struct AppendLeavesRequest {
    count: u64,
    start_index: u64,
    block_number: u64,
}

#[derive(Serialize)]
struct AppendLeavesResponse {
    new_leaf_count: u64,
    new_root: String,
}

#[derive(Serialize)]
struct TestStatsResponse {
    leaf_count: u64,
    bagged_root: String,
    db_leaf_count: u64,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: relayer_test_server <port> <db_path>");
        std::process::exit(1);
    }

    let port: u16 = args[1].parse().expect("valid port");
    let db_path = PathBuf::from(&args[2]);

    std::env::set_var("ZELTRA_DB_KEY", "relayer-test-encryption-key-32b");

    let db = Database::new(&db_path).await?;
    let state = AppState::new(db, None).await;

    // Build relayer router with standard endpoints + test instrumentation endpoints
    let test_routes = axum::Router::new()
        .route("/test/append_leaves", post(handle_test_append_leaves))
        .route("/test/rollback/{block}", post(handle_test_rollback))
        .route("/test/stats", axum::routing::get(handle_test_stats))
        .with_state(state.clone());

    let app = create_relayer_router(state).merge(test_routes);

    let listener = TcpListener::bind(format!("127.0.0.1:{}", port)).await?;
    println!("RELAYER_READY:http://127.0.0.1:{}", port);

    axum::serve(listener, app).await?;
    Ok(())
}

async fn handle_test_append_leaves(
    State(state): State<AppState>,
    Json(req): Json<AppendLeavesRequest>,
) -> Response {
    let mut tree = state.in_memory_mmr.write().await;
    let mut new_records = Vec::new();

    for i in 0..req.count {
        let leaf_idx = req.start_index + i;
        let fr = Fr::from((leaf_idx + 1) * 1111);
        let cm_hex = format!("0x{}", hex::encode(fr_to_be_bytes(&fr)));

        let (_idx, _root) = tree.append(fr);

        new_records.push(MmrLeafRecord {
            leaf_index: leaf_idx,
            commitment: cm_hex,
            tx_hash: format!("0xpy_tx_{leaf_idx}"),
            block_number: req.block_number,
            encrypted_memo: vec![0xee; 96],
            created_at: None,
        });
    }

    if let Err(e) = state
        .db
        .atomic_append_mmr_leaves_and_state_flat(&new_records, &tree, req.block_number)
        .await
    {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("DB append failed: {}", e),
        )
            .into_response();
    }

    {
        let mut blocks = state.in_memory_leaf_blocks.write().await;
        for _ in 0..req.count {
            blocks.push(req.block_number);
        }
    }
    state
        .last_indexed_block
        .store(req.block_number, std::sync::atomic::Ordering::SeqCst);

    let root_hex = format!("0x{}", hex::encode(fr_to_be_bytes(&tree.get_root())));
    let count = tree.leaf_count();

    (
        StatusCode::OK,
        Json(AppendLeavesResponse {
            new_leaf_count: count,
            new_root: root_hex,
        }),
    )
        .into_response()
}

async fn handle_test_rollback(State(state): State<AppState>, Path(block): Path<u64>) -> Response {
    match state.rollback_mmr_to_block(block).await {
        Ok(_) => {
            let tree = state.in_memory_mmr.read().await;
            (
                StatusCode::OK,
                Json(AppendLeavesResponse {
                    new_leaf_count: tree.leaf_count(),
                    new_root: format!("0x{}", hex::encode(fr_to_be_bytes(&tree.get_root()))),
                }),
            )
                .into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Rollback failed: {}", e),
        )
            .into_response(),
    }
}

async fn handle_test_stats(State(state): State<AppState>) -> Response {
    let (leaf_count, bagged_root) = {
        let tree = state.in_memory_mmr.read().await;
        (
            tree.leaf_count(),
            format!("0x{}", hex::encode(fr_to_be_bytes(&tree.get_root()))),
        )
    };

    let db_leaf_count = state
        .db
        .get_all_mmr_leaves()
        .await
        .map(|l| l.len() as u64)
        .unwrap_or(0);

    (
        StatusCode::OK,
        Json(TestStatsResponse {
            leaf_count,
            bagged_root,
            db_leaf_count,
        }),
    )
        .into_response()
}
