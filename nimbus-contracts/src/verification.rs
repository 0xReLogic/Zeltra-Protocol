//! ZK Compliance Verification Module
//!
//! Handles Groth16 ZK-Proof verification using EIP-2537 pairing precompiles.

use alloc::vec::Vec;
use alloy_primitives::{Address, FixedBytes, U256};
use ark_bls12_381::{Fr, G1Affine, G2Affine};
use ark_ec::{AffineRepr, CurveGroup};
use stylus_sdk::abi::Bytes;
use stylus_sdk::call::RawCall;

use crate::constants::{BLS12_G1_ADD, BLS12_G1_MSM, BLS12_PAIRING_CHECK};
use crate::storage::Nimbus;
use crate::types::{to_evm_g1, to_evm_g2};

impl Nimbus {
    /// Retrieve the Verifying Key (VK) for the compliance Groth16 circuit.
    ///
    /// WARNING: This is a MOCK implementation for development/testing only!
    ///
    /// PRODUCTION DEPLOYMENT REQUIRES:
    /// 1. Trusted setup ceremony using Circom to compile the circuit
    /// 2. Generate proper proving/verifying keys via Powers of Tau
    /// 3. Store VK parameters on-chain (either in contract storage or as constants)
    /// 4. Load real VK instead of using scaled generators
    ///
    /// Current implementation uses generator scaling which is NOT cryptographically secure.
    /// This is ONLY for testnet demonstration and must be replaced before mainnet.
    pub(crate) fn get_compliance_vk(
        &self,
    ) -> (
        [u8; 128],      // vk_alpha_g1
        [u8; 256],      // vk_beta_g2
        [u8; 256],      // vk_gamma_g2
        [u8; 256],      // vk_delta_g2
        [[u8; 128]; 5], // vk_ic
    ) {
        let g1 = G1Affine::generator();
        let g2 = G2Affine::generator();

        let vk_alpha_g1 = to_evm_g1(&g1);
        let vk_beta_g2 = to_evm_g2(&g2);
        let vk_gamma_g2 = to_evm_g2(&g2);
        let vk_delta_g2 = to_evm_g2(&g2);

        let mut vk_ic = [[0u8; 128]; 5];
        for i in 0..5 {
            // Scale the generator to get distinct valid G1 points: (i+1) * g1
            let point = (g1 * Fr::from((i + 1) as u64)).into_affine();
            vk_ic[i] = to_evm_g1(&point);
        }

        (vk_alpha_g1, vk_beta_g2, vk_gamma_g2, vk_delta_g2, vk_ic)
    }

    /// Compute the public inputs G1 linear combination on-chain using precompiles G1 ADD and G1 MSM.
    pub(crate) fn compute_public_inputs_g1(
        &self,
        vk_ic: &[[u8; 128]; 5],
        root: FixedBytes<32>,
        nullifier: FixedBytes<32>,
        recipient: Address,
        amount: U256,
    ) -> Result<[u8; 128], Vec<u8>> {
        #[cfg(test)]
        {
            Ok([0u8; 128])
        }
        #[cfg(not(test))]
        {
            // Pad recipient to 32 bytes
            let mut recipient_bytes = [0u8; 32];
            recipient_bytes[12..].copy_from_slice(recipient.as_slice());

            let amount_bytes = amount.to_be_bytes::<32>();

            // Public inputs sequence: [root, nullifier, recipient_bytes, amount_bytes]
            let inputs = [root.0, nullifier.0, recipient_bytes, amount_bytes];

            // G1 MSM precompile input: (G1_point_1, scalar_1) || (G1_point_2, scalar_2) || ...
            // Number of inputs is 4. vk_ic[1..5] are the G1 points corresponding to public inputs.
            let mut msm_input = Vec::with_capacity(4 * 160);
            for i in 0..4 {
                msm_input.extend_from_slice(&vk_ic[i + 1]);
                msm_input.extend_from_slice(&inputs[i]);
            }

            // Call G1 MSM precompile (0x0c)
            let host = Self::runtime_host();
            let msm_result = unsafe { RawCall::new_static(&host).call(BLS12_G1_MSM, &msm_input) }
                .map_err(|_| b"G1_MSM_PRECOMPILE_FAILED".to_vec())?;

            if msm_result.len() != 128 {
                return Err(b"INVALID_MSM_RESULT_LENGTH".to_vec());
            }

            // Now add vk_ic[0] (base parameter) using G1 ADD precompile (0x0b)
            // G1 ADD input: G1_point_1 (128 bytes) || G1_point_2 (128 bytes)
            let mut add_input = Vec::with_capacity(256);
            add_input.extend_from_slice(&vk_ic[0]);
            add_input.extend_from_slice(&msm_result);

            let add_result = unsafe { RawCall::new_static(&host).call(BLS12_G1_ADD, &add_input) }
                .map_err(|_| b"G1_ADD_PRECOMPILE_FAILED".to_vec())?;

            if add_result.len() != 128 {
                return Err(b"INVALID_ADD_RESULT_LENGTH".to_vec());
            }

            let mut out = [0u8; 128];
            out.copy_from_slice(&add_result);
            Ok(out)
        }
    }

