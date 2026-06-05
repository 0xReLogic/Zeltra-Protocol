import sys
from web3 import Web3
from eth_account import Account

RPC_URL = "https://sepolia-rollup.arbitrum.io/rpc"
CONTRACT_ADDRESS = "0x582fdc90f17a15d27b76175402898ab48f6e6ef3"

# ABIs
NIMBUS_ABI = [
    {
        "inputs": [],
        "name": "stablecoin",
        "outputs": [{"internalType": "address", "name": "", "type": "address"}],
        "stateMutability": "view",
        "type": "function"
    },
    {
        "inputs": [],
        "name": "fee_recipient",
        "outputs": [{"internalType": "address", "name": "", "type": "address"}],
        "stateMutability": "view",
        "type": "function"
    },
    {
        "inputs": [],
        "name": "fast_path_phase",
        "outputs": [{"internalType": "uint256", "name": "", "type": "uint256"}],
        "stateMutability": "view",
        "type": "function"
    }
]

def main():
    w3 = Web3(Web3.HTTPProvider(RPC_URL))
    contract_addr = Web3.to_checksum_address(CONTRACT_ADDRESS.lower())
    contract = w3.eth.contract(address=contract_addr, abi=NIMBUS_ABI)
    
    print("Querying contract view functions...")
    for func_name in ["stablecoin", "fee_recipient", "fast_path_phase"]:
        try:
            func = getattr(contract.functions, func_name)
            res = func().call()
            print(f"  {func_name}(): {res}")
        except Exception as e:
            print(f"  {func_name}() failed: {e}")

if __name__ == '__main__':
    main()
