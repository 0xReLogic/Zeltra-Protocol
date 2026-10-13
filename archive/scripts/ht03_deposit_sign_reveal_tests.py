#!/usr/bin/env python3
"""
HT-03: Atomic Deposit -> Sign -> Reveal On-Chain Tests
======================================================

Runs all deposit, threshold sign verification (MSM), and reveal test cases
against the deployed Nimbus contract on Arbitrum Sepolia.

For each case, verifies:
  1. Transaction status (success vs revert) or eth_call return values
  2. On-chain principal changes appropriately
  3. Proper error handling and error reasons

Test matrix:
  PT-01: Happy path deposit and reveal (revealMaskKey returns true)
  NT-01: Reveal with mismatched k (EIP-2537 MSM output mismatch)
  NT-02: Reveal with wrong pk_iss (EIP-2537 MSM output mismatch)
  NT-03: Reveal with wrong com_k (reverts with COMMITMENT_MISMATCH)
  NT-04: Reveal for non-existent sid (reverts with NO_DEPOSIT_FOUND)
  NT-05: Duplicate reveal (returns false since already resolved)
  NT-06: Reveal with invalid parameter length (reverts with INVALID_*_LENGTH)
  NT-07: Deposit with duplicate sid (reverts with SESSION_ALREADY_EXISTS)
  NT-08: Deposit with invalid commitment length (reverts with INVALID_COMMITMENT_LENGTH)
"""

import json
import os
import subprocess
import sys
import time
from datetime import datetime, timezone
from web3 import Web3
from eth_account import Account

# ─── Configuration ───────────────────────────────────────────────────────────

RPC_URL = os.environ.get("ARB_SEPOLIA_RPC", "https://arbitrum-sepolia.core.chainstack.com/d18e11a2327c1a17c030975e3e0c8e24")
CONTRACT_ADDRESS = os.environ.get("NIMBUS_CONTRACT_ADDRESS", "0x62ca774e20b76431d189e1635400b91b03c2b031")
USDC_ADDRESS = os.environ.get("USDC_ADDRESS", "0x75faf114eafb1bdbe2f0316df893fd58ce46aa4d")
PRIVATE_KEY = os.environ.get("NIMBUS_RELAYER_PRIVATE_KEY", os.environ.get("PRIVATE_KEY", ""))
CHAIN_ID = 421614
DEPOSIT_AMOUNT = 10_000_000  # 10 USDC

if not PRIVATE_KEY:
    print("ERROR: Set NIMBUS_RELAYER_PRIVATE_KEY or PRIVATE_KEY environment variable")
    sys.exit(1)

# ─── Web3 Setup ──────────────────────────────────────────────────────────────

w3 = Web3(Web3.HTTPProvider(RPC_URL))
if not w3.is_connected():
    print("ERROR: Cannot connect to RPC")
    sys.exit(1)

account = Account.from_key(PRIVATE_KEY)
MY_ADDRESS = account.address

CONTRACT_ABI = [
    {"inputs": [{"name": "pk_iss_bytes", "type": "bytes"}], "name": "isIssuerKeyTrusted", "outputs": [{"name": "", "type": "bool"}], "stateMutability": "view", "type": "function"},
    {"inputs": [{"name": "pk_iss_bytes", "type": "bytes"}], "name": "registerIssuerKey", "outputs": [], "stateMutability": "nonpayable", "type": "function"},
    {"inputs": [{"name": "sid", "type": "bytes32"}, {"name": "_com_k_bytes", "type": "bytes"}, {"name": "amount", "type": "uint256"}], "name": "deposit", "outputs": [], "stateMutability": "nonpayable", "type": "function"},
    {"inputs": [{"name": "sid", "type": "bytes32"}, {"name": "k_bytes", "type": "bytes"}, {"name": "pk_iss_bytes", "type": "bytes"}, {"name": "com_k_bytes", "type": "bytes"}], "name": "revealMaskKey", "outputs": [{"name": "", "type": "bool"}], "stateMutability": "nonpayable", "type": "function"},
    {"inputs": [], "name": "totalDepositedPrincipal", "outputs": [{"name": "", "type": "uint256"}], "stateMutability": "view", "type": "function"},
]

