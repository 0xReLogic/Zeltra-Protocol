#![cfg_attr(all(not(feature = "export-abi"), not(test)), no_main)]
#![allow(unused_variables, dead_code, unused_imports)]
extern crate alloc;

mod types;
mod interfaces;
mod storage;
mod constants;
mod helpers;
mod verification;
mod vault;
mod deposit;
mod spend;

use alloc::vec::Vec;
use ark_bls12_381::{Fr, G1Affine, G2Affine};
use ark_ec::AffineRepr;
use ark_ff::Field;

use alloy_primitives::{keccak256, Address, FixedBytes};
use stylus_sdk::{prelude::*, alloy_primitives::U256, call::RawCall, abi::Bytes};

pub use types::{to_evm_g1, to_evm_g2, to_evm_scalar};
pub use interfaces::*;
pub use storage::Nimbus;
pub use constants::*;

#[public]
impl Nimbus {
    /// Initialize the contract and set the owner, stablecoin, and fee recipient addresses.
    pub fn init(&mut self, owner: Address, stablecoin_addr: Address, fee_recipient_addr: Address) -> Result<(), Vec<u8>> {
        if self.owner.get() != Address::ZERO {
            return Err(b"ALREADY_INITIALIZED".to_vec());
        }
        if owner == Address::ZERO {
            return Err(b"INVALID_OWNER".to_vec());
        }
        self.owner.set(owner);
        self.paused.set(false);
        self.stablecoin.set(stablecoin_addr);
        self.fee_recipient.set(fee_recipient_addr);
        self.fast_path_phase.set(U256::from(1));
        self.target_cash_pct.set(U256::from(30));
        self.epoch_start_timestamp.set(U256::from(self.block_timestamp()));
        self.current_epoch_id.set(U256::ZERO);
        self.current_epoch_volume.set(U256::ZERO);
        Ok(())
    }

    pub fn stablecoin(&self) -> Result<Address, Vec<u8>> {
        Ok(self.stablecoin.get())
    }

    pub fn fee_recipient(&self) -> Result<Address, Vec<u8>> {
        Ok(self.fee_recipient.get())
    }

    // --- Two-Step Governance (Issue 10) ---

    pub fn propose_owner(&mut self, new_owner: Address) -> Result<(), Vec<u8>> {
        self.check_owner()?;
        if new_owner == Address::ZERO {
            return Err(b"INVALID_NEW_OWNER".to_vec());
        }
        self.pending_owner.set(new_owner);
        Ok(())
    }

    pub fn claim_ownership(&mut self) -> Result<(), Vec<u8>> {
        let pending = self.pending_owner.get();
        if pending != self.msg_sender() {
            return Err(b"NOT_PENDING_OWNER".to_vec());
        }
        self.owner.set(pending);
        self.pending_owner.set(Address::ZERO);
        Ok(())
    }

    // --- Timelocked Admin Parameter Changes (Issue 10) ---

    pub fn propose_fee_recipient(&mut self, recipient: Address) -> Result<(), Vec<u8>> {
        self.check_owner()?;
        if recipient == Address::ZERO {
            return Err(b"INVALID_RECIPIENT".to_vec());
        }
        self.proposed_fee_recipient.set(recipient);
        self.fee_recipient_eta.set(U256::from(self.block_timestamp() + 86400));
        Ok(())
    }

    pub fn execute_fee_recipient(&mut self) -> Result<(), Vec<u8>> {
        self.check_owner()?;
        let eta = self.fee_recipient_eta.get();
        if eta == U256::ZERO {
            return Err(b"NO_PROPOSAL_ACTIVE".to_vec());
        }
        let current_time = U256::from(self.block_timestamp());
        if current_time < eta {
            return Err(b"TIMELOCK_NOT_EXPIRED".to_vec());
        }
        let recipient = self.proposed_fee_recipient.get();
        self.fee_recipient.set(recipient);
        self.fee_recipient_eta.set(U256::ZERO);
        Ok(())
    }

    pub fn fast_path_phase(&self) -> Result<U256, Vec<u8>> {
        Ok(self.fast_path_phase.get())
    }

    pub fn propose_fast_path_phase(&mut self, phase: U256) -> Result<(), Vec<u8>> {
        self.check_owner()?;
        if phase < U256::from(1) || phase > U256::from(3) {
            return Err(b"INVALID_PHASE".to_vec());
        }
        self.proposed_fast_path_phase.set(phase);
        self.fast_path_phase_eta.set(U256::from(self.block_timestamp() + 86400));
        Ok(())
    }

    pub fn execute_fast_path_phase(&mut self) -> Result<(), Vec<u8>> {
        self.check_owner()?;
        let eta = self.fast_path_phase_eta.get();
        if eta == U256::ZERO {
            return Err(b"NO_PROPOSAL_ACTIVE".to_vec());
        }
        let current_time = U256::from(self.block_timestamp());
        if current_time < eta {
            return Err(b"TIMELOCK_NOT_EXPIRED".to_vec());
        }
        let phase = self.proposed_fast_path_phase.get();
        self.fast_path_phase.set(phase);
        self.fast_path_phase_eta.set(U256::ZERO);
        Ok(())
    }

    pub fn total_lp_liquidity(&self) -> Result<U256, Vec<u8>> {
        Ok(self.total_lp_liquidity.get())
    }

    pub fn utilized_lp_liquidity(&self) -> Result<U256, Vec<u8>> {
        Ok(self.utilized_lp_liquidity.get())
    }

    pub fn set_lp_liquidity(&mut self, total: U256, utilized: U256) -> Result<(), Vec<u8>> {
        self.check_owner()?;
        if utilized > total {
            return Err(b"INVALID_UTILIZATION".to_vec());
        }
        self.total_lp_liquidity.set(total);
        self.utilized_lp_liquidity.set(utilized);
        Ok(())
    }

    pub fn propose_aave_params(&mut self, pool: Address, a_token: Address) -> Result<(), Vec<u8>> {
        self.check_owner()?;
        self.proposed_aave_pool.set(pool);
        self.proposed_a_token.set(a_token);
        self.aave_params_eta.set(U256::from(self.block_timestamp() + 86400));
        Ok(())
    }

