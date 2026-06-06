//! Agent Token Pool & Non-Interactive Refill Manager

use super::codec::{build_nimbus_payment_signature, encode_payment_signature};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use wasm_bindgen::prelude::*;
use nimbus_core::*;
use ark_ff::Zero;
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

impl Drop for PendingToken {
    fn drop(&mut self) {
        crate::secure_zeroize_string(&mut self.blinding_factor_hex);
        crate::secure_zeroize_string(&mut self.message);
        crate::secure_zeroize_string(&mut self.session_id);
        crate::secure_zeroize_string(&mut self.blinded_message_hex);
        if let Some(ref mut k) = self.com_k_hex {
            crate::secure_zeroize_string(k);
        }
        if let Some(ref mut sig) = self.masked_sig_hex {
            crate::secure_zeroize_string(sig);
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ReadyToken {
    pub message: String,
    pub unmasked_sig_hex: String,
    pub amount: u64,
}

impl Drop for ReadyToken {
    fn drop(&mut self) {
        crate::secure_zeroize_string(&mut self.unmasked_sig_hex);
        crate::secure_zeroize_string(&mut self.message);
    }
}

#[wasm_bindgen]
#[derive(Default, Serialize, Deserialize)]
pub struct AgentTokenPool {
    #[wasm_bindgen(skip)]
    pub pending_tokens: HashMap<String, PendingToken>,
    #[wasm_bindgen(skip)]
    pub ready_tokens: Vec<ReadyToken>,
    #[wasm_bindgen(skip)]
    pub spent_nullifiers: HashSet<String>,
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
            .map_err(|_| JsValue::from_str("Invalid input data"))
    }

    #[wasm_bindgen]
    pub fn deserialize_pool(json_str: &str) -> Result<AgentTokenPool, JsValue> {
        serde_json::from_str(json_str)
            .map_err(|_| JsValue::from_str("Invalid input data"))
    }

    #[wasm_bindgen]
    pub fn prepare_blind_token(&mut self, amount: u64, message: &str) -> Result<String, JsValue> {
        if message.len() > 1024 {
            return Err(JsValue::from_str("Invalid input data"));
        }
        let mut rng = rand::rngs::OsRng;
        let (blinded, mut r) = client_blind(message.as_bytes(), &mut rng);
        
        let blinded_hex = hex::encode(serialize_to_bytes(&blinded));
        let mut r_hex = hex::encode(serialize_to_bytes(&r));
        
        // Generate random session_id using cryptographically secure OsRng
        let mut session_bytes = [0u8; 16];
        rand::RngCore::fill_bytes(&mut rng, &mut session_bytes);
        let session_id = hex::encode(session_bytes);
        
        let pending = PendingToken {
            session_id: session_id.clone(),
            message: message.to_string(),
            blinded_message_hex: blinded_hex,
            blinding_factor_hex: r_hex.clone(),
            amount,
            com_k_hex: None,
            masked_sig_hex: None,
        };
        
        // Clean up temporary secrets
        crate::secure_zeroize(&mut r);
        crate::secure_zeroize_string(&mut r_hex);
        
        self.pending_tokens.insert(session_id.clone(), pending);
        Ok(session_id)
    }

    #[wasm_bindgen]
    pub fn get_blinded_message(&self, session_id: &str) -> Result<String, JsValue> {
        let token = self.pending_tokens.get(session_id)
            .ok_or_else(|| JsValue::from_str("Invalid input data"))?;
        Ok(token.blinded_message_hex.clone())
    }

    #[wasm_bindgen]
    pub fn register_signing_result(
        &mut self,
        session_id: &str,
        masked_sig_hex: &str,
        com_k_hex: &str,
        issuer_signature_hex: Option<String>,
        expected_issuer_pk: Option<String>,
    ) -> Result<bool, JsValue> {
        let token = self.pending_tokens.get_mut(session_id)
            .ok_or_else(|| JsValue::from_str("Invalid input data"))?;
            
        // 1. Verify that the issuer signature on com_k is valid (if provided)
        if let (Some(sig_hex), Some(pk_hex)) = (issuer_signature_hex, expected_issuer_pk) {
            let com_k_bytes = hex::decode(com_k_hex)
                .map_err(|_| JsValue::from_str("Invalid input data"))?;
            let sig_bytes = hex::decode(&sig_hex)
                .map_err(|_| JsValue::from_str("Invalid input data"))?;
            let pk_bytes = hex::decode(&pk_hex)
                .map_err(|_| JsValue::from_str("Invalid input data"))?;
                
            let signature: UnmaskedSignature = deserialize_from_bytes(&sig_bytes)
                .ok_or_else(|| JsValue::from_str("Invalid input data"))?;
            let pk_iss: IssuerPublicKey = deserialize_from_bytes(&pk_bytes)
                .ok_or_else(|| JsValue::from_str("Invalid input data"))?;
                
            // Check structural validity (prevent identity elements)
            if signature.0.is_zero() || pk_iss.0.is_zero() {
                return Err(JsValue::from_str("Invalid input data"));
            }
                
            let is_valid_issuer_sig = verify_unmasked(&com_k_bytes, &signature, &pk_iss);
            if !is_valid_issuer_sig {
                return Err(JsValue::from_str("Invalid input data"));
            }
        }

        let blinded_bytes = hex::decode(&token.blinded_message_hex)
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
            
        // Structure validation: check for zero/identity points
        if x.0.is_zero() || commitment.0.is_zero() || sig.0.is_zero() {
            return Err(JsValue::from_str("Invalid input data"));
        }
            
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
            .ok_or_else(|| JsValue::from_str("Invalid input data"))?;
            
        let masked_sig_hex = token.masked_sig_hex.as_ref()
            .ok_or_else(|| JsValue::from_str("Invalid input data"))?;
            
        let sig_bytes = hex::decode(masked_sig_hex)
            .map_err(|_| JsValue::from_str("Invalid input data"))?;
        let r_bytes = hex::decode(&token.blinding_factor_hex)
            .map_err(|_| JsValue::from_str("Invalid input data"))?;
        let k_bytes = hex::decode(masking_key_hex)
            .map_err(|_| JsValue::from_str("Invalid input data"))?;
            
        let sig: MaskedBlindSignature = deserialize_from_bytes(&sig_bytes)
            .ok_or_else(|| JsValue::from_str("Invalid input data"))?;
        let r_factor: BlindingFactor = deserialize_from_bytes(&r_bytes)
            .ok_or_else(|| JsValue::from_str("Invalid input data"))?;
        let k_key: MaskingKey = deserialize_from_bytes(&k_bytes)
            .ok_or_else(|| JsValue::from_str("Invalid input data"))?;
            
        // Structure validation: check for zero/identity points and fields
        if sig.0.is_zero() || r_factor.0.is_zero() || k_key.0.is_zero() {
            return Err(JsValue::from_str("Invalid input data"));
        }
            
        let unmasked = client_unmask(&sig, &r_factor, &k_key)
            .ok_or_else(|| JsValue::from_str("Invalid input data"))?;
            
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
            .ok_or_else(|| JsValue::from_str("Invalid input data"))?;
            
        let token = self.ready_tokens.remove(index);
        
        // Compute EVM inputs
        let alpha_neg_bytes = crate::client_get_alpha_neg_evm(&token.unmasked_sig_hex)?;
        let hm_bytes = crate::client_get_hm_evm(&token.message);
        let pk_iss_evm = crate::client_get_pk_iss_evm(pk_iss_hex)?;
        
        // The contract binds replay protection directly to the signed curve point.
        let nullifier = hex::encode(sha3::Keccak256::digest(
            hex::decode(&hm_bytes)
                .map_err(|_| JsValue::from_str("Invalid H(m) encoding"))?,
        ));
        
        // Replay protection: check local spent nullifier cache
        if self.spent_nullifiers.contains(&nullifier) {
            return Err(JsValue::from_str("Token already spent"));
        }
        self.spent_nullifiers.insert(nullifier.clone());
        
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
