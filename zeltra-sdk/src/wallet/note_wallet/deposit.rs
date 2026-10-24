use ark_bls12_381::Fr;
use ark_ff::UniformRand;
use rand::rngs::OsRng;
use zeltra_core::{
    derive_viewing_keys, encrypt_note_memo_optional, fr_to_be_bytes, note_commitment,
};

use super::types::*;
use super::PrivateNoteWallet;

impl PrivateNoteWallet {
    /// Prepares payload for on-chain `deposit_direct_note` transaction (DEC-038C), defaulting to USDC (asset_id = 1).
    pub fn prepare_direct_deposit(
        &mut self,
        amount: u64,
        recipient_key: Option<Fr>,
    ) -> Result<DirectDepositPayload, WalletError> {
        self.prepare_direct_deposit_asset(amount, 1 /* USDC */, recipient_key)
    }

    /// Prepares payload for on-chain `deposit_direct_note` transaction for a specific asset (DEC-034C).
    pub fn prepare_direct_deposit_asset(
        &mut self,
        amount: u64,
        asset_id: u8,
        recipient_key: Option<Fr>,
    ) -> Result<DirectDepositPayload, WalletError> {
        if asset_id > 1 {
            return Err(WalletError::UnsupportedAssetId(asset_id));
        }
        let spending_key = match recipient_key {
            Some(key) => key,
            None => self.spending_key()?,
        };
        let mut rng = OsRng;
        let rho = Fr::rand(&mut rng);
        let r = zeltra_core::derive_note_randomness(spending_key, rho);
        let cm = note_commitment(amount, asset_id, spending_key, rho, r);

        let ivk = match recipient_key {
            Some(key) => {
                let sk_bytes = fr_to_be_bytes(&key);
                let (ivk, _) = derive_viewing_keys(&sk_bytes);
                ivk
            }
            None => self.incoming_viewing_key()?,
        };

        let memo =
            encrypt_note_memo_optional(amount, rho, asset_id as u64, &ivk).unwrap_or_else(Vec::new);

        self.stage_pending_deposit_note_asset(cm, amount, asset_id, rho, r)?;

        Ok(DirectDepositPayload {
            amount,
            commitment_bytes: fr_to_be_bytes(&cm),
            encrypted_memo: memo,
        })
    }

    /// Stages a pending direct deposit note locally with NoteStatus::PendingDeposit (DEC-038C), defaulting to USDC (asset_id = 1).
    pub fn stage_pending_deposit_note(
        &mut self,
        cm: Fr,
        amount: u64,
        rho: Fr,
        rand_val: Fr,
    ) -> Result<WalletNote, WalletError> {
        self.stage_pending_deposit_note_asset(cm, amount, 1 /* USDC */, rho, rand_val)
    }

    /// Stages a pending direct deposit note locally with NoteStatus::PendingDeposit for a specific asset (DEC-034C).
    pub fn stage_pending_deposit_note_asset(
        &mut self,
        cm: Fr,
        amount: u64,
        asset_id: u8,
        rho: Fr,
        rand_val: Fr,
    ) -> Result<WalletNote, WalletError> {
        let cm_hex = hex::encode(fr_to_be_bytes(&cm));
        let sk = self.spending_key()?;
        let sk_hex = hex::encode(fr_to_be_bytes(&sk));
        let rho_hex = hex::encode(fr_to_be_bytes(&rho));
        let rand_hex = hex::encode(fr_to_be_bytes(&rand_val));

        let now_secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let note = WalletNote {
            commitment_hex: cm_hex.clone(),
            value: amount,
            asset_id,
            owner_key_hex: sk_hex,
            rho_hex,
            randomness_hex: rand_hex,
            leaf_index: None,
            leaf_count: None,
            epoch_id: Some(0),
            merkle_path_hex: None,
            status: NoteStatus::PendingDeposit,
            classification: NoteClassification::ActiveSpendable,
            created_at_secs: now_secs,
            session_id: None,
        };

        self.notes.insert(cm_hex, note.clone());
        Ok(note)
    }

    /// Finds a pending note mutably by commitment bytes (DEC-038C).
    pub fn find_pending_note_mut(&mut self, commitment: &[u8; 32]) -> Option<&mut WalletNote> {
        let cm_hex = hex::encode(commitment);
        if self.notes.contains_key(&cm_hex) {
            self.notes.get_mut(&cm_hex)
        } else {
            let cm_0x = format!("0x{}", cm_hex);
            self.notes.get_mut(&cm_0x)
        }
    }

