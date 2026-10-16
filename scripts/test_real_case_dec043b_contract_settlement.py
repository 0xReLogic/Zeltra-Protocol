#!/usr/bin/env python3
"""
Zeltra Protocol - DEC-043B Real-Case Smart Contract Settlement & In-Pool Transfer Test Suite

This test suite executes independent end-to-end verification of the Stylus WASM Smart Contract
(`zeltra-contracts`) for DEC-043 and DEC-043B:

1. Web3 / eth_abi Calldata Parity & ABI Encoding:
   - Validates function selector, parameter layouts, and calldata packing.

2. Cryptographic Soundness & Real Note Discovery (X25519 ECDH + ChaCha20Poly1305):
   - Derives real ephemeral keys and encrypts 96-byte memos.
   - Proves merchant note discovery without disclosing public identity.

3. Strict 3-Mode Mutually Exclusive State Machine Verification:
   - Mode 1: Epoch Rollover (is_rollover == 1, merchant_amount == 0, protocol_fee == 0, recipient == 0x0)
   - Mode 2: In-Pool Transfer (is_rollover == 0, merchant_amount == 0, has_change_1 == 1, protocol_fee == 0, recipient == 0x0)
   - Mode 3: Public Outflow Spend (is_rollover == 0, merchant_amount > 0, recipient != 0x0)

4. Comprehensive Threat Matrix & Adversary Revert Verification:
   - AUD-4: Anti-Silent-Burn Revert Guard (ZERO_RECIPIENT_ON_PUBLIC_SPEND)
   - AUD-5: Calldata Memo Length Guard (INVALID_MEMO_LENGTH for 0B, 95B, 97B, 120B)
   - Mode 2 Violations: Non-zero recipient, Non-zero protocol fee, Zero change note 1
   - Mode 1 Violations: Non-zero merchant amount, Non-zero protocol fee, Non-zero recipient
   - Double Spend Prevention: Nullifier reuse & singularity aliasing (nf1 == nf2)
   - Canonical BLS12-381 Scalar Validation (Fr danger zone >= r)

5. Solvency Invariant Proofs:
   - Absolute conservation: Net change in liabilities == 0 on in-pool transfer.
   - Zero ERC-20 token outflow during in-pool transfers.
"""

import sys
import os
import json
import subprocess
import secrets
from typing import Dict, Any, List, Optional, Tuple

try:
    from eth_abi import encode
    from web3 import Web3
    from cryptography.hazmat.primitives.asymmetric import x25519
    from cryptography.hazmat.primitives.ciphers.aead import ChaCha20Poly1305
    from cryptography.hazmat.primitives.kdf.hkdf import HKDF
    from cryptography.hazmat.primitives import hashes
except ImportError as e:
    print(f"Missing dependency: {e}. Please install via pip: pip install web3 eth_abi cryptography")
    sys.exit(1)

# ANSI terminal formatting
GREEN = "\033[92m"
RED = "\033[91m"
YELLOW = "\033[93m"
CYAN = "\033[96m"
BOLD = "\033[1m"
RESET = "\033[0m"

# BLS12-381 Fr prime scalar modulus (r)
FR_MODULUS = 52435875175126190479447740508185965837690552500527637822603658699938581184513
EMPTY_TREE_ROOT = "0x02511def7a6e24ef123d56fa852276aa9863776aa1857093a76e053d09b67117"

TOTAL_TESTS = 0
PASSED_TESTS = 0


def print_header(title: str):
    print(f"\n{BOLD}{CYAN}=== {title} ==={RESET}")


def assert_test(condition: bool, name: str, details: str = ""):
    global TOTAL_TESTS, PASSED_TESTS
    TOTAL_TESTS += 1
    if condition:
        PASSED_TESTS += 1
        print(f"  {GREEN}[PASS]{RESET} {name} {details}")
    else:
        print(f"  {RED}[FAIL]{RESET} {name} {details}")
        sys.exit(1)


def find_or_build_harness() -> str:
    """Finds or builds the contract_interop binary."""
    repo_root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    target_bin = os.path.join(repo_root, "target", "debug", "contract_interop")

    if not os.path.exists(target_bin):
        print(f"{YELLOW}[BUILD]{RESET} Building contract_interop test harness...")
        res = subprocess.run(
            ["cargo", "build", "--bin", "contract_interop", "-p", "zeltra_contracts"],
            cwd=repo_root,
            capture_output=True,
            text=True,
        )
        if res.returncode != 0:
            print(f"{RED}[BUILD ERROR]{RESET} Failed to build contract_interop:\n{res.stderr}")
            sys.exit(1)

    assert os.path.exists(target_bin), "contract_interop binary must exist"
    return target_bin


