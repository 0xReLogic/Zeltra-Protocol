//! Crash-Safe Two-Phase Commit (2PC) Note Storage & AEAD SQLite Engine (DEC-036B Phase 5)
//!
//! Academic Foundations & References:
//! - ZK-Security Audit (2025): "Audit of Aleph Zero Shielder: State Persistence and Synchronization Pitfalls
//!   in Shielded Account Notes" (Prevention of orphaned notes and state desynchronization during browser/OS crash).
//! - DEC-024 / DEC-030 / DEC-036B: Two-Phase Commit Engine with WAL journaling.
//!
//! Lifecycle Status Machine:
//! - Phase 1 (Pre-Broadcast Lock):
//!   `SPENT_PENDING` on input notes + `UNCONFIRMED` on change notes + `PENDING_BROADCAST` on session.
//! - Phase 2A (Post-Confirmation Finalization):
//!   `SPENT_CONFIRMED` on input notes + `ACTIVE` on change notes + `CONFIRMED` on session.
//! - Phase 2B (Rollback on Revert / Timeout):
//!   `ACTIVE` rollback on input notes + deletion of `UNCONFIRMED` change notes + `FAILED_ROLLBACK` on session.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DbNoteStatus {
    Unconfirmed,
    Active,
    SpentPending,
    SpentConfirmed,
}

impl DbNoteStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Unconfirmed => "UNCONFIRMED",
            Self::Active => "ACTIVE",
            Self::SpentPending => "SPENT_PENDING",
            Self::SpentConfirmed => "SPENT_CONFIRMED",
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "UNCONFIRMED" => Some(Self::Unconfirmed),
            "ACTIVE" => Some(Self::Active),
            "SPENT_PENDING" => Some(Self::SpentPending),
            "SPENT_CONFIRMED" => Some(Self::SpentConfirmed),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DbSessionStatus {
    PendingBroadcast,
    Broadcasted,
    Confirmed,
    FailedRollback,
}

impl DbSessionStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::PendingBroadcast => "PENDING_BROADCAST",
            Self::Broadcasted => "BROADCASTED",
            Self::Confirmed => "CONFIRMED",
            Self::FailedRollback => "FAILED_ROLLBACK",
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "PENDING_BROADCAST" => Some(Self::PendingBroadcast),
            "BROADCASTED" => Some(Self::Broadcasted),
            "CONFIRMED" => Some(Self::Confirmed),
            "FAILED_ROLLBACK" => Some(Self::FailedRollback),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DbNoteRecord {
    pub commitment: String,
    pub leaf_index: Option<u64>,
    pub value: u64,
    pub owner_pk: String,
    pub rho: String,
    pub rand: String,
    pub nullifier_key: String,
    pub epoch_id: u32,
    pub status: DbNoteStatus,
    pub created_at: u64,
    pub updated_at: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DbSessionRecord {
    pub session_id: String,
    pub input_nf_1: String,
    pub input_nf_2: String,
    pub output_cm_1: String,
    pub output_cm_2: String,
    pub tx_hash: Option<String>,
    pub status: DbSessionStatus,
    pub created_at: u64,
}

#[derive(Debug)]
pub enum StorageError {
    DbError(String),
    NoteNotFound(String),
    SessionNotFound(String),
    InvalidState(String),
}

impl fmt::Display for StorageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DbError(e) => write!(f, "Database error: {e}"),
            Self::NoteNotFound(id) => write!(f, "Note not found: {id}"),
            Self::SessionNotFound(id) => write!(f, "Session not found: {id}"),
            Self::InvalidState(e) => write!(f, "Invalid state transition: {e}"),
        }
    }
}

impl std::error::Error for StorageError {}

#[cfg(not(target_arch = "wasm32"))]
pub struct SqlCipherNoteStore {
    conn: std::sync::Mutex<rusqlite::Connection>,
}

#[cfg(not(target_arch = "wasm32"))]
impl SqlCipherNoteStore {
    /// Opens or creates an encrypted SQLCipher SQLite database with WAL journaling (B5.1).
    pub fn open<P: AsRef<Path>>(path: P, encryption_key: &str) -> Result<Self, StorageError> {
        let conn = rusqlite::Connection::open(path)
            .map_err(|e| StorageError::DbError(format!("Failed to open DB: {e}")))?;

        let store = Self {
            conn: std::sync::Mutex::new(conn),
        };
        store.init(encryption_key)?;
        Ok(store)
    }

