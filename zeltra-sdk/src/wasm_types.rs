//! WASM-compatible type wrappers and DTOs for client-side Groth16 proving (DEC-041)

use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

// ═══════════════════════════════════════════════════════════════════════════
// Legacy types (kept for backward compatibility)
// ═══════════════════════════════════════════════════════════════════════════

#[wasm_bindgen]
pub struct BlindedOutput {
    blinded_message: String,
    blinding_factor: String,
}

#[wasm_bindgen]
impl BlindedOutput {
    #[wasm_bindgen(getter)]
    pub fn blinded_message(&self) -> String {
        self.blinded_message.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn blinding_factor(&self) -> String {
        self.blinding_factor.clone()
    }
}

impl BlindedOutput {
    pub fn new(blinded_message: String, blinding_factor: String) -> Self {
        Self {
            blinded_message,
            blinding_factor,
        }
    }
}


// ═══════════════════════════════════════════════════════════════════════════
// DEC-041: Groth16 Prover Output DTOs
// ═══════════════════════════════════════════════════════════════════════════

/// Canonical result structure returned by WASM Groth16 provers to JavaScript.
/// Serialized to both camelCase and snake_case aliases for JS/TS ergonomic compatibility.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WasmProverResult {
    /// EVM-compatible -A G1 point (128 hex chars / 64 bytes)
    #[serde(alias = "proof_a", alias = "proof_a_neg_hex")]
    pub proof_a: String,

    /// EVM-compatible B G2 point (256 hex chars / 128 bytes)
    #[serde(alias = "proof_b", alias = "proof_b_hex")]
    pub proof_b: String,

    /// EVM-compatible C G1 point (128 hex chars / 64 bytes)
    #[serde(alias = "proof_c", alias = "proof_c_hex")]
    pub proof_c: String,

    /// Canonical Big-Endian 32-byte public inputs as hex strings (16, 19, or 5 PIs)
    #[serde(alias = "public_inputs")]
    pub public_inputs: Vec<String>,

    /// Elapsed time in milliseconds spent during witness synthesis and proof generation
    #[serde(alias = "proving_time_ms")]
    pub proving_time_ms: u64,
}

