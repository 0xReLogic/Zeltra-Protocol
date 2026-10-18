use anyhow::Result;
use rusqlite::{params, OptionalExtension, TransactionBehavior};
use tokio::task;

use super::types::{QueuedSpend, UnreconciledSpend};
use super::{database_key, open_connection, unix_timestamp, Database};

impl Database {
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
             SET status = 'submitted', tx_hash = ?, updated_at = ?
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

    pub async fn mark_batch_spend_confirmed(
        &self,
        items: Vec<(i64, String)>,
        tx_hash: &str,
        block_number: u64,
    ) -> Result<()> {
        let path = self.path.clone();
        let db_key = database_key()?;
        let tx_hash = tx_hash.to_string();

        task::spawn_blocking(move || -> Result<()> {
            let mut conn = open_connection(&path, &db_key)?;
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let now = unix_timestamp()?;

            for (id, nullifier) in items {
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
            }
            tx.commit()?;
            Ok(())
        })
        .await?
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn store_batch_metadata(
        &self,
        batch_id: &str,
        item_count: usize,
        tx_hash: &str,
        gas_used: u64,
        effective_gas_price: u128,
        total_cost_eth: f64,
        margin_usdc: f64,
        execution_fee_usdc: u64,
    ) -> Result<()> {
        let path = self.path.clone();
        let db_key = database_key()?;
        let batch_id = batch_id.to_string();
        let tx_hash = tx_hash.to_string();

        task::spawn_blocking(move || -> Result<()> {
            let conn = open_connection(&path, &db_key)?;
            let now = unix_timestamp()?;
            conn.execute(
                "INSERT INTO spend_batches (batch_id, item_count, tx_hash, gas_used, effective_gas_price, total_cost_eth, margin_usdc, created_at, execution_fee_usdc, claimed)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 0)",
                params![
                    batch_id,
                    item_count as i64,
                    tx_hash,
                    gas_used as i64,
                    effective_gas_price as i64,
                    total_cost_eth,
                    margin_usdc,
                    now,
                    execution_fee_usdc as i64,
                ],
            )?;
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

    /// Retrieve active/in-flight spends for startup reconciliation (DEC-019).
    pub async fn get_unreconciled_spends(&self) -> Result<Vec<UnreconciledSpend>> {
        let path = self.path.clone();
        let db_key = database_key()?;
        task::spawn_blocking(move || -> Result<Vec<UnreconciledSpend>> {
            let conn = open_connection(&path, &db_key)?;
            let mut stmt = conn.prepare(
                "SELECT id, nullifier, tx_hash, status
                 FROM spend_queue
                 WHERE status IN ('queued', 'retryable', 'broadcasting', 'submitted')",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok(UnreconciledSpend {
                    id: row.get::<_, i64>(0)?,
                    nullifier: row.get::<_, String>(1)?,
                    tx_hash: row.get::<_, Option<String>>(2)?,
                    status: row.get::<_, String>(3)?,
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

    /// Mark spend as confirmed during reconciliation because nullifier was already spent on-chain (DEC-019).
    pub async fn reconcile_spend_as_confirmed(
        &self,
        id: i64,
        nullifier: &str,
        tx_hash: Option<&str>,
    ) -> Result<()> {
        let path = self.path.clone();
        let db_key = database_key()?;
        let nullifier = nullifier.to_string();
        let tx_hash = tx_hash.unwrap_or("0x_reconciled_onchain_spend").to_string();

        task::spawn_blocking(move || -> Result<()> {
            let mut conn = open_connection(&path, &db_key)?;
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let now = unix_timestamp()?;

            tx.execute(
                "INSERT OR IGNORE INTO nullifiers (nullifier, spent_at, tx_hash)
                 VALUES (?, ?, ?)",
                params![nullifier, now, tx_hash],
            )?;

            tx.execute(
                "UPDATE spend_queue
                 SET status = 'confirmed', processed = 1, tx_hash = COALESCE(tx_hash, ?),
                     lease_until = NULL, updated_at = ?
                 WHERE id = ?",
                params![tx_hash, now, id],
            )?;

            tx.commit()?;
            Ok(())
        })
        .await?
    }

    /// Release expired lease on restart if spend is not yet spent on-chain (DEC-019).
    pub async fn reset_unspent_lease(&self, id: i64) -> Result<()> {
        let path = self.path.clone();
        let db_key = database_key()?;
        task::spawn_blocking(move || -> Result<()> {
            let conn = open_connection(&path, &db_key)?;
            let now = unix_timestamp()?;
            conn.execute(
                "UPDATE spend_queue
                 SET status = 'queued', lease_until = NULL, updated_at = ?
                 WHERE id = ? AND status IN ('broadcasting', 'submitted')",
                params![now, id],
            )?;
            Ok(())
        })
        .await?
    }

    /// Retrieve active/in-flight spends whose leases have expired (DEC-027).
    pub async fn get_expired_leased_spends(&self, now: i64) -> Result<Vec<UnreconciledSpend>> {
        let path = self.path.clone();
        let db_key = database_key()?;
        task::spawn_blocking(move || -> Result<Vec<UnreconciledSpend>> {
            let conn = open_connection(&path, &db_key)?;
            let mut stmt = conn.prepare(
                "SELECT id, nullifier, tx_hash, status
                 FROM spend_queue
                 WHERE status IN ('broadcasting', 'submitted')
                   AND lease_until IS NOT NULL
                   AND lease_until <= ?",
            )?;
            let rows = stmt.query_map(params![now], |row| {
                Ok(UnreconciledSpend {
                    id: row.get::<_, i64>(0)?,
                    nullifier: row.get::<_, String>(1)?,
                    tx_hash: row.get::<_, Option<String>>(2)?,
                    status: row.get::<_, String>(3)?,
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

    /// Mark an unspent spend as failed_expired to release the nullifier lease (DEC-027).
    pub async fn expire_unspent_lease(&self, id: i64) -> Result<()> {
        let path = self.path.clone();
        let db_key = database_key()?;
        task::spawn_blocking(move || -> Result<()> {
            let conn = open_connection(&path, &db_key)?;
            let now = unix_timestamp()?;
            conn.execute(
                "UPDATE spend_queue
                 SET status = 'failed_expired',
                     last_error = 'mempool lease expired without on-chain inclusion (DEC-027)',
                     lease_until = NULL,
                     updated_at = ?
                 WHERE id = ? AND status IN ('broadcasting', 'submitted')",
                params![now, id],
            )?;
            Ok(())
        })
        .await?
    }

    pub async fn get_oldest_queued_spend_timestamp(&self) -> Result<Option<i64>> {
        let path = self.path.clone();
        let db_key = database_key()?;
        task::spawn_blocking(move || -> Result<Option<i64>> {
            let conn = open_connection(&path, &db_key)?;
            let mut stmt = conn.prepare(
                "SELECT MIN(created_at) FROM spend_queue WHERE status IN ('queued', 'retryable')",
            )?;
            let timestamp: Option<i64> = stmt.query_row([], |row| row.get(0))?;
            Ok(timestamp)
        })
        .await?
    }
}