    pub fn execute_aave_params(&mut self) -> Result<(), Vec<u8>> {
        self.check_owner()?;
        let eta = self.aave_params_eta.get();
        if eta == U256::ZERO {
            return Err(b"NO_PROPOSAL_ACTIVE".to_vec());
        }
        let current_time = U256::from(self.block_timestamp());
        if current_time < eta {
            return Err(b"TIMELOCK_NOT_EXPIRED".to_vec());
        }
        let pool = self.proposed_aave_pool.get();
        let a_token = self.proposed_a_token.get();
        self.aave_pool.set(pool);
        self.a_token.set(a_token);
        self.aave_params_eta.set(U256::ZERO);
        Ok(())
    }

    pub fn propose_rwa_token(&mut self, rwa: Address) -> Result<(), Vec<u8>> {
        self.check_owner()?;
        self.proposed_rwa_token.set(rwa);
        self.rwa_token_eta.set(U256::from(self.block_timestamp() + 86400));
        Ok(())
    }

    pub fn execute_rwa_token(&mut self) -> Result<(), Vec<u8>> {
        self.check_owner()?;
        let eta = self.rwa_token_eta.get();
        if eta == U256::ZERO {
            return Err(b"NO_PROPOSAL_ACTIVE".to_vec());
        }
        let current_time = U256::from(self.block_timestamp());
        if current_time < eta {
            return Err(b"TIMELOCK_NOT_EXPIRED".to_vec());
        }
        let rwa = self.proposed_rwa_token.get();
        self.rwa_token.set(rwa);
        self.rwa_token_eta.set(U256::ZERO);
        Ok(())
    }

    pub fn aave_pool(&self) -> Result<Address, Vec<u8>> {
        Ok(self.aave_pool.get())
    }

    pub fn a_token(&self) -> Result<Address, Vec<u8>> {
        Ok(self.a_token.get())
    }

    pub fn rwa_token(&self) -> Result<Address, Vec<u8>> {
        Ok(self.rwa_token.get())
    }

    pub fn total_deposited_principal(&self) -> Result<U256, Vec<u8>> {
        Ok(self.total_deposited_principal.get())
    }

    /// Returns the current dynamic cash reserve percentage (range 15-45, default 30).
    pub fn target_cash_pct(&self) -> Result<U256, Vec<u8>> {
        Ok(self.target_cash_pct.get())
    }

    /// Returns the current epoch ID (increments every 24h).
    pub fn current_epoch_id(&self) -> Result<U256, Vec<u8>> {
        Ok(self.current_epoch_id.get())
    }

    /// Returns the accumulated transaction volume in the current epoch.
    pub fn current_epoch_volume(&self) -> Result<U256, Vec<u8>> {
        Ok(self.current_epoch_volume.get())
    }

    /// Returns the historical volume for a given epoch ID.
    pub fn historical_epoch_volume(&self, epoch_id: U256) -> Result<U256, Vec<u8>> {
        Ok(self.historical_epoch_volumes.get(epoch_id))
    }

    /// Returns the CCIP Router address configured for the contract.
    pub fn ccip_router(&self) -> Result<Address, Vec<u8>> {
        Ok(self.ccip_router.get())
    }

    /// Sets the CCIP Router address (Admin only).
    pub fn set_ccip_router(&mut self, router: Address) -> Result<(), Vec<u8>> {
        self.check_owner()?;
        self.ccip_router.set(router);
        Ok(())
    }

    /// Pause the contract.
    pub fn pause(&mut self) -> Result<(), Vec<u8>> {
        self.check_owner()?;
        self.paused.set(true);
        Ok(())
    }

    /// Unpause the contract.
    pub fn unpause(&mut self) -> Result<(), Vec<u8>> {
        self.check_owner()?;
        self.paused.set(false);
        Ok(())
    }

    /// Registers a new clean association set Merkle root (Admin/Compliance Oracle).
    pub fn register_clean_root(&mut self, root: FixedBytes<32>) -> Result<(), Vec<u8>> {
        self.check_not_paused()?;
        self.check_owner()?;
        self.clean_association_roots.insert(root, true);
        Ok(())
    }

    pub fn register_issuer_key(&mut self, pk_iss_bytes: Bytes) -> Result<(), Vec<u8>> {
        self.check_owner()?;
        if pk_iss_bytes.len() != 256 {
            return Err(b"INVALID_PUBLIC_KEY_LENGTH".to_vec());
        }
        if pk_iss_bytes.iter().all(|byte| *byte == 0) {
            return Err(b"POINT_AT_INFINITY_NOT_ALLOWED".to_vec());
        }
        self.trusted_issuer_keys.insert(keccak256(&pk_iss_bytes), true);
        Ok(())
    }

    pub fn revoke_issuer_key(&mut self, pk_iss_bytes: Bytes) -> Result<(), Vec<u8>> {
        self.check_owner()?;
        if pk_iss_bytes.len() != 256 {
            return Err(b"INVALID_PUBLIC_KEY_LENGTH".to_vec());
        }
        self.trusted_issuer_keys.insert(keccak256(&pk_iss_bytes), false);
        Ok(())
    }

    pub fn is_issuer_key_trusted(&self, pk_iss_bytes: Bytes) -> Result<bool, Vec<u8>> {
        if pk_iss_bytes.len() != 256 {
            return Err(b"INVALID_PUBLIC_KEY_LENGTH".to_vec());
        }
        Ok(self.trusted_issuer_keys.get(keccak256(&pk_iss_bytes)))
    }

    // --- EVM Public Delegates to Modular Component Implementations ---

    pub fn deposit(&mut self, sid: FixedBytes<32>, com_k_bytes: Bytes, amount: U256) -> Result<(), Vec<u8>> {
        self._deposit(sid, com_k_bytes, amount)
    }

    pub fn reveal_mask_key(
        &mut self,
        sid: FixedBytes<32>,
        k_bytes: Bytes,
        pk_iss_bytes: Bytes,
        com_k_bytes: Bytes,
    ) -> Result<bool, Vec<u8>> {
        self._reveal_mask_key(sid, k_bytes, pk_iss_bytes, com_k_bytes)
    }

    pub fn claim_refund(&mut self, sid: FixedBytes<32>) -> Result<(), Vec<u8>> {
        self._claim_refund(sid)
    }

    pub fn spend(
        &mut self,
        nullifier: FixedBytes<32>,
        alpha_neg_bytes: Bytes,
        pk_iss_bytes: Bytes,
        recipient: Address,
        amount: U256,
        recipient_or_intent_hash: FixedBytes<32>,
        expiry: U256,
        nonce: FixedBytes<32>,
    ) -> Result<bool, Vec<u8>> {
        self._spend(
            nullifier,
            alpha_neg_bytes,
            pk_iss_bytes,
            recipient,
            amount,
            recipient_or_intent_hash,
            expiry,
            nonce,
        )
    }

