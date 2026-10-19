use anyhow::Result;
use rusqlite::{params, OptionalExtension, TransactionBehavior};
use tokio::task;

use super::types::{LeafMemoRow, MmrLeafRecord, MmrStateRecord};
use super::{database_key, open_connection, Database};

impl Database {
    /// Insert an indexed MMR leaf note commitment (DEC-035C / DEC-037)
    #[allow(dead_code)]
    pub async fn insert_mmr_leaf(
        &self,
        leaf_index: u64,
        commitment: &str,
        tx_hash: &str,
        block_number: u64,
        encrypted_memo: &[u8],
    ) -> Result<()> {
        let path = self.path.clone();
        let commitment = commitment.to_string();
        let tx_hash = tx_hash.to_string();
        let memo = encrypted_memo.to_vec();
        let db_key = database_key()?;

        task::spawn_blocking(move || -> Result<()> {
            let conn = open_connection(&path, &db_key)?;
            conn.execute(
                "INSERT INTO mmr_leaves (leaf_index, commitment, tx_hash, block_number, encrypted_memo)
                 VALUES (?, ?, ?, ?, ?)",
                params![
                    leaf_index as i64,
                    commitment,
                    tx_hash,
                    block_number as i64,
                    memo
                ],
            )?;
            Ok(())
        })
        .await?
    }

    /// Batch insert multiple indexed MMR leaf commitments inside a single transaction (DEC-035C / DEC-044)
    pub async fn batch_insert_mmr_leaves(
        &self,
        leaves: Vec<(u64, String, String, u64, Vec<u8>)>,
    ) -> Result<()> {
        let path = self.path.clone();
        let db_key = database_key()?;

        task::spawn_blocking(move || -> Result<()> {
            let mut conn = open_connection(&path, &db_key)?;
            let tx = conn.transaction()?;
            {
                let mut stmt = tx.prepare(
                    "INSERT INTO mmr_leaves (leaf_index, commitment, tx_hash, block_number, encrypted_memo)
                     VALUES (?, ?, ?, ?, ?)",
                )?;
                for (leaf_index, commitment, tx_hash, block_number, memo) in leaves {
                    stmt.execute(params![
                        leaf_index as i64,
                        commitment,
                        tx_hash,
                        block_number as i64,
                        memo
                    ])?;
                }
            }
            tx.commit()?;
            Ok(())
        })
        .await?
    }

    /// Retrieve an MMR leaf record by leaf index (DEC-035C / DEC-037)
    pub async fn get_mmr_leaf(&self, leaf_index: u64) -> Result<Option<MmrLeafRecord>> {
        let path = self.path.clone();
        let db_key = database_key()?;

        task::spawn_blocking(move || -> Result<Option<MmrLeafRecord>> {
            let conn = open_connection(&path, &db_key)?;
            let record = conn
                .query_row(
                    "SELECT leaf_index, commitment, tx_hash, block_number, encrypted_memo, created_at
                     FROM mmr_leaves WHERE leaf_index = ?",
                    params![leaf_index as i64],
                    |row| {
                        Ok(MmrLeafRecord {
                            leaf_index: row.get::<_, i64>(0)? as u64,
                            commitment: row.get(1)?,
                            tx_hash: row.get(2)?,
                            block_number: row.get::<_, i64>(3)? as u64,
                            encrypted_memo: row.get::<_, Vec<u8>>(4).unwrap_or_default(),
                            created_at: row.get(5)?,
                        })
                    },
                )
                .optional()?;
            Ok(record)
        })
        .await?
    }

