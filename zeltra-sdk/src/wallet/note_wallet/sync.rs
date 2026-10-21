use ark_bls12_381::Fr;
use zeltra_core::{
    decrypt_committing_note_memo, decrypt_committing_note_memo_payload,
    decrypt_note_memo_deterministic, decrypt_note_memo_payload, derive_nullifier, derive_owner_key,
    fr_to_be_bytes, note_commitment, IncomingViewingKey,
};

use super::types::*;
use super::PrivateNoteWallet;

impl PrivateNoteWallet {
    /// Canonical big-endian deserialization of on-chain note plaintext (DEC-034C / Patch Audit #6).
    /// Format: Bytes 0..8: value, Bytes 8..40: rho EVM scalar, Bytes 40..48: asset_id.
    pub fn process_decrypted_plaintext(
        &mut self,
        plaintext_bytes: &[u8; 48],
        shared_secret: &[u8; 32],
    ) -> Result<(), WalletError> {
        let value = u64::from_be_bytes(plaintext_bytes[0..8].try_into().unwrap());
        let mut rho_bytes = [0u8; 32];
        rho_bytes.copy_from_slice(&plaintext_bytes[8..40]);
        let rho =
            zeltra_core::from_evm_scalar(&rho_bytes).ok_or(WalletError::NonCanonicalRhoScalar)?;
        let asset_id_u64 = u64::from_be_bytes(plaintext_bytes[40..48].try_into().unwrap());

        if asset_id_u64 > 1 {
            return Err(WalletError::UnsupportedAssetId(asset_id_u64 as u8));
        }
        let asset_id = asset_id_u64 as u8;

        let randomness = derive_note_randomness(shared_secret, &rho);
        let owner_key = self.incoming_viewing_key()?.owner_key();
        let note = PrivateNoteV1 {
            value,
            asset_id,
            owner_key,
            rho,
            randomness,
        };

        if asset_id == 0 {
            self.weth_notes.push(note);
        } else {
            self.usdc_notes.push(note);
        }

        Ok(())
    }

    /// Recovers notes from on-chain NoteEvent logs, checking nullifiers on-chain (DEC-037).
    /// Recovers notes from on-chain NoteEvent logs using batch nullifier checks (DEC-037).
    ///
    /// Collects candidate notes whose trial decryption succeeds, groups their nullifiers,
    /// and invokes a SINGLE batch call to verify double-spend status without N+1 RPC queries.
    pub fn recover_notes_batch<F>(
        &mut self,
        events: &[NoteEvent],
        check_nullifiers_spent_batch: F,
    ) -> Result<RecoveryResult, WalletError>
    where
        F: FnOnce(&[String]) -> Result<Vec<bool>, WalletError>,
    {
        let ivk = self.incoming_viewing_key()?;
        let sk = self.spending_key()?;
        let nk = self.nullifier_key()?;

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
            asset_id: u64,
            rho: Fr,
            deterministic_r: Fr,
            owner_key: Fr,
        }

        let mut candidates = Vec::new();

