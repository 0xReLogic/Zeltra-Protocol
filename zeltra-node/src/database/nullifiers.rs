use anyhow::Result;
use rusqlite::{params, OptionalExtension};
use tokio::task;

use super::{database_key, open_connection, Database};

impl Database {
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

    /// Check and insert dual nullifiers atomically in a single transaction (DEC-036C C2.1).
    /// Both nullifiers must not exist, and nullifier_1 must not equal nullifier_2.
    /// If either already exists, returns Ok(false) with no state changes.
    pub async fn check_and_insert_dual_nullifiers(
        &self,
        nullifier_1: &str,
        nullifier_2: &str,
        tx_hash: Option<&str>,
    ) -> Result<bool> {
        if nullifier_1 == nullifier_2 {
            return Ok(false);
        }
        let path = self.path.clone();
        let nf1 = nullifier_1.to_string();
        let nf2 = nullifier_2.to_string();
        let tx_hash = tx_hash.map(|s| s.to_string());
        let db_key = database_key()?;

        task::spawn_blocking(move || -> Result<bool> {
            let mut conn = open_connection(&path, &db_key)?;
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_secs() as i64;

            let tx = conn.transaction()?;

            let exists_1: Option<i64> = tx
                .query_row(
                    "SELECT 1 FROM nullifiers WHERE nullifier = ?",
                    params![nf1],
                    |row| row.get(0),
                )
                .optional()?;

            let exists_2: Option<i64> = tx
                .query_row(
                    "SELECT 1 FROM nullifiers WHERE nullifier = ?",
                    params![nf2],
                    |row| row.get(0),
                )
                .optional()?;

            if exists_1.is_some() || exists_2.is_some() {
                return Ok(false);
            }

            tx.execute(
                "INSERT INTO nullifiers (nullifier, spent_at, tx_hash) VALUES (?, ?, ?)",
                params![nf1, now, tx_hash],
            )?;
            tx.execute(
                "INSERT INTO nullifiers (nullifier, spent_at, tx_hash) VALUES (?, ?, ?)",
                params![nf2, now, tx_hash],
            )?;

            tx.commit()?;
            Ok(true)
        })
        .await?
    }

    /// Releases reserved nullifiers if downstream simulation, preflight, or broadcast fails (DEC-036C).
    pub async fn release_nullifiers(&self, nullifier_1: &str, nullifier_2: &str) -> Result<()> {
        let path = self.path.clone();
        let nf1 = nullifier_1.to_string();
        let nf2 = nullifier_2.to_string();
        let db_key = database_key()?;

        task::spawn_blocking(move || -> Result<()> {
            let mut conn = open_connection(&path, &db_key)?;
            let tx = conn.transaction()?;
            tx.execute("DELETE FROM nullifiers WHERE nullifier = ?", params![nf1])?;
            tx.execute("DELETE FROM nullifiers WHERE nullifier = ?", params![nf2])?;
            tx.commit()?;
            Ok(())
        })
        .await?
    }


}
