use ark_bls12_381::{G1Affine, G2Affine};
use ark_ec::AffineRepr;
use ark_serialize::{CanonicalDeserialize, CanonicalSerialize};

/// BLS12-381 base field modulus q in Big-Endian bytes (48 bytes):
/// q = 0x1a0111ea397fe69a4b1ba7b6434bacd764774b84f38512bf6730d2a0f6b0f6241eabfffeb153ffffb9feffffffffaaab
pub const BLS12_381_Q_BE: [u8; 48] = [
    0x1a, 0x01, 0x11, 0xea, 0x39, 0x7f, 0xe6, 0x9a, 0x4b, 0x1b, 0xa7, 0xb6, 0x43, 0x4b, 0xac, 0xd7,
    0x64, 0x77, 0x4b, 0x84, 0xf3, 0x85, 0x12, 0xbf, 0x67, 0x30, 0xd2, 0xa0, 0xf6, 0xb0, 0xf6, 0x24,
    0x1e, 0xab, 0xff, 0xfe, 0xb1, 0x53, 0xff, 0xff, 0xb9, 0xfe, 0xff, 0xff, 0xff, 0xff, 0xaa, 0xab,
];

/// Strict error enumeration for curve point sanitation under DEC-052.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CurveSanitationError {
    /// Coordinate padding bytes non-zero or coordinate value >= field modulus q.
    NonCanonicalFieldElement,
    /// Point at infinity (all zeros or (0, 0) coordinates) rejected in Groth16 proof inputs.
    PointAtInfinityNotAllowed,
    /// Point does not satisfy the curve equation: y^2 != x^3 + b.
    CurvePointNotOnCurve,
    /// Point is not in the prime-order subgroup: [r]P != O.
    SubgroupCheckFailed,
}

impl core::fmt::Display for CurveSanitationError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::NonCanonicalFieldElement => write!(f, "NonCanonicalFieldElement"),
            Self::PointAtInfinityNotAllowed => write!(f, "PointAtInfinityNotAllowed"),
            Self::CurvePointNotOnCurve => write!(f, "CurvePointNotOnCurve"),
            Self::SubgroupCheckFailed => write!(f, "SubgroupCheckFailed"),
        }
    }
}

impl std::error::Error for CurveSanitationError {}

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
/// Helper to serialize a scalar Fr to EVM Big-Endian format (32 bytes).
pub fn to_evm_scalar(scalar: &ark_bls12_381::Fr) -> [u8; 32] {
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
    // Point at infinity in EVM is all zeros
    if bytes.iter().all(|&b| b == 0) {
        return Some(G1Affine::zero());
    }
    sanitize_evm_g1(bytes).ok()
}

/// Helper to deserialize a G2 point from EVM Big-Endian format (256 bytes).
/// EVM G2 point: X0, X1, Y0, Y1 (each 64 bytes, 16 zero padding + 48 bytes limb).
/// Validates padding, coordinates, curve equation, and prime-order subgroup.
pub fn from_evm_g2(bytes: &[u8; 256]) -> Option<G2Affine> {
    // Point at infinity in EVM is all zeros
    if bytes.iter().all(|&b| b == 0) {
        return Some(G2Affine::zero());
    }
    sanitize_evm_g2(bytes).ok()
}