    /// Retrieve all indexed MMR leaves ordered by leaf index ascending (DEC-035C / DEC-037)
    #[allow(dead_code)]
    pub async fn get_all_mmr_leaves(&self) -> Result<Vec<MmrLeafRecord>> {
        let path = self.path.clone();
        let db_key = database_key()?;

        task::spawn_blocking(move || -> Result<Vec<MmrLeafRecord>> {
            let conn = open_connection(&path, &db_key)?;
            let mut stmt = conn.prepare(
                "SELECT leaf_index, commitment, tx_hash, block_number, encrypted_memo, created_at
                 FROM mmr_leaves ORDER BY leaf_index ASC",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok(MmrLeafRecord {
                    leaf_index: row.get::<_, i64>(0)? as u64,
                    commitment: row.get(1)?,
                    tx_hash: row.get(2)?,
                    block_number: row.get::<_, i64>(3)? as u64,
                    encrypted_memo: row.get::<_, Vec<u8>>(4).unwrap_or_default(),
                    created_at: row.get(5)?,
                })
            })?;
            let mut leaves = Vec::new();
            for row in rows {
                leaves.push(row?);
            }
            Ok(leaves)
        })
        .await?
    }

    /// Retrieve paginated chunk of note commitments and encrypted memos for client sync (DEC-037).
    pub async fn get_notes_memo_chunk(
        &self,
        from_leaf: u64,
        limit: u64,
    ) -> Result<Vec<LeafMemoRow>> {
        let path = self.path.clone();
        let db_key = database_key()?;

        task::spawn_blocking(move || -> Result<Vec<LeafMemoRow>> {
            let conn = open_connection(&path, &db_key)?;
            let mut stmt = conn.prepare(
                "SELECT leaf_index, commitment, encrypted_memo, block_number, tx_hash
                 FROM mmr_leaves
                 WHERE leaf_index >= ?
                 ORDER BY leaf_index ASC
                 LIMIT ?",
            )?;
            let rows = stmt.query_map(params![from_leaf as i64, limit as i64], |row| {
                let memo_bytes: Vec<u8> = row.get::<_, Vec<u8>>(2).unwrap_or_default();
                let memo_hex = if memo_bytes.is_empty() {
                    String::new()
                } else {
                    format!("0x{}", hex::encode(memo_bytes))
                };
                Ok(LeafMemoRow {
                    leaf_index: row.get::<_, i64>(0)? as u64,
                    commitment: row.get(1)?,
                    encrypted_memo: memo_hex,
                    block_number: row.get::<_, i64>(3)? as u64,
                    tx_hash: row.get(4)?,
                })
            })?;
            let mut list = Vec::new();
            for r in rows {
                list.push(r?);
            }
            Ok(list)
        })
        .await?
    }

    /// Save MMR state snapshot (leaf count, bagged root, peaks JSON, checkpoint block) (DEC-035C)
    #[allow(dead_code)]
    pub async fn save_mmr_state(
        &self,
        mmr: &zeltra_core::MerkleMountainRange,
        last_indexed_block: u64,
    ) -> Result<()> {
        let path = self.path.clone();
        let leaf_count = mmr.leaf_count as u64;
        let root_bytes = zeltra_core::fr_to_be_bytes(&mmr.get_root());
        let bagged_root = format!("0x{}", hex::encode(root_bytes));
        let peaks: Vec<String> = mmr
            .get_peaks()
            .iter()
            .map(|p| format!("0x{}", hex::encode(zeltra_core::fr_to_be_bytes(p))))
            .collect();
        let peaks_json = serde_json::to_string(&peaks)?;
        let db_key = database_key()?;

        task::spawn_blocking(move || -> Result<()> {
            let conn = open_connection(&path, &db_key)?;
            conn.execute(
                "INSERT INTO mmr_state (id, leaf_count, bagged_root, peaks_json, last_indexed_block, updated_at)
                 VALUES (1, ?, ?, ?, ?, CURRENT_TIMESTAMP)
                 ON CONFLICT(id) DO UPDATE SET
                     leaf_count = excluded.leaf_count,
                     bagged_root = excluded.bagged_root,
                     peaks_json = excluded.peaks_json,
                     last_indexed_block = excluded.last_indexed_block,
                     updated_at = CURRENT_TIMESTAMP",
                params![leaf_count as i64, bagged_root, peaks_json, last_indexed_block as i64],
            )?;
            Ok(())
        })
        .await?
    }

    /// Retrieve active MMR state snapshot (DEC-035C)
    pub async fn get_mmr_state(&self) -> Result<Option<MmrStateRecord>> {
        let path = self.path.clone();
        let db_key = database_key()?;

        task::spawn_blocking(move || -> Result<Option<MmrStateRecord>> {
            let conn = open_connection(&path, &db_key)?;
            let row_opt = conn
                .query_row(
                    "SELECT leaf_count, bagged_root, peaks_json, last_indexed_block, updated_at
                     FROM mmr_state WHERE id = 1",
                    [],
                    |row| {
                        let leaf_count = row.get::<_, i64>(0)? as u64;
                        let bagged_root = row.get::<_, String>(1)?;
                        let peaks_json = row.get::<_, String>(2)?;
                        let last_indexed_block = row.get::<_, i64>(3)? as u64;
                        let updated_at = row.get::<_, Option<String>>(4)?;
                        Ok((
                            leaf_count,
                            bagged_root,
                            peaks_json,
                            last_indexed_block,
                            updated_at,
                        ))
                    },
                )
                .optional()?;

            match row_opt {
                Some((leaf_count, bagged_root, peaks_json, last_indexed_block, updated_at)) => {
                    let peaks: Vec<String> = serde_json::from_str(&peaks_json)?;
                    Ok(Some(MmrStateRecord {
                        leaf_count,
                        bagged_root,
                        peaks,
                        last_indexed_block,
                        updated_at,
                    }))
                }
                None => Ok(None),
            }
        })
        .await?
    }

    /// Get last indexed block for MMR indexer worker (DEC-035C)
    pub async fn get_mmr_last_indexed_block(&self) -> Result<Option<u64>> {
        let path = self.path.clone();
        let db_key = database_key()?;

        task::spawn_blocking(move || -> Result<Option<u64>> {
            let conn = open_connection(&path, &db_key)?;
            let block: Option<i64> = conn
                .query_row(
                    "SELECT last_indexed_block FROM mmr_state WHERE id = 1",
                    [],
                    |row| row.get(0),
                )
                .optional()?;
            Ok(block.map(|b| b as u64))
        })
        .await?
    }

    /// Update last indexed block for MMR indexer checkpoint (DEC-035C)
    pub async fn update_mmr_indexed_block(&self, last_block: u64) -> Result<()> {
        let path = self.path.clone();
        let db_key = database_key()?;

        task::spawn_blocking(move || -> Result<()> {
            let conn = open_connection(&path, &db_key)?;
            conn.execute(
                "INSERT INTO mmr_state (id, leaf_count, bagged_root, peaks_json, last_indexed_block, updated_at)
                 VALUES (1, 0, '0x0000000000000000000000000000000000000000000000000000000000000000', '[]', ?, CURRENT_TIMESTAMP)
                 ON CONFLICT(id) DO UPDATE SET
                     last_indexed_block = excluded.last_indexed_block,
                     updated_at = CURRENT_TIMESTAMP",
                params![last_block as i64],
            )?;
            Ok(())
        })
        .await?
    }

    /// Reconstruct local MerkleMountainRange from persistent mmr_leaves and verify against mmr_state (DEC-035C)
    pub async fn load_mmr_tree(&self) -> Result<zeltra_core::MerkleMountainRange> {
        let path = self.path.clone();
        let db_key = database_key()?;

        task::spawn_blocking(move || -> Result<zeltra_core::MerkleMountainRange> {
            let conn = open_connection(&path, &db_key)?;
            let mut stmt = conn.prepare(
                "SELECT leaf_index, commitment
                 FROM mmr_leaves ORDER BY leaf_index ASC",
            )?;
            let rows = stmt.query_map([], |row| {
                let leaf_index = row.get::<_, i64>(0)? as u64;
                let commitment = row.get::<_, String>(1)?;
                Ok((leaf_index, commitment))
            })?;

            let mut mmr = zeltra_core::MerkleMountainRange::new();
            for row in rows {
                let (leaf_index, commitment) = row?;
                if leaf_index != mmr.leaf_count as u64 {
                    anyhow::bail!(
                        "Non-contiguous MMR leaves in storage: expected index {}, found {}",
                        mmr.leaf_count,
                        leaf_index
                    );
                }
                let clean = commitment.trim_start_matches("0x");
                let bytes = hex::decode(clean).map_err(|e| {
                    anyhow::anyhow!("Invalid hex commitment at leaf {}: {}", leaf_index, e)
                })?;
                let bytes_arr: [u8; 32] = bytes.as_slice().try_into().map_err(|_| {
                    anyhow::anyhow!("Commitment at leaf {} must be 32 bytes", leaf_index)
                })?;
                let fr = zeltra_core::from_evm_scalar(&bytes_arr).ok_or_else(|| {
                    anyhow::anyhow!("Invalid Fr scalar commitment at leaf {}", leaf_index)
                })?;
                mmr.append(fr);
            }

            // Parity assertion if mmr_state exists
            let state_opt = conn
                .query_row(
                    "SELECT leaf_count, bagged_root FROM mmr_state WHERE id = 1",
                    [],
                    |row| Ok((row.get::<_, i64>(0)? as u64, row.get::<_, String>(1)?)),
                )
                .optional()?;

            if let Some((stored_count, stored_root)) = state_opt {
                if mmr.leaf_count as u64 != stored_count {
                    anyhow::bail!(
                        "MMR state desync: reconstructed leaf count {} != stored checkpoint {}",
                        mmr.leaf_count,
                        stored_count
                    );
                }
                if mmr.leaf_count > 0 {
                    let computed_root = format!(
                        "0x{}",
                        hex::encode(zeltra_core::fr_to_be_bytes(&mmr.get_root()))
                    );
                    if computed_root.to_lowercase() != stored_root.to_lowercase() {
                        anyhow::bail!(
                            "MMR state desync: reconstructed root {} != stored checkpoint {}",
                            computed_root,
                            stored_root
                        );
                    }
                }
            }

            Ok(mmr)
        })
        .await?
    }

    /// Roll back MMR leaves and peak state to a specific fork block (DEC-035C INV-1 / Reorg Protection)
    #[allow(dead_code)]
    pub async fn rollback_mmr_to_block(&self, fork_block: u64) -> Result<()> {
        let path = self.path.clone();
        let db_key = database_key()?;

        task::spawn_blocking(move || -> Result<()> {
            let mut conn = open_connection(&path, &db_key)?;
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;

            // 1. Delete all leaves indexed above the reorg fork height
            tx.execute(
                "DELETE FROM mmr_leaves WHERE block_number > ?",
                params![fork_block as i64],
            )?;

            // 2. Re-read all remaining leaves in order
            let remaining_leaves: Vec<(u64, String)> = {
                let mut stmt = tx.prepare(
                    "SELECT leaf_index, commitment
                     FROM mmr_leaves ORDER BY leaf_index ASC",
                )?;
                let rows = stmt.query_map([], |row| {
                    let leaf_index = row.get::<_, i64>(0)? as u64;
                    let commitment = row.get::<_, String>(1)?;
                    Ok((leaf_index, commitment))
                })?;
                let mut items = Vec::new();
                for r in rows {
                    items.push(r?);
                }
                items
            };

            let mut mmr = zeltra_core::MerkleMountainRange::new();
            for (leaf_index, commitment) in remaining_leaves {
                let clean = commitment.trim_start_matches("0x");
                let bytes = hex::decode(clean)?;
                let bytes_arr: [u8; 32] = bytes
                    .as_slice()
                    .try_into()
                    .map_err(|_| anyhow::anyhow!("Commitment at leaf {} must be 32 bytes", leaf_index))?;
                let fr = zeltra_core::from_evm_scalar(&bytes_arr)
                    .ok_or_else(|| anyhow::anyhow!("Invalid scalar at leaf {}", leaf_index))?;
                mmr.append(fr);
            }

            // 3. Re-derive peak state and update mmr_state
            let leaf_count = mmr.leaf_count as u64;
            let bagged_root = if leaf_count == 0 {
                "0x0000000000000000000000000000000000000000000000000000000000000000".to_string()
            } else {
                format!("0x{}", hex::encode(zeltra_core::fr_to_be_bytes(&mmr.get_root())))
            };
            let peaks: Vec<String> = mmr
                .get_peaks()
                .iter()
                .map(|p| format!("0x{}", hex::encode(zeltra_core::fr_to_be_bytes(p))))
                .collect();
            let peaks_json = serde_json::to_string(&peaks)?;

            tx.execute(
                "INSERT INTO mmr_state (id, leaf_count, bagged_root, peaks_json, last_indexed_block, updated_at)
                 VALUES (1, ?, ?, ?, ?, CURRENT_TIMESTAMP)
                 ON CONFLICT(id) DO UPDATE SET
                     leaf_count = excluded.leaf_count,
                     bagged_root = excluded.bagged_root,
                     peaks_json = excluded.peaks_json,
                     last_indexed_block = excluded.last_indexed_block,
                     updated_at = CURRENT_TIMESTAMP",
                params![leaf_count as i64, bagged_root, peaks_json, fork_block as i64],
            )?;

            tx.commit()?;
            Ok(())
        })
        .await?
    }

    /// Atomically insert leaves and update MMR state checkpoint (DEC-035C INV-5 / Atomic Checkpoints)
    pub async fn atomic_append_mmr_leaves_and_state(
        &self,
        leaves: &[MmrLeafRecord],
        mmr: &zeltra_core::MerkleMountainRange,
        to_block: u64,
    ) -> Result<()> {
        let path = self.path.clone();
        let leaves_owned = leaves.to_vec();
        let leaf_count = mmr.leaf_count as u64;
        let root_bytes = zeltra_core::fr_to_be_bytes(&mmr.get_root());
        let bagged_root = format!("0x{}", hex::encode(root_bytes));
        let peaks: Vec<String> = mmr
            .get_peaks()
            .iter()
            .map(|p| format!("0x{}", hex::encode(zeltra_core::fr_to_be_bytes(p))))
            .collect();
        let peaks_json = serde_json::to_string(&peaks)?;
        let db_key = database_key()?;

        task::spawn_blocking(move || -> Result<()> {
            let mut conn = open_connection(&path, &db_key)?;
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;

            for leaf in &leaves_owned {
                tx.execute(
                    "INSERT INTO mmr_leaves (leaf_index, commitment, tx_hash, block_number, encrypted_memo)
                     VALUES (?, ?, ?, ?, ?)",
                    params![
                        leaf.leaf_index as i64,
                        leaf.commitment,
                        leaf.tx_hash,
                        leaf.block_number as i64,
                        leaf.encrypted_memo,
                    ],
                )?;
            }

            tx.execute(
                "INSERT INTO mmr_state (id, leaf_count, bagged_root, peaks_json, last_indexed_block, updated_at)
                 VALUES (1, ?, ?, ?, ?, CURRENT_TIMESTAMP)
                 ON CONFLICT(id) DO UPDATE SET
                     leaf_count = excluded.leaf_count,
                     bagged_root = excluded.bagged_root,
                     peaks_json = excluded.peaks_json,
                     last_indexed_block = excluded.last_indexed_block,
                     updated_at = CURRENT_TIMESTAMP",
                params![leaf_count as i64, bagged_root, peaks_json, to_block as i64],
            )?;

            tx.commit()?;
            Ok(())
        })
        .await?
    }
}
