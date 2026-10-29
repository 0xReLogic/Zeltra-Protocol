//! Merkle Mountain Belt (MMB) & Increment Proof Engine — arXiv:2511.13582v1 & DEC-057
//!
//! Provides the off-chain Merkle Mountain Belt accumulator featuring:
//! 1. O(1) worst-case lazy-merging append (at most 1 binary merge per leaf).
//! 2. Double-bagging accumulator with bitwise range split derivation.
//! 3. O(log k) increment proof generation for state transitions from size m to m + k.
//! 4. O(log k) client frontier verification and synchronization (`verify_and_sync`).
//!
//! Note: Strictly off-chain / client-relayer sync protocol per DEC-057 architectural invariant.

use ark_bls12_381::Fr;
use serde::{Deserialize, Serialize};

use crate::merkle::merkle_hash;
use crate::note::{fr_from_be_bytes, fr_to_be_bytes};

// ─── Serde Helpers for Fr in MMB ───────────────────────────────────────────

fn serialize_fr<S: serde::Serializer>(val: &Fr, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(&hex::encode(fr_to_be_bytes(val)))
}

fn deserialize_fr<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Fr, D::Error> {
    let s = String::deserialize(deserializer)?;
    let clean = s.strip_prefix("0x").unwrap_or(&s);
    let bytes = hex::decode(clean).map_err(serde::de::Error::custom)?;
    if bytes.len() != 32 {
        return Err(serde::de::Error::custom("expected 32 bytes for Fr scalar"));
    }
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&bytes);
    fr_from_be_bytes(&arr)
        .ok_or_else(|| serde::de::Error::custom("scalar element >= r (non-canonical)"))
}

fn serialize_opt_fr<S: serde::Serializer>(
    val: &Option<Fr>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    match val {
        Some(fr) => serializer.serialize_some(&hex::encode(fr_to_be_bytes(fr))),
        None => serializer.serialize_none(),
    }
}

fn deserialize_opt_fr<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<Fr>, D::Error> {
    let opt: Option<String> = Option::deserialize(deserializer)?;
    match opt {
        Some(s) => {
            let clean = s.strip_prefix("0x").unwrap_or(&s);
            let bytes = hex::decode(clean).map_err(serde::de::Error::custom)?;
            if bytes.len() != 32 {
                return Err(serde::de::Error::custom("expected 32 bytes for Fr scalar"));
            }
            let mut arr = [0u8; 32];
            arr.copy_from_slice(&bytes);
            let fr = fr_from_be_bytes(&arr)
                .ok_or_else(|| serde::de::Error::custom("scalar element >= r (non-canonical)"))?;
            Ok(Some(fr))
        }
        None => Ok(None),
    }
}

// ─── Errors ─────────────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MmbError {
    InvalidSizeTransition { from: usize, to: usize },
    RootMismatch { expected: Fr, actual: Fr },
    LeafCountMismatch { expected: usize, actual: usize },
    CanonicalHeightMismatch { position: usize, expected: usize, actual: usize },
    PeakCountMismatch { expected: usize, actual: usize },
    InvalidHighMerge { pos: usize, peak_count: usize },
    DeserializationError(String),
}

impl std::fmt::Display for MmbError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidSizeTransition { from, to } => {
                write!(f, "Invalid size transition: from {from} > to {to}")
            }
            Self::RootMismatch { expected, actual } => {
                write!(
                    f,
                    "MMB root mismatch: expected 0x{}, got 0x{}",
                    hex::encode(fr_to_be_bytes(expected)),
                    hex::encode(fr_to_be_bytes(actual))
                )
            }
            Self::LeafCountMismatch { expected, actual } => {
                write!(f, "Leaf count mismatch: expected {expected}, got {actual}")
            }
            Self::CanonicalHeightMismatch { position, expected, actual } => {
                write!(
                    f,
                    "Canonical height mismatch at pos {position}: expected {expected}, got {actual}"
                )
            }
            Self::PeakCountMismatch { expected, actual } => {
                write!(f, "Peak count mismatch: expected {expected}, got {actual}")
            }
            Self::InvalidHighMerge { pos, peak_count } => {
                write!(f, "Invalid high merge at pos {pos} with peak count {peak_count}")
            }
            Self::DeserializationError(msg) => {
                write!(f, "MMB deserialization error: {msg}")
            }
        }
    }
}

