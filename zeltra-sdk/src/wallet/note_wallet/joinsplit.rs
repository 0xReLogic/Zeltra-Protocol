use ark_bls12_381::{Bls12_381, Fr};
use ark_ff::{PrimeField, UniformRand};
use rand::rngs::OsRng;
use zeltra_core::{
    derive_joinsplit_dummy_nullifier, derive_joinsplit_nullifier, derive_nullifier_key,
    encrypt_committing_note_memo, extract_joinsplit_public_inputs, fr_to_be_bytes,
    generate_joinsplit_proof, note_commitment, split_quote_hash_to_limbs, to_evm_g1, to_evm_g2,
    JoinSplitCircuit, ZeltraShieldedAddress, CANONICAL_DUMMY_LEAF_INDEX, CANONICAL_DUMMY_OWNER,
    CANONICAL_DUMMY_RANDOMNESS, CANONICAL_DUMMY_RHO, CANONICAL_DUMMY_VALUE,
};

use super::types::*;
use super::PrivateNoteWallet;

impl PrivateNoteWallet {
    /// Pre-flight gas fee and round estimate for In-Pool Progressive Consolidation (DEC-036B B2.4).
    pub fn estimate_consolidation(
        &self,
        target_amount: Option<u64>,
        execution_fee_per_round: u64,
        current_time_secs: u64,
        current_epoch: u32,
    ) -> Result<ConsolidationEstimate, WalletError> {
        let spendable: Vec<&WalletNote> = self
            .notes
            .values()
            .filter(|n| {
                if n.check_epoch_status(current_epoch) == NoteEpochStatus::ExpiredRequiresRollover {
                    return false;
                }
                match &n.status {
                    NoteStatus::Unspent => true,
                    NoteStatus::Reserved {
                        lease_expiry_secs, ..
                    } => *lease_expiry_secs <= current_time_secs,
                    _ => false,
                }
            })
            .collect();

        let initial_note_count = spendable.len();
        if initial_note_count <= 2 {
            let total_avail: u64 = spendable.iter().map(|n| n.value).sum();
            return Ok(ConsolidationEstimate {
                rounds_needed: 0,
                total_estimated_gas_fees: 0,
                initial_note_count,
                estimated_final_note_count: initial_note_count,
                can_satisfy_target: target_amount.is_none_or(|t| total_avail >= t),
            });
        }

        // Each round merges 2 notes into 1, reducing total count by 1.
        // N notes down to 2 notes takes N - 2 rounds.
        let rounds_needed = initial_note_count.saturating_sub(2);
        let total_estimated_gas_fees = (rounds_needed as u64) * execution_fee_per_round;
        let total_value: u64 = spendable.iter().map(|n| n.value).sum();
        let remaining_value = total_value.saturating_sub(total_estimated_gas_fees);
        let can_satisfy_target = target_amount.is_none_or(|t| remaining_value >= t);

        Ok(ConsolidationEstimate {
            rounds_needed,
            total_estimated_gas_fees,
            initial_note_count,
            estimated_final_note_count: 2,
            can_satisfy_target,
        })
    }

    /// Executes In-Pool Progressive Consolidation loop (DEC-036B Phase 2).
    ///
    /// Consolidates fragmented wallets (N > 2 notes) down to <= 2 notes via iterative
    /// self-spend zero-payout JoinSplit proofs (v_merchant = 0, v_protocol_fee = 0, recipient = contract_address).
    #[allow(clippy::too_many_arguments)]
    pub fn consolidate_notes(
        &mut self,
        target_amount: Option<u64>,
        current_time_secs: u64,
        current_epoch: u32,
        execution_fee_per_round: u64,
        contract_address_hex: &str,
        chain_id: u64,
        joinsplit_pk: &ark_groth16::ProvingKey<Bls12_381>,
    ) -> Result<ConsolidationSummary, WalletError> {
        let initial_count = self
            .notes
            .values()
            .filter(|n| n.status == NoteStatus::Unspent)
            .count();

        let mut rounds_executed = 0;
        let mut total_fees_paid = 0u64;
        let mut total_consolidated_val = 0u64;
        let mut round_results = Vec::new();

        loop {
            // Find active unspent notes
            let mut spendable: Vec<WalletNote> = self
                .notes
                .values()
                .filter(|n| {
                    if n.check_epoch_status(current_epoch)
                        == NoteEpochStatus::ExpiredRequiresRollover
                    {
                        return false;
                    }
                    n.status == NoteStatus::Unspent
                })
                .cloned()
                .collect();

            // Stop condition: note count <= 2
            if spendable.len() <= 2 {
                if let Some(target) = target_amount {
                    let total_avail: u64 = spendable.iter().map(|n| n.value).sum();
                    if total_avail >= target {
                        break;
                    }
                } else {
                    break;
                }
            }

            // Pick 2 smallest notes to consolidate (B2.2)
            spendable.sort_by_key(|n| n.value);
            let note_1 = &spendable[0];
            let note_2 = &spendable[1];

            let sum_in = note_1
                .value
                .checked_add(note_2.value)
                .ok_or_else(|| WalletError::CryptoError("Consolidation sum overflow".into()))?;

            if sum_in <= execution_fee_per_round {
                return Err(WalletError::CryptoError(format!(
                    "Note values ({sum_in}) cannot cover relayer execution fee ({execution_fee_per_round})"
                )));
            }

            let consolidated_out = sum_in - execution_fee_per_round;

            let selected = SelectedSpend {
                input_commitment_hex: note_1.commitment_hex.clone(),
                input_value: note_1.value,
                second_input_commitment_hex: Some(note_2.commitment_hex.clone()),
                second_input_value: note_2.value,
                merchant_amount: 0,
                protocol_fee: 0,
                execution_fee: execution_fee_per_round,
                total_required: sum_in,
                change_amount: consolidated_out,
                has_change: true,
            };

            let session_id = format!("consolidate_{}_{}", rounds_executed, current_time_secs);
            let quote_hash = "0x0000000000000000000000000000000000000000000000000000000000000000";
            let expiry = current_time_secs + 3600;

            let payload = self.prepare_joinsplit_spend_proof(
                &selected,
                &session_id,
                contract_address_hex,
                execution_fee_per_round,
                quote_hash,
                chain_id,
                contract_address_hex,
                expiry,
                current_time_secs,
                joinsplit_pk,
            )?;

            // Automatically finalize this consolidation round in the local wallet state:
            // Inputs marked Spent, change note becomes Unspent and added to local MMR
            self.commit_joinsplit_spend(
                &session_id,
                &payload.input_nullifier_1_hex,
                Some(&payload.input_nullifier_2_hex),
                current_time_secs,
                None,
                None,
                None,
            )?;

            // Append change note commitment to local MMR so it has valid witness for next round
            let out_cm = &payload.output_commitment_1_hex;
            let out_cm_fr = parse_fr_from_hex(out_cm)?;
            let (new_idx, new_root) = self.mmr.append(out_cm_fr);
            if let Some(ch_note) = self.notes.get_mut(out_cm) {
                ch_note.leaf_index = Some(new_idx as u64);
                ch_note.leaf_count = Some(self.mmr.leaf_count as u64);
                ch_note.merkle_path_hex = Some(vec![]);
                ch_note.status = NoteStatus::Unspent;
            }
            self.current_merkle_root_hex =
                Some(format!("0x{}", hex::encode(fr_to_be_bytes(&new_root))));

            rounds_executed += 1;
            total_fees_paid += execution_fee_per_round;
            total_consolidated_val = consolidated_out;

            round_results.push(ConsolidationRoundResult {
                round_index: rounds_executed,
                session_id,
                input_commitment_1: note_1.commitment_hex.clone(),
                input_commitment_2: note_2.commitment_hex.clone(),
                consolidated_output_commitment: out_cm.clone(),
                input_1_value: note_1.value,
                input_2_value: note_2.value,
                output_value: consolidated_out,
                execution_fee_paid: execution_fee_per_round,
                payload,
            });
        }

        let final_count = self
            .notes
            .values()
            .filter(|n| n.status == NoteStatus::Unspent)
            .count();

        Ok(ConsolidationSummary {
            rounds_executed,
            initial_note_count: initial_count,
            final_note_count: final_count,
            total_relayer_fees_paid: total_fees_paid,
            total_value_consolidated: total_consolidated_val,
            rounds: round_results,
        })
    }

