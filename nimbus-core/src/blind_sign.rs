use crate::crypto::hash_to_g1;
use crate::types::{
    BlindedMessage, BlindingFactor, IssuerPublicKey, IssuerSecretKey, MaskedBlindSignature,
    MaskingKey, MaskingKeyCommitment, UnmaskedSignature,
};
use ark_bls12_381::{Bls12_381, Fr, G2Projective};
use ark_ec::pairing::Pairing;
use ark_ec::PrimeGroup;
use ark_ff::{Field, UniformRand};
use ark_std::Zero;
use rand::Rng;

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

/// Verifies that a revealed masking key k matches its commitment: com_k == k * pk_iss on G2.
pub fn verify_masking_key_commitment(
    pk_iss: &IssuerPublicKey,
    k: &MaskingKey,
    com_k: &MaskingKeyCommitment,
) -> bool {
    let expected_com_k = pk_iss.0 * k.0;
    expected_com_k == com_k.0
}

/// Client: unmasks the signature once the masking key k is revealed on-chain.
/// alpha = (r * k)^-1 * masked_sig
pub fn client_unmask(
    masked_sig: &MaskedBlindSignature,
    r: &BlindingFactor,
    k: &MaskingKey,
) -> Option<UnmaskedSignature> {
    if r.0.is_zero() || k.0.is_zero() {
        return None;
    }
    let r_k = r.0 * k.0;
    r_k.inverse()
        .map(|inv| UnmaskedSignature(masked_sig.0 * inv))
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

#[cfg(test)]
mod tests {
    use super::*;
    use ark_ff::UniformRand;

    #[test]
    fn test_verify_masking_key_commitment_positive() {
        let mut rng = rand::thread_rng();
        let sk = Fr::rand(&mut rng);
        let pk_iss = IssuerPublicKey(G2Projective::generator() * sk);

        let k_scalar = Fr::rand(&mut rng);
        let k = MaskingKey(k_scalar);
        let com_k = MaskingKeyCommitment(pk_iss.0 * k_scalar);

        assert!(verify_masking_key_commitment(&pk_iss, &k, &com_k));
    }

    #[test]
    fn test_verify_masking_key_commitment_negative_tampered_k() {
        let mut rng = rand::thread_rng();
        let sk = Fr::rand(&mut rng);
        let pk_iss = IssuerPublicKey(G2Projective::generator() * sk);

        let k_scalar = Fr::rand(&mut rng);
        let com_k = MaskingKeyCommitment(pk_iss.0 * k_scalar);

        let tampered_k = MaskingKey(k_scalar + Fr::from(1u64));
        assert!(!verify_masking_key_commitment(&pk_iss, &tampered_k, &com_k));
    }

    #[test]
    fn test_verify_masking_key_commitment_negative_tampered_com_k() {
        let mut rng = rand::thread_rng();
        let sk = Fr::rand(&mut rng);
        let pk_iss = IssuerPublicKey(G2Projective::generator() * sk);

        let k_scalar = Fr::rand(&mut rng);
        let k = MaskingKey(k_scalar);
        let com_k = MaskingKeyCommitment(pk_iss.0 * k_scalar);

        let tampered_com_k = MaskingKeyCommitment(com_k.0 + G2Projective::generator());
        assert!(!verify_masking_key_commitment(&pk_iss, &k, &tampered_com_k));
    }

    #[test]
    fn test_verify_masking_key_commitment_negative_wrong_issuer() {
        let mut rng = rand::thread_rng();
        let sk1 = Fr::rand(&mut rng);
        let pk_iss1 = IssuerPublicKey(G2Projective::generator() * sk1);
        let sk2 = Fr::rand(&mut rng);
        let pk_iss2 = IssuerPublicKey(G2Projective::generator() * sk2);

        let k_scalar = Fr::rand(&mut rng);
        let k = MaskingKey(k_scalar);
        let com_k = MaskingKeyCommitment(pk_iss1.0 * k_scalar);

        assert!(!verify_masking_key_commitment(&pk_iss2, &k, &com_k));
    }
}
