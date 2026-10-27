//! CCIP Inbound Programmable Deposit Implementation (DEC-055)
//!
//! Handles cross-chain programmable token transfers from Chainlink CCIP,
//! verifying the trusted router, allowlisted senders, message uniqueness,
//! and unwrapping the 128-byte wire payload (32B Note Commitment + 96B Encrypted Memo)
//! directly into the Merkle Mountain Range (MMR).

use alloc::vec::Vec;
use alloy_primitives::{keccak256, Address, FixedBytes, U256};
use alloy_sol_types::sol;

use crate::storage::Zeltra;
use crate::types::from_evm_scalar;

sol! {
    /// EVM token amount struct conforming to Chainlink CCIP Client.sol
    #[derive(Debug, PartialEq, Eq)]
    struct EVMTokenAmount {
        address token;
        uint256 amount;
    }

    /// Any2EVMMessage struct conforming to Chainlink CCIP Client.sol
    #[derive(Debug, PartialEq, Eq)]
    struct Any2EVMMessage {
        bytes32 messageId;
        uint64 sourceChainSelector;
        bytes sender;
        bytes data;
        EVMTokenAmount[] destTokenAmounts;
    }
}

pub use Any2EVMMessage as CcipAny2EVMMessage;
pub use EVMTokenAmount as CcipEVMTokenAmount;

impl stylus_sdk::abi::AbiType for EVMTokenAmount {
    type SolType = EVMTokenAmount;
    const ABI: stylus_sdk::abi::ConstString = stylus_sdk::abi::ConstString::new("(address,uint256)");
}

impl stylus_sdk::abi::AbiType for Any2EVMMessage {
    type SolType = Any2EVMMessage;
    const ABI: stylus_sdk::abi::ConstString =
        stylus_sdk::abi::ConstString::new("(bytes32,uint64,bytes,bytes,(address,uint256)[])");
}

pub const CCIP_RECEIVER_INTERFACE_ID: [u8; 4] = [0x8a, 0x92, 0xdc, 0xed];
pub const ERC165_INTERFACE_ID: [u8; 4] = [0x01, 0xff, 0xc9, 0xa7];

impl Zeltra {
    /// Standard ERC-165 interface query
    pub fn _supports_interface(&self, interface_id: FixedBytes<4>) -> bool {
        let id = interface_id.0;
        id == ERC165_INTERFACE_ID || id == CCIP_RECEIVER_INTERFACE_ID
    }

