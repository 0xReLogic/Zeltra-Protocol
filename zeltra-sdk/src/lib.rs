pub mod client;
pub mod coin_selection;
pub mod compliance;
pub mod eip712;
mod fee_wasm;
pub mod storage;
pub mod wallet;
mod wasm_types;
pub mod x402;
mod zk_wasm;

pub use client::*;
pub use coin_selection::*;
pub use compliance::*;
pub use eip712::*;
pub use fee_wasm::*;
pub use storage::*;
pub use wallet::*;
pub use wasm_types::*;
pub use zk_wasm::*;

// Re-export MMB frontier increment sync structures (DEC-057)
pub use zeltra_core::{MmbFrontier, MmbIncrementProof, MmbPeak};

pub use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

/// Securely zero out memory of a value to prevent sensitive data leakage.
/// Uses volatile writes and a compiler memory barrier to prevent Dead Store Elimination (DSE).
pub fn secure_zeroize<T>(val: &mut T) {
    let ptr = val as *mut T as *mut u8;
    let size = std::mem::size_of::<T>();
    for i in 0..size {
        unsafe {
            std::ptr::write_volatile(ptr.add(i), 0);
        }
    }
    core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
}

/// Securely zero out a byte vector using audited zeroize compiler barriers.
pub fn secure_zeroize_vec(val: &mut Vec<u8>) {
    val.zeroize();
}

/// Securely zero out a string's underlying heap representation.
pub fn secure_zeroize_string(val: &mut String) {
    val.zeroize();
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
    inner_hasher.update(ipad);
    inner_hasher.update(message);
    let inner_hash = inner_hasher.finalize();

    let mut outer_hasher = Sha256::new();
    outer_hasher.update(opad);
    outer_hasher.update(inner_hash);
    let outer_hash = outer_hasher.finalize();

    let mut result = [0u8; 32];
    result.copy_from_slice(&outer_hash);
    result
}