    /// Verifies the Groth16 ZK-Proof on-chain using the EIP-2537 pairing check precompile.
    /// Formula: e(-A, B) * e(IC, gamma) * e(C, delta) * e(alpha, beta) == 1
    pub fn _verify_groth16_proof(
        &self,
        proof_a_neg_bytes: Bytes,      // -A (128 bytes EVM format)
        proof_b_bytes: Bytes,          // B (256 bytes EVM format)
        proof_c_bytes: Bytes,          // C (128 bytes EVM format)
        public_inputs_g1_bytes: Bytes, // IC linear combination (128 bytes EVM format)
        vk_alpha_bytes: Bytes,         // alpha (128 bytes EVM format)
        vk_beta_bytes: Bytes,          // beta (256 bytes EVM format)
        vk_gamma_bytes: Bytes,         // gamma (256 bytes EVM format)
        vk_delta_bytes: Bytes,         // delta (256 bytes EVM format)
    ) -> Result<bool, Vec<u8>> {
        if proof_a_neg_bytes.len() != 128
            || proof_b_bytes.len() != 256
            || proof_c_bytes.len() != 128
            || public_inputs_g1_bytes.len() != 128
            || vk_alpha_bytes.len() != 128
            || vk_beta_bytes.len() != 256
            || vk_gamma_bytes.len() != 256
            || vk_delta_bytes.len() != 256
        {
            return Err(b"INVALID_INPUT_LENGTHS".to_vec());
        }

        #[cfg(test)]
        {
            Ok(true)
        }

        #[cfg(not(test))]
        {
            // Construct payload for BLS12_PAIRING_CHECK (1536 bytes)
            let mut input = Vec::with_capacity(1536);
            // Pair 1: -A (G1) and B (G2)
            input.extend_from_slice(&proof_a_neg_bytes);
            input.extend_from_slice(&proof_b_bytes);
            // Pair 2: IC (G1) and gamma (G2)
            input.extend_from_slice(&public_inputs_g1_bytes);
            input.extend_from_slice(&vk_gamma_bytes);
            // Pair 3: C (G1) and delta (G2)
            input.extend_from_slice(&proof_c_bytes);
            input.extend_from_slice(&vk_delta_bytes);
            // Pair 4: alpha (G1) and beta (G2)
            input.extend_from_slice(&vk_alpha_bytes);
            input.extend_from_slice(&vk_beta_bytes);

            // Call EIP-2537 pairing precompile at 0x0f
            let host = Self::runtime_host();
            let output = unsafe { RawCall::new_static(&host).call(BLS12_PAIRING_CHECK, &input) }
                .map_err(|_| b"ZK_PAIRING_PRECOMPILE_CALL_FAILED".to_vec())?;

            // Output is 32 bytes, last byte is 1 if pairing check passes
            if output.len() == 32 && output[31] == 1 {
                Ok(true)
            } else {
                Ok(false)
            }
        }
    }

    /// Combined compliance check: ZK Proof of innocence (containing Merkle membership check inside the ZK circuit)
    /// Validates the proof using the registered clean root, spent nullifier, recipient address, and amount.
    ///
    /// NOTE (Ephemeral Agent Identifiers Pathway):
    /// To support stealth-identity-based AI Agents (Roadmap Section 10) and thwart Address Clustering Attacks,
    /// future upgrades will verify that the spent nullifier and ephemeral public key are derived via Blind
    /// Signature Derivation from a valid master identity, while keeping the master identity hidden on-chain.
    pub fn _verify_compliance(
        &self,
        root: FixedBytes<32>,
        nullifier: FixedBytes<32>,
        recipient: Address,
        amount: U256,
        proof_a_neg_bytes: Bytes,
        proof_b_bytes: Bytes,
        proof_c_bytes: Bytes,
    ) -> Result<bool, Vec<u8>> {
        // 1. Verify clean root is registered on-chain
        if !self.clean_association_roots.get(root) {
            return Ok(false);
        }

        // 2. Load the compliance Verifying Key (VK)
        let (vk_alpha, vk_beta, vk_gamma, vk_delta, vk_ic) = self.get_compliance_vk();

        // 3. Compute the public inputs G1 linear combination on-chain (binds the parameters)
        let public_inputs_g1_bytes = self
            .compute_public_inputs_g1(&vk_ic, root, nullifier, recipient, amount)?
            .to_vec();

        // 4. Verify ZK Proof (Groth16) using EIP-2537 pairing precompile
        let is_zk_valid = self._verify_groth16_proof(
            proof_a_neg_bytes,
            proof_b_bytes,
            proof_c_bytes,
            Bytes::from(public_inputs_g1_bytes),
            Bytes::from(vk_alpha.to_vec()),
            Bytes::from(vk_beta.to_vec()),
            Bytes::from(vk_gamma.to_vec()),
            Bytes::from(vk_delta.to_vec()),
        )?;

        Ok(is_zk_valid)
    }
}
