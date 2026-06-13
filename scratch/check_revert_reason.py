import urllib.request
import json
import sys

rpc_url = "https://arbitrum-sepolia.infura.io/v3/e0442523234742288f49543cb9e16da9"
tx_hash = "0xac175659f1abb67b2cc2819b35287d1868385b31ba7c86b79d1df3cb40bb0d79"

if len(sys.argv) > 1:
    tx_hash = sys.argv[1]

def get_tx_receipt(tx_hash):
    payload = {
        "jsonrpc": "2.0",
        "method": "eth_getTransactionReceipt",
        "params": [tx_hash],
        "id": 1
    }
    req = urllib.request.Request(
        rpc_url,
        data=json.dumps(payload).encode('utf-8'),
        headers={'Content-Type': 'application/json'}
    )
    with urllib.request.urlopen(req) as response:
        res = json.loads(response.read().decode('utf-8'))
        return res.get('result')

def get_tx(tx_hash):
    payload = {
        "jsonrpc": "2.0",
        "method": "eth_getTransactionByHash",
        "params": [tx_hash],
        "id": 1
    }
    req = urllib.request.Request(
        rpc_url,
        data=json.dumps(payload).encode('utf-8'),
        headers={'Content-Type': 'application/json'}
    )
    with urllib.request.urlopen(req) as response:
        res = json.loads(response.read().decode('utf-8'))
        return res.get('result')

def check_revert_reason(tx, block_number):
    # Try calling eth_call at block_number
    # Sometimes node doesn't have state for that block, so we'll try 'latest' too if it fails
    payload = {
        "jsonrpc": "2.0",
        "method": "eth_call",
        "params": [{
            "from": tx["from"],
            "to": tx["to"],
            "gas": tx["gas"],
            "value": tx["value"],
            "data": tx["input"]
        }, block_number],
        "id": 2
    }
    req = urllib.request.Request(
        rpc_url,
        data=json.dumps(payload).encode('utf-8'),
        headers={'Content-Type': 'application/json'}
    )
    try:
        with urllib.request.urlopen(req) as response:
            res = json.loads(response.read().decode('utf-8'))
            return res
    except Exception as e:
        return {"error": str(e)}

receipt = get_tx_receipt(tx_hash)
tx = get_tx(tx_hash)

if not receipt:
    print(f"Receipt not found for {tx_hash}")
    sys.exit(1)

print("Receipt Status:", receipt.get("status"))
print("Gas Used:", int(receipt.get("gasUsed", "0x0"), 16))

if receipt and tx:
    block_num_hex = receipt["blockNumber"]
    prev_block = hex(int(block_num_hex, 16) - 1)
    print(f"Calling eth_call at previous block {prev_block}...")
    revert_res = check_revert_reason(tx, prev_block)
    if "error" in revert_res or (revert_res.get("result") == "0x" or not revert_res.get("result")):
        print(f"Calling eth_call at latest...")
        revert_res = check_revert_reason(tx, "latest")
    
    print("\nRevert Result:")
    print(json.dumps(revert_res, indent=2))
    
    if revert_res and "result" in revert_res:
        result_hex = revert_res["result"]
        print(f"\nResult Hex: {result_hex}")
        if result_hex.startswith("0x08c379a0"): # Error(string)
            data = bytes.fromhex(result_hex[10:])
            try:
                # offset is 32 bytes, length is 32 bytes, then string
                str_len = int.from_bytes(data[32:64], byteorder='big')
                str_val = data[64:64+str_len].decode('utf-8', errors='ignore')
                print(f"Decoded Revert String: {str_val}")
            except Exception as e:
                print(f"Decoding failed: {e}")
        elif len(result_hex) > 10:
            # Check for standard custom error signature
            # e.g. bytes representation
            try:
                err_bytes = bytes.fromhex(result_hex[2:])
                print(f"Raw bytes representation: {err_bytes}")
                print(f"Decoded ascii (errors ignore): {err_bytes.decode('utf-8', errors='ignore')}")
            except Exception as e:
                pass
