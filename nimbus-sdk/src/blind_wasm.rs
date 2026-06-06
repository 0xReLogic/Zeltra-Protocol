//! WASM bindings for blind signature operations

use crate::wasm_types::BlindedOutput;
use wasm_bindgen::prelude::*;
use nimbus_core::*;
use ark_ff::Zero;
use rand::rngs::OsRng;

/// Client: blinds a message (ephemeral public key) using a random blinding factor.
/// Returns a struct containing the hex-encoded blinded message and blinding factor.
#[wasm_bindgen]
pub fn client_blind_message(message: &str) -> Result<BlindedOutput, JsValue> {
    if message.len() > 1024 {
        return Err(JsValue::from_str("Invalid input data"));
    }
    let mut rng = OsRng;
    let (blinded, mut r) = client_blind(message.as_bytes(), &mut rng);
    
    let blinded_hex = hex::encode(serialize_to_bytes(&blinded));
    let mut r_hex = hex::encode(serialize_to_bytes(&r));
    
    let result = BlindedOutput::new(blinded_hex, r_hex.clone());
    
    // Clean up temporary secret values from memory
    crate::secure_zeroize(&mut r);
    crate::secure_zeroize_string(&mut r_hex);
    
    Ok(result)
}

/// Client: verifies the masked signature from the issuer off-chain.
#[wasm_bindgen]
pub fn client_verify_masked_signature(
    blinded_hex: &str,
    com_k_hex: &str,
    masked_sig_hex: &str,
) -> Result<bool, JsValue> {
    let blinded_bytes = hex::decode(blinded_hex)
        .map_err(|_| JsValue::from_str("Invalid input data"))?;
    let com_k_bytes = hex::decode(com_k_hex)
        .map_err(|_| JsValue::from_str("Invalid input data"))?;
    let masked_sig_bytes = hex::decode(masked_sig_hex)
        .map_err(|_| JsValue::from_str("Invalid input data"))?;
        
    let x: BlindedMessage = deserialize_from_bytes(&blinded_bytes)
        .ok_or_else(|| JsValue::from_str("Invalid input data"))?;
    let commitment: MaskingKeyCommitment = deserialize_from_bytes(&com_k_bytes)
        .ok_or_else(|| JsValue::from_str("Invalid input data"))?;
    let sig: MaskedBlindSignature = deserialize_from_bytes(&masked_sig_bytes)
        .ok_or_else(|| JsValue::from_str("Invalid input data"))?;
        
    // Structure validation: prevent zero-point/identity elements
    if x.0.is_zero() || commitment.0.is_zero() || sig.0.is_zero() {
        return Err(JsValue::from_str("Invalid input data"));
    }
        
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
        .map_err(|_| JsValue::from_str("Invalid input data"))?;
    let mut r_bytes = hex::decode(r_hex)
        .map_err(|_| JsValue::from_str("Invalid input data"))?;
    let mut k_bytes = hex::decode(k_hex)
        .map_err(|_| JsValue::from_str("Invalid input data"))?;
        
    let sig: MaskedBlindSignature = deserialize_from_bytes(&sig_bytes)
        .ok_or_else(|| JsValue::from_str("Invalid input data"))?;
    let mut r_factor: BlindingFactor = deserialize_from_bytes(&r_bytes)
        .ok_or_else(|| JsValue::from_str("Invalid input data"))?;
    let mut k_key: MaskingKey = deserialize_from_bytes(&k_bytes)
        .ok_or_else(|| JsValue::from_str("Invalid input data"))?;
        
    // Structure validation: check for zero/identity points and fields
    if sig.0.is_zero() || r_factor.0.is_zero() || k_key.0.is_zero() {
        crate::secure_zeroize_vec(&mut r_bytes);
        crate::secure_zeroize_vec(&mut k_bytes);
        crate::secure_zeroize(&mut r_factor);
        crate::secure_zeroize(&mut k_key);
        return Err(JsValue::from_str("Invalid input data"));
    }
        
    let unmasked = client_unmask(&sig, &r_factor, &k_key)
        .ok_or_else(|| {
            crate::secure_zeroize_vec(&mut r_bytes);
            crate::secure_zeroize_vec(&mut k_bytes);
            crate::secure_zeroize(&mut r_factor);
            crate::secure_zeroize(&mut k_key);
            JsValue::from_str("Invalid input data")
        })?;
        
    let result = hex::encode(serialize_to_bytes(&unmasked));
    
    // Clean up sensitive memory
    crate::secure_zeroize_vec(&mut r_bytes);
    crate::secure_zeroize_vec(&mut k_bytes);
    crate::secure_zeroize(&mut r_factor);
    crate::secure_zeroize(&mut k_key);
    
    Ok(result)
}

/// Verifier: verifies the unmasked final signature.
#[wasm_bindgen]
pub fn client_verify_final_signature(
    message: &str,
    sig_hex: &str,
    pk_hex: &str,
) -> Result<bool, JsValue> {
    if message.len() > 1024 {
        return Err(JsValue::from_str("Invalid input data"));
    }
    let sig_bytes = hex::decode(sig_hex)
        .map_err(|_| JsValue::from_str("Invalid input data"))?;
    let pk_bytes = hex::decode(pk_hex)
        .map_err(|_| JsValue::from_str("Invalid input data"))?;
        
    let signature: UnmaskedSignature = deserialize_from_bytes(&sig_bytes)
        .ok_or_else(|| JsValue::from_str("Invalid input data"))?;
    let pk_iss: IssuerPublicKey = deserialize_from_bytes(&pk_bytes)
        .ok_or_else(|| JsValue::from_str("Invalid input data"))?;
        
    // Structure validation
    if signature.0.is_zero() || pk_iss.0.is_zero() {
        return Err(JsValue::from_str("Invalid input data"));
    }
        
    Ok(verify_unmasked(message.as_bytes(), &signature, &pk_iss))
}
