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

transfer_event_topic = "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef"

for tx_hash in tx_hashes:
    try:
        tx = w3.eth.get_transaction(tx_hash)
        rec = w3.eth.get_transaction_receipt(tx_hash)
        print(f"\nTx: {tx_hash} | To: {tx['to']} | Status: {rec['status']}")
        print(f"  Input: {tx['input'].hex()[:30]}...")
        # Look for Transfer events (USDC has 6 decimals)
        for log in rec.get("logs", []):
            print(f"  Log from address: {log['address']}")
            topics = [t.hex() for t in log.get("topics", [])]
            print(f"    Topics: {topics}")
            print(f"    Data: {log['data'].hex()}")
            if topics and topics[0] == transfer_event_topic:
                src = "0x" + topics[1][-40:]
                dst = "0x" + topics[2][-40:]
                val = int(log["data"].hex(), 16) / 1e6
                print(f"      USDC Transfer: {src} -> {dst} | Value: {val} USDC")
    except Exception as e:
        print(f"Tx: {tx_hash} | Error: {e}")