    /// Prepares a Universal 2-in-2-out JoinSplit Groth16 proof locally using `zeltra-core` JoinSplitCircuit (DEC-030).
    ///
    /// Supports both 1-note spend (using Canonical Dummy Zero-Note as input 2) and 2-note JoinSplit.
    /// NEVER transmits the private spending key or note witnesses to any relayer.
    #[allow(clippy::too_many_arguments)]
    pub fn prepare_joinsplit_spend_proof(
        &mut self,
        selected: &SelectedSpend,
        session_id: &str,
        recipient_evm_address: &str,
        max_execution_fee: u64,
        quote_hash_hex: &str,
        chain_id: u64,
        contract_address_hex: &str,
        expiry: u64,
        current_time_secs: u64,
        joinsplit_pk: &ark_groth16::ProvingKey<Bls12_381>,
    ) -> Result<JoinSplitSpendProofPayload, WalletError> {
        // 1. Reserve Note 1
        self.reserve_note(
            &selected.input_commitment_hex,
            session_id,
            current_time_secs,
            DEFAULT_RESERVATION_TTL_SECS,
        )?;

        // 2. Reserve Note 2 if 2-note JoinSplit
        if let Some(ref cm2_hex) = selected.second_input_commitment_hex {
            if let Err(e) = self.reserve_note(
                cm2_hex,
                session_id,
                current_time_secs,
                DEFAULT_RESERVATION_TTL_SECS,
            ) {
                // Rollback note 1 on failure
                let _ = self.rollback_spend(session_id);
                return Err(e);
            }
        }

        // 3. Load witnesses for Input Note 1
        let input_note_1 = self
            .notes
            .get(&selected.input_commitment_hex)
            .ok_or_else(|| WalletError::NoteNotFound(selected.input_commitment_hex.clone()))?
            .clone();

        let leaf_index_1 = input_note_1
            .leaf_index
            .ok_or_else(|| WalletError::NoteMissingWitness("input 1 leaf_index".into()))?;

        let sk = self.spending_key()?;
        let in1_rho = parse_fr_from_hex(&input_note_1.rho_hex)?;
        let in1_rand = parse_fr_from_hex(&input_note_1.randomness_hex)?;
        let in1_cm = note_commitment(
            input_note_1.value,
            input_note_1.asset_id,
            sk,
            in1_rho,
            in1_rand,
        );

        // Pre-fetch Input Note 2 if present
        let input_note_2_opt = if let Some(ref cm2_hex) = selected.second_input_commitment_hex {
            Some(
                self.notes
                    .get(cm2_hex)
                    .ok_or_else(|| WalletError::NoteNotFound(cm2_hex.clone()))?
                    .clone(),
            )
        } else {
            None
        };

        // Ensure Note 1 is in local MMR
        if self.mmr.leaf_count <= leaf_index_1 as usize {
            while self.mmr.leaf_count < leaf_index_1 as usize {
                self.mmr.append(Fr::from(0u64));
            }
            self.mmr.append(in1_cm);
        }

        // If Note 2 is real, ensure Note 2 is also in local MMR before generating proofs
        if let Some(ref input_note_2) = input_note_2_opt {
            let leaf_index_2 = input_note_2
                .leaf_index
                .ok_or_else(|| WalletError::NoteMissingWitness("input 2 leaf_index".into()))?;
            let rho_2 = parse_fr_from_hex(&input_note_2.rho_hex)?;
            let rand_2 = parse_fr_from_hex(&input_note_2.randomness_hex)?;
            let cm_2 =
                note_commitment(input_note_2.value, input_note_2.asset_id, sk, rho_2, rand_2);

            if self.mmr.leaf_count <= leaf_index_2 as usize {
                while self.mmr.leaf_count < leaf_index_2 as usize {
                    self.mmr.append(Fr::from(0u64));
                }
                self.mmr.append(cm_2);
            }
        }

        // Generate MMR proof for Note 1
        let proof1 = self.mmr.generate_proof(leaf_index_1 as usize);
        let in1_mountain_height = proof1.mountain_height as u8;
        let mut in1_mountain_siblings = [Fr::from(0u64); 32];
        for (i, &s) in proof1.mountain_siblings.iter().take(32).enumerate() {
            in1_mountain_siblings[i] = s;
        }

        let nk = self.nullifier_key()?;
        let nullifier_1 = derive_joinsplit_nullifier(nk, in1_rho, input_note_1.epoch() as u64);

        // 4. Load witnesses or generate canonical dummy witness for Input Note 2 (B4.2 / B4.3)
        let (
            in2_val,
            in2_rho,
            in2_rand,
            in2_index,
            in2_mountain_height,
            in2_mountain_siblings,
            in2_is_dummy,
            in2_nonce,
            nullifier_2,
            epoch_2,
        ) = if let Some(ref input_note_2) = input_note_2_opt {
            let leaf_index_2 = input_note_2
                .leaf_index
                .ok_or_else(|| WalletError::NoteMissingWitness("input 2 leaf_index".into()))?;

            let rho_2 = parse_fr_from_hex(&input_note_2.rho_hex)?;
            let rand_2 = parse_fr_from_hex(&input_note_2.randomness_hex)?;

            let proof2 = self.mmr.generate_proof(leaf_index_2 as usize);
            let mut in2_siblings = [Fr::from(0u64); 32];
            for (i, &s) in proof2.mountain_siblings.iter().take(32).enumerate() {
                in2_siblings[i] = s;
            }
            let nf_2 = derive_joinsplit_nullifier(nk, rho_2, input_note_2.epoch() as u64);

            (
                Fr::from(input_note_2.value),
                rho_2,
                rand_2,
                Fr::from(leaf_index_2),
                proof2.mountain_height as u8,
                in2_siblings,
                Fr::from(0u64), // real note
                Fr::from(0u64),
                nf_2,
                input_note_2.epoch(),
            )
        } else {
            // Canonical Dummy Witness Synthesis (B4.2)
            let mut rng = OsRng;
            let dummy_nonce = Fr::rand(&mut rng);
            let dummy_nk = derive_nullifier_key(CANONICAL_DUMMY_OWNER);
            let dummy_nf = derive_joinsplit_dummy_nullifier(dummy_nk, dummy_nonce);
            (
                Fr::from(CANONICAL_DUMMY_VALUE),
                CANONICAL_DUMMY_RHO,
                CANONICAL_DUMMY_RANDOMNESS,
                Fr::from(CANONICAL_DUMMY_LEAF_INDEX),
                0u8,
                [Fr::from(0u64); 32],
                Fr::from(1u64), // is dummy
                dummy_nonce,
                dummy_nf,
                input_note_1.epoch(), // epoch aligned with input 1
            )
        };

        // Snapshot current MMR peaks and root
        let mut mmr_peaks = [Fr::from(0u64); 32];
        for (i, &p) in self.mmr.get_peaks().iter().take(32).enumerate() {
            mmr_peaks[i] = p;
        }
        let note_root = self.mmr.get_root();
        let total_leaves = self.mmr.leaf_count as u64;

        // 5. Change Note 1 (Output 1)
        let (ch1_val, ch1_cm, ch1_rho, ch1_rand) = if selected.has_change {
            let mut rng = OsRng;
            let c_rho = Fr::rand(&mut rng);
            let c_rand = Fr::rand(&mut rng);
            let c_cm = note_commitment(
                selected.change_amount,
                input_note_1.asset_id,
                sk,
                c_rho,
                c_rand,
            );

            let c_cm_hex = hex::encode(fr_to_be_bytes(&c_cm));
            let c_rho_hex = hex::encode(fr_to_be_bytes(&c_rho));
            let c_rand_hex = hex::encode(fr_to_be_bytes(&c_rand));
            let sk_hex = hex::encode(fr_to_be_bytes(&sk));

            // Persist unconfirmed change note (2PC Phase 1)
            let change_note = WalletNote {
                commitment_hex: c_cm_hex,
                value: selected.change_amount,
                asset_id: input_note_1.asset_id,
                owner_key_hex: sk_hex,
                rho_hex: c_rho_hex,
                randomness_hex: c_rand_hex,
                leaf_index: None,
                leaf_count: None,
                epoch_id: input_note_1.epoch_id.or(Some(0)),
                merkle_path_hex: None,
                status: NoteStatus::Unconfirmed,
                created_at_secs: current_time_secs,
                session_id: Some(session_id.to_string()),
            };
            self.notes
                .insert(change_note.commitment_hex.clone(), change_note);

            (Fr::from(selected.change_amount), c_cm, c_rho, c_rand)
        } else {
            // Direction B: Canonical dummy output commitment (value = 0)
            let c_cm = note_commitment(
                CANONICAL_DUMMY_VALUE,
                input_note_1.asset_id,
                CANONICAL_DUMMY_OWNER,
                CANONICAL_DUMMY_RHO,
                CANONICAL_DUMMY_RANDOMNESS,
            );
            (
                Fr::from(CANONICAL_DUMMY_VALUE),
                c_cm,
                CANONICAL_DUMMY_RHO,
                CANONICAL_DUMMY_RANDOMNESS,
            )
        };

        // Output 2 is canonical dummy output commitment with value = 0 (ZIP 315 Constant-Arity Output Gadget)
        let ch2_val = Fr::from(CANONICAL_DUMMY_VALUE);
        let ch2_rho = CANONICAL_DUMMY_RHO;
        let ch2_rand = CANONICAL_DUMMY_RANDOMNESS;
        let ch2_cm = note_commitment(
            CANONICAL_DUMMY_VALUE,
            input_note_1.asset_id,
            CANONICAL_DUMMY_OWNER,
            CANONICAL_DUMMY_RHO,
            CANONICAL_DUMMY_RANDOMNESS,
        );

        // 6. EVM parameters & domain scope
        let recipient_bytes = parse_bytes32_from_hex(recipient_evm_address, true)?;
        let recipient_fr = Fr::from_be_bytes_mod_order(&recipient_bytes);

        let quote_bytes = parse_bytes32_from_hex(quote_hash_hex, false)?;
        let (q_hi, q_lo) = split_quote_hash_to_limbs(&quote_bytes);

        let contract_bytes = parse_bytes32_from_hex(contract_address_hex, true)?;
        let contract_fr = Fr::from_be_bytes_mod_order(&contract_bytes);

        let scope_hash = zeltra_core::compute_scope_hash(
            recipient_fr,
            Fr::from(chain_id),
            contract_fr,
            Fr::from(expiry),
        );
        let expected_binding = zeltra_core::compute_binding_commitment(q_hi, q_lo, scope_hash);

        let has_change_1_fr = if selected.has_change {
            Fr::from(1u64)
        } else {
            Fr::from(0u64)
        };
        let has_change_2_fr = Fr::from(0u64);
        let asset_id_u8 = input_note_1.asset_id;
        let asset_id_fr = Fr::from(asset_id_u8 as u64);
        let flags_packed_fr =
            has_change_1_fr + has_change_2_fr * Fr::from(2u64) + asset_id_fr * Fr::from(4u64);

        // 7. Construct JoinSplitCircuit (DEC-036A)
        let circuit = JoinSplitCircuit {
            note_root: Some(note_root),
            leaf_count: Some(Fr::from(total_leaves)),
            input_nullifier_1: Some(nullifier_1),
            input_nullifier_2: Some(nullifier_2),
            epoch_id_1: Some(Fr::from(input_note_1.epoch() as u64)),
            epoch_id_2: Some(Fr::from(epoch_2 as u64)),
            output_commitment_1: Some(ch1_cm),
            output_commitment_2: Some(ch2_cm),
            recipient: Some(recipient_fr),
            merchant_amount: Some(Fr::from(selected.merchant_amount)),
            protocol_fee: Some(Fr::from(selected.protocol_fee)),
            execution_fee: Some(Fr::from(selected.execution_fee)),
            quote_hash_hi: Some(q_hi),
            quote_hash_lo: Some(q_lo),
            chain_id: Some(Fr::from(chain_id)),
            contract_address: Some(contract_fr),
            expiry: Some(Fr::from(expiry)),
            flags_packed: Some(flags_packed_fr),
            is_rollover: Some(Fr::from(0u64)),

            in1_value: Some(Fr::from(input_note_1.value)),
            in1_owner_key: Some(sk),
            in1_rho: Some(in1_rho),
            in1_randomness: Some(in1_rand),
            in1_leaf_index: Some(Fr::from(leaf_index_1)),
            in1_mountain_height: Some(in1_mountain_height),
            in1_mountain_siblings: Some(in1_mountain_siblings),

            in2_value: Some(in2_val),
            in2_owner_key: Some(if in2_is_dummy == Fr::from(1u64) {
                CANONICAL_DUMMY_OWNER
            } else {
                sk
            }),
            in2_rho: Some(if in2_is_dummy == Fr::from(1u64) {
                CANONICAL_DUMMY_RHO
            } else {
                in2_rho
            }),
            in2_randomness: Some(if in2_is_dummy == Fr::from(1u64) {
                CANONICAL_DUMMY_RANDOMNESS
            } else {
                in2_rand
            }),
            in2_leaf_index: Some(if in2_is_dummy == Fr::from(1u64) {
                Fr::from(CANONICAL_DUMMY_LEAF_INDEX)
            } else {
                in2_index
            }),
            in2_mountain_height: Some(in2_mountain_height),
            in2_mountain_siblings: Some(in2_mountain_siblings),
            in2_is_dummy: Some(in2_is_dummy),
            in2_session_nonce: Some(in2_nonce),

            mmr_peaks: Some(mmr_peaks),

            out1_value: Some(ch1_val),
            out1_owner_key: Some(if selected.has_change {
                sk
            } else {
                CANONICAL_DUMMY_OWNER
            }),
            out1_rho: Some(if selected.has_change {
                ch1_rho
            } else {
                CANONICAL_DUMMY_RHO
            }),
            out1_randomness: Some(if selected.has_change {
                ch1_rand
            } else {
                CANONICAL_DUMMY_RANDOMNESS
            }),

            out2_value: Some(ch2_val),
            out2_owner_key: Some(CANONICAL_DUMMY_OWNER),
            out2_rho: Some(ch2_rho),
            out2_randomness: Some(ch2_rand),

            has_change_1: Some(has_change_1_fr),
            has_change_2: Some(has_change_2_fr),
            expected_binding: Some(expected_binding),

            public_asset_id: Some(asset_id_fr),
            in1_asset_id: Some(asset_id_fr),
            in2_asset_id: Some(asset_id_fr),
            out1_asset_id: Some(asset_id_fr),
            out2_asset_id: Some(asset_id_fr),
        };

        // 8. Generate Groth16 Proof
        let proof = generate_joinsplit_proof(circuit.clone(), joinsplit_pk).map_err(|e| {
            WalletError::CryptoError(format!("JoinSplit proof generation failed: {e:?}"))
        })?;

        // 9. Format EVM Proof Points
        let proof_a_neg = to_evm_g1(&-proof.a);
        let proof_b = to_evm_g2(&proof.b);
        let proof_c = to_evm_g1(&proof.c);

        // 10. Format canonical 19 Public Inputs (DEC-036A Section 5)
        let pis = extract_joinsplit_public_inputs(&circuit);
        let public_inputs: Vec<String> = pis
            .iter()
            .map(|fr| hex::encode(fr_to_be_bytes(fr)))
            .collect();

        let sender_pk_ivk = self.incoming_viewing_public_key()?;
        let memo_1 = if selected.has_change {
            zeltra_core::encrypt_committing_note_memo(
                selected.change_amount,
                ch1_rho,
                input_note_1.asset_id as u64,
                &sender_pk_ivk,
            )
            .map(|(m, _)| m)
            .unwrap_or_else(|_| vec![0u8; 96])
        } else {
            zeltra_core::encrypt_committing_note_memo(
                0,
                ch1_rho,
                input_note_1.asset_id as u64,
                &sender_pk_ivk,
            )
            .map(|(m, _)| m)
            .unwrap_or_else(|_| vec![0u8; 96])
        };
        let memo_2 = zeltra_core::encrypt_committing_note_memo(
            0,
            ch2_rho,
            input_note_1.asset_id as u64,
            &sender_pk_ivk,
        )
        .map(|(m, _)| m)
        .unwrap_or_else(|_| vec![0u8; 96]);

        Ok(JoinSplitSpendProofPayload {
            session_id: session_id.to_string(),
            input_commitment_1_hex: selected.input_commitment_hex.clone(),
            input_commitment_2_hex: selected.second_input_commitment_hex.clone(),
            input_nullifier_1_hex: hex::encode(fr_to_be_bytes(&nullifier_1)),
            input_nullifier_2_hex: hex::encode(fr_to_be_bytes(&nullifier_2)),
            note_root_hex: hex::encode(fr_to_be_bytes(&note_root)),
            output_commitment_1_hex: hex::encode(fr_to_be_bytes(&ch1_cm)),
            output_commitment_2_hex: hex::encode(fr_to_be_bytes(&ch2_cm)),
            recipient_hex: recipient_evm_address.to_string(),
            merchant_amount: selected.merchant_amount,
            protocol_fee: selected.protocol_fee,
            execution_fee: selected.execution_fee,
            max_execution_fee,
            quote_hash_hex: quote_hash_hex.to_string(),
            chain_id,
            contract_address_hex: contract_address_hex.to_string(),
            expiry,
            has_change_1: selected.has_change,
            has_change_2: false,
            proof_a_neg_hex: hex::encode(proof_a_neg),
            proof_b_hex: hex::encode(proof_b),
            proof_c_hex: hex::encode(proof_c),
            public_inputs_hex: public_inputs,
            encrypted_memo_1: hex::encode(memo_1),
            encrypted_memo_2: hex::encode(memo_2),
        })
    }

