//! Compliance Module (DEC-040B & DEC-040C).
//!
//! Provides B2B Double-Entry Audit Trail generation, UTXO conservation validation,
//! asset-partitioned balance sheet reconciliation, anti-CSV formula injection sanitization,
//! and CEX Selective Disclosure proof generation.

pub mod argo;
pub mod exporter;
pub mod models;
pub mod verifier;

pub use argo::*;
pub use exporter::*;
pub use models::*;
pub use verifier::*;

use std::collections::HashMap;

/// Compliance and accounting verification errors (DEC-040B & DEC-040C).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComplianceError {
    /// An outflow spend or consolidation violates exact UTXO value conservation.
    AccountingReconciliationMismatch {
        record_id: u64,
        expected: u64,
        actual: u64,
    },
    /// Balance sheet equation: Opening + Inflow - (Payout + ProtocolFees + ExecutionFees) != Closing,
    /// or closing balance diverges from local unspent wallet UTXOs.
    BalanceSheetMismatch {
        asset_id: u8,
        expected: u64,
        actual: u64,
    },
    /// Non-canonical or malformed decimal string representation.
    InvalidDecimalFormat { raw: String, asset_id: u8 },
    /// JSON serialization or deserialization failure.
    SerializationError(String),
    /// CSV export encoding error.
    CsvExportError(String),
    /// Malformed or inconsistent audit record.
    InvalidRecord(String),
    /// Unsupported asset identifier (only 0 for WETH and 1 for USDC).
    UnsupportedAssetId(u8),
    /// Cryptographic or general verification failure.
    VerificationError(String),
    /// Commitment preimage does not match expected commitment (Tahap 1, DEC-040C).
    CommitmentPreimageMismatch,
    /// Merkle Mountain Range inclusion proof or peak bagging calculation failed (Tahap 2, DEC-040C).
    InvalidMmrInclusionProof,
    /// MMR Root is not accepted on-chain by the smart contract (Tahap 2, DEC-040C).
    MmrRootNotAcceptedOnChain,
    /// Verifier ID in proof does not match the inspecting verifier's identity (Tahap 3, DEC-040C).
    VerifierIdMismatch { expected: String, actual: String },
    /// Proof timestamp has expired (> 24 hours or past expires_at) (Tahap 3, DEC-040C).
    ProofExpired,
    /// Blake2s verifier binding challenge hash does not match computed digest (Tahap 3, DEC-040C).
    ChallengeHashMismatch,
    /// Malformed or non-canonical Groth16 proof bytes (Tahap 4, DEC-040C).
    InvalidProofBytes,
    /// Groth16 zero-knowledge proof verification failed against public inputs (Tahap 4, DEC-040C).
    InvalidDisclosureZkProof,
    /// Note nullifier has not been spent on-chain (Tahap 5, DEC-040C).
    NullifierNotSpentOnChain,
    /// Transaction receipt recipient or asset does not match expected payout (Tahap 5, DEC-040C).
    InvalidRecipientOrTx(String),
    /// RPC or network connectivity failure (Fail-Closed, DEC-040C).
    RpcError(String),
}

impl std::fmt::Display for ComplianceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AccountingReconciliationMismatch {
                record_id,
                expected,
                actual,
            } => {
                write!(
                    f,
                    "Double-entry UTXO conservation mismatch on record {record_id}: expected input total {expected}, got outputs sum {actual}"
                )
            }
            Self::BalanceSheetMismatch {
                asset_id,
                expected,
                actual,
            } => {
                write!(
                    f,
                    "Balance sheet reconciliation mismatch for asset {asset_id}: expected closing balance {expected}, got {actual}"
                )
            }
            Self::InvalidDecimalFormat { raw, asset_id } => {
                write!(f, "Invalid decimal format '{raw}' for asset {asset_id}")
            }
            Self::SerializationError(msg) => write!(f, "Compliance serialization error: {msg}"),
            Self::CsvExportError(msg) => write!(f, "CSV export error: {msg}"),
            Self::InvalidRecord(msg) => write!(f, "Invalid audit record: {msg}"),
            Self::UnsupportedAssetId(id) => {
                write!(f, "Unsupported asset ID {id} (expected 0 or 1)")
            }
            Self::VerificationError(msg) => write!(f, "Verification error: {msg}"),
            Self::CommitmentPreimageMismatch => {
                write!(f, "Commitment preimage mismatch: recomputed Poseidon-W5 commitment does not match note commitment")
            }
            Self::InvalidMmrInclusionProof => {
                write!(f, "Invalid MMR inclusion proof: reconstructed peak or bagged root does not match claimed MMR root")
            }
            Self::MmrRootNotAcceptedOnChain => {
                write!(f, "MMR root is not accepted on-chain by the smart contract")
            }
            Self::VerifierIdMismatch { expected, actual } => {
                write!(
                    f,
                    "Verifier ID mismatch: expected '{expected}', actual '{actual}'"
                )
            }
            Self::ProofExpired => {
                write!(f, "Selective disclosure proof has expired")
            }
            Self::ChallengeHashMismatch => {
                write!(f, "Blake2s verifier binding challenge hash mismatch")
            }
            Self::InvalidProofBytes => {
                write!(f, "Invalid or non-canonical Groth16 proof bytes")
            }
            Self::InvalidDisclosureZkProof => {
                write!(
                    f,
                    "Groth16 zero-knowledge proof verification failed against public inputs"
                )
            }
            Self::NullifierNotSpentOnChain => {
                write!(f, "Note nullifier is not marked spent on-chain")
            }
            Self::InvalidRecipientOrTx(msg) => {
                write!(f, "Invalid spend transaction or recipient mismatch: {msg}")
            }
            Self::RpcError(msg) => {
                write!(f, "RPC connectivity failure (Fail-Closed): {msg}")
            }
        }
    }
}

