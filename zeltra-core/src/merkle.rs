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

/// Domain tag for MMR peak bagging: Poseidon_W5([acc, Fr(leaf_count), 0, 0], DOMAIN_MMR_BAG).
pub fn domain_mmr_bag() -> Fr {
    Fr::from_be_bytes_mod_order(&DOMAIN_MMR_BAG_BYTES)
}

/// Hash two child nodes to produce a parent node.
///
/// parent = Poseidon_W5(left, right, 0, 0; DOMAIN_MERKLE_NODE_V1)
pub fn merkle_hash(left: Fr, right: Fr) -> Fr {
    let inputs = [left, right, Fr::from(0u64), Fr::from(0u64)];
    native_poseidon_w5(&inputs, domain_merkle_node())
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