    pub fn spend_and_buy_shares(
        &mut self,
        nullifier: FixedBytes<32>,
        alpha_neg_bytes: Bytes,
        pk_iss_bytes: Bytes,
        polymarket_ctf: Address,
        collateral_token: Address,
        condition_id: FixedBytes<32>,
        amount: U256,
        expiry: U256,
        nonce: FixedBytes<32>,
    ) -> Result<bool, Vec<u8>> {
        self._spend_and_buy_shares(
            nullifier,
            alpha_neg_bytes,
            pk_iss_bytes,
            polymarket_ctf,
            collateral_token,
            condition_id,
            amount,
            expiry,
            nonce,
        )
    }

    pub fn get_failed_intent_refund(&self, nullifier: FixedBytes<32>) -> Result<U256, Vec<u8>> {
        self._get_failed_intent_refund(nullifier)
    }

    pub fn claim_failed_intent_refund(
        &mut self,
        nullifier: FixedBytes<32>,
        recipient: Address,
    ) -> Result<bool, Vec<u8>> {
        self._claim_failed_intent_refund(nullifier, recipient)
    }

    pub fn ccip_receive(
        &mut self,
        message_id: FixedBytes<32>,
        source_chain_selector: u64,
        sender: Bytes,
        payload: Bytes,
    ) -> Result<(), Vec<u8>> {
        self._ccip_receive(message_id, source_chain_selector, sender, payload)
    }

    pub fn verify_groth16_proof(
        &self,
        proof_a_neg_bytes: Bytes,
        proof_b_bytes: Bytes,
        proof_c_bytes: Bytes,
        public_inputs_g1_bytes: Bytes,
        vk_alpha_bytes: Bytes,
        vk_beta_bytes: Bytes,
        vk_gamma_bytes: Bytes,
        vk_delta_bytes: Bytes,
    ) -> Result<bool, Vec<u8>> {
        self._verify_groth16_proof(
            proof_a_neg_bytes,
            proof_b_bytes,
            proof_c_bytes,
            public_inputs_g1_bytes,
            vk_alpha_bytes,
            vk_beta_bytes,
            vk_gamma_bytes,
            vk_delta_bytes,
        )
    }

    pub fn verify_compliance(
        &self,
        root: FixedBytes<32>,
        nullifier: FixedBytes<32>,
        recipient: Address,
        amount: U256,
        proof_a_neg_bytes: Bytes,
        proof_b_bytes: Bytes,
        proof_c_bytes: Bytes,
    ) -> Result<bool, Vec<u8>> {
        self._verify_compliance(
            root,
            nullifier,
            recipient,
            amount,
            proof_a_neg_bytes,
            proof_b_bytes,
            proof_c_bytes,
        )
    }

    pub fn calculate_fast_path_premium(&self, amount: U256) -> Result<U256, Vec<u8>> {
        self._calculate_fast_path_premium(amount)
    }

    pub fn total_assets(&mut self) -> Result<U256, Vec<u8>> {
        self._total_assets()
    }