impl std::error::Error for MmbError {}

// ─── Core Data Structures ───────────────────────────────────────────────────

/// Peak descriptor in the Merkle Mountain Belt mountain forest.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MmbPeak {
    #[serde(serialize_with = "serialize_fr", deserialize_with = "deserialize_fr")]
    pub hash: Fr,
    pub height: usize,
    pub position: usize,
}

/// Active Light Client Frontier (minimal state kept in client memory).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MmbFrontier {
    pub leaf_count: usize,
    pub peaks: Vec<MmbPeak>,
    #[serde(serialize_with = "serialize_fr", deserialize_with = "deserialize_fr")]
    pub root: Fr,
}

impl Default for MmbFrontier {
    fn default() -> Self {
        Self {
            leaf_count: 0,
            peaks: Vec::new(),
            root: Fr::from(0u64),
        }
    }
}

impl MmbFrontier {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_peaks(leaf_count: usize, peaks: Vec<MmbPeak>, root: Fr) -> Self {
        Self { leaf_count, peaks, root }
    }

    /// Verifies the O(log k) increment proof and updates the frontier in O(log k) time and memory.
    pub fn verify_and_sync(&mut self, proof: &MmbIncrementProof) -> Result<(), MmbError> {
        if proof.from_size != self.leaf_count {
            return Err(MmbError::LeafCountMismatch {
                expected: self.leaf_count,
                actual: proof.from_size,
            });
        }
        if proof.to_size < proof.from_size {
            return Err(MmbError::InvalidSizeTransition {
                from: proof.from_size,
                to: proof.to_size,
            });
        }
        if proof.from_size == proof.to_size {
            if self.leaf_count > 0 && proof.old_root != self.root {
                return Err(MmbError::RootMismatch {
                    expected: self.root,
                    actual: proof.old_root,
                });
            }
            return Ok(());
        }

        if self.leaf_count > 0 && proof.old_root != self.root {
            return Err(MmbError::RootMismatch {
                expected: self.root,
                actual: proof.old_root,
            });
        }

        let expected_h = MerkleMountainBelt::canonical_heights(proof.to_size);
        let expected_t = expected_h.len();

        let mut candidate_peaks: Vec<MmbPeak>;

        if self.leaf_count == 0 {
            // Full frontier delivered via delta_peaks
            if proof.delta_peaks.len() != expected_t {
                return Err(MmbError::PeakCountMismatch {
                    expected: expected_t,
                    actual: proof.delta_peaks.len(),
                });
            }
            candidate_peaks = proof.delta_peaks.clone();
            for (idx, p) in candidate_peaks.iter_mut().enumerate() {
                p.position = idx;
            }
        } else {
            let mut client_peaks = self.peaks.clone();

            // 1. If a high merge occurred, merge the pair locally
            if let Some(pos) = proof.high_merge_pos {
                if pos + 1 >= client_peaks.len() {
                    return Err(MmbError::InvalidHighMerge {
                        pos,
                        peak_count: client_peaks.len(),
                    });
                }
                let left = client_peaks[pos].clone();
                let right = client_peaks[pos + 1].clone();
                if left.height != right.height {
                    return Err(MmbError::CanonicalHeightMismatch {
                        position: pos,
                        expected: left.height,
                        actual: right.height,
                    });
                }
                let merged_hash = merkle_hash(left.hash, right.hash);
                let merged_height = left.height + 1;
                client_peaks[pos] = MmbPeak {
                    hash: merged_hash,
                    height: merged_height,
                    position: pos,
                };
                client_peaks.remove(pos + 1);
            }

            // 2. Determine prefix needed to complement delta_peaks
            if proof.delta_peaks.len() > expected_t {
                return Err(MmbError::PeakCountMismatch {
                    expected: expected_t,
                    actual: proof.delta_peaks.len(),
                });
            }
            let needed_prefix_count = expected_t - proof.delta_peaks.len();
            if client_peaks.len() < needed_prefix_count {
                return Err(MmbError::PeakCountMismatch {
                    expected: needed_prefix_count,
                    actual: client_peaks.len(),
                });
            }

            candidate_peaks = Vec::with_capacity(expected_t);
            candidate_peaks.extend_from_slice(&client_peaks[..needed_prefix_count]);
            candidate_peaks.extend_from_slice(&proof.delta_peaks);

            for (idx, p) in candidate_peaks.iter_mut().enumerate() {
                p.position = idx;
            }
        }

        // 3. Verify that candidate peaks match canonical height sequence
        if candidate_peaks.len() != expected_t {
            return Err(MmbError::PeakCountMismatch {
                expected: expected_t,
                actual: candidate_peaks.len(),
            });
        }
        for (i, p) in candidate_peaks.iter().enumerate() {
            if p.height != expected_h[i] {
                return Err(MmbError::CanonicalHeightMismatch {
                    position: i,
                    expected: expected_h[i],
                    actual: p.height,
                });
            }
        }

        // 4. Compute double-bagged root from candidate peaks
        let computed_root = MerkleMountainBelt::compute_root_from_peaks(proof.to_size, &candidate_peaks);
        if computed_root != proof.new_root {
            return Err(MmbError::RootMismatch {
                expected: proof.new_root,
                actual: computed_root,
            });
        }

        // 5. Update client frontier
        self.leaf_count = proof.to_size;
        self.peaks = candidate_peaks;
        self.root = computed_root;

        Ok(())
    }
}

