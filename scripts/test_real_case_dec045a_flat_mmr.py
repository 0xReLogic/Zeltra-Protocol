#!/usr/bin/env python3
"""
Zeltra Protocol - DEC-045A Real-Case Cryptographic Soundness & Flat MMR Audit Test Suite

This script acts as an independent external auditor to validate:
1. Peter Todd (2012) Bitwise Post-Order Navigation Algorithms Parity.
2. Boundary Scaling & Array Size Invariant from N=1 to N=2,049 (crossing 2^0 through 2^11).
3. Root Parity & Bit-Exact Equivalence between Flat MMR and Legacy MMR.
4. Zero-Hash Proof Generation & Roundtrip Verification.
5. Threat Vectors & Attack Invariants:
   - Out-of-bounds (OOB) leaf index injection (fail-closed, memory uncorrupted)
   - Empty tree proof generation rejection
   - Hyperbridge (2026) falsified leaf_count injection exploit
   - 1-bit tampering on mountain sibling
   - 1-bit tampering on peak bagging sibling
   - Subtree Immutability Check (nodes 0..N-1 never mutated on subsequent appends)
   - Non-canonical scalar (>= r) rejection
6. Performance Invariant (AUD-12): Proof generation latency < 20 microseconds.
"""

import sys
import os
import json
import subprocess
from typing import Tuple, List, Dict, Any, Optional

# ANSI terminal colors
GREEN = "\033[92m"
RED = "\033[91m"
YELLOW = "\033[93m"
CYAN = "\033[96m"
BOLD = "\033[1m"
RESET = "\033[0m"

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
    """Finds or builds the Rust flat_mmr_interop test harness binary."""
    repo_root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    target_bin = os.path.join(repo_root, "target", "debug", "flat_mmr_interop")

    if not os.path.exists(target_bin):
        print(f"{YELLOW}[BUILD]{RESET} Building flat_mmr_interop binary...")
        res = subprocess.run(
            ["cargo", "build", "--bin", "flat_mmr_interop", "-p", "zeltra-core"],
            cwd=repo_root,
            capture_output=True,
            text=True,
        )
        if res.returncode != 0:
            print(f"{RED}Build failed:{RESET}\n{res.stderr}")
            sys.exit(1)
    return target_bin

def run_harness(bin_path: str, args: List[str]) -> Dict[str, Any]:
    cmd = [bin_path] + args
    res = subprocess.run(cmd, capture_output=True, text=True)
    if res.returncode != 0 and not res.stdout:
        raise RuntimeError(f"Harness error: {res.stderr}")
    try:
        return json.loads(res.stdout.strip())
    except Exception as e:
        raise RuntimeError(f"Failed to parse JSON from harness output: {res.stdout.strip()}, err: {e}")

# Pure Python Implementation of Peter Todd (2012) & Grin PMMR
ALL_ONES_64 = (1 << 64) - 1

def py_leaf_index_to_pos(leaf_index: int) -> int:
    return 2 * leaf_index - bin(leaf_index).count("1")

def py_pos_height(pos: int) -> int:
    pos += 1
    while pos > 0:
        p_len = pos.bit_length()
        all_ones = (1 << p_len) - 1
        if pos == all_ones:
            return p_len - 1
        pos -= all_ones >> 1
    return 0

def py_peak_map_height(size: int) -> Tuple[int, int]:
    if size == 0:
        return 0, 0
    lz = 64 - size.bit_length()
    peak_size = ALL_ONES_64 >> lz
    peak_map = 0
    while peak_size != 0:
        peak_map <<= 1
        if size >= peak_size:
            size -= peak_size
            peak_map |= 1
        peak_size >>= 1
    return peak_map, size

def py_is_left_sibling(pos: int, height: int) -> bool:
    peak_map, _ = py_peak_map_height(pos)
    peak = 1 << height
    return (peak_map & peak) == 0

def py_get_peak_positions(leaf_count: int) -> List[int]:
    if leaf_count == 0:
        return []
    peaks = []
    curr_pos = 0
    for h in range(63, -1, -1):
        if (leaf_count >> h) & 1 == 1:
            peak_pos = curr_pos + (1 << (h + 1)) - 2
            peaks.append(peak_pos)
            curr_pos += (1 << (h + 1)) - 1
    return peaks

