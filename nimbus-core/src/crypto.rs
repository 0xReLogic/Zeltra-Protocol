use crate::types::{IssuerPublicKey, IssuerSecretKey};
use ark_bls12_381::{Fr, G1Projective, G2Projective};
use ark_ec::PrimeGroup;
use ark_ff::{PrimeField, UniformRand};
use rand::Rng;
use sha2::{Digest, Sha256};

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
