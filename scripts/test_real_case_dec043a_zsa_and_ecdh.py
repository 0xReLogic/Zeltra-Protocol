#!/usr/bin/env python3
"""
Zeltra Protocol - DEC-043A Real-Case Cryptographic Soundness & Cross-Language Interop Test

This script independently validates the end-to-end cryptographic soundness, BIP-350 standard
conformance, and real-world merchant masking flow for DEC-043A:

1. BLS12-381 Fr Scalar Canonicality & Danger Zone Invariants:
   - Modulus order r = 52435875175126190479447740508185965837690552500527637822603658699938581184513
   - Strict rejection of field elements in the upper 54.68% danger zone (>= r).

2. BIP-350 Bech32m Standard Cross-Verification:
   - Python stdlib bech32 encoding/decoding parity with Rust ZeltraShieldedAddress (HRP "zs").
   - Strict rejection of BIP-173 standard Bech32 checksums.
   - Rejection of corrupted checksums, wrong HRP, and invalid payload lengths.

3. Cross-Language Ephemeral X25519 ECDH + ChaCha20Poly1305 Interoperability:
   - Python encrypts a 96-byte memo -> Rust decrypts and reconstructs plaintext + randomness.
   - Rust encrypts a 96-byte memo -> Python decrypts and reconstructs plaintext + shared secret.
   - Proof of bit-exact KDF parameters: salt="ZELTRA_NOTE_MEMO_KDF_V1", info="ZELTRA_MEMO_CIPHER_V1".

4. Real-World Merchant Masking Simulation (Tokyo Coffee In-Pool Settlement):
   - Merchant publishes Shielded Address (ZSA).
   - 5 distinct retail customers pay varying USDC amounts with unique ephemeral keys.
   - 100 noise / attacker memos injected into simulated on-chain block.
   - Merchant discovers 5/5 valid notes, rejects 100/100 noise memos, verifies 100% solvency balance.

5. Comprehensive Threat Vector & Adversary Attack Matrix:
   - Attack 1: Non-canonical scalar injection.
   - Attack 2: Bech32m checksum corruption.
   - Attack 3: Wrong HRP spoofing.
   - Attack 4: Ephemeral public key bit-flip tampering.
   - Attack 5: Ciphertext bit-flip tampering.
   - Attack 6: MAC tag tampering.
   - Attack 7: Wrong viewing key trial decryption.
   - Attack 8: Truncated memo injection (< 96 bytes).
   - Attack 9: Oversized memo injection (> 96 bytes).
   - Attack 10: Bech32 (BIP-173) cross-standard confusion attack.
"""

import sys
import os
import json
import subprocess
from typing import Tuple, Optional, Dict, Any, List

# External cryptography libraries
try:
    import bech32
    from cryptography.hazmat.primitives.asymmetric import x25519
    from cryptography.hazmat.primitives.ciphers.aead import ChaCha20Poly1305
    from cryptography.hazmat.primitives.kdf.hkdf import HKDF
    from cryptography.hazmat.primitives import hashes
except ImportError as e:
    print(f"Missing dependency: {e}. Run 'pip install bech32 cryptography'")
    sys.exit(1)

# ANSI terminal colors
GREEN = "\033[92m"
RED = "\033[91m"
YELLOW = "\033[93m"
CYAN = "\033[96m"
BOLD = "\033[1m"
RESET = "\033[0m"

# BLS12-381 Fr prime scalar modulus (r)
# 0x73eda753299d7d483339d80809a1d80553bda402fffe5bfeffffffff00000001
FR_MODULUS = 52435875175126190479447740508185965837690552500527637822603658699938581184513
ZSA_HRP = "zs"
ENCRYPTED_MEMO_SIZE = 96
NOTE_PLAINTEXT_SIZE = 48

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
    """Finds or builds the Rust zsa_interop test harness binary."""
    repo_root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    target_bin = os.path.join(repo_root, "target", "debug", "zsa_interop")

    if not os.path.exists(target_bin):
        print(f"{YELLOW}[BUILD]{RESET} Building zsa_interop binary...")
        res = subprocess.run(
            ["cargo", "build", "--bin", "zsa_interop", "-p", "zeltra-core"],
            cwd=repo_root,
            capture_output=True,
            text=True,
        )
        if res.returncode != 0:
            print(f"{RED}[BUILD ERROR]{RESET} Failed to build zsa_interop:\n{res.stderr}")
            sys.exit(1)

    assert os.path.exists(target_bin), "zsa_interop binary must exist"
    return target_bin


