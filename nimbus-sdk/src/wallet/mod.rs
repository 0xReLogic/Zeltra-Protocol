//! Private Note Wallet Module (Gate E - DEC-024)
//!
//! Provides UTXO note state management, crash-safe Two-Phase Commit (2PC) lifecycle,
//! privacy-preserving coin selection, local Groth16 proof generation, and encrypted backup.

pub mod note_wallet;

pub use note_wallet::{
    JoinSplitSpendProofPayload, NoteStatus, PrivateNoteWallet, SelectedSpend, SpendProofPayload,
    WalletError, WalletNote,
};
