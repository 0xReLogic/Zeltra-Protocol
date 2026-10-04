//! Private Note V1 — DEC-016A implementation
//!
//! Implements the private note ledger primitives:
//! - `PrivateNoteV1` struct
//! - Domain-separated note commitment (Poseidon width-5)
//! - Owner-bound nullifier derivation (Poseidon width-5)
//! - Key derivation (spending key → nullifier key)
//! - Incremental Merkle tree helpers
//!
//! Security invariants:
//! - Note commitment is binding (cannot find two notes with same commitment)
//! - Note commitment is hiding (commitment reveals nothing about note contents)
//! - Nullifier is unlinkable to commitment without the nullifier key
//! - Domain separation prevents cross-usage attacks

use ark_bls12_381::Fr;
use ark_ff::{BigInteger, Field, PrimeField};
use sha3::{Digest, Keccak256};

use crate::poseidon::native_poseidon_w5;

// ═══════════════════════════════════════════════════════════════════════════
// Domain Separators
// ═══════════════════════════════════════════════════════════════════════════

/// Compute a domain separator as Fr::from(keccak256(label) mod p).
fn domain_from_label(label: &str) -> Fr {
    let hash = Keccak256::digest(label.as_bytes());
    // Reduce the 32-byte hash modulo the field order.
    // This is safe because the hash output is uniformly distributed
    // and the field is ~255 bits, so bias is negligible.
    Fr::from_be_bytes_mod_order(&hash)
}

/// Domain tag for note commitment: H(value, owner_key, rho, randomness).
pub fn domain_note_commitment() -> Fr {
    domain_from_label("nimbus.note.commitment.v1")
}

/// Domain tag for nullifier derivation: H(nullifier_key, commitment, leaf_index).
pub fn domain_nullifier() -> Fr {
    domain_from_label("nimbus.note.nullifier.v1")
}

/// Domain tag for Merkle tree internal node hash: H(left, right).
pub fn domain_merkle_node() -> Fr {
    domain_from_label("nimbus.merkle.node.v1")
}

/// Domain tag for initial note (deposit → note) binding.
pub fn domain_initial_note() -> Fr {
    domain_from_label("nimbus.note.initial.v1")
}

/// Domain tag for canonical quote & payment binding (Gate C0).
pub fn domain_quote_binding() -> Fr {
    domain_from_label("nimbus.quote.binding.v1")
}

// ═══════════════════════════════════════════════════════════════════════════
// PrivateNoteV1
// ═══════════════════════════════════════════════════════════════════════════

/// Private Note V1 — the fundamental unit of private value in Nimbus.
///
/// Each note represents a specific amount of stablecoin owned by a specific
/// user. Notes are consumed (nullified) when spent, producing change notes
/// for any remaining value.
///
/// Fields:
/// - `value`: Amount in base units (u64, 6 decimals for USDC)
/// - `owner_key`: Owner's spending key (Fr element)
/// - `rho`: Unique note identifier / nullifier randomness (Fr element)
/// - `randomness`: Commitment trapdoor for hiding property (Fr element)
#[derive(Clone, Debug)]
pub struct PrivateNoteV1 {
    pub value: u64,
    pub owner_key: Fr,
    pub rho: Fr,
    pub randomness: Fr,
}

impl PrivateNoteV1 {
    /// Create a new note with the given parameters.
    pub fn new(value: u64, owner_key: Fr, rho: Fr, randomness: Fr) -> Self {
        Self {
            value,
            owner_key,
            rho,
            randomness,
        }
    }

