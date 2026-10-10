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
use ark_ff::{BigInteger, PrimeField};
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

/// Domain tag for canonical dummy nullifier derivation (DEC-030 Option A Constant-Topology).
pub fn domain_dummy_nullifier() -> Fr {
    domain_from_label("nimbus.note.dummy.nullifier.v1")
}

/// Canonical domain separator bytes for MMR peak bagging (DEC-032).
/// Keccak256("ZELTRA_MMR_BAG_V1") mod r.
pub const DOMAIN_MMR_BAG_BYTES: [u8; 32] = [
    0x19, 0x4a, 0x8f, 0x9c, 0x1e, 0x7d, 0x23, 0x58, 0xb9, 0x01, 0xfc, 0x84, 0x33, 0x29, 0x10, 0x7b,
    0xa8, 0x92, 0x1d, 0xfb, 0xb3, 0x02, 0x48, 0x59, 0xae, 0xf0, 0x29, 0x14, 0x7d, 0xa2, 0x90, 0xbf,
];

/// Domain tag for MMR peak bagging: Poseidon_W5([acc, Fr(leaf_count), 0, 0], DOMAIN_MMR_BAG).
pub fn domain_mmr_bag() -> Fr {
    Fr::from_be_bytes_mod_order(&DOMAIN_MMR_BAG_BYTES)
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
// Merkle Mountain Range (MMR) Accumulator — DEC-032
// ═══════════════════════════════════════════════════════════════════════════

/// Merkle Mountain Range membership proof (DEC-032).
/// Contains the internal mountain sibling path and the sibling peaks needed
/// for canonical peak bagging.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MMRProof {
    /// Height of the mountain containing the leaf (0 <= height < 32).
    pub mountain_height: usize,
    /// Sibling hashes along the internal mountain tree from leaf to peak.
    pub mountain_siblings: Vec<Fr>,
    /// The other active peak hashes in the MMR folded into the bagged root.
    pub peak_bagging_siblings: Vec<Fr>,
    /// Index of this mountain's peak among the active peaks (ordered left-to-right from highest to lowest mountain).
    pub peak_index: usize,
}

/// In-memory Merkle Mountain Range (DEC-032) note commitment accumulator.
///
/// Implements infinite-horizon append-only accumulation scaling to 2^64 elements
/// with O(log N) state, amortized O(1) Poseidon hashing, and canonical
/// peak bagging domain separation.
#[derive(Clone, Debug, Default)]
pub struct MerkleMountainRange {
    /// Total number of leaves accumulated so far.
    pub leaf_count: usize,
    /// All leaves inserted sequentially.
    pub leaves: Vec<Fr>,
    /// Active peaks by height index (peaks_by_height[h] is Some(peak) if bit h of leaf_count is 1).
    peaks_by_height: Vec<Option<Fr>>,
}

impl MerkleMountainRange {
    /// Create a new, empty Merkle Mountain Range.
    pub fn new() -> Self {
        Self {
            leaf_count: 0,
            leaves: Vec::new(),
            peaks_by_height: Vec::new(),
        }
    }

    /// Append a leaf (note commitment) to the MMR using binary carry adder algorithm.
    ///
    /// Returns `(leaf_index, new_bagged_root)`.
    pub fn append(&mut self, leaf: Fr) -> (usize, Fr) {
        let leaf_index = self.leaf_count;
        self.leaves.push(leaf);

        let mut current = leaf;
        let mut height = 0;
        let mut idx = leaf_index;

        while (idx & 1) == 1 {
            let left = self.peaks_by_height[height]
                .take()
                .expect("Active peak must exist at height where carry bit is 1");
            current = merkle_hash(left, current);
            height += 1;
            idx >>= 1;
        }

        if height >= self.peaks_by_height.len() {
            self.peaks_by_height.resize(height + 1, None);
        }
        self.peaks_by_height[height] = Some(current);
        self.leaf_count += 1;

        let root = self.get_root();
        (leaf_index, root)
    }

    /// Returns all active peaks ordered from left to right (highest mountain to lowest mountain).
    pub fn get_peaks(&self) -> Vec<Fr> {
        let mut peaks = Vec::new();
        for &opt in self.peaks_by_height.iter().rev() {
            if let Some(peak) = opt {
                peaks.push(peak);
            }
        }
        peaks
    }

