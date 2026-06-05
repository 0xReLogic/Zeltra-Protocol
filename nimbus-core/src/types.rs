use ark_bls12_381::{Fr, G1Projective, G2Projective};
use ark_serialize::{CanonicalDeserialize, CanonicalSerialize};

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

#[derive(Clone, Debug, CanonicalSerialize, CanonicalDeserialize)]
pub struct PartialBlindSignature(pub G1Projective);
