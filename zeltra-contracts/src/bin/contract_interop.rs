#![allow(
    clippy::missing_safety_doc,
    clippy::op_ref,
    clippy::lines_filter_map_ok,
    clippy::too_many_arguments,
    clippy::manual_strip
)]

use alloy_primitives::{keccak256, Address, FixedBytes, U256};
use std::collections::HashMap;
use std::io::{self, BufRead};
use std::sync::Mutex;
use stylus_sdk::abi::Bytes;
use zeltra_contracts::Zeltra;

pub const EMPTY_TREE_ROOT_BYTES: [u8; 32] = [
    0x02, 0x51, 0x1d, 0xef, 0x7a, 0x6e, 0x24, 0xef, 0x12, 0x3d, 0x56, 0xfa, 0x85, 0x22, 0x76, 0xaa,
    0x98, 0x63, 0x77, 0x6a, 0xa1, 0x85, 0x70, 0x93, 0xa7, 0x6e, 0x05, 0x3d, 0x09, 0xb6, 0x71, 0x17,
];

#[derive(Clone, Debug)]
pub struct Erc20Transfer {
    pub token: Address,
    pub recipient: Address,
    pub amount: U256,
}

static STORAGE: Mutex<Option<HashMap<[u8; 32], [u8; 32]>>> = Mutex::new(None);
static MSG_SENDER: Mutex<[u8; 20]> = Mutex::new([1u8; 20]);
static LAST_RETURN_DATA_SIZE: Mutex<usize> = Mutex::new(0);
static ERC20_TRANSFERS: Mutex<Vec<Erc20Transfer>> = Mutex::new(Vec::new());

fn reset_host_state() {
    let mut lock = STORAGE.lock().unwrap();
    *lock = Some(HashMap::new());
    *MSG_SENDER.lock().unwrap() = [1u8; 20];
    *LAST_RETURN_DATA_SIZE.lock().unwrap() = 0;
    ERC20_TRANSFERS.lock().unwrap().clear();
}

#[no_mangle]
pub unsafe extern "C" fn msg_value(dest: *mut u8) {
    let dest_slice = std::slice::from_raw_parts_mut(dest, 32);
    for b in dest_slice.iter_mut() {
        *b = 0;
    }
}

#[no_mangle]
pub unsafe extern "C" fn msg_sender(dest: *mut u8) {
    let dest_slice = std::slice::from_raw_parts_mut(dest, 20);
    let sender = MSG_SENDER.lock().unwrap();
    dest_slice.copy_from_slice(&*sender);
}

#[no_mangle]
pub unsafe extern "C" fn contract_address(dest: *mut u8) {
    let dest_slice = std::slice::from_raw_parts_mut(dest, 20);
    dest_slice.copy_from_slice(&[5u8; 20]);
}

#[no_mangle]
pub unsafe extern "C" fn block_timestamp() -> u64 {
    1_700_000_000
}

#[no_mangle]
pub unsafe extern "C" fn chainid() -> u64 {
    421614 // Arbitrum Sepolia
}

#[no_mangle]
pub unsafe extern "C" fn emit_log(_data: *const u8, _len: usize, _topics: usize) {}

