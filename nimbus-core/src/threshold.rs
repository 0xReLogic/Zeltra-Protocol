use crate::types::{BlindedMessage, IssuerSecretKey, MaskedBlindSignature, PartialBlindSignature};
use ark_bls12_381::{Fr, G1Projective};
use ark_ec::Group;
use ark_ff::{Field, UniformRand};
use rand::Rng;

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
