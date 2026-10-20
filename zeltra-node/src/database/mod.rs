//! Production-grade SQLite database for Zeltra Relayer Node
//!
//! Based on 2026 best practices from:
//! - SQLite Renaissance (WAL mode, PRAGMA tuning)
//! - ZK Rollup nullifier tracking patterns (Nullmask, Umbra, Anubis)
//! - Rust async/blocking patterns (tokio spawn_blocking)

use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use tokio::task;

pub mod ccip;
pub mod idempotency;
pub mod mmr;
pub mod nullifiers;
pub mod quotes;
pub mod relayer_tx;
pub mod spends;
pub mod types;

#[cfg(test)]
mod tests;

#[allow(unused_imports)]
pub use mmr::{MMRStore, SqliteMmrStore};
pub use types::*;

/// Production SQLite database with optimized PRAGMAs
#[derive(Clone)]
pub struct Database {
    pub(crate) path: String,
}

pub(crate) const DEFAULT_DB_KEY: &str = "default-change-in-production";

pub(crate) fn database_key() -> Result<String> {
    match std::env::var("ZELTRA_DB_KEY") {
        Ok(value) if !value.trim().is_empty() && value != DEFAULT_DB_KEY => Ok(value),
        _ if crate::config::runtime_mode().is_strict() => {
            anyhow::bail!(
                "ZELTRA_DB_KEY must be explicitly configured in {} mode",
                crate::config::runtime_mode().label()
            )
        }
        _ => Ok(DEFAULT_DB_KEY.to_string()),
    }
}

pub(crate) fn open_connection(path: &str, key: &str) -> Result<Connection> {
    let conn = Connection::open(path)?;
    conn.pragma_update(None, "key", key)?;
    // SQLCipher default KDF iteration count is 256,000, which takes ~2.3 seconds per connection open on VM.
    // Reducing it to 1000 iteration rounds for testing avoids HTTP request hang and allows proper batching concurrency.
    conn.pragma_update(None, "kdf_iter", "1000")?;
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    Ok(conn)
}

pub(crate) fn unix_timestamp() -> Result<i64> {
    Ok(std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs() as i64)
}

impl Database {
    pub async fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path_str = path.as_ref().to_string_lossy().to_string();
        let path_clone = path_str.clone();

        // Get encryption key from environment variable
        // TODO: In production, integrate with KMS (OpenBao/Vault) instead of environment variables
        // See kms.rs for KMS integration pattern
        let db_key = database_key()?;
        let db_key_clone = db_key.clone();