impl std::error::Error for ComplianceError {}

/// Computes non-malleable Blake2s-256 verifier challenge hash (DEC-040 Section 4.B & DEC-040C).
#[allow(clippy::too_many_arguments)]
pub fn compute_verifier_challenge(
    verifier_id: &str,
    account_uid: &str,
    generated_at: u64,
    expires_at: u64,
    nonce: &str,
    cm: &ark_bls12_381::Fr,
    nullifier: &ark_bls12_381::Fr,
    spent_tx_hash: &str,
    payout_recipient: &str,
) -> [u8; 32] {
    use blake2::{Blake2s256, Digest};
    let mut hasher = Blake2s256::new();
    hasher.update(b"ZELTRA_CEX_DISCLOSURE_V1");
    hasher.update(verifier_id.as_bytes());
    hasher.update(account_uid.as_bytes());
    hasher.update(generated_at.to_be_bytes());
    hasher.update(expires_at.to_be_bytes());
    hasher.update(nonce.as_bytes());
    hasher.update(zeltra_core::fr_to_be_bytes(cm));
    hasher.update(zeltra_core::fr_to_be_bytes(nullifier));
    hasher.update(spent_tx_hash.as_bytes());
    hasher.update(payout_recipient.as_bytes());
    hasher.finalize().into()
}

/// Returns the circuit decimal places for an asset (DEC-034A).
/// - Asset 0 (WETH): 9 decimals (Gwei)
/// - Asset 1 (USDC): 6 decimals (Micro-USDC)
pub fn asset_decimals(asset_id: u8) -> Result<u8, ComplianceError> {
    match asset_id {
        0 => Ok(9),
        1 => Ok(6),
        _ => Err(ComplianceError::UnsupportedAssetId(asset_id)),
    }
}

/// Returns the canonical ticker symbol for an asset.
pub fn asset_symbol(asset_id: u8) -> &'static str {
    match asset_id {
        0 => "WETH",
        1 => "USDC",
        _ => "UNKNOWN",
    }
}

/// Formats a raw integer unit into an exact, zero-loss decimal string.
/// - Asset 0 (WETH): 9 decimals (e.g. 5_000_000_000 -> "5.000000000")
/// - Asset 1 (USDC): 6 decimals (e.g. 50_000_000_000 -> "50000.000000")
pub fn format_asset_amount(value: u64, asset_id: u8) -> String {
    match asset_id {
        0 => {
            let int_part = value / 1_000_000_000;
            let frac_part = value % 1_000_000_000;
            format!("{}.{:09}", int_part, frac_part)
        }
        _ => {
            let int_part = value / 1_000_000;
            let frac_part = value % 1_000_000;
            format!("{}.{:06}", int_part, frac_part)
        }
    }
}

/// Parses a decimal string back into its raw integer unit.
///
/// Guaranteed zero floating-point arithmetic.
pub fn parse_asset_amount(s: &str, asset_id: u8) -> Result<u64, ComplianceError> {
    let decimals = asset_decimals(asset_id)? as usize;
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return Ok(0);
    }

    if let Some((int_str, frac_str)) = trimmed.split_once('.') {
        let int_val: u64 = int_str
            .parse()
            .map_err(|_| ComplianceError::InvalidDecimalFormat {
                raw: s.to_string(),
                asset_id,
            })?;

        let scale = 10u64.checked_pow(decimals as u32).ok_or_else(|| {
            ComplianceError::InvalidDecimalFormat {
                raw: s.to_string(),
                asset_id,
            }
        })?;

        let mut frac_padded = frac_str.to_string();
        if frac_padded.len() > decimals {
            return Err(ComplianceError::InvalidDecimalFormat {
                raw: s.to_string(),
                asset_id,
            });
        }
        while frac_padded.len() < decimals {
            frac_padded.push('0');
        }

        let frac_val: u64 =
            frac_padded
                .parse()
                .map_err(|_| ComplianceError::InvalidDecimalFormat {
                    raw: s.to_string(),
                    asset_id,
                })?;

        int_val
            .checked_mul(scale)
            .and_then(|base| base.checked_add(frac_val))
            .ok_or_else(|| ComplianceError::InvalidDecimalFormat {
                raw: s.to_string(),
                asset_id,
            })
    } else {
        let int_val: u64 = trimmed
            .parse()
            .map_err(|_| ComplianceError::InvalidDecimalFormat {
                raw: s.to_string(),
                asset_id,
            })?;
        let scale = 10u64.checked_pow(decimals as u32).ok_or_else(|| {
            ComplianceError::InvalidDecimalFormat {
                raw: s.to_string(),
                asset_id,
            }
        })?;
        int_val
            .checked_mul(scale)
            .ok_or_else(|| ComplianceError::InvalidDecimalFormat {
                raw: s.to_string(),
                asset_id,
            })
    }
}

