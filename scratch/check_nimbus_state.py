import sys
from web3 import Web3

RPC_URL = "https://arbitrum-sepolia.infura.io/v3/e0442523234742288f49543cb9e16da9"
CONTRACT_ADDRESS = "0xbda5fea381775a74ba8986090a919b17bb632a6d"

w3 = Web3(Web3.HTTPProvider(RPC_URL))
if not w3.is_connected():
    print("Error: Failed to connect to RPC")
    sys.exit(1)

ABI = [
    {"inputs": [], "name": "stablecoin", "outputs": [{"type": "address"}], "stateMutability": "view", "type": "function"},
    {"inputs": [], "name": "fee_recipient", "outputs": [{"type": "address"}], "stateMutability": "view", "type": "function"},
    {"inputs": [], "name": "totalDepositedPrincipal", "outputs": [{"type": "uint256"}], "stateMutability": "view", "type": "function"},
    {"inputs": [], "name": "feeBps", "outputs": [{"type": "uint256"}], "stateMutability": "view", "type": "function"},
    {"inputs": [], "name": "owner", "outputs": [{"type": "address"}], "stateMutability": "view", "type": "function"},
]

contract = w3.eth.contract(address=w3.to_checksum_address(CONTRACT_ADDRESS), abi=ABI)

for method in ["stablecoin", "fee_recipient", "totalDepositedPrincipal", "feeBps", "owner"]:
    try:
        res = getattr(contract.functions, method)().call()
        if method in ["totalDepositedPrincipal"]:
            print(f"{method}: {res / 1e6} USDC")
        else:
            print(f"{method}: {res}")
    except Exception as e:
        print(f"{method} failed: {e}")