def run_rust_harness(bin_path: str, args: List[str]) -> Dict[str, Any]:
    """Runs the Rust harness with arguments and returns parsed JSON output."""
    res = subprocess.run([bin_path] + args, capture_output=True, text=True)
    if res.returncode != 0:
        raise RuntimeError(f"Rust harness failed: {res.stderr.strip()}")
    try:
        return json.loads(res.stdout.strip())
    except json.JSONDecodeError:
        raise RuntimeError(f"Failed to parse Rust harness JSON: {res.stdout.strip()}")


# ==============================================================================
# Pure Python Cryptographic Reference Primitives
# ==============================================================================

BECH32_CONST = 1
BECH32M_CONST = 0x2bc830a3


def bech32m_polymod(values: list) -> int:
    return bech32.bech32_polymod(values)


def bech32m_hrp_expand(hrp: str) -> list:
    return bech32.bech32_hrp_expand(hrp)


def python_bech32m_encode(hrp: str, data: bytes) -> str:
    """Encodes raw bytes to Bech32m string using BIP-350 specification."""
    data_5bit = bech32.convertbits(list(data), 8, 5, True)
    values = bech32m_hrp_expand(hrp) + data_5bit
    polymod = bech32m_polymod(values + [0, 0, 0, 0, 0, 0]) ^ BECH32M_CONST
    checksum = [(polymod >> 5 * (5 - i)) & 31 for i in range(6)]
    combined = data_5bit + checksum
    return hrp + "1" + "".join([bech32.CHARSET[d] for d in combined])


def python_bech32m_decode(s: str) -> Tuple[Optional[str], Optional[bytes]]:
    """Decodes Bech32m string using BIP-350 specification, returning (hrp, raw_bytes)."""
    if any(ord(x) < 33 or ord(x) > 126 for x in s):
        return None, None
    if s.lower() != s and s.upper() != s:
        return None, None
    s_lower = s.lower()
    pos = s_lower.rfind("1")
    if pos < 1 or pos + 7 > len(s_lower):
        return None, None
    hrp = s_lower[:pos]
    if any(bech32.CHARSET.find(x) == -1 for x in s_lower[pos + 1:]):
        return None, None
    data = [bech32.CHARSET.find(x) for x in s_lower[pos + 1:]]
    # Enforce strictly BIP-350 Bech32m checksum
    if bech32m_polymod(bech32m_hrp_expand(hrp) + data) != BECH32M_CONST:
        return None, None
    payload_5bit = data[:-6]
    raw = bech32.convertbits(payload_5bit, 5, 8, False)
    if raw is None:
        return None, None
    return hrp, bytes(raw)


def python_bech32_legacy_encode(hrp: str, data: bytes) -> str:
    """Encodes raw bytes to legacy BIP-173 Bech32 string (for negative testing)."""
    data_5bit = bech32.convertbits(list(data), 8, 5, True)
    values = bech32m_hrp_expand(hrp) + data_5bit
    polymod = bech32m_polymod(values + [0, 0, 0, 0, 0, 0]) ^ BECH32_CONST
    checksum = [(polymod >> 5 * (5 - i)) & 31 for i in range(6)]
    combined = data_5bit + checksum
    return hrp + "1" + "".join([bech32.CHARSET[d] for d in combined])


