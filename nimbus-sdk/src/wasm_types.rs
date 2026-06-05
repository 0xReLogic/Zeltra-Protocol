//! WASM-compatible type wrappers

use wasm_bindgen::prelude::*;

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

#[wasm_bindgen]
pub struct ZkComplianceProof {
    proof_a_neg_hex: String,
    proof_b_hex: String,
    proof_c_hex: String,
    public_inputs_g1_hex: String,
}

#[wasm_bindgen]
impl ZkComplianceProof {
    #[wasm_bindgen(getter)]
    pub fn proof_a_neg_hex(&self) -> String {
        self.proof_a_neg_hex.clone()
    }
    #[wasm_bindgen(getter)]
    pub fn proof_b_hex(&self) -> String {
        self.proof_b_hex.clone()
    }
    #[wasm_bindgen(getter)]
    pub fn proof_c_hex(&self) -> String {
        self.proof_c_hex.clone()
    }
    #[wasm_bindgen(getter)]
    pub fn public_inputs_g1_hex(&self) -> String {
        self.public_inputs_g1_hex.clone()
    }
}

impl ZkComplianceProof {
    pub fn new(
        proof_a_neg_hex: String,
        proof_b_hex: String,
        proof_c_hex: String,
        public_inputs_g1_hex: String,
    ) -> Self {
        Self {
            proof_a_neg_hex,
            proof_b_hex,
            proof_c_hex,
            public_inputs_g1_hex,
        }
    }
}
