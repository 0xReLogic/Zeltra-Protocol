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

        // Mapping of valid Merkle roots of clean association sets (Fase A: ZK-Compliance)
        mapping(bytes32 => bool) clean_association_roots;

        // Owner address for admin operations (Fase E: Security)
        address owner;
        bool paused;

        // Session timestamp to enforce timelocks for auto-refund
        mapping(bytes32 => uint256) session_timestamp;

        // The ERC-20 stablecoin contract address
        address stablecoin;
        // Address that receives the protocol fees
        address fee_recipient;

        // --- Fast-Path Liquidity Premium (Roadmap Fase 1-3) ---
        // Active phase: 1 = Standard CCIP, 2 = Treasury-funded, 3 = Public LP with Dynamic Cap
        uint256 fast_path_phase;
        // LP Pool tracking variables for Fase 3
        uint256 total_lp_liquidity;
        uint256 utilized_lp_liquidity;

        // --- DeFi & RWA Integration (Roadmap Item 2) ---
        address aave_pool;
        address a_token;
        address rwa_token;
        uint256 total_deposited_principal;

        // --- Dynamic Liquidity Rebalancing (Moving Average Volatility) ---
        uint256 current_epoch_id;
        uint256 current_epoch_volume;
        uint256 epoch_start_timestamp;
        mapping(uint256 => uint256) historical_epoch_volumes;
        uint256 target_cash_pct;

        // --- Polymarket CTF Fallback (Roadmap Item 3) ---
        mapping(bytes32 => uint256) failed_intent_refunds;

        // --- Two-Step Governance & Timelocked Parameter Changes (Issue 10) ---
        address pending_owner;
        address proposed_fee_recipient;
        uint256 fee_recipient_eta;
        uint256 proposed_fast_path_phase;
        uint256 fast_path_phase_eta;
        address proposed_aave_pool;
        address proposed_a_token;
        uint256 aave_params_eta;
        address proposed_rwa_token;
        uint256 rwa_token_eta;
        
        // --- CCIP Router Address Configuration (Production Security Best Practice) ---
        address ccip_router;
    }
}

impl Default for Nimbus {
    fn default() -> Self {
        unsafe { Self::new(stylus_sdk::alloy_primitives::U256::ZERO, 0) }
    }
}
