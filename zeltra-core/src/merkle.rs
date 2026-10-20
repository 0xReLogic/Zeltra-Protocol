//! Merkle Mountain Range (MMR) & Merkle Tree Primitives — DEC-032
//!
//! Provides the in-memory MMR accumulator for note commitment history,
//! incremental binary carry adder accumulation, canonical peak bagging,
//! and out-of-bounds protected membership proof generation/verification.

use ark_bls12_381::Fr;
use ark_ff::PrimeField;

use crate::note::domain_from_label;
use crate::poseidon::native_poseidon_w5;

/// Canonical domain separator for MMR peak bagging (DEC-032).
/// Keccak256("ZELTRA_MMR_BAG_V1") mod r.
pub const DOMAIN_MMR_BAG_BYTES: [u8; 32] = [
    0x19, 0x4a, 0x8f, 0x9c, 0x1e, 0x7d, 0x23, 0x58, 0xb9, 0x01, 0xfc, 0x84, 0x33, 0x29, 0x10, 0x7b,
    0xa8, 0x92, 0x1d, 0xfb, 0xb3, 0x02, 0x48, 0x59, 0xae, 0xf0, 0x29, 0x14, 0x7d, 0xa2, 0x90, 0xbf,
];

/// Domain tag for Merkle tree internal node hash: H(left, right).
pub fn domain_merkle_node() -> Fr {
    domain_from_label("zeltra.merkle.node.v1")
}

/// Canonical domain separator for Merkle tree node merge (DEC-032 / DEC-045A).
/// Alias for domain_merkle_node(): Poseidon_W5([left, right, 0, 0], DOMAIN_MMR_MERGE).
pub fn domain_mmr_merge() -> Fr {
    domain_merkle_node()
}

/// Domain tag for MMR peak bagging: Poseidon_W5([acc, Fr(leaf_count), 0, 0], DOMAIN_MMR_BAG).
pub fn domain_mmr_bag() -> Fr {
    Fr::from_be_bytes_mod_order(&DOMAIN_MMR_BAG_BYTES)
}

/// Hash two child nodes to produce a parent node.
///
/// parent = Poseidon_W5(left, right, 0, 0; DOMAIN_MERKLE_NODE_V1)
pub fn merkle_hash(left: Fr, right: Fr) -> Fr {
    let inputs = [left, right, Fr::from(0u64), Fr::from(0u64)];
    native_poseidon_w5(&inputs, domain_mmr_merge())
}

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
    /// Cached bagged root (computed on append, O(1) get_root lookup).
    cached_root: Option<Fr>,
}

impl MerkleMountainRange {
    /// Create a new, empty Merkle Mountain Range.
    pub fn new() -> Self {
        Self {
            leaf_count: 0,
            leaves: Vec::new(),
            peaks_by_height: Vec::new(),
            cached_root: None,
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

        let root = Self::bag_peaks(&self.get_peaks(), self.leaf_count);
        self.cached_root = Some(root);
        (leaf_index, root)
    }

    /// Helper to append a leaf, alias for append.
    pub fn append_leaf(&mut self, leaf: Fr) -> (usize, Fr) {
        self.append(leaf)
    }

    /// Total number of leaves as u64.
    pub fn leaf_count(&self) -> u64 {
        self.leaf_count as u64
    }

    /// Current bagged root, alias for get_root.
    pub fn bagged_root(&self) -> Fr {
        self.get_root()
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

    /// Compute the current bagged MMR root (O(1) when cached).
    pub fn get_root(&self) -> Fr {
        match self.cached_root {
            Some(root) => root,
            None => Self::bag_peaks(&self.get_peaks(), self.leaf_count),
        }
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

/// Errors occurring during FlatMerkleMountainRange operations (DEC-045A).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FlatMmrError {
    LeafIndexOutOfBounds(usize, usize),
    EmptyTree,
    NonCanonicalScalar,
    CorruptedPeakStructure(usize),
}

impl core::fmt::Display for FlatMmrError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::LeafIndexOutOfBounds(leaf_idx, leaf_count) => {
                write!(
                    f,
                    "Leaf index {} out of bounds for leaf count {}",
                    leaf_idx, leaf_count
                )
            }
            Self::EmptyTree => write!(f, "Cannot generate proof from an empty MMR tree"),
            Self::NonCanonicalScalar => {
                write!(f, "Non-canonical scalar field element provided")
            }
            Self::CorruptedPeakStructure(height) => {
                write!(
                    f,
                    "Corrupted peak structure: expected active peak at height {}",
                    height
                )
            }
        }
    }
}

