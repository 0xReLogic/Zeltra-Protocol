use crate::types::{IssuerPublicKey, IssuerSecretKey};
use ark_bls12_381::{Fr, G1Projective, G2Projective};
use ark_ec::{AffineRepr, PrimeGroup};
use ark_ff::UniformRand;
use rand::Rng;
use sha2::Sha256;

use ark_bls12_381::g1::Config as G1Config;
use ark_ec::hashing::curve_maps::wb::WBMap;
use ark_ec::hashing::map_to_curve_hasher::MapToCurveBasedHasher;
use ark_ec::hashing::HashToCurve;
use ark_ff::fields::field_hashers::DefaultFieldHasher;

/// Hash a message to G1Projective per RFC 9380 compliant hash-to-curve.
pub fn hash_to_g1(message: &[u8]) -> G1Projective {
    let hasher = MapToCurveBasedHasher::<
        G1Projective,
        DefaultFieldHasher<Sha256, 128>,
        WBMap<G1Config>,
    >::new(b"BLS_SIG_BLS12381G1_XMD:SHA-256_SSWU_RO_NUL_")
    .unwrap();

    let affine = hasher.hash(message).unwrap();
    affine.into_group()
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
