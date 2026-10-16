//! Standalone CEX Selective Disclosure Verifier Engine (DEC-040C).
//!
//! Provides a stateless, zero-secret verification engine that centralized exchanges (Binance, Coinbase, Kraken)
//! and external compliance auditors can run independently to validate source-of-funds proofs.
//!
//! Executes a 5-Stage deterministic verification pipeline:
//! 1. Format validation & Poseidon-W5 commitment re-computation with asset bit-packing (DEC-034A).
//! 2. Merkle Mountain Range (MMR) inclusion path reconstruction & peak bagging (DEC-032).
//! 3. Verifier binding anti-impersonation challenge hash & expiry check (DEC-040 Section 4.B).
//! 4. Groth16 NIZK proof verification over 5 public inputs (DEC-040A).
//! 5. On-chain double-spend nullifier status & payout transaction receipt verification.

use ark_bls12_381::{Bls12_381, Fr};
use ark_serialize::CanonicalDeserialize;
use serde_json::Value;
use std::collections::{HashMap, HashSet};

use crate::compliance::models::{CexProofPayload, VerificationSummary};
use crate::compliance::{compute_verifier_challenge, ComplianceError};

/// Blockchain RPC client trait for on-chain state attestation (DEC-040C).
pub trait BlockchainRpcClient: Send + Sync {
    /// Checks if a root hash is currently accepted on the smart contract.
    fn is_accepted_note_root(
        &self,
        root: [u8; 32],
    ) -> impl std::future::Future<Output = Result<bool, ComplianceError>> + Send;

    /// Checks if a nullifier is marked spent on the smart contract.
    fn is_nullifier_spent(
        &self,
        nullifier: [u8; 32],
    ) -> impl std::future::Future<Output = Result<bool, ComplianceError>> + Send;

    /// Verifies that a transaction receipt exists, succeeded, and transferred to expected recipient.
    fn verify_spend_receipt_recipient(
        &self,
        tx_hash: &str,
        asset_id: u8,
        expected_recipient: &str,
    ) -> impl std::future::Future<Output = Result<bool, ComplianceError>> + Send;
}

/// Standalone CEX Proof Verifier executing 5-stage deterministic validation.
pub struct CexProofVerifier;

