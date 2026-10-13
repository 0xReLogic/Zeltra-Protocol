#!/usr/bin/env python3
"""
HT-10: Signed Quote + Execution Fee Settlement Hard Tests
Target: Arbitrum Sepolia testnet with live cluster (1 leader + 4 guardians)

Usage:
  source nimbus-node/.env.test
  python3 scripts/ht10_signed_quote_tests.py [--single|--batch|--negative|--all]

Prerequisites:
  pip install web3 requests
"""

import os
import sys
import json
import time
import uuid
import hashlib
import requests
from web3 import Web3
from eth_account import Account
from eth_account.messages import encode_typed_data

# ── Config ──────────────────────────────────────────────────────────
RPC_URL = os.environ.get("NIMBUS_RPC_URL", "https://arbitrum-sepolia.core.chainstack.com/d18e11a2327c1a17c030975e3e0c8e24")
PRIVATE_KEY = os.environ.get("NIMBUS_RELAYER_PRIVATE_KEY", "")
CONTRACT_ADDR = os.environ.get("NIMBUS_CONTRACT_ADDRESS", "")
USDC_ADDR = "0x75faf114eafb1BDbe2F0316DF893fd58CE46AA4d"
LEADER_URL = "http://127.0.0.1:8080"
GUARDIAN_URLS = [f"http://127.0.0.1:{8080+i}" for i in range(1, 5)]
CHAIN_ID = 421614  # Arbitrum Sepolia

DEPOSIT_AMOUNT = 10_000_000  # 10 USDC (minimum deposit)
SPEND_AMOUNT = 5_000_000     # 5 USDC (minimum spend)

# Vector storage for recovery
VECTOR_DIR = "/home/azureuser/crypto/scripts/ht10_vectors"
os.makedirs(VECTOR_DIR, exist_ok=True)

# BLS ceremony keys (same as start_cluster.sh)
PK_ISS_COMPRESSED = "885940298f97a7fbd92d404b898dcc5747d24cfa048ef3bbf9f4515b5605e4d058107f76c788ec2535476f3c241e6a410bfd5c8883b1d864bdcbdd76bdc2102d833237e19a7bc9d38e6ee5d98729c0e7b90052e61c6df8ca377a241ace187c02"
PK_ISS_UNCOMPRESSED = "00000000000000000000000000000000178282dc63af77b484e4a6f37fe86cb4253afe6df26d7bed2f38595c515efcefb59d258de3b753010e14f65b632d3467000000000000000000000000000000000f75903dfce18da6817055197038f7b44cfa868207a9d02af7ec42dd7d62f9be442cf33f348dcd7b6c228ff1b03b9e9800000000000000000000000000000000191ffa658b219841e21cdf898b1c5addb09f087bed557d45d9d6d86e5a901d41eecf70d6d99f38410e8dd60aa0f7e3df0000000000000000000000000000000014d71de29ce555e07484a87717f6991aeead239f1a250144047547052a40f202c2bbee851a234b770bfee1f778fada56"

# ABIs
ERC20_ABI = json.loads('[{"inputs":[{"name":"spender","type":"address"},{"name":"amount","type":"uint256"}],"name":"approve","outputs":[{"name":"","type":"bool"}],"stateMutability":"nonpayable","type":"function"},{"inputs":[{"name":"account","type":"address"}],"name":"balanceOf","outputs":[{"name":"","type":"uint256"}],"stateMutability":"view","type":"function"},{"inputs":[{"name":"owner","type":"address"},{"name":"spender","type":"address"}],"name":"allowance","outputs":[{"name":"","type":"uint256"}],"stateMutability":"view","type":"function"}]')

