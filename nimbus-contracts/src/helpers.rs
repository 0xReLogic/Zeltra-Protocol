//! Private helper methods for Nimbus contract

use crate::storage::Nimbus;
use alloc::vec::Vec;
use alloy_primitives::Address;

impl Nimbus {
    #[inline(always)]
    pub(crate) fn runtime_host() -> stylus_sdk::host::WasmVM {
        stylus_sdk::host::WasmVM {}
    }

    #[inline(always)]
    pub(crate) fn msg_sender(&self) -> Address {
        #[cfg(test)]
        {
            crate::tests::MSG_SENDER.with(|s| *s.borrow())
        }
        #[cfg(not(test))]
        {
            use stylus_sdk::stylus_core::host::MessageAccess;
            Self::runtime_host().msg_sender()
        }
    }

    #[inline(always)]
    pub(crate) fn block_timestamp(&self) -> u64 {
        #[cfg(test)]
        {
            crate::tests::BLOCK_TIMESTAMP.with(|t| *t.borrow())
        }
        #[cfg(not(test))]
        {
            use stylus_sdk::stylus_core::host::BlockAccess;
            Self::runtime_host().block_timestamp()
        }
    }

    #[inline(always)]
    pub(crate) fn env_chain_id(&self) -> u64 {
        #[cfg(test)]
        {
            1337
        }
        #[cfg(not(test))]
        {
            use stylus_sdk::stylus_core::host::ChainAccess;
            Self::runtime_host().chain_id()
        }
    }

    #[inline(always)]
    pub(crate) fn env_contract_address(&self) -> Address {
        #[cfg(test)]
        {
            Address::ZERO
        }
        #[cfg(not(test))]
        {
            use stylus_sdk::stylus_core::host::AccountAccess;
            Self::runtime_host().contract_address()
        }
    }

    pub(crate) fn check_owner(&self) -> Result<(), Vec<u8>> {
        if self.owner.get() != self.msg_sender() {
            return Err(b"NOT_OWNER".to_vec());
        }
        Ok(())
    }

    pub(crate) fn check_not_paused(&self) -> Result<(), Vec<u8>> {
        if self.paused.get() {
            return Err(b"CONTRACT_PAUSED".to_vec());
        }
        Ok(())
    }
}

use ark_bls12_381::{g1::Config as G1Config, G1Affine, G1Projective};
use ark_ec::hashing::curve_maps::wb::WBMap;
use ark_ec::hashing::map_to_curve_hasher::MapToCurveBasedHasher;
use ark_ec::hashing::HashToCurve;
use ark_ff::fields::field_hashers::DefaultFieldHasher;
use sha2::Sha256;

pub fn hash_to_g1(message: &[u8]) -> G1Affine {
    let hasher = MapToCurveBasedHasher::<
        G1Projective,
        DefaultFieldHasher<Sha256, 128>,
        WBMap<G1Config>,
    >::new(b"BLS_SIG_BLS12381G1_XMD:SHA-256_SSWU_RO_NUL_")
    .unwrap();

    hasher.hash(message).unwrap()
}

use alloy_primitives::FixedBytes;
use alloy_primitives::U256;

pub fn compute_spend_hash(
    chain_id: U256,
    contract_address: Address,
    amount: U256,
    recipient_or_intent_hash: FixedBytes<32>,
    expiry: U256,
    nonce: FixedBytes<32>,
) -> [u8; 32] {
    let mut msg_bytes = Vec::with_capacity(5 + 32 + 20 + 32 + 32 + 32 + 32);
    msg_bytes.extend_from_slice(b"SPEND");
    msg_bytes.extend_from_slice(&chain_id.to_be_bytes::<32>());
    msg_bytes.extend_from_slice(contract_address.as_slice());
    msg_bytes.extend_from_slice(&amount.to_be_bytes::<32>());
    msg_bytes.extend_from_slice(recipient_or_intent_hash.as_slice());
    msg_bytes.extend_from_slice(&expiry.to_be_bytes::<32>());
    msg_bytes.extend_from_slice(nonce.as_slice());

    alloy_primitives::keccak256(&msg_bytes).0
}
