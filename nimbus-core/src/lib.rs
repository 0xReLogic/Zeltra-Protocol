pub use ark_bls12_381::{Bls12_381, Fr, G1Projective, G2Projective};
pub use ark_ec::pairing::Pairing;
pub use ark_ec::Group;
pub use ark_ff::{Field, PrimeField, UniformRand};
use ark_serialize::{CanonicalDeserialize, CanonicalSerialize};
use rand::Rng;
use sha2::{Digest, Sha256};

pub fn serialize_to_bytes<T: CanonicalSerialize>(val: &T) -> Vec<u8> {
    let mut buf = vec![];
    val.serialize_compressed(&mut buf).unwrap();
    buf
}

pub fn deserialize_from_bytes<T: CanonicalDeserialize>(bytes: &[u8]) -> Option<T> {
    T::deserialize_compressed(bytes).ok()
}


#[derive(Clone, Debug, CanonicalSerialize, CanonicalDeserialize)]
pub struct IssuerSecretKey(pub Fr);

#[derive(Clone, Debug, CanonicalSerialize, CanonicalDeserialize)]
pub struct IssuerPublicKey(pub G2Projective);

#[derive(Clone, Debug, CanonicalSerialize, CanonicalDeserialize)]
pub struct BlindedMessage(pub G1Projective);

#[derive(Clone, Debug, CanonicalSerialize, CanonicalDeserialize)]
pub struct BlindingFactor(pub Fr);

#[derive(Clone, Debug, CanonicalSerialize, CanonicalDeserialize)]
pub struct MaskedBlindSignature(pub G1Projective);

#[derive(Clone, Debug, CanonicalSerialize, CanonicalDeserialize)]
pub struct MaskingKey(pub Fr);

#[derive(Clone, Debug, CanonicalSerialize, CanonicalDeserialize)]
pub struct MaskingKeyCommitment(pub G2Projective);

#[derive(Clone, Debug, CanonicalSerialize, CanonicalDeserialize)]
pub struct UnmaskedSignature(pub G1Projective);


/// Hash a message to G1Projective. For this prototype, we hash to a scalar and multiply by the generator.
pub fn hash_to_g1(message: &[u8]) -> G1Projective {
    let mut hasher = Sha256::new();
    hasher.update(message);
    let result = hasher.finalize();
    let scalar = Fr::from_le_bytes_mod_order(&result);
    G1Projective::generator() * scalar
}

impl IssuerSecretKey {
    /// Generate a random issuer secret key.
    pub fn generate<R: Rng>(rng: &mut R) -> Self {
        Self(Fr::rand(rng))
    }

    /// Derives the public key from the secret key: pk = sk * G2
    pub fn public_key(&self) -> IssuerPublicKey {
        IssuerPublicKey(G2Projective::generator() * self.0)
    }
}

/// Client: blinds a message (ephemeral public key) using a random blinding factor.
/// X = r * H(m)
pub fn client_blind<R: Rng>(message: &[u8], rng: &mut R) -> (BlindedMessage, BlindingFactor) {
    let r = Fr::rand(rng);
    let hm = hash_to_g1(message);
    let x = hm * r;
    (BlindedMessage(x), BlindingFactor(r))
}

/// Issuer: signs a blinded message using the secret key and a temporary masking key k.
/// com_k = k * pk_iss
/// masked_sig = (k * sk_iss) * X
pub fn issuer_sign_blinded<R: Rng>(
    sk_iss: &IssuerSecretKey,
    x: &BlindedMessage,
    rng: &mut R,
) -> (MaskedBlindSignature, MaskingKey, MaskingKeyCommitment) {
    let k = Fr::rand(rng);
    let pk_iss = sk_iss.public_key();
    let com_k = pk_iss.0 * k;
    let scalar = k * sk_iss.0;
    let masked_sig = x.0 * scalar;
    (
        MaskedBlindSignature(masked_sig),
        MaskingKey(k),
        MaskingKeyCommitment(com_k),
    )
}

/// Client: verifies the masked signature from the issuer using bilinear pairings before proceeding to reveal the masking key.
/// e(masked_sig, G2) == e(X, com_k)
pub fn client_verify_masked(
    x: &BlindedMessage,
    com_k: &MaskingKeyCommitment,
    masked_sig: &MaskedBlindSignature,
) -> bool {
    let lhs = Bls12_381::pairing(masked_sig.0, G2Projective::generator());
    let rhs = Bls12_381::pairing(x.0, com_k.0);
    lhs == rhs
}

/// Client: unmasks the signature once the masking key k is revealed on-chain.
/// alpha = (r * k)^-1 * masked_sig
pub fn client_unmask(
    masked_sig: &MaskedBlindSignature,
    r: &BlindingFactor,
    k: &MaskingKey,
) -> Option<UnmaskedSignature> {
    let r_k = r.0 * k.0;
    r_k.inverse().map(|inv| UnmaskedSignature(masked_sig.0 * inv))
}