/// Increment Proof wire structure for size transition from_size -> to_size.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MmbIncrementProof {
    pub from_size: usize,
    pub to_size: usize,
    #[serde(serialize_with = "serialize_fr", deserialize_with = "deserialize_fr")]
    pub old_root: Fr,
    #[serde(serialize_with = "serialize_fr", deserialize_with = "deserialize_fr")]
    pub new_root: Fr,
    pub high_merge_pos: Option<usize>,
    pub delta_peaks: Vec<MmbPeak>,
    #[serde(default, skip_serializing_if = "Option::is_none", serialize_with = "serialize_opt_fr", deserialize_with = "deserialize_opt_fr")]
    pub belt_node_a: Option<Fr>,
    #[serde(default, skip_serializing_if = "Option::is_none", serialize_with = "serialize_opt_fr", deserialize_with = "deserialize_opt_fr")]
    pub range_node_b: Option<Fr>,
    #[serde(default, skip_serializing_if = "Option::is_none", serialize_with = "serialize_opt_fr", deserialize_with = "deserialize_opt_fr")]
    pub range_node_c: Option<Fr>,
}

impl MmbIncrementProof {
    /// Serializes proof into compact canonical binary representation.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(96 + self.delta_peaks.len() * 34);
        bytes.extend_from_slice(&(self.from_size as u64).to_le_bytes());
        bytes.extend_from_slice(&(self.to_size as u64).to_le_bytes());
        bytes.extend_from_slice(&fr_to_be_bytes(&self.old_root));
        bytes.extend_from_slice(&fr_to_be_bytes(&self.new_root));

        match self.high_merge_pos {
            Some(pos) => bytes.push(pos as u8),
            None => bytes.push(0xFF),
        }

        bytes.extend_from_slice(&(self.delta_peaks.len() as u16).to_le_bytes());
        for p in &self.delta_peaks {
            bytes.extend_from_slice(&fr_to_be_bytes(&p.hash));
            bytes.push(p.height as u8);
            bytes.push(p.position as u8);
        }

