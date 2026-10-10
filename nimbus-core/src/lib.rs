mod accounting;
mod blind_sign;
mod compliance_circuit;
mod crypto;
mod evm;
pub mod fees;
pub mod joinsplit_circuit;
mod note;
mod note_circuit;
mod poseidon;
mod serialization;
mod threshold;
mod types;

pub use ark_bls12_381::{Bls12_381, Fr, G1Projective, G2Projective};
pub use ark_ec::pairing::Pairing;
pub use ark_ec::PrimeGroup;
pub use ark_ff::{Field, PrimeField, UniformRand};

// Re-export types for backward compatibility
pub use types::{
    BlindedMessage, BlindingFactor, IssuerPublicKey, IssuerSecretKey, MaskedBlindSignature,
    MaskingKey, MaskingKeyCommitment, PartialBlindSignature, UnmaskedSignature,
};

// Re-export serialization helpers
pub use serialization::{deserialize_from_bytes, serialize_to_bytes};

// Re-export crypto functions
pub use crypto::hash_to_g1;

// Re-export EVM helpers
pub use evm::{
    from_evm_g1, from_evm_g2, from_evm_proof, from_evm_scalar, get_alpha_neg_evm, get_hm_evm,
    get_pk_iss_evm, to_evm_g1, to_evm_g2,
};

// Re-export blind signature protocol
pub use blind_sign::{
    client_blind, client_unmask, client_verify_masked, issuer_sign_blinded,
    verify_masking_key_commitment, verify_unmasked,
};

// Re-export threshold cryptography
pub use threshold::{
    aggregate_shares, compute_lagrange_coefficient, public_key_for_share, sign_share,
    split_secret_key, verify_partial_signature,
};

// Re-export fee policy helpers
pub use fees::{
    ceil_div, deposit_fee, fee_round_up, gross_up_for_exact_net, gross_up_private_spend_amount,
    net_after_fee, private_spend_fee, quote_cross_chain_private_spend_with_fee,
    quote_execution_fee, quote_execution_fee_with_pass_through, quote_private_spend,
    quote_private_spend_with_fee, quote_standalone_rollover, spend_fee_bps_for_holding, SpendQuote,
    DEFAULT_RELAYER_MARKUP_BPS, DEPOSIT_FEE_BPS, FEE_DENOMINATOR_BPS,
    MIN_STANDALONE_ROLLOVER_THRESHOLD_USDC, PRIVATE_SPEND_FEE_BPS, SPEND_FEE_30DAY_BPS,
    THIRTY_DAYS_SECS,
};

// Re-export compliance circuit (deprecated prototype)
#[allow(deprecated)]
pub use compliance_circuit::{
    generate_compliance_keys, generate_compliance_proof, generate_evm_vk_constants,
    serialize_vk_bytes, verify_compliance_proof, ComplianceCircuit, ComplianceKeys,
    TRUSTED_SETUP_SEED,
};

// Re-export Poseidon hash & canonical Grain LFSR (DEC-021)
pub use poseidon::{
    compute_nullifier, generate_grain_round_constants, native_poseidon_w5, GrainLfsr,
};

// Re-export Private Note V1 (DEC-016A & DEC-032 & DEC-035A & DEC-035B)
pub use note::{
    combine_limbs_to_quote_hash, compute_binding_commitment, compute_empty_hashes,
    compute_merkle_root, compute_scope_hash, derive_nullifier, derive_nullifier_epoch,
    derive_nullifier_key, derive_nullifier_v1, domain_binding_v1, domain_dummy_nullifier,
    domain_initial_note, domain_merkle_node, domain_mmr_bag, domain_note_commitment,
    domain_nullifier, domain_quote_binding, domain_scope_v1, fr_from_be_bytes, fr_to_be_bytes,
    merkle_append, merkle_hash, note_commitment, split_quote_hash_to_limbs, verify_merkle_proof,
    verify_value_conservation, MMRProof, MerkleMountainRange, PrivateNoteV1, DOMAIN_BINDING_V1_HEX,
    DOMAIN_MMR_BAG_BYTES, DOMAIN_SCOPE_V1_HEX, MERKLE_TREE_DEPTH,
};

// Re-export accounting state machine (DEC-016A Gate B)
pub use accounting::{ContractAccounting, TransitionResult};