    pub fn claim_accumulated_yield(&mut self) -> Result<U256, Vec<u8>> {
        self._claim_accumulated_yield()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ark_bls12_381::{G1Affine, G2Affine, Fr};
    use ark_ff::UniformRand;
    use ark_serialize::{CanonicalDeserialize, CanonicalSerialize};
    use rand::thread_rng;
    use alloy_primitives::address;

    // --- Mock Stylus HostIO Symbols to satisfy the linker during host tests ---
    use std::cell::RefCell;
    use std::collections::HashMap;

    thread_local! {
        pub(crate) static STORAGE: RefCell<HashMap<[u8; 32], [u8; 32]>> = RefCell::new(HashMap::new());
        pub(crate) static MSG_SENDER: RefCell<Address> = RefCell::new(Address::ZERO);
        pub(crate) static BLOCK_TIMESTAMP: RefCell<u64> = RefCell::new(0);
        static PAIRING_RESULT: RefCell<u8> = RefCell::new(1);
    }

    fn reset_test_state() {
        STORAGE.with(|s| s.borrow_mut().clear());
        MSG_SENDER.with(|s| *s.borrow_mut() = Address::ZERO);
        BLOCK_TIMESTAMP.with(|t| *t.borrow_mut() = 0);
        PAIRING_RESULT.with(|result| *result.borrow_mut() = 1);
    }

    fn set_msg_sender(sender: Address) {
        MSG_SENDER.with(|s| *s.borrow_mut() = sender);
    }

    fn set_block_timestamp(ts: u64) {
        BLOCK_TIMESTAMP.with(|t| *t.borrow_mut() = ts);
    }

    fn set_pairing_result(result: bool) {
        PAIRING_RESULT.with(|value| *value.borrow_mut() = u8::from(result));
    }

    fn register_mock_issuer(
        contract: &mut Nimbus,
        owner: Address,
        amount: U256,
        recipient_or_intent_hash: FixedBytes<32>,
    ) -> (Vec<u8>, Vec<u8>, Vec<u8>, FixedBytes<32>) {
        let alpha_neg = vec![0x11; 128];
        let pk_iss = vec![0x33; 256];
        set_msg_sender(owner);
        contract.register_issuer_key(pk_iss.clone().into()).unwrap();

        let chain_id = U256::from(1337);
        let contract_address = Address::ZERO;
        let expiry = U256::ZERO;
        let nonce = FixedBytes::ZERO;

        let m_hash = helpers::compute_spend_hash(
            chain_id,
            contract_address,
            amount,
            recipient_or_intent_hash,
            expiry,
            nonce,
        );
        let hm_affine = helpers::hash_to_g1(&m_hash);
        let hm_evm_bytes = types::to_evm_g1(&hm_affine);
        let nullifier = keccak256(&hm_evm_bytes);

        (alpha_neg, hm_evm_bytes.to_vec(), pk_iss, nullifier)
    }

    #[no_mangle]
    pub unsafe extern "C" fn msg_sender(dest: *mut u8) {
        let dest_slice = std::slice::from_raw_parts_mut(dest, 20);
        MSG_SENDER.with(|sender| {
            dest_slice.copy_from_slice(sender.borrow().as_slice());
        });
    }

    #[no_mangle]
    pub unsafe extern "C" fn contract_address(dest: *mut u8) {
        let dest_slice = std::slice::from_raw_parts_mut(dest, 20);
        dest_slice.copy_from_slice(&[5u8; 20]);
    }

    #[no_mangle]
    pub unsafe extern "C" fn block_timestamp() -> u64 {
        BLOCK_TIMESTAMP.with(|ts| {
            *ts.borrow()
        })
    }

    #[no_mangle]
    pub unsafe extern "C" fn storage_load_bytes32(key: *const u8, dest: *mut u8) {
        let key_slice = std::slice::from_raw_parts(key, 32);
        let dest_slice = std::slice::from_raw_parts_mut(dest, 32);
        let mut k = [0u8; 32];
        k.copy_from_slice(key_slice);
        
        STORAGE.with(|storage| {
            if let Some(val) = storage.borrow().get(&k) {
                dest_slice.copy_from_slice(val);
            } else {
                for byte in dest_slice.iter_mut() {
                    *byte = 0;
                }
            }
        });
    }

    #[no_mangle]
    pub unsafe extern "C" fn storage_cache_bytes32(key: *const u8, src: *const u8) {
        let key_slice = std::slice::from_raw_parts(key, 32);
        let src_slice = std::slice::from_raw_parts(src, 32);
        let mut k = [0u8; 32];
        k.copy_from_slice(key_slice);
        let mut v = [0u8; 32];
        v.copy_from_slice(src_slice);
        
        STORAGE.with(|storage| {
            storage.borrow_mut().insert(k, v);
        });
    }

    #[no_mangle]
    pub unsafe extern "C" fn native_keccak256(bytes: *const u8, len: usize, output: *mut u8) {
        use tiny_keccak::{Hasher, Keccak};
        let slice = std::slice::from_raw_parts(bytes, len);
        let mut hasher = Keccak::v256();
        hasher.update(slice);
        let out_slice = std::slice::from_raw_parts_mut(output, 32);
        hasher.finalize(out_slice);
    }

    #[no_mangle]
    pub unsafe extern "C" fn call_contract(
        _contract: *const u8,
        _calldata: *const u8,
        _calldata_len: usize,
        _value: *const u8,
        _gas: u64,
        _return_data_len: *mut usize,
    ) -> u8 {
        0
    }

    #[no_mangle]
    pub unsafe extern "C" fn delegate_call_contract(
        _contract: *const u8,
        _calldata: *const u8,
        _calldata_len: usize,
        _gas: u64,
        _return_data_len: *mut usize,
    ) -> u8 {
        0
    }

    #[no_mangle]
    pub unsafe extern "C" fn static_call_contract(
        _contract: *const u8,
        _calldata: *const u8,
        _calldata_len: usize,
        _gas: u64,
        return_data_len: *mut usize,
    ) -> u8 {
        if !return_data_len.is_null() {
            *return_data_len = 32;
        }
        0 // Status code 0 is Success
    }

    #[no_mangle]
    pub unsafe extern "C" fn storage_flush_cache() {}

    #[no_mangle]
    pub unsafe extern "C" fn return_data_size() -> usize {
        32
    }

    #[no_mangle]
    pub unsafe extern "C" fn read_return_data(
        dest: *mut u8,
        _offset: usize,
        size: usize,
    ) -> usize {
        let dest_slice = std::slice::from_raw_parts_mut(dest, size);
        for byte in dest_slice.iter_mut() {
            *byte = 0;
        }
        if size == 32 {
            PAIRING_RESULT.with(|result| dest_slice[31] = *result.borrow());
        }
        size
    }

    #[test]
    fn test_to_evm_g1() {
        let mut rng = thread_rng();
        let g1 = G1Affine::rand(&mut rng);
        let evm_g1 = to_evm_g1(&g1);
        
        assert_eq!(evm_g1.len(), 128);
        for i in 0..16 {
            assert_eq!(evm_g1[i], 0);
            assert_eq!(evm_g1[64 + i], 0);
        }
    }

    #[test]
    fn test_to_evm_g2() {
        let mut rng = thread_rng();
        let g2 = G2Affine::rand(&mut rng);
        let evm_g2 = to_evm_g2(&g2);
        
        assert_eq!(evm_g2.len(), 256);
        for i in 0..16 {
            assert_eq!(evm_g2[i], 0);
            assert_eq!(evm_g2[64 + i], 0);
            assert_eq!(evm_g2[128 + i], 0);
            assert_eq!(evm_g2[192 + i], 0);
        }
    }

    #[test]
    fn test_to_evm_scalar() {
        let mut rng = thread_rng();
        let scalar = Fr::rand(&mut rng);
        let evm_scalar = to_evm_scalar(&scalar);
        assert_eq!(evm_scalar.len(), 32);
    }





    #[test]
    fn test_groth16_input_length_validation() {
        let nimbus_contract = Nimbus::default();
        let result = nimbus_contract.verify_groth16_proof(
            vec![0; 100].into(),
            vec![0; 256].into(),
            vec![0; 128].into(),
            vec![0; 128].into(),
            vec![0; 128].into(),
            vec![0; 256].into(),
            vec![0; 256].into(),
            vec![0; 256].into(),
        );
        assert_eq!(result, Err(b"INVALID_INPUT_LENGTHS".to_vec()));
    }

    #[test]
    fn test_mocked_groth16_verification() {
        let nimbus_contract = Nimbus::default();
        // Since we stubbed static_call_contract, return_data_size and read_return_data to return success,
        // this test should successfully verify the Groth16 proof with valid lengths!
        let is_valid = nimbus_contract.verify_groth16_proof(
            vec![0; 128].into(),
            vec![0; 256].into(),
            vec![0; 128].into(),
            vec![0; 128].into(),
            vec![0; 128].into(),
            vec![0; 256].into(),
            vec![0; 256].into(),
            vec![0; 256].into(),
        ).unwrap();
        assert!(is_valid, "Mocked Groth16 proof verification failed!");
    }

    #[test]
    fn test_ccip_receive_decoding_and_execution() {
        reset_test_state();
        let owner = address!("1111111111111111111111111111111111111111");
        set_msg_sender(owner);
        let mut nimbus_contract = Nimbus::default();
        nimbus_contract
            .init(owner, Address::ZERO, Address::ZERO)
            .unwrap();
        nimbus_contract
            .deposit(
                FixedBytes::repeat_byte(0x66),
                vec![0x42; 256].into(),
                U256::from(20_000_000),
            )
            .unwrap();
        let amount = U256::from(10_000_000);
        let (alpha_neg, hm_evm_bytes, pk_iss, nullifier) =
            register_mock_issuer(&mut nimbus_contract, owner, amount, FixedBytes::ZERO);

        let mut payload = vec![0u8; 584];
        payload[0..32].copy_from_slice(nullifier.as_slice());
        payload[32..160].copy_from_slice(&alpha_neg);
        payload[160..416].copy_from_slice(&pk_iss);
        // payload[416..436] is polymarket_ctf (zeros)
        // payload[436..456] is collateral_token (zeros)
        // payload[456..488] is condition_id (zeros)
        payload[488..520].copy_from_slice(&amount.to_be_bytes::<32>());
        // payload[520..552] is expiry (zeros)
        // payload[552..584] is nonce (zeros)

        let result = nimbus_contract.ccip_receive(
            FixedBytes::ZERO,
            1,
            vec![].into(),
            payload.into(),
        );
        match result {
            Ok(_) => {}
            Err(e) => {
                panic!("ccip_receive returned error: {:?}", String::from_utf8_lossy(&e));
            }
        }
    }

    #[test]
    fn test_initialization() {
        reset_test_state();
        let owner = address!("1111111111111111111111111111111111111111");
        set_msg_sender(owner);
        
        let mut contract = Nimbus::default();
        // Init with ZERO owner should fail
        assert_eq!(contract.init(Address::ZERO, Address::ZERO, Address::ZERO), Err(b"INVALID_OWNER".to_vec()));

        // First init should succeed
        assert!(contract.init(owner, Address::ZERO, Address::ZERO).is_ok());
        
        // Second init should fail
        assert_eq!(contract.init(owner, Address::ZERO, Address::ZERO), Err(b"ALREADY_INITIALIZED".to_vec()));
    }

    #[test]
    fn test_pause_unpause() {
        reset_test_state();
        let owner = address!("1111111111111111111111111111111111111111");
        let non_owner = address!("2222222222222222222222222222222222222222");
        
        set_msg_sender(owner);
        let mut contract = Nimbus::default();
        contract.init(owner, Address::ZERO, Address::ZERO).unwrap();
        
        // Non-owner pausing should fail
        set_msg_sender(non_owner);
        assert_eq!(contract.pause(), Err(b"NOT_OWNER".to_vec()));
        
        // Owner pausing should succeed
        set_msg_sender(owner);
        assert!(contract.pause().is_ok());
        assert_eq!(contract.paused.get(), true);
        
        // Non-owner unpausing should fail
        set_msg_sender(non_owner);
        assert_eq!(contract.unpause(), Err(b"NOT_OWNER".to_vec()));
        
        // Owner unpausing should succeed
        set_msg_sender(owner);
        assert!(contract.unpause().is_ok());
        assert_eq!(contract.paused.get(), false);
    }

    #[test]
    fn test_guarded_functions_fail_when_paused() {
        reset_test_state();
        let owner = address!("1111111111111111111111111111111111111111");
        set_msg_sender(owner);
        
        let mut contract = Nimbus::default();
        contract.init(owner, Address::ZERO, Address::ZERO).unwrap();
        contract.pause().unwrap();
        
        // Try guarded operations
        let sid = FixedBytes::ZERO;
        assert_eq!(
            contract.deposit(sid, vec![].into(), U256::from(10_000_000)),
            Err(b"CONTRACT_PAUSED".to_vec())
        );
        assert_eq!(
            contract.reveal_mask_key(sid, vec![].into(), vec![].into(), vec![].into()),
            Err(b"CONTRACT_PAUSED".to_vec())
        );
        assert_eq!(
            contract.spend(
                sid,
                vec![].into(),
                vec![].into(),
                Address::ZERO,
                U256::ZERO,
                FixedBytes::ZERO,
                U256::ZERO,
                FixedBytes::ZERO,
            ),
            Err(b"CONTRACT_PAUSED".to_vec())
        );
        assert_eq!(
            contract.spend_and_buy_shares(
                sid,
                vec![].into(),
                vec![].into(),
                Address::ZERO,
                Address::ZERO,
                FixedBytes::ZERO,
                U256::ZERO,
                U256::ZERO,
                FixedBytes::ZERO,
            ),
            Err(b"CONTRACT_PAUSED".to_vec())
        );

        assert_eq!(
            contract.register_clean_root(FixedBytes::ZERO),
            Err(b"CONTRACT_PAUSED".to_vec())
        );
        assert_eq!(
            contract.claim_refund(sid),
            Err(b"CONTRACT_PAUSED".to_vec())
        );
    }

    #[test]
    fn test_claim_refund_timelock() {
        reset_test_state();
        let owner = address!("1111111111111111111111111111111111111111");
        let client = address!("2222222222222222222222222222222222222222");
        let other = address!("3333333333333333333333333333333333333333");
        
        set_msg_sender(owner);
        let mut contract = Nimbus::default();
        contract.init(owner, Address::ZERO, Address::ZERO).unwrap();
        
        let sid = FixedBytes::repeat_byte(0xab);
        
        // Deposit (must be >= 10_000_000)
        set_msg_sender(client);
        set_block_timestamp(1000);
        contract
            .deposit(sid, vec![0x42; 256].into(), U256::from(10_000_000))
            .unwrap();
        
        // Claim refund from other address should fail
        set_msg_sender(other);
        assert_eq!(contract.claim_refund(sid), Err(b"NOT_SESSION_CLIENT".to_vec()));
        
        // Claim refund from client before timelock (24 hours = 86400 secs)
        set_msg_sender(client);
        set_block_timestamp(1000 + 86399); // 1 second before expiry
        assert_eq!(contract.claim_refund(sid), Err(b"TIMELOCK_NOT_EXPIRED".to_vec()));
        
        // Claim refund at expiry should succeed
        set_block_timestamp(1000 + 86400);
        assert!(contract.claim_refund(sid).is_ok());
        
        // Claiming again should fail
        assert_eq!(contract.claim_refund(sid), Err(b"SESSION_ALREADY_RESOLVED".to_vec()));
    }

    #[test]
    fn test_deposit_binds_commitment_and_rejects_duplicate_session() {
        reset_test_state();
        let owner = address!("1111111111111111111111111111111111111111");
        let client = address!("2222222222222222222222222222222222222222");

        set_msg_sender(owner);
        let mut contract = Nimbus::default();
        contract.init(owner, Address::ZERO, Address::ZERO).unwrap();

        let sid = FixedBytes::repeat_byte(0xac);
        let commitment = vec![0x42; 256];
        set_msg_sender(client);

        assert_eq!(
            contract.deposit(sid, vec![0x42; 255].into(), U256::from(10_000_000)),
            Err(b"INVALID_COMMITMENT_LENGTH".to_vec())
        );
        contract
            .deposit(
                sid,
                commitment.clone().into(),
                U256::from(10_000_000),
            )
            .unwrap();
        assert_eq!(
            contract.deposit(sid, commitment.into(), U256::from(10_000_000)),
            Err(b"SESSION_ALREADY_EXISTS".to_vec())
        );
    }

    #[test]
    fn test_reveal_cannot_release_collateral_or_change_commitment() {
        reset_test_state();
        let owner = address!("1111111111111111111111111111111111111111");
        let client = address!("2222222222222222222222222222222222222222");
        let guardian = address!("3333333333333333333333333333333333333333");

        set_msg_sender(owner);
        let mut contract = Nimbus::default();
        contract.init(owner, Address::ZERO, Address::ZERO).unwrap();

        let sid = FixedBytes::repeat_byte(0xad);
        let commitment = vec![0x42; 256];
        set_msg_sender(client);
        contract
            .deposit(
                sid,
                commitment.clone().into(),
                U256::from(10_000_000),
            )
            .unwrap();
        let principal = contract.total_deposited_principal().unwrap();

        set_msg_sender(guardian);
        assert_eq!(
            contract.reveal_mask_key(
                sid,
                vec![0x11; 32].into(),
                vec![0x22; 256].into(),
                vec![0x43; 256].into(),
            ),
            Err(b"COMMITMENT_MISMATCH".to_vec())
        );
        assert_eq!(contract.total_deposited_principal().unwrap(), principal);
        assert!(!contract.session_resolved.get(sid));

        assert!(contract
            .reveal_mask_key(
                sid,
                vec![0x11; 32].into(),
                vec![0x22; 256].into(),
                commitment.into(),
            )
            .unwrap());
        assert!(contract.session_resolved.get(sid));
        assert_eq!(contract.total_deposited_principal().unwrap(), principal);

        set_msg_sender(client);
        assert_eq!(
            contract.claim_refund(sid),
            Err(b"SESSION_ALREADY_RESOLVED".to_vec())
        );
    }

    #[test]
    fn test_register_clean_root_authorization() {
        reset_test_state();
        let owner = address!("1111111111111111111111111111111111111111");
        let non_owner = address!("2222222222222222222222222222222222222222");
        
        set_msg_sender(owner);
        let mut contract = Nimbus::default();
        contract.init(owner, Address::ZERO, Address::ZERO).unwrap();
        
        // Non-owner should not be able to register clean root
        set_msg_sender(non_owner);
        assert_eq!(contract.register_clean_root(FixedBytes::ZERO), Err(b"NOT_OWNER".to_vec()));
        
        // Owner should be able to register clean root
        set_msg_sender(owner);
        assert!(contract.register_clean_root(FixedBytes::ZERO).is_ok());
    }

    #[test]
    fn test_issuer_key_registration_authorization_and_revocation() {
        reset_test_state();
        let owner = address!("1111111111111111111111111111111111111111");
        let other = address!("2222222222222222222222222222222222222222");
        let pk_iss = vec![0x33; 256];

        set_msg_sender(owner);
        let mut contract = Nimbus::default();
        contract.init(owner, Address::ZERO, Address::ZERO).unwrap();

        set_msg_sender(other);
        assert_eq!(
            contract.register_issuer_key(pk_iss.clone().into()),
            Err(b"NOT_OWNER".to_vec())
        );

        set_msg_sender(owner);
        assert_eq!(
            contract.register_issuer_key(vec![0x33; 255].into()),
            Err(b"INVALID_PUBLIC_KEY_LENGTH".to_vec())
        );
        assert_eq!(
            contract.register_issuer_key(vec![0; 256].into()),
            Err(b"POINT_AT_INFINITY_NOT_ALLOWED".to_vec())
        );
        contract.register_issuer_key(pk_iss.clone().into()).unwrap();
        assert!(contract.is_issuer_key_trusted(pk_iss.clone().into()).unwrap());
        contract.revoke_issuer_key(pk_iss.clone().into()).unwrap();
        assert!(!contract.is_issuer_key_trusted(pk_iss.into()).unwrap());
    }

    #[test]
    fn test_spend_rejects_untrusted_key_and_message_replay() {
        reset_test_state();
        let owner = address!("1111111111111111111111111111111111111111");
        set_msg_sender(owner);
        let mut contract = Nimbus::default();
        contract.init(owner, Address::ZERO, Address::ZERO).unwrap();

        let alpha_neg = vec![0x11; 128];
        let pk_iss = vec![0x33; 256];
        
        let amount = U256::from(5_000_000);
        let recipient_or_intent_hash = FixedBytes::ZERO;
        let expiry = U256::ZERO;
        let nonce = FixedBytes::ZERO;
        
        let chain_id = U256::from(1337);
        let contract_address = Address::ZERO;
        
        let m_hash = helpers::compute_spend_hash(
            chain_id,
            contract_address,
            amount,
            recipient_or_intent_hash,
            expiry,
            nonce,
        );
        let hm_affine = helpers::hash_to_g1(&m_hash);
        let hm_evm_bytes = types::to_evm_g1(&hm_affine);
        let nullifier = keccak256(&hm_evm_bytes);

        assert_eq!(
            contract.spend(
                nullifier,
                alpha_neg.clone().into(),
                pk_iss.clone().into(),
                Address::ZERO,
                amount,
                recipient_or_intent_hash,
                expiry,
                nonce,
            ),
            Err(b"UNTRUSTED_ISSUER_KEY".to_vec())
        );

        contract.register_issuer_key(pk_iss.clone().into()).unwrap();
        assert_eq!(
            contract.spend(
                FixedBytes::repeat_byte(0x99),
                alpha_neg.into(),
                pk_iss.into(),
                Address::ZERO,
                amount,
                recipient_or_intent_hash,
                expiry,
                nonce,
            ),
            Err(b"NULLIFIER_MESSAGE_MISMATCH".to_vec())
        );
        assert!(!contract.nullifiers.get(nullifier));
    }

    #[test]
    fn test_spend_rejects_malformed_infinity_and_false_pairing() {
        reset_test_state();
        let owner = address!("1111111111111111111111111111111111111111");
        set_msg_sender(owner);
        let mut contract = Nimbus::default();
        contract.init(owner, Address::ZERO, Address::ZERO).unwrap();
        let (alpha_neg, hm, pk_iss, nullifier) = register_mock_issuer(&mut contract, owner, U256::from(5_000_000), FixedBytes::ZERO);

        assert_eq!(
            contract.spend(
                nullifier,
                vec![0x11; 127].into(),
                pk_iss.clone().into(),
                Address::ZERO,
                U256::from(5_000_000),
                FixedBytes::ZERO,
                U256::ZERO,
                FixedBytes::ZERO,
            ),
            Err(b"INVALID_G1_INPUT_LENGTH".to_vec())
        );

        let infinity_alpha = vec![0; 128];
        assert_eq!(
            contract.spend(
                nullifier,
                infinity_alpha.into(),
                pk_iss.clone().into(),
                Address::ZERO,
                U256::from(5_000_000),
                FixedBytes::ZERO,
                U256::ZERO,
                FixedBytes::ZERO,
            ),
            Err(b"POINT_AT_INFINITY_NOT_ALLOWED".to_vec())
        );

        set_pairing_result(false);
        assert!(!contract
            .spend(
                nullifier,
                alpha_neg.into(),
                pk_iss.into(),
                Address::ZERO,
                U256::from(5_000_000),
                FixedBytes::ZERO,
                U256::ZERO,
                FixedBytes::ZERO,
            )
            .unwrap());
        assert!(!contract.nullifiers.get(nullifier));
        assert_eq!(contract.total_deposited_principal().unwrap(), U256::ZERO);
    }

    #[test]
    fn test_spend_replay_and_insufficient_principal_preserve_state() {
        reset_test_state();
        let owner = address!("1111111111111111111111111111111111111111");
        set_msg_sender(owner);
        let mut contract = Nimbus::default();
        contract.init(owner, Address::ZERO, Address::ZERO).unwrap();
        let (alpha_neg, hm, pk_iss, nullifier) = register_mock_issuer(&mut contract, owner, U256::from(5_000_000), FixedBytes::ZERO);

        assert_eq!(
            contract.spend(
                nullifier,
                alpha_neg.clone().into(),
                pk_iss.clone().into(),
                Address::ZERO,
                U256::from(5_000_000),
                FixedBytes::ZERO,
                U256::ZERO,
                FixedBytes::ZERO,
            ),
            Err(b"INSUFFICIENT_PRINCIPAL".to_vec())
        );
        assert!(!contract.nullifiers.get(nullifier));

        contract
            .deposit(
                FixedBytes::repeat_byte(0x77),
                vec![0x42; 256].into(),
                U256::from(10_000_000),
            )
            .unwrap();
        assert!(contract
            .spend(
                nullifier,
                alpha_neg.clone().into(),
                pk_iss.clone().into(),
                Address::ZERO,
                U256::from(5_000_000),
                FixedBytes::ZERO,
                U256::ZERO,
                FixedBytes::ZERO,
            )
            .unwrap());
        let principal = contract.total_deposited_principal().unwrap();
        assert!(!contract
            .spend(
                nullifier,
                alpha_neg.into(),
                pk_iss.into(),
                Address::ZERO,
                U256::from(5_000_000),
                FixedBytes::ZERO,
                U256::ZERO,
                FixedBytes::ZERO,
            )
            .unwrap());
        assert_eq!(contract.total_deposited_principal().unwrap(), principal);
    }

    #[test]
    fn test_fast_path_liquidity_premium_phases() {
        reset_test_state();
        let owner = address!("1111111111111111111111111111111111111111");
        set_msg_sender(owner);
        
        let mut contract = Nimbus::default();
        contract.init(owner, Address::ZERO, Address::ZERO).unwrap();
        
        // Default phase should be 1
        assert_eq!(contract.fast_path_phase().unwrap(), U256::from(1));
        
        // Fase 1: Premium is always 0
        let amount = U256::from(10_000_000); // 10,000,000 (10 USDC)
        assert_eq!(contract.calculate_fast_path_premium(amount).unwrap(), U256::ZERO);
        
        // Change to Fase 2 via propose & execute
        contract.propose_fast_path_phase(U256::from(2)).unwrap();
        set_block_timestamp(86401);
        contract.execute_fast_path_phase().unwrap();
        assert_eq!(contract.fast_path_phase().unwrap(), U256::from(2));
        // Fase 2: 0.05% flat premium => 10,000,000 * 5 / 10,000 = 5000
        assert_eq!(contract.calculate_fast_path_premium(amount).unwrap(), U256::from(5000));
        
        // Change to Fase 3 via propose & execute
        contract.propose_fast_path_phase(U256::from(3)).unwrap();
        set_block_timestamp(86401 * 2);
        contract.execute_fast_path_phase().unwrap();
        assert_eq!(contract.fast_path_phase().unwrap(), U256::from(3));
        
        // Fase 3 with zero liquidity should fail
        assert!(contract.calculate_fast_path_premium(amount).is_err());
        
        // Set LP liquidity: total = 100,000,000 (100 USDC), utilized = 0
        contract.set_lp_liquidity(U256::from(100000000), U256::from(0)).unwrap();
        // New utilization after adding amount(10,000,000) is 10,000,000 / 100,000,000 = 10% (1,000 bps)
        // Rate = 5 + 10 * 1,000 / 10,000 = 5 + 1 = 6 bps
        // Premium = 10,000,000 * 6 / 10,000 = 6000
        assert_eq!(contract.calculate_fast_path_premium(amount).unwrap(), U256::from(6000));
        
        // Set utilized to 80,000,000 (80 USDC)
        contract.set_lp_liquidity(U256::from(100000000), U256::from(80000000)).unwrap();
        // New utilization after adding amount(10,000,000) is 90,000,000 / 100,000,000 = 90% (9,000 bps)
        // Rate = 5 + 10 * 9,000 / 10,000 = 5 + 9 = 14 bps
        // Premium = 10,000,000 * 14 / 10,000 = 14000
        assert_eq!(contract.calculate_fast_path_premium(amount).unwrap(), U256::from(14000));
        
        // Request amount exceeding capacity (capacity is 20,000,000, we request 30,000,000)
        let large_amount = U256::from(30000000);
        // Should trigger Dynamic Pool Cap limit and error
        assert!(contract.calculate_fast_path_premium(large_amount).is_err());
    }

    #[test]
    fn test_defi_rwa_cascading_buffer() {
        reset_test_state();
        let owner = address!("1111111111111111111111111111111111111111");
        set_msg_sender(owner);
        
        let mut contract = Nimbus::default();
        contract.init(owner, Address::ZERO, Address::ZERO).unwrap();
        
        // Assert initial addresses
        assert_eq!(contract.aave_pool().unwrap(), Address::ZERO);
        assert_eq!(contract.a_token().unwrap(), Address::ZERO);
        assert_eq!(contract.rwa_token().unwrap(), Address::ZERO);
        assert_eq!(contract.total_deposited_principal().unwrap(), U256::ZERO);
        
        // Test propose & execute params
        let pool = address!("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
        let a_token = address!("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb");
        let rwa = address!("cccccccccccccccccccccccccccccccccccccccc");
        
        contract.propose_aave_params(pool, a_token).unwrap();
        contract.propose_rwa_token(rwa).unwrap();
        set_block_timestamp(86401);
        contract.execute_aave_params().unwrap();
        contract.execute_rwa_token().unwrap();
        
        assert_eq!(contract.aave_pool().unwrap(), pool);
        assert_eq!(contract.a_token().unwrap(), a_token);
        assert_eq!(contract.rwa_token().unwrap(), rwa);
        
        // Test deposit increases principal (must be >= 10_000_000)
        let sid = FixedBytes::repeat_byte(0xde);
        contract
            .deposit(sid, vec![0x42; 256].into(), U256::from(20_000_000))
            .unwrap();
        
        // fee = (20,000,000 + 999) / 1000 = 20000
        // net_amount = 20,000,000 - 20,000 = 19,980,000 net
        assert_eq!(contract.total_deposited_principal().unwrap(), U256::from(19_980_000));
        
        // Test spend decreases principal
        let (alpha_neg, hm, pk_iss, nullifier) = register_mock_issuer(&mut contract, owner, U256::from(10_000_000), FixedBytes::ZERO);
        let is_valid = contract.spend(
            nullifier,
            alpha_neg.into(),
            pk_iss.into(),
            Address::ZERO,
            U256::from(10_000_000),
            FixedBytes::ZERO,
            U256::ZERO,
            FixedBytes::ZERO,
        ).unwrap();
        
        assert!(is_valid);
        assert_eq!(contract.total_deposited_principal().unwrap(), U256::from(9_980_000));
        
        // Test yield claim under test (where total assets = principal, so yield is 0)
        assert_eq!(contract.claim_accumulated_yield().unwrap(), U256::ZERO);
    }

    #[test]
    fn test_polymarket_fallback_refund() {
        reset_test_state();
        let owner = address!("1111111111111111111111111111111111111111");
        set_msg_sender(owner);
        
        let mut contract = Nimbus::default();
        contract.init(owner, Address::ZERO, Address::ZERO).unwrap();
        
        // Set principal
        contract.deposit(
            FixedBytes::repeat_byte(0x99),
            vec![0x42; 256].into(),
            U256::from(20_000_000),
        ).unwrap();
        let (alpha_neg, hm, pk_iss, nullifier) = register_mock_issuer(&mut contract, owner, U256::from(10_000_000), FixedBytes::ZERO);
        
        // Call spend_and_buy_shares with polymarket_ctf = Address::ZERO (which triggers mock fallback in tests)
        let success = contract.spend_and_buy_shares(
            nullifier,
            alpha_neg.into(),
            pk_iss.into(),
            Address::ZERO, // triggers fallback simulation in test block
            Address::ZERO,
            FixedBytes::ZERO,
            U256::from(10_000_000),
            U256::ZERO,
            FixedBytes::ZERO,
        ).unwrap();
        
        // Under our mock try-catch, it should return true (gracefully handled)
        assert!(success);
        
        // The net payout should be calculated:
        // 10,000,000 - base_fee = 10,000,000 - 15000 = 9,985,000
        let expected_payout = U256::from(9_985_000);
        assert_eq!(contract.get_failed_intent_refund(nullifier).unwrap(), expected_payout);
        
        // Claim the refund to a recipient
        let recipient = address!("4444444444444444444444444444444444444444");
        let claim_ok = contract.claim_failed_intent_refund(nullifier, recipient).unwrap();
        assert!(claim_ok);
        
        // The refund amount should now be cleared (zero)
        assert_eq!(contract.get_failed_intent_refund(nullifier).unwrap(), U256::ZERO);
        
        // Claiming again should fail
        assert_eq!(
            contract.claim_failed_intent_refund(nullifier, recipient),
            Err(b"NO_REFUND_AVAILABLE".to_vec())
        );
    }

    #[test]
    fn test_verify_compliance_flow() {
        reset_test_state();
        let owner = address!("1111111111111111111111111111111111111111");
        let mut contract = Nimbus::default();
        contract.init(owner, Address::ZERO, Address::ZERO).unwrap();

        let root = FixedBytes::repeat_byte(0x11);
        let nullifier = FixedBytes::repeat_byte(0x22);
        let recipient = address!("3333333333333333333333333333333333333333");
        let amount = U256::from(10_000_000);

        // When root is not registered, verify_compliance should return Ok(false)
        let is_valid_unregistered = contract.verify_compliance(
            root,
            nullifier,
            recipient,
            amount,
            vec![0; 128].into(),
            vec![0; 256].into(),
            vec![0; 128].into(),
        ).unwrap();
        assert!(!is_valid_unregistered);

        // Register clean root
        contract.clean_association_roots.insert(root, true);

        // When root is registered, it calls get_compliance_vk, compute_public_inputs_g1, and verify_groth16_proof.
        // Under #[cfg(test)], these are mocked to succeed, so it should return Ok(true).
        let is_valid_registered = contract.verify_compliance(
            root,
            nullifier,
            recipient,
            amount,
            vec![0; 128].into(),
            vec![0; 256].into(),
            vec![0; 128].into(),
        ).unwrap();
        assert!(is_valid_registered);
    }
}
