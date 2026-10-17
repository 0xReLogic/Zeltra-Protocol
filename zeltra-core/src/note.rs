//! Private Note V1 Primitives & Commitments — DEC-016A & DEC-035B & DEC-036A
//!
//! Implements core private note ledger primitives:
//! - `PrivateNoteV1` struct with bit-packed multi-asset value representation (DEC-034A)
//! - Domain-separated note commitments via Poseidon width-5
//! - Owner-bound nullifier derivation (evolving epoch & Universal JoinSplit)
//! - Canonical field serialization and 128-bit limb decomposition (DEC-035A)
//! - Semantic scope and binding commitments (DEC-035B)
//!
//! Re-exports key derivation, encrypted memo, and MMR primitives from dedicated submodules:
//! - `crate::keys::*`
//! - `crate::memo::*`
//! - `crate::merkle::*`

use ark_bls12_381::Fr;
use ark_ff::{BigInteger, PrimeField};
use sha3::{Digest, Keccak256};

use crate::poseidon::native_poseidon_w5;

// Re-export dedicated submodules for complete backward compatibility
pub use crate::keys::*;
pub use crate::memo::*;
pub use crate::merkle::*;

// ═══════════════════════════════════════════════════════════════════════════
// Domain Separators
// ═══════════════════════════════════════════════════════════════════════════

/// Compute a domain separator as Fr::from(keccak256(label) mod p).
pub(crate) fn domain_from_label(label: &str) -> Fr {
    let hash = Keccak256::digest(label.as_bytes());
    // Reduce the 32-byte hash modulo the field order.
    // This is safe because the hash output is uniformly distributed
    // and the field is ~255 bits, so bias is negligible.
    Fr::from_be_bytes_mod_order(&hash)
}

/// Domain tag for note commitment: H(value, owner_key, rho, randomness).
pub fn domain_note_commitment() -> Fr {
    domain_from_label("zeltra.note.commitment.v1")
}

/// Domain tag for nullifier derivation: H(nullifier_key, commitment, leaf_index).
pub fn domain_nullifier() -> Fr {
    domain_from_label("zeltra.note.nullifier.v1")
}

/// Domain tag for nullifier key derivation (DEC-040A): H(spending_key, DOMAIN_NULLIFIER_KEY).
/// Canonical alias for domain_nullifier() preserving DEC-016A KAV compatibility.
pub fn domain_nullifier_key() -> Fr {
    domain_nullifier()
}

/// Domain tag for initial note (deposit -> note) binding.
pub fn domain_initial_note() -> Fr {
    domain_from_label("zeltra.note.initial.v1")
}

/// Domain tag for canonical quote & payment binding (Gate C0).
pub fn domain_quote_binding() -> Fr {
    domain_from_label("zeltra.quote.binding.v1")
}

/// Domain tag for canonical dummy nullifier derivation (DEC-030 Option A Constant-Topology).
pub fn domain_dummy_nullifier() -> Fr {
    domain_from_label("zeltra.note.dummy.nullifier.v1")
}

/// Canonical domain separator for contextual scope hash (DEC-035B: "ZELTRA_SCOPE_V1" = 0x5a454c5452415f53434f50455f5631).
pub const DOMAIN_SCOPE_V1_HEX: &str = "0x5a454c5452415f53434f50455f5631";

/// Domain tag for contextual scope hash (DEC-035B Stage 1).
pub fn domain_scope_v1() -> Fr {
    Fr::from_be_bytes_mod_order(b"ZELTRA_SCOPE_V1")
}

/// Canonical domain separator for formal binding commitment (DEC-035B: "ZELTRA_BINDING_V1" = 0x5a454c5452415f42494e44494e475f5631).
pub const DOMAIN_BINDING_V1_HEX: &str = "0x5a454c5452415f42494e44494e475f5631";

/// Domain tag for formal binding commitment (DEC-035B Stage 2).
pub fn domain_binding_v1() -> Fr {
    Fr::from_be_bytes_mod_order(b"ZELTRA_BINDING_V1")
}

