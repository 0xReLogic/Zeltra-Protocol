#!/usr/bin/env python3
import json
import time
from web3 import Web3
from eth_account import Account
from web3.exceptions import ContractLogicError

# --- Configurations ---
RPC_URL = "https://arbitrum-sepolia.infura.io/v3/e0442523234742288f49543cb9e16da9"
CONTRACT_ADDRESS = "0xbda5fea381775a74ba8986090a919b17bb632a6d"
PRIVATE_KEY = "0xb89bc61712cfa0c890c0967f186c23afdf0b770743bc4f5505300100e8c7226e"
USDC_ADDRESS = "0x75faf114eafb1bdbe2f0316df893fd58ce46aa4d"
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

def main():
    print(f"Connecting to RPC: {RPC_URL}...")
    if not w3.is_connected():
        print("Failed to connect to RPC")
        return

    print(f"USDC Contract: {USDC_ADDRESS}")
    print(f"Nimbus Contract: {CONTRACT_ADDRESS}")
    print(f"Wallet Address: {MY_ADDRESS}")

    # Transfer event signature topic
    transfer_topic = "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef"
    from_topic = "0x" + MY_ADDRESS[2:].lower().rjust(64, '0')
    to_topic = "0x" + CONTRACT_ADDRESS[2:].lower().rjust(64, '0')

    print("\nQuerying transfer logs from USDC contract...")
    # Fetch logs of USDC transfer from wallet to Nimbus contract
    logs = w3.eth.get_logs({
        "fromBlock": 276388000, # Start block slightly before deployment
        "toBlock": "latest",
        "address": w3.to_checksum_address(USDC_ADDRESS),
        "topics": [transfer_topic, from_topic, to_topic]
    })
    
    print(f"Found {len(logs)} USDC transfer logs to the contract.")

    tx_hashes = sorted(list(set([l["transactionHash"].hex() for l in logs])))
    print(f"Unique transaction hashes to check: {len(tx_hashes)}")

    sids_to_refund = []

    for idx, tx_hash in enumerate(tx_hashes):
        try:
            tx = w3.eth.get_transaction(tx_hash)
            input_data = tx["input"]
            
            # Check if calling the deposit method (method selector: 0xc391f7c5)
            if input_data.hex().startswith("c391f7c5"):
                func, args = contract.decode_function_input(input_data)
                sid = args["sid"]
                sid_hex = w3.to_hex(sid)
                amount_usdc = args["amount"] / 1e6
                print(f"[{idx+1}/{len(tx_hashes)}] Found Deposit sid: {sid_hex} | Amount: {amount_usdc} USDC")
                sids_to_refund.append((sid, sid_hex, amount_usdc))
        except Exception as e:
            print(f"Error checking tx {tx_hash}: {e}")

    if not sids_to_refund:
        print("\nNo deposit sessions found to refund.")
        return

    print(f"\nScanning {len(sids_to_refund)} deposits for refund eligibility...")
    refundable_sids = []
    
    for sid, sid_hex, amount in sids_to_refund:
        try:
            # Dry-run call claim_refund
            contract.functions.claim_refund(sid).call({'from': MY_ADDRESS})
            print(f"  -> [REFUNDABLE] sid: {sid_hex} | Amount: {amount} USDC is ready to be refunded.")
            refundable_sids.append(sid)
        except ContractLogicError as cle:
            reason = str(cle)
            if "TIMELOCK_NOT_EXPIRED" in reason:
                print(f"  -> [LOCKED] sid: {sid_hex} | Amount: {amount} USDC (24h timelock not expired).")
            elif "SESSION_ALREADY_RESOLVED" in reason:
                # Already spent or refunded
                pass
            else:
                print(f"  -> [SKIPPED] sid: {sid_hex} | Revert reason: {reason}")
        except Exception as e:
            print(f"  -> [SKIPPED] sid: {sid_hex} | Error: {e}")

    if not refundable_sids:
        print("\nNo refundable sessions are eligible right now.")
        return

    print(f"\nProcessing refunds for {len(refundable_sids)} sessions...")
    nonce_tx = w3.eth.get_transaction_count(MY_ADDRESS)
    
    refunded_count = 0
    for sid in refundable_sids:
        try:
            gas_estimate = contract.functions.claim_refund(sid).estimate_gas({'from': MY_ADDRESS})
            tx_build = contract.functions.claim_refund(sid).build_transaction({
                'from': MY_ADDRESS,
                'nonce': nonce_tx,
                'gas': int(gas_estimate * 1.25),
                'maxFeePerGas': w3.to_wei(0.15, 'gwei'),
                'maxPriorityFeePerGas': w3.to_wei(0.005, 'gwei'),
                'chainId': CHAIN_ID
            })
            signed = w3.eth.account.sign_transaction(tx_build, PRIVATE_KEY)
            h = w3.eth.send_raw_transaction(signed.raw_transaction)
            print(f"Sent refund transaction for sid {w3.to_hex(sid)[:16]}... | Tx Hash: {w3.to_hex(h)}")
            receipt = w3.eth.wait_for_transaction_receipt(h, timeout=60)
            if receipt['status'] == 1:
                print(f"  Confirmed in block {receipt['blockNumber']} (Success)")
                refunded_count += 1
                nonce_tx += 1
            else:
                print(f"  Transaction REVERTED")
        except Exception as e:
            print(f"Failed to execute refund for sid {w3.to_hex(sid)}: {e}")

    print(f"\nSuccessfully refunded {refunded_count} sessions.")

if __name__ == "__main__":
    main()
