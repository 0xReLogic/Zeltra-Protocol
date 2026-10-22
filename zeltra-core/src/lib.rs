pub mod accounting;
pub mod circuit_gadgets;
pub mod evm;
pub mod fees;
pub mod jacobian;
pub mod joinsplit_circuit;
pub mod keys;
pub mod memo;
pub mod memo_ct;
pub mod merkle;
pub mod note;
pub mod note_circuit;
pub mod poseidon;
pub mod selective_disclosure_circuit;
pub mod serialization;

pub use ark_bls12_381::{Bls12_381, Fr, FrConfig, G1Projective, G2Projective};
pub use ark_ec::pairing::Pairing;
pub use ark_ec::PrimeGroup;
pub use ark_ff::{Field, PrimeField, UniformRand};

// Re-export circuit gadgets (DEC-034A & DEC-040A)
pub use circuit_gadgets::{
    enforce_dummy_range_binding, enforce_joinsplit_flags_packed, enforce_packed_value_and_asset,
    enforce_private_note_flags_packed, poseidon_w3_gadget, poseidon_w5_gadget,
};

// Re-export serialization helpers
pub use serialization::{deserialize_from_bytes, serialize_to_bytes};

// Re-export EVM helpers
pub use evm::{
    from_evm_g1, from_evm_g2, from_evm_proof, from_evm_scalar, to_evm_g1, to_evm_g2, to_evm_scalar,
};

// Re-export fee policy helpers
pub use fees::{
    ceil_div, deposit_fee, fee_round_up, gross_up_for_exact_net, gross_up_private_spend_amount,
    gwei_to_wei, net_after_fee, private_spend_fee, quote_cross_chain_private_spend_with_fee,
    quote_execution_fee, quote_execution_fee_with_pass_through, quote_private_spend,
    quote_private_spend_with_fee, quote_standalone_rollover, spend_fee_bps_for_holding,
    wei_to_gwei, SpendQuote, DEFAULT_RELAYER_MARKUP_BPS, DEPOSIT_FEE_BPS, FEE_DENOMINATOR_BPS,
    MIN_STANDALONE_ROLLOVER_THRESHOLD_USDC, PRIVATE_SPEND_FEE_BPS, SPEND_FEE_30DAY_BPS,
    THIRTY_DAYS_SECS, WEI_PER_GWEI,
};

// Re-export Poseidon hash & canonical Grain LFSR (DEC-021)
pub use poseidon::{
    compute_nullifier, generate_grain_round_constants, native_poseidon_w3, native_poseidon_w5,
    GrainLfsr,
};

// Re-export keys, memo, merkle, and note primitives (DEC-016A, DEC-032, DEC-035A, DEC-035B, DEC-037, DEC-040A, DEC-043A, DEC-044)
#[allow(unused_imports)]
pub use keys::*;
#[allow(unused_imports)]
pub use memo::*;
#[allow(unused_imports)]
pub use memo_ct::*;
#[allow(unused_imports)]
pub use merkle::*;
pub use note::*;

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

// Re-export Universal 2-in-2-out JoinSplit Circuit (DEC-036A Gate C0)
pub use joinsplit_circuit::{
    create_dummy_joinsplit_circuit, extract_joinsplit_public_inputs,
    generate_joinsplit_circuit_keys, generate_joinsplit_proof, get_or_init_joinsplit_circuit_keys,
    verify_joinsplit_proof, JoinSplitCircuit, JoinSplitCircuitKeys, JOINSPLIT_CIRCUIT_SETUP_SEED,
    NUM_JOINSPLIT_PUBLIC_INPUTS,
};

// Re-export Canonical Dummy Constants (DEC-046)
pub use circuit_gadgets::{
    CANONICAL_DUMMY_DIFF_INV, CANONICAL_DUMMY_LEAF_INDEX, CANONICAL_DUMMY_OWNER,
    CANONICAL_DUMMY_RANDOMNESS, CANONICAL_DUMMY_RHO, CANONICAL_DUMMY_SESSION_NONCE,
    CANONICAL_DUMMY_VALUE,
};

// Re-export Selective Disclosure Circuit (DEC-040A)
pub use selective_disclosure_circuit::{
    decompose_challenge_to_limbs, generate_disclosure_proof, setup_disclosure_circuit_params,
    verify_disclosure_proof, verify_disclosure_proof_unprepared, SelectiveDisclosureCircuit,
    NUM_DISCLOSURE_PUBLIC_INPUTS,
};

// Re-export Automated Jacobian Diagnostics & Rigidity (DEC-047)
pub use jacobian::{
    extract_witness_and_matrices, run_jacobian_diagnostic, AffineLineCertificate,
    AugmentedJacobian, DiagnosticClassification, EchelonBasis, SparseEntry, SparseMatrixFr,
};

pub use chacha20;
pub use hkdf;
pub use subtle;
pub use x25519_dalek;

#[cfg(test)]
mod formal_tests;
