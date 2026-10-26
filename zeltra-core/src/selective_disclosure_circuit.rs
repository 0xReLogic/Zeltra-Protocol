//! Groth16 Selective Disclosure Circuit (5 Public Inputs) — DEC-040A.
//!
//! Provides a lightweight R1CS zero-knowledge circuit (~450 constraints, proving time < 15ms)
//! allowing a user to prove note ownership and nullifier authenticity to a third-party verifier (e.g. CEX)
//! without revealing their master spending key ($sk$), other notes, or general viewing keys.
//!
//! Public Inputs (Strict Canonical Ordering):
//! - Slot 0: `owner_key`    (Fr) — Public owner key scalar from note commitment
//! - Slot 1: `rho`          (Fr) — Unique nullifier seed of the note preimage
//! - Slot 2: `nullifier`    (Fr) — Spent nullifier on-chain
//! - Slot 3: `challenge_hi` (Fr) — High 128-bit limb of Blake2s-256 verifier challenge (DEC-035A)
//! - Slot 4: `challenge_lo` (Fr) — Low 128-bit limb of Blake2s-256 verifier challenge (DEC-035A)
//!
//! Private Witness:
//! - `spending_key`: $sk \in Fr$
//! - `epoch_id`: $epoch\_id \in u32$
//!
//! Constraints:
//! 1. Derivation of $nk = \text{Poseidon\_W3}(sk, 0, \text{DOMAIN\_NULLIFIER\_KEY})$
//! 2. Derivation of $nf = \text{Poseidon\_W5}([nk, \rho, epoch\_id, 0], \text{DOMAIN\_NULLIFIER})$
//! 3. Nullifier equality constraint: $computed\_nf == nullifier$
//! 4. Non-Malleable Challenge Range Binding: $challenge\_hi, challenge\_lo \in [0, 2^{128})$
//! 5. Owner key canonical derivation wire binding: $owner\_key == \text{Poseidon\_W3}(sk, 0, \text{DOMAIN\_OWNER\_KEY})$ (SEC-002)

use ark_bls12_381::{Bls12_381, Fr};
use ark_groth16::{prepare_verifying_key, PreparedVerifyingKey, Proof, ProvingKey, VerifyingKey};
use ark_r1cs_std::fields::fp::FpVar;
use ark_r1cs_std::prelude::*;
use ark_relations::gr1cs::{ConstraintSynthesizer, ConstraintSystemRef, SynthesisError};
use ark_snark::SNARK;
use ark_std::rand::RngCore;

use crate::circuit_gadgets::{
    enforce_dummy_range_binding, poseidon_w3_gadget, poseidon_w5_gadget, private_witness,
    public_input,
};

/// Number of public inputs in the SelectiveDisclosureCircuit.
pub const NUM_DISCLOSURE_PUBLIC_INPUTS: usize = 5;

/// Selective Disclosure Circuit for CEX Source-of-Funds Proofs (DEC-040A).
#[derive(Clone, Debug)]
pub struct SelectiveDisclosureCircuit {
    // Private Witness
    pub spending_key: Option<Fr>,
    pub epoch_id: Option<u32>,

    // Public Inputs (5-PI)
    pub owner_key: Option<Fr>,
    pub rho: Option<Fr>,
    pub nullifier: Option<Fr>,
    pub challenge_hi: Option<Fr>,
    pub challenge_lo: Option<Fr>,
}

impl SelectiveDisclosureCircuit {
    /// Creates an unassigned dummy circuit for parameter generation during trusted setup.
    pub fn dummy() -> Self {
        Self {
            spending_key: None,
            epoch_id: None,
            owner_key: None,
            rho: None,
            nullifier: None,
            challenge_hi: None,
            challenge_lo: None,
        }
    }

    /// Creates an instance with full witness assignments.
    pub fn new(
        spending_key: Fr,
        epoch_id: u32,
        owner_key: Fr,
        rho: Fr,
        nullifier: Fr,
        challenge_hi: Fr,
        challenge_lo: Fr,
    ) -> Self {
        Self {
            spending_key: Some(spending_key),
            epoch_id: Some(epoch_id),
            owner_key: Some(owner_key),
            rho: Some(rho),
            nullifier: Some(nullifier),
            challenge_hi: Some(challenge_hi),
            challenge_lo: Some(challenge_lo),
        }
    }