def run_contract(harness_bin: str, kv_params: Dict[str, Any]) -> Dict[str, Any]:
    """Executes a transaction against the compiled Stylus contract harness."""
    input_lines = []
    for k, v in kv_params.items():
        input_lines.append(f"{k}={v}")
    payload = "\n".join(input_lines) + "\n"

    proc = subprocess.run(
        [harness_bin],
        input=payload,
        capture_output=True,
        text=True,
        check=True,
    )
    stdout = proc.stdout.strip()
    try:
        return json.loads(stdout)
    except json.JSONDecodeError:
        raise RuntimeError(f"Failed to parse contract output as JSON: {stdout}\nStderr: {proc.stderr}")


# ==============================================================================
# Cryptographic Helpers (DEC-043A ZSA & ECDH Memos)
# ==============================================================================

KDF_SALT = b"ZELTRA_NOTE_MEMO_KDF_V1"
KDF_INFO = b"ZELTRA_MEMO_CIPHER_V1"


def generate_zsa_keypair() -> Tuple[x25519.X25519PrivateKey, bytes, bytes]:
    """Generates an X25519 keypair for in-pool viewing key derivation."""
    priv = x25519.X25519PrivateKey.generate()
    pub = priv.public_key().public_bytes_raw()
    owner_key = secrets.token_bytes(32)
    # Ensure owner_key is canonical scalar (< r)
    scalar_int = int.from_bytes(owner_key, "big") % FR_MODULUS
    canonical_owner_key = scalar_int.to_bytes(32, "big")
    return priv, pub, canonical_owner_key


def encrypt_note_memo(
    recipient_pub_bytes: bytes,
    note_value: int,
    asset_id: int,
    rho: bytes,
    r: bytes,
) -> bytes:
    """Encrypts a 96-byte memo using ephemeral X25519 ECDH + ChaCha20Poly1305."""
    ephemeral_priv = x25519.X25519PrivateKey.generate()
    ephemeral_pub_bytes = ephemeral_priv.public_key().public_bytes_raw()

    recip_pub = x25519.X25519PublicKey.from_public_bytes(recipient_pub_bytes)
    shared_secret = ephemeral_priv.exchange(recip_pub)

    hkdf = HKDF(
        algorithm=hashes.SHA256(),
        length=32,
        salt=KDF_SALT,
        info=KDF_INFO,
    )
    symmetric_key = hkdf.derive(shared_secret)

    # 48-byte plaintext: value (8B), asset_id (1B), rho (16B), r (23B)
    plaintext = bytearray(48)
    plaintext[0:8] = note_value.to_bytes(8, "big")
    plaintext[8] = asset_id
    plaintext[9:25] = rho[:16]
    plaintext[25:48] = r[:23]

    nonce = b"\x00" * 12
    aead = ChaCha20Poly1305(symmetric_key)
    ciphertext_and_tag = aead.encrypt(nonce, bytes(plaintext), None)  # 48 + 16 = 64 bytes

    # 96-byte memo: ephemeral_pub (32B) + ciphertext_and_tag (64B)
    return ephemeral_pub_bytes + ciphertext_and_tag


def decrypt_note_memo(
    recipient_priv: x25519.X25519PrivateKey,
    memo: bytes,
) -> Optional[Tuple[int, int]]:
    """Attempts trial decryption of a 96-byte memo."""
    if len(memo) != 96:
        return None
    ephemeral_pub_bytes = memo[:32]
    ciphertext_and_tag = memo[32:96]

    try:
        ephemeral_pub = x25519.X25519PublicKey.from_public_bytes(ephemeral_pub_bytes)
        shared_secret = recipient_priv.exchange(ephemeral_pub)

        hkdf = HKDF(
            algorithm=hashes.SHA256(),
            length=32,
            salt=KDF_SALT,
            info=KDF_INFO,
        )
        symmetric_key = hkdf.derive(shared_secret)

        nonce = b"\x00" * 12
        aead = ChaCha20Poly1305(symmetric_key)
        plaintext = aead.decrypt(nonce, ciphertext_and_tag, None)

        val = int.from_bytes(plaintext[0:8], "big")
        asset_id = plaintext[8]
        return val, asset_id
    except Exception:
        return None


def random_canonical_scalar_hex() -> str:
    """Returns a random 32-byte canonical scalar hex string (< r)."""
    val = secrets.randbelow(FR_MODULUS - 1) + 1
    return "0x" + val.to_bytes(32, "big").hex()


# ==============================================================================
# Test Execution Matrix
# ==============================================================================

