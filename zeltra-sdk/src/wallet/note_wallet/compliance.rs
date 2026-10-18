use ark_bls12_381::{Bls12_381, Fr};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use zeltra_core::{
    decrypt_note_memo, decrypt_note_memo_deterministic, derive_joinsplit_nullifier,
    derive_nullifier, derive_nullifier_key, fr_to_be_bytes, note_commitment, FullViewingKey,
    IncomingViewingKey,
};

use super::types::*;
use super::PrivateNoteWallet;
use crate::compliance::*;

impl PrivateNoteWallet {
    /// Records an audit journal entry into the wallet's double-entry transaction trail.
    pub fn record_audit_entry(&mut self, record: AuditRecord) {
        self.audit_records.push(record);
    }

    /// Exports B2B Double-Entry Audit Report (IFRS / GAAP compliant) with fail-closed reconciliation.
    pub fn export_audit_trail(&self, filter: AuditFilter) -> Result<AuditReport, WalletError> {
        let mut unspent_map = HashMap::new();
        unspent_map.insert(0, self.get_unspent_balance(0));
        unspent_map.insert(1, self.get_unspent_balance(1));

        let records = if !self.audit_records.is_empty() {
            self.audit_records.clone()
        } else {
            let mut synth = Vec::new();
            for (id, (cm, note)) in (1u64..).zip(self.notes.iter()) {
                let sym = if note.asset_id == 0 { "WETH" } else { "USDC" };
                let val_str = format_asset_amount(note.value, note.asset_id);
                let zero_str = format_asset_amount(0, note.asset_id);
                synth.push(AuditRecord {
                    record_id: id,
                    timestamp: note.created_at_secs,
                    date_iso: format_iso8601(note.created_at_secs),
                    tx_hash: "0x0000000000000000000000000000000000000000000000000000000000000000"
                        .to_string(),
                    direction: RecordDirection::InflowDeposit,
                    asset_id: note.asset_id,
                    symbol: sym.to_string(),
                    input_notes_total: zero_str.clone(),
                    deposit_amount: val_str,
                    merchant_payout: zero_str.clone(),
                    protocol_fee: zero_str.clone(),
                    relayer_execution_fee: zero_str.clone(),
                    change_amount: zero_str,
                    created_commitments: vec![cm.clone()],
                    spent_nullifiers: vec![],
                    recipient: None,
                    status: "SETTLED".to_string(),
                });
            }
            synth
        };

        build_audit_report_inner(&records, &filter, &unspent_map)
            .map_err(|e| WalletError::ComplianceError(e.to_string()))
    }