impl CexProofVerifier {
    /// Executes the 5-stage verification algorithm against a `CexProofPayload`.
    ///
    /// Fail-closed: any format error, cryptographic mismatch, or RPC failure immediately aborts.
    pub async fn verify<C: BlockchainRpcClient>(
        payload: &CexProofPayload,
        expected_verifier_id: &str,
        current_time: u64,
        rpc_client: &C,
    ) -> Result<VerificationSummary, ComplianceError> {
        // ═══════════════════════════════════════════════════════════════════
        // TAHAP 1: Validasi Format & Re-komputasi Komitmen Poseidon-W5 (DEC-034A)
        // ═══════════════════════════════════════════════════════════════════
        let asset_id = payload.asset.asset_id;
        if asset_id > 1 {
            return Err(ComplianceError::UnsupportedAssetId(asset_id));
        }

        let raw_val: u64 = payload.asset.raw_circuit_value.parse().map_err(|_| {
            ComplianceError::InvalidDecimalFormat {
                raw: payload.asset.raw_circuit_value.clone(),
                asset_id,
            }
        })?;

        let owner_key = parse_fr(&payload.note_cryptography.owner_key)?;
        let rho = parse_fr(&payload.note_cryptography.rho)?;
        let randomness = parse_fr(&payload.note_cryptography.randomness)?;
        let claimed_cm = parse_fr(&payload.note_cryptography.commitment)?;

        // value_packed = value + asset_id * 2^64 (DEC-034A)
        let two_pow_64 = Fr::from(1u128 << 64);
        let value_packed = Fr::from(raw_val) + (Fr::from(asset_id as u64) * two_pow_64);
        let computed_cm =
            zeltra_core::note_commitment_packed(value_packed, owner_key, rho, randomness);

        if computed_cm != claimed_cm {
            return Err(ComplianceError::CommitmentPreimageMismatch);
        }

        // ═══════════════════════════════════════════════════════════════════
        // TAHAP 2: Verifikasi Jalur Merkle Mountain Range (MMR, DEC-032)
        // ═══════════════════════════════════════════════════════════════════
        let leaf_count = payload.mmr_proof.leaf_count as usize;
        let leaf_index = payload.mmr_proof.leaf_index as usize;

        if leaf_count == 0 || leaf_index >= leaf_count {
            return Err(ComplianceError::InvalidMmrInclusionProof);
        }

        let mut mountain_siblings = Vec::with_capacity(payload.mmr_proof.sibling_path.len());
        for s in &payload.mmr_proof.sibling_path {
            mountain_siblings.push(parse_fr(s)?);
        }

        // Reconstruct mountain peak from leaf and sibling path
        let mut current = claimed_cm;
        let mut idx = leaf_index;
        for sibling in &mountain_siblings {
            current = if idx.is_multiple_of(2) {
                zeltra_core::merkle_hash(current, *sibling)
            } else {
                zeltra_core::merkle_hash(*sibling, current)
            };
            idx /= 2;
        }
        let computed_peak = current;

        let mut peaks = Vec::with_capacity(payload.mmr_proof.mmr_peaks.len());
        for p in &payload.mmr_proof.mmr_peaks {
            peaks.push(parse_fr(p)?);
        }

        // Verify peak consistency
        if peaks.is_empty() {
            if leaf_count == 1 && mountain_siblings.is_empty() {
                peaks.push(computed_peak);
            } else {
                return Err(ComplianceError::InvalidMmrInclusionProof);
            }
        } else if payload.mmr_proof.peak_index < peaks.len() {
            if peaks[payload.mmr_proof.peak_index] != computed_peak {
                return Err(ComplianceError::InvalidMmrInclusionProof);
            }
        } else {
            return Err(ComplianceError::InvalidMmrInclusionProof);
        }

        let computed_root = zeltra_core::MerkleMountainRange::bag_peaks(&peaks, leaf_count);
        let claimed_root = parse_fr(&payload.mmr_proof.mmr_root)?;

        if computed_root != claimed_root {
            return Err(ComplianceError::InvalidMmrInclusionProof);
        }

        // Verify that the root has been attested on-chain by the smart contract
        let claimed_root_bytes = zeltra_core::fr_to_be_bytes(&claimed_root);
        let is_root_valid = rpc_client.is_accepted_note_root(claimed_root_bytes).await?;
        if !is_root_valid {
            return Err(ComplianceError::MmrRootNotAcceptedOnChain);
        }

        // ═══════════════════════════════════════════════════════════════════
        // TAHAP 3: Verifikasi Verifier Binding & Kedaluwarsa (DEC-040)
        // ═══════════════════════════════════════════════════════════════════
        if payload.verifier_binding.verifier_id != expected_verifier_id {
            return Err(ComplianceError::VerifierIdMismatch {
                expected: expected_verifier_id.to_string(),
                actual: payload.verifier_binding.verifier_id.clone(),
            });
        }

        if current_time > payload.verifier_binding.expires_at {
            return Err(ComplianceError::ProofExpired);
        }

        // Maximum allowed validity window is 24 hours (86,400 seconds)
        if payload.verifier_binding.expires_at > payload.generated_at.saturating_add(86400) {
            return Err(ComplianceError::VerificationError(
                "Verifier binding expiry exceeds maximum allowed 24-hour window".to_string(),
            ));
        }

        let nullifier_fr = parse_fr(&payload.note_cryptography.nullifier)?;

        let expected_challenge_bytes = compute_verifier_challenge(
            &payload.verifier_binding.verifier_id,
            &payload.verifier_binding.account_uid,
            payload.generated_at,
            payload.verifier_binding.expires_at,
            &payload.verifier_binding.nonce,
            &claimed_cm,
            &nullifier_fr,
            &payload.note_cryptography.spent_tx_hash,
            &payload.note_cryptography.payout_recipient,
        );

        let clean_challenge_hex = payload
            .verifier_binding
            .challenge_hash
            .strip_prefix("0x")
            .unwrap_or(&payload.verifier_binding.challenge_hash);
        let expected_challenge_hex = hex::encode(expected_challenge_bytes);

        if clean_challenge_hex.to_lowercase() != expected_challenge_hex.to_lowercase() {
            return Err(ComplianceError::ChallengeHashMismatch);
        }

        // ═══════════════════════════════════════════════════════════════════
        // TAHAP 4: Verifikasi Bukti Groth16 NIZK (SelectiveDisclosureCircuit)
        // ═══════════════════════════════════════════════════════════════════
        let clean_proof = payload
            .ownership_zk_proof
            .proof_bytes
            .strip_prefix("0x")
            .unwrap_or(&payload.ownership_zk_proof.proof_bytes);
        let proof_bytes =
            hex::decode(clean_proof).map_err(|_| ComplianceError::InvalidProofBytes)?;

        let proof = ark_groth16::Proof::<Bls12_381>::deserialize_compressed(&proof_bytes[..])
            .map_err(|_| ComplianceError::InvalidProofBytes)?;

        let (challenge_hi, challenge_lo) =
            zeltra_core::selective_disclosure_circuit::decompose_challenge_to_limbs(
                &expected_challenge_bytes,
            );

        let public_inputs = vec![owner_key, rho, nullifier_fr, challenge_hi, challenge_lo];

        let (_, vk) = crate::wallet::note_wallet::get_or_init_disclosure_keys();
        let pvk = ark_groth16::prepare_verifying_key(vk);

        let is_zk_valid = zeltra_core::selective_disclosure_circuit::verify_disclosure_proof(
            &proof,
            &public_inputs,
            &pvk,
        )
        .map_err(|e| ComplianceError::VerificationError(format!("ZK synthesis error: {e}")))?;

        if !is_zk_valid {
            return Err(ComplianceError::InvalidDisclosureZkProof);
        }

        // ═══════════════════════════════════════════════════════════════════
        // TAHAP 5: Verifikasi Status On-Chain Transaksi Pencairan
        // ═══════════════════════════════════════════════════════════════════
        let nf_bytes = zeltra_core::fr_to_be_bytes(&nullifier_fr);
        let is_spent = rpc_client.is_nullifier_spent(nf_bytes).await?;
        if !is_spent {
            return Err(ComplianceError::NullifierNotSpentOnChain);
        }

        let receipt_valid = rpc_client
            .verify_spend_receipt_recipient(
                &payload.note_cryptography.spent_tx_hash,
                asset_id,
                &payload.note_cryptography.payout_recipient,
            )
            .await?;

        if !receipt_valid {
            return Err(ComplianceError::InvalidRecipientOrTx(
                "Spend transaction receipt failed validation or recipient mismatch".to_string(),
            ));
        }

        Ok(VerificationSummary {
            status: "SUCCESS".into(),
            asset_symbol: payload.asset.symbol.clone(),
            amount_human: payload.asset.human_value.clone(),
            recipient: payload.note_cryptography.payout_recipient.clone(),
            account_uid: payload.verifier_binding.account_uid.clone(),
            verified_at: current_time,
        })
    }
}

