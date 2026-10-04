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

/// Helper to deserialize a G1 point from EVM Big-Endian format (128 bytes).
/// EVM G1 point: X (64 bytes, 16 zero padding + 48 bytes coordinate), Y (64 bytes, 16 zero padding + 48 bytes coordinate).
/// Validates padding, coordinates, curve equation, and prime-order subgroup.
pub fn from_evm_g1(bytes: &[u8; 128]) -> Option<G1Affine> {
    use ark_ec::AffineRepr;
    use ark_serialize::CanonicalDeserialize;
    // Point at infinity in EVM is all zeros
    if bytes.iter().all(|&b| b == 0) {
        return Some(G1Affine::zero());
    }
    // Leading 16 bytes of each 64-byte block must be zero
    if bytes[0..16].iter().any(|&b| b != 0) || bytes[64..80].iter().any(|&b| b != 0) {
        return None;
    }
    let mut buf = [0u8; 96];
    buf[0..48].copy_from_slice(&bytes[16..64]);
    buf[48..96].copy_from_slice(&bytes[80..128]);
    G1Affine::deserialize_uncompressed(&buf[..]).ok()
}

/// Helper to deserialize a G2 point from EVM Big-Endian format (256 bytes).
/// EVM G2 point: X0, X1, Y0, Y1 (each 64 bytes, 16 zero padding + 48 bytes limb).
/// Validates padding, coordinates, curve equation, and prime-order subgroup.
pub fn from_evm_g2(bytes: &[u8; 256]) -> Option<G2Affine> {
    use ark_ec::AffineRepr;
    use ark_serialize::CanonicalDeserialize;
    // Point at infinity in EVM is all zeros
    if bytes.iter().all(|&b| b == 0) {
        return Some(G2Affine::zero());
    }
    for i in 0..4 {
        if bytes[i * 64..i * 64 + 16].iter().any(|&b| b != 0) {
            return None;
        }
    }
    // Reverse the limb mapping used in to_evm_g2:
    // EVM: X0(c0), X1(c1), Y0(c0), Y1(c1)
    // Arkworks: X.c1 || X.c0 || Y.c1 || Y.c0
    let mut buf = [0u8; 192];
    buf[0..48].copy_from_slice(&bytes[80..128]); // X.c1 (from EVM limb 1: 64..128)
    buf[48..96].copy_from_slice(&bytes[16..64]); // X.c0 (from EVM limb 0: 0..64)
    buf[96..144].copy_from_slice(&bytes[208..256]); // Y.c1 (from EVM limb 3: 192..256)
    buf[144..192].copy_from_slice(&bytes[144..192]); // Y.c0 (from EVM limb 2: 128..192)
    G2Affine::deserialize_uncompressed(&buf[..]).ok()
}

/// Helper to deserialize an EVM Groth16 proof into an Arkworks Proof<Bls12_381>.
/// In EIP-2537, the prover supplies -A as proof_a_neg.
/// This function negates -A back to A, and reconstructs B and C.
pub fn from_evm_proof(
    proof_a_neg: &[u8; 128],
    proof_b: &[u8; 256],
    proof_c: &[u8; 128],
) -> Option<ark_groth16::Proof<ark_bls12_381::Bls12_381>> {
    let a_neg = from_evm_g1(proof_a_neg)?;
    let a = -a_neg;
    let b = from_evm_g2(proof_b)?;
    let c = from_evm_g1(proof_c)?;
    Some(ark_groth16::Proof { a, b, c })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ark_bls12_381::{G1Affine, G2Affine};
    use ark_ec::AffineRepr;

    #[test]
    fn test_evm_g1_roundtrip() {
        let generator = G1Affine::generator();
        let evm_bytes = to_evm_g1(&generator);
        assert_eq!(evm_bytes.len(), 128);

        let mut fixed = [0u8; 128];
        fixed.copy_from_slice(&evm_bytes);

        let decoded = from_evm_g1(&fixed).expect("Should deserialize G1 generator");
        assert_eq!(decoded, generator);

        // Test identity / zero
        let zero = G1Affine::zero();
        let zero_bytes = [0u8; 128];
        let decoded_zero = from_evm_g1(&zero_bytes).expect("Should deserialize zero G1");
        assert_eq!(decoded_zero, zero);

        // Test non-zero padding rejected
        let mut invalid_padding = fixed;
        invalid_padding[0] = 1;
        assert!(from_evm_g1(&invalid_padding).is_none());
    }

    #[test]
    fn test_evm_g2_roundtrip() {
        let generator = G2Affine::generator();
        let evm_bytes = to_evm_g2(&generator);
        assert_eq!(evm_bytes.len(), 256);

        let mut fixed = [0u8; 256];
        fixed.copy_from_slice(&evm_bytes);

        let decoded = from_evm_g2(&fixed).expect("Should deserialize G2 generator");
        assert_eq!(decoded, generator);

        // Test identity / zero
        let zero = G2Affine::zero();
        let zero_bytes = [0u8; 256];
        let decoded_zero = from_evm_g2(&zero_bytes).expect("Should deserialize zero G2");
        assert_eq!(decoded_zero, zero);

        // Test non-zero padding rejected
        let mut invalid_padding = fixed;
        invalid_padding[0] = 1;
        assert!(from_evm_g2(&invalid_padding).is_none());
    }

    #[test]
    fn test_evm_proof_deserialization() {
        let a = G1Affine::generator();
        let a_neg = -a;
        let b = G2Affine::generator();
        let c = G1Affine::generator();

        let mut a_neg_bytes = [0u8; 128];
        a_neg_bytes.copy_from_slice(&to_evm_g1(&a_neg));

        let mut b_bytes = [0u8; 256];
        b_bytes.copy_from_slice(&to_evm_g2(&b));

        let mut c_bytes = [0u8; 128];
        c_bytes.copy_from_slice(&to_evm_g1(&c));

        let proof = from_evm_proof(&a_neg_bytes, &b_bytes, &c_bytes).expect("Valid proof points");
        assert_eq!(proof.a, a, "-(-A) must equal A");
        assert_eq!(proof.b, b);
        assert_eq!(proof.c, c);
    }
}