impl std::error::Error for FlatMmrError {}

/// Maps a 0-based leaf index to its 0-based post-order position in the flat MMR array (Peter Todd 2012).
///
/// Formula: pos(k) = 2 * k - count_ones(k)
#[inline]
pub fn leaf_index_to_pos(leaf_index: usize) -> usize {
    2 * leaf_index - leaf_index.count_ones() as usize
}

/// Computes the height of a node at position `pos` within the binary tree (DEC-045A Section 2.B).
///
/// Returns 0 for leaves, 1 for immediate parents, etc.
#[inline]
pub fn pos_height(mut pos: usize) -> usize {
    pos += 1;
    while pos > 0 {
        let shift = usize::BITS - 1 - pos.leading_zeros();
        let all_ones = if shift >= usize::BITS - 1 {
            usize::MAX
        } else {
            (1usize << (shift + 1)) - 1
        };
        if pos == all_ones {
            return shift as usize;
        }
        pos -= all_ones >> 1;
    }
    0
}

/// Bitmask and height analysis for a given MMR size/position (Grin PMMR / Peter Todd).
#[inline]
pub fn peak_map_height(mut size: usize) -> (usize, usize) {
    if size == 0 {
        return (0, 0);
    }
    let mut peak_size = usize::MAX >> size.leading_zeros();
    let mut peak_map = 0;
    while peak_size != 0 {
        peak_map <<= 1;
        if size >= peak_size {
            size -= peak_size;
            peak_map |= 1;
        }
        peak_size >>= 1;
    }
    (peak_map, size)
}

/// Determines whether the node at `pos` with height `height` is the left sibling of its parent.
#[inline]
pub fn is_left_sibling(pos: usize, height: usize) -> bool {
    let (peak_map, _) = peak_map_height(pos);
    let peak = 1usize << height;
    (peak_map & peak) == 0
}

/// Decomposes `leaf_count` into active peak positions from left to right (highest mountain to lowest).
pub fn get_peak_positions(leaf_count: usize) -> Vec<usize> {
    if leaf_count == 0 {
        return Vec::new();
    }
    let mut peaks = Vec::new();
    let mut curr_pos = 0;
    for h in (0..usize::BITS).rev() {
        if (leaf_count >> h) & 1 == 1 {
            let peak_pos = curr_pos + (1usize << (h + 1)) - 2;
            peaks.push(peak_pos);
            curr_pos += (1usize << (h + 1)) - 1;
        }
    }
    peaks
}

/// High-Performance Flat Post-Order Merkle Mountain Range (DEC-045A).
///
/// Stores all nodes (leaves and intermediate parent nodes) in a single continuous flat array.
/// Guarantees membership proof generation (`generate_proof`) with strictly ZERO (0) Poseidon hashes,
/// dropping proof latency from ~366 ms to < 20 microseconds (O(log N) memory lookups).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FlatMerkleMountainRange {
    /// All nodes in post-order traversal order (leaves + intermediate parent hashes).
    pub nodes: Vec<Fr>,
    /// Positions of active peaks from left to right (highest mountain to lowest mountain).
    pub peak_positions: Vec<usize>,
    /// Total number of commitment leaves accumulated.
    pub leaf_count: usize,
    /// Cached canonical bagged root (O(1) lookup).
    pub cached_root: Option<Fr>,
}