    /// Handles an on-chain NoteCommitmentAppended event (DEC-038C).
    /// Atomically mutates status of matching pending note from PendingDeposit to Unspent (2PC State Invariant).
    pub fn on_commitment_appended(
        &mut self,
        leaf_index: u64,
        commitment: [u8; 32],
        root: [u8; 32],
    ) -> Result<bool, WalletError> {
        let cm_hex = hex::encode(commitment);
        let cm_0x = format!("0x{}", cm_hex);
        let root_hex = hex::encode(root);
        self.current_merkle_root_hex = Some(format!("0x{}", root_hex));

        let note = if let Some(n) = self.notes.get_mut(&cm_hex) {
            Some(n)
        } else {
            self.notes.get_mut(&cm_0x)
        };

        if let Some(note) = note {
            if note.status == NoteStatus::PendingDeposit || note.status == NoteStatus::Unconfirmed {
                note.leaf_index = Some(leaf_index);
                note.status = NoteStatus::Unspent;
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Creates a new note for an on-chain deposit, defaulting to USDC (asset_id = 1).
    ///
    /// Returns the created `WalletNote` and its `commitment_hex` ready to be passed
    /// to `Zeltra::deposit(amount, commitment)`.
    pub fn create_deposit_note(
        &mut self,
        amount: u64,
        current_time_secs: u64,
    ) -> Result<(WalletNote, String), WalletError> {
        self.create_deposit_note_asset(amount, 1 /* USDC */, current_time_secs)
    }

    /// Creates a new note for an on-chain deposit with a specific asset (DEC-034C).
    pub fn create_deposit_note_asset(
        &mut self,
        amount: u64,
        asset_id: u8,
        current_time_secs: u64,
    ) -> Result<(WalletNote, String), WalletError> {
        if asset_id > 1 {
            return Err(WalletError::UnsupportedAssetId(asset_id));
        }
        let mut rng = OsRng;
        let sk = self.spending_key()?;
        let rho = Fr::rand(&mut rng);
        let rand_val = Fr::rand(&mut rng);

        let cm = note_commitment(amount, asset_id, sk, rho, rand_val);
        let cm_hex = hex::encode(fr_to_be_bytes(&cm));
        let sk_hex = hex::encode(fr_to_be_bytes(&sk));
        let rho_hex = hex::encode(fr_to_be_bytes(&rho));
        let rand_hex = hex::encode(fr_to_be_bytes(&rand_val));

        let note = WalletNote {
            commitment_hex: cm_hex.clone(),
            value: amount,
            asset_id,
            owner_key_hex: sk_hex,
            rho_hex,
            randomness_hex: rand_hex,
            leaf_index: None,
            leaf_count: None,
            epoch_id: Some(0),
            merkle_path_hex: None,
            status: NoteStatus::Unconfirmed,
            classification: NoteClassification::ActiveSpendable,
            created_at_secs: current_time_secs,
            session_id: None,
        };

        self.notes.insert(cm_hex.clone(), note.clone());
        Ok((note, cm_hex))
    }

    /// Creates an unconfirmed note for a deposit transaction with a specific epoch (DEC-033), defaulting to USDC (asset_id = 1).
    pub fn create_deposit_note_with_epoch(
        &mut self,
        amount: u64,
        epoch_id: u32,
        current_time_secs: u64,
    ) -> Result<(WalletNote, String), WalletError> {
        self.create_deposit_note_with_epoch_asset(
            amount,
            1, /* USDC */
            epoch_id,
            current_time_secs,
        )
    }

    /// Creates an unconfirmed note for a deposit transaction with a specific epoch and asset (DEC-034C).
    pub fn create_deposit_note_with_epoch_asset(
        &mut self,
        amount: u64,
        asset_id: u8,
        epoch_id: u32,
        current_time_secs: u64,
    ) -> Result<(WalletNote, String), WalletError> {
        if asset_id > 1 {
            return Err(WalletError::UnsupportedAssetId(asset_id));
        }
        let mut rng = OsRng;
        let sk = self.spending_key()?;
        let rho = Fr::rand(&mut rng);
        let rand_val = Fr::rand(&mut rng);

        let cm = note_commitment(amount, asset_id, sk, rho, rand_val);
        let cm_hex = hex::encode(fr_to_be_bytes(&cm));
        let sk_hex = hex::encode(fr_to_be_bytes(&sk));
        let rho_hex = hex::encode(fr_to_be_bytes(&rho));
        let rand_hex = hex::encode(fr_to_be_bytes(&rand_val));

        let note = WalletNote {
            commitment_hex: cm_hex.clone(),
            value: amount,
            asset_id,
            owner_key_hex: sk_hex,
            rho_hex,
            randomness_hex: rand_hex,
            leaf_index: None,
            leaf_count: None,
            epoch_id: Some(epoch_id),
            merkle_path_hex: None,
            status: NoteStatus::Unconfirmed,
            classification: NoteClassification::ActiveSpendable,
            created_at_secs: current_time_secs,
            session_id: None,
        };

        self.notes.insert(cm_hex.clone(), note.clone());
        Ok((note, cm_hex))
    }

    /// Checks the epoch status of a note relative to current on-chain epoch (DEC-033).
    pub fn check_note_epoch_status(
        &self,
        note: &WalletNote,
        current_epoch: u32,
    ) -> NoteEpochStatus {
        note.check_epoch_status(current_epoch)
    }

    /// Confirms a deposit once mined on-chain, associating its Merkle leaf index and path.
    pub fn confirm_deposit(
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

        let cm_fr = parse_fr_from_hex(commitment_hex)?;

        let clean = commitment_hex.strip_prefix("0x").unwrap_or(commitment_hex);
        let with_0x = format!("0x{}", clean);
        let note = if self.notes.contains_key(commitment_hex) {
            self.notes.get_mut(commitment_hex)
        } else if self.notes.contains_key(clean) {
            self.notes.get_mut(clean)
        } else {
            self.notes.get_mut(&with_0x)
        }
        .ok_or_else(|| WalletError::NoteNotFound(commitment_hex.to_string()))?;

        if self.mmr.leaf_count <= leaf_index as usize {
            while self.mmr.leaf_count < leaf_index as usize {
                self.mmr.append(Fr::from(0u64));
            }
            self.mmr.append(cm_fr);
        }

        note.leaf_index = Some(leaf_index);
        note.leaf_count = Some(self.mmr.leaf_count as u64);
        note.merkle_path_hex = Some(merkle_path);
        note.status = NoteStatus::Unspent;
        self.current_merkle_root_hex = Some(root_hex.to_string());

        Ok(())
    }
}
