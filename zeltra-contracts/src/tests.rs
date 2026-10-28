use super::*;
use alloy_primitives::address;
use ark_bls12_381::{Fr, G1Affine, G2Affine};
use ark_ff::UniformRand;
use ark_serialize::{CanonicalDeserialize, CanonicalSerialize};
use rand::thread_rng;

// --- Mock Stylus HostIO Symbols to satisfy the linker during host tests ---
use std::cell::RefCell;
use std::collections::HashMap;

thread_local! {
    pub(crate) static STORAGE: RefCell<HashMap<[u8; 32], [u8; 32]>> = RefCell::new(HashMap::new());
    pub(crate) static MSG_SENDER: RefCell<Address> = const { RefCell::new(Address::ZERO) };
    pub(crate) static BLOCK_TIMESTAMP: RefCell<u64> = const { RefCell::new(0) };
    pub(crate) static MSG_VALUE: RefCell<U256> = const { RefCell::new(U256::ZERO) };
    pub(crate) static ERC20_TRANSFER_FROM_RESULT: RefCell<Result<bool, Vec<u8>>> = const { RefCell::new(Ok(true)) };
    pub(crate) static ERC20_TRANSFER_RESULT: RefCell<Result<bool, Vec<u8>>> = const { RefCell::new(Ok(true)) };
    pub(crate) static WETH9_DEPOSIT_CALLS: RefCell<Vec<(Address, U256)>> = const { RefCell::new(Vec::new()) };
    pub(crate) static MOCK_ASSET_BALANCES: RefCell<HashMap<u8, U256>> = RefCell::new(HashMap::new());
    static PAIRING_RESULT: RefCell<u8> = const { RefCell::new(1) };
    static LAST_RETURN_DATA_SIZE: RefCell<usize> = const { RefCell::new(32) };
}

pub(crate) fn reset_test_state() {
    STORAGE.with(|s| s.borrow_mut().clear());
    MSG_SENDER.with(|s| *s.borrow_mut() = Address::ZERO);
    BLOCK_TIMESTAMP.with(|t| *t.borrow_mut() = 0);
    MSG_VALUE.with(|v| *v.borrow_mut() = U256::ZERO);
    ERC20_TRANSFER_FROM_RESULT.with(|r| *r.borrow_mut() = Ok(true));
    ERC20_TRANSFER_RESULT.with(|r| *r.borrow_mut() = Ok(true));
    WETH9_DEPOSIT_CALLS.with(|c| c.borrow_mut().clear());
    MOCK_ASSET_BALANCES.with(|m| m.borrow_mut().clear());
    PAIRING_RESULT.with(|result| *result.borrow_mut() = 1);
    LAST_RETURN_DATA_SIZE.with(|s| *s.borrow_mut() = 32);
}

pub(crate) fn set_msg_sender(sender: Address) {
    MSG_SENDER.with(|s| *s.borrow_mut() = sender);
}

pub(crate) fn set_msg_value(value: U256) {
    MSG_VALUE.with(|v| *v.borrow_mut() = value);
}

pub(crate) fn set_block_timestamp(ts: u64) {
    BLOCK_TIMESTAMP.with(|t| *t.borrow_mut() = ts);
}

pub(crate) fn set_erc20_transfer_from_result(res: Result<bool, Vec<u8>>) {
    ERC20_TRANSFER_FROM_RESULT.with(|r| *r.borrow_mut() = res);
}

pub(crate) fn set_erc20_transfer_result(res: Result<bool, Vec<u8>>) {
    ERC20_TRANSFER_RESULT.with(|r| *r.borrow_mut() = res);
}

pub(crate) fn set_mock_asset_balance(asset_id: u8, balance: U256) {
    MOCK_ASSET_BALANCES.with(|m| {
        m.borrow_mut().insert(asset_id, balance);
    });
}

pub(crate) fn set_pairing_result(result: bool) {
    PAIRING_RESULT.with(|value| *value.borrow_mut() = u8::from(result));
}

#[no_mangle]
pub unsafe extern "C" fn msg_value(dest: *mut u8) {
    let dest_slice = std::slice::from_raw_parts_mut(dest, 32);
    MSG_VALUE.with(|v| {
        dest_slice.copy_from_slice(&v.borrow().to_be_bytes::<32>());
    });
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
    BLOCK_TIMESTAMP.with(|ts| *ts.borrow())
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
    let addr_slice = std::slice::from_raw_parts(_contract, 20);
    let len = if addr_slice[19] == 0x0c || addr_slice[19] == 0x0b {
        128
    } else {
        32
    };
    LAST_RETURN_DATA_SIZE.with(|s| *s.borrow_mut() = len);
    if !return_data_len.is_null() {
        *return_data_len = len;
    }
    0 // Status code 0 is Success
}

#[no_mangle]
pub unsafe extern "C" fn storage_flush_cache() {}

#[no_mangle]
pub unsafe extern "C" fn return_data_size() -> usize {
    LAST_RETURN_DATA_SIZE.with(|s| *s.borrow())
}

#[no_mangle]
pub unsafe extern "C" fn read_return_data(dest: *mut u8, _offset: usize, size: usize) -> usize {
    let dest_slice = std::slice::from_raw_parts_mut(dest, size);
    for byte in dest_slice.iter_mut() {
        *byte = 0;
    }
    if size == 32 {
        PAIRING_RESULT.with(|result| dest_slice[31] = *result.borrow());
    } else if size == 128 {
        use ark_ec::AffineRepr;
        let g1_bytes = to_evm_g1(&G1Affine::generator());
        dest_slice.copy_from_slice(&g1_bytes);
    }
    size
}

pub(crate) fn valid_test_g1() -> Bytes {
    use ark_ec::AffineRepr;
    to_evm_g1(&G1Affine::generator()).to_vec().into()
}

