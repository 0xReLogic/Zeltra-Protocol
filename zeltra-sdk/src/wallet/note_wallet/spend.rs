use ark_bls12_381::{Bls12_381, Fr};
use ark_ff::{PrimeField, UniformRand};
use rand::rngs::OsRng;
use zeltra_core::{
    derive_nullifier, encrypt_committing_note_memo, fr_to_be_bytes, generate_note_proof,
    note_commitment, to_evm_g1, to_evm_g2,
};

use super::types::*;
use super::PrivateNoteWallet;

impl PrivateNoteWallet {
    /// Updates the Merkle witness and promotes an Unconfirmed or stale note to Unspent (DEC-030).
    ///
    /// Essential for asynchronous MMR indexing (eliminates Ghost Notes) and local witness fast-forwarding.
    pub fn update_note_witness(
        &mut self,
        commitment_hex: &str,
        leaf_index: u64,
        merkle_path: Vec<String>,
        root_hex: &str,
    ) -> Result<(), WalletError> {
        if merkle_path.is_empty() {
            return Err(WalletError::CryptoError(
                "Merkle path cannot be empty".to_string(),
            ));
        }

        let clean = commitment_hex.strip_prefix("0x").unwrap_or(commitment_hex);
        let with_0x = format!("0x{}", clean);
        if !self.notes.contains_key(commitment_hex)
            && !self.notes.contains_key(clean)
            && !self.notes.contains_key(&with_0x)
        {
            return Err(WalletError::NoteNotFound(commitment_hex.to_string()));
        }

        let cm_fr = parse_fr_from_hex(commitment_hex)?;

        if self.mmr.leaf_count <= leaf_index as usize {
            while self.mmr.leaf_count < leaf_index as usize {
                self.mmr.append(Fr::from(0u64));
            }
            self.mmr.append(cm_fr);
        }

        let note = if self.notes.contains_key(commitment_hex) {
            self.notes.get_mut(commitment_hex)
        } else if self.notes.contains_key(clean) {
            self.notes.get_mut(clean)
        } else {
            self.notes.get_mut(&with_0x)
        }
        .ok_or_else(|| WalletError::NoteNotFound(commitment_hex.to_string()))?;

        note.leaf_index = Some(leaf_index);
        note.leaf_count = Some(self.mmr.leaf_count as u64);
        note.merkle_path_hex = Some(merkle_path);
        note.status = NoteStatus::Unspent;
        self.current_merkle_root_hex = Some(root_hex.to_string());

        Ok(())
    }

    /// Multi-UTXO Stochastic Knapsack Coin Selection Pipeline (DEC-036B Phase 1).
    ///
    /// Executes the 4-tier pipeline:
    /// - Tier 1: Exact match single note (v = T, zero change).
    /// - Tier 2: Smallest sufficient single note (v > T, dust absorption).
    /// - Tier 3: 2-note combination via Stochastic Knapsack (v1 + v2 >= T, entropy noise).
    /// - Tier 4: Fragmentation lockout error advising `consolidate_notes()`.
    pub fn select_note_for_spend(
        &self,
        merchant_amount: u64,
        protocol_fee: u64,
        execution_fee: u64,
        current_time_secs: u64,
    ) -> Result<SelectedSpend, WalletError> {
        self.select_note_for_spend_with_epoch(
            merchant_amount,
            protocol_fee,
            execution_fee,
            current_time_secs,
            0,
        )
    }

    /// Multi-UTXO Stochastic Knapsack Coin Selection with specific epoch filtering (DEC-036B).
    pub fn select_note_for_spend_with_epoch(
        &self,
        merchant_amount: u64,
        protocol_fee: u64,
        execution_fee: u64,
        current_time_secs: u64,
        current_epoch: u32,
    ) -> Result<SelectedSpend, WalletError> {
        let candidates: Vec<&WalletNote> = self.notes.values().collect();
        crate::coin_selection::select_notes_stochastic_knapsack(
            &candidates,
            merchant_amount,
            protocol_fee,
            execution_fee,
            current_time_secs,
            current_epoch,
            &crate::coin_selection::KnapsackConfig::default(),
        )
    }

