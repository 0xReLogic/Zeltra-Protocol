use super::types::*;
use super::PrivateNoteWallet;

impl PrivateNoteWallet {
    /// Acquires a lease on a note for an active spend session (Crash Safety Phase 1).
    pub fn reserve_note(
        &mut self,
        commitment_hex: &str,
        session_id: &str,
        current_time_secs: u64,
        ttl_secs: u64,
    ) -> Result<(), WalletError> {
        let note = self
            .notes
            .get_mut(commitment_hex)
            .ok_or_else(|| WalletError::NoteNotFound(commitment_hex.to_string()))?;

        match &note.status {
            NoteStatus::Unspent => {}
            NoteStatus::Reserved {
                lease_expiry_secs, ..
            } => {
                if *lease_expiry_secs > current_time_secs {
                    return Err(WalletError::NoteAlreadyReserved(commitment_hex.to_string()));
                }
            }
            _ => return Err(WalletError::NoteNotSpendable(commitment_hex.to_string())),
        }

        note.status = NoteStatus::Reserved {
            session_id: session_id.to_string(),
            lease_expiry_secs: current_time_secs + ttl_secs,
        };
        note.session_id = Some(session_id.to_string());

        Ok(())
    }


    /// Finalizes transaction receipt (2PC Phase 2 - Commit).
    ///
    /// Transitions input note to `Spent` and change note to `Unspent`
    /// once confirmed on-chain with its Merkle index & path.
    pub fn commit_spend(
        &mut self,
        session_id: &str,
        nullifier_hex: &str,
        current_time_secs: u64,
        change_leaf_index: Option<u64>,
        change_merkle_path: Option<Vec<String>>,
        new_merkle_root: Option<&str>,
    ) -> Result<(), WalletError> {
        // 1. Mark input note(s) as Spent
        let mut found_input = false;
        for note in self.notes.values_mut() {
            if note.session_id.as_deref() == Some(session_id) {
                if let NoteStatus::Reserved { .. } = &note.status {
                    note.status = NoteStatus::Spent {
                        nullifier_hex: nullifier_hex.to_string(),
                        spent_at_secs: current_time_secs,
                    };
                    found_input = true;
                }
            }
        }

        if !found_input {
            return Err(WalletError::NoteNotFound(format!(
                "Session {session_id} input note"
            )));
        }

        // 2. Promote Change Note to Unspent (if present and witness provided)
        if let (Some(idx), Some(path)) = (change_leaf_index, change_merkle_path) {
            for note in self.notes.values_mut() {
                if note.session_id.as_deref() == Some(session_id)
                    && note.status == NoteStatus::Unconfirmed
                {
                    note.leaf_index = Some(idx);
                    note.merkle_path_hex = Some(path);
                    note.status = NoteStatus::Unspent;
                    break;
                }
            }
        }

        if let Some(root) = new_merkle_root {
            self.current_merkle_root_hex = Some(root.to_string());
        }

        Ok(())
    }

    /// Finalizes a 2-note JoinSplit spend transaction receipt (DEC-030).
    ///
    /// Transitions input note 1 to `Spent(nullifier_1)` and input note 2 (if present)
    /// to `Spent(nullifier_2)`, then promotes the change note to `Unspent`.
    #[allow(clippy::too_many_arguments)]
    pub fn commit_joinsplit_spend(
        &mut self,
        session_id: &str,
        nullifier_1_hex: &str,
        nullifier_2_hex: Option<&str>,
        current_time_secs: u64,
        change_leaf_index: Option<u64>,
        change_merkle_path: Option<Vec<String>>,
        new_merkle_root: Option<&str>,
    ) -> Result<(), WalletError> {
        let mut reserved_cms: Vec<String> = Vec::new();
        for (cm, note) in &self.notes {
            if note.session_id.as_deref() == Some(session_id) {
                if let NoteStatus::Reserved { .. } = &note.status {
                    reserved_cms.push(cm.clone());
                }
            }
        }

        if reserved_cms.is_empty() {
            return Err(WalletError::NoteNotFound(format!(
                "Session {session_id} input notes"
            )));
        }

        // Mark note 1 as Spent with nullifier 1
        if let Some(note1) = self.notes.get_mut(&reserved_cms[0]) {
            note1.status = NoteStatus::Spent {
                nullifier_hex: nullifier_1_hex.to_string(),
                spent_at_secs: current_time_secs,
            };
        }

        // If there is a note 2, mark as Spent with nullifier 2
        if reserved_cms.len() > 1 {
            let n2_hex = nullifier_2_hex.unwrap_or(nullifier_1_hex);
            if let Some(note2) = self.notes.get_mut(&reserved_cms[1]) {
                note2.status = NoteStatus::Spent {
                    nullifier_hex: n2_hex.to_string(),
                    spent_at_secs: current_time_secs,
                };
            }
        }

        // Promote Change Note to Unspent (if present and witness provided)
        if let (Some(idx), Some(path)) = (change_leaf_index, change_merkle_path) {
            for note in self.notes.values_mut() {
                if note.session_id.as_deref() == Some(session_id)
                    && note.status == NoteStatus::Unconfirmed
                {
                    note.leaf_index = Some(idx);
                    note.merkle_path_hex = Some(path);
                    note.status = NoteStatus::Unspent;
                    break;
                }
            }
        }

        if let Some(root) = new_merkle_root {
            self.current_merkle_root_hex = Some(root.to_string());
        }

        Ok(())
    }

    /// Aborts an active session on failure or timeout (2PC Phase 2 - Rollback).
    ///
    /// Restores input note to `Unspent` and discards unconfirmed change notes.
    pub fn rollback_spend(&mut self, session_id: &str) -> Result<(), WalletError> {
        let mut found_input = false;
        let mut change_cm_to_remove = None;

        for (cm, note) in self.notes.iter_mut() {
            if note.session_id.as_deref() == Some(session_id) {
                match &note.status {
                    NoteStatus::Reserved { .. } => {
                        note.status = NoteStatus::Unspent;
                        note.session_id = None;
                        found_input = true;
                    }
                    NoteStatus::Unconfirmed => {
                        change_cm_to_remove = Some(cm.clone());
                    }
                    _ => {}
                }
            }
        }

        if let Some(cm) = change_cm_to_remove {
            self.notes.remove(&cm);
        }

        if !found_input {
            return Err(WalletError::NoteNotFound(format!("Session {session_id}")));
        }

        Ok(())
    }

    /// Automatically detects expired note reservations and restores them to `Unspent` (DEC-027).
    ///
    /// Cleans up any unconfirmed change notes created for those sessions, restoring strict self-custody.
    /// Returns the number of notes reclaimed.
    pub fn reclaim_expired_reservations(&mut self, current_time_secs: u64) -> usize {
        let mut expired_sessions = Vec::new();

        for note in self.notes.values() {
            if let NoteStatus::Reserved {
                lease_expiry_secs,
                session_id,
            } = &note.status
            {
                if *lease_expiry_secs <= current_time_secs {
                    expired_sessions.push(session_id.clone());
                }
            }
        }

        let mut reclaimed_count = 0;
        for session_id in expired_sessions {
            if self.rollback_spend(&session_id).is_ok() {
                reclaimed_count += 1;
            }
        }

        reclaimed_count
    }

    /// Explicitly syncs a mempool expiration for a session, restoring input note to `Unspent` (DEC-027).
    pub fn sync_mempool_expiry(&mut self, session_id: &str) -> Result<(), WalletError> {
        self.rollback_spend(session_id)
    }

}
