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

use alloy_primitives::{Address, FixedBytes};
use stylus_sdk::{prelude::*, alloy_primitives::U256, call::RawCall};

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
    }

    fn reset_test_state() {
        STORAGE.with(|s| s.borrow_mut().clear());
        MSG_SENDER.with(|s| *s.borrow_mut() = Address::ZERO);
        BLOCK_TIMESTAMP.with(|t| *t.borrow_mut() = 0);
    }

    fn set_msg_sender(sender: Address) {
        MSG_SENDER.with(|s| *s.borrow_mut() = sender);
    }

    fn set_block_timestamp(ts: u64) {
        BLOCK_TIMESTAMP.with(|t| *t.borrow_mut() = ts);
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
    pub unsafe extern "C" fn read_return_data(_offset: usize, size: usize, dest: *mut u8) {
        let dest_slice = std::slice::from_raw_parts_mut(dest, size);
        for byte in dest_slice.iter_mut() {
            *byte = 0;
        }
        if size == 32 {
            dest_slice[31] = 1; // Simulate verification success (last byte is 1)
        }
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
            vec![0; 100],
            vec![0; 256],
            vec![0; 128],
            vec![0; 128],
            vec![0; 128],
            vec![0; 256],
            vec![0; 256],
            vec![0; 256],
        );
        assert_eq!(result, Err(b"INVALID_INPUT_LENGTHS".to_vec()));
    }

    #[test]
    fn test_mocked_groth16_verification() {
        let nimbus_contract = Nimbus::default();
        // Since we stubbed static_call_contract, return_data_size and read_return_data to return success,
        // this test should successfully verify the Groth16 proof with valid lengths!
        let is_valid = nimbus_contract.verify_groth16_proof(
            vec![0; 128],
            vec![0; 256],
            vec![0; 128],
            vec![0; 128],
            vec![0; 128],
            vec![0; 256],
            vec![0; 256],
            vec![0; 256],
        ).unwrap();
        assert!(is_valid, "Mocked Groth16 proof verification failed!");
    }

    #[test]
    fn test_ccip_receive_decoding_and_execution() {
        let mut payload = vec![0u8; 648];
        let amount = U256::from(1_000_000);
        payload[616..648].copy_from_slice(&amount.to_be_bytes::<32>());
        
        let mut nimbus_contract = Nimbus::default();
        let result = nimbus_contract.ccip_receive(
            FixedBytes::ZERO,
            1,
            vec![],
            payload,
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
            contract.deposit(sid, vec![], U256::from(1_000_000)),
            Err(b"CONTRACT_PAUSED".to_vec())
        );
        assert_eq!(
            contract.reveal_mask_key(sid, vec![], vec![], vec![]),
            Err(b"CONTRACT_PAUSED".to_vec())
        );
        assert_eq!(
            contract.spend(sid, vec![], vec![], vec![], Address::ZERO, U256::ZERO),
            Err(b"CONTRACT_PAUSED".to_vec())
        );
        assert_eq!(
            contract.spend_and_buy_shares(sid, vec![], vec![], vec![], Address::ZERO, Address::ZERO, FixedBytes::ZERO, U256::ZERO),
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
        
        // Deposit (must be >= 1_000_000)
        set_msg_sender(client);
        set_block_timestamp(1000);
        contract.deposit(sid, vec![], U256::from(1_000_000)).unwrap();
        
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
    fn test_fast_path_liquidity_premium_phases() {
        reset_test_state();
        let owner = address!("1111111111111111111111111111111111111111");
        set_msg_sender(owner);
        
        let mut contract = Nimbus::default();
        contract.init(owner, Address::ZERO, Address::ZERO).unwrap();
        
        // Default phase should be 1
        assert_eq!(contract.fast_path_phase().unwrap(), U256::from(1));
        
        // Fase 1: Premium is always 0
        let amount = U256::from(1_000_000); // 1,000,000 (e.g. 1 USDC)
        assert_eq!(contract.calculate_fast_path_premium(amount).unwrap(), U256::ZERO);
        
        // Change to Fase 2 via propose & execute
        contract.propose_fast_path_phase(U256::from(2)).unwrap();
        set_block_timestamp(86401);
        contract.execute_fast_path_phase().unwrap();
        assert_eq!(contract.fast_path_phase().unwrap(), U256::from(2));
        // Fase 2: 0.05% flat premium => 1,000,000 * 5 / 10,000 = 500
        assert_eq!(contract.calculate_fast_path_premium(amount).unwrap(), U256::from(500));
        
        // Change to Fase 3 via propose & execute
        contract.propose_fast_path_phase(U256::from(3)).unwrap();
        set_block_timestamp(86401 * 2);
        contract.execute_fast_path_phase().unwrap();
        assert_eq!(contract.fast_path_phase().unwrap(), U256::from(3));
        
        // Fase 3 with zero liquidity should fail
        assert!(contract.calculate_fast_path_premium(amount).is_err());
        
        // Set LP liquidity: total = 10,000,000, utilized = 0
        contract.set_lp_liquidity(U256::from(10000000), U256::from(0)).unwrap();
        // New utilization after adding amount(1,000,000) is 1,000,000 / 10,000,000 = 10% (1,000 bps)
        // Rate = 5 + 10 * 1,000 / 10,000 = 5 + 1 = 6 bps
        // Premium = 1,000,000 * 6 / 10,000 = 600
        assert_eq!(contract.calculate_fast_path_premium(amount).unwrap(), U256::from(600));
        
        // Set utilized to 8,000,000
        contract.set_lp_liquidity(U256::from(10000000), U256::from(8000000)).unwrap();
        // New utilization after adding amount(1,000,000) is 9,000,000 / 10,000,000 = 90% (9,000 bps)
        // Rate = 5 + 10 * 9,000 / 10,000 = 5 + 9 = 14 bps
        // Premium = 1,000,000 * 14 / 10,000 = 1400
        assert_eq!(contract.calculate_fast_path_premium(amount).unwrap(), U256::from(1400));
        
        // Request amount exceeding capacity (capacity is 2,000,000, we request 3,000,000)
        let large_amount = U256::from(3000000);
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
        
        // Test deposit increases principal (must be >= 1_000_000)
        let sid = FixedBytes::repeat_byte(0xde);
        contract.deposit(sid, vec![], U256::from(2_000_000)).unwrap();
        
        // fee = (2,000,000 + 999) / 1000 = 2000
        // net_amount = 2,000,000 - 2,000 = 1,998,000 net
        assert_eq!(contract.total_deposited_principal().unwrap(), U256::from(1_998_000));
        
        // Test spend decreases principal
        let nullifier = FixedBytes::repeat_byte(0xef);
        let is_valid = contract.spend(
            nullifier,
            vec![],
            vec![],
            vec![],
            Address::ZERO,
            U256::from(1_000_000),
        ).unwrap();
        
        assert!(is_valid);
        assert_eq!(contract.total_deposited_principal().unwrap(), U256::from(998_000));
        
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
        
        let nullifier = FixedBytes::repeat_byte(0xd1);
        
        // Set principal
        contract.deposit(FixedBytes::repeat_byte(0x99), vec![], U256::from(2_000_000)).unwrap();
        
        // Call spend_and_buy_shares with polymarket_ctf = Address::ZERO (which triggers mock fallback in tests)
        let success = contract.spend_and_buy_shares(
            nullifier,
            vec![],
            vec![],
            vec![],
            Address::ZERO, // triggers fallback simulation in test block
            Address::ZERO,
            FixedBytes::ZERO,
            U256::from(1_000_000),
        ).unwrap();
        
        // Under our mock try-catch, it should return true (gracefully handled)
        assert!(success);
        
        // The net payout should be calculated:
        // 1,000,000 - base_fee = 1,000,000 - 1500 = 998,500
        let expected_payout = U256::from(998_500);
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
        let amount = U256::from(1_000_000);

        // When root is not registered, verify_compliance should return Ok(false)
        let is_valid_unregistered = contract.verify_compliance(
            root,
            nullifier,
            recipient,
            amount,
            vec![0; 128],
            vec![0; 256],
            vec![0; 128],
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
            vec![0; 128],
            vec![0; 256],
            vec![0; 128],
        ).unwrap();
        assert!(is_valid_registered);
    }
}