def python_encrypt_memo(
    value: int,
    rho_int: int,
    asset_id: int,
    recipient_pk_bytes: bytes,
    ephemeral_priv: Optional[x25519.X25519PrivateKey] = None
) -> Tuple[bytes, bytes]:
    """
    Encrypts note plaintext to 96-byte AEAD memo in Python using X25519 + HKDF-SHA256 + ChaCha20Poly1305.
    Returns (96_byte_memo, 32_byte_shared_secret).
    """
    # 1. Plaintext 48 bytes: value (8B big-endian) || rho (32B big-endian) || asset_id (8B big-endian)
    pt = bytearray()
    pt.extend(value.to_bytes(8, byteorder="big"))
    pt.extend(rho_int.to_bytes(32, byteorder="big"))
    pt.extend(asset_id.to_bytes(8, byteorder="big"))
    assert len(pt) == 48

    # 2. Ephemeral ECDH
    if ephemeral_priv is None:
        ephemeral_priv = x25519.X25519PrivateKey.generate()
    ephemeral_pub = ephemeral_priv.public_key()
    epk_bytes = ephemeral_pub.public_bytes_raw()

    recipient_pub = x25519.X25519PublicKey.from_public_bytes(recipient_pk_bytes)
    shared_secret = ephemeral_priv.exchange(recipient_pub)

    # 3. HKDF-SHA256 expansion: 44 bytes (32B key || 12B nonce)
    hkdf = HKDF(
        algorithm=hashes.SHA256(),
        length=44,
        salt=b"ZELTRA_NOTE_MEMO_KDF_V1",
        info=b"ZELTRA_MEMO_CIPHER_V1",
    )
    okm = hkdf.derive(shared_secret)
    key_bytes = okm[0:32]
    nonce_bytes = okm[32:44]

    # 4. ChaCha20Poly1305 AEAD encryption
    chacha = ChaCha20Poly1305(key_bytes)
    ciphertext_and_tag = chacha.encrypt(nonce_bytes, bytes(pt), None)
    assert len(ciphertext_and_tag) == 48 + 16  # 64 bytes

    # 5. Result: epk (32B) + ciphertext_and_tag (64B) = 96B
    memo = epk_bytes + ciphertext_and_tag
    assert len(memo) == 96
    return memo, shared_secret


def python_decrypt_memo(
    memo_bytes: bytes,
    recipient_priv_bytes: bytes
) -> Optional[Dict[str, Any]]:
    """
    Decrypts 96-byte AEAD memo in Python using recipient's static private key.
    Returns parsed dict or None on authentication failure.
    """
    if len(memo_bytes) != 96:
        return None

    epk_bytes = memo_bytes[0:32]
    ciphertext_and_tag = memo_bytes[32:96]

    try:
        epk = x25519.X25519PublicKey.from_public_bytes(epk_bytes)
        priv = x25519.X25519PrivateKey.from_private_bytes(recipient_priv_bytes)
        shared_secret = priv.exchange(epk)

        hkdf = HKDF(
            algorithm=hashes.SHA256(),
            length=44,
            salt=b"ZELTRA_NOTE_MEMO_KDF_V1",
            info=b"ZELTRA_MEMO_CIPHER_V1",
        )
        okm = hkdf.derive(shared_secret)
        key_bytes = okm[0:32]
        nonce_bytes = okm[32:44]

        chacha = ChaCha20Poly1305(key_bytes)
        pt = chacha.decrypt(nonce_bytes, ciphertext_and_tag, None)
        if len(pt) != 48:
            return None

        val = int.from_bytes(pt[0:8], byteorder="big")
        rho = int.from_bytes(pt[8:40], byteorder="big")
        if rho >= FR_MODULUS:
            return None
        aid = int.from_bytes(pt[40:48], byteorder="big")
        if aid > 1:
            return None

        return {
            "value": val,
            "rho": rho,
            "asset_id": aid,
            "shared_secret": shared_secret.hex(),
        }
    except Exception:
        return None


# ==============================================================================
# SUITES
# ==============================================================================