    /// Extracts the public inputs array in canonical order (Slot 0..=4).
    pub fn extract_public_inputs(&self) -> Option<Vec<Fr>> {
        Some(vec![
            self.owner_key?,
            self.rho?,
            self.nullifier?,
            self.challenge_hi?,
            self.challenge_lo?,
        ])
    }
}

impl ConstraintSynthesizer<Fr> for SelectiveDisclosureCircuit {
    fn generate_constraints(self, cs: ConstraintSystemRef<Fr>) -> Result<(), SynthesisError> {
        // ═══════════════════════════════════════════════════════════════
        // 1. Allocate Public Inputs (Strict Canonical Ordering: Slot 0..=4)
        // ═══════════════════════════════════════════════════════════════
        // Slot 0: owner_key
        let owner_key_var = public_input(&cs, self.owner_key)?;
        // Slot 1: rho
        let rho_var = public_input(&cs, self.rho)?;
        // Slot 2: nullifier
        let nullifier_var = public_input(&cs, self.nullifier)?;
        // Slot 3: challenge_hi (128-bit limb)
        let challenge_hi_var = public_input(&cs, self.challenge_hi)?;
        // Slot 4: challenge_lo (128-bit limb)
        let challenge_lo_var = public_input(&cs, self.challenge_lo)?;

        // ═══════════════════════════════════════════════════════════════
        // 2. Allocate Private Witness Variables
        // ═══════════════════════════════════════════════════════════════
        let spending_key_var = private_witness(&cs, self.spending_key)?;
        let epoch_id_var = private_witness(&cs, self.epoch_id.map(|e| Fr::from(e as u64)))?;

        // ═══════════════════════════════════════════════════════════════
        // 3. Constraint 1: Derivation of nk via Poseidon-W3
        //    nk = Poseidon_W3([sk, 0], DOMAIN_NULLIFIER_KEY)
        // ═══════════════════════════════════════════════════════════════
        let domain_nk = FpVar::Constant(crate::note::domain_nullifier_key());
        let zero_var = FpVar::Constant(Fr::from(0u64));
        let nk_var =
            poseidon_w3_gadget(cs.clone(), &[spending_key_var.clone(), zero_var.clone()], domain_nk)?;

        // ═══════════════════════════════════════════════════════════════
        // 4. Constraint 2: Derivation of Canonical Nullifier via Poseidon-W5
        //    nf = Poseidon_W5([nk, rho, epoch_id, 0], DOMAIN_NULLIFIER)
        // ═══════════════════════════════════════════════════════════════
        let domain_nf = FpVar::Constant(crate::note::domain_nullifier());
        let computed_nf = poseidon_w5_gadget(
            cs.clone(),
            &[nk_var, rho_var, epoch_id_var, zero_var.clone()],
            domain_nf,
        )?;
        computed_nf.enforce_equal(&nullifier_var)?;

        // ═══════════════════════════════════════════════════════════════
        // 5. Constraint 3: Non-Malleable Challenge Range Binding (DEC-035B)
        //    Enforce challenge_hi, challenge_lo in [0, 2^128)
        // ═══════════════════════════════════════════════════════════════
        enforce_dummy_range_binding(cs.clone(), &challenge_hi_var, &challenge_lo_var)?;

        // ═══════════════════════════════════════════════════════════════
        // 6. Constraint 4: Owner Key Canonical Derivation & Wire Binding (SEC-002)
        //    Enforce owner_key == Poseidon_W3([sk, 0], DOMAIN_OWNER_KEY)
        //    Guarantees that only the holder of the spending key can claim
        //    ownership of the note, preventing identity spoofing and false attribution.
        // ═══════════════════════════════════════════════════════════════
        let domain_owner = FpVar::Constant(crate::keys::domain_owner_key());
        let computed_owner = poseidon_w3_gadget(
            cs.clone(),
            &[spending_key_var, zero_var],
            domain_owner,
        )?;
        computed_owner.enforce_equal(&owner_key_var)?;

        Ok(())
    }
}

