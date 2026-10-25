//! EVM type conversion helpers for BLS12-381 points and scalars

use alloc::vec;
use alloc::vec::Vec;
use ark_bls12_381::{Fr, G1Affine, G2Affine};
use ark_serialize::CanonicalSerialize;

/// Helper to serialize a G1 point to EVM Big-Endian format (128 bytes)
/// EVM G1 point: X (64 bytes, big-endian), Y (64 bytes, big-endian)
/// Arkworks G1Affine uncompressed: X (48 bytes, big-endian), Y (48 bytes, big-endian)
pub fn to_evm_g1(point: &G1Affine) -> [u8; 128] {
    let mut buf = vec![];
    if point.serialize_uncompressed(&mut buf).is_err() {
        return [0u8; 128];
    }
    if buf.len() != 96 {
        return [0u8; 128];
    }
    let mut evm_buf = [0u8; 128];
    // Each coordinate has a 64-byte block in EVM, padded with 16 leading zeros
    for i in 0..2 {
        for j in 0..48 {
            evm_buf[i * 64 + 16 + j] = buf[i * 48 + j];
        }
    }
    evm_buf
}

/// Helper to serialize a G2 point to EVM Big-Endian format (256 bytes)
/// EVM G2 point: X0, X1, Y0, Y1 (each 64 bytes, big-endian)
/// Arkworks G2Affine uncompressed: X1, X0, Y1, Y0 (each 48 bytes, big-endian)
pub fn to_evm_g2(point: &G2Affine) -> [u8; 256] {
    let mut buf = vec![];
    if point.serialize_uncompressed(&mut buf).is_err() {
        return [0u8; 256];
    }
    if buf.len() != 192 {
        return [0u8; 256];
    }
    let mut evm_buf = [0u8; 256];

    // Arkworks serializes G2 Fp2 limbs as c1 || c0, while EIP-2537 expects c0 || c1.
    let src_indices = [1, 0, 3, 2];
    for i in 0..4 {
        let src_idx = src_indices[i];
        for j in 0..48 {
            evm_buf[i * 64 + 16 + j] = buf[src_idx * 48 + j];
        }
    }
    evm_buf
}

/// Helper to serialize a scalar Fr to EVM Big-Endian format (32 bytes)
/// Arkworks Fr uncompressed: 32 bytes (little-endian)
pub fn to_evm_scalar(scalar: &Fr) -> [u8; 32] {
    let mut buf = vec![];
    if scalar.serialize_uncompressed(&mut buf).is_err() {
        return [0u8; 32];
    }
    if buf.len() != 32 {
        return [0u8; 32];
    }
    let mut evm_buf = [0u8; 32];
    for j in 0..32 {
        evm_buf[j] = buf[31 - j];
    }
    evm_buf
}

/// Helper to deserialize a scalar Fr from EVM Big-Endian format (32 bytes).
/// Validates that the scalar element is strictly within the field modulus.
pub fn from_evm_scalar(bytes: &[u8; 32]) -> Option<Fr> {
    use ark_serialize::CanonicalDeserialize;
    let mut le_buf = [0u8; 32];
    for j in 0..32 {
        le_buf[j] = bytes[31 - j];
    }
    Fr::deserialize_uncompressed(&le_buf[..]).ok()
}

/// BLS12-381 base field modulus q in Big-Endian bytes (48 bytes)
pub const BLS12_381_Q_BE: [u8; 48] = [
    0x1a, 0x01, 0x11, 0xea, 0x39, 0x7f, 0xe6, 0x9a, 0x4b, 0x1b, 0xa7, 0xb6, 0x43, 0x4b, 0xac, 0xd7,
    0x64, 0x77, 0x4b, 0x84, 0xf3, 0x85, 0x12, 0xbf, 0x67, 0x30, 0xd2, 0xa0, 0xf6, 0xb0, 0xf6, 0x24,
    0x1e, 0xab, 0xff, 0xfe, 0xb1, 0x53, 0xff, 0xff, 0xb9, 0xfe, 0xff, 0xff, 0xff, 0xff, 0xaa, 0xab,
];

