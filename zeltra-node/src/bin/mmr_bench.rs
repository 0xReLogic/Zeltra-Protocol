//! MMR Benchmark CLI & Real-time Server (DEC-044 Empirical Verification)
//!
//! Provides CLI commands for empirical benchmarking:
//! - populate: Batch insert N canonical leaves into SQLCipher encrypted database
//! - bench_load: Measure exact load_mmr_tree() latency vs N
//! - bench_proof: Compare load_mmr_tree() + generate_proof() vs in-memory cached proof
//! - bench_incremental: Compare incremental append vs full tree reload
//! - serve: Run live Axum server exposing both current (/proof/:idx) and cached (/cached-proof/:idx) endpoints

use std::env;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use std::time::Instant;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;
use tokio::net::TcpListener;

use zeltra_core::{fr_to_be_bytes, Fr, MerkleMountainRange};
use zeltra_node::database::Database;
use zeltra_node::dto::{MmrErrorResponse, MmrProofResponse};
use zeltra_node::handlers::mmr::get_mmr_proof;
use zeltra_node::state::AppState;

#[derive(Clone)]
struct BenchServerState {
    app_state: AppState,
    in_memory_mmr: Arc<RwLock<MerkleMountainRange>>,
}

#[derive(Serialize)]
struct BenchResult {
    leaf_count: usize,
    load_time_ms: f64,
    load_time_us: u128,
    proof_gen_time_us: u128,
    total_rebuild_and_proof_ms: f64,
    cached_proof_time_us: u128,
    speedup_factor: f64,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: mmr_bench <command> [args...]");
        eprintln!("Commands:");
        eprintln!("  populate <db_path> <leaf_count>");
        eprintln!("  bench_load <db_path>");
        eprintln!("  bench_proof <db_path> <leaf_index>");
        eprintln!("  bench_incremental <db_path>");
        eprintln!("  serve <db_path> <port>");
        std::process::exit(1);
    }

    if env::var("ZELTRA_DB_KEY").is_err() {
        env::set_var("ZELTRA_DB_KEY", "empirical-benchmark-key-dec044");
    }

    let command = &args[1];
    match command.as_str() {
        "populate" => {
            if args.len() < 4 {
                eprintln!("Usage: mmr_bench populate <db_path> <leaf_count>");
                std::process::exit(1);
            }
            let db_path = PathBuf::from(&args[2]);
            let leaf_count: usize = args[3].parse()?;

            if db_path.exists() {
                let _ = fs::remove_file(&db_path);
            }

            let start = Instant::now();
            let db = Database::new(&db_path).await?;

            let mut mmr = MerkleMountainRange::new();
            let mut leaves_data = Vec::with_capacity(leaf_count);

            for i in 0..leaf_count {
                let fr = Fr::from((i as u64) + 1_000_000u64);
                mmr.append_leaf(fr);
                let hex_cm = format!("0x{}", hex::encode(fr_to_be_bytes(&fr)));
                leaves_data.push((
                    i as u64,
                    hex_cm,
                    "0xbench_tx".to_string(),
                    1000 + (i as u64),
                    vec![0u8; 96],
                ));
            }

            db.batch_insert_mmr_leaves(leaves_data).await?;
            db.save_mmr_state(&mmr, 1000 + (leaf_count as u64)).await?;
            let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;

            println!(
                r#"{{"status":"ok","leaf_count":{},"populate_time_ms":{:.2},"bagged_root":"0x{}"}}"#,
                leaf_count,
                elapsed_ms,
                hex::encode(fr_to_be_bytes(&mmr.get_root()))
            );
        }

        "bench_load" => {
            if args.len() < 3 {
                eprintln!("Usage: mmr_bench bench_load <db_path>");
                std::process::exit(1);
            }
            let db_path = PathBuf::from(&args[2]);
            let db = Database::new(&db_path).await?;

            // Warmup
            let _ = db.load_mmr_tree().await?;

            // 5 iterations measurement
            let mut durations = Vec::new();
            for _ in 0..5 {
                let start = Instant::now();
                let tree = db.load_mmr_tree().await?;
                durations.push((start.elapsed(), tree.leaf_count()));
            }

            let avg_duration: std::time::Duration = durations
                .iter()
                .map(|(d, _)| *d)
                .sum::<std::time::Duration>()
                / 5;
            let leaf_count = durations[0].1;

            println!(
                r#"{{"leaf_count":{},"avg_load_time_ms":{:.3},"avg_load_time_us":{}}}"#,
                leaf_count,
                avg_duration.as_secs_f64() * 1000.0,
                avg_duration.as_micros()
            );
        }

        "bench_proof" => {
            if args.len() < 4 {
                eprintln!("Usage: mmr_bench bench_proof <db_path> <leaf_index>");
                std::process::exit(1);
            }
            let db_path = PathBuf::from(&args[2]);
            let leaf_index: usize = args[3].parse()?;
            let db = Database::new(&db_path).await?;

            // 1. Measure Current Path: load_mmr_tree() + generate_proof()
            let t0 = Instant::now();
            let tree = db.load_mmr_tree().await?;
            let load_elapsed = t0.elapsed();

            let t1 = Instant::now();
            let _proof = tree.generate_proof(leaf_index);
            let proof_elapsed = t1.elapsed();
            let total_current = t0.elapsed();

            // 2. Measure In-Memory Cached Path: generate_proof() directly from pre-loaded tree
            // Warmup
            let _ = tree.generate_proof(leaf_index);
            let iters = 100;
            let t2 = Instant::now();
            for _ in 0..iters {
                let _ = tree.generate_proof(leaf_index);
            }
            let cached_avg_us = (t2.elapsed().as_micros() as f64) / (iters as f64);

            let speedup = total_current.as_micros() as f64 / cached_avg_us.max(1.0);

            let res = BenchResult {
                leaf_count: tree.leaf_count as usize,
                load_time_ms: load_elapsed.as_secs_f64() * 1000.0,
                load_time_us: load_elapsed.as_micros(),
                proof_gen_time_us: proof_elapsed.as_micros(),
                total_rebuild_and_proof_ms: total_current.as_secs_f64() * 1000.0,
                cached_proof_time_us: cached_avg_us as u128,
                speedup_factor: speedup,
            };

            println!("{}", serde_json::to_string(&res)?);
        }

        "bench_incremental" => {
            if args.len() < 3 {
                eprintln!("Usage: mmr_bench bench_incremental <db_path>");
                std::process::exit(1);
            }
            let db_path = PathBuf::from(&args[2]);
            let db = Database::new(&db_path).await?;

            // Measure Full Rebuild + Append (Current indexer pattern)
            let t0 = Instant::now();
            let mut tree_rebuilt = db.load_mmr_tree().await?;
            let new_leaf = Fr::from(999999999u64);
            tree_rebuilt.append_leaf(new_leaf);
            let rebuild_append_us = t0.elapsed().as_micros();

            // Measure Pure Incremental Append on In-Memory State
            let mut in_memory_tree = db.load_mmr_tree().await?;
            let t1 = Instant::now();
            in_memory_tree.append_leaf(new_leaf);
            let pure_append_us = t1.elapsed().as_micros();

            let speedup = (rebuild_append_us as f64) / (pure_append_us.max(1) as f64);

            println!(
                r#"{{"leaf_count":{},"rebuild_and_append_us":{},"pure_incremental_append_us":{},"speedup_factor":{:.2}}}"#,
                tree_rebuilt.leaf_count, rebuild_append_us, pure_append_us, speedup
            );
        }

        "serve" => {
            if args.len() < 4 {
                eprintln!("Usage: mmr_bench serve <db_path> <port>");
                std::process::exit(1);
            }
            let db_path = PathBuf::from(&args[2]);
            let port: u16 = args[3].parse()?;

            let db = Database::new(&db_path).await?;
            let app_state = AppState::new(db.clone(), None).await;

            // Load initial canonical tree into in-memory cache
            println!("Pre-loading canonical MMR tree into in-memory state...");
            let tree = db.load_mmr_tree().await?;
            let initial_count = tree.leaf_count();
            println!("In-memory MMR tree ready with {} leaves", initial_count);

            let bench_state = BenchServerState {
                app_state: app_state.clone(),
                in_memory_mmr: Arc::new(RwLock::new(tree)),
            };

            let app = Router::new()
                // Current path: calls load_mmr_tree() on every request
                .route(
                    "/api/v1/mmr/proof/{leaf_index}",
                    get(get_mmr_proof).with_state(app_state),
                )
                // Proposed DEC-044 path: serves directly from in-memory cache
                .route(
                    "/api/v1/mmr/cached-proof/{leaf_index}",
                    get(handle_cached_mmr_proof).with_state(bench_state),
                );

            let listener = TcpListener::bind(format!("127.0.0.1:{}", port)).await?;
            println!("BENCH_SERVER_READY:http://127.0.0.1:{}", port);

            axum::serve(listener, app).await?;
        }

        _ => {
            eprintln!("Unknown command: {}", command);
            std::process::exit(1);
        }
    }

    Ok(())
}