/// Parses an arkworks Fr scalar from a 0x-prefixed or bare hex string.
pub fn parse_fr(s: &str) -> Result<Fr, ComplianceError> {
    let clean = s.strip_prefix("0x").unwrap_or(s);
    let bytes = hex::decode(clean)
        .map_err(|e| ComplianceError::VerificationError(format!("Invalid hex for Fr: {e}")))?;
    if bytes.len() != 32 {
        return Err(ComplianceError::VerificationError(format!(
            "Invalid byte length for Fr: expected 32, got {}",
            bytes.len()
        )));
    }
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&bytes);
    zeltra_core::fr_from_be_bytes(&arr)
        .ok_or_else(|| ComplianceError::VerificationError("Non-canonical Fr scalar".to_string()))
}

/// Parses a 32-byte array from a 0x-prefixed or bare hex string.
pub fn parse_bytes32(s: &str) -> Result<[u8; 32], ComplianceError> {
    let clean = s.strip_prefix("0x").unwrap_or(s);
    let bytes = hex::decode(clean)
        .map_err(|e| ComplianceError::VerificationError(format!("Invalid hex for bytes32: {e}")))?;
    if bytes.len() != 32 {
        return Err(ComplianceError::VerificationError(format!(
            "Invalid byte length: expected 32, got {}",
            bytes.len()
        )));
    }
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&bytes);
    Ok(arr)
}

