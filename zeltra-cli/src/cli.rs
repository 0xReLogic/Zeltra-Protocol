//! CLI argument definitions using Clap (DEC-038A / DEC-038C)

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "zeltra-cli")]
#[command(about = "Zeltra Protocol ZK-UTXO CLI Wallet & Cryptography Utility", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Generate a new random spending secret, full viewing key, and incoming viewing key (DEC-038C)
    GenerateWallet {
        /// Optional path to export wallet credentials as JSON
        #[arg(short, long)]
        out_file: Option<String>,
    },

    /// Create a private note and note commitment with encrypted memo for direct deposit (DEC-034C / DEC-038C)
    CreateDepositNote {
        /// Amount in native token units (Micro-USDC for USDC, Gwei for WETH)
        #[arg(short, long)]
        amount: u64,

        /// Asset type: 'usdc' (default), 'eth', or 'weth'
        #[arg(short = 'A', long, default_value = "usdc")]
        asset: String,

        /// Optional recipient address hex
        #[arg(short, long)]
        recipient: Option<String>,

        /// Optional spending key hex (0x-prefixed 32 bytes)
        #[arg(short, long)]
        spending_key: Option<String>,
    },

    /// Spend a private note with ZK proof generation (DEC-034C)
    SpendPrivateNote {
        /// Amount to spend in native units (Micro-USDC for USDC, Gwei for WETH)
        #[arg(short, long)]
        amount: u64,

        /// Asset type: 'usdc' (default) or 'weth'
        #[arg(short = 'A', long, default_value = "usdc")]
        asset: String,

        /// Recipient address hex (0x-prefixed 20 bytes)
        #[arg(short, long)]
        to: String,

        /// Optional wallet file path containing credentials
        #[arg(short, long)]
        wallet_file: Option<String>,
    },

    /// Display partitioned shielded balances for WETH and USDC (DEC-034C)
    Balance {
        /// Optional path to wallet credentials JSON file
        #[arg(short, long)]
        wallet_file: Option<String>,
    },

    /// Inspect Merkle Mountain Range peaks and root for note commitments (DEC-038C)
    InspectMmr {
        /// Comma-separated list of 32-byte leaf commitment hexes
        #[arg(short, long)]
        commitments: Option<String>,

        /// Optional RPC URL or relayer URL to inspect
        #[arg(short, long)]
        rpc_url: Option<String>,
    },

    /// Verify a Groth16 proof locally using Arkworks verifier (DEC-038C)
    VerifyProof {
        /// Circuit type: 'joinsplit' (default) or 'private_note'
        #[arg(short, long, default_value = "joinsplit")]
        circuit: String,

        /// Proof-A-neg (G1, 128 bytes EVM hex)
        #[arg(long)]
        proof_a_neg: Option<String>,

        /// Proof-B (G2, 256 bytes EVM hex)
        #[arg(long)]
        proof_b: Option<String>,

        /// Proof-C (G1, 128 bytes EVM hex)
        #[arg(long)]
        proof_c: Option<String>,

        /// Comma-separated list of 32-byte public input hexes
        #[arg(long)]
        public_inputs: Option<String>,

        /// Path to proof JSON file containing proof_a_neg, proof_b, proof_c, public_inputs
        #[arg(long)]
        proof_file: Option<String>,

        /// Path to public inputs JSON file (Vec<String>)
        #[arg(long)]
        inputs_file: Option<String>,
    },

    /// Export double-entry audit trail report (DEC-040B / DEC-040C) in JSON or CSV
    ExportAudit {
        /// Format: 'json' or 'csv'
        #[arg(short = 'f', long, default_value = "json")]
        format: String,

        /// Asset filter: 'all', 'usdc', or 'weth'
        #[arg(short = 'A', long, default_value = "all")]
        asset: String,

        /// Starting date filter (YYYY-MM-DD)
        #[arg(long)]
        from: Option<String>,

        /// Ending date filter (YYYY-MM-DD)
        #[arg(long)]
        to: Option<String>,

        /// Optional wallet file path containing notes or master credentials
        #[arg(short = 'w', long)]
        wallet_file: Option<String>,

        /// Output file path (written with strict 0600 permissions)
        #[arg(short = 'o', long)]
        out: String,
    },

    /// Export single-note CEX selective disclosure proof (DEC-040C Tier 1)
    ExportCexProof {
        /// 32-byte hex commitment of the note to disclose
        #[arg(short = 'c', long)]
        commitment: String,

        /// Verifier identifier (e.g. "BINANCE_KYC_DESK")
        #[arg(short = 'v', long)]
        verifier: String,

        /// Exchange account UID (e.g. "USER-98124712")
        #[arg(short = 'u', long)]
        account_uid: String,

        /// On-chain withdrawal / spend transaction hash
        #[arg(long)]
        spent_tx: String,

        /// User deposit recipient address on the exchange
        #[arg(short = 'r', long)]
        recipient: String,

        /// Optional wallet file path containing credentials
        #[arg(short = 'w', long)]
        wallet_file: Option<String>,

        /// Output proof JSON file path
        #[arg(short = 'o', long)]
        out: String,
    },

    /// Verify a CEX selective disclosure proof independently (DEC-040C)
    VerifyCexProof {
        /// Path to zeltra_cex_proof_v1.json file
        #[arg(short = 'f', long)]
        file: String,

        /// Expected verifier identifier (e.g. "BINANCE_KYC_DESK")
        #[arg(short = 'v', long)]
        expected_verifier: String,

        /// Arbitrum RPC URL (Arbitrum Sepolia/One)
        #[arg(long)]
        rpc: Option<String>,

        /// Stylus contract address override (optional)
        #[arg(long)]
        contract: Option<String>,
    },

    /// Export viewing keys for compliance delegation (DEC-040A / DEC-040C)
    ExportViewingKey {
        /// Viewing key type: 'ivk', 'ovk', or 'fvk'
        #[arg(short = 't', long, default_value = "fvk")]
        key_type: String,

        /// Optional wallet file path containing credentials
        #[arg(short = 'w', long)]
        wallet_file: Option<String>,

        /// Output file path
        #[arg(short = 'o', long)]
        out: String,
    },
}
