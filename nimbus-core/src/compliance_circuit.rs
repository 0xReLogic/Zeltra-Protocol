//! # Compliance Circuit (EXPERIMENTAL PROTOTYPE - NOT USED IN PRODUCTION SETTLEMENT)
//!
//! > ⚠️ **ARCHITECTURAL & SECURITY NOTICE (DEC-028 Finding R1 / DEC-026)**:
//! > This circuit is a standalone, early experimental proof-of-concept for testing
//! > Poseidon nullifier preimage derivation.
//! >
//! > **DO NOT USE FOR FINANCIAL VALUE TRANSFERS OR SETTLEMENT IN PRODUCTION.**
//! >
//! > 1. **Unconstrained Public Inputs**: The variables `_root_var`, `_recipient_var`,
//! >    and `_amount_var` in `generate_constraints` are intentionally unconstrained in this prototype.
//! > 2. **Production Settlement Rails**: Production shielded value transfers in Zeltra Protocol
//! >    use [`PrivateNoteCircuit`](crate::note_circuit::PrivateNoteCircuit) (12 public inputs)
//! >    and [`JoinSplitCircuit`](crate::joinsplit_circuit::JoinSplitCircuit) (14 public inputs).
//! >    Both circuits strictly enforce exact value conservation, 64-bit integer range constraints,
//! >    Poseidon nullifier derivations, Merkle membership paths, and EVM target domain bindings.
//! > 3. **Future ASP Roadmap (Phase 3)**: Formal Association Set Provider (ASP) membership proofs
//! >    will be implemented as part of DEC-026 (Receiver-Enforced Compliance) with dedicated Merkle
//! >    inclusion gadgets, separate from this legacy prototype.
//!
//! Based on 2026 implementations from orbinum/groth16-proofs and ark-groth16 v0.6.0.

use ark_bls12_381::{Bls12_381, Fr};
use ark_relations::gr1cs::{ConstraintSynthesizer, ConstraintSystemRef, SynthesisError};
use ark_snark::SNARK;

use crate::poseidon::{compute_nullifier, poseidon_hash};

/// Compliance circuit that verifies spend validity (EXPERIMENTAL PROTOTYPE).
///
/// > ⚠️ **WARNING**: This circuit is an early prototype and does NOT constrain
/// > `root`, `recipient`, or `amount` in R1CS (see DEC-028 Finding R1).
/// > Production settlement MUST use [`PrivateNoteCircuit`](crate::PrivateNoteCircuit)
/// > or [`JoinSplitCircuit`](crate::joinsplit_circuit::JoinSplitCircuit).
///
/// Public inputs: root, nullifier, recipient, amount
/// Private witnesses: secret, randomness
///
/// Core constraint: nullifier = Poseidon(secret, randomness)
#[deprecated(
    since = "0.2.0",
    note = "ComplianceCircuit is an unconstrained experimental prototype. Production settlement uses PrivateNoteCircuit and JoinSplitCircuit (DEC-028 / DEC-030)."
)]
pub struct ComplianceCircuit {
    /// Public input: Merkle root
    pub root: Option<Fr>,
    /// Public input: Nullifier (derived from commitment via Poseidon)
    pub nullifier: Option<Fr>,
    /// Public input: Recipient address
    pub recipient: Option<Fr>,
    /// Public input: Spend amount
    pub amount: Option<Fr>,
    /// Private witness: Secret key
    pub secret: Option<Fr>,
    /// Private witness: Randomness
    pub randomness: Option<Fr>,
}