/// In-memory mock implementation of `BlockchainRpcClient` for testing.
#[derive(Clone, Debug, Default)]
pub struct MockBlockchainRpcClient {
    pub accepted_roots: HashSet<[u8; 32]>,
    pub spent_nullifiers: HashSet<[u8; 32]>,
    pub confirmed_spends: HashMap<String, (u8, String)>,
    pub should_fail_rpc: bool,
}

impl MockBlockchainRpcClient {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_accepted_root(&mut self, root: [u8; 32]) {
        self.accepted_roots.insert(root);
    }

    pub fn mark_nullifier_spent(&mut self, nullifier: [u8; 32]) {
        self.spent_nullifiers.insert(nullifier);
    }

    pub fn register_spend_receipt(&mut self, tx_hash: &str, asset_id: u8, recipient: &str) {
        self.confirmed_spends
            .insert(tx_hash.to_lowercase(), (asset_id, recipient.to_lowercase()));
    }

    pub fn set_fail_rpc(&mut self, fail: bool) {
        self.should_fail_rpc = fail;
    }
}

impl BlockchainRpcClient for MockBlockchainRpcClient {
    async fn is_accepted_note_root(&self, root: [u8; 32]) -> Result<bool, ComplianceError> {
        if self.should_fail_rpc {
            return Err(ComplianceError::RpcError(
                "RPC connection failure (Fail-Closed)".into(),
            ));
        }
        Ok(self.accepted_roots.contains(&root))
    }

    async fn is_nullifier_spent(&self, nullifier: [u8; 32]) -> Result<bool, ComplianceError> {
        if self.should_fail_rpc {
            return Err(ComplianceError::RpcError(
                "RPC connection failure (Fail-Closed)".into(),
            ));
        }
        Ok(self.spent_nullifiers.contains(&nullifier))
    }

