//! Incremental Note Commitment Merkle Tree (LeanIMT Depth 20)
//!
//! Implements an append-only on-chain Merkle tree of depth 20 (up to 2^20 leaves)
//! using Poseidon-W5 hashing and domain separation per DEC-016A.
//!
//! Security guarantees:
//! - Leaf insertion is strictly gated: only callable via confirmed & revealed
//!   deposit or valid ZK spend change output. There is NO public unconstrained insert.
//! - Leaves are validated as canonical field elements (Fr < p) to prevent wrap-around attacks.
//! - Bounded ring-buffer root history (size 100) and accepted_note_roots mapping
//!   prevent race conditions when transactions are submitted against recent roots.

use alloc::vec::Vec;
use alloy_primitives::{FixedBytes, U256};
use ark_bls12_381::Fr;

use crate::poseidon_w5_constants::{
    DOMAIN_MERKLE_NODE_BYTES, DOMAIN_MMR_BAG_BYTES, EMPTY_SUBTREE_HASHES_BYTES,
    EMPTY_TREE_ROOT_BYTES, MERKLE_TREE_DEPTH, POSEIDON_W5_MDS_BYTES, POSEIDON_W5_RC_BYTES,
    ROOT_HISTORY_SIZE,
};
use crate::storage::Nimbus;
use crate::types::{from_evm_scalar, to_evm_scalar};

/// S-box: x^5 over Fr
#[inline(always)]
fn sbox(x: Fr) -> Fr {
    let x2 = x * x;
    let x4 = x2 * x2;
    x4 * x
}

/// 5x5 MDS matrix-vector multiplication
fn w5_mds_mul(state: &[Fr; 5], mds: &[[Fr; 5]; 5]) -> [Fr; 5] {
    let mut result = [Fr::from(0u64); 5];
    for i in 0..5 {
        for j in 0..5 {
            result[i] += state[j] * mds[i][j];
        }
    }
    result
}

/// Native width-5 Poseidon permutation
pub fn poseidon_w5_permute(state: &mut [Fr; 5], rc: &[Fr; 345], mds: &[[Fr; 5]; 5]) {
    let mut rc_offset = 0;
    for i in 0..5 {
        state[i] += rc[rc_offset + i];
    }
    rc_offset += 5;

    // First 4 full rounds (S-box on all 5 elements)
    for _ in 0..4 {
        for s in state.iter_mut() {
            *s = sbox(*s);
        }
        *state = w5_mds_mul(state, mds);
        for i in 0..5 {
            state[i] += rc[rc_offset + i];
        }
        rc_offset += 5;
    }

    // 60 partial rounds (S-box on state[0] only)
    for _ in 0..60 {
        state[0] = sbox(state[0]);
        *state = w5_mds_mul(state, mds);
        for i in 0..5 {
            state[i] += rc[rc_offset + i];
        }
        rc_offset += 5;
    }

    // Second 4 full rounds (S-box on all 5 elements)
    for _ in 0..4 {
        for s in state.iter_mut() {
            *s = sbox(*s);
        }
        *state = w5_mds_mul(state, mds);
        for i in 0..5 {
            state[i] += rc[rc_offset + i];
        }
        rc_offset += 5;
    }
}

/// Merkle node hash: H(left, right) using Poseidon-W5 with domain tag
pub fn merkle_hash(left: Fr, right: Fr, rc: &[Fr; 345], mds: &[[Fr; 5]; 5], domain: Fr) -> Fr {
    let mut state = [left, right, Fr::from(0u64), Fr::from(0u64), domain];
    poseidon_w5_permute(&mut state, rc, mds);
    state[0]
}

/// Load precomputed round constants into Fr elements
pub fn load_round_constants() -> [Fr; 345] {
    let mut rc = [Fr::from(0u64); 345];
    for i in 0..345 {
        rc[i] = from_evm_scalar(&POSEIDON_W5_RC_BYTES[i]).expect("valid RC scalar");
    }
    rc
}

