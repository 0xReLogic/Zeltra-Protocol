//! EVM type conversion helpers for BLS12-381 points and scalars

use alloc::vec;
use alloc::vec::Vec;
use ark_bls12_381::{Fr, G1Affine, G2Affine};
use ark_serialize::CanonicalSerialize;

/// Helper to serialize a G1 point to EVM Big-Endian format (128 bytes)
/// EVM G1 point: X (64 bytes, big-endian), Y (64 bytes, big-endian)
/// Arkworks G1Affine uncompressed: X (48 bytes, little-endian), Y (48 bytes, little-endian)
pub fn to_evm_g1(point: &G1Affine) -> [u8; 128] {
    let mut buf = vec![];
    point.serialize_uncompressed(&mut buf).unwrap();
    let mut evm_buf = [0u8; 128];
    // Each coordinate has a 64-byte block in EVM, padded with 16 leading zeros
    for i in 0..2 {
        for j in 0..48 {
            evm_buf[i * 64 + 16 + j] = buf[i * 48 + (47 - j)];
        }
    }
    evm_buf
}

/// Helper to serialize a G2 point to EVM Big-Endian format (256 bytes)
/// EVM G2 point: X1, X0, Y1, Y0 (each 64 bytes, big-endian)
/// Arkworks G2Affine uncompressed: X0, X1, Y0, Y1 (each 48 bytes, little-endian)
pub fn to_evm_g2(point: &G2Affine) -> [u8; 256] {
    let mut buf = vec![];
    point.serialize_uncompressed(&mut buf).unwrap();
    let mut evm_buf = [0u8; 256];
    
    // EVM blocks: block 0 is X1, block 1 is X0, block 2 is Y1, block 3 is Y0
    // Arkworks order of elements: coeff 0 is X0, coeff 1 is X1, coeff 2 is Y0, coeff 3 is Y1
    let src_indices = [1, 0, 3, 2];
    for i in 0..4 {
        let src_idx = src_indices[i];
        for j in 0..48 {
            evm_buf[i * 64 + 16 + j] = buf[src_idx * 48 + (47 - j)];
        }
    }
    evm_buf
}

/// Helper to serialize a scalar Fr to EVM Big-Endian format (32 bytes)
/// Arkworks Fr uncompressed: 32 bytes (little-endian)
pub fn to_evm_scalar(scalar: &Fr) -> [u8; 32] {
    let mut buf = vec![];
    scalar.serialize_uncompressed(&mut buf).unwrap();
    let mut evm_buf = [0u8; 32];
    for j in 0..32 {
        evm_buf[j] = buf[31 - j];
    }
    evm_buf
}