/// Verifier: verifies the unmasked signature.
/// e(alpha, G2) == e(H(m), pk_iss)
pub fn verify_unmasked(
    message: &[u8],
    alpha: &UnmaskedSignature,
    pk_iss: &IssuerPublicKey,
) -> bool {
    let lhs = Bls12_381::pairing(alpha.0, G2Projective::generator());
    let hm = hash_to_g1(message);
    let rhs = Bls12_381::pairing(hm, pk_iss.0);
    lhs == rhs
}

/// Represents an offline spend proof containing the challenge x and response y
#[derive(Clone, Debug, CanonicalSerialize, CanonicalDeserialize)]
pub struct OfflineSpendProof {
    pub x: Fr,
    pub y: Fr,
}

/// Client: Generates the offline response y = a * x + I (mod p)
pub fn generate_offline_response(a: Fr, x: Fr, identity: Fr) -> Fr {
    (a * x) + identity
}

/// Smart Contract: Reconstructs the identity I from two different offline spend proofs
pub fn reconstruct_identity(proof1: &OfflineSpendProof, proof2: &OfflineSpendProof) -> Option<Fr> {
    if proof1.x == proof2.x {
        return None; // Cannot reconstruct from the same challenge
    }
    
    // a = (y2 - y1) / (x2 - x1)
    let y_diff = proof2.y - proof1.y;
    let x_diff = proof2.x - proof1.x;
    
    let x_diff_inv = x_diff.inverse()?;
    let a = y_diff * x_diff_inv;
    
    // I = y1 - a * x1
    let identity = proof1.y - (a * proof1.x);
    Some(identity)
}

use ark_ec::CurveGroup;
use ark_bls12_381::{G1Affine, G2Affine};

/// Helper to serialize a G1 point to EVM Big-Endian format (128 bytes)
pub fn to_evm_g1(point: &G1Affine) -> Vec<u8> {
    let mut buf = vec![];
    point.serialize_uncompressed(&mut buf).unwrap();
    let mut evm_buf = vec![0u8; 128];
    for i in 0..2 {
        for j in 0..48 {
            evm_buf[i * 64 + 16 + j] = buf[i * 48 + (47 - j)];
        }
    }
    evm_buf
}

