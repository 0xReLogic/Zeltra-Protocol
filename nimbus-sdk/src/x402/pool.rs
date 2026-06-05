//! Agent Token Pool & Non-Interactive Refill Manager

use super::codec::{build_nimbus_payment_signature, encode_payment_signature};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use wasm_bindgen::prelude::*;
use nimbus_core::*;
use sha2::Digest;

#[derive(Clone, Serialize, Deserialize)]
pub struct PendingToken {
    pub session_id: String,
    pub message: String,
    pub blinded_message_hex: String,
    pub blinding_factor_hex: String,
    pub amount: u64,
    pub com_k_hex: Option<String>,
    pub masked_sig_hex: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ReadyToken {
    pub message: String,
    pub unmasked_sig_hex: String,
    pub amount: u64,
}

#[wasm_bindgen]
#[derive(Default, Serialize, Deserialize)]
pub struct AgentTokenPool {
    #[wasm_bindgen(skip)]
    pub pending_tokens: HashMap<String, PendingToken>,
    #[wasm_bindgen(skip)]
    pub ready_tokens: Vec<ReadyToken>,
}

#[wasm_bindgen]
impl AgentTokenPool {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self::default()
    }

    #[wasm_bindgen]
    pub fn serialize_pool(&self) -> Result<String, JsValue> {
        serde_json::to_string(self)
            .map_err(|e| JsValue::from_str(&format!("Serialize error: {}", e)))
    }

    #[wasm_bindgen]
    pub fn deserialize_pool(json_str: &str) -> Result<AgentTokenPool, JsValue> {
        serde_json::from_str(json_str)
            .map_err(|e| JsValue::from_str(&format!("Deserialize error: {}", e)))
    }

    #[wasm_bindgen]
    pub fn prepare_blind_token(&mut self, amount: u64, message: &str) -> Result<String, JsValue> {
        let mut rng = rand::thread_rng();
        let (blinded, r) = client_blind(message.as_bytes(), &mut rng);
        
        let blinded_hex = hex::encode(serialize_to_bytes(&blinded));
        let r_hex = hex::encode(serialize_to_bytes(&r));
        
        // Generate random session_id
        let session_id = hex::encode(rand::random::<[u8; 16]>());
        
        let pending = PendingToken {
            session_id: session_id.clone(),
            message: message.to_string(),
            blinded_message_hex: blinded_hex,
            blinding_factor_hex: r_hex,
            amount,
            com_k_hex: None,
            masked_sig_hex: None,
        };
        
        self.pending_tokens.insert(session_id.clone(), pending);
        Ok(session_id)
    }

    #[wasm_bindgen]
    pub fn get_blinded_message(&self, session_id: &str) -> Result<String, JsValue> {
        let token = self.pending_tokens.get(session_id)
            .ok_or_else(|| JsValue::from_str("Session ID not found"))?;
        Ok(token.blinded_message_hex.clone())
    }

    #[wasm_bindgen]
    pub fn register_signing_result(
        &mut self,
        session_id: &str,
        masked_sig_hex: &str,
        com_k_hex: &str,
    ) -> Result<bool, JsValue> {
        let token = self.pending_tokens.get_mut(session_id)
            .ok_or_else(|| JsValue::from_str("Session ID not found"))?;
            
        let blinded_bytes = hex::decode(&token.blinded_message_hex)
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
            
        let is_valid = client_verify_masked(&x, &commitment, &sig);
        if is_valid {
            token.masked_sig_hex = Some(masked_sig_hex.to_string());
            token.com_k_hex = Some(com_k_hex.to_string());
        }
        
        Ok(is_valid)
    }

    #[wasm_bindgen]
    pub fn unmask_token(&mut self, session_id: &str, masking_key_hex: &str) -> Result<bool, JsValue> {
        let token = self.pending_tokens.get(session_id)
            .ok_or_else(|| JsValue::from_str("Session ID not found"))?;
            
        let masked_sig_hex = token.masked_sig_hex.as_ref()
            .ok_or_else(|| JsValue::from_str("Token has not been signed or registered yet"))?;
            
        let sig_bytes = hex::decode(masked_sig_hex)
            .map_err(|e| JsValue::from_str(&format!("Invalid masked_sig hex: {}", e)))?;
        let r_bytes = hex::decode(&token.blinding_factor_hex)
            .map_err(|e| JsValue::from_str(&format!("Invalid r hex: {}", e)))?;
        let k_bytes = hex::decode(masking_key_hex)
            .map_err(|e| JsValue::from_str(&format!("Invalid k hex: {}", e)))?;
            
        let sig: MaskedBlindSignature = deserialize_from_bytes(&sig_bytes)
            .ok_or_else(|| JsValue::from_str("Failed to deserialize masked signature"))?;
        let r_factor: BlindingFactor = deserialize_from_bytes(&r_bytes)
            .ok_or_else(|| JsValue::from_str("Failed to deserialize blinding factor"))?;
        let k_key: MaskingKey = deserialize_from_bytes(&k_bytes)
            .ok_or_else(|| JsValue::from_str("Failed to deserialize masking key"))?;
            
        let unmasked = client_unmask(&sig, &r_factor, &k_key)
            .ok_or_else(|| JsValue::from_str("Failed to compute unmask key inverse"))?;
            
        let unmasked_sig_hex = hex::encode(serialize_to_bytes(&unmasked));
        
        // Move from pending to ready
        let ready = ReadyToken {
            message: token.message.clone(),
            unmasked_sig_hex,
            amount: token.amount,
        };
        
        self.ready_tokens.push(ready);
        self.pending_tokens.remove(session_id);
        
        Ok(true)
    }

    #[wasm_bindgen]
    pub fn get_ready_token_count(&self, amount: u64) -> usize {
        self.ready_tokens.iter().filter(|t| t.amount == amount).count()
    }

    #[wasm_bindgen]
    pub fn get_pending_token_count(&self) -> usize {
        self.pending_tokens.len()
    }

    #[wasm_bindgen]
    pub fn spend_any_token(
        &mut self,
        amount: u64,
        scheme: &str,
        network: &str,
        pk_iss_hex: &str,
    ) -> Result<String, JsValue> {
        // Find index of first token with matching amount
        let index = self.ready_tokens.iter().position(|t| t.amount == amount)
            .ok_or_else(|| JsValue::from_str("No ready tokens with the requested amount available"))?;
            
        let token = self.ready_tokens.remove(index);
        
        // Compute EVM inputs
        let alpha_neg_bytes = crate::client_get_alpha_neg_evm(&token.unmasked_sig_hex)?;
        let hm_bytes = crate::client_get_hm_evm(&token.message);
        let pk_iss_evm = crate::client_get_pk_iss_evm(pk_iss_hex)?;
        
        // Use message hash as nullifier candidate
        let nullifier = hex::encode(sha2::Sha256::digest(token.message.as_bytes()));
        
        let sig = build_nimbus_payment_signature(
            &nullifier,
            &alpha_neg_bytes,
            &hm_bytes,
            &pk_iss_evm,
            amount,
            scheme,
            network,
        );
        
        let header_value = encode_payment_signature(&sig)
            .map_err(|e| JsValue::from_str(&e))?;
            
        Ok(header_value)
    }
}
