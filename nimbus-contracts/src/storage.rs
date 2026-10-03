//! Contract storage definition

use stylus_sdk::prelude::*;

sol_storage! {
    #[entrypoint]
    pub struct Nimbus {
        // Mapping of Issuer Public Key hash to their escrowed collateral
        mapping(bytes32 => uint256) collateral;

        // Mapping of Session ID to their client address, masking key commitment, and deposit amount
        mapping(bytes32 => address) session_client;
        mapping(bytes32 => uint256) session_amount;
        mapping(bytes32 => bool) session_resolved;

        // Nullifier mapping to prevent double-spending of ephemeral keys
        mapping(bytes32 => bool) nullifiers;

        // Mapping of valid Merkle roots of clean association sets to their registration timestamp (Fase A: ZK-Compliance)
        mapping(bytes32 => uint256) clean_association_roots;

        // Owner address for admin operations (Fase E: Security)
        address owner;
        bool paused;

        // Session timestamp to enforce timelocks for auto-refund
        mapping(bytes32 => uint256) session_timestamp;

        // The ERC-20 stablecoin contract address
        address stablecoin;
        // Address that receives the protocol fees
        address fee_recipient;

        // --- Principal Tracking for Liability Invariant ---
        uint256 total_deposited_principal;

        // --- Polymarket CTF Fallback (Roadmap Item 3) ---
        mapping(bytes32 => uint256) failed_intent_refunds;

        // --- Two-Step Governance & Timelocked Parameter Changes (Issue 10) ---
        address pending_owner;
        address proposed_fee_recipient;
        uint256 fee_recipient_eta;

        // --- CCIP Router Address Configuration (Production Security Best Practice) ---
        address ccip_router;

        // --- Deposit commitment binding (DEC-001) ---
        // Appended to preserve the existing storage layout.
        mapping(bytes32 => bytes32) session_commitment_hash;
        mapping(bytes32 => bool) session_exists;

        // --- Spend authorization (DEC-002) ---
        mapping(bytes32 => bool) trusted_issuer_keys;

        // --- CCIP Security (DEC-015) ---
        mapping(bytes32 => bool) ccip_processed_messages;
        mapping(bytes32 => bool) ccip_allowed_senders;

        // --- Execution Fee Accumulation (Signed Quote Revenue Engine) ---
        // Accumulated execution fees from spends, claimable by execution_fee_recipient
        uint256 accumulated_execution_fees;
        // Address authorized to claim accumulated execution fees (relayer/operator)
        address execution_fee_recipient;

        // --- Multi-Liability Storage Accounting (DEC-016 & Gate B) ---
        // Sum of all unspent private note values held by users
        uint256 user_note_liability;
        // Sum of unresolved deposits that can still be refunded
        uint256 refundable_deposit_liability;
        // Execution fees accrued to relayer but not yet claimed
        uint256 accrued_execution_fee_liability;
        // Protocol fees realized and retained as protocol equity
        uint256 realized_protocol_fees;
    }
}

impl Default for Nimbus {
    fn default() -> Self {
        let host = stylus_sdk::host::VM {
            host: stylus_sdk::host::WasmVM {},
        };
        unsafe { Self::new(stylus_sdk::alloy_primitives::U256::ZERO, 0, host) }
    }
}
