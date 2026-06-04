use wasm_bindgen::prelude::*;
use nimbus_core::*;
use rand::thread_rng;

pub mod x402;

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

/// Client: blinds a message (ephemeral public key) using a random blinding factor.
/// Returns a struct containing the hex-encoded blinded message and blinding factor.
#[wasm_bindgen]
pub fn client_blind_message(message: &str) -> Result<BlindedOutput, JsValue> {
    let mut rng = thread_rng();
    let (blinded, r) = client_blind(message.as_bytes(), &mut rng);
    
    let blinded_hex = hex::encode(serialize_to_bytes(&blinded));
    let r_hex = hex::encode(serialize_to_bytes(&r));
    
    Ok(BlindedOutput {
        blinded_message: blinded_hex,
        blinding_factor: r_hex,
    })
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

/// Helper: Converts unmasked signature to EVM-compatible negated signature (-alpha) hex string (128 bytes).
#[wasm_bindgen]
pub fn client_get_alpha_neg_evm(sig_hex: &str) -> Result<String, JsValue> {
    let sig_bytes = hex::decode(sig_hex)
        .map_err(|e| JsValue::from_str(&format!("Invalid signature hex: {}", e)))?;
    let signature: UnmaskedSignature = deserialize_from_bytes(&sig_bytes)
        .ok_or_else(|| JsValue::from_str("Failed to deserialize signature"))?;
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
        .map_err(|e| JsValue::from_str(&format!("Invalid public key hex: {}", e)))?;
    let pk_iss: IssuerPublicKey = deserialize_from_bytes(&pk_bytes)
        .ok_or_else(|| JsValue::from_str("Failed to deserialize public key"))?;
    let pk_iss_evm = get_pk_iss_evm(&pk_iss);
    Ok(hex::encode(pk_iss_evm))
}

/// Client/Wallet: Generates a random scalar (e.g. for identity or slope).
#[wasm_bindgen]
pub fn client_generate_random_scalar() -> String {
    let mut rng = thread_rng();
    let scalar = Fr::rand(&mut rng);
    hex::encode(serialize_to_bytes(&scalar))
}

/// Client: Generates the offline response y = a * x + I (mod p).
#[wasm_bindgen]
pub fn client_generate_offline_response(
    a_hex: &str,
    x_hex: &str,
    identity_hex: &str,
) -> Result<String, JsValue> {
    let a_bytes = hex::decode(a_hex)
        .map_err(|e| JsValue::from_str(&format!("Invalid a hex: {}", e)))?;
    let x_bytes = hex::decode(x_hex)
        .map_err(|e| JsValue::from_str(&format!("Invalid x hex: {}", e)))?;
    let identity_bytes = hex::decode(identity_hex)
        .map_err(|e| JsValue::from_str(&format!("Invalid identity hex: {}", e)))?;

    let a: Fr = deserialize_from_bytes(&a_bytes)
        .ok_or_else(|| JsValue::from_str("Failed to deserialize slope a"))?;
    let x: Fr = deserialize_from_bytes(&x_bytes)
        .ok_or_else(|| JsValue::from_str("Failed to deserialize challenge x"))?;
    let identity: Fr = deserialize_from_bytes(&identity_bytes)
        .ok_or_else(|| JsValue::from_str("Failed to deserialize identity"))?;

    let y = generate_offline_response(a, x, identity);
    let y_bytes = serialize_to_bytes(&y);
    Ok(hex::encode(y_bytes))
}

/// Smart Contract / Challenger: Reconstructs the identity I of a double spender from two offline transaction proofs.
#[wasm_bindgen]
pub fn client_reconstruct_identity(
    x1_hex: &str,
    y1_hex: &str,
    x2_hex: &str,
    y2_hex: &str,
) -> Result<String, JsValue> {
    let x1_bytes = hex::decode(x1_hex)
        .map_err(|e| JsValue::from_str(&format!("Invalid x1 hex: {}", e)))?;
    let y1_bytes = hex::decode(y1_hex)
        .map_err(|e| JsValue::from_str(&format!("Invalid y1 hex: {}", e)))?;
    let x2_bytes = hex::decode(x2_hex)
        .map_err(|e| JsValue::from_str(&format!("Invalid x2 hex: {}", e)))?;
    let y2_bytes = hex::decode(y2_hex)
        .map_err(|e| JsValue::from_str(&format!("Invalid y2 hex: {}", e)))?;

    let x1: Fr = deserialize_from_bytes(&x1_bytes)
        .ok_or_else(|| JsValue::from_str("Failed to deserialize x1"))?;
    let y1: Fr = deserialize_from_bytes(&y1_bytes)
        .ok_or_else(|| JsValue::from_str("Failed to deserialize y1"))?;
    let x2: Fr = deserialize_from_bytes(&x2_bytes)
        .ok_or_else(|| JsValue::from_str("Failed to deserialize x2"))?;
    let y2: Fr = deserialize_from_bytes(&y2_bytes)
        .ok_or_else(|| JsValue::from_str("Failed to deserialize y2"))?;

    let proof1 = OfflineSpendProof { x: x1, y: y1 };
    let proof2 = OfflineSpendProof { x: x2, y: y2 };

    let identity = reconstruct_identity(&proof1, &proof2)
        .ok_or_else(|| JsValue::from_str("Failed to reconstruct identity (challenges might be identical)"))?;

    let identity_bytes = serialize_to_bytes(&identity);
    Ok(hex::encode(identity_bytes))
}