        let mut witness_mask = 0u8;
        if self.belt_node_a.is_some() { witness_mask |= 1; }
        if self.range_node_b.is_some() { witness_mask |= 2; }
        if self.range_node_c.is_some() { witness_mask |= 4; }
        bytes.push(witness_mask);

        if let Some(a) = &self.belt_node_a {
            bytes.extend_from_slice(&fr_to_be_bytes(a));
        }
        if let Some(b) = &self.range_node_b {
            bytes.extend_from_slice(&fr_to_be_bytes(b));
        }
        if let Some(c) = &self.range_node_c {
            bytes.extend_from_slice(&fr_to_be_bytes(c));
        }

        bytes
    }

    /// Deserializes proof from compact canonical binary representation.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, MmbError> {
        if bytes.len() < 83 {
            return Err(MmbError::DeserializationError("binary proof buffer too short".into()));
        }
        let mut offset = 0;

        let from_size = u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap()) as usize;
        offset += 8;
        let to_size = u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap()) as usize;
        offset += 8;

        let old_root_bytes: [u8; 32] = bytes[offset..offset + 32].try_into().unwrap();
        offset += 32;
        let old_root = fr_from_be_bytes(&old_root_bytes)
            .ok_or_else(|| MmbError::DeserializationError("non-canonical old_root".into()))?;

        let new_root_bytes: [u8; 32] = bytes[offset..offset + 32].try_into().unwrap();
        offset += 32;
        let new_root = fr_from_be_bytes(&new_root_bytes)
            .ok_or_else(|| MmbError::DeserializationError("non-canonical new_root".into()))?;

        let high_byte = bytes[offset];
        offset += 1;
        let high_merge_pos = if high_byte == 0xFF { None } else { Some(high_byte as usize) };

        let delta_len = u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap()) as usize;
        offset += 2;

        let mut delta_peaks = Vec::with_capacity(delta_len);
        for _ in 0..delta_len {
            if offset + 34 > bytes.len() {
                return Err(MmbError::DeserializationError("truncated delta peak in binary proof".into()));
            }
            let hash_bytes: [u8; 32] = bytes[offset..offset + 32].try_into().unwrap();
            offset += 32;
            let hash = fr_from_be_bytes(&hash_bytes)
                .ok_or_else(|| MmbError::DeserializationError("non-canonical peak hash".into()))?;
            let height = bytes[offset] as usize;
            offset += 1;
            let position = bytes[offset] as usize;
            offset += 1;

            delta_peaks.push(MmbPeak { hash, height, position });
        }

        if offset >= bytes.len() {
            return Ok(Self {
                from_size,
                to_size,
                old_root,
                new_root,
                high_merge_pos,
                delta_peaks,
                belt_node_a: None,
                range_node_b: None,
                range_node_c: None,
            });
        }

        let witness_mask = bytes[offset];
        offset += 1;

        let mut belt_node_a = None;
        if witness_mask & 1 != 0 {
            if offset + 32 > bytes.len() {
                return Err(MmbError::DeserializationError("truncated belt_node_a".into()));
            }
            let h_bytes: [u8; 32] = bytes[offset..offset + 32].try_into().unwrap();
            offset += 32;
            belt_node_a = Some(fr_from_be_bytes(&h_bytes)
                .ok_or_else(|| MmbError::DeserializationError("non-canonical belt_node_a".into()))?);
        }

        let mut range_node_b = None;
        if witness_mask & 2 != 0 {
            if offset + 32 > bytes.len() {
                return Err(MmbError::DeserializationError("truncated range_node_b".into()));
            }
            let h_bytes: [u8; 32] = bytes[offset..offset + 32].try_into().unwrap();
            offset += 32;
            range_node_b = Some(fr_from_be_bytes(&h_bytes)
                .ok_or_else(|| MmbError::DeserializationError("non-canonical range_node_b".into()))?);
        }

        let mut range_node_c = None;
        if witness_mask & 4 != 0 {
            if offset + 32 > bytes.len() {
                return Err(MmbError::DeserializationError("truncated range_node_c".into()));
            }
            let h_bytes: [u8; 32] = bytes[offset..offset + 32].try_into().unwrap();
            #[allow(unused_assignments)]
            { offset += 32; }
            range_node_c = Some(fr_from_be_bytes(&h_bytes)
                .ok_or_else(|| MmbError::DeserializationError("non-canonical range_node_c".into()))?);
        }

        Ok(Self {
            from_size,
            to_size,
            old_root,
            new_root,
            high_merge_pos,
            delta_peaks,
            belt_node_a,
            range_node_b,
            range_node_c,
        })
    }
}

