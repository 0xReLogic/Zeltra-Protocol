import sys
from web3 import Web3

RPC_URL = "https://arbitrum-sepolia.infura.io/v3/e0442523234742288f49543cb9e16da9"
w3 = Web3(Web3.HTTPProvider(RPC_URL))

tx_hashes = [
    "0x282c71ce8f355980fbfbf90f30140c1cc950ffad705484e1eaae60ca788546a5",
    "0xe9abd973f4e956bb8ef382ee373689bf52c6ad06bd627fc02bc6dfc0f960717f",
    "0xb9b836894c73bf4bbed8e0f1ae7788b0a5f724c0ce5733a6554ecd6027be71b4",
    "0x7ff9f0275f22717952126b6677b801af7d38656992b605735932ea1d2cb5e607"
]

for tx in tx_hashes:
    try:
        rec = w3.eth.get_transaction_receipt(tx)
        print(f"Tx: {tx} | Status: {rec['status']} | Gas Used: {rec['gasUsed']}")
    except Exception as e:
        print(f"Tx: {tx} | Error: {e}")