#[allow(deprecated)]
impl ConstraintSynthesizer<Fr> for ComplianceCircuit {
    fn generate_constraints(self, cs: ConstraintSystemRef<Fr>) -> Result<(), SynthesisError> {
        use ark_r1cs_std::eq::EqGadget;
        use ark_r1cs_std::fields::fp::{AllocatedFp, FpVar};

        let (rc, mds) = &*crate::poseidon::CACHED_W3_PARAMS;

        // NOTE & WARNING (DEC-028 Finding R1):
        // In this experimental prototype, only the nullifier derivation is mathematically constrained.
        // `_root_var`, `_recipient_var`, and `_amount_var` are unconstrained placeholders.
        // DO NOT use for production fund transfers. Production settlement runs through
        // `PrivateNoteCircuit` (12 public inputs) or `JoinSplitCircuit` (14 public inputs)
        // where all parameters are strictly bound and range-checked.
        let _root_var =
            cs.new_input_variable(|| self.root.ok_or(SynthesisError::AssignmentMissing))?;
        let nullifier_var =
            cs.new_input_variable(|| self.nullifier.ok_or(SynthesisError::AssignmentMissing))?;
        let _recipient_var =
            cs.new_input_variable(|| self.recipient.ok_or(SynthesisError::AssignmentMissing))?;
        let _amount_var =
            cs.new_input_variable(|| self.amount.ok_or(SynthesisError::AssignmentMissing))?;

        // Allocate private witnesses via gr1cs API, then wrap as FpVar
        let secret_var =
            cs.new_witness_variable(|| self.secret.ok_or(SynthesisError::AssignmentMissing))?;
        let randomness_var =
            cs.new_witness_variable(|| self.randomness.ok_or(SynthesisError::AssignmentMissing))?;

        let secret_fp = FpVar::Var(AllocatedFp::new(self.secret, secret_var, cs.clone()));
        let randomness_fp = FpVar::Var(AllocatedFp::new(
            self.randomness,
            randomness_var,
            cs.clone(),
        ));

        // Constraint 1: Nullifier = Poseidon(secret, randomness)
        // This is a proper cryptographic hash constraint — collision-resistant
        // and non-algebraic, preventing double-spending via nullifier forgery.
        let computed_nullifier = poseidon_hash(&secret_fp, &randomness_fp, rc, mds)?;

        // Wrap the public nullifier variable as FpVar for enforce_equal
        let nullifier_fp = FpVar::Var(AllocatedFp::new(self.nullifier, nullifier_var, cs.clone()));

        // Enforce: computed_nullifier == nullifier (public input)
        computed_nullifier.enforce_equal(&nullifier_fp)?;

        Ok(())
    }
}

/// Proving key and verifying key for compliance circuit
pub struct ComplianceKeys {
    pub proving_key: ark_groth16::ProvingKey<Bls12_381>,
    pub verifying_key: ark_groth16::VerifyingKey<Bls12_381>,
}

/// Deterministic seed for Phase A trusted setup.
/// Phase B: replace with MPC ceremony output.
pub const TRUSTED_SETUP_SEED: u64 = 0x4e696d6275735453; // "NimbusTS"

/// Generate proving and verifying keys for compliance circuit.
///
/// Uses a deterministic seeded RNG for reproducible setup (Phase A).
/// The VK produced by this function MUST match the VK embedded in the
/// contract — they are two halves of the same trusted setup artifact.
///
/// Phase B: Replace with MPC ceremony output distributed as versioned artifacts.
#[allow(deprecated)]
pub fn generate_compliance_keys() -> Result<ComplianceKeys, SynthesisError> {
    use ark_groth16::Groth16;
    use ark_std::rand::rngs::StdRng;
    use ark_std::rand::SeedableRng;

    let mut rng = StdRng::seed_from_u64(TRUSTED_SETUP_SEED);

    // Create a dummy circuit for key generation (setup only needs structure)
    let dummy_secret = Fr::from(1u64);
    let dummy_randomness = Fr::from(2u64);
    let dummy_nullifier = compute_nullifier(dummy_secret, dummy_randomness);

    let circuit = ComplianceCircuit {
        root: Some(Fr::from(1u64)),
        nullifier: Some(dummy_nullifier),
        recipient: Some(Fr::from(3u64)),
        amount: Some(Fr::from(1000u64)),
        secret: Some(dummy_secret),
        randomness: Some(dummy_randomness),
    };

    let (pk, _) = Groth16::<Bls12_381>::circuit_specific_setup(circuit, &mut rng)?;
    let vk = pk.vk.clone();

    Ok(ComplianceKeys {
        proving_key: pk,
        verifying_key: vk,
    })
}

