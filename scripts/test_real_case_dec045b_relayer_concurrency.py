#!/usr/bin/env python3
"""
Zeltra Protocol - DEC-045B Real-Case Relayer Concurrency & Crash Recovery Audit Suite

Validates real-world production invariants:
1. Real-Case 1 (High-Concurrency Hammering):
   50 concurrent HTTP threads querying MMR proof simultaneously (< 5 ms latency target).
2. Real-Case 2 (Concurrent Read-While-Writing):
   30 concurrent readers continuously fetching proofs while indexer appends 100 new leaves.
   Validates zero deadlock, zero panic, zero lock starvation.
3. Real-Case 3 (Simulated Crash & Cold Boot Recovery):
   Abrupt termination (SIGKILL) of relayer process.
   Cold-boot relayer re-initializes from SQLite; verifies exact leaf count and root bit-parity.
4. Real-Case 4 (Reorg Simulation):
   2-block L2 reorg rollback.
   Proves in-memory Flat MMR array and DB roll back to pre-reorg state and accept new canonical fork.
5. Real-Case 5 (DoS Ingress Attack Vectors):
   OOB leaf index, malformed path string, invalid index values.
   Proves fail-closed rejection without crash or resource leak.
"""

import sys
import os
import json
import time
import signal
import socket
import tempfile
import urllib.request
import urllib.error
import http.client
import subprocess
import threading
from concurrent.futures import ThreadPoolExecutor, as_completed
from typing import Dict, Any, List, Tuple, Optional

# ANSI colors
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

def get_free_port() -> int:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
        s.bind(('127.0.0.1', 0))
        return s.getsockname()[1]

def build_server_binary() -> str:
    repo_root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    target_bin = os.path.join(repo_root, "target", "release", "relayer_test_server")
    if not os.path.exists(target_bin):
        print(f"{YELLOW}[BUILD]{RESET} Building relayer_test_server (release mode)...")
        res = subprocess.run(
            ["cargo", "build", "--release", "--bin", "relayer_test_server", "-p", "zeltra-node"],
            cwd=repo_root,
            capture_output=True,
            text=True
        )
        if res.returncode != 0:
            print(f"{RED}Build failed:{RESET}\n{res.stderr}")
            sys.exit(1)
    return target_bin

def start_relayer(server_bin: str, port: int, db_path: str) -> subprocess.Popen:
    env = os.environ.copy()
    env["ZELTRA_DB_KEY"] = "relayer-test-encryption-key-32b"
    env["ZELTRA_RATE_LIMIT_PER_SEC"] = "50000"
    proc = subprocess.Popen(
        [server_bin, str(port), db_path],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        env=env
    )
    # Wait for server ready signal
    start_time = time.time()
    while time.time() - start_time < 10.0:
        line = proc.stdout.readline()
        if "RELAYER_READY" in line:
            return proc
        if proc.poll() is not None:
            stderr = proc.stderr.read()
            raise RuntimeError(f"Server exited prematurely: {stderr}")
        time.sleep(0.05)
    raise TimeoutError("Server failed to start within 10s")

def http_get(url: str) -> Tuple[int, Any]:
    req = urllib.request.Request(url)
    try:
        with urllib.request.urlopen(req, timeout=5) as resp:
            data = resp.read().decode('utf-8')
            try:
                return resp.status, json.loads(data)
            except json.JSONDecodeError:
                return resp.status, data
    except urllib.error.HTTPError as e:
        data = e.read().decode('utf-8')
        try:
            return e.code, json.loads(data)
        except json.JSONDecodeError:
            return e.code, data

def http_post(url: str, body: Dict[str, Any]) -> Tuple[int, Any]:
    json_data = json.dumps(body).encode('utf-8')
    req = urllib.request.Request(
        url,
        data=json_data,
        headers={'Content-Type': 'application/json'}
    )
    try:
        with urllib.request.urlopen(req, timeout=5) as resp:
            data = resp.read().decode('utf-8')
            try:
                return resp.status, json.loads(data)
            except json.JSONDecodeError:
                return resp.status, data
    except urllib.error.HTTPError as e:
        data = e.read().decode('utf-8')
        try:
            return e.code, json.loads(data)
        except json.JSONDecodeError:
            return e.code, data

