use anyhow::Result;
use rusqlite::{params, OptionalExtension};
use tokio::task;

use super::types::RelayerTxRecord;
use super::{database_key, open_connection, unix_timestamp, Database};

impl Database {
    // --- Relayer Transactions Tracking (DEC-017 / Receipt Finality & Nonce Tracking) ---

    pub async fn record_relayer_tx(
        &self,
        tx_hash: &str,
        nonce: u64,
        status: &str,
        signer_address: &str,
    ) -> Result<()> {
        let path = self.path.clone();
        let db_key = database_key()?;
        let tx_hash = tx_hash.to_string();
        let status = status.to_string();
        let signer_address = signer_address.to_string();
        task::spawn_blocking(move || -> Result<()> {
            let conn = open_connection(&path, &db_key)?;
            let now = unix_timestamp()?;
            conn.execute(
                "INSERT INTO relayer_transactions (
                    tx_hash, nonce, signer_address, status, confirmations, created_at, updated_at
                 ) VALUES (?, ?, ?, ?, 0, ?, ?)
                 ON CONFLICT(tx_hash) DO UPDATE SET
                    status = excluded.status,
                    updated_at = excluded.updated_at",
                params![tx_hash, nonce as i64, signer_address, status, now, now],
            )?;
            Ok(())
        })
        .await?
    }

    pub async fn update_relayer_tx_confirmed(
        &self,
        tx_hash: &str,
        block_number: u64,
        gas_used: u64,
        effective_gas_price: u128,
        confirmations: u64,
    ) -> Result<()> {
        let path = self.path.clone();
        let db_key = database_key()?;
        let tx_hash = tx_hash.to_string();
        task::spawn_blocking(move || -> Result<()> {
            let conn = open_connection(&path, &db_key)?;
            let now = unix_timestamp()?;
            let eff_price_str = effective_gas_price.to_string();
            conn.execute(
                "UPDATE relayer_transactions
                 SET status = 'confirmed',
                     block_number = ?,
                     gas_used = ?,
                     effective_gas_price = ?,
                     confirmations = ?,
                     updated_at = ?
                 WHERE tx_hash = ?",
                params![
                    block_number as i64,
                    gas_used as i64,
                    eff_price_str,
                    confirmations as i64,
                    now,
                    tx_hash
                ],
            )?;
            Ok(())
        })
        .await?
    }

    pub async fn update_relayer_tx_failed(
        &self,
        tx_hash: &str,
        block_number: Option<u64>,
        gas_used: Option<u64>,
        error_reason: &str,
    ) -> Result<()> {
        let path = self.path.clone();
        let db_key = database_key()?;
        let tx_hash = tx_hash.to_string();
        let error_reason = error_reason.to_string();
        task::spawn_blocking(move || -> Result<()> {
            let conn = open_connection(&path, &db_key)?;
            let now = unix_timestamp()?;
            conn.execute(
                "UPDATE relayer_transactions
                 SET status = 'failed',
                     block_number = ?,
                     gas_used = ?,
                     error_reason = ?,
                     updated_at = ?
                 WHERE tx_hash = ?",
                params![
                    block_number.map(|b| b as i64),
                    gas_used.map(|g| g as i64),
                    error_reason,
                    now,
                    tx_hash
                ],
            )?;
            Ok(())
        })
        .await?
    }

    #[allow(dead_code)]
    pub async fn update_relayer_tx_confirmations(
        &self,
        tx_hash: &str,
        confirmations: u64,
    ) -> Result<()> {
        let path = self.path.clone();
        let db_key = database_key()?;
        let tx_hash = tx_hash.to_string();
        task::spawn_blocking(move || -> Result<()> {
            let conn = open_connection(&path, &db_key)?;
            let now = unix_timestamp()?;
            conn.execute(
                "UPDATE relayer_transactions
                 SET confirmations = ?,
                     updated_at = ?
                 WHERE tx_hash = ?",
                params![confirmations as i64, now, tx_hash],
            )?;
            Ok(())
        })
        .await?
    }

    pub async fn get_max_relayer_nonce(&self, signer_address: &str) -> Result<Option<u64>> {
        let path = self.path.clone();
        let db_key = database_key()?;
        let signer_address = signer_address.to_string();
        task::spawn_blocking(move || -> Result<Option<u64>> {
            let conn = open_connection(&path, &db_key)?;
            let mut stmt = conn.prepare(
                "SELECT MAX(nonce) FROM relayer_transactions
                 WHERE signer_address = ? AND status IN ('pending', 'confirmed')",
            )?;
            let max_nonce: Option<i64> =
                stmt.query_row(params![signer_address], |row| row.get(0))?;
            Ok(max_nonce.map(|n| n as u64))
        })
        .await?
    }

    pub async fn get_relayer_tx(&self, tx_hash: &str) -> Result<Option<RelayerTxRecord>> {
        let path = self.path.clone();
        let db_key = database_key()?;
        let tx_hash = tx_hash.to_string();
        task::spawn_blocking(move || -> Result<Option<RelayerTxRecord>> {
            let conn = open_connection(&path, &db_key)?;
            let mut stmt = conn.prepare(
                "SELECT tx_hash, nonce, signer_address, status, block_number, gas_used,
                        effective_gas_price, confirmations, error_reason, created_at, updated_at
                 FROM relayer_transactions WHERE tx_hash = ?",
            )?;
            let record = stmt
                .query_row(params![tx_hash], |row| {
                    let eff_gas_price: Option<String> = row.get(6)?;
                    let eff_gas_price_u128 = eff_gas_price.and_then(|s| s.parse::<u128>().ok());
                    let block_num: Option<i64> = row.get(4)?;
                    let gas_u: Option<i64> = row.get(5)?;
                    let confs: i64 = row.get(7)?;
                    let c_at: i64 = row.get(9)?;
                    let u_at: i64 = row.get(10)?;
                    Ok(RelayerTxRecord {
                        tx_hash: row.get(0)?,
                        nonce: row.get::<_, i64>(1)? as u64,
                        signer_address: row.get(2)?,
                        status: row.get(3)?,
                        block_number: block_num.map(|b| b as u64),
                        gas_used: gas_u.map(|g| g as u64),
                        effective_gas_price: eff_gas_price_u128,
                        confirmations: confs as u64,
                        error_reason: row.get(8)?,
                        created_at: c_at as u64,
                        updated_at: u_at as u64,
                    })
                })
                .optional()?;
            Ok(record)
        })
        .await?
    }

    pub async fn get_tx_status_any(
        &self,
        tx_hash: &str,
    ) -> Result<Option<crate::dto::TxStatusResponse>> {
        // 1. Check relayer_transactions table
        if let Some(record) = self.get_relayer_tx(tx_hash).await? {
            return Ok(Some(crate::dto::TxStatusResponse {
                tx_hash: record.tx_hash,
                status: record.status,
                block_number: record.block_number,
                gas_used: record.gas_used,
                effective_gas_price: record.effective_gas_price,
                confirmations: record.confirmations,
                error_reason: record.error_reason,
            }));
        }

        // 2. Check spend_queue table
        let path = self.path.clone();
        let db_key = database_key()?;
        let hash = tx_hash.to_string();
        let queue_match = task::spawn_blocking(move || -> Result<Option<crate::dto::TxStatusResponse>> {
            let conn = open_connection(&path, &db_key)?;
            let mut stmt = conn.prepare(
                "SELECT status, block_number, last_error FROM spend_queue WHERE tx_hash = ? LIMIT 1",
            )?;
            let row = stmt.query_row(params![hash], |r| {
                let status_str: String = r.get(0)?;
                let block_num: Option<i64> = r.get(1)?;
                let last_err: Option<String> = r.get(2)?;
                Ok((status_str, block_num, last_err))
            }).optional()?;

            if let Some((raw_status, b_num, err)) = row {
                let status = match raw_status.as_str() {
                    "confirmed" => "confirmed",
                    "failed" => "failed",
                    _ => "pending",
                };
                let confirmations = if status == "confirmed" { 1 } else { 0 };
                return Ok(Some(crate::dto::TxStatusResponse {
                    tx_hash: hash,
                    status: status.to_string(),
                    block_number: b_num.map(|b| b as u64),
                    gas_used: None,
                    effective_gas_price: None,
                    confirmations,
                    error_reason: err,
                }));
            }
            Ok(None)
        })
        .await??;

        if queue_match.is_some() {
            return Ok(queue_match);
        }

        // 3. Check spend_batches table
        let path = self.path.clone();
        let db_key = database_key()?;
        let hash = tx_hash.to_string();
        let batch_match =
            task::spawn_blocking(move || -> Result<Option<crate::dto::TxStatusResponse>> {
                let conn = open_connection(&path, &db_key)?;
                let mut stmt = conn.prepare(
                "SELECT gas_used, effective_gas_price FROM spend_batches WHERE tx_hash = ? LIMIT 1",
            )?;
                let row = stmt
                    .query_row(params![hash], |r| {
                        let gas_u: i64 = r.get(0)?;
                        let eff_p: i64 = r.get(1)?;
                        Ok((gas_u, eff_p))
                    })
                    .optional()?;

                if let Some((gas_u, eff_p)) = row {
                    return Ok(Some(crate::dto::TxStatusResponse {
                        tx_hash: hash,
                        status: "confirmed".to_string(),
                        block_number: None,
                        gas_used: Some(gas_u as u64),
                        effective_gas_price: Some(eff_p as u128),
                        confirmations: 1,
                        error_reason: None,
                    }));
                }
                Ok(None)
            })
            .await??;

        Ok(batch_match)
    }
}