    /// Generates a Tier 1 Single-Note Selective Disclosure Proof for Centralized Exchange (CEX) deposit compliance.
    ///
    /// Cryptographically proves note ownership, validity, and on-chain nullifier without exposing spending key
    /// or other wallet UTXOs (DEC-040 & DEC-040A).
    pub fn create_cex_proof(
        &self,
        commitment: [u8; 32],
        binding: VerifierBinding,
    ) -> Result<CexProofPayload, WalletError> {
        let cm_hex = hex::encode(commitment);
        let cm_0x = format!("0x{}", cm_hex);
        let note = self
            .notes
            .get(&cm_hex)
            .or_else(|| self.notes.get(&cm_0x))
            .ok_or_else(|| WalletError::NoteNotFound(cm_hex.clone()))?;

        let sk = self.spending_key()?;
        let nk = derive_nullifier_key(sk);

        let owner_key = parse_fr_from_hex(&note.owner_key_hex)?;
        let rho = parse_fr_from_hex(&note.rho_hex)?;
        let randomness = parse_fr_from_hex(&note.randomness_hex)?;
        let epoch_id = note.epoch();
        let leaf_index = note.leaf_index.unwrap_or(0);

        let cm_fr = parse_fr_from_hex(&cm_hex)?;
        let nullifier = derive_joinsplit_nullifier(nk, rho, epoch_id as u64);
        let nullifier_hex = format!("0x{}", hex::encode(fr_to_be_bytes(&nullifier)));

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // 1. Verifier Binding & Non-Malleable Challenge Hash (DEC-040 Section 4.B)
        use blake2::{Blake2s256, Digest as Blake2Digest};
        let mut hasher = Blake2s256::new();
        hasher.update(b"ZELTRA_CEX_DISCLOSURE_V1");
        hasher.update(binding.verifier_id.as_bytes());
        hasher.update(binding.account_uid.as_bytes());
        hasher.update(now.to_be_bytes());
        hasher.update(binding.expires_at.to_be_bytes());
        hasher.update(binding.nonce.as_bytes());
        hasher.update(fr_to_be_bytes(&cm_fr));
        hasher.update(fr_to_be_bytes(&nullifier));
        hasher.update(binding.spent_tx_hash.as_bytes());
        hasher.update(binding.payout_recipient.as_bytes());
        let challenge_bytes: [u8; 32] = hasher.finalize().into();
        let challenge_hash_hex = format!("0x{}", hex::encode(challenge_bytes));

        let (challenge_hi, challenge_lo) =
            zeltra_core::selective_disclosure_circuit::decompose_challenge_to_limbs(
                &challenge_bytes,
            );

        // 2. Synthesize SelectiveDisclosureCircuit and generate Groth16 proof
        let circuit = zeltra_core::selective_disclosure_circuit::SelectiveDisclosureCircuit {
            spending_key: Some(sk),
            epoch_id: Some(epoch_id),
            owner_key: Some(owner_key),
            rho: Some(rho),
            nullifier: Some(nullifier),
            challenge_hi: Some(challenge_hi),
            challenge_lo: Some(challenge_lo),
        };

        let (pk, vk) = get_or_init_disclosure_keys();
        let proof =
            zeltra_core::selective_disclosure_circuit::generate_disclosure_proof(circuit, pk)
                .map_err(|e| {
                    WalletError::CryptoError(format!("Failed to generate disclosure proof: {e}"))
                })?;

        // Verify with verifying key to guarantee non-repudiation
        let pvk = ark_groth16::prepare_verifying_key(vk);
        let public_inputs = vec![owner_key, rho, nullifier, challenge_hi, challenge_lo];
        let is_valid = zeltra_core::selective_disclosure_circuit::verify_disclosure_proof(
            &proof,
            &public_inputs,
            &pvk,
        )
        .map_err(|e| {
            WalletError::CryptoError(format!("Verification of generated proof failed: {e}"))
        })?;

        if !is_valid {
            return Err(WalletError::CryptoError(
                "Generated disclosure proof failed local Groth16 verification".to_string(),
            ));
        }

        use ark_serialize::CanonicalSerialize;
        let mut proof_bytes_vec = Vec::new();
        proof
            .serialize_compressed(&mut proof_bytes_vec)
            .map_err(|e| WalletError::SerializationError(e.to_string()))?;
        let proof_bytes_hex = format!("0x{}", hex::encode(proof_bytes_vec));

        let public_inputs_hex: Vec<String> = public_inputs
            .iter()
            .map(|fr| format!("0x{}", hex::encode(fr_to_be_bytes(fr))))
            .collect();

        // 3. Asset Disclosure
        let (symbol, circuit_unit_name, circuit_decimals, erc20_decimals) = if note.asset_id == 0 {
            ("WETH".to_string(), "GWEI".to_string(), 9, 18)
        } else {
            ("USDC".to_string(), "MICRO_USDC".to_string(), 6, 6)
        };

        let two_pow_64 = Fr::from(1u128 << 64);
        let value_packed = Fr::from(note.value) + (Fr::from(note.asset_id as u64) * two_pow_64);
        let value_packed_scalar = format!("0x{}", hex::encode(fr_to_be_bytes(&value_packed)));
        let human_value = format_asset_amount(note.value, note.asset_id);

        let asset_payload = AssetDisclosurePayload {
            asset_id: note.asset_id,
            symbol,
            circuit_unit_name,
            circuit_decimals,
            erc20_decimals,
            raw_circuit_value: note.value.to_string(),
            value_packed_scalar,
            human_value,
        };

        // 4. Note Cryptography
        let nullifier_status = match note.status {
            NoteStatus::Spent { .. } => "SPENT".to_string(),
            _ => "UNSPENT".to_string(),
        };

        let note_crypto_payload = NoteCryptographyPayload {
            commitment: format!("0x{}", cm_hex),
            owner_key: format!("0x{}", hex::encode(fr_to_be_bytes(&owner_key))),
            rho: format!("0x{}", hex::encode(fr_to_be_bytes(&rho))),
            randomness: format!("0x{}", hex::encode(fr_to_be_bytes(&randomness))),
            nullifier: nullifier_hex,
            nullifier_status,
            spent_tx_hash: binding.spent_tx_hash.clone(),
            payout_recipient: binding.payout_recipient.clone(),
        };

        // 5. MMR Proof Payload
        let (sibling_path, mmr_peaks, mmr_root, peak_index, leaf_count) =
            if self.mmr.leaf_count > leaf_index as usize {
                let p = self.mmr.generate_proof(leaf_index as usize);
                let siblings: Vec<String> = p
                    .mountain_siblings
                    .iter()
                    .map(|fr| format!("0x{}", hex::encode(fr_to_be_bytes(fr))))
                    .collect();
                let peaks: Vec<String> = self
                    .mmr
                    .get_peaks()
                    .iter()
                    .map(|fr| format!("0x{}", hex::encode(fr_to_be_bytes(fr))))
                    .collect();
                let root = format!("0x{}", hex::encode(fr_to_be_bytes(&self.mmr.get_root())));
                (
                    siblings,
                    peaks,
                    root,
                    p.peak_index,
                    self.mmr.leaf_count as u64,
                )
            } else if let Some(cached) = self.get_any_cached_mmr_proof_for_leaf(leaf_index) {
                (
                    cached.mountain_siblings.clone(),
                    cached.peak_bagging_siblings.clone(),
                    cached.bagged_root.clone(),
                    0,
                    cached.leaf_count,
                )
            } else {
                let siblings = note
                    .merkle_path_hex
                    .clone()
                    .unwrap_or_default()
                    .iter()
                    .map(|s| {
                        let clean = s.strip_prefix("0x").unwrap_or(s);
                        format!("0x{}", clean)
                    })
                    .collect();
                let root = self.current_merkle_root_hex.clone().unwrap_or_else(|| {
                    "0x0000000000000000000000000000000000000000000000000000000000000000".to_string()
                });
                (siblings, vec![], root, 0, note.leaf_count.unwrap_or(1))
            };

        let mmr_proof_payload = MmrProofPayload {
            leaf_index,
            leaf_count,
            peak_index,
            sibling_path,
            mmr_peaks,
            mmr_root,
        };

        let verifier_binding_payload = VerifierBindingPayload {
            verifier_id: binding.verifier_id,
            account_uid: binding.account_uid,
            nonce: binding.nonce,
            expires_at: binding.expires_at,
            challenge_hash: challenge_hash_hex,
        };

        let ownership_zk_proof_payload = OwnershipZkProofPayload {
            protocol: "GROTH16_BLS12_381_DISCLOSURE_V1".to_string(),
            public_inputs: public_inputs_hex,
            proof_bytes: proof_bytes_hex,
        };

        Ok(CexProofPayload {
            schema_version: "zeltra.compliance.cex.v1".to_string(),
            generated_at: now,
            verifier_binding: verifier_binding_payload,
            asset: asset_payload,
            note_cryptography: note_crypto_payload,
            mmr_proof: mmr_proof_payload,
            ownership_zk_proof: ownership_zk_proof_payload,
        })
    }
}

