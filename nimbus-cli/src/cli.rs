//! CLI argument definitions using Clap

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "nimbus-cli")]
#[command(about = "Nimbus Protocol CLI Wallet & Cryptography Utility", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Generate a new random Issuer keypair (sk and pk)
    GenerateKeys,

    /// Split a secret key into n shares with threshold t using Shamir Secret Sharing
    SplitKey {
        /// Secret key in hex
        #[arg(short, long)]
        sk: String,

        /// Threshold t
        #[arg(short, long, default_value_t = 3)]
        threshold: usize,

        /// Total shares n
        #[arg(short = 'n', long, default_value_t = 5)]
        total: usize,
    },

    /// Aggregate partial signatures from a threshold of guardians
    Aggregate {
        /// Comma-separated list of guardian indices (e.g. "1,2,3")
        #[arg(short, long)]
        indices: String,

        /// Comma-separated list of partial signatures in hex
        #[arg(short, long)]
        signatures: String,
    },

    /// Blind a message (e.g. ephemeral public key) using a random blinding factor
    Blind {
        /// The message to blind (e.g. "ephemeral_public_key_abc123")
        #[arg(short, long)]
        message: String,
    },

    /// Sign a blinded message (masked signing) using the issuer secret key
    Sign {
        /// Blinded message in hex
        #[arg(short, long)]
        blinded: String,

        /// Issuer secret key in hex
        #[arg(short, long)]
        sk: String,
    },

    /// Verify a masked signature (client check)
    VerifyMasked {
        /// Blinded message in hex
        #[arg(short, long)]
        blinded: String,

        /// Masking key commitment in hex
        #[arg(short, long)]
        com_k: String,

        /// Masked signature in hex
        #[arg(short, long)]
        masked_sig: String,
    },

    /// Unmask a masked signature once masking key is revealed on-chain
    Unmask {
        /// Masked signature in hex
        #[arg(short, long)]
        masked_sig: String,

        /// Blinding factor in hex
        #[arg(short, long)]
        r: String,

        /// Masking key in hex
        #[arg(short, long)]
        k: String,
    },

    /// Verify the final unblinded signature
    Verify {
        /// The original message string
        #[arg(short, long)]
        message: String,

        /// Unmasked signature in hex
        #[arg(short, long)]
        sig: String,

        /// Issuer public key in hex
        #[arg(short, long)]
        pk: String,
    },
}
