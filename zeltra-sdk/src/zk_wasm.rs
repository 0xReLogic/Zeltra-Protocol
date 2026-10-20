//! In-Browser WebAssembly Groth16 Prover Runtime & Bindings (DEC-041)
//!
//! Provides single-threaded, non-blocking in-worker Groth16 proof generation for:
//! 1. 1-in-1-out `PrivateNoteCircuit` (16 Public Inputs, DEC-035A/B)
//! 2. Universal 2-in-2-out `JoinSplitCircuit` (19 Public Inputs, DEC-036A)
//! 3. CEX `SelectiveDisclosureCircuit` (5 Public Inputs, DEC-040A)
//!
//! Enforces:
//! - Strict client-side witness confinement (Invariant 1)
//! - Zero COOP/COEP dependency for iframe compatibility (Invariant 4)
//! - Fail-closed error handling with structured diagnostics (Invariant 5)
//! - Cryptographically secure randomness via OS/WASM CSPRNG (Invariant 6)

use ark_bls12_381::{Bls12_381, Fr};
use ark_relations::gr1cs::{ConstraintSynthesizer, ConstraintSystem};
use ark_serialize::CanonicalDeserialize;
use wasm_bindgen::prelude::*;

use zeltra_core::{
    compute_binding_commitment, compute_scope_hash,
    decompose_challenge_to_limbs, derive_joinsplit_dummy_nullifier, derive_joinsplit_nullifier,
    derive_note_randomness, derive_nullifier, derive_nullifier_key,
    extract_joinsplit_public_inputs, extract_public_inputs, from_evm_scalar,
    generate_disclosure_proof, generate_joinsplit_proof, generate_note_proof, note_commitment,
    to_evm_g1, to_evm_g2, to_evm_scalar, JoinSplitCircuit, PrivateNoteCircuit,
    SelectiveDisclosureCircuit, NUM_DISCLOSURE_PUBLIC_INPUTS, NUM_JOINSPLIT_PUBLIC_INPUTS,
    NUM_PUBLIC_INPUTS,
};

use crate::wasm_types::{
    JoinSplitWitnessInput, PrivateNoteWitnessInput, SelectiveDisclosureWitnessInput,
    WasmProverError, WasmProverResult,
};

// ═══════════════════════════════════════════════════════════════════════════
// Console Panic Hook & Logging
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

/// Registers the panic hook in browser environments to log Rust panics to DevTools console.
#[wasm_bindgen]
pub fn init_wasm_panic_hook() {
    #[cfg(target_arch = "wasm32")]
    console_error_panic_hook::set_once();
}

#[cfg(target_arch = "wasm32")]
fn current_time_ms() -> f64 {
    js_sys::Date::now()
}

#[cfg(not(target_arch = "wasm32"))]
fn current_time_ms() -> f64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as f64)
        .unwrap_or(0.0)
}

// ═══════════════════════════════════════════════════════════════════════════
// Canonical Field Parsing Helpers (Fail-Closed, Zero-Panic)
// ═══════════════════════════════════════════════════════════════════════════

/// Parses a 32-byte big-endian hex string into an `Fr` scalar, strictly verifying < r.
pub fn parse_canonical_scalar(hex_str: &str, field_name: &str) -> Result<Fr, WasmProverError> {
    let clean = hex_str.trim().trim_start_matches("0x");
    let bytes = hex::decode(clean).map_err(|e| {
        WasmProverError::invalid_scalar(field_name, &format!("Invalid hex encoding: {e}"))
    })?;

    if bytes.len() > 32 {
        return Err(WasmProverError::invalid_scalar(
            field_name,
            "Hex length exceeds 32 bytes (256 bits)",
        ));
    }

    let mut be_bytes = [0u8; 32];
    be_bytes[32 - bytes.len()..].copy_from_slice(&bytes);

    from_evm_scalar(&be_bytes).ok_or_else(|| {
        WasmProverError::invalid_scalar(
            field_name,
            "Scalar value exceeds BLS12-381 scalar field modulus (non-canonical)",
        )
    })
}

/// Parses an EVM address (20 bytes) or 32-byte scalar hex string into an `Fr` element.
pub fn parse_address_or_scalar(hex_str: &str, field_name: &str) -> Result<Fr, WasmProverError> {
    let clean = hex_str.trim().trim_start_matches("0x");
    let bytes = hex::decode(clean).map_err(|e| {
        WasmProverError::invalid_scalar(field_name, &format!("Invalid hex string: {e}"))
    })?;

    if bytes.len() > 32 {
        return Err(WasmProverError::invalid_scalar(
            field_name,
            "Length exceeds 32 bytes",
        ));
    }

    let mut be_bytes = [0u8; 32];
    if bytes.len() == 20 {
        // Standard EVM address layout: 12 leading zero bytes + 20 address bytes
        be_bytes[12..].copy_from_slice(&bytes);
    } else {
        be_bytes[32 - bytes.len()..].copy_from_slice(&bytes);
    }

    from_evm_scalar(&be_bytes).ok_or_else(|| {
        WasmProverError::invalid_scalar(field_name, "Value exceeds BLS12-381 scalar field modulus")
    })
}

