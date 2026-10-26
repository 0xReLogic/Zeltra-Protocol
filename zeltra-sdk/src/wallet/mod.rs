//! Private Note Wallet Module (Gate E - DEC-024)
//!
//! Provides UTXO note state management, crash-safe Two-Phase Commit (2PC) lifecycle,
//! privacy-preserving coin selection, local Groth16 proof generation, and encrypted backup.

pub mod note_wallet;

pub use note_wallet::{
    build_audit_report_inner, check_nullifiers_spent_batch, get_or_init_disclosure_keys,
    recover_from_viewing_key, recover_from_viewing_key_batch, ConsolidationEstimate,
    ConsolidationRoundResult, ConsolidationSummary, DirectDepositPayload,
    JoinSplitSpendProofPayload, NoteClassification, NoteEpochStatus, NoteEvent, NoteStatus,
    PrivateNoteWallet, PrivateNoteWitness, ReadOnlyAuditorSession, RecoveredNote, RecoveryResult,
    SelectedSpend, SelfBroadcastTransactionPayload, SpendProofPayload, WalletError, WalletNote,
};