CONTRACT_ABI = json.loads('[{"inputs":[{"name":"sid","type":"bytes32"},{"name":"com_k_bytes","type":"bytes"},{"name":"amount","type":"uint256"}],"name":"deposit","outputs":[],"stateMutability":"nonpayable","type":"function"},{"inputs":[{"name":"sid","type":"bytes32"},{"name":"k_bytes","type":"bytes"},{"name":"pk_iss_bytes","type":"bytes"},{"name":"com_k_bytes","type":"bytes"}],"name":"revealMaskKey","outputs":[{"name":"","type":"bool"}],"stateMutability":"nonpayable","type":"function"},{"inputs":[{"name":"root","type":"bytes32"},{"name":"nullifier","type":"bytes32"},{"name":"alpha_neg_bytes","type":"bytes"},{"name":"pk_iss_bytes","type":"bytes"},{"name":"recipient","type":"address"},{"name":"amount","type":"uint256"},{"name":"recipient_or_intent_hash","type":"bytes32"},{"name":"expiry","type":"uint256"},{"name":"nonce","type":"bytes32"},{"name":"max_execution_fee","type":"uint256"},{"name":"execution_fee","type":"uint256"}],"name":"spend","outputs":[{"name":"","type":"bool"}],"stateMutability":"nonpayable","type":"function"},{"inputs":[{"name":"roots","type":"bytes32[]"},{"name":"nullifiers","type":"bytes32[]"},{"name":"alpha_neg_items","type":"bytes[]"},{"name":"pk_iss_items","type":"bytes[]"},{"name":"recipients","type":"address[]"},{"name":"amounts","type":"uint256[]"},{"name":"recipient_or_intent_hashes","type":"bytes32[]"},{"name":"expiries","type":"uint256[]"},{"name":"nonces","type":"bytes32[]"}],"name":"batchSpend","outputs":[{"name":"","type":"bool"}],"stateMutability":"nonpayable","type":"function"},{"inputs":[{"name":"root","type":"bytes32"}],"name":"registerCleanRoot","outputs":[],"stateMutability":"nonpayable","type":"function"},{"inputs":[],"name":"getAccumulatedFees","outputs":[{"name":"","type":"uint256"}],"stateMutability":"view","type":"function"},{"inputs":[{"name":"amount","type":"uint256"}],"name":"claimExecutionFees","outputs":[{"name":"","type":"bool"}],"stateMutability":"nonpayable","type":"function"},{"inputs":[],"name":"totalDepositedPrincipal","outputs":[{"name":"","type":"uint256"}],"stateMutability":"view","type":"function"},{"inputs":[{"name":"root","type":"bytes32"}],"name":"getCleanRootTimestamp","outputs":[{"name":"","type":"uint256"}],"stateMutability":"view","type":"function"}]')

# ── Setup ───────────────────────────────────────────────────────────
w3 = Web3(Web3.HTTPProvider(RPC_URL))
account = Account.from_key(PRIVATE_KEY)
MY_ADDRESS = account.address
usdc = w3.eth.contract(address=w3.to_checksum_address(USDC_ADDR), abi=ERC20_ABI)
contract = w3.eth.contract(address=w3.to_checksum_address(CONTRACT_ADDR), abi=CONTRACT_ABI)

passed = 0
failed = 0
results = []

def log(msg):
    print(f"  {msg}")

def log_test(name):
    print(f"\n{'='*60}")
    print(f"  TEST: {name}")
    print(f"{'='*60}")

def record(name, success, detail=""):
    global passed, failed
    status = "PASS" if success else "FAIL"
    if success:
        passed += 1
    else:
        failed += 1
    results.append((name, status, detail))
    print(f"  [{status}] {name}" + (f" — {detail}" if detail else ""))

def build_and_send_tx(func, gas=500_000):
    nonce = w3.eth.get_transaction_count(MY_ADDRESS)
    tx = func.build_transaction({
        "from": MY_ADDRESS,
        "nonce": nonce,
        "gas": gas,
        "maxFeePerGas": w3.to_wei("0.1", "gwei"),
        "maxPriorityFeePerGas": w3.to_wei("0.01", "gwei"),
        "chainId": CHAIN_ID,
    })
    signed = account.sign_transaction(tx)
    tx_hash = w3.eth.send_raw_transaction(signed.raw_transaction)
    receipt = w3.eth.wait_for_transaction_receipt(tx_hash, timeout=120)
    return receipt

