//! Zeltra Relayer Node Library
//!
//! This library exposes internal modules for testing purposes.

pub mod batch_processor;
pub mod config;
pub mod database;
pub mod dto;
pub mod evm_client;
pub mod execution_fee_claimer;
pub mod handlers;
pub mod mempool_watchdog;
pub mod mmr_indexer;
pub mod router;
pub mod sanctions_sync;
pub mod state;
pub mod validation;

pub use router::create_relayer_router;
