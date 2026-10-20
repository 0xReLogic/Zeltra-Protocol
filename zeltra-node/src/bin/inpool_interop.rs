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
use zeltra_core::PrimeField;
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

        "derive_view_tag" => {
            if args.len() < 3 {
                eprintln!("Usage: inpool_interop derive_view_tag <shared_secret_hex>");
                std::process::exit(1);
            }
            let secret_clean = args[2].strip_prefix("0x").unwrap_or(&args[2]);
            let secret_bytes = hex::decode(secret_clean).expect("valid shared_secret hex");
            let secret_arr: [u8; 32] = secret_bytes.try_into().expect("32 bytes shared secret");
            let tag = zeltra_core::derive_view_tag(&secret_arr);
            println!(r#"{{"view_tag":{}}}"#, tag);
        }

        "encrypt_memo_v2" => {
            // encrypt_memo_v2 <recipient_pk_ivk_hex> <value> <rho_hex> <asset_id>
            if args.len() < 6 {
                eprintln!("Usage: inpool_interop encrypt_memo_v2 <recipient_pk_ivk_hex> <value> <rho_hex> <asset_id>");
                std::process::exit(1);
            }
            let pk_clean = args[2].strip_prefix("0x").unwrap_or(&args[2]);
            let pk_bytes: [u8; 32] = hex::decode(pk_clean)
                .expect("valid pk hex")
                .try_into()
                .expect("32 bytes pk");
            let value: u64 = args[3].parse().expect("valid value");
            let rho = parse_fr_from_hex(&args[4]).expect("valid rho hex");
            let asset_id: u64 = args[5].parse().expect("valid asset_id");

            let (memo, rand) =
                zeltra_core::encrypt_note_memo_deterministic(value, rho, asset_id, &pk_bytes)
                    .expect("memo encryption");
            let view_tag = memo[32];
            let owner_fr = if args.len() > 6 {
                parse_fr_from_hex(&args[6])
                    .unwrap_or_else(|_| zeltra_core::Fr::from_be_bytes_mod_order(&pk_bytes))
            } else {
                zeltra_core::Fr::from_be_bytes_mod_order(&pk_bytes)
            };
            let cm = zeltra_core::note_commitment(value, asset_id as u8, owner_fr, rho, rand);

            println!(
                r#"{{"memo_hex":"{}","view_tag":{},"randomness_hex":"{}","commitment_hex":"{}"}}"#,
                hex::encode(&memo),
                view_tag,
                hex::encode(zeltra_core::fr_to_be_bytes(&rand)),
                hex::encode(zeltra_core::fr_to_be_bytes(&cm))
            );
        }

        "encrypt_legacy_memo" => {
            // encrypt_legacy_memo <recipient_pk_ivk_hex> <value> <rho_hex> <asset_id> [owner_key_hex]
            if args.len() < 6 {
                eprintln!("Usage: inpool_interop encrypt_legacy_memo <recipient_pk_ivk_hex> <value> <rho_hex> <asset_id> [owner_key_hex]");
                std::process::exit(1);
            }
            let pk_clean = args[2].strip_prefix("0x").unwrap_or(&args[2]);
            let pk_bytes: [u8; 32] = hex::decode(pk_clean)
                .expect("valid pk hex")
                .try_into()
                .expect("32 bytes pk");
            let value: u64 = args[3].parse().expect("valid value");
            let rho = parse_fr_from_hex(&args[4]).expect("valid rho hex");
            let asset_id: u64 = args[5].parse().expect("valid asset_id");

            let (memo, rand) =
                zeltra_core::create_legacy_dec037_memo(value, rho, asset_id, pk_bytes);
            let owner_fr = if args.len() > 6 {
                parse_fr_from_hex(&args[6])
                    .unwrap_or_else(|_| zeltra_core::Fr::from_be_bytes_mod_order(&pk_bytes))
            } else {
                zeltra_core::Fr::from_be_bytes_mod_order(&pk_bytes)
            };
            let cm = zeltra_core::note_commitment(value, asset_id as u8, owner_fr, rho, rand);

            println!(
                r#"{{"memo_hex":"{}","randomness_hex":"{}","commitment_hex":"{}"}}"#,
                hex::encode(&memo),
                hex::encode(zeltra_core::fr_to_be_bytes(&rand)),
                hex::encode(zeltra_core::fr_to_be_bytes(&cm))
            );
        }

        "decrypt_memo_v2" => {
            // decrypt_memo_v2 <ivk_secret_hex> <memo_hex>
            if args.len() < 4 {
                eprintln!("Usage: inpool_interop decrypt_memo_v2 <ivk_secret_hex> <memo_hex>");
                std::process::exit(1);
            }
            let ivk_clean = args[2].strip_prefix("0x").unwrap_or(&args[2]);
            let ivk_bytes: [u8; 32] = hex::decode(ivk_clean)
                .expect("valid ivk hex")
                .try_into()
                .expect("32 bytes ivk");
            let ivk = zeltra_core::IncomingViewingKey::from_bytes(&ivk_bytes);
            let memo_clean = args[3].strip_prefix("0x").unwrap_or(&args[3]);
            let memo_bytes = hex::decode(memo_clean).expect("valid memo hex");

            if let Some((pt, rand)) =
                zeltra_core::decrypt_note_memo_deterministic(&memo_bytes, &ivk)
            {
                println!(
                    r#"{{"success":true,"value":{},"asset_id":{},"rho_hex":"{}","randomness_hex":"{}"}}"#,
                    pt.value,
                    pt.asset_id,
                    hex::encode(zeltra_core::fr_to_be_bytes(&pt.rho)),
                    hex::encode(zeltra_core::fr_to_be_bytes(&rand))
                );
            } else {
                println!(r#"{{"success":false}}"#);
            }
        }

        "fast_scan_memos" => {
            // fast_scan_memos <recipient_seed> <events_json> [asset_id]
            if args.len() < 4 {
                eprintln!("Usage: inpool_interop fast_scan_memos <recipient_seed> <events_json> [asset_id]");
                std::process::exit(1);
            }
            let seed_bytes = hex::decode(&args[2]).unwrap_or_else(|_| args[2].as_bytes().to_vec());
            let mut seed_arr = [0u8; 32];
            for (i, b) in seed_bytes.iter().take(32).enumerate() {
                seed_arr[i] = *b;
            }
            let mut wallet = PrivateNoteWallet::new(&seed_arr);
            let events_str = if args[3].starts_with('@') {
                fs::read_to_string(&args[3][1..]).expect("readable events file")
            } else if std::path::Path::new(&args[3]).is_file() {
                fs::read_to_string(&args[3]).expect("readable events file")
            } else {
                args[3].clone()
            };
            let events_raw: Vec<serde_json::Value> =
                serde_json::from_str(&events_str).expect("valid events json");
            let mut events = Vec::new();
            for ev in events_raw {
                let leaf_index = ev["leaf_index"].as_u64().unwrap_or(0);
                let cm_hex = ev["commitment_hex"].as_str().unwrap_or("").to_string();
                let memo_hex = ev["encrypted_memo_hex"].as_str().unwrap_or("");
                let memo_clean = memo_hex.strip_prefix("0x").unwrap_or(memo_hex);
                let memo_bytes = hex::decode(memo_clean).unwrap_or_default();
                events.push(NoteEvent {
                    leaf_index,
                    commitment_hex: cm_hex,
                    new_mmr_root_hex: "0xroot".to_string(),
                    leaf_count: leaf_index + 1,
                    encrypted_memo: memo_bytes,
                    block_number: Some(100),
                });
            }
            let asset_id: u8 = if args.len() > 4 {
                args[4].parse().unwrap_or(1)
            } else {
                1
            };

            let rec = wallet.scan_memos_fast(&events).expect("fast scan");
            let wallet_bal = wallet.get_balance(asset_id).unwrap_or(0);
            println!(
                r#"{{"recovered_count":{},"total_unspent_balance":{},"wallet_balance":{}}}"#,
                rec.recovered_notes.len(),
                rec.total_unspent_balance,
                wallet_bal
            );
        }

        "test_null_branch_witness" => {
            // DEC-046: Test null-branch witness injection resistance
            // Usage: inpool_interop test_null_branch_witness <field> [value]
            if args.len() < 3 {
                eprintln!("Usage: inpool_interop test_null_branch_witness <field> [value]");
                std::process::exit(1);
            }
            let field = args[2].as_str();
            let mut circuit = zeltra_core::create_dummy_joinsplit_circuit();

            if field == "baseline" {
                match zeltra_core::joinsplit_circuit::check_joinsplit_circuit_satisfaction(circuit) {
                    Ok(true) => {
                        println!(r#"{{"satisfied":true,"status":"SATISFIED","field":"baseline"}}"#);
                        std::process::exit(0);
                    }
                    Ok(false) => {
                        eprintln!("Constraint system not satisfied: baseline circuit failed");
                        println!(r#"{{"satisfied":false,"status":"REJECTED","field":"baseline"}}"#);
                        std::process::exit(1);
                    }
                    Err(e) => {
                        eprintln!("Constraint synthesis error: {e}");
                        println!(r#"{{"satisfied":false,"status":"ERROR","error":"{e}"}}"#);
                        std::process::exit(1);
                    }
                }
            }

            if args.len() < 4 {
                eprintln!("Usage: inpool_interop test_null_branch_witness <field> <value>");
                std::process::exit(1);
            }

            let val_str = &args[3];
            let tamper_val = parse_fr_flexible(val_str).unwrap_or_else(|e| {
                eprintln!("Failed to parse scalar: {e}");
                std::process::exit(1);
            });

            let is_diff_inv = field == "diff_inv";
            if is_diff_inv {
                zeltra_core::joinsplit_circuit::set_tamper_diff_inv(Some(tamper_val));
            } else {
                match field {
                    "in2_rho" => circuit.in2_rho = Some(tamper_val),
                    "in2_owner" | "in2_owner_key" => circuit.in2_owner_key = Some(tamper_val),
                    "in2_randomness" | "in2_rand" => circuit.in2_randomness = Some(tamper_val),
                    "in2_leaf_index" | "in2_index" => circuit.in2_leaf_index = Some(tamper_val),
                    "out1_owner" | "out1_owner_key" => circuit.out1_owner_key = Some(tamper_val),
                    "out1_rho" => circuit.out1_rho = Some(tamper_val),
                    "out1_randomness" | "out1_rand" => circuit.out1_randomness = Some(tamper_val),
                    "out2_owner" | "out2_owner_key" => circuit.out2_owner_key = Some(tamper_val),
                    "out2_rho" => circuit.out2_rho = Some(tamper_val),
                    "out2_randomness" | "out2_rand" => circuit.out2_randomness = Some(tamper_val),
                    _ => {
                        eprintln!("Unknown tamper field: {}", field);
                        std::process::exit(1);
                    }
                }
            }

            let res = zeltra_core::joinsplit_circuit::check_joinsplit_circuit_satisfaction(circuit);
            if is_diff_inv {
                zeltra_core::joinsplit_circuit::set_tamper_diff_inv(None);
            }

            match res {
                Ok(true) => {
                    println!(r#"{{"satisfied":true,"status":"UNEXPECTEDLY_SATISFIED","field":"{}"}}"#, field);
                    std::process::exit(0);
                }
                Ok(false) => {
                    eprintln!("Constraint system not satisfied: DEC-046 null-branch constraint rejected tampered witness for field '{}'", field);
                    println!(r#"{{"satisfied":false,"status":"REJECTED","field":"{}"}}"#, field);
                    std::process::exit(1);
                }
                Err(e) => {
                    eprintln!("Constraint synthesis error: {e}");
                    println!(r#"{{"satisfied":false,"status":"ERROR","error":"{e}"}}"#);
                    std::process::exit(1);
                }
            }
        }

        _ => {
            eprintln!("Unknown command: {}", command);
            std::process::exit(1);
        }
    }

    Ok(())
}

fn parse_fr_flexible(s: &str) -> Result<zeltra_core::Fr, String> {
    let s = s.trim();
    if s == "-1" {
        return Ok(-zeltra_core::Fr::from(1u64));
    }
    if let Ok(v) = s.parse::<u64>() {
        return Ok(zeltra_core::Fr::from(v));
    }
    let clean_str = s.strip_prefix("0x").unwrap_or(s);
    let clean = if !clean_str.len().is_multiple_of(2) {
        format!("0{}", clean_str)
    } else {
        clean_str.to_string()
    };
    let bytes = hex::decode(&clean).map_err(|e| format!("Invalid hex: {e}"))?;
    if bytes.len() <= 32 {
        let mut padded = [0u8; 32];
        padded[32 - bytes.len()..].copy_from_slice(&bytes);
        zeltra_core::fr_from_be_bytes(&padded)
            .ok_or_else(|| "Non-canonical field scalar (>= r)".to_string())
    } else {
        Err(format!("Hex too long: {} bytes", bytes.len()))
    }
}