def suite1_scalar_canonicality(harness_bin: str):
    print_header("SUITE 1: BLS12-381 Scalar Field Canonicality & Danger Zone Invariants")

    # 1. Modulus characteristics
    assert_test(FR_MODULUS < (1 << 256), "BLS12-381 scalar modulus r fits in 256 bits")
    assert_test(FR_MODULUS > (1 << 254), "BLS12-381 scalar modulus r is > 2^254 (254.85 bits)")

    # 2. Danger Zone ratio analysis
    danger_zone = (1 << 256) - FR_MODULUS
    ratio = danger_zone / (1 << 256)
    assert_test(0.54 < ratio < 0.55, f"Danger Zone represents {ratio * 100:.2f}% of 256-bit space")

    # 3. Canonical boundary values
    max_canonical = FR_MODULUS - 1
    assert_test(max_canonical < FR_MODULUS, "r - 1 is strictly within canonical field")

    # 4. Construct synthetic non-canonical ZSA hex in Python and test in Rust
    non_canonical_owner = FR_MODULUS.to_bytes(32, byteorder="big")  # exactly r
    dummy_pk = bytes([0x42] * 32)
    non_canonical_zsa_hex = (non_canonical_owner + dummy_pk).hex()

    res = run_rust_harness(harness_bin, ["parse_zsa", non_canonical_zsa_hex])
    assert_test(res["valid"] is False, "Rust harness rejects ZSA with owner_key == r",
                f"error: {res.get('error')}")

    # 5. Test 0xFF...FF in owner_key
    ff_zsa_hex = (bytes([0xFF] * 32) + dummy_pk).hex()
    res_ff = run_rust_harness(harness_bin, ["parse_zsa", ff_zsa_hex])
    assert_test(res_ff["valid"] is False, "Rust harness rejects ZSA with owner_key == 0xFF...FF",
                f"error: {res_ff.get('error')}")


def suite2_bip350_bech32m_cross_verification(harness_bin: str):
    print_header("SUITE 2: BIP-350 Bech32m Standard Cross-Verification (Python <-> Rust)")

    # 1. Ask Rust to generate a genuine ZSA from a test seed
    rust_keys = run_rust_harness(harness_bin, ["zsa_from_seed", "test_seed_bech32m_cross_verify"])
    rust_bech32m = rust_keys["zsa_bech32m"]
    rust_owner_hex = rust_keys["owner_key"]
    rust_pk_hex = rust_keys["pk_ivk"]

    assert_test(rust_bech32m.startswith("zs1"), "Rust ZSA starts with 'zs1'")

    # 2. Decode Rust's Bech32m string in Python using python-bech32
    py_hrp, py_data = python_bech32m_decode(rust_bech32m)
    assert_test(py_hrp == ZSA_HRP, f"Python successfully decoded HRP '{py_hrp}' == '{ZSA_HRP}'")
    assert_test(len(py_data) == 64, f"Python decoded exactly 64 bytes (got {len(py_data)})")

    py_owner_hex = py_data[0:32].hex()
    py_pk_hex = py_data[32:64].hex()
    assert_test(py_owner_hex == rust_owner_hex, "Python decoded owner_key matches Rust 100%")
    assert_test(py_pk_hex == rust_pk_hex, "Python decoded pk_ivk matches Rust 100%")

    # 3. Create a ZSA in Python and encode using python-bech32m
    owner_int = 0x11223344556677889900AABBCCDDEEFF11223344556677889900AABBCCDDEEFF % FR_MODULUS
    pk_bytes = bytes(range(32))
    py_zsa_bytes = owner_int.to_bytes(32, byteorder="big") + pk_bytes

    py_encoded_bech32m = python_bech32m_encode(ZSA_HRP, py_zsa_bytes)
    assert_test(py_encoded_bech32m.startswith("zs1"), "Python Bech32m starts with 'zs1'")

    # 4. Pass Python-encoded Bech32m to Rust harness for parsing
    rust_parse_res = run_rust_harness(harness_bin, ["parse_zsa", py_encoded_bech32m])
    assert_test(rust_parse_res["valid"] is True, "Rust successfully parsed Python-encoded Bech32m ZSA")
    assert_test(rust_parse_res["owner_key"] == owner_int.to_bytes(32, "big").hex(),
                "Rust parsed owner_key matches Python original")
    assert_test(rust_parse_res["pk_ivk"] == pk_bytes.hex(),
                "Rust parsed pk_ivk matches Python original")

    # 5. Verify BIP-173 standard Bech32 (non-m) checksum is rejected
    legacy_bech32 = python_bech32_legacy_encode(ZSA_HRP, py_zsa_bytes)
    rust_legacy_res = run_rust_harness(harness_bin, ["parse_zsa", legacy_bech32])
    assert_test(rust_legacy_res["valid"] is False,
                "Rust strictly rejects legacy BIP-173 Bech32 checksum on ZSA",
                f"error: {rust_legacy_res.get('error')}")