/// Compute the native contextual scope hash via Poseidon-W5 (DEC-035B Stage 1).
pub fn compute_scope_hash(recipient: Fr, chain_id: Fr, contract_address: Fr, expiry: Fr) -> Fr {
    native_poseidon_w5(
        &[recipient, chain_id, contract_address, expiry],
        domain_scope_v1(),
    )
}

/// Compute the native formal binding commitment via Poseidon-W5 (DEC-035B Stage 2).
pub fn compute_binding_commitment(quote_hash_hi: Fr, quote_hash_lo: Fr, scope_hash: Fr) -> Fr {
    native_poseidon_w5(
        &[quote_hash_hi, quote_hash_lo, scope_hash, Fr::from(0u64)],
        domain_binding_v1(),
    )
}

/// Domain tag for deterministic note randomness derivation (DEC-037).
pub fn domain_note_randomness() -> Fr {
    domain_from_label("zeltra.note.randomness.v1")
}

/// Deterministically derives the note hiding randomness r from owner spending key and rho (DEC-037).
/// r = Poseidon_W5([owner_key, rho, 0, 0], domain_note_randomness)
pub fn derive_note_randomness(owner_key: Fr, rho: Fr) -> Fr {
    native_poseidon_w5(
        &[owner_key, rho, Fr::from(0u64), Fr::from(0u64)],
        domain_note_randomness(),
    )
}

// ═══════════════════════════════════════════════════════════════════════════
// PrivateNoteV1
// ═══════════════════════════════════════════════════════════════════════════

pub const ASSET_ID_WETH: u8 = 0;
pub const ASSET_ID_USDC: u8 = 1;

/// Private Note V1 — the fundamental unit of private value in Zeltra.
///
/// Each note represents a specific amount of stablecoin or native asset owned by a specific
/// user. Notes are consumed (nullified) when spent, producing change notes
/// for any remaining value.
///
/// Fields:
/// - `value`: Amount in base units (u64, 6 decimals for USDC, 9 decimals Gwei for WETH)
/// - `asset_id`: 0 for WETH, 1 for USDC
/// - `owner_key`: Owner's spending key (Fr element)
/// - `rho`: Unique note identifier / nullifier randomness (Fr element)
/// - `randomness`: Commitment trapdoor for hiding property (Fr element)
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrivateNoteV1 {
    pub value: u64,
    pub asset_id: u8,
    pub owner_key: Fr,
    pub rho: Fr,
    pub randomness: Fr,
}

impl PrivateNoteV1 {
    /// Create a new note with the given parameters.
    pub fn new(value: u64, asset_id: u8, owner_key: Fr, rho: Fr, randomness: Fr) -> Self {
        Self {
            value,
            asset_id,
            owner_key,
            rho,
            randomness,
        }
    }

    /// Compute the field bit-packed value Fr representation (DEC-034A).
    /// value_packed = value + asset_id * 2^64
    pub fn value_packed(&self) -> Fr {
        let two_pow_64 = Fr::from(1u128 << 64);
        Fr::from(self.value) + (Fr::from(self.asset_id as u64) * two_pow_64)
    }

