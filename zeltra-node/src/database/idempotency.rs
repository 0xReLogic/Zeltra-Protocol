use anyhow::Result;
use rusqlite::{params, OptionalExtension};
use tokio::task;

use super::types::DbStats;
use super::{database_key, open_connection, Database};

impl Database {
    /// Get database statistics (for monitoring)
    pub async fn get_stats(&self) -> Result<DbStats> {
        let path = self.path.clone();

        // Get encryption key - TODO: Integrate with KMS (OpenBao/Vault) for production
        let db_key = database_key()?;

        task::spawn_blocking(move || -> Result<DbStats> {
            let conn = open_connection(&path, &db_key)?;

            let total_nullifiers: i64 =
                conn.query_row("SELECT COUNT(*) FROM nullifiers", [], |row| row.get(0))?;

            let queued_spends: i64 = conn.query_row(
                "SELECT COUNT(*) FROM spend_queue WHERE processed = 0",
                [],
                |row| row.get(0),
            )?;

            let (
                batch_count,
                total_batch_margin_usdc,
                avg_batch_size,
                total_execution_fees_usdc,
                claimed_execution_fees_usdc,
            ): (i64, f64, f64, i64, i64) = conn.query_row(
                "SELECT 
                    COUNT(*), 
                    COALESCE(SUM(margin_usdc), 0.0), 
                    COALESCE(AVG(item_count), 0.0),
                    COALESCE(SUM(execution_fee_usdc), 0),
                    COALESCE(SUM(CASE WHEN claimed = 1 THEN execution_fee_usdc ELSE 0 END), 0)
                 FROM spend_batches",
                [],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                    ))
                },
            )?;

            let unclaimed_execution_fees_usdc =
                total_execution_fees_usdc.saturating_sub(claimed_execution_fees_usdc) as u64;

            Ok(DbStats {
                total_nullifiers,
                queued_spends,
                batch_count,
                total_batch_margin_usdc,
                avg_batch_size,
                total_execution_fees_usdc: total_execution_fees_usdc as u64,
                claimed_execution_fees_usdc: claimed_execution_fees_usdc as u64,
                unclaimed_execution_fees_usdc,
            })
        })
        .await?
    }

    /// Query batch profitability and operational fee metrics directly (DEC-020)
    pub async fn get_batch_metrics(&self) -> Result<crate::dto::BatchMetricsDto> {
        let stats = self.get_stats().await?;
        Ok(crate::dto::BatchMetricsDto {
            batch_count: stats.batch_count,
            total_batch_margin_usdc: stats.total_batch_margin_usdc,
            avg_batch_size: stats.avg_batch_size,
            total_execution_fees_usdc: stats.total_execution_fees_usdc,
            claimed_execution_fees_usdc: stats.claimed_execution_fees_usdc,
            unclaimed_execution_fees_usdc: stats.unclaimed_execution_fees_usdc,
        })
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