/// Load precomputed Cauchy MDS matrix into Fr elements
pub fn load_mds() -> [[Fr; 5]; 5] {
    let mut mds = [[Fr::from(0u64); 5]; 5];
    for i in 0..5 {
        for j in 0..5 {
            mds[i][j] = from_evm_scalar(&POSEIDON_W5_MDS_BYTES[i][j]).expect("valid MDS scalar");
        }
    }
    mds
}

/// Load canonical empty subtree hash for a given tree level
pub fn load_empty_subtree_hash(level: usize) -> Fr {
    from_evm_scalar(&EMPTY_SUBTREE_HASHES_BYTES[level]).expect("valid empty subtree hash")
}

/// Load all canonical empty subtree hashes for levels 0..=20 once into Fr elements
pub fn load_all_empty_subtree_hashes() -> [Fr; 21] {
    let mut arr = [Fr::from(0u64); 21];
    for i in 0..21 {
        arr[i] = from_evm_scalar(&EMPTY_SUBTREE_HASHES_BYTES[i]).expect("valid empty subtree hash");
    }
    arr
}

/// Load canonical domain separator for Merkle node hashing
pub fn load_domain_merkle_node() -> Fr {
    from_evm_scalar(&DOMAIN_MERKLE_NODE_BYTES).expect("valid domain")
}

/// Load canonical domain separator for MMR peak bagging (DEC-032)
pub fn load_domain_mmr_bag() -> Fr {
    from_evm_scalar(&DOMAIN_MMR_BAG_BYTES).expect("valid MMR bag domain")
}

/// Canonical peak bagging per DEC-032 Section 3.C:
/// - If peaks is empty: returns Fr(0)
/// - If m == 1: Root = Poseidon_W5([P_0, Fr(N), 0, 0], DOMAIN_MMR_BAG)
/// - If m > 1:
///   Acc_{m-1} = P_{m-1}
///   Acc_i = Poseidon_W5([P_i, Acc_{i+1}, 0, 0], DOMAIN_MERKLE_NODE) for i = m-2 down to 0
///   Root = Poseidon_W5([Acc_0, Fr(N), 0, 0], DOMAIN_MMR_BAG)
pub fn bag_peaks(
    peaks: &[Fr],
    leaf_count: usize,
    rc: &[Fr; 345],
    mds: &[[Fr; 5]; 5],
    domain_node: Fr,
    domain_bag: Fr,
) -> Fr {
    if peaks.is_empty() {
        return Fr::from(0u64);
    }
    let mut acc = peaks[peaks.len() - 1];
    for i in (0..peaks.len() - 1).rev() {
        acc = merkle_hash(peaks[i], acc, rc, mds, domain_node);
    }
    let mut state = [
        acc,
        Fr::from(leaf_count as u64),
        Fr::from(0u64),
        Fr::from(0u64),
        domain_bag,
    ];
    poseidon_w5_permute(&mut state, rc, mds);
    state[0]
}