    /// Compute the commitment for this note.
    ///
    /// commitment = Poseidon_W5(value_packed, owner_key, rho, randomness; domain)
    ///
    /// The commitment is binding (cannot change any field without changing
    /// the commitment) and hiding (reveals nothing about the note contents
    /// without knowing the preimage).
    pub fn commitment(&self) -> Fr {
        note_commitment(
            self.value,
            self.asset_id,
            self.owner_key,
            self.rho,
            self.randomness,
        )
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Note Commitment
// ═══════════════════════════════════════════════════════════════════════════

/// Compute a note commitment from individual fields including asset_id.
///
/// commitment = Poseidon_W5(value_packed, owner_key, rho, randomness; DOMAIN_NOTE_COMMITMENT_V1)
/// where value_packed = value + asset_id * 2^64
///
/// State layout: [value_packed, owner_key, rho, randomness, domain_tag]
pub fn note_commitment(value: u64, asset_id: u8, owner_key: Fr, rho: Fr, randomness: Fr) -> Fr {
    let two_pow_64 = Fr::from(1u128 << 64);
    let value_packed = Fr::from(value) + (Fr::from(asset_id as u64) * two_pow_64);
    note_commitment_packed(value_packed, owner_key, rho, randomness)
}

/// Compute a note commitment directly from value_packed field element.
pub fn note_commitment_packed(value_packed: Fr, owner_key: Fr, rho: Fr, randomness: Fr) -> Fr {
    let inputs = [value_packed, owner_key, rho, randomness];
    native_poseidon_w5(&inputs, domain_note_commitment())
}

// ═══════════════════════════════════════════════════════════════════════════
// Nullifier Derivation
// ═══════════════════════════════════════════════════════════════════════════

/// Derive an evolving nullifier for a note at a given Merkle tree position and epoch (DEC-033).
///
/// nullifier = Poseidon_W5(nullifier_key, commitment, Fr(leaf_index), Fr(epoch_id); DOMAIN_NULLIFIER_V1)
///
/// The nullifier binds to:
/// - The owner's nullifier key (proves ownership)
/// - The note commitment (identifies which note)
/// - The leaf index (prevents commitment reuse at different positions)
/// - The epoch ID (evolves nullifiers across generational windows, preventing cross-epoch replay)
/// - Domain tag (prevents cross-usage)
///
/// Privacy: Without knowledge of the nullifier key, it is infeasible to
/// link a nullifier to its corresponding commitment.
pub fn derive_nullifier(nullifier_key: Fr, commitment: Fr, leaf_index: u64, epoch_id: u32) -> Fr {
    let inputs = [
        nullifier_key,
        commitment,
        Fr::from(leaf_index),
        Fr::from(epoch_id as u64),
    ];
    native_poseidon_w5(&inputs, domain_nullifier())
}

/// Legacy 3-argument nullifier derivation defaulting to epoch_id = 0.
pub fn derive_nullifier_v1(nullifier_key: Fr, commitment: Fr, leaf_index: u64) -> Fr {
    derive_nullifier(nullifier_key, commitment, leaf_index, 0)
}

/// Alias for derive_nullifier with epoch_id.
pub fn derive_nullifier_epoch(
    nullifier_key: Fr,
    commitment: Fr,
    leaf_index: u64,
    epoch_id: u32,
) -> Fr {
    derive_nullifier(nullifier_key, commitment, leaf_index, epoch_id)
}

/// Canonical nullifier derivation for Universal JoinSplit note (DEC-036A / INV-JS-6):
/// nf = Poseidon_W5([nk, rho, epoch_id, 0], DOMAIN_NULLIFIER)
pub fn derive_joinsplit_nullifier(nullifier_key: Fr, rho: Fr, epoch_id: u64) -> Fr {
    let inputs = [nullifier_key, rho, Fr::from(epoch_id), Fr::from(0u64)];
    native_poseidon_w5(&inputs, domain_nullifier())
}

/// Canonical dummy nullifier derivation for JoinSplit dummy slot (DEC-036A / A2.3):
/// nf_dummy = Poseidon_W5([nk, session_nonce, 0, 0], DOMAIN_DUMMY_NULLIFIER)
pub fn derive_joinsplit_dummy_nullifier(nullifier_key: Fr, session_nonce: Fr) -> Fr {
    let inputs = [nullifier_key, session_nonce, Fr::from(0u64), Fr::from(0u64)];
    native_poseidon_w5(&inputs, domain_dummy_nullifier())
}

// ═══════════════════════════════════════════════════════════════════════════
// Value Conservation
// ═══════════════════════════════════════════════════════════════════════════

/// Verify value conservation for a spend transaction.
///
/// sum(input_values) == merchant_amount + protocol_fee + execution_fee + sum(change_values)
///
/// All arithmetic uses checked u64 operations. Overflow returns false.
pub fn verify_value_conservation(
    input_values: &[u64],
    merchant_amount: u64,
    protocol_fee: u64,
    execution_fee: u64,
    change_values: &[u64],
) -> bool {
    let sum_inputs: Option<u64> = input_values
        .iter()
        .try_fold(0u64, |acc, &v| acc.checked_add(v));
    let sum_changes: Option<u64> = change_values
        .iter()
        .try_fold(0u64, |acc, &v| acc.checked_add(v));

    match (sum_inputs, sum_changes) {
        (Some(inputs), Some(changes)) => {
            let total_debits = merchant_amount
                .checked_add(protocol_fee)
                .and_then(|s| s.checked_add(execution_fee))
                .and_then(|s| s.checked_add(changes));
            total_debits == Some(inputs)
        }
        _ => false, // overflow
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Serialization Helpers
// ═══════════════════════════════════════════════════════════════════════════

/// Serialize a field element to 32-byte big-endian representation.
pub fn fr_to_be_bytes(f: &Fr) -> [u8; 32] {
    let bigint = f.into_bigint();
    let mut bytes = [0u8; 32];
    // Arkworks BigInteger256 stores limbs in little-endian order.
    // We need big-endian byte output.
    let limb_bytes = bigint.to_bytes_le();
    // Reverse to get big-endian
    for (i, b) in limb_bytes.iter().rev().enumerate() {
        if i < 32 {
            bytes[i] = *b;
        }
    }
    bytes
}

/// Deserialize a field element from 32-byte big-endian representation.
/// Validates that the scalar element is strictly within the field modulus (< r).
pub fn fr_from_be_bytes(bytes: &[u8; 32]) -> Option<Fr> {
    use ark_serialize::CanonicalDeserialize;
    let mut le_bytes = [0u8; 32];
    for (i, b) in bytes.iter().rev().enumerate() {
        le_bytes[i] = *b;
    }
    Fr::deserialize_uncompressed(&le_bytes[..]).ok()
}

/// Decompose a 32-byte Big-Endian Keccak-256 quote hash into two canonical 128-bit field elements (DEC-035A).
///
/// Returns (quote_hash_hi, quote_hash_lo) where:
/// - quote_hash_hi = bytes[0..16] left-padded with 16 zeros
/// - quote_hash_lo = bytes[16..32] left-padded with 16 zeros
///
/// Because each 128-bit limb is strictly in [0, 2^128 - 1] and 2^128 - 1 < r_BLS12-381,
/// both limbs are guaranteed to be canonical field elements in Fr.
pub fn split_quote_hash_to_limbs(digest: &[u8; 32]) -> (Fr, Fr) {
    let mut hi_padded = [0u8; 32];
    hi_padded[16..32].copy_from_slice(&digest[0..16]);
    let mut lo_padded = [0u8; 32];
    lo_padded[16..32].copy_from_slice(&digest[16..32]);

    let hi = fr_from_be_bytes(&hi_padded)
        .expect("128-bit value left-padded with 16 zero bytes is strictly < r");
    let lo = fr_from_be_bytes(&lo_padded)
        .expect("128-bit value left-padded with 16 zero bytes is strictly < r");
    (hi, lo)
}

/// Recombine two 128-bit field elements back into a 32-byte Big-Endian quote hash (DEC-035A).
///
/// Extracts the lower 16 bytes of both hi and lo field elements:
/// - digest[0..16] = hi_bytes[16..32]
/// - digest[16..32] = lo_bytes[16..32]
pub fn combine_limbs_to_quote_hash(hi: &Fr, lo: &Fr) -> [u8; 32] {
    let hi_bytes = fr_to_be_bytes(hi);
    let lo_bytes = fr_to_be_bytes(lo);
    let mut digest = [0u8; 32];
    digest[0..16].copy_from_slice(&hi_bytes[16..32]);
    digest[16..32].copy_from_slice(&lo_bytes[16..32]);
    digest
}

// ═══════════════════════════════════════════════════════════════════════════
// Tests (Modularized into note_tests.rs)
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
#[path = "note_tests.rs"]
mod tests;