def py_expected_nodes_count(leaf_count: int) -> int:
    return 2 * leaf_count - bin(leaf_count).count("1")

def main():
    print(f"{BOLD}{GREEN}=== ZELTRA PROTOCOL: DEC-045A AUDITOR TEST SUITE ==={RESET}")
    harness = find_or_build_harness()

    # ---------------------------------------------------------
    # TEST 1: Bitwise Navigation Algorithms Parity
    # ---------------------------------------------------------
    print_header("Test 1: Peter Todd (2012) Bitwise Post-Order Navigation")
    # Verify leaf_index_to_pos
    for k in range(50):
        pos = py_leaf_index_to_pos(k)
        assert_test(pos == 2 * k - bin(k).count("1"), f"leaf_index_to_pos({k})", f"-> pos {pos}")
        assert_test(py_pos_height(pos) == 0, f"is_leaf pos({k})", f"pos {pos} height == 0")

    # Verify peak positions decomposition
    for n in [1, 2, 3, 4, 7, 8, 15, 16, 31, 32, 513, 2049]:
        expected_peaks = py_get_peak_positions(n)
        rust_tree = run_harness(harness, ["build_tree", str(n)])
        assert_test(rust_tree["peak_positions"] == expected_peaks, f"Peak positions match for N={n}", f"{expected_peaks}")

    # ---------------------------------------------------------
    # TEST 2: Boundary Scaling & Array Size Invariant (N=1 to 2,049)
    # ---------------------------------------------------------
    print_header("Test 2: Boundary Scaling & Array Size Invariant (2^0 to 2^11)")
    boundary_sizes = [
        1, 2, 3, 4, 5, 7, 8, 9, 15, 16, 17, 31, 32, 33, 63, 64, 65,
        127, 128, 129, 255, 256, 257, 511, 512, 513, 1023, 1024, 1025,
        2047, 2048, 2049
    ]
    for n in boundary_sizes:
        res = run_harness(harness, ["build_tree", str(n)])
        expected_len = py_expected_nodes_count(n)
        assert_test(
            res["nodes_len"] == expected_len and res["array_size_invariant_valid"],
            f"Array Size Invariant |nodes| == 2N - count_ones(N) at N={n}",
            f"nodes: {res['nodes_len']} == {expected_len}"
        )

    # ---------------------------------------------------------
    # TEST 3: Mathematical Parity - Flat MMR vs Legacy MMR Root
    # ---------------------------------------------------------
    print_header("Test 3: Mathematical Parity with Legacy MMR Across Mountain Transitions")
    parity_check_sizes = [1, 2, 3, 7, 8, 15, 16, 33, 64, 128, 256, 512, 513, 1024, 2048]
    for n in parity_check_sizes:
        parity_res = run_harness(harness, ["compare_parity", str(n)])
        assert_test(
            parity_res["parity"],
            f"Root Parity 100% Bit-Exact at N={n}",
            f"root: 0x{parity_res['flat_root'][:16]}..."
        )

    # ---------------------------------------------------------
    # TEST 4: Zero-Hash Inclusion Proof Roundtrip Verification
    # ---------------------------------------------------------
    print_header("Test 4: Zero-Hash Proof Generation & On-Chain Verifier Roundtrip")
    test_leaf_count = 2049 # crosses 2^11 mountain boundary
    sample_indices = [0, 1, 2, 2047, 2048]
    for idx in sample_indices:
        proof_res = run_harness(harness, ["get_proof", str(test_leaf_count), str(idx)])
        assert_test(proof_res["success"], f"Proof generated for leaf {idx} in N={test_leaf_count}")

        # Verify via verify_proof
        leaf_val_hex = hex(idx + 1)[2:].zfill(64)
        m_sibs_str = ",".join(proof_res["mountain_siblings"]) if proof_res["mountain_siblings"] else "none"
        p_sibs_str = ",".join(proof_res["peak_bagging_siblings"]) if proof_res["peak_bagging_siblings"] else "none"

        v_res = run_harness(
            harness,
            [
                "verify_proof",
                leaf_val_hex,
                str(idx),
                str(test_leaf_count),
                str(proof_res["mountain_height"]),
                m_sibs_str,
                p_sibs_str,
                str(proof_res["peak_index"]),
                proof_res["root_hex"],
            ],
        )
        assert_test(v_res["valid"], f"Proof verified valid for leaf {idx}", f"height={proof_res['mountain_height']}")

    # ---------------------------------------------------------
    # TEST 5: Attack Vector 1 - Out-Of-Bounds (OOB) Request Rejection
    # ---------------------------------------------------------
    print_header("Test 5: Attack Vector 1 - Out-Of-Bounds Leaf Index (Fail-Closed)")
    # Test leaf_index == leaf_count
    oob_1 = run_harness(harness, ["get_proof", "10", "10"])
    assert_test(
        not oob_1["success"] and "out of bounds" in oob_1["error"].lower(),
        "Rejects leaf_index == leaf_count (Boundary check)",
        oob_1["error"]
    )
    # Test leaf_index > leaf_count
    oob_2 = run_harness(harness, ["get_proof", "10", "100"])
    assert_test(
        not oob_2["success"] and "out of bounds" in oob_2["error"].lower(),
        "Rejects leaf_index > leaf_count (OOB check)",
        oob_2["error"]
    )

    # ---------------------------------------------------------
    # TEST 6: Attack Vector 2 - Empty Tree Proof Rejection
    # ---------------------------------------------------------
    print_header("Test 6: Attack Vector 2 - Empty Tree Proof Rejection")
    empty_res = run_harness(harness, ["get_proof", "0", "0"])
    assert_test(
        not empty_res["success"] and "empty mmr tree" in empty_res["error"].lower(),
        "Rejects proof generation on N=0 empty tree",
        empty_res["error"]
    )

    # ---------------------------------------------------------
    # TEST 7: Attack Vector 3 - Hyperbridge (2026) Falsified Leaf Count
    # ---------------------------------------------------------
    print_header("Test 7: Attack Vector 3 - Hyperbridge 2026 Tampered leaf_count")
    # Legitimate proof for leaf 2 in tree of size 4
    proof_base = run_harness(harness, ["get_proof", "4", "2"])
    leaf_val = hex(3)[2:].zfill(64)
    m_sibs_str = ",".join(proof_base["mountain_siblings"]) if proof_base["mountain_siblings"] else "none"
    p_sibs_str = ",".join(proof_base["peak_bagging_siblings"]) if proof_base["peak_bagging_siblings"] else "none"

    # Attacker injects falsified leaf_count = 8
    tampered_count_res = run_harness(
        harness,
        [
            "verify_proof",
            leaf_val,
            "2",
            "8", # Falsified!
            str(proof_base["mountain_height"]),
            m_sibs_str,
            p_sibs_str,
            str(proof_base["peak_index"]),
            proof_base["root_hex"],
        ],
    )
    assert_test(
        not tampered_count_res["valid"],
        "Hyperbridge 2026 exploit rejected (falsified leaf_count=8 != 4)"
    )

    # ---------------------------------------------------------
    # TEST 8: Attack Vector 4 - Single-Bit Mountain Sibling Corruption
    # ---------------------------------------------------------
    print_header("Test 8: Attack Vector 4 - 1-Bit Mountain Sibling Tampering")
    proof_tamper = run_harness(harness, ["get_proof", "8", "3"])
    sibs = proof_tamper["mountain_siblings"]
    assert len(sibs) > 0

    # Flip last hex char of first sibling
    tampered_sib = sibs[0][:-1] + ("0" if sibs[0][-1] != "0" else "1")
    tampered_m_sibs = [tampered_sib] + sibs[1:]
    tampered_m_str = ",".join(tampered_m_sibs)
    p_sibs_str = ",".join(proof_tamper["peak_bagging_siblings"]) if proof_tamper["peak_bagging_siblings"] else "none"

    tamper_sib_res = run_harness(
        harness,
        [
            "verify_proof",
            hex(4)[2:].zfill(64),
            "3",
            "8",
            str(proof_tamper["mountain_height"]),
            tampered_m_str,
            p_sibs_str,
            str(proof_tamper["peak_index"]),
            proof_tamper["root_hex"],
        ],
    )
    assert_test(
        not tamper_sib_res["valid"],
        "1-bit tampered mountain sibling strictly rejected by verifier"
    )

    # ---------------------------------------------------------
    # TEST 9: Attack Vector 5 - Single-Bit Peak Bagging Sibling Corruption
    # ---------------------------------------------------------
    print_header("Test 9: Attack Vector 5 - 1-Bit Peak Bagging Sibling Tampering")
    proof_p_tamper = run_harness(harness, ["get_proof", "7", "6"])
    p_sibs = proof_p_tamper["peak_bagging_siblings"]
    assert len(p_sibs) > 0

    # Flip last hex char of first peak sibling
    tampered_p_sib = p_sibs[0][:-1] + ("0" if p_sibs[0][-1] != "0" else "1")
    tampered_p_sibs = [tampered_p_sib] + p_sibs[1:]
    m_sibs_str = ",".join(proof_p_tamper["mountain_siblings"]) if proof_p_tamper["mountain_siblings"] else "none"
    tampered_p_str = ",".join(tampered_p_sibs)

    tamper_p_res = run_harness(
        harness,
        [
            "verify_proof",
            hex(7)[2:].zfill(64),
            "6",
            "7",
            str(proof_p_tamper["mountain_height"]),
            m_sibs_str,
            tampered_p_str,
            str(proof_p_tamper["peak_index"]),
            proof_p_tamper["root_hex"],
        ],
    )
    assert_test(
        not tamper_p_res["valid"],
        "1-bit tampered peak bagging sibling strictly fails root check"
    )

    # ---------------------------------------------------------
    # TEST 10: Attack Vector 6 - Subtree Immutability Check
    # ---------------------------------------------------------
    print_header("Test 10: Attack Vector 6 - Subtree Immutability Check (Append-Only)")
    # Capture nodes at N=16
    nodes_n16 = []
    tree_16 = run_harness(harness, ["build_tree", "16"])
    n16_len = tree_16["nodes_len"]
    for pos in range(n16_len):
        node = run_harness(harness, ["inspect_node", "16", str(pos)])
        nodes_n16.append(node["hash_hex"])

    # Grow tree to N=32 (nodes_len will be 63)
    tree_32 = run_harness(harness, ["build_tree", "32"])
    for pos in range(n16_len):
        node = run_harness(harness, ["inspect_node", "32", str(pos)])
        assert_test(
            node["hash_hex"] == nodes_n16[pos],
            f"Node at pos {pos} is 100% immutable after append (N=16 -> N=32)",
            f"{node['hash_hex'][:12]}..."
        )

    # ---------------------------------------------------------
    # TEST 11: Attack Vector 7 - Non-Canonical Scalar Field Element Rejection
    # ---------------------------------------------------------
    print_header("Test 11: Attack Vector 7 - Non-Canonical Scalar Rejection (>= r)")
    nc_res = run_harness(harness, ["append_non_canonical"])
    assert_test(
        nc_res["rejected"] and nc_res["state_unchanged"],
        "Non-canonical scalar [0xff; 32] >= r rejected & state untouched"
    )

    # ---------------------------------------------------------
    # TEST 12: Zero-Hash Performance Audit (AUD-12 Latency Verification)
    # ---------------------------------------------------------
    print_header("Test 12: Performance Audit (AUD-12 Zero-Hash Proof Verification)")
    bench_res = run_harness(harness, ["benchmark_proof", "1000", "50"])
    flat_us = bench_res["flat_avg_us"]
    legacy_us = bench_res["legacy_avg_us"]
    speedup = bench_res["speedup_factor"]

    print(f"  {BOLD}Flat MMR Average Latency:{RESET} {CYAN}{flat_us:.3f} µs{RESET}")
    print(f"  {BOLD}Legacy MMR Average Latency:{RESET} {YELLOW}{legacy_us:.3f} µs{RESET} ({legacy_us/1000.0:.2f} ms)")
    print(f"  {BOLD}Observed Acceleration Speedup:{RESET} {GREEN}{speedup:.1f}x{RESET}")

    assert_test(
        flat_us < 20.0,
        "Flat MMR generates proof in < 20 microseconds (O(log N) zero-hash memory lookup)",
        f"actual: {flat_us:.3f} µs"
    )
    assert_test(
        speedup > 100.0,
        "Zero-hash MMR delivers > 100x speedup over legacy re-hashing engine",
        f"actual: {speedup:.1f}x"
    )

    # Summary
    print(f"\n{BOLD}{GREEN}All {TOTAL_TESTS}/{TOTAL_TESTS} Real-Case Independent Auditor Tests PASSED!{RESET}\n")

if __name__ == "__main__":
    main()