impl Nimbus {
    /// Inserts a 32-byte BE leaf (note commitment) into the Merkle Mountain Range (DEC-032).
    ///
    /// Algorithm:
    /// - Checks contract is not paused
    /// - Validates leaf is within scalar field modulus Fr (< r)
    /// - Uses binary carry-adder to merge existing peaks into new mountains
    /// - Collects all active peaks (ordered from left to right: highest to lowest)
    /// - Folds peaks right-to-left and binds total leaf_count via DOMAIN_MMR_BAG
    /// - Updates `note_tree_root` with the canonical bagged root
    /// - Increments `mmr_leaf_count`
    /// - Records new root into `accepted_note_roots` with current block timestamp
    /// - Records new root into bounded `root_history` ring buffer (size 100)
    /// - Emits `NoteCommitmentAppended` and `ChangeCommitment` events
    /// - Returns (leaf_index, new_root)
    pub fn _mmr_insert(&mut self, leaf: FixedBytes<32>) -> Result<(U256, FixedBytes<32>), Vec<u8>> {
        self.check_not_paused()?;

        // Leaf must be a valid Fr scalar (anti-wrap-around security check)
        let leaf_fr = from_evm_scalar(&leaf.0).ok_or_else(|| b"INVALID_LEAF_SCALAR".to_vec())?;

        let leaf_index = self.mmr_leaf_count.get();
        if leaf_index >= U256::from(u64::MAX) {
            return Err(b"MMR_CAPACITY_EXCEEDED".to_vec());
        }

        let rc = load_round_constants();
        let mds = load_mds();
        let domain_node = load_domain_merkle_node();
        let domain_bag = load_domain_mmr_bag();

        // 1. Binary carry-adder peak update
        let mut current = leaf_fr;
        let mut height = 0u64;
        let mut idx = leaf_index.to::<u64>();

        while (idx & 1) == 1 {
            let height_u256 = U256::from(height);
            let left_bytes = self.mmr_peaks.get(height_u256);
            let left_fr =
                from_evm_scalar(&left_bytes.0).ok_or_else(|| b"CORRUPTED_PEAK".to_vec())?;
            current = merkle_hash(left_fr, current, &rc, &mds, domain_node);
            self.mmr_peaks.insert(height_u256, FixedBytes::ZERO);
            height += 1;
            idx >>= 1;
        }

        let height_u256 = U256::from(height);
        self.mmr_peaks
            .insert(height_u256, FixedBytes::from(to_evm_scalar(&current)));

        let new_leaf_count = leaf_index + U256::from(1);
        self.mmr_leaf_count.set(new_leaf_count);

        // 2. Collect active peaks in descending height order (left-to-right)
        let new_count_u64 = new_leaf_count.to::<u64>();
        let mut active_peaks = Vec::new();
        for h in (0..64).rev() {
            if (new_count_u64 >> h) & 1 == 1 {
                let p_bytes = self.mmr_peaks.get(U256::from(h));
                let p_fr = from_evm_scalar(&p_bytes.0).ok_or_else(|| b"CORRUPTED_PEAK".to_vec())?;
                active_peaks.push(p_fr);
            }
        }

        // 3. Canonical peak bagging
        let new_root_fr = bag_peaks(
            &active_peaks,
            new_count_u64 as usize,
            &rc,
            &mds,
            domain_node,
            domain_bag,
        );
        let new_root_bytes = FixedBytes::from(to_evm_scalar(&new_root_fr));

        self.note_tree_root.set(new_root_bytes);

        // Record in accepted roots mapping with block timestamp
        let timestamp = U256::from(self.block_timestamp());
        self.accepted_note_roots.insert(new_root_bytes, timestamp);

        // Record in ring buffer history (native u64 modulo)
        let hist_idx = self.root_history_index.get();
        let slot = U256::from(hist_idx.to::<u64>() % (ROOT_HISTORY_SIZE as u64));
        self.root_history.insert(slot, new_root_bytes);
        self.root_history_index.set(hist_idx + U256::from(1));

        // Emit NoteCommitmentAppended event (DEC-032)
        crate::events::emit_event(crate::events::NoteCommitmentAppended {
            leaf_index,
            commitment: leaf,
            new_mmr_root: new_root_bytes,
            leaf_count: new_leaf_count,
        });

        // Emit ChangeCommitment event for backward compatibility
        crate::events::emit_event(crate::events::ChangeCommitment {
            leaf_index,
            commitment: leaf,
            new_root: new_root_bytes,
        });

        Ok((leaf_index, new_root_bytes))
    }

    /// Alias for backwards compatibility with earlier gates.
    #[inline]
    pub fn _merkle_insert(
        &mut self,
        leaf: FixedBytes<32>,
    ) -> Result<(U256, FixedBytes<32>), Vec<u8>> {
        self._mmr_insert(leaf)
    }

    /// Checks if a root hash is currently accepted:
    /// - Either present in accepted_note_roots (timestamp > 0)
    /// - Or equal to current note_tree_root
    /// - Or equal to canonical EMPTY_TREE_ROOT
    pub fn _is_accepted_note_root(&self, root: FixedBytes<32>) -> bool {
        if root == FixedBytes::from(EMPTY_TREE_ROOT_BYTES) {
            return true;
        }
        if root == self.note_tree_root.get() {
            return true;
        }
        self.accepted_note_roots.get(root) > U256::ZERO
    }
}