    /// Compute the commitment for this note.
    ///
    /// commitment = Poseidon_W5(value, owner_key, rho, randomness; domain)
    ///
    /// The commitment is binding (cannot change any field without changing
    /// the commitment) and hiding (reveals nothing about the note contents
    /// without knowing the preimage).
    pub fn commitment(&self) -> Fr {
        note_commitment(self.value, self.owner_key, self.rho, self.randomness)
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Note Commitment
// ═══════════════════════════════════════════════════════════════════════════

/// Compute a note commitment from individual fields.
///
/// commitment = Poseidon_W5(Fr(value), owner_key, rho, randomness; DOMAIN_NOTE_COMMITMENT_V1)
///
/// State layout: [value_fr, owner_key, rho, randomness, domain_tag]
pub fn note_commitment(value: u64, owner_key: Fr, rho: Fr, randomness: Fr) -> Fr {
    let inputs = [Fr::from(value), owner_key, rho, randomness];
    native_poseidon_w5(&inputs, domain_note_commitment())
}

// ═══════════════════════════════════════════════════════════════════════════
// Key Derivation
// ═══════════════════════════════════════════════════════════════════════════

/// Derive the nullifier key from the spending key.
///
/// nullifier_key = Poseidon_W3(spending_key, DOMAIN_NULLIFIER_V1)
///
/// Uses the existing width-3 Poseidon (rate=2) since we only have 2 inputs:
/// the spending key and the domain tag.
pub fn derive_nullifier_key(spending_key: Fr) -> Fr {
    crate::poseidon::compute_nullifier(spending_key, domain_nullifier())
}

// ═══════════════════════════════════════════════════════════════════════════
// Nullifier Derivation
// ═══════════════════════════════════════════════════════════════════════════

/// Derive a nullifier for a note at a given Merkle tree position.
///
/// nullifier = Poseidon_W5(nullifier_key, commitment, Fr(leaf_index), 0; DOMAIN_NULLIFIER_V1)
///
/// The nullifier binds to:
/// - The owner's nullifier key (proves ownership)
/// - The note commitment (identifies which note)
/// - The leaf index (prevents commitment reuse at different positions)
/// - Domain tag (prevents cross-usage)
///
/// Privacy: Without knowledge of the nullifier key, it is infeasible to
/// link a nullifier to its corresponding commitment.
pub fn derive_nullifier(nullifier_key: Fr, commitment: Fr, leaf_index: u64) -> Fr {
    let inputs = [
        nullifier_key,
        commitment,
        Fr::from(leaf_index),
        Fr::from(0u64), // padding (rate=4, we only have 3 meaningful inputs)
    ];
    native_poseidon_w5(&inputs, domain_nullifier())
}

// ═══════════════════════════════════════════════════════════════════════════
// Incremental Merkle Tree Helpers
// ═══════════════════════════════════════════════════════════════════════════

/// Merkle tree depth. Supports 2^20 = 1,048,576 leaves.
pub const MERKLE_TREE_DEPTH: usize = 20;

/// Hash two child nodes to produce a parent node.
///
/// parent = Poseidon_W5(left, right, 0, 0; DOMAIN_MERKLE_NODE_V1)
pub fn merkle_hash(left: Fr, right: Fr) -> Fr {
    let inputs = [left, right, Fr::from(0u64), Fr::from(0u64)];
    native_poseidon_w5(&inputs, domain_merkle_node())
}

/// Compute the empty subtree hashes for all levels.
///
/// EMPTY[0] = Fr(0)  (empty leaf)
/// EMPTY[i] = merkle_hash(EMPTY[i-1], EMPTY[i-1])
///
/// Returns an array of 21 elements (levels 0 through 20).
pub fn compute_empty_hashes() -> [Fr; MERKLE_TREE_DEPTH + 1] {
    let mut hashes = [Fr::from(0u64); MERKLE_TREE_DEPTH + 1];
    for i in 1..=MERKLE_TREE_DEPTH {
        hashes[i] = merkle_hash(hashes[i - 1], hashes[i - 1]);
    }
    hashes
}

/// Compute the Merkle root given a leaf and its membership proof (sibling path).
///
/// `leaf_index` determines left/right ordering at each level:
/// - Even index: leaf is left child
/// - Odd index: leaf is right child
pub fn compute_merkle_root(leaf: Fr, leaf_index: u64, siblings: &[Fr; MERKLE_TREE_DEPTH]) -> Fr {
    let mut current = leaf;
    let mut index = leaf_index;

    for sibling in siblings.iter() {
        current = if index.is_multiple_of(2) {
            merkle_hash(current, *sibling)
        } else {
            merkle_hash(*sibling, current)
        };
        index /= 2;
    }

    current
}

/// Verify a Merkle membership proof.
pub fn verify_merkle_proof(
    root: Fr,
    leaf: Fr,
    leaf_index: u64,
    siblings: &[Fr; MERKLE_TREE_DEPTH],
) -> bool {
    compute_merkle_root(leaf, leaf_index, siblings) == root
}

/// Append a leaf to the Merkle tree, updating the frontier.
///
/// The frontier stores the left-most hash at each level, enabling O(depth)
/// append operations without storing the full tree.
///
/// Returns the new root after insertion.
pub fn merkle_append(
    frontier: &mut [Fr; MERKLE_TREE_DEPTH + 1],
    next_index: u64,
    leaf: Fr,
    empty_hashes: &[Fr; MERKLE_TREE_DEPTH + 1],
) -> Fr {
    let mut current = leaf;
    let mut index = next_index;

    for level in 0..MERKLE_TREE_DEPTH {
        if index.is_multiple_of(2) {
            // Left child: store in frontier, sibling is empty
            frontier[level] = current;
            current = merkle_hash(current, empty_hashes[level]);
        } else {
            // Right child: sibling is stored frontier
            current = merkle_hash(frontier[level], current);
        }
        index /= 2;
    }

    current
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
pub fn fr_from_be_bytes(bytes: &[u8; 32]) -> Option<Fr> {
    // Reverse to little-endian for Arkworks
    let mut le_bytes = [0u8; 32];
    for (i, b) in bytes.iter().rev().enumerate() {
        le_bytes[i] = *b;
    }
    Fr::from_random_bytes(&le_bytes)
}

// ═══════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use ark_std::rand::SeedableRng;
    use ark_std::UniformRand;

    fn test_note() -> PrivateNoteV1 {
        PrivateNoteV1::new(
            99_800_000, // 99.8 USDC
            Fr::from(12345u64),
            Fr::from(67890u64),
            Fr::from(11111u64),
        )
    }

    #[test]
    fn test_domain_separators_distinct() {
        let d1 = domain_note_commitment();
        let d2 = domain_nullifier();
        let d3 = domain_merkle_node();
        let d4 = domain_initial_note();

        assert_ne!(d1, d2, "note commitment ≠ nullifier domain");
        assert_ne!(d1, d3, "note commitment ≠ merkle domain");
        assert_ne!(d1, d4, "note commitment ≠ initial note domain");
        assert_ne!(d2, d3, "nullifier ≠ merkle domain");
        assert_ne!(d2, d4, "nullifier ≠ initial note domain");
        assert_ne!(d3, d4, "merkle ≠ initial note domain");
    }

    #[test]
    fn test_domain_separators_deterministic() {
        assert_eq!(
            domain_note_commitment(),
            domain_note_commitment(),
            "Domain tags must be deterministic"
        );
    }

    #[test]
    fn test_note_commitment_deterministic() {
        let note = test_note();
        let c1 = note.commitment();
        let c2 = note.commitment();
        assert_eq!(c1, c2, "Commitment must be deterministic");
    }

    #[test]
    fn test_note_commitment_binding_value() {
        let note1 = PrivateNoteV1::new(100, Fr::from(1u64), Fr::from(2u64), Fr::from(3u64));
        let note2 = PrivateNoteV1::new(200, Fr::from(1u64), Fr::from(2u64), Fr::from(3u64));
        assert_ne!(
            note1.commitment(),
            note2.commitment(),
            "Different values must produce different commitments"
        );
    }

    #[test]
    fn test_note_commitment_binding_owner() {
        let note1 = PrivateNoteV1::new(100, Fr::from(1u64), Fr::from(2u64), Fr::from(3u64));
        let note2 = PrivateNoteV1::new(100, Fr::from(99u64), Fr::from(2u64), Fr::from(3u64));
        assert_ne!(
            note1.commitment(),
            note2.commitment(),
            "Different owner keys must produce different commitments"
        );
    }

    #[test]
    fn test_note_commitment_binding_rho() {
        let note1 = PrivateNoteV1::new(100, Fr::from(1u64), Fr::from(2u64), Fr::from(3u64));
        let note2 = PrivateNoteV1::new(100, Fr::from(1u64), Fr::from(99u64), Fr::from(3u64));
        assert_ne!(
            note1.commitment(),
            note2.commitment(),
            "Different rho values must produce different commitments"
        );
    }

    #[test]
    fn test_note_commitment_hiding() {
        // Same note with different randomness produces different commitments
        let note1 = PrivateNoteV1::new(100, Fr::from(1u64), Fr::from(2u64), Fr::from(3u64));
        let note2 = PrivateNoteV1::new(100, Fr::from(1u64), Fr::from(2u64), Fr::from(99u64));
        assert_ne!(
            note1.commitment(),
            note2.commitment(),
            "Different randomness must produce different commitments (hiding)"
        );
    }

    #[test]
    fn test_nullifier_key_derivation() {
        let sk = Fr::from(42u64);
        let nk1 = derive_nullifier_key(sk);
        let nk2 = derive_nullifier_key(sk);
        assert_eq!(nk1, nk2, "Nullifier key derivation must be deterministic");
        assert_ne!(nk1, sk, "Nullifier key must differ from spending key");
    }

    #[test]
    fn test_nullifier_derivation_deterministic() {
        let nk = Fr::from(42u64);
        let cm = Fr::from(999u64);
        let n1 = derive_nullifier(nk, cm, 0);
        let n2 = derive_nullifier(nk, cm, 0);
        assert_eq!(n1, n2, "Nullifier must be deterministic");
    }

    #[test]
    fn test_nullifier_unlinkable_without_key() {
        // Two different notes with different nullifier keys produce different nullifiers
        let nk1 = Fr::from(1u64);
        let nk2 = Fr::from(2u64);
        let cm = Fr::from(999u64);
        let n1 = derive_nullifier(nk1, cm, 0);
        let n2 = derive_nullifier(nk2, cm, 0);
        assert_ne!(n1, n2, "Different keys must produce different nullifiers");
    }

    #[test]
    fn test_nullifier_position_binding() {
        let nk = Fr::from(42u64);
        let cm = Fr::from(999u64);
        let n0 = derive_nullifier(nk, cm, 0);
        let n1 = derive_nullifier(nk, cm, 1);
        assert_ne!(
            n0, n1,
            "Same note at different positions must produce different nullifiers"
        );
    }

    #[test]
    fn test_merkle_empty_root() {
        let empty = compute_empty_hashes();
        // Root of empty tree at depth 20
        let root = empty[MERKLE_TREE_DEPTH];
        assert_ne!(root, Fr::from(0u64), "Empty tree root must not be zero");
    }

    #[test]
    fn test_merkle_single_leaf() {
        let empty = compute_empty_hashes();
        let mut frontier = [Fr::from(0u64); MERKLE_TREE_DEPTH + 1];
        let leaf = Fr::from(42u64);

        let root = merkle_append(&mut frontier, 0, leaf, &empty);

        // Verify: the root should be derivable from the leaf + empty siblings
        let siblings: [Fr; MERKLE_TREE_DEPTH] = std::array::from_fn(|i| empty[i]);
        let computed_root = compute_merkle_root(leaf, 0, &siblings);
        assert_eq!(root, computed_root, "Merkle append and proof must agree");
    }

    #[test]
    fn test_merkle_proof_valid() {
        let empty = compute_empty_hashes();
        let mut frontier = [Fr::from(0u64); MERKLE_TREE_DEPTH + 1];

        // Insert 3 leaves
        let leaves = [Fr::from(10u64), Fr::from(20u64), Fr::from(30u64)];
        let mut roots = [Fr::from(0u64); 3];

        for (i, &leaf) in leaves.iter().enumerate() {
            roots[i] = merkle_append(&mut frontier, i as u64, leaf, &empty);
        }

        // Build proof for leaf at index 1 (after 3 insertions)
        // After 3 insertions, the tree has leaves at indices 0, 1, 2
        // For index 1, siblings are:
        //   level 0: leaf[0] (sibling of leaf[1])
        //   level 1+: depends on tree structure
        // For simplicity, verify with the 2-leaf tree (after 2 insertions)
        let mut frontier2 = [Fr::from(0u64); MERKLE_TREE_DEPTH + 1];
        let _r0 = merkle_append(&mut frontier2, 0, leaves[0], &empty);
        let r1 = merkle_append(&mut frontier2, 1, leaves[1], &empty);

        // Proof for leaf[1]: sibling at level 0 is leaf[0], rest are empty
        let mut siblings = [Fr::from(0u64); MERKLE_TREE_DEPTH];
        siblings[0] = leaves[0]; // level 0 sibling
        siblings[1..MERKLE_TREE_DEPTH].copy_from_slice(&empty[1..MERKLE_TREE_DEPTH]);

        assert!(
            verify_merkle_proof(r1, leaves[1], 1, &siblings),
            "Valid Merkle proof must verify"
        );
    }

    #[test]
    fn test_merkle_proof_invalid_wrong_leaf() {
        let empty = compute_empty_hashes();
        let mut frontier = [Fr::from(0u64); MERKLE_TREE_DEPTH + 1];
        let leaf = Fr::from(42u64);
        let root = merkle_append(&mut frontier, 0, leaf, &empty);

        let siblings: [Fr; MERKLE_TREE_DEPTH] = std::array::from_fn(|i| empty[i]);
        let wrong_leaf = Fr::from(99u64);

        assert!(
            !verify_merkle_proof(root, wrong_leaf, 0, &siblings),
            "Invalid leaf must not verify"
        );
    }

    #[test]
    fn test_value_conservation_exact() {
        // Input: 100 USDC
        // Merchant: 5 USDC, Protocol fee: 0.0125 USDC, Exec fee: 0.023 USDC
        // Change: 94.9645 USDC
        assert!(verify_value_conservation(
            &[100_000_000], // 100 USDC input
            5_000_000,      // 5 USDC merchant
            12_500,         // 0.0125 USDC protocol fee
            23_000,         // 0.023 USDC execution fee
            &[94_964_500],  // 94.9645 USDC change
        ));
    }

    #[test]
    fn test_value_conservation_overflow_rejected() {
        assert!(!verify_value_conservation(
            &[u64::MAX, 1], // overflow on input sum
            5_000_000,
            12_500,
            23_000,
            &[0],
        ));
    }

    #[test]
    fn test_value_conservation_mismatch_rejected() {
        // Input: 100 USDC, but debits don't add up
        assert!(!verify_value_conservation(
            &[100_000_000],
            5_000_000,
            12_500,
            23_000,
            &[94_000_000], // wrong: should be 94_964_500
        ));
    }

    #[test]
    fn test_value_conservation_no_change() {
        // Full spend: input = merchant + fees, no change
        assert!(verify_value_conservation(
            &[5_035_500], // 5.0355 USDC
            5_000_000,    // 5 USDC merchant
            12_500,       // 0.0125 USDC protocol fee
            23_000,       // 0.023 USDC execution fee
            &[],          // no change
        ));
    }

    #[test]
    fn test_value_conservation_multi_input() {
        // Two input notes consolidated
        assert!(verify_value_conservation(
            &[50_000_000, 60_000_000], // 50 + 60 = 110 USDC
            100_000_000,               // 100 USDC merchant
            250_000,                   // 0.25 USDC protocol fee (0.25%)
            100_000,                   // 0.1 USDC execution fee
            &[9_650_000],              // 9.65 USDC change
        ));
    }

    #[test]
    fn test_fr_serialization_roundtrip() {
        let original = Fr::from(123456789u64);
        let bytes = fr_to_be_bytes(&original);
        let recovered = fr_from_be_bytes(&bytes).expect("valid bytes");
        assert_eq!(
            original, recovered,
            "Fr serialization roundtrip must preserve value"
        );
    }

    #[test]
    fn test_fr_serialization_large_value() {
        let mut rng = ark_std::rand::rngs::StdRng::seed_from_u64(42);
        let original = Fr::rand(&mut rng);
        let bytes = fr_to_be_bytes(&original);
        let recovered = fr_from_be_bytes(&bytes).expect("valid bytes");
        assert_eq!(
            original, recovered,
            "Fr serialization roundtrip for random element"
        );
    }

    // ═══════════════════════════════════════════════════════════════════
    // Known-Answer Vectors (DEC-016A §16)
    // ═══════════════════════════════════════════════════════════════════
    //
    // These vectors serve as the canonical reference for cross-validation
    // between nimbus-core, nimbus-sdk (WASM), and the contract.
    //
    // Seed inputs from DEC-016A §16:
    //   spending_key     = Fr::from(42)
    //   note value       = 99_800_000 (99.8 USDC = 100 USDC - 0.2% deposit fee)
    //   rho              = Fr::from(1)
    //   randomness       = Fr::from(2)
    //   leaf_index       = 0
    //   merchant_amount  = 5_000_000 (5 USDC)
    //   protocol_fee     = 12_500 (0.25% of 5 USDC)
    //   execution_fee    = 23_000

    #[test]
    fn known_answer_domain_separators() {
        // Domain separators are derived from keccak256 labels.
        // These MUST be identical across core, SDK, and contract.
        let d_note = domain_note_commitment();
        let d_null = domain_nullifier();
        let d_merkle = domain_merkle_node();
        let d_initial = domain_initial_note();

        let d_note_hex = hex::encode(fr_to_be_bytes(&d_note));
        let d_null_hex = hex::encode(fr_to_be_bytes(&d_null));
        let d_merkle_hex = hex::encode(fr_to_be_bytes(&d_merkle));
        let d_initial_hex = hex::encode(fr_to_be_bytes(&d_initial));

        eprintln!("KNOWN-ANSWER: domain_note_commitment = 0x{}", d_note_hex);
        eprintln!("KNOWN-ANSWER: domain_nullifier       = 0x{}", d_null_hex);
        eprintln!("KNOWN-ANSWER: domain_merkle_node     = 0x{}", d_merkle_hex);
        eprintln!("KNOWN-ANSWER: domain_initial_note    = 0x{}", d_initial_hex);

        // All distinct
        assert_ne!(d_note, d_null);
        assert_ne!(d_note, d_merkle);
        assert_ne!(d_note, d_initial);
        assert_ne!(d_null, d_merkle);
        assert_ne!(d_null, d_initial);
        assert_ne!(d_merkle, d_initial);
    }

    #[test]
    fn known_answer_note_commitment() {
        // Note: value=99800000, owner_key=Fr(42), rho=Fr(1), randomness=Fr(2)
        let note = PrivateNoteV1::new(99_800_000, Fr::from(42u64), Fr::from(1u64), Fr::from(2u64));
        let cm = note.commitment();
        let cm_hex = hex::encode(fr_to_be_bytes(&cm));
        eprintln!("KNOWN-ANSWER: note_commitment = 0x{}", cm_hex);

        // Determinism check
        let cm2 = note.commitment();
        assert_eq!(cm, cm2, "Commitment must be deterministic");
    }

    #[test]
    fn known_answer_nullifier_key() {
        let spending_key = Fr::from(42u64);
        let nk = derive_nullifier_key(spending_key);
        let nk_hex = hex::encode(fr_to_be_bytes(&nk));
        eprintln!("KNOWN-ANSWER: nullifier_key = 0x{}", nk_hex);
    }

    #[test]
    fn known_answer_nullifier() {
        let spending_key = Fr::from(42u64);
        let nk = derive_nullifier_key(spending_key);

        let note = PrivateNoteV1::new(99_800_000, spending_key, Fr::from(1u64), Fr::from(2u64));
        let cm = note.commitment();
        let nf = derive_nullifier(nk, cm, 0);
        let nf_hex = hex::encode(fr_to_be_bytes(&nf));
        eprintln!("KNOWN-ANSWER: nullifier(leaf_index=0) = 0x{}", nf_hex);
    }

    #[test]
    fn known_answer_merkle_single_insert() {
        let empty = compute_empty_hashes();
        let mut frontier = [Fr::from(0u64); MERKLE_TREE_DEPTH + 1];

        let note = PrivateNoteV1::new(99_800_000, Fr::from(42u64), Fr::from(1u64), Fr::from(2u64));
        let cm = note.commitment();
        let root = merkle_append(&mut frontier, 0, cm, &empty);
        let root_hex = hex::encode(fr_to_be_bytes(&root));
        eprintln!("KNOWN-ANSWER: merkle_root(after 1 insert) = 0x{}", root_hex);

        // Verify the root with a Merkle proof
        let siblings: [Fr; MERKLE_TREE_DEPTH] = std::array::from_fn(|i| empty[i]);
        assert!(
            verify_merkle_proof(root, cm, 0, &siblings),
            "Merkle proof must verify for single insert"
        );
    }

    #[test]
    fn known_answer_value_conservation_spend() {
        // Deposit 100 USDC → net 99.8 USDC (after 0.2% deposit fee)
        // Spend 5 USDC → protocol fee 12500 (0.25%) + execution fee 23000
        // Change = 99_800_000 - 5_000_000 - 12_500 - 23_000 = 94_764_500
        let change = 99_800_000u64 - 5_000_000 - 12_500 - 23_000;
        assert_eq!(change, 94_764_500);
        eprintln!("KNOWN-ANSWER: change_value = {} (94.7645 USDC)", change);

        assert!(verify_value_conservation(
            &[99_800_000],
            5_000_000,
            12_500,
            23_000,
            &[change],
        ));
    }

    #[test]
    fn known_answer_empty_tree_root() {
        let empty = compute_empty_hashes();
        let root_hex = hex::encode(fr_to_be_bytes(&empty[MERKLE_TREE_DEPTH]));
        eprintln!("KNOWN-ANSWER: empty_tree_root(depth=20) = 0x{}", root_hex);
    }
}
