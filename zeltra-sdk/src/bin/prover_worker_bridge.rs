//! Real Groth16 Worker Prover Bridge for Automated Testing (DEC-041)

use ark_serialize::CanonicalSerialize;
use rand::rngs::StdRng;
use rand::SeedableRng;
use serde::{Deserialize, Serialize};
use std::io::{self, Read};

use zeltra_core::{
    get_or_init_joinsplit_circuit_keys, get_or_init_note_circuit_keys,
    selective_disclosure_circuit::setup_disclosure_circuit_params,
};
use zeltra_sdk::{
    prove_joinsplit_with_pk, prove_private_note_with_pk, prove_selective_disclosure_with_pk,
    JoinSplitWitnessInput, PrivateNoteWitnessInput, SelectiveDisclosureWitnessInput,
    WasmProverResult,
};

#[derive(Deserialize)]
struct ProverRequest {
    action: String,
    #[serde(default)]
    private_note_witness: Option<PrivateNoteWitnessInput>,
    #[serde(default)]
    joinsplit_witness: Option<JoinSplitWitnessInput>,
    #[serde(default)]
    disclosure_witness: Option<SelectiveDisclosureWitnessInput>,
}

#[derive(Serialize)]
struct ProverResponse {
    status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<WasmProverResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error_type: Option<String>,
}

fn main() {
    let mut input_str = String::new();
    if let Err(e) = io::stdin().read_to_string(&mut input_str) {
        let resp = ProverResponse {
            status: "ERROR".to_string(),
            result: None,
            error: Some(format!("Failed to read stdin: {e}")),
            error_type: Some("IoError".to_string()),
        };
        println!("{}", serde_json::to_string(&resp).unwrap());
        return;
    }

    let req: ProverRequest = match serde_json::from_str(&input_str) {
        Ok(r) => r,
        Err(e) => {
            let resp = ProverResponse {
                status: "ERROR".to_string(),
                result: None,
                error: Some(format!("Failed to parse request JSON: {e}")),
                error_type: Some("JsonParseError".to_string()),
            };
            println!("{}", serde_json::to_string(&resp).unwrap());
            return;
        }
    };

    match req.action.as_str() {
        "PROVE_PRIVATE_NOTE" => {
            let Some(witness) = req.private_note_witness else {
                let resp = ProverResponse {
                    status: "ERROR".to_string(),
                    result: None,
                    error: Some("Missing private_note_witness".to_string()),
                    error_type: Some("MissingInput".to_string()),
                };
                println!("{}", serde_json::to_string(&resp).unwrap());
                return;
            };

            let keys = get_or_init_note_circuit_keys();
            match prove_private_note_with_pk(&witness, &keys.proving_key) {
                Ok(res) => {
                    let resp = ProverResponse {
                        status: "SUCCESS".to_string(),
                        result: Some(res),
                        error: None,
                        error_type: None,
                    };
                    println!("{}", serde_json::to_string(&resp).unwrap());
                }
                Err(err) => {
                    let resp = ProverResponse {
                        status: "ERROR".to_string(),
                        result: None,
                        error: Some(err.message),
                        error_type: Some(err.error_type),
                    };
                    println!("{}", serde_json::to_string(&resp).unwrap());
                }
            }
        }
        "PROVE_JOINSPLIT" => {
            let Some(witness) = req.joinsplit_witness else {
                let resp = ProverResponse {
                    status: "ERROR".to_string(),
                    result: None,
                    error: Some("Missing joinsplit_witness".to_string()),
                    error_type: Some("MissingInput".to_string()),
                };
                println!("{}", serde_json::to_string(&resp).unwrap());
                return;
            };

            let keys = get_or_init_joinsplit_circuit_keys();
            match prove_joinsplit_with_pk(&witness, &keys.proving_key) {
                Ok(res) => {
                    let resp = ProverResponse {
                        status: "SUCCESS".to_string(),
                        result: Some(res),
                        error: None,
                        error_type: None,
                    };
                    println!("{}", serde_json::to_string(&resp).unwrap());
                }
                Err(err) => {
                    let resp = ProverResponse {
                        status: "ERROR".to_string(),
                        result: None,
                        error: Some(err.message),
                        error_type: Some(err.error_type),
                    };
                    println!("{}", serde_json::to_string(&resp).unwrap());
                }
            }
        }
        "PROVE_DISCLOSURE" => {
            let Some(witness) = req.disclosure_witness else {
                let resp = ProverResponse {
                    status: "ERROR".to_string(),
                    result: None,
                    error: Some("Missing disclosure_witness".to_string()),
                    error_type: Some("MissingInput".to_string()),
                };
                println!("{}", serde_json::to_string(&resp).unwrap());
                return;
            };

            let mut rng = StdRng::seed_from_u64(0xdec041);
            let (pk, _) = setup_disclosure_circuit_params(&mut rng);
            match prove_selective_disclosure_with_pk(&witness, &pk) {
                Ok(res) => {
                    let resp = ProverResponse {
                        status: "SUCCESS".to_string(),
                        result: Some(res),
                        error: None,
                        error_type: None,
                    };
                    println!("{}", serde_json::to_string(&resp).unwrap());
                }
                Err(err) => {
                    let resp = ProverResponse {
                        status: "ERROR".to_string(),
                        result: None,
                        error: Some(err.message),
                        error_type: Some(err.error_type),
                    };
                    println!("{}", serde_json::to_string(&resp).unwrap());
                }
            }
        }
        "EXPORT_PK_BYTES" => {
            let keys = get_or_init_note_circuit_keys();
            let mut pk_bytes = Vec::new();
            keys.proving_key
                .serialize_compressed(&mut pk_bytes)
                .unwrap();
            println!("{}", hex::encode(pk_bytes));
        }
        other => {
            let resp = ProverResponse {
                status: "ERROR".to_string(),
                result: None,
                error: Some(format!("Unknown action: {other}")),
                error_type: Some("InvalidAction".to_string()),
            };
            println!("{}", serde_json::to_string(&resp).unwrap());
        }
    }
}