ERC20_ABI = [
    {"inputs": [{"name": "account", "type": "address"}], "name": "balanceOf", "outputs": [{"name": "", "type": "uint256"}], "stateMutability": "view", "type": "function"},
    {"inputs": [{"name": "owner", "type": "address"}, {"name": "spender", "type": "address"}], "name": "allowance", "outputs": [{"name": "", "type": "uint256"}], "stateMutability": "view", "type": "function"},
    {"inputs": [{"name": "spender", "type": "address"}, {"name": "amount", "type": "uint256"}], "name": "approve", "outputs": [{"name": "", "type": "bool"}], "stateMutability": "nonpayable", "type": "function"},
]

contract = w3.eth.contract(address=w3.to_checksum_address(CONTRACT_ADDRESS), abi=CONTRACT_ABI)
usdc = w3.eth.contract(address=w3.to_checksum_address(USDC_ADDRESS), abi=ERC20_ABI)

# ─── Helpers ─────────────────────────────────────────────────────────────────

results = []

def build_and_send_tx(fn, gas=3_000_000):
    """Build, sign, send transaction and return receipt."""
    nonce_tx = w3.eth.get_transaction_count(MY_ADDRESS)
    tx = fn.build_transaction({
        'from': MY_ADDRESS,
        'nonce': nonce_tx,
        'gas': gas,
        'maxFeePerGas': w3.to_wei(0.1, 'gwei'),
        'maxPriorityFeePerGas': w3.to_wei(0.001, 'gwei'),
        'chainId': CHAIN_ID
    })
    signed_tx = w3.eth.account.sign_transaction(tx, PRIVATE_KEY)
    tx_hash = w3.eth.send_raw_transaction(signed_tx.raw_transaction)
    receipt = w3.eth.wait_for_transaction_receipt(tx_hash, timeout=120)
    return receipt


def check_error(error_msg, expected_ascii):
    expected_hex = expected_ascii.encode('ascii').hex().lower()
    error_str = str(error_msg).lower()
    return (expected_ascii.lower() in error_str) or (expected_hex in error_str)


def try_eth_call(fn, from_address=MY_ADDRESS):
    """Try eth_call simulation. Returns (success: bool, result_or_error_msg)."""
    try:
        result = fn.call({'from': from_address})
        return True, result
    except Exception as e:
        return False, str(e)


def get_principal():
    return contract.functions.totalDepositedPrincipal().call()


def generate_bls_vectors(nonce_hex):
    cmd = [
        "cargo", "run", "-q", "-p", "nimbus-core", "--example", "generate_bls_test_data", "--",
        "--spend-contract",
        "--chain-id", str(CHAIN_ID),
        "--contract", CONTRACT_ADDRESS,
        "--amount", "5000000",
        "--recipient", MY_ADDRESS,
        "--nonce", nonce_hex,
    ]
    res = subprocess.run(cmd, capture_output=True, text=True, cwd="/home/azureuser/crypto")
    if res.returncode != 0:
        print(f"  ERROR: BLS generator failed: {res.stderr}")
        sys.exit(1)
    data = {}
    for line in res.stdout.strip().split("\n"):
        if ":" in line:
            k, v = line.split(":", 1)
            data[k.strip()] = v.strip()
    return data


def record_result(test_id, description, passed, tx_hash=None, details=None):
    status = "✅ PASS" if passed else "❌ FAIL"
    print(f"  {status}: {test_id} — {description}")
    if details:
        print(f"    Details: {details}")
    results.append({
        "test_id": test_id,
        "description": description,
        "passed": passed,
        "tx_hash": tx_hash,
        "details": details,
        "timestamp": datetime.now(timezone.utc).isoformat(),
    })


