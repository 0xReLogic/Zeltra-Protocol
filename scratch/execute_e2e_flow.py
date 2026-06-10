import sys
import subprocess
import os
import time
from web3 import Web3
from eth_account import Account

rpc_url = "https://arbitrum-sepolia.core.chainstack.com/d18e11a2327c1a17c030975e3e0c8e24"
contract_address = "0x62ca774e20b76431d189e1635400b91b03c2b031"
usdc_address = "0x75faf114eafb1bdbe2f0316df893fd58ce46aa4d"
private_key = "0xb89bc61712cfa0c890c0967f186c23afdf0b770743bc4f5505300100e8c7226e"

w3 = Web3(Web3.HTTPProvider(rpc_url))
if not w3.is_connected():
    print("Error: Cannot connect to RPC")
    sys.exit(1)

account = Account.from_key(private_key)
my_address = account.address

# Generate unique session ID and nonce
sid = os.urandom(32)
nonce = os.urandom(32)
nonce_hex = nonce.hex()
sid_hex = "0x" + sid.hex()

print("==========================================================")
print("RUNNING REAL E2E DEPOSIT (10 USDC) & SPEND (5 USDC) FLOW")
print("==========================================================")
print(f"Session ID: {sid_hex}")
print(f"Nonce: 0x{nonce_hex}")
print(f"User address: {my_address}")

# Execute cargo run to get BLS test vector
print("\n[Step 1] Generating cryptographic BLS signatures...")
cmd = [
    "cargo", "run", "-q", "-p", "nimbus-core", "--example", "generate_bls_test_data", "--",
    "--spend-contract",
    "--chain-id", "421614",
    "--contract", contract_address,
    "--amount", "5000000",
    "--recipient", my_address,
    "--nonce", nonce_hex
]
res = subprocess.run(cmd, capture_output=True, text=True, cwd="/home/azureuser/crypto")
if res.returncode != 0:
    print("Error executing BLS generator:")
    print(res.stderr)
    sys.exit(1)

lines = res.stdout.strip().split("\n")
data = {}
for line in lines:
    if ":" in line:
        k, v = line.split(":", 1)
        data[k.strip()] = v.strip()

nullifier_hex = data["nullifier_hex"]
alpha_neg_hex = data["alpha_neg_hex"]
hm_hex = data["hm_hex"]
pk_iss_hex = data["pk_iss_hex"]
k_hex = data["k_hex"]
com_k_hex = data["com_k_hex"]

print(f"  Nullifier: {nullifier_hex}")
print(f"  Alpha Neg: {alpha_neg_hex[:30]}...")
print(f"  H(m):      {hm_hex[:30]}...")
print(f"  pk_iss:    {pk_iss_hex[:30]}...")
print(f"  k:         {k_hex}")
print(f"  com_k:     {com_k_hex[:30]}...")

