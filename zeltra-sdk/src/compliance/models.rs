//! Compliance and Audit Data Models (DEC-040B & DEC-040C).
//!
//! Provides data structures for:
//! - B2B Double-Entry Audit Trail reports (IFRS / GAAP compliant)
//! - Asset-partitioned balance summaries and UTXO journals
//! - Single-Note Selective Disclosure CEX proof payloads with cryptographic verifier binding

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Filter query for scoped audit reporting.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditFilter {
    /// None = all assets, Some(0) = WETH, Some(1) = USDC
    pub asset_id: Option<u8>,
    /// Minimum transaction timestamp (Unix timestamp in seconds, inclusive)
    pub from_timestamp: Option<u64>,
    /// Maximum transaction timestamp (Unix timestamp in seconds, inclusive)
    pub to_timestamp: Option<u64>,
    /// Whether to include spent notes / outflow records in the report
    pub include_spent: bool,
    /// Optional organization name (defaults to "Zeltra Protocol Client")
    pub organization_name: Option<String>,
}

/// Principal B2B Double-Entry Audit Report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditReport {
    /// Schema version string (e.g. "zeltra.compliance.audit.v1")
    pub schema_version: String,
    /// Report generation timestamp (Unix timestamp in seconds)
    pub generated_at: u64,
    /// Reporting legal entity or AI Agent Fleet identity
    pub organization_name: String,
    /// Reporting period range
    pub report_period: PeriodRange,
    /// Asset-partitioned balance summaries ("usdc", "weth")
    pub summary_by_asset: HashMap<String, AssetSummary>,
    /// Chronological journal records
    pub records: Vec<AuditRecord>,
}

/// Period date range for audit report.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeriodRange {
    pub from_timestamp: u64,
    pub to_timestamp: u64,
}

/// Balance sheet summary for a single asset (DEC-040B Section 2).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetSummary {
    pub asset_id: u8,
    pub symbol: String,
    pub circuit_decimals: u8,
    pub opening_balance: String,
    pub total_inflow_deposit: String,
    pub total_merchant_payout: String,
    pub total_protocol_fees: String,
    pub total_relayer_execution_fees: String,
    pub closing_unspent_balance: String,
}

/// Transaction direction classification.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecordDirection {
    #[serde(rename = "INFLOW_DEPOSIT")]
    InflowDeposit,
    #[serde(rename = "OUTFLOW_SPEND")]
    OutflowSpend,
    #[serde(rename = "IN_POOL_CONSOLIDATION")]
    InPoolConsolidation,
}

impl RecordDirection {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::InflowDeposit => "INFLOW_DEPOSIT",
            Self::OutflowSpend => "OUTFLOW_SPEND",
            Self::InPoolConsolidation => "IN_POOL_CONSOLIDATION",
        }
    }
}

/// Individual transaction journal entry in double-entry UTXO bookkeeping.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditRecord {
    pub record_id: u64,
    pub timestamp: u64,
    pub date_iso: String,
    pub tx_hash: String,
    pub direction: RecordDirection,
    pub asset_id: u8,
    pub symbol: String,
    pub input_notes_total: String,
    pub deposit_amount: String,
    pub merchant_payout: String,
    pub protocol_fee: String,
    pub relayer_execution_fee: String,
    pub change_amount: String,
    pub created_commitments: Vec<String>,
    pub spent_nullifiers: Vec<String>,
    pub recipient: Option<String>,
    pub status: String,
}

// ═══════════════════════════════════════════════════════════════════════════
// CEX Selective Disclosure Models (Tier 1 - DEC-040 & DEC-040C)
// ═══════════════════════════════════════════════════════════════════════════

/// Input binding challenge parameters provided by verifier (e.g. CEX KYC desk).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerifierBinding {
    pub verifier_id: String,      // e.g. "BINANCE_KYC_DESK"
    pub account_uid: String,      // e.g. "USER-98124712"
    pub nonce: String,            // 16-byte hex unique nonce
    pub expires_at: u64,          // Unix timestamp (max 24h)
    pub spent_tx_hash: String,    // On-chain L2 transaction hash of withdrawal/spend
    pub payout_recipient: String, // CEX user deposit address
}

/// Full CEX Selective Disclosure Proof Payload (Tier 1).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CexProofPayload {
    pub schema_version: String, // "zeltra.compliance.cex.v1"
    pub generated_at: u64,
    pub verifier_binding: VerifierBindingPayload,
    pub asset: AssetDisclosurePayload,
    pub note_cryptography: NoteCryptographyPayload,
    pub mmr_proof: MmrProofPayload,
    pub ownership_zk_proof: OwnershipZkProofPayload,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerifierBindingPayload {
    pub verifier_id: String,
    pub account_uid: String,
    pub nonce: String,
    pub expires_at: u64,
    pub challenge_hash: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetDisclosurePayload {
    pub asset_id: u8,
    pub symbol: String,
    pub circuit_unit_name: String,
    pub circuit_decimals: u8,
    pub erc20_decimals: u8,
    pub raw_circuit_value: String,
    pub value_packed_scalar: String,
    pub human_value: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoteCryptographyPayload {
    pub commitment: String,
    pub owner_key: String,
    pub rho: String,
    pub randomness: String,
    pub nullifier: String,
    pub nullifier_status: String, // "SPENT" or "UNSPENT"
    pub spent_tx_hash: String,
    pub payout_recipient: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MmrProofPayload {
    pub leaf_index: u64,
    pub leaf_count: u64,
    pub peak_index: usize,
    pub sibling_path: Vec<String>,
    pub mmr_peaks: Vec<String>,
    pub mmr_root: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OwnershipZkProofPayload {
    pub protocol: String, // "GROTH16_BLS12_381_DISCLOSURE_V1"
    pub public_inputs: Vec<String>,
    pub proof_bytes: String,
}

/// Summary result of successful 5-stage deterministic verification by CexProofVerifier (DEC-040C).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerificationSummary {
    pub status: String,
    pub asset_symbol: String,
    pub amount_human: String,
    pub recipient: String,
    pub account_uid: String,
    pub verified_at: u64,
}