    async fn verify_spend_receipt_recipient(
        &self,
        tx_hash: &str,
        asset_id: u8,
        expected_recipient: &str,
    ) -> Result<bool, ComplianceError> {
        if self.should_fail_rpc {
            return Err(ComplianceError::RpcError(
                "RPC connection failure (Fail-Closed)".into(),
            ));
        }
        if let Some((stored_asset, stored_recipient)) =
            self.confirmed_spends.get(&tx_hash.to_lowercase())
        {
            if *stored_asset == asset_id
                && stored_recipient.eq_ignore_ascii_case(expected_recipient)
            {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

/// Production JSON-RPC Client communicating with Arbitrum node over HTTP.
pub struct HttpBlockchainRpcClient {
    pub rpc_url: String,
    pub contract_address: String,
    http_client: reqwest::Client,
}

impl HttpBlockchainRpcClient {
    pub fn new(rpc_url: &str, contract_address: &str) -> Self {
        Self {
            rpc_url: rpc_url.to_string(),
            contract_address: contract_address.to_string(),
            http_client: reqwest::Client::new(),
        }
    }

    fn calculate_selector(signature: &str) -> [u8; 4] {
        use sha3::{Digest, Keccak256};
        let mut hasher = Keccak256::new();
        hasher.update(signature.as_bytes());
        let hash = hasher.finalize();
        let mut sel = [0u8; 4];
        sel.copy_from_slice(&hash[0..4]);
        sel
    }

    async fn eth_call(&self, data: &[u8]) -> Result<Vec<u8>, ComplianceError> {
        let payload = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_call",
            "params": [
                {
                    "to": self.contract_address,
                    "data": format!("0x{}", hex::encode(data))
                },
                "latest"
            ],
            "id": 1
        });

        let resp = self
            .http_client
            .post(&self.rpc_url)
            .json(&payload)
            .send()
            .await
            .map_err(|e| ComplianceError::RpcError(format!("HTTP request error: {e}")))?;

        if !resp.status().is_success() {
            return Err(ComplianceError::RpcError(format!(
                "RPC HTTP error status: {}",
                resp.status()
            )));
        }

        let body: Value = resp
            .json()
            .await
            .map_err(|e| ComplianceError::RpcError(format!("Failed to parse RPC response: {e}")))?;

        if let Some(err) = body.get("error") {
            return Err(ComplianceError::RpcError(format!(
                "RPC error response: {err}"
            )));
        }

        let result_hex = body
            .get("result")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ComplianceError::RpcError("Missing result in RPC response".into()))?;

        let clean = result_hex.strip_prefix("0x").unwrap_or(result_hex);
        hex::decode(clean)
            .map_err(|e| ComplianceError::RpcError(format!("Invalid hex in RPC response: {e}")))
    }
}

impl BlockchainRpcClient for HttpBlockchainRpcClient {
    async fn is_accepted_note_root(&self, root: [u8; 32]) -> Result<bool, ComplianceError> {
        // Try snake_case selector: is_accepted_note_root(bytes32)
        let sel = Self::calculate_selector("is_accepted_note_root(bytes32)");
        let mut call_data = Vec::with_capacity(36);
        call_data.extend_from_slice(&sel);
        call_data.extend_from_slice(&root);

        match self.eth_call(&call_data).await {
            Ok(bytes) => {
                if !bytes.is_empty() {
                    return Ok(bytes.last().copied().unwrap_or(0) != 0);
                }
            }
            Err(_) => {
                // Try camelCase selector: isAcceptedNoteRoot(bytes32)
                let alt_sel = Self::calculate_selector("isAcceptedNoteRoot(bytes32)");
                let mut alt_data = Vec::with_capacity(36);
                alt_data.extend_from_slice(&alt_sel);
                alt_data.extend_from_slice(&root);

                let bytes = self.eth_call(&alt_data).await?;
                if !bytes.is_empty() {
                    return Ok(bytes.last().copied().unwrap_or(0) != 0);
                }
            }
        }
        Ok(false)
    }

    async fn is_nullifier_spent(&self, nullifier: [u8; 32]) -> Result<bool, ComplianceError> {
        // Try snake_case selector: is_nullifier_spent(bytes32)
        let sel = Self::calculate_selector("is_nullifier_spent(bytes32)");
        let mut call_data = Vec::with_capacity(36);
        call_data.extend_from_slice(&sel);
        call_data.extend_from_slice(&nullifier);

        match self.eth_call(&call_data).await {
            Ok(bytes) => {
                if !bytes.is_empty() {
                    return Ok(bytes.last().copied().unwrap_or(0) != 0);
                }
            }
            Err(_) => {
                // Try camelCase selector: isNullifierSpent(bytes32)
                let alt_sel = Self::calculate_selector("isNullifierSpent(bytes32)");
                let mut alt_data = Vec::with_capacity(36);
                alt_data.extend_from_slice(&alt_sel);
                alt_data.extend_from_slice(&nullifier);

                let bytes = self.eth_call(&alt_data).await?;
                if !bytes.is_empty() {
                    return Ok(bytes.last().copied().unwrap_or(0) != 0);
                }
            }
        }
        Ok(false)
    }

    async fn verify_spend_receipt_recipient(
        &self,
        tx_hash: &str,
        _asset_id: u8,
        expected_recipient: &str,
    ) -> Result<bool, ComplianceError> {
        let payload = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_getTransactionReceipt",
            "params": [tx_hash],
            "id": 1
        });

        let resp = self
            .http_client
            .post(&self.rpc_url)
            .json(&payload)
            .send()
            .await
            .map_err(|e| ComplianceError::RpcError(format!("Receipt fetch error: {e}")))?;

        if !resp.status().is_success() {
            return Err(ComplianceError::RpcError(format!(
                "Receipt HTTP error: {}",
                resp.status()
            )));
        }

