//! WASM bindings for threshold cryptography operations

use wasm_bindgen::prelude::*;
use nimbus_core::*;
use ark_ff::Zero;
use rand::rngs::OsRng;

#[derive(serde::Serialize)]
struct KeyShare {
    index: u32,
    share: String,
}

/// Client/Wallet: Generates a random scalar (e.g. for identity or slope).
#[wasm_bindgen]
pub fn client_generate_random_scalar() -> String {
    let mut rng = OsRng;
    let mut scalar = Fr::rand(&mut rng);
    let hex_scalar = hex::encode(serialize_to_bytes(&scalar));
    crate::secure_zeroize(&mut scalar);
    hex_scalar
}

/// Client/Leader: Splits the main secret key into n shares with threshold t.
/// Returns a JSON string containing the shares.
#[wasm_bindgen]
pub fn client_split_secret_key(
    secret_key_hex: &str,
    t: u32,
    n: u32,
) -> Result<String, JsValue> {
    let mut sk_bytes = hex::decode(secret_key_hex)
        .map_err(|_| JsValue::from_str("Invalid input data"))?;
    let mut sk: IssuerSecretKey = deserialize_from_bytes(&sk_bytes)
        .ok_or_else(|| JsValue::from_str("Invalid input data"))?;
        
    // Structural validation
    if sk.0.is_zero() {
        crate::secure_zeroize_vec(&mut sk_bytes);
        crate::secure_zeroize(&mut sk);
        return Err(JsValue::from_str("Invalid input data"));
    }
        
    let mut rng = OsRng;
    let mut shares = split_secret_key(&sk, t as usize, n as usize, &mut rng);
    
    let key_shares: Vec<KeyShare> = shares
        .iter()
        .map(|(i, val)| KeyShare {
            index: *i as u32,
            share: hex::encode(serialize_to_bytes(val)),
        })
        .collect();
        
    let json_result = serde_json::to_string(&key_shares)
        .map_err(|_| JsValue::from_str("Invalid input data"));
        
    // Securely clear secret keys and shares from memory
    crate::secure_zeroize_vec(&mut sk_bytes);
    crate::secure_zeroize(&mut sk);
    for (_, val) in shares.iter_mut() {
        crate::secure_zeroize(val);
    }
        
    json_result
}

/// Validator: signs a blinded message using their secret key share and a temporary masking key k.
#[wasm_bindgen]
pub fn client_sign_share(
    share_sk_hex: &str,
    blinded_hex: &str,
    k_hex: &str,
) -> Result<String, JsValue> {
    let mut share_sk_bytes = hex::decode(share_sk_hex)
        .map_err(|_| JsValue::from_str("Invalid input data"))?;
    let blinded_bytes = hex::decode(blinded_hex)
        .map_err(|_| JsValue::from_str("Invalid input data"))?;
    let mut k_bytes = hex::decode(k_hex)
        .map_err(|_| JsValue::from_str("Invalid input data"))?;
        
    let mut share_sk: Fr = deserialize_from_bytes(&share_sk_bytes)
        .ok_or_else(|| JsValue::from_str("Invalid input data"))?;
    let x: BlindedMessage = deserialize_from_bytes(&blinded_bytes)
        .ok_or_else(|| JsValue::from_str("Invalid input data"))?;
    let mut k: Fr = deserialize_from_bytes(&k_bytes)
        .ok_or_else(|| JsValue::from_str("Invalid input data"))?;
        
    // Structural validation
    if share_sk.is_zero() || x.0.is_zero() || k.is_zero() {
        crate::secure_zeroize_vec(&mut share_sk_bytes);
        crate::secure_zeroize_vec(&mut k_bytes);
        crate::secure_zeroize(&mut share_sk);
        crate::secure_zeroize(&mut k);
        return Err(JsValue::from_str("Invalid input data"));
    }
        
    let sig_share = sign_share(&share_sk, &x, &k);
    let result = hex::encode(serialize_to_bytes(&sig_share));
    
    // Clear sensitive key components
    crate::secure_zeroize_vec(&mut share_sk_bytes);
    crate::secure_zeroize_vec(&mut k_bytes);
    crate::secure_zeroize(&mut share_sk);
    crate::secure_zeroize(&mut k);
    
    Ok(result)
}

/// Client: aggregates partial signatures from a subset of validators using Lagrange interpolation.
#[wasm_bindgen]
pub fn client_aggregate_signatures(
    indices: Vec<u32>,
    signatures_hex: Vec<String>,
) -> Result<String, JsValue> {
    if indices.len() != signatures_hex.len() {
        return Err(JsValue::from_str("Invalid input data"));
    }
    let mut partial_sigs = vec![];
    for i in 0..indices.len() {
        let sig_bytes = hex::decode(&signatures_hex[i])
            .map_err(|_| JsValue::from_str("Invalid input data"))?;
        let sig: PartialBlindSignature = deserialize_from_bytes(&sig_bytes)
            .ok_or_else(|| JsValue::from_str("Invalid input data"))?;
            
        // Structural validation
        if sig.0.is_zero() {
            return Err(JsValue::from_str("Invalid input data"));
        }
            
        partial_sigs.push((indices[i] as usize, sig));
    }
    let aggregated = aggregate_shares(&partial_sigs)
        .map_err(|e| JsValue::from_str(&e))?;
    Ok(hex::encode(serialize_to_bytes(&aggregated)))
}
