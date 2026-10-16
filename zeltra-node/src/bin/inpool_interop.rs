//! Zeltra In-Pool Transfer & Relayer Interop CLI Test Harness (DEC-043C).
//!
//! Provides CLI interface for independent cross-language testing (Python Real-Case Suite):
//! 1. Wallet creation, ZSA export, and direct deposit staging.
//! 2. Client Groth16 Mode 2 transfer proof generation with dual 96B memos.
//! 3. Relayer preflight verification (Strict 3-Mode, fail-closed memo validation, sanctions filtering).
//! 4. Merchant trial decryption recovery and balance synchronization.
//! 5. Groth16 JoinSplit proof verification.

use axum::extract::State;
use axum::Json;
use std::env;
use std::fs;
use std::path::PathBuf;
use zeltra_core::from_evm_proof;
use zeltra_core::joinsplit_circuit::{get_or_init_joinsplit_circuit_keys, verify_joinsplit_proof};
use zeltra_node::database::Database;
use zeltra_node::dto::{JoinSplitSpendRequest, JoinSplitSpendResponse};
use zeltra_node::handlers::spend::handle_spend_joinsplit;
use zeltra_node::state::AppState;
use zeltra_sdk::wallet::note_wallet::{parse_fr_from_hex, NoteEvent, PrivateNoteWallet};

