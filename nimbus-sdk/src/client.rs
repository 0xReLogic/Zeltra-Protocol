//! Anti-Snooping Paginated Bulk MMR Tree Sync & Relayer HTTP Client (DEC-036B Phase 3)
//!
//! Academic Foundations & References:
//! - Zcash ZIP 314 & Issue#434 (2021–2024): "Privacy upgrades to the Zcash light client protocol"
//!   (Defense against Lightwalletd transaction graph recovery).
//! - DEC-032 / DEC-035C / DEC-036B: Bulk Leaf Accumulator Synchronization.
//!
//! Mitigates ZIP 314 metadata snooping by strictly fetching note commitments in uniform 1,000-leaf chunks
//! (`GET /api/v1/mmr/leaves?from={idx}&limit=1000`). BANS single-leaf proof queries in production SDK,
//! forcing client-side local peak authentication path synthesis in WASM/CPU memory.

use crate::wallet::note_wallet::{JoinSplitSpendProofPayload, WalletError};
use serde::{Deserialize, Serialize};

/// Standard pagination chunk limit for bulk MMR leaves (DEC-036B).
pub const BULK_LEAVES_CHUNK_SIZE: u64 = 1_000;

/// Relayer MMR state snapshot response (DEC-035C / DEC-036B)
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MmrStatusResponse {
    pub latest_leaf_count: u64,
    pub note_root: String,
    #[serde(default)]
    pub last_indexed_block: Option<u64>,
}

/// Paginated bulk leaves container
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LeavesResponse {
    pub from: u64,
    pub limit: u64,
    pub total: u64,
    pub leaves: Vec<String>,
}

/// Relayer quote request
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QuoteRequest {
    pub recipient: String,
    pub amount: u64,
    pub chain_id: u64,
}

/// Relayer quote response
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QuoteResponse {
    pub quote_hash: String,
    pub execution_fee: u64,
    pub max_execution_fee: u64,
    pub expiry: u64,
    pub signature_hex: String,
}

/// Response returned after broadcasting a spend transaction
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SpendSubmissionResponse {
    pub status: String,
    pub tx_hash: String,
    pub session_id: String,
}

/// Anti-snooping HTTP client for communication with Relayer node
#[derive(Clone, Debug)]
pub struct NimbusClient {
    base_url: String,
    http_client: reqwest::Client,
}

impl NimbusClient {
    /// Creates a new client instance configured with the Relayer base URL.
    pub fn new(base_url: &str) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            http_client: reqwest::Client::new(),
        }
    }

    /// Returns the base URL of the relayer.
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Fetches the latest MMR accumulator status from the relayer (leaf count and bagged root).
    pub async fn fetch_mmr_status(&self) -> Result<MmrStatusResponse, WalletError> {
        let url = format!("{}/api/v1/mmr/status", self.base_url);
        let resp =
            self.http_client.get(&url).send().await.map_err(|e| {
                WalletError::RelayerError(format!("Failed to fetch MMR status: {e}"))
            })?;

        if !resp.status().is_success() {
            return Err(WalletError::RelayerError(format!(
                "MMR status request failed with HTTP {}",
                resp.status()
            )));
        }

        resp.json::<MmrStatusResponse>()
            .await
            .map_err(|e| WalletError::RelayerError(format!("Invalid MMR status JSON: {e}")))
    }

    /// Anti-Snooping Bulk Leaves Routine (DEC-036B B3.1 / ZIP 314 Defense).
    ///
    /// Fetches a uniform chunk of note commitment leaves without revealing
    /// which specific leaves belong to the client.
    pub async fn fetch_mmr_leaves(
        &self,
        from_index: u64,
        limit: u64,
    ) -> Result<Vec<String>, WalletError> {
        let url = format!(
            "{}/api/v1/mmr/leaves?from={}&limit={}",
            self.base_url, from_index, limit
        );
        let resp =
            self.http_client.get(&url).send().await.map_err(|e| {
                WalletError::RelayerError(format!("Failed to fetch bulk leaves: {e}"))
            })?;

        if !resp.status().is_success() {
            return Err(WalletError::RelayerError(format!(
                "Bulk leaves request failed with HTTP {}",
                resp.status()
            )));
        }

        let body = resp
            .text()
            .await
            .map_err(|e| WalletError::RelayerError(e.to_string()))?;

        // Accepts both raw array of hex strings `["0x..", "0x.."]` and wrapped container
        if let Ok(leaves) = serde_json::from_str::<Vec<String>>(&body) {
            Ok(leaves)
        } else if let Ok(wrapper) = serde_json::from_str::<LeavesResponse>(&body) {
            Ok(wrapper.leaves)
        } else {
            Err(WalletError::RelayerError(
                "Unable to parse leaves response payload".into(),
            ))
        }
    }

    /// Submits a Universal JoinSplit spend proof payload to the relayer.
    pub async fn submit_joinsplit_spend(
        &self,
        payload: &JoinSplitSpendProofPayload,
    ) -> Result<SpendSubmissionResponse, WalletError> {
        let url = format!("{}/api/v1/spend-joinsplit", self.base_url);
        let resp = self
            .http_client
            .post(&url)
            .json(payload)
            .send()
            .await
            .map_err(|e| WalletError::RelayerError(format!("Failed to submit spend: {e}")))?;

        if !resp.status().is_success() {
            let err_text = resp.text().await.unwrap_or_default();
            return Err(WalletError::RelayerError(format!(
                "Spend submission failed: {err_text}"
            )));
        }

        resp.json::<SpendSubmissionResponse>()
            .await
            .map_err(|e| WalletError::RelayerError(format!("Invalid response JSON: {e}")))
    }
}