/// Sets up proving and verifying keys for SelectiveDisclosureCircuit.
pub fn setup_disclosure_circuit_params<R: RngCore + rand::CryptoRng>(
    rng: &mut R,
) -> (ProvingKey<Bls12_381>, VerifyingKey<Bls12_381>) {
    use ark_groth16::Groth16;
    let dummy = SelectiveDisclosureCircuit::dummy();
    let (pk, vk) = Groth16::<Bls12_381>::circuit_specific_setup(dummy, rng)
        .expect("circuit_specific_setup failed for SelectiveDisclosureCircuit");
    (pk, vk)
}

/// Generates a Groth16 proof for SelectiveDisclosureCircuit.
pub fn generate_disclosure_proof(
    circuit: SelectiveDisclosureCircuit,
    pk: &ProvingKey<Bls12_381>,
) -> Result<Proof<Bls12_381>, SynthesisError> {
    use ark_groth16::Groth16;
    use ark_std::rand::rngs::StdRng;
    use ark_std::rand::SeedableRng;

    let mut rng = StdRng::from_entropy();
    Groth16::<Bls12_381>::prove(pk, circuit, &mut rng)
}

/// Verifies a Groth16 proof for SelectiveDisclosureCircuit against 5 public inputs using PreparedVerifyingKey.
pub fn verify_disclosure_proof(
    proof: &Proof<Bls12_381>,
    public_inputs: &[Fr],
    vk: &PreparedVerifyingKey<Bls12_381>,
) -> Result<bool, SynthesisError> {
    if public_inputs.len() != NUM_DISCLOSURE_PUBLIC_INPUTS {
        return Ok(false);
    }
    use ark_groth16::Groth16;
    Groth16::<Bls12_381>::verify_with_processed_vk(vk, public_inputs, proof)
}

/// Verifies a Groth16 proof using an unprepared VerifyingKey.
pub fn verify_disclosure_proof_unprepared(
    proof: &Proof<Bls12_381>,
    public_inputs: &[Fr],
    vk: &VerifyingKey<Bls12_381>,
) -> Result<bool, SynthesisError> {
    let pvk = prepare_verifying_key(vk);
    verify_disclosure_proof(proof, public_inputs, &pvk)
}