    /// High-level client API: executes coin selection and generates a JoinSplit spend proof payload (DEC-030).
    #[allow(clippy::too_many_arguments)]
    pub fn pay_joinsplit(
        &mut self,
        recipient_evm_address: &str,
        merchant_amount: u64,
        protocol_fee: u64,
        execution_fee: u64,
        max_execution_fee: u64,
        quote_hash_hex: &str,
        chain_id: u64,
        contract_address_hex: &str,
        expiry: u64,
        current_time_secs: u64,
        joinsplit_pk: &ark_groth16::ProvingKey<Bls12_381>,
    ) -> Result<JoinSplitSpendProofPayload, WalletError> {
        let selected = self.select_note_for_spend(
            merchant_amount,
            protocol_fee,
            execution_fee,
            current_time_secs,
        )?;

        let session_id = format!(
            "joinsplit_{}_{}",
            current_time_secs,
            hex::encode(rand::random::<[u8; 8]>())
        );

        self.prepare_joinsplit_spend_proof(
            &selected,
            &session_id,
            recipient_evm_address,
            max_execution_fee,
            quote_hash_hex,
            chain_id,
            contract_address_hex,
            expiry,
            current_time_secs,
            joinsplit_pk,
        )
    }

    /// Prepares a Universal 2-in-2-out JoinSplit Groth16 proof locally for a shielded-to-shielded
    /// in-pool transfer to a recipient's Zeltra Shielded Address (ZSA) (DEC-043 / DEC-043C).
    ///
    /// - Mode 2 Gating: merchant_amount = 0, protocol_fee = 0, recipient = Address::ZERO
    /// - Output 1: recipient note with deterministic randomness derived from recipient ZSA
    /// - Output 2: sender change note (or random dummy note if exact payment)
    /// - Dual-memo (96B each) included in JoinSplitSpendProofPayload
    #[allow(clippy::too_many_arguments)]
    pub fn prepare_shielded_transfer_proof(
        &mut self,
        recipient_zsa_bech32m_or_hex: &str,
        transfer_amount: u64,
        asset_id: u8,
        max_execution_fee: u64,
        quote_hash_hex: &str,
        chain_id: u64,
        contract_address_hex: &str,
        expiry: u64,
        current_time_secs: u64,
        joinsplit_pk: &ark_groth16::ProvingKey<Bls12_381>,
    ) -> Result<JoinSplitSpendProofPayload, WalletError> {
        // 1. Validasi nominal transfer (AUD-6)
        if transfer_amount == 0 {
            return Err(WalletError::CryptoError(
                "Transfer amount must be strictly greater than zero".into(),
            ));
        }

        if asset_id > 1 {
            return Err(WalletError::UnsupportedAssetId(asset_id));
        }

        // 2. Parsing ZSA penerima (64B) (AUD-1)
        let recipient_zsa = if recipient_zsa_bech32m_or_hex.starts_with("zs") {
            ZeltraShieldedAddress::from_bech32m(recipient_zsa_bech32m_or_hex)
                .map_err(|e| WalletError::CryptoError(format!("Invalid Bech32m ZSA: {e}")))?
        } else {
            ZeltraShieldedAddress::from_hex(recipient_zsa_bech32m_or_hex)
                .map_err(|e| WalletError::CryptoError(format!("Invalid Hex ZSA: {e}")))?
        };

        // 3. Stochastic Knapsack Coin Selection
        let total_required = transfer_amount
            .checked_add(max_execution_fee)
            .ok_or_else(|| WalletError::CryptoError("Transfer amount overflow".into()))?;
        let selected = self.select_notes_knapsack_asset(total_required, asset_id)?;

        let session_id = format!(
            "transfer_{}_{}",
            current_time_secs,
            hex::encode(rand::random::<[u8; 8]>())
        );

        // 4. Reserve Input Note 1
        self.reserve_note(
            &selected.input_commitment_hex,
            &session_id,
            current_time_secs,
            DEFAULT_RESERVATION_TTL_SECS,
        )?;

        // Reserve Input Note 2 if 2-note JoinSplit
        if let Some(ref cm2_hex) = selected.second_input_commitment_hex {
            if let Err(e) = self.reserve_note(
                cm2_hex,
                &session_id,
                current_time_secs,
                DEFAULT_RESERVATION_TTL_SECS,
            ) {
                let _ = self.rollback_spend(&session_id);
                return Err(e);
            }
        }

        // 5. Load witnesses for Input Note 1
        let input_note_1 = self
            .notes
            .get(&selected.input_commitment_hex)
            .ok_or_else(|| WalletError::NoteNotFound(selected.input_commitment_hex.clone()))?
            .clone();

        let leaf_index_1 = input_note_1
            .leaf_index
            .ok_or_else(|| WalletError::NoteMissingWitness("input 1 leaf_index".into()))?;

        let sk = self.spending_key()?;
        let in1_rho = parse_fr_from_hex(&input_note_1.rho_hex)?;
        let in1_rand = parse_fr_from_hex(&input_note_1.randomness_hex)?;
        let in1_owner = parse_fr_from_hex(&input_note_1.owner_key_hex).unwrap_or(sk);
        let in1_cm = note_commitment(
            input_note_1.value,
            input_note_1.asset_id,
            in1_owner,
            in1_rho,
            in1_rand,
        );

        // Pre-fetch Input Note 2 if present
        let input_note_2_opt = if let Some(ref cm2_hex) = selected.second_input_commitment_hex {
            Some(
                self.notes
                    .get(cm2_hex)
                    .ok_or_else(|| WalletError::NoteNotFound(cm2_hex.clone()))?
                    .clone(),
            )
        } else {
            None
        };

        // Ensure Note 1 in local MMR
        if self.mmr.leaf_count <= leaf_index_1 as usize {
            while self.mmr.leaf_count < leaf_index_1 as usize {
                self.mmr.append(Fr::from(0u64));
            }
            self.mmr.append(in1_cm);
        }

        // If Note 2 is real, ensure Note 2 in local MMR
        if let Some(ref input_note_2) = input_note_2_opt {
            let leaf_index_2 = input_note_2
                .leaf_index
                .ok_or_else(|| WalletError::NoteMissingWitness("input 2 leaf_index".into()))?;
            let rho_2 = parse_fr_from_hex(&input_note_2.rho_hex)?;
            let rand_2 = parse_fr_from_hex(&input_note_2.randomness_hex)?;
            let in2_owner = parse_fr_from_hex(&input_note_2.owner_key_hex).unwrap_or(sk);
            let cm_2 = note_commitment(
                input_note_2.value,
                input_note_2.asset_id,
                in2_owner,
                rho_2,
                rand_2,
            );

            if self.mmr.leaf_count <= leaf_index_2 as usize {
                while self.mmr.leaf_count < leaf_index_2 as usize {
                    self.mmr.append(Fr::from(0u64));
                }
                self.mmr.append(cm_2);
            }
        }

        // Generate MMR proof for Note 1
        let proof1 = self.mmr.generate_proof(leaf_index_1 as usize);
        let in1_mountain_height = proof1.mountain_height as u8;
        let mut in1_mountain_siblings = [Fr::from(0u64); 32];
        for (i, &s) in proof1.mountain_siblings.iter().take(32).enumerate() {
            in1_mountain_siblings[i] = s;
        }

        let nk = self.nullifier_key()?;
        let nullifier_1 = derive_joinsplit_nullifier(nk, in1_rho, input_note_1.epoch() as u64);

        // 6. Load witnesses or generate canonical dummy witness for Input Note 2
        let (
            in2_val,
            in2_owner,
            in2_rho,
            in2_rand,
            in2_index,
            in2_mountain_height,
            in2_mountain_siblings,
            in2_is_dummy,
            in2_nonce,
            nullifier_2,
            epoch_2,
        ) = if let Some(ref input_note_2) = input_note_2_opt {
            let leaf_index_2 = input_note_2
                .leaf_index
                .ok_or_else(|| WalletError::NoteMissingWitness("input 2 leaf_index".into()))?;

            let rho_2 = parse_fr_from_hex(&input_note_2.rho_hex)?;
            let rand_2 = parse_fr_from_hex(&input_note_2.randomness_hex)?;
            let in2_owner_key = parse_fr_from_hex(&input_note_2.owner_key_hex).unwrap_or(sk);

            let proof2 = self.mmr.generate_proof(leaf_index_2 as usize);
            let mut in2_siblings = [Fr::from(0u64); 32];
            for (i, &s) in proof2.mountain_siblings.iter().take(32).enumerate() {
                in2_siblings[i] = s;
            }
            let nf_2 = derive_joinsplit_nullifier(nk, rho_2, input_note_2.epoch() as u64);

            (
                Fr::from(input_note_2.value),
                in2_owner_key,
                rho_2,
                rand_2,
                Fr::from(leaf_index_2),
                proof2.mountain_height as u8,
                in2_siblings,
                Fr::from(0u64), // real note
                Fr::from(0u64),
                nf_2,
                input_note_2.epoch(),
            )
        } else {
            // Canonical Dummy Witness Synthesis
            let mut rng = OsRng;
            let dummy_nonce = Fr::rand(&mut rng);
            let dummy_nk = derive_nullifier_key(CANONICAL_DUMMY_OWNER);
            let dummy_nf = derive_joinsplit_dummy_nullifier(dummy_nk, dummy_nonce);
            (
                Fr::from(CANONICAL_DUMMY_VALUE),
                CANONICAL_DUMMY_OWNER,
                CANONICAL_DUMMY_RHO,
                CANONICAL_DUMMY_RANDOMNESS,
                Fr::from(CANONICAL_DUMMY_LEAF_INDEX),
                0u8,
                [Fr::from(0u64); 32],
                Fr::from(1u64), // is dummy
                dummy_nonce,
                dummy_nf,
                input_note_1.epoch(),
            )
        };

        // Snapshot current MMR peaks and root
        let mut mmr_peaks = [Fr::from(0u64); 32];
        for (i, &p) in self.mmr.get_peaks().iter().take(32).enumerate() {
            mmr_peaks[i] = p;
        }
        let note_root = self.mmr.get_root();
        let total_leaves = self.mmr.leaf_count as u64;

        // 7. Output 1: Recipient Note with Deterministic Randomness (AUD-2)
        let mut rng = OsRng;
        let rho_1 = Fr::rand(&mut rng);
        let (memo_1, rand_1) = encrypt_committing_note_memo(
            transfer_amount,
            rho_1,
            asset_id as u64,
            &recipient_zsa.incoming_viewing_public_key,
        )
        .map_err(|e| WalletError::CryptoError(format!("Memo 1 encryption failed: {e}")))?;

        let cm_1 = note_commitment(
            transfer_amount,
            asset_id,
            recipient_zsa.owner_key,
            rho_1,
            rand_1,
        );

        // 8. Output 2: Sender Change Note (AUD-3 & AUD-7)
        let change_amount = selected.change_amount;
        let sender_owner_key = self.owner_key()?;
        let sender_pk_ivk = self.incoming_viewing_public_key()?;

        let (ch2_val, cm_2, memo_2, has_change_2, ch2_rho, ch2_rand) = if change_amount > 0 {
            let rho_2 = Fr::rand(&mut rng);
            let (memo_2, rand_2) = encrypt_committing_note_memo(
                change_amount,
                rho_2,
                asset_id as u64,
                &sender_pk_ivk,
            )
            .map_err(|e| WalletError::CryptoError(format!("Memo 2 encryption failed: {e}")))?;

            let c_cm = note_commitment(change_amount, asset_id, sender_owner_key, rho_2, rand_2);

            let c_cm_hex = hex::encode(fr_to_be_bytes(&c_cm));
            let c_rho_hex = hex::encode(fr_to_be_bytes(&rho_2));
            let c_rand_hex = hex::encode(fr_to_be_bytes(&rand_2));
            let owner_hex = hex::encode(fr_to_be_bytes(&sender_owner_key));

            // Persist unconfirmed change note (2PC Phase 1)
            let change_note = WalletNote {
                commitment_hex: c_cm_hex,
                value: change_amount,
                asset_id,
                owner_key_hex: owner_hex,
                rho_hex: c_rho_hex,
                randomness_hex: c_rand_hex,
                leaf_index: None,
                leaf_count: None,
                epoch_id: input_note_1.epoch_id.or(Some(0)),
                merkle_path_hex: None,
                status: NoteStatus::Unconfirmed,
                created_at_secs: current_time_secs,
                session_id: Some(session_id.clone()),
            };
            self.notes
                .insert(change_note.commitment_hex.clone(), change_note);

            (Fr::from(change_amount), c_cm, memo_2, true, rho_2, rand_2)
        } else {
            // Random encryption for dummy memo (AUD-7: anti-topologic leak)
            let rho_2 = Fr::rand(&mut rng);
            let (dummy_memo, _rand_2) =
                encrypt_committing_note_memo(0, rho_2, asset_id as u64, &sender_pk_ivk)
                    .map_err(|e| {
                        WalletError::CryptoError(format!("Dummy memo encryption failed: {e}"))
                    })?;

            let dummy_cm = note_commitment(
                CANONICAL_DUMMY_VALUE,
                asset_id,
                CANONICAL_DUMMY_OWNER,
                CANONICAL_DUMMY_RHO,
                CANONICAL_DUMMY_RANDOMNESS,
            );

            (
                Fr::from(CANONICAL_DUMMY_VALUE),
                dummy_cm,
                dummy_memo,
                false,
                CANONICAL_DUMMY_RHO,
                CANONICAL_DUMMY_RANDOMNESS,
            )
        };

        // 9. EVM parameters & domain scope (Mode 2: merchant_amount = 0, protocol_fee = 0, recipient = Address::ZERO)
        let recipient_fr = Fr::from(0u64);
        let quote_bytes = parse_bytes32_from_hex(quote_hash_hex, false)?;
        let (q_hi, q_lo) = split_quote_hash_to_limbs(&quote_bytes);

        let contract_bytes = parse_bytes32_from_hex(contract_address_hex, true)?;
        let contract_fr = Fr::from_be_bytes_mod_order(&contract_bytes);

        let scope_hash = zeltra_core::compute_scope_hash(
            recipient_fr,
            Fr::from(chain_id),
            contract_fr,
            Fr::from(expiry),
        );
        let expected_binding = zeltra_core::compute_binding_commitment(q_hi, q_lo, scope_hash);

        let has_change_1_fr = Fr::from(1u64); // Output 1 is recipient active note
        let has_change_2_fr = if has_change_2 {
            Fr::from(1u64)
        } else {
            Fr::from(0u64)
        };
        let asset_id_fr = Fr::from(asset_id as u64);
        let flags_packed_fr =
            has_change_1_fr + has_change_2_fr * Fr::from(2u64) + asset_id_fr * Fr::from(4u64);

        // 10. Construct JoinSplitCircuit (Mode 2)
        let circuit = JoinSplitCircuit {
            note_root: Some(note_root),
            leaf_count: Some(Fr::from(total_leaves)),
            input_nullifier_1: Some(nullifier_1),
            input_nullifier_2: Some(nullifier_2),
            epoch_id_1: Some(Fr::from(input_note_1.epoch() as u64)),
            epoch_id_2: Some(Fr::from(epoch_2 as u64)),
            output_commitment_1: Some(cm_1),
            output_commitment_2: Some(cm_2),
            recipient: Some(recipient_fr),
            merchant_amount: Some(Fr::from(0u64)),
            protocol_fee: Some(Fr::from(0u64)),
            execution_fee: Some(Fr::from(max_execution_fee)),
            quote_hash_hi: Some(q_hi),
            quote_hash_lo: Some(q_lo),
            chain_id: Some(Fr::from(chain_id)),
            contract_address: Some(contract_fr),
            expiry: Some(Fr::from(expiry)),
            flags_packed: Some(flags_packed_fr),
            is_rollover: Some(Fr::from(0u64)),

            in1_value: Some(Fr::from(input_note_1.value)),
            in1_owner_key: Some(in1_owner),
            in1_rho: Some(in1_rho),
            in1_randomness: Some(in1_rand),
            in1_leaf_index: Some(Fr::from(leaf_index_1)),
            in1_mountain_height: Some(in1_mountain_height),
            in1_mountain_siblings: Some(in1_mountain_siblings),

            in2_value: Some(in2_val),
            in2_owner_key: Some(in2_owner),
            in2_rho: Some(in2_rho),
            in2_randomness: Some(in2_rand),
            in2_leaf_index: Some(in2_index),
            in2_mountain_height: Some(in2_mountain_height),
            in2_mountain_siblings: Some(in2_mountain_siblings),
            in2_is_dummy: Some(in2_is_dummy),
            in2_session_nonce: Some(in2_nonce),

            mmr_peaks: Some(mmr_peaks),

            out1_value: Some(Fr::from(transfer_amount)),
            out1_owner_key: Some(recipient_zsa.owner_key),
            out1_rho: Some(rho_1),
            out1_randomness: Some(rand_1),

            out2_value: Some(ch2_val),
            out2_owner_key: Some(if has_change_2 {
                sender_owner_key
            } else {
                CANONICAL_DUMMY_OWNER
            }),
            out2_rho: Some(if has_change_2 {
                ch2_rho
            } else {
                CANONICAL_DUMMY_RHO
            }),
            out2_randomness: Some(if has_change_2 {
                ch2_rand
            } else {
                CANONICAL_DUMMY_RANDOMNESS
            }),

            has_change_1: Some(has_change_1_fr),
            has_change_2: Some(has_change_2_fr),
            expected_binding: Some(expected_binding),

            public_asset_id: Some(asset_id_fr),
            in1_asset_id: Some(asset_id_fr),
            in2_asset_id: Some(asset_id_fr),
            out1_asset_id: Some(asset_id_fr),
            out2_asset_id: Some(asset_id_fr),
        };

        // 11. Generate Groth16 Proof
        let proof = generate_joinsplit_proof(circuit.clone(), joinsplit_pk).map_err(|e| {
            WalletError::CryptoError(format!("JoinSplit transfer proof generation failed: {e:?}"))
        })?;

        // 12. Format EVM Proof Points & Public Inputs
        let proof_a_neg = to_evm_g1(&-proof.a);
        let proof_b = to_evm_g2(&proof.b);
        let proof_c = to_evm_g1(&proof.c);

        let pis = extract_joinsplit_public_inputs(&circuit);
        let public_inputs: Vec<String> = pis
            .iter()
            .map(|fr| hex::encode(fr_to_be_bytes(fr)))
            .collect();

        Ok(JoinSplitSpendProofPayload {
            session_id,
            input_commitment_1_hex: selected.input_commitment_hex.clone(),
            input_commitment_2_hex: selected.second_input_commitment_hex.clone(),
            input_nullifier_1_hex: hex::encode(fr_to_be_bytes(&nullifier_1)),
            input_nullifier_2_hex: hex::encode(fr_to_be_bytes(&nullifier_2)),
            note_root_hex: hex::encode(fr_to_be_bytes(&note_root)),
            output_commitment_1_hex: hex::encode(fr_to_be_bytes(&cm_1)),
            output_commitment_2_hex: hex::encode(fr_to_be_bytes(&cm_2)),
            recipient_hex: "0x0000000000000000000000000000000000000000".to_string(),
            merchant_amount: 0,
            protocol_fee: 0,
            execution_fee: max_execution_fee,
            max_execution_fee,
            quote_hash_hex: quote_hash_hex.to_string(),
            chain_id,
            contract_address_hex: contract_address_hex.to_string(),
            expiry,
            has_change_1: true,
            has_change_2,
            proof_a_neg_hex: hex::encode(proof_a_neg),
            proof_b_hex: hex::encode(proof_b),
            proof_c_hex: hex::encode(proof_c),
            public_inputs_hex: public_inputs,
            encrypted_memo_1: hex::encode(memo_1),
            encrypted_memo_2: hex::encode(memo_2),
        })
    }

