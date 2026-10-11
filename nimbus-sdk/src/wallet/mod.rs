//! Private Note Wallet Module (Gate E - DEC-024)
//!
//! Provides UTXO note state management, crash-safe Two-Phase Commit (2PC) lifecycle,
//! privacy-preserving coin selection, local Groth16 proof generation, and encrypted backup.

pub mod note_wallet;

pub use note_wallet::{
    check_nullifiers_spent_batch, recover_from_viewing_key, recover_from_viewing_key_batch,
    ConsolidationEstimate, ConsolidationRoundResult, ConsolidationSummary,
    JoinSplitSpendProofPayload, NoteEpochStatus, NoteEvent, NoteStatus, PrivateNoteWallet,
    PrivateNoteWitness, RecoveredNote, RecoveryResult, SelectedSpend, SpendProofPayload,
    WalletError, WalletNote,
};
