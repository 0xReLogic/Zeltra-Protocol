use crate::crypto::hash_to_g1;
use crate::types::{IssuerPublicKey, UnmaskedSignature};
use ark_bls12_381::{G1Affine, G2Affine};
use ark_ec::CurveGroup;
use ark_serialize::CanonicalSerialize;

/// Helper to serialize a G1 point to EVM Big-Endian format (128 bytes)
pub fn to_evm_g1(point: &G1Affine) -> Vec<u8> {
    let mut buf = vec![];
    if point.serialize_uncompressed(&mut buf).is_err() {
        return vec![0u8; 128];
    }
    if buf.len() != 96 {
        return vec![0u8; 128];
    }
    let mut evm_buf = vec![0u8; 128];
    for i in 0..2 {
        for j in 0..48 {
            evm_buf[i * 64 + 16 + j] = buf[i * 48 + j];
        }
    }
    evm_buf
}

/// Helper to serialize a G2 point to EVM Big-Endian format (256 bytes)
pub fn to_evm_g2(point: &G2Affine) -> Vec<u8> {
    let mut buf = vec![];
    if point.serialize_uncompressed(&mut buf).is_err() {
        return vec![0u8; 256];
    }
    if buf.len() != 192 {
        return vec![0u8; 256];
    }
    let mut evm_buf = vec![0u8; 256];
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

/// Computes the negated unmasked signature (-alpha) in EVM G1 format (128 bytes)
pub fn get_alpha_neg_evm(alpha: &UnmaskedSignature) -> Vec<u8> {
    let negated = -alpha.0;
    let negated_affine = negated.into_affine();
    to_evm_g1(&negated_affine)
}

/// Computes the hash of the message H(m) in EVM G1 format (128 bytes)
pub fn get_hm_evm(message: &[u8]) -> Vec<u8> {
    let hm = hash_to_g1(message);
    let hm_affine = hm.into_affine();
    to_evm_g1(&hm_affine)
}

/// Computes the issuer public key pk_iss in EVM G2 format (256 bytes)
pub fn get_pk_iss_evm(pk_iss: &IssuerPublicKey) -> Vec<u8> {
    let pk_affine = pk_iss.0.into_affine();
    to_evm_g2(&pk_affine)
}

/// Helper to deserialize a scalar Fr from EVM Big-Endian format (32 bytes).
/// Validates that the scalar element is strictly within the field modulus (< r).
pub fn from_evm_scalar(bytes: &[u8; 32]) -> Option<ark_bls12_381::Fr> {
    use ark_serialize::CanonicalDeserialize;
    let mut le_buf = [0u8; 32];
    for j in 0..32 {
        le_buf[j] = bytes[31 - j];
    }
    ark_bls12_381::Fr::deserialize_uncompressed(&le_buf[..]).ok()
}