static DISCLOSURE_KEYS: std::sync::OnceLock<(
    ark_groth16::ProvingKey<Bls12_381>,
    ark_groth16::VerifyingKey<Bls12_381>,
)> = std::sync::OnceLock::new();

/// Retrieve or lazily initialize cached proving and verifying keys for SelectiveDisclosureCircuit.
pub fn get_or_init_disclosure_keys() -> &'static (
    ark_groth16::ProvingKey<Bls12_381>,
    ark_groth16::VerifyingKey<Bls12_381>,
) {
    DISCLOSURE_KEYS.get_or_init(|| {
        use rand::SeedableRng;
        let mut rng = rand::rngs::StdRng::seed_from_u64(0x42_45_43_30_34_30_41);
        zeltra_core::selective_disclosure_circuit::setup_disclosure_circuit_params(&mut rng)
    })
}

/// Internal core engine to build, filter, and mathematically reconcile an AuditReport (DEC-040B).
pub fn build_audit_report_inner(
    source_records: &[AuditRecord],
    filter: &AuditFilter,
    unspent_map: &HashMap<u8, u64>,
) -> Result<AuditReport, ComplianceError> {
    // 1. Filter records
    let mut filtered_records: Vec<AuditRecord> = source_records
        .iter()
        .filter(|r| {
            if let Some(target_asset) = filter.asset_id {
                if r.asset_id != target_asset {
                    return false;
                }
            }
            if let Some(from) = filter.from_timestamp {
                if r.timestamp < from {
                    return false;
                }
            }
            if let Some(to) = filter.to_timestamp {
                if r.timestamp > to {
                    return false;
                }
            }
            if !filter.include_spent && r.direction == RecordDirection::OutflowSpend {
                return false;
            }
            true
        })
        .cloned()
        .collect();

    // Sort by timestamp and record_id
    filtered_records.sort_by_key(|r| (r.timestamp, r.record_id));

    // 2. Validate per-record UTXO conservation
    validate_utxo_conservation(&filtered_records)?;

    // 3. Determine period range
    let from_timestamp = filter.from_timestamp.unwrap_or_else(|| {
        filtered_records
            .iter()
            .map(|r| r.timestamp)
            .min()
            .unwrap_or(0)
    });
    let to_timestamp = filter.to_timestamp.unwrap_or_else(|| {
        filtered_records
            .iter()
            .map(|r| r.timestamp)
            .max()
            .unwrap_or_else(|| {
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs()
            })
    });

    // 4. Determine assets to summarize
    let asset_ids: Vec<u8> = if let Some(id) = filter.asset_id {
        vec![id]
    } else {
        let mut ids: Vec<u8> = filtered_records.iter().map(|r| r.asset_id).collect();
        for &k in unspent_map.keys() {
            if !ids.contains(&k) {
                ids.push(k);
            }
        }
        if ids.is_empty() {
            ids.push(1); // default USDC
        }
        ids.sort();
        ids.dedup();
        ids
    };

    let mut summary_by_asset = HashMap::new();

    for asset_id in asset_ids {
        let decimals = crate::compliance::asset_decimals(asset_id)?;
        let symbol = crate::compliance::asset_symbol(asset_id).to_string();

        // Calculate opening balance from prior records before from_timestamp
        let opening_balance_raw = if let Some(from) = filter.from_timestamp {
            let mut prior_inflow = 0u64;
            let mut prior_outflow = 0u64;
            for r in source_records {
                if r.asset_id == asset_id && r.timestamp < from {
                    match r.direction {
                        RecordDirection::InflowDeposit => {
                            let dep = parse_asset_amount(&r.deposit_amount, asset_id)?;
                            prior_inflow = prior_inflow.saturating_add(dep);
                        }
                        RecordDirection::OutflowSpend => {
                            let payout = parse_asset_amount(&r.merchant_payout, asset_id)?;
                            let proto = parse_asset_amount(&r.protocol_fee, asset_id)?;
                            let exec = parse_asset_amount(&r.relayer_execution_fee, asset_id)?;
                            let spent = payout.saturating_add(proto).saturating_add(exec);
                            prior_outflow = prior_outflow.saturating_add(spent);
                        }
                        RecordDirection::InPoolConsolidation => {
                            let proto = parse_asset_amount(&r.protocol_fee, asset_id)?;
                            let exec = parse_asset_amount(&r.relayer_execution_fee, asset_id)?;
                            prior_outflow =
                                prior_outflow.saturating_add(proto).saturating_add(exec);
                        }
                    }
                }
            }
            prior_inflow.saturating_sub(prior_outflow)
        } else {
            0u64
        };

        let mut total_inflow_raw = 0u64;
        let mut total_payout_raw = 0u64;
        let mut total_proto_fee_raw = 0u64;
        let mut total_exec_fee_raw = 0u64;

        for r in &filtered_records {
            if r.asset_id == asset_id {
                match r.direction {
                    RecordDirection::InflowDeposit => {
                        let dep = parse_asset_amount(&r.deposit_amount, asset_id)?;
                        total_inflow_raw = total_inflow_raw.saturating_add(dep);
                    }
                    RecordDirection::OutflowSpend => {
                        let payout = parse_asset_amount(&r.merchant_payout, asset_id)?;
                        let proto = parse_asset_amount(&r.protocol_fee, asset_id)?;
                        let exec = parse_asset_amount(&r.relayer_execution_fee, asset_id)?;
                        total_payout_raw = total_payout_raw.saturating_add(payout);
                        total_proto_fee_raw = total_proto_fee_raw.saturating_add(proto);
                        total_exec_fee_raw = total_exec_fee_raw.saturating_add(exec);
                    }
                    RecordDirection::InPoolConsolidation => {
                        let proto = parse_asset_amount(&r.protocol_fee, asset_id)?;
                        let exec = parse_asset_amount(&r.relayer_execution_fee, asset_id)?;
                        total_proto_fee_raw = total_proto_fee_raw.saturating_add(proto);
                        total_exec_fee_raw = total_exec_fee_raw.saturating_add(exec);
                    }
                }
            }
        }

        let total_expenses = total_payout_raw
            .saturating_add(total_proto_fee_raw)
            .saturating_add(total_exec_fee_raw);
        let closing_raw = opening_balance_raw
            .saturating_add(total_inflow_raw)
            .saturating_sub(total_expenses);

        let summary = AssetSummary {
            asset_id,
            symbol,
            circuit_decimals: decimals,
            opening_balance: format_asset_amount(opening_balance_raw, asset_id),
            total_inflow_deposit: format_asset_amount(total_inflow_raw, asset_id),
            total_merchant_payout: format_asset_amount(total_payout_raw, asset_id),
            total_protocol_fees: format_asset_amount(total_proto_fee_raw, asset_id),
            total_relayer_execution_fees: format_asset_amount(total_exec_fee_raw, asset_id),
            closing_unspent_balance: format_asset_amount(closing_raw, asset_id),
        };

        let key = if asset_id == 0 { "weth" } else { "usdc" };
        summary_by_asset.insert(key.to_string(), summary);
    }

    let report = AuditReport {
        schema_version: "zeltra.compliance.audit.v1".to_string(),
        generated_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
        organization_name: filter
            .organization_name
            .clone()
            .unwrap_or_else(|| "Zeltra Protocol Client".to_string()),
        report_period: PeriodRange {
            from_timestamp,
            to_timestamp,
        },
        summary_by_asset,
        records: filtered_records,
    };

    // 5. Fail-closed balance sheet reconciliation
    validate_balance_sheet_reconciliation(&report, unspent_map)?;

    Ok(report)
}

