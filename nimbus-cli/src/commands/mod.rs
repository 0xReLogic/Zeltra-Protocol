//! Command handler modules (DEC-038A)

pub mod zk;

use crate::cli::Commands;
use rand::Rng;

/// Dispatch command to appropriate handler
pub fn handle_command<R: Rng>(command: Commands, rng: &mut R) {
    match command {
        Commands::GenerateWallet => {
            zk::generate_wallet(rng);
        }

        Commands::CreateDepositNote { amount } => {
            zk::create_deposit_note(amount, rng);
        }

        Commands::InspectMmr { commitments } => {
            zk::inspect_mmr(&commitments);
        }
    }
}