/// Helper to serialize a G2 point to EVM Big-Endian format (256 bytes)
pub fn to_evm_g2(point: &G2Affine) -> Vec<u8> {
    let mut buf = vec![];
    point.serialize_uncompressed(&mut buf).unwrap();
    let mut evm_buf = vec![0u8; 256];
    let src_indices = [1, 0, 3, 2];
    for i in 0..4 {
        let src_idx = src_indices[i];
        for j in 0..48 {
            evm_buf[i * 64 + 16 + j] = buf[src_idx * 48 + (47 - j)];
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

#[derive(Clone, Debug, CanonicalSerialize, CanonicalDeserialize)]
pub struct PartialBlindSignature(pub G1Projective);

/// Shamir Secret Sharing: Splits the issuer secret key into n shares with threshold t.
pub fn split_secret_key<R: Rng>(
    sk: &IssuerSecretKey,
    t: usize,
    n: usize,
    rng: &mut R,
) -> Vec<(usize, Fr)> {
    assert!(t <= n, "Threshold cannot be greater than n");
    assert!(t > 0, "Threshold must be greater than 0");
    
    // Generate coefficients a_1 to a_{t-1}
    let mut coefficients = vec![sk.0];
    for _ in 1..t {
        coefficients.push(Fr::rand(rng));
    }
    
    let mut shares = vec![];
    for i in 1..=n {
        let x = Fr::from(i as u64);
        let mut y = Fr::from(0);
        let mut x_pow = Fr::from(1);
        for &coeff in &coefficients {
            y += coeff * x_pow;
            x_pow *= x;
        }
        shares.push((i, y));
    }
    shares
}

/// Evaluates a Lagrange coefficient at x=0 for validator i in a subset of validators S.
pub fn compute_lagrange_coefficient(i: usize, s: &[usize]) -> Fr {
    let mut num = Fr::from(1);
    let mut den = Fr::from(1);
    let i_fr = Fr::from(i as u64);
    for &j in s {
        if j == i {
            continue;
        }
        let j_fr = Fr::from(j as u64);
        num *= j_fr;
        den *= j_fr - i_fr;
    }
    let den_inv = den.inverse().expect("Lagrange denominator inverse failed");
    num * den_inv
}

/// Validator: signs a blinded message using their secret key share and a temporary masking key k.
pub fn sign_share(
    share_sk: &Fr,
    x: &BlindedMessage,
    k: &Fr,
) -> PartialBlindSignature {
    let scalar = (*k) * (*share_sk);
    PartialBlindSignature(x.0 * scalar)
}

/// Client: aggregates partial blind signatures from a subset of validators using Lagrange interpolation.
pub fn aggregate_shares(
    partial_sigs: &[(usize, PartialBlindSignature)],
) -> MaskedBlindSignature {
    let indices: Vec<usize> = partial_sigs.iter().map(|(i, _)| *i).collect();
    let mut sum = G1Projective::generator() * Fr::from(0u64);
    for &(i, ref sig) in partial_sigs {
        let l_i = compute_lagrange_coefficient(i, &indices);
        sum += sig.0 * l_i;
    }
    MaskedBlindSignature(sum)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::thread_rng;

    #[test]
    fn test_bat_protocol_flow() {
        let mut rng = thread_rng();

        // 1. Setup Issuer
        let sk_iss = IssuerSecretKey::generate(&mut rng);
        let pk_iss = sk_iss.public_key();

        // 2. Client Blinds Message
        let message = b"ephemeral_public_key_123456";
        let (x, r) = client_blind(message, &mut rng);

        // 3. Issuer Signs Blinded Message with Masking Key
        let (masked_sig, k, com_k) = issuer_sign_blinded(&sk_iss, &x, &mut rng);

        // 4. Client Verifies Masked Signature off-chain (No trust needed yet)
        let verify_masked = client_verify_masked(&x, &com_k, &masked_sig);
        assert!(verify_masked, "Masked signature verification failed!");

        // 5. Client Unmasks Signature (after k is revealed on-chain)
        let alpha = client_unmask(&masked_sig, &r, &k).expect("Failed to invert blinding key");

        // 6. Verifier checks the final unmasked signature
        let verify_final = verify_unmasked(message, &alpha, &pk_iss);
        assert!(verify_final, "Final unmasked signature verification failed!");
    }

    #[test]
    fn test_offline_double_spend_detection() {
        let mut rng = thread_rng();

        // 1. Setup identity (I) and random slope (a)
        let identity = Fr::rand(&mut rng);
        let a = Fr::rand(&mut rng);

        // 2. First spend (Merchant A challenges with x1)
        let x1 = Fr::rand(&mut rng);
        let y1 = generate_offline_response(a, x1, identity);
        let proof1 = OfflineSpendProof { x: x1, y: y1 };

        // 3. Second spend (Merchant B challenges with x2)
        // Make sure x2 != x1
        let mut x2 = Fr::rand(&mut rng);
        while x2 == x1 {
            x2 = Fr::rand(&mut rng);
        }
        let y2 = generate_offline_response(a, x2, identity);
        let proof2 = OfflineSpendProof { x: x2, y: y2 };

        // 4. Smart contract reconstructs the identity
        let reconstructed_identity = reconstruct_identity(&proof1, &proof2)
            .expect("Failed to reconstruct identity");
        
        assert_eq!(reconstructed_identity, identity, "Reconstructed identity does not match original!");
    }

    #[test]
    fn test_threshold_bdhke_flow() {
        let mut rng = thread_rng();

        // 1. Setup Issuer and split the key into 5 shares with threshold 3
        let sk_iss = IssuerSecretKey::generate(&mut rng);
        let pk_iss = sk_iss.public_key();
        
        let t = 3;
        let n = 5;
        let shares = split_secret_key(&sk_iss, t, n, &mut rng);
        assert_eq!(shares.len(), 5);

        // 2. Client Blinds Message
        let message = b"secret_message_for_federation";
        let (x, r) = client_blind(message, &mut rng);

        // 3. Leader generates random masking key k and its commitment com_k
        let k = Fr::rand(&mut rng);
        let com_k = MaskingKeyCommitment(pk_iss.0 * k);

        // 4. Three Guardians (e.g. 1, 3, and 5) sign the blinded message
        let sig_share1 = sign_share(&shares[0].1, &x, &k); // share 1 (index 1)
        let sig_share3 = sign_share(&shares[2].1, &x, &k); // share 3 (index 3)
        let sig_share5 = sign_share(&shares[4].1, &x, &k); // share 5 (index 5)

        // 5. Client aggregates partial signatures
        let gathered = vec![
            (1, sig_share1),
            (3, sig_share3),
            (5, sig_share5),
        ];
        let aggregated_masked_sig = aggregate_shares(&gathered);

        // 6. Client verifies aggregated masked signature against com_k
        let verify_masked = client_verify_masked(&x, &com_k, &aggregated_masked_sig);
        assert!(verify_masked, "Aggregated masked signature verification failed!");

        // 7. Client unmasks signature once k is revealed
        let masking_key = MaskingKey(k);
        let alpha = client_unmask(&aggregated_masked_sig, &r, &masking_key)
            .expect("Failed to unmask aggregated signature");

        // 8. Verifier checks the final unmasked signature against aggregate public key pk_iss
        let verify_final = verify_unmasked(message, &alpha, &pk_iss);
        assert!(verify_final, "Final unmasked signature verification failed!");
    }
}