        for ev in events {
            if ev.encrypted_memo.len() != 96 {
                continue;
            }

            // Dual-path decryption:
            // Step 1: Fast pre-filter with 1-byte View Tag (integrated into decrypt_committing_note_memo)
            // Step 2: Attempt DEC-048 committing memo decryption via decrypt_committing_note_memo (EtH-KS)
            // Step 3: If committing decryption returns None, fall back gracefully to legacy memo decryption (DEC-045C / DEC-037)
            let candidate_pair = decrypt_committing_note_memo(&ev.encrypted_memo, &ivk)
                .or_else(|| decrypt_note_memo_deterministic(&ev.encrypted_memo, &ivk));

            if let Some((pt, deterministic_r)) = candidate_pair {
                // Skip dummy zero-notes to keep active UTXO state clean (AUD-7)
                if pt.value == 0 {
                    continue;
                }

                let ev_cm_clean = ev
                    .commitment_hex
                    .strip_prefix("0x")
                    .unwrap_or(&ev.commitment_hex);

                // Reconstruct candidate cm
                let owner_zsa = derive_owner_key(sk);
                let cm_zsa = note_commitment(
                    pt.value,
                    pt.asset_id as u8,
                    owner_zsa,
                    pt.rho,
                    deterministic_r,
                );
                let cm_direct =
                    note_commitment(pt.value, pt.asset_id as u8, sk, pt.rho, deterministic_r);
                let r_legacy = zeltra_core::derive_note_randomness(sk, pt.rho);
                let cm_legacy = note_commitment(pt.value, pt.asset_id as u8, sk, pt.rho, r_legacy);
                let cm_legacy_zsa =
                    note_commitment(pt.value, pt.asset_id as u8, owner_zsa, pt.rho, r_legacy);

                let (matched_cm, matched_r, matched_owner) = if hex::encode(fr_to_be_bytes(&cm_zsa))
                    .eq_ignore_ascii_case(ev_cm_clean)
                {
                    (cm_zsa, deterministic_r, owner_zsa)
                } else if hex::encode(fr_to_be_bytes(&cm_direct)).eq_ignore_ascii_case(ev_cm_clean)
                {
                    (cm_direct, deterministic_r, sk)
                } else if hex::encode(fr_to_be_bytes(&cm_legacy)).eq_ignore_ascii_case(ev_cm_clean)
                {
                    (cm_legacy, r_legacy, sk)
                } else if hex::encode(fr_to_be_bytes(&cm_legacy_zsa))
                    .eq_ignore_ascii_case(ev_cm_clean)
                {
                    (cm_legacy_zsa, r_legacy, owner_zsa)
                } else {
                    continue;
                };

                let cm_hex = format!("0x{}", hex::encode(fr_to_be_bytes(&matched_cm)));
                let nullifier = derive_nullifier(nk, matched_cm, ev.leaf_index, 0);
                let nf_hex = format!("0x{}", hex::encode(fr_to_be_bytes(&nullifier)));

                candidates.push(Candidate {
                    cm_hex,
                    nf_hex,
                    leaf_index: ev.leaf_index,
                    leaf_count: ev.leaf_count,
                    value: pt.value,
                    asset_id: pt.asset_id,
                    rho: pt.rho,
                    deterministic_r: matched_r,
                    owner_key: matched_owner,
                });
            }
        }

        if candidates.is_empty() {
            return Ok(res);
        }

        let nfs: Vec<String> = candidates.iter().map(|c| c.nf_hex.clone()).collect();
        let spent_flags = check_nullifiers_spent_batch(&nfs)?;
        if spent_flags.len() != candidates.len() {
            return Err(WalletError::CryptoError(format!(
                "Batch nullifier spent count mismatch: expected {}, got {}",
                candidates.len(),
                spent_flags.len()
            )));
        }

        for (cand, &is_spent) in candidates.into_iter().zip(spent_flags.iter()) {
            if is_spent {
                res.total_spent_balance += cand.value;
            } else {
                res.total_unspent_balance += cand.value;
            }

            let recovered = RecoveredNote {
                commitment_hex: cand.cm_hex.clone(),
                nullifier_hex: cand.nf_hex.clone(),
                leaf_index: cand.leaf_index,
                value: cand.value,
                asset_id: cand.asset_id,
                rho_hex: format!("0x{}", hex::encode(fr_to_be_bytes(&cand.rho))),
                is_spent,
            };

            let wallet_note = WalletNote {
                commitment_hex: cand.cm_hex.clone(),
                value: cand.value,
                asset_id: cand.asset_id as u8,
                owner_key_hex: hex::encode(fr_to_be_bytes(&cand.owner_key)),
                rho_hex: hex::encode(fr_to_be_bytes(&cand.rho)),
                randomness_hex: hex::encode(fr_to_be_bytes(&cand.deterministic_r)),
                leaf_index: Some(cand.leaf_index),
                leaf_count: Some(cand.leaf_count),
                epoch_id: Some(0),
                merkle_path_hex: None,
                status: if is_spent {
                    NoteStatus::Spent {
                        nullifier_hex: cand.nf_hex.clone(),
                        spent_at_secs: 0,
                    }
                } else {
                    NoteStatus::Unspent
                },
                created_at_secs: 0,
                session_id: None,
            };

            self.notes.insert(cand.cm_hex, wallet_note);
            res.recovered_notes.push(recovered);
        }