def main():
    print(f"{BOLD}{GREEN}Starting DEC-045B Real-Case Relayer Concurrency & Fault-Tolerance Audit{RESET}")
    server_bin = build_server_binary()
    temp_dir = tempfile.mkdtemp(prefix="zeltra_audit_")
    db_path = os.path.join(temp_dir, "relayer_audit.db")
    port = get_free_port()

    print(f"Test Environment: DB={db_path}, Port={port}")
    proc = start_relayer(server_bin, port, db_path)
    base_url = f"http://127.0.0.1:{port}"

    try:
        # Pre-seed tree with 200 initial leaves via batch endpoint
        print(f"\n{YELLOW}[SETUP]{RESET} Seeding initial 200 note commitments into MMR...")
        status, resp = http_post(f"{base_url}/test/append_leaves", {
            "count": 200,
            "start_index": 0,
            "block_number": 100
        })
        assert_test(status == 200, "Initial 200 leaves seeded successfully", f"count={resp.get('new_leaf_count')}")

        status, stats = http_get(f"{base_url}/test/stats")
        assert_test(status == 200 and stats.get("leaf_count") == 200, "Relayer stats verify 200 leaves in memory & SQLite")

        # =========================================================================
        # REAL-CASE 1: High-Concurrency Hammering (50 Threads)
        # =========================================================================
        print_header("Real-Case 1: High-Concurrency Hammering (50 Threads)")
        NUM_THREADS = 50
        REQUESTS_PER_THREAD = 20
        TOTAL_REQUESTS = NUM_THREADS * REQUESTS_PER_THREAD
        print(f"Firing {TOTAL_REQUESTS} parallel requests across {NUM_THREADS} threads...")

        latencies = []
        errors = []

        def hammer_worker(thread_id: int):
            thread_latencies = []
            conn = http.client.HTTPConnection("127.0.0.1", port, timeout=5)
            try:
                conn.connect()
                if conn.sock:
                    conn.sock.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
                # Warm-up request to prime HTTP connection pipeline
                conn.request("GET", "/api/v1/mmr/proof/0", headers={"Connection": "keep-alive"})
                w_resp = conn.getresponse()
                w_resp.read()

                for req_id in range(REQUESTS_PER_THREAD):
                    target_leaf = (thread_id * 17 + req_id) % 200
                    t0 = time.perf_counter()
                    conn.request("GET", f"/api/v1/mmr/proof/{target_leaf}", headers={"Connection": "keep-alive"})
                    resp = conn.getresponse()
                    raw_data = resp.read()
                    elapsed_ms = (time.perf_counter() - t0) * 1000.0
                    thread_latencies.append(elapsed_ms)
                    status = resp.status
                    try:
                        data = json.loads(raw_data.decode('utf-8'))
                    except Exception:
                        data = raw_data
                    if status != 200:
                        errors.append((status, data))
                    elif isinstance(data, dict) and (data.get("leaf_index") != target_leaf or data.get("leaf_count") != 200):
                        errors.append((status, "Data mismatch"))
                    time.sleep(0.002)
            finally:
                conn.close()
            return thread_latencies

        t_start = time.perf_counter()
        with ThreadPoolExecutor(max_workers=NUM_THREADS) as executor:
            futures = [executor.submit(hammer_worker, tid) for tid in range(NUM_THREADS)]
            for fut in as_completed(futures):
                latencies.extend(fut.result())
        total_time_s = time.perf_counter() - t_start

        latencies.sort()
        p50 = latencies[int(len(latencies) * 0.50)]
        p95 = latencies[int(len(latencies) * 0.95)]
        p99 = latencies[int(len(latencies) * 0.99)]
        rps = TOTAL_REQUESTS / total_time_s

        print(f"  Throughput: {rps:.2f} RPS | Total Time: {total_time_s:.2f}s")
        print(f"  Latency: p50 = {p50:.3f} ms | p95 = {p95:.3f} ms | p99 = {p99:.3f} ms")

        assert_test(len(errors) == 0, "Real-Case 1: 100% of proof requests succeeded with HTTP 200", f"errors={len(errors)}")
        assert_test(p50 < 5.0, "Real-Case 1: p50 serving latency < 5.0 ms", f"p50={p50:.3f} ms")
        assert_test(p99 < 50.0, "Real-Case 1: p99 serving latency bounded (< 50.0 ms)", f"p99={p99:.3f} ms")

        # =========================================================================
        # REAL-CASE 2: Concurrent Read-While-Writing
        # =========================================================================
        print_header("Real-Case 2: Concurrent Read-While-Writing (Zero Deadlock)")
        stop_readers = False
        reader_success = 0
        reader_errors = 0

        def reader_loop():
            nonlocal reader_success, reader_errors
            conn = http.client.HTTPConnection("127.0.0.1", port, timeout=5)
            try:
                while not stop_readers:
                    target = int(time.time() * 1000) % 200
                    try:
                        conn.request("GET", f"/api/v1/mmr/proof/{target}")
                        resp = conn.getresponse()
                        _ = resp.read()
                        if resp.status == 200:
                            reader_success += 1
                        else:
                            reader_errors += 1
                    except Exception:
                        reader_errors += 1
                    time.sleep(0.005)
            finally:
                conn.close()

        reader_threads = []
        for _ in range(30):
            t = threading.Thread(target=reader_loop)
            t.daemon = True
            t.start()
            reader_threads.append(t)

        print("Appending 100 new leaves in 10 sequential batches while 30 readers query continuously...")
        current_idx = 200
        for batch in range(10):
            st, resp = http_post(f"{base_url}/test/append_leaves", {
                "count": 10,
                "start_index": current_idx,
                "block_number": 101 + batch
            })
            assert st == 200, f"Batch {batch} append failed"
            current_idx += 10
            time.sleep(0.03)

        stop_readers = True
        for t in reader_threads:
            t.join(timeout=1.0)

        assert_test(reader_errors == 0, "Real-Case 2: Zero reader errors during concurrent writes", f"errors={reader_errors}")
        assert_test(reader_success > 100, "Real-Case 2: Concurrent readers served successfully during writes", f"served={reader_success}")

        st, stats = http_get(f"{base_url}/test/stats")
        assert_test(stats.get("leaf_count") == 300, "Real-Case 2: State updated accurately to exactly 300 leaves")
        pre_crash_root = stats.get("bagged_root")
        print(f"  Pre-crash canonical root: {pre_crash_root}")

        # =========================================================================
        # REAL-CASE 3: Simulated Crash (SIGKILL) & Cold Boot Recovery
        # =========================================================================
        print_header("Real-Case 3: Simulated Crash (SIGKILL) & Cold Boot Recovery")
        print(f"Killing relayer process PID {proc.pid} with SIGKILL (unclean exit)...")
        os.kill(proc.pid, signal.SIGKILL)
        proc.wait()
        print("  Relayer process forcefully killed.")

        time.sleep(0.5)

        print("Re-booting relayer node from existing SQLCipher database (Cold Startup)...")
        new_port = get_free_port()
        proc = start_relayer(server_bin, new_port, db_path)
        base_url = f"http://127.0.0.1:{new_port}"
        print(f"  Cold boot complete. New listening port: {new_port}")

        st, cold_stats = http_get(f"{base_url}/test/stats")
        assert_test(st == 200, "Cold boot stats endpoint responds with HTTP 200")
        assert_test(cold_stats.get("leaf_count") == 300, "Cold boot restored exact leaf_count (300) without drift")
        assert_test(cold_stats.get("bagged_root") == pre_crash_root, "Cold boot reconstructed 100% identical bagged root (zero parity loss)")

        # Verify inclusion proof for the latest leaf immediately upon reboot
        st_proof, proof_data = http_get(f"{base_url}/api/v1/mmr/proof/299")
        assert_test(st_proof == 200 and proof_data.get("leaf_count") == 300, "Cold boot immediately serves valid proof for latest leaf (299)")

        # =========================================================================
        # REAL-CASE 4: Reorg Simulation (2-Block Rollback)
        # =========================================================================
        print_header("Real-Case 4: Reorg Simulation (2-Block Rollback)")
        # Ingest 20 leaves on block 111 and 20 leaves on block 112 (total 340 leaves)
        http_post(f"{base_url}/test/append_leaves", {
            "count": 20,
            "start_index": 300,
            "block_number": 111
        })
        http_post(f"{base_url}/test/append_leaves", {
            "count": 20,
            "start_index": 320,
            "block_number": 112
        })
        st, stats_before_reorg = http_get(f"{base_url}/test/stats")
        assert_test(stats_before_reorg.get("leaf_count") == 340, "Pre-reorg tree grown to 340 leaves across blocks 111-112")

        # Simulate 2-block Arbitrum Nitro reorg: unwind to block 110
        print("Simulating 2-block L2 reorg detection: rolling back state to block 110...")
        st_rb, rb_resp = http_post(f"{base_url}/test/rollback/110", {})
        assert_test(st_rb == 200, "Rollback endpoint returned HTTP 200 OK")

        st, stats_after_reorg = http_get(f"{base_url}/test/stats")
        assert_test(stats_after_reorg.get("leaf_count") == 300, "Rollback restored in-memory and SQLite state to exactly 300 leaves")
        assert_test(stats_after_reorg.get("bagged_root") == pre_crash_root, "Post-reorg root matches canonical pre-fork root")

        # Ingest alternative canonical Branch B on block 111
        print("Ingesting alternative canonical Branch B (15 replacement leaves)...")
        http_post(f"{base_url}/test/append_leaves", {
            "count": 15,
            "start_index": 300,
            "block_number": 111
        })
        st, stats_branch_b = http_get(f"{base_url}/test/stats")
        assert_test(stats_branch_b.get("leaf_count") == 315, "Branch B committed cleanly, new leaf_count = 315")

        # =========================================================================
        # REAL-CASE 5: DoS Ingress Attack Vectors
        # =========================================================================
        print_header("Real-Case 5: DoS Ingress Attack Vectors (Fail-Closed & Immunity)")
        # 5a: Out-of-bounds leaf index
        st_oob, resp_oob = http_get(f"{base_url}/api/v1/mmr/proof/999999")
        assert_test(st_oob == 404, "Attack Vector 5a: Out-of-bounds leaf_index rejected with 404 Not Found")
        assert_test(resp_oob.get("code") == "LeafIndexOutOfBounds", "Error code strictly 'LeafIndexOutOfBounds'")

        # 5b: Malformed string path parameter
        st_malformed, _ = http_get(f"{base_url}/api/v1/mmr/proof/abcxyz")
        assert_test(st_malformed in (400, 404), "Attack Vector 5b: Non-numeric path rejected with 400/404 without crashing")

        # 5c: Negative integer path
        st_neg, _ = http_get(f"{base_url}/api/v1/mmr/proof/-42")
        assert_test(st_neg in (400, 404), "Attack Vector 5c: Negative integer path rejected with 400/404 without crashing")

        # 5d: Verify server remains 100% operational after attacks
        st_healthy, healthy_proof = http_get(f"{base_url}/api/v1/mmr/proof/0")
        assert_test(st_healthy == 200 and healthy_proof.get("leaf_index") == 0, "Server remains 100% responsive and healthy post-attack")

    finally:
        # Graceful cleanup
        if proc.poll() is None:
            proc.terminate()
            try:
                proc.wait(timeout=2.0)
            except subprocess.TimeoutExpired:
                proc.kill()
        # Remove temp files
        for f in os.listdir(temp_dir):
            try:
                os.remove(os.path.join(temp_dir, f))
            except Exception:
                pass
        try:
            os.rmdir(temp_dir)
        except Exception:
            pass

    print_header("SUMMARY OF AUDIT RESULTS")
    print(f"Total Tests Executed: {TOTAL_TESTS}")
    print(f"Tests Passed:         {GREEN}{PASSED_TESTS}/{TOTAL_TESTS} (100% SUCCESS){RESET}")
    print(f"\n{BOLD}{GREEN}ALL 5 REAL-CASE SCENARIOS PASSED WITH ZERO ERRORS!{RESET}")

if __name__ == "__main__":
    main()