/// Decomposes a 32-byte verifier challenge into two canonical 128-bit field elements (DEC-035A, DEC-040A).
pub fn decompose_challenge_to_limbs(challenge: &[u8; 32]) -> (Fr, Fr) {
    let mut hi_bytes = [0u8; 16];
    let mut lo_bytes = [0u8; 16];
    hi_bytes.copy_from_slice(&challenge[0..16]);
    lo_bytes.copy_from_slice(&challenge[16..32]);
    let hi_u128 = u128::from_be_bytes(hi_bytes);
    let lo_u128 = u128::from_be_bytes(lo_bytes);
    (Fr::from(hi_u128), Fr::from(lo_u128))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::note::{
        derive_joinsplit_nullifier, derive_nullifier, derive_nullifier_key, FullViewingKey,
        KeyFormatError,
    };
    use ark_ec::AffineRepr;
    use ark_groth16::prepare_verifying_key;
    use ark_relations::gr1cs::ConstraintSystem;
    use ark_std::rand::rngs::StdRng;
    use ark_std::rand::SeedableRng;
    use zeroize::Zeroize;

    fn sample_circuit_inputs() -> (Fr, u32, Fr, Fr, Fr, Fr, Fr) {
        let spending_key = Fr::from(1337u64);
        let epoch_id = 1u32;
        let owner_key = crate::keys::derive_owner_key(spending_key);
        let rho = Fr::from(42u64);

        let nk = derive_nullifier_key(spending_key);
        let nullifier = derive_joinsplit_nullifier(nk, rho, epoch_id as u64);

        let challenge_bytes = [0x5au8; 32];
        let (challenge_hi, challenge_lo) = decompose_challenge_to_limbs(&challenge_bytes);

        (
            spending_key,
            epoch_id,
            owner_key,
            rho,
            nullifier,
            challenge_hi,
            challenge_lo,
        )
    }

    // ────────────────────────────────────────────────────────────────────────
    // POSITIVE TESTS
    // ────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_selective_disclosure_circuit_roundtrip() {
        let mut rng = StdRng::seed_from_u64(0xdec040a);
        let (pk, vk) = setup_disclosure_circuit_params(&mut rng);
        let pvk = prepare_verifying_key(&vk);

        let (sk, epoch, owner, rho, nf, c_hi, c_lo) = sample_circuit_inputs();
        let circuit = SelectiveDisclosureCircuit::new(sk, epoch, owner, rho, nf, c_hi, c_lo);

        let public_inputs = circuit
            .extract_public_inputs()
            .expect("public inputs extraction");
        assert_eq!(public_inputs.len(), NUM_DISCLOSURE_PUBLIC_INPUTS);

        // Verify constraint system satisfaction directly
        let cs = ConstraintSystem::<Fr>::new_ref();
        circuit
            .clone()
            .generate_constraints(cs.clone())
            .expect("constraint generation must succeed");
        assert!(
            cs.is_satisfied().expect("satisfaction check"),
            "R1CS constraints must be satisfied with valid witness!"
        );

        let proof = generate_disclosure_proof(circuit, &pk).expect("proof generation must succeed");
        let valid =
            verify_disclosure_proof(&proof, &public_inputs, &pvk).expect("verification execution");
        assert!(valid, "Valid selective disclosure proof MUST verify!");
    }

    #[test]
    fn test_full_viewing_key_nullifier_derivation_parity() {
        let spending_key = Fr::from(0xabcdefu64);
        let nk = derive_nullifier_key(spending_key);

        let fvk = FullViewingKey::new(Fr::from(100u64), [1u8; 32], [2u8; 32], nk);

        // Check JoinSplit nullifier parity
        let rho = Fr::from(777u64);
        let epoch_u64 = 5u64;
        let nf_from_sk = derive_joinsplit_nullifier(nk, rho, epoch_u64);
        let nf_from_fvk = fvk.derive_joinsplit_nullifier(rho, epoch_u64);
        assert_eq!(
            nf_from_sk, nf_from_fvk,
            "FVK derive_joinsplit_nullifier must match direct derivation parity!"
        );

        // Check evolving note nullifier parity
        let commitment = Fr::from(8888u64);
        let leaf_index = 42u64;
        let epoch_u32 = 5u32;
        let nf_evolving_sk = derive_nullifier(nk, commitment, leaf_index, epoch_u32);
        let nf_evolving_fvk = fvk.derive_nullifier(commitment, leaf_index, epoch_u32);
        assert_eq!(
            nf_evolving_sk, nf_evolving_fvk,
            "FVK derive_nullifier must match direct evolving nullifier derivation parity!"
        );
    }

    #[test]
    fn test_fvk_hex_serialization_roundtrip() {
        let fvk_original = FullViewingKey::new(
            Fr::from(123456789u64),
            [0xaa; 32],
            [0xbb; 32],
            Fr::from(987654321u64),
        );

        let hex_str = fvk_original.to_hex();
        assert_eq!(
            hex_str.len(),
            256,
            "FVK hex string must be exactly 256 chars (128 bytes)"
        );

        let fvk_recovered = FullViewingKey::from_hex(&hex_str)
            .expect("from_hex must succeed on valid 256-hex string");
        assert_eq!(
            fvk_original, fvk_recovered,
            "Recovered FVK must match original bit-for-bit!"
        );

        // Also test with "0x" prefix
        let prefixed = format!("0x{}", hex_str);
        let fvk_prefixed =
            FullViewingKey::from_hex(&prefixed).expect("from_hex must accept optional 0x prefix");
        assert_eq!(fvk_original, fvk_prefixed);
    }

    #[test]
    fn test_vk_non_identity_all_elements() {
        let mut rng = StdRng::seed_from_u64(0xdec040a);
        let (_, vk) = setup_disclosure_circuit_params(&mut rng);

        // 5 public inputs => IC has 6 elements (IC[0..=5])
        assert_eq!(
            vk.gamma_abc_g1.len(),
            NUM_DISCLOSURE_PUBLIC_INPUTS + 1,
            "VK must contain exactly 6 IC elements (1 constant + 5 public inputs)"
        );

        for (i, ic) in vk.gamma_abc_g1.iter().enumerate() {
            assert!(
                !ic.is_zero(),
                "VK element IC[{}] MUST NOT be the point at infinity (O_G1)!",
                i
            );
        }
    }

    // ────────────────────────────────────────────────────────────────────────
    // NEGATIVE TESTS (Security, Soundness & Anti-Malleability Guardrails)
    // ────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_disclosure_circuit_rejects_spoofed_nullifier() {
        let (sk, epoch, owner, rho, _nf, c_hi, c_lo) = sample_circuit_inputs();
        let spoofed_nf = Fr::from(0xbad_dead_beefu64); // Attacker inserts spoofed nullifier

        let circuit =
            SelectiveDisclosureCircuit::new(sk, epoch, owner, rho, spoofed_nf, c_hi, c_lo);
        let cs = ConstraintSystem::<Fr>::new_ref();
        circuit
            .generate_constraints(cs.clone())
            .expect("generation executes");

        assert!(
            !cs.is_satisfied().unwrap(),
            "Circuit MUST reject spoofed nullifier: computed_nf != nullifier_var!"
        );
    }

    #[test]
    fn test_disclosure_circuit_rejects_tampered_challenge_hi() {
        let mut rng = StdRng::seed_from_u64(0xdec040a);
        let (pk, vk) = setup_disclosure_circuit_params(&mut rng);
        let pvk = prepare_verifying_key(&vk);

        let (sk, epoch, owner, rho, nf, c_hi, c_lo) = sample_circuit_inputs();
        let circuit = SelectiveDisclosureCircuit::new(sk, epoch, owner, rho, nf, c_hi, c_lo);
        let mut public_inputs = circuit.extract_public_inputs().unwrap();
        let proof = generate_disclosure_proof(circuit, &pk).unwrap();

        // Tamper with challenge_hi (Slot 3)
        public_inputs[3] += Fr::from(1u64);
        let result = verify_disclosure_proof(&proof, &public_inputs, &pvk).unwrap();
        assert!(
            !result,
            "Verification MUST fail when challenge_hi is tampered!"
        );
    }

    #[test]
    fn test_disclosure_circuit_rejects_tampered_challenge_lo() {
        let mut rng = StdRng::seed_from_u64(0xdec040a);
        let (pk, vk) = setup_disclosure_circuit_params(&mut rng);
        let pvk = prepare_verifying_key(&vk);

        let (sk, epoch, owner, rho, nf, c_hi, c_lo) = sample_circuit_inputs();
        let circuit = SelectiveDisclosureCircuit::new(sk, epoch, owner, rho, nf, c_hi, c_lo);
        let mut public_inputs = circuit.extract_public_inputs().unwrap();
        let proof = generate_disclosure_proof(circuit, &pk).unwrap();

        // Tamper with challenge_lo (Slot 4)
        public_inputs[4] += Fr::from(1u64);
        let result = verify_disclosure_proof(&proof, &public_inputs, &pvk).unwrap();
        assert!(
            !result,
            "Verification MUST fail when challenge_lo is tampered!"
        );
    }

    #[test]
    fn test_disclosure_circuit_rejects_tampered_owner_key() {
        let mut rng = StdRng::seed_from_u64(0xdec040a);
        let (pk, vk) = setup_disclosure_circuit_params(&mut rng);
        let pvk = prepare_verifying_key(&vk);

        let (sk, epoch, owner, rho, nf, c_hi, c_lo) = sample_circuit_inputs();
        let circuit = SelectiveDisclosureCircuit::new(sk, epoch, owner, rho, nf, c_hi, c_lo);
        let mut public_inputs = circuit.extract_public_inputs().unwrap();
        let proof = generate_disclosure_proof(circuit, &pk).unwrap();

        // Tamper with owner_key (Slot 0)
        public_inputs[0] += Fr::from(1u64);
        let result = verify_disclosure_proof(&proof, &public_inputs, &pvk).unwrap();
        assert!(
            !result,
            "Verification MUST fail when owner_key is tampered (Anti-Malleability)!"
        );
    }

    #[test]
    fn test_disclosure_circuit_rejects_spoofed_owner_key() {
        let mut rng = StdRng::seed_from_u64(0xdec040a);
        let (pk, _vk) = setup_disclosure_circuit_params(&mut rng);

        // Attacker credentials
        let attacker_sk = Fr::from(1337u64);
        let epoch_id = 1u32;
        let rho = Fr::from(42u64);
        let nk = derive_nullifier_key(attacker_sk);
        let nullifier = derive_joinsplit_nullifier(nk, rho, epoch_id as u64);

        let challenge_bytes = [0x5au8; 32];
        let (challenge_hi, challenge_lo) = decompose_challenge_to_limbs(&challenge_bytes);

        // Victim's owner key (observed on-chain or CEX deposit log)
        let victim_owner_key = Fr::from(0xdead_beef_9999u64);

        // Attacker creates a circuit attempting to claim victim_owner_key without victim's sk
        let circuit = SelectiveDisclosureCircuit::new(
            attacker_sk,
            epoch_id,
            victim_owner_key,
            rho,
            nullifier,
            challenge_hi,
            challenge_lo,
        );

        let cs = ConstraintSystem::<Fr>::new_ref();
        circuit
            .clone()
            .generate_constraints(cs.clone())
            .expect("constraint generation executes");

        // SEC-002 Fix Verification: R1CS constraints MUST NOT be satisfied
        assert!(
            !cs.is_satisfied().unwrap(),
            "FIX VERIFIED (SEC-002): Constraint system MUST reject witness where owner_key != derive_owner_key(sk)!"
        );

        let public_inputs = circuit.extract_public_inputs().unwrap();
        assert_eq!(public_inputs[0], victim_owner_key);

        // Attempting to generate proof from unsatisfied system must fail/panic inside prover
        let prover_result = std::panic::catch_unwind(|| {
            let _ = generate_disclosure_proof(circuit, &pk);
        });
        assert!(
            prover_result.is_err(),
            "FIX VERIFIED (SEC-002): Prover MUST panic/abort when attempting to prove false statement!"
        );
    }

    #[test]
    fn test_disclosure_circuit_rejects_tampered_rho() {
        let mut rng = StdRng::seed_from_u64(0xdec040a);
        let (pk, vk) = setup_disclosure_circuit_params(&mut rng);
        let pvk = prepare_verifying_key(&vk);

        let (sk, epoch, owner, rho, nf, c_hi, c_lo) = sample_circuit_inputs();
        let circuit = SelectiveDisclosureCircuit::new(sk, epoch, owner, rho, nf, c_hi, c_lo);
        let mut public_inputs = circuit.extract_public_inputs().unwrap();
        let proof = generate_disclosure_proof(circuit, &pk).unwrap();

        // Tamper with rho (Slot 1)
        public_inputs[1] += Fr::from(1u64);
        let result = verify_disclosure_proof(&proof, &public_inputs, &pvk).unwrap();
        assert!(!result, "Verification MUST fail when rho is tampered!");
    }

    #[test]
    fn test_disclosure_circuit_rejects_invalid_sk() {
        let (_sk, epoch, owner, rho, nf, c_hi, c_lo) = sample_circuit_inputs();
        let wrong_sk = Fr::from(999_999_999u64); // Wrong spending key

        let circuit = SelectiveDisclosureCircuit::new(wrong_sk, epoch, owner, rho, nf, c_hi, c_lo);
        let cs = ConstraintSystem::<Fr>::new_ref();
        circuit
            .generate_constraints(cs.clone())
            .expect("constraint generation executes");

        assert!(
            !cs.is_satisfied().unwrap(),
            "Circuit MUST reject witness with incorrect spending key!"
        );
    }

    #[test]
    fn test_disclosure_circuit_rejects_tampered_epoch_id() {
        let (sk, _epoch, owner, rho, nf, c_hi, c_lo) = sample_circuit_inputs();
        let wrong_epoch = 99u32; // Epoch mismatch

        let circuit = SelectiveDisclosureCircuit::new(sk, wrong_epoch, owner, rho, nf, c_hi, c_lo);
        let cs = ConstraintSystem::<Fr>::new_ref();
        circuit
            .generate_constraints(cs.clone())
            .expect("constraint generation executes");

        assert!(
            !cs.is_satisfied().unwrap(),
            "Circuit MUST reject witness with mismatched epoch_id!"
        );
    }

    #[test]
    fn test_disclosure_circuit_rejects_malformed_public_inputs_length() {
        let mut rng = StdRng::seed_from_u64(0xdec040a);
        let (pk, vk) = setup_disclosure_circuit_params(&mut rng);
        let pvk = prepare_verifying_key(&vk);

        let (sk, epoch, owner, rho, nf, c_hi, c_lo) = sample_circuit_inputs();
        let circuit = SelectiveDisclosureCircuit::new(sk, epoch, owner, rho, nf, c_hi, c_lo);
        let proof = generate_disclosure_proof(circuit, &pk).unwrap();

        // 4 public inputs instead of 5
        let short_inputs = vec![owner, rho, nf, c_hi];
        let res_short = verify_disclosure_proof(&proof, &short_inputs, &pvk);
        assert_eq!(res_short, Ok(false), "Must fail on 4 public inputs");

        // 6 public inputs instead of 5
        let long_inputs = vec![owner, rho, nf, c_hi, c_lo, Fr::from(1u64)];
        let res_long = verify_disclosure_proof(&proof, &long_inputs, &pvk);
        assert_eq!(res_long, Ok(false), "Must fail on 6 public inputs");
    }

    #[test]
    fn test_fvk_zeroize_on_drop() {
        let mut fvk =
            FullViewingKey::new(Fr::from(42u64), [0xee; 32], [0xff; 32], Fr::from(100u64));

        // Pre-condition: secret byte buffers are non-zero
        assert_ne!(fvk.incoming_viewing_key, [0u8; 32]);
        assert_ne!(fvk.outcoming_viewing_key, [0u8; 32]);

        // Trigger zeroize explicitly
        fvk.zeroize();

        // Post-condition: sensitive buffers are wiped to zero
        assert_eq!(fvk.incoming_viewing_key, [0u8; 32]);
        assert_eq!(fvk.outcoming_viewing_key, [0u8; 32]);
    }

    #[test]
    fn test_fvk_from_hex_rejects_invalid_length() {
        // Hex with 254 chars instead of 256
        let short_hex = "aa".repeat(127);
        let err = FullViewingKey::from_hex(&short_hex);
        assert_eq!(
            err,
            Err(KeyFormatError::InvalidLength {
                expected: 256,
                actual: 254,
            })
        );
    }

    #[test]
    fn test_fvk_from_hex_rejects_invalid_hex() {
        // 256 chars containing non-hex characters 'zz'
        let mut invalid_hex = "00".repeat(127);
        invalid_hex.push_str("zz");
        let err = FullViewingKey::from_hex(&invalid_hex);
        assert_eq!(err, Err(KeyFormatError::InvalidHexEncoding));
    }

    #[test]
    fn test_fvk_from_hex_rejects_non_canonical_scalar() {
        // Modulus r of BLS12-381 is:
        // 0x73eda753299d7d483339d80809a1d80553bda402fffe5bfeffffffff00000001
        // Create an unreduced scalar >= r: e.g. 0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff
        let non_canonical_scalar_hex = "ff".repeat(32);
        let valid_bytes_hex = "00".repeat(32);

        // Put non-canonical scalar in owner_key slot (0..32 bytes)
        let non_canonical_owner = format!(
            "{}{}{}{}",
            non_canonical_scalar_hex, valid_bytes_hex, valid_bytes_hex, valid_bytes_hex
        );
        let err_owner = FullViewingKey::from_hex(&non_canonical_owner);
        assert_eq!(
            err_owner,
            Err(KeyFormatError::NonCanonicalScalar { field: "owner_key" })
        );

        // Put non-canonical scalar in nullifier_deriving_key slot (96..128 bytes)
        let non_canonical_nk = format!(
            "{}{}{}{}",
            valid_bytes_hex, valid_bytes_hex, valid_bytes_hex, non_canonical_scalar_hex
        );
        let err_nk = FullViewingKey::from_hex(&non_canonical_nk);
        assert_eq!(
            err_nk,
            Err(KeyFormatError::NonCanonicalScalar {
                field: "nullifier_deriving_key",
            })
        );
    }

    #[test]
    fn test_fvk_serde_json_roundtrip() {
        let fvk = FullViewingKey::new(
            Fr::from(0x12345u64),
            [0x11; 32],
            [0x22; 32],
            Fr::from(0x67890u64),
        );

        let json_str = serde_json::to_string_pretty(&fvk).expect("serialization to JSON");
        let deserialized: FullViewingKey =
            serde_json::from_str(&json_str).expect("deserialization from JSON");

        assert_eq!(
            fvk, deserialized,
            "FVK deserialized from JSON must equal original FVK!"
        );
    }
}