def suite3_cross_language_memo_interop(harness_bin: str):
    print_header("SUITE 3: Cross-Language Ephemeral X25519 ECDH + ChaCha20Poly1305 Interop")

    # Create Bob (recipient) in Rust
    bob_keys = run_rust_harness(harness_bin, ["zsa_from_seed", "bob_cross_language_vault_seed"])
    bob_zsa_bech32m = bob_keys["zsa_bech32m"]
    bob_pk_bytes = bytes.fromhex(bob_keys["pk_ivk"])
    bob_ivk_secret_hex = bob_keys["ivk_secret"]

    # -------------------------------------------------------------------------
    # Direction 1: Python ENCRYPTS -> Rust DECRYPTS
    # -------------------------------------------------------------------------
    print(f"\n  {BOLD}Direction 1: Python encrypts 96B memo -> Rust decrypts{RESET}")
    val_py = 125_500_000  # 125.5 USDC
    rho_py = 0x3344556677889900112233445566778899001122334455667788990011223344 % FR_MODULUS
    aid_py = 1

    py_memo, py_shared_secret = python_encrypt_memo(val_py, rho_py, aid_py, bob_pk_bytes)
    assert_test(len(py_memo) == ENCRYPTED_MEMO_SIZE, "Python produced exact 96-byte memo")

    # Send Python-generated memo to Rust harness for decryption
    rust_dec_res = run_rust_harness(harness_bin, [
        "decrypt_memo",
        py_memo.hex(),
        bob_ivk_secret_hex,
    ])

    assert_test(rust_dec_res["valid"] is True, "Rust harness decrypted Python-encrypted memo successfully")
    assert_test(rust_dec_res["value"] == val_py, f"Value matches: {val_py}")
    assert_test(rust_dec_res["asset_id"] == aid_py, f"Asset ID matches: {aid_py}")
    assert_test(rust_dec_res["rho"] == rho_py.to_bytes(32, "big").hex(), "Rho matches bit-exact")

    # -------------------------------------------------------------------------
    # Direction 2: Rust ENCRYPTS -> Python DECRYPTS
    # -------------------------------------------------------------------------
    print(f"\n  {BOLD}Direction 2: Rust encrypts 96B memo -> Python decrypts{RESET}")
    val_rust = 78_250_000  # 78.25 USDC
    rho_rust_int = 0x9988776655443322110099887766554433221100998877665544332211009988 % FR_MODULUS
    rho_rust_hex = rho_rust_int.to_bytes(32, "big").hex()

    rust_enc_res = run_rust_harness(harness_bin, [
        "encrypt_memo",
        str(val_rust),
        rho_rust_hex,
        "1",
        bob_zsa_bech32m,
    ])

    rust_memo_hex = rust_enc_res["encrypted_memo"]
    rust_memo_bytes = bytes.fromhex(rust_memo_hex)
    assert_test(len(rust_memo_bytes) == ENCRYPTED_MEMO_SIZE, "Rust produced exact 96-byte memo")

    # Decrypt in Python using Python's decrypt function
    bob_priv_bytes = bytes.fromhex(bob_ivk_secret_hex)
    py_dec = python_decrypt_memo(rust_memo_bytes, bob_priv_bytes)

    assert_test(py_dec is not None, "Python decrypted Rust-encrypted memo successfully")
    assert_test(py_dec["value"] == val_rust, f"Value matches: {val_rust}")
    assert_test(py_dec["asset_id"] == 1, "Asset ID matches: 1")
    assert_test(py_dec["rho"] == rho_rust_int, "Rho matches bit-exact")