#[no_mangle]
pub unsafe extern "C" fn storage_load_bytes32(key: *const u8, dest: *mut u8) {
    let key_slice = std::slice::from_raw_parts(key, 32);
    let dest_slice = std::slice::from_raw_parts_mut(dest, 32);
    let mut k = [0u8; 32];
    k.copy_from_slice(key_slice);

    let mut lock = STORAGE.lock().unwrap();
    let storage = lock.get_or_insert_with(HashMap::new);
    if let Some(val) = storage.get(&k) {
        dest_slice.copy_from_slice(val);
    } else {
        for byte in dest_slice.iter_mut() {
            *byte = 0;
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn storage_cache_bytes32(key: *const u8, src: *const u8) {
    let key_slice = std::slice::from_raw_parts(key, 32);
    let src_slice = std::slice::from_raw_parts(src, 32);
    let mut k = [0u8; 32];
    k.copy_from_slice(key_slice);
    let mut v = [0u8; 32];
    v.copy_from_slice(src_slice);

    let mut lock = STORAGE.lock().unwrap();
    let storage = lock.get_or_insert_with(HashMap::new);
    storage.insert(k, v);
}

#[no_mangle]
pub unsafe extern "C" fn storage_flush_cache() {}

#[no_mangle]
pub unsafe extern "C" fn native_keccak256(bytes: *const u8, len: usize, output: *mut u8) {
    let slice = std::slice::from_raw_parts(bytes, len);
    let hash = keccak256(slice);
    let out_slice = std::slice::from_raw_parts_mut(output, 32);
    out_slice.copy_from_slice(hash.as_slice());
}

#[no_mangle]
pub unsafe extern "C" fn call_contract(
    contract: *const u8,
    calldata: *const u8,
    calldata_len: usize,
    _value: *const u8,
    _gas: u64,
    return_data_len: *mut usize,
) -> u8 {
    let token_slice = std::slice::from_raw_parts(contract, 20);
    let token = Address::from_slice(token_slice);
    let cdata = std::slice::from_raw_parts(calldata, calldata_len);

    *LAST_STATIC_IS_BALANCE.lock().unwrap() = false;
    *LAST_RETURN_DATA_SIZE.lock().unwrap() = 32;
    if !return_data_len.is_null() {
        *return_data_len = 32;
    }

    // ERC-20 transfer(address,uint256) selector: 0xa9059cbb
    if cdata.len() >= 68 && &cdata[0..4] == &[0xa9, 0x05, 0x9c, 0xbb] {
        let recipient = Address::from_slice(&cdata[16..36]);
        let amount = U256::from_be_slice(&cdata[36..68]);
        ERC20_TRANSFERS.lock().unwrap().push(Erc20Transfer {
            token,
            recipient,
            amount,
        });
    }

    0 // Success
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

static MOCK_BALANCE: Mutex<U256> = Mutex::new(U256::ZERO);
static CURRENT_ASSET_ID: Mutex<u64> = Mutex::new(1);
static LAST_STATIC_IS_BALANCE: Mutex<bool> = Mutex::new(false);

#[no_mangle]
pub unsafe extern "C" fn static_call_contract(
    contract: *const u8,
    calldata: *const u8,
    calldata_len: usize,
    _gas: u64,
    return_data_len: *mut usize,
) -> u8 {
    let addr_slice = std::slice::from_raw_parts(contract, 20);
    let cdata = std::slice::from_raw_parts(calldata, calldata_len);

    let is_balance_of = cdata.len() >= 4 && &cdata[0..4] == &[0x70, 0xa0, 0x82, 0x31];
    *LAST_STATIC_IS_BALANCE.lock().unwrap() = is_balance_of;

    let len = if addr_slice[19] == 0x0c || addr_slice[19] == 0x0b {
        128
    } else {
        32
    };
    *LAST_RETURN_DATA_SIZE.lock().unwrap() = len;
    if !return_data_len.is_null() {
        *return_data_len = len;
    }
    0
}

#[no_mangle]
pub unsafe extern "C" fn return_data_size() -> usize {
    *LAST_RETURN_DATA_SIZE.lock().unwrap()
}

#[no_mangle]
pub unsafe extern "C" fn read_return_data(dest: *mut u8, _offset: usize, size: usize) -> usize {
    let dest_slice = std::slice::from_raw_parts_mut(dest, size);
    for byte in dest_slice.iter_mut() {
        *byte = 0;
    }
    if size == 32 {
        if *LAST_STATIC_IS_BALANCE.lock().unwrap() {
            let bal = *MOCK_BALANCE.lock().unwrap();
            let raw_bal = if *CURRENT_ASSET_ID.lock().unwrap() == 0 {
                bal.saturating_mul(U256::from(1_000_000_000u64))
            } else {
                bal
            };
            let bal_bytes = raw_bal.to_be_bytes::<32>();
            dest_slice.copy_from_slice(&bal_bytes);
        } else {
            dest_slice[31] = 1; // Pairing check success
        }
    }
    size
}

fn hex_to_bytes(s: &str) -> Vec<u8> {
    let s = s.trim().strip_prefix("0x").unwrap_or(s.trim());
    let mut bytes = Vec::new();
    for i in (0..s.len()).step_by(2) {
        if i + 2 <= s.len() {
            if let Ok(b) = u8::from_str_radix(&s[i..i + 2], 16) {
                bytes.push(b);
            }
        }
    }
    bytes
}

fn bytes_to_hex(bytes: &[u8]) -> String {
    let mut hex = String::with_capacity(bytes.len() * 2 + 2);
    hex.push_str("0x");
    for b in bytes {
        hex.push_str(&format!("{:02x}", b));
    }
    hex
}

fn u256_from_hex_or_dec(s: &str) -> U256 {
    let s = s.trim();
    if s.starts_with("0x") || s.starts_with("0X") {
        let b = hex_to_bytes(s);
        if b.is_empty() {
            U256::ZERO
        } else {
            U256::from_be_slice(&b)
        }
    } else {
        s.parse::<U256>().unwrap_or(U256::ZERO)
    }
}

fn fixed_bytes_32_or_default(
    kv: &HashMap<String, String>,
    key: &str,
    default_byte: u8,
) -> FixedBytes<32> {
    if let Some(s) = kv.get(key) {
        let b = hex_to_bytes(s);
        if b.len() == 32 {
            return FixedBytes::<32>::from_slice(&b);
        }
    }
    FixedBytes::<32>::from([default_byte; 32])
}

fn address_or_zero(kv: &HashMap<String, String>, key: &str) -> Address {
    if let Some(s) = kv.get(key) {
        let b = hex_to_bytes(s);
        if b.len() == 20 {
            return Address::from_slice(&b);
        }
    }
    Address::ZERO
}

fn main() {
    let stdin = io::stdin();
    let lines: Vec<String> = stdin.lock().lines().filter_map(|l| l.ok()).collect();

    let mut kv: HashMap<String, String> = HashMap::new();
    for line in lines {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = trimmed.split_once('=') {
            kv.insert(k.trim().to_string(), v.trim().to_string());
        }
    }

    reset_host_state();

    let owner = address_or_zero(&kv, "owner");
    let owner = if owner == Address::ZERO {
        Address::repeat_byte(0x11)
    } else {
        owner
    };

    let stablecoin = address_or_zero(&kv, "stablecoin");
    let stablecoin = if stablecoin == Address::ZERO {
        Address::repeat_byte(0x22)
    } else {
        stablecoin
    };

    let weth = address_or_zero(&kv, "weth");
    let weth = if weth == Address::ZERO {
        Address::repeat_byte(0x33)
    } else {
        weth
    };

    let fee_recipient = address_or_zero(&kv, "fee_recipient");
    let fee_recipient = if fee_recipient == Address::ZERO {
        Address::repeat_byte(0x44)
    } else {
        fee_recipient
    };

    let caller = address_or_zero(&kv, "caller");
    let caller = if caller == Address::ZERO {
        owner
    } else {
        caller
    };

    MSG_SENDER
        .lock()
        .unwrap()
        .copy_from_slice(caller.as_slice());

    let mut contract = Zeltra::default();
    contract
        .init_multi_asset(owner, weth, stablecoin, fee_recipient)
        .expect("init failed");

    let asset_id_u64 = kv
        .get("asset_id")
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(1);
    let asset_id = U256::from(asset_id_u64);
    *CURRENT_ASSET_ID.lock().unwrap() = asset_id_u64;

    if let Some(init_liab_str) = kv.get("initial_liability") {
        let init_liab = u256_from_hex_or_dec(init_liab_str);
        contract
            .user_note_liabilities
            .setter(asset_id)
            .set(init_liab);
        *MOCK_BALANCE.lock().unwrap() = init_liab;
    }

    if let Some(assets_str) = kv.get("initial_assets") {
        let assets = u256_from_hex_or_dec(assets_str);
        *MOCK_BALANCE.lock().unwrap() = assets;
    }

    if let Some(root_hex) = kv.get("accepted_root") {
        let b = hex_to_bytes(root_hex);
        if b.len() == 32 {
            let root_fb = FixedBytes::<32>::from_slice(&b);
            contract
                .accepted_note_roots
                .insert(root_fb, U256::from(1000));
        }
    }

    // Default empty root as accepted root if none provided
    let empty_root = FixedBytes::<32>::from_slice(&EMPTY_TREE_ROOT_BYTES);
    contract
        .accepted_note_roots
        .insert(empty_root, U256::from(1000));

    // Prepare 22 spend_joinsplit parameters
    let proof_a: Bytes = kv
        .get("proof_a")
        .map(|s| hex_to_bytes(s))
        .unwrap_or_else(|| vec![0x11; 128])
        .into();

    let proof_b: Bytes = kv
        .get("proof_b")
        .map(|s| hex_to_bytes(s))
        .unwrap_or_else(|| vec![0x22; 256])
        .into();

    let proof_c: Bytes = kv
        .get("proof_c")
        .map(|s| hex_to_bytes(s))
        .unwrap_or_else(|| vec![0x33; 128])
        .into();

    let note_root = if let Some(s) = kv.get("note_root") {
        let b = hex_to_bytes(s);
        if b.len() == 32 {
            FixedBytes::<32>::from_slice(&b)
        } else {
            empty_root
        }
    } else {
        empty_root
    };

    let leaf_count = kv
        .get("leaf_count")
        .map(|s| u256_from_hex_or_dec(s))
        .unwrap_or(U256::from(1));
    let nullifier_1 = fixed_bytes_32_or_default(&kv, "nullifier_1", 0x81);
    let nullifier_2 = fixed_bytes_32_or_default(&kv, "nullifier_2", 0x82);
    let epoch_id_1 = kv
        .get("epoch_id_1")
        .map(|s| u256_from_hex_or_dec(s))
        .unwrap_or(U256::ZERO);
    let epoch_id_2 = kv
        .get("epoch_id_2")
        .map(|s| u256_from_hex_or_dec(s))
        .unwrap_or(U256::ZERO);
    let output_cm_1 = fixed_bytes_32_or_default(&kv, "output_cm_1", 0xc1);
    let output_cm_2 = fixed_bytes_32_or_default(&kv, "output_cm_2", 0xc2);
    let recipient = address_or_zero(&kv, "recipient");
    let merchant_amount = kv
        .get("merchant_amount")
        .map(|s| u256_from_hex_or_dec(s))
        .unwrap_or(U256::ZERO);
    let protocol_fee = kv
        .get("protocol_fee")
        .map(|s| u256_from_hex_or_dec(s))
        .unwrap_or(U256::ZERO);
    let execution_fee = kv
        .get("execution_fee")
        .map(|s| u256_from_hex_or_dec(s))
        .unwrap_or(U256::ZERO);
    let max_execution_fee = kv
        .get("max_execution_fee")
        .map(|s| u256_from_hex_or_dec(s))
        .unwrap_or(U256::from(20_000));
    let quote_hash = fixed_bytes_32_or_default(&kv, "quote_hash", 0x00);
    let expiry = kv
        .get("expiry")
        .map(|s| u256_from_hex_or_dec(s))
        .unwrap_or(U256::from(2_000_000_000));
    let flags_packed = kv
        .get("flags_packed")
        .map(|s| u256_from_hex_or_dec(s))
        .unwrap_or_else(|| {
            if asset_id_u64 == 1 {
                U256::from(7)
            } else {
                U256::from(3)
            }
        });
    let is_rollover = kv
        .get("is_rollover")
        .map(|s| u256_from_hex_or_dec(s))
        .unwrap_or(U256::ZERO);

    let memo_1: Bytes = kv
        .get("memo_1")
        .map(|s| hex_to_bytes(s))
        .unwrap_or_else(|| vec![0xaa; 96])
        .into();

    let memo_2: Bytes = kv
        .get("memo_2")
        .map(|s| hex_to_bytes(s))
        .unwrap_or_else(|| vec![0xbb; 96])
        .into();

    // Execute spend_joinsplit
    let res = contract.spend_joinsplit(
        proof_a,
        proof_b,
        proof_c,
        note_root,
        leaf_count,
        nullifier_1,
        nullifier_2,
        epoch_id_1,
        epoch_id_2,
        output_cm_1,
        output_cm_2,
        recipient,
        merchant_amount,
        protocol_fee,
        execution_fee,
        max_execution_fee,
        quote_hash,
        expiry,
        flags_packed,
        is_rollover,
        memo_1,
        memo_2,
    );

    let (success, revert_reason) = match res {
        Ok(_) => (true, "null".to_string()),
        Err(e) => {
            let err_str = String::from_utf8(e.clone()).unwrap_or_else(|_| bytes_to_hex(&e));
            (false, format!("\"{}\"", err_str))
        }
    };

    let leaf_count = contract.mmr_leaf_count().unwrap_or(U256::ZERO);
    let root = contract.note_tree_root().unwrap_or(FixedBytes::ZERO);
    let root_hex = bytes_to_hex(root.as_slice());

    let nf1_spent = contract.is_nullifier_spent(nullifier_1).unwrap_or(false);
    let nf2_spent = contract.is_nullifier_spent(nullifier_2).unwrap_or(false);

    let user_liab_usdc = contract
        .user_note_liabilities(U256::from(1))
        .unwrap_or(U256::ZERO);
    let relayer_fee_usdc = contract
        .accrued_execution_fee_liabilities(U256::from(1))
        .unwrap_or(U256::ZERO);
    let proto_fee_usdc = contract
        .realized_protocol_fees_of(U256::from(1))
        .unwrap_or(U256::ZERO);

    let user_liab_weth = contract
        .user_note_liabilities(U256::ZERO)
        .unwrap_or(U256::ZERO);
    let relayer_fee_weth = contract
        .accrued_execution_fee_liabilities(U256::ZERO)
        .unwrap_or(U256::ZERO);
    let proto_fee_weth = contract
        .realized_protocol_fees_of(U256::ZERO)
        .unwrap_or(U256::ZERO);

    // Collect ERC-20 transfers
    let transfers = ERC20_TRANSFERS.lock().unwrap().clone();
    let mut transfers_json = String::from("[");
    for (i, t) in transfers.iter().enumerate() {
        if i > 0 {
            transfers_json.push(',');
        }
        transfers_json.push_str(&format!(
            r#"{{"token":"{}","recipient":"{}","amount":"{}"}}"#,
            bytes_to_hex(t.token.as_slice()),
            bytes_to_hex(t.recipient.as_slice()),
            t.amount
        ));
    }
    transfers_json.push(']');

    println!(
        r#"{{"success":{},"revert_reason":{},"erc20_transfers":{},"mmr_leaf_count":{},"mmr_root":"{}","nullifier_1_spent":{},"nullifier_2_spent":{},"user_note_liabilities_usdc":"{}","accrued_execution_fee_liabilities_usdc":"{}","realized_protocol_fees_usdc":"{}","user_note_liabilities_weth":"{}","accrued_execution_fee_liabilities_weth":"{}","realized_protocol_fees_weth":"{}"}}"#,
        success,
        revert_reason,
        transfers_json,
        leaf_count,
        root_hex,
        nf1_spent,
        nf2_spent,
        user_liab_usdc,
        relayer_fee_usdc,
        proto_fee_usdc,
        user_liab_weth,
        relayer_fee_weth,
        proto_fee_weth
    );
}