/// Read-Only Auditor Session (Tier 3 FVK — DEC-040B Section 5).
///
/// Holds a FullViewingKey (`fvk`) and can decrypt incoming memos, monitor on-chain nullifiers,
/// calculate balances, and export audit trails.
/// Strictly forbids spending or spend proof generation with `WalletError::SpendAuthorityRequired`.
#[derive(Clone, Serialize, Deserialize)]
pub struct ReadOnlyAuditorSession {
    pub fvk: FullViewingKey,
    pub notes: HashMap<String, WalletNote>,
    #[serde(default)]
    pub usdc_notes: Vec<PrivateNoteV1>,
    #[serde(default)]
    pub weth_notes: Vec<PrivateNoteV1>,
    #[serde(default)]
    pub audit_records: Vec<AuditRecord>,
    pub current_merkle_root_hex: Option<String>,
}

impl ReadOnlyAuditorSession {
    pub fn new(fvk: FullViewingKey) -> Self {
        Self {
            fvk,
            notes: HashMap::new(),
            usdc_notes: Vec::new(),
            weth_notes: Vec::new(),
            audit_records: Vec::new(),
            current_merkle_root_hex: None,
        }
    }

    pub fn fvk(&self) -> &FullViewingKey {
        &self.fvk
    }

    pub fn record_audit_entry(&mut self, record: AuditRecord) {
        self.audit_records.push(record);
    }