contract_abi = [
    {
        "inputs": [{"name": "pk_iss_bytes", "type": "bytes"}],
        "name": "isIssuerKeyTrusted",
        "outputs": [{"name": "", "type": "bool"}],
        "stateMutability": "view",
        "type": "function"
    },
    {
        "inputs": [{"name": "pk_iss_bytes", "type": "bytes"}],
        "name": "registerIssuerKey",
        "outputs": [],
        "stateMutability": "nonpayable",
        "type": "function"
    },
    {
        "inputs": [
            {"name": "sid", "type": "bytes32"},
            {"name": "_com_k_bytes", "type": "bytes"},
            {"name": "amount", "type": "uint256"}
        ],
        "name": "deposit",
        "outputs": [],
        "stateMutability": "nonpayable",
        "type": "function"
    },
    {
        "inputs": [
            {"name": "sid", "type": "bytes32"},
            {"name": "k_bytes", "type": "bytes"},
            {"name": "pk_iss_bytes", "type": "bytes"},
            {"name": "com_k_bytes", "type": "bytes"}
        ],
        "name": "revealMaskKey",
        "outputs": [{"name": "", "type": "bool"}],
        "stateMutability": "nonpayable",
        "type": "function"
    },
    {
        "inputs": [
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

erc20_abi = [
    {
        "inputs": [{"name": "account", "type": "address"}],
        "name": "balanceOf",
        "outputs": [{"name": "", "type": "uint256"}],
        "stateMutability": "view",
        "type": "function"
    },
    {
        "inputs": [
            {"name": "owner", "type": "address"},
            {"name": "spender", "type": "address"}
        ],
        "name": "allowance",
        "outputs": [{"name": "", "type": "uint256"}],
        "stateMutability": "view",
        "type": "function"
    },
    {
        "inputs": [
            {"name": "spender", "type": "address"},
            {"name": "amount", "type": "uint256"}
        ],
        "name": "approve",
        "outputs": [{"name": "", "type": "bool"}],
        "stateMutability": "nonpayable",
        "type": "function"
    }
]

contract = w3.eth.contract(address=w3.to_checksum_address(contract_address), abi=contract_abi)
usdc = w3.eth.contract(address=w3.to_checksum_address(usdc_address), abi=erc20_abi)

# Check USDC Balance
initial_balance = usdc.functions.balanceOf(my_address).call()
print(f"\nInitial USDC Balance: {initial_balance / 1_000_000:.2f} USDC")

# 2. Register Issuer Key if not trusted
print("\n[Step 2] Checking if issuer key is trusted...")
pk_iss_bytes = bytes.fromhex(pk_iss_hex.replace("0x", ""))
is_trusted = contract.functions.isIssuerKeyTrusted(pk_iss_bytes).call()
if not is_trusted:
    print("  Issuer key is not trusted. Registering now...")
    nonce_tx = w3.eth.get_transaction_count(my_address)
    tx = contract.functions.registerIssuerKey(pk_iss_bytes).build_transaction({
        'from': my_address,
        'nonce': nonce_tx,
        'gas': 1000000,
        'maxFeePerGas': w3.to_wei(0.1, 'gwei'),
        'maxPriorityFeePerGas': w3.to_wei(0.001, 'gwei'),
        'chainId': w3.eth.chain_id
    })
    signed_tx = w3.eth.account.sign_transaction(tx, private_key)
    tx_hash = w3.eth.send_raw_transaction(signed_tx.raw_transaction)
    print(f"  Sent registerIssuerKey transaction: {tx_hash.hex()}")
    receipt = w3.eth.wait_for_transaction_receipt(tx_hash)
    print(f"  Status: {receipt['status']}")
else:
    print("  Issuer key is already trusted.")

# 3. Check and approve allowance
print("\n[Step 3] Checking USDC allowance...")
allowance = usdc.functions.allowance(my_address, contract.address).call()
if allowance < 10_000_000:
    print(f"  Allowance is {allowance / 1_000_000:.2f} USDC. Approving 10 USDC...")
    nonce_tx = w3.eth.get_transaction_count(my_address)
    tx = usdc.functions.approve(contract.address, 10_000_000).build_transaction({
        'from': my_address,
        'nonce': nonce_tx,
        'gas': 200000,
        'maxFeePerGas': w3.to_wei(0.1, 'gwei'),
        'maxPriorityFeePerGas': w3.to_wei(0.001, 'gwei'),
        'chainId': w3.eth.chain_id
    })
    signed_tx = w3.eth.account.sign_transaction(tx, private_key)
    tx_hash = w3.eth.send_raw_transaction(signed_tx.raw_transaction)
    print(f"  Sent approve transaction: {tx_hash.hex()}")
    receipt = w3.eth.wait_for_transaction_receipt(tx_hash)
    print(f"  Status: {receipt['status']}")
else:
    print(f"  Allowance is sufficient: {allowance / 1_000_000:.2f} USDC")

# 4. Perform Deposit
print("\n[Step 4] Depositing 10 USDC to contract...")
com_k_bytes = bytes.fromhex(com_k_hex.replace("0x", ""))
nonce_tx = w3.eth.get_transaction_count(my_address)
tx = contract.functions.deposit(sid, com_k_bytes, 10_000_000).build_transaction({
    'from': my_address,
    'nonce': nonce_tx,
    'gas': 2000000,
    'maxFeePerGas': w3.to_wei(0.1, 'gwei'),
    'maxPriorityFeePerGas': w3.to_wei(0.001, 'gwei'),
    'chainId': w3.eth.chain_id
})
signed_tx = w3.eth.account.sign_transaction(tx, private_key)
tx_hash = w3.eth.send_raw_transaction(signed_tx.raw_transaction)
print(f"  Sent deposit transaction: {tx_hash.hex()}")
receipt = w3.eth.wait_for_transaction_receipt(tx_hash)
print(f"  Status: {receipt['status']}")
if receipt['status'] != 1:
    print("Error: Deposit transaction failed.")
    sys.exit(1)

# 5. Perform Reveal
print("\n[Step 5] Revealing masking key k...")
k_bytes = bytes.fromhex(k_hex.replace("0x", ""))
nonce_tx = w3.eth.get_transaction_count(my_address)
tx = contract.functions.revealMaskKey(sid, k_bytes, pk_iss_bytes, com_k_bytes).build_transaction({
    'from': my_address,
    'nonce': nonce_tx,
    'gas': 2000000,
    'maxFeePerGas': w3.to_wei(0.1, 'gwei'),
    'maxPriorityFeePerGas': w3.to_wei(0.001, 'gwei'),
    'chainId': w3.eth.chain_id
})
signed_tx = w3.eth.account.sign_transaction(tx, private_key)
tx_hash = w3.eth.send_raw_transaction(signed_tx.raw_transaction)
print(f"  Sent revealMaskKey transaction: {tx_hash.hex()}")
receipt = w3.eth.wait_for_transaction_receipt(tx_hash)
print(f"  Status: {receipt['status']}")
if receipt['status'] != 1:
    print("Error: Reveal transaction failed.")
    sys.exit(1)

# 6. Perform Spend
print("\n[Step 6] Spending 5 USDC...")
nullifier_bytes = bytes.fromhex(nullifier_hex.replace("0x", ""))
alpha_neg_bytes = bytes.fromhex(alpha_neg_hex.replace("0x", ""))
recipient_or_intent_hash_bytes = bytes.fromhex(data["recipient_or_intent_hash_hex"].replace("0x", ""))
expiry_val = int(data["expiry"])
nonce_bytes = bytes.fromhex(data["nonce_hex"])

nonce_tx = w3.eth.get_transaction_count(my_address)
tx = contract.functions.spend(
    nullifier_bytes,
    alpha_neg_bytes,
    pk_iss_bytes,
    my_address, # Recipient is our wallet address
    5_000_000,
    recipient_or_intent_hash_bytes,
    expiry_val,
    nonce_bytes
).build_transaction({
    'from': my_address,
    'nonce': nonce_tx,
    'gas': 3000000,
    'maxFeePerGas': w3.to_wei(0.1, 'gwei'),
    'maxPriorityFeePerGas': w3.to_wei(0.001, 'gwei'),
    'chainId': w3.eth.chain_id
})
signed_tx = w3.eth.account.sign_transaction(tx, private_key)
tx_hash = w3.eth.send_raw_transaction(signed_tx.raw_transaction)
print(f"  Sent spend transaction: {tx_hash.hex()}")
receipt = w3.eth.wait_for_transaction_receipt(tx_hash)
print(f"  Status: {receipt['status']}")
if receipt['status'] != 1:
    print("Error: Spend transaction failed.")
    sys.exit(1)

# Verify Final Balance
time.sleep(2)
final_balance = usdc.functions.balanceOf(my_address).call()
print(f"\nFinal USDC Balance: {final_balance / 1_000_000:.2f} USDC")

# Calculate Net Change
# Initial: balance
# Deposit: -10 USDC + 0.01 USDC fee paid to fee_recipient (which is us) = -9.99 USDC net change.
# Spend payout: +4.9925 USDC (recipient receives 5 USDC net, but spend fee 0.15% is deducted, which is 0.0075, paid to fee recipient (us)!)
# Wait, if we are depositor, fee_recipient, and recipient:
# Deposit fee (0.01) is transferred to us. Spend fee (0.0075) is transferred to us. Payout (4.9925) is transferred to us.
# So net change is -5.00 USDC! (Since 10 USDC collateral was locked, 5 USDC payout received, 5 USDC remains in the contract as outstanding liability).
expected_final = initial_balance - 5_000_000
diff = final_balance - expected_final
print(f"Balance Difference: {diff / 1_000_000:.4f} USDC (Expected change: -5.00 USDC)")
if abs(diff) < 1000: # Allow small rounding
    print("\nE2E ON-CHAIN DEPOSIT & SPEND TRANSACTION FLOW PASSED SUCCESSFULLY!")
else:
    print("\nE2E FLOW COMPLETED BUT BALANCE DELTA MISMATCHED.")