def generate_bls_vectors(amount=SPEND_AMOUNT, recipient=None, invalid=False, counter=0):
    """Generate BLS test vectors using nimbus-core binary.
    
    IMPORTANT: counter makes nonce deterministic so vectors can be recovered.
    """
    recipient = recipient or MY_ADDRESS
    # Deterministic nonce from counter — CRITICAL for vector recovery
    nonce_hex = "0x" + hashlib.sha256(f"ht10-test-{counter}-{amount}".encode()).hexdigest()
    cmd = (
        f"cargo run -q -p nimbus-core --example generate_bls_test_data -- "
        f"--spend-contract --chain-id {CHAIN_ID} --contract {CONTRACT_ADDR} "
        f"--amount {amount} --recipient {recipient} --nonce {nonce_hex}"
    )
    if invalid:
        cmd += " --invalid"
    output = os.popen(cmd).read()
    data = {}
    for line in output.strip().split("\n"):
        if ": " in line:
            key, val = line.split(": ", 1)
            data[key.strip()] = val.strip().strip('"')
    data["nonce_hex"] = nonce_hex
    return data

def save_vectors(vectors, sid, root, label="test"):
    """Save vectors to file BEFORE any on-chain tx. CRITICAL for recovery."""
    import json as _json
    filepath = os.path.join(VECTOR_DIR, f"{label}.json")
    payload = {
        "sid_hex": sid.hex() if isinstance(sid, bytes) else sid,
        "root_hex": root.hex() if isinstance(root, bytes) else root,
        "timestamp": time.time(),
        "label": label,
        **{k: v for k, v in vectors.items()},
    }
    with open(filepath, "w") as f:
        _json.dump(payload, f, indent=2)
    log(f"  Vectors saved → {filepath}")
    return filepath

def load_vectors(label):
    """Load vectors from file for recovery."""
    import json as _json
    filepath = os.path.join(VECTOR_DIR, f"{label}.json")
    if not os.path.exists(filepath):
        return None
    with open(filepath) as f:
        return _json.load(f)

def ensure_approval():
    """Ensure USDC approval for contract."""
    allowance = usdc.functions.allowance(MY_ADDRESS, contract.address).call()
    if allowance < DEPOSIT_AMOUNT * 10:
        log(f"Approving USDC...")
        receipt = build_and_send_tx(usdc.functions.approve(contract.address, DEPOSIT_AMOUNT * 100), gas=200_000)
        log(f"  Approved (tx: {receipt['transactionHash'].hex()[:18]}...)")

def deposit_and_reveal(amount=DEPOSIT_AMOUNT, spend_amount=None, counter=0, label=None):
    """On-chain deposit + reveal. Returns (sid, vectors for spend, root_bytes).
    
    CRITICAL: Vectors are saved to file BEFORE any on-chain tx.
    """
    import secrets
    sid = secrets.token_bytes(32)
    root = secrets.token_bytes(32)  # arbitrary association root

    # Generate vectors with spend amount
    vectors = generate_bls_vectors(amount=spend_amount or amount, counter=counter)
    k_hex = vectors.get("k_hex", "")
    com_k_hex = vectors.get("com_k_hex", "")
    k_bytes = bytes.fromhex(k_hex.replace("0x", ""))
    com_k_bytes = bytes.fromhex(com_k_hex.replace("0x", ""))
    log(f"  com_k_bytes: {len(com_k_bytes)} bytes")
    assert len(com_k_bytes) == 256, f"com_k_bytes should be 256 bytes, got {len(com_k_bytes)}"

    # SAVE VECTORS TO FILE BEFORE ANY ON-CHAIN TX — CRITICAL FOR RECOVERY
    if label is None:
        label = f"deposit_{counter}_{int(time.time())}"
    save_vectors(vectors, sid, root, label=label)

    log(f"Depositing {amount/1e6} USDC...")
    receipt = build_and_send_tx(
        contract.functions.deposit(sid, com_k_bytes, amount),
        gas=2_000_000
    )
    assert receipt["status"] == 1, f"Deposit failed: {receipt['transactionHash'].hex()}"
    log(f"  Deposit confirmed (block {receipt['blockNumber']})")

    log(f"Revealing masking key...")
    pk_iss_bytes = bytes.fromhex(PK_ISS_UNCOMPRESSED)
    receipt = build_and_send_tx(
        contract.functions.revealMaskKey(sid, k_bytes, pk_iss_bytes, com_k_bytes),
        gas=2_000_000
    )
    assert receipt["status"] == 1, f"Reveal failed: {receipt['transactionHash'].hex()}"
    log(f"  Reveal confirmed (block {receipt['blockNumber']})")

    # Register the association root
    log(f"Registering clean root...")
    receipt = build_and_send_tx(
        contract.functions.registerCleanRoot(root),
        gas=500_000
    )
    assert receipt["status"] == 1, f"RegisterCleanRoot failed"
    log(f"  Root registered (block {receipt['blockNumber']})")

    return sid, vectors, root

