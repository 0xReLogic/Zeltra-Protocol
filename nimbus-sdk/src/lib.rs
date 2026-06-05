mod wasm_types;
mod blind_wasm;
mod evm_wasm;
mod threshold_wasm;
mod zk_wasm;
pub mod x402;

pub use wasm_types::*;
pub use blind_wasm::*;
pub use evm_wasm::*;
pub use threshold_wasm::*;
pub use zk_wasm::*;

#[cfg(test)]
mod sdk_tests {
    use super::*;

    #[test]
    fn test_client_generate_compliance_proof() {
        let root = hex::encode(vec![1u8; 32]);
        let nullifier = hex::encode(vec![2u8; 32]);
        let recipient = hex::encode(vec![3u8; 20]);
        let amount = hex::encode(vec![4u8; 32]);
        
        let proof_res = client_generate_compliance_proof(&root, &nullifier, &recipient, &amount);
        assert!(proof_res.is_ok());
        
        let proof = proof_res.unwrap();
        assert_eq!(proof.proof_a_neg_hex().len(), 256); // 128 bytes hex
        assert_eq!(proof.proof_b_hex().len(), 512);     // 256 bytes hex
        assert_eq!(proof.proof_c_hex().len(), 256);     // 128 bytes hex
        assert_eq!(proof.public_inputs_g1_hex().len(), 256); // 128 bytes hex
    }
}