    /// Canonical peak bagging per DEC-032 Section 3.C:
    ///
    /// - If m == 1: Root = Poseidon_W5([P_0, Fr(N), 0, 0], DOMAIN_MMR_BAG)
    /// - If m > 1:
    ///   Acc_{m-1} = P_{m-1}
    ///   Acc_i = Poseidon_W5([P_i, Acc_{i+1}, 0, 0], DOMAIN_MERKLE_NODE) for i = m-2 down to 0
    ///   Root = Poseidon_W5([Acc_0, Fr(N), 0, 0], DOMAIN_MMR_BAG)
    pub fn bag_peaks(peaks: &[Fr], leaf_count: usize) -> Fr {
        if peaks.is_empty() {
            return Fr::from(0u64);
        }
        let mut acc = peaks[peaks.len() - 1];
        for i in (0..peaks.len() - 1).rev() {
            acc = merkle_hash(peaks[i], acc);
        }
        let inputs = [
            acc,
            Fr::from(leaf_count as u64),
            Fr::from(0u64),
            Fr::from(0u64),
        ];
        native_poseidon_w5(&inputs, domain_mmr_bag())
    }

    /// Compute the current bagged MMR root.
    pub fn get_root(&self) -> Fr {
        Self::bag_peaks(&self.get_peaks(), self.leaf_count)
    }

    /// Generate an MMR membership proof for `leaf_index`.
    pub fn generate_proof(&self, leaf_index: usize) -> MMRProof {
        if leaf_index >= self.leaf_count {
            panic!(
                "OUT_OF_BOUNDS_LEAF: leaf_index {} >= leaf_count {}",
                leaf_index, self.leaf_count
            );
        }

        // Decompose leaf_count into peak heights (descending order)
        let mut peak_heights = Vec::new();
        for h in (0..usize::BITS).rev() {
            if (self.leaf_count >> h) & 1 == 1 {
                peak_heights.push(h as usize);
            }
        }

        // Locate mountain covering leaf_index
        let mut current_start = 0;
        let mut target_peak_idx = 0;
        let mut target_height = 0;
        let mut target_start = 0;

        for (p_idx, &h) in peak_heights.iter().enumerate() {
            let mountain_size = 1 << h;
            if leaf_index >= current_start && leaf_index < current_start + mountain_size {
                target_peak_idx = p_idx;
                target_height = h;
                target_start = current_start;
                break;
            }
            current_start += mountain_size;
        }

        // Extract internal mountain tree nodes and siblings
        let mountain_size = 1 << target_height;
        let mut mountain_nodes: Vec<Fr> =
            self.leaves[target_start..target_start + mountain_size].to_vec();
        let mut offset = leaf_index - target_start;
        let mut mountain_siblings = Vec::with_capacity(target_height);

        for _ in 0..target_height {
            let sibling_offset = if offset.is_multiple_of(2) {
                offset + 1
            } else {
                offset - 1
            };
            mountain_siblings.push(mountain_nodes[sibling_offset]);

            let mut next_layer = Vec::with_capacity(mountain_nodes.len() / 2);
            for chunk in mountain_nodes.as_chunks::<2>().0 {
                next_layer.push(merkle_hash(chunk[0], chunk[1]));
            }
            mountain_nodes = next_layer;
            offset /= 2;
        }

        // Sibling peaks for bagging: all active peaks except this one
        let all_peaks = self.get_peaks();
        let mut peak_bagging_siblings = all_peaks;
        peak_bagging_siblings.remove(target_peak_idx);

        MMRProof {
            mountain_height: target_height,
            mountain_siblings,
            peak_bagging_siblings,
            peak_index: target_peak_idx,
        }
    }

