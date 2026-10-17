use anyhow::Result;
use rusqlite::params;
use tokio::task;

use super::{database_key, open_connection, unix_timestamp, Database};

impl Database {
    /// Mark a spend as CCIP cross-chain and store the message ID for destination tracking.
    pub async fn set_ccip_tracking(
        &self,
        id: i64,
        message_id: &str,
        chain_selector: u64,
    ) -> Result<()> {
        let path = self.path.clone();
        let db_key = database_key()?;
        let message_id = message_id.to_string();
        task::spawn_blocking(move || -> Result<()> {
            let conn = open_connection(&path, &db_key)?;
            let now = unix_timestamp()?;
            let changed = conn.execute(
                "UPDATE spend_queue
                 SET ccip_message_id = ?,
                     destination_chain_selector = ?,
                     destination_status = 'pending',
                     destination_updated_at = ?
                 WHERE id = ?",
                params![message_id, chain_selector as i64, now, id],
            )?;
            if changed != 1 {
                anyhow::bail!("queue item {} not found for CCIP tracking", id);
            }
            Ok(())
        })
        .await?
    }

    /// Get all CCIP spends awaiting destination confirmation.
    /// Returns (id, message_id, chain_selector).
    pub async fn get_pending_ccip_spends(&self) -> Result<Vec<(i64, String, u64)>> {
        let path = self.path.clone();
        let db_key = database_key()?;
        task::spawn_blocking(move || -> Result<Vec<(i64, String, u64)>> {
            let conn = open_connection(&path, &db_key)?;
            let mut stmt = conn.prepare(
                "SELECT id, ccip_message_id, destination_chain_selector
                 FROM spend_queue
                 WHERE destination_status = 'pending'
                   AND ccip_message_id IS NOT NULL
                   AND destination_chain_selector IS NOT NULL",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)? as u64,
                ))
            })?;
            let mut results = Vec::new();
            for row in rows {
                results.push(row?);
            }
            Ok(results)
        })
        .await?
    }

    /// Update destination execution status for a CCIP spend.
    /// status: 'delivered' (success) or 'failed' (failure)
    pub async fn update_destination_status(
        &self,
        id: i64,
        status: &str,
        error: Option<&str>,
    ) -> Result<()> {
        let path = self.path.clone();
        let db_key = database_key()?;
        let status = status.to_string();
        let error = error.map(str::to_string);
        task::spawn_blocking(move || -> Result<()> {
            let conn = open_connection(&path, &db_key)?;
            let now = unix_timestamp()?;
            let changed = conn.execute(
                "UPDATE spend_queue
                 SET destination_status = ?,
                     destination_error = ?,
                     destination_updated_at = ?,
                     status = CASE
                         WHEN ? = 'delivered' THEN 'confirmed'
                         ELSE status
                     END
                 WHERE id = ?",
                params![status, error, now, status, id],
            )?;
            if changed != 1 {
                anyhow::bail!("queue item {} not found for destination status update", id);
            }
            Ok(())
        })
        .await?
    }

    /// Count CCIP spends awaiting destination confirmation.
    pub async fn ccip_pending_count(&self) -> Result<i64> {
        let path = self.path.clone();
        let db_key = database_key()?;
        task::spawn_blocking(move || -> Result<i64> {
            let conn = open_connection(&path, &db_key)?;
            let count = conn.query_row(
                "SELECT COUNT(*) FROM spend_queue WHERE destination_status = 'pending'",
                [],
                |row| row.get::<_, i64>(0),
            )?;
            Ok(count)
        })
        .await?
    }

    /// Count CCIP spends that failed on destination chain.
    pub async fn ccip_failure_count(&self) -> Result<i64> {
        let path = self.path.clone();
        let db_key = database_key()?;
        task::spawn_blocking(move || -> Result<i64> {
            let conn = open_connection(&path, &db_key)?;
            let count = conn.query_row(
                "SELECT COUNT(*) FROM spend_queue WHERE destination_status = 'failed'",
                [],
                |row| row.get::<_, i64>(0),
            )?;
            Ok(count)
        })
        .await?
    }

    /// Authorize refund for a failed CCIP spend: free the nullifier so user can re-spend.
    /// Safety: only allowed if destination_status = 'failed' and refund timeout has elapsed.
    pub async fn authorize_ccip_refund(
        &self,
        nullifier: &str,
        refund_timeout_secs: u64,
    ) -> Result<bool> {
        let path = self.path.clone();
        let db_key = database_key()?;
        let nullifier = nullifier.to_string();
        task::spawn_blocking(move || -> Result<bool> {
            let conn = open_connection(&path, &db_key)?;
            let now = unix_timestamp()?;
            let timeout_threshold = now - refund_timeout_secs as i64;

            // Check if this is a failed CCIP spend that's eligible for refund
            let eligible = conn.query_row(
                "SELECT destination_updated_at FROM spend_queue
                 WHERE nullifier = ?
                   AND destination_status = 'failed'
                   AND destination_updated_at IS NOT NULL
                   AND destination_updated_at <= ?",
                params![nullifier, timeout_threshold],
                |row| row.get::<_, Option<i64>>(0),
            )?;

            if eligible.is_none() {
                return Ok(false);
            }

            // Free the nullifier so user can re-spend
            conn.execute(
                "DELETE FROM nullifiers WHERE nullifier = ?",
                params![nullifier],
            )?;

            // Mark spend as refunded
            conn.execute(
                "UPDATE spend_queue
                 SET status = 'refunded',
                     destination_status = 'refunded',
                     updated_at = ?
                 WHERE nullifier = ?",
                params![now, nullifier],
            )?;

            Ok(true)
        })
        .await?
    }


}