impl WasmProverResult {
    pub fn new(
        proof_a: String,
        proof_b: String,
        proof_c: String,
        public_inputs: Vec<String>,
        proving_time_ms: u64,
    ) -> Self {
        Self {
            proof_a,
            proof_b,
            proof_c,
            public_inputs,
            proving_time_ms,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// DEC-041: Witness Inputs DTOs
// ═══════════════════════════════════════════════════════════════════════════

/// Witness parameter assignments for 1-in-1-out PrivateNoteCircuit (16 Public Inputs, DEC-035A/B)
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrivateNoteWitnessInput {
    // ── MMR & Tree Context ──
    #[serde(alias = "note_root_hex")]
    pub note_root_hex: String,
    #[serde(alias = "leaf_count")]
    pub leaf_count: u64,
    #[serde(default, alias = "note_epoch_id")]
    pub note_epoch_id: Option<u64>,

    // ── Payment & Target Scope Context ──
    #[serde(alias = "recipient_hex")]
    pub recipient_hex: String,
    #[serde(alias = "merchant_amount")]
    pub merchant_amount: u64,
    #[serde(alias = "protocol_fee")]
    pub protocol_fee: u64,
    #[serde(alias = "execution_fee")]
    pub execution_fee: u64,
    #[serde(alias = "quote_hash_hi_hex")]
    pub quote_hash_hi_hex: String,
    #[serde(alias = "quote_hash_lo_hex")]
    pub quote_hash_lo_hex: String,
    #[serde(alias = "chain_id")]
    pub chain_id: u64,
    #[serde(alias = "contract_address_hex")]
    pub contract_address_hex: String,
    #[serde(alias = "expiry")]
    pub expiry: u64,
    #[serde(alias = "has_change")]
    pub has_change: bool,
    #[serde(default, alias = "flags_packed")]
    pub flags_packed: Option<u64>,
    #[serde(default, alias = "is_rollover")]
    pub is_rollover: Option<bool>,
    #[serde(default = "default_asset_id", alias = "asset_id")]
    pub asset_id: u8,

    // ── Input Note Witness ──
    #[serde(alias = "input_value")]
    pub input_value: u64,
    #[serde(alias = "input_spending_key_hex")]
    pub input_spending_key_hex: String,
    #[serde(alias = "input_rho_hex")]
    pub input_rho_hex: String,
    #[serde(alias = "input_randomness_hex")]
    pub input_randomness_hex: String,
    #[serde(alias = "input_leaf_index")]
    pub input_leaf_index: u64,
    #[serde(alias = "mountain_height")]
    pub mountain_height: u8,
    #[serde(alias = "mountain_siblings_hex")]
    pub mountain_siblings_hex: Vec<String>,
    #[serde(alias = "peak_bagging_siblings_hex")]
    pub peak_bagging_siblings_hex: Vec<String>,

    // ── Change Note Witness ──
    #[serde(default, alias = "change_value")]
    pub change_value: Option<u64>,
    #[serde(default, alias = "change_owner_key_hex")]
    pub change_owner_key_hex: Option<String>,
    #[serde(default, alias = "change_rho_hex")]
    pub change_rho_hex: Option<String>,
    #[serde(default, alias = "change_randomness_hex")]
    pub change_randomness_hex: Option<String>,

    // ── Scope Binding Override (Optional) ──
    #[serde(default, alias = "expected_binding_hex")]
    pub expected_binding_hex: Option<String>,
}

fn default_asset_id() -> u8 {
    1 // USDC
}

/// Witness parameter assignments for Universal 2-in-2-out JoinSplitCircuit (19 Public Inputs, DEC-036A)
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JoinSplitWitnessInput {
    // ── MMR & Context ──
    #[serde(alias = "note_root_hex")]
    pub note_root_hex: String,
    #[serde(alias = "leaf_count")]
    pub leaf_count: u64,
    #[serde(alias = "recipient_hex")]
    pub recipient_hex: String,
    #[serde(alias = "merchant_amount")]
    pub merchant_amount: u64,
    #[serde(alias = "protocol_fee")]
    pub protocol_fee: u64,
    #[serde(alias = "execution_fee")]
    pub execution_fee: u64,
    #[serde(alias = "quote_hash_hi_hex")]
    pub quote_hash_hi_hex: String,
    #[serde(alias = "quote_hash_lo_hex")]
    pub quote_hash_lo_hex: String,
    #[serde(alias = "chain_id")]
    pub chain_id: u64,
    #[serde(alias = "contract_address_hex")]
    pub contract_address_hex: String,
    #[serde(alias = "expiry")]
    pub expiry: u64,
    #[serde(default, alias = "flags_packed")]
    pub flags_packed: Option<u64>,
    #[serde(default, alias = "is_rollover")]
    pub is_rollover: Option<bool>,
    #[serde(default = "default_asset_id", alias = "asset_id")]
    pub asset_id: u8,

    // ── Input Note 1 ──
    #[serde(alias = "in1_value")]
    pub in1_value: u64,
    #[serde(alias = "in1_spending_key_hex")]
    pub in1_spending_key_hex: String,
    #[serde(alias = "in1_rho_hex")]
    pub in1_rho_hex: String,
    #[serde(alias = "in1_randomness_hex")]
    pub in1_randomness_hex: String,
    #[serde(alias = "in1_leaf_index")]
    pub in1_leaf_index: u64,
    #[serde(alias = "in1_mountain_height")]
    pub in1_mountain_height: u8,
    #[serde(alias = "in1_mountain_siblings_hex")]
    pub in1_mountain_siblings_hex: Vec<String>,
    #[serde(default, alias = "in1_epoch_id")]
    pub in1_epoch_id: Option<u64>,

    // ── Input Note 2 ──
    #[serde(alias = "in2_is_dummy")]
    pub in2_is_dummy: bool,
    #[serde(default, alias = "in2_value")]
    pub in2_value: Option<u64>,
    #[serde(default, alias = "in2_spending_key_hex")]
    pub in2_spending_key_hex: Option<String>,
    #[serde(default, alias = "in2_rho_hex")]
    pub in2_rho_hex: Option<String>,
    #[serde(default, alias = "in2_randomness_hex")]
    pub in2_randomness_hex: Option<String>,
    #[serde(default, alias = "in2_leaf_index")]
    pub in2_leaf_index: Option<u64>,
    #[serde(default, alias = "in2_mountain_height")]
    pub in2_mountain_height: Option<u8>,
    #[serde(default, alias = "in2_mountain_siblings_hex")]
    pub in2_mountain_siblings_hex: Option<Vec<String>>,
    #[serde(default, alias = "in2_session_nonce_hex")]
    pub in2_session_nonce_hex: Option<String>,
    #[serde(default, alias = "in2_epoch_id")]
    pub in2_epoch_id: Option<u64>,

    // ── MMR Peaks ──
    #[serde(alias = "mmr_peaks_hex")]
    pub mmr_peaks_hex: Vec<String>,

    // ── Output 1 ──
    #[serde(alias = "out1_value")]
    pub out1_value: u64,
    #[serde(alias = "out1_owner_key_hex")]
    pub out1_owner_key_hex: String,
    #[serde(alias = "out1_rho_hex")]
    pub out1_rho_hex: String,
    #[serde(alias = "out1_randomness_hex")]
    pub out1_randomness_hex: String,

    // ── Output 2 ──
    #[serde(alias = "out2_value")]
    pub out2_value: u64,
    #[serde(alias = "out2_owner_key_hex")]
    pub out2_owner_key_hex: String,
    #[serde(alias = "out2_rho_hex")]
    pub out2_rho_hex: String,
    #[serde(alias = "out2_randomness_hex")]
    pub out2_randomness_hex: String,

    // ── Flags & Binding ──
    #[serde(alias = "has_change_1")]
    pub has_change_1: bool,
    #[serde(alias = "has_change_2")]
    pub has_change_2: bool,
    #[serde(default, alias = "expected_binding_hex")]
    pub expected_binding_hex: Option<String>,
}

/// Witness parameter assignments for CEX Selective Disclosure Circuit (5 Public Inputs, DEC-040A)
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectiveDisclosureWitnessInput {
    #[serde(alias = "spending_key_hex")]
    pub spending_key_hex: String,
    #[serde(alias = "epoch_id")]
    pub epoch_id: u32,
    #[serde(alias = "owner_key_hex")]
    pub owner_key_hex: String,
    #[serde(alias = "rho_hex")]
    pub rho_hex: String,
    #[serde(alias = "nullifier_hex")]
    pub nullifier_hex: String,
    #[serde(default, alias = "challenge_hex")]
    pub challenge_hex: Option<String>,
    #[serde(default, alias = "challenge_hi_hex")]
    pub challenge_hi_hex: Option<String>,
    #[serde(default, alias = "challenge_lo_hex")]
    pub challenge_lo_hex: Option<String>,
}

/// Structured error payload returned to WebAssembly callers on failure.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WasmProverError {
    pub error_type: String,
    pub message: String,
}

impl WasmProverError {
    pub fn new(error_type: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            error_type: error_type.into(),
            message: message.into(),
        }
    }

    pub fn invalid_scalar(field: &str, msg: &str) -> Self {
        Self::new(
            "InvalidScalarModulus",
            format!("Field '{field}' exceeds modulus or is non-canonical: {msg}"),
        )
    }

    pub fn constraint_unsatisfied(msg: &str) -> Self {
        Self::new("ConstraintUnsatisfied", msg)
    }

    pub fn deserialization_failure(target: &str, msg: &str) -> Self {
        Self::new(
            "DeserializationFailure",
            format!("Failed to deserialize {target}: {msg}"),
        )
    }
}
