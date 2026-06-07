use crate::types::{
    BlindedMessage, IssuerPublicKey, IssuerSecretKey, MaskedBlindSignature,
    PartialBlindSignature,
};
use ark_bls12_381::{Bls12_381, Fr, G1Projective, G2Projective};
use ark_ec::pairing::Pairing;
use ark_ec::PrimeGroup;
use ark_ff::{Field, UniformRand};
use rand::Rng;
use ark_std::Zero;

/// Shamir Secret Sharing: Splits the issuer secret key into n shares with threshold t.
pub fn split_secret_key<R: Rng>(
    sk: &IssuerSecretKey,
    t: usize,
    n: usize,
    rng: &mut R,
) -> Vec<(usize, Fr)> {
    assert!(t <= n, "Threshold cannot be greater than n");
    assert!(t > 0, "Threshold must be greater than 0");
    assert!(n <= 1000, "Validator set size n cannot exceed 1000");
    assert!(!sk.0.is_zero(), "Secret key cannot be zero");
    
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
pub fn compute_lagrange_coefficient(i: usize, s: &[usize]) -> Result<Fr, String> {
    // 1. Validate that indices are unique
    let mut unique_indices = std::collections::HashSet::new();
    for &val in s {
        if !unique_indices.insert(val) {
            return Err("Duplicate indices detected in s".to_string());
        }
    }
    
    // 2. Validate that i is in s
    if !s.contains(&i) {
        return Err("Target index not found in validator set s".to_string());
    }

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
    
    // 3. Check for zero denominator before inverting
    if den.is_zero() {
        return Err("Zero denominator in Lagrange coefficient computation".to_string());
    }
    
    den.inverse()
        .map(|inv| num * inv)
        .ok_or_else(|| "Lagrange denominator inverse failed".to_string())
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

/// Derives the public commitment for one Shamir share.
pub fn public_key_for_share(share_sk: &Fr) -> IssuerPublicKey {
    IssuerPublicKey(G2Projective::generator() * share_sk)
}

/// Verifies one masked partial signature against its pinned public share.
pub fn verify_partial_signature(
    x: &BlindedMessage,
    k: &Fr,
    partial_sig: &PartialBlindSignature,
    public_share: &IssuerPublicKey,
) -> bool {
    if partial_sig.0.is_zero() || public_share.0.is_zero() || k.is_zero() {
        return false;
    }

    let lhs = Bls12_381::pairing(partial_sig.0, G2Projective::generator());
    let rhs = Bls12_381::pairing(x.0, public_share.0 * k);
    lhs == rhs
}

/// Client: aggregates partial blind signatures from a subset of validators using Lagrange interpolation.
pub fn aggregate_shares(
    partial_sigs: &[(usize, PartialBlindSignature)],
) -> Result<MaskedBlindSignature, String> {
    let indices: Vec<usize> = partial_sigs.iter().map(|(i, _)| *i).collect();
    
    // 1. Validate uniqueness of indices
    let mut unique_indices = std::collections::HashSet::new();
    for &i in &indices {
        if !unique_indices.insert(i) {
            return Err("Duplicate indices detected in partial signatures".to_string());
        }
    }

    // 2. Check if we have sufficient partial signatures
    if partial_sigs.is_empty() {
        return Err("Cannot aggregate empty partial signatures".to_string());
    }

    let mut sum = G1Projective::generator() * Fr::from(0u64);
    for &(i, ref sig) in partial_sigs {
        let l_i = compute_lagrange_coefficient(i, &indices)?;
        sum += sig.0 * l_i;
    }
    Ok(MaskedBlindSignature(sum))
}