pub(crate) fn valid_test_g2() -> Bytes {
    use ark_ec::AffineRepr;
    to_evm_g2(&G2Affine::generator()).to_vec().into()
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
    let zeltra_contract = Zeltra::default();
    let result = zeltra_contract.verify_groth16_proof(
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
    let zeltra_contract = Zeltra::default();
    // Since we stubbed static_call_contract, return_data_size and read_return_data to return success,
    // this test should successfully verify the Groth16 proof with valid lengths!
    let is_valid = zeltra_contract
        .verify_groth16_proof(
            vec![0; 128].into(),
            vec![0; 256].into(),
            vec![0; 128].into(),
            vec![0; 128].into(),
            vec![0; 128].into(),
            vec![0; 256].into(),
            vec![0; 256].into(),
            vec![0; 256].into(),
        )
        .unwrap();
    assert!(is_valid, "Mocked Groth16 proof verification failed!");
}

#[test]
fn test_erc165_supports_interface() {
    reset_test_state();
    let contract = Zeltra::default();

    // ERC-165 interface ID: 0x01ffc9a7
    assert_eq!(
        contract.supports_interface(FixedBytes::from(ERC165_INTERFACE_ID)),
        Ok(true)
    );

    // CCIP Receiver interface ID: 0x8a92dced
    assert_eq!(
        contract.supports_interface(FixedBytes::from(CCIP_RECEIVER_INTERFACE_ID)),
        Ok(true)
    );

    // Random non-matching interface ID: 0x12345678
    assert_eq!(
        contract.supports_interface(FixedBytes::from([0x12, 0x34, 0x56, 0x78])),
        Ok(false)
    );
}

#[test]
fn test_ccip_receive_programmable_deposit_usdc_success() {
    reset_test_state();
    let owner = address!("1111111111111111111111111111111111111111");
    let router = address!("2222222222222222222222222222222222222222");
    let usdc_token = address!("3333333333333333333333333333333333333333");
    let valid_sender: Bytes = alloc::vec![0xaa; 20].into();
    set_msg_sender(owner);

    let mut contract = Zeltra::default();
    contract.init(owner, usdc_token, Address::ZERO).unwrap();
    contract.set_asset_token(U256::from(1), usdc_token).unwrap();
    contract.set_ccip_router(router).unwrap();
    contract
        .set_ccip_sender_allowlist(10344971235874465080, valid_sender.clone(), true)
        .unwrap();

    // Prepare 128-byte wire payload: 32B canonical note commitment + 96B encrypted memo
    let mut payload = vec![0u8; 128];
    payload[31] = 0x42; // canonical note commitment < r
    payload[32..128].copy_from_slice(&[0xee; 96]); // 96B memo

    let message_id = FixedBytes::repeat_byte(0x77);
    let amount = U256::from(100_000_000u64); // 100 USDC

    // Ensure mock contract balance satisfies solvency
    set_mock_asset_balance(1, amount);

    let msg = CcipAny2EVMMessage {
        messageId: message_id,
        sourceChainSelector: 10344971235874465080,
        sender: valid_sender.clone(),
        data: payload.into(),
        destTokenAmounts: vec![CcipEVMTokenAmount {
            token: usdc_token,
            amount,
        }],
    };

    // Caller is CCIP router
    set_msg_sender(router);
    let result = contract.ccip_receive(msg.clone());
    assert!(result.is_ok(), "CCIP USDC deposit failed: {:?}", result);

    // Verify state effects
    assert!(contract.ccip_processed_messages.get(message_id));
    assert_eq!(
        contract.user_note_liabilities.get(U256::from(1)),
        amount
    );
    assert_eq!(contract.mmr_leaf_count.get(), U256::from(1));

    // Replay attack must fail
    let replay_result = contract.ccip_receive(msg);
    assert_eq!(
        replay_result,
        Err(b"CCIP_MESSAGE_ALREADY_PROCESSED".to_vec())
    );
}

#[test]
fn test_ccip_receive_programmable_deposit_weth_success() {
    reset_test_state();
    let owner = address!("1111111111111111111111111111111111111111");
    let router = address!("2222222222222222222222222222222222222222");
    let weth_token = address!("4444444444444444444444444444444444444444");
    let valid_sender: Bytes = alloc::vec![0xbb; 20].into();
    set_msg_sender(owner);

    let mut contract = Zeltra::default();
    contract.init(owner, Address::ZERO, Address::ZERO).unwrap();
    contract.set_asset_token(U256::ZERO, weth_token).unwrap();
    contract.set_ccip_router(router).unwrap();
    contract
        .set_ccip_sender_allowlist(16015286601757825753, valid_sender.clone(), true)
        .unwrap();

    let mut payload = vec![0u8; 128];
    payload[31] = 0x99;
    payload[32..128].copy_from_slice(&[0xdd; 96]);

    let message_id = FixedBytes::repeat_byte(0x88);
    let amount_wei = U256::from(500_000_000_000_000_000u64); // 0.5 ETH in wei
    let amount_gwei = U256::from(500_000_000u64); // normalized liability

    set_mock_asset_balance(0, amount_wei);

    let msg = CcipAny2EVMMessage {
        messageId: message_id,
        sourceChainSelector: 16015286601757825753,
        sender: valid_sender,
        data: payload.into(),
        destTokenAmounts: vec![CcipEVMTokenAmount {
            token: weth_token,
            amount: amount_wei,
        }],
    };

    set_msg_sender(router);
    let result = contract.ccip_receive(msg);
    assert!(result.is_ok(), "CCIP WETH deposit failed: {:?}", result);

    assert_eq!(
        contract.user_note_liabilities.get(U256::ZERO),
        amount_gwei
    );
    assert_eq!(contract.mmr_leaf_count.get(), U256::from(1));
}

#[test]
fn test_ccip_receive_negative_cases() {
    reset_test_state();
    let owner = address!("1111111111111111111111111111111111111111");
    let router = address!("2222222222222222222222222222222222222222");
    let usdc_token = address!("3333333333333333333333333333333333333333");
    let valid_sender: Bytes = alloc::vec![0xaa; 20].into();
    set_msg_sender(owner);

    let mut contract = Zeltra::default();
    contract.init(owner, usdc_token, Address::ZERO).unwrap();
    contract.set_asset_token(U256::from(1), usdc_token).unwrap();

    let mut payload = vec![0u8; 128];
    payload[31] = 0x01;
    let message_id = FixedBytes::repeat_byte(0x55);
    let amount = U256::from(50_000_000u64);

    let make_msg = |data: Vec<u8>, token: Address, amt: U256, sel: u64, snd: Bytes, id: FixedBytes<32>| CcipAny2EVMMessage {
        messageId: id,
        sourceChainSelector: sel,
        sender: snd,
        data: data.into(),
        destTokenAmounts: vec![CcipEVMTokenAmount { token, amount: amt }],
    };

    // Case 1: Router not configured
    set_msg_sender(router);
    let msg = make_msg(payload.clone(), usdc_token, amount, 1, valid_sender.clone(), message_id);
    assert_eq!(contract.ccip_receive(msg.clone()), Err(b"CCIP_ROUTER_NOT_CONFIGURED".to_vec()));

    // Case 2: Router set to ZERO rejected
    set_msg_sender(owner);
    assert_eq!(contract.set_ccip_router(Address::ZERO), Err(b"INVALID_ROUTER_ADDRESS".to_vec()));
    contract.set_ccip_router(router).unwrap();
    contract.set_ccip_sender_allowlist(1, valid_sender.clone(), true).unwrap();

    // Case 3: Caller is not router (caller is owner)
    assert_eq!(contract.ccip_receive(msg.clone()), Err(b"ONLY_CCIP_ROUTER_ALLOWED".to_vec()));

    // Switch to router caller
    set_msg_sender(router);

    // Case 4: Message ID is ZERO
    let zero_id_msg = make_msg(payload.clone(), usdc_token, amount, 1, valid_sender.clone(), FixedBytes::ZERO);
    assert_eq!(contract.ccip_receive(zero_id_msg), Err(b"INVALID_CCIP_MESSAGE_ID".to_vec()));

    // Case 5: Source chain selector is 0
    let zero_sel_msg = make_msg(payload.clone(), usdc_token, amount, 0, valid_sender.clone(), message_id);
    assert_eq!(contract.ccip_receive(zero_sel_msg), Err(b"INVALID_SOURCE_CHAIN_SELECTOR".to_vec()));

    // Case 6: Sender is empty
    let empty_sender_msg = make_msg(payload.clone(), usdc_token, amount, 1, vec![].into(), message_id);
    assert_eq!(contract.ccip_receive(empty_sender_msg), Err(b"EMPTY_CCIP_SENDER".to_vec()));

    // Case 7: Sender is not allowlisted
    let unallowlisted_msg = make_msg(payload.clone(), usdc_token, amount, 2, valid_sender.clone(), message_id);
    assert_eq!(contract.ccip_receive(unallowlisted_msg), Err(b"CCIP_SENDER_NOT_ALLOWED".to_vec()));

    // Case 8: Invalid payload length != 128 (e.g. 96 bytes)
    let bad_len_msg = make_msg(vec![0u8; 96], usdc_token, amount, 1, valid_sender.clone(), message_id);
    assert_eq!(contract.ccip_receive(bad_len_msg), Err(b"INVALID_CCIP_PAYLOAD_LENGTH_EXPECTED_128".to_vec()));

    // Case 9: Zero commitment rejected
    let zero_cm_payload = vec![0u8; 128];
    let zero_cm_msg = make_msg(zero_cm_payload, usdc_token, amount, 1, valid_sender.clone(), message_id);
    assert_eq!(contract.ccip_receive(zero_cm_msg), Err(b"ZERO_COMMITMENT_REJECTED".to_vec()));

    // Case 10: Non-canonical scalar overflow (>= r)
    let overflow_payload = vec![0xff; 128]; // 0xff...ff >= BLS12-381 Fr modulus
    let overflow_msg = make_msg(overflow_payload, usdc_token, amount, 1, valid_sender.clone(), message_id);
    assert_eq!(contract.ccip_receive(overflow_msg), Err(b"INVALID_NOTE_COMMITMENT_SCALAR".to_vec()));

    // Case 11: Unsupported token
    let random_token = address!("9999999999999999999999999999999999999999");
    let unsupp_token_msg = make_msg(payload.clone(), random_token, amount, 1, valid_sender.clone(), message_id);
    assert_eq!(contract.ccip_receive(unsupp_token_msg), Err(b"UNSUPPORTED_CCIP_TOKEN".to_vec()));

    // Case 12: Amount too small for USDC (< 10 USDC)
    let too_small_msg = make_msg(payload.clone(), usdc_token, U256::from(5_000_000u64), 1, valid_sender.clone(), message_id);
    assert_eq!(contract.ccip_receive(too_small_msg), Err(b"AMOUNT_TOO_SMALL_MIN_10_USDC".to_vec()));
}

#[test]
fn test_initialization() {
    reset_test_state();
    let owner = address!("1111111111111111111111111111111111111111");
    set_msg_sender(owner);

    let mut contract = Zeltra::default();
    // Init with ZERO owner should fail
    assert_eq!(
        contract.init(Address::ZERO, Address::ZERO, Address::ZERO),
        Err(b"INVALID_OWNER".to_vec())
    );

    // First init should succeed
    assert!(contract.init(owner, Address::ZERO, Address::ZERO).is_ok());

    // Second init should fail
    assert_eq!(
        contract.init(owner, Address::ZERO, Address::ZERO),
        Err(b"ALREADY_INITIALIZED".to_vec())
    );
}

#[test]
fn test_pause_unpause() {
    reset_test_state();
    let owner = address!("1111111111111111111111111111111111111111");
    let non_owner = address!("2222222222222222222222222222222222222222");

    set_msg_sender(owner);
    let mut contract = Zeltra::default();
    contract.init(owner, Address::ZERO, Address::ZERO).unwrap();

    // Non-owner pausing should fail
    set_msg_sender(non_owner);
    assert_eq!(contract.pause(), Err(b"NOT_OWNER".to_vec()));

    // Owner pausing should succeed
    set_msg_sender(owner);
    assert!(contract.pause().is_ok());
    assert!(contract.paused.get());

    // Non-owner unpausing should fail
    set_msg_sender(non_owner);
    assert_eq!(contract.unpause(), Err(b"NOT_OWNER".to_vec()));

    // Owner unpausing should succeed
    set_msg_sender(owner);
    assert!(contract.unpause().is_ok());
    assert!(!contract.paused.get());
}

#[test]
fn test_guarded_functions_fail_when_paused() {
    reset_test_state();
    let owner = address!("1111111111111111111111111111111111111111");
    set_msg_sender(owner);

    let mut contract = Zeltra::default();
    contract.init(owner, Address::ZERO, Address::ZERO).unwrap();
    contract.pause().unwrap();

    assert_eq!(
        contract.deposit_direct_note(
            U256::from(10_000_000),
            FixedBytes::repeat_byte(0x01),
            vec![0u8; 64].into()
        ),
        Err(b"CONTRACT_PAUSED".to_vec())
    );
    assert_eq!(
        contract.register_clean_root(FixedBytes::ZERO),
        Err(b"CONTRACT_PAUSED".to_vec())
    );
}

#[test]
fn test_deposit_direct_note_pos1_success() {
    reset_test_state();
    let owner = Address::repeat_byte(0x01);
    let client = Address::repeat_byte(0x02);
    set_msg_sender(owner);
    set_block_timestamp(1_700_000_000);

    let mut contract = Zeltra::default();
    contract
        .init(
            owner,
            Address::repeat_byte(0x03),
            Address::repeat_byte(0x04),
        )
        .unwrap();

    set_msg_sender(client);
    let amount = U256::from(10_000_000);
    let commitment = FixedBytes::repeat_byte(0x01);
    let memo = vec![0xaa; 96].into();

    let res = contract.deposit_direct_note(amount, commitment, memo);
    assert!(res.is_ok());

    assert_eq!(contract.user_note_liabilities.get(U256::from(1)), amount);
    assert_eq!(
        contract.total_deposited_principals.get(U256::from(1)),
        amount
    );
    assert_eq!(contract.mmr_leaf_count.get(), U256::from(1));

    let root = contract.note_tree_root.get();
    assert_ne!(root, FixedBytes::ZERO);
    assert!(contract._is_accepted_note_root(root));
    assert_eq!(
        contract.accepted_note_roots.get(root),
        U256::from(1_700_000_000)
    );
    assert_eq!(contract.root_history.get(U256::ZERO), root);
    assert_eq!(contract.root_history_index.get(), U256::from(1));
}

#[test]
fn test_deposit_direct_note_neg1_non_canonical_scalar() {
    reset_test_state();
    let owner = Address::repeat_byte(0x01);
    set_msg_sender(owner);
    let mut contract = Zeltra::default();
    contract
        .init(
            owner,
            Address::repeat_byte(0x03),
            Address::repeat_byte(0x04),
        )
        .unwrap();

    let non_canonical = FixedBytes::repeat_byte(0xff);
    let res =
        contract.deposit_direct_note(U256::from(10_000_000), non_canonical, vec![0xaa; 96].into());
    assert_eq!(res, Err(b"INVALID_NOTE_COMMITMENT_SCALAR".to_vec()));
    assert_eq!(
        contract.user_note_liabilities.get(U256::from(1)),
        U256::ZERO
    );
    assert_eq!(
        contract.total_deposited_principals.get(U256::from(1)),
        U256::ZERO
    );
    assert_eq!(contract.mmr_leaf_count.get(), U256::ZERO);
    assert_eq!(contract.note_tree_root.get(), FixedBytes::ZERO);
}

#[test]
fn test_deposit_direct_note_neg2_zero_commitment() {
    reset_test_state();
    let owner = Address::repeat_byte(0x01);
    set_msg_sender(owner);
    let mut contract = Zeltra::default();
    contract
        .init(
            owner,
            Address::repeat_byte(0x03),
            Address::repeat_byte(0x04),
        )
        .unwrap();

    let res = contract.deposit_direct_note(
        U256::from(10_000_000),
        FixedBytes::ZERO,
        vec![0xaa; 64].into(),
    );
    assert_eq!(res, Err(b"ZERO_COMMITMENT_REJECTED".to_vec()));
    assert_eq!(
        contract.user_note_liabilities.get(U256::from(1)),
        U256::ZERO
    );
    assert_eq!(
        contract.total_deposited_principals.get(U256::from(1)),
        U256::ZERO
    );
    assert_eq!(contract.mmr_leaf_count.get(), U256::ZERO);
    assert_eq!(contract.note_tree_root.get(), FixedBytes::ZERO);
}

#[test]
fn test_deposit_direct_note_neg3_amount_too_small() {
    reset_test_state();
    let owner = Address::repeat_byte(0x01);
    set_msg_sender(owner);
    let mut contract = Zeltra::default();
    contract
        .init(
            owner,
            Address::repeat_byte(0x03),
            Address::repeat_byte(0x04),
        )
        .unwrap();

    let res = contract.deposit_direct_note(
        U256::from(9_999_999),
        FixedBytes::repeat_byte(0x01),
        vec![0xaa; 64].into(),
    );
    assert_eq!(res, Err(b"AMOUNT_TOO_SMALL".to_vec()));
    assert_eq!(
        contract.user_note_liabilities.get(U256::from(1)),
        U256::ZERO
    );
    assert_eq!(
        contract.total_deposited_principals.get(U256::from(1)),
        U256::ZERO
    );
    assert_eq!(contract.mmr_leaf_count.get(), U256::ZERO);
    assert_eq!(contract.note_tree_root.get(), FixedBytes::ZERO);
}

#[test]
fn test_deposit_direct_note_neg4_invalid_memo_length() {
    reset_test_state();
    let owner = Address::repeat_byte(0x01);
    set_msg_sender(owner);
    let mut contract = Zeltra::default();
    contract
        .init(
            owner,
            Address::repeat_byte(0x03),
            Address::repeat_byte(0x04),
        )
        .unwrap();

    let res_short = contract.deposit_direct_note(
        U256::from(10_000_000),
        FixedBytes::repeat_byte(0x01),
        vec![0xaa; 95].into(),
    );
    assert_eq!(res_short, Err(b"INVALID_MEMO_LENGTH".to_vec()));
    assert_eq!(
        contract.user_note_liabilities.get(U256::from(1)),
        U256::ZERO
    );
    assert_eq!(
        contract.total_deposited_principals.get(U256::from(1)),
        U256::ZERO
    );
    assert_eq!(contract.mmr_leaf_count.get(), U256::ZERO);
    assert_eq!(contract.note_tree_root.get(), FixedBytes::ZERO);

    let res_long = contract.deposit_direct_note(
        U256::from(10_000_000),
        FixedBytes::repeat_byte(0x01),
        vec![0xaa; 97].into(),
    );
    assert_eq!(res_long, Err(b"INVALID_MEMO_LENGTH".to_vec()));
    assert_eq!(
        contract.user_note_liabilities.get(U256::from(1)),
        U256::ZERO
    );
    assert_eq!(
        contract.total_deposited_principals.get(U256::from(1)),
        U256::ZERO
    );
    assert_eq!(contract.mmr_leaf_count.get(), U256::ZERO);
    assert_eq!(contract.note_tree_root.get(), FixedBytes::ZERO);
}

#[test]
fn test_deposit_direct_note_with_empty_memo() {
    reset_test_state();
    let owner = Address::repeat_byte(0x01);
    let client = Address::repeat_byte(0x02);
    set_msg_sender(owner);

    let mut contract = Zeltra::default();
    contract
        .init(
            owner,
            Address::repeat_byte(0x03),
            Address::repeat_byte(0x04),
        )
        .unwrap();

    set_msg_sender(client);
    let amount = U256::from(10_000_000);
    let commitment = FixedBytes::repeat_byte(0x01);
    let empty_memo: Bytes = vec![].into();

    set_erc20_transfer_from_result(Ok(true));
    let res = contract.deposit_direct_note(amount, commitment, empty_memo);
    assert_eq!(res, Ok(()));

    assert_eq!(contract.user_note_liabilities.get(U256::from(1)), amount);
    assert_eq!(contract.mmr_leaf_count.get(), U256::from(1));
}

#[test]
fn test_deposit_direct_note_neg5_paused() {
    reset_test_state();
    let owner = Address::repeat_byte(0x01);
    set_msg_sender(owner);
    let mut contract = Zeltra::default();
    contract
        .init(
            owner,
            Address::repeat_byte(0x03),
            Address::repeat_byte(0x04),
        )
        .unwrap();
    contract.pause().unwrap();

    let res = contract.deposit_direct_note(
        U256::from(10_000_000),
        FixedBytes::repeat_byte(0x01),
        vec![0xaa; 96].into(),
    );
    assert_eq!(res, Err(b"CONTRACT_PAUSED".to_vec()));
    assert_eq!(
        contract.user_note_liabilities.get(U256::from(1)),
        U256::ZERO
    );
    assert_eq!(
        contract.total_deposited_principals.get(U256::from(1)),
        U256::ZERO
    );
    assert_eq!(contract.mmr_leaf_count.get(), U256::ZERO);
    assert_eq!(contract.note_tree_root.get(), FixedBytes::ZERO);
}

#[test]
fn test_deposit_direct_note_neg6_erc20_transfer_from_fails() {
    reset_test_state();
    let owner = Address::repeat_byte(0x01);
    let client = Address::repeat_byte(0x02);
    set_msg_sender(owner);

    let mut contract = Zeltra::default();
    contract
        .init(
            owner,
            Address::repeat_byte(0x03),
            Address::repeat_byte(0x04),
        )
        .unwrap();

    set_msg_sender(client);
    let amount = U256::from(10_000_000);
    let commitment = FixedBytes::repeat_byte(0x01);
    let memo: Bytes = vec![0xaa; 96].into();

    // Subcase A: transfer_from returns boolean false (silent failure token)
    set_erc20_transfer_from_result(Ok(false));
    let res_false = contract.deposit_direct_note(amount, commitment, memo.clone());
    assert_eq!(res_false, Err(b"TRANSFER_FROM_FAILED".to_vec()));
    assert_eq!(
        contract.user_note_liabilities.get(U256::from(1)),
        U256::ZERO
    );
    assert_eq!(
        contract.total_deposited_principals.get(U256::from(1)),
        U256::ZERO
    );
    assert_eq!(contract.mmr_leaf_count.get(), U256::ZERO);
    assert_eq!(contract.note_tree_root.get(), FixedBytes::ZERO);

    // Subcase B: transfer_from reverts with low-level call failure
    set_erc20_transfer_from_result(Err(b"TRANSFER_FROM_FAILED".to_vec()));
    let res_err = contract.deposit_direct_note(amount, commitment, memo);
    assert_eq!(res_err, Err(b"TRANSFER_FROM_FAILED".to_vec()));
    assert_eq!(
        contract.user_note_liabilities.get(U256::from(1)),
        U256::ZERO
    );
    assert_eq!(
        contract.total_deposited_principals.get(U256::from(1)),
        U256::ZERO
    );
    assert_eq!(contract.mmr_leaf_count.get(), U256::ZERO);
    assert_eq!(contract.note_tree_root.get(), FixedBytes::ZERO);
}

#[test]
fn test_deposit_direct_note_pos2_unverified_96_byte_memo_accepted() {
    reset_test_state();
    let owner = Address::repeat_byte(0x01);
    let client = Address::repeat_byte(0x02);
    set_msg_sender(owner);

    let mut contract = Zeltra::default();
    contract
        .init(
            owner,
            Address::repeat_byte(0x03),
            Address::repeat_byte(0x04),
        )
        .unwrap();

    set_msg_sender(client);
    let amount = U256::from(10_000_000);
    let valid_commitment = FixedBytes::from([0x01; 32]);
    let unverified_memo: Bytes = vec![0xde; 96].into();

    set_erc20_transfer_from_result(Ok(true));
    let res = contract.deposit_direct_note(amount, valid_commitment, unverified_memo);

    assert_eq!(res, Ok(()));
    assert_eq!(contract.user_note_liabilities.get(U256::from(1)), amount);
    assert_eq!(
        contract.total_deposited_principals.get(U256::from(1)),
        amount
    );
    assert_eq!(contract.mmr_leaf_count.get(), U256::from(1));

    let inserted_root = contract.note_tree_root.get();
    assert_ne!(inserted_root, FixedBytes::ZERO);
}

#[test]
fn test_set_execution_fee_recipient() {
    reset_test_state();
    let mut contract = Zeltra::default();
    let owner = Address::repeat_byte(0x01);
    let new_recipient = Address::repeat_byte(0x04);

    contract
        .init(
            owner,
            Address::repeat_byte(0x02),
            Address::repeat_byte(0x03),
        )
        .unwrap();

    // Set execution fee recipient
    set_msg_sender(owner);
    contract.set_execution_fee_recipient(new_recipient).unwrap();

    let recipient = contract.get_execution_fee_recipient().unwrap();
    assert_eq!(recipient, new_recipient);
}

#[test]
fn test_set_execution_fee_recipient_non_owner_fails() {
    reset_test_state();
    let mut contract = Zeltra::default();
    let owner = Address::repeat_byte(0x01);
    let non_owner = Address::repeat_byte(0x99);

    contract
        .init(
            owner,
            Address::repeat_byte(0x02),
            Address::repeat_byte(0x03),
        )
        .unwrap();

    // Try to set execution fee recipient as non-owner
    set_msg_sender(non_owner);
    let result = contract.set_execution_fee_recipient(Address::repeat_byte(0x04));

    assert_eq!(result, Err(b"NOT_OWNER".to_vec()));
}

#[test]
fn test_claim_execution_fees_insufficient_balance() {
    reset_test_state();
    let mut contract = Zeltra::default();
    let owner = Address::repeat_byte(0x01);
    let recipient = Address::repeat_byte(0x04);

    contract
        .init(
            owner,
            Address::repeat_byte(0x02),
            Address::repeat_byte(0x03),
        )
        .unwrap();

    // Set execution fee recipient
    set_msg_sender(owner);
    contract.set_execution_fee_recipient(recipient).unwrap();

    // Try to claim more than accumulated (0)
    let result = contract.claim_execution_fees(1, U256::from(1000));
    assert_eq!(result, Err(b"INSUFFICIENT_ACCUMULATED_FEES".to_vec()));
}

#[test]
fn test_claim_execution_fees_no_recipient_set() {
    reset_test_state();
    let mut contract = Zeltra::default();
    let owner = Address::repeat_byte(0x01);

    contract
        .init(
            owner,
            Address::repeat_byte(0x02),
            Address::repeat_byte(0x03),
        )
        .unwrap();

    // Try to claim without setting recipient
    set_msg_sender(owner);
    let result = contract.claim_execution_fees(1, U256::from(1000));
    assert_eq!(result, Err(b"NO_RECIPIENT_SET".to_vec()));
}

#[test]
fn test_merkle_tree_empty_root_matches_kav() {
    reset_test_state();
    let contract = Zeltra::default();

    let expected_empty_root = FixedBytes::from(crate::poseidon_w5_constants::EMPTY_TREE_ROOT_BYTES);
    assert_eq!(
        contract.note_tree_root().unwrap(),
        expected_empty_root,
        "Initial root must match canonical empty tree root"
    );
    assert_eq!(contract.note_tree_next_index().unwrap(), U256::ZERO);
    assert!(
        contract.is_accepted_note_root(expected_empty_root).unwrap(),
        "Canonical empty root must be accepted"
    );
}

#[test]
fn test_merkle_tree_single_insert_matches_kav() {
    reset_test_state();
    let mut contract = Zeltra::default();

    // KAV note commitment from DEC-016A
    let leaf = FixedBytes::from(alloy_primitives::hex!(
        "46d1b90c8a28c364fefdbb95bd709956e2f39a411750e6797fe1112b27635c59"
    ));
    // Canonical DEC-032 MMR bagged root after 1 insert: PoseidonW5([leaf, Fr(1), 0, 0], DOMAIN_MMR_BAG)
    let expected_root_after_1 = FixedBytes::from(alloy_primitives::hex!(
        "00cb33439376a56bb3e3cc73ef00b8cf8c14d6e69d27ecf2bd69e62ef2656844"
    ));

    set_block_timestamp(500);
    let (leaf_idx, new_root) = contract._merkle_insert(leaf).unwrap();

    assert_eq!(leaf_idx, U256::ZERO);
    assert_eq!(
        new_root, expected_root_after_1,
        "Root after 1 insert must exactly match DEC-032 canonical MMR root"
    );
    assert_eq!(contract.note_tree_root().unwrap(), expected_root_after_1);
    assert_eq!(contract.note_tree_next_index().unwrap(), U256::from(1));

    // Must be in accepted roots and in ring buffer history
    assert!(contract.is_accepted_note_root(new_root).unwrap());
    assert_eq!(
        contract.get_root_history(U256::ZERO).unwrap(),
        expected_root_after_1
    );
}

#[test]
fn test_deposit_fee_governance_timelock_and_max_cap() {
    reset_test_state();
    let mut contract = Zeltra::default();
    let owner = Address::repeat_byte(0x01);
    let non_owner = Address::repeat_byte(0x99);

    contract
        .init(
            owner,
            Address::repeat_byte(0x02),
            Address::repeat_byte(0x03),
        )
        .unwrap();

    // 1. Non-owner cannot change fee
    set_msg_sender(non_owner);
    assert_eq!(
        contract.set_deposit_fee_bps(U256::from(10)),
        Err(b"NOT_OWNER".to_vec())
    );
    assert_eq!(
        contract.propose_deposit_fee_bps(U256::from(10)),
        Err(b"NOT_OWNER".to_vec())
    );

    // 2. Fee > 50 bps hard-cap is strictly rejected
    set_msg_sender(owner);
    assert_eq!(
        contract.set_deposit_fee_bps(U256::from(51)),
        Err(b"DEPOSIT_FEE_EXCEEDS_MAX".to_vec())
    );
    assert_eq!(
        contract.propose_deposit_fee_bps(U256::from(51)),
        Err(b"DEPOSIT_FEE_EXCEEDS_MAX".to_vec())
    );

    // 3. Two-step governance with 24h timelock
    set_block_timestamp(1000);
    contract.propose_deposit_fee_bps(U256::from(15)).unwrap();

    // Calling before timelock expires must fail
    set_block_timestamp(1000 + 3600);
    assert_eq!(
        contract.execute_deposit_fee_bps(),
        Err(b"TIMELOCK_NOT_EXPIRED".to_vec())
    );

    // Calling after 24h timelock succeeds
    set_block_timestamp(1000 + 86401);
    contract.execute_deposit_fee_bps().unwrap();
    assert_eq!(contract.deposit_fee_bps().unwrap(), U256::from(15));
}

#[test]
fn test_default_uninitialized_deposit_fee() {
    reset_test_state();
    let contract = Zeltra::default();
    // Defaults to 0 bps (0.00%) when uninitialized (zero deposit fee policy)
    assert_eq!(contract.deposit_fee_bps().unwrap(), U256::ZERO);
}

#[test]
fn test_spend_private_note_success_with_change() {
    reset_test_state();
    let owner = address!("1111111111111111111111111111111111111111");
    let merchant = address!("2222222222222222222222222222222222222222");
    let stablecoin = address!("3333333333333333333333333333333333333333");
    set_msg_sender(owner);
    set_block_timestamp(1000);

    let mut contract = Zeltra::default();
    contract.init(owner, stablecoin, Address::ZERO).unwrap();

    let initial_root =
        FixedBytes::<32>::from_slice(&crate::poseidon_w5_constants::EMPTY_TREE_ROOT_BYTES);
    contract
        .accepted_note_roots
        .insert(initial_root, U256::from(1000));
    contract
        .user_note_liabilities
        .setter(U256::ZERO)
        .set(U256::from(100_000_000)); // 100 USDC liability

    let nullifier = FixedBytes::<32>::from_slice(&[0x0a; 32]);
    let change_cm = FixedBytes::<32>::from_slice(&[0x01; 32]);
    let quote_hash = FixedBytes::<32>::from_slice(&[0x05; 32]);

    let proof_a = valid_test_g1();
    let proof_b = valid_test_g2();
    let proof_c = valid_test_g1();

    let result = contract.spend_private_note(
        initial_root,
        U256::from(1),
        nullifier,
        U256::ZERO,
        change_cm,
        merchant,
        U256::from(5_000_000), // 5 USDC
        U256::from(22_500),    // protocol fee
        U256::from(23_000),    // execution fee
        U256::from(25_000),    // max execution fee
        quote_hash,
        U256::from(2000), // expiry
        U256::from(1),    // has_change = 1
        U256::ZERO,       // is_rollover = 0
        proof_a,
        proof_b,
        proof_c,
        vec![].into(),
    );

    assert_eq!(result, Ok(true));
    assert!(contract.note_nullifiers.get(nullifier));
    // Remaining liability = 100M - 5M - 22.5k - 23k = 94_954_500
    assert_eq!(
        contract.user_note_liabilities.get(U256::ZERO),
        U256::from(94_954_500)
    );
    assert_eq!(
        contract.accrued_execution_fee_liabilities.get(U256::ZERO),
        U256::from(23_000)
    );
    assert_eq!(
        contract.realized_protocol_fees.get(U256::ZERO),
        U256::from(22_500)
    );
    assert_eq!(contract.mmr_leaf_count.get(), U256::from(1));
}

#[test]
fn test_spend_private_note_double_spend_rejected() {
    reset_test_state();
    let owner = address!("1111111111111111111111111111111111111111");
    let merchant = address!("2222222222222222222222222222222222222222");
    set_msg_sender(owner);
    set_block_timestamp(1000);

    let mut contract = Zeltra::default();
    contract.init(owner, Address::ZERO, Address::ZERO).unwrap();

    let initial_root =
        FixedBytes::<32>::from_slice(&crate::poseidon_w5_constants::EMPTY_TREE_ROOT_BYTES);
    contract
        .accepted_note_roots
        .insert(initial_root, U256::from(1000));
    contract
        .user_note_liabilities
        .setter(U256::ZERO)
        .set(U256::from(100_000_000));

    let nullifier = FixedBytes::<32>::from_slice(&[0x0b; 32]);
    let change_cm = FixedBytes::<32>::from_slice(&[0x02; 32]);

    // First spend succeeds
    contract
        .spend_private_note(
            initial_root,
            U256::from(1),
            nullifier,
            U256::ZERO,
            change_cm,
            merchant,
            U256::from(5_000_000),
            U256::from(22_500),
            U256::from(23_000),
            U256::from(25_000),
            FixedBytes::ZERO,
            U256::from(2000),
            U256::from(1),
            U256::ZERO,
            valid_test_g1(),
            valid_test_g2(),
            valid_test_g1(),
            vec![].into(),
        )
        .unwrap();

    // Second spend with identical nullifier MUST revert
    let result = contract.spend_private_note(
        initial_root,
        U256::from(1),
        nullifier,
        U256::ZERO,
        change_cm,
        merchant,
        U256::from(5_000_000),
        U256::from(22_500),
        U256::from(23_000),
        U256::from(25_000),
        FixedBytes::ZERO,
        U256::from(2000),
        U256::from(1),
        U256::ZERO,
        valid_test_g1(),
        valid_test_g2(),
        valid_test_g1(),
        vec![].into(),
    );

    assert_eq!(result, Err(b"NOTE_ALREADY_SPENT".to_vec()));
}

#[test]
fn test_spend_private_note_unaccepted_root_rejected() {
    reset_test_state();
    let owner = address!("1111111111111111111111111111111111111111");
    let mut contract = Zeltra::default();
    contract.init(owner, Address::ZERO, Address::ZERO).unwrap();

    let fake_root = FixedBytes::<32>::from_slice(&[0xee; 32]);
    let nullifier = FixedBytes::<32>::from_slice(&[0xcc; 32]);

    let result = contract.spend_private_note(
        fake_root,
        U256::from(1),
        nullifier,
        U256::ZERO,
        FixedBytes::ZERO,
        Address::ZERO,
        U256::from(1_000_000),
        U256::ZERO,
        U256::ZERO,
        U256::ZERO,
        FixedBytes::ZERO,
        U256::ZERO,
        U256::ZERO,
        U256::ZERO,
        vec![0x11; 128].into(),
        vec![0x22; 256].into(),
        vec![0x33; 128].into(),
        vec![].into(),
    );

    assert_eq!(result, Err(b"UNACCEPTED_NOTE_ROOT".to_vec()));
}

#[test]
fn test_spend_private_note_expired_rejected() {
    reset_test_state();
    let owner = address!("1111111111111111111111111111111111111111");
    let mut contract = Zeltra::default();
    contract.init(owner, Address::ZERO, Address::ZERO).unwrap();

    let root = FixedBytes::<32>::from_slice(&crate::poseidon_w5_constants::EMPTY_TREE_ROOT_BYTES);
    contract.accepted_note_roots.insert(root, U256::from(1000));
    set_block_timestamp(5000);

    let result = contract.spend_private_note(
        root,
        U256::from(1),
        FixedBytes::ZERO,
        U256::ZERO,
        FixedBytes::ZERO,
        Address::ZERO,
        U256::from(1_000_000),
        U256::ZERO,
        U256::ZERO,
        U256::ZERO,
        FixedBytes::ZERO,
        U256::from(4999), // expired
        U256::ZERO,
        U256::ZERO,
        vec![0x11; 128].into(),
        vec![0x22; 256].into(),
        vec![0x33; 128].into(),
        vec![].into(),
    );

    assert_eq!(result, Err(b"TRANSACTION_EXPIRED".to_vec()));
}

#[test]
fn test_spend_private_note_execution_fee_exceeded_rejected() {
    reset_test_state();
    let owner = address!("1111111111111111111111111111111111111111");
    let mut contract = Zeltra::default();
    contract.init(owner, Address::ZERO, Address::ZERO).unwrap();

    let root = FixedBytes::<32>::from_slice(&crate::poseidon_w5_constants::EMPTY_TREE_ROOT_BYTES);
    contract.accepted_note_roots.insert(root, U256::from(1000));

    let result = contract.spend_private_note(
        root,
        U256::from(1),
        FixedBytes::ZERO,
        U256::ZERO,
        FixedBytes::ZERO,
        Address::ZERO,
        U256::from(1_000_000),
        U256::ZERO,
        U256::from(30_000), // actual fee
        U256::from(25_000), // max fee
        FixedBytes::ZERO,
        U256::ZERO,
        U256::ZERO,
        U256::ZERO,
        vec![0x11; 128].into(),
        vec![0x22; 256].into(),
        vec![0x33; 128].into(),
        vec![].into(),
    );

    assert_eq!(result, Err(b"EXECUTION_FEE_EXCEEDS_MAX".to_vec()));
}

#[test]
fn test_spend_private_note_zero_change_inconsistent_commitment_rejected() {
    reset_test_state();
    let owner = address!("1111111111111111111111111111111111111111");
    let mut contract = Zeltra::default();
    contract.init(owner, Address::ZERO, Address::ZERO).unwrap();

    let root = FixedBytes::<32>::from_slice(&crate::poseidon_w5_constants::EMPTY_TREE_ROOT_BYTES);
    contract.accepted_note_roots.insert(root, U256::from(1000));

    let non_zero_commitment = FixedBytes::<32>::from_slice(&[0x77; 32]);

    let result = contract.spend_private_note(
        root,
        U256::from(1),
        FixedBytes::ZERO,
        U256::ZERO,
        non_zero_commitment,
        Address::ZERO,
        U256::from(1_000_000),
        U256::ZERO,
        U256::ZERO,
        U256::ZERO,
        FixedBytes::ZERO,
        U256::ZERO,
        U256::ZERO, // has_change = 0 but non_zero_commitment
        U256::ZERO,
        vec![0x11; 128].into(),
        vec![0x22; 256].into(),
        vec![0x33; 128].into(),
        vec![].into(),
    );

    assert_eq!(result, Err(b"NONZERO_COMMITMENT_WITH_ZERO_CHANGE".to_vec()));
}

#[test]
fn test_spend_private_note_insufficient_note_liability_rejected() {
    reset_test_state();
    let owner = address!("1111111111111111111111111111111111111111");
    let mut contract = Zeltra::default();
    contract.init(owner, Address::ZERO, Address::ZERO).unwrap();

    let root = FixedBytes::<32>::from_slice(&crate::poseidon_w5_constants::EMPTY_TREE_ROOT_BYTES);
    contract.accepted_note_roots.insert(root, U256::from(1000));
    contract
        .user_note_liabilities
        .setter(U256::ZERO)
        .set(U256::from(1_000_000)); // only 1 USDC liability

    let result = contract.spend_private_note(
        root,
        U256::from(1),
        FixedBytes::ZERO,
        U256::ZERO,
        FixedBytes::ZERO,
        Address::ZERO,
        U256::from(5_000_000), // requires 5 USDC
        U256::ZERO,
        U256::ZERO,
        U256::ZERO,
        FixedBytes::ZERO,
        U256::ZERO,
        U256::ZERO,
        U256::ZERO,
        vec![0x11; 128].into(),
        vec![0x22; 256].into(),
        vec![0x33; 128].into(),
        vec![].into(),
    );

    assert_eq!(result, Err(b"INSUFFICIENT_NOTE_LIABILITY".to_vec()));
}

#[test]
fn test_spend_private_note_invalid_groth16_proof_rejected() {
    reset_test_state();
    let owner = address!("1111111111111111111111111111111111111111");
    let mut contract = Zeltra::default();
    contract.init(owner, Address::ZERO, Address::ZERO).unwrap();

    let root = FixedBytes::<32>::from_slice(&crate::poseidon_w5_constants::EMPTY_TREE_ROOT_BYTES);
    contract.accepted_note_roots.insert(root, U256::from(1000));
    contract
        .user_note_liabilities
        .setter(U256::ZERO)
        .set(U256::from(100_000_000));

    // Set mock pairing check to return false (0)
    set_pairing_result(false);

    let result = contract.spend_private_note(
        root,
        U256::from(1),
        FixedBytes::ZERO,
        U256::ZERO,
        FixedBytes::ZERO,
        Address::ZERO,
        U256::from(1_000_000),
        U256::ZERO,
        U256::ZERO,
        U256::ZERO,
        FixedBytes::ZERO,
        U256::ZERO,
        U256::ZERO,
        U256::ZERO,
        valid_test_g1(),
        valid_test_g2(),
        valid_test_g1(),
        vec![].into(),
    );

    assert_eq!(result, Err(b"INVALID_GROTH16_NOTE_PROOF".to_vec()));
}

#[test]
fn test_mmr_hyperbridge_out_of_bounds_leaf_rejected() {
    reset_test_state();
    let owner = address!("1111111111111111111111111111111111111111");
    let mut contract = Zeltra::default();
    contract.init(owner, Address::ZERO, Address::ZERO).unwrap();

    let root = FixedBytes::<32>::from_slice(&crate::poseidon_w5_constants::EMPTY_TREE_ROOT_BYTES);
    contract.accepted_note_roots.insert(root, U256::from(1000));
    contract
        .user_note_liabilities
        .setter(U256::ZERO)
        .set(U256::from(100_000_000));

    // Attempting spend with leaf_count == 0 must fail closed
    let result = contract.spend_private_note(
        root,
        U256::ZERO, // leaf_count == 0 (illegal in DEC-032)
        FixedBytes::ZERO,
        U256::ZERO,
        FixedBytes::ZERO,
        Address::ZERO,
        U256::from(1_000_000),
        U256::ZERO,
        U256::ZERO,
        U256::ZERO,
        FixedBytes::ZERO,
        U256::ZERO,
        U256::ZERO,
        U256::ZERO,
        vec![0x11; 128].into(),
        vec![0x22; 256].into(),
        vec![0x33; 128].into(),
        vec![].into(),
    );

    assert_eq!(result, Err(b"INVALID_LEAF_COUNT".to_vec()));
}

#[test]
fn test_mmr_tampered_leaf_count_rejected() {
    reset_test_state();
    let owner = address!("1111111111111111111111111111111111111111");
    let mut contract = Zeltra::default();
    contract.init(owner, Address::ZERO, Address::ZERO).unwrap();

    // Valid leaf insert
    let leaf = FixedBytes::from(alloy_primitives::hex!(
        "46d1b90c8a28c364fefdbb95bd709956e2f39a411750e6797fe1112b27635c59"
    ));
    let (_, _canonical_root) = contract._mmr_insert(leaf, vec![]).unwrap();

    // Tampering with leaf_count produces a different root that is not accepted
    let fake_root = FixedBytes::repeat_byte(0x99);
    assert!(!contract.is_accepted_note_root(fake_root).unwrap());

    let result = contract.spend_private_note(
        fake_root,
        U256::from(999), // tampered leaf count
        FixedBytes::ZERO,
        U256::ZERO,
        FixedBytes::ZERO,
        Address::ZERO,
        U256::from(1_000_000),
        U256::ZERO,
        U256::ZERO,
        U256::ZERO,
        FixedBytes::ZERO,
        U256::ZERO,
        U256::ZERO,
        U256::ZERO,
        vec![0x11; 128].into(),
        vec![0x22; 256].into(),
        vec![0x33; 128].into(),
        vec![].into(),
    );

    assert_eq!(result, Err(b"UNACCEPTED_NOTE_ROOT".to_vec()));
}

#[test]
fn test_mmr_wrap_around_scalar_rejected() {
    reset_test_state();
    let mut contract = Zeltra::default();

    // 0xff...ff is >= r (modulus of BLS12-381 Fr)
    let non_canonical_leaf = FixedBytes::repeat_byte(0xff);
    let result = contract._mmr_insert(non_canonical_leaf, vec![]);

    assert_eq!(result, Err(b"INVALID_LEAF_SCALAR".to_vec()));
}

#[test]
fn test_mmr_corrupted_peak_non_canonical_rejected() {
    reset_test_state();
    let mut contract = Zeltra::default();
    let owner = address!("1111111111111111111111111111111111111111");
    contract.init(owner, Address::ZERO, Address::ZERO).unwrap();

    // Set leaf_count = 1 (bit 0 is 1). Next insert (leaf_index=1) will carry and load mmr_peaks[0]
    contract.mmr_leaf_count.set(U256::from(1));
    // Inject non-canonical bytes (>= r) into peak height 0
    contract.mmr_peaks.insert(U256::ZERO, FixedBytes::repeat_byte(0xff));

    let valid_leaf = FixedBytes::from(alloy_primitives::hex!(
        "46d1b90c8a28c364fefdbb95bd709956e2f39a411750e6797fe1112b27635c59"
    ));
    let result = contract._mmr_insert(valid_leaf, vec![]);
    assert_eq!(result, Err(b"CORRUPTED_PEAK".to_vec()));
}

#[test]
fn test_mmr_corrupted_peak_empty_slot_rejected() {
    reset_test_state();
    let mut contract = Zeltra::default();
    let owner = address!("1111111111111111111111111111111111111111");
    contract.init(owner, Address::ZERO, Address::ZERO).unwrap();

    // Set leaf_count = 1. A peak at height 0 is mathematically REQUIRED because bit 0 of 1 is 1.
    // However, storage slot mmr_peaks[0] is empty (FixedBytes::ZERO).
    contract.mmr_leaf_count.set(U256::from(1));
    assert_eq!(contract.mmr_peaks.get(U256::ZERO), FixedBytes::ZERO);

    let valid_leaf = FixedBytes::from(alloy_primitives::hex!(
        "46d1b90c8a28c364fefdbb95bd709956e2f39a411750e6797fe1112b27635c59"
    ));
    let result = contract._mmr_insert(valid_leaf, vec![]);
    // With zero-check guard in place, empty peak slot is strictly rejected as CORRUPTED_PEAK!
    assert_eq!(result, Err(b"CORRUPTED_PEAK".to_vec()));
}

#[test]
fn test_mmr_corrupted_peak_empty_active_peak_in_bagging_rejected() {
    reset_test_state();
    let mut contract = Zeltra::default();
    let owner = address!("1111111111111111111111111111111111111111");
    contract.init(owner, Address::ZERO, Address::ZERO).unwrap();

    // Set leaf_count = 2 (binary 10). Next insert will be idx = 2 (bit 0 is 0),
    // so it puts new peak at height 0 and does not carry.
    // New leaf count will be 3 (binary 11: peaks at height 1 and height 0).
    // But height 1 is empty (FixedBytes::ZERO)!
    contract.mmr_leaf_count.set(U256::from(2));
    assert_eq!(contract.mmr_peaks.get(U256::from(1)), FixedBytes::ZERO);

    let valid_leaf = FixedBytes::from(alloy_primitives::hex!(
        "46d1b90c8a28c364fefdbb95bd709956e2f39a411750e6797fe1112b27635c59"
    ));
    let result = contract._mmr_insert(valid_leaf, vec![]);
    // Phase 2 peak bagging loop detects missing active peak at height 1 and reverts with CORRUPTED_PEAK!
    assert_eq!(result, Err(b"CORRUPTED_PEAK".to_vec()));
}

// ═══════════════════════════════════════════════════════════════════════
// DEC-033: Generational Epoch Window & Rollover Tests (Positive & Negative)
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_epoch_normal_spend_in_same_epoch() {
    reset_test_state();
    let owner = address!("1111111111111111111111111111111111111111");
    let merchant = address!("2222222222222222222222222222222222222222");
    set_msg_sender(owner);
    set_block_timestamp(1000);

    let mut contract = Zeltra::default();
    contract.init(owner, Address::ZERO, Address::ZERO).unwrap();

    assert_eq!(contract.get_current_epoch().unwrap(), 0);

    let root = FixedBytes::<32>::from_slice(&crate::poseidon_w5_constants::EMPTY_TREE_ROOT_BYTES);
    contract.accepted_note_roots.insert(root, U256::from(1000));
    contract
        .user_note_liabilities
        .setter(U256::ZERO)
        .set(U256::from(50_000_000));

    let nullifier = FixedBytes::<32>::from_slice(&[0x1a; 32]);
    let change_cm = FixedBytes::<32>::from_slice(&[0x1b; 32]);

    // Spend in epoch 0 with note_epoch_id = 0
    let res = contract.spend_private_note(
        root,
        U256::from(1),
        nullifier,
        U256::ZERO, // note_epoch_id = 0
        change_cm,
        merchant,
        U256::from(5_000_000),
        U256::from(22_500),
        U256::from(23_000),
        U256::from(25_000),
        FixedBytes::ZERO,
        U256::from(2000),
        U256::from(1),
        U256::ZERO, // is_rollover = 0
        valid_test_g1(),
        valid_test_g2(),
        valid_test_g1(),
        vec![].into(),
    );

    assert_eq!(res, Ok(true));
    assert!(contract
        .is_generational_nullifier_spent(0, nullifier)
        .unwrap());
}

#[test]
fn test_epoch_grace_window_spend_epoch_minus_one() {
    reset_test_state();
    let owner = address!("1111111111111111111111111111111111111111");
    let merchant = address!("2222222222222222222222222222222222222222");
    set_msg_sender(owner);
    set_block_timestamp(1000);

    let mut contract = Zeltra::default();
    contract.init(owner, Address::ZERO, Address::ZERO).unwrap();

    // Advance to epoch 1
    set_block_timestamp(2000);
    let new_epoch = contract.rotate_epoch().unwrap();
    assert_eq!(new_epoch, 1);
    assert_eq!(contract.get_current_epoch().unwrap(), 1);

    let root = FixedBytes::<32>::from_slice(&crate::poseidon_w5_constants::EMPTY_TREE_ROOT_BYTES);
    contract.accepted_note_roots.insert(root, U256::from(1000));
    contract
        .user_note_liabilities
        .setter(U256::ZERO)
        .set(U256::from(50_000_000));

    let nullifier = FixedBytes::<32>::from_slice(&[0x2a; 32]);
    let change_cm = FixedBytes::<32>::from_slice(&[0x2b; 32]);

    // Spend in epoch 1 with note from epoch 0 (E - 1 grace window)
    let res = contract.spend_private_note(
        root,
        U256::from(1),
        nullifier,
        U256::ZERO, // note_epoch_id = 0 (E-1)
        change_cm,
        merchant,
        U256::from(5_000_000),
        U256::from(22_500),
        U256::from(23_000),
        U256::from(25_000),
        FixedBytes::ZERO,
        U256::from(5000),
        U256::from(1),
        U256::ZERO,
        valid_test_g1(),
        valid_test_g2(),
        valid_test_g1(),
        vec![].into(),
    );

    assert_eq!(res, Ok(true));
    assert!(contract
        .is_generational_nullifier_spent(0, nullifier)
        .unwrap());
    assert_eq!(contract.mmr_leaf_count.get(), U256::from(1));
}

#[test]
fn test_epoch_standalone_refresh_execution() {
    reset_test_state();
    let owner = address!("1111111111111111111111111111111111111111");
    set_msg_sender(owner);
    set_block_timestamp(1000);

    let mut contract = Zeltra::default();
    contract.init(owner, Address::ZERO, Address::ZERO).unwrap();

    let root = FixedBytes::<32>::from_slice(&crate::poseidon_w5_constants::EMPTY_TREE_ROOT_BYTES);
    contract.accepted_note_roots.insert(root, U256::from(1000));
    contract
        .user_note_liabilities
        .setter(U256::ZERO)
        .set(U256::from(50_000_000));

    let nullifier = FixedBytes::<32>::from_slice(&[0x3a; 32]);
    let renewed_cm = FixedBytes::<32>::from_slice(&[0x3b; 32]);

    // Standalone refresh: is_rollover = 1, merchant = 0, pfee = 0, has_change = 1
    let res = contract.spend_private_note(
        root,
        U256::from(1),
        nullifier,
        U256::ZERO, // note_epoch_id = 0
        renewed_cm,
        Address::ZERO,
        U256::ZERO,         // merchant_amount = 0
        U256::ZERO,         // protocol_fee = 0
        U256::from(21_275), // execution fee with 15% execution overhead buffer
        U256::from(25_000),
        FixedBytes::ZERO,
        U256::from(2000),
        U256::from(1), // has_change = 1
        U256::from(1), // is_rollover = 1
        valid_test_g1(),
        valid_test_g2(),
        valid_test_g1(),
        vec![].into(),
    );

    assert_eq!(res, Ok(true));
    assert!(contract
        .is_generational_nullifier_spent(0, nullifier)
        .unwrap());
    assert_eq!(
        contract.accrued_execution_fee_liabilities.get(U256::ZERO),
        U256::from(21_275)
    );
}

#[test]
fn test_epoch_expired_note_direct_spend_rejected() {
    reset_test_state();
    let owner = address!("1111111111111111111111111111111111111111");
    set_msg_sender(owner);
    set_block_timestamp(1000);

    let mut contract = Zeltra::default();
    contract.init(owner, Address::ZERO, Address::ZERO).unwrap();

    // Advance epoch twice: 0 -> 1 -> 2 (active window is 2 and 1, epoch 0 is expired)
    contract.rotate_epoch().unwrap();
    contract.rotate_epoch().unwrap();
    assert_eq!(contract.get_current_epoch().unwrap(), 2);

    let root = FixedBytes::<32>::from_slice(&crate::poseidon_w5_constants::EMPTY_TREE_ROOT_BYTES);
    contract.accepted_note_roots.insert(root, U256::from(1000));
    contract
        .user_note_liabilities
        .setter(U256::ZERO)
        .set(U256::from(50_000_000));

    let nullifier = FixedBytes::<32>::from_slice(&[0xe1; 32]);

    // Attempting to spend note from epoch 0 (E - 2) MUST revert EPOCH_EXPIRED_REQUIRE_ROLLOVER
    let res = contract.spend_private_note(
        root,
        U256::from(1),
        nullifier,
        U256::ZERO, // note_epoch_id = 0 (< current_epoch - 1)
        FixedBytes::ZERO,
        Address::ZERO,
        U256::from(1_000_000),
        U256::ZERO,
        U256::from(10_000),
        U256::from(20_000),
        FixedBytes::ZERO,
        U256::ZERO,
        U256::ZERO,
        U256::ZERO,
        vec![0x11; 128].into(),
        vec![0x22; 256].into(),
        vec![0x33; 128].into(),
        vec![].into(),
    );

    assert_eq!(res, Err(b"EPOCH_EXPIRED_REQUIRE_ROLLOVER".to_vec()));
}

#[test]
fn test_epoch_in_future_rejected() {
    reset_test_state();
    let owner = address!("1111111111111111111111111111111111111111");
    set_msg_sender(owner);
    set_block_timestamp(1000);

    let mut contract = Zeltra::default();
    contract.init(owner, Address::ZERO, Address::ZERO).unwrap();

    let root = FixedBytes::<32>::from_slice(&crate::poseidon_w5_constants::EMPTY_TREE_ROOT_BYTES);
    contract.accepted_note_roots.insert(root, U256::from(1000));
    contract
        .user_note_liabilities
        .setter(U256::ZERO)
        .set(U256::from(50_000_000));

    // Note epoch 5 when current_epoch is 0
    let res = contract.spend_private_note(
        root,
        U256::from(1),
        FixedBytes::<32>::repeat_byte(0x12),
        U256::from(5), // Future epoch
        FixedBytes::ZERO,
        Address::ZERO,
        U256::from(1_000_000),
        U256::ZERO,
        U256::from(10_000),
        U256::from(20_000),
        FixedBytes::ZERO,
        U256::ZERO,
        U256::ZERO,
        U256::ZERO,
        vec![0x11; 128].into(),
        vec![0x22; 256].into(),
        vec![0x33; 128].into(),
        vec![].into(),
    );

    assert_eq!(res, Err(b"EPOCH_IN_FUTURE".to_vec()));
}

#[test]
fn test_epoch_slot_desynchronized_rejected() {
    reset_test_state();
    let owner = address!("1111111111111111111111111111111111111111");
    set_msg_sender(owner);
    set_block_timestamp(1000);

    let mut contract = Zeltra::default();
    contract.init(owner, Address::ZERO, Address::ZERO).unwrap();

    let root = FixedBytes::<32>::from_slice(&crate::poseidon_w5_constants::EMPTY_TREE_ROOT_BYTES);
    contract.accepted_note_roots.insert(root, U256::from(1000));
    contract
        .user_note_liabilities
        .setter(U256::ZERO)
        .set(U256::from(50_000_000));

    // Corrupt slot 0 epoch id to 999
    contract
        .generation_epoch_id
        .setter(U256::ZERO)
        .set(U256::from(999));

    let res = contract.spend_private_note(
        root,
        U256::from(1),
        FixedBytes::<32>::repeat_byte(0x12),
        U256::ZERO,
        FixedBytes::ZERO,
        Address::ZERO,
        U256::from(1_000_000),
        U256::ZERO,
        U256::from(10_000),
        U256::from(20_000),
        FixedBytes::ZERO,
        U256::ZERO,
        U256::ZERO,
        U256::ZERO,
        vec![0x11; 128].into(),
        vec![0x22; 256].into(),
        vec![0x33; 128].into(),
        vec![].into(),
    );

    assert_eq!(res, Err(b"EPOCH_SLOT_DESYNCHRONIZED".to_vec()));
}

#[test]
fn test_epoch_rollover_with_nonzero_merchant_rejected() {
    reset_test_state();
    let owner = address!("1111111111111111111111111111111111111111");
    let merchant = address!("2222222222222222222222222222222222222222");
    set_msg_sender(owner);
    set_block_timestamp(1000);

    let mut contract = Zeltra::default();
    contract.init(owner, Address::ZERO, Address::ZERO).unwrap();

    let root = FixedBytes::<32>::from_slice(&crate::poseidon_w5_constants::EMPTY_TREE_ROOT_BYTES);
    contract.accepted_note_roots.insert(root, U256::from(1000));

    // is_rollover = 1 with merchant > 0 MUST fail
    let res = contract.spend_private_note(
        root,
        U256::from(1),
        FixedBytes::<32>::repeat_byte(0x33),
        U256::ZERO,
        FixedBytes::<32>::repeat_byte(0x44),
        merchant,
        U256::from(5_000_000), // Non-zero merchant
        U256::ZERO,
        U256::from(10_000),
        U256::from(20_000),
        FixedBytes::ZERO,
        U256::ZERO,
        U256::from(1),
        U256::from(1), // is_rollover = 1
        vec![0x11; 128].into(),
        vec![0x22; 256].into(),
        vec![0x33; 128].into(),
        vec![].into(),
    );

    assert_eq!(
        res,
        Err(b"MERCHANT_AMOUNT_MUST_BE_ZERO_IN_ROLLOVER".to_vec())
    );
}

#[test]
fn test_epoch_rollover_with_nonzero_protocol_fee_rejected() {
    reset_test_state();
    let owner = address!("1111111111111111111111111111111111111111");
    set_msg_sender(owner);
    set_block_timestamp(1000);

    let mut contract = Zeltra::default();
    contract.init(owner, Address::ZERO, Address::ZERO).unwrap();

    let root = FixedBytes::<32>::from_slice(&crate::poseidon_w5_constants::EMPTY_TREE_ROOT_BYTES);
    contract.accepted_note_roots.insert(root, U256::from(1000));

    // is_rollover = 1 with protocol_fee > 0 MUST fail
    let res = contract.spend_private_note(
        root,
        U256::from(1),
        FixedBytes::<32>::repeat_byte(0x33),
        U256::ZERO,
        FixedBytes::<32>::repeat_byte(0x44),
        Address::ZERO,
        U256::ZERO,
        U256::from(22_500), // Non-zero protocol fee
        U256::from(10_000),
        U256::from(20_000),
        FixedBytes::ZERO,
        U256::ZERO,
        U256::from(1),
        U256::from(1), // is_rollover = 1
        vec![0x11; 128].into(),
        vec![0x22; 256].into(),
        vec![0x33; 128].into(),
        vec![].into(),
    );

    assert_eq!(res, Err(b"PROTOCOL_FEE_MUST_BE_ZERO_IN_ROLLOVER".to_vec()));
}

#[test]
fn test_epoch_rollover_without_change_note_rejected() {
    reset_test_state();
    let owner = address!("1111111111111111111111111111111111111111");
    set_msg_sender(owner);
    set_block_timestamp(1000);

    let mut contract = Zeltra::default();
    contract.init(owner, Address::ZERO, Address::ZERO).unwrap();

    let root = FixedBytes::<32>::from_slice(&crate::poseidon_w5_constants::EMPTY_TREE_ROOT_BYTES);
    contract.accepted_note_roots.insert(root, U256::from(1000));

    // is_rollover = 1 with has_change = 0 MUST fail
    let res = contract.spend_private_note(
        root,
        U256::from(1),
        FixedBytes::<32>::repeat_byte(0x33),
        U256::ZERO,
        FixedBytes::ZERO,
        Address::ZERO,
        U256::ZERO,
        U256::ZERO,
        U256::from(10_000),
        U256::from(20_000),
        FixedBytes::ZERO,
        U256::ZERO,
        U256::ZERO,    // has_change = 0
        U256::from(1), // is_rollover = 1
        vec![0x11; 128].into(),
        vec![0x22; 256].into(),
        vec![0x33; 128].into(),
        vec![].into(),
    );

    assert_eq!(res, Err(b"ROLLOVER_MUST_HAVE_CHANGE".to_vec()));
}

#[test]
fn test_epoch_rotation_non_owner_rejected() {
    reset_test_state();
    let owner = address!("1111111111111111111111111111111111111111");
    let non_owner = address!("9999999999999999999999999999999999999999");
    set_msg_sender(owner);

    let mut contract = Zeltra::default();
    contract.init(owner, Address::ZERO, Address::ZERO).unwrap();

    set_msg_sender(non_owner);
    let res = contract.rotate_epoch();
    assert_eq!(res, Err(b"NOT_OWNER".to_vec()));
}

#[test]
fn test_epoch_generational_nullifier_double_spend_rejected() {
    reset_test_state();
    let owner = address!("1111111111111111111111111111111111111111");
    let merchant = address!("2222222222222222222222222222222222222222");
    set_msg_sender(owner);
    set_block_timestamp(1000);

    let mut contract = Zeltra::default();
    contract.init(owner, Address::ZERO, Address::ZERO).unwrap();

    let root = FixedBytes::<32>::from_slice(&crate::poseidon_w5_constants::EMPTY_TREE_ROOT_BYTES);
    contract.accepted_note_roots.insert(root, U256::from(1000));
    contract
        .user_note_liabilities
        .setter(U256::ZERO)
        .set(U256::from(100_000_000));

    let nullifier = FixedBytes::<32>::from_slice(&[0x4a; 32]);
    let change_cm = FixedBytes::<32>::from_slice(&[0x4b; 32]);

    // Spend succeeds
    contract
        .spend_private_note(
            root,
            U256::from(1),
            nullifier,
            U256::ZERO,
            change_cm,
            merchant,
            U256::from(5_000_000),
            U256::from(22_500),
            U256::from(23_000),
            U256::from(25_000),
            FixedBytes::ZERO,
            U256::from(2000),
            U256::from(1),
            U256::ZERO,
            valid_test_g1(),
            valid_test_g2(),
            valid_test_g1(),
            vec![].into(),
        )
        .unwrap();

    // Attempt second spend in the same epoch with same nullifier
    let res = contract.spend_private_note(
        root,
        U256::from(1),
        nullifier,
        U256::ZERO,
        change_cm,
        merchant,
        U256::from(5_000_000),
        U256::from(22_500),
        U256::from(23_000),
        U256::from(25_000),
        FixedBytes::ZERO,
        U256::from(2000),
        U256::from(1),
        U256::ZERO,
        valid_test_g1(),
        valid_test_g2(),
        valid_test_g1(),
        vec![].into(),
    );

    assert_eq!(res, Err(b"NOTE_ALREADY_SPENT".to_vec()));
}

#[test]
fn test_spend_joinsplit_success_1_in_dummy_change() {
    reset_test_state();
    let owner = address!("1111111111111111111111111111111111111111");
    let merchant = address!("2222222222222222222222222222222222222222");
    let stablecoin = address!("3333333333333333333333333333333333333333");
    set_msg_sender(owner);
    set_block_timestamp(1000);

    let mut contract = Zeltra::default();
    contract.init(owner, stablecoin, Address::ZERO).unwrap();

    let initial_root =
        FixedBytes::<32>::from_slice(&crate::poseidon_w5_constants::EMPTY_TREE_ROOT_BYTES);
    contract
        .accepted_note_roots
        .insert(initial_root, U256::from(1000));
    contract
        .user_note_liabilities
        .setter(U256::ZERO)
        .set(U256::from(100_000_000)); // 100 USDC liability

    let nullifier_1 = FixedBytes::<32>::from_slice(&[0x1a; 32]);
    let nullifier_2 = FixedBytes::<32>::from_slice(&[0x1b; 32]);
    let change_cm_1 = FixedBytes::<32>::from_slice(&[0x2a; 32]);
    let change_cm_2 = FixedBytes::<32>::from_slice(&[0x2b; 32]);
    let quote_hash = FixedBytes::<32>::from_slice(&[0x3a; 32]);

    let proof_a = valid_test_g1();
    let proof_b = valid_test_g2();
    let proof_c = valid_test_g1();

    let result = contract.spend_joinsplit(
        proof_a,
        proof_b,
        proof_c,
        initial_root,
        U256::from(1),
        nullifier_1,
        nullifier_2,
        U256::ZERO,
        U256::ZERO,
        change_cm_1,
        change_cm_2,
        merchant,
        U256::from(5_000_000), // 5 USDC
        U256::from(22_500),    // protocol fee
        U256::from(23_000),    // execution fee
        U256::from(25_000),    // max execution fee
        quote_hash,
        U256::from(2000), // expiry
        U256::from(1),    // flags_packed: 1 change note, 1 dummy
        U256::ZERO,       // is_rollover: 0
        vec![0u8; 96].into(),
        vec![0u8; 96].into(),
    );

    assert_eq!(result, Ok(true));
    assert!(contract.note_nullifiers.get(nullifier_1));
    assert!(contract.note_nullifiers.get(nullifier_2));
    // Remaining liability = 100M - 5M - 22.5k - 23k = 94_954_500
    assert_eq!(
        contract.user_note_liabilities.get(U256::ZERO),
        U256::from(94_954_500)
    );
    assert_eq!(
        contract.accrued_execution_fee_liabilities.get(U256::ZERO),
        U256::from(23_000)
    );
    assert_eq!(
        contract.realized_protocol_fees.get(U256::ZERO),
        U256::from(22_500)
    );
    // Direction B Constant Arity: exactly 2 MMR insertions
    assert_eq!(contract.mmr_leaf_count.get(), U256::from(2));
}

#[test]
fn test_spend_joinsplit_singularity_aliasing_rejected() {
    reset_test_state();
    let owner = address!("1111111111111111111111111111111111111111");
    let merchant = address!("2222222222222222222222222222222222222222");
    set_msg_sender(owner);
    set_block_timestamp(1000);

    let mut contract = Zeltra::default();
    contract.init(owner, Address::ZERO, Address::ZERO).unwrap();

    let initial_root =
        FixedBytes::<32>::from_slice(&crate::poseidon_w5_constants::EMPTY_TREE_ROOT_BYTES);
    contract
        .accepted_note_roots
        .insert(initial_root, U256::from(1000));
    contract
        .user_note_liabilities
        .setter(U256::ZERO)
        .set(U256::from(100_000_000));

    let same_nullifier = FixedBytes::<32>::from_slice(&[0x77; 32]);
    let change_cm_1 = FixedBytes::<32>::from_slice(&[0x2a; 32]);
    let change_cm_2 = FixedBytes::<32>::from_slice(&[0x2b; 32]);

    // Attempt spend with identical nullifier_1 and nullifier_2
    let result = contract.spend_joinsplit(
        vec![0x11; 128].into(),
        vec![0x22; 256].into(),
        vec![0x33; 128].into(),
        initial_root,
        U256::from(1),
        same_nullifier,
        same_nullifier,
        U256::ZERO,
        U256::ZERO,
        change_cm_1,
        change_cm_2,
        merchant,
        U256::from(5_000_000),
        U256::from(22_500),
        U256::from(23_000),
        U256::from(25_000),
        FixedBytes::ZERO,
        U256::from(2000),
        U256::ZERO,
        U256::ZERO,
        vec![0u8; 96].into(),
        vec![0u8; 96].into(),
    );

    assert_eq!(result, Err(b"DuplicateNullifierInSameTx".to_vec()));
    assert!(!contract.note_nullifiers.get(same_nullifier));
    assert_eq!(
        contract.user_note_liabilities.get(U256::ZERO),
        U256::from(100_000_000)
    );
    assert_eq!(contract.mmr_leaf_count.get(), U256::ZERO);
}

#[test]
fn test_spend_joinsplit_double_spend_nullifier_1_rejected() {
    reset_test_state();
    let owner = address!("1111111111111111111111111111111111111111");
    let merchant = address!("2222222222222222222222222222222222222222");
    set_msg_sender(owner);
    set_block_timestamp(1000);

    let mut contract = Zeltra::default();
    contract.init(owner, Address::ZERO, Address::ZERO).unwrap();

    let initial_root =
        FixedBytes::<32>::from_slice(&crate::poseidon_w5_constants::EMPTY_TREE_ROOT_BYTES);
    contract
        .accepted_note_roots
        .insert(initial_root, U256::from(1000));
    contract
        .user_note_liabilities
        .setter(U256::ZERO)
        .set(U256::from(100_000_000));

    let nf1 = FixedBytes::<32>::from_slice(&[0x88; 32]);
    let nf2 = FixedBytes::<32>::from_slice(&[0x89; 32]);

    // Pre-mark nullifier 1 as spent
    contract.note_nullifiers.insert(nf1, true);

    let result = contract.spend_joinsplit(
        vec![0x11; 128].into(),
        vec![0x22; 256].into(),
        vec![0x33; 128].into(),
        initial_root,
        U256::from(1),
        nf1,
        nf2,
        U256::ZERO,
        U256::ZERO,
        FixedBytes::ZERO,
        FixedBytes::ZERO,
        merchant,
        U256::from(5_000_000),
        U256::from(22_500),
        U256::from(23_000),
        U256::from(25_000),
        FixedBytes::ZERO,
        U256::from(2000),
        U256::ZERO,
        U256::ZERO,
        vec![0u8; 96].into(),
        vec![0u8; 96].into(),
    );

    assert_eq!(result, Err(b"Nullifier1AlreadySpent".to_vec()));
    assert!(!contract.note_nullifiers.get(nf2));
    assert_eq!(
        contract.user_note_liabilities.get(U256::ZERO),
        U256::from(100_000_000)
    );
}

#[test]
fn test_spend_joinsplit_double_spend_nullifier_2_rejected() {
    reset_test_state();
    let owner = address!("1111111111111111111111111111111111111111");
    let merchant = address!("2222222222222222222222222222222222222222");
    set_msg_sender(owner);
    set_block_timestamp(1000);

    let mut contract = Zeltra::default();
    contract.init(owner, Address::ZERO, Address::ZERO).unwrap();

    let initial_root =
        FixedBytes::<32>::from_slice(&crate::poseidon_w5_constants::EMPTY_TREE_ROOT_BYTES);
    contract
        .accepted_note_roots
        .insert(initial_root, U256::from(1000));
    contract
        .user_note_liabilities
        .setter(U256::ZERO)
        .set(U256::from(100_000_000));

    let nf1 = FixedBytes::<32>::from_slice(&[0x98; 32]);
    let nf2 = FixedBytes::<32>::from_slice(&[0x99; 32]);

    // Pre-mark nullifier 2 as spent
    contract.note_nullifiers.insert(nf2, true);

    let result = contract.spend_joinsplit(
        vec![0x11; 128].into(),
        vec![0x22; 256].into(),
        vec![0x33; 128].into(),
        initial_root,
        U256::from(1),
        nf1,
        nf2,
        U256::ZERO,
        U256::ZERO,
        FixedBytes::ZERO,
        FixedBytes::ZERO,
        merchant,
        U256::from(5_000_000),
        U256::from(22_500),
        U256::from(23_000),
        U256::from(25_000),
        FixedBytes::ZERO,
        U256::from(2000),
        U256::ZERO,
        U256::ZERO,
        vec![0u8; 96].into(),
        vec![0u8; 96].into(),
    );

    assert_eq!(result, Err(b"Nullifier2AlreadySpent".to_vec()));
    assert!(!contract.note_nullifiers.get(nf1));
    assert_eq!(
        contract.user_note_liabilities.get(U256::ZERO),
        U256::from(100_000_000)
    );
}

#[test]
fn test_spend_joinsplit_quote_expired_rejected() {
    reset_test_state();
    let owner = address!("1111111111111111111111111111111111111111");
    let merchant = address!("2222222222222222222222222222222222222222");
    set_msg_sender(owner);
    set_block_timestamp(5000);

    let mut contract = Zeltra::default();
    contract.init(owner, Address::ZERO, Address::ZERO).unwrap();

    let initial_root =
        FixedBytes::<32>::from_slice(&crate::poseidon_w5_constants::EMPTY_TREE_ROOT_BYTES);
    contract
        .accepted_note_roots
        .insert(initial_root, U256::from(1000));
    contract
        .user_note_liabilities
        .setter(U256::ZERO)
        .set(U256::from(100_000_000));

    let nf1 = FixedBytes::<32>::from_slice(&[0xa1; 32]);
    let nf2 = FixedBytes::<32>::from_slice(&[0xa2; 32]);

    let result = contract.spend_joinsplit(
        vec![0x11; 128].into(),
        vec![0x22; 256].into(),
        vec![0x33; 128].into(),
        initial_root,
        U256::from(1),
        nf1,
        nf2,
        U256::ZERO,
        U256::ZERO,
        FixedBytes::ZERO,
        FixedBytes::ZERO,
        merchant,
        U256::from(5_000_000),
        U256::from(22_500),
        U256::from(23_000),
        U256::from(25_000),
        FixedBytes::ZERO,
        U256::from(4999), // expiry < 5000
        U256::ZERO,
        U256::ZERO,
        vec![0u8; 96].into(),
        vec![0u8; 96].into(),
    );

    assert_eq!(result, Err(b"QuoteExpired".to_vec()));
}

#[test]
fn test_spend_joinsplit_insufficient_liability_rejected() {
    reset_test_state();
    let owner = address!("1111111111111111111111111111111111111111");
    let merchant = address!("2222222222222222222222222222222222222222");
    set_msg_sender(owner);
    set_block_timestamp(1000);

    let mut contract = Zeltra::default();
    contract.init(owner, Address::ZERO, Address::ZERO).unwrap();

    let initial_root =
        FixedBytes::<32>::from_slice(&crate::poseidon_w5_constants::EMPTY_TREE_ROOT_BYTES);
    contract
        .accepted_note_roots
        .insert(initial_root, U256::from(1000));
    contract
        .user_note_liabilities
        .setter(U256::ZERO)
        .set(U256::from(1_000_000)); // Only 1 USDC liability

    let nf1 = FixedBytes::<32>::from_slice(&[0xb1; 32]);
    let nf2 = FixedBytes::<32>::from_slice(&[0xb2; 32]);

    // Attempt spend of 5 USDC (> 1 USDC liability)
    let result = contract.spend_joinsplit(
        vec![0x11; 128].into(),
        vec![0x22; 256].into(),
        vec![0x33; 128].into(),
        initial_root,
        U256::from(1),
        nf1,
        nf2,
        U256::ZERO,
        U256::ZERO,
        FixedBytes::ZERO,
        FixedBytes::ZERO,
        merchant,
        U256::from(5_000_000),
        U256::from(22_500),
        U256::from(23_000),
        U256::from(25_000),
        FixedBytes::ZERO,
        U256::from(2000),
        U256::ZERO,
        U256::ZERO,
        vec![0u8; 96].into(),
        vec![0u8; 96].into(),
    );

    assert_eq!(result, Err(b"INSUFFICIENT_NOTE_LIABILITY".to_vec()));
}

#[test]
fn test_spend_joinsplit_invalid_proof_rejected() {
    reset_test_state();
    let owner = address!("1111111111111111111111111111111111111111");
    let merchant = address!("2222222222222222222222222222222222222222");
    set_msg_sender(owner);
    set_block_timestamp(1000);

    let mut contract = Zeltra::default();
    contract.init(owner, Address::ZERO, Address::ZERO).unwrap();

    let initial_root =
        FixedBytes::<32>::from_slice(&crate::poseidon_w5_constants::EMPTY_TREE_ROOT_BYTES);
    contract
        .accepted_note_roots
        .insert(initial_root, U256::from(1000));
    contract
        .user_note_liabilities
        .setter(U256::ZERO)
        .set(U256::from(100_000_000));

    let nf1 = FixedBytes::<32>::from_slice(&[0x1c; 32]);
    let nf2 = FixedBytes::<32>::from_slice(&[0x1d; 32]);

    set_pairing_result(false);

    let result = contract.spend_joinsplit(
        valid_test_g1(),
        valid_test_g2(),
        valid_test_g1(),
        initial_root,
        U256::from(1),
        nf1,
        nf2,
        U256::ZERO,
        U256::ZERO,
        FixedBytes::ZERO,
        FixedBytes::ZERO,
        merchant,
        U256::from(5_000_000),
        U256::from(22_500),
        U256::from(23_000),
        U256::from(25_000),
        FixedBytes::ZERO,
        U256::from(2000),
        U256::ZERO,
        U256::ZERO,
        vec![0u8; 96].into(),
        vec![0u8; 96].into(),
    );

    assert_eq!(result, Err(b"INVALID_GROTH16_JOINSPLIT_PROOF".to_vec()));
}

#[test]
fn test_spend_joinsplit_non_canonical_scalar_rejected() {
    reset_test_state();
    let owner = address!("1111111111111111111111111111111111111111");
    let merchant = address!("2222222222222222222222222222222222222222");
    set_msg_sender(owner);
    set_block_timestamp(1000);

    let mut contract = Zeltra::default();
    contract.init(owner, Address::ZERO, Address::ZERO).unwrap();

    let initial_root =
        FixedBytes::<32>::from_slice(&crate::poseidon_w5_constants::EMPTY_TREE_ROOT_BYTES);
    contract
        .accepted_note_roots
        .insert(initial_root, U256::from(1000));
    contract
        .user_note_liabilities
        .setter(U256::ZERO)
        .set(U256::from(100_000_000));

    // Non-canonical scalar: 0xFF >= r
    let non_canonical_nf = FixedBytes::<32>::from_slice(&[0xff; 32]);
    let nf2 = FixedBytes::<32>::from_slice(&[0x1d; 32]);

    let result = contract.spend_joinsplit(
        vec![0x11; 128].into(),
        vec![0x22; 256].into(),
        vec![0x33; 128].into(),
        initial_root,
        U256::from(1),
        non_canonical_nf,
        nf2,
        U256::ZERO,
        U256::ZERO,
        FixedBytes::ZERO,
        FixedBytes::ZERO,
        merchant,
        U256::from(5_000_000),
        U256::from(22_500),
        U256::from(23_000),
        U256::from(25_000),
        FixedBytes::ZERO,
        U256::from(2000),
        U256::ZERO,
        U256::ZERO,
        vec![0u8; 96].into(),
        vec![0u8; 96].into(),
    );

    assert_eq!(result, Err(b"NON_CANONICAL_PUBLIC_INPUT_SCALAR".to_vec()));
}

// ═══════════════════════════════════════════════════════════════════════
// DEC-037: Note Encryption Memo & Viewing Key Event Pipeline
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_dec037_encrypted_memo_lifecycle() {
    reset_test_state();
    let owner = address!("1111111111111111111111111111111111111111");
    let stablecoin = address!("3333333333333333333333333333333333333333");
    let client = address!("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
    let merchant = address!("2222222222222222222222222222222222222222");
    set_msg_sender(owner);
    set_block_timestamp(1000);

    let mut contract = Zeltra::default();
    contract.init(owner, stablecoin, Address::ZERO).unwrap();

    // 1. Direct deposit with note commitment and memo (DEC-038B)
    set_msg_sender(client);
    let initial_cm = FixedBytes::<32>::from_slice(&[0x0a; 32]);
    let deposit_amount = U256::from(20_000_000); // 20 USDC
    let deposit_memo: Bytes = vec![0x7a; 96].into();

    contract
        .deposit_direct_note(deposit_amount, initial_cm, deposit_memo)
        .unwrap();

    assert_eq!(contract.mmr_leaf_count().unwrap(), U256::from(1));
    assert_eq!(contract.user_note_liability().unwrap(), deposit_amount);

    // 3. Spend private note and append change note with change_encrypted_memo
    let current_root = contract.note_tree_root().unwrap();
    let nullifier = FixedBytes::<32>::from_slice(&[0x0a; 32]);
    let change_cm = FixedBytes::<32>::from_slice(&[0x0b; 32]);
    let change_memo: Bytes = vec![0x8b; 96].into();

    let spend_ok = contract
        .spend_private_note(
            current_root,
            U256::from(1),
            nullifier,
            U256::ZERO,
            change_cm,
            merchant,
            U256::from(5_000_000), // 5 USDC
            U256::from(22_500),    // 0.45% protocol fee
            U256::from(23_000),    // execution fee
            U256::from(25_000),    // max execution fee
            FixedBytes::ZERO,
            U256::from(5000),
            U256::from(1), // has_change = 1
            U256::from(2), // flags_packed: is_rollover = 0, asset_id = 1 (USDC)
            valid_test_g1(),
            valid_test_g2(),
            valid_test_g1(),
            change_memo,
        )
        .unwrap();

    assert!(spend_ok);
    assert!(contract.is_nullifier_spent(nullifier).unwrap());
    let unspent_nf = FixedBytes::repeat_byte(0xfe);
    let batch_res = contract
        .are_nullifiers_spent(vec![nullifier, unspent_nf])
        .unwrap();
    assert_eq!(batch_res, vec![true, false]);
    // MMR leaf count incremented to 2 (1 initial deposit + 1 change note)
    assert_eq!(contract.mmr_leaf_count().unwrap(), U256::from(2));
}

// ═══════════════════════════════════════════════════════════════════════
// TASK B (DEC-034B): Multi-Asset Siloed Accounting & Solvency Tests
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_weth_payable_wrap_success_cei_order() {
    reset_test_state();
    let owner = address!("1111111111111111111111111111111111111111");
    let weth9 = address!("2222222222222222222222222222222222222222");
    let usdc = address!("3333333333333333333333333333333333333333");
    let fee_recipient = address!("4444444444444444444444444444444444444444");
    let user = address!("5555555555555555555555555555555555555555");

    let mut contract = Zeltra::default();
    contract
        .init_multi_asset(owner, weth9, usdc, fee_recipient)
        .unwrap();

    // 0.01 ETH = 10^16 Wei = 10_000_000 Gwei
    let amount_wei = U256::from(10_000_000_000_000_000u64);
    let amount_gwei = U256::from(10_000_000u64);
    set_msg_sender(user);
    set_msg_value(amount_wei);

    let note_cm = FixedBytes::repeat_byte(0x01);
    let memo: Bytes = vec![0x77; 96].into();

    // Pre-condition
    assert_eq!(
        contract.user_note_liabilities(U256::ZERO).unwrap(),
        U256::ZERO
    );
    assert_eq!(
        contract.total_deposited_principals(U256::ZERO).unwrap(),
        U256::ZERO
    );
    assert_eq!(contract.mmr_leaf_count.get(), U256::ZERO);

    // Execute payable native ETH auto-wrap deposit
    contract.deposit_direct_native_eth(note_cm, memo).unwrap();

    // Verify post-conditions
    assert_eq!(
        contract.user_note_liabilities(U256::ZERO).unwrap(),
        amount_gwei
    );
    assert_eq!(
        contract.total_deposited_principals(U256::ZERO).unwrap(),
        amount_gwei
    );
    assert_eq!(contract.mmr_leaf_count.get(), U256::from(1));

    // Verify IWeth9::deposit{value: amount_wei}() interaction was recorded with exact Wei
    let deposit_calls = WETH9_DEPOSIT_CALLS.with(|c| c.borrow().clone());
    assert_eq!(deposit_calls.len(), 1);
    assert_eq!(deposit_calls[0], (weth9, amount_wei));
}

#[test]
fn test_unaligned_gwei_wei_rejection() {
    reset_test_state();
    let owner = address!("1111111111111111111111111111111111111111");
    let weth9 = address!("2222222222222222222222222222222222222222");
    let usdc = address!("3333333333333333333333333333333333333333");
    let user = address!("5555555555555555555555555555555555555555");

    let mut contract = Zeltra::default();
    contract
        .init_multi_asset(owner, weth9, usdc, Address::ZERO)
        .unwrap();

    set_msg_sender(user);
    let note_cm = FixedBytes::repeat_byte(0x01);
    let memo: Bytes = vec![0x77; 96].into();

    // 1. Unaligned Native ETH: 0.005 ETH + 1 wei = 5_000_000_000_000_001 Wei
    let unaligned_eth = U256::from(5_000_000_000_000_001u64);
    set_msg_value(unaligned_eth);
    let err_eth = contract.deposit_direct_native_eth(note_cm, memo.clone());
    assert_eq!(err_eth, Err(b"ETH_VALUE_NOT_GWEI_ALIGNED".to_vec()));

    // 2. Unaligned WETH ERC-20: 5_000_000_000_000_999 Wei
    let unaligned_weth = U256::from(5_000_000_000_000_999u64);
    let err_weth = contract.deposit_direct_weth(unaligned_weth, note_cm, memo);
    assert_eq!(err_weth, Err(b"WETH_AMOUNT_NOT_GWEI_ALIGNED".to_vec()));

    // Verify state is completely unchanged
    assert_eq!(
        contract.user_note_liabilities(U256::ZERO).unwrap(),
        U256::ZERO
    );
    assert_eq!(contract.mmr_leaf_count.get(), U256::ZERO);
}

#[test]
fn test_caller_token_injection_impossible() {
    reset_test_state();
    let owner = address!("1111111111111111111111111111111111111111");
    let weth9 = address!("2222222222222222222222222222222222222222");
    let usdc = address!("3333333333333333333333333333333333333333");
    let merchant = address!("6666666666666666666666666666666666666666");

    let mut contract = Zeltra::default();
    contract
        .init_multi_asset(owner, weth9, usdc, Address::ZERO)
        .unwrap();

    // Verify storage registration: calldata cannot alter target_token
    assert_eq!(contract.get_asset_token(U256::ZERO).unwrap(), weth9);
    assert_eq!(contract.get_asset_token(U256::from(1)).unwrap(), usdc);

    let initial_root =
        FixedBytes::<32>::from_slice(&crate::poseidon_w5_constants::EMPTY_TREE_ROOT_BYTES);
    contract
        .accepted_note_roots
        .insert(initial_root, U256::from(1000));
    contract
        .user_note_liabilities
        .setter(U256::from(1))
        .set(U256::from(50_000_000));

    // When unconfigured asset is accessed via flags_packed
    // (e.g. asset_tokens mapping at slot 1 is Address::ZERO)
    contract
        .asset_tokens
        .setter(U256::from(1))
        .set(Address::ZERO);

    let res = contract.spend_private_note(
        initial_root,
        U256::from(1),
        FixedBytes::repeat_byte(0x11),
        U256::ZERO,
        FixedBytes::repeat_byte(0x22),
        merchant,
        U256::from(5_000_000),
        U256::from(22_500),
        U256::from(23_000),
        U256::from(25_000),
        FixedBytes::ZERO,
        U256::from(2000),
        U256::from(1),
        U256::from(2), // asset_id = 1, is_rollover = 0
        vec![0x11; 128].into(),
        vec![0x22; 256].into(),
        vec![0x33; 128].into(),
        vec![].into(),
    );

    assert_eq!(res, Err(b"ASSET_NOT_CONFIGURED".to_vec()));
}

#[test]
fn test_weth_relayer_fee_claim_scaling_exactness() {
    reset_test_state();
    let owner = address!("1111111111111111111111111111111111111111");
    let weth9 = address!("2222222222222222222222222222222222222222");
    let usdc = address!("3333333333333333333333333333333333333333");
    let fee_recipient = address!("4444444444444444444444444444444444444444");

    set_msg_sender(owner);
    let mut contract = Zeltra::default();
    contract
        .init_multi_asset(owner, weth9, usdc, fee_recipient)
        .unwrap();
    contract.set_execution_fee_recipient(fee_recipient).unwrap();

    // 25_000 Gwei accrued execution fee for WETH (asset_id = 0)
    let fee_gwei = U256::from(25_000u64);
    contract
        .accrued_execution_fee_liabilities
        .setter(U256::ZERO)
        .set(fee_gwei);

    // Unauthorized caller fails
    let hacker = address!("9999999999999999999999999999999999999999");
    set_msg_sender(hacker);
    let err_auth = contract.claim_execution_fees(0, fee_gwei);
    assert_eq!(err_auth, Err(b"NOT_AUTHORIZED".to_vec()));

    // Amount > accrued fails
    set_msg_sender(fee_recipient);
    let err_over = contract.claim_execution_fees(0, fee_gwei + U256::from(1));
    assert_eq!(err_over, Err(b"INSUFFICIENT_ACCUMULATED_FEES".to_vec()));

    // Zero amount fails
    let err_zero = contract.claim_execution_fees(0, U256::ZERO);
    assert_eq!(err_zero, Err(b"ZERO_AMOUNT".to_vec()));

    // Valid claim of full fee
    let ok = contract.claim_execution_fees(0, fee_gwei).unwrap();
    assert!(ok);
    assert_eq!(
        contract
            .accrued_execution_fee_liabilities(U256::ZERO)
            .unwrap(),
        U256::ZERO
    );
}

#[test]
fn test_flags_packed_malleable_bits_rejection() {
    reset_test_state();
    let owner = address!("1111111111111111111111111111111111111111");
    let stablecoin = address!("3333333333333333333333333333333333333333");
    let merchant = address!("2222222222222222222222222222222222222222");
    set_msg_sender(owner);
    set_block_timestamp(1000);

    let mut contract = Zeltra::default();
    contract.init(owner, stablecoin, Address::ZERO).unwrap();

    let initial_root =
        FixedBytes::<32>::from_slice(&crate::poseidon_w5_constants::EMPTY_TREE_ROOT_BYTES);
    contract
        .accepted_note_roots
        .insert(initial_root, U256::from(1000));
    contract
        .user_note_liabilities
        .setter(U256::ZERO)
        .set(U256::from(100_000_000));

    // 1. Private note spend: valid range is 0..=3
    // flags_packed = 4 (> 3) must revert FLAGS_PACKED_MALLEABLE_BITS
    let res_spend_4 = contract.spend_private_note(
        initial_root,
        U256::from(1),
        FixedBytes::repeat_byte(0x01),
        U256::ZERO,
        FixedBytes::repeat_byte(0x02),
        merchant,
        U256::from(5_000_000),
        U256::from(22_500),
        U256::from(23_000),
        U256::from(25_000),
        FixedBytes::ZERO,
        U256::from(2000),
        U256::from(1),
        U256::from(4), // MALLEABLE BIT (> 3)
        vec![0x11; 128].into(),
        vec![0x22; 256].into(),
        vec![0x33; 128].into(),
        vec![].into(),
    );
    assert_eq!(res_spend_4, Err(b"FLAGS_PACKED_MALLEABLE_BITS".to_vec()));

    // flags_packed = 255 must also revert
    let res_spend_255 = contract.spend_private_note(
        initial_root,
        U256::from(1),
        FixedBytes::repeat_byte(0x01),
        U256::ZERO,
        FixedBytes::repeat_byte(0x02),
        merchant,
        U256::from(5_000_000),
        U256::from(22_500),
        U256::from(23_000),
        U256::from(25_000),
        FixedBytes::ZERO,
        U256::from(2000),
        U256::from(1),
        U256::from(255), // MALLEABLE BITS (> 3)
        vec![0x11; 128].into(),
        vec![0x22; 256].into(),
        vec![0x33; 128].into(),
        vec![].into(),
    );
    assert_eq!(res_spend_255, Err(b"FLAGS_PACKED_MALLEABLE_BITS".to_vec()));

    // 2. JoinSplit spend: valid range is 0..=7
    // flags_packed = 8 (> 7) must revert FLAGS_PACKED_MALLEABLE_BITS
    let res_js_8 = contract.spend_joinsplit(
        vec![0x11; 128].into(),
        vec![0x22; 256].into(),
        vec![0x33; 128].into(),
        initial_root,
        U256::from(1),
        FixedBytes::repeat_byte(0x03),
        FixedBytes::repeat_byte(0x04),
        U256::ZERO,
        U256::ZERO,
        FixedBytes::ZERO,
        FixedBytes::ZERO,
        merchant,
        U256::from(5_000_000),
        U256::from(22_500),
        U256::from(23_000),
        U256::from(25_000),
        FixedBytes::ZERO,
        U256::from(2000),
        U256::from(8), // MALLEABLE BIT (> 7)
        U256::ZERO,
        vec![0u8; 96].into(),
        vec![0u8; 96].into(),
    );
    assert_eq!(res_js_8, Err(b"FLAGS_PACKED_MALLEABLE_BITS".to_vec()));

    // flags_packed = 255 must also revert
    let res_js_255 = contract.spend_joinsplit(
        vec![0x11; 128].into(),
        vec![0x22; 256].into(),
        vec![0x33; 128].into(),
        initial_root,
        U256::from(1),
        FixedBytes::repeat_byte(0x03),
        FixedBytes::repeat_byte(0x04),
        U256::ZERO,
        U256::ZERO,
        FixedBytes::ZERO,
        FixedBytes::ZERO,
        merchant,
        U256::from(5_000_000),
        U256::from(22_500),
        U256::from(23_000),
        U256::from(25_000),
        FixedBytes::ZERO,
        U256::from(2000),
        U256::from(255), // MALLEABLE BITS (> 7)
        U256::ZERO,
        vec![0u8; 96].into(),
        vec![0u8; 96].into(),
    );
    assert_eq!(res_js_255, Err(b"FLAGS_PACKED_MALLEABLE_BITS".to_vec()));
}

#[test]
fn test_usdc_revert_leaves_weth_solvent() {
    reset_test_state();
    let owner = address!("1111111111111111111111111111111111111111");
    let weth9 = address!("2222222222222222222222222222222222222222");
    let usdc = address!("3333333333333333333333333333333333333333");
    let user = address!("5555555555555555555555555555555555555555");

    let mut contract = Zeltra::default();
    contract
        .init_multi_asset(owner, weth9, usdc, Address::ZERO)
        .unwrap();

    // 1. Establish WETH pool state (0.01 ETH = 10_000_000 Gwei)
    set_msg_sender(user);
    let amount_wei = U256::from(10_000_000_000_000_000u64);
    set_msg_value(amount_wei);
    let note_cm = FixedBytes::repeat_byte(0x05);
    let memo: Bytes = vec![0x77; 96].into();
    contract
        .deposit_direct_native_eth(note_cm, memo.clone())
        .unwrap();

    // 2. Simulate Circle freezing / blacklisting USDC transfers
    set_erc20_transfer_from_result(Err(b"USDC_BLACKLISTED".to_vec()));

    // USDC deposit fails
    let res_usdc =
        contract.deposit_direct_note(U256::from(20_000_000), FixedBytes::repeat_byte(0x06), memo);
    assert_eq!(res_usdc, Err(b"USDC_BLACKLISTED".to_vec()));

    // 3. Theorem Verification: WETH pool accounting and solvency invariant are completely unaffected!
    assert!(contract.check_asset_solvency_invariant(0).is_ok());
    assert_eq!(
        contract.user_note_liabilities(U256::ZERO).unwrap(),
        U256::from(10_000_000)
    );
    assert_eq!(
        contract.user_note_liabilities(U256::from(1)).unwrap(),
        U256::ZERO
    );
}

// ═══════════════════════════════════════════════════════════════════════
// DEC-052: Strict 5-Gate Curve Point Sanitation Contract Tests
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_contract_sanitizes_infinity_point() {
    let inf_point: Bytes = vec![0u8; 128].into();
    let err = crate::types::sanitize_g1_point(&inf_point).unwrap_err();
    assert_eq!(err, b"PointAtInfinityNotAllowed".to_vec());
}

#[test]
fn test_contract_sanitizes_non_canonical_coordinates() {
    let mut bytes = vec![0u8; 128];
    bytes[63] = 1;
    bytes[127] = 1;
    // Non-zero padding in leading 16 bytes
    bytes[0] = 0x01;
    let err = crate::types::sanitize_g1_point(&bytes).unwrap_err();
    assert_eq!(err, b"NonCanonicalFieldElement".to_vec());
}

#[test]
fn test_contract_sanitizes_off_curve_point() {
    // Canonical coordinates but not on curve
    let mut bytes = [0u8; 128];
    bytes[63] = 1; // x = 1
    bytes[127] = 1; // y = 1 (y^2 = 1 != 1^3 + 4 = 5)
    let err = crate::types::sanitize_g1_point(&bytes).unwrap_err();
    assert_eq!(err, b"CurvePointNotOnCurve".to_vec());
}

#[test]
fn test_contract_sanitizes_small_subgroup_order3_point() {
    // (0, 2) is on curve y^2 = x^3 + 4 but has small order 3 (divides cofactor h1)
    let mut bytes = [0u8; 128];
    bytes[127] = 2; // y = 2, x = 0
    let err = crate::types::sanitize_g1_point(&bytes).unwrap_err();
    assert_eq!(err, b"SubgroupCheckFailed".to_vec());
}

#[test]
fn test_contract_spend_private_note_rejects_subgroup_injected_proof() {
    reset_test_state();
    let owner = address!("1111111111111111111111111111111111111111");
    let merchant = address!("2222222222222222222222222222222222222222");
    let mut contract = Zeltra::default();
    contract.init(owner, Address::ZERO, Address::ZERO).unwrap();

    let root = FixedBytes::<32>::from_slice(&crate::poseidon_w5_constants::EMPTY_TREE_ROOT_BYTES);
    contract.accepted_note_roots.insert(root, U256::from(1000));
    contract
        .user_note_liabilities
        .setter(U256::ZERO)
        .set(U256::from(100_000_000));

    // Subgroup-injected proof: A has order 3 (x=0, y=2)
    let mut bad_proof_a = vec![0u8; 128];
    bad_proof_a[127] = 2;

    let canonical_nullifier = FixedBytes::<32>::from_slice(&[0x0a; 32]);
    let canonical_change_cm = FixedBytes::<32>::from_slice(&[0x0b; 32]);

    let res = contract.spend_private_note(
        root,
        U256::from(1),
        canonical_nullifier,
        U256::ZERO,
        canonical_change_cm,
        merchant,
        U256::from(5_000_000),
        U256::from(22_500),
        U256::from(23_000),
        U256::from(25_000),
        FixedBytes::ZERO,
        U256::from(2000),
        U256::from(1),
        U256::ZERO,
        bad_proof_a.into(),
        valid_test_g2(),
        valid_test_g1(),
        vec![].into(),
    );

    assert_eq!(res, Err(b"SubgroupCheckFailed".to_vec()));
}

#[test]
fn test_ammboost_bounded_ring_buffer_eviction_pos() {
    reset_test_state();
    set_block_timestamp(1_700_000_000);
    let owner = Address::repeat_byte(0x01);
    set_msg_sender(owner);
    let mut contract = Zeltra::default();
    contract
        .init(
            owner,
            Address::repeat_byte(0x03),
            Address::repeat_byte(0x04),
        )
        .unwrap();

    let mut roots = Vec::new();
    for i in 1..=105u64 {
        let mut leaf_bytes = [0u8; 32];
        leaf_bytes[24..32].copy_from_slice(&i.to_be_bytes());
        let leaf = FixedBytes::<32>::from(leaf_bytes);
        let (_, new_root) = contract._mmr_insert(leaf, vec![]).unwrap();
        roots.push(new_root);
    }

    // Root 0 (at index 0) was evicted when leaf 101 (index 100) wrapped around slot 0
    let root_0 = roots[0];
    assert!(
        !contract._is_accepted_note_root(root_0),
        "Root 0 must be evicted after ring-buffer wrap-around per DEC-053"
    );

    // Root 1 (at index 1) was evicted when leaf 102 (index 101) wrapped around slot 1
    let root_1 = roots[1];
    assert!(
        !contract._is_accepted_note_root(root_1),
        "Root 1 must be evicted after ring-buffer wrap-around per DEC-053"
    );

    // Recent roots (indices 5..104) must remain accepted
    let recent_root = roots[104];
    assert!(
        contract._is_accepted_note_root(recent_root),
        "Recent root 104 must be accepted"
    );
    let root_100 = roots[100];
    assert!(
        contract._is_accepted_note_root(root_100),
        "Root 100 must be accepted"
    );
}

#[test]
fn test_ammboost_eviction_no_premature_eviction() {
    reset_test_state();
    set_block_timestamp(1_700_000_000);
    let owner = Address::repeat_byte(0x01);
    set_msg_sender(owner);
    let mut contract = Zeltra::default();
    contract
        .init(
            owner,
            Address::repeat_byte(0x03),
            Address::repeat_byte(0x04),
        )
        .unwrap();

    let mut roots = Vec::new();
    // Insert 50 leaves (half of ring buffer)
    for i in 1..=50u64 {
        let mut leaf_bytes = [0u8; 32];
        leaf_bytes[24..32].copy_from_slice(&i.to_be_bytes());
        let leaf = FixedBytes::<32>::from(leaf_bytes);
        let (_, new_root) = contract._mmr_insert(leaf, vec![]).unwrap();
        roots.push(new_root);
    }

    // None of the first 50 roots should be evicted prematurely
    for (idx, r) in roots.iter().enumerate() {
        assert!(
            contract._is_accepted_note_root(*r),
            "Root {} must remain accepted before buffer wrap-around",
            idx
        );
    }
}

#[test]
fn test_ammboost_evicted_root_spend_rejected() {
    reset_test_state();
    set_block_timestamp(1_700_000_000);
    let owner = Address::repeat_byte(0x01);
    let merchant = Address::repeat_byte(0x02);
    set_msg_sender(owner);
    let mut contract = Zeltra::default();
    contract
        .init(
            owner,
            Address::repeat_byte(0x03),
            Address::repeat_byte(0x04),
        )
        .unwrap();

    let mut roots = Vec::new();
    for i in 1..=105u64 {
        let mut leaf_bytes = [0u8; 32];
        leaf_bytes[24..32].copy_from_slice(&i.to_be_bytes());
        let leaf = FixedBytes::<32>::from(leaf_bytes);
        let (_, new_root) = contract._mmr_insert(leaf, vec![]).unwrap();
        roots.push(new_root);
    }

    let evicted_root = roots[0];
    assert!(!contract._is_accepted_note_root(evicted_root));

    // Attempting to spend against an evicted root must fail with UNACCEPTED_NOTE_ROOT
    let canonical_nullifier = FixedBytes::<32>::from_slice(&[0x0a; 32]);
    let canonical_change_cm = FixedBytes::<32>::from_slice(&[0x0b; 32]);
    let res = contract.spend_private_note(
        evicted_root,
        U256::from(1),
        canonical_nullifier,
        U256::ZERO,
        canonical_change_cm,
        merchant,
        U256::from(5_000_000),
        U256::from(22_500),
        U256::from(23_000),
        U256::from(25_000),
        FixedBytes::ZERO,
        U256::from(1_700_000_000 + 3600),
        U256::from(1),
        U256::ZERO,
        valid_test_g1(),
        valid_test_g2(),
        valid_test_g1(),
        vec![].into(),
    );

    assert_eq!(res, Err(b"UNACCEPTED_NOTE_ROOT".to_vec()));
}
