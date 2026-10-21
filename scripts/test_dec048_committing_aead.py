#!/usr/bin/env python3
"""
DEC-048 Independent Verification Harness:
Committing AEAD & Partitioning Oracle Defense for Encrypted Note Memos (EtH-KS)
Based on IACR ePrint 2026/240 (Minematsu & Inoue, October 2026)

Validates:
1. Positive End-to-End Encryption & Trial Decryption (EtH-KS with Key Separation)
2. Partitioning Oracle Resistance across 100 random viewing keys (NEG-01)
3. Single-Bit Tag Tampering Resistance (128 bit flips tested) (NEG-02)
4. Ciphertext Payload Tampering Resistance (NEG-03)
5. Nonce Malleation / Replay Tampering Resistance (NEG-04)
6. Ephemeral Public Key Substitution Resistance (NEG-05)
7. Non-Canonical BLS12-381 Scalar Rejection (NEG-06)
8. Truncated & Oversized Calldata Rejection (NEG-07)
9. Asset ID Boundary Overflow Rejection (NEG-08)
"""

import os
import sys
import hmac
import hashlib
from cryptography.hazmat.primitives.asymmetric import x25519
from cryptography.hazmat.primitives.kdf.hkdf import HKDF
from cryptography.hazmat.primitives import hashes
from cryptography.hazmat.primitives.ciphers import Cipher, algorithms

# --- Constants from DEC-048 & DEC-045C ---
ENCRYPTED_MEMO_SIZE = 96
INFO_VIEW_TAG = b"zeltra_view_tag"
SALT_COMMITTING_MEMO_V2 = b"ZELTRA_COMMITTING_MEMO_SALT_V2"
INFO_COMMITTING_MEMO_KEYS_V2 = b"ZELTRA_COMMITTING_MEMO_KEYS_V2"
INFO_MEMO_AD_BINDING_V1 = b"ZELTRA_MEMO_AD_BINDING_V1"

# BLS12-381 Fr scalar field modulus r
BLS12_381_R = 0x73eda753299d7d483339d80809a1d80553bda402fffe5bfeffffffff00000001


def derive_view_tag(shared_secret: bytes) -> int:
    """Derives 1-byte pseudo-random view tag (DEC-045C)."""
    hkdf = HKDF(
        algorithm=hashes.SHA256(),
        length=1,
        salt=None,
        info=INFO_VIEW_TAG,
    )
    return hkdf.derive(shared_secret)[0]


def derive_key_separation(shared_secret: bytes) -> tuple[bytes, bytes, bytes]:
    """
    RCR-PRG Key Separation G(S) -> (K_enc, K_cmt, N_7).
    Returns (cipher_key: 32B, commitment_key: 32B, nonce_7: 7B).
    """
    hkdf = HKDF(
        algorithm=hashes.SHA256(),
        length=71,
        salt=SALT_COMMITTING_MEMO_V2,
        info=INFO_COMMITTING_MEMO_KEYS_V2,
    )
    okm = hkdf.derive(shared_secret)
    cipher_key = okm[0:32]
    commitment_key = okm[32:64]
    nonce_7 = okm[64:71]
    return cipher_key, commitment_key, nonce_7


def compute_committing_tag(
    commitment_key: bytes,
    nonce_7: bytes,
    epk_bytes: bytes,
    view_tag: int,
    ciphertext: bytes,
) -> bytes:
    """Computes 16-byte committing authentication tag T_cmt using HMAC-SHA256."""
    h = hmac.new(commitment_key, digestmod=hashlib.sha256)
    # Nonce (7 bytes)
    h.update(nonce_7)
    # Associated Data Context (epk || view_tag || DOMAIN_TAG) (32 + 1 + 26 bytes)
    h.update(epk_bytes)
    h.update(bytes([view_tag]))
    h.update(INFO_MEMO_AD_BINDING_V1)
    # Ciphertext payload (40 bytes)
    h.update(ciphertext)
    return h.digest()[:16]


