//! Production-grade SQLite database for Nimbus Relayer Node
//!
//! Based on 2026 best practices from:
//! - SQLite Renaissance (WAL mode, PRAGMA tuning)
//! - ZK Rollup nullifier tracking patterns (Nullmask, Umbra, Anubis)
//! - Rust async/blocking patterns (tokio spawn_blocking)

use anyhow::{Context, Result};
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde_json;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use tokio::task;

/// Production SQLite database with optimized PRAGMAs
#[derive(Clone)]
pub struct Database {
    path: String,
}

#[derive(Debug)]
pub struct QueuedSpend {
    pub id: i64,
    pub request: crate::dto::SpendRequest,
    pub retry_count: u32,
}

#[derive(Debug, Clone)]
pub struct StalledSessionInfo {
    pub session_id: String,
    pub amount: i64,
    pub client_address: String,
    pub created_at: i64,
    pub deposit_confirmed: i64,
    pub resolved: i64,
}

const DEFAULT_DB_KEY: &str = "default-change-in-production";

fn database_key() -> Result<String> {
    match std::env::var("NIMBUS_DB_KEY") {
        Ok(value) if !value.trim().is_empty() && value != DEFAULT_DB_KEY => Ok(value),
        Ok(_) | Err(_) if crate::config::runtime_mode().is_strict() => {
            anyhow::bail!(
                "NIMBUS_DB_KEY must be explicitly configured in {} mode",
                crate::config::runtime_mode().label()
            )
        }
        Ok(value) => Ok(value),
        Err(_) => Ok(DEFAULT_DB_KEY.to_string()),
    }
}

fn open_connection(path: &str, key: &str) -> Result<Connection> {
    let conn = Connection::open(path)?;
    conn.pragma_update(None, "key", key)?;
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    Ok(conn)
}

fn unix_timestamp() -> Result<i64> {
    Ok(std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs() as i64)
}

impl Database {
    /// Initialize database with production-grade configuration
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
                -- Session tracking (deposit -> reveal lifecycle)
                CREATE TABLE IF NOT EXISTS sessions (
                    session_id TEXT PRIMARY KEY,
                    com_k_hex TEXT NOT NULL,
                    amount INTEGER NOT NULL,
                    resolved BOOLEAN NOT NULL DEFAULT 0,
                    deposit_confirmed BOOLEAN NOT NULL DEFAULT 0,
                    masking_key_hex TEXT,
                    client_address TEXT NOT NULL,
                    created_at INTEGER NOT NULL,
                    resolved_at INTEGER
                );
                CREATE INDEX IF NOT EXISTS idx_sessions_resolved ON sessions(resolved);
                CREATE INDEX IF NOT EXISTS idx_sessions_client ON sessions(client_address);