// ─── Merkle Mountain Belt Accumulator ───────────────────────────────────────

/// Full In-Memory Merkle Mountain Belt Accumulator (Server / Relayer State).
#[derive(Clone, Debug, Default)]
pub struct MerkleMountainBelt {
    pub leaf_count: usize,
    pub leaves: Vec<Fr>,
    pub peaks: Vec<MmbPeak>,
    pub mergeable_pairs: Vec<usize>,
    pub cached_root: Option<Fr>,
}

impl MerkleMountainBelt {
    pub fn new() -> Self {
        Self::default()
    }

    /// Derives the expected mountain heights (from left to right) for leaf count `n`.
    ///
    /// Per Lemma 6: For n+1 in binary (b_t ... b_0)_2 with t = floor(log2(n + 1)):
    /// Mountain M_d at distance d from right (0 <= d < t) has height s_d = d + b_d.
    pub fn canonical_heights(n: usize) -> Vec<usize> {
        if n == 0 {
            return Vec::new();
        }
        let n_plus_1 = n + 1;
        let t = (usize::BITS - n_plus_1.leading_zeros() - 1) as usize;
        let mut heights = Vec::with_capacity(t);
        for d in (0..t).rev() {
            let b_d = (n_plus_1 >> d) & 1;
            heights.push(d + b_d);
        }
        heights
    }

    /// Computes the double-bagged belt root from an arbitrary slice of peaks for a given `n`.
    pub fn compute_root_from_peaks(n: usize, peaks: &[MmbPeak]) -> Fr {
        if peaks.is_empty() || n == 0 {
            return Fr::from(0u64);
        }
        let ranges = Self::compute_ranges_for_size(n, peaks.len());
        if ranges.is_empty() {
            return Fr::from(0u64);
        }

        let mut range_roots = Vec::with_capacity(ranges.len());
        for r in &ranges {
            if r.is_empty() {
                continue;
            }
            let mut acc = peaks[r[0]].hash;
            for &idx in &r[1..] {
                acc = merkle_hash(acc, peaks[idx].hash);
            }
            range_roots.push(acc);
        }

        if range_roots.is_empty() {
            return Fr::from(0u64);
        }

        let mut belt_acc = range_roots[0];
        for &r_root in &range_roots[1..] {
            belt_acc = merkle_hash(belt_acc, r_root);
        }
        belt_acc
    }

    /// Partitions peak indices 0..t into ranges according to Lemma 28 / bitwise split condition.
    pub fn compute_ranges_for_size(n: usize, t: usize) -> Vec<Vec<usize>> {
        if t == 0 || n == 0 {
            return Vec::new();
        }
        let mut ranges = Vec::new();
        let mut curr_range = vec![0];
        let n_plus_1 = n + 1;

        for p in 0..(t - 1) {
            let d = t - 1 - p;
            let b_d = (n_plus_1 >> d) & 1;
            let b_prev = if d > 0 { (n_plus_1 >> (d - 1)) & 1 } else { 0 };
            let b_next = (n_plus_1 >> (d + 1)) & 1;

            let is_split = (b_d == 1) && (b_prev == 0 || (d + 1 < t && b_next == 0));
            if is_split {
                ranges.push(curr_range);
                curr_range = vec![p + 1];
            } else {
                curr_range.push(p + 1);
            }
        }
        ranges.push(curr_range);
        ranges
    }