/// Sanitizes a G1 point from EVM calldata bytes before sending to EIP-2537 precompiles (DEC-052).
pub fn sanitize_g1_point(bytes: &[u8]) -> Result<(), Vec<u8>> {
    if bytes.len() != 128 {
        return Err(b"INVALID_PROOF_LENGTHS".to_vec());
    }
    // GATE 2: Reject point at infinity (all zeros)
    if bytes.iter().all(|&b| b == 0) {
        return Err(b"PointAtInfinityNotAllowed".to_vec());
    }
    let x_bytes = &bytes[16..64];
    let y_bytes = &bytes[80..128];
    if x_bytes.iter().all(|&b| b == 0) && y_bytes.iter().all(|&b| b == 0) {
        return Err(b"PointAtInfinityNotAllowed".to_vec());
    }

    // GATE 4: Canonical coordinate bounds:
    // Leading 16 bytes of each 64-byte block must be zero
    if bytes[0..16].iter().any(|&b| b != 0) || bytes[64..80].iter().any(|&b| b != 0) {
        return Err(b"NonCanonicalFieldElement".to_vec());
    }
    let q_slice = &BLS12_381_Q_BE[..];
    if x_bytes >= q_slice || y_bytes >= q_slice {
        return Err(b"NonCanonicalFieldElement".to_vec());
    }

    // GATE 1 & GATE 3: On-curve and Subgroup check via Arkworks
    use ark_ec::AffineRepr;
    use ark_serialize::CanonicalDeserialize;
    let mut buf = [0u8; 96];
    buf[0..48].copy_from_slice(x_bytes);
    buf[48..96].copy_from_slice(y_bytes);
    let point = G1Affine::deserialize_uncompressed_unchecked(&buf[..])
        .map_err(|_| b"NonCanonicalFieldElement".to_vec())?;

    if !point.is_on_curve() {
        return Err(b"CurvePointNotOnCurve".to_vec());
    }
    if !point.is_in_correct_subgroup_assuming_on_curve() {
        return Err(b"SubgroupCheckFailed".to_vec());
    }

    Ok(())
}

/// Sanitizes a G2 point from EVM calldata bytes before sending to EIP-2537 precompiles (DEC-052).
pub fn sanitize_g2_point(bytes: &[u8]) -> Result<(), Vec<u8>> {
    if bytes.len() != 256 {
        return Err(b"INVALID_PROOF_LENGTHS".to_vec());
    }
    // GATE 2: Reject point at infinity (all zeros)
    if bytes.iter().all(|&b| b == 0) {
        return Err(b"PointAtInfinityNotAllowed".to_vec());
    }
    for i in 0..4 {
        if bytes[i * 64..i * 64 + 16].iter().any(|&b| b != 0) {
            return Err(b"NonCanonicalFieldElement".to_vec());
        }
    }
    let x0_bytes = &bytes[16..64];
    let x1_bytes = &bytes[80..128];
    let y0_bytes = &bytes[144..192];
    let y1_bytes = &bytes[208..256];

    if x0_bytes.iter().all(|&b| b == 0)
        && x1_bytes.iter().all(|&b| b == 0)
        && y0_bytes.iter().all(|&b| b == 0)
        && y1_bytes.iter().all(|&b| b == 0)
    {
        return Err(b"PointAtInfinityNotAllowed".to_vec());
    }

    let q_slice = &BLS12_381_Q_BE[..];
    if x0_bytes >= q_slice || x1_bytes >= q_slice || y0_bytes >= q_slice || y1_bytes >= q_slice {
        return Err(b"NonCanonicalFieldElement".to_vec());
    }

    use ark_ec::AffineRepr;
    use ark_serialize::CanonicalDeserialize;
    let mut buf = [0u8; 192];
    buf[0..48].copy_from_slice(x1_bytes);
    buf[48..96].copy_from_slice(x0_bytes);
    buf[96..144].copy_from_slice(y1_bytes);
    buf[144..192].copy_from_slice(y0_bytes);

    let point = G2Affine::deserialize_uncompressed_unchecked(&buf[..])
        .map_err(|_| b"NonCanonicalFieldElement".to_vec())?;

    if !point.is_on_curve() {
        return Err(b"CurvePointNotOnCurve".to_vec());
    }
    if !point.is_in_correct_subgroup_assuming_on_curve() {
        return Err(b"SubgroupCheckFailed".to_vec());
    }

    Ok(())
}