    /// Multi-UTXO Stochastic Knapsack Coin Selection filtered by asset ID (DEC-043C).
    pub fn select_notes_knapsack_asset(
        &self,
        total_required: u64,
        asset_id: u8,
    ) -> Result<SelectedSpend, WalletError> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let candidates: Vec<&WalletNote> = self
            .notes
            .values()
            .filter(|n| n.asset_id == asset_id)
            .collect();
        crate::coin_selection::select_notes_stochastic_knapsack(
            &candidates,
            total_required,
            0,
            0,
            now,
            0,
            &crate::coin_selection::KnapsackConfig::default(),
        )
    }

    /// Prepares a Groth16 spend proof locally using `zeltra-core` PrivateNoteCircuit.
    ///
    /// Prepares a Groth16 spend proof locally using `zeltra-core` PrivateNoteCircuit.
    ///
    /// NEVER transmits the private spending key or note witnesses to any relayer.
    #[allow(clippy::too_many_arguments)]
    pub fn prepare_spend_proof(
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
        pk: &ark_groth16::ProvingKey<Bls12_381>,
    ) -> Result<SpendProofPayload, WalletError> {
        self.prepare_spend_proof_with_epoch(
            selected,
            session_id,
            recipient_evm_address,
            max_execution_fee,
            quote_hash_hex,
            chain_id,
            contract_address_hex,
            expiry,
            None,
            false,
            current_time_secs,
            pk,
        )
    }

    /// Prepares a Groth16 spend proof locally with optional target change epoch and rollover mode (DEC-033).
    #[allow(clippy::too_many_arguments)]
    pub fn prepare_spend_proof_with_epoch(
        &mut self,
        selected: &SelectedSpend,
        session_id: &str,
        recipient_evm_address: &str,
        max_execution_fee: u64,
        quote_hash_hex: &str,
        chain_id: u64,
        contract_address_hex: &str,
        expiry: u64,
        target_change_epoch: Option<u32>,
        is_rollover: bool,
        current_time_secs: u64,
        pk: &ark_groth16::ProvingKey<Bls12_381>,
    ) -> Result<SpendProofPayload, WalletError> {
        if selected.is_joinsplit() {
            return Err(WalletError::CryptoError(
                "Selected spend requires 2-note JoinSplit; please use prepare_joinsplit_spend_proof or pay_joinsplit".into(),
            ));
        }

        // Reserve the input note
        self.reserve_note(
            &selected.input_commitment_hex,
            session_id,
            current_time_secs,
            DEFAULT_RESERVATION_TTL_SECS,
        )?;

        let input_note = self
            .notes
            .get(&selected.input_commitment_hex)
            .ok_or_else(|| WalletError::NoteNotFound(selected.input_commitment_hex.clone()))?
            .clone();

        let leaf_index = input_note
            .leaf_index
            .ok_or_else(|| WalletError::NoteMissingWitness("leaf_index".into()))?;
        let merkle_path_strings = input_note
            .merkle_path_hex
            .as_ref()
            .ok_or_else(|| WalletError::NoteMissingWitness("merkle_path".into()))?;

        if merkle_path_strings.is_empty() {
            return Err(WalletError::CryptoError(
                "Merkle path cannot be empty".into(),
            ));
        }

        let sk = self.spending_key()?;
        let in_rho = parse_fr_from_hex(&input_note.rho_hex)?;
        let in_rand = parse_fr_from_hex(&input_note.randomness_hex)?;

        let in_cm = note_commitment(input_note.value, input_note.asset_id, sk, in_rho, in_rand);
        let nk = self.nullifier_key()?;
        let note_epoch_id = input_note.epoch_id.unwrap_or(0);
        let nullifier = derive_nullifier(nk, in_cm, leaf_index, note_epoch_id);

        // Synchronize local MMR if needed, or use cached canonical MMR proof (C4.3)
        let (
            note_root,
            total_leaves,
            mountain_height,
            mountain_siblings,
            peak_bagging_siblings,
            peak_bagging_count,
        ) = if let Some(cached) = self.get_any_cached_mmr_proof_for_leaf(leaf_index) {
            let (m_sibs, p_sibs) = Self::deserialize_mmr_siblings(cached)?;
            let root = parse_fr_from_hex(&cached.bagged_root)?;
            (
                root,
                cached.leaf_count,
                cached.mountain_height as u8,
                m_sibs,
                p_sibs,
                cached.peak_bagging_siblings.len() as u8,
            )
        } else {
            if self.mmr.leaf_count <= leaf_index as usize {
                while self.mmr.leaf_count < leaf_index as usize {
                    self.mmr.append(Fr::from(0u64));
                }
                self.mmr.append(in_cm);
            }

            let mmr_proof = self.mmr.generate_proof(leaf_index as usize);
            let note_root = self.mmr.get_root();
            let total_leaves = self.mmr.leaf_count as u64;

            let mut mountain_siblings = [Fr::from(0u64); 32];
            for (i, &s) in mmr_proof.mountain_siblings.iter().take(32).enumerate() {
                mountain_siblings[i] = s;
            }

            let mut peak_bagging_siblings = [Fr::from(0u64); 32];
            for (i, &s) in mmr_proof.peak_bagging_siblings.iter().take(32).enumerate() {
                peak_bagging_siblings[i] = s;
            }

            (
                note_root,
                total_leaves,
                mmr_proof.mountain_height as u8,
                mountain_siblings,
                peak_bagging_siblings,
                mmr_proof.peak_bagging_siblings.len() as u8,
            )
        };

        // Handle Change Note
        let (ch_val, ch_cm, ch_rho, ch_rand, change_encrypted_memo_hex) = if selected.has_change {
            let mut rng = OsRng;
            let c_rho = Fr::rand(&mut rng);
            let (change_memo, c_rand) = match self.incoming_viewing_key() {
                Ok(ivk) => match encrypt_committing_note_memo(
                    selected.change_amount,
                    c_rho,
                    input_note.asset_id as u64,
                    &ivk,
                ) {
                    Ok((memo_bytes, rand)) => (Some(hex::encode(memo_bytes)), rand),
                    Err(_) => (None, zeltra_core::derive_note_randomness(sk, c_rho)),
                },
                Err(_) => (None, zeltra_core::derive_note_randomness(sk, c_rho)),
            };

            let c_cm = note_commitment(
                selected.change_amount,
                input_note.asset_id,
                sk,
                c_rho,
                c_rand,
            );

            let c_cm_hex = hex::encode(fr_to_be_bytes(&c_cm));
            let c_rho_hex = hex::encode(fr_to_be_bytes(&c_rho));
            let c_rand_hex = hex::encode(fr_to_be_bytes(&c_rand));
            let sk_hex = hex::encode(fr_to_be_bytes(&sk));

            let change_epoch = target_change_epoch.or(input_note.epoch_id).or(Some(0));

            // Persist unconfirmed change note (2PC Phase 1)
            let change_note = WalletNote {
                commitment_hex: c_cm_hex,
                value: selected.change_amount,
                asset_id: input_note.asset_id,
                owner_key_hex: sk_hex,
                rho_hex: c_rho_hex,
                randomness_hex: c_rand_hex,
                leaf_index: None,
                leaf_count: None,
                epoch_id: change_epoch,
                merkle_path_hex: None,
                status: NoteStatus::Unconfirmed,
                created_at_secs: current_time_secs,
                session_id: Some(session_id.to_string()),
            };
            self.notes
                .insert(change_note.commitment_hex.clone(), change_note);

            (
                Fr::from(selected.change_amount),
                c_cm,
                c_rho,
                c_rand,
                change_memo,
            )
        } else {
            (
                Fr::from(0u64),
                Fr::from(0u64),
                Fr::from(0u64),
                Fr::from(0u64),
                None,
            )
        };

        // Parse EVM public input scalars
        let recipient_bytes = parse_bytes32_from_hex(recipient_evm_address, true)?;
        let recipient_fr = Fr::from_be_bytes_mod_order(&recipient_bytes);

        let quote_bytes = parse_bytes32_from_hex(quote_hash_hex, false)?;
        let (quote_hi_fr, quote_lo_fr) = zeltra_core::split_quote_hash_to_limbs(&quote_bytes);

        let contract_bytes = parse_bytes32_from_hex(contract_address_hex, true)?;
        let contract_fr = Fr::from_be_bytes_mod_order(&contract_bytes);

        let has_change_fr = if selected.has_change {
            Fr::from(1u64)
        } else {
            Fr::from(0u64)
        };

        let is_rollover_fr = if is_rollover {
            Fr::from(1u64)
        } else {
            Fr::from(0u64)
        };
        let note_epoch_id_fr = Fr::from(note_epoch_id as u64);

        let is_rollover_u64 = if is_rollover { 1u64 } else { 0u64 };
        let flags_packed_val = is_rollover_u64 + 2 * (input_note.asset_id as u64);
        let flags_packed_fr = Fr::from(flags_packed_val);

        // Compute Scope Binding Commitment (DEC-035B Stage 1 & Stage 2)
        let scope_hash = zeltra_core::compute_scope_hash(
            recipient_fr,
            Fr::from(chain_id),
            contract_fr,
            Fr::from(expiry),
        );
        let expected_binding =
            zeltra_core::compute_binding_commitment(quote_hi_fr, quote_lo_fr, scope_hash);

        // Construct PrivateNoteCircuit (DEC-032, DEC-033, DEC-035A, DEC-035B 16 Public Inputs)
        let circuit = zeltra_core::PrivateNoteCircuit {
            note_root: Some(note_root),
            leaf_count: Some(Fr::from(total_leaves)),
            input_nullifier: Some(nullifier),
            note_epoch_id: Some(note_epoch_id_fr),
            output_commitment: Some(ch_cm),
            recipient: Some(recipient_fr),
            merchant_amount: Some(Fr::from(selected.merchant_amount)),
            protocol_fee: Some(Fr::from(selected.protocol_fee)),
            execution_fee: Some(Fr::from(selected.execution_fee)),
            quote_hash_hi: Some(quote_hi_fr),
            quote_hash_lo: Some(quote_lo_fr),
            chain_id: Some(Fr::from(chain_id)),
            contract_address: Some(contract_fr),
            expiry: Some(Fr::from(expiry)),
            has_change: Some(has_change_fr),
            flags_packed: Some(flags_packed_fr),

            is_rollover: Some(is_rollover_fr),
            asset_id: Some(Fr::from(input_note.asset_id as u64)),

            input_value: Some(Fr::from(input_note.value)),
            input_owner_key: Some(sk),
            input_rho: Some(in_rho),
            input_randomness: Some(in_rand),
            input_leaf_index: Some(Fr::from(leaf_index)),
            mountain_height: Some(mountain_height),
            mountain_siblings: Some(mountain_siblings),
            peak_bagging_siblings: Some(peak_bagging_siblings),
            peak_bagging_count: Some(peak_bagging_count),

            change_value: Some(ch_val),
            change_owner_key: Some(sk),
            change_rho: Some(ch_rho),
            change_randomness: Some(ch_rand),
            expected_binding: Some(expected_binding),
        };

        // Generate Groth16 Proof
        let proof = generate_note_proof(circuit, pk)
            .map_err(|e| WalletError::CryptoError(format!("Proof generation failed: {e:?}")))?;

        // Format EVM Proof Points
        let proof_a_neg = to_evm_g1(&-proof.a);
        let proof_b = to_evm_g2(&proof.b);
        let proof_c = to_evm_g1(&proof.c);

        // Format 16 Public Inputs (DEC-035A)
        let public_inputs = vec![
            hex::encode(fr_to_be_bytes(&note_root)),
            hex::encode(fr_to_be_bytes(&Fr::from(total_leaves))),
            hex::encode(fr_to_be_bytes(&nullifier)),
            hex::encode(fr_to_be_bytes(&note_epoch_id_fr)),
            hex::encode(fr_to_be_bytes(&ch_cm)),
            hex::encode(recipient_bytes),
            hex::encode(fr_to_be_bytes(&Fr::from(selected.merchant_amount))),
            hex::encode(fr_to_be_bytes(&Fr::from(selected.protocol_fee))),
            hex::encode(fr_to_be_bytes(&Fr::from(selected.execution_fee))),
            hex::encode(fr_to_be_bytes(&quote_hi_fr)),
            hex::encode(fr_to_be_bytes(&quote_lo_fr)),
            hex::encode(fr_to_be_bytes(&Fr::from(chain_id))),
            hex::encode(contract_bytes),
            hex::encode(fr_to_be_bytes(&Fr::from(expiry))),
            hex::encode(fr_to_be_bytes(&has_change_fr)),
            hex::encode(fr_to_be_bytes(&flags_packed_fr)),
        ];

        Ok(SpendProofPayload {
            session_id: session_id.to_string(),
            input_commitment_hex: selected.input_commitment_hex.clone(),
            input_nullifier_hex: hex::encode(fr_to_be_bytes(&nullifier)),
            note_root_hex: hex::encode(fr_to_be_bytes(&note_root)),
            leaf_count: total_leaves,
            note_epoch_id,
            output_commitment_hex: hex::encode(fr_to_be_bytes(&ch_cm)),
            recipient_hex: recipient_evm_address.to_string(),
            merchant_amount: selected.merchant_amount,
            protocol_fee: selected.protocol_fee,
            execution_fee: selected.execution_fee,
            max_execution_fee,
            quote_hash_hex: quote_hash_hex.to_string(),
            chain_id,
            contract_address_hex: contract_address_hex.to_string(),
            expiry,
            has_change: selected.has_change,
            is_rollover,
            proof_a_neg_hex: hex::encode(proof_a_neg),
            proof_b_hex: hex::encode(proof_b),
            proof_c_hex: hex::encode(proof_c),
            public_inputs_hex: public_inputs,
            change_encrypted_memo_hex,
        })
    }

    /// Prepares spend proof using an explicit MmrProofResponse (C4.3).
    #[allow(clippy::too_many_arguments)]
    pub fn prepare_spend_proof_with_mmr(
        &mut self,
        selected: &SelectedSpend,
        session_id: &str,
        recipient_evm_address: &str,
        max_execution_fee: u64,
        quote_hash_hex: &str,
        chain_id: u64,
        contract_address_hex: &str,
        expiry: u64,
        target_change_epoch: Option<u32>,
        is_rollover: bool,
        mmr_proof: &MmrProofResponse,
        current_time_secs: u64,
        pk: &ark_groth16::ProvingKey<Bls12_381>,
    ) -> Result<SpendProofPayload, WalletError> {
        self.cache_mmr_proof(mmr_proof.clone());
        self.prepare_spend_proof_with_epoch(
            selected,
            session_id,
            recipient_evm_address,
            max_execution_fee,
            quote_hash_hex,
            chain_id,
            contract_address_hex,
            expiry,
            target_change_epoch,
            is_rollover,
            current_time_secs,
            pk,
        )
    }

    /// Builds an in-flight spend rollover proof (DEC-033).
    /// Spends a note from an older epoch (E-1 or earlier) while refreshing the change note into `target_epoch` (E).
    #[allow(clippy::too_many_arguments)]
    pub fn build_in_flight_spend_rollover(
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
        target_epoch: u32,
        current_time_secs: u64,
        pk: &ark_groth16::ProvingKey<Bls12_381>,
    ) -> Result<SpendProofPayload, WalletError> {
        let selected = self.select_note_for_spend(
            merchant_amount,
            protocol_fee,
            execution_fee,
            current_time_secs,
        )?;

        let session_id = format!(
            "rollover_spend_{}_{}",
            current_time_secs,
            hex::encode(rand::random::<[u8; 8]>())
        );

        self.prepare_spend_proof_with_epoch(
            &selected,
            &session_id,
            recipient_evm_address,
            max_execution_fee,
            quote_hash_hex,
            chain_id,
            contract_address_hex,
            expiry,
            Some(target_epoch),
            false,
            current_time_secs,
            pk,
        )
    }

    /// Builds a standalone note refresh proof (DEC-033).
    /// Refreshes a note into `target_epoch` without paying a merchant (merchant_amount = 0, protocol_fee = 0).
    /// The note must satisfy `input_value >= MIN_STANDALONE_ROLLOVER_THRESHOLD_USDC` ($2.00 USDC).
    /// Execution fee (gas reimbursement + 15% relayer markup) is deducted from the note value.
    #[allow(clippy::too_many_arguments)]
    pub fn build_standalone_refresh(
        &mut self,
        input_commitment_hex: &str,
        execution_fee: u64,
        max_execution_fee: u64,
        quote_hash_hex: &str,
        chain_id: u64,
        contract_address_hex: &str,
        expiry: u64,
        target_epoch: u32,
        current_time_secs: u64,
        pk: &ark_groth16::ProvingKey<Bls12_381>,
    ) -> Result<SpendProofPayload, WalletError> {
        let input_note = self
            .notes
            .get(input_commitment_hex)
            .ok_or_else(|| WalletError::NoteNotFound(input_commitment_hex.to_string()))?
            .clone();

        // Check dust threshold (DEC-033: MIN_STANDALONE_ROLLOVER_THRESHOLD_USDC = 2_000_000)
        if input_note.value < zeltra_core::fees::MIN_STANDALONE_ROLLOVER_THRESHOLD_USDC {
            return Err(WalletError::CryptoError(
                "DUST_NOTE_ROLLOVER_REJECTED: Note value is below $2.00 USDC minimum threshold"
                    .into(),
            ));
        }

        // Check sufficient balance for gas
        if input_note.value < execution_fee {
            return Err(WalletError::InsufficientBalance {
                requested: execution_fee,
                available: input_note.value,
            });
        }

        let change_amount = input_note.value - execution_fee;
        let selected = SelectedSpend {
            input_commitment_hex: input_commitment_hex.to_string(),
            input_value: input_note.value,
            second_input_commitment_hex: None,
            second_input_value: 0,
            merchant_amount: 0,
            protocol_fee: 0,
            execution_fee,
            total_required: execution_fee,
            change_amount,
            has_change: true,
        };

        let session_id = format!(
            "refresh_{}_{}",
            current_time_secs,
            hex::encode(rand::random::<[u8; 8]>())
        );

        let zero_address = "0x0000000000000000000000000000000000000000";

        self.prepare_spend_proof_with_epoch(
            &selected,
            &session_id,
            zero_address,
            max_execution_fee,
            quote_hash_hex,
            chain_id,
            contract_address_hex,
            expiry,
            Some(target_epoch),
            true, // is_rollover: true
            current_time_secs,
            pk,
        )
    }

    /// High-level client API: executes coin selection and generates spend proof payload.
    #[allow(clippy::too_many_arguments)]
    pub fn pay(
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
        pk: &ark_groth16::ProvingKey<Bls12_381>,
    ) -> Result<SpendProofPayload, WalletError> {
        let selected = self.select_note_for_spend(
            merchant_amount,
            protocol_fee,
            execution_fee,
            current_time_secs,
        )?;

        let session_id = format!(
            "spend_{}_{}",
            current_time_secs,
            hex::encode(rand::random::<[u8; 8]>())
        );

        self.prepare_spend_proof(
            &selected,
            &session_id,
            recipient_evm_address,
            max_execution_fee,
            quote_hash_hex,
            chain_id,
            contract_address_hex,
            expiry,
            current_time_secs,
            pk,
        )
    }
}