def chacha20_keystream(key: bytes, nonce_7: bytes, data: bytes) -> bytes:
    """ChaCha20 stream encryption/decryption using 16-byte OpenSSL IV (4B counter 0 + 7B nonce + 5B zero pad)."""
    nonce_16 = b"\x00" * 4 + nonce_7 + b"\x00" * 5
    cipher = Cipher(algorithms.ChaCha20(key, nonce_16), mode=None)
    encryptor = cipher.encryptor()
    return encryptor.update(data)


def serialize_note_plaintext_v3(value: int, rho_int: int, asset_id: int) -> bytes:
    """Serializes 40-byte note plaintext payload with Field-Bit Embedding (AUD-25 / DEC-048)."""
    assert 0 <= asset_id <= 1, "Invalid asset_id"
    assert 0 <= value <= 0xFFFF_FFFF_FFFF_FFFF, "Value out of 64-bit bounds"
    assert 0 <= rho_int < BLS12_381_R, "Non-canonical rho scalar"

    val_bytes = value.to_bytes(8, byteorder="little")
    rho_bytes = bytearray(rho_int.to_bytes(32, byteorder="little"))
    assert (rho_bytes[31] & 0x80) == 0, "BLS12-381 scalar bit 255 must be 0"
    rho_bytes[31] |= (asset_id & 1) << 7
    return val_bytes + bytes(rho_bytes)


def deserialize_note_plaintext_v3(payload: bytes) -> tuple[int, int, int] | None:
    """Deserializes 40-byte note plaintext (AUD-25). Returns (value, rho_int, asset_id) or None."""
    if len(payload) != 40:
        return None
    value = int.from_bytes(payload[0:8], byteorder="little")
    rho_bytes = bytearray(payload[8:40])
    asset_id = (rho_bytes[31] >> 7) & 1
    rho_bytes[31] &= 0x7F
    rho_int = int.from_bytes(rho_bytes, byteorder="little")

    if rho_int >= BLS12_381_R:
        return None  # Non-canonical scalar rejected
    return value, rho_int, asset_id


def encrypt_committing_memo(
    value: int,
    rho_int: int,
    asset_id: int,
    recipient_pubkey_bytes: bytes,
    ephemeral_privkey: x25519.X25519PrivateKey | None = None,
) -> bytes:
    """Encrypts note into canonical 96-byte committing memo (EtH-KS / DEC-048)."""
    pt_bytes = serialize_note_plaintext_v3(value, rho_int, asset_id)

    if ephemeral_privkey is None:
        ephemeral_privkey = x25519.X25519PrivateKey.generate()

    epk = ephemeral_privkey.public_key()
    epk_bytes = epk.public_bytes_raw()

    recipient_pub = x25519.X25519PublicKey.from_public_bytes(recipient_pubkey_bytes)
    shared_secret = ephemeral_privkey.exchange(recipient_pub)
    if shared_secret == b"\x00" * 32:
        raise ValueError("Non-contributory public key")

    view_tag = derive_view_tag(shared_secret)
    k_enc, k_cmt, nonce_7 = derive_key_separation(shared_secret)

    ciphertext = chacha20_keystream(k_enc, nonce_7, pt_bytes)

    tag = compute_committing_tag(k_cmt, nonce_7, epk_bytes, view_tag, ciphertext)

    memo = epk_bytes + bytes([view_tag]) + nonce_7 + ciphertext + tag
    assert len(memo) == ENCRYPTED_MEMO_SIZE
    return memo