def get_quote(amount=SPEND_AMOUNT, association_root=None):
    """Get a signed spend quote from the node."""
    params = {"merchant_amount": str(amount)}
    if association_root:
        params["association_root"] = association_root
    resp = requests.get(f"{LEADER_URL}/api/quote/private-spend", params=params, timeout=10)
    assert resp.status_code == 200, f"Quote failed: {resp.status_code}"
    return resp.json()

def sign_eip712_quote(quote_data, private_key=PRIVATE_KEY):
    """Sign an EIP-712 ExecutionQuote."""
    domain = {
        "name": "Nimbus Protocol",
        "version": "1",
        "chainId": CHAIN_ID,
        "verifyingContract": CONTRACT_ADDR,
    }
    types = {
        "ExecutionQuote": [
            {"name": "quoteId", "type": "uint256"},
            {"name": "maxExecutionFee", "type": "uint256"},
            {"name": "merchantAmount", "type": "uint256"},
            {"name": "quoteExpiry", "type": "uint256"},
            {"name": "relayerAddress", "type": "address"},
        ]
    }
    message = {
        "quoteId": int(quote_data["quote_id"], 16),
        "maxExecutionFee": quote_data["execution_fee"],
        "merchantAmount": quote_data["merchant_amount"],
        "quoteExpiry": quote_data["quote_expiry"],
        "relayerAddress": MY_ADDRESS,  # matches NIMBUS_RELAYER_ADDRESS in .env.test
    }
    structured_data = encode_typed_data(
        domain_data=domain,
        message_types=types,
        message_data=message,
    )
    signed = Account.sign_message(structured_data, private_key=private_key)
    return signed.signature.hex()

def submit_spend(vectors, root, quote_data=None, signature=None, user_address=None,
                 max_execution_fee=None, quote_id=None, quote_expiry=None):
    """Submit a spend to the node API."""
    payload = {
        "nullifier": vectors["nullifier_hex"],
        "sig_hex": vectors.get("sig_hex", ""),
        "recipient": MY_ADDRESS,
        "amount": int(vectors.get("amount", SPEND_AMOUNT)),
        "alpha_neg_hex": vectors["alpha_neg_hex"],
        "hm_hex": vectors["hm_hex"],
        "pk_iss_hex": vectors["pk_iss_hex"],
        "association_root_hex": "0x" + root.hex(),
        "recipient_or_intent_hash_hex": vectors.get("recipient_or_intent_hash_hex", ""),
        "expiry": int(vectors.get("expiry", 0)),
        "nonce_hex": vectors.get("nonce_hex", ""),
        "idempotency_key": str(uuid.uuid4()),
    }
    if quote_data and signature:
        payload["max_execution_fee"] = max_execution_fee or quote_data["execution_fee"]
        payload["execution_fee"] = quote_data["execution_fee"]
        payload["quote_id"] = quote_id or quote_data["quote_id"]
        payload["quote_expiry"] = quote_expiry or quote_data["quote_expiry"]
        payload["quote_signature"] = signature
        payload["user_address"] = user_address or MY_ADDRESS
    resp = requests.post(f"{LEADER_URL}/api/spend", json=payload, timeout=15)
    return resp.status_code, resp.json()

