//! Command handler modules

pub mod keys;
pub mod blind;
pub mod threshold;

use crate::cli::Commands;
use rand::Rng;

/// Dispatch command to appropriate handler
pub fn handle_command<R: Rng>(command: Commands, rng: &mut R) {
    match command {
        Commands::GenerateKeys => {
            keys::generate_keys(rng);
        }
        
        Commands::SplitKey { sk, threshold, total } => {
            keys::split_key(sk, threshold, total, rng);
        }
        
        Commands::Aggregate { indices, signatures } => {
            threshold::aggregate(indices, signatures);
        }
        
        Commands::Blind { message } => {
            blind::blind(message, rng);
        }
        
        Commands::Sign { blinded, sk } => {
            blind::sign(blinded, sk, rng);
        }
        
        Commands::VerifyMasked { blinded, com_k, masked_sig } => {
            blind::verify_masked(blinded, com_k, masked_sig);
        }
        
        Commands::Unmask { masked_sig, r, k } => {
            blind::unmask(masked_sig, r, k);
        }
        
        Commands::Verify { message, sig, pk } => {
            blind::verify(message, sig, pk);
        }
    }
}
