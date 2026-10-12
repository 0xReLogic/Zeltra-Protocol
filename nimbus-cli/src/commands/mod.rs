//! Command handler modules (DEC-038A)

pub mod zk;

use crate::cli::Commands;
use rand::Rng;

/// Dispatch command to appropriate handler
pub fn handle_command<R: Rng>(command: Commands, rng: &mut R) {
    match command {
        Commands::GenerateWallet { out_file } => {
            zk::generate_wallet(out_file.as_deref(), rng);
        }

        Commands::CreateDepositNote {
            amount,
            spending_key,
        } => {
            zk::create_deposit_note(amount, spending_key.as_deref(), rng);
        }

        Commands::InspectMmr {
            commitments,
            rpc_url,
        } => {
            zk::inspect_mmr(commitments.as_deref(), rpc_url.as_deref());
        }

        Commands::VerifyProof {
            circuit,
            proof_a_neg,
            proof_b,
            proof_c,
            public_inputs,
            proof_file,
            inputs_file,
        } => {
            zk::verify_proof(
                &circuit,
                proof_a_neg.as_deref(),
                proof_b.as_deref(),
                proof_c.as_deref(),
                public_inputs.as_deref(),
                proof_file.as_deref(),
                inputs_file.as_deref(),
            );
        }
    }
}
