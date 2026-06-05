//! WASM bindings for threshold cryptography operations

use wasm_bindgen::prelude::*;
use nimbus_core::*;
use rand::thread_rng;

#[derive(serde::Serialize)]
struct KeyShare {
    index: u32,
    share: String,
}

/// Client/Wallet: Generates a random scalar (e.g. for identity or slope).
#[wasm_bindgen]
pub fn client_generate_random_scalar() -> String {
    let mut rng = thread_rng();
    let scalar = Fr::rand(&mut rng);
    hex::encode(serialize_to_bytes(&scalar))
}

/// Client/Leader: Splits the main secret key into n shares with threshold t.
/// Returns a JSON string containing the shares.
#[wasm_bindgen]
pub fn client_split_secret_key(
    secret_key_hex: &str,
    t: u32,
    n: u32,
) -> Result<String, JsValue> {
    let sk_bytes = hex::decode(secret_key_hex)
        .map_err(|e| JsValue::from_str(&format!("Invalid secret key hex: {}", e)))?;
    let sk: IssuerSecretKey = deserialize_from_bytes(&sk_bytes)
        .ok_or_else(|| JsValue::from_str("Failed to deserialize secret key"))?;
        
    let mut rng = thread_rng();
    let shares = split_secret_key(&sk, t as usize, n as usize, &mut rng);
    
    let key_shares: Vec<KeyShare> = shares
        .into_iter()
        .map(|(i, val)| KeyShare {
            index: i as u32,
            share: hex::encode(serialize_to_bytes(&val)),
        })
        .collect();
        
    serde_json::to_string(&key_shares)
        .map_err(|e| JsValue::from_str(&format!("Failed to serialize key shares to JSON: {}", e)))
}

/// Validator: signs a blinded message using their secret key share and a temporary masking key k.
#[wasm_bindgen]
pub fn client_sign_share(
    share_sk_hex: &str,
    blinded_hex: &str,
    k_hex: &str,
) -> Result<String, JsValue> {
    let share_sk_bytes = hex::decode(share_sk_hex)
        .map_err(|e| JsValue::from_str(&format!("Invalid share_sk hex: {}", e)))?;
    let blinded_bytes = hex::decode(blinded_hex)
        .map_err(|e| JsValue::from_str(&format!("Invalid blinded hex: {}", e)))?;
    let k_bytes = hex::decode(k_hex)
        .map_err(|e| JsValue::from_str(&format!("Invalid k hex: {}", e)))?;
        
    let share_sk: Fr = deserialize_from_bytes(&share_sk_bytes)
        .ok_or_else(|| JsValue::from_str("Failed to deserialize share_sk"))?;
    let x: BlindedMessage = deserialize_from_bytes(&blinded_bytes)
        .ok_or_else(|| JsValue::from_str("Failed to deserialize blinded message"))?;
    let k: Fr = deserialize_from_bytes(&k_bytes)
        .ok_or_else(|| JsValue::from_str("Failed to deserialize k"))?;
        
    let sig_share = sign_share(&share_sk, &x, &k);
    Ok(hex::encode(serialize_to_bytes(&sig_share)))
}

/// Client: aggregates partial signatures from a subset of validators using Lagrange interpolation.
#[wasm_bindgen]
pub fn client_aggregate_signatures(
    indices: Vec<u32>,
    signatures_hex: Vec<String>,
) -> Result<String, JsValue> {
    if indices.len() != signatures_hex.len() {
        return Err(JsValue::from_str("Indices and signatures count mismatch"));
    }
    let mut partial_sigs = vec![];
    for i in 0..indices.len() {
        let sig_bytes = hex::decode(&signatures_hex[i])
            .map_err(|e| JsValue::from_str(&format!("Invalid signature hex: {}", e)))?;
        let sig: PartialBlindSignature = deserialize_from_bytes(&sig_bytes)
            .ok_or_else(|| JsValue::from_str("Failed to deserialize partial signature"))?;
        partial_sigs.push((indices[i] as usize, sig));
    }
    let aggregated = aggregate_shares(&partial_sigs);
    Ok(hex::encode(serialize_to_bytes(&aggregated)))
}
