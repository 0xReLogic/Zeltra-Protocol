from web3 import Web3

RPC_URL = "https://arbitrum-sepolia.infura.io/v3/e0442523234742288f49543cb9e16da9"
USDC_ADDRESS = "0x75faf114eafb1bdbe2f0316df893fd58ce46aa4d"
RELAYER_ADDRESS = "0x23e32d309c575a3d5e7cd2867be12b00efa44bb1"

w3 = Web3(Web3.HTTPProvider(RPC_URL))
if not w3.is_connected():
    print("Error: Failed to connect to RPC")
    exit(1)

checksum_addr = w3.to_checksum_address(RELAYER_ADDRESS)
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
