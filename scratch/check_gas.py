import sys
from web3 import Web3

# Use Infura first since Chainstack seems to be slow/hanging
rpcs = [
    "https://arbitrum-sepolia.infura.io/v3/e0442523234742288f49543cb9e16da9",
    "https://arbitrum-sepolia.core.chainstack.com/d18e11a2327c1a17c030975e3e0c8e24",
    "https://sepolia-rollup.arbitrum.io/rpc"
]

txs = {
    "Approve": "0x33ec8d287afdce1fac69193fa8f4e5329f1bd802f989bb3d10905649a2dd2285",
    "Deposit": "0x487adb72ea9d6e71f0019571397583a74882d0f35e48bbc0efb97dfae074c625",
    "Reveal":  "0xdcf165e5a1533ae552c4e02b6c8d065d06d02d795b625adc784ecbafb1c6e9c6",
    "Spend":   "0x4d734b76673b7c213f32434852e717752320dae93f0d72de8846e569d73bf231"
}

w3 = None
for rpc in rpcs:
    print(f"Connecting to RPC: {rpc}...")
    try:
        temp_w3 = Web3(Web3.HTTPProvider(rpc, request_kwargs={'timeout': 10}))
        if temp_w3.is_connected():
            w3 = temp_w3
            print("Connected successfully!")
            break
    except Exception as e:
        print(f"Failed to connect to {rpc}: {e}")

if w3 is None:
    print("Error: Could not connect to any RPC endpoint.")
    sys.exit(1)

for name, tx_hash in txs.items():
    try:
        print(f"Fetching receipt for {name} ({tx_hash[:10]}...)...")
        receipt = w3.eth.get_transaction_receipt(tx_hash)
        gas_used = receipt.gasUsed
        gas_price_gwei = receipt.effectiveGasPrice / 1e9
        cost_eth = (gas_used * receipt.effectiveGasPrice) / 1e18
        print(f"  -> {name} gasUsed: {gas_used} | Gas Price: {gas_price_gwei:.6f} gwei | Cost: {cost_eth:.8f} ETH")
    except Exception as e:
        print(f"  -> Failed to fetch {name}: {e}")