        let body: Value = resp.json().await.map_err(|e| {
            ComplianceError::RpcError(format!("Failed to parse receipt response: {e}"))
        })?;

        let receipt = body
            .get("result")
            .ok_or_else(|| ComplianceError::RpcError("Receipt result not found".into()))?;

        if receipt.is_null() {
            return Ok(false);
        }

        // Verify status == 0x1
        let status = receipt
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("0x0");
        if status != "0x1" {
            return Ok(false);
        }

        // Check if recipient address appears in receipt logs (Transfer event or PrivateNoteSpend)
        let clean_expected = expected_recipient.trim_start_matches("0x").to_lowercase();
        if let Some(logs) = receipt.get("logs").and_then(|v| v.as_array()) {
            for log in logs {
                if let Some(data) = log.get("data").and_then(|v| v.as_str()) {
                    if data.to_lowercase().contains(&clean_expected) {
                        return Ok(true);
                    }
                }
                if let Some(topics) = log.get("topics").and_then(|v| v.as_array()) {
                    for topic in topics {
                        if let Some(t_str) = topic.as_str() {
                            if t_str.to_lowercase().contains(&clean_expected) {
                                return Ok(true);
                            }
                        }
                    }
                }
            }
        }

        // If logs inspection didn't find recipient, also check if tx was sent directly to recipient
        if let Some(to) = receipt.get("to").and_then(|v| v.as_str()) {
            if to
                .trim_start_matches("0x")
                .eq_ignore_ascii_case(&clean_expected)
            {
                return Ok(true);
            }
        }

        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compliance::models::VerifierBinding;
    use crate::wallet::note_wallet::{NoteStatus, PrivateNoteWallet, WalletNote};
    use ark_ff::UniformRand;
    use rand::rngs::OsRng;
    use zeltra_core::{fr_to_be_bytes, note_commitment};

    fn setup_test_payload() -> (CexProofPayload, MockBlockchainRpcClient) {
        let seed = [0x88u8; 32];
        let mut wallet = PrivateNoteWallet::new(&seed);
        let sk = wallet.spending_key().unwrap();

        let mut rng = OsRng;
        let rho = Fr::rand(&mut rng);
        let r = Fr::rand(&mut rng);
        let val = 25_000_000_000u64; // 25,000 USDC
        let cm = note_commitment(val, 1, sk, rho, r);
        let cm_bytes = fr_to_be_bytes(&cm);
        let cm_hex = format!("0x{}", hex::encode(cm_bytes));

        wallet.mmr.append(cm);

        wallet.notes.insert(
            cm_hex.clone(),
            WalletNote {
                commitment_hex: cm_hex.clone(),
                value: val,
                asset_id: 1,
                owner_key_hex: hex::encode(fr_to_be_bytes(&sk)),
                rho_hex: hex::encode(fr_to_be_bytes(&rho)),
                randomness_hex: hex::encode(fr_to_be_bytes(&r)),
                leaf_index: Some(0),
                leaf_count: Some(1),
                epoch_id: Some(0),
                merkle_path_hex: Some(vec!["00".repeat(32)]),
                status: NoteStatus::Unspent,
                created_at_secs: 1773619200,
                session_id: None,
            },
        );

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let binding = VerifierBinding {
            verifier_id: "BINANCE_KYC_DESK".into(),
            account_uid: "USER-98124712".into(),
            nonce: "a7f3e829bc14d0219901ab23456789cd".into(),
            expires_at: now + 86400,
            spent_tx_hash: "0x44ca000000000000000000000000000000000000000000000000000000000990"
                .into(),
            payout_recipient: "0xBinanceUserDepositAddress0000000000000000".into(),
        };

        let payload = wallet
            .create_cex_proof(cm_bytes, binding)
            .expect("CEX proof creation");

        let mut rpc = MockBlockchainRpcClient::new();
        let root_bytes = parse_bytes32(&payload.mmr_proof.mmr_root).unwrap();
        let nf_bytes = parse_bytes32(&payload.note_cryptography.nullifier).unwrap();

        rpc.add_accepted_root(root_bytes);
        rpc.mark_nullifier_spent(nf_bytes);
        rpc.register_spend_receipt(
            &payload.note_cryptography.spent_tx_hash,
            1,
            &payload.note_cryptography.payout_recipient,
        );

        (payload, rpc)
    }