/// Strict G1 point sanitization under DEC-052.
/// Enforces all 4 verification gates:
/// - Gate 2: Non-infinity rejection (all-zeros or (0, 0) coordinates).
/// - Gate 4: Canonical field coordinates (padding == 0, x, y < q).
/// - Gate 1: Curve equation validation (y^2 == x^3 + 4 in Fq).
/// - Gate 3: Prime-order subgroup membership ([r]P == O via fast endomorphism).
pub fn sanitize_evm_g1(bytes: &[u8; 128]) -> Result<G1Affine, CurveSanitationError> {
    // GATE 2: Reject point at infinity (EVM all-zeros representation or (0, 0) coordinates)
    if bytes.iter().all(|&b| b == 0) {
        return Err(CurveSanitationError::PointAtInfinityNotAllowed);
    }
    let x_bytes: &[u8; 48] = bytes[16..64].try_into().unwrap();
    let y_bytes: &[u8; 48] = bytes[80..128].try_into().unwrap();
    if x_bytes.iter().all(|&b| b == 0) && y_bytes.iter().all(|&b| b == 0) {
        return Err(CurveSanitationError::PointAtInfinityNotAllowed);
    }

    // GATE 4: Canonical coordinate bounds:
    // Leading 16 bytes of each 64-byte block must be strictly zero.
    if bytes[0..16].iter().any(|&b| b != 0) || bytes[64..80].iter().any(|&b| b != 0) {
        return Err(CurveSanitationError::NonCanonicalFieldElement);
    }
    // Field elements must be strictly < q (lexicographical comparison on big-endian 48 bytes)
    if x_bytes >= &BLS12_381_Q_BE || y_bytes >= &BLS12_381_Q_BE {
        return Err(CurveSanitationError::NonCanonicalFieldElement);
    }

    let mut buf = [0u8; 96];
    buf[0..48].copy_from_slice(x_bytes);
    buf[48..96].copy_from_slice(y_bytes);

    let point = G1Affine::deserialize_uncompressed_unchecked(&buf[..])
        .map_err(|_| CurveSanitationError::NonCanonicalFieldElement)?;

    // GATE 1: On-curve check
    if !point.is_on_curve() {
        return Err(CurveSanitationError::CurvePointNotOnCurve);
    }

    // GATE 3: Prime-order subgroup check: [r]P = O
    if !point.is_in_correct_subgroup_assuming_on_curve() {
        return Err(CurveSanitationError::SubgroupCheckFailed);
    }

    Ok(point)
}

/// Strict G2 point sanitization under DEC-052.
/// Enforces all 4 verification gates:
/// - Gate 2: Non-infinity rejection (all-zeros representation).
/// - Gate 4: Canonical field coordinates (padding == 0, x0, x1, y0, y1 < q).
/// - Gate 1: Curve equation validation (y^2 == x^3 + 4(1 + i) in Fq2).
/// - Gate 3: Prime-order subgroup membership ([r]Q == O via Frobenius endomorphism).
pub fn sanitize_evm_g2(bytes: &[u8; 256]) -> Result<G2Affine, CurveSanitationError> {
    // GATE 2: Reject point at infinity (EVM all-zeros representation)
    if bytes.iter().all(|&b| b == 0) {
        return Err(CurveSanitationError::PointAtInfinityNotAllowed);
    }

    // GATE 4: Canonical coordinate bounds:
    // Leading 16 bytes of each 64-byte block must be strictly zero.
    for i in 0..4 {
        if bytes[i * 64..i * 64 + 16].iter().any(|&b| b != 0) {
            return Err(CurveSanitationError::NonCanonicalFieldElement);
        }
    }
    let x0_bytes: &[u8; 48] = bytes[16..64].try_into().unwrap();
    let x1_bytes: &[u8; 48] = bytes[80..128].try_into().unwrap();
    let y0_bytes: &[u8; 48] = bytes[144..192].try_into().unwrap();
    let y1_bytes: &[u8; 48] = bytes[208..256].try_into().unwrap();

    if x0_bytes.iter().all(|&b| b == 0)
        && x1_bytes.iter().all(|&b| b == 0)
        && y0_bytes.iter().all(|&b| b == 0)
        && y1_bytes.iter().all(|&b| b == 0)
    {
        return Err(CurveSanitationError::PointAtInfinityNotAllowed);
    }

    if x0_bytes >= &BLS12_381_Q_BE
        || x1_bytes >= &BLS12_381_Q_BE
        || y0_bytes >= &BLS12_381_Q_BE
        || y1_bytes >= &BLS12_381_Q_BE
    {
        return Err(CurveSanitationError::NonCanonicalFieldElement);
    }

    // Reverse limb mapping used in to_evm_g2:
    // EVM: X0(c0), X1(c1), Y0(c0), Y1(c1)
    // Arkworks: X.c1 || X.c0 || Y.c1 || Y.c0
    let mut buf = [0u8; 192];
    buf[0..48].copy_from_slice(x1_bytes);
    buf[48..96].copy_from_slice(x0_bytes);
    buf[96..144].copy_from_slice(y1_bytes);
    buf[144..192].copy_from_slice(y0_bytes);

    let point = G2Affine::deserialize_uncompressed_unchecked(&buf[..])
        .map_err(|_| CurveSanitationError::NonCanonicalFieldElement)?;

    // GATE 1: On-curve check
    if !point.is_on_curve() {
        return Err(CurveSanitationError::CurvePointNotOnCurve);
    }

    // GATE 3: Prime-order subgroup check via Frobenius endomorphism
    if !point.is_in_correct_subgroup_assuming_on_curve() {
        return Err(CurveSanitationError::SubgroupCheckFailed);
    }

    Ok(point)
}