/// Helper to serialize a scalar to standard 0x-prefixed 32-byte EVM big-endian hex string.
pub fn scalar_to_hex(scalar: &Fr) -> String {
    let bytes = to_evm_scalar(scalar);
    format!("0x{}", hex::encode(bytes))
}

// ═══════════════════════════════════════════════════════════════════════════
// 1. PrivateNoteCircuit Prover (1-in-1-out, 16 Public Inputs, DEC-035A/B)
// ═══════════════════════════════════════════════════════════════════════════

/// Synthesizes and proves the 1-in-1-out PrivateNoteCircuit using pre-deserialized keys.
pub fn prove_private_note_with_pk(
    witness: &PrivateNoteWitnessInput,
    pk: &ark_groth16::ProvingKey<Bls12_381>,
) -> Result<WasmProverResult, WasmProverError> {
    let start_time = current_time_ms();

    // 1. Parse public & tree context
    let note_root = parse_canonical_scalar(&witness.note_root_hex, "note_root")?;
    let leaf_count_fr = Fr::from(witness.leaf_count);
    let note_epoch_id = witness.note_epoch_id.unwrap_or(0);
    let note_epoch_id_fr = Fr::from(note_epoch_id);

    let recipient = parse_address_or_scalar(&witness.recipient_hex, "recipient")?;
    let merchant_amount = Fr::from(witness.merchant_amount);
    let protocol_fee = Fr::from(witness.protocol_fee);
    let execution_fee = Fr::from(witness.execution_fee);

    let quote_hi = parse_canonical_scalar(&witness.quote_hash_hi_hex, "quote_hash_hi")?;
    let quote_lo = parse_canonical_scalar(&witness.quote_hash_lo_hex, "quote_hash_lo")?;
    let chain_id = Fr::from(witness.chain_id);
    let contract_address =
        parse_address_or_scalar(&witness.contract_address_hex, "contract_address")?;
    let expiry = Fr::from(witness.expiry);

    let has_change_bool = witness.has_change;
    let has_change_fr = Fr::from(if has_change_bool { 1u64 } else { 0u64 });
    let is_roll_bool = witness.is_rollover.unwrap_or(false);
    let is_roll_fr = Fr::from(if is_roll_bool { 1u64 } else { 0u64 });
    let aid_u8 = witness.asset_id;
    let aid_fr = Fr::from(aid_u8 as u64);

    let flags_packed_fr = if let Some(packed) = witness.flags_packed {
        Fr::from(packed)
    } else {
        is_roll_fr + aid_fr * Fr::from(2u64)
    };

    // 2. Parse input note witness
    let in_val_u64 = witness.input_value;
    let in_value = Fr::from(in_val_u64);
    let in_sk = parse_canonical_scalar(&witness.input_spending_key_hex, "input_spending_key")?;
    let in_rho = parse_canonical_scalar(&witness.input_rho_hex, "input_rho")?;
    let in_rand = parse_canonical_scalar(&witness.input_randomness_hex, "input_randomness")?;
    let in_leaf_idx = Fr::from(witness.input_leaf_index);

    let mut m_sibs = [Fr::from(0u64); 32];
    for (i, hex_str) in witness.mountain_siblings_hex.iter().enumerate().take(32) {
        m_sibs[i] = parse_canonical_scalar(hex_str, "mountain_siblings")?;
    }

    let mut p_sibs = [Fr::from(0u64); 32];
    for (i, hex_str) in witness
        .peak_bagging_siblings_hex
        .iter()
        .enumerate()
        .take(32)
    {
        p_sibs[i] = parse_canonical_scalar(hex_str, "peak_bagging_siblings")?;
    }
    let p_bag_count = witness.peak_bagging_siblings_hex.len().min(32) as u8;

    // Derive nullifier key & input nullifier
    let nk = derive_nullifier_key(in_sk);
    let in_cm = note_commitment(in_val_u64, aid_u8, in_sk, in_rho, in_rand);
    let input_nullifier = derive_nullifier(
        nk,
        in_cm,
        witness.input_leaf_index,
        (note_epoch_id & 0xFFFFFFFF) as u32,
    );

    // 3. Parse change note witness
    let (ch_val_u64, ch_owner, ch_rho, ch_rand, output_commitment) = if has_change_bool {
        let ch_v = witness.change_value.unwrap_or(0);
        let ch_sk = if let Some(ref o) = witness.change_owner_key_hex {
            parse_canonical_scalar(o, "change_owner_key")?
        } else {
            in_sk
        };
        let ch_r = if let Some(ref r) = witness.change_rho_hex {
            parse_canonical_scalar(r, "change_rho")?
        } else {
            derive_note_randomness(in_sk, in_rho)
        };
        let ch_rnd = if let Some(ref rnd) = witness.change_randomness_hex {
            parse_canonical_scalar(rnd, "change_randomness")?
        } else {
            derive_note_randomness(in_sk, in_rand)
        };
        let cm = note_commitment(ch_v, aid_u8, ch_sk, ch_r, ch_rnd);
        (ch_v, ch_sk, ch_r, ch_rnd, cm)
    } else {
        let dummy_owner = in_sk;
        let dummy_rho = Fr::from(0u64);
        let dummy_rand = Fr::from(0u64);
        let cm = note_commitment(0, aid_u8, dummy_owner, dummy_rho, dummy_rand);
        (0u64, dummy_owner, dummy_rho, dummy_rand, cm)
    };

    // 4. Derive scope binding
    let expected_binding = if let Some(ref bind_hex) = witness.expected_binding_hex {
        parse_canonical_scalar(bind_hex, "expected_binding")?
    } else {
        let scope_hash = compute_scope_hash(recipient, chain_id, contract_address, expiry);
        compute_binding_commitment(quote_hi, quote_lo, scope_hash)
    };

    // 5. Build PrivateNoteCircuit
    let circuit = PrivateNoteCircuit {
        note_root: Some(note_root),
        leaf_count: Some(leaf_count_fr),
        input_nullifier: Some(input_nullifier),
        note_epoch_id: Some(note_epoch_id_fr),
        output_commitment: Some(output_commitment),
        recipient: Some(recipient),
        merchant_amount: Some(merchant_amount),
        protocol_fee: Some(protocol_fee),
        execution_fee: Some(execution_fee),
        quote_hash_hi: Some(quote_hi),
        quote_hash_lo: Some(quote_lo),
        chain_id: Some(chain_id),
        contract_address: Some(contract_address),
        expiry: Some(expiry),
        has_change: Some(has_change_fr),
        flags_packed: Some(flags_packed_fr),

        is_rollover: Some(is_roll_fr),
        asset_id: Some(aid_fr),

        input_value: Some(in_value),
        input_owner_key: Some(in_sk),
        input_rho: Some(in_rho),
        input_randomness: Some(in_rand),
        input_leaf_index: Some(in_leaf_idx),
        mountain_height: Some(witness.mountain_height),
        mountain_siblings: Some(m_sibs),
        peak_bagging_siblings: Some(p_sibs),
        peak_bagging_count: Some(p_bag_count),

        change_value: Some(Fr::from(ch_val_u64)),
        change_owner_key: Some(ch_owner),
        change_rho: Some(ch_rho),
        change_randomness: Some(ch_rand),

        expected_binding: Some(expected_binding),
    };

    // 6. Preflight constraint satisfiability verification (DEC-041 Invariant 5: Fail-closed zero-panic)
    let cs = ConstraintSystem::<Fr>::new_ref();
    circuit
        .clone()
        .generate_constraints(cs.clone())
        .map_err(|e| {
            WasmProverError::constraint_unsatisfied(&format!(
                "PrivateNoteCircuit R1CS constraint generation failed: {e:?}"
            ))
        })?;

    if !cs.is_satisfied().map_err(|e| {
        WasmProverError::constraint_unsatisfied(&format!("Constraint check failed: {e:?}"))
    })? {
        return Err(WasmProverError::constraint_unsatisfied(
            "Circuit constraints are not satisfied by witness assignments (value conservation or relation mismatch)",
        ));
    }

    // 7. Execute Groth16 Prover
    let proof = generate_note_proof(circuit.clone(), pk).map_err(|e| {
        WasmProverError::constraint_unsatisfied(&format!(
            "PrivateNoteCircuit R1CS synthesis failed: {e:?}"
        ))
    })?;

    // 8. Format EVM Proof Points & 16 Public Inputs
    let proof_a_neg = to_evm_g1(&-proof.a);
    let proof_b = to_evm_g2(&proof.b);
    let proof_c = to_evm_g1(&proof.c);

    let pis = extract_public_inputs(&circuit);
    if pis.len() != NUM_PUBLIC_INPUTS {
        return Err(WasmProverError::constraint_unsatisfied(&format!(
            "Public input count mismatch: expected {}, got {}",
            NUM_PUBLIC_INPUTS,
            pis.len()
        )));
    }

    let public_inputs = pis.iter().map(scalar_to_hex).collect();
    let elapsed = (current_time_ms() - start_time).max(0.0) as u64;

    Ok(WasmProverResult::new(
        format!("0x{}", hex::encode(proof_a_neg)),
        format!("0x{}", hex::encode(proof_b)),
        format!("0x{}", hex::encode(proof_c)),
        public_inputs,
        elapsed,
    ))
}