/// Handler for the proposed DEC-044 cached in-memory proof serving
async fn handle_cached_mmr_proof(
    State(state): State<BenchServerState>,
    Path(leaf_index): Path<u64>,
) -> Response {
    // 1. Read directly from in-memory MMR with zero disk read and zero tree rebuild
    let (proof, leaf_count, bagged_root) = {
        let tree = state.in_memory_mmr.read().unwrap();
        let leaf_count = tree.leaf_count();
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
        let proof = tree.generate_proof(leaf_index as usize);
        let bagged_root = format!("0x{}", hex::encode(fr_to_be_bytes(&tree.bagged_root())));
        (proof, leaf_count, bagged_root)
    };

    // 2. Query leaf record metadata (cached or point lookup)
    let leaf_record = match state.app_state.db.get_mmr_leaf(leaf_index).await {
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
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(MmrErrorResponse {
                    code: "InternalStorageError".to_string(),
                    message: e.to_string(),
                }),
            )
                .into_response();
        }
    };

    let mountain_siblings: Vec<String> = proof
        .mountain_siblings
        .iter()
        .map(|s| format!("0x{}", hex::encode(fr_to_be_bytes(s))))
        .collect();

    let peak_bagging_siblings: Vec<String> = proof
        .peak_bagging_siblings
        .iter()
        .map(|s| format!("0x{}", hex::encode(fr_to_be_bytes(s))))
        .collect();

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
