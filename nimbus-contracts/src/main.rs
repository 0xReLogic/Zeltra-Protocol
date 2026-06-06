#![cfg_attr(not(any(test, feature = "export-abi")), no_main)]

#[cfg(not(any(test, feature = "export-abi")))]
#[no_mangle]
pub extern "C" fn main() {}

#[cfg(feature = "export-abi")]
fn main() {
    use stylus_sdk::abi::export::print_abi;
    print_abi::<nimbus_contracts::Nimbus>("UNLICENSED", "pragma solidity ^0.8.21;");
}