fn get_temp_db_path() -> PathBuf {
    let rand_id = rand::random::<u64>();
    env::temp_dir().join(format!(
        "zeltra_interop_{}_{}.db",
        std::process::id(),
        rand_id
    ))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: inpool_interop <command> [args...]");
        std::process::exit(1);
    }

    let command = &args[1];
    match command.as_str() {
        "create_wallet" => {
            if args.len() < 3 {
                eprintln!("Usage: inpool_interop create_wallet <seed_hex_or_string>");
                std::process::exit(1);
            }
            let seed_bytes = hex::decode(&args[2]).unwrap_or_else(|_| args[2].as_bytes().to_vec());
            let mut seed_arr = [0u8; 32];
            for (i, b) in seed_bytes.iter().take(32).enumerate() {
                seed_arr[i] = *b;
            }
            let wallet = PrivateNoteWallet::new(&seed_arr);
            let zsa = wallet.shielded_address().expect("shielded address");
            let ivk = wallet.incoming_viewing_key().expect("ivk");

            println!(
                r#"{{"spending_key":"{}","owner_key":"{}","pk_ivk":"{}","ivk_secret":"{}","zsa_bech32m":"{}","zsa_hex":"{}"}}"#,
                hex::encode(zeltra_core::fr_to_be_bytes(&wallet.spending_key().unwrap())),
                hex::encode(zeltra_core::fr_to_be_bytes(&zsa.owner_key)),
                hex::encode(zsa.incoming_viewing_public_key),
                hex::encode(ivk.secret_bytes()),
                zsa.to_bech32m().unwrap(),
                zsa.to_hex()
            );
        }

        "prepare_transfer" => {
            // prepare_transfer <sender_seed_hex> <recipient_zsa> <deposit_amount> <transfer_amount> <asset_id> <max_fee>
            if args.len() < 8 {
                eprintln!("Usage: inpool_interop prepare_transfer <sender_seed> <recipient_zsa> <deposit_amount> <transfer_amount> <asset_id> <max_fee>");
                std::process::exit(1);
            }
            let seed_bytes = hex::decode(&args[2]).unwrap_or_else(|_| args[2].as_bytes().to_vec());
            let mut seed_arr = [0u8; 32];
            for (i, b) in seed_bytes.iter().take(32).enumerate() {
                seed_arr[i] = *b;
            }
            let mut wallet = PrivateNoteWallet::new(&seed_arr);

            let recipient_zsa_str = &args[3];
            let deposit_amount: u64 = args[4].parse().expect("valid deposit_amount");
            let transfer_amount: u64 = args[5].parse().expect("valid transfer_amount");
            let asset_id: u8 = args[6].parse().expect("valid asset_id");
            let max_fee: u64 = args[7].parse().expect("valid max_fee");

            // Setup deposit note
            if deposit_amount > 0 {
                let (_, cm) = wallet
                    .create_deposit_note_asset(deposit_amount, asset_id, 1000)
                    .expect("create deposit note");
                wallet
                    .confirm_deposit(&cm, 0, vec!["0x04".to_string()], "0xroot")
                    .expect("confirm deposit");
            }

            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let expiry = now + 3600;
            let keys = get_or_init_joinsplit_circuit_keys();
            let res = wallet.prepare_shielded_transfer_proof(
                recipient_zsa_str,
                transfer_amount,
                asset_id,
                max_fee,
                "0x1111111111111111111111111111111111111111111111111111111111111111",
                421614,
                "0x2222222222222222222222222222222222222222",
                expiry,
                1000,
                &keys.proving_key,
            );

            match res {
                Ok(payload) => {
                    let wallet_file = format!("/tmp/zeltra_wallet_{}.json", args[2]);
                    if let Ok(mut f) = fs::File::create(&wallet_file) {
                        let _ = serde_json::to_writer(&mut f, &wallet);
                    }
                    let out_json = serde_json::to_string(&payload).unwrap();
                    println!(
                        r#"{{"success":true,"payload":{},"initial_balance":{}}}"#,
                        out_json,
                        wallet.get_balance(asset_id).unwrap_or(0)
                    );
                }
                Err(e) => {
                    let err_json = serde_json::to_string(&format!("{e:?}")).unwrap();
                    println!(r#"{{"success":false,"error":{err_json}}}"#);
                }
            }
        }

        "commit_spend" => {
            // commit_spend <sender_seed> <session_id> <nullifier> <has_change> <asset_id>
            if args.len() < 7 {
                eprintln!("Usage: inpool_interop commit_spend <sender_seed> <session_id> <nullifier> <has_change> <asset_id> <deposit_amount>");
                std::process::exit(1);
            }
            let seed_bytes = hex::decode(&args[2]).unwrap_or_else(|_| args[2].as_bytes().to_vec());
            let mut seed_arr = [0u8; 32];
            for (i, b) in seed_bytes.iter().take(32).enumerate() {
                seed_arr[i] = *b;
            }
            let wallet_file = format!("/tmp/zeltra_wallet_{}.json", args[2]);
            let mut wallet: PrivateNoteWallet = if let Ok(f) = fs::File::open(&wallet_file) {
                serde_json::from_reader(f).unwrap_or_else(|_| PrivateNoteWallet::new(&seed_arr))
            } else {
                PrivateNoteWallet::new(&seed_arr)
            };
            let session_id = &args[3];
            let nullifier = &args[4];
            let has_change: bool = args[5].parse().unwrap_or(false);
            let asset_id: u8 = args[6].parse().unwrap_or(1);

            let commit_res = if has_change {
                wallet.commit_spend(
                    session_id,
                    nullifier,
                    1005,
                    Some(2),
                    Some(vec!["0x04".to_string()]),
                    Some("0xnew_root"),
                )
            } else {
                wallet.commit_spend(session_id, nullifier, 1005, None, None, None)
            };
            let _ = fs::remove_file(&wallet_file);

            match commit_res {
                Ok(_) => {
                    let new_bal = wallet.get_balance(asset_id).unwrap_or(0);
                    println!(r#"{{"success":true,"new_balance":{new_bal}}}"#);
                }
                Err(e) => {
                    let err_json = serde_json::to_string(&format!("{e:?}")).unwrap();
                    println!(r#"{{"success":false,"error":{err_json}}}"#);
                }
            }
        }

        "relayer_preflight" => {
            // relayer_preflight <json_request_payload>
            if args.len() < 3 {
                eprintln!("Usage: inpool_interop relayer_preflight <request_json>");
                std::process::exit(1);
            }
            let req_json = &args[2];
            let req: JoinSplitSpendRequest = match serde_json::from_str(req_json) {
                Ok(r) => r,
                Err(e) => {
                    println!(
                        r#"{{"accepted":false,"status":"REJECTED","message":"JSON deserialization error: {e}"}}"#
                    );
                    return Ok(());
                }
            };

            let db_path = get_temp_db_path();
            let db = Database::new(&db_path).await.expect("temp db");
            let state = AppState::new(db, None).await;

            let response: Json<JoinSplitSpendResponse> =
                handle_spend_joinsplit(State(state), Json(req)).await;

            let accepted = response.0.status == "ACCEPTED" || response.0.status == "QUEUED";
            let resp_json = serde_json::to_string(&response.0).unwrap();
            println!(r#"{{"accepted":{},"response":{}}}"#, accepted, resp_json);

            let _ = fs::remove_file(&db_path);
        }

        "recover_notes" => {
            // recover_notes <recipient_seed> <leaf_index> <commitment_hex> <memo_hex> <asset_id>
            if args.len() < 6 {
                eprintln!("Usage: inpool_interop recover_notes <recipient_seed> <leaf_index> <commitment_hex> <memo_hex> [asset_id]");
                std::process::exit(1);
            }
            let seed_bytes = hex::decode(&args[2]).unwrap_or_else(|_| args[2].as_bytes().to_vec());
            let mut seed_arr = [0u8; 32];
            for (i, b) in seed_bytes.iter().take(32).enumerate() {
                seed_arr[i] = *b;
            }
            let mut wallet = PrivateNoteWallet::new(&seed_arr);

            let leaf_index: u64 = args[3].parse().expect("valid leaf index");
            let cm_hex = args[4].clone();
            let memo_hex = args[5].clone();
            let asset_id: u8 = if args.len() > 6 {
                args[6].parse().unwrap_or(1)
            } else {
                1
            };

            let memo_bytes = hex::decode(memo_hex.strip_prefix("0x").unwrap_or(&memo_hex))
                .expect("valid memo hex");

            let event = NoteEvent {
                leaf_index,
                commitment_hex: cm_hex,
                new_mmr_root_hex: "0xroot".into(),
                leaf_count: leaf_index + 1,
                encrypted_memo: memo_bytes,
                block_number: Some(100),
            };

            let rec = wallet
                .recover_notes_batch(&[event], |_| Ok(vec![false]))
                .expect("recover batch");

            let wallet_bal = wallet.get_balance(asset_id).unwrap_or(0);
            println!(
                r#"{{"recovered_count":{},"total_unspent_balance":{},"wallet_balance":{}}}"#,
                rec.recovered_notes.len(),
                rec.total_unspent_balance,
                wallet_bal
            );
        }

        "verify_proof" => {
            // verify_proof <proof_a_neg_hex> <proof_b_hex> <proof_c_hex> <public_inputs_json_array>
            if args.len() < 6 {
                eprintln!("Usage: inpool_interop verify_proof <proof_a_neg> <proof_b> <proof_c> <public_inputs_json>");
                std::process::exit(1);
            }
            let a_neg_bytes = hex::decode(&args[2]).expect("proof_a hex");
            let b_bytes = hex::decode(&args[3]).expect("proof_b hex");
            let c_bytes = hex::decode(&args[4]).expect("proof_c hex");
            let pis_strs: Vec<String> = serde_json::from_str(&args[5]).expect("pis json array");

            let a_neg_fixed: [u8; 128] = a_neg_bytes.try_into().expect("128 bytes a_neg");
            let b_fixed: [u8; 256] = b_bytes.try_into().expect("256 bytes b");
            let c_fixed: [u8; 128] = c_bytes.try_into().expect("128 bytes c");

            let proof =
                from_evm_proof(&a_neg_fixed, &b_fixed, &c_fixed).expect("valid proof points");

            let mut pis = Vec::new();
            for s in &pis_strs {
                pis.push(parse_fr_from_hex(s).expect("canonical fr pi"));
            }

            let keys = get_or_init_joinsplit_circuit_keys();
            let verified = verify_joinsplit_proof(&keys.verifying_key, &proof, &pis)
                .expect("verification execution");

            println!(r#"{{"verified":{}}}"#, verified);
        }

        _ => {
            eprintln!("Unknown command: {}", command);
            std::process::exit(1);
        }
    }

    Ok(())
}
