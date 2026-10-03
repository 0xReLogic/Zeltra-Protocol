//! Solidity ABI-compatible event definitions for Nimbus Protocol
//!
//! Defined using alloy_sol_types::sol! for seamless EVM logging and indexer compatibility.

use alloy_sol_types::sol;

sol! {
    /// Emitted when a deposit is processed with its fee breakdown (DEC-016 & Gate D).
    event DepositFee(
        bytes32 indexed session_id,
        bytes32 indexed com_k_hash,
        address indexed client,
        uint256 gross_amount,
        uint256 fee,
        uint256 net_amount
    );

    /// Emitted when a spend transaction incurs protocol fees.
    event ProtocolFee(
        bytes32 indexed nullifier,
        address indexed recipient,
        uint256 amount,
        uint256 fee_bps,
        uint256 protocol_fee
    );

    /// Emitted when an execution fee is charged/accrued during spend execution.
    event ExecutionFee(
        bytes32 indexed nullifier,
        uint256 execution_fee,
        uint256 max_execution_fee
    );

    /// Emitted when a note commitment (initial or change note) is appended to the Merkle tree.
    event ChangeCommitment(
        uint256 indexed leaf_index,
        bytes32 indexed commitment,
        bytes32 new_root
    );

    /// Emitted when accrued execution fees or protocol yields are claimed.
    event FeeClaim(
        address indexed recipient,
        uint256 amount,
        bytes32 indexed fee_type
    );

    /// Emitted when deposit fee basis points configuration is updated by governance.
    event DepositFeeConfigUpdated(
        uint256 old_fee_bps,
        uint256 new_fee_bps
    );

    /// Emitted when an unresolved deposit is refunded after timelock expiry.
    event DepositRefunded(
        bytes32 indexed session_id,
        address indexed client,
        uint256 amount
    );
}

/// Safe event emission helper that logs to EVM in production and operates safely in unit tests.
#[inline]
pub fn emit_event<T: alloy_sol_types::SolEvent>(_event: T) {
    #[cfg(not(test))]
    {
        use stylus_sdk::stylus_core::host::LogAccess;
        crate::storage::Nimbus::runtime_host().log(_event);
    }
    #[cfg(test)]
    let _ = _event;
}
