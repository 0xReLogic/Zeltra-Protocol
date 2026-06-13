import sys
from web3 import Web3

RPC_URL = "https://arbitrum-sepolia.infura.io/v3/e0442523234742288f49543cb9e16da9"
MY_ADDRESS = "0x23e32D309c575A3D5E7CD2867BE12B00efa44Bb1"
CONTRACT_ADDRESS = "0xbda5fea381775a74ba8986090a919b17bb632a6d"
USDC_ADDRESS = "0x75faf114eafb1bdbe2f0316df893fd58ce46aa4d"

w3 = Web3(Web3.HTTPProvider(RPC_URL))
if not w3.is_connected():
    print("Failed to connect to RPC")
    sys.exit(1)

# Get USDC Balance of Wallet
erc20_abi = [
    {"inputs": [{"name": "account", "type": "address"}], "name": "balanceOf", "outputs": [{"name": "", "type": "uint256"}], "stateMutability": "view", "type": "function"},
]
usdc = w3.eth.contract(address=w3.to_checksum_address(USDC_ADDRESS), abi=erc20_abi)
wallet_usdc = usdc.functions.balanceOf(MY_ADDRESS).call() / 1e6
contract_usdc = usdc.functions.balanceOf(w3.to_checksum_address(CONTRACT_ADDRESS)).call() / 1e6

print(f"Wallet USDC Balance:   {wallet_usdc:.6f} USDC")
print(f"Contract USDC Balance: {contract_usdc:.6f} USDC")