def register_issuer_if_untrusted(pk_iss_bytes):
    is_trusted = contract.functions.isIssuerKeyTrusted(pk_iss_bytes).call()
    if not is_trusted:
        print("  Issuer key is not trusted. Registering on-chain...")
        receipt = build_and_send_tx(contract.functions.registerIssuerKey(pk_iss_bytes))
        print(f"  Register issuer status: {receipt['status']}")
        if receipt['status'] != 1:
            print("  ERROR: Failed to register issuer key")
            sys.exit(1)


# ─── Main ────────────────────────────────────────────────────────────────────

def main():
    print("=" * 70)
    print("HT-03: ATOMIC DEPOSIT -> SIGN -> REVEAL ON-CHAIN TESTS")
    print("=" * 70)
    print(f"Contract:  {CONTRACT_ADDRESS}")
    print(f"Wallet:    {MY_ADDRESS}")
    print(f"Chain:     Arbitrum Sepolia ({CHAIN_ID})")
    print(f"Timestamp: {datetime.now(timezone.utc).isoformat()}")
    print()

    # Approve USDC if needed
    allowance = usdc.functions.allowance(MY_ADDRESS, contract.address).call()
    if allowance < DEPOSIT_AMOUNT * 10:
        print(f"Approving USDC...")
        receipt = build_and_send_tx(usdc.functions.approve(contract.address, DEPOSIT_AMOUNT * 50), gas=200_000)
        print(f"Approve status: {receipt['status']}")

    # ── PT-01: Happy Path Deposit & Reveal ───────────────────────────
    print("\n[PT-01] Testing happy path deposit & reveal...")
    sid_pt01 = os.urandom(32)
    nonce_pt01 = os.urandom(32).hex()
    bls_data = generate_bls_vectors(nonce_pt01)
    
    com_k_pt01 = bytes.fromhex(bls_data["com_k_hex"].replace("0x", ""))
    k_pt01 = bytes.fromhex(bls_data["k_hex"].replace("0x", ""))
    pk_iss_pt01 = bytes.fromhex(bls_data["pk_iss_hex"].replace("0x", ""))

    register_issuer_if_untrusted(pk_iss_pt01)

    principal_before = get_principal()

    print(f"  Depositing {DEPOSIT_AMOUNT / 1e6} USDC for session {sid_pt01.hex()}...")
    receipt_dep = build_and_send_tx(contract.functions.deposit(sid_pt01, com_k_pt01, DEPOSIT_AMOUNT))
    print(f"  Deposit status: {receipt_dep['status']}")
    if receipt_dep['status'] != 1:
        record_result("PT-01", "Happy path deposit", False, details="Deposit tx reverted")
        sys.exit(1)

    principal_after = get_principal()
    fee = (DEPOSIT_AMOUNT + 999) // 1000
    net_amount = DEPOSIT_AMOUNT - fee
    principal_expected = principal_before + net_amount

    if principal_after != principal_expected:
        record_result("PT-01", "Happy path deposit (principal change)", False, 
                      details=f"Expected principal {principal_expected}, got {principal_after}")
        sys.exit(1)

    # Simulate revealMaskKey call first to make sure it returns True
    success, call_res = try_eth_call(contract.functions.revealMaskKey(sid_pt01, k_pt01, pk_iss_pt01, com_k_pt01))
    if not success or not call_res:
        record_result("PT-01", "Happy path reveal simulation", False, 
                      details=f"Reveal simulation failed or returned False: {call_res}")
        sys.exit(1)

    print("  Sending revealMaskKey transaction...")
    receipt_rev = build_and_send_tx(contract.functions.revealMaskKey(sid_pt01, k_pt01, pk_iss_pt01, com_k_pt01))
    print(f"  Reveal status: {receipt_rev['status']}")
    
    # Verify that calling revealMaskKey again returns False (since it is already resolved)
    _, call_res_after = try_eth_call(contract.functions.revealMaskKey(sid_pt01, k_pt01, pk_iss_pt01, com_k_pt01))

    passed = (receipt_rev['status'] == 1) and (call_res_after == False)
    record_result("PT-01", "Happy path deposit and reveal", passed, 
                  tx_hash=receipt_rev['transactionHash'].hex(), 
                  details=f"Deposit & Reveal succeeded. Secondary reveal returned {call_res_after} as expected.")

    # ── NT-01: Reveal with mismatched k ──────────────────────────────
    print("\n[NT-01] Testing reveal with mismatched k...")
    sid_nt01 = os.urandom(32)
    nonce_nt01 = os.urandom(32).hex()
    bls_data = generate_bls_vectors(nonce_nt01)
    com_k_nt01 = bytes.fromhex(bls_data["com_k_hex"].replace("0x", ""))
    k_nt01 = bytes.fromhex(bls_data["k_hex"].replace("0x", ""))
    pk_iss_nt01 = bytes.fromhex(bls_data["pk_iss_hex"].replace("0x", ""))

    print(f"  Depositing for session {sid_nt01.hex()}...")
    build_and_send_tx(contract.functions.deposit(sid_nt01, com_k_nt01, DEPOSIT_AMOUNT))

    # Mismatch k by modifying last byte
    mismatched_k = bytearray(k_nt01)
    mismatched_k[-1] ^= 0x01
    mismatched_k = bytes(mismatched_k)

    # Should return false because EIP-2537 MSM won't match commitment com_k
    success, call_res = try_eth_call(contract.functions.revealMaskKey(sid_nt01, mismatched_k, pk_iss_nt01, com_k_nt01))
    passed = success and (call_res == False)
    record_result("NT-01", "Reveal with mismatched k (returns false)", passed,
                  details=f"Returned {call_res}. Expected False.")

    # ── NT-02: Reveal with wrong pk_iss ──────────────────────────────
    print("\n[NT-02] Testing reveal with wrong pk_iss...")
    # Change first byte of public key to something else
    wrong_pk = bytearray(pk_iss_nt01)
    wrong_pk[16] ^= 0x01  # mutate part of G2 point field representation
    wrong_pk = bytes(wrong_pk)

    success, call_res = try_eth_call(contract.functions.revealMaskKey(sid_nt01, k_nt01, wrong_pk, com_k_nt01))
    passed = (not success) and check_error(call_res, "MSM_PRECOMPILE_CALL_FAILED")
    record_result("NT-02", "Reveal with wrong pk_iss (returns false)", passed,
                  details=f"Returned {call_res}. Expected precompile failure.")

    # ── NT-03: Reveal with wrong com_k ───────────────────────────────
    print("\n[NT-03] Testing reveal with wrong com_k...")
    wrong_com = bytearray(com_k_nt01)
    wrong_com[16] ^= 0x01
    wrong_com = bytes(wrong_com)

    success, call_res = try_eth_call(contract.functions.revealMaskKey(sid_nt01, k_nt01, pk_iss_nt01, wrong_com))
    passed = (not success) and check_error(call_res, "COMMITMENT_MISMATCH")
    record_result("NT-03", "Reveal with wrong com_k (reverts)", passed,
                  details=f"Result: {call_res}")

    # ── NT-04: Reveal for non-existent sid ───────────────────────────
    print("\n[NT-04] Testing reveal for non-existent sid...")
    random_sid = os.urandom(32)
    success, call_res = try_eth_call(contract.functions.revealMaskKey(random_sid, k_nt01, pk_iss_nt01, com_k_nt01))
    passed = (not success) and check_error(call_res, "NO_DEPOSIT_FOUND")
    record_result("NT-04", "Reveal for non-existent sid (reverts)", passed,
                  details=f"Result: {call_res}")

    # ── NT-05: Duplicate reveal ──────────────────────────────────────
    print("\n[NT-05] Testing duplicate reveal on resolved session...")
    # Using PT-01 resolved session
    success, call_res = try_eth_call(contract.functions.revealMaskKey(sid_pt01, k_pt01, pk_iss_pt01, com_k_pt01))
    passed = success and (call_res == False)
    record_result("NT-05", "Duplicate reveal (returns false)", passed,
                  details=f"Result: {call_res}")

    # ── NT-06: Reveal with invalid parameter length ──────────────────
    print("\n[NT-06] Testing parameter lengths...")
    short_k = k_nt01[:-1]
    success_k, call_res_k = try_eth_call(contract.functions.revealMaskKey(sid_nt01, short_k, pk_iss_nt01, com_k_nt01))
    passed_k = (not success_k) and check_error(call_res_k, "INVALID_SCALAR_LENGTH")
    record_result("NT-06a", "Reveal with short k (reverts)", passed_k, details=f"Result: {call_res_k}")

    short_pk = pk_iss_nt01[:-1]
    success_pk, call_res_pk = try_eth_call(contract.functions.revealMaskKey(sid_nt01, k_nt01, short_pk, com_k_nt01))
    passed_pk = (not success_pk) and check_error(call_res_pk, "INVALID_PUBLIC_KEY_LENGTH")
    record_result("NT-06b", "Reveal with short pk_iss (reverts)", passed_pk, details=f"Result: {call_res_pk}")

    short_com = com_k_nt01[:-1]
    success_com, call_res_com = try_eth_call(contract.functions.revealMaskKey(sid_nt01, k_nt01, pk_iss_nt01, short_com))
    passed_com = (not success_com) and check_error(call_res_com, "INVALID_COMMITMENT_LENGTH")
    record_result("NT-06c", "Reveal with short com_k (reverts)", passed_com, details=f"Result: {call_res_com}")

    # ── NT-07: Deposit with duplicate sid ────────────────────────────
    print("\n[NT-07] Testing deposit with duplicate sid...")
    # Deposit to already existing sid_pt01
    success, call_res = try_eth_call(contract.functions.deposit(sid_pt01, com_k_pt01, DEPOSIT_AMOUNT))
    passed = (not success) and check_error(call_res, "SESSION_ALREADY_EXISTS")
    record_result("NT-07", "Deposit with duplicate sid (reverts)", passed, details=f"Result: {call_res}")

    # ── NT-08: Deposit with invalid commitment length ────────────────
    print("\n[NT-08] Testing deposit with invalid commitment length...")
    success, call_res = try_eth_call(contract.functions.deposit(os.urandom(32), short_com, DEPOSIT_AMOUNT))
    passed = (not success) and check_error(call_res, "INVALID_COMMITMENT_LENGTH")
    record_result("NT-08", "Deposit with invalid commitment length (reverts)", passed, details=f"Result: {call_res}")

    # ── Summary ──────────────────────────────────────────────────────
    print()
    print("=" * 70)
    print("RESULTS SUMMARY")
    print("=" * 70)

    passed = sum(1 for r in results if r["passed"])
    failed = sum(1 for r in results if not r["passed"])
    total = len(results)

    for r in results:
        status = "✅" if r["passed"] else "❌"
        tx_info = f" tx:{r['tx_hash'][:16]}..." if r.get("tx_hash") else ""
        print(f"  {status} {r['test_id']:8s} {r['description'][:55]}{tx_info}")

    print()
    print(f"  Total: {total}  |  Passed: {passed}  |  Failed: {failed}")

    # Save report
    report = {
        "test_suite": "HT-03 Atomic Deposit -> Sign -> Reveal On-Chain Tests",
        "contract": CONTRACT_ADDRESS,
        "chain_id": CHAIN_ID,
        "wallet": MY_ADDRESS,
        "timestamp": datetime.now(timezone.utc).isoformat(),
        "results": results,
        "summary": {
            "total": total,
            "passed": passed,
            "failed": failed,
        }
    }

    report_path = "/home/azureuser/crypto/test-reports/ht03_deposit_sign_reveal_tests.json"
    os.makedirs(os.path.dirname(report_path), exist_ok=True)
    with open(report_path, "w") as f:
        json.dump(report, f, indent=2)
    print(f"\n  Report saved: {report_path}")

    sys.exit(0 if failed == 0 else 1)


if __name__ == "__main__":
    main()
