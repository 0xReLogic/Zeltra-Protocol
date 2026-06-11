//! External contract interfaces

use stylus_sdk::prelude::*;

sol_interface! {
    interface IErc20 {
        function transferFrom(address from, address to, uint256 value) external returns (bool);
        function transfer(address to, uint256 value) external returns (bool);
        function balanceOf(address owner) external view returns (uint256);
        function approve(address spender, uint256 value) external returns (bool);
    }

    interface IConditionalTokens {
        function splitPosition(
            address collateral_token,
            bytes32 parent_collection_id,
            bytes32 condition_id,
            uint256[] partition,
            uint256 amount
        ) external;
    }
}