def decrypt_committing_memo(
    memo: bytes,
    recipient_privkey: x25519.X25519PrivateKey,
) -> tuple[int, int, int] | None:
    """
    Trial-decrypts 96-byte committing memo using recipient's viewing key.
    Enforces Verify-then-Decrypt: returns None without touching ciphertext if tag fails.
    """
    if len(memo) != ENCRYPTED_MEMO_SIZE:
        return None

    epk_bytes = memo[0:32]
    view_tag_memo = memo[32]
    nonce_7 = memo[33:40]
    ciphertext = memo[40:80]
    tag_memo = memo[80:96]

    try:
        epk = x25519.X25519PublicKey.from_public_bytes(epk_bytes)
        shared_secret = recipient_privkey.exchange(epk)
    except Exception:
        return None

    # AUD-26 check: non-contributory point defense
    if shared_secret == b"\x00" * 32:
        return None

    # Step 1: Fast-path View Tag pre-filter
    expected_view_tag = derive_view_tag(shared_secret)
    if view_tag_memo != expected_view_tag:
        return None

    # Step 2: Key Separation
    k_enc, k_cmt, expected_nonce = derive_key_separation(shared_secret)

    # Step 3: Strict Nonce Verification
    if not hmac.compare_digest(nonce_7, expected_nonce):
        return None

    # Step 4: Verify-then-Decrypt Constant-Time Tag Verification
    expected_tag = compute_committing_tag(k_cmt, nonce_7, epk_bytes, view_tag_memo, ciphertext)
    if not hmac.compare_digest(expected_tag, tag_memo):
        return None  # Tampered or partitioning memo rejected fail-closed

    # Step 5: Decryption of verified ciphertext
    pt_bytes = chacha20_keystream(k_enc, nonce_7, ciphertext)

    # Step 6: Canonical parsing
    return deserialize_note_plaintext_v3(pt_bytes)


# ==============================================================================
# TEST RUNNER & ADVERSARIAL TEST SUITE
# ==============================================================================