    pub fn get_balance(&self, asset_id: u8) -> Result<u64, WalletError> {
        match asset_id {
            0 => {
                let from_notes: u64 = self
                    .notes
                    .values()
                    .filter(|n| n.status == NoteStatus::Unspent && n.asset_id == 0)
                    .map(|n| n.value)
                    .sum();
                if from_notes > 0 {
                    Ok(from_notes)
                } else {
                    Ok(self.weth_notes.iter().map(|n| n.value).sum())
                }
            }
            1 => {
                let from_notes: u64 = self
                    .notes
                    .values()
                    .filter(|n| n.status == NoteStatus::Unspent && n.asset_id == 1)
                    .map(|n| n.value)
                    .sum();
                if from_notes > 0 {
                    Ok(from_notes)
                } else {
                    Ok(self.usdc_notes.iter().map(|n| n.value).sum())
                }
            }
            _ => Err(WalletError::UnsupportedAssetId(asset_id)),
        }
    }

    pub fn get_human_balance(&self, asset_id: u8) -> Result<String, WalletError> {
        let raw = self.get_balance(asset_id)?;
        match asset_id {
            0 => Ok(format!("{:.6} ETH", raw as f64 / 1_000_000_000.0)),
            1 => Ok(format!("${:.2} USDC", raw as f64 / 1_000_000.0)),
            _ => Err(WalletError::UnsupportedAssetId(asset_id)),
        }
    }

    pub fn get_unspent_balance(&self, asset_id: u8) -> u64 {
        self.get_balance(asset_id).unwrap_or(0)
    }