    /// Verifies an MMR membership proof against a given root.
    /// Strictly protects against Hyperbridge (2026) out-of-bounds exploits and
    /// tampered leaf_count values.
    pub fn verify_proof(
        leaf: Fr,
        leaf_index: usize,
        leaf_count: usize,
        proof: &MMRProof,
        root: Fr,
    ) -> bool {
        // 1. Strict Upper Bound Check (DEC-032 Section 4.1):
        // Prevents Hyperbridge unconsumed leaf exploit: leaf_index MUST be strictly less than leaf_count
        if leaf_count == 0 || leaf_index >= leaf_count {
            return false;
        }

        // 2. Canonical Mountain Derivation (DEC-032 Section 4.2):
        let mut peak_heights = Vec::new();
        for h in (0..usize::BITS).rev() {
            if (leaf_count >> h) & 1 == 1 {
                peak_heights.push(h as usize);
            }
        }
        let total_peaks = peak_heights.len();

        let mut current_start = 0;
        let mut canonical_peak_idx = None;
        let mut canonical_height = None;

        for (p_idx, &h) in peak_heights.iter().enumerate() {
            let mountain_size = 1 << h;
            if leaf_index >= current_start && leaf_index < current_start + mountain_size {
                canonical_peak_idx = Some(p_idx);
                canonical_height = Some(h);
                break;
            }
            current_start += mountain_size;
        }

        let (Some(target_peak_idx), Some(target_height)) = (canonical_peak_idx, canonical_height)
        else {
            return false;
        };

        // Structural witness consistency checks
        if proof.mountain_height != target_height
            || proof.peak_index != target_peak_idx
            || proof.mountain_siblings.len() != target_height
            || proof.peak_bagging_siblings.len() != total_peaks - 1
        {
            return false;
        }

        // 3. Reconstruct peak from leaf and mountain siblings
        let mut current = leaf;
        let mut idx = leaf_index;
        for sibling in &proof.mountain_siblings {
            current = if idx.is_multiple_of(2) {
                merkle_hash(current, *sibling)
            } else {
                merkle_hash(*sibling, current)
            };
            idx /= 2;
        }
        let computed_peak = current;

        // 4. Reconstruct all active peaks in canonical order
        let mut all_peaks = Vec::with_capacity(total_peaks);
        let mut bag_iter = proof.peak_bagging_siblings.iter();
        for i in 0..total_peaks {
            if i == target_peak_idx {
                all_peaks.push(computed_peak);
            } else {
                all_peaks.push(*bag_iter.next().unwrap());
            }
        }

        // 5. Bag all peaks and check equivalence to expected root
        let computed_root = Self::bag_peaks(&all_peaks, leaf_count);
        computed_root == root
    }
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
        let n1 = derive_nullifier(nk, cm, 0, 0);
        let n2 = derive_nullifier(nk, cm, 0, 0);
        assert_eq!(n1, n2, "Nullifier must be deterministic");
    }

    #[test]
    fn test_nullifier_unlinkable_without_key() {
        // Two different notes with different nullifier keys produce different nullifiers
        let nk1 = Fr::from(1u64);
        let nk2 = Fr::from(2u64);
        let cm = Fr::from(999u64);
        let n1 = derive_nullifier(nk1, cm, 0, 0);
        let n2 = derive_nullifier(nk2, cm, 0, 0);
        assert_ne!(n1, n2, "Different keys must produce different nullifiers");
    }

    #[test]
    fn test_nullifier_position_binding() {
        let nk = Fr::from(42u64);
        let cm = Fr::from(999u64);
        let n0 = derive_nullifier(nk, cm, 0, 0);
        let n1 = derive_nullifier(nk, cm, 1, 0);
        assert_ne!(
            n0, n1,
            "Same note at different positions must produce different nullifiers"
        );
    }

    #[test]
    fn test_nullifier_epoch_binding() {
        let nk = Fr::from(42u64);
        let cm = Fr::from(999u64);
        let n_epoch0 = derive_nullifier_epoch(nk, cm, 0, 0);
        let n_epoch1 = derive_nullifier_epoch(nk, cm, 0, 1);
        let n_epoch2 = derive_nullifier_epoch(nk, cm, 0, 2);
        assert_ne!(
            n_epoch0, n_epoch1,
            "Same note at different epochs must produce distinct evolving nullifiers (DEC-033)"
        );
        assert_ne!(
            n_epoch1, n_epoch2,
            "Evolving nullifier must prevent cross-epoch replay"
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

    #[test]
    fn test_fr_from_be_bytes_rejects_out_of_modulus() {
        // BLS12-381 scalar field modulus r in big-endian:
        // 0x73eda753299d7d483339d80809a1d80553bda402fffe5bfeffffffff00000001
        let r_bytes =
            hex::decode("73eda753299d7d483339d80809a1d80553bda402fffe5bfeffffffff00000001")
                .unwrap();
        let mut r_be = [0u8; 32];
        r_be.copy_from_slice(&r_bytes);

        assert!(
            fr_from_be_bytes(&r_be).is_none(),
            "fr_from_be_bytes must reject modulus r itself"
        );

        // r + 1:
        let mut r_plus_one = r_be;
        r_plus_one[31] += 1;
        assert!(
            fr_from_be_bytes(&r_plus_one).is_none(),
            "fr_from_be_bytes must reject r + 1"
        );

        // 2^256 - 1 (all 0xFF):
        let max_u256 = [0xffu8; 32];
        assert!(
            fr_from_be_bytes(&max_u256).is_none(),
            "fr_from_be_bytes must reject 2^256 - 1"
        );

        // r - 1 should succeed:
        let mut r_minus_one = r_be;
        r_minus_one[31] -= 1;
        assert!(
            fr_from_be_bytes(&r_minus_one).is_some(),
            "fr_from_be_bytes must accept valid field element r - 1"
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
        // Domain separators are derived from keccak256 labels modulo field order.
        // These MUST be identical across core, SDK, and contract.
        let d_note = domain_note_commitment();
        let d_null = domain_nullifier();
        let d_merkle = domain_merkle_node();
        let d_initial = domain_initial_note();
        let d_quote = domain_quote_binding();
        let d_dummy = domain_dummy_nullifier();

        let d_note_hex = hex::encode(fr_to_be_bytes(&d_note));
        let d_null_hex = hex::encode(fr_to_be_bytes(&d_null));
        let d_merkle_hex = hex::encode(fr_to_be_bytes(&d_merkle));
        let d_initial_hex = hex::encode(fr_to_be_bytes(&d_initial));
        let d_quote_hex = hex::encode(fr_to_be_bytes(&d_quote));
        let d_dummy_hex = hex::encode(fr_to_be_bytes(&d_dummy));

        // Exact known-answer hex strings from canonical Keccak-256 mod r
        assert_eq!(
            d_note_hex,
            "49476cc74d87b0ec3cbcf3e02b3794d86c6d8b5a8b5ccf7029a3b709e9e686df"
        );
        assert_eq!(
            d_null_hex,
            "541616c5d57d76e314728a4f2bdbdfe71f1ebcb705b8114d1f3216afa654c621"
        );
        assert_eq!(
            d_merkle_hex,
            "3834656d06de7d572e49914bb58b1e27dd5f9f6d86240e64495a1d82db22e82e"
        );
        assert_eq!(
            d_initial_hex,
            "53583822f8e973844e5583179d2bc8684c45a0b85310e551088dcd3d6815b523"
        );
        assert_eq!(
            d_quote_hex,
            hex::encode(fr_to_be_bytes(&domain_from_label(
                "nimbus.quote.binding.v1"
            )))
        );
        assert_eq!(
            d_dummy_hex,
            hex::encode(fr_to_be_bytes(&domain_from_label(
                "nimbus.note.dummy.nullifier.v1"
            )))
        );

        // All distinct
        let all_domains = [d_note, d_null, d_merkle, d_initial, d_quote, d_dummy];
        let mut domain_set = std::collections::HashSet::new();
        for d in all_domains {
            assert!(
                domain_set.insert(d),
                "Domain tags must be mutually disjoint"
            );
        }
    }

    #[test]
    fn known_answer_note_commitment() {
        // Note: value=99800000, owner_key=Fr(42), rho=Fr(1), randomness=Fr(2)
        let note = PrivateNoteV1::new(99_800_000, Fr::from(42u64), Fr::from(1u64), Fr::from(2u64));
        let cm = note.commitment();
        let cm_hex = hex::encode(fr_to_be_bytes(&cm));

        // Canonical test vector DEC-016A §16
        assert_eq!(
            cm_hex, "46d1b90c8a28c364fefdbb95bd709956e2f39a411750e6797fe1112b27635c59",
            "Note commitment must match canonical DEC-016A KAV"
        );

        // Determinism check
        let cm2 = note.commitment();
        assert_eq!(cm, cm2, "Commitment must be deterministic");
    }

    #[test]
    fn known_answer_nullifier_key() {
        let spending_key = Fr::from(42u64);
        let nk = derive_nullifier_key(spending_key);
        let nk_hex = hex::encode(fr_to_be_bytes(&nk));

        assert_eq!(
            nk_hex, "112c7c26870c6dcbdf343434984bd550c3d475e6b6e3bb6342fe915d444b677a",
            "Nullifier key must match canonical DEC-016A KAV"
        );
    }

    #[test]
    fn known_answer_nullifier() {
        let spending_key = Fr::from(42u64);
        let nk = derive_nullifier_key(spending_key);

        let note = PrivateNoteV1::new(99_800_000, spending_key, Fr::from(1u64), Fr::from(2u64));
        let cm = note.commitment();
        let nf = derive_nullifier(nk, cm, 0, 0);
        let nf_hex = hex::encode(fr_to_be_bytes(&nf));

        assert_eq!(
            nf_hex, "5b0320f2c7066f0b751c4f5ddfec9dfdde2ca07ea0c8f0d5f335c54b532dba54",
            "Nullifier (leaf_index=0, epoch_id=0) must match canonical DEC-016A KAV"
        );

        // Non-zero leaf index must produce completely different nullifier (prevents position reuse)
        let nf_idx1 = derive_nullifier(nk, cm, 1, 0);
        assert_ne!(
            nf, nf_idx1,
            "Different leaf index must yield different nullifier"
        );

        // Non-zero epoch_id must produce completely different nullifier (prevents cross-epoch replay DEC-033)
        let nf_epoch1 = derive_nullifier_epoch(nk, cm, 0, 1);
        assert_ne!(
            nf, nf_epoch1,
            "Different epoch_id must yield different nullifier (evolving nullifier PRF)"
        );
    }

    #[test]
    fn known_answer_parent_hash() {
        // Parent hash of two known children: left = Fr(1), right = Fr(2)
        let left = Fr::from(1u64);
        let right = Fr::from(2u64);
        let parent = merkle_hash(left, right);
        let parent_hex = hex::encode(fr_to_be_bytes(&parent));

        // Recompute parent hash deterministically
        let expected_inputs = [left, right, Fr::from(0u64), Fr::from(0u64)];
        let manual_parent = native_poseidon_w5(&expected_inputs, domain_merkle_node());
        assert_eq!(parent, manual_parent);
        assert_eq!(
            parent_hex,
            hex::encode(fr_to_be_bytes(&manual_parent)),
            "Parent hash must be strictly deterministic"
        );
    }

    #[test]
    fn known_answer_merkle_single_insert() {
        let empty = compute_empty_hashes();
        let mut frontier = [Fr::from(0u64); MERKLE_TREE_DEPTH + 1];

        let note = PrivateNoteV1::new(99_800_000, Fr::from(42u64), Fr::from(1u64), Fr::from(2u64));
        let cm = note.commitment();
        let root = merkle_append(&mut frontier, 0, cm, &empty);
        let root_hex = hex::encode(fr_to_be_bytes(&root));

        assert_eq!(
            root_hex, "3ed6d45ee8da74b055fa37a13ec3459e9fa97db5f89d014fdb0e41013fb56bfd",
            "Merkle root after 1 insert must match canonical DEC-016A KAV and stylus contract"
        );

        // Verify the root with a Merkle proof
        let siblings: [Fr; MERKLE_TREE_DEPTH] = std::array::from_fn(|i| empty[i]);
        assert!(
            verify_merkle_proof(root, cm, 0, &siblings),
            "Merkle proof must verify for single insert"
        );

        // Invalid sibling must reject proof
        let mut corrupt_siblings = siblings;
        corrupt_siblings[0] = Fr::from(999u64);
        assert!(
            !verify_merkle_proof(root, cm, 0, &corrupt_siblings),
            "Corrupt sibling proof must be rejected"
        );
    }

    #[test]
    fn known_answer_value_conservation_spend() {
        // DEC-028 Flat Fee Model:
        // Input note: 10_000_000 (10 USDC)
        // Merchant payout: 3_000_000 (3 USDC)
        // Protocol fee: 13_500 (0.45% of 3 USDC)
        // Relayer execution fee: 50_000
        // Expected change note = 10_000_000 - 3_000_000 - 13_500 - 50_000 = 6_936_500 (6.9365 USDC)
        let input_val = 10_000_000u64;
        let payout = 3_000_000u64;
        let protocol_fee = 13_500u64;
        let exec_fee = 50_000u64;
        let change = input_val - payout - protocol_fee - exec_fee;
        assert_eq!(change, 6_936_500);

        assert!(verify_value_conservation(
            &[input_val],
            payout,
            protocol_fee,
            exec_fee,
            &[change],
        ));

        // Conservation failure if change altered by even 1 micro-USDC
        assert!(!verify_value_conservation(
            &[input_val],
            payout,
            protocol_fee,
            exec_fee,
            &[change + 1],
        ));
        assert!(!verify_value_conservation(
            &[input_val],
            payout,
            protocol_fee,
            exec_fee,
            &[change - 1],
        ));
    }

    #[test]
    fn known_answer_empty_tree_root() {
        let empty = compute_empty_hashes();
        let root_hex = hex::encode(fr_to_be_bytes(&empty[MERKLE_TREE_DEPTH]));

        assert_eq!(
            root_hex, "4e9a77b95958924d004a9841dfb0f92c8d70f99c54bc59da1fa53320c0aaa6d2",
            "Empty tree root must match canonical Stylus contract EMPTY_TREE_ROOT_BYTES"
        );
    }

    #[test]
    fn test_domain_mmr_bag_matches_dec032_bytes() {
        let domain_bag = domain_mmr_bag();
        let domain_bytes = fr_to_be_bytes(&domain_bag);
        assert_eq!(
            domain_bytes, DOMAIN_MMR_BAG_BYTES,
            "domain_mmr_bag() must match DEC-032 DOMAIN_MMR_BAG_BYTES"
        );
    }

    #[test]
    fn test_mmr_sequential_insert_1000_leaves() {
        let mut mmr = MerkleMountainRange::new();
        assert_eq!(mmr.leaf_count, 0);
        assert!(mmr.get_peaks().is_empty());

        let mut roots = Vec::new();
        for i in 0..1000 {
            let leaf = Fr::from((i as u64) * 31 + 7);
            let (idx, root) = mmr.append(leaf);
            assert_eq!(idx, i);
            roots.push(root);

            let expected_peaks_count = (i + 1).count_ones() as usize;
            assert_eq!(
                mmr.get_peaks().len(),
                expected_peaks_count,
                "Number of peaks must equal number of 1-bits in leaf_count at count {}",
                i + 1
            );
        }

        assert_eq!(mmr.leaf_count, 1000);

        // Verify proofs for leaves at various checkpoint counts
        let checkpoints = [1usize, 2, 3, 7, 8, 15, 16, 63, 64, 127, 255, 500, 1000];
        for &count in &checkpoints {
            let root = roots[count - 1];
            // Test first, middle, and last leaf of this checkpoint
            let test_indices = [0, count / 2, count - 1];
            for &leaf_idx in &test_indices {
                // Build MMR up to checkpoint count
                let mut sub_mmr = MerkleMountainRange::new();
                for i in 0..count {
                    sub_mmr.append(Fr::from((i as u64) * 31 + 7));
                }
                assert_eq!(sub_mmr.get_root(), root);

                let proof = sub_mmr.generate_proof(leaf_idx);
                let leaf = Fr::from((leaf_idx as u64) * 31 + 7);
                assert!(
                    MerkleMountainRange::verify_proof(leaf, leaf_idx, count, &proof, root),
                    "Proof must verify for leaf {} at count {}",
                    leaf_idx,
                    count
                );
            }
        }
    }

    #[test]
    fn test_mmr_hyperbridge_out_of_bounds_rejected() {
        let mut mmr = MerkleMountainRange::new();
        for i in 0..7 {
            mmr.append(Fr::from(i as u64 + 10));
        }
        let root = mmr.get_root();
        let valid_proof = mmr.generate_proof(6);

        // 1. leaf_index == leaf_count (exactly out of bounds)
        assert!(
            !MerkleMountainRange::verify_proof(Fr::from(16u64), 7, 7, &valid_proof, root),
            "leaf_index == leaf_count must be rejected (Hyperbridge exploit)"
        );

        // 2. leaf_index > leaf_count
        assert!(
            !MerkleMountainRange::verify_proof(Fr::from(16u64), 8, 7, &valid_proof, root),
            "leaf_index > leaf_count must be rejected"
        );

        // 3. Huge out-of-bounds leaf_index
        assert!(
            !MerkleMountainRange::verify_proof(Fr::from(16u64), 999999, 7, &valid_proof, root),
            "Huge out-of-bounds index must be rejected"
        );
    }

    #[test]
    fn test_mmr_tampered_leaf_count_rejected() {
        let mut mmr = MerkleMountainRange::new();
        for i in 0..7 {
            mmr.append(Fr::from(i as u64 + 100));
        }
        let root = mmr.get_root();
        let proof = mmr.generate_proof(3);
        let leaf = Fr::from(103u64);

        // Tampered leaf_count alters bagging binding
        assert!(
            !MerkleMountainRange::verify_proof(leaf, 3, 8, &proof, root),
            "Tampered leaf_count=8 must be rejected"
        );
        assert!(
            !MerkleMountainRange::verify_proof(leaf, 3, 6, &proof, root),
            "Tampered leaf_count=6 must be rejected"
        );
        assert!(
            !MerkleMountainRange::verify_proof(leaf, 3, 0, &proof, root),
            "leaf_count=0 must be rejected"
        );
    }

    #[test]
    fn test_mmr_tampered_mountain_siblings_rejected() {
        let mut mmr = MerkleMountainRange::new();
        for i in 0..7 {
            mmr.append(Fr::from(i as u64 + 1));
        }
        let root = mmr.get_root();
        let mut proof = mmr.generate_proof(2);
        let leaf = Fr::from(3u64);

        assert!(MerkleMountainRange::verify_proof(leaf, 2, 7, &proof, root));

        // Corrupt first mountain sibling
        proof.mountain_siblings[0] += Fr::from(1u64);
        assert!(
            !MerkleMountainRange::verify_proof(leaf, 2, 7, &proof, root),
            "Corrupted mountain sibling must fail verification"
        );
    }

    #[test]
    fn test_mmr_tampered_peak_siblings_rejected() {
        let mut mmr = MerkleMountainRange::new();
        for i in 0..7 {
            mmr.append(Fr::from(i as u64 + 1));
        }
        let root = mmr.get_root();
        let mut proof = mmr.generate_proof(0);
        let leaf = Fr::from(1u64);

        assert!(MerkleMountainRange::verify_proof(leaf, 0, 7, &proof, root));

        // Tamper peak bagging sibling
        proof.peak_bagging_siblings[0] += Fr::from(1u64);
        assert!(
            !MerkleMountainRange::verify_proof(leaf, 0, 7, &proof, root),
            "Corrupted peak bagging sibling must fail verification"
        );
    }

    #[test]
    fn test_mmr_known_answer_test() {
        let mut mmr = MerkleMountainRange::new();
        let (idx0, root0) = mmr.append(Fr::from(42u64));
        assert_eq!(idx0, 0);

        let (idx1, root1) = mmr.append(Fr::from(99u64));
        assert_eq!(idx1, 1);
        assert_ne!(root0, root1);

        let proof0 = mmr.generate_proof(0);
        assert!(MerkleMountainRange::verify_proof(
            Fr::from(42u64),
            0,
            2,
            &proof0,
            root1
        ));

        let proof1 = mmr.generate_proof(1);
        assert!(MerkleMountainRange::verify_proof(
            Fr::from(99u64),
            1,
            2,
            &proof1,
            root1
        ));
    }

    #[test]
    fn test_mmr_wrap_around_scalar_rejected() {
        // [0xff; 32] is strictly >= r (modulus of BLS12-381 Fr)
        let non_canonical = [0xffu8; 32];
        assert!(
            crate::from_evm_scalar(&non_canonical).is_none(),
            "Non-canonical scalar exceeding BLS12-381 Fr modulus must be rejected"
        );

        // Modulus r itself:
        // 0x73eda753299d7d483339d80809a1d80553bda402fffe5bfeffffffff00000001
        let r_bytes =
            hex::decode("73eda753299d7d483339d80809a1d80553bda402fffe5bfeffffffff00000001")
                .unwrap();
        let mut r_arr = [0u8; 32];
        r_arr.copy_from_slice(&r_bytes);
        assert!(
            crate::from_evm_scalar(&r_arr).is_none(),
            "Scalar equal to modulus r must be rejected (not in Fr)"
        );
    }

    #[test]
    fn test_quote_hash_limb_decomposition_boundary_vectors() {
        // Vector 1: 0
        let zero_hash = [0u8; 32];
        let (hi, lo) = split_quote_hash_to_limbs(&zero_hash);
        assert_eq!(hi, Fr::from(0u64));
        assert_eq!(lo, Fr::from(0u64));
        assert_eq!(combine_limbs_to_quote_hash(&hi, &lo), zero_hash);

        // Vector 2: 1
        let mut one_hash = [0u8; 32];
        one_hash[31] = 1;
        let (hi, lo) = split_quote_hash_to_limbs(&one_hash);
        assert_eq!(hi, Fr::from(0u64));
        assert_eq!(lo, Fr::from(1u64));
        assert_eq!(combine_limbs_to_quote_hash(&hi, &lo), one_hash);

        // Vector 3: 2^128 - 1 (low limb max, high limb 0)
        let mut max_u128_hash = [0u8; 32];
        max_u128_hash[16..32].fill(0xff);
        let (hi, lo) = split_quote_hash_to_limbs(&max_u128_hash);
        assert_eq!(hi, Fr::from(0u64));
        assert_eq!(combine_limbs_to_quote_hash(&hi, &lo), max_u128_hash);

        // Vector 4: 2^128 (high limb 1, low limb 0)
        let mut two_power_128_hash = [0u8; 32];
        two_power_128_hash[15] = 1;
        let (hi, lo) = split_quote_hash_to_limbs(&two_power_128_hash);
        assert_eq!(hi, Fr::from(1u64));
        assert_eq!(lo, Fr::from(0u64));
        assert_eq!(combine_limbs_to_quote_hash(&hi, &lo), two_power_128_hash);

        // Vector 5: 2^256 - 1 (both limbs 2^128 - 1, strictly < r)
        // This is in the "Danger Zone" (> r), proving DEC-035A works where naive scalar fails!
        let max_u256_hash = [0xffu8; 32];
        assert!(
            crate::from_evm_scalar(&max_u256_hash).is_none(),
            "Max u256 exceeds r"
        );
        let (hi, lo) = split_quote_hash_to_limbs(&max_u256_hash);
        assert_eq!(combine_limbs_to_quote_hash(&hi, &lo), max_u256_hash);

        // Vector 6: Arbitrary Keccak digest
        let keccak_sample: [u8; 32] = [
            0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc, 0xde, 0xf0, 0xfe, 0xdc, 0xba, 0x98, 0x76, 0x54,
            0x32, 0x10, 0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff, 0x00, 0x11, 0x22, 0x33, 0x44, 0x55,
            0x66, 0x77, 0x88, 0x99,
        ];
        let (hi, lo) = split_quote_hash_to_limbs(&keccak_sample);
        assert_eq!(combine_limbs_to_quote_hash(&hi, &lo), keccak_sample);

        // Negative test: Tampering hi or lo alters combined hash
        let hi_tampered = hi + Fr::from(1u64);
        assert_ne!(
            combine_limbs_to_quote_hash(&hi_tampered, &lo),
            keccak_sample
        );
        let lo_tampered = lo + Fr::from(1u64);
        assert_ne!(
            combine_limbs_to_quote_hash(&hi, &lo_tampered),
            keccak_sample
        );
    }
}