    /// In-Pool Autonomous Consolidation (DEC-030 Branch 4).
    ///
    /// Combines two fragmented notes (`note_cm_1` and `note_cm_2`) into a single
    /// consolidated change note within the shielded pool without revealing balances
    /// to public accounts on-chain.
    #[allow(clippy::too_many_arguments)]
    pub fn consolidate_note_pair(
        &mut self,
        note_cm_1: &str,
        note_cm_2: &str,
        session_id: &str,
        execution_fee: u64,
        max_execution_fee: u64,
        quote_hash_hex: &str,
        chain_id: u64,
        contract_address_hex: &str,
        expiry: u64,
        current_time_secs: u64,
        joinsplit_pk: &ark_groth16::ProvingKey<Bls12_381>,
    ) -> Result<JoinSplitSpendProofPayload, WalletError> {
        let (val1, val2) = {
            let n1 = self
                .notes
                .get(note_cm_1)
                .ok_or_else(|| WalletError::NoteNotFound(note_cm_1.to_string()))?;
            let n2 = self
                .notes
                .get(note_cm_2)
                .ok_or_else(|| WalletError::NoteNotFound(note_cm_2.to_string()))?;

            if !matches!(n1.status, NoteStatus::Unspent) {
                return Err(WalletError::NoteNotSpendable(note_cm_1.to_string()));
            }
            if !matches!(n2.status, NoteStatus::Unspent) {
                return Err(WalletError::NoteNotSpendable(note_cm_2.to_string()));
            }
            (n1.value, n2.value)
        };

        let total_in = val1
            .checked_add(val2)
            .ok_or_else(|| WalletError::CryptoError("Combined note value overflow".into()))?;

        if total_in <= execution_fee {
            return Err(WalletError::CryptoError(
                "Combined note value cannot cover execution fee".into(),
            ));
        }

        let consolidated_value = total_in - execution_fee;

        let selected = SelectedSpend {
            input_commitment_hex: note_cm_1.to_string(),
            input_value: val1,
            second_input_commitment_hex: Some(note_cm_2.to_string()),
            second_input_value: val2,
            merchant_amount: 0,
            protocol_fee: 0,
            execution_fee,
            total_required: execution_fee,
            change_amount: consolidated_value,
            has_change: true,
        };

        // Recipient is Address::ZERO for autonomous in-pool consolidation
        let zero_address = "0x0000000000000000000000000000000000000000";

        self.prepare_joinsplit_spend_proof(
            &selected,
            session_id,
            zero_address,
            max_execution_fee,
            quote_hash_hex,
            chain_id,
            contract_address_hex,
            expiry,
            current_time_secs,
            joinsplit_pk,
        )
    }

