//! External contract interfaces

use stylus_sdk::prelude::*;

sol_interface! {
    interface IErc20 {
        function transferFrom(address from, address to, uint256 value) external returns (bool);
        function transfer(address to, uint256 value) external returns (bool);
        function balanceOf(address owner) external view returns (uint256);
        function approve(address spender, uint256 value) external returns (bool);
    }

    interface IWeth9 {
        function deposit() external payable;
        function withdraw(uint256 wad) external;
        function transfer(address to, uint256 value) external returns (bool);
        function transferFrom(address from, address to, uint256 value) external returns (bool);
        function balanceOf(address owner) external view returns (uint256);
        function approve(address spender, uint256 value) external returns (bool);
    }
}
