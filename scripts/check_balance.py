from web3 import Web3
import os
import sys

RPC_URL = os.environ.get("RPC_URL")
USDC_ADDRESS = os.environ.get("USDC_ADDRESS", "0x75faf114eafb1bdbe2f0316df893fd58ce46aa4d")
ADDRESS = os.environ.get("ADDRESS") or os.environ.get("NIMBUS_RELAYER_ADDRESS")

if not RPC_URL:
    print("Error: missing required environment variable: RPC_URL")
    sys.exit(2)

if not ADDRESS:
    print("Error: missing required environment variable: ADDRESS or NIMBUS_RELAYER_ADDRESS")
    sys.exit(2)

w3 = Web3(Web3.HTTPProvider(RPC_URL))
if not w3.is_connected():
    print("Error: Failed to connect to RPC")
    exit(1)

checksum_addr = w3.to_checksum_address(ADDRESS)
eth_bal = w3.eth.get_balance(checksum_addr)
print(f"Address: {checksum_addr}")
print(f"ETH Balance: {w3.from_wei(eth_bal, 'ether')} ETH")

# ERC-20 BalanceOf
abi = [{
    "inputs": [{"name": "account", "type": "address"}],
    "name": "balanceOf",
    "outputs": [{"name": "", "type": "uint256"}],
    "stateMutability": "view",
    "type": "function"
}]
usdc_contract = w3.eth.contract(address=w3.to_checksum_address(USDC_ADDRESS), abi=abi)
usdc_bal = usdc_contract.functions.balanceOf(checksum_addr).call()
print(f"USDC Balance: {usdc_bal / 1_000_000} USDC")
