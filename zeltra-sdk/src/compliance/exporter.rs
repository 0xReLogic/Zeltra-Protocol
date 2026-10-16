//! Compliance Report Exporters (JSON & Sanitized CSV) — DEC-040B.
//!
//! Provides formatting routines for:
//! - Structured pretty-printed JSON (IFRS audit compliant)
//! - Sanitized CSV with CWE-1236 Formula Injection mitigation

use crate::compliance::models::*;
use crate::compliance::{format_asset_amount, parse_asset_amount, ComplianceError};

/// Sanitizes a string cell against CSV formula execution attacks (CWE-1236 / OWASP).
///
/// If a trimmed string starts with an execution character (`=`, `+`, `-`, `@`, `\t`, `\r`),
/// it is prefixed with a single quote `'` to force spreadsheets (Excel, Google Sheets, LibreOffice)
/// to interpret the cell as plain text rather than an executable DDE/shell formula.
pub fn sanitize_csv_cell(input: &str) -> String {
    if input.is_empty() {
        return String::new();
    }

    // Dangerous formula trigger characters per OWASP & CWE-1236
    let trigger_chars = ['=', '+', '-', '@', '\t', '\r'];
    if trigger_chars.iter().any(|&c| input.starts_with(c)) {
        format!("'{}", input)
    } else {
        let trimmed_start = input.trim_start();
        if !trimmed_start.is_empty() && trigger_chars.iter().any(|&c| trimmed_start.starts_with(c))
        {
            format!("'{}", input)
        } else {
            input.to_string()
        }
    }
}

/// Encodes a value for CSV output: sanitizes against formula injection and escapes RFC 4180 delimiters.
pub fn encode_csv_cell(input: &str) -> String {
    let sanitized = sanitize_csv_cell(input);
    if sanitized.contains(',')
        || sanitized.contains('"')
        || sanitized.contains('\n')
        || sanitized.contains('\r')
    {
        format!("\"{}\"", sanitized.replace('"', "\"\""))
    } else {
        sanitized
    }
}

/// Exports an AuditReport to pretty-printed JSON format.
pub fn export_audit_json(report: &AuditReport) -> Result<String, ComplianceError> {
    serde_json::to_string_pretty(report)
        .map_err(|e| ComplianceError::SerializationError(e.to_string()))
}

/// Exports an AuditReport to sanitized CSV format with strict CWE-1236 protection.
///
/// Header format:
/// `RecordID,Timestamp,DateISO,Direction,AssetID,AssetSymbol,GrossAmount,FeePaid,NetAmount,Recipient,TxHash,Commitment,Nullifier,Status`
pub fn export_audit_csv(report: &AuditReport) -> Result<String, ComplianceError> {
    let mut out = String::new();
    out.push_str("RecordID,Timestamp,DateISO,Direction,AssetID,AssetSymbol,GrossAmount,FeePaid,NetAmount,Recipient,TxHash,Commitment,Nullifier,Status\n");

    for record in &report.records {
        let proto_fee_raw = parse_asset_amount(&record.protocol_fee, record.asset_id)?;
        let exec_fee_raw = parse_asset_amount(&record.relayer_execution_fee, record.asset_id)?;
        let total_fee_raw = proto_fee_raw.saturating_add(exec_fee_raw);
        let fee_paid_str = format_asset_amount(total_fee_raw, record.asset_id);

        let (gross_str, net_str) = match record.direction {
            RecordDirection::InflowDeposit => {
                let dep_raw = parse_asset_amount(&record.deposit_amount, record.asset_id)?;
                (
                    format_asset_amount(dep_raw, record.asset_id),
                    format_asset_amount(dep_raw, record.asset_id),
                )
            }
            RecordDirection::OutflowSpend => {
                let payout_raw = parse_asset_amount(&record.merchant_payout, record.asset_id)?;
                let net_raw = payout_raw.saturating_sub(total_fee_raw);
                (
                    format_asset_amount(payout_raw, record.asset_id),
                    format_asset_amount(net_raw, record.asset_id),
                )
            }
            RecordDirection::InPoolConsolidation => {
                let input_raw = parse_asset_amount(&record.input_notes_total, record.asset_id)?;
                let change_raw = parse_asset_amount(&record.change_amount, record.asset_id)?;
                (
                    format_asset_amount(input_raw, record.asset_id),
                    format_asset_amount(change_raw, record.asset_id),
                )
            }
        };

        let recipient_str = record.recipient.as_deref().unwrap_or("N/A");
        let commitment_str = record
            .created_commitments
            .first()
            .map(|s| s.as_str())
            .unwrap_or("N/A");
        let nullifier_str = record
            .spent_nullifiers
            .first()
            .map(|s| s.as_str())
            .unwrap_or("N/A");

        let row = format!(
            "{},{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
            encode_csv_cell(&record.record_id.to_string()),
            encode_csv_cell(&record.timestamp.to_string()),
            encode_csv_cell(&record.date_iso),
            encode_csv_cell(record.direction.as_str()),
            encode_csv_cell(&record.asset_id.to_string()),
            encode_csv_cell(&record.symbol),
            encode_csv_cell(&gross_str),
            encode_csv_cell(&fee_paid_str),
            encode_csv_cell(&net_str),
            encode_csv_cell(recipient_str),
            encode_csv_cell(&record.tx_hash),
            encode_csv_cell(commitment_str),
            encode_csv_cell(nullifier_str),
            encode_csv_cell(&record.status),
        );

        out.push_str(&row);
    }

    Ok(out)
}