/// Generate a compliance proof with multithreading support.
///
/// The caller must provide `secret` and `randomness` such that
/// `nullifier == Poseidon(secret, randomness)`.
#[allow(deprecated)]
pub fn generate_compliance_proof(
    root: Fr,
    nullifier: Fr,
    recipient: Fr,
    amount: Fr,
    secret: Fr,
    randomness: Fr,
    pk: &ark_groth16::ProvingKey<Bls12_381>,
) -> Result<ark_groth16::Proof<Bls12_381>, SynthesisError> {
    use ark_groth16::Groth16;
    use ark_std::rand::rngs::StdRng;
    use ark_std::rand::SeedableRng;

    let mut rng = StdRng::from_entropy();

    let circuit = ComplianceCircuit {
        root: Some(root),
        nullifier: Some(nullifier),
        recipient: Some(recipient),
        amount: Some(amount),
        secret: Some(secret),
        randomness: Some(randomness),
    };

    Groth16::<Bls12_381>::prove(pk, circuit, &mut rng)
}

/// Verify a compliance proof off-chain (for testing and SDK use).
pub fn verify_compliance_proof(
    proof: &ark_groth16::Proof<Bls12_381>,
    vk: &ark_groth16::VerifyingKey<Bls12_381>,
    root: Fr,
    nullifier: Fr,
    recipient: Fr,
    amount: Fr,
) -> bool {
    use ark_groth16::{prepare_verifying_key, Groth16};

    let public_inputs = vec![root, nullifier, recipient, amount];
    let pvk = prepare_verifying_key(vk);
    Groth16::<Bls12_381>::verify_with_processed_vk(&pvk, &public_inputs, proof).unwrap_or(false)
}

/// Serialize the verifying key to compressed bytes.
///
/// Used to produce the artifact that gets embedded in the contract.
/// The contract uses EVM precompiles for verification, so it needs
/// the VK in EVM point format — see `generate_evm_vk_constants()`.
pub fn serialize_vk_bytes(vk: &ark_groth16::VerifyingKey<Bls12_381>) -> Vec<u8> {
    use ark_serialize::CanonicalSerialize;
    let mut buf = vec![];
    vk.serialize_compressed(&mut buf)
        .expect("VK serialization should not fail");
    buf
}

/// Generate EVM-formatted VK constants for the on-chain verifier.
///
/// Returns the 5 VK components in EVM precompile format:
/// - alpha_g1: [u8; 128]
/// - beta_g2:  [u8; 256]
/// - gamma_g2: [u8; 256]
/// - delta_g2: [u8; 256]
/// - ic[5]:    [[u8; 128]; 5]
///
/// These must be hardcoded in `verification.rs::get_compliance_vk()`.
#[allow(clippy::type_complexity)]
pub fn generate_evm_vk_constants(
    vk: &ark_groth16::VerifyingKey<Bls12_381>,
) -> ([u8; 128], [u8; 256], [u8; 256], [u8; 256], [[u8; 128]; 5]) {
    use crate::evm::{to_evm_g1, to_evm_g2};

    fn to_g1_array(v: &[u8]) -> [u8; 128] {
        let mut a = [0u8; 128];
        let len = v.len().min(128);
        a[..len].copy_from_slice(&v[..len]);
        a
    }

    fn to_g2_array(v: &[u8]) -> [u8; 256] {
        let mut a = [0u8; 256];
        let len = v.len().min(256);
        a[..len].copy_from_slice(&v[..len]);
        a
    }

    let alpha_g1 = to_g1_array(&to_evm_g1(&vk.alpha_g1));
    let beta_g2 = to_g2_array(&to_evm_g2(&vk.beta_g2));
    let gamma_g2 = to_g2_array(&to_evm_g2(&vk.gamma_g2));
    let delta_g2 = to_g2_array(&to_evm_g2(&vk.delta_g2));

    let mut ic = [[0u8; 128]; 5];
    for (i, ic_point) in vk.gamma_abc_g1.iter().take(5).enumerate() {
        ic[i] = to_g1_array(&to_evm_g1(ic_point));
    }

    (alpha_g1, beta_g2, gamma_g2, delta_g2, ic)
}

#[cfg(test)]
#[allow(deprecated)]
mod tests {
    use super::*;