        Ok(res)
    }

    /// Recovers notes from on-chain NoteEvent logs, checking nullifiers on-chain (DEC-037).
    pub fn recover_notes<F>(
        &mut self,
        events: &[NoteEvent],
        is_nullifier_spent_fn: F,
    ) -> Result<RecoveryResult, WalletError>
    where
        F: Fn(&str) -> bool,
    {
        self.recover_notes_batch(events, |nfs| {
            Ok(nfs.iter().map(|nf| is_nullifier_spent_fn(nf)).collect())
        })
    }

    /// Fast-scanning note scanner with 1-Byte View Tag acceleration (DEC-045C).
    ///
    /// Pre-filters incoming on-chain events by checking `memo[32] == derive_view_tag(&shared_secret)`.
    /// 99.6% of non-owned memos are dropped immediately without invoking ChaCha20Poly1305.
    /// Also supports transparent fallback for legacy DEC-037 memos.
    pub fn scan_memos_fast(
        &mut self,
        events: &[NoteEvent],
    ) -> Result<RecoveryResult, WalletError> {
        self.scan_memos_fast_with_checker(events, |nfs| Ok(vec![false; nfs.len()]))
    }

    /// Fast-scanning note scanner with 1-Byte View Tag acceleration and batch nullifier verification (DEC-045C).
    pub fn scan_memos_fast_with_checker<F>(
        &mut self,
        events: &[NoteEvent],
        check_nullifiers_spent_batch: F,
    ) -> Result<RecoveryResult, WalletError>
    where
        F: FnOnce(&[String]) -> Result<Vec<bool>, WalletError>,
    {
        let ivk = self.incoming_viewing_key()?;
        let sk = self.spending_key()?;
        let nk = self.nullifier_key()?;

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
            asset_id: u64,
            rho: Fr,
            deterministic_r: Fr,
            owner_key: Fr,
        }

        let mut candidates = Vec::new();

        for ev in events {
            if ev.encrypted_memo.len() != 96 {
                // Bounds validation: reject invalid length memos (INV-45C-1, C5.6)
                continue;
            }

            let memo = &ev.encrypted_memo;
            let eph_pk_bytes: [u8; 32] = memo[0..32].try_into().unwrap();
            let memo_view_tag = memo[32];

            // 1. Compute ECDH shared secret
            let shared_secret = ivk.diffie_hellman(&eph_pk_bytes);

            // 2. Expected view tag
            let expected_view_tag = zeltra_core::derive_view_tag(&shared_secret);

            let mut candidate_pair = None;

            // Step 1: Fast pre-filter with 1-byte View Tag (99.6% non-owned memos dropped with 0 AEAD invocations)
            if memo_view_tag == expected_view_tag {
                // Step 2: Attempt DEC-048 committing memo decryption via EtH-KS payload
                if let Some((pt, deterministic_r)) =
                    decrypt_committing_note_memo_payload(memo, &shared_secret)
                {
                    candidate_pair = Some((pt, deterministic_r));
                } else if let Some((pt, deterministic_r)) =
                    decrypt_note_memo_payload(memo, &shared_secret)
                {
                    // Step 3a: Fall back to DEC-045C fast payload decryption
                    candidate_pair = Some((pt, deterministic_r));
                }
            }

            // Step 3b: Fallback for historical DEC-037 notes or view tag collisions where fast payload did not match
            if candidate_pair.is_none() {
                if let Some((pt, deterministic_r)) =
                    decrypt_note_memo_deterministic(memo, &ivk)
                {
                    candidate_pair = Some((pt, deterministic_r));
                }
            }

            if let Some((pt, deterministic_r)) = candidate_pair {
                // Dummy Note Discarding: ignore zero-value change notes (C3.2, AUD-7)
                if pt.value == 0 {
                    continue;
                }

                let ev_cm_clean = ev
                    .commitment_hex
                    .strip_prefix("0x")
                    .unwrap_or(&ev.commitment_hex);

                // Canonical Commitment Verification (C3.3)
                let owner_zsa = derive_owner_key(sk);
                let cm_zsa = note_commitment(
                    pt.value,
                    pt.asset_id as u8,
                    owner_zsa,
                    pt.rho,
                    deterministic_r,
                );
                let cm_direct =
                    note_commitment(pt.value, pt.asset_id as u8, sk, pt.rho, deterministic_r);
                let r_legacy = zeltra_core::derive_note_randomness(sk, pt.rho);
                let cm_legacy = note_commitment(pt.value, pt.asset_id as u8, sk, pt.rho, r_legacy);
                let cm_legacy_zsa =
                    note_commitment(pt.value, pt.asset_id as u8, owner_zsa, pt.rho, r_legacy);

                let (matched_cm, matched_r, matched_owner) = if hex::encode(fr_to_be_bytes(&cm_zsa))
                    .eq_ignore_ascii_case(ev_cm_clean)
                {
                    (cm_zsa, deterministic_r, owner_zsa)
                } else if hex::encode(fr_to_be_bytes(&cm_direct)).eq_ignore_ascii_case(ev_cm_clean)
                {
                    (cm_direct, deterministic_r, sk)
                } else if hex::encode(fr_to_be_bytes(&cm_legacy)).eq_ignore_ascii_case(ev_cm_clean)
                {
                    (cm_legacy, r_legacy, sk)
                } else if hex::encode(fr_to_be_bytes(&cm_legacy_zsa))
                    .eq_ignore_ascii_case(ev_cm_clean)
                {
                    (cm_legacy_zsa, r_legacy, owner_zsa)
                } else {
                    continue;
                };

                let cm_hex = format!("0x{}", hex::encode(fr_to_be_bytes(&matched_cm)));
                let nullifier = derive_nullifier(nk, matched_cm, ev.leaf_index, 0);
                let nf_hex = format!("0x{}", hex::encode(fr_to_be_bytes(&nullifier)));

                candidates.push(Candidate {
                    cm_hex,
                    nf_hex,
                    leaf_index: ev.leaf_index,
                    leaf_count: ev.leaf_count,
                    value: pt.value,
                    asset_id: pt.asset_id,
                    rho: pt.rho,
                    deterministic_r: matched_r,
                    owner_key: matched_owner,
                });
            }
        }

        if candidates.is_empty() {
            return Ok(res);
        }

        let nfs: Vec<String> = candidates.iter().map(|c| c.nf_hex.clone()).collect();
        let spent_flags = check_nullifiers_spent_batch(&nfs)?;
        if spent_flags.len() != candidates.len() {
            return Err(WalletError::CryptoError(format!(
                "Batch nullifier spent count mismatch: expected {}, got {}",
                candidates.len(),
                spent_flags.len()
            )));
        }

        for (cand, &is_spent) in candidates.into_iter().zip(spent_flags.iter()) {
            if is_spent {
                res.total_spent_balance += cand.value;
            } else {
                res.total_unspent_balance += cand.value;
            }

            let recovered = RecoveredNote {
                commitment_hex: cand.cm_hex.clone(),
                nullifier_hex: cand.nf_hex.clone(),
                leaf_index: cand.leaf_index,
                value: cand.value,
                asset_id: cand.asset_id,
                rho_hex: format!("0x{}", hex::encode(fr_to_be_bytes(&cand.rho))),
                is_spent,
            };

            let wallet_note = WalletNote {
                commitment_hex: cand.cm_hex.clone(),
                value: cand.value,
                asset_id: cand.asset_id as u8,
                owner_key_hex: hex::encode(fr_to_be_bytes(&cand.owner_key)),
                rho_hex: hex::encode(fr_to_be_bytes(&cand.rho)),
                randomness_hex: hex::encode(fr_to_be_bytes(&cand.deterministic_r)),
                leaf_index: Some(cand.leaf_index),
                leaf_count: Some(cand.leaf_count),
                epoch_id: Some(0),
                merkle_path_hex: None,
                status: if is_spent {
                    NoteStatus::Spent {
                        nullifier_hex: cand.nf_hex.clone(),
                        spent_at_secs: 0,
                    }
                } else {
                    NoteStatus::Unspent
                },
                created_at_secs: 0,
                session_id: None,
            };

            self.notes.insert(cand.cm_hex, wallet_note);
            res.recovered_notes.push(recovered);
        }

        Ok(res)
    }

    /// Bulk syncs notes directly from Relayer bulk sync API in chunks of 2,000 notes (DEC-037).
    ///
    /// Avoids slow raw `eth_getLogs` by leveraging `GET /api/v1/notes/sync?from_leaf={}&limit={}`
    /// and performs single-round batch double-spend verification for discovered notes.
    pub async fn sync_from_indexer<F>(
        &mut self,
        node_url: &str,
        ivk: &IncomingViewingKey,
        start_leaf: u64,
        check_nullifiers_batch_fn: F,
    ) -> Result<RecoveryResult, WalletError>
    where
        F: Fn(&[String]) -> Result<Vec<bool>, WalletError>,
    {
        let client = crate::client::ZeltraClient::new(node_url);
        let chunk_size = 2000u64;
        let mut current_leaf = start_leaf;
        let mut aggregated_result = RecoveryResult::default();

        loop {
            let items = client.fetch_notes_sync(current_leaf, chunk_size).await?;
            if items.is_empty() {
                break;
            }

            let fetched_count = items.len() as u64;
            let max_leaf = items
                .iter()
                .map(|it| it.leaf_index)
                .max()
                .unwrap_or(current_leaf);

            let mut events = Vec::with_capacity(items.len());
            for it in items {
                let memo_bytes = if it.encrypted_memo.is_empty() {
                    Vec::new()
                } else {
                    let clean = it
                        .encrypted_memo
                        .strip_prefix("0x")
                        .unwrap_or(&it.encrypted_memo);
                    hex::decode(clean).unwrap_or_default()
                };
                events.push(NoteEvent {
                    leaf_index: it.leaf_index,
                    commitment_hex: it.commitment,
                    new_mmr_root_hex: String::new(),
                    leaf_count: it.leaf_index + 1,
                    encrypted_memo: memo_bytes,
                    block_number: Some(it.block_number),
                });
            }

            // Trial decrypt against passed IVK and batch check discovered candidates
            let sk = self.spending_key()?;
            let nk = self.nullifier_key()?;

            struct ChunkCandidate {
                cm_hex: String,
                nf_hex: String,
                leaf_index: u64,
                leaf_count: u64,
                value: u64,
                asset_id: u64,
                rho: Fr,
                deterministic_r: Fr,
            }

            let mut chunk_candidates = Vec::new();
            for ev in &events {
                if ev.encrypted_memo.len() != 96 {
                    continue;
                }
                let candidate_pair = decrypt_committing_note_memo(&ev.encrypted_memo, ivk)
                    .or_else(|| decrypt_note_memo_deterministic(&ev.encrypted_memo, ivk));

                if let Some((pt, deterministic_r)) = candidate_pair {
                    if pt.value == 0 {
                        continue;
                    }
                    let ev_cm_clean = ev
                        .commitment_hex
                        .strip_prefix("0x")
                        .unwrap_or(&ev.commitment_hex);

                    let owner_zsa = derive_owner_key(sk);
                    let cm_zsa = note_commitment(
                        pt.value,
                        pt.asset_id as u8,
                        owner_zsa,
                        pt.rho,
                        deterministic_r,
                    );
                    let cm_direct =
                        note_commitment(pt.value, pt.asset_id as u8, sk, pt.rho, deterministic_r);
                    let r_legacy = zeltra_core::derive_note_randomness(sk, pt.rho);
                    let cm_legacy = note_commitment(pt.value, pt.asset_id as u8, sk, pt.rho, r_legacy);
                    let cm_legacy_zsa =
                        note_commitment(pt.value, pt.asset_id as u8, owner_zsa, pt.rho, r_legacy);

                    let (matched_cm, matched_r) = if hex::encode(fr_to_be_bytes(&cm_zsa))
                        .eq_ignore_ascii_case(ev_cm_clean)
                    {
                        (cm_zsa, deterministic_r)
                    } else if hex::encode(fr_to_be_bytes(&cm_direct)).eq_ignore_ascii_case(ev_cm_clean)
                    {
                        (cm_direct, deterministic_r)
                    } else if hex::encode(fr_to_be_bytes(&cm_legacy)).eq_ignore_ascii_case(ev_cm_clean)
                    {
                        (cm_legacy, r_legacy)
                    } else if hex::encode(fr_to_be_bytes(&cm_legacy_zsa))
                        .eq_ignore_ascii_case(ev_cm_clean)
                    {
                        (cm_legacy_zsa, r_legacy)
                    } else {
                        continue;
                    };

                    let cm_hex = format!("0x{}", hex::encode(fr_to_be_bytes(&matched_cm)));
                    let nullifier = derive_nullifier(nk, matched_cm, ev.leaf_index, 0);
                    let nf_hex = format!("0x{}", hex::encode(fr_to_be_bytes(&nullifier)));

                    chunk_candidates.push(ChunkCandidate {
                        cm_hex,
                        nf_hex,
                        leaf_index: ev.leaf_index,
                        leaf_count: ev.leaf_count,
                        value: pt.value,
                        asset_id: pt.asset_id,
                        rho: pt.rho,
                        deterministic_r: matched_r,
                    });
                }
            }

            aggregated_result.scanned_events += events.len();

            if !chunk_candidates.is_empty() {
                let nfs: Vec<String> = chunk_candidates.iter().map(|c| c.nf_hex.clone()).collect();
                let spent_flags = check_nullifiers_batch_fn(&nfs)?;
                if spent_flags.len() != chunk_candidates.len() {
                    return Err(WalletError::CryptoError(format!(
                        "Batch nullifier spent count mismatch: expected {}, got {}",
                        chunk_candidates.len(),
                        spent_flags.len()
                    )));
                }

                for (cand, &is_spent) in chunk_candidates.into_iter().zip(spent_flags.iter()) {
                    if is_spent {
                        aggregated_result.total_spent_balance += cand.value;
                    } else {
                        aggregated_result.total_unspent_balance += cand.value;
                    }

                    let recovered = RecoveredNote {
                        commitment_hex: cand.cm_hex.clone(),
                        nullifier_hex: cand.nf_hex.clone(),
                        leaf_index: cand.leaf_index,
                        value: cand.value,
                        asset_id: cand.asset_id,
                        rho_hex: format!("0x{}", hex::encode(fr_to_be_bytes(&cand.rho))),
                        is_spent,
                    };

                    let wallet_note = WalletNote {
                        commitment_hex: cand.cm_hex.clone(),
                        value: cand.value,
                        asset_id: cand.asset_id as u8,
                        owner_key_hex: hex::encode(fr_to_be_bytes(&sk)),
                        rho_hex: hex::encode(fr_to_be_bytes(&cand.rho)),
                        randomness_hex: hex::encode(fr_to_be_bytes(&cand.deterministic_r)),
                        leaf_index: Some(cand.leaf_index),
                        leaf_count: Some(cand.leaf_count),
                        epoch_id: Some(0),
                        merkle_path_hex: None,
                        status: if is_spent {
                            NoteStatus::Spent {
                                nullifier_hex: cand.nf_hex.clone(),
                                spent_at_secs: 0,
                            }
                        } else {
                            NoteStatus::Unspent
                        },
                        created_at_secs: 0,
                        session_id: None,
                    };

                    self.notes.insert(cand.cm_hex, wallet_note);
                    aggregated_result.recovered_notes.push(recovered);
                }
            }

            if fetched_count < chunk_size {
                break;
            }
            current_leaf = max_leaf + 1;
        }

        Ok(aggregated_result)
    }

    pub fn recover_note_manual(&mut self, value: u64, rho: Fr) -> Result<WalletNote, WalletError> {
        let sk = self.spending_key()?;
        let r = zeltra_core::derive_note_randomness(sk, rho);
        let cm = note_commitment(value, 1 /* USDC */, sk, rho, r);
        let cm_raw_hex = hex::encode(fr_to_be_bytes(&cm));
        let cm_hex = format!("0x{}", cm_raw_hex);

        let note = WalletNote {
            commitment_hex: cm_hex.clone(),
            value,
            asset_id: 1,
            owner_key_hex: hex::encode(fr_to_be_bytes(&sk)),
            rho_hex: hex::encode(fr_to_be_bytes(&rho)),
            randomness_hex: hex::encode(fr_to_be_bytes(&r)),
            leaf_index: None,
            leaf_count: None,
            epoch_id: Some(0),
            merkle_path_hex: None,
            status: NoteStatus::Unconfirmed,
            created_at_secs: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            session_id: None,
        };

        self.notes.insert(cm_hex, note.clone());
        Ok(note)
    }

    /// Deserializes sibling hex strings from MmrProofResponse into Fr scalar arrays (C4.2).
    pub fn deserialize_mmr_siblings(
        proof_data: &MmrProofResponse,
    ) -> Result<([Fr; 32], [Fr; 32]), WalletError> {
        if proof_data.mountain_siblings.len() > 32 {
            return Err(WalletError::CryptoError(format!(
                "mountain_siblings length {} exceeds max 32",
                proof_data.mountain_siblings.len()
            )));
        }
        if proof_data.peak_bagging_siblings.len() > 32 {
            return Err(WalletError::CryptoError(format!(
                "peak_bagging_siblings length {} exceeds max 32",
                proof_data.peak_bagging_siblings.len()
            )));
        }

        let mut mountain_siblings = [Fr::from(0u64); 32];
        for (i, hex_str) in proof_data.mountain_siblings.iter().enumerate() {
            mountain_siblings[i] = parse_fr_from_hex(hex_str)?;
        }

        let mut peak_bagging_siblings = [Fr::from(0u64); 32];
        for (i, hex_str) in proof_data.peak_bagging_siblings.iter().enumerate() {
            peak_bagging_siblings[i] = parse_fr_from_hex(hex_str)?;
        }

        Ok((mountain_siblings, peak_bagging_siblings))
    }

    /// Caches an MMR inclusion proof locally (C4.4).
    pub fn cache_mmr_proof(&mut self, proof: MmrProofResponse) {
        self.mmr_proof_cache
            .insert((proof.leaf_index, proof.bagged_root.clone()), proof);
    }

    /// Retrieves a cached MMR inclusion proof matching leaf_index and bagged_root (C4.4).
    pub fn get_cached_mmr_proof(
        &self,
        leaf_index: u64,
        bagged_root: &str,
    ) -> Option<&MmrProofResponse> {
        let clean = bagged_root.strip_prefix("0x").unwrap_or(bagged_root);
        self.mmr_proof_cache
            .iter()
            .find(|((idx, root), _)| {
                *idx == leaf_index
                    && (root
                        .strip_prefix("0x")
                        .unwrap_or(root)
                        .eq_ignore_ascii_case(clean))
            })
            .map(|(_, v)| v)
    }

    /// Retrieves any cached MMR inclusion proof for the specified leaf_index (most recent or available).
    pub fn get_any_cached_mmr_proof_for_leaf(&self, leaf_index: u64) -> Option<&MmrProofResponse> {
        self.mmr_proof_cache
            .iter()
            .find(|((idx, _), _)| *idx == leaf_index)
            .map(|(_, v)| v)
    }

    /// Clears the local MMR proof cache.
    pub fn clear_mmr_proof_cache(&mut self) {
        self.mmr_proof_cache.clear();
    }

    /// Appends a contiguous batch of leaf commitments (hex-encoded) to the client's local MMR (B3.2).
    /// If `expected_root` is provided, asserts that the local bagged root matches the expected root (B3.3).
    pub fn append_bulk_leaves(
        &mut self,
        leaves_hex: &[String],
        expected_root: Option<&str>,
    ) -> Result<u64, WalletError> {
        for leaf_hex in leaves_hex {
            let cm_fr = parse_fr_from_hex(leaf_hex)?;
            self.mmr.append(cm_fr);
        }
        let local_root = self.mmr.get_root();
        let local_root_hex = hex::encode(fr_to_be_bytes(&local_root));

        if let Some(exp_root) = expected_root {
            let clean_exp = exp_root.strip_prefix("0x").unwrap_or(exp_root);
            if !local_root_hex.eq_ignore_ascii_case(clean_exp) {
                return Err(WalletError::CryptoError(format!(
                    "Bagged root assertion failed: local 0x{local_root_hex} != expected 0x{clean_exp}"
                )));
            }
        }

        self.current_merkle_root_hex = Some(format!("0x{local_root_hex}"));
        Ok(self.mmr.leaf_count as u64)
    }

    /// Anti-Snooping Paginated Bulk MMR Tree Sync (DEC-036B B3.1 / B3.2 / B3.3 / ZIP 314 Defense).
    ///
    /// Downloads leaf commitments in uniform chunks of 1,000 leaves and builds the MMR
    /// entirely in local CPU/WASM memory. Never transmits individual leaf indices.
    pub async fn sync_mmr_bulk(
        &mut self,
        client: &crate::client::ZeltraClient,
        expected_root: Option<&str>,
    ) -> Result<u64, WalletError> {
        let chunk_size = crate::client::BULK_LEAVES_CHUNK_SIZE;
        let mut from_index = self.mmr.leaf_count as u64;

        loop {
            let leaves = client.fetch_mmr_leaves(from_index, chunk_size).await?;
            if leaves.is_empty() {
                break;
            }
            let count = leaves.len() as u64;
            self.append_bulk_leaves(&leaves, None)?;
            from_index += count;
            if count < chunk_size {
                break;
            }
        }

        if let Some(exp_root) = expected_root {
            let local_root = hex::encode(fr_to_be_bytes(&self.mmr.get_root()));
            let clean_exp = exp_root.strip_prefix("0x").unwrap_or(exp_root);
            if !local_root.eq_ignore_ascii_case(clean_exp) {
                return Err(WalletError::CryptoError(format!(
                    "MMR bulk sync root mismatch: local 0x{local_root} != expected 0x{clean_exp}"
                )));
            }
        }

        Ok(self.mmr.leaf_count as u64)
    }
}
