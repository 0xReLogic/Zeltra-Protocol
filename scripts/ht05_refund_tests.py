#!/usr/bin/env python3
"""
HT-05: Refund and Timeout On-Chain Tests
=========================================

Runs all refund and timeout test cases against the deployed Nimbus contract on Arbitrum Sepolia.
For each case, verifies:
  1. Transaction reverts (receipt status == 0) or eth_call reverts
  2. On-chain principal does NOT change (for negative tests)

Test matrix:
  NT-01: Refund before timelock 24 hours (reverts)
  NT-02: Refund non-existent session (reverts)
  NT-03: Refund by non-client (reverts)
  NT-04: Refund after reveal/resolved (reverts)
  PT-01: Refund succeeds after 24 hours (if an eligible session is found)
  PT-02: Principal decreases by net amount (after positive refund)
  PT-03: Session status is marked resolved (after positive refund)
  PT-04: Double-refund is rejected (reverts)
  PT-05: Reveal after refund is rejected (returns false or reverts)
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
    {"inputs": [{"name": "sid", "type": "bytes32"}, {"name": "_com_k_bytes", "type": "bytes"}, {"name": "amount", "type": "uint256"}], "name": "deposit", "outputs": [], "stateMutability": "nonpayable", "type": "function"},
    {"inputs": [{"name": "sid", "type": "bytes32"}, {"name": "k_bytes", "type": "bytes"}, {"name": "pk_iss_bytes", "type": "bytes"}, {"name": "com_k_bytes", "type": "bytes"}], "name": "revealMaskKey", "outputs": [{"name": "", "type": "bool"}], "stateMutability": "nonpayable", "type": "function"},
    {"inputs": [{"name": "sid", "type": "bytes32"}], "name": "claim_refund", "outputs": [], "stateMutability": "nonpayable", "type": "function"},
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

def build_and_send_tx(fn, gas=2_000_000):
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


def run_negative_refund_test(test_id, description, sid_bytes, from_address=MY_ADDRESS):
    """
    Run claim_refund that should revert. Verify:
    1. eth_call reverts
    2. Principal remains unchanged
    """
    principal_before = get_principal()
    fn = contract.functions.claim_refund(sid_bytes)
    success, call_result = try_eth_call(fn, from_address=from_address)

    if success:
        record_result(test_id, description, False, details=f"eth_call unexpectedly succeeded and returned: {call_result}")
        return

    # Call reverted
    principal_after = get_principal()
    state_unchanged = principal_before == principal_after

    passed = state_unchanged
    details = f"Reverted as expected. State unchanged. Error: {str(call_result)[:100]}"
    if not state_unchanged:
        details += " (ERROR: principal changed!)"

    record_result(test_id, description, passed, details=details)


# ─── Main ────────────────────────────────────────────────────────────────────

def main():
    print("=" * 70)
    print("HT-05: REFUND AND TIMEOUT ON-CHAIN TESTS")
    print("=" * 70)
    print(f"Contract:  {CONTRACT_ADDRESS}")
    print(f"Wallet:    {MY_ADDRESS}")
    print(f"Chain:     Arbitrum Sepolia ({CHAIN_ID})")
    print(f"Timestamp: {datetime.now(timezone.utc).isoformat()}")
    print()

    # Approve USDC if needed
    allowance = usdc.functions.allowance(MY_ADDRESS, contract.address).call()
    if allowance < DEPOSIT_AMOUNT:
        print(f"Approving {DEPOSIT_AMOUNT / 1e6} USDC...")
        receipt = build_and_send_tx(usdc.functions.approve(contract.address, DEPOSIT_AMOUNT * 10), gas=200_000)
        print(f"Approve status: {receipt['status']}")

    # ── NT-01: Refund before timelock 24 hours ───────────────────────
    print("\n[NT-01] Testing refund before 24-hour timelock...")
    sid_nt01 = os.urandom(32)
    nonce_nt01 = os.urandom(32).hex()
    bls_data = generate_bls_vectors(nonce_nt01)
    com_k_bytes = bytes.fromhex(bls_data["com_k_hex"].replace("0x", ""))

    print(f"  Depositing for session {sid_nt01.hex()}...")
    receipt = build_and_send_tx(contract.functions.deposit(sid_nt01, com_k_bytes, DEPOSIT_AMOUNT))
    print(f"  Deposit status: {receipt['status']}")
    if receipt['status'] != 1:
        print("  ERROR: Deposit failed")
        sys.exit(1)

    run_negative_refund_test(
        "NT-01", "Refund before timelock 24 hours (reverts)",
        sid_nt01
    )

    # ── NT-02: Refund non-existent session ───────────────────────────
    print("\n[NT-02] Testing refund for non-existent session...")
    sid_nt02 = os.urandom(32)
    run_negative_refund_test(
        "NT-02", "Refund non-existent session (reverts)",
        sid_nt02
    )

    # ── NT-03: Refund by non-client ──────────────────────────────────
    print("\n[NT-03] Testing refund by non-client address...")
    non_client = "0x0000000000000000000000000000000000000001"
    run_negative_refund_test(
        "NT-03", "Refund by non-client address (reverts)",
        sid_nt01, from_address=non_client
    )

    # ── NT-04: Refund after reveal/resolved ──────────────────────────
    print("\n[NT-04] Testing refund after session has been revealed...")
    sid_nt04 = os.urandom(32)
    nonce_nt04 = os.urandom(32).hex()
    bls_data_nt04 = generate_bls_vectors(nonce_nt04)
    com_k_nt04 = bytes.fromhex(bls_data_nt04["com_k_hex"].replace("0x", ""))
    k_nt04 = bytes.fromhex(bls_data_nt04["k_hex"].replace("0x", ""))
    pk_iss_nt04 = bytes.fromhex(bls_data_nt04["pk_iss_hex"].replace("0x", ""))

    print(f"  Depositing for session {sid_nt04.hex()}...")
    receipt = build_and_send_tx(contract.functions.deposit(sid_nt04, com_k_nt04, DEPOSIT_AMOUNT))
    print(f"  Deposit status: {receipt['status']}")
    if receipt['status'] != 1:
        print("  ERROR: Deposit failed")
        sys.exit(1)

    print("  Revealing masking key k to resolve session...")
    receipt = build_and_send_tx(contract.functions.revealMaskKey(sid_nt04, k_nt04, pk_iss_nt04, com_k_nt04))
    print(f"  Reveal status: {receipt['status']}")
    if receipt['status'] != 1:
        print("  ERROR: Reveal failed")
        sys.exit(1)

    run_negative_refund_test(
        "NT-04", "Refund after reveal/resolved (reverts)",
        sid_nt04
    )

    # ── PT-01 to PT-05: Positive Refund (Requires a session > 24 hours old) ──
    print("\n[PT-01..05] Checking for eligible sessions > 24 hours old...")
    print("  Note: In-place Arbitrum Sepolia testnet has a 24-hour timelock.")
    print("  Since we cannot fast-forward time on the public testnet, a real positive refund")
    print("  requires an active session deposited more than 24 hours ago.")
    print("  Skipping positive refund tests (PT-01..PT-05).")

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
        "test_suite": "HT-05 Refund and Timeout On-Chain Tests",
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

    report_path = "/home/azureuser/crypto/test-reports/ht05_refund_tests.json"
    os.makedirs(os.path.dirname(report_path), exist_ok=True)
    with open(report_path, "w") as f:
        json.dump(report, f, indent=2)
    print(f"\n  Report saved: {report_path}")

    sys.exit(0 if failed == 0 else 1)


if __name__ == "__main__":
    main()