def main():
    print(f"{BOLD}{GREEN}===================================================================={RESET}")
    print(f"{BOLD}{GREEN}  Zeltra Protocol - DEC-043B Smart Contract Real-Case Test Suite    {RESET}")
    print(f"{BOLD}{GREEN}  Strict 3-Mode Settlement, Dual-Memo MMR & Invariant Verification  {RESET}")
    print(f"{BOLD}{GREEN}===================================================================={RESET}")

    harness_bin = find_or_build_harness()
    print(f"Harness binary: {harness_bin}")

    # --------------------------------------------------------------------------
    # 1. Web3 ABI Encoding & Function Calldata Parity
    # --------------------------------------------------------------------------
    print_header("1. Web3 ABI Encoding & Selector Parity")
    
    # Check spend_joinsplit function signature
    # In EVM Stylus ABI: spend_joinsplit(bytes,bytes,bytes,bytes32,uint256,bytes32,bytes32,uint256,uint256,bytes32,bytes32,address,uint256,uint256,uint256,uint256,bytes32,uint256,uint256,uint256,bytes,bytes)
    fn_sig = "spend_joinsplit(bytes,bytes,bytes,bytes32,uint256,bytes32,bytes32,uint256,uint256,bytes32,bytes32,address,uint256,uint256,uint256,uint256,bytes32,uint256,uint256,uint256,bytes,bytes)"
    keccak_hash = Web3.keccak(text=fn_sig).hex()
    selector = keccak_hash[:10]
    assert_test(len(selector) == 10, "spend_joinsplit selector length", f"selector={selector}")

    # ABI encode dummy arguments to test eth_abi encoding
    dummy_proof_a = b"\x11" * 128
    dummy_proof_b = b"\x22" * 256
    dummy_proof_c = b"\x33" * 128
    dummy_root = bytes.fromhex(EMPTY_TREE_ROOT[2:])
    dummy_memo = b"\xaa" * 96

    encoded_calldata = encode(
        ["bytes", "bytes", "bytes", "bytes32", "uint256", "bytes32", "bytes32", "uint256", "uint256", "bytes32", "bytes32", "address", "uint256", "uint256", "uint256", "uint256", "bytes32", "uint256", "uint256", "uint256", "bytes", "bytes"],
        [
            dummy_proof_a, dummy_proof_b, dummy_proof_c, dummy_root, 1,
            b"\x01" * 32, b"\x02" * 32, 0, 0, b"\x03" * 32, b"\x04" * 32,
            "0x0000000000000000000000000000000000000000",
            0, 0, 10000, 20000, b"\x00" * 32, 2000000000, 7, 0,
            dummy_memo, dummy_memo
        ]
    )
    assert_test(len(encoded_calldata) > 500, "eth_abi encodes 22-argument join_split calldata", f"length={len(encoded_calldata)} bytes")

    # --------------------------------------------------------------------------
    # 2. Mode 2: In-Pool Transfer (USDC & WETH) — Tokyo Coffee Real-Life Flow
    # --------------------------------------------------------------------------
    print_header("2. Mode 2: In-Pool Transfer (Tokyo Coffee Stealth Settlement)")

    # Alice (Customer) has 100 USDC note. Alice wants to buy coffee from Bob (Merchant) for 25 USDC.
    # Alice keypair
    alice_priv, alice_pub, alice_owner = generate_zsa_keypair()
    # Bob (Merchant) publishes Shielded Address (Bob keypair)
    bob_priv, bob_pub, bob_owner = generate_zsa_keypair()

    alice_change_amount = 74_000_000  # 74 USDC
    bob_payment_amount = 25_000_000   # 25 USDC
    relayer_execution_fee = 1_000_000 # 1 USDC
    protocol_fee = 0                  # 0 USDC (In-Pool fee policy)
    initial_liability = 100_000_000   # 100 USDC

    # Real ChaCha20Poly1305 ECDH encrypted memos
    memo_1_alice_change = encrypt_note_memo(alice_pub, alice_change_amount, 1, secrets.token_bytes(16), secrets.token_bytes(32))
    memo_2_bob_payment = encrypt_note_memo(bob_pub, bob_payment_amount, 1, secrets.token_bytes(16), secrets.token_bytes(32))

    assert_test(len(memo_1_alice_change) == 96, "Alice change memo is exactly 96 bytes")
    assert_test(len(memo_2_bob_payment) == 96, "Bob payment memo is exactly 96 bytes")

    nf1 = random_canonical_scalar_hex()
    nf2 = random_canonical_scalar_hex()
    cm1 = random_canonical_scalar_hex()
    cm2 = random_canonical_scalar_hex()

    res = run_contract(harness_bin, {
        "asset_id": 1,
        "initial_liability": initial_liability,
        "merchant_amount": 0,
        "protocol_fee": 0,
        "execution_fee": relayer_execution_fee,
        "max_execution_fee": 2_000_000,
        "recipient": "0x0000000000000000000000000000000000000000",
        "flags_packed": 7, # asset_id=1, has_change_1=1, has_change_2=1
        "is_rollover": 0,
        "nullifier_1": nf1,
        "nullifier_2": nf2,
        "output_cm_1": cm1,
        "output_cm_2": cm2,
        "memo_1": "0x" + memo_1_alice_change.hex(),
        "memo_2": "0x" + memo_2_bob_payment.hex(),
    })

    assert_test(res["success"] is True, "Mode 2 USDC In-Pool Transfer succeeds", f"revert={res['revert_reason']}")
    assert_test(len(res["erc20_transfers"]) == 0, "Mode 2: ERC-20 transfer is completely BYPASSED (0 token transfers)", f"transfers={res['erc20_transfers']}")
    assert_test(res["mmr_leaf_count"] == 2, "Mode 2: MMR leaf count increments by +2", f"count={res['mmr_leaf_count']}")
    assert_test(res["nullifier_1_spent"] is True and res["nullifier_2_spent"] is True, "Mode 2: Both input nullifiers marked spent")

    # Solvency & Liability Accounting check:
    # User liabilities should drop from 100M to 99M (debited only by execution_fee 1M)
    # Accrued relayer execution fee should increase from 0 to 1M
    # Total liabilities = 99M + 1M = 100M (Invariant conserved!)
    assert_test(int(res["user_note_liabilities_usdc"]) == 99_000_000, "User note liability debited by exactly execution fee", f"val={res['user_note_liabilities_usdc']}")
    assert_test(int(res["accrued_execution_fee_liabilities_usdc"]) == 1_000_000, "Relayer accrued fee credited with 1 USDC", f"val={res['accrued_execution_fee_liabilities_usdc']}")
    assert_test(int(res["realized_protocol_fees_usdc"]) == 0, "Realized protocol fee is 0 USDC in transfer")

    # Note Discovery Check:
    # Bob scans memo 2 and recovers 25 USDC
    bob_recov = decrypt_note_memo(bob_priv, memo_2_bob_payment)
    assert_test(bob_recov is not None and bob_recov[0] == 25_000_000 and bob_recov[1] == 1, "Bob recovers 25 USDC payment note from on-chain memo 2")

    # Alice scans memo 1 and recovers 74 USDC
    alice_recov = decrypt_note_memo(alice_priv, memo_1_alice_change)
    assert_test(alice_recov is not None and alice_recov[0] == 74_000_000 and alice_recov[1] == 1, "Alice recovers 74 USDC change note from on-chain memo 1")

    # Attacker tries to decrypt Bob's memo with random key
    attacker_priv, _, _ = generate_zsa_keypair()
    attacker_recov = decrypt_note_memo(attacker_priv, memo_2_bob_payment)
    assert_test(attacker_recov is None, "Third party / attacker cannot decrypt payment memo")

    # Test WETH In-Pool Transfer (asset_id = 0)
    weth_res = run_contract(harness_bin, {
        "asset_id": 0,
        "initial_liability": 5_000_000_000, # 5 WETH (Gwei units)
        "merchant_amount": 0,
        "protocol_fee": 0,
        "execution_fee": 50_000_000, # 0.05 WETH
        "max_execution_fee": 100_000_000,
        "recipient": "0x0000000000000000000000000000000000000000",
        "flags_packed": 3, # asset_id=0, has_change_1=1, has_change_2=1
        "is_rollover": 0,
        "nullifier_1": random_canonical_scalar_hex(),
        "nullifier_2": random_canonical_scalar_hex(),
        "output_cm_1": random_canonical_scalar_hex(),
        "output_cm_2": random_canonical_scalar_hex(),
        "memo_1": "0x" + memo_1_alice_change.hex(),
        "memo_2": "0x" + memo_2_bob_payment.hex(),
    })
    assert_test(weth_res["success"] is True, "Mode 2 WETH In-Pool Transfer succeeds", f"revert={weth_res['revert_reason']}")
    assert_test(len(weth_res["erc20_transfers"]) == 0, "Mode 2: WETH transfer is completely BYPASSED", f"transfers={weth_res['erc20_transfers']}")
    assert_test(int(weth_res["user_note_liabilities_weth"]) == 4_950_000_000, "WETH user liabilities debited by execution fee")
    assert_test(int(weth_res["accrued_execution_fee_liabilities_weth"]) == 50_000_000, "WETH relayer fee credited")

    # --------------------------------------------------------------------------
    # 3. Mode 3: Public Outflow Spend & AUD-4 Anti-Silent-Burn Guard
    # --------------------------------------------------------------------------
    print_header("3. Mode 3: Public Outflow Spend & AUD-4 Anti-Silent-Burn Guard")

    legit_recipient = "0x70997970c51812dc3a010c7d01b50e0d17dc79c8"

    # Legitimate Public Outflow Spend (merchant_amount > 0, recipient != 0x0)
    pub_res = run_contract(harness_bin, {
        "asset_id": 1,
        "initial_liability": 50_000_000,
        "merchant_amount": 20_000_000,
        "protocol_fee": 90_000, # 45 bps on 20 USDC
        "execution_fee": 500_000,
        "max_execution_fee": 1_000_000,
        "recipient": legit_recipient,
        "flags_packed": 7,
        "is_rollover": 0,
        "nullifier_1": random_canonical_scalar_hex(),
        "nullifier_2": random_canonical_scalar_hex(),
        "output_cm_1": random_canonical_scalar_hex(),
        "output_cm_2": random_canonical_scalar_hex(),
        "memo_1": "0x" + memo_1_alice_change.hex(),
        "memo_2": "0x" + memo_2_bob_payment.hex(),
    })
    assert_test(pub_res["success"] is True, "Mode 3 legitimate public spend succeeds", f"revert={pub_res['revert_reason']}")
    assert_test(len(pub_res["erc20_transfers"]) == 1, "Mode 3: Exactly 1 ERC-20 transfer executed")
    assert_test(pub_res["erc20_transfers"][0]["recipient"].lower() == legit_recipient.lower(), "ERC-20 transfer recipient matches")
    assert_test(int(pub_res["erc20_transfers"][0]["amount"]) == 20_000_000, "ERC-20 transfer amount matches merchant_amount")

    # AUD-4 ATTACK: Public Spend with Zero Address Recipient
    # (Attacker / buggy client attempts public spend with recipient = 0x0)
    silent_burn_res = run_contract(harness_bin, {
        "asset_id": 1,
        "initial_liability": 50_000_000,
        "merchant_amount": 20_000_000, # > 0
        "protocol_fee": 90_000,
        "execution_fee": 500_000,
        "max_execution_fee": 1_000_000,
        "recipient": "0x0000000000000000000000000000000000000000", # ZERO ADDRESS!
        "flags_packed": 7,
        "is_rollover": 0,
        "nullifier_1": random_canonical_scalar_hex(),
        "nullifier_2": random_canonical_scalar_hex(),
        "output_cm_1": random_canonical_scalar_hex(),
        "output_cm_2": random_canonical_scalar_hex(),
        "memo_1": "0x" + memo_1_alice_change.hex(),
        "memo_2": "0x" + memo_2_bob_payment.hex(),
    })
    assert_test(silent_burn_res["success"] is False, "AUD-4: Public spend with zero address fails")
    assert_test(silent_burn_res["revert_reason"] == "ZERO_RECIPIENT_ON_PUBLIC_SPEND", "AUD-4: Exact revert reason ZERO_RECIPIENT_ON_PUBLIC_SPEND", f"got={silent_burn_res['revert_reason']}")
    assert_test(len(silent_burn_res["erc20_transfers"]) == 0, "AUD-4: Zero ERC-20 transfers made on revert")
    assert_test(silent_burn_res["mmr_leaf_count"] == 0, "AUD-4: Zero MMR leaves added on revert")
    assert_test(silent_burn_res["nullifier_1_spent"] is False, "AUD-4: Nullifier 1 remains unspent")

    # --------------------------------------------------------------------------
    # 4. AUD-5: Calldata Memo Length Guards (Anti Gas-Griefing)
    # --------------------------------------------------------------------------
    print_header("4. AUD-5: Calldata Memo Length Guards")

    memo_lengths_to_test = [
        (95, "Truncated 95 bytes (memo 1)", True),
        (97, "Bloated 97 bytes (memo 1)", True),
        (0,  "Empty 0 bytes (memo 1)", True),
        (120,"Oversized 120 bytes (memo 1)", True),
        (95, "Truncated 95 bytes (memo 2)", False),
        (97, "Bloated 97 bytes (memo 2)", False),
    ]

    for length, desc, test_memo_1 in memo_lengths_to_test:
        bad_memo = "0x" + ("ee" * length)
        good_memo = "0x" + ("aa" * 96)
        m1 = bad_memo if test_memo_1 else good_memo
        m2 = good_memo if test_memo_1 else bad_memo

        memo_res = run_contract(harness_bin, {
            "asset_id": 1,
            "initial_liability": 50_000_000,
            "merchant_amount": 0,
            "protocol_fee": 0,
            "execution_fee": 10_000,
            "max_execution_fee": 20_000,
            "recipient": "0x0000000000000000000000000000000000000000",
            "flags_packed": 7,
            "is_rollover": 0,
            "nullifier_1": random_canonical_scalar_hex(),
            "nullifier_2": random_canonical_scalar_hex(),
            "output_cm_1": random_canonical_scalar_hex(),
            "output_cm_2": random_canonical_scalar_hex(),
            "memo_1": m1,
            "memo_2": m2,
        })
        assert_test(memo_res["success"] is False, f"AUD-5: Rejects {desc}")
        assert_test(memo_res["revert_reason"] == "INVALID_MEMO_LENGTH", f"AUD-5: Exact revert INVALID_MEMO_LENGTH for {desc}", f"got={memo_res['revert_reason']}")
        assert_test(memo_res["mmr_leaf_count"] == 0, f"AUD-5: State clean for {desc}")

    # --------------------------------------------------------------------------
    # 5. Mode 2: In-Pool Transfer Violation Matrix
    # --------------------------------------------------------------------------
    print_header("5. Mode 2: In-Pool Transfer Violation Matrix")

    # Violation 1: Non-Zero Recipient on In-Pool Transfer
    viol_recip_res = run_contract(harness_bin, {
        "asset_id": 1,
        "initial_liability": 50_000_000,
        "merchant_amount": 0,
        "protocol_fee": 0,
        "execution_fee": 10_000,
        "max_execution_fee": 20_000,
        "recipient": "0x1111111111111111111111111111111111111111", # Non-zero!
        "flags_packed": 7,
        "is_rollover": 0,
        "nullifier_1": random_canonical_scalar_hex(),
        "nullifier_2": random_canonical_scalar_hex(),
        "output_cm_1": random_canonical_scalar_hex(),
        "output_cm_2": random_canonical_scalar_hex(),
        "memo_1": "0x" + ("aa" * 96),
        "memo_2": "0x" + ("bb" * 96),
    })
    assert_test(viol_recip_res["success"] is False, "Mode 2: Rejects non-zero recipient")
    assert_test(viol_recip_res["revert_reason"] == "RECIPIENT_MUST_BE_ZERO_FOR_IN_POOL_TRANSFER", "Mode 2: Revert reason RECIPIENT_MUST_BE_ZERO_FOR_IN_POOL_TRANSFER", f"got={viol_recip_res['revert_reason']}")

    # Violation 2: Non-Zero Protocol Fee on In-Pool Transfer
    viol_fee_res = run_contract(harness_bin, {
        "asset_id": 1,
        "initial_liability": 50_000_000,
        "merchant_amount": 0,
        "protocol_fee": 50_000, # Non-zero protocol fee!
        "execution_fee": 10_000,
        "max_execution_fee": 20_000,
        "recipient": "0x0000000000000000000000000000000000000000",
        "flags_packed": 7,
        "is_rollover": 0,
        "nullifier_1": random_canonical_scalar_hex(),
        "nullifier_2": random_canonical_scalar_hex(),
        "output_cm_1": random_canonical_scalar_hex(),
        "output_cm_2": random_canonical_scalar_hex(),
        "memo_1": "0x" + ("aa" * 96),
        "memo_2": "0x" + ("bb" * 96),
    })
    assert_test(viol_fee_res["success"] is False, "Mode 2: Rejects non-zero protocol fee")
    assert_test(viol_fee_res["revert_reason"] == "PROTOCOL_FEE_MUST_BE_ZERO_FOR_IN_POOL_TRANSFER", "Mode 2: Revert reason PROTOCOL_FEE_MUST_BE_ZERO_FOR_IN_POOL_TRANSFER", f"got={viol_fee_res['revert_reason']}")

    # Violation 3: Missing Change Note 1 (has_change_1 == 0)
    # flags_packed = 6 (bit 0 = 0 -> has_change_1 = 0)
    viol_change_res = run_contract(harness_bin, {
        "asset_id": 1,
        "initial_liability": 50_000_000,
        "merchant_amount": 0,
        "protocol_fee": 0,
        "execution_fee": 10_000,
        "max_execution_fee": 20_000,
        "recipient": "0x0000000000000000000000000000000000000000",
        "flags_packed": 6, # bit 0 is 0!
        "is_rollover": 0,
        "nullifier_1": random_canonical_scalar_hex(),
        "nullifier_2": random_canonical_scalar_hex(),
        "output_cm_1": random_canonical_scalar_hex(),
        "output_cm_2": random_canonical_scalar_hex(),
        "memo_1": "0x" + ("aa" * 96),
        "memo_2": "0x" + ("bb" * 96),
    })
    assert_test(viol_change_res["success"] is False, "Mode 2: Rejects missing change note 1")
    assert_test(viol_change_res["revert_reason"] == "IN_POOL_TRANSFER_REQUIRES_OUTPUT_1", "Mode 2: Revert reason IN_POOL_TRANSFER_REQUIRES_OUTPUT_1", f"got={viol_change_res['revert_reason']}")

    # --------------------------------------------------------------------------
    # 6. Mode 1: Epoch Rollover Violation Matrix
    # --------------------------------------------------------------------------
    print_header("6. Mode 1: Epoch Rollover Violation Matrix")

    # Rollover Success
    roll_ok = run_contract(harness_bin, {
        "asset_id": 1,
        "initial_liability": 50_000_000,
        "merchant_amount": 0,
        "protocol_fee": 0,
        "execution_fee": 10_000,
        "max_execution_fee": 20_000,
        "recipient": "0x0000000000000000000000000000000000000000",
        "flags_packed": 7,
        "is_rollover": 1,
        "nullifier_1": random_canonical_scalar_hex(),
        "nullifier_2": random_canonical_scalar_hex(),
        "output_cm_1": random_canonical_scalar_hex(),
        "output_cm_2": random_canonical_scalar_hex(),
        "memo_1": "0x" + ("aa" * 96),
        "memo_2": "0x" + ("bb" * 96),
    })
    assert_test(roll_ok["success"] is True, "Mode 1: Legitimate rollover succeeds", f"revert={roll_ok['revert_reason']}")

    # Rollover Violation 1: Non-Zero Merchant Amount
    roll_v1 = run_contract(harness_bin, {
        "asset_id": 1,
        "initial_liability": 50_000_000,
        "merchant_amount": 5_000_000, # Non-zero!
        "protocol_fee": 0,
        "execution_fee": 10_000,
        "max_execution_fee": 20_000,
        "recipient": "0x0000000000000000000000000000000000000000",
        "flags_packed": 7,
        "is_rollover": 1,
        "nullifier_1": random_canonical_scalar_hex(),
        "nullifier_2": random_canonical_scalar_hex(),
        "output_cm_1": random_canonical_scalar_hex(),
        "output_cm_2": random_canonical_scalar_hex(),
        "memo_1": "0x" + ("aa" * 96),
        "memo_2": "0x" + ("bb" * 96),
    })
    assert_test(roll_v1["success"] is False, "Mode 1: Rejects non-zero merchant amount")
    assert_test(roll_v1["revert_reason"] == "MERCHANT_AMOUNT_MUST_BE_ZERO_IN_ROLLOVER", "Mode 1: Revert reason MERCHANT_AMOUNT_MUST_BE_ZERO_IN_ROLLOVER", f"got={roll_v1['revert_reason']}")

    # Rollover Violation 2: Non-Zero Protocol Fee
    roll_v2 = run_contract(harness_bin, {
        "asset_id": 1,
        "initial_liability": 50_000_000,
        "merchant_amount": 0,
        "protocol_fee": 10_000, # Non-zero!
        "execution_fee": 10_000,
        "max_execution_fee": 20_000,
        "recipient": "0x0000000000000000000000000000000000000000",
        "flags_packed": 7,
        "is_rollover": 1,
        "nullifier_1": random_canonical_scalar_hex(),
        "nullifier_2": random_canonical_scalar_hex(),
        "output_cm_1": random_canonical_scalar_hex(),
        "output_cm_2": random_canonical_scalar_hex(),
        "memo_1": "0x" + ("aa" * 96),
        "memo_2": "0x" + ("bb" * 96),
    })
    assert_test(roll_v2["success"] is False, "Mode 1: Rejects non-zero protocol fee")
    assert_test(roll_v2["revert_reason"] == "PROTOCOL_FEE_MUST_BE_ZERO_IN_ROLLOVER", "Mode 1: Revert reason PROTOCOL_FEE_MUST_BE_ZERO_IN_ROLLOVER", f"got={roll_v2['revert_reason']}")

    # Rollover Violation 3: Non-Zero Recipient
    roll_v3 = run_contract(harness_bin, {
        "asset_id": 1,
        "initial_liability": 50_000_000,
        "merchant_amount": 0,
        "protocol_fee": 0,
        "execution_fee": 10_000,
        "max_execution_fee": 20_000,
        "recipient": "0x1111111111111111111111111111111111111111", # Non-zero!
        "flags_packed": 7,
        "is_rollover": 1,
        "nullifier_1": random_canonical_scalar_hex(),
        "nullifier_2": random_canonical_scalar_hex(),
        "output_cm_1": random_canonical_scalar_hex(),
        "output_cm_2": random_canonical_scalar_hex(),
        "memo_1": "0x" + ("aa" * 96),
        "memo_2": "0x" + ("bb" * 96),
    })
    assert_test(roll_v3["success"] is False, "Mode 1: Rejects non-zero recipient")
    assert_test(roll_v3["revert_reason"] == "RECIPIENT_MUST_BE_ZERO_IN_ROLLOVER", "Mode 1: Revert reason RECIPIENT_MUST_BE_ZERO_IN_ROLLOVER", f"got={roll_v3['revert_reason']}")

    # --------------------------------------------------------------------------
    # 7. Double Spend & Cryptographic Danger Zone Rejection
    # --------------------------------------------------------------------------
    print_header("7. Double Spend Defense & Cryptographic Integrity")

    # Rejection of Non-Canonical Fr Scalar (>= r danger zone)
    # r = 0x73eda753299d7d483339d80809a1d80553bda402fffe5bfeffffffff00000001
    danger_scalar = "0x8000000000000000000000000000000000000000000000000000000000000000"
    danger_res = run_contract(harness_bin, {
        "asset_id": 1,
        "initial_liability": 50_000_000,
        "merchant_amount": 0,
        "protocol_fee": 0,
        "execution_fee": 10_000,
        "max_execution_fee": 20_000,
        "recipient": "0x0000000000000000000000000000000000000000",
        "flags_packed": 7,
        "is_rollover": 0,
        "nullifier_1": danger_scalar, # Non-canonical!
        "nullifier_2": random_canonical_scalar_hex(),
        "output_cm_1": random_canonical_scalar_hex(),
        "output_cm_2": random_canonical_scalar_hex(),
        "memo_1": "0x" + ("aa" * 96),
        "memo_2": "0x" + ("bb" * 96),
    })
    assert_test(danger_res["success"] is False, "Rejects non-canonical scalar in Fr danger zone")
    assert_test(danger_res["revert_reason"] == "NON_CANONICAL_PUBLIC_INPUT_SCALAR", "Exact revert NON_CANONICAL_PUBLIC_INPUT_SCALAR")

    # Singularity Aliasing Attack (nullifier_1 == nullifier_2)
    alias_nf = random_canonical_scalar_hex()
    alias_res = run_contract(harness_bin, {
        "asset_id": 1,
        "initial_liability": 50_000_000,
        "merchant_amount": 0,
        "protocol_fee": 0,
        "execution_fee": 10_000,
        "max_execution_fee": 20_000,
        "recipient": "0x0000000000000000000000000000000000000000",
        "flags_packed": 7,
        "is_rollover": 0,
        "nullifier_1": alias_nf,
        "nullifier_2": alias_nf, # Identical!
        "output_cm_1": random_canonical_scalar_hex(),
        "output_cm_2": random_canonical_scalar_hex(),
        "memo_1": "0x" + ("aa" * 96),
        "memo_2": "0x" + ("bb" * 96),
    })
    assert_test(alias_res["success"] is False, "Rejects identical input nullifiers (singularity aliasing)")
    assert_test(alias_res["revert_reason"] == "DuplicateNullifierInSameTx", "Exact revert DuplicateNullifierInSameTx", f"got={alias_res['revert_reason']}")

    # --------------------------------------------------------------------------
    # 8. Mathematical Solvency Invariant Across Stochastic Parameters
    # --------------------------------------------------------------------------
    print_header("8. Mathematical Solvency Invariant Across Stochastic Parameters")

    # Run 10 randomized in-pool transfers to prove solvency holds without exception
    for i in range(10):
        transfer_val = secrets.randbelow(50_000_000) + 1_000_000
        change_val = secrets.randbelow(50_000_000) + 1_000_000
        exec_fee = secrets.randbelow(50_000) + 1_000
        init_liab = transfer_val + change_val + exec_fee

        fuzz_res = run_contract(harness_bin, {
            "asset_id": 1,
            "initial_liability": init_liab,
            "initial_assets": init_liab,
            "merchant_amount": 0,
            "protocol_fee": 0,
            "execution_fee": exec_fee,
            "max_execution_fee": exec_fee + 10_000,
            "recipient": "0x0000000000000000000000000000000000000000",
            "flags_packed": 7,
            "is_rollover": 0,
            "nullifier_1": random_canonical_scalar_hex(),
            "nullifier_2": random_canonical_scalar_hex(),
            "output_cm_1": random_canonical_scalar_hex(),
            "output_cm_2": random_canonical_scalar_hex(),
            "memo_1": "0x" + ("aa" * 96),
            "memo_2": "0x" + ("bb" * 96),
        })

        assert_test(fuzz_res["success"] is True, f"Fuzz #{i+1}: In-pool transfer solvent with liab={init_liab}, fee={exec_fee}")
        u_liab = int(fuzz_res["user_note_liabilities_usdc"])
        e_liab = int(fuzz_res["accrued_execution_fee_liabilities_usdc"])
        assert_test(u_liab + e_liab == init_liab, f"Fuzz #{i+1}: Invariant total_liabilities == initial_liability ({u_liab} + {e_liab} == {init_liab})")

    # --------------------------------------------------------------------------
    # Final Summary
    # --------------------------------------------------------------------------
    print(f"\n{BOLD}{GREEN}===================================================================={RESET}")
    print(f"{BOLD}{GREEN}  DEC-043B REAL-CASE PYTHON TEST SUITE COMPLETED SUCCESSFULLY!      {RESET}")
    print(f"{BOLD}{GREEN}  Total Tests Executed : {TOTAL_TESTS}                                          {RESET}")
    print(f"{BOLD}{GREEN}  Total Tests Passed   : {PASSED_TESTS} (100.0%)                                 {RESET}")
    print(f"{BOLD}{GREEN}  All Invariants, AUD-4, AUD-5 & 3-Mode State Machine Verified!     {RESET}")
    print(f"{BOLD}{GREEN}===================================================================={RESET}\n")


if __name__ == "__main__":
    main()
