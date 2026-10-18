use anyhow::{Context, Result};
use rusqlite::{params, OptionalExtension};
use tokio::task;

use super::types::UnclaimedBatchInfo;
use super::{database_key, open_connection, unix_timestamp, Database};

impl Database {
    /// Check if quote_id has been used and atomically insert it if not
    /// Returns true if successfully inserted (not used before), false if already exists
    pub async fn check_and_insert_quote_id(
        &self,
        quote_id: &str,
        user_address: &str,
    ) -> Result<bool> {
        let path = self.path.clone();
        let quote_id = quote_id.to_string();
        let user_address = user_address.to_string();
        let db_key = database_key()?;

        task::spawn_blocking(move || -> Result<bool> {
            let conn = open_connection(&path, &db_key)?;
            let now = unix_timestamp()?;

            // ATOMIC: INSERT OR IGNORE (race-safe!)
            let changes = conn.execute(
                "INSERT OR IGNORE INTO quote_ids_used (quote_id, user_address, used_at)
                 VALUES (?, ?, ?)",
                params![quote_id, user_address, now],
            )?;

            Ok(changes > 0)
        })
        .await?
    }

    /// Check if quote_id exists (read-only check)
    #[allow(dead_code)]
    pub async fn is_quote_id_used(&self, quote_id: &str) -> Result<bool> {
        let path = self.path.clone();
        let quote_id = quote_id.to_string();
        let db_key = database_key()?;

        task::spawn_blocking(move || -> Result<bool> {
            let conn = open_connection(&path, &db_key)?;
            let exists: Option<i64> = conn
                .query_row(
                    "SELECT 1 FROM quote_ids_used WHERE quote_id = ?",
                    params![quote_id],
                    |row| row.get(0),
                )
                .optional()?;

            Ok(exists.is_some())
        })
        .await?
    }

    /// Get total unclaimed execution fees and batch IDs
    /// Returns (total_fee_usdc, batch_ids)
    pub async fn get_unclaimed_execution_fees(&self) -> Result<(u64, Vec<String>)> {
        let path = self.path.clone();
        let db_key = database_key()?;

        task::spawn_blocking(move || -> Result<(u64, Vec<String>)> {
            let conn = open_connection(&path, &db_key)?;

            let total: i64 = conn
                .query_row(
                    "SELECT COALESCE(SUM(execution_fee_usdc), 0)
                     FROM spend_batches
                     WHERE claimed = 0",
                    [],
                    |row| row.get(0),
                )
                .context("Failed to sum unclaimed execution fees")?;

            let mut stmt = conn
                .prepare("SELECT batch_id FROM spend_batches WHERE claimed = 0")
                .context("Failed to prepare batch ID query")?;

            let batch_ids: Vec<String> = stmt
                .query_map([], |row| row.get(0))
                .context("Failed to query unclaimed batch IDs")?
                .filter_map(|r| r.ok())
                .collect();

            Ok((total as u64, batch_ids))
        })
        .await?
    }

    /// Mark batches as claimed after successful on-chain claim
    pub async fn mark_batches_claimed(&self, batch_ids: &[String]) -> Result<()> {
        let path = self.path.clone();
        let batch_ids = batch_ids.to_vec();
        let db_key = database_key()?;

        task::spawn_blocking(move || -> Result<()> {
            let conn = open_connection(&path, &db_key)?;
            let now = unix_timestamp()?;

            for batch_id in batch_ids {
                conn.execute(
                    "UPDATE spend_batches SET claimed = 1, claimed_at = ? WHERE batch_id = ?",
                    params![now, batch_id],
                )?;
            }

            Ok(())
        })
        .await?
    }

    /// Get batch IDs that have unclaimed execution fees
    #[allow(dead_code)]
    pub async fn get_unclaimed_batch_ids(&self) -> Result<Vec<String>> {
        let path = self.path.clone();
        let db_key = database_key()?;

        task::spawn_blocking(move || -> Result<Vec<String>> {
            let conn = open_connection(&path, &db_key)?;

            let mut stmt = conn
                .prepare("SELECT batch_id FROM spend_batches WHERE claimed = 0")
                .context("Failed to prepare unclaimed batch ID query")?;

            let batch_ids: Vec<String> = stmt
                .query_map([], |row| row.get(0))
                .context("Failed to query unclaimed batch IDs")?
                .filter_map(|r| r.ok())
                .collect();

            Ok(batch_ids)
        })
        .await?
    }

    /// Get unclaimed batches with metadata for receipt and accounting reconciliation (DEC-031)
    pub async fn get_unclaimed_batches_with_details(&self) -> Result<Vec<UnclaimedBatchInfo>> {
        let path = self.path.clone();
        let db_key = database_key()?;

        task::spawn_blocking(move || -> Result<Vec<UnclaimedBatchInfo>> {
            let conn = open_connection(&path, &db_key)?;

            let mut stmt = conn
                .prepare(
                    "SELECT batch_id, tx_hash, execution_fee_usdc, item_count, created_at \
                     FROM spend_batches WHERE claimed = 0 ORDER BY created_at ASC",
                )
                .context("Failed to prepare unclaimed batch query")?;

            let batches: Vec<UnclaimedBatchInfo> = stmt
                .query_map([], |row| {
                    Ok(UnclaimedBatchInfo {
                        batch_id: row.get(0)?,
                        tx_hash: row.get(1)?,
                        execution_fee_usdc: row.get::<_, i64>(2)? as u64,
                        item_count: row.get::<_, i64>(3)? as usize,
                        created_at: row.get(4)?,
                    })
                })
                .context("Failed to query unclaimed batch details")?
                .filter_map(|r| r.ok())
                .collect();

            Ok(batches)
        })
        .await?
    }
}