    #[test]
    fn test_compliance_circuit_valid_proof() {
        // Generate keys with deterministic setup
        let keys = generate_compliance_keys().unwrap();

        // Compute valid nullifier via Poseidon
        let root = Fr::from(1u64);
        let recipient = Fr::from(3u64);
        let amount = Fr::from(1000u64);
        let secret = Fr::from(42u64);
        let randomness = Fr::from(99u64);
        let nullifier = compute_nullifier(secret, randomness);

        let proof = generate_compliance_proof(
            root,
            nullifier,
            recipient,
            amount,
            secret,
            randomness,
            &keys.proving_key,
        )
        .unwrap();

        // Verify proof
        let is_valid = verify_compliance_proof(
            &proof,
            &keys.verifying_key,
            root,
            nullifier,
            recipient,
            amount,
        );
        assert!(is_valid, "Valid proof should verify successfully");
    }

    #[test]
    fn test_compliance_circuit_tampered_nullifier() {
        let keys = generate_compliance_keys().unwrap();

        let root = Fr::from(1u64);
        let recipient = Fr::from(3u64);
        let amount = Fr::from(1000u64);
        let secret = Fr::from(42u64);
        let randomness = Fr::from(99u64);
        let correct_nullifier = compute_nullifier(secret, randomness);

        let proof = generate_compliance_proof(
            root,
            correct_nullifier,
            recipient,
            amount,
            secret,
            randomness,
            &keys.proving_key,
        )
        .unwrap();

        // Correct nullifier verifies
        let is_valid = verify_compliance_proof(
            &proof,
            &keys.verifying_key,
            root,
            correct_nullifier,
            recipient,
            amount,
        );
        assert!(is_valid, "Correct nullifier must verify");

        // Tampered nullifier (off by 1) must fail
        let tampered_nullifier = correct_nullifier + Fr::from(1u64);
        let is_valid = verify_compliance_proof(
            &proof,
            &keys.verifying_key,
            root,
            tampered_nullifier,
            recipient,
            amount,
        );
        assert!(
            !is_valid,
            "Proof with tampered nullifier must fail verification"
        );
    }

