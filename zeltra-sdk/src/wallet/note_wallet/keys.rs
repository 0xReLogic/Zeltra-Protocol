use ark_bls12_381::Fr;
use zeltra_core::{
    derive_nullifier_key, derive_owner_key, FullViewingKey,
    IncomingViewingKey, OutcomingViewingKey, ZeltraShieldedAddress,
};

use super::compliance::ReadOnlyAuditorSession;
use super::types::{parse_fr_from_hex, WalletError};
use super::PrivateNoteWallet;

impl PrivateNoteWallet {
    /// Spending key as Fr
    pub fn spending_key(&self) -> Result<Fr, WalletError> {
        parse_fr_from_hex(&self.spending_key_hex)
    }

    /// Nullifier key as Fr
    pub fn nullifier_key(&self) -> Result<Fr, WalletError> {
        parse_fr_from_hex(&self.nullifier_key_hex)
    }

    /// Owner key as Fr (derived from spending key, DEC-043A)
    pub fn owner_key(&self) -> Result<Fr, WalletError> {
        let sk = self.spending_key()?;
        Ok(derive_owner_key(sk))
    }

    /// Shielded address (ZSA) representing this wallet (DEC-043A)
    pub fn shielded_address(&self) -> Result<ZeltraShieldedAddress, WalletError> {
        let sk = self.spending_key()?;
        let pk = self.incoming_viewing_public_key()?;
        Ok(ZeltraShieldedAddress::from_keys(sk, pk))
    }

    /// Returns the Incoming Viewing Key (IVK) (DEC-037).
    pub fn incoming_viewing_key(&self) -> Result<IncomingViewingKey, WalletError> {
        if self.incoming_viewing_key_hex.is_empty() {
            return Err(WalletError::CryptoError(
                "Incoming viewing key not initialized".into(),
            ));
        }
        let bytes = hex::decode(&self.incoming_viewing_key_hex)
            .map_err(|e| WalletError::InvalidHex(e.to_string()))?;
        if bytes.len() != 32 {
            return Err(WalletError::CryptoError(
                "Invalid viewing key length".into(),
            ));
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes);
        Ok(IncomingViewingKey::from_bytes(&arr))
    }

    /// Returns the public key associated with the Incoming Viewing Key (32 bytes).
    pub fn incoming_viewing_public_key(&self) -> Result<[u8; 32], WalletError> {
        self.incoming_viewing_key().map(|ivk| ivk.public_key())
    }

    /// Returns hex-encoded incoming viewing public key.
    pub fn incoming_viewing_public_key_hex(&self) -> &str {
        &self.incoming_viewing_public_key_hex
    }

    /// Returns the Outcoming Viewing Key (OVK) (DEC-037).
    pub fn outcoming_viewing_key(&self) -> Result<OutcomingViewingKey, WalletError> {
        if self.outcoming_viewing_key_hex.is_empty() {
            return Err(WalletError::CryptoError(
                "Outcoming viewing key not initialized".into(),
            ));
        }
        let bytes = hex::decode(&self.outcoming_viewing_key_hex)
            .map_err(|e| WalletError::InvalidHex(e.to_string()))?;
        if bytes.len() != 32 {
            return Err(WalletError::CryptoError(
                "Invalid viewing key length".into(),
            ));
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes);
        Ok(OutcomingViewingKey::from_bytes(&arr))
    }

    /// Exports Full Viewing Key (FVK) for Tier 3 read-only compliance audit delegation (DEC-040 Section 3).
    pub fn export_full_viewing_key(&self) -> Result<FullViewingKey, WalletError> {
        let ivk = self.incoming_viewing_key()?;
        let ovk = self.outcoming_viewing_key()?;
        let sk = self.spending_key()?;
        let nk = derive_nullifier_key(sk);
        Ok(FullViewingKey::new(
            ivk.owner_key(),
            ivk.secret_bytes(),
            *ovk.as_bytes(),
            nk,
        ))
    }

    /// Creates a read-only auditor session from a FullViewingKey (DEC-040B Section 5).
    pub fn create_auditor_session(
        fvk: FullViewingKey,
    ) -> Result<ReadOnlyAuditorSession, WalletError> {
        Ok(ReadOnlyAuditorSession::new(fvk))
    }

    /// Opens a read-only auditor session derived from this wallet's viewing keys.
    pub fn open_auditor_session(&self) -> Result<ReadOnlyAuditorSession, WalletError> {
        let fvk = self.export_full_viewing_key()?;
        Ok(ReadOnlyAuditorSession::new(fvk))
    }

}
