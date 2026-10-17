use serde::{Deserialize, Serialize};

#[derive(Debug)]
pub struct QueuedSpend {
    pub id: i64,
    pub request: crate::dto::SpendRequest,
    pub retry_count: u32,
}

#[derive(Debug, Clone)]
pub struct UnreconciledSpend {
    pub id: i64,
    pub nullifier: String,
    pub tx_hash: Option<String>,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelayerTxRecord {
    pub tx_hash: String,
    pub nonce: u64,
    pub signer_address: String,
    pub status: String, // "pending", "confirmed", "failed"
    pub block_number: Option<u64>,
    pub gas_used: Option<u64>,
    pub effective_gas_price: Option<u128>,
    pub confirmations: u64,
    pub error_reason: Option<String>,
    pub created_at: u64,
    pub updated_at: u64,
}

/// Indexed MMR leaf record representing an on-chain note commitment (DEC-035C)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MmrLeafRecord {
    pub leaf_index: u64,
    pub commitment: String,
    pub tx_hash: String,
    pub block_number: u64,
    #[serde(default)]
    pub encrypted_memo: Vec<u8>,
    pub created_at: Option<String>,
}

/// Note memo row returned for bulk client sync (DEC-037)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LeafMemoRow {
    pub leaf_index: u64,
    pub commitment: String,
    pub encrypted_memo: String,
    pub block_number: u64,
    pub tx_hash: String,
}

/// Persistent snapshot of the Merkle Mountain Range peak states (DEC-035C)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MmrStateRecord {
    pub leaf_count: u64,
    pub bagged_root: String,
    pub peaks: Vec<String>,
    pub last_indexed_block: u64,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnclaimedBatchInfo {
    pub batch_id: String,
    pub tx_hash: String,
    pub execution_fee_usdc: u64,
    pub item_count: usize,
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub struct DbStats {
    pub total_nullifiers: i64,
    pub queued_spends: i64,
    pub batch_count: i64,
    pub total_batch_margin_usdc: f64,
    pub avg_batch_size: f64,
    pub total_execution_fees_usdc: u64,
    pub claimed_execution_fees_usdc: u64,
    pub unclaimed_execution_fees_usdc: u64,
}