    /// High-level client API: transfers funds to another EVM address or stealth address.
    #[allow(clippy::too_many_arguments)]
    pub fn send_to_wallet(
        &mut self,
        recipient_wallet: &str,
        amount: u64,
        protocol_fee: u64,
        execution_fee: u64,
        max_execution_fee: u64,
        quote_hash_hex: &str,
        chain_id: u64,
        contract_address_hex: &str,
        expiry: u64,
        current_time_secs: u64,
        pk: &ark_groth16::ProvingKey<Bls12_381>,
    ) -> Result<SpendProofPayload, WalletError> {
        self.pay(
            recipient_wallet,
            amount,
            protocol_fee,
            execution_fee,
            max_execution_fee,
            quote_hash_hex,
            chain_id,
            contract_address_hex,
            expiry,
            current_time_secs,
            pk,
        )
    }

    /// High-level client API: generates withdrawal spend proofs to sweep entire balance to an owner wallet.
    #[allow(clippy::too_many_arguments)]
    pub fn withdraw_all(
        &mut self,
        owner_wallet: &str,
        protocol_fee_per_note: u64,
        execution_fee_per_note: u64,
        max_execution_fee_per_note: u64,
        quote_hash_hex: &str,
        chain_id: u64,
        contract_address_hex: &str,
        expiry: u64,
        current_time_secs: u64,
        pk: &ark_groth16::ProvingKey<Bls12_381>,
    ) -> Result<Vec<SpendProofPayload>, WalletError> {
        let spendable_cms: Vec<(String, u64)> = self
            .notes
            .values()
            .filter(|n| {
                if n.merkle_path_hex.is_none() || n.leaf_index.is_none() {
                    return false;
                }
                match &n.status {
                    NoteStatus::Unspent => true,
                    NoteStatus::Reserved {
                        lease_expiry_secs, ..
                    } => *lease_expiry_secs <= current_time_secs,
                    _ => false,
                }
            })
            .map(|n| (n.commitment_hex.clone(), n.value))
            .collect();

        if spendable_cms.is_empty() {
            return Err(WalletError::InsufficientBalance {
                requested: 1,
                available: 0,
            });
        }

        let mut payloads = Vec::with_capacity(spendable_cms.len());
        for (cm, val) in spendable_cms {
            let total_fees = protocol_fee_per_note + execution_fee_per_note;
            if val <= total_fees {
                continue; // Skip dust notes smaller than fees
            }
            let net_merchant = val - total_fees;
            let selected = SelectedSpend {
                input_commitment_hex: cm,
                input_value: val,
                second_input_commitment_hex: None,
                second_input_value: 0,
                merchant_amount: net_merchant,
                protocol_fee: protocol_fee_per_note,
                execution_fee: execution_fee_per_note,
                total_required: val,
                change_amount: 0,
                has_change: false,
            };

            let session_id = format!(
                "withdraw_{}_{}",
                current_time_secs,
                hex::encode(rand::random::<[u8; 8]>())
            );
            let payload = self.prepare_spend_proof(
                &selected,
                &session_id,
                owner_wallet,
                max_execution_fee_per_note,
                quote_hash_hex,
                chain_id,
                contract_address_hex,
                expiry,
                current_time_secs,
                pk,
            )?;
            payloads.push(payload);
        }

        Ok(payloads)
    }
}