    pub fn export_audit_trail(&self, filter: AuditFilter) -> Result<AuditReport, WalletError> {
        let mut unspent_map = HashMap::new();
        unspent_map.insert(0, self.get_unspent_balance(0));
        unspent_map.insert(1, self.get_unspent_balance(1));

        let records = if !self.audit_records.is_empty() {
            self.audit_records.clone()
        } else {
            let mut synth = Vec::new();
            for (id, (cm, note)) in (1u64..).zip(self.notes.iter()) {
                let sym = if note.asset_id == 0 { "WETH" } else { "USDC" };
                let val_str = format_asset_amount(note.value, note.asset_id);
                let zero_str = format_asset_amount(0, note.asset_id);
                synth.push(AuditRecord {
                    record_id: id,
                    timestamp: note.created_at_secs,
                    date_iso: format_iso8601(note.created_at_secs),
                    tx_hash: "0x0000000000000000000000000000000000000000000000000000000000000000"
                        .to_string(),
                    direction: RecordDirection::InflowDeposit,
                    asset_id: note.asset_id,
                    symbol: sym.to_string(),
                    input_notes_total: zero_str.clone(),
                    deposit_amount: val_str,
                    merchant_payout: zero_str.clone(),
                    protocol_fee: zero_str.clone(),
                    relayer_execution_fee: zero_str.clone(),
                    change_amount: zero_str,
                    created_commitments: vec![cm.clone()],
                    spent_nullifiers: vec![],
                    recipient: None,
                    status: "SETTLED".to_string(),
                });
            }
            synth
        };

        build_audit_report_inner(&records, &filter, &unspent_map)
            .map_err(|e| WalletError::ComplianceError(e.to_string()))
    }

    /// Recovers notes from on-chain NoteEvent logs using batch nullifier checks (DEC-037 & DEC-040B).
    pub fn recover_notes_batch<F>(
        &mut self,
        events: &[NoteEvent],
        check_nullifiers_spent_batch: F,
    ) -> Result<RecoveryResult, WalletError>
    where
        F: FnOnce(&[String]) -> Result<Vec<bool>, WalletError>,
    {
        let ivk = IncomingViewingKey::from_bytes(&self.fvk.incoming_viewing_key);
        let mut res = RecoveryResult {
            scanned_events: events.len(),
            ..Default::default()
        };

        struct Candidate {
            cm_hex: String,
            nf_hex: String,
            leaf_index: u64,
            leaf_count: u64,
            value: u64,
            asset_id: u8,
            rho: Fr,
            randomness: Fr,
        }

        let mut candidates = Vec::new();

        for ev in events {
            if ev.encrypted_memo.is_empty() {
                continue;
            }
            if let Some((pt, rand)) = decrypt_note_memo_deterministic(&ev.encrypted_memo, &ivk) {
                if pt.value == 0 {
                    continue;
                }
                let ev_cm_clean = ev
                    .commitment_hex
                    .strip_prefix("0x")
                    .unwrap_or(&ev.commitment_hex);

                let cm_ecdh = note_commitment(
                    pt.value,
                    pt.asset_id as u8,
                    self.fvk.owner_key,
                    pt.rho,
                    rand,
                );
                let r_legacy = zeltra_core::derive_note_randomness(self.fvk.owner_key, pt.rho);
                let cm_legacy = note_commitment(
                    pt.value,
                    pt.asset_id as u8,
                    self.fvk.owner_key,
                    pt.rho,
                    r_legacy,
                );

                let (matched_cm, matched_rand) = if hex::encode(fr_to_be_bytes(&cm_ecdh))
                    .eq_ignore_ascii_case(ev_cm_clean)
                {
                    (cm_ecdh, rand)
                } else if hex::encode(fr_to_be_bytes(&cm_legacy)).eq_ignore_ascii_case(ev_cm_clean)
                {
                    (cm_legacy, r_legacy)
                } else {
                    continue;
                };

                let cm_hex = format!("0x{}", hex::encode(fr_to_be_bytes(&matched_cm)));
                let nullifier = self.fvk.derive_nullifier(matched_cm, ev.leaf_index, 0);
                let nf_hex = format!("0x{}", hex::encode(fr_to_be_bytes(&nullifier)));

                candidates.push(Candidate {
                    cm_hex,
                    nf_hex,
                    leaf_index: ev.leaf_index,
                    leaf_count: ev.leaf_count,
                    value: pt.value,
                    asset_id: pt.asset_id as u8,
                    rho: pt.rho,
                    randomness: matched_rand,
                });
            }
        }

        if candidates.is_empty() {
            return Ok(res);
        }

        let nfs: Vec<String> = candidates.iter().map(|c| c.nf_hex.clone()).collect();
        let spent_flags = check_nullifiers_spent_batch(&nfs)?;
        if spent_flags.len() != candidates.len() {
            return Err(WalletError::CryptoError(
                "Batch nullifier response count mismatch".to_string(),
            ));
        }

        let now_secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        for (cand, is_spent) in candidates.into_iter().zip(spent_flags) {
            let status = if is_spent {
                res.total_spent_balance += cand.value;
                NoteStatus::Spent {
                    nullifier_hex: cand.nf_hex.clone(),
                    spent_at_secs: now_secs,
                }
            } else {
                res.total_unspent_balance += cand.value;
                NoteStatus::Unspent
            };

            let wallet_note = WalletNote {
                commitment_hex: cand.cm_hex.clone(),
                value: cand.value,
                asset_id: cand.asset_id,
                owner_key_hex: hex::encode(fr_to_be_bytes(&self.fvk.owner_key)),
                rho_hex: hex::encode(fr_to_be_bytes(&cand.rho)),
                randomness_hex: hex::encode(fr_to_be_bytes(&cand.randomness)),
                leaf_index: Some(cand.leaf_index),
                leaf_count: Some(cand.leaf_count),
                epoch_id: Some(0),
                merkle_path_hex: None,
                status,
                created_at_secs: now_secs,
                session_id: None,
            };

            res.recovered_notes.push(RecoveredNote {
                commitment_hex: cand.cm_hex.clone(),
                nullifier_hex: cand.nf_hex,
                leaf_index: cand.leaf_index,
                value: cand.value,
                asset_id: cand.asset_id as u64,
                rho_hex: format!("0x{}", hex::encode(fr_to_be_bytes(&cand.rho))),
                is_spent,
            });

            self.notes.insert(cand.cm_hex, wallet_note);
        }

        Ok(res)
    }

