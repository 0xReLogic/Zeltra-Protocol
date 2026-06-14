mod blind_wasm;
pub mod eip712;
mod evm_wasm;
mod fee_wasm;
mod threshold_wasm;
mod wasm_types;
pub mod x402;
mod zk_wasm;

pub use blind_wasm::*;
pub use eip712::*;
pub use evm_wasm::*;
pub use fee_wasm::*;
pub use threshold_wasm::*;
pub use wasm_types::*;
pub use zk_wasm::*;

/// Securely zero out memory of a value to prevent sensitive data leakage.
/// Uses volatile writes to ensure the compiler doesn't optimize it away.
pub fn secure_zeroize<T>(val: &mut T) {
    let ptr = val as *mut T as *mut u8;
    let size = std::mem::size_of::<T>();
    for i in 0..size {
        unsafe {
            std::ptr::write_volatile(ptr.add(i), 0);
        }
    }
}

/// Securely zero out a byte vector.
pub fn secure_zeroize_vec(val: &mut Vec<u8>) {
    let ptr = val.as_mut_ptr();
    for i in 0..val.len() {
        unsafe {
            std::ptr::write_volatile(ptr.add(i), 0);
        }
    }
    val.clear();
}

/// Securely zero out a string's underlying heap representation.
pub fn secure_zeroize_string(val: &mut String) {
    let bytes = unsafe { val.as_mut_vec() };
    let ptr = bytes.as_mut_ptr();
    for i in 0..bytes.len() {
        unsafe {
            std::ptr::write_volatile(ptr.add(i), 0);
        }
    }
    bytes.clear();
}

/// Computes HMAC-SHA256 of a message with a given key.
/// Implemented from scratch to avoid external dependency bloat.
pub fn hmac_sha256(key: &[u8], message: &[u8]) -> [u8; 32] {
    use sha2::{Digest, Sha256};

    let mut k_padded = [0u8; 64];
    if key.len() > 64 {
        let hash = Sha256::digest(key);
        k_padded[..32].copy_from_slice(&hash);
    } else {
        k_padded[..key.len()].copy_from_slice(key);
    }

    let mut ipad = [0x36u8; 64];
    let mut opad = [0x5cu8; 64];
    for i in 0..64 {
        ipad[i] ^= k_padded[i];
        opad[i] ^= k_padded[i];
    }

    let mut inner_hasher = Sha256::new();
    inner_hasher.update(&ipad);
    inner_hasher.update(message);
    let inner_hash = inner_hasher.finalize();

    let mut outer_hasher = Sha256::new();
    outer_hasher.update(&opad);
    outer_hasher.update(&inner_hash);
    let outer_hash = outer_hasher.finalize();

    let mut result = [0u8; 32];
    result.copy_from_slice(&outer_hash);
    result
}

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
        assert_eq!(proof.proof_b_hex().len(), 512); // 256 bytes hex
        assert_eq!(proof.proof_c_hex().len(), 256); // 128 bytes hex
        assert_eq!(proof.public_inputs_g1_hex().len(), 256); // 128 bytes hex
    }
}