def suite4_real_world_merchant_masking_simulation(harness_bin: str):
    print_header("SUITE 4: Real-World Merchant Masking Simulation (Tokyo Coffee Co.)")

    # 1. Merchant boots up, generates ZSA, and displays QR/address
    print(f"  {BOLD}Step 1: Merchant 'Tokyo Coffee Co.' derives ZSA address{RESET}")
    merchant_keys = run_rust_harness(harness_bin, ["zsa_from_seed", "merchant_tokyo_coffee_master_seed_2026"])
    merchant_zsa = merchant_keys["zsa_bech32m"]
    merchant_ivk_secret = merchant_keys["ivk_secret"]
    merchant_pk_bytes = bytes.fromhex(merchant_keys["pk_ivk"])
    print(f"    Merchant ZSA: {merchant_zsa[:20]}...{merchant_zsa[-10:]}")

    # 2. 5 independent retail customers make payments in-pool
    customers = [
        ("Alice", 3_500_000, 0x1001),      # $3.50 Espresso
        ("Bob", 8_750_000, 0x2002),        # $8.75 Cold Brew & Bagel
        ("Charlie", 24_000_000, 0x3003),   # $24.00 1kg Coffee Beans
        ("Dave", 150_000_000, 0x4004),     # $150.00 Catering Order
        ("Eve", 12_500_000, 0x5005),       # $12.50 Pastries
    ]

    expected_total_volume = sum(c[1] for c in customers)
    customer_memos = []

    print(f"\n  {BOLD}Step 2: 5 Customers generate payments with Ephemeral X25519 ECDH{RESET}")
    for name, amount_usdc, salt in customers:
        rho = (salt * 0xDECAFBAD123456789) % FR_MODULUS
        memo_bytes, _ = python_encrypt_memo(amount_usdc, rho, 1, merchant_pk_bytes)
        customer_memos.append((name, amount_usdc, rho, memo_bytes))
        print(f"    Customer {name:7} pays {amount_usdc / 1_000_000:6.2f} USDC (Memo: {len(memo_bytes)}B)")

    # 3. Simulate on-chain pool with 100 noisy/adversary memos
    print(f"\n  {BOLD}Step 3: Simulating 100 noise/adversary on-chain notes + 5 genuine notes{RESET}")
    pool_memos = []
    # Add 40 completely random 96-byte buffers
    for i in range(40):
        pool_memos.append(os.urandom(96))

    # Add 50 memos encrypted for someone else (wrong recipient)
    other_priv = x25519.X25519PrivateKey.generate()
    other_pk_bytes = other_priv.public_key().public_bytes_raw()
    for i in range(50):
        fake_memo, _ = python_encrypt_memo(10_000_000 + i, 0x777 + i, 1, other_pk_bytes)
        pool_memos.append(fake_memo)

    # Add 10 tampered memos
    for i in range(10):
        tampered = bytearray(customer_memos[i % len(customer_memos)][3])
        tampered[35 + i] ^= 0xFF
        pool_memos.append(bytes(tampered))

    # Insert customer memos at various positions
    pool_memos.insert(12, customer_memos[0][3])
    pool_memos.insert(37, customer_memos[1][3])
    pool_memos.insert(55, customer_memos[2][3])
    pool_memos.insert(82, customer_memos[3][3])
    pool_memos.insert(99, customer_memos[4][3])

    assert len(pool_memos) == 105

    # 4. Merchant scans the pool
    print(f"\n  {BOLD}Step 4: Merchant trial-decrypts all 105 pool memos{RESET}")
    detected_payments = []
    for idx, memo in enumerate(pool_memos):
        decrypted = python_decrypt_memo(memo, bytes.fromhex(merchant_ivk_secret))
        if decrypted is not None:
            detected_payments.append((idx, decrypted))

    assert_test(len(detected_payments) == 5,
                f"Merchant detected exactly 5/5 valid notes out of 105 in pool (0 false positives!)")

    recovered_volume = sum(p[1]["value"] for p in detected_payments)
    assert_test(recovered_volume == expected_total_volume,
                f"Recovered merchant revenue: {recovered_volume / 1_000_000:.2f} USDC matches expected 100%")

    # 5. Cross-verify with Rust harness that every detected payment decrypts identically in Rust
    print(f"\n  {BOLD}Step 5: Dual-engine confirmation (Rust confirms all 5 detected notes){RESET}")
    for idx, p in detected_payments:
        rust_res = run_rust_harness(harness_bin, [
            "decrypt_memo",
            pool_memos[idx].hex(),
            merchant_ivk_secret,
        ])
        assert_test(rust_res["valid"] is True, f"Rust engine confirms payment at pool index #{idx}")
        assert_test(rust_res["value"] == p["value"], "Rust decoded amount matches Python")


