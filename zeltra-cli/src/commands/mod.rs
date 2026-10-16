//! Command handler modules (DEC-038A / DEC-040C)

pub mod compliance;
pub mod zk;

use crate::cli::Commands;
use rand::Rng;

/// Dispatch command to appropriate handler
pub async fn handle_command<R: Rng>(command: Commands, rng: &mut R) {
    match command {
        Commands::GenerateWallet { out_file } => {
            zk::generate_wallet(out_file.as_deref(), rng);
        }

        Commands::CreateDepositNote {
            amount,
            asset,
            recipient,
            spending_key,
        } => {
            zk::create_deposit_note(
                amount,
                &asset,
                recipient.as_deref(),
                spending_key.as_deref(),
                rng,
            );
        }

        Commands::SpendPrivateNote {
            amount,
            asset,
            to,
            wallet_file,
        } => {
            zk::spend_private_note(amount, &asset, &to, wallet_file.as_deref(), rng);
        }

        Commands::Balance { wallet_file } => {
            zk::display_balance(wallet_file.as_deref());
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

        Commands::ExportAudit {
            format,
            asset,
            from,
            to,
            wallet_file,
            out,
        } => {
            if let Err(e) = compliance::handle_export_audit(
                &format,
                &asset,
                from.as_deref(),
                to.as_deref(),
                wallet_file.as_deref(),
                &out,
            )
            .await
            {
                eprintln!("\x1b[31mError:\x1b[0m {}", e);
                std::process::exit(1);
            }
        }

        Commands::ExportCexProof {
            commitment,
            verifier,
            account_uid,
            spent_tx,
            recipient,
            wallet_file,
            out,
        } => {
            if let Err(e) = compliance::handle_export_cex_proof(
                &commitment,
                &verifier,
                &account_uid,
                &spent_tx,
                &recipient,
                wallet_file.as_deref(),
                &out,
            )
            .await
            {
                eprintln!("\x1b[31mError:\x1b[0m {}", e);
                std::process::exit(1);
            }
        }

        Commands::VerifyCexProof {
            file,
            expected_verifier,
            rpc,
            contract,
        } => {
            if let Err(e) = compliance::handle_verify_cex_proof(
                &file,
                &expected_verifier,
                rpc.as_deref(),
                contract.as_deref(),
            )
            .await
            {
                eprintln!("\x1b[31mError:\x1b[0m {}", e);
                std::process::exit(1);
            }
        }

        Commands::ExportViewingKey {
            key_type,
            wallet_file,
            out,
        } => {
            if let Err(e) =
                compliance::handle_export_viewing_key(&key_type, wallet_file.as_deref(), &out).await
            {
                eprintln!("\x1b[31mError:\x1b[0m {}", e);
                std::process::exit(1);
            }
        }
    }
}