    /// Recovers notes from on-chain NoteEvent logs using scalar predicate checking (DEC-037).
    pub fn recover_notes<F>(
        &mut self,
        events: &[NoteEvent],
        is_nullifier_spent_fn: F,
    ) -> RecoveryResult
    where
        F: Fn(&str) -> bool,
    {
        self.recover_notes_batch(events, |nfs| {
            Ok(nfs.iter().map(|nf| is_nullifier_spent_fn(nf)).collect())
        })
        .unwrap_or_default()
    }

    // ═══════════════════════════════════════════════════════════════════════
    // Spend Authority Guardrails: Fails closed with SpendAuthorityRequired
    // ═══════════════════════════════════════════════════════════════════════

    pub fn prepare_spend_proof(
        &self,
        _commitment_hex: &str,
        _recipient: [u8; 32],
        _amount: u64,
        _protocol_fee: u64,
        _execution_fee: u64,
        _chain_id: u64,
    ) -> Result<SpendProofPayload, WalletError> {
        Err(WalletError::SpendAuthorityRequired)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn prepare_spend_proof_with_epoch(
        &self,
        _commitment_hex: &str,
        _recipient: [u8; 32],
        _amount: u64,
        _protocol_fee: u64,
        _execution_fee: u64,
        _chain_id: u64,
        _current_epoch: u32,
    ) -> Result<SpendProofPayload, WalletError> {
        Err(WalletError::SpendAuthorityRequired)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn prepare_spend_proof_with_mmr(
        &self,
        _commitment_hex: &str,
        _recipient: [u8; 32],
        _amount: u64,
        _protocol_fee: u64,
        _execution_fee: u64,
        _chain_id: u64,
        _contract_address: [u8; 32],
        _expiry: u64,
        _quote_hash: [u8; 32],
        _current_epoch: u32,
    ) -> Result<SpendProofPayload, WalletError> {
        Err(WalletError::SpendAuthorityRequired)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn prepare_joinsplit_spend_proof(
        &self,
        _selected: &SelectedSpend,
        _recipient: [u8; 32],
        _chain_id: u64,
        _contract_address: [u8; 32],
        _expiry: u64,
        _quote_hash: [u8; 32],
        _current_epoch: u32,
    ) -> Result<JoinSplitSpendProofPayload, WalletError> {
        Err(WalletError::SpendAuthorityRequired)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn prepare_shielded_transfer_proof(
        &self,
        _recipient_zsa_bech32m_or_hex: &str,
        _transfer_amount: u64,
        _asset_id: u8,
        _max_execution_fee: u64,
        _quote_hash_hex: &str,
        _chain_id: u64,
        _contract_address_hex: &str,
        _expiry: u64,
        _current_time_secs: u64,
        _joinsplit_pk: &ark_groth16::ProvingKey<Bls12_381>,
    ) -> Result<JoinSplitSpendProofPayload, WalletError> {
        Err(WalletError::SpendAuthorityRequired)
    }

    pub fn create_cex_proof(
        &self,
        _commitment: [u8; 32],
        _binding: VerifierBinding,
    ) -> Result<CexProofPayload, WalletError> {
        Err(WalletError::SpendAuthorityRequired)
    }
}

/// Standalone recovery function using batch nullifier checking (DEC-037).
pub fn recover_from_viewing_key_batch<F>(
    ivk: &IncomingViewingKey,
    spending_key: Option<Fr>,
    events: &[NoteEvent],
    check_nullifiers_spent_batch: F,
) -> Result<RecoveryResult, WalletError>
where
    F: FnOnce(&[String]) -> Result<Vec<bool>, WalletError>,
{
    let mut res = RecoveryResult {
        scanned_events: events.len(),
        ..Default::default()
    };

    let nk = spending_key.map(derive_nullifier_key);

    struct Cand {
        cm_hex: String,
        nf_hex: String,
        leaf_index: u64,
        value: u64,
        asset_id: u64,
        rho: Fr,
    }

    let mut candidates = Vec::new();

    for ev in events {
        if ev.encrypted_memo.is_empty() {
            continue;
        }
        if let Some(pt) = decrypt_note_memo(&ev.encrypted_memo, ivk) {
            let (cm_hex, nf_hex) = if let (Some(sk), Some(nk_val)) = (spending_key, nk) {
                let note = pt.to_note(sk);
                let cm = note.commitment();
                let cm_hex = format!("0x{}", hex::encode(fr_to_be_bytes(&cm)));
                let ev_cm_clean = ev
                    .commitment_hex
                    .strip_prefix("0x")
                    .unwrap_or(&ev.commitment_hex);
                if hex::encode(fr_to_be_bytes(&cm)).to_lowercase() != ev_cm_clean.to_lowercase() {
                    continue;
                }
                let nullifier = derive_nullifier(nk_val, cm, ev.leaf_index, 0);
                let nf_hex = format!("0x{}", hex::encode(fr_to_be_bytes(&nullifier)));
                (cm_hex, nf_hex)
            } else {
                (ev.commitment_hex.clone(), String::new())
            };

            candidates.push(Cand {
                cm_hex,
                nf_hex,
                leaf_index: ev.leaf_index,
                value: pt.value,
                asset_id: pt.asset_id,
                rho: pt.rho,
            });
        }
    }

    if candidates.is_empty() {
        return Ok(res);
    }

    // Only query batch nullifier status if spending_key is present (and thus nullifiers derived)
    let spent_flags = if spending_key.is_some() {
        let nfs: Vec<String> = candidates.iter().map(|c| c.nf_hex.clone()).collect();
        check_nullifiers_spent_batch(&nfs)?
    } else {
        vec![false; candidates.len()]
    };

    for (cand, is_spent) in candidates.into_iter().zip(spent_flags) {
        if is_spent {
            res.total_spent_balance += cand.value;
        } else {
            res.total_unspent_balance += cand.value;
        }

        res.recovered_notes.push(RecoveredNote {
            commitment_hex: cand.cm_hex,
            nullifier_hex: cand.nf_hex,
            leaf_index: cand.leaf_index,
            value: cand.value,
            asset_id: cand.asset_id,
            rho_hex: format!("0x{}", hex::encode(fr_to_be_bytes(&cand.rho))),
            is_spent,
        });
    }

    Ok(res)
}

/// Standalone recovery function from an Incoming Viewing Key (IVK) (DEC-037).
/// Allows recovery and audit scanning without requiring the wallet instance or spending key.
pub fn recover_from_viewing_key<F>(
    ivk: &IncomingViewingKey,
    spending_key: Option<Fr>,
    events: &[NoteEvent],
    is_nullifier_spent_fn: F,
) -> RecoveryResult
where
    F: Fn(&str) -> bool,
{
    recover_from_viewing_key_batch(ivk, spending_key, events, |nfs| {
        Ok(nfs.iter().map(|nf| is_nullifier_spent_fn(nf)).collect())
    })
    .unwrap_or_else(|_| RecoveryResult {
        scanned_events: events.len(),
        ..Default::default()
    })
}

/// Helper function to batch query double-spend status of nullifiers
pub fn check_nullifiers_spent_batch<F>(
    nullifiers: &[String],
    is_spent_query: F,
) -> Result<Vec<bool>, WalletError>
where
    F: Fn(&[String]) -> Result<Vec<bool>, WalletError>,
{
    is_spent_query(nullifiers)
}
