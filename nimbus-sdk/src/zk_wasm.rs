//! WASM bindings for ZK compliance proof generation

use crate::wasm_types::ZkComplianceProof;
use wasm_bindgen::prelude::*;
use nimbus_core::*;
use rand::rngs::OsRng;


#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

use std::sync::OnceLock;

// Global proving key cache (initialized once)
static PROVING_KEY: OnceLock<ark_groth16::ProvingKey<Bls12_381>> = OnceLock::new();
static VERIFYING_KEY: OnceLock<ark_groth16::VerifyingKey<Bls12_381>> = OnceLock::new();

/// Initialize the compliance circuit keys (proving key and verifying key)
///
/// This should be called once at application startup. The keys are cached globally
/// to avoid regenerating them for each proof generation.
#[wasm_bindgen]
pub fn init_compliance_keys() -> Result<(), JsValue> {
    #[cfg(target_arch = "wasm32")]
    log("ZK Prover: Initializing compliance circuit keys...");
    
    // Generate keys (in production, these would be loaded from a trusted setup)
    match generate_compliance_keys() {
        Ok(keys) => {
            let _ = PROVING_KEY.set(keys.proving_key);
            let _ = VERIFYING_KEY.set(keys.verifying_key);
            #[cfg(target_arch = "wasm32")]
            log("ZK Prover: Compliance circuit keys initialized successfully");
        }
        Err(_) => {
            #[cfg(target_arch = "wasm32")]
            log("ZK Prover: Failed to initialize keys");
        }
    }
    
    Ok(())
}

/// Client: Generates a compliance ZK proof locally.
/// It uses multithreading and WASM-SIMD optimizations (Pippenger MSM parallelization)
/// to compute the proof in <5 seconds.
///
/// NOTE (Ephemeral Agent Identifiers Pathway):
/// As planned in Section 10 of the roadmap, future versions will incorporate Blind Signature
/// Derivation (AIP/EIP-2537). The client SDK will generate hundreds of ephemeral stealth public keys
/// from a master AI Agent identity. The ZK compliance proof generated here will verify that these
/// disposable keys belong to a valid registered master identity in the association set, completely
/// hiding the master identity link on-chain to prevent Address Clustering Attacks.
#[wasm_bindgen]
pub fn client_generate_compliance_proof(
    root_hex: &str,
    nullifier_hex: &str,
    recipient_hex: &str,
    amount_hex: &str,
) -> Result<ZkComplianceProof, JsValue> {
    // Ensure keys are initialized
    init_compliance_keys()?;
    
    let mut root_bytes = hex::decode(root_hex)
        .map_err(|_| JsValue::from_str("Invalid input data"))?;
    let mut nullifier_bytes = hex::decode(nullifier_hex)
        .map_err(|_| JsValue::from_str("Invalid input data"))?;
    let mut recipient_bytes_decoded = hex::decode(recipient_hex)
        .map_err(|_| JsValue::from_str("Invalid input data"))?;
    let mut amount_bytes = hex::decode(amount_hex)
        .map_err(|_| JsValue::from_str("Invalid input data"))?;

    if root_bytes.len() != 32 || nullifier_bytes.len() != 32 || amount_bytes.len() != 32 {
        crate::secure_zeroize_vec(&mut root_bytes);
        crate::secure_zeroize_vec(&mut nullifier_bytes);
        crate::secure_zeroize_vec(&mut recipient_bytes_decoded);
        crate::secure_zeroize_vec(&mut amount_bytes);
        return Err(JsValue::from_str("Invalid input data"));
    }

    let mut recipient_bytes = [0u8; 32];
    if recipient_bytes_decoded.len() == 20 {
        recipient_bytes[12..].copy_from_slice(&recipient_bytes_decoded);
    } else if recipient_bytes_decoded.len() == 32 {
        recipient_bytes.copy_from_slice(&recipient_bytes_decoded);
    } else {
        crate::secure_zeroize_vec(&mut root_bytes);
        crate::secure_zeroize_vec(&mut nullifier_bytes);
        crate::secure_zeroize_vec(&mut recipient_bytes_decoded);
        crate::secure_zeroize_vec(&mut amount_bytes);
        return Err(JsValue::from_str("Invalid input data"));
    }

    // Convert inputs to Fr scalars (EVM uses big-endian scalars)
    let mut root_fr = Fr::from_be_bytes_mod_order(&root_bytes);
    let mut nullifier_fr = Fr::from_be_bytes_mod_order(&nullifier_bytes);
    let mut recipient_fr = Fr::from_be_bytes_mod_order(&recipient_bytes);
    let mut amount_fr = Fr::from_be_bytes_mod_order(&amount_bytes);

    // Generate secret and randomness for the proof using cryptographically secure OsRng
    let mut rng = OsRng;
    let mut secret = Fr::rand(&mut rng);
    let mut randomness = nullifier_fr - secret;

    // Get the proving key
    let pk = PROVING_KEY.get()
        .ok_or_else(|| JsValue::from_str("Proving key not initialized"))?;

    // Generate the compliance proof
    // Generate the compliance proof
    let proof = generate_compliance_proof(
        root_fr,
        nullifier_fr,
        recipient_fr,
        amount_fr,
        secret,
        randomness,
        pk,
    ).map_err(|_| JsValue::from_str("Proof generation failed"))?;

    // Compute public inputs for verification before zeroizing the inputs
    let g1 = G1Projective::generator();
    let pub_inputs = g1 * (root_fr + nullifier_fr + recipient_fr + amount_fr);
    let pub_inputs_evm = to_evm_g1(&pub_inputs.into_affine());

    // Zero out sensitive cryptographic values from memory immediately after use
    crate::secure_zeroize(&mut root_fr);
    crate::secure_zeroize(&mut nullifier_fr);
    crate::secure_zeroize(&mut recipient_fr);
    crate::secure_zeroize(&mut amount_fr);
    crate::secure_zeroize(&mut secret);
    crate::secure_zeroize(&mut randomness);
    crate::secure_zeroize_vec(&mut root_bytes);
    crate::secure_zeroize_vec(&mut nullifier_bytes);
    crate::secure_zeroize_vec(&mut recipient_bytes_decoded);
    crate::secure_zeroize_vec(&mut amount_bytes);
    crate::secure_zeroize(&mut recipient_bytes);

    // Convert proof to EVM format
    use ark_ec::CurveGroup;
    let proof_a_neg = -proof.a;
    let proof_a_neg_evm = to_evm_g1(&proof_a_neg);
    let proof_b_evm = to_evm_g2(&proof.b);
    let proof_c_evm = to_evm_g1(&proof.c);

    Ok(ZkComplianceProof::new(
        hex::encode(proof_a_neg_evm),
        hex::encode(proof_b_evm),
        hex::encode(proof_c_evm),
        hex::encode(pub_inputs_evm),
    ))
}