    #[tokio::test]
    async fn test_cex_verifier_full_flow_success() {
        let (payload, rpc) = setup_test_payload();
        let summary = CexProofVerifier::verify(
            &payload,
            "BINANCE_KYC_DESK",
            payload.generated_at + 100,
            &rpc,
        )
        .await
        .expect("Verification must succeed");

        assert_eq!(summary.status, "SUCCESS");
        assert_eq!(summary.asset_symbol, "USDC");
        assert_eq!(summary.amount_human, "25000.000000");
        assert_eq!(summary.account_uid, "USER-98124712");
    }

    #[tokio::test]
    async fn test_cex_verifier_rejects_wrong_verifier_id() {
        let (payload, rpc) = setup_test_payload();
        let err = CexProofVerifier::verify(
            &payload,
            "COINBASE_COMPLIANCE",
            payload.generated_at + 100,
            &rpc,
        )
        .await
        .expect_err("Must reject wrong verifier ID");

        match err {
            ComplianceError::VerifierIdMismatch { expected, actual } => {
                assert_eq!(expected, "COINBASE_COMPLIANCE");
                assert_eq!(actual, "BINANCE_KYC_DESK");
            }
            _ => panic!("Expected VerifierIdMismatch, got {:?}", err),
        }
    }

    #[tokio::test]
    async fn test_cex_verifier_rejects_expired_proof() {
        let (payload, rpc) = setup_test_payload();
        let err = CexProofVerifier::verify(
            &payload,
            "BINANCE_KYC_DESK",
            payload.verifier_binding.expires_at + 1,
            &rpc,
        )
        .await
        .expect_err("Must reject expired proof");

        assert_eq!(err, ComplianceError::ProofExpired);
    }

    #[tokio::test]
    async fn test_cex_verifier_rejects_tampered_amount() {
        let (mut payload, rpc) = setup_test_payload();
        payload.asset.raw_circuit_value = "999999999999".to_string();

        let err = CexProofVerifier::verify(
            &payload,
            "BINANCE_KYC_DESK",
            payload.generated_at + 100,
            &rpc,
        )
        .await
        .expect_err("Must reject tampered amount");

        assert_eq!(err, ComplianceError::CommitmentPreimageMismatch);
    }

    #[tokio::test]
    async fn test_cex_verifier_rejects_unaccepted_mmr_root() {
        let (payload, mut rpc) = setup_test_payload();
        rpc.accepted_roots.clear(); // Root not on chain!

        let err = CexProofVerifier::verify(
            &payload,
            "BINANCE_KYC_DESK",
            payload.generated_at + 100,
            &rpc,
        )
        .await
        .expect_err("Must reject unaccepted MMR root");

        assert_eq!(err, ComplianceError::MmrRootNotAcceptedOnChain);
    }

    #[tokio::test]
    async fn test_cex_verifier_rejects_unspent_nullifier() {
        let (payload, mut rpc) = setup_test_payload();
        rpc.spent_nullifiers.clear(); // Nullifier not spent!

        let err = CexProofVerifier::verify(
            &payload,
            "BINANCE_KYC_DESK",
            payload.generated_at + 100,
            &rpc,
        )
        .await
        .expect_err("Must reject unspent nullifier");

        assert_eq!(err, ComplianceError::NullifierNotSpentOnChain);
    }

    #[tokio::test]
    async fn test_cex_verifier_fail_closed_on_rpc_error() {
        let (payload, mut rpc) = setup_test_payload();
        rpc.set_fail_rpc(true);

        let err = CexProofVerifier::verify(
            &payload,
            "BINANCE_KYC_DESK",
            payload.generated_at + 100,
            &rpc,
        )
        .await
        .expect_err("Must fail closed on RPC error");

        match err {
            ComplianceError::RpcError(_) => {}
            _ => panic!("Expected RpcError, got {:?}", err),
        }
    }
}