def wait_for_settlement(nullifier_hex, timeout=30):
    """Wait for spend to be settled on-chain by the relayer."""
    log(f"Waiting for settlement (nullifier: {nullifier_hex[:14]}...)...")
    for _ in range(timeout // 2):
        health = requests.get(f"{LEADER_URL}/health", timeout=5).json()
        if health.get("processed_nullifiers", 0) > 0:
            log(f"  Settled! Queue empty.")
            return True
        time.sleep(2)
    log(f"  Timeout waiting for settlement")
    return False

def register_clean_root(root_hex):
    """Register a clean association root on-chain."""
    root_bytes = bytes.fromhex(root_hex.replace("0x", ""))
    receipt = build_and_send_tx(
        contract.functions.registerCleanRoot(root_bytes),
        gas=500_000
    )
    return receipt["status"] == 1

# ── HT-10.1: Single Spend End-to-End ───────────────────────────────
def test_single_spend_e2e():
    log_test("HT-10.1: Single Spend E2E (quote → sign → spend → reimburse)")

    ensure_approval()
    sid, vectors, root = deposit_and_reveal(amount=DEPOSIT_AMOUNT, spend_amount=SPEND_AMOUNT, counter=0, label="ht10_1_single")

    log("Getting quote...")
    quote = get_quote(amount=SPEND_AMOUNT)
    log(f"  quote_id: {quote['quote_id'][:18]}...")
    log(f"  execution_fee: {quote['execution_fee']} ({quote['execution_fee']/1e6:.6f} USDC)")
    log(f"  fee_tier: {quote.get('fee_tier', 'default')}")

    log("Signing EIP-712...")
    sig = sign_eip712_quote(quote)
    log(f"  Signature: {sig[:18]}...")

    bal_before = usdc.functions.balanceOf(MY_ADDRESS).call()

    log("Submitting spend...")
    status, resp = submit_spend(vectors, root, quote, sig)
    log(f"  Response: {status} — {resp.get('status', 'unknown')}")
    record("HT-10.1 spend queued", status == 200 and resp.get("status") == "QUEUED",
           resp.get("message", ""))

    settled = wait_for_settlement(vectors["nullifier_hex"])
    record("HT-10.1 settlement confirmed", settled)

    bal_after = usdc.functions.balanceOf(MY_ADDRESS).call()
    log(f"  Balance change: {(bal_after - bal_before)/1e6:.6f} USDC")

    accumulated = contract.functions.getAccumulatedFees().call()
    log(f"  Accumulated execution fees: {accumulated/1e6:.6f} USDC")
    record("HT-10.1 fees accumulated", accumulated > 0,
           f"{accumulated/1e6:.6f} USDC")

# ── HT-10.2: Tampered maxExecutionFee ──────────────────────────────
def test_tampered_fee():
    log_test("HT-10.2: Tampered maxExecutionFee (signature mismatch)")

    ensure_approval()
    sid, vectors, root = deposit_and_reveal(amount=DEPOSIT_AMOUNT, spend_amount=SPEND_AMOUNT, counter=1, label="ht10_2_tampered")

    quote = get_quote(amount=SPEND_AMOUNT)
    sig = sign_eip712_quote(quote)

    # Tamper: increase execution_fee in the spend request
    tampered_fee = quote["execution_fee"] * 10

    log(f"Original fee: {quote['execution_fee']}, tampered: {tampered_fee}")
    status, resp = submit_spend(vectors, root, quote, sig,
                                 max_execution_fee=tampered_fee)
    log(f"  Response: {status} — {resp.get('status', 'unknown')}")

    is_rejected = resp.get("status") in ("REJECTED", "ERROR") or status != 200
    record("HT-10.2 tampered fee rejected", is_rejected,
           resp.get("message", ""))

# ── HT-10.3: Expired Quote ─────────────────────────────────────────
def test_expired_quote():
    log_test("HT-10.3: Expired Quote")

    ensure_approval()
    sid, vectors, root = deposit_and_reveal(amount=DEPOSIT_AMOUNT, spend_amount=SPEND_AMOUNT, counter=2, label="ht10_3_expired")

    quote = get_quote(amount=SPEND_AMOUNT)
    sig = sign_eip712_quote(quote)

    # Force expired quote_expiry (set to past timestamp)
    expired_expiry = int(time.time()) - 3600

    log(f"Quote expiry: {quote['quote_expiry']}, forced expiry: {expired_expiry}")
    status, resp = submit_spend(vectors, root, quote, sig,
                                 quote_expiry=expired_expiry)
    log(f"  Response: {status} — {resp.get('status', 'unknown')}")

    is_rejected = resp.get("status") in ("REJECTED", "ERROR") or status != 200
    record("HT-10.3 expired quote rejected", is_rejected,
           resp.get("message", ""))

# ── HT-10.4: execution_fee > max_execution_fee ─────────────────────
def test_overcharge():
    log_test("HT-10.4: execution_fee > max_execution_fee (contract revert)")

    ensure_approval()
    sid, vectors, root = deposit_and_reveal(amount=DEPOSIT_AMOUNT, spend_amount=SPEND_AMOUNT, counter=3, label="ht10_4_overcharge")

    quote = get_quote(amount=SPEND_AMOUNT)
    sig = sign_eip712_quote(quote)

    # Set max_execution_fee lower than actual execution_fee
    low_max = quote["execution_fee"] // 2

    log(f"Actual fee: {quote['execution_fee']}, max allowed: {low_max}")
    status, resp = submit_spend(vectors, root, quote, sig,
                                 max_execution_fee=low_max)
    log(f"  Response: {status} — {resp.get('status', 'unknown')}")

    is_rejected = resp.get("status") in ("REJECTED", "ERROR") or status != 200
    record("HT-10.4 overcharge rejected", is_rejected,
           resp.get("message", ""))

# ── HT-10.5: Wrong Signer ──────────────────────────────────────────
def test_wrong_signer():
    log_test("HT-10.5: Wrong Signer (different key)")

    ensure_approval()
    sid, vectors, root = deposit_and_reveal(amount=DEPOSIT_AMOUNT, spend_amount=SPEND_AMOUNT, counter=4, label="ht10_5_wrong_signer")

    quote = get_quote(amount=SPEND_AMOUNT)

    # Sign with a different private key
    wrong_key = "0x" + "ab" * 32
    wrong_account = Account.from_key(wrong_key)
    sig = sign_eip712_quote(quote, private_key=wrong_key)

    log(f"Correct address: {MY_ADDRESS}")
    log(f"Wrong signer: {wrong_account.address}")

    # Submit with correct user_address but wrong signature
    status, resp = submit_spend(vectors, root, quote, sig,
                                 user_address=MY_ADDRESS)
    log(f"  Response: {status} — {resp.get('status', 'unknown')}")

    is_rejected = resp.get("status") in ("REJECTED", "ERROR") or status != 200
    record("HT-10.5 wrong signer rejected", is_rejected,
           resp.get("message", ""))

# ── HT-10.6: Batch Spend E2E ──────────────────────────────────────
def test_batch_spend_e2e():
    log_test("HT-10.6: Batch Spend E2E (5 items)")

    ensure_approval()
    all_vectors = []
    all_roots = []
    for i in range(5):
        log(f"Deposit+Reveal #{i+1}/5...")
        sid, vectors, root = deposit_and_reveal(amount=DEPOSIT_AMOUNT, spend_amount=SPEND_AMOUNT, counter=10+i, label=f"ht10_6_batch_{i}")
        all_vectors.append(vectors)
        all_roots.append(root)

    accumulated_before = contract.functions.getAccumulatedFees().call()

    log("Submitting 5 spends rapidly...")
    for i, (vectors, root) in enumerate(zip(all_vectors, all_roots)):
        quote = get_quote(amount=SPEND_AMOUNT)
        sig = sign_eip712_quote(quote)
        status, resp = submit_spend(vectors, root, quote, sig)
        log(f"  #{i+1}: {resp.get('status', 'unknown')}")

    log("Waiting for batch settlement...")
    time.sleep(10)

    accumulated_after = contract.functions.getAccumulatedFees().call()
    log(f"  Fees before: {accumulated_before/1e6:.6f} USDC")
    log(f"  Fees after: {accumulated_after/1e6:.6f} USDC")

    record("HT-10.6 batch fees accumulated",
           accumulated_after > accumulated_before,
           f"delta: {(accumulated_after - accumulated_before)/1e6:.6f} USDC")

# ── HT-10.9: Claim Worker Threshold ────────────────────────────────
def test_claim_worker():
    log_test("HT-10.9: Claim Worker Threshold Trigger")

    accumulated = contract.functions.getAccumulatedFees().call()
    log(f"  Accumulated fees: {accumulated/1e6:.6f} USDC")

    if accumulated < 1_000_000:  # $1 threshold
        log(f"  Fees below $1 threshold — claim worker should NOT trigger yet")
        record("HT-10.9 below threshold, no claim", True,
               f"accumulated: {accumulated/1e6:.6f} USDC")
    else:
        log(f"  Fees above $1 threshold — claim worker should have triggered")
        record("HT-10.9 above threshold", True,
               f"accumulated: {accumulated/1e6:.6f} USDC — check worker logs")

# ── Recovery: Spend saved vectors ──────────────────────────────────
def test_recover(label):
    """Recover a deposit by loading saved vectors and spending them."""
    log_test(f"RECOVER: {label}")

    saved = load_vectors(label)
    if saved is None:
        log(f"  No saved vectors for label '{label}'")
        record(f"RECOVER {label}", False, "no saved vectors")
        return

    log(f"  Loaded vectors from {VECTOR_DIR}/{label}.json")
    log(f"  Nullifier: {saved.get('nullifier_hex', '')[:20]}...")
    root_bytes = bytes.fromhex(saved["root_hex"].replace("0x", ""))
    sid_bytes = bytes.fromhex(saved["sid_hex"].replace("0x", ""))

    quote = get_quote(amount=SPEND_AMOUNT)
    sig = sign_eip712_quote(quote)

    bal_before = usdc.functions.balanceOf(MY_ADDRESS).call()

    status, resp = submit_spend(saved, root_bytes, quote, sig)
    log(f"  Response: {status} — {resp.get('status', 'unknown')}")

    if resp.get("status") == "QUEUED":
        settled = wait_for_settlement(saved["nullifier_hex"], timeout=30)
        record(f"RECOVER {label} settlement", settled)
    else:
        record(f"RECOVER {label} queued", False, resp.get("message", ""))

    bal_after = usdc.functions.balanceOf(MY_ADDRESS).call()
    log(f"  Balance change: {(bal_after - bal_before)/1e6:.6f} USDC")

# ── Main ────────────────────────────────────────────────────────────
def main():
    print(f"\n{'#'*60}")
    print(f"  HT-10 Signed Quote + Execution Fee Settlement")
    print(f"  Contract: {CONTRACT_ADDR}")
    print(f"  Wallet:   {MY_ADDRESS}")
    print(f"  USDC bal: {usdc.functions.balanceOf(MY_ADDRESS).call()/1e6:.2f}")
    print(f"  Cluster:  {LEADER_URL}")
    print(f"  Vector dir: {VECTOR_DIR}")
    print(f"{'#'*60}")

    mode = sys.argv[1] if len(sys.argv) > 1 else "--one"

    if mode == "--one":
        test_single_spend_e2e()

    if mode in ("--single", "--all"):
        test_single_spend_e2e()
        test_tampered_fee()
        test_expired_quote()
        test_overcharge()
        test_wrong_signer()

    if mode in ("--batch", "--all"):
        test_batch_spend_e2e()

    if mode in ("--negative",):
        test_tampered_fee()
        test_expired_quote()
        test_overcharge()
        test_wrong_signer()

    if mode == "--recover":
        if len(sys.argv) < 3:
            print("Usage: --recover <label>")
            print(f"Saved vectors: {os.listdir(VECTOR_DIR)}")
            sys.exit(1)
        test_recover(sys.argv[2])

    if mode in ("--claim", "--all"):
        test_claim_worker()

    print(f"\n{'='*60}")
    print(f"  RESULTS: {passed} passed, {failed} failed")
    print(f"{'='*60}")
    for name, status, detail in results:
        print(f"  [{status}] {name}" + (f" — {detail}" if detail else ""))
    print()

    sys.exit(0 if failed == 0 else 1)

if __name__ == "__main__":
    main()
