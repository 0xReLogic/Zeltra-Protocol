//! WASM bindings for blind signature operations

use crate::wasm_types::BlindedOutput;
use wasm_bindgen::prelude::*;
use nimbus_core::*;
use rand::thread_rng;

/// Client: blinds a message (ephemeral public key) using a random blinding factor.
/// Returns a struct containing the hex-encoded blinded message and blinding factor.
#[wasm_bindgen]
pub fn client_blind_message(message: &str) -> Result<BlindedOutput, JsValue> {
    let mut rng = thread_rng();
    let (blinded, r) = client_blind(message.as_bytes(), &mut rng);
    
    let blinded_hex = hex::encode(serialize_to_bytes(&blinded));
    let r_hex = hex::encode(serialize_to_bytes(&r));
    
    Ok(BlindedOutput::new(blinded_hex, r_hex))
}

/// Client: verifies the masked signature from the issuer off-chain.
#[wasm_bindgen]
pub fn client_verify_masked_signature(
    blinded_hex: &str,
    com_k_hex: &str,
    masked_sig_hex: &str,
) -> Result<bool, JsValue> {
    let blinded_bytes = hex::decode(blinded_hex)
        .map_err(|e| JsValue::from_str(&format!("Invalid blinded hex: {}", e)))?;
    let com_k_bytes = hex::decode(com_k_hex)
        .map_err(|e| JsValue::from_str(&format!("Invalid com_k hex: {}", e)))?;
    let masked_sig_bytes = hex::decode(masked_sig_hex)
        .map_err(|e| JsValue::from_str(&format!("Invalid masked_sig hex: {}", e)))?;
        
    let x: BlindedMessage = deserialize_from_bytes(&blinded_bytes)
        .ok_or_else(|| JsValue::from_str("Failed to deserialize blinded message"))?;
    let commitment: MaskingKeyCommitment = deserialize_from_bytes(&com_k_bytes)
        .ok_or_else(|| JsValue::from_str("Failed to deserialize commitment"))?;
    let sig: MaskedBlindSignature = deserialize_from_bytes(&masked_sig_bytes)
        .ok_or_else(|| JsValue::from_str("Failed to deserialize masked signature"))?;
        
    Ok(client_verify_masked(&x, &commitment, &sig))
}

/// Client: unmasks the signature once the masking key k is revealed on-chain.
/// Returns the hex-encoded unmasked signature.
#[wasm_bindgen]
pub fn client_unmask_signature(
    masked_sig_hex: &str,
    r_hex: &str,
    k_hex: &str,
) -> Result<String, JsValue> {
    let sig_bytes = hex::decode(masked_sig_hex)
        .map_err(|e| JsValue::from_str(&format!("Invalid masked_sig hex: {}", e)))?;
    let r_bytes = hex::decode(r_hex)
        .map_err(|e| JsValue::from_str(&format!("Invalid r hex: {}", e)))?;
    let k_bytes = hex::decode(k_hex)
        .map_err(|e| JsValue::from_str(&format!("Invalid k hex: {}", e)))?;
        
    let sig: MaskedBlindSignature = deserialize_from_bytes(&sig_bytes)
        .ok_or_else(|| JsValue::from_str("Failed to deserialize masked signature"))?;
    let r_factor: BlindingFactor = deserialize_from_bytes(&r_bytes)
        .ok_or_else(|| JsValue::from_str("Failed to deserialize blinding factor"))?;
    let k_key: MaskingKey = deserialize_from_bytes(&k_bytes)
        .ok_or_else(|| JsValue::from_str("Failed to deserialize masking key"))?;
        
    let unmasked = client_unmask(&sig, &r_factor, &k_key)
        .ok_or_else(|| JsValue::from_str("Failed to compute unmask key inverse"))?;
        
    Ok(hex::encode(serialize_to_bytes(&unmasked)))
}

/// Verifier: verifies the unmasked final signature.
#[wasm_bindgen]
pub fn client_verify_final_signature(
    message: &str,
    sig_hex: &str,
    pk_hex: &str,
) -> Result<bool, JsValue> {
    let sig_bytes = hex::decode(sig_hex)
        .map_err(|e| JsValue::from_str(&format!("Invalid signature hex: {}", e)))?;
    let pk_bytes = hex::decode(pk_hex)
        .map_err(|e| JsValue::from_str(&format!("Invalid public key hex: {}", e)))?;
        
    let signature: UnmaskedSignature = deserialize_from_bytes(&sig_bytes)
        .ok_or_else(|| JsValue::from_str("Failed to deserialize signature"))?;
    let pk_iss: IssuerPublicKey = deserialize_from_bytes(&pk_bytes)
        .ok_or_else(|| JsValue::from_str("Failed to deserialize public key"))?;
        
    Ok(verify_unmasked(message.as_bytes(), &signature, &pk_iss))
}