    /// Appends a new leaf using Algorithm 1 lazy merging (worst-case at most 1 merge step).
    pub fn append(&mut self, leaf: Fr) -> (usize, Fr) {
        let leaf_idx = self.leaf_count;
        self.leaf_count += 1;
        self.leaves.push(leaf);

        // 1. ADD step
        let new_peak = MmbPeak {
            hash: leaf,
            height: 0,
            position: self.peaks.len(),
        };

        if let Some(last) = self.peaks.last() {
            if last.height == 0 {
                self.mergeable_pairs.push(self.peaks.len());
            }
        }
        self.peaks.push(new_peak);

        // 2. MERGE step (at most 1 merge)
        if let Some(right_idx) = self.mergeable_pairs.pop() {
            let left_idx = right_idx - 1;
            let left_peak = self.peaks[left_idx].clone();
            let right_peak = self.peaks[right_idx].clone();
            debug_assert_eq!(left_peak.height, right_peak.height);

            let parent_hash = merkle_hash(left_peak.hash, right_peak.hash);
            let parent_height = left_peak.height + 1;

            self.peaks[left_idx] = MmbPeak {
                hash: parent_hash,
                height: parent_height,
                position: left_idx,
            };
            self.peaks.remove(right_idx);

            // Re-index remaining peaks' positions
            for (idx, p) in self.peaks.iter_mut().enumerate() {
                p.position = idx;
            }

            // Check if new parent has left neighbor with identical height
            if left_idx > 0 && self.peaks[left_idx - 1].height == parent_height {
                self.mergeable_pairs.push(left_idx);
            }
        }

        let root = self.recompute_belt_root();
        self.cached_root = Some(root);
        (leaf_idx, root)
    }

    /// Recomputes the double-bagged root of the current accumulator state.
    pub fn recompute_belt_root(&self) -> Fr {
        Self::compute_root_from_peaks(self.leaf_count, &self.peaks)
    }

    /// Partitions current mountains into ranges.
    pub fn compute_ranges(&self) -> Vec<Vec<usize>> {
        Self::compute_ranges_for_size(self.leaf_count, self.peaks.len())
    }

    /// Gets current root.
    pub fn get_root(&self) -> Fr {
        self.cached_root.unwrap_or_else(|| self.recompute_belt_root())
    }

    /// Extracts current frontier state for a light client.
    pub fn get_frontier(&self) -> MmbFrontier {
        MmbFrontier {
            leaf_count: self.leaf_count,
            peaks: self.peaks.clone(),
            root: self.get_root(),
        }
    }

    /// Builds the peak set and root at a specific historical size `size <= self.leaf_count`.
    pub fn peaks_at_size(&self, size: usize) -> (Vec<MmbPeak>, Fr) {
        if size == self.leaf_count {
            return (self.peaks.clone(), self.get_root());
        }
        if size == 0 {
            return (Vec::new(), Fr::from(0u64));
        }

        let mut sub_belt = Self::new();
        for &leaf in &self.leaves[..size] {
            sub_belt.append(leaf);
        }
        let root = sub_belt.get_root();
        (sub_belt.peaks, root)
    }