    #[test]
    fn test_compliance_circuit_wrong_nullifier_for_secret() {
        let secret = Fr::from(42u64);
        let randomness = Fr::from(99u64);
        let correct_nullifier = compute_nullifier(secret, randomness);

        // Old linear formula must differ from Poseidon
        let wrong_nullifier = secret + randomness;
        assert_ne!(
            wrong_nullifier, correct_nullifier,
            "Linear nullifier must differ from Poseidon nullifier"
        );

        // Prover panics on unsatisfied constraints (ark-groth16 assert),
        // so we catch the panic.
        let keys = generate_compliance_keys().unwrap();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            generate_compliance_proof(
                Fr::from(1u64),
                wrong_nullifier,
                Fr::from(3u64),
                Fr::from(1000u64),
                secret,
                randomness,
                &keys.proving_key,
            )
        }));
        assert!(
            result.is_err() || result.unwrap().is_err(),
            "Proof with wrong nullifier must fail to generate"
        );
    }

    #[test]
    fn test_compliance_circuit_wrong_vk() {
        let keys = generate_compliance_keys().unwrap();

        // Generate a second set of keys with a DIFFERENT seed
        use ark_groth16::Groth16;
        use ark_std::rand::rngs::StdRng;
        use ark_std::rand::SeedableRng;

        let mut rng2 = StdRng::seed_from_u64(0xDEADBEEF);
        let dummy_secret = Fr::from(1u64);
        let dummy_randomness = Fr::from(2u64);
        let dummy_nullifier = compute_nullifier(dummy_secret, dummy_randomness);

        let circuit2 = ComplianceCircuit {
            root: Some(Fr::from(1u64)),
            nullifier: Some(dummy_nullifier),
            recipient: Some(Fr::from(3u64)),
            amount: Some(Fr::from(1000u64)),
            secret: Some(dummy_secret),
            randomness: Some(dummy_randomness),
        };

        let (wrong_pk, _) =
            Groth16::<Bls12_381>::circuit_specific_setup(circuit2, &mut rng2).unwrap();
        let wrong_vk = wrong_pk.vk.clone();

        // Generate proof with correct PK
        let secret = Fr::from(42u64);
        let randomness = Fr::from(99u64);
        let root = Fr::from(1u64);
        let recipient = Fr::from(3u64);
        let amount = Fr::from(1000u64);
        let nullifier = compute_nullifier(secret, randomness);

        let proof = generate_compliance_proof(
            root,
            nullifier,
            recipient,
            amount,
            secret,
            randomness,
            &keys.proving_key,
        )
        .unwrap();

        // Verify with WRONG VK must fail
        let is_valid =
            verify_compliance_proof(&proof, &wrong_vk, root, nullifier, recipient, amount);
        assert!(!is_valid, "Proof verified with wrong VK must fail");
    }

    #[test]
    fn print_evm_vk_hex() {
        let keys = generate_compliance_keys().unwrap();
        let (alpha, beta, gamma, delta, ic) = generate_evm_vk_constants(&keys.verifying_key);

        let to_hex =
            |b: &[u8]| -> String { b.iter().map(|x| format!("{x:02x}")).collect::<String>() };

        eprintln!("// --- Phase A Trusted Setup VK Constants ---");
        eprintln!("// Seed: 0x{:016X}", TRUSTED_SETUP_SEED);
        eprintln!("// Generated by: generate_evm_vk_constants()");
        eprintln!(
            "// Circuit: Poseidon nullifier (width=3, alpha=5, R_F=8, R_P=57) over BLS12-381"
        );
        eprintln!();
        eprintln!("// vk_alpha_g1 (128 bytes)");
        eprintln!(
            "let vk_alpha_g1_bytes: [u8; 128] = hex!(\"{}\");",
            to_hex(&alpha)
        );
        eprintln!();
        eprintln!("// vk_beta_g2 (256 bytes)");
        eprintln!(
            "let vk_beta_g2_bytes: [u8; 256] = hex!(\"{}\");",
            to_hex(&beta)
        );
        eprintln!();
        eprintln!("// vk_gamma_g2 (256 bytes)");
        eprintln!(
            "let vk_gamma_g2_bytes: [u8; 256] = hex!(\"{}\");",
            to_hex(&gamma)
        );
        eprintln!();
        eprintln!("// vk_delta_g2 (256 bytes)");
        eprintln!(
            "let vk_delta_g2_bytes: [u8; 256] = hex!(\"{}\");",
            to_hex(&delta)
        );
        eprintln!();
        for (i, ic_point) in ic.iter().enumerate() {
            eprintln!("// vk_ic[{}] (128 bytes)", i);
            eprintln!(
                "let vk_ic_{}_bytes: [u8; 128] = hex!(\"{}\");",
                i,
                to_hex(ic_point)
            );
            eprintln!();
        }
    }

    #[test]
    fn test_vk_serialization_roundtrip() {
        use ark_serialize::CanonicalDeserialize;

        let keys = generate_compliance_keys().unwrap();
        let vk_bytes = serialize_vk_bytes(&keys.verifying_key);
        assert!(!vk_bytes.is_empty());

        // Deserialize and verify it matches
        let vk_restored =
            ark_groth16::VerifyingKey::<Bls12_381>::deserialize_compressed(&vk_bytes[..]).unwrap();
        assert_eq!(
            keys.verifying_key.alpha_g1, vk_restored.alpha_g1,
            "Deserialized VK must match original"
        );
    }

    #[test]
    fn test_evm_vk_constants_generation() {
        let keys = generate_compliance_keys().unwrap();
        let (alpha, beta, gamma, delta, ic) = generate_evm_vk_constants(&keys.verifying_key);

        // Verify non-zero
        assert!(alpha.iter().any(|&b| b != 0), "alpha_g1 must be non-zero");
        assert!(beta.iter().any(|&b| b != 0), "beta_g2 must be non-zero");
        assert!(gamma.iter().any(|&b| b != 0), "gamma_g2 must be non-zero");
        assert!(delta.iter().any(|&b| b != 0), "delta_g2 must be non-zero");
        for (i, ic_point) in ic.iter().enumerate() {
            assert!(ic_point.iter().any(|&b| b != 0), "ic[{i}] must be non-zero");
        }
    }
}