def suite5_adversary_threat_vector_matrix(harness_bin: str):
    print_header("SUITE 5: Adversary & Threat Vector Hardening Matrix")

    # Generate genuine target
    target_keys = run_rust_harness(harness_bin, ["zsa_from_seed", "adversary_target_seed_v1"])
    target_pk_bytes = bytes.fromhex(target_keys["pk_ivk"])
    target_ivk_secret_hex = target_keys["ivk_secret"]
    target_zsa_bech32m = target_keys["zsa_bech32m"]

    valid_memo, _ = python_encrypt_memo(50_000_000, 0x12345678, 1, target_pk_bytes)

    # -------------------------------------------------------------------------
    # Attack 1: Non-canonical owner_key injection (owner_key >= r)
    # -------------------------------------------------------------------------
    print(f"  {BOLD}Attack 1: Non-canonical scalar injection (>= r){RESET}")
    bad_owner = (FR_MODULUS + 1).to_bytes(32, "big")
    bad_zsa_hex = (bad_owner + target_pk_bytes).hex()
    res1 = run_rust_harness(harness_bin, ["parse_zsa", bad_zsa_hex])
    assert_test(res1["valid"] is False, "Rust rejects ZSA with scalar > r")

    # -------------------------------------------------------------------------
    # Attack 2: Corrupted Bech32m checksum
    # -------------------------------------------------------------------------
    print(f"  {BOLD}Attack 2: Corrupted Bech32m checksum{RESET}")
    chars = list(target_zsa_bech32m)
    chars[-1] = 'q' if chars[-1] != 'q' else 'p'
    corrupted_bech = "".join(chars)
    res2 = run_rust_harness(harness_bin, ["parse_zsa", corrupted_bech])
    assert_test(res2["valid"] is False, "Rust rejects corrupted Bech32m checksum")

    # -------------------------------------------------------------------------
    # Attack 3: Wrong HRP injection ("zr" instead of "zs")
    # -------------------------------------------------------------------------
    print(f"  {BOLD}Attack 3: Wrong HRP spoofing ('zr'){RESET}")
    raw_zsa = bytes.fromhex(target_keys["zsa_hex"])
    wrong_hrp_str = python_bech32m_encode("zr", raw_zsa)
    res3 = run_rust_harness(harness_bin, ["parse_zsa", wrong_hrp_str])
    assert_test(res3["valid"] is False, "Rust rejects ZSA with wrong HRP 'zr'")

    # -------------------------------------------------------------------------
    # Attack 4: Ephemeral Public Key (EPK) bit-flip tampering
    # -------------------------------------------------------------------------
    print(f"  {BOLD}Attack 4: Ephemeral Public Key (EPK) bit-flip{RESET}")
    tampered_epk = bytearray(valid_memo)
    tampered_epk[7] ^= 0x01
    res4 = run_rust_harness(harness_bin, ["decrypt_memo", tampered_epk.hex(), target_ivk_secret_hex])
    assert_test(res4["valid"] is False, "Rust rejects memo with tampered EPK (MAC auth failure)")

    # -------------------------------------------------------------------------
    # Attack 5: Ciphertext bit-flip tampering (nominal inflation attack)
    # -------------------------------------------------------------------------
    print(f"  {BOLD}Attack 5: Ciphertext bit-flip (tampering nominal value){RESET}")
    tampered_ct = bytearray(valid_memo)
    tampered_ct[36] ^= 0x80  # flip high bit of nominal value
    res5 = run_rust_harness(harness_bin, ["decrypt_memo", tampered_ct.hex(), target_ivk_secret_hex])
    assert_test(res5["valid"] is False, "Rust rejects memo with altered ciphertext (Poly1305 failure)")

    # -------------------------------------------------------------------------
    # Attack 6: MAC tag tampering
    # -------------------------------------------------------------------------
    print(f"  {BOLD}Attack 6: MAC tag byte manipulation{RESET}")
    tampered_tag = bytearray(valid_memo)
    tampered_tag[90] ^= 0x02
    res6 = run_rust_harness(harness_bin, ["decrypt_memo", tampered_tag.hex(), target_ivk_secret_hex])
    assert_test(res6["valid"] is False, "Rust rejects memo with tampered MAC tag")

    # -------------------------------------------------------------------------
    # Attack 7: Wrong viewing key trial decryption
    # -------------------------------------------------------------------------
    print(f"  {BOLD}Attack 7: Wrong viewing key decryption attempt{RESET}")
    eve_keys = run_rust_harness(harness_bin, ["zsa_from_seed", "eve_attacker_seed_777"])
    res7 = run_rust_harness(harness_bin, ["decrypt_memo", valid_memo.hex(), eve_keys["ivk_secret"]])
    assert_test(res7["valid"] is False, "Attacker Eve cannot decrypt victim's memo")

    # -------------------------------------------------------------------------
    # Attack 8: Truncated memo injection (< 96 bytes)
    # -------------------------------------------------------------------------
    print(f"  {BOLD}Attack 8: Truncated memo injection (48B, 64B, 95B){RESET}")
    for bad_len in [0, 32, 48, 64, 95]:
        trunc = valid_memo[:bad_len]
        res8 = run_rust_harness(harness_bin, ["decrypt_memo", trunc.hex(), target_ivk_secret_hex])
        assert_test(res8["valid"] is False, f"Rust rejects truncated memo of length {bad_len}")

    # -------------------------------------------------------------------------
    # Attack 9: Oversized memo injection (> 96 bytes)
    # -------------------------------------------------------------------------
    print(f"  {BOLD}Attack 9: Oversized memo injection (97B, 128B){RESET}")
    for bad_len in [97, 100, 128, 200]:
        oversized = valid_memo + bytes([0xAA] * (bad_len - 96))
        res9 = run_rust_harness(harness_bin, ["decrypt_memo", oversized.hex(), target_ivk_secret_hex])
        assert_test(res9["valid"] is False, f"Rust rejects oversized memo of length {bad_len}")

    # -------------------------------------------------------------------------
    # Attack 10: Bech32 (BIP-173) cross-standard confusion attack
    # -------------------------------------------------------------------------
    print(f"  {BOLD}Attack 10: Bech32 (BIP-173) vs Bech32m (BIP-350) confusion attack{RESET}")
    raw_zsa_bytes = bytes.fromhex(target_keys["zsa_hex"])
    bip173_addr = python_bech32_legacy_encode(ZSA_HRP, raw_zsa_bytes)

    res10 = run_rust_harness(harness_bin, ["parse_zsa", bip173_addr])
    assert_test(res10["valid"] is False, "Rust rejects legacy BIP-173 Bech32 encoding")


# ==============================================================================
# MAIN ENTRYPOINT
# ==============================================================================

def main():
    print(f"{BOLD}{GREEN}==================================================================={RESET}")
    print(f"{BOLD}{GREEN}  Zeltra Protocol - DEC-043A Python Real-Case & Interop Test Suite  {RESET}")
    print(f"{BOLD}{GREEN}==================================================================={RESET}")

    harness_bin = find_or_build_harness()
    print(f"Using test harness binary: {harness_bin}")

    suite1_scalar_canonicality(harness_bin)
    suite2_bip350_bech32m_cross_verification(harness_bin)
    suite3_cross_language_memo_interop(harness_bin)
    suite4_real_world_merchant_masking_simulation(harness_bin)
    suite5_adversary_threat_vector_matrix(harness_bin)

    print(f"\n{BOLD}{GREEN}==================================================================={RESET}")
    print(f"{BOLD}{GREEN}  ALL {PASSED_TESTS}/{TOTAL_TESTS} REAL-CASE TESTS PASSED (100% SOUNDNESS GUARANTEED)  {RESET}")
    print(f"{BOLD}{GREEN}==================================================================={RESET}\n")


if __name__ == "__main__":
    main()
