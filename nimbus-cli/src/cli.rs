//! CLI argument definitions using Clap (DEC-038A)

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "nimbus-cli")]
#[command(about = "Zeltra Protocol ZK-UTXO CLI Wallet & Cryptography Utility", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Generate a new random spending secret and viewing key for ZK-UTXO wallet
    GenerateWallet,

    /// Create a private note and note commitment for direct deposit (DEC-038B)
    CreateDepositNote {
        /// Amount in micro-USDC (6 decimals, e.g. 10000000 for 10 USDC)
        #[arg(short, long)]
        amount: u64,
    },

    /// Inspect Merkle Mountain Range peaks and root for a set of note commitments
    InspectMmr {
        /// Comma-separated list of 32-byte leaf commitment hexes
        #[arg(short, long)]
        commitments: String,
    },
}