// Re-export Private Note Circuit (DEC-016A Gate C & DEC-026 C-01 & DEC-035B)
pub use note_circuit::{
    create_dummy_circuit, extract_public_inputs, generate_note_circuit_evm_vk,
    generate_note_circuit_keys, generate_note_proof, get_or_init_note_circuit_keys,
    get_prepared_note_vk, verify_evm_note_proof, verify_note_proof, NoteCircuitKeys,
    PrivateNoteCircuit, DOMAIN_BINDING_V1, DOMAIN_SCOPE_V1, NOTE_CIRCUIT_SETUP_SEED,
    NUM_PUBLIC_INPUTS,
};

// Re-export Universal 2-in-2-out JoinSplit Circuit (DEC-030 Gate C0)
pub use joinsplit_circuit::{
    create_dummy_joinsplit_circuit, extract_joinsplit_public_inputs,
    generate_joinsplit_circuit_keys, generate_joinsplit_proof, verify_joinsplit_proof,
    JoinSplitCircuit, JoinSplitCircuitKeys, JOINSPLIT_CIRCUIT_SETUP_SEED,
    NUM_JOINSPLIT_PUBLIC_INPUTS,
};

#[cfg(test)]
mod tests {
    use super::*;
    use rand::thread_rng;

    #[test]
    fn test_bat_protocol_flow() {
        let mut rng = thread_rng();

        // 1. Setup Issuer
        let sk_iss = IssuerSecretKey::generate(&mut rng);
        let pk_iss = sk_iss.public_key();

        // 2. Client Blinds Message
        let message = b"ephemeral_public_key_123456";
        let (x, r) = client_blind(message, &mut rng);

        // 3. Issuer Signs Blinded Message with Masking Key
        let (masked_sig, k, com_k) = issuer_sign_blinded(&sk_iss, &x, &mut rng);

        // 4. Client Verifies Masked Signature off-chain (No trust needed yet)
        let verify_masked = client_verify_masked(&x, &com_k, &masked_sig);
        assert!(verify_masked, "Masked signature verification failed!");

        // 5. Client Unmasks Signature (after k is revealed on-chain)
        let alpha = client_unmask(&masked_sig, &r, &k).expect("Failed to invert blinding key");

        // 6. Verifier checks the final unmasked signature
        let verify_final = verify_unmasked(message, &alpha, &pk_iss);
        assert!(
            verify_final,
            "Final unmasked signature verification failed!"
        );
    }

    #[test]
    fn test_threshold_bdhke_flow() {
        let mut rng = thread_rng();

        // 1. Setup Issuer and split the key into 5 shares with threshold 3
        let sk_iss = IssuerSecretKey::generate(&mut rng);
        let pk_iss = sk_iss.public_key();

        let t = 3;
        let n = 5;
        let shares = split_secret_key(&sk_iss, t, n, &mut rng);
        assert_eq!(shares.len(), 5);

        // 2. Client Blinds Message
        let message = b"secret_message_for_federation";
        let (x, r) = client_blind(message, &mut rng);

        // 3. Leader generates random masking key k and its commitment com_k
        let k = Fr::rand(&mut rng);
        let com_k = MaskingKeyCommitment(pk_iss.0 * k);

        // 4. Three Guardians (e.g. 1, 3, and 5) sign the blinded message
        let sig_share1 = sign_share(&shares[0].1, &x, &k); // share 1 (index 1)
        let sig_share3 = sign_share(&shares[2].1, &x, &k); // share 3 (index 3)
        let sig_share5 = sign_share(&shares[4].1, &x, &k); // share 5 (index 5)

        // 5. Client aggregates partial signatures
        let gathered = vec![(1, sig_share1), (3, sig_share3), (5, sig_share5)];
        let aggregated_masked_sig = aggregate_shares(&gathered).unwrap();

        // 6. Client verifies aggregated masked signature against com_k
        let verify_masked = client_verify_masked(&x, &com_k, &aggregated_masked_sig);
        assert!(
            verify_masked,
            "Aggregated masked signature verification failed!"
        );

        // 7. Client unmasks signature once k is revealed
        let masking_key = MaskingKey(k);
        let alpha = client_unmask(&aggregated_masked_sig, &r, &masking_key)
            .expect("Failed to unmask aggregated signature");

        // 8. Verifier checks the final unmasked signature against aggregate public key pk_iss
        let verify_final = verify_unmasked(message, &alpha, &pk_iss);
        assert!(
            verify_final,
            "Final unmasked signature verification failed!"
        );
    }
}

#[cfg(test)]
mod formal_tests;