impl FlatMerkleMountainRange {
    /// Creates a new, empty flat Merkle Mountain Range.
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            peak_positions: Vec::new(),
            leaf_count: 0,
            cached_root: None,
        }
    }

    /// Constructs a FlatMerkleMountainRange from a slice of leaf elements.
    pub fn from_leaves(leaves: &[Fr]) -> Self {
        let mut mmr = Self::new();
        for &leaf in leaves {
            mmr.append(leaf);
        }
        mmr
    }

    /// Appends a leaf (note commitment) to the flat MMR.
    /// Intermediate parent nodes are computed once and stored permanently in `nodes`.
    ///
    /// Complexity: Amortized O(1) Poseidon hashing, O(log N) worst-case.
    /// Returns `(leaf_index, new_bagged_root)`.
    pub fn append(&mut self, leaf: Fr) -> (usize, Fr) {
        let leaf_idx = self.leaf_count;
        let mut current_hash = leaf;
        let mut height = 0;

        self.nodes.push(leaf);

        while (leaf_idx >> height) & 1 == 1 {
            let left_pos = match self.peak_positions.pop() {
                Some(p) => p,
                None => break,
            };
            let left_hash = match self.nodes.get(left_pos) {
                Some(&h) => h,
                None => break,
            };

            current_hash = merkle_hash(left_hash, current_hash);
            height += 1;
            self.nodes.push(current_hash);
        }

        let new_peak_pos = self.nodes.len().saturating_sub(1);
        self.peak_positions.push(new_peak_pos);
        self.leaf_count += 1;

        debug_assert_eq!(
            self.nodes.len(),
            2 * self.leaf_count - self.leaf_count.count_ones() as usize,
            "INV-45A-1 Array Size Invariant violation"
        );

        let root = self.recompute_bagged_root();
        self.cached_root = Some(root);

        (leaf_idx, root)
    }

    /// Append note commitment leaf alias matching MerkleMountainRange::append_leaf.
    pub fn append_leaf(&mut self, leaf: Fr) -> (usize, Fr) {
        self.append(leaf)
    }

    /// Appends a commitment scalar from 32-byte Big-Endian EVM representation.
    /// Validates scalar field canonicality (< r) before mutating state.
    pub fn append_from_bytes(&mut self, bytes: &[u8; 32]) -> Result<(usize, Fr), FlatMmrError> {
        let leaf = crate::from_evm_scalar(bytes).ok_or(FlatMmrError::NonCanonicalScalar)?;
        Ok(self.append(leaf))
    }

    /// Total number of leaves accumulated as u64.
    pub fn leaf_count(&self) -> u64 {
        self.leaf_count as u64
    }

    /// Current bagged root, alias for `get_root()`.
    pub fn bagged_root(&self) -> Fr {
        self.get_root()
    }

    /// Returns all active peaks ordered from left to right (highest mountain to lowest mountain).
    pub fn get_peaks(&self) -> Vec<Fr> {
        self.peak_positions
            .iter()
            .filter_map(|&pos| self.nodes.get(pos).copied())
            .collect()
    }

    /// Returns all leaf commitments in sequential insertion order.
    pub fn get_leaves(&self) -> Vec<Fr> {
        let mut leaves = Vec::with_capacity(self.leaf_count);
        for i in 0..self.leaf_count {
            let pos = leaf_index_to_pos(i);
            if let Some(&leaf) = self.nodes.get(pos) {
                leaves.push(leaf);
            }
        }
        leaves
    }

    /// Returns a specific leaf commitment by its 0-based leaf index.
    pub fn get_leaf(&self, leaf_index: usize) -> Option<Fr> {
        if leaf_index >= self.leaf_count {
            return None;
        }
        let pos = leaf_index_to_pos(leaf_index);
        self.nodes.get(pos).copied()
    }

    /// Verifies INV-45A-1: Array Size Invariant: |nodes| == 2N - count_ones(N).
    pub fn verify_array_size_invariant(&self) -> bool {
        self.nodes.len() == 2 * self.leaf_count - self.leaf_count.count_ones() as usize
    }

    /// Recomputes the canonical bagged root from active peaks per DEC-032 Section 3.C.
    pub fn recompute_bagged_root(&self) -> Fr {
        let peaks = self.get_peaks();
        MerkleMountainRange::bag_peaks(&peaks, self.leaf_count)
    }

    /// Returns the current bagged root (O(1) lookup when cached).
    pub fn get_root(&self) -> Fr {
        match self.cached_root {
            Some(root) => root,
            None => self.recompute_bagged_root(),
        }
    }

    /// Generates an MMR membership proof for `leaf_index`.
    ///
    /// GUARANTEE (AUD-12 / INV-45A-2): Strictly ZERO (0) Poseidon hashes are executed.
    /// Complexity: O(log N) memory lookups (< 20 microseconds).
    pub fn generate_proof(&self, leaf_index: usize) -> Result<MMRProof, FlatMmrError> {
        if self.leaf_count == 0 {
            return Err(FlatMmrError::EmptyTree);
        }
        if leaf_index >= self.leaf_count {
            return Err(FlatMmrError::LeafIndexOutOfBounds(
                leaf_index,
                self.leaf_count,
            ));
        }

        let mut pos = leaf_index_to_pos(leaf_index);
        let mut height = 0;
        let mut mountain_siblings = Vec::new();

        // 1. Ekstraksi Sibling Internal Gunung (Mendaki ke Puncak Lokal)
        while !self.peak_positions.contains(&pos) {
            let is_left = is_left_sibling(pos, height);
            let delta = (2usize << height) - 1;
            let sibling_pos = if is_left {
                pos + delta
            } else {
                if pos < delta {
                    return Err(FlatMmrError::CorruptedPeakStructure(height));
                }
                pos - delta
            };

            let sibling_node = match self.nodes.get(sibling_pos) {
                Some(&node) => node,
                None => return Err(FlatMmrError::CorruptedPeakStructure(height)),
            };
            mountain_siblings.push(sibling_node);

            pos = if is_left { sibling_pos + 1 } else { pos + 1 };
            height += 1;
        }

        // 2. Tentukan Peak Index dan Sibling Peaks untuk Bagging
        let target_peak_idx = match self.peak_positions.iter().position(|&p| p == pos) {
            Some(idx) => idx,
            None => return Err(FlatMmrError::CorruptedPeakStructure(height)),
        };

        let mut peak_bagging_siblings =
            Vec::with_capacity(self.peak_positions.len().saturating_sub(1));
        for (idx, &p_pos) in self.peak_positions.iter().enumerate() {
            if idx != target_peak_idx {
                let peak_node = match self.nodes.get(p_pos) {
                    Some(&n) => n,
                    None => return Err(FlatMmrError::CorruptedPeakStructure(idx)),
                };
                peak_bagging_siblings.push(peak_node);
            }
        }

        Ok(MMRProof {
            mountain_height: height,
            mountain_siblings,
            peak_bagging_siblings,
            peak_index: target_peak_idx,
        })
    }

    /// Verifies an MMR membership proof against a given root.
    /// Bit-exact equivalent to `MerkleMountainRange::verify_proof`.
    pub fn verify_proof(
        leaf: Fr,
        leaf_index: usize,
        leaf_count: usize,
        proof: &MMRProof,
        root: Fr,
    ) -> bool {
        MerkleMountainRange::verify_proof(leaf, leaf_index, leaf_count, proof, root)
    }
}