        // Initialize schema and PRAGMAs in blocking context
        task::spawn_blocking(move || -> Result<()> {
            let conn = Connection::open(&path_clone)?;

            // CRITICAL: Set encryption key (SQLCipher)
            conn.pragma_update(None, "key", &db_key_clone)?;
            conn.pragma_update(None, "kdf_iter", "1000")?;

            // CRITICAL: Production PRAGMAs (2026 best practices)
            conn.execute_batch(
                "
                -- Write-Ahead Logging for concurrent reads
                PRAGMA journal_mode = WAL;

                -- Balance durability vs performance
                PRAGMA synchronous = NORMAL;

                -- 64MB cache for better performance
                PRAGMA cache_size = -64000;

                -- Enforce referential integrity
                PRAGMA foreign_keys = ON;

                -- Temp tables in memory
                PRAGMA temp_store = MEMORY;

                -- 5 second busy timeout for concurrent access
                PRAGMA busy_timeout = 5000;

                -- Secure delete for sensitive data
                PRAGMA secure_delete = ON;
            ",
            )?;

            // Create tables
            conn.execute_batch(
                "


                -- Indexer state tracking (DEC-018 / On-chain Event Indexer)
                CREATE TABLE IF NOT EXISTS indexer_state (
                    key TEXT PRIMARY KEY,
                    last_block INTEGER NOT NULL,
                    updated_at INTEGER NOT NULL
                );

                -- Nullifier tracking (double-spend prevention)
                -- Inspired by ZK Rollup nullifier registries
                CREATE TABLE IF NOT EXISTS nullifiers (
                    nullifier TEXT PRIMARY KEY,
                    spent_at INTEGER NOT NULL,
                    tx_hash TEXT
                );
                CREATE INDEX IF NOT EXISTS idx_nullifiers_spent ON nullifiers(spent_at);

                -- Fast Nullifier Cache (DEC-038C / Anti-Spam & Preflight)
                CREATE TABLE IF NOT EXISTS nullifier_cache (
                    nullifier TEXT PRIMARY KEY,
                    spent_at_block INTEGER,
                    first_seen_at INTEGER NOT NULL
                );
                CREATE INDEX IF NOT EXISTS idx_nullifier_cache_seen ON nullifier_cache(first_seen_at);

                -- Spend queue (batching optimization)
                CREATE TABLE IF NOT EXISTS spend_queue (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    nullifier TEXT NOT NULL,
                    sig_hex TEXT NOT NULL,
                    recipient TEXT NOT NULL,
                    amount INTEGER NOT NULL,
                    created_at INTEGER NOT NULL,
                    processed BOOLEAN NOT NULL DEFAULT 0,
                    tx_hash TEXT
                );
                CREATE INDEX IF NOT EXISTS idx_queue_processed ON spend_queue(processed);

                -- Idempotency cache (request deduplication)
                CREATE TABLE IF NOT EXISTS idempotency_cache (
                    idempotency_key TEXT PRIMARY KEY,
                    endpoint TEXT NOT NULL,
                    response_json TEXT NOT NULL,
                    created_at INTEGER NOT NULL
                );
                CREATE INDEX IF NOT EXISTS idx_idempotency_created ON idempotency_cache(created_at);

                -- Batch metadata tracking (Finding #10 / Margin Tracking)
                CREATE TABLE IF NOT EXISTS spend_batches (
                    batch_id TEXT PRIMARY KEY,
                    item_count INTEGER NOT NULL,
                    tx_hash TEXT NOT NULL,
                    gas_used INTEGER NOT NULL,
                    effective_gas_price INTEGER NOT NULL,
                    total_cost_eth REAL NOT NULL,
                    margin_usdc REAL NOT NULL,
                    created_at INTEGER NOT NULL,
                    execution_fee_usdc INTEGER NOT NULL DEFAULT 0,
                    claimed INTEGER NOT NULL DEFAULT 0,
                    claimed_at INTEGER
                );
                CREATE INDEX IF NOT EXISTS idx_batches_created ON spend_batches(created_at);
                CREATE INDEX IF NOT EXISTS idx_batches_claimed ON spend_batches(claimed);

                -- Quote ID tracking (EIP-712 replay protection)
                CREATE TABLE IF NOT EXISTS quote_ids_used (
                    quote_id TEXT PRIMARY KEY,
                    user_address TEXT NOT NULL,
                    used_at INTEGER NOT NULL
                );
                CREATE INDEX IF NOT EXISTS idx_quote_ids_used_at ON quote_ids_used(used_at);

                -- Relayer transactions tracking (DEC-017 / Receipt Finality & Nonce Tracking)
                CREATE TABLE IF NOT EXISTS relayer_transactions (
                    tx_hash TEXT PRIMARY KEY,
                    nonce INTEGER NOT NULL,
                    signer_address TEXT NOT NULL DEFAULT '',
                    status TEXT NOT NULL,
                    block_number INTEGER,
                    gas_used INTEGER,
                    effective_gas_price TEXT,
                    confirmations INTEGER NOT NULL DEFAULT 0,
                    error_reason TEXT,
                    created_at INTEGER NOT NULL,
                    updated_at INTEGER NOT NULL
                );
                CREATE INDEX IF NOT EXISTS idx_relayer_tx_status ON relayer_transactions(status);
                CREATE INDEX IF NOT EXISTS idx_relayer_tx_nonce ON relayer_transactions(nonce);
                CREATE INDEX IF NOT EXISTS idx_relayer_tx_signer ON relayer_transactions(signer_address);

                -- MMR note commitment leaves (DEC-035C / DEC-037 / Merkle Mountain Range)
                CREATE TABLE IF NOT EXISTS mmr_leaves (
                    leaf_index INTEGER PRIMARY KEY,
                    commitment TEXT NOT NULL,
                    tx_hash TEXT NOT NULL,
                    block_number INTEGER NOT NULL,
                    encrypted_memo BLOB NOT NULL DEFAULT X'',
                    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
                );
                CREATE INDEX IF NOT EXISTS idx_mmr_leaves_block ON mmr_leaves(block_number);

                -- MMR state checkpoint (DEC-035C)
                CREATE TABLE IF NOT EXISTS mmr_state (
                    id INTEGER PRIMARY KEY CHECK (id = 1),
                    leaf_count INTEGER NOT NULL,
                    bagged_root TEXT NOT NULL,
                    peaks_json TEXT NOT NULL,
                    last_indexed_block INTEGER NOT NULL,
                    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
                );

                -- MMR post-order nodes storage (DEC-045B / Generic Storage Abstraction)
                CREATE TABLE IF NOT EXISTS mmr_nodes (
                    pos INTEGER PRIMARY KEY,
                    hash TEXT NOT NULL
                );
                CREATE INDEX IF NOT EXISTS idx_mmr_nodes_pos ON mmr_nodes(pos);
            ",
            )?;

            // Run migration for existing databases (fails silently if column already exists)
            let _ = conn.execute(
                "ALTER TABLE mmr_leaves ADD COLUMN encrypted_memo BLOB NOT NULL DEFAULT X''",
                params![],
            );
            let queue_migrations = [
                "ALTER TABLE spend_queue ADD COLUMN request_json TEXT",
                "ALTER TABLE spend_queue ADD COLUMN status TEXT NOT NULL DEFAULT 'queued'",
                "ALTER TABLE spend_queue ADD COLUMN retry_count INTEGER NOT NULL DEFAULT 0",
                "ALTER TABLE spend_queue ADD COLUMN last_error TEXT",
                "ALTER TABLE spend_queue ADD COLUMN available_at INTEGER NOT NULL DEFAULT 0",
                "ALTER TABLE spend_queue ADD COLUMN lease_until INTEGER",
                "ALTER TABLE spend_queue ADD COLUMN updated_at INTEGER NOT NULL DEFAULT 0",
                "ALTER TABLE spend_queue ADD COLUMN block_number INTEGER",
                // CCIP destination tracking columns
                "ALTER TABLE spend_queue ADD COLUMN ccip_message_id TEXT",
                "ALTER TABLE spend_queue ADD COLUMN destination_chain_selector INTEGER",
                "ALTER TABLE spend_queue ADD COLUMN destination_status TEXT NOT NULL DEFAULT 'none'",
                "ALTER TABLE spend_queue ADD COLUMN destination_error TEXT",
                "ALTER TABLE spend_queue ADD COLUMN destination_updated_at INTEGER",
            ];
            for migration in queue_migrations {
                let _ = conn.execute(migration, params![]);
            }
            // Execution fee tracking migrations for spend_batches
            let batch_migrations = [
                "ALTER TABLE spend_batches ADD COLUMN execution_fee_usdc INTEGER NOT NULL DEFAULT 0",
                "ALTER TABLE spend_batches ADD COLUMN claimed INTEGER NOT NULL DEFAULT 0",
                "ALTER TABLE spend_batches ADD COLUMN claimed_at INTEGER",
            ];
            for migration in batch_migrations {
                let _ = conn.execute(migration, params![]);
            }
            // Relayer transactions table migration for existing databases
            let _ = conn.execute(
                "CREATE TABLE IF NOT EXISTS relayer_transactions (
                    tx_hash TEXT PRIMARY KEY,
                    nonce INTEGER NOT NULL,
                    signer_address TEXT NOT NULL DEFAULT '',
                    status TEXT NOT NULL,
                    block_number INTEGER,
                    gas_used INTEGER,
                    effective_gas_price TEXT,
                    confirmations INTEGER NOT NULL DEFAULT 0,
                    error_reason TEXT,
                    created_at INTEGER NOT NULL,
                    updated_at INTEGER NOT NULL
                )",
                params![],
            );
            let _ = conn.execute(
                "CREATE INDEX IF NOT EXISTS idx_relayer_tx_status ON relayer_transactions(status)",
                params![],
            );
            let _ = conn.execute(
                "CREATE INDEX IF NOT EXISTS idx_relayer_tx_nonce ON relayer_transactions(nonce)",
                params![],
            );
            let _ = conn.execute(
                "CREATE INDEX IF NOT EXISTS idx_relayer_tx_signer ON relayer_transactions(signer_address)",
                params![],
            );
            conn.execute_batch(
                "
                CREATE INDEX IF NOT EXISTS idx_spend_queue_claim
                ON spend_queue(status, available_at, lease_until, created_at);

                UPDATE spend_queue
                SET status = 'failed',
                    last_error = 'legacy queue row has no canonical request payload',
                    updated_at = CAST(strftime('%s','now') AS INTEGER)
                WHERE request_json IS NULL AND status = 'queued';
            ",
            )?;

            // Migrations for DEC-018 on-chain deposit indexer
            let _ = conn.execute(
                "CREATE TABLE IF NOT EXISTS indexer_state (
                    key TEXT PRIMARY KEY,
                    last_block INTEGER NOT NULL,
                    updated_at INTEGER NOT NULL
                )",
                params![],
            );

            // Migrations for DEC-035C MMR indexer and state persistence
            let _ = conn.execute(
                "CREATE TABLE IF NOT EXISTS mmr_leaves (
                    leaf_index INTEGER PRIMARY KEY,
                    commitment TEXT NOT NULL,
                    tx_hash TEXT NOT NULL,
                    block_number INTEGER NOT NULL,
                    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
                )",
                params![],
            );
            let _ = conn.execute(
                "CREATE INDEX IF NOT EXISTS idx_mmr_leaves_block ON mmr_leaves(block_number)",
                params![],
            );
            let _ = conn.execute(
                "CREATE TABLE IF NOT EXISTS mmr_state (
                    id INTEGER PRIMARY KEY CHECK (id = 1),
                    leaf_count INTEGER NOT NULL,
                    bagged_root TEXT NOT NULL,
                    peaks_json TEXT NOT NULL,
                    last_indexed_block INTEGER NOT NULL,
                    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
                )",
                params![],
            );

            Ok(())
        })
        .await
        .context("Database initialization failed")??;

        // Set file permissions to 600 (owner-only read/write)
        let path_ref = path.as_ref();
        if path_ref.exists() {
            let mut perms = std::fs::metadata(path_ref)?.permissions();
            perms.set_mode(0o600);
            std::fs::set_permissions(path_ref, perms)?;

            // Also protect WAL and SHM files if they exist
            let wal_path = path_ref.with_extension("db-wal");
            if wal_path.exists() {
                let mut perms = std::fs::metadata(&wal_path)?.permissions();
                perms.set_mode(0o600);
                std::fs::set_permissions(&wal_path, perms)?;
            }

            let shm_path = path_ref.with_extension("db-shm");
            if shm_path.exists() {
                let mut perms = std::fs::metadata(&shm_path)?.permissions();
                perms.set_mode(0o600);
                std::fs::set_permissions(&shm_path, perms)?;
            }
        }

        Ok(Self { path: path_str })
    }
}
