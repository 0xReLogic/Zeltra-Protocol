import json
import urllib.request
import urllib.parse
import time
from web3 import Web3
from eth_account import Account
from web3.exceptions import ContractLogicError

# --- Configurations ---
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
            {"name": "sid", "type": "bytes32"},
            {"name": "com_k_bytes", "type": "bytes"},
            {"name": "amount", "type": "uint256"}
        ],
        "name": "deposit",
        "outputs": [],
        "stateMutability": "nonpayable",
        "type": "function"
    },
    {
        "inputs": [{"name": "sid", "type": "bytes32"}],
        "name": "claim_refund",
        "outputs": [],
        "stateMutability": "nonpayable",
        "type": "function"
    }
]

contract = w3.eth.contract(address=w3.to_checksum_address(CONTRACT_ADDRESS), abi=CONTRACT_ABI)

def fetch_tx_history(address):
    url = f"https://api-sepolia.arbiscan.io/api?module=account&action=txlist&address={address}&startblock=0&endblock=latest&sort=asc"
    req = urllib.request.Request(
        url,
        headers={"User-Agent": "Mozilla/5.0"}
    )
    try:
        with urllib.request.urlopen(req, timeout=15) as response:
            data = json.loads(response.read().decode())
            if data.get("status") == "1":
                return data.get("result", [])
            else:
                print(f"Arbiscan API error: {data.get('message')}")
                return []
    except Exception as e:
        print(f"Failed to fetch transaction history from Arbiscan: {e}")
        return []

def main():
    print(f"Scanning transaction history for wallet: {MY_ADDRESS}...")
    txs = fetch_tx_history(MY_ADDRESS)
    print(f"Found {len(txs)} total transactions.")

    if not txs:
        return

    # Filter deposits to the contract
    deposits = []
    for tx in txs:
        if tx.get("to", "").lower() == CONTRACT_ADDRESS.lower():
            input_data = tx.get("input", "")
            if input_data.startswith("0x2d1f7c24") or "deposit" in input_data: # deposit signature or heuristic
                deposits.append(tx)

    print(f"Found {len(deposits)} deposit transactions to the contract.")

    sids_processed = set()
    refundable_count = 0
    refunded_count = 0

    # Get latest transaction nonce
    nonce_tx = w3.eth.get_transaction_count(MY_ADDRESS)
    print(f"Starting wallet transaction nonce: {nonce_tx}")

    for idx, tx in enumerate(deposits):
        try:
            func, args = contract.decode_function_input(tx["input"])
            if func.fn_name == "deposit":
                sid = args["sid"]
                amount_usdc = args["amount"] / 1e6
                sid_hex = w3.to_hex(sid)

                if sid_hex in sids_processed:
                    continue
                sids_processed.add(sid_hex)

                print(f"\n[{idx+1}/{len(deposits)}] Checking Deposit sid: {sid_hex} | Amount: {amount_usdc} USDC")
                
                # Dry-run call claim_refund
                try:
                    contract.functions.claim_refund(sid).call({'from': MY_ADDRESS})
                    print(f"  -> [REFUNDABLE] Deposit of {amount_usdc} USDC is ready to be refunded!")
                    refundable_count += 1
                    
                    # Build and broadcast the transaction
                    gas_estimate = contract.functions.claim_refund(sid).estimate_gas({'from': MY_ADDRESS})
                    tx_build = contract.functions.claim_refund(sid).build_transaction({
                        'from': MY_ADDRESS,
                        'nonce': nonce_tx,
                        'gas': int(gas_estimate * 1.2),
                        'maxFeePerGas': w3.to_wei(0.15, 'gwei'),
                        'maxPriorityFeePerGas': w3.to_wei(0.005, 'gwei'),
                        'chainId': CHAIN_ID
                    })
                    
                    print("  Broadcasting refund transaction...")
                    signed = w3.eth.account.sign_transaction(tx_build, PRIVATE_KEY)
                    tx_hash = w3.eth.send_raw_transaction(signed.raw_transaction)
                    print(f"  Tx Hash: {w3.to_hex(tx_hash)}")
                    
                    print("  Waiting for confirmation...")
                    receipt = w3.eth.wait_for_transaction_receipt(tx_hash, timeout=60)
                    print(f"  Confirmed in block {receipt['blockNumber']} (Status: {receipt['status']})")
                    
                    if receipt['status'] == 1:
                        refunded_count += 1
                        nonce_tx += 1
                    
                except ContractLogicError as cle:
                    # Parse revert reason
                    reason = str(cle)
                    if "TIMELOCK_NOT_EXPIRED" in reason:
                        print("  -> Skip: Timelock (24h) has not expired yet.")
                    elif "SESSION_ALREADY_RESOLVED" in reason:
                        print("  -> Skip: Already resolved (spent or refunded).")
                    elif "NO_DEPOSIT_FOUND" in reason:
                        print("  -> Skip: No deposit found on-chain.")
                    else:
                        print(f"  -> Skip (Revert reason): {reason}")
                except Exception as e:
                    print(f"  -> Skip (Dry-run failed): {e}")

        except Exception as e:
            print(f"Error decoding transaction {tx.get('hash')}: {e}")

    print("\n" + "="*50)
    print(f"Refund scan completed.")
    print(f"Total deposits checked: {len(sids_processed)}")
    print(f"Total refundable found: {refundable_count}")
    print(f"Total successfully refunded: {refunded_count}")
    print("="*50)

if __name__ == "__main__":
    main()
