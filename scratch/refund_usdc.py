import json
import subprocess
import time
from web3 import Web3
from eth_account import Account

RPC_URL = "https://arbitrum-sepolia.infura.io/v3/e0442523234742288f49543cb9e16da9"
CONTRACT_ADDRESS = "0xbda5fea381775a74ba8986090a919b17bb632a6d"
PRIVATE_KEY = "0xb89bc61712cfa0c890c0967f186c23afdf0b770743bc4f5505300100e8c7226e"
CHAIN_ID = 421614

w3 = Web3(Web3.HTTPProvider(RPC_URL))
account = Account.from_key(PRIVATE_KEY)
MY_ADDRESS = account.address

CONTRACT_ABI = [
    {
        "inputs": [
            {"name": "root", "type": "bytes32"},
            {"name": "nullifier", "type": "bytes32"},
            {"name": "alpha_neg_bytes", "type": "bytes"},
            {"name": "pk_iss_bytes", "type": "bytes"},
            {"name": "recipient", "type": "address"},
            {"name": "amount", "type": "uint256"},
            {"name": "recipient_or_intent_hash", "type": "bytes32"},
            {"name": "expiry", "type": "uint256"},
            {"name": "nonce", "type": "bytes32"}
        ],
        "name": "spend",
        "outputs": [{"name": "", "type": "bool"}],
        "stateMutability": "nonpayable",
        "type": "function"
    }
]

contract = w3.eth.contract(address=w3.to_checksum_address(CONTRACT_ADDRESS), abi=CONTRACT_ABI)

def query_encrypted_db(query):
    cmd = [
        "cargo", "run", "-q", "-p", "nimbus-node", "--bin", "query_db", "--",
        "test_leader.db", "my-hard-test-secure-db-key-12345", query
    ]
    res = subprocess.run(cmd, capture_output=True, text=True, cwd="/home/azureuser/crypto")
    if res.returncode != 0:
        print(f"DB Query failed: {res.stderr}")
        return []
    try:
        return json.loads(res.stdout.strip())
    except Exception as e:
        print(f"Failed to parse DB response: {e}")
        return []

def main():
    print("Fetching failed spends from DB...")
    rows = query_encrypted_db("SELECT id, request_json FROM spend_queue WHERE id IN (11, 21)")
    print(f"Found {len(rows)} spends in DB.")
    
    if not rows:
        print("No spends to process.")
        return

    # Get initial nonce
    nonce_tx = w3.eth.get_transaction_count(MY_ADDRESS)
    print(f"Current wallet nonce: {nonce_tx}")

    for idx, row in enumerate(rows):
        item_id = row["id"]
        req = json.loads(row["request_json"])
        
        # Parse fields
        nullifier = bytes.fromhex(req["nullifier"].replace("0x", ""))
        alpha_neg = bytes.fromhex(req["alpha_neg_hex"].replace("0x", ""))
        pk_iss = bytes.fromhex(req["pk_iss_hex"].replace("0x", ""))
        recipient = w3.to_checksum_address(req["recipient"])
        amount = int(req["amount"])
        
        root_hex = req.get("association_root_hex") or "0x0000000000000000000000000000000000000000000000000000000000000000"
        root = bytes.fromhex(root_hex.replace("0x", ""))
        
        intent_hex = req.get("recipient_or_intent_hash_hex") or "0x0000000000000000000000000000000000000000000000000000000000000000"
        intent = bytes.fromhex(intent_hex.replace("0x", ""))
        
        expiry = int(req.get("expiry") or 0)
        
        nonce_hex = req.get("nonce_hex") or "0x0000000000000000000000000000000000000000000000000000000000000000"
        nonce_val = bytes.fromhex(nonce_hex.replace("0x", ""))

        print(f"\n[{idx+1}/{len(rows)}] Processing DB item {item_id} (Nullifier: {req['nullifier'][:10]}...)")
        
        # Prepare transaction
        fn = contract.functions.spend(
            root,
            nullifier,
            alpha_neg,
            pk_iss,
            recipient,
            amount,
            intent,
            expiry,
            nonce_val
        )
        
        try:
            tx = fn.build_transaction({
                'from': MY_ADDRESS,
                'nonce': nonce_tx,
                'gas': 1500000,
                'maxFeePerGas': w3.to_wei(0.15, 'gwei'),
                'maxPriorityFeePerGas': w3.to_wei(0.005, 'gwei'),
                'chainId': CHAIN_ID
            })
            signed_tx = w3.eth.account.sign_transaction(tx, PRIVATE_KEY)
            print("  Broadcasting transaction...")
            tx_hash = w3.eth.send_raw_transaction(signed_tx.raw_transaction)
            print(f"  Tx Hash: {w3.to_hex(tx_hash)}")
            
            print("  Waiting for confirmation...")
            receipt = w3.eth.wait_for_transaction_receipt(tx_hash, timeout=60)
            print(f"  Confirmed in block {receipt['blockNumber']} (Status: {receipt['status']})")
            
            if receipt['status'] == 1:
                # Update DB to confirmed
                query_encrypted_db(f"UPDATE spend_queue SET status = 'confirmed', tx_hash = '{w3.to_hex(tx_hash)}' WHERE id = {item_id}")
                print(f"  DB updated successfully for item {item_id}.")
                nonce_tx += 1
            else:
                print(f"  WARNING: Transaction reverted on-chain for item {item_id}.")
                
        except Exception as e:
            print(f"  Error processing item {item_id}: {e}")
            # Refresh nonce in case of failure
            nonce_tx = w3.eth.get_transaction_count(MY_ADDRESS)

if __name__ == "__main__":
    main()