/// Helper to deserialize a sanitized G1 point from EVM format (DEC-052).
pub fn from_evm_g1_sanitized(bytes: &[u8; 128]) -> Option<G1Affine> {
    sanitize_evm_g1(bytes).ok()
}

/// Helper to deserialize a sanitized G2 point from EVM format (DEC-052).
pub fn from_evm_g2_sanitized(bytes: &[u8; 256]) -> Option<G2Affine> {
    sanitize_evm_g2(bytes).ok()
}

/// Deserializes and strictly sanitizes an EVM Groth16 proof under DEC-052.
pub fn from_evm_proof_sanitized(
    proof_a_neg: &[u8; 128],
    proof_b: &[u8; 256],
    proof_c: &[u8; 128],
) -> Result<ark_groth16::Proof<ark_bls12_381::Bls12_381>, CurveSanitationError> {
    let a_neg = sanitize_evm_g1(proof_a_neg)?;
    let b = sanitize_evm_g2(proof_b)?;
    let c = sanitize_evm_g1(proof_c)?;
    let a = -a_neg;
    Ok(ark_groth16::Proof { a, b, c })
}

/// Helper to deserialize an EVM Groth16 proof into an Arkworks Proof<Bls12_381>.
/// In EIP-2537, the prover supplies -A as proof_a_neg.
/// Under DEC-052, this strictly sanitizes points A, B, and C against all 4 curve gates.
pub fn from_evm_proof(
    proof_a_neg: &[u8; 128],
    proof_b: &[u8; 256],
    proof_c: &[u8; 128],
) -> Option<ark_groth16::Proof<ark_bls12_381::Bls12_381>> {
    from_evm_proof_sanitized(proof_a_neg, proof_b, proof_c).ok()
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

    #[test]
    fn test_sanitize_evm_g1_gates() {
        let gen = G1Affine::generator();
        let gen_bytes: [u8; 128] = to_evm_g1(&gen).try_into().unwrap();

        // 1. Happy path: valid G1 point passes all 4 gates
        let parsed = sanitize_evm_g1(&gen_bytes).expect("Generator must pass all gates");
        assert_eq!(parsed, gen);

        // 2. Gate 2: Point at infinity rejected
        let zero_bytes = [0u8; 128];
        assert_eq!(
            sanitize_evm_g1(&zero_bytes),
            Err(CurveSanitationError::PointAtInfinityNotAllowed)
        );

        // 3. Gate 4: Non-canonical padding rejected
        let mut tampered_pad = gen_bytes;
        tampered_pad[0] = 0x01;
        assert_eq!(
            sanitize_evm_g1(&tampered_pad),
            Err(CurveSanitationError::NonCanonicalFieldElement)
        );
        let mut tampered_pad_y = gen_bytes;
        tampered_pad_y[64] = 0xff;
        assert_eq!(
            sanitize_evm_g1(&tampered_pad_y),
            Err(CurveSanitationError::NonCanonicalFieldElement)
        );

        // 4. Gate 4: Coordinate >= q rejected
        let mut overflow_x = gen_bytes;
        overflow_x[16..64].copy_from_slice(&BLS12_381_Q_BE);
        assert_eq!(
            sanitize_evm_g1(&overflow_x),
            Err(CurveSanitationError::NonCanonicalFieldElement)
        );

        // 5. Gate 1: Off-curve point rejected
        let mut off_curve = gen_bytes;
        off_curve[127] ^= 0x01; // flip last byte of Y
        assert_eq!(
            sanitize_evm_g1(&off_curve),
            Err(CurveSanitationError::CurvePointNotOnCurve)
        );

        // 6. Gate 3: Small-order point (order 3) on-curve rejected for subgroup membership
        // Point (0, 2): 2^2 = 0^3 + 4 = 4 on BLS12-381 G1 curve.
        // Has exact order 3 (divides cofactor h1 = (x-1)^2 / 3).
        let mut small_order_3 = [0u8; 128];
        small_order_3[63] = 0x00; // x = 0
        small_order_3[127] = 0x02; // y = 2
        assert_eq!(
            sanitize_evm_g1(&small_order_3),
            Err(CurveSanitationError::SubgroupCheckFailed),
            "(0, 2) is an order-3 point on the curve, must fail prime-order subgroup check"
        );
    }

    #[test]
    fn test_sanitize_evm_g2_gates() {
        let gen = G2Affine::generator();
        let gen_bytes: [u8; 256] = to_evm_g2(&gen).try_into().unwrap();

        // 1. Happy path: valid G2 generator passes all gates
        let parsed = sanitize_evm_g2(&gen_bytes).expect("Generator must pass all gates");
        assert_eq!(parsed, gen);

        // 2. Gate 2: Point at infinity rejected
        let zero_bytes = [0u8; 256];
        assert_eq!(
            sanitize_evm_g2(&zero_bytes),
            Err(CurveSanitationError::PointAtInfinityNotAllowed)
        );

        // 3. Gate 4: Non-canonical padding rejected
        let mut tampered_pad = gen_bytes;
        tampered_pad[0] = 0x01;
        assert_eq!(
            sanitize_evm_g2(&tampered_pad),
            Err(CurveSanitationError::NonCanonicalFieldElement)
        );
        let mut tampered_pad_limb3 = gen_bytes;
        tampered_pad_limb3[192] = 0x01;
        assert_eq!(
            sanitize_evm_g2(&tampered_pad_limb3),
            Err(CurveSanitationError::NonCanonicalFieldElement)
        );

        // 4. Gate 4: Coordinate >= q rejected
        let mut overflow_limb = gen_bytes;
        overflow_limb[80..128].copy_from_slice(&BLS12_381_Q_BE);
        assert_eq!(
            sanitize_evm_g2(&overflow_limb),
            Err(CurveSanitationError::NonCanonicalFieldElement)
        );

        // 5. Gate 1: Off-curve point rejected
        let mut off_curve = gen_bytes;
        off_curve[255] ^= 0x01;
        assert_eq!(
            sanitize_evm_g2(&off_curve),
            Err(CurveSanitationError::CurvePointNotOnCurve)
        );
    }

    #[test]
    fn test_proof_sanitation_adversarial() {
        let a = G1Affine::generator();
        let a_neg = -a;
        let b = G2Affine::generator();
        let c = G1Affine::generator();

        let a_neg_bytes: [u8; 128] = to_evm_g1(&a_neg).try_into().unwrap();
        let b_bytes: [u8; 256] = to_evm_g2(&b).try_into().unwrap();
        let c_bytes: [u8; 128] = to_evm_g1(&c).try_into().unwrap();

        // Valid proof deserializes cleanly
        let proof = from_evm_proof_sanitized(&a_neg_bytes, &b_bytes, &c_bytes).unwrap();
        assert_eq!(proof.a, a);

        // Adversarial test: A is point at infinity
        let zero_g1 = [0u8; 128];
        assert_eq!(
            from_evm_proof_sanitized(&zero_g1, &b_bytes, &c_bytes),
            Err(CurveSanitationError::PointAtInfinityNotAllowed)
        );

        // Adversarial test: B is point at infinity
        let zero_g2 = [0u8; 256];
        assert_eq!(
            from_evm_proof_sanitized(&a_neg_bytes, &zero_g2, &c_bytes),
            Err(CurveSanitationError::PointAtInfinityNotAllowed)
        );

        // Adversarial test: C is order-3 subgroup point
        let mut small_order_c = [0u8; 128];
        small_order_c[127] = 0x02; // (0, 2)
        assert_eq!(
            from_evm_proof_sanitized(&a_neg_bytes, &b_bytes, &small_order_c),
            Err(CurveSanitationError::SubgroupCheckFailed)
        );
    }
}
