//! Production-grade SQLite database for Nimbus Relayer Node
//!
//! Based on 2026 best practices from:
//! - SQLite Renaissance (WAL mode, PRAGMA tuning)
//! - ZK Rollup nullifier tracking patterns (Nullmask, Umbra, Anubis)
//! - Rust async/blocking patterns (tokio spawn_blocking)

use rusqlite::{Connection, OptionalExtension, params};
use std::path::Path;
use tokio::task;
use anyhow::{Result, Context};

/// Production SQLite database with optimized PRAGMAs
#[derive(Clone)]
pub struct Database {
    path: String,
}

impl Database {
    /// Initialize database with production-grade configuration
    pub async fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path_str = path.as_ref().to_string_lossy().to_string();
        let path_clone = path_str.clone();
        
        // Initialize schema and PRAGMAs in blocking context
        task::spawn_blocking(move || -> Result<()> {
            let conn = Connection::open(&path_clone)?;
            
            // CRITICAL: Production PRAGMAs (2026 best practices)
            conn.execute_batch("
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
            ")?;
            
            // Create tables
            conn.execute_batch("
                -- Session tracking (deposit -> reveal lifecycle)
                CREATE TABLE IF NOT EXISTS sessions (
                    session_id TEXT PRIMARY KEY,
                    com_k_hex TEXT NOT NULL,
                    amount INTEGER NOT NULL,
                    resolved BOOLEAN NOT NULL DEFAULT 0,
                    masking_key_hex TEXT,
                    client_address TEXT NOT NULL,
                    created_at INTEGER NOT NULL,
                    resolved_at INTEGER
                );
                CREATE INDEX IF NOT EXISTS idx_sessions_resolved ON sessions(resolved);
                CREATE INDEX IF NOT EXISTS idx_sessions_client ON sessions(client_address);
                
                -- Nullifier tracking (double-spend prevention)
                -- Inspired by Nullmask, Anubis, Umbra nullifier registries
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
            ")?;
            
            Ok(())
        })
        .await
        .context("Database initialization failed")??;
        
        Ok(Self { path: path_str })
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
        
        task::spawn_blocking(move || -> Result<bool> {
            let conn = Connection::open(&path)?;
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
    pub async fn resolve_session(
        &self,
        session_id: &str,
        masking_key_hex: &str,
    ) -> Result<bool> {
        let path = self.path.clone();
        let session_id = session_id.to_string();
        let masking_key_hex = masking_key_hex.to_string();
        
        task::spawn_blocking(move || -> Result<bool> {
            let conn = Connection::open(&path)?;
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
    pub async fn check_and_insert_nullifier(
        &self,
        nullifier: &str,
        tx_hash: Option<&str>,
    ) -> Result<bool> {
        let path = self.path.clone();
        let nullifier = nullifier.to_string();
        let tx_hash = tx_hash.map(|s| s.to_string());
        
        task::spawn_blocking(move || -> Result<bool> {
            let conn = Connection::open(&path)?;
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
        
        task::spawn_blocking(move || -> Result<bool> {
            let conn = Connection::open(&path)?;
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
        
        task::spawn_blocking(move || -> Result<DbStats> {
            let conn = Connection::open(&path)?;
            
            let total_sessions: i64 = conn.query_row(
                "SELECT COUNT(*) FROM sessions",
                [],
                |row| row.get(0),
            )?;
            
            let resolved_sessions: i64 = conn.query_row(
                "SELECT COUNT(*) FROM sessions WHERE resolved = 1",
                [],
                |row| row.get(0),
            )?;
            
            let total_nullifiers: i64 = conn.query_row(
                "SELECT COUNT(*) FROM nullifiers",
                [],
                |row| row.get(0),
            )?;
            
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
        
        task::spawn_blocking(move || -> Result<usize> {
            let conn = Connection::open(&path)?;
            let cutoff_timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_secs() as i64 - (days_old as i64 * 86400);
            
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
        
        task::spawn_blocking(move || -> Result<()> {
            let conn = Connection::open(&path)?;
            conn.execute_batch("VACUUM;")?;
            Ok(())
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
    use tempfile::TempDir;
    
    #[tokio::test]
    async fn test_nullifier_double_spend_prevention() {
        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("test.db");
        let db = Database::new(&db_path).await.unwrap();
        
        let nullifier = "0xdeadbeef";
        
        // First spend should succeed
        let first = db.check_and_insert_nullifier(nullifier, None).await.unwrap();
        assert!(first, "First spend should succeed");
        
        // Second spend should fail (double-spend detected!)
        let second = db.check_and_insert_nullifier(nullifier, None).await.unwrap();
        assert!(!second, "Second spend should be rejected (double-spend!)");
        
        // Check should confirm it's spent
        let is_spent = db.is_nullifier_spent(nullifier).await.unwrap();
        assert!(is_spent, "Nullifier should be marked as spent");
    }
    
    #[tokio::test]
    async fn test_session_lifecycle() {
        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("test.db");
        let db = Database::new(&db_path).await.unwrap();
        
        let sid = "test_session_123";
        let com_k = "0xabc123";
        let masking_key = "0xdef456";
        
        // Insert session
        let inserted = db.insert_session(sid, com_k, 1000, "0xclient").await.unwrap();
        assert!(inserted);
        
        // Duplicate insert should be ignored
        let dup = db.insert_session(sid, com_k, 1000, "0xclient").await.unwrap();
        assert!(!dup);
        
        // Resolve session
        let resolved = db.resolve_session(sid, masking_key).await.unwrap();
        assert!(resolved);
        
        // Can't resolve twice
        let resolved_again = db.resolve_session(sid, masking_key).await.unwrap();
        assert!(!resolved_again);
    }
    
    #[tokio::test]
    async fn test_concurrent_nullifier_inserts() {
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
        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("test.db");
        let db = Database::new(&db_path).await.unwrap();
        
        // Insert and resolve 5 sessions
        for i in 0..5 {
            let sid = format!("session_{}", i);
            db.insert_session(&sid, "0xabc", 1000, "0xclient").await.unwrap();
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