    /// Generates an O(log k) increment proof given known historic peaks and root at from_size.
    pub fn generate_increment_proof_with_from_peaks(
        &self,
        from_size: usize,
        peaks_from: &[MmbPeak],
        old_root: Fr,
        to_size: usize,
    ) -> Result<MmbIncrementProof, MmbError> {
        if from_size > to_size || to_size > self.leaf_count {
            return Err(MmbError::InvalidSizeTransition {
                from: from_size,
                to: to_size,
            });
        }

        let (peaks_to, new_root) = if to_size == self.leaf_count {
            (self.peaks.clone(), self.get_root())
        } else {
            self.peaks_at_size(to_size)
        };

        if from_size == to_size {
            return Ok(MmbIncrementProof {
                from_size,
                to_size,
                old_root,
                new_root,
                high_merge_pos: None,
                delta_peaks: Vec::new(),
                belt_node_a: None,
                range_node_b: None,
                range_node_c: None,
            });
        }

        if from_size == 0 {
            return Ok(MmbIncrementProof {
                from_size,
                to_size,
                old_root,
                new_root,
                high_merge_pos: None,
                delta_peaks: peaks_to,
                belt_node_a: None,
                range_node_b: None,
                range_node_c: None,
            });
        }

        // 1. Find matching prefix of peaks between from_size and to_size
        let mut prefix_len = 0;
        while prefix_len < peaks_from.len()
            && prefix_len < peaks_to.len()
            && peaks_from[prefix_len].hash == peaks_to[prefix_len].hash
            && peaks_from[prefix_len].height == peaks_to[prefix_len].height
        {
            prefix_len += 1;
        }

        // 2. Check if the first differing peak in to_size is formed by merging two adjacent peaks of from_size
        let mut high_merge_pos = None;
        if prefix_len + 1 < peaks_from.len() && prefix_len < peaks_to.len() {
            let left = &peaks_from[prefix_len];
            let right = &peaks_from[prefix_len + 1];
            if left.height == right.height {
                let candidate_hash = merkle_hash(left.hash, right.hash);
                if candidate_hash == peaks_to[prefix_len].hash
                    && left.height + 1 == peaks_to[prefix_len].height
                {
                    high_merge_pos = Some(prefix_len);
                    prefix_len += 1;
                }
            }
        }

        // 3. The remaining peaks in peaks_to form delta_peaks (O(log k) elements)
        let delta_peaks = peaks_to[prefix_len..].to_vec();

        // 4. Compute optional bagging witnesses
        let ranges = Self::compute_ranges_for_size(to_size, peaks_to.len());
        let mut belt_node_a = None;
        let mut range_node_b = None;

        if !ranges.is_empty() {
            // Find how many complete ranges fit strictly within prefix_len
            let mut completed_ranges = 0;
            for r in &ranges {
                if let Some(&last_idx) = r.last() {
                    if last_idx < prefix_len {
                        completed_ranges += 1;
                    } else {
                        break;
                    }
                }
            }

            if completed_ranges > 0 {
                let mut range_roots = Vec::with_capacity(completed_ranges);
                for r in &ranges[..completed_ranges] {
                    let mut acc = peaks_to[r[0]].hash;
                    for &idx in &r[1..] {
                        acc = merkle_hash(acc, peaks_to[idx].hash);
                    }
                    range_roots.push(acc);
                }
                let mut b_acc = range_roots[0];
                for &rr in &range_roots[1..] {
                    b_acc = merkle_hash(b_acc, rr);
                }
                belt_node_a = Some(b_acc);
            }

            if completed_ranges < ranges.len() {
                let partial_range = &ranges[completed_ranges];
                let mut partial_count = 0;
                for &idx in partial_range {
                    if idx < prefix_len {
                        partial_count += 1;
                    } else {
                        break;
                    }
                }
                if partial_count > 0 {
                    let mut acc = peaks_to[partial_range[0]].hash;
                    for &idx in &partial_range[1..partial_count] {
                        acc = merkle_hash(acc, peaks_to[idx].hash);
                    }
                    range_node_b = Some(acc);
                }
            }
        }

        Ok(MmbIncrementProof {
            from_size,
            to_size,
            old_root,
            new_root,
            high_merge_pos,
            delta_peaks,
            belt_node_a,
            range_node_b,
            range_node_c: None,
        })
    }