                -- Nullifier tracking (double-spend prevention)
                -- Inspired by ZK Rollup nullifier registries
                CREATE TABLE IF NOT EXISTS nullifiers (
                    nullifier TEXT PRIMARY KEY,
                    spent_at INTEGER NOT NULL,
                    tx_hash TEXT
                );
                CREATE INDEX IF NOT EXISTS idx_nullifiers_spent ON nullifiers(spent_at);

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
            ",
            )?;

            // Run migration for existing databases (fails silently if column already exists)
            let _ = conn.execute(
                "ALTER TABLE sessions ADD COLUMN deposit_confirmed BOOLEAN NOT NULL DEFAULT 0",
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
            ];
            for migration in queue_migrations {
                let _ = conn.execute(migration, params![]);
            }
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

    /// Persist a spend before acknowledging it to the caller.
    /// Returns the queue id, or None when the nullifier already has an active
    /// or confirmed settlement.
    pub async fn enqueue_spend(&self, request: &crate::dto::SpendRequest) -> Result<Option<i64>> {
        let path = self.path.clone();
        let request = request.clone();
        let db_key = database_key()?;

        task::spawn_blocking(move || -> Result<Option<i64>> {
            let mut conn = open_connection(&path, &db_key)?;
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let duplicate: Option<i64> = tx
                .query_row(
                    "SELECT id FROM spend_queue
                     WHERE nullifier = ?
                       AND status IN (
                           'queued','retryable','broadcasting','submitted','confirmed'
                       )
                     LIMIT 1",
                    params![request.nullifier],
                    |row| row.get(0),
                )
                .optional()?;
            if duplicate.is_some() {
                tx.rollback()?;
                return Ok(None);
            }

            let now = unix_timestamp()?;
            let request_json = serde_json::to_string(&request)?;
            tx.execute(
                "INSERT INTO spend_queue
                 (nullifier, sig_hex, recipient, amount, created_at, processed,
                  request_json, status, retry_count, available_at, updated_at)
                 VALUES (?, ?, ?, ?, ?, 0, ?, 'queued', 0, ?, ?)",
                params![
                    request.nullifier,
                    request.sig_hex,
                    request.recipient,
                    request.amount as i64,
                    now,
                    request_json,
                    now,
                    now
                ],
            )?;
            let id = tx.last_insert_rowid();
            tx.commit()?;
            Ok(Some(id))
        })
        .await?
    }

    /// Atomically lease queued work. Expired broadcasting leases are recovered
    /// because their process may have died before persisting a tx hash.
    pub async fn claim_spends(&self, limit: usize, lease_seconds: i64) -> Result<Vec<QueuedSpend>> {
        let path = self.path.clone();
        let db_key = database_key()?;

        task::spawn_blocking(move || -> Result<Vec<QueuedSpend>> {
            let mut conn = open_connection(&path, &db_key)?;
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let now = unix_timestamp()?;
            let lease_until = now + lease_seconds;

            let ids = {
                let mut stmt = tx.prepare(
                    "SELECT id FROM spend_queue
                     WHERE (
                         status IN ('queued','retryable') AND available_at <= ?
                     ) OR (
                         status = 'broadcasting' AND lease_until IS NOT NULL AND lease_until <= ?
                     )
                     ORDER BY created_at, id
                     LIMIT ?",
                )?;
                let rows = stmt
                    .query_map(params![now, now, limit as i64], |row| row.get::<_, i64>(0))?
                    .collect::<rusqlite::Result<Vec<_>>>()?;
                rows
            };

            let mut claimed = Vec::with_capacity(ids.len());
            for id in ids {
                let changed = tx.execute(
                    "UPDATE spend_queue
                     SET status = 'broadcasting', lease_until = ?, updated_at = ?
                     WHERE id = ?
                       AND (
                           (status IN ('queued','retryable') AND available_at <= ?)
                           OR (status = 'broadcasting' AND lease_until <= ?)
                       )",
                    params![lease_until, now, id, now, now],
                )?;
                if changed == 0 {
                    continue;
                }
                let row = tx.query_row(
                    "SELECT id, request_json, retry_count
                     FROM spend_queue WHERE id = ?",
                    params![id],
                    |row| {
                        Ok((
                            row.get::<_, i64>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, i64>(2)?,
                        ))
                    },
                )?;
                claimed.push(QueuedSpend {
                    id: row.0,
                    request: serde_json::from_str(&row.1)?,
                    retry_count: row.2 as u32,
                });
            }
            tx.commit()?;
            Ok(claimed)
        })
        .await?
    }

    pub async fn mark_spend_submitted(&self, id: i64, tx_hash: &str) -> Result<()> {
        self.update_queue(
            id,
            "UPDATE spend_queue
             SET status = 'submitted', tx_hash = ?, lease_until = NULL, updated_at = ?
             WHERE id = ? AND status = 'broadcasting'",
            Some(tx_hash),
            None,
        )
        .await
    }

    pub async fn mark_spend_confirmed(
        &self,
        id: i64,
        nullifier: &str,
        tx_hash: &str,
        block_number: u64,
    ) -> Result<()> {
        let path = self.path.clone();
        let db_key = database_key()?;
        let nullifier = nullifier.to_string();
        let tx_hash = tx_hash.to_string();

        task::spawn_blocking(move || -> Result<()> {
            let mut conn = open_connection(&path, &db_key)?;
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let now = unix_timestamp()?;
            tx.execute(
                "INSERT OR IGNORE INTO nullifiers (nullifier, spent_at, tx_hash)
                 VALUES (?, ?, ?)",
                params![nullifier, now, tx_hash],
            )?;
            let changed = tx.execute(
                "UPDATE spend_queue
                 SET status = 'confirmed', processed = 1, tx_hash = ?,
                     block_number = ?, lease_until = NULL, updated_at = ?
                 WHERE id = ? AND status IN ('broadcasting','submitted')",
                params![tx_hash, block_number as i64, now, id],
            )?;
            if changed != 1 {
                anyhow::bail!("queue item {} was not in a confirmable state", id);
            }
            tx.commit()?;
            Ok(())
        })
        .await?
    }

    pub async fn retry_spend(&self, id: i64, error: &str, retry_count: u32) -> Result<()> {
        let delay = 2_i64.saturating_pow(retry_count.min(8)).min(300);
        let path = self.path.clone();
        let db_key = database_key()?;
        let error = error.to_string();
        task::spawn_blocking(move || -> Result<()> {
            let conn = open_connection(&path, &db_key)?;
            let now = unix_timestamp()?;
            conn.execute(
                "UPDATE spend_queue
                 SET status = 'retryable', retry_count = retry_count + 1,
                     last_error = ?, available_at = ?, lease_until = NULL,
                     updated_at = ?
                 WHERE id = ? AND status = 'broadcasting'",
                params![error, now + delay, now, id],
            )?;
            Ok(())
        })
        .await?
    }

    pub async fn fail_spend(&self, id: i64, tx_hash: Option<&str>, error: &str) -> Result<()> {
        self.update_queue(
            id,
            "UPDATE spend_queue
             SET status = 'failed', processed = 1, tx_hash = COALESCE(?, tx_hash),
                 last_error = ?, lease_until = NULL, updated_at = ?
             WHERE id = ? AND status IN ('broadcasting','submitted')",
            tx_hash,
            Some(error),
        )
        .await
    }

    async fn update_queue(
        &self,
        id: i64,
        sql: &'static str,
        tx_hash: Option<&str>,
        error: Option<&str>,
    ) -> Result<()> {
        let path = self.path.clone();
        let db_key = database_key()?;
        let tx_hash = tx_hash.map(str::to_string);
        let error = error.map(str::to_string);
        task::spawn_blocking(move || -> Result<()> {
            let conn = open_connection(&path, &db_key)?;
            let now = unix_timestamp()?;
            let changed = if error.is_some() {
                conn.execute(sql, params![tx_hash, error, now, id])?
            } else {
                conn.execute(sql, params![tx_hash, now, id])?
            };
            if changed != 1 {
                anyhow::bail!("queue item {} transition rejected", id);
            }
            Ok(())
        })
        .await?
    }

    pub async fn queued_spend_count(&self) -> Result<usize> {
        let path = self.path.clone();
        let db_key = database_key()?;
        task::spawn_blocking(move || -> Result<usize> {
            let conn = open_connection(&path, &db_key)?;
            let count = conn.query_row(
                "SELECT COUNT(*) FROM spend_queue
                 WHERE status IN ('queued','retryable','broadcasting','submitted')",
                [],
                |row| row.get::<_, i64>(0),
            )?;
            Ok(count as usize)
        })
        .await?
    }

    /// Insert session (deposit)
    /// Returns true if inserted, false if already exists
    pub async fn insert_session(
        &self,
        session_id: &str,
        com_k_hex: &str,
        amount: u64,
        client_address: &str,
    ) -> Result<bool> {
        let path = self.path.clone();
        let session_id = session_id.to_string();
        let com_k_hex = com_k_hex.to_string();
        let client_address = client_address.to_string();

        // Get encryption key - TODO: Integrate with KMS (OpenBao/Vault) for production
        let db_key = database_key()?;

        task::spawn_blocking(move || -> Result<bool> {
            let conn = open_connection(&path, &db_key)?;
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_secs() as i64;

            // Try insert (ignore duplicates)
            let changes = conn.execute(
                "INSERT OR IGNORE INTO sessions
                 (session_id, com_k_hex, amount, client_address, created_at)
                 VALUES (?, ?, ?, ?, ?)",
                params![session_id, com_k_hex, amount as i64, client_address, now],
            )?;

            Ok(changes > 0)
        })
        .await?
    }

    /// Resolve session (reveal masking key)
    /// Returns true if resolved, false if not found or already resolved
    #[allow(dead_code)]
    pub async fn resolve_session(&self, session_id: &str, masking_key_hex: &str) -> Result<bool> {
        let path = self.path.clone();
        let session_id = session_id.to_string();
        let masking_key_hex = masking_key_hex.to_string();

        // Get encryption key - TODO: Integrate with KMS (OpenBao/Vault) for production
        let db_key = database_key()?;

        task::spawn_blocking(move || -> Result<bool> {
            let conn = open_connection(&path, &db_key)?;
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_secs() as i64;

            let changes = conn.execute(
                "UPDATE sessions
                 SET resolved = 1, masking_key_hex = ?, resolved_at = ?
                 WHERE session_id = ? AND resolved = 0",
                params![masking_key_hex, now, session_id],
            )?;

            Ok(changes > 0)
        })
        .await?
    }

    /// Insert a signing session before deposit occurs (Atomic Release flow)
    /// Returns true if inserted, false if already exists
    pub async fn insert_signing_session(
        &self,
        session_id: &str,
        com_k_hex: &str,
        amount: u64,
        client_address: &str,
        masking_key_hex: &str,
    ) -> Result<bool> {
        let path = self.path.clone();
        let session_id = session_id.to_string();
        let com_k_hex = com_k_hex.to_string();
        let client_address = client_address.to_string();
        let masking_key_hex = masking_key_hex.to_string();

        let db_key = database_key()?;

        task::spawn_blocking(move || -> Result<bool> {
            let conn = open_connection(&path, &db_key)?;
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_secs() as i64;

            let changes = conn.execute(
                "INSERT OR IGNORE INTO sessions
                 (session_id, com_k_hex, amount, resolved, deposit_confirmed, masking_key_hex, client_address, created_at)
                 VALUES (?, ?, ?, 0, 0, ?, ?, ?)",
                params![session_id, com_k_hex, amount as i64, masking_key_hex, client_address, now],
            )?;

            Ok(changes > 0)
        })
        .await?
    }

    /// Confirm deposit for a session by verifying session_id, amount, and com_k
    /// Updates deposit_confirmed to 1
    /// Returns true if updated (matching session exists and was updated), false otherwise
    pub async fn confirm_deposit(
        &self,
        session_id: &str,
        amount: u64,
        com_k_hex: &str,
    ) -> Result<bool> {
        let path = self.path.clone();
        let session_id = session_id.to_string();
        let com_k_hex = com_k_hex.to_string();

        let db_key = database_key()?;

        task::spawn_blocking(move || -> Result<bool> {
            let conn = open_connection(&path, &db_key)?;

            let changes = conn.execute(
                "UPDATE sessions
                 SET deposit_confirmed = 1
                 WHERE session_id = ? AND amount = ? AND com_k_hex = ? AND deposit_confirmed = 0",
                params![session_id, amount as i64, com_k_hex],
            )?;

            Ok(changes > 0)
        })
        .await?
    }

    /// Validate that a session is valid for signing
    /// Returns Ok(true) if session exists, is not resolved, and matches amount/com_k
    /// Returns Ok(false) if session doesn't exist, is resolved, or parameters don't match
    /// Returns Error for database errors
    pub async fn validate_signing_session(
        &self,
        session_id: &str,
        amount: u64,
        com_k_hex: &str,
    ) -> Result<bool> {
        let path = self.path.clone();
        let session_id = session_id.to_string();
        let com_k_hex = com_k_hex.to_string();

        let db_key = database_key()?;

        task::spawn_blocking(move || -> Result<bool> {
            let conn = open_connection(&path, &db_key)?;

            // Check if session exists, is not resolved, and parameters match
            let (count, created_at): (i64, i64) = conn
                .query_row(
                    "SELECT COUNT(*), created_at FROM sessions
                     WHERE session_id = ? AND amount = ? AND com_k_hex = ? AND resolved = 0",
                    params![session_id, amount as i64, com_k_hex],
                    |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
                )
                .unwrap_or((0, 0));

            if count == 0 {
                return Ok(false);
            }

            // Check expiry: sessions expire after 1 hour (3600 seconds)
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_secs() as i64;

            let expiry = created_at + 3600; // 1 hour expiry
            if now > expiry {
                return Ok(false); // Session expired
            }

            Ok(true)
        })
        .await?
    }

    /// Resolve session and retrieve masking key (Atomic Release flow)
    /// Sets resolved = 1 and returns the masking_key_hex ONLY IF deposit_confirmed = 1
    /// Returns Ok(Some(masking_key_hex)) if successful, Ok(None) if not found, already resolved, or not confirmed.
    pub async fn resolve_session_release(&self, session_id: &str) -> Result<Option<String>> {
        let path = self.path.clone();
        let session_id = session_id.to_string();

        let db_key = database_key()?;

        task::spawn_blocking(move || -> Result<Option<String>> {
            let conn = open_connection(&path, &db_key)?;

            // First check if the session is confirmed and not resolved
            let session_info: Option<(String, i64, i64)> = conn
                .query_row(
                    "SELECT masking_key_hex, resolved, deposit_confirmed FROM sessions WHERE session_id = ?",
                    params![session_id],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?, row.get::<_, i64>(2)?)),
                )
                .optional()?;

            if let Some((masking_key_hex, resolved, deposit_confirmed)) = session_info {
                if deposit_confirmed == 1 && resolved == 0 {
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)?
                        .as_secs() as i64;

                    conn.execute(
                        "UPDATE sessions
                         SET resolved = 1, resolved_at = ?
                         WHERE session_id = ? AND resolved = 0",
                        params![now, session_id],
                    )?;

                    Ok(Some(masking_key_hex))
                } else {
                    Ok(None)
                }
            } else {
                Ok(None)
            }
        })
        .await?
    }

    /// Zeroize (clear) the masking key from a session after reveal (DEC-011)
    /// This should be called after the masking key has been successfully used
    /// Returns true if the key was zeroized, false if session not found
    pub async fn zeroize_session_masking_key(&self, session_id: &str) -> Result<bool> {
        let path = self.path.clone();
        let session_id = session_id.to_string();

        let db_key = database_key()?;

        task::spawn_blocking(move || -> Result<bool> {
            let conn = open_connection(&path, &db_key)?;

            // Zeroize the masking key by setting it to a dummy value
            let changes = conn.execute(
                "UPDATE sessions
                 SET masking_key_hex = 'ZEROIZED'
                 WHERE session_id = ? AND masking_key_hex != 'ZEROIZED'",
                params![session_id],
            )?;

            Ok(changes > 0)
        })
        .await?
    }

    /// Cleanup expired sessions by zeroizing their keys (DEC-011)
    /// Should be called periodically by a background job
    /// Returns the number of sessions cleaned up
    #[allow(dead_code)]
    pub async fn cleanup_expired_sessions(&self) -> Result<u64> {
        let path = self.path.clone();

        let db_key = database_key()?;

        task::spawn_blocking(move || -> Result<u64> {
            let conn = open_connection(&path, &db_key)?;

            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_secs() as i64;

            // Sessions expire after 1 hour (3600 seconds)
            let expiry_threshold = now - 3600;

            // Zeroize masking keys for expired sessions
            let changes = conn.execute(
                "UPDATE sessions
                 SET masking_key_hex = 'ZEROIZED'
                 WHERE created_at < ? AND masking_key_hex != 'ZEROIZED'",
                params![expiry_threshold],
            )?;

            Ok(changes as u64)
        })
        .await?
    }

    /// Get stalled signing sessions for monitoring (DEC-012)
    /// Returns sessions that are deposit_confirmed but not resolved after threshold duration
    pub async fn get_stalled_signing_sessions(
        &self,
        threshold_seconds: i64,
    ) -> Result<Vec<StalledSessionInfo>> {
        let path = self.path.clone();

        let db_key = database_key()?;

        task::spawn_blocking(move || -> Result<Vec<StalledSessionInfo>> {
            let conn = open_connection(&path, &db_key)?;

            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_secs() as i64;

            let threshold = now - threshold_seconds;

            // Get sessions that are deposit confirmed, not resolved, and created before threshold
            let mut stmt = conn.prepare(
                "SELECT session_id, amount, client_address, created_at, deposit_confirmed, resolved
                 FROM sessions
                 WHERE deposit_confirmed = 1 AND resolved = 0 AND created_at < ?",
            )?;

            let sessions = stmt
                .query_map(params![threshold], |row| {
                    Ok(StalledSessionInfo {
                        session_id: row.get::<_, String>(0)?,
                        amount: row.get::<_, i64>(1)?,
                        client_address: row.get::<_, String>(2)?,
                        created_at: row.get::<_, i64>(3)?,
                        deposit_confirmed: row.get::<_, i64>(4)?,
                        resolved: row.get::<_, i64>(5)?,
                    })
                })?
                .collect::<std::result::Result<Vec<_>, _>>()?;

            Ok(sessions)
        })
        .await?
    }

    /// Check and insert nullifier atomically (double-spend prevention)
    ///
    /// Returns:
    /// - Ok(true): Nullifier inserted successfully (first time spend)
    /// - Ok(false): Nullifier already exists (DOUBLE-SPEND DETECTED!)
    ///
    /// This is the CRITICAL security check inspired by:
    /// - Nullmask NullifierRegistry
    /// - Umbra PrivateUTXOLedger
    /// - Anubis NullifierSet
    #[allow(dead_code)]
    pub async fn check_and_insert_nullifier(
        &self,
        nullifier: &str,
        tx_hash: Option<&str>,
    ) -> Result<bool> {
        let path = self.path.clone();
        let nullifier = nullifier.to_string();
        let tx_hash = tx_hash.map(|s| s.to_string());

        // Get encryption key - TODO: Integrate with KMS (OpenBao/Vault) for production
        let db_key = database_key()?;

        task::spawn_blocking(move || -> Result<bool> {
            let conn = open_connection(&path, &db_key)?;
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_secs() as i64;

            // ATOMIC: INSERT OR IGNORE (race-safe!)
            // If nullifier exists, changes will be 0
            let changes = conn.execute(
                "INSERT OR IGNORE INTO nullifiers (nullifier, spent_at, tx_hash)
                 VALUES (?, ?, ?)",
                params![nullifier, now, tx_hash],
            )?;

            // changes == 0 means nullifier already exists (DOUBLE-SPEND!)
            Ok(changes > 0)
        })
        .await?
    }

    /// Check if nullifier exists (read-only check)
    pub async fn is_nullifier_spent(&self, nullifier: &str) -> Result<bool> {
        let path = self.path.clone();
        let nullifier = nullifier.to_string();

        // Get encryption key - TODO: Integrate with KMS (OpenBao/Vault) for production
        let db_key = database_key()?;

        task::spawn_blocking(move || -> Result<bool> {
            let conn = open_connection(&path, &db_key)?;
            let exists: Option<i64> = conn
                .query_row(
                    "SELECT 1 FROM nullifiers WHERE nullifier = ?",
                    params![nullifier],
                    |row| row.get(0),
                )
                .optional()?;

            Ok(exists.is_some())
        })
        .await?
    }

    /// Get database statistics (for monitoring)
    pub async fn get_stats(&self) -> Result<DbStats> {
        let path = self.path.clone();

        // Get encryption key - TODO: Integrate with KMS (OpenBao/Vault) for production
        let db_key = database_key()?;

        task::spawn_blocking(move || -> Result<DbStats> {
            let conn = open_connection(&path, &db_key)?;

            let total_sessions: i64 =
                conn.query_row("SELECT COUNT(*) FROM sessions", [], |row| row.get(0))?;

            let resolved_sessions: i64 = conn.query_row(
                "SELECT COUNT(*) FROM sessions WHERE resolved = 1",
                [],
                |row| row.get(0),
            )?;

            let total_nullifiers: i64 =
                conn.query_row("SELECT COUNT(*) FROM nullifiers", [], |row| row.get(0))?;

            let queued_spends: i64 = conn.query_row(
                "SELECT COUNT(*) FROM spend_queue WHERE processed = 0",
                [],
                |row| row.get(0),
            )?;

            Ok(DbStats {
                total_sessions,
                resolved_sessions,
                total_nullifiers,
                queued_spends,
            })
        })
        .await?
    }

    /// Cleanup old resolved sessions (default: older than 30 days)
    /// Returns number of rows deleted
    pub async fn cleanup_old_sessions(&self, days_old: u32) -> Result<usize> {
        let path = self.path.clone();

        // Get encryption key - TODO: Integrate with KMS (OpenBao/Vault) for production
        let db_key = database_key()?;

        task::spawn_blocking(move || -> Result<usize> {
            let conn = open_connection(&path, &db_key)?;
            let cutoff_timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_secs() as i64
                - (days_old as i64 * 86400);

            let deleted = conn.execute(
                "DELETE FROM sessions WHERE resolved = 1 AND resolved_at < ?",
                params![cutoff_timestamp],
            )?;

            Ok(deleted)
        })
        .await?
    }

    /// Get database file size in bytes
    pub async fn get_db_size(&self) -> Result<u64> {
        let path = self.path.clone();

        task::spawn_blocking(move || -> Result<u64> {
            let metadata = std::fs::metadata(&path)?;
            Ok(metadata.len())
        })
        .await?
    }

    /// Vacuum database to reclaim space after cleanup
    pub async fn vacuum(&self) -> Result<()> {
        let path = self.path.clone();

        // Get encryption key - TODO: Integrate with KMS (OpenBao/Vault) for production
        let db_key = database_key()?;

        task::spawn_blocking(move || -> Result<()> {
            let conn = open_connection(&path, &db_key)?;
            conn.execute_batch("VACUUM;")?;
            Ok(())
        })
        .await?
    }

    /// Check if an idempotency key exists and return the cached response if so
    pub async fn check_idempotency(&self, key: &str, endpoint: &str) -> Result<Option<String>> {
        let path = self.path.clone();
        let key = key.to_string();
        let endpoint = endpoint.to_string();

        let db_key = database_key()?;

        task::spawn_blocking(move || -> Result<Option<String>> {
            let conn = open_connection(&path, &db_key)?;

            // Check with 24h TTL (86400 seconds)
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_secs() as i64;
            let cutoff = now - 86400;

            let result: Option<String> = conn
                .query_row(
                    "SELECT response_json FROM idempotency_cache
                     WHERE idempotency_key = ? AND endpoint = ? AND created_at > ?",
                    params![key, endpoint, cutoff],
                    |row| row.get(0),
                )
                .optional()?;

            Ok(result)
        })
        .await?
    }

    /// Store an idempotency key with its response for future deduplication
    pub async fn store_idempotency(
        &self,
        key: &str,
        endpoint: &str,
        response_json: &str,
    ) -> Result<()> {
        let path = self.path.clone();
        let key = key.to_string();
        let endpoint = endpoint.to_string();
        let response_json = response_json.to_string();

        let db_key = database_key()?;

        task::spawn_blocking(move || -> Result<()> {
            let conn = open_connection(&path, &db_key)?;
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_secs() as i64;

            conn.execute(
                "INSERT OR REPLACE INTO idempotency_cache
                 (idempotency_key, endpoint, response_json, created_at)
                 VALUES (?, ?, ?, ?)",
                params![key, endpoint, response_json, now],
            )?;

            Ok(())
        })
        .await?
    }

    /// Cleanup expired idempotency keys (older than 24h)
    pub async fn cleanup_idempotency_cache(&self) -> Result<usize> {
        let path = self.path.clone();

        let db_key = database_key()?;

        task::spawn_blocking(move || -> Result<usize> {
            let conn = open_connection(&path, &db_key)?;
            let cutoff = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_secs() as i64
                - 86400;

            let deleted = conn.execute(
                "DELETE FROM idempotency_cache WHERE created_at < ?",
                params![cutoff],
            )?;

            Ok(deleted)
        })
        .await?
    }
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct DbStats {
    pub total_sessions: i64,
    pub resolved_sessions: i64,
    pub total_nullifiers: i64,
    pub queued_spends: i64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dto::SpendRequest;
    use tempfile::TempDir;

    fn sample_spend(nullifier: &str) -> SpendRequest {
        SpendRequest {
            nullifier: nullifier.to_string(),
            sig_hex: "0xsig".to_string(),
            recipient: "0x0000000000000000000000000000000000000001".to_string(),
            amount: 10_000_000,
            eip7702_auth: None,
            cross_chain: None,
            alpha_neg_hex: format!("0x{}", "11".repeat(128)),
            hm_hex: format!("0x{}", "22".repeat(128)),
            pk_iss_hex: format!("0x{}", "33".repeat(256)),
            recipient_or_intent_hash_hex: Some(format!("0x{}", "44".repeat(32))),
            expiry: Some(u64::MAX),
            nonce_hex: Some(format!("0x{}", "55".repeat(32))),
            min_payout: None,
            deadline: None,
            idempotency_key: Some(format!("idem-{}", nullifier)),
        }
    }

    #[tokio::test]
    async fn test_persistent_spend_queue_lifecycle() {
        std::env::set_var("NIMBUS_DB_KEY", "test-encryption-key");
        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("queue.db");
        let request = sample_spend("0xqueue-lifecycle");

        let db = Database::new(&db_path).await.unwrap();
        let queue_id = db.enqueue_spend(&request).await.unwrap().unwrap();
        assert_eq!(db.queued_spend_count().await.unwrap(), 1);
        assert!(db.enqueue_spend(&request).await.unwrap().is_none());
        drop(db);

        let reopened = Database::new(&db_path).await.unwrap();
        let claimed = reopened.claim_spends(10, 60).await.unwrap();
        assert_eq!(claimed.len(), 1);
        assert_eq!(claimed[0].id, queue_id);
        assert_eq!(claimed[0].request.nullifier, request.nullifier);
        assert!(reopened.claim_spends(10, 60).await.unwrap().is_empty());

        reopened
            .mark_spend_submitted(queue_id, "0xtx")
            .await
            .unwrap();
        reopened
            .mark_spend_confirmed(queue_id, &request.nullifier, "0xtx", 42)
            .await
            .unwrap();
        assert!(reopened
            .is_nullifier_spent(&request.nullifier)
            .await
            .unwrap());
        assert_eq!(reopened.queued_spend_count().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn test_retry_requeues_without_confirming_nullifier() {
        std::env::set_var("NIMBUS_DB_KEY", "test-encryption-key");
        let tmp = TempDir::new().unwrap();
        let db = Database::new(tmp.path().join("retry.db")).await.unwrap();
        let request = sample_spend("0xretry");
        let queue_id = db.enqueue_spend(&request).await.unwrap().unwrap();
        let item = db.claim_spends(1, 60).await.unwrap().pop().unwrap();

        db.retry_spend(item.id, "rpc unavailable", item.retry_count)
            .await
            .unwrap();
        assert!(!db.is_nullifier_spent(&request.nullifier).await.unwrap());

        // First retry uses a short backoff; force availability to avoid sleeping.
        let conn = open_connection(&db.path, &database_key().unwrap()).unwrap();
        conn.execute(
            "UPDATE spend_queue SET available_at = 0 WHERE id = ?",
            params![queue_id],
        )
        .unwrap();
        let retried = db.claim_spends(1, 60).await.unwrap();
        assert_eq!(retried.len(), 1);
        assert_eq!(retried[0].retry_count, 1);
    }

    #[tokio::test]
    async fn test_concurrent_workers_claim_only_once() {
        std::env::set_var("NIMBUS_DB_KEY", "test-encryption-key");
        let tmp = TempDir::new().unwrap();
        let db = Database::new(tmp.path().join("claim-race.db"))
            .await
            .unwrap();
        db.enqueue_spend(&sample_spend("0xclaim-race"))
            .await
            .unwrap()
            .unwrap();

        let worker_a = db.clone();
        let worker_b = db.clone();
        let (claimed_a, claimed_b) =
            tokio::join!(worker_a.claim_spends(1, 60), worker_b.claim_spends(1, 60),);
        let total_claimed = claimed_a.unwrap().len() + claimed_b.unwrap().len();
        assert_eq!(total_claimed, 1);
    }

    #[tokio::test]
    async fn test_nullifier_double_spend_prevention() {
        // Set encryption key for tests
        std::env::set_var("NIMBUS_DB_KEY", "test-encryption-key");
        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("test.db");
        let db = Database::new(&db_path).await.unwrap();

        let nullifier = "0xdeadbeef";

        // First spend should succeed
        let first = db
            .check_and_insert_nullifier(nullifier, None)
            .await
            .unwrap();
        assert!(first, "First spend should succeed");

        // Second spend should fail (double-spend detected!)
        let second = db
            .check_and_insert_nullifier(nullifier, None)
            .await
            .unwrap();
        assert!(!second, "Second spend should be rejected (double-spend!)");

        // Check should confirm it's spent
        let is_spent = db.is_nullifier_spent(nullifier).await.unwrap();
        assert!(is_spent, "Nullifier should be marked as spent");
    }

    #[tokio::test]
    async fn test_session_lifecycle() {
        // Set encryption key for tests
        std::env::set_var("NIMBUS_DB_KEY", "test-encryption-key");
        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("test.db");
        let db = Database::new(&db_path).await.unwrap();

        let sid = "test_session_123";
        let com_k = "0xabc123";
        let masking_key = "0xdef456";

        // Insert session
        let inserted = db
            .insert_session(sid, com_k, 1000, "0xclient")
            .await
            .unwrap();
        assert!(inserted);

        // Duplicate insert should be ignored
        let dup = db
            .insert_session(sid, com_k, 1000, "0xclient")
            .await
            .unwrap();
        assert!(!dup);

        // Resolve session
        let resolved = db.resolve_session(sid, masking_key).await.unwrap();
        assert!(resolved);

        // Can't resolve twice
        let resolved_again = db.resolve_session(sid, masking_key).await.unwrap();
        assert!(!resolved_again);
    }

    #[tokio::test]
    async fn test_atomic_release_session_lifecycle() {
        std::env::set_var("NIMBUS_DB_KEY", "test-encryption-key");
        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("test_atomic.db");
        let db = Database::new(&db_path).await.unwrap();

        let sid = "session_atomic_123";
        let com_k = "0xcom_k_123";
        let client = "0xclient_123";
        let masking_key = "0xmasking_key_123";
        let amount = 2000u64;

        // 1. Insert signing session
        let inserted = db
            .insert_signing_session(sid, com_k, amount, client, masking_key)
            .await
            .unwrap();
        assert!(inserted);

        // Try resolving before deposit is confirmed -> should fail (return None)
        let resolved_before = db.resolve_session_release(sid).await.unwrap();
        assert!(resolved_before.is_none());

        // 2. Confirm deposit with wrong amount or com_k -> should fail (return false)
        let confirmed_wrong_amount = db.confirm_deposit(sid, amount + 1, com_k).await.unwrap();
        assert!(!confirmed_wrong_amount);

        let confirmed_wrong_com_k = db
            .confirm_deposit(sid, amount, "0xwrong_com_k")
            .await
            .unwrap();
        assert!(!confirmed_wrong_com_k);

        // 3. Confirm deposit with correct details -> should succeed (return true)
        let confirmed_correct = db.confirm_deposit(sid, amount, com_k).await.unwrap();
        assert!(confirmed_correct);

        // Confirming again should be rejected (idempotent / already confirmed)
        let confirmed_again = db.confirm_deposit(sid, amount, com_k).await.unwrap();
        assert!(!confirmed_again);

        // 4. Resolve session after deposit confirmed -> should succeed and return the masking key
        let resolved_after = db.resolve_session_release(sid).await.unwrap();
        assert_eq!(resolved_after, Some(masking_key.to_string()));

        // Resolving again should return None (already resolved)
        let resolved_again = db.resolve_session_release(sid).await.unwrap();
        assert!(resolved_again.is_none());
    }

    #[tokio::test]
    async fn test_concurrent_nullifier_inserts() {
        // Set encryption key for tests
        std::env::set_var("NIMBUS_DB_KEY", "test-encryption-key");
        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("test.db");
        let db = Database::new(&db_path).await.unwrap();

        let nullifier = "0xrace_condition";

        // Simulate concurrent inserts (race condition test)
        let db1 = db.clone();
        let db2 = db.clone();
        let n1 = nullifier.to_string();
        let n2 = nullifier.to_string();

        let (r1, r2): (Result<bool>, Result<bool>) = tokio::join!(
            db1.check_and_insert_nullifier(&n1, None),
            db2.check_and_insert_nullifier(&n2, None),
        );

        // Only ONE should succeed (atomic INSERT OR IGNORE!)
        let successes = [r1.unwrap(), r2.unwrap()].iter().filter(|&&x| x).count();
        assert_eq!(successes, 1, "Only one concurrent insert should succeed");
    }

    #[tokio::test]
    async fn test_cleanup_old_sessions() {
        // Set encryption key for tests
        std::env::set_var("NIMBUS_DB_KEY", "test-encryption-key");
        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("test.db");
        let db = Database::new(&db_path).await.unwrap();

        // Insert and resolve 5 sessions
        for i in 0..5 {
            let sid = format!("session_{}", i);
            db.insert_session(&sid, "0xabc", 1000, "0xclient")
                .await
                .unwrap();
            db.resolve_session(&sid, "0xkey").await.unwrap();
        }

        // Check stats
        let stats = db.get_stats().await.unwrap();
        assert_eq!(stats.total_sessions, 5);
        assert_eq!(stats.resolved_sessions, 5);

        // Cleanup sessions older than 1000 days (none should be deleted - too recent)
        let deleted = db.cleanup_old_sessions(1000).await.unwrap();
        assert_eq!(deleted, 0, "No sessions should be old enough to delete");

        // Verify sessions still exist
        let stats = db.get_stats().await.unwrap();
        assert_eq!(stats.total_sessions, 5, "All sessions should still exist");

        // Test vacuum (shouldn't error)
        db.vacuum().await.unwrap();

        // Test get_db_size
        let size = db.get_db_size().await.unwrap();
        assert!(size > 0, "DB file should exist and have size");
    }
}