/// Formats a Unix timestamp (seconds) into ISO-8601 UTC date-time string (YYYY-MM-DDTHH:MM:SSZ).
pub fn format_iso8601(timestamp: u64) -> String {
    let secs = timestamp % 60;
    let mins = (timestamp / 60) % 60;
    let hours = (timestamp / 3600) % 24;
    let z = (timestamp / 86400) + 719468;
    let era = z / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}T{hours:02}:{mins:02}:{secs:02}Z")
}

/// Validates per-record UTXO double-entry conservation (DEC-040B Section 3.A).
///
/// For each OutflowSpend and InPoolConsolidation record:
/// `input_notes_total == merchant_payout + protocol_fee + relayer_execution_fee + change_amount`
pub fn validate_utxo_conservation(records: &[AuditRecord]) -> Result<(), ComplianceError> {
    for record in records {
        match record.direction {
            RecordDirection::OutflowSpend | RecordDirection::InPoolConsolidation => {
                let input_total = parse_asset_amount(&record.input_notes_total, record.asset_id)?;
                let payout = parse_asset_amount(&record.merchant_payout, record.asset_id)?;
                let proto_fee = parse_asset_amount(&record.protocol_fee, record.asset_id)?;
                let exec_fee = parse_asset_amount(&record.relayer_execution_fee, record.asset_id)?;
                let change = parse_asset_amount(&record.change_amount, record.asset_id)?;

                let sum_outputs = payout
                    .checked_add(proto_fee)
                    .and_then(|s| s.checked_add(exec_fee))
                    .and_then(|s| s.checked_add(change))
                    .ok_or_else(|| {
                        ComplianceError::InvalidRecord(format!(
                            "Overflow in sum of outputs for record {}",
                            record.record_id
                        ))
                    })?;

                if input_total != sum_outputs {
                    return Err(ComplianceError::AccountingReconciliationMismatch {
                        record_id: record.record_id,
                        expected: input_total,
                        actual: sum_outputs,
                    });
                }
            }
            RecordDirection::InflowDeposit => {
                // Inflow deposit verification: gross deposit matches deposit_amount
                let deposit = parse_asset_amount(&record.deposit_amount, record.asset_id)?;
                if deposit == 0 {
                    return Err(ComplianceError::InvalidRecord(format!(
                        "Inflow deposit record {} has zero deposit_amount",
                        record.record_id
                    )));
                }
            }
        }
    }
    Ok(())
}

/// Validates the balance sheet equation and compares closing balances against actual unspent wallet notes (DEC-040B Section 3.B).
///
/// Equation: `OpeningBalance + TotalInflow - (TotalMerchantPayout + TotalProtocolFees + TotalExecutionFees) == ClosingBalance`
pub fn validate_balance_sheet_reconciliation(
    report: &AuditReport,
    current_unspent: &HashMap<u8, u64>,
) -> Result<(), ComplianceError> {
    for summary in report.summary_by_asset.values() {
        let asset_id = summary.asset_id;
        let opening = parse_asset_amount(&summary.opening_balance, asset_id)?;
        let inflow = parse_asset_amount(&summary.total_inflow_deposit, asset_id)?;
        let payout = parse_asset_amount(&summary.total_merchant_payout, asset_id)?;
        let proto_fees = parse_asset_amount(&summary.total_protocol_fees, asset_id)?;
        let exec_fees = parse_asset_amount(&summary.total_relayer_execution_fees, asset_id)?;
        let closing = parse_asset_amount(&summary.closing_unspent_balance, asset_id)?;

        let total_consumed = payout
            .checked_add(proto_fees)
            .and_then(|s| s.checked_add(exec_fees))
            .ok_or_else(|| {
                ComplianceError::InvalidRecord(format!(
                    "Overflow in total consumed expenses for asset {asset_id}"
                ))
            })?;

        let expected_closing = opening
            .checked_add(inflow)
            .and_then(|total| total.checked_sub(total_consumed))
            .ok_or_else(|| ComplianceError::BalanceSheetMismatch {
                asset_id,
                expected: opening.saturating_add(inflow),
                actual: total_consumed,
            })?;

        if expected_closing != closing {
            return Err(ComplianceError::BalanceSheetMismatch {
                asset_id,
                expected: expected_closing,
                actual: closing,
            });
        }

        // Compare against local unspent balance if provided for this asset
        if let Some(&actual_local_unspent) = current_unspent.get(&asset_id) {
            if actual_local_unspent != closing {
                return Err(ComplianceError::BalanceSheetMismatch {
                    asset_id,
                    expected: actual_local_unspent,
                    actual: closing,
                });
            }
        }
    }
    Ok(())
}
