//! Contract storage definition

use stylus_sdk::prelude::*;

sol_storage! {
    #[entrypoint]
    pub struct Zeltra {
        // Mapping of valid Merkle roots of clean association sets to their registration timestamp (Fase A: ZK-Compliance)
        mapping(bytes32 => uint256) clean_association_roots;

        // Owner address for admin operations (Fase E: Security)
        address owner;
        bool paused;

        // The ERC-20 stablecoin contract address (kept for backwards-compatibility)
        address stablecoin;
        // Address that receives the protocol fees
        address fee_recipient;

        // --- Principal Tracking for Liability Invariant (DEC-034B Multi-Asset) ---
        // 0: WETH (Gwei), 1: USDC (Micro-units)
        mapping(uint256 => uint256) total_deposited_principals;

        // --- Two-Step Governance & Timelocked Parameter Changes (Issue 10) ---
        address pending_owner;
        address proposed_fee_recipient;
        uint256 fee_recipient_eta;

        // --- CCIP Router Address Configuration (Production Security Best Practice) ---
        address ccip_router;

        // --- CCIP Security (DEC-015) ---
        mapping(bytes32 => bool) ccip_processed_messages;
        mapping(bytes32 => bool) ccip_allowed_senders;

        // --- Pendaftaran Token Resmi (DEC-034B Curated Pair) ---
        // mapping asset_id (0: WETH9, 1: USDC) => alamat smart contract ERC-20
        mapping(uint256 => address) asset_tokens;
        address weth9_address;

        // --- Execution Fee Recipient (Signed Quote Revenue Engine) ---
        address execution_fee_recipient;

        // --- Multi-Liability Storage Accounting (DEC-034B & DEC-038B) ---
        // Sum of all unspent private note values held by users per asset_id (0: WETH Gwei, 1: USDC Micro)
        mapping(uint256 => uint256) user_note_liabilities;
        // Execution fees accrued to relayer but not yet claimed per asset_id
        mapping(uint256 => uint256) accrued_execution_fee_liabilities;
        // Protocol fees realized and retained as protocol equity per asset_id
        mapping(uint256 => uint256) realized_protocol_fees;

        // --- Note Commitment Merkle Mountain Range (DEC-032) ---
        // Current total leaf count in the MMR (0 .. 2^64)
        uint256 mmr_leaf_count;
        // Current Merkle tree root hash (canonical bagged root)
        bytes32 note_tree_root;
        // Active MMR peaks mapped by mountain height level (0 .. 63)
        mapping(uint256 => bytes32) mmr_peaks;
        // Bounded accepted roots: mapping root hash to registration block timestamp
        mapping(bytes32 => uint256) accepted_note_roots;
        // Ring buffer history of recent roots (size 100)
        mapping(uint256 => bytes32) root_history;
        // Pointer index in the root history ring buffer (0 .. 99)
        uint256 root_history_index;

        // --- Dynamic Deposit Fee Configuration (docs/todos/fee-policy-zero-deposit.md) ---
        // Configurable deposit fee in basis points (e.g. 0 for 0%, 20 for 0.20%)
        uint256 deposit_fee_bps;
        bool deposit_fee_initialized;
        // Two-step governance for deposit fee change
        uint256 proposed_deposit_fee_bps;
        uint256 deposit_fee_eta;

        // --- Generational Epoch Window & Nullifier Pruning (DEC-033) ---
        // Current active epoch ID (starts at 0)
        uint256 current_epoch;
        // Timestamp when the current epoch started
        uint256 epoch_start_timestamp;
        // Generational nullifiers: mapping composite key keccak256(slot, nullifier) => bool
        // where slot = epoch_id % 2
        mapping(bytes32 => bool) generation_nullifiers;
        // Generation epoch ID: mapping slot (0 or 1) => epoch_id
        mapping(uint256 => uint256) generation_epoch_id;
    }
}

/// Compute composite storage key for generational nullifier: keccak256(slot || nullifier)
pub fn generational_nullifier_key(
    slot: u8,
    nullifier: alloy_primitives::FixedBytes<32>,
) -> alloy_primitives::FixedBytes<32> {
    use alloy_primitives::keccak256;
    let mut buf = [0u8; 33];
    buf[0] = slot;
    buf[1..33].copy_from_slice(nullifier.as_slice());
    keccak256(buf)
}

impl Default for Zeltra {
    fn default() -> Self {
        let host = stylus_sdk::host::VM {
            host: stylus_sdk::host::WasmVM {},
        };
        unsafe { Self::new(stylus_sdk::alloy_primitives::U256::ZERO, 0, host) }
    }
}
