use anyhow::Result;
use rusqlite::{params, OptionalExtension};
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
            let eligible = conn
                .query_row(
                    "SELECT destination_updated_at FROM spend_queue
                     WHERE nullifier = ?
                       AND destination_status = 'failed'
                       AND destination_updated_at IS NOT NULL
                       AND destination_updated_at <= ?",
                    params![nullifier, timeout_threshold],
                    |row| row.get::<_, Option<i64>>(0),
                )
                .optional()?;

            if eligible.is_none() || eligible.flatten().is_none() {
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

    /// Record a new CCIP inbound programmable deposit in tracking table (DEC-055 / DEC-056)
    pub async fn record_ccip_inbound_deposit(
        &self,
        record: &CcipInboundDepositRecord,
    ) -> Result<()> {
        let path = self.path.clone();
        let db_key = database_key()?;
        let rec = record.clone();
        task::spawn_blocking(move || -> Result<()> {
            let conn = open_connection(&path, &db_key)?;
            let now = unix_timestamp()?;
            conn.execute(
                "INSERT INTO ccip_inbound_deposits (
                     message_id, source_chain_selector, asset_id, amount,
                     leaf_index, note_commitment, status, tx_hash, block_number, updated_at
                 ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                 ON CONFLICT(message_id) DO UPDATE SET
                     status = excluded.status,
                     leaf_index = COALESCE(excluded.leaf_index, ccip_inbound_deposits.leaf_index),
                     tx_hash = COALESCE(excluded.tx_hash, ccip_inbound_deposits.tx_hash),
                     block_number = COALESCE(excluded.block_number, ccip_inbound_deposits.block_number),
                     updated_at = excluded.updated_at",
                params![
                    rec.message_id,
                    rec.source_chain_selector as i64,
                    rec.asset_id as i64,
                    rec.amount as i64,
                    rec.leaf_index.map(|idx| idx as i64),
                    rec.note_commitment,
                    rec.status,
                    rec.tx_hash,
                    rec.block_number.map(|b| b as i64),
                    now,
                ],
            )?;
            Ok(())
        })
        .await?
    }

    /// Retrieve an inbound CCIP deposit record by message ID
    pub async fn get_ccip_inbound_deposit(
        &self,
        message_id: &str,
    ) -> Result<Option<CcipInboundDepositRecord>> {
        let path = self.path.clone();
        let db_key = database_key()?;
        let message_id = message_id.to_string();
        task::spawn_blocking(move || -> Result<Option<CcipInboundDepositRecord>> {
            let conn = open_connection(&path, &db_key)?;
            let mut stmt = conn.prepare(
                "SELECT message_id, source_chain_selector, asset_id, amount,
                        leaf_index, note_commitment, status, tx_hash, block_number, updated_at
                 FROM ccip_inbound_deposits
                 WHERE message_id = ?",
            )?;
            let mut rows = stmt.query(params![message_id])?;
            if let Some(row) = rows.next()? {
                Ok(Some(CcipInboundDepositRecord {
                    message_id: row.get::<_, String>(0)?,
                    source_chain_selector: row.get::<_, i64>(1)? as u64,
                    asset_id: row.get::<_, i64>(2)? as u8,
                    amount: row.get::<_, i64>(3)? as u64,
                    leaf_index: row.get::<_, Option<i64>>(4)?.map(|idx| idx as u64),
                    note_commitment: row.get::<_, String>(5)?,
                    status: row.get::<_, String>(6)?,
                    tx_hash: row.get::<_, Option<String>>(7)?,
                    block_number: row.get::<_, Option<i64>>(8)?.map(|b| b as u64),
                    updated_at: row.get::<_, i64>(9)?,
                }))
            } else {
                Ok(None)
            }
        })
        .await?
    }

    /// Retrieve all pending / in-flight inbound deposits to monitor
    pub async fn get_pending_ccip_deposits(&self) -> Result<Vec<CcipInboundDepositRecord>> {
        let path = self.path.clone();
        let db_key = database_key()?;
        task::spawn_blocking(move || -> Result<Vec<CcipInboundDepositRecord>> {
            let conn = open_connection(&path, &db_key)?;
            let mut stmt = conn.prepare(
                "SELECT message_id, source_chain_selector, asset_id, amount,
                        leaf_index, note_commitment, status, tx_hash, block_number, updated_at
                 FROM ccip_inbound_deposits
                 WHERE status != 'confirmed_in_pool' AND status != 'failed'",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok(CcipInboundDepositRecord {
                    message_id: row.get::<_, String>(0)?,
                    source_chain_selector: row.get::<_, i64>(1)? as u64,
                    asset_id: row.get::<_, i64>(2)? as u8,
                    amount: row.get::<_, i64>(3)? as u64,
                    leaf_index: row.get::<_, Option<i64>>(4)?.map(|idx| idx as u64),
                    note_commitment: row.get::<_, String>(5)?,
                    status: row.get::<_, String>(6)?,
                    tx_hash: row.get::<_, Option<String>>(7)?,
                    block_number: row.get::<_, Option<i64>>(8)?.map(|b| b as u64),
                    updated_at: row.get::<_, i64>(9)?,
                })
            })?;
            let mut results = Vec::new();
            for r in rows {
                results.push(r?);
            }
            Ok(results)
        })
        .await?
    }

    /// Mark an inbound CCIP deposit as confirmed in pool with its minted leaf index
    pub async fn confirm_ccip_inbound_deposit(
        &self,
        message_id: &str,
        leaf_index: u64,
        tx_hash: &str,
        block_number: u64,
    ) -> Result<()> {
        let path = self.path.clone();
        let db_key = database_key()?;
        let message_id = message_id.to_string();
        let tx_hash = tx_hash.to_string();
        task::spawn_blocking(move || -> Result<()> {
            let conn = open_connection(&path, &db_key)?;
            let now = unix_timestamp()?;
            conn.execute(
                "UPDATE ccip_inbound_deposits
                 SET status = 'confirmed_in_pool',
                     leaf_index = ?,
                     tx_hash = ?,
                     block_number = ?,
                     updated_at = ?
                 WHERE message_id = ?",
                params![leaf_index as i64, tx_hash, block_number as i64, now, message_id],
            )?;
            Ok(())
        })
        .await?
    }

    /// Get outbound CCIP spend tracking record by message ID
    pub async fn get_ccip_spend_by_message_id(
        &self,
        message_id: &str,
    ) -> Result<Option<CcipSpendRecord>> {
        let path = self.path.clone();
        let db_key = database_key()?;
        let message_id = message_id.to_string();
        task::spawn_blocking(move || -> Result<Option<CcipSpendRecord>> {
            let conn = open_connection(&path, &db_key)?;
            let mut stmt = conn.prepare(
                "SELECT id, nullifier, recipient, amount, ccip_message_id,
                        destination_chain_selector, destination_status, destination_error,
                        tx_hash, block_number, updated_at
                 FROM spend_queue
                 WHERE ccip_message_id = ?",
            )?;
            let mut rows = stmt.query(params![message_id])?;
            if let Some(row) = rows.next()? {
                Ok(Some(CcipSpendRecord {
                    id: row.get::<_, i64>(0)?,
                    nullifier: row.get::<_, String>(1)?,
                    recipient: row.get::<_, String>(2)?,
                    amount: row.get::<_, i64>(3)? as u64,
                    ccip_message_id: row.get::<_, String>(4)?,
                    destination_chain_selector: row.get::<_, i64>(5)? as u64,
                    destination_status: row.get::<_, String>(6)?,
                    destination_error: row.get::<_, Option<String>>(7)?,
                    tx_hash: row.get::<_, Option<String>>(8)?,
                    block_number: row.get::<_, Option<i64>>(9)?.map(|b| b as u64),
                    updated_at: row.get::<_, i64>(10)?,
                }))
            } else {
                Ok(None)
            }
        })
        .await?
    }
}

/// Data structure for an inbound CCIP programmable deposit record (DEC-055 / DEC-056)
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct CcipInboundDepositRecord {
    pub message_id: String,
    pub source_chain_selector: u64,
    pub asset_id: u8,
    pub amount: u64,
    pub leaf_index: Option<u64>,
    pub note_commitment: String,
    pub status: String, // "in_flight", "confirmed_in_pool", "failed"
    pub tx_hash: Option<String>,
    pub block_number: Option<u64>,
    pub updated_at: i64,
}

/// Data structure for an outbound CCIP spend tracking record (DEC-056)
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct CcipSpendRecord {
    pub id: i64,
    pub nullifier: String,
    pub recipient: String,
    pub amount: u64,
    pub ccip_message_id: String,
    pub destination_chain_selector: u64,
    pub destination_status: String, // "pending", "delivered", "failed"
    pub destination_error: Option<String>,
    pub tx_hash: Option<String>,
    pub block_number: Option<u64>,
    pub updated_at: i64,
}