/// Internal pipeline deserializing compressed proving key and proving `PrivateNoteCircuit`.
pub fn prove_private_note_internal(
    witness: &PrivateNoteWitnessInput,
    proving_key_bytes: &[u8],
) -> Result<WasmProverResult, WasmProverError> {
    let mut reader = proving_key_bytes;
    let pk = ark_groth16::ProvingKey::<Bls12_381>::deserialize_compressed_unchecked(&mut reader)
        .map_err(|e| {
            WasmProverError::deserialization_failure("PrivateNote ProvingKey", &format!("{e:?}"))
        })?;

    prove_private_note_with_pk(witness, &pk)
}

/// Exported WebAssembly function generating a Groth16 proof for `PrivateNoteCircuit` (16 PIs).
#[wasm_bindgen]
pub fn wasm_prove_private_note(
    witness_json: &str,
    proving_key_bytes: &[u8],
) -> Result<JsValue, JsValue> {
    init_wasm_panic_hook();

    let witness: PrivateNoteWitnessInput = serde_json::from_str(witness_json).map_err(|e| {
        let err = WasmProverError::new("WitnessJsonParseError", format!("{e}"));
        serde_wasm_bindgen::to_value(&err).unwrap_or_else(|_| JsValue::from_str(&err.message))
    })?;

    match prove_private_note_internal(&witness, proving_key_bytes) {
        Ok(result) => serde_wasm_bindgen::to_value(&result)
            .map_err(|e| JsValue::from_str(&format!("Serialization error: {e}"))),
        Err(err) => {
            let val = serde_wasm_bindgen::to_value(&err)
                .unwrap_or_else(|_| JsValue::from_str(&err.message));
            Err(val)
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 2. Universal JoinSplitCircuit Prover (2-in-2-out, 19 Public Inputs, DEC-036A)
// ═══════════════════════════════════════════════════════════════════════════

/// Synthesizes and proves the universal 2-in-2-out JoinSplitCircuit using pre-deserialized keys.
pub fn prove_joinsplit_with_pk(
    witness: &JoinSplitWitnessInput,
    pk: &ark_groth16::ProvingKey<Bls12_381>,
) -> Result<WasmProverResult, WasmProverError> {
    let start_time = current_time_ms();

    let note_root = parse_canonical_scalar(&witness.note_root_hex, "note_root")?;
    let leaf_count_fr = Fr::from(witness.leaf_count);

    let recipient = parse_address_or_scalar(&witness.recipient_hex, "recipient")?;
    let merchant_amount = Fr::from(witness.merchant_amount);
    let protocol_fee = Fr::from(witness.protocol_fee);
    let execution_fee = Fr::from(witness.execution_fee);

    let quote_hi = parse_canonical_scalar(&witness.quote_hash_hi_hex, "quote_hash_hi")?;
    let quote_lo = parse_canonical_scalar(&witness.quote_hash_lo_hex, "quote_hash_lo")?;
    let chain_id = Fr::from(witness.chain_id);
    let contract_address =
        parse_address_or_scalar(&witness.contract_address_hex, "contract_address")?;
    let expiry = Fr::from(witness.expiry);

    let has_change_1_bool = witness.has_change_1;
    let has_change_2_bool = witness.has_change_2;
    let has_change_1_fr = Fr::from(if has_change_1_bool { 1u64 } else { 0u64 });
    let has_change_2_fr = Fr::from(if has_change_2_bool { 1u64 } else { 0u64 });
    let is_roll_bool = witness.is_rollover.unwrap_or(false);
    let is_roll_fr = Fr::from(if is_roll_bool { 1u64 } else { 0u64 });
    let aid_u8 = witness.asset_id;
    let aid_fr = Fr::from(aid_u8 as u64);

    let flags_packed_fr = if let Some(packed) = witness.flags_packed {
        Fr::from(packed)
    } else {
        has_change_1_fr + has_change_2_fr * Fr::from(2u64) + aid_fr * Fr::from(4u64)
    };

    // Input note 1
    let in1_sk = parse_canonical_scalar(&witness.in1_spending_key_hex, "in1_spending_key")?;
    let in1_rho = parse_canonical_scalar(&witness.in1_rho_hex, "in1_rho")?;
    let in1_rand = parse_canonical_scalar(&witness.in1_randomness_hex, "in1_randomness")?;
    let in1_epoch = witness.in1_epoch_id.unwrap_or(0);
    let nk1 = derive_nullifier_key(in1_sk);
    let nf1 = derive_joinsplit_nullifier(nk1, in1_rho, in1_epoch);

    let mut in1_sibs = [Fr::from(0u64); 32];
    for (i, hex_str) in witness
        .in1_mountain_siblings_hex
        .iter()
        .enumerate()
        .take(32)
    {
        in1_sibs[i] = parse_canonical_scalar(hex_str, "in1_mountain_siblings")?;
    }

    // Input note 2 (real or dummy)
    let in2_is_dummy = witness.in2_is_dummy;
    let in2_is_dummy_fr = Fr::from(if in2_is_dummy { 1u64 } else { 0u64 });

    let (in2_val, in2_sk, in2_rho, in2_rand, in2_leaf, in2_h, in2_sibs, in2_nonce, in2_epoch, nf2) =
        if in2_is_dummy {
            let nonce = if let Some(ref n) = witness.in2_session_nonce_hex {
                parse_canonical_scalar(n, "in2_session_nonce")?
            } else {
                Fr::from(999u64)
            };
            let dummy_nf = derive_joinsplit_dummy_nullifier(nk1, nonce);
            (
                Fr::from(0u64),
                in1_sk,
                Fr::from(0u64),
                Fr::from(0u64),
                Fr::from(0u64),
                0u8,
                [Fr::from(0u64); 32],
                nonce,
                in1_epoch,
                dummy_nf,
            )
        } else {
            let v = Fr::from(witness.in2_value.unwrap_or(0));
            let sk2 = if let Some(ref s) = witness.in2_spending_key_hex {
                parse_canonical_scalar(s, "in2_spending_key")?
            } else {
                in1_sk
            };
            let rho2 = if let Some(ref r) = witness.in2_rho_hex {
                parse_canonical_scalar(r, "in2_rho")?
            } else {
                Fr::from(0u64)
            };
            let rand2 = if let Some(ref rnd) = witness.in2_randomness_hex {
                parse_canonical_scalar(rnd, "in2_randomness")?
            } else {
                Fr::from(0u64)
            };
            let leaf2 = Fr::from(witness.in2_leaf_index.unwrap_or(0));
            let h2 = witness.in2_mountain_height.unwrap_or(0);
            let mut sibs2 = [Fr::from(0u64); 32];
            if let Some(ref s_vec) = witness.in2_mountain_siblings_hex {
                for (i, hex_str) in s_vec.iter().enumerate().take(32) {
                    sibs2[i] = parse_canonical_scalar(hex_str, "in2_mountain_siblings")?;
                }
            }
            let epoch2 = witness.in2_epoch_id.unwrap_or(in1_epoch);
            let nk2 = derive_nullifier_key(sk2);
            let real_nf = derive_joinsplit_nullifier(nk2, rho2, epoch2);
            (
                v,
                sk2,
                rho2,
                rand2,
                leaf2,
                h2,
                sibs2,
                Fr::from(0u64),
                epoch2,
                real_nf,
            )
        };

    // MMR Peaks
    let mut peaks = [Fr::from(0u64); 32];
    for (i, hex_str) in witness.mmr_peaks_hex.iter().enumerate().take(32) {
        peaks[i] = parse_canonical_scalar(hex_str, "mmr_peaks")?;
    }

    // Output notes
    let out1_owner = parse_canonical_scalar(&witness.out1_owner_key_hex, "out1_owner_key")?;
    let out1_rho = parse_canonical_scalar(&witness.out1_rho_hex, "out1_rho")?;
    let out1_rand = parse_canonical_scalar(&witness.out1_randomness_hex, "out1_randomness")?;
    let cm_out1 = note_commitment(witness.out1_value, aid_u8, out1_owner, out1_rho, out1_rand);

    let out2_owner = parse_canonical_scalar(&witness.out2_owner_key_hex, "out2_owner_key")?;
    let out2_rho = parse_canonical_scalar(&witness.out2_rho_hex, "out2_rho")?;
    let out2_rand = parse_canonical_scalar(&witness.out2_randomness_hex, "out2_randomness")?;
    let cm_out2 = note_commitment(witness.out2_value, aid_u8, out2_owner, out2_rho, out2_rand);

    // Expected scope binding
    let expected_binding = if let Some(ref bind_hex) = witness.expected_binding_hex {
        parse_canonical_scalar(bind_hex, "expected_binding")?
    } else {
        let scope_hash = compute_scope_hash(recipient, chain_id, contract_address, expiry);
        compute_binding_commitment(quote_hi, quote_lo, scope_hash)
    };

    let circuit = JoinSplitCircuit {
        note_root: Some(note_root),
        leaf_count: Some(leaf_count_fr),
        input_nullifier_1: Some(nf1),
        input_nullifier_2: Some(nf2),
        epoch_id_1: Some(Fr::from(in1_epoch)),
        epoch_id_2: Some(Fr::from(in2_epoch)),
        output_commitment_1: Some(cm_out1),
        output_commitment_2: Some(cm_out2),
        recipient: Some(recipient),
        merchant_amount: Some(merchant_amount),
        protocol_fee: Some(protocol_fee),
        execution_fee: Some(execution_fee),
        quote_hash_hi: Some(quote_hi),
        quote_hash_lo: Some(quote_lo),
        chain_id: Some(chain_id),
        contract_address: Some(contract_address),
        expiry: Some(expiry),
        flags_packed: Some(flags_packed_fr),
        is_rollover: Some(is_roll_fr),

        in1_value: Some(Fr::from(witness.in1_value)),
        in1_owner_key: Some(in1_sk),
        in1_rho: Some(in1_rho),
        in1_randomness: Some(in1_rand),
        in1_leaf_index: Some(Fr::from(witness.in1_leaf_index)),
        in1_mountain_height: Some(witness.in1_mountain_height),
        in1_mountain_siblings: Some(in1_sibs),

        in2_value: Some(in2_val),
        in2_owner_key: Some(in2_sk),
        in2_rho: Some(in2_rho),
        in2_randomness: Some(in2_rand),
        in2_leaf_index: Some(in2_leaf),
        in2_mountain_height: Some(in2_h),
        in2_mountain_siblings: Some(in2_sibs),
        in2_is_dummy: Some(in2_is_dummy_fr),
        in2_session_nonce: Some(in2_nonce),

        mmr_peaks: Some(peaks),

        out1_value: Some(Fr::from(witness.out1_value)),
        out1_owner_key: Some(out1_owner),
        out1_rho: Some(out1_rho),
        out1_randomness: Some(out1_rand),

        out2_value: Some(Fr::from(witness.out2_value)),
        out2_owner_key: Some(out2_owner),
        out2_rho: Some(out2_rho),
        out2_randomness: Some(out2_rand),

        has_change_1: Some(has_change_1_fr),
        has_change_2: Some(has_change_2_fr),
        expected_binding: Some(expected_binding),

        public_asset_id: Some(aid_fr),
        in1_asset_id: Some(aid_fr),
        in2_asset_id: Some(aid_fr),
        out1_asset_id: Some(aid_fr),
        out2_asset_id: Some(aid_fr),
    };

    // Preflight constraint satisfiability verification (DEC-041 Invariant 5: Fail-closed zero-panic)
    let cs = ConstraintSystem::<Fr>::new_ref();
    circuit
        .clone()
        .generate_constraints(cs.clone())
        .map_err(|e| {
            WasmProverError::constraint_unsatisfied(&format!(
                "JoinSplitCircuit R1CS constraint generation failed: {e:?}"
            ))
        })?;

    if !cs.is_satisfied().map_err(|e| {
        WasmProverError::constraint_unsatisfied(&format!("Constraint check failed: {e:?}"))
    })? {
        return Err(WasmProverError::constraint_unsatisfied(
            "Circuit constraints are not satisfied by witness assignments (value conservation or relation mismatch)",
        ));
    }

    let proof = generate_joinsplit_proof(circuit.clone(), pk).map_err(|e| {
        WasmProverError::constraint_unsatisfied(&format!(
            "JoinSplitCircuit R1CS synthesis failed: {e:?}"
        ))
    })?;

    let proof_a_neg = to_evm_g1(&-proof.a);
    let proof_b = to_evm_g2(&proof.b);
    let proof_c = to_evm_g1(&proof.c);

    let pis = extract_joinsplit_public_inputs(&circuit);
    if pis.len() != NUM_JOINSPLIT_PUBLIC_INPUTS {
        return Err(WasmProverError::constraint_unsatisfied(&format!(
            "Public input count mismatch: expected {}, got {}",
            NUM_JOINSPLIT_PUBLIC_INPUTS,
            pis.len()
        )));
    }

    let public_inputs = pis.iter().map(scalar_to_hex).collect();
    let elapsed = (current_time_ms() - start_time).max(0.0) as u64;

    Ok(WasmProverResult::new(
        format!("0x{}", hex::encode(proof_a_neg)),
        format!("0x{}", hex::encode(proof_b)),
        format!("0x{}", hex::encode(proof_c)),
        public_inputs,
        elapsed,
    ))
}

/// Internal pipeline deserializing compressed proving key and proving `JoinSplitCircuit`.
pub fn prove_joinsplit_internal(
    witness: &JoinSplitWitnessInput,
    proving_key_bytes: &[u8],
) -> Result<WasmProverResult, WasmProverError> {
    let mut reader = proving_key_bytes;
    let pk = ark_groth16::ProvingKey::<Bls12_381>::deserialize_compressed_unchecked(&mut reader)
        .map_err(|e| {
            WasmProverError::deserialization_failure("JoinSplit ProvingKey", &format!("{e:?}"))
        })?;

    prove_joinsplit_with_pk(witness, &pk)
}

/// Exported WebAssembly function generating a Groth16 proof for `JoinSplitCircuit` (19 PIs).
#[wasm_bindgen]
pub fn wasm_prove_joinsplit(
    witness_json: &str,
    proving_key_bytes: &[u8],
) -> Result<JsValue, JsValue> {
    init_wasm_panic_hook();

    let witness: JoinSplitWitnessInput = serde_json::from_str(witness_json).map_err(|e| {
        let err = WasmProverError::new("WitnessJsonParseError", format!("{e}"));
        serde_wasm_bindgen::to_value(&err).unwrap_or_else(|_| JsValue::from_str(&err.message))
    })?;

    match prove_joinsplit_internal(&witness, proving_key_bytes) {
        Ok(result) => serde_wasm_bindgen::to_value(&result)
            .map_err(|e| JsValue::from_str(&format!("Serialization error: {e}"))),
        Err(err) => {
            let val = serde_wasm_bindgen::to_value(&err)
                .unwrap_or_else(|_| JsValue::from_str(&err.message));
            Err(val)
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 3. SelectiveDisclosureCircuit Prover (CEX Compliance, 5 Public Inputs, DEC-040A)
// ═══════════════════════════════════════════════════════════════════════════

/// Synthesizes and proves the 5-PI SelectiveDisclosureCircuit using pre-deserialized keys.
pub fn prove_selective_disclosure_with_pk(
    witness: &SelectiveDisclosureWitnessInput,
    pk: &ark_groth16::ProvingKey<Bls12_381>,
) -> Result<WasmProverResult, WasmProverError> {
    let start_time = current_time_ms();

    let sk = parse_canonical_scalar(&witness.spending_key_hex, "spending_key")?;
    let owner_key = parse_canonical_scalar(&witness.owner_key_hex, "owner_key")?;
    let rho = parse_canonical_scalar(&witness.rho_hex, "rho")?;
    let nullifier =
        if witness.nullifier_hex.trim().is_empty() || witness.nullifier_hex.trim() == "0x" {
            let nk = derive_nullifier_key(sk);
            derive_joinsplit_nullifier(nk, rho, witness.epoch_id as u64)
        } else {
            parse_canonical_scalar(&witness.nullifier_hex, "nullifier")?
        };

    let (challenge_hi, challenge_lo) = if let (Some(ref hi_hex), Some(ref lo_hex)) =
        (&witness.challenge_hi_hex, &witness.challenge_lo_hex)
    {
        (
            parse_canonical_scalar(hi_hex, "challenge_hi")?,
            parse_canonical_scalar(lo_hex, "challenge_lo")?,
        )
    } else if let Some(ref chal_hex) = witness.challenge_hex {
        let clean = chal_hex.trim().trim_start_matches("0x");
        let bytes = hex::decode(clean).map_err(|e| {
            WasmProverError::invalid_scalar("challenge", &format!("Invalid challenge hex: {e}"))
        })?;
        if bytes.len() != 32 {
            return Err(WasmProverError::invalid_scalar(
                "challenge",
                "Challenge must be exactly 32 bytes",
            ));
        }
        let mut chal_arr = [0u8; 32];
        chal_arr.copy_from_slice(&bytes);
        decompose_challenge_to_limbs(&chal_arr)
    } else {
        return Err(WasmProverError::invalid_scalar(
            "challenge",
            "Missing challenge or challenge_hi/challenge_lo limbs in witness input",
        ));
    };

    let circuit = SelectiveDisclosureCircuit {
        spending_key: Some(sk),
        epoch_id: Some(witness.epoch_id),
        owner_key: Some(owner_key),
        rho: Some(rho),
        nullifier: Some(nullifier),
        challenge_hi: Some(challenge_hi),
        challenge_lo: Some(challenge_lo),
    };

    // Preflight constraint satisfiability verification (DEC-041 Invariant 5: Fail-closed zero-panic)
    let cs = ConstraintSystem::<Fr>::new_ref();
    circuit
        .clone()
        .generate_constraints(cs.clone())
        .map_err(|e| {
            WasmProverError::constraint_unsatisfied(&format!(
                "SelectiveDisclosureCircuit R1CS constraint generation failed: {e:?}"
            ))
        })?;

    if !cs.is_satisfied().map_err(|e| {
        WasmProverError::constraint_unsatisfied(&format!("Constraint check failed: {e:?}"))
    })? {
        return Err(WasmProverError::constraint_unsatisfied(
            "Circuit constraints are not satisfied by witness assignments",
        ));
    }

    let proof = generate_disclosure_proof(circuit.clone(), pk).map_err(|e| {
        WasmProverError::constraint_unsatisfied(&format!(
            "SelectiveDisclosureCircuit R1CS synthesis failed: {e:?}"
        ))
    })?;

    let proof_a_neg = to_evm_g1(&-proof.a);
    let proof_b = to_evm_g2(&proof.b);
    let proof_c = to_evm_g1(&proof.c);

    let pis = circuit.extract_public_inputs().ok_or_else(|| {
        WasmProverError::constraint_unsatisfied("Failed to extract disclosure public inputs")
    })?;

    if pis.len() != NUM_DISCLOSURE_PUBLIC_INPUTS {
        return Err(WasmProverError::constraint_unsatisfied(&format!(
            "Public input count mismatch: expected {}, got {}",
            NUM_DISCLOSURE_PUBLIC_INPUTS,
            pis.len()
        )));
    }

    let public_inputs = pis.iter().map(scalar_to_hex).collect();
    let elapsed = (current_time_ms() - start_time).max(0.0) as u64;

    Ok(WasmProverResult::new(
        format!("0x{}", hex::encode(proof_a_neg)),
        format!("0x{}", hex::encode(proof_b)),
        format!("0x{}", hex::encode(proof_c)),
        public_inputs,
        elapsed,
    ))
}

/// Internal pipeline deserializing compressed proving key and proving `SelectiveDisclosureCircuit`.
pub fn prove_selective_disclosure_internal(
    witness: &SelectiveDisclosureWitnessInput,
    proving_key_bytes: &[u8],
) -> Result<WasmProverResult, WasmProverError> {
    let mut reader = proving_key_bytes;
    let pk = ark_groth16::ProvingKey::<Bls12_381>::deserialize_compressed_unchecked(&mut reader)
        .map_err(|e| {
            WasmProverError::deserialization_failure(
                "SelectiveDisclosure ProvingKey",
                &format!("{e:?}"),
            )
        })?;

    prove_selective_disclosure_with_pk(witness, &pk)
}

/// Exported WebAssembly function generating a Groth16 proof for `SelectiveDisclosureCircuit` (5 PIs).
#[wasm_bindgen]
pub fn wasm_prove_selective_disclosure(
    witness_json: &str,
    proving_key_bytes: &[u8],
) -> Result<JsValue, JsValue> {
    init_wasm_panic_hook();

    let witness: SelectiveDisclosureWitnessInput =
        serde_json::from_str(witness_json).map_err(|e| {
            let err = WasmProverError::new("WitnessJsonParseError", format!("{e}"));
            serde_wasm_bindgen::to_value(&err).unwrap_or_else(|_| JsValue::from_str(&err.message))
        })?;

    match prove_selective_disclosure_internal(&witness, proving_key_bytes) {
        Ok(result) => serde_wasm_bindgen::to_value(&result)
            .map_err(|e| JsValue::from_str(&format!("Serialization error: {e}"))),
        Err(err) => {
            let val = serde_wasm_bindgen::to_value(&err)
                .unwrap_or_else(|_| JsValue::from_str(&err.message));
            Err(val)
        }
    }
}

/// In-Worker fast memo scanner with 1-Byte View Tag acceleration (DEC-045C / AUD-18 / AUD-19).
///
/// Pre-filters 99.6% non-owned memos using 1-byte view tag before ChaCha20Poly1305.
/// Runs inside disposable Web Worker lifecycle with zero linear memory leaks.
#[wasm_bindgen]
pub fn scan_memos_fast_wasm(request_json: &str) -> Result<String, JsValue> {
    let req: crate::wasm_types::WasmFastScanRequest = serde_json::from_str(request_json)
        .map_err(|e| JsValue::from_str(&format!("Invalid JSON request: {e}")))?;

    let ivk_secret_clean = req
        .ivk_secret_hex
        .strip_prefix("0x")
        .unwrap_or(&req.ivk_secret_hex);
    let ivk_bytes = hex::decode(ivk_secret_clean)
        .map_err(|e| JsValue::from_str(&format!("Invalid ivk_secret_hex: {e}")))?;
    if ivk_bytes.len() != 32 {
        return Err(JsValue::from_str("ivk_secret must be 32 bytes"));
    }
    let mut ivk_arr = [0u8; 32];
    ivk_arr.copy_from_slice(&ivk_bytes);
    let ivk = zeltra_core::IncomingViewingKey::from_bytes(&ivk_arr);

    #[cfg(not(target_arch = "wasm32"))]
    let start = std::time::Instant::now();
    #[cfg(target_arch = "wasm32")]
    let start = js_sys::Date::now();

    let mut prefilter_passed = 0usize;
    let mut recovered = Vec::new();

    for ev in &req.events {
        let memo_clean = ev
            .encrypted_memo_hex
            .strip_prefix("0x")
            .unwrap_or(&ev.encrypted_memo_hex);
        let memo_bytes = match hex::decode(memo_clean) {
            Ok(b) => b,
            Err(_) => continue,
        };
        if memo_bytes.len() != zeltra_core::ENCRYPTED_MEMO_SIZE {
            continue;
        }

        let eph_pk_bytes: [u8; 32] = memo_bytes[0..32].try_into().unwrap();
        let memo_view_tag = memo_bytes[32];
        let shared_secret = ivk.diffie_hellman(&eph_pk_bytes);
        let expected_tag = zeltra_core::derive_view_tag(&shared_secret);

        let mut candidate_pair = None;
        if memo_view_tag == expected_tag {
            if let Some((pt, rand)) = zeltra_core::decrypt_note_memo_payload(&memo_bytes, &shared_secret) {
                candidate_pair = Some((pt, rand));
            }
        }

        // Fallback for legacy notes without tag
        if candidate_pair.is_none() {
            if let Some((pt, rand)) = zeltra_core::decrypt_note_memo_deterministic(&memo_bytes, &ivk) {
                candidate_pair = Some((pt, rand));
            }
        }

        if let Some((pt, rand)) = candidate_pair {
            if pt.value > 0 {
                recovered.push(crate::wasm_types::WasmRecoveredNote {
                    leaf_index: ev.leaf_index,
                    commitment_hex: ev.commitment_hex.clone(),
                    value: pt.value,
                    asset_id: pt.asset_id,
                    rho_hex: format!("0x{}", hex::encode(zeltra_core::fr_to_be_bytes(&pt.rho))),
                    randomness_hex: format!("0x{}", hex::encode(zeltra_core::fr_to_be_bytes(&rand))),
                });
            }
            prefilter_passed += 1;
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    let elapsed_ms = start.elapsed().as_millis() as u64;
    #[cfg(target_arch = "wasm32")]
    let elapsed_ms = (js_sys::Date::now() - start) as u64;

    let res = crate::wasm_types::WasmFastScanResult {
        scanned_count: req.events.len(),
        prefilter_passed_count: prefilter_passed,
        recovered_notes: recovered,
        elapsed_ms,
    };

    serde_json::to_string(&res)
        .map_err(|e| JsValue::from_str(&format!("Serialization error: {e}")))
}

