//! EIP-2537 BLS12-381 precompiled contract addresses

use alloy_primitives::{Address, address};

// Precompiled contracts introduced by EIP-2537 in the Pectra upgrade
pub const BLS12_G1_ADD: Address = address!("000000000000000000000000000000000000000b");
pub const BLS12_G1_MSM: Address = address!("000000000000000000000000000000000000000c");
pub const BLS12_G2_MSM: Address = address!("000000000000000000000000000000000000000e");
pub const BLS12_PAIRING_CHECK: Address = address!("000000000000000000000000000000000000000f");