impl From<&MerkleMountainRange> for FlatMerkleMountainRange {
    fn from(legacy: &MerkleMountainRange) -> Self {
        Self::from_leaves(&legacy.leaves)
    }
}

impl From<MerkleMountainRange> for FlatMerkleMountainRange {
    fn from(legacy: MerkleMountainRange) -> Self {
        Self::from_leaves(&legacy.leaves)
    }
}

impl From<&FlatMerkleMountainRange> for MerkleMountainRange {
    fn from(flat: &FlatMerkleMountainRange) -> Self {
        let mut legacy = MerkleMountainRange::new();
        for i in 0..flat.leaf_count {
            let pos = leaf_index_to_pos(i);
            if let Some(&leaf) = flat.nodes.get(pos) {
                legacy.append(leaf);
            }
        }
        legacy
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bitwise_navigation_peter_todd_mapping() {
        assert_eq!(leaf_index_to_pos(0), 0);
        assert_eq!(leaf_index_to_pos(1), 1);
        assert_eq!(leaf_index_to_pos(2), 3);
        assert_eq!(leaf_index_to_pos(3), 4);
        assert_eq!(leaf_index_to_pos(4), 7);
        assert_eq!(leaf_index_to_pos(5), 8);
        assert_eq!(leaf_index_to_pos(6), 10);
        assert_eq!(leaf_index_to_pos(7), 11);
        assert_eq!(leaf_index_to_pos(8), 15);
    }

    #[test]
    fn test_pos_height_determination() {
        assert_eq!(pos_height(0), 0);
        assert_eq!(pos_height(1), 0);
        assert_eq!(pos_height(2), 1);
        assert_eq!(pos_height(3), 0);
        assert_eq!(pos_height(4), 0);
        assert_eq!(pos_height(5), 1);
        assert_eq!(pos_height(6), 2);
        assert_eq!(pos_height(14), 3);
    }

    #[test]
    fn test_is_left_sibling_predicate() {
        assert!(is_left_sibling(0, 0));
        assert!(!is_left_sibling(1, 0));
        assert!(is_left_sibling(2, 1));
        assert!(is_left_sibling(3, 0));
        assert!(!is_left_sibling(4, 0));
        assert!(!is_left_sibling(5, 1));
        assert!(is_left_sibling(6, 2));
    }

    #[test]
    fn test_get_peak_positions_extractor() {
        assert_eq!(get_peak_positions(0), Vec::<usize>::new());
        assert_eq!(get_peak_positions(1), vec![0]);
        assert_eq!(get_peak_positions(2), vec![2]);
        assert_eq!(get_peak_positions(3), vec![2, 3]);
        assert_eq!(get_peak_positions(4), vec![6]);
        assert_eq!(get_peak_positions(7), vec![6, 9, 10]);
        assert_eq!(get_peak_positions(8), vec![14]);
    }

    // A5.1 — Positive: Append and Bagged Root Parity
    #[test]
    fn test_flat_mmr_append_and_bagged_root_parity() {
        let mut flat_mmr = FlatMerkleMountainRange::new();
        let mut legacy_mmr = MerkleMountainRange::new();

        for i in 1..=1000 {
            let leaf = Fr::from(i as u64);
            let (flat_idx, flat_root) = flat_mmr.append(leaf);
            let (legacy_idx, legacy_root) = legacy_mmr.append(leaf);

            assert_eq!(flat_idx, legacy_idx);
            assert_eq!(flat_root, legacy_root);
            assert_eq!(flat_mmr.get_root(), legacy_mmr.get_root());
            assert!(flat_mmr.verify_array_size_invariant());
            assert_eq!(flat_mmr.leaf_count(), i as u64);
        }
    }

    // A5.2 — Positive: Zero-Hash Proof Generation and Verification
    #[test]
    fn test_flat_mmr_zero_hash_proof_generation_and_verification() {
        let mut flat_mmr = FlatMerkleMountainRange::new();
        let mut legacy_mmr = MerkleMountainRange::new();

        for i in 1..=513 {
            let leaf = Fr::from(i as u64);
            flat_mmr.append(leaf);
            legacy_mmr.append(leaf);
        }

        let root = flat_mmr.get_root();
        assert_eq!(root, legacy_mmr.get_root());

        for i in 0..513 {
            let leaf = Fr::from((i + 1) as u64);
            let flat_proof = flat_mmr.generate_proof(i).expect("valid proof");
            let legacy_proof = legacy_mmr.generate_proof(i);

            // Assert bit-exact proof equality
            assert_eq!(flat_proof, legacy_proof);

            // Verify using FlatMerkleMountainRange::verify_proof
            assert!(FlatMerkleMountainRange::verify_proof(
                leaf,
                i,
                513,
                &flat_proof,
                root
            ));
        }
    }

    // A5.3 — Positive: Single Leaf Boundary
    #[test]
    fn test_flat_mmr_single_leaf_boundary() {
        let mut flat_mmr = FlatMerkleMountainRange::new();
        let leaf = Fr::from(42u64);
        let (idx, root) = flat_mmr.append(leaf);

        assert_eq!(idx, 0);
        assert_eq!(flat_mmr.nodes.len(), 1);
        assert_eq!(flat_mmr.peak_positions, vec![0]);
        assert_eq!(flat_mmr.nodes[0], leaf);

        let proof = flat_mmr.generate_proof(0).expect("valid proof");
        assert_eq!(proof.mountain_height, 0);
        assert!(proof.mountain_siblings.is_empty());
        assert!(proof.peak_bagging_siblings.is_empty());
        assert_eq!(proof.peak_index, 0);

        assert!(FlatMerkleMountainRange::verify_proof(
            leaf, 0, 1, &proof, root
        ));
    }

    // A5.4 — Negative: Out of Bounds Leaf Index
    #[test]
    fn test_flat_mmr_rejects_out_of_bounds_leaf_index() {
        let mut flat_mmr = FlatMerkleMountainRange::new();
        for i in 1..=10 {
            flat_mmr.append(Fr::from(i as u64));
        }

        let initial_nodes_len = flat_mmr.nodes.len();
        let initial_leaf_count = flat_mmr.leaf_count;
        let initial_root = flat_mmr.get_root();

        // Exactly at leaf_count
        let err1 = flat_mmr.generate_proof(10);
        assert_eq!(err1, Err(FlatMmrError::LeafIndexOutOfBounds(10, 10)));

        // Beyond leaf_count
        let err2 = flat_mmr.generate_proof(20);
        assert_eq!(err2, Err(FlatMmrError::LeafIndexOutOfBounds(20, 10)));

        // State remains strictly unchanged
        assert_eq!(flat_mmr.nodes.len(), initial_nodes_len);
        assert_eq!(flat_mmr.leaf_count, initial_leaf_count);
        assert_eq!(flat_mmr.get_root(), initial_root);
    }

    // A5.5 — Negative: Empty Tree Proof Rejection
    #[test]
    fn test_flat_mmr_rejects_empty_tree_proof() {
        let flat_mmr = FlatMerkleMountainRange::new();
        let err = flat_mmr.generate_proof(0);
        assert_eq!(err, Err(FlatMmrError::EmptyTree));
        assert_eq!(flat_mmr.leaf_count, 0);
        assert!(flat_mmr.nodes.is_empty());
    }

    // A5.6 — Negative: Tampered Mountain Sibling Rejection
    #[test]
    fn test_flat_mmr_rejects_tampered_sibling_proof() {
        let mut flat_mmr = FlatMerkleMountainRange::new();
        for i in 1..=8 {
            flat_mmr.append(Fr::from(i as u64));
        }
        let root = flat_mmr.get_root();
        let mut proof = flat_mmr.generate_proof(3).expect("valid proof");
        assert!(!proof.mountain_siblings.is_empty());

        // Mutate mountain sibling (1-bit tampering)
        proof.mountain_siblings[0] += Fr::from(1u64);

        let valid = FlatMerkleMountainRange::verify_proof(Fr::from(4u64), 3, 8, &proof, root);
        assert!(
            !valid,
            "Tampered mountain sibling must strictly fail verification"
        );
    }

    // A5.7 — Negative: Tampered Leaf Count (Hyperbridge 2026 Exploit Pattern)
    #[test]
    fn test_flat_mmr_rejects_tampered_leaf_count_hyperbridge_exploit() {
        let mut flat_mmr = FlatMerkleMountainRange::new();
        for i in 1..=4 {
            flat_mmr.append(Fr::from(i as u64));
        }
        let root = flat_mmr.get_root();
        let proof = flat_mmr.generate_proof(2).expect("valid proof");

        // Falsify leaf_count: actual is 4, claim is 8
        let falsified_count = 8;
        let valid =
            FlatMerkleMountainRange::verify_proof(Fr::from(3u64), 2, falsified_count, &proof, root);
        assert!(
            !valid,
            "Falsified leaf count must be rejected by canonical structure checks"
        );
    }

    // A5.8 — Negative: Tampered Peak Bagging Sibling Rejection
    #[test]
    fn test_flat_mmr_rejects_tampered_peak_bagging_sibling() {
        let mut flat_mmr = FlatMerkleMountainRange::new();
        for i in 1..=7 {
            flat_mmr.append(Fr::from(i as u64));
        }
        let root = flat_mmr.get_root();
        let mut proof = flat_mmr.generate_proof(6).expect("valid proof");
        assert!(!proof.peak_bagging_siblings.is_empty());

        // Mutate peak bagging sibling
        proof.peak_bagging_siblings[0] += Fr::from(1u64);

        let valid = FlatMerkleMountainRange::verify_proof(Fr::from(7u64), 6, 7, &proof, root);
        assert!(
            !valid,
            "Tampered peak bagging sibling must fail root verification"
        );
    }

    // A5.9 — Negative: Subtree Immutability Assertion
    #[test]
    fn test_flat_mmr_subtree_immutability_assertion() {
        let mut flat_mmr = FlatMerkleMountainRange::new();
        for i in 1..=8 {
            flat_mmr.append(Fr::from(i as u64));
        }

        // Snapshot nodes at N=8 (15 nodes: 0..14)
        let snapshot_nodes = flat_mmr.nodes.clone();
        assert_eq!(snapshot_nodes.len(), 15);

        // Append 8 more leaves to reach N=16
        for i in 9..=16 {
            flat_mmr.append(Fr::from(i as u64));
        }

        assert_eq!(flat_mmr.leaf_count, 16);
        assert_eq!(flat_mmr.nodes.len(), 31);

        // Subtree nodes 0..15 must be 100% bit-identical
        assert_eq!(
            &flat_mmr.nodes[0..15],
            snapshot_nodes.as_slice(),
            "Prior subtree nodes must remain strictly immutable after subsequent appends"
        );
    }

    // A5.10 — Negative: Non-Canonical Leaf Input Rejection
    #[test]
    fn test_flat_mmr_rejects_non_canonical_leaf_input() {
        let mut flat_mmr = FlatMerkleMountainRange::new();
        flat_mmr.append(Fr::from(1u64));
        let initial_len = flat_mmr.nodes.len();
        let initial_count = flat_mmr.leaf_count;

        // Non-canonical scalar: [0xff; 32] >= r
        let non_canonical_bytes = [0xffu8; 32];
        let res = flat_mmr.append_from_bytes(&non_canonical_bytes);
        assert_eq!(res, Err(FlatMmrError::NonCanonicalScalar));

        // State must remain strictly unchanged
        assert_eq!(flat_mmr.nodes.len(), initial_len);
        assert_eq!(flat_mmr.leaf_count, initial_count);

        // Modulus r itself:
        let r_bytes =
            hex::decode("73eda753299d7d483339d80809a1d80553bda402fffe5bfeffffffff00000001")
                .unwrap();
        let mut r_arr = [0u8; 32];
        r_arr.copy_from_slice(&r_bytes);
        let res_r = flat_mmr.append_from_bytes(&r_arr);
        assert_eq!(res_r, Err(FlatMmrError::NonCanonicalScalar));

        assert_eq!(flat_mmr.nodes.len(), initial_len);
        assert_eq!(flat_mmr.leaf_count, initial_count);
    }
}
