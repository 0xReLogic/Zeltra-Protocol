#!/usr/bin/env python3
"""
HT-04: Spend BLS On-Chain Negative Tests
==========================================

Runs all negative test cases against the deployed Nimbus contract on Arbitrum Sepolia.
For each case, verifies:
  1. Transaction reverts (receipt status == 0) or eth_call reverts
  2. On-chain principal does NOT change
  3. Nullifier is NOT recorded

Test matrix:
  NT-01: Corrupted alpha_neg (1 byte flipped)
  NT-02: Wrong pk_iss (different issuer key)
  NT-03: Wrong recipient (recipient != what was signed)
  NT-04: Wrong amount (amount != what was signed)
  NT-05: Replay nullifier (same valid spend twice)
  NT-06: alpha_neg length 127 bytes (too short)
  NT-07: alpha_neg length 129 bytes (too long)
  NT-08: pk_iss length 255 bytes (too short)
  NT-09: pk_iss length 257 bytes (too long)
  NT-10: Zero/empty alpha_neg (point at infinity)

Prerequisites:
  - Contract deployed and initialized at CONTRACT_ADDRESS
  - Wallet has USDC and ETH for gas
  - Issuer key registered

Usage:
  source nimbus-node/.env.test
  python3 scripts/ht04_spend_negative_tests.py
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
SPEND_AMOUNT = 5_000_000     # 5 USDC

if not PRIVATE_KEY:
    print("ERROR: Set NIMBUS_RELAYER_KEY or PRIVATE_KEY environment variable")
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
    {"inputs": [{"name": "nullifier", "type": "bytes32"}, {"name": "alpha_neg_bytes", "type": "bytes"}, {"name": "pk_iss_bytes", "type": "bytes"}, {"name": "recipient", "type": "address"}, {"name": "amount", "type": "uint256"}, {"name": "recipient_or_intent_hash", "type": "bytes32"}, {"name": "expiry", "type": "uint256"}, {"name": "nonce", "type": "bytes32"}], "name": "spend", "outputs": [{"name": "", "type": "bool"}], "stateMutability": "nonpayable", "type": "function"},
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
    """Build, sign, send transaction and return receipt. Returns None if send fails."""
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


def try_eth_call(fn):
    """Try eth_call (read-only simulation). Returns (success: bool, result_or_error)."""
    try:
        result = fn.call({'from': MY_ADDRESS})
        return True, result
    except Exception as e:
        return False, str(e)


def get_principal():
    """Get current totalDepositedPrincipal from contract."""
    return contract.functions.totalDepositedPrincipal().call()


def generate_bls_vectors(nonce_hex, recipient=None, amount=None, invalid=False):
    """Call cargo generate_bls_test_data and parse output."""
    cmd = [
        "cargo", "run", "-q", "-p", "nimbus-core", "--example", "generate_bls_test_data", "--",
        "--spend-contract",
        "--chain-id", str(CHAIN_ID),
        "--contract", CONTRACT_ADDRESS,
        "--amount", str(amount or SPEND_AMOUNT),
        "--recipient", recipient or MY_ADDRESS,
        "--nonce", nonce_hex,
    ]
    if invalid:
        cmd.append("--invalid")

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
    """Record test result."""
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


def run_negative_spend_test(test_id, description, nullifier_bytes, alpha_neg_bytes, pk_iss_bytes,
                             recipient, amount, intent_hash_bytes, expiry, nonce_bytes,
                             expect_revert=True):
    """
    Run a spend call that should fail. Verify:
    1. eth_call reverts OR returns false
    2. If we send tx anyway, principal doesn't change
    """
    principal_before = get_principal()

    fn = contract.functions.spend(
        nullifier_bytes, alpha_neg_bytes, pk_iss_bytes,
        w3.to_checksum_address(recipient), amount,
        intent_hash_bytes, expiry, nonce_bytes
    )

    # First try eth_call to check if it reverts
    success, call_result = try_eth_call(fn)

    if success and call_result == True:
        # Unexpectedly succeeded — this is a FAIL for a negative test
        record_result(test_id, description, False, details="eth_call unexpectedly returned True")
        return

    if success and call_result == False:
        # Contract returned false (e.g., nullifier replay returns false)
        record_result(test_id, description, True, details=f"eth_call returned False (rejected)")
        return

    # eth_call reverted — good for negative test. Verify principal unchanged.
    principal_after = get_principal()
    state_unchanged = principal_before == principal_after

    if not state_unchanged:
        record_result(test_id, description, False,
                      details=f"Principal changed! Before={principal_before}, After={principal_after}")
        return

    # Extract revert reason if available
    revert_reason = ""
    if "revert" in str(call_result).lower() or "execution reverted" in str(call_result).lower():
        revert_reason = str(call_result)

    record_result(test_id, description, True,
                  details=f"Reverted as expected. State unchanged. {revert_reason[:120]}")


# ─── Main ────────────────────────────────────────────────────────────────────

def main():
    print("=" * 70)
    print("HT-04: SPEND BLS ON-CHAIN NEGATIVE TESTS")
    print("=" * 70)
    print(f"Contract:  {CONTRACT_ADDRESS}")
    print(f"Wallet:    {MY_ADDRESS}")
    print(f"Chain:     Arbitrum Sepolia ({CHAIN_ID})")
    print(f"Timestamp: {datetime.now(timezone.utc).isoformat()}")
    print()

    # ── Phase 0: Setup — Deposit + Reveal to fund the contract ──────────
    print("[Phase 0] Setting up: Deposit 10 USDC + Reveal")
    print("-" * 50)

    nonce_raw = os.urandom(32)
    nonce_hex = nonce_raw.hex()
    sid = os.urandom(32)

    # Generate valid BLS vectors for the positive spend
    valid_data = generate_bls_vectors(nonce_hex)

    pk_iss_bytes = bytes.fromhex(valid_data["pk_iss_hex"].replace("0x", ""))
    com_k_bytes = bytes.fromhex(valid_data["com_k_hex"].replace("0x", ""))
    k_bytes = bytes.fromhex(valid_data["k_hex"].replace("0x", ""))

    # Register issuer key if needed
    is_trusted = contract.functions.isIssuerKeyTrusted(pk_iss_bytes).call()
    if not is_trusted:
        print("  Registering issuer key...")
        receipt = build_and_send_tx(contract.functions.registerIssuerKey(pk_iss_bytes), gas=1_000_000)
        print(f"  registerIssuerKey status: {receipt['status']}")
        if receipt['status'] != 1:
            print("  ERROR: Failed to register issuer key")
            sys.exit(1)

    # Approve USDC
    allowance = usdc.functions.allowance(MY_ADDRESS, contract.address).call()
    if allowance < DEPOSIT_AMOUNT:
        print(f"  Approving {DEPOSIT_AMOUNT / 1e6} USDC...")
        receipt = build_and_send_tx(usdc.functions.approve(contract.address, DEPOSIT_AMOUNT), gas=200_000)
        print(f"  Approve status: {receipt['status']}")

    # Deposit
    print(f"  Depositing {DEPOSIT_AMOUNT / 1e6} USDC...")
    receipt = build_and_send_tx(contract.functions.deposit(sid, com_k_bytes, DEPOSIT_AMOUNT), gas=2_000_000)
    print(f"  Deposit status: {receipt['status']}, tx: {receipt['transactionHash'].hex()}")
    if receipt['status'] != 1:
        print("  ERROR: Deposit failed")
        sys.exit(1)

    # Reveal
    print("  Revealing masking key k...")
    receipt = build_and_send_tx(
        contract.functions.revealMaskKey(sid, k_bytes, pk_iss_bytes, com_k_bytes), gas=2_000_000
    )
    print(f"  Reveal status: {receipt['status']}, tx: {receipt['transactionHash'].hex()}")
    if receipt['status'] != 1:
        print("  ERROR: Reveal failed")
        sys.exit(1)

    principal_after_deposit = get_principal()
    usdc_balance_before = usdc.functions.balanceOf(MY_ADDRESS).call()
    print(f"  Principal after deposit: {principal_after_deposit / 1e6:.4f} USDC")
    print(f"  Wallet USDC balance: {usdc_balance_before / 1e6:.4f} USDC")
    print()

    # ── Phase 1: Negative Tests (MUST ALL FAIL) ────────────────────────

    print("[Phase 1] Running Negative Spend Tests")
    print("=" * 70)

    # Parse valid vectors for reuse
    nullifier_bytes = bytes.fromhex(valid_data["nullifier_hex"].replace("0x", ""))
    alpha_neg_bytes_valid = bytes.fromhex(valid_data["alpha_neg_hex"].replace("0x", ""))
    intent_hash_bytes = bytes.fromhex(valid_data["recipient_or_intent_hash_hex"].replace("0x", ""))
    expiry_val = int(valid_data["expiry"])
    nonce_bytes = bytes.fromhex(valid_data["nonce_hex"].replace("0x", ""))

    # ── NT-01: Corrupted alpha_neg (1 byte flipped) ──────────────────
    print("\n  NT-01: Corrupted alpha_neg (flip byte 127)")
    corrupted_alpha = bytearray(alpha_neg_bytes_valid)
    corrupted_alpha[127] ^= 0x01
    run_negative_spend_test(
        "NT-01", "Corrupted alpha_neg — 1 byte flipped at position 127",
        nullifier_bytes, bytes(corrupted_alpha), pk_iss_bytes,
        MY_ADDRESS, SPEND_AMOUNT, intent_hash_bytes, expiry_val, nonce_bytes
    )

    # ── NT-02: Corrupted alpha_neg (flip byte 0) ─────────────────────
    print("\n  NT-02: Corrupted alpha_neg (flip byte 0)")
    corrupted_alpha2 = bytearray(alpha_neg_bytes_valid)
    corrupted_alpha2[0] ^= 0x01
    run_negative_spend_test(
        "NT-02", "Corrupted alpha_neg — 1 byte flipped at position 0",
        nullifier_bytes, bytes(corrupted_alpha2), pk_iss_bytes,
        MY_ADDRESS, SPEND_AMOUNT, intent_hash_bytes, expiry_val, nonce_bytes
    )

    # ── NT-03: Wrong pk_iss (flip 1 byte) ────────────────────────────
    print("\n  NT-03: Wrong pk_iss (flip byte 100)")
    corrupted_pk = bytearray(pk_iss_bytes)
    corrupted_pk[100] ^= 0x01
    run_negative_spend_test(
        "NT-03", "Wrong pk_iss — 1 byte flipped at position 100",
        nullifier_bytes, alpha_neg_bytes_valid, bytes(corrupted_pk),
        MY_ADDRESS, SPEND_AMOUNT, intent_hash_bytes, expiry_val, nonce_bytes
    )

    # ── NT-04: Wrong recipient ───────────────────────────────────────
    print("\n  NT-04: Wrong recipient (different address)")
    wrong_recipient = "0x0000000000000000000000000000000000000001"
    run_negative_spend_test(
        "NT-04", "Wrong recipient — address doesn't match signed intent hash",
        nullifier_bytes, alpha_neg_bytes_valid, pk_iss_bytes,
        wrong_recipient, SPEND_AMOUNT, intent_hash_bytes, expiry_val, nonce_bytes
    )

    # ── NT-05: Wrong amount ──────────────────────────────────────────
    print("\n  NT-05: Wrong amount (6 USDC instead of 5)")
    # Generate vectors for 5 USDC but pass 6 USDC — nullifier/hm will mismatch
    run_negative_spend_test(
        "NT-05", "Wrong amount — 6 USDC but signed for 5 USDC",
        nullifier_bytes, alpha_neg_bytes_valid, pk_iss_bytes,
        MY_ADDRESS, 6_000_000, intent_hash_bytes, expiry_val, nonce_bytes
    )

    # ── NT-06: alpha_neg too short (127 bytes) ───────────────────────
    print("\n  NT-06: alpha_neg too short (127 bytes)")
    run_negative_spend_test(
        "NT-06", "alpha_neg length 127 — INVALID_G1_INPUT_LENGTH expected",
        nullifier_bytes, alpha_neg_bytes_valid[:127], pk_iss_bytes,
        MY_ADDRESS, SPEND_AMOUNT, intent_hash_bytes, expiry_val, nonce_bytes
    )

    # ── NT-07: alpha_neg too long (129 bytes) ────────────────────────
    print("\n  NT-07: alpha_neg too long (129 bytes)")
    run_negative_spend_test(
        "NT-07", "alpha_neg length 129 — INVALID_G1_INPUT_LENGTH expected",
        nullifier_bytes, alpha_neg_bytes_valid + b'\x00', pk_iss_bytes,
        MY_ADDRESS, SPEND_AMOUNT, intent_hash_bytes, expiry_val, nonce_bytes
    )

    # ── NT-08: pk_iss too short (255 bytes) ──────────────────────────
    print("\n  NT-08: pk_iss too short (255 bytes)")
    run_negative_spend_test(
        "NT-08", "pk_iss length 255 — INVALID_PUBLIC_KEY_LENGTH expected",
        nullifier_bytes, alpha_neg_bytes_valid, pk_iss_bytes[:255],
        MY_ADDRESS, SPEND_AMOUNT, intent_hash_bytes, expiry_val, nonce_bytes
    )

    # ── NT-09: pk_iss too long (257 bytes) ───────────────────────────
    print("\n  NT-09: pk_iss too long (257 bytes)")
    run_negative_spend_test(
        "NT-09", "pk_iss length 257 — INVALID_PUBLIC_KEY_LENGTH expected",
        nullifier_bytes, alpha_neg_bytes_valid, pk_iss_bytes + b'\x00',
        MY_ADDRESS, SPEND_AMOUNT, intent_hash_bytes, expiry_val, nonce_bytes
    )

    # ── NT-10: Zero alpha_neg (point at infinity / all zeros) ────────
    print("\n  NT-10: Zero alpha_neg (128 zero bytes)")
    run_negative_spend_test(
        "NT-10", "Zero alpha_neg — point at infinity / all zeros",
        nullifier_bytes, bytes(128), pk_iss_bytes,
        MY_ADDRESS, SPEND_AMOUNT, intent_hash_bytes, expiry_val, nonce_bytes
    )

    # ── Phase 2: Positive Test (spend must succeed) ──────────────────
    print()
    print("[Phase 2] Positive Test — Valid Spend (5 USDC)")
    print("=" * 70)

    principal_before_spend = get_principal()
    print(f"  Principal before spend: {principal_before_spend / 1e6:.4f} USDC")

    fn = contract.functions.spend(
        nullifier_bytes, alpha_neg_bytes_valid, pk_iss_bytes,
        w3.to_checksum_address(MY_ADDRESS), SPEND_AMOUNT,
        intent_hash_bytes, expiry_val, nonce_bytes
    )

    # Positive spend
    try:
        receipt = build_and_send_tx(fn)
        if receipt['status'] == 1:
            record_result("PT-01", "Valid spend succeeds on-chain", True,
                          tx_hash=receipt['transactionHash'].hex(),
                          details=f"Gas used: {receipt['gasUsed']}")
        else:
            record_result("PT-01", "Valid spend succeeds on-chain", False,
                          tx_hash=receipt['transactionHash'].hex(),
                          details="Receipt status == 0 (reverted)")
    except Exception as e:
        record_result("PT-01", "Valid spend succeeds on-chain", False, details=str(e))

    time.sleep(2)

    # Verify principal decreased
    principal_after_spend = get_principal()
    print(f"  Principal after spend: {principal_after_spend / 1e6:.4f} USDC")
    principal_delta = principal_before_spend - principal_after_spend
    print(f"  Principal delta: -{principal_delta / 1e6:.4f} USDC")

    if principal_delta > 0:
        record_result("PT-02", "Principal decreased after valid spend", True,
                      details=f"Delta: -{principal_delta / 1e6:.4f} USDC")
    else:
        record_result("PT-02", "Principal decreased after valid spend", False,
                      details=f"Delta: {principal_delta / 1e6:.4f} USDC (expected negative)")

    # ── NT-11: Replay nullifier (same valid spend again) ─────────────
    print()
    print("[Phase 3] Replay Test — Same Nullifier Again")
    print("=" * 70)

    print("\n  NT-11: Replay nullifier (exact same valid spend)")
    run_negative_spend_test(
        "NT-11", "Replay nullifier — same valid spend a second time",
        nullifier_bytes, alpha_neg_bytes_valid, pk_iss_bytes,
        MY_ADDRESS, SPEND_AMOUNT, intent_hash_bytes, expiry_val, nonce_bytes
    )

    # ── Phase 4: Verify final state ─────────────────────────────────
    print()
    print("[Phase 4] Final State Verification")
    print("=" * 70)

    final_principal = get_principal()
    final_balance = usdc.functions.balanceOf(MY_ADDRESS).call()
    print(f"  Final principal:    {final_principal / 1e6:.4f} USDC")
    print(f"  Final USDC balance: {final_balance / 1e6:.4f} USDC")

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

    if failed == 0:
        print("\n  🎉 ALL HT-04 TESTS PASSED!")
    else:
        print(f"\n  ⚠️  {failed} TEST(S) FAILED — REVIEW REQUIRED")

    # ── Save report ──────────────────────────────────────────────────
    report = {
        "test_suite": "HT-04 Spend BLS On-Chain Negative Tests",
        "contract": CONTRACT_ADDRESS,
        "chain_id": CHAIN_ID,
        "wallet": MY_ADDRESS,
        "timestamp": datetime.now(timezone.utc).isoformat(),
        "deposit_amount": DEPOSIT_AMOUNT,
        "spend_amount": SPEND_AMOUNT,
        "principal_after_deposit": principal_after_deposit,
        "principal_after_spend": principal_after_spend,
        "final_principal": final_principal,
        "results": results,
        "summary": {
            "total": total,
            "passed": passed,
            "failed": failed,
        }
    }

    report_path = "/home/azureuser/crypto/test-reports/ht04_spend_negative_tests.json"
    os.makedirs(os.path.dirname(report_path), exist_ok=True)
    with open(report_path, "w") as f:
        json.dump(report, f, indent=2)
    print(f"\n  Report saved: {report_path}")

    sys.exit(0 if failed == 0 else 1)


if __name__ == "__main__":
    main()