    /// Generates an O(log k) increment proof transitioning client from `from_size` to `to_size`.
    pub fn generate_increment_proof(
        &self,
        from_size: usize,
        to_size: usize,
    ) -> Result<MmbIncrementProof, MmbError> {
        let (peaks_from, old_root) = self.peaks_at_size(from_size);
        self.generate_increment_proof_with_from_peaks(from_size, &peaks_from, old_root, to_size)
    }
}

// ─── Unit Tests ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mmb_append_and_canonical_heights() {
        let mut belt = MerkleMountainBelt::new();
        for i in 1..=16 {
            let leaf = Fr::from(i as u64);
            let (idx, root) = belt.append(leaf);
            assert_eq!(idx, i - 1);
            assert_eq!(belt.leaf_count, i);

            let expected_h = MerkleMountainBelt::canonical_heights(i);
            let actual_h: Vec<usize> = belt.peaks.iter().map(|p| p.height).collect();
            assert_eq!(actual_h, expected_h, "Height mismatch at N={i}");
            assert_eq!(root, belt.recompute_belt_root());
        }
    }

    #[test]
    fn test_mmb_increment_proof_k_cases() {
        let mut belt = MerkleMountainBelt::new();
        for i in 1..=500 {
            belt.append(Fr::from(i as u64));
        }

        // Test transitions across different lags
        for from in [0, 1, 10, 50, 100, 255] {
            for lag in [0, 1, 5, 10, 50, 100, 200] {
                let to = from + lag;
                if to > 500 {
                    continue;
                }

                let proof = belt.generate_increment_proof(from, to).expect("generate proof");
                assert_eq!(proof.from_size, from);
                assert_eq!(proof.to_size, to);

                let mut client_frontier = if from == 0 {
                    MmbFrontier::new()
                } else {
                    let (peaks, root) = belt.peaks_at_size(from);
                    MmbFrontier::from_peaks(from, peaks, root)
                };

                client_frontier.verify_and_sync(&proof).expect("client verify and sync");
                assert_eq!(client_frontier.leaf_count, to);
                assert_eq!(client_frontier.root, proof.new_root);

                let (expected_peaks, expected_root) = belt.peaks_at_size(to);
                assert_eq!(client_frontier.root, expected_root);
                assert_eq!(client_frontier.peaks, expected_peaks);
            }
        }
    }

    #[test]
    fn test_mmb_binary_and_json_serialization() {
        let mut belt = MerkleMountainBelt::new();
        for i in 1..=100 {
            belt.append(Fr::from(i as u64));
        }

        let proof = belt.generate_increment_proof(10, 100).expect("proof");

        // Test JSON serde
        let json_bytes = serde_json::to_vec(&proof).expect("serialize json");
        let proof_from_json: MmbIncrementProof =
            serde_json::from_slice(&json_bytes).expect("deserialize json");
        assert_eq!(proof, proof_from_json);

        // Test Binary serde
        let bin_bytes = proof.to_bytes();
        let proof_from_bin = MmbIncrementProof::from_bytes(&bin_bytes).expect("deserialize binary");
        assert_eq!(proof, proof_from_bin);
    }

    #[test]
    fn test_mmb_tampered_proof_rejection() {
        let mut belt = MerkleMountainBelt::new();
        for i in 1..=20 {
            belt.append(Fr::from(i as u64));
        }

        let mut proof = belt.generate_increment_proof(5, 20).expect("proof");
        let (peaks_5, root_5) = belt.peaks_at_size(5);

        // Tamper with root
        proof.new_root = Fr::from(999999u64);
        let mut client = MmbFrontier::from_peaks(5, peaks_5.clone(), root_5);
        assert!(client.verify_and_sync(&proof).is_err());

        // Tamper with delta peaks
        let mut proof2 = belt.generate_increment_proof(5, 20).expect("proof");
        if let Some(first_delta) = proof2.delta_peaks.first_mut() {
            first_delta.hash = Fr::from(888888u64);
        }
        let mut client2 = MmbFrontier::from_peaks(5, peaks_5, root_5);
        assert!(client2.verify_and_sync(&proof2).is_err());
    }
}