    /// Canonical CCIP programmable deposit receiver hook
    pub fn _ccip_receive_programmable(
        &mut self,
        message: Any2EVMMessage,
    ) -> Result<(), Vec<u8>> {
        // 1. CHECKS KEAMANAN ROUTER & AKSES
        self.check_not_paused()?;
        let caller = self.msg_sender();
        let configured_router = self.ccip_router.get();

        if configured_router == Address::ZERO {
            return Err(b"CCIP_ROUTER_NOT_CONFIGURED".to_vec());
        }
        if caller != configured_router {
            return Err(b"ONLY_CCIP_ROUTER_ALLOWED".to_vec());
        }
        if message.messageId == FixedBytes::ZERO {
            return Err(b"INVALID_CCIP_MESSAGE_ID".to_vec());
        }
        if self.ccip_processed_messages.get(message.messageId) {
            return Err(b"CCIP_MESSAGE_ALREADY_PROCESSED".to_vec());
        }
        if message.sourceChainSelector == 0 {
            return Err(b"INVALID_SOURCE_CHAIN_SELECTOR".to_vec());
        }
        if message.sender.is_empty() {
            return Err(b"EMPTY_CCIP_SENDER".to_vec());
        }

        // Verifikasi allowlist pengirim: keccak256(source_chain_selector || sender)
        let allowlist_key = self.ccip_allowlist_key(message.sourceChainSelector, &message.sender);
        if !self.ccip_allowed_senders.get(allowlist_key) {
            return Err(b"CCIP_SENDER_NOT_ALLOWED".to_vec());
        }

        // 2. VALIDASI TOKEN YANG DITERIMA (Programmable Token Transfer)
        if message.destTokenAmounts.len() != 1 {
            return Err(b"EXACTLY_ONE_TOKEN_REQUIRED".to_vec());
        }
        let token_info = &message.destTokenAmounts[0];
        let received_token = token_info.token;
        let received_raw_amount = token_info.amount;

        // Tentukan Asset ID dan normalisasi satuan sirkuit
        let (asset_id, norm_liability_amount) = if received_token == self.asset_tokens.get(U256::from(1)) {
            // Asset ID 1: USDC (6 Desimal, 1 Raw = 1 Micro-USDC)
            if received_raw_amount < U256::from(10_000_000u64) {
                return Err(b"AMOUNT_TOO_SMALL_MIN_10_USDC".to_vec());
            }
            if received_raw_amount > U256::from(u64::MAX) {
                return Err(b"AMOUNT_EXCEEDS_U64_CIRCUIT_BOUND".to_vec());
            }
            (1u8, received_raw_amount)
        } else if received_token == self.asset_tokens.get(U256::ZERO)
            || (self.weth9_address.get() != Address::ZERO && received_token == self.weth9_address.get())
        {
            // Asset ID 0: WETH (18 Desimal, Raw = Wei, Norm = Gwei)
            if received_raw_amount < U256::from(5_000_000_000_000_000u64) {
                return Err(b"AMOUNT_TOO_SMALL_MIN_0_005_WETH".to_vec());
            }
            let gwei_scale = U256::from(1_000_000_000u64);
            if received_raw_amount % gwei_scale != U256::ZERO {
                return Err(b"WETH_AMOUNT_NOT_GWEI_ALIGNED".to_vec());
            }
            let amount_gwei = received_raw_amount / gwei_scale;
            if amount_gwei > U256::from(u64::MAX) {
                return Err(b"AMOUNT_EXCEEDS_U64_CIRCUIT_BOUND".to_vec());
            }
            (0u8, amount_gwei)
        } else {
            return Err(b"UNSUPPORTED_CCIP_TOKEN".to_vec());
        };

        // 3. UNPACK & VALIDASI WIRE PAYLOAD 128 BYTES
        if message.data.len() != 128 {
            return Err(b"INVALID_CCIP_PAYLOAD_LENGTH_EXPECTED_128".to_vec());
        }

        let mut note_cm_bytes = [0u8; 32];
        note_cm_bytes.copy_from_slice(&message.data[0..32]);
        let note_commitment = FixedBytes::<32>::from(note_cm_bytes);

        if note_commitment == FixedBytes::<32>::ZERO {
            return Err(b"ZERO_COMMITMENT_REJECTED".to_vec());
        }
        if from_evm_scalar(&note_commitment.0).is_none() {
            return Err(b"INVALID_NOTE_COMMITMENT_SCALAR".to_vec());
        }

        let encrypted_memo = message.data[32..128].to_vec();

        // 4. MUTASI STATE (EFFECTS - Strict CEI Compliant)
        self.ccip_processed_messages.insert(message.messageId, true);

        // Catat liabilitas 1:1 (0 bps protokol fee untuk deposit)
        let asset_key = U256::from(asset_id);
        let cur_liab = self.user_note_liabilities.get(asset_key);
        self.user_note_liabilities.setter(asset_key).set(
            cur_liab
                .checked_add(norm_liability_amount)
                .ok_or_else(|| b"LIABILITY_OVERFLOW".to_vec())?,
        );

        let cur_principal = self.total_deposited_principals.get(asset_key);
        self.total_deposited_principals.setter(asset_key).set(
            cur_principal
                .checked_add(norm_liability_amount)
                .ok_or_else(|| b"PRINCIPAL_OVERFLOW".to_vec())?,
        );

        // Sisipkan komitmen langsung ke Merkle Mountain Range (MMR)
        self._mmr_insert(note_commitment, encrypted_memo)?;

        // 5. PENGAWAL SOLVABILITAS SILOED
        self._check_asset_solvency_invariant(asset_id)?;

        // 6. EMIT EVENTS
        crate::events::emit_event(crate::events::CCIPDepositExecuted {
            message_id: message.messageId,
            source_chain_selector: message.sourceChainSelector,
            sender_hash: keccak256(&message.sender),
            note_commitment,
            asset_id,
            normalized_amount: norm_liability_amount,
        });

        Ok(())
    }
}