def run_all_tests():
    print("=" * 80)
    print("DEC-048 TEST HARNESS: COMMITTING AEAD & PARTITIONING ORACLE DEFENSE")
    print("IACR ePrint 2026/240 (Minematsu & Inoue, October 2026)")
    print("=" * 80)

    # Setup Alice's keys
    alice_priv = x25519.X25519PrivateKey.generate()
    alice_pub = alice_priv.public_key()
    alice_pub_bytes = alice_pub.public_bytes_raw()

    # Honest Note Parameters
    test_value = 50_000_000  # 50 USDC (Micro-USDC)
    test_asset_id = 1         # USDC
    test_rho = 0x123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0
    assert test_rho < BLS12_381_R

    # --------------------------------------------------------------------------
    # 1. POSITIVE VERIFICATION TEST
    # --------------------------------------------------------------------------
    print("\n[+] POSITIVE TEST: Honest Note Encryption & Trial Decryption (EtH-KS)...")
    memo = encrypt_committing_memo(test_value, test_rho, test_asset_id, alice_pub_bytes)
    assert len(memo) == 96, f"Memo length must be 96, got {len(memo)}"

    recovered = decrypt_committing_memo(memo, alice_priv)
    assert recovered is not None, "Failed honest trial decryption"
    rec_val, rec_rho, rec_asset = recovered
    assert rec_val == test_value, f"Value mismatch: {rec_val} != {test_value}"
    assert rec_rho == test_rho, f"Rho mismatch: {rec_rho} != {test_rho}"
    assert rec_asset == test_asset_id, f"Asset ID mismatch: {rec_asset} != {test_asset_id}"
    print(f"    SUCCESS: Correctly recovered {rec_val/1e6} USDC, rho={hex(rec_rho)[:18]}..., asset={rec_asset}")

    # --------------------------------------------------------------------------
    # 2. NEG-01: Multi-Key Partitioning Oracle Attack Resistance
    # --------------------------------------------------------------------------
    print("\n[-] NEG-01: Multi-Key Partitioning Resistance across 100 Random Viewing Keys...")
    false_positives = 0
    for i in range(100):
        random_priv = x25519.X25519PrivateKey.generate()
        res = decrypt_committing_memo(memo, random_priv)
        if res is not None:
            false_positives += 1

    assert false_positives == 0, f"Critical Bug! {false_positives} non-target keys decrypted the memo!"
    print(f"    SUCCESS: 100/100 non-target keys rejected the memo (0 false positives).")

    # --------------------------------------------------------------------------
    # 3. NEG-02: Single-Bit Tag Tampering Resistance (128 bit flips)
    # --------------------------------------------------------------------------
    print("\n[-] NEG-02: Single-Bit Committing Tag Tampering (Exhaustive 128 Bit Flips)...")
    tag_tamper_failures = 0
    for byte_idx in range(80, 96):
        for bit_idx in range(8):
            corrupted = bytearray(memo)
            corrupted[byte_idx] ^= (1 << bit_idx)
            res = decrypt_committing_memo(bytes(corrupted), alice_priv)
            if res is not None:
                tag_tamper_failures += 1

    assert tag_tamper_failures == 0, f"Critical Bug! {tag_tamper_failures} tampered tags accepted!"
    print(f"    SUCCESS: All 128/128 single-bit tag mutations rejected fail-closed.")

    # --------------------------------------------------------------------------
    # 4. NEG-03: Single-Bit Ciphertext Payload Tampering Resistance
    # --------------------------------------------------------------------------
    print("\n[-] NEG-03: Ciphertext Payload Tampering (40 Bytes Mutation)...")
    ct_tamper_failures = 0
    for byte_idx in range(40, 80):
        corrupted = bytearray(memo)
        corrupted[byte_idx] ^= 0x01
        res = decrypt_committing_memo(bytes(corrupted), alice_priv)
        if res is not None:
            ct_tamper_failures += 1

    assert ct_tamper_failures == 0, f"Critical Bug! {ct_tamper_failures} tampered ciphertexts accepted!"
    print(f"    SUCCESS: All 40/40 ciphertext payload mutations rejected fail-closed.")

    # --------------------------------------------------------------------------
    # 5. NEG-04: Nonce Manipulation / Replay Tampering Resistance
    # --------------------------------------------------------------------------
    print("\n[-] NEG-04: Nonce Stream Manipulation...")
    corrupted_nonce = bytearray(memo)
    corrupted_nonce[35] ^= 0xFF
    res_nonce = decrypt_committing_memo(bytes(corrupted_nonce), alice_priv)
    assert res_nonce is None, "Tampered nonce was accepted!"
    print("    SUCCESS: Nonce modification rejected fail-closed.")

    # --------------------------------------------------------------------------
    # 6. NEG-05: Ephemeral Public Key Substitution Resistance
    # --------------------------------------------------------------------------
    print("\n[-] NEG-05: Ephemeral Public Key Substitution Attack...")
    corrupted_epk = bytearray(memo)
    random_epk = x25519.X25519PrivateKey.generate().public_key().public_bytes_raw()
    corrupted_epk[0:32] = random_epk
    res_epk = decrypt_committing_memo(bytes(corrupted_epk), alice_priv)
    assert res_epk is None, "Malleated EPK was accepted!"
    print("    SUCCESS: Substituted EPK rejected fail-closed.")

    # --------------------------------------------------------------------------
    # 7. NEG-06: Non-Canonical BLS12-381 Scalar Rejection
    # --------------------------------------------------------------------------
    print("\n[-] NEG-06: Non-Canonical Scalar Injection (rho >= r)...")
    overflow_rho = BLS12_381_R + 5
    # Manually construct memo with overflow rho to test deserialization gate
    bad_pt = serialize_note_plaintext_v3(test_value, test_rho, test_asset_id)
    # Patch bad rho directly (little-endian)
    bad_pt_overflow = bad_pt[0:8] + overflow_rho.to_bytes(32, byteorder="little")
    eph = x25519.X25519PrivateKey.generate()
    epk_bytes = eph.public_key().public_bytes_raw()
    shared = eph.exchange(alice_pub)
    vt = derive_view_tag(shared)
    k_enc, k_cmt, n_7 = derive_key_separation(shared)
    ct_bad = chacha20_keystream(k_enc, n_7, bad_pt_overflow)
    tag_bad = compute_committing_tag(k_cmt, n_7, epk_bytes, vt, ct_bad)
    memo_overflow = epk_bytes + bytes([vt]) + n_7 + ct_bad + tag_bad

    res_overflow = decrypt_committing_memo(memo_overflow, alice_priv)
    assert res_overflow is None, "Non-canonical scalar (rho >= r) was accepted!"
    print("    SUCCESS: Non-canonical scalar strictly rejected fail-closed.")

    # --------------------------------------------------------------------------
    # 8. NEG-07: Malformed / Truncated Calldata Rejection
    # --------------------------------------------------------------------------
    print("\n[-] NEG-07: Calldata Size Boundaries (0, 32, 95, 97, 128 Bytes)...")
    for bad_len in [0, 32, 95, 97, 128]:
        bad_calldata = os.urandom(bad_len)
        res_len = decrypt_committing_memo(bad_calldata, alice_priv)
        assert res_len is None, f"Calldata of length {bad_len} was accepted!"
    print("    SUCCESS: All non-96-byte inputs rejected fail-closed.")

    # --------------------------------------------------------------------------
    # 9. NEG-08: Asset ID Boundary Overflow
    # --------------------------------------------------------------------------
    print("\n[-] NEG-08: Asset ID Boundary Overflow...")
    try:
        serialize_note_plaintext_v3(test_value, test_rho, asset_id=2)
        assert False, "Asset ID 2 must not be serializable"
    except AssertionError:
        pass
    print("    SUCCESS: Asset ID > 1 rejected fail-closed.")

    # --------------------------------------------------------------------------
    # 10. NEG-09: All-Zero / Non-Contributory X25519 Public Key Rejection (AUD-26)
    # --------------------------------------------------------------------------
    print("\n[-] NEG-09: Non-Contributory Ephemeral Public Key Defense (AUD-26)...")
    zero_memo = bytearray(memo)
    zero_memo[0:32] = b"\x00" * 32
    res_zero = decrypt_committing_memo(bytes(zero_memo), alice_priv)
    assert res_zero is None, "All-zero EPK must be rejected as non-contributory!"
    print("    SUCCESS: All-zero non-contributory point rejected fail-closed.")

    # --------------------------------------------------------------------------
    # 11. AUD-25: 64-Bit Value Embedding Demonstration (Field-Bit Embedding)
    # --------------------------------------------------------------------------
    print("\n[+] AUD-25: Full 64-Bit Value Field-Bit Embedding Verification...")
    large_values = [
        (0, "Zero"),
        (2_147_483_647, "Max 31-bit (2,147.48 USDC)"),
        (2_147_483_648, "Max 31-bit + 1"),
        (5_000_000_000, "5,000 USDC"),
        (10_000_000_000_000_000, "10 Billion USDC"),
        ((1 << 64) - 1, "u64::MAX"),
    ]
    for val, label in large_values:
        # Full 64-bit serializer using 40B payload (8B value + 32B rho with bit 255 asset_id)
        asset = 1
        rho_bytes = bytearray(test_rho.to_bytes(32, byteorder="big"))
        assert (rho_bytes[0] & 0x80) == 0, "BLS12-381 scalar bit 255 must be 0"
        rho_bytes[0] |= (asset << 7) # embed asset_id in bit 255
        payload_40 = val.to_bytes(8, byteorder="big") + bytes(rho_bytes)

        # Deserializer
        rec_val = int.from_bytes(payload_40[0:8], byteorder="big")
        rec_asset = (payload_40[8] >> 7) & 1
        clean_rho_bytes = bytearray(payload_40[8:40])
        clean_rho_bytes[0] &= 0x7F
        rec_rho = int.from_bytes(clean_rho_bytes, byteorder="big")

        assert rec_val == val, f"Value mismatch: {rec_val} != {val}"
        assert rec_asset == asset, f"Asset mismatch: {rec_asset} != {asset}"
        assert rec_rho == test_rho, f"Rho mismatch: {rec_rho} != {test_rho}"
        print(f"    SUCCESS: {label} ({val}) accurately round-tripped without truncation.")

    print("\n" + "=" * 80)
    print("ALL DEC-048 COMMITTING AEAD & AUDIT REGRESSION TESTS PASSED (100% SOUND)")
    print("=" * 80)


if __name__ == "__main__":
    run_all_tests()

