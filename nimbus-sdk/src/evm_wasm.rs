//! WASM bindings for EVM conversion helpers

use wasm_bindgen::prelude::*;
use nimbus_core::*;
use ark_ff::Zero;

/// Helper: Converts unmasked signature to EVM-compatible negated signature (-alpha) hex string (128 bytes).
#[wasm_bindgen]
pub fn client_get_alpha_neg_evm(sig_hex: &str) -> Result<String, JsValue> {
    let sig_bytes = hex::decode(sig_hex)
        .map_err(|_| JsValue::from_str("Invalid input data"))?;
    let signature: UnmaskedSignature = deserialize_from_bytes(&sig_bytes)
        .ok_or_else(|| JsValue::from_str("Invalid input data"))?;
        
    // Structure validation: check for zero/identity elements
    if signature.0.is_zero() {
        return Err(JsValue::from_str("Invalid input data"));
    }
        
    let alpha_neg = get_alpha_neg_evm(&signature);
    Ok(hex::encode(alpha_neg))
}

/// Helper: Converts message string to EVM-compatible hash-to-curve H(m) G1 point hex string (128 bytes).
#[wasm_bindgen]
pub fn client_get_hm_evm(message: &str) -> String {
    let hm = get_hm_evm(message.as_bytes());
    hex::encode(hm)
}

/// Helper: Converts public key to EVM-compatible public key (pk_iss) G2 point hex string (256 bytes).
#[wasm_bindgen]
pub fn client_get_pk_iss_evm(pk_hex: &str) -> Result<String, JsValue> {
    let pk_bytes = hex::decode(pk_hex)
        .map_err(|_| JsValue::from_str("Invalid input data"))?;
    let pk_iss: IssuerPublicKey = deserialize_from_bytes(&pk_bytes)
        .ok_or_else(|| JsValue::from_str("Invalid input data"))?;
        
    // Structure validation: check for zero/identity elements
    if pk_iss.0.is_zero() {
        return Err(JsValue::from_str("Invalid input data"));
    }
        
    let pk_iss_evm = get_pk_iss_evm(&pk_iss);
    Ok(hex::encode(pk_iss_evm))
}