    /// Opens an in-memory SQLCipher SQLite database for tests.
    pub fn open_in_memory(encryption_key: &str) -> Result<Self, StorageError> {
        let conn = rusqlite::Connection::open_in_memory()
            .map_err(|e| StorageError::DbError(format!("Failed to open in-memory DB: {e}")))?;

        let store = Self {
            conn: std::sync::Mutex::new(conn),
        };
        store.init(encryption_key)?;
        Ok(store)
    }

    fn init(&self, encryption_key: &str) -> Result<(), StorageError> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| StorageError::DbError("Mutex poisoned".into()))?;

        // 1. Configure SQLCipher AES-256 encryption key
        if !encryption_key.is_empty() {
            let pragma_key = format!("PRAGMA key = '{}';", encryption_key.replace('\'', "''"));
            conn.execute_batch(&pragma_key)
                .map_err(|e| StorageError::DbError(format!("SQLCipher key pragma failed: {e}")))?;
        }

        // 2. Enable WAL mode for crash-safety and read-concurrency
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA foreign_keys = ON;",
        )
        .map_err(|e| StorageError::DbError(format!("Pragma WAL failed: {e}")))?;

        // 3. Migrate tables according to DEC-036B Section 8
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS private_notes (
                commitment TEXT PRIMARY KEY,
                leaf_index INTEGER,
                value INTEGER NOT NULL,
                owner_pk TEXT NOT NULL,
                rho TEXT NOT NULL,
                rand TEXT NOT NULL,
                nullifier_key TEXT NOT NULL,
                epoch_id INTEGER NOT NULL,
                status TEXT NOT NULL CHECK(status IN ('UNCONFIRMED', 'ACTIVE', 'SPENT_PENDING', 'SPENT_CONFIRMED')),
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS joinsplit_sessions (
                session_id TEXT PRIMARY KEY,
                input_nf_1 TEXT NOT NULL,
                input_nf_2 TEXT NOT NULL,
                output_cm_1 TEXT NOT NULL,
                output_cm_2 TEXT NOT NULL,
                tx_hash TEXT,
                status TEXT NOT NULL CHECK(status IN ('PENDING_BROADCAST', 'BROADCASTED', 'CONFIRMED', 'FAILED_ROLLBACK')),
                created_at INTEGER NOT NULL
            );",
        )
        .map_err(|e| StorageError::DbError(format!("Migration failed: {e}")))?;

        Ok(())
    }

    /// Inserts or replaces a private note record.
    pub fn insert_note(&self, note: &DbNoteRecord) -> Result<(), StorageError> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| StorageError::DbError("Mutex poisoned".into()))?;

        conn.execute(
            "INSERT INTO private_notes (
                commitment, leaf_index, value, owner_pk, rho, rand, nullifier_key, epoch_id, status, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
            ON CONFLICT(commitment) DO UPDATE SET
                leaf_index = excluded.leaf_index,
                value = excluded.value,
                status = excluded.status,
                updated_at = excluded.updated_at;",
            rusqlite::params![
                note.commitment,
                note.leaf_index.map(|idx| idx as i64),
                note.value as i64,
                note.owner_pk,
                note.rho,
                note.rand,
                note.nullifier_key,
                note.epoch_id as i64,
                note.status.as_str(),
                note.created_at as i64,
                note.updated_at as i64,
            ],
        )
        .map_err(|e| StorageError::DbError(format!("Insert note failed: {e}")))?;

        Ok(())
    }

    /// Retrieves a note record by commitment.
    pub fn get_note(&self, commitment: &str) -> Result<Option<DbNoteRecord>, StorageError> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| StorageError::DbError("Mutex poisoned".into()))?;

        let mut stmt = conn
            .prepare(
                "SELECT commitment, leaf_index, value, owner_pk, rho, rand, nullifier_key, epoch_id, status, created_at, updated_at
                 FROM private_notes WHERE commitment = ?1;",
            )
            .map_err(|e| StorageError::DbError(e.to_string()))?;

        let mut rows = stmt
            .query(rusqlite::params![commitment])
            .map_err(|e| StorageError::DbError(e.to_string()))?;

        if let Some(row) = rows
            .next()
            .map_err(|e| StorageError::DbError(e.to_string()))?
        {
            let status_str: String = row
                .get(8)
                .map_err(|e| StorageError::DbError(e.to_string()))?;
            let status = DbNoteStatus::from_str(&status_str).ok_or_else(|| {
                StorageError::InvalidState(format!("Unknown status: {status_str}"))
            })?;

            let leaf_idx_i64: Option<i64> = row
                .get(1)
                .map_err(|e| StorageError::DbError(e.to_string()))?;
            let val_i64: i64 = row
                .get(2)
                .map_err(|e| StorageError::DbError(e.to_string()))?;
            let epoch_i64: i64 = row
                .get(7)
                .map_err(|e| StorageError::DbError(e.to_string()))?;
            let created_i64: i64 = row
                .get(9)
                .map_err(|e| StorageError::DbError(e.to_string()))?;
            let updated_i64: i64 = row
                .get(10)
                .map_err(|e| StorageError::DbError(e.to_string()))?;

            Ok(Some(DbNoteRecord {
                commitment: row
                    .get(0)
                    .map_err(|e| StorageError::DbError(e.to_string()))?,
                leaf_index: leaf_idx_i64.map(|idx| idx as u64),
                value: val_i64 as u64,
                owner_pk: row
                    .get(3)
                    .map_err(|e| StorageError::DbError(e.to_string()))?,
                rho: row
                    .get(4)
                    .map_err(|e| StorageError::DbError(e.to_string()))?,
                rand: row
                    .get(5)
                    .map_err(|e| StorageError::DbError(e.to_string()))?,
                nullifier_key: row
                    .get(6)
                    .map_err(|e| StorageError::DbError(e.to_string()))?,
                epoch_id: epoch_i64 as u32,
                status,
                created_at: created_i64 as u64,
                updated_at: updated_i64 as u64,
            }))
        } else {
            Ok(None)
        }
    }

    /// Retrieves all ACTIVE notes.
    pub fn get_active_notes(&self) -> Result<Vec<DbNoteRecord>, StorageError> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| StorageError::DbError("Mutex poisoned".into()))?;

        let mut stmt = conn
            .prepare(
                "SELECT commitment, leaf_index, value, owner_pk, rho, rand, nullifier_key, epoch_id, status, created_at, updated_at
                 FROM private_notes WHERE status = 'ACTIVE' ORDER BY value ASC;",
            )
            .map_err(|e| StorageError::DbError(e.to_string()))?;

        let rows = stmt
            .query_map([], |row| {
                let status_str: String = row.get(8)?;
                let status = DbNoteStatus::from_str(&status_str).unwrap_or(DbNoteStatus::Active);
                let leaf_idx_i64: Option<i64> = row.get(1)?;
                let val_i64: i64 = row.get(2)?;
                let epoch_i64: i64 = row.get(7)?;
                let created_i64: i64 = row.get(9)?;
                let updated_i64: i64 = row.get(10)?;
                Ok(DbNoteRecord {
                    commitment: row.get(0)?,
                    leaf_index: leaf_idx_i64.map(|idx| idx as u64),
                    value: val_i64 as u64,
                    owner_pk: row.get(3)?,
                    rho: row.get(4)?,
                    rand: row.get(5)?,
                    nullifier_key: row.get(6)?,
                    epoch_id: epoch_i64 as u32,
                    status,
                    created_at: created_i64 as u64,
                    updated_at: updated_i64 as u64,
                })
            })
            .map_err(|e| StorageError::DbError(e.to_string()))?;

        let mut list = Vec::new();
        for r in rows {
            list.push(r.map_err(|e| StorageError::DbError(e.to_string()))?);
        }
        Ok(list)
    }

    /// Phase 1: Pre-Broadcast Atomic Lock (B5.2).
    ///
    /// Locks input notes to SPENT_PENDING, creates UNCONFIRMED change notes,
    /// and records the session in PENDING_BROADCAST in a single atomic SQLite transaction.
    pub fn phase1_pre_broadcast_lock(
        &self,
        session_id: &str,
        input_commitments: &[&str],
        input_nullifiers: (&str, &str),
        change_notes: &[DbNoteRecord],
        current_time: u64,
    ) -> Result<(), StorageError> {
        let mut conn = self
            .conn
            .lock()
            .map_err(|_| StorageError::DbError("Mutex poisoned".into()))?;

        let tx = conn
            .transaction()
            .map_err(|e| StorageError::DbError(format!("Begin tx failed: {e}")))?;

        let current_time_i64 = current_time as i64;

        // 1. Mark inputs as SPENT_PENDING
        for cm in input_commitments {
            let updated = tx
                .execute(
                    "UPDATE private_notes SET status = 'SPENT_PENDING', updated_at = ?1
                     WHERE commitment = ?2 AND status = 'ACTIVE';",
                    rusqlite::params![current_time_i64, cm],
                )
                .map_err(|e| StorageError::DbError(format!("Update input status failed: {e}")))?;

            if updated == 0 {
                return Err(StorageError::InvalidState(format!(
                    "Note {cm} is not ACTIVE or already locked"
                )));
            }
        }

        // 2. Insert change notes as UNCONFIRMED
        for ch in change_notes {
            tx.execute(
                "INSERT INTO private_notes (
                    commitment, leaf_index, value, owner_pk, rho, rand, nullifier_key, epoch_id, status, created_at, updated_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'UNCONFIRMED', ?9, ?10);",
                rusqlite::params![
                    ch.commitment,
                    ch.leaf_index.map(|idx| idx as i64),
                    ch.value as i64,
                    ch.owner_pk,
                    ch.rho,
                    ch.rand,
                    ch.nullifier_key,
                    ch.epoch_id as i64,
                    current_time_i64,
                    current_time_i64,
                ],
            )
            .map_err(|e| StorageError::DbError(format!("Insert unconfirmed change failed: {e}")))?;
        }

        // 3. Insert session as PENDING_BROADCAST
        let out1 = change_notes
            .first()
            .map(|n| n.commitment.as_str())
            .unwrap_or("");
        let out2 = change_notes
            .get(1)
            .map(|n| n.commitment.as_str())
            .unwrap_or("");
        tx.execute(
            "INSERT INTO joinsplit_sessions (
                session_id, input_nf_1, input_nf_2, output_cm_1, output_cm_2, tx_hash, status, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, NULL, 'PENDING_BROADCAST', ?6);",
            rusqlite::params![
                session_id,
                input_nullifiers.0,
                input_nullifiers.1,
                out1,
                out2,
                current_time_i64,
            ],
        )
        .map_err(|e| StorageError::DbError(format!("Insert session failed: {e}")))?;

        tx.commit()
            .map_err(|e| StorageError::DbError(format!("Commit phase 1 failed: {e}")))?;

        Ok(())
    }

    /// Phase 2A: Post-Confirmation Finalization (B5.3).
    ///
    /// On on-chain confirmation receipt (`status == 1`), sets inputs to SPENT_CONFIRMED,
    /// sets change notes with value > 0 to ACTIVE, and marks session as CONFIRMED.
    pub fn phase2_finalize_confirmation(
        &self,
        session_id: &str,
        tx_hash: &str,
        current_time: u64,
    ) -> Result<(), StorageError> {
        let mut conn = self
            .conn
            .lock()
            .map_err(|_| StorageError::DbError("Mutex poisoned".into()))?;

        let tx = conn
            .transaction()
            .map_err(|e| StorageError::DbError(format!("Begin tx failed: {e}")))?;

        let current_time_i64 = current_time as i64;

        // Retrieve session
        let (out1, out2): (String, String) = tx
            .query_row(
                "SELECT output_cm_1, output_cm_2 FROM joinsplit_sessions WHERE session_id = ?1;",
                rusqlite::params![session_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .map_err(|_| StorageError::SessionNotFound(session_id.to_string()))?;

        // 1. Mark inputs as SPENT_CONFIRMED
        tx.execute(
            "UPDATE private_notes SET status = 'SPENT_CONFIRMED', updated_at = ?1
             WHERE status = 'SPENT_PENDING';",
            rusqlite::params![current_time_i64],
        )
        .map_err(|e| StorageError::DbError(e.to_string()))?;

        // 2. Mark change notes as ACTIVE if value > 0
        if !out1.is_empty() {
            tx.execute(
                "UPDATE private_notes SET status = 'ACTIVE', updated_at = ?1
                 WHERE commitment = ?2 AND value > 0;",
                rusqlite::params![current_time_i64, out1],
            )
            .map_err(|e| StorageError::DbError(e.to_string()))?;
        }
        if !out2.is_empty() {
            tx.execute(
                "UPDATE private_notes SET status = 'ACTIVE', updated_at = ?1
                 WHERE commitment = ?2 AND value > 0;",
                rusqlite::params![current_time_i64, out2],
            )
            .map_err(|e| StorageError::DbError(e.to_string()))?;
        }

        // 3. Mark session as CONFIRMED
        tx.execute(
            "UPDATE joinsplit_sessions SET status = 'CONFIRMED', tx_hash = ?1
             WHERE session_id = ?2;",
            rusqlite::params![tx_hash, session_id],
        )
        .map_err(|e| StorageError::DbError(e.to_string()))?;

        tx.commit()
            .map_err(|e| StorageError::DbError(format!("Commit confirmation failed: {e}")))?;

        Ok(())
    }

    /// Phase 2B: Deterministic Rollback on Revert or Timeout (B5.3).
    ///
    /// Restores inputs to ACTIVE, purges UNCONFIRMED change notes, and marks session as FAILED_ROLLBACK.
    pub fn phase2_rollback(
        &self,
        session_id: &str,
        input_commitments: &[&str],
        current_time: u64,
    ) -> Result<(), StorageError> {
        let mut conn = self
            .conn
            .lock()
            .map_err(|_| StorageError::DbError("Mutex poisoned".into()))?;

        let tx = conn
            .transaction()
            .map_err(|e| StorageError::DbError(format!("Begin tx failed: {e}")))?;

        let current_time_i64 = current_time as i64;

        // 1. Rollback input notes to ACTIVE
        for cm in input_commitments {
            tx.execute(
                "UPDATE private_notes SET status = 'ACTIVE', updated_at = ?1
                 WHERE commitment = ?2 AND status = 'SPENT_PENDING';",
                rusqlite::params![current_time_i64, cm],
            )
            .map_err(|e| StorageError::DbError(e.to_string()))?;
        }

        // 2. Retrieve change notes for session and delete them
        let session_res = tx.query_row(
            "SELECT output_cm_1, output_cm_2 FROM joinsplit_sessions WHERE session_id = ?1;",
            rusqlite::params![session_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        );

        if let Ok((out1, out2)) = session_res {
            if !out1.is_empty() {
                tx.execute(
                    "DELETE FROM private_notes WHERE commitment = ?1 AND status = 'UNCONFIRMED';",
                    rusqlite::params![out1],
                )
                .map_err(|e| StorageError::DbError(e.to_string()))?;
            }
            if !out2.is_empty() {
                tx.execute(
                    "DELETE FROM private_notes WHERE commitment = ?1 AND status = 'UNCONFIRMED';",
                    rusqlite::params![out2],
                )
                .map_err(|e| StorageError::DbError(e.to_string()))?;
            }
        }

        // 3. Mark session as FAILED_ROLLBACK
        tx.execute(
            "UPDATE joinsplit_sessions SET status = 'FAILED_ROLLBACK' WHERE session_id = ?1;",
            rusqlite::params![session_id],
        )
        .map_err(|e| StorageError::DbError(e.to_string()))?;

        tx.commit()
            .map_err(|e| StorageError::DbError(format!("Commit rollback failed: {e}")))?;

        Ok(())
    }

    /// Startup Recovery Routine (B6.4 / Crash Recovery).
    ///
    /// Reconciles or rolls back abandoned pending sessions.
    pub fn recover_pending_sessions(
        &self,
        timeout_secs: u64,
        current_time: u64,
    ) -> Result<usize, StorageError> {
        let mut conn = self
            .conn
            .lock()
            .map_err(|_| StorageError::DbError("Mutex poisoned".into()))?;

        let mut stmt = conn
            .prepare(
                "SELECT session_id, output_cm_1, output_cm_2, created_at FROM joinsplit_sessions
                 WHERE status IN ('PENDING_BROADCAST', 'BROADCASTED');",
            )
            .map_err(|e| StorageError::DbError(e.to_string()))?;

        let sessions: Vec<(String, String, String, u64)> = stmt
            .query_map([], |row| {
                let s_id: String = row.get(0)?;
                let o1: String = row.get(1)?;
                let o2: String = row.get(2)?;
                let cr_i64: i64 = row.get(3)?;
                Ok((s_id, o1, o2, cr_i64 as u64))
            })
            .map_err(|e| StorageError::DbError(e.to_string()))?
            .filter_map(|r| r.ok())
            .collect();

        drop(stmt);

        let mut recovered = 0;
        let current_time_i64 = current_time as i64;
        for (sess_id, out1, out2, created) in sessions {
            if current_time.saturating_sub(created) >= timeout_secs {
                let tx = conn
                    .transaction()
                    .map_err(|e| StorageError::DbError(e.to_string()))?;

                // Rollback any SPENT_PENDING input notes to ACTIVE
                tx.execute(
                    "UPDATE private_notes SET status = 'ACTIVE', updated_at = ?1
                     WHERE status = 'SPENT_PENDING';",
                    rusqlite::params![current_time_i64],
                )
                .map_err(|e| StorageError::DbError(e.to_string()))?;

                // Delete unconfirmed change outputs
                if !out1.is_empty() {
                    let _ = tx.execute(
                        "DELETE FROM private_notes WHERE commitment = ?1 AND status = 'UNCONFIRMED';",
                        rusqlite::params![out1],
                    );
                }
                if !out2.is_empty() {
                    let _ = tx.execute(
                        "DELETE FROM private_notes WHERE commitment = ?1 AND status = 'UNCONFIRMED';",
                        rusqlite::params![out2],
                    );
                }

                // Update session
                tx.execute(
                    "UPDATE joinsplit_sessions SET status = 'FAILED_ROLLBACK' WHERE session_id = ?1;",
                    rusqlite::params![sess_id],
                )
                .map_err(|e| StorageError::DbError(e.to_string()))?;

                tx.commit()
                    .map_err(|e| StorageError::DbError(e.to_string()))?;
                recovered += 1;
            }
        }

        Ok(recovered)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_storage_2pc_lifecycle_success() {
        let store = SqlCipherNoteStore::open_in_memory("test_secret").unwrap();

        // 1. Insert 2 active input notes
        let n1 = DbNoteRecord {
            commitment: "cm_in1".into(),
            leaf_index: Some(0),
            value: 10_000_000,
            owner_pk: "0xpk".into(),
            rho: "0xrho1".into(),
            rand: "0xrand1".into(),
            nullifier_key: "0xnk".into(),
            epoch_id: 1,
            status: DbNoteStatus::Active,
            created_at: 1000,
            updated_at: 1000,
        };
        let n2 = DbNoteRecord {
            commitment: "cm_in2".into(),
            leaf_index: Some(1),
            value: 10_000_000,
            owner_pk: "0xpk".into(),
            rho: "0xrho2".into(),
            rand: "0xrand2".into(),
            nullifier_key: "0xnk".into(),
            epoch_id: 1,
            status: DbNoteStatus::Active,
            created_at: 1000,
            updated_at: 1000,
        };
        store.insert_note(&n1).unwrap();
        store.insert_note(&n2).unwrap();

        assert_eq!(store.get_active_notes().unwrap().len(), 2);

        // 2. Phase 1 Pre-Broadcast Lock
        let ch1 = DbNoteRecord {
            commitment: "cm_out1".into(),
            leaf_index: None,
            value: 5_000_000,
            owner_pk: "0xpk".into(),
            rho: "0xc_rho".into(),
            rand: "0xc_rand".into(),
            nullifier_key: "0xnk".into(),
            epoch_id: 1,
            status: DbNoteStatus::Unconfirmed,
            created_at: 1100,
            updated_at: 1100,
        };

        store
            .phase1_pre_broadcast_lock(
                "session_1",
                &["cm_in1", "cm_in2"],
                ("0xnf1", "0xnf2"),
                &[ch1],
                1100,
            )
            .unwrap();

        // Active notes must now be 0
        assert_eq!(store.get_active_notes().unwrap().len(), 0);
        assert_eq!(
            store.get_note("cm_in1").unwrap().unwrap().status,
            DbNoteStatus::SpentPending
        );
        assert_eq!(
            store.get_note("cm_out1").unwrap().unwrap().status,
            DbNoteStatus::Unconfirmed
        );

        // 3. Phase 2A Finalization
        store
            .phase2_finalize_confirmation("session_1", "0xtx_hash_123", 1200)
            .unwrap();

        assert_eq!(
            store.get_note("cm_in1").unwrap().unwrap().status,
            DbNoteStatus::SpentConfirmed
        );
        assert_eq!(
            store.get_note("cm_out1").unwrap().unwrap().status,
            DbNoteStatus::Active
        );
        assert_eq!(store.get_active_notes().unwrap().len(), 1);
    }

    #[test]
    fn test_storage_2pc_rollback_on_revert() {
        let store = SqlCipherNoteStore::open_in_memory("test_secret").unwrap();

        let n1 = DbNoteRecord {
            commitment: "cm_in1".into(),
            leaf_index: Some(0),
            value: 10_000_000,
            owner_pk: "0xpk".into(),
            rho: "0xrho1".into(),
            rand: "0xrand1".into(),
            nullifier_key: "0xnk".into(),
            epoch_id: 1,
            status: DbNoteStatus::Active,
            created_at: 1000,
            updated_at: 1000,
        };
        store.insert_note(&n1).unwrap();

        let ch1 = DbNoteRecord {
            commitment: "cm_out1".into(),
            leaf_index: None,
            value: 3_000_000,
            owner_pk: "0xpk".into(),
            rho: "0xc_rho".into(),
            rand: "0xc_rand".into(),
            nullifier_key: "0xnk".into(),
            epoch_id: 1,
            status: DbNoteStatus::Unconfirmed,
            created_at: 1100,
            updated_at: 1100,
        };

        store
            .phase1_pre_broadcast_lock(
                "session_revert",
                &["cm_in1"],
                ("0xnf1", "0xnf_dummy"),
                &[ch1],
                1100,
            )
            .unwrap();

        // Rollback
        store
            .phase2_rollback("session_revert", &["cm_in1"], 1200)
            .unwrap();

        // Note 1 must be ACTIVE again
        assert_eq!(
            store.get_note("cm_in1").unwrap().unwrap().status,
            DbNoteStatus::Active
        );
        // Change note must be deleted
        assert!(store.get_note("cm_out1").unwrap().is_none());
        assert_eq!(store.get_active_notes().unwrap().len(), 1);
    }

    #[test]
    fn test_crash_recovery_simulation() {
        let store = SqlCipherNoteStore::open_in_memory("test_secret").unwrap();

        let n1 = DbNoteRecord {
            commitment: "cm_crash".into(),
            leaf_index: Some(0),
            value: 8_000_000,
            owner_pk: "0xpk".into(),
            rho: "0xrho".into(),
            rand: "0xrand".into(),
            nullifier_key: "0xnk".into(),
            epoch_id: 1,
            status: DbNoteStatus::Active,
            created_at: 1000,
            updated_at: 1000,
        };
        store.insert_note(&n1).unwrap();

        let ch = DbNoteRecord {
            commitment: "cm_ch_crash".into(),
            leaf_index: None,
            value: 2_000_000,
            owner_pk: "0xpk".into(),
            rho: "0xc_rho".into(),
            rand: "0xc_rand".into(),
            nullifier_key: "0xnk".into(),
            epoch_id: 1,
            status: DbNoteStatus::Unconfirmed,
            created_at: 1000,
            updated_at: 1000,
        };

        store
            .phase1_pre_broadcast_lock(
                "session_crash",
                &["cm_crash"],
                ("0xnf", "0xnf_dum"),
                &[ch],
                1000,
            )
            .unwrap();

        // Simulate crash: timeout elapsed (1000 + 300 = 1300 > 1200)
        let recovered = store.recover_pending_sessions(120, 1300).unwrap();
        assert_eq!(recovered, 1);

        assert_eq!(
            store.get_note("cm_crash").unwrap().unwrap().status,
            DbNoteStatus::Active
        );
        assert!(store.get_note("cm_ch_crash").unwrap().is_none());
    }
}
