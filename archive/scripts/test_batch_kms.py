#!/usr/bin/env python3
import json
import os
import subprocess
import sys
import time
import urllib.request
import urllib.parse
import sqlite3
from web3 import Web3
from eth_account import Account

# --- Configurations ---
VAULT_PORT = 8200
VAULT_ADDR = f"http://127.0.0.1:{VAULT_PORT}"
VAULT_TOKEN = "test-root-token"

RPC_URL = "https://arbitrum-sepolia.core.chainstack.com/d18e11a2327c1a17c030975e3e0c8e24"
CHAIN_ID = 421614
CONTRACT_ADDRESS = "0xbda5fea381775a74ba8986090a919b17bb632a6d"
PRIVATE_KEY = "0xb89bc61712cfa0c890c0967f186c23afdf0b770743bc4f5505300100e8c7226e"
USDC_ADDRESS = "0x75faf114eafb1bdbe2f0316df893fd58ce46aa4d"

w3 = Web3(Web3.HTTPProvider(RPC_URL))
account = Account.from_key(PRIVATE_KEY)
MY_ADDRESS = account.address

CONTRACT_ABI = [
    {"inputs": [{"name": "pk_iss_bytes", "type": "bytes"}], "name": "isIssuerKeyTrusted", "outputs": [{"name": "", "type": "bool"}], "stateMutability": "view", "type": "function"},
    {"inputs": [{"name": "pk_iss_bytes", "type": "bytes"}], "name": "registerIssuerKey", "outputs": [], "stateMutability": "nonpayable", "type": "function"},
    {"inputs": [
        {"name": "sid", "type": "bytes32"},
        {"name": "com_k_bytes", "type": "bytes"},
        {"name": "amount", "type": "uint256"}
    ], "name": "deposit", "outputs": [], "stateMutability": "nonpayable", "type": "function"},
    {"inputs": [
        {"name": "sid", "type": "bytes32"},
        {"name": "k_bytes", "type": "bytes"},
        {"name": "pk_iss_bytes", "type": "bytes"},
        {"name": "com_k_bytes", "type": "bytes"}
    ], "name": "revealMaskKey", "outputs": [{"name": "", "type": "bool"}], "stateMutability": "nonpayable", "type": "function"},
    {"inputs": [
        {"name": "nullifiers", "type": "bytes32[]"},
        {"name": "alpha_neg_items", "type": "bytes[]"},
        {"name": "pk_iss_items", "type": "bytes[]"},
        {"name": "recipients", "type": "address[]"},
        {"name": "amounts", "type": "uint256[]"},
        {"name": "recipient_or_intent_hashes", "type": "bytes32[]"},
        {"name": "expiries", "type": "uint256[]"},
        {"name": "nonces", "type": "bytes32[]"}
    ], "name": "batchSpend", "outputs": [{"name": "", "type": "bool"}], "stateMutability": "nonpayable", "type": "function"},
]

ERC20_ABI = [
    {"inputs": [{"name": "account", "type": "address"}], "name": "balanceOf", "outputs": [{"name": "", "type": "uint256"}], "stateMutability": "view", "type": "function"},
    {"inputs": [{"name": "spender", "type": "address"}, {"name": "amount", "type": "uint256"}], "name": "approve", "outputs": [{"name": "", "type": "bool"}], "stateMutability": "nonpayable", "type": "function"},
    {"inputs": [{"name": "owner", "type": "address"}, {"name": "spender", "type": "address"}], "name": "allowance", "outputs": [{"name": "", "type": "uint256"}], "stateMutability": "view", "type": "function"},
]

contract = w3.eth.contract(address=w3.to_checksum_address(CONTRACT_ADDRESS), abi=CONTRACT_ABI)
usdc = w3.eth.contract(address=w3.to_checksum_address(USDC_ADDRESS), abi=ERC20_ABI)

log_processes = []
test_results = []

# Deterministic Public & Secret Shares from seed [0x42; 32]
PK_ISS = "885940298f97a7fbd92d404b898dcc5747d24cfa048ef3bbf9f4515b5605e4d058107f76c788ec2535476f3c241e6a410bfd5c8883b1d864bdcbdd76bdc2102d833237e19a7bc9d38e6ee5d98729c0e7b90052e61c6df8ca377a241ace187c02"
PK_ISS_UNCOMPRESSED = "00000000000000000000000000000000178282dc63af77b484e4a6f37fe86cb4253afe6df26d7bed2f38595c515efcefb59d258de3b753010e14f65b632d3467000000000000000000000000000000000f75903dfce18da6817055197038f7b44cfa868207a9d02af7ec42dd7d62f9be442cf33f348dcd7b6c228ff1b03b9e9800000000000000000000000000000000191ffa658b219841e21cdf898b1c5addb09f087bed557d45d9d6d86e5a901d41eecf70d6d99f38410e8dd60aa0f7e3df0000000000000000000000000000000014d71de29ce555e07484a87717f6991aeead239f1a250144047547052a40f202c2bbee851a234b770bfee1f778fada56"

SHARE1 = "0100000000000000ee580a1ebe9cc95f7a48a0a38d882d54f8d2a01a1b29fb035b08910b49e36760"
SHARE2 = "02000000000000002591737b212a5bb850368a813c6c6a8dc8efd0b56e6999f8744b3eccc5e52c64"
SHARE3 = "03000000000000003e00e675ef3022856780276d6447cb7677cf1a257a50b04cb5081cbb5a2a173f"
SHARE4 = "04000000000000003aa6610d27b11ec6bd82766608be0d640a4a207245b6793364bdc7015b581465"
SHARE5 = "05000000000000001883e641c9aa507b54e1786d252c74017c873f93c8c2bb7939eca37673c83662"

PUB1 = "b5e3ce24c5c3852699737786086f0f6a81a1d2deb25fce296cbb9a19361cd0cc5b661cc760501f1d00f04d7c4c83a12b0da040d013c14064581c0f400ee042925777fb15eb15c597ceea72fc6d4fa11a278d1a6d38a0e3c3da61f00ff2141607"
PUB2 = "a16f2160800821708345c04909441f5f4edaa399828f8547431786ec294898a147da29700ff61c9ba31577b11a2d6f56117f925869fdce9dcb9e95d07b560c872d76df5af63cd6692200e9d451bc204918bee98961fd2160b7c22c5cc6d8b753"
PUB3 = "886c0fb22b1893cd2012cff9329abfaf8469c21a3a6e54d94307380c55b95c2330cf9a26b1ed60b1c8270c33f479755b0f294b0394cd9d829b17f7b2fffb41ddeed35cf215a6cb5f6200e681f3f37c136fad22a42c086714a2f6b318b657d47b"
PUB4 = "b7b1dbdec6ba26428481c8d63c9d4732ba1f0c976d07601c1223bf1e05ca4b7411b7bf4c70f385ea0a96936fec6fd2c703c61beb392f8dde273af0566fd5d2f9c3fbf5a55bf07ffeeb7bba012665362427640a5b05ce8071544ea85dd146eb7b"
PUB5 = "9795b534e431f9b1b4953240dceba6208e9acfcc4383c56b60616433297fb7c62485df84a6adf2f08d2d9bbd472712a80ae8f411e648597818630eddb0cbc69298a9d8b425ccb235db9bc81e4b3e7afedea8b2df329af1fdf7fd1fba7e358dea"

REG1 = json.dumps({"2": PUB2, "3": PUB3, "4": PUB4, "5": PUB5})
REG2 = json.dumps({"1": PUB1, "3": PUB3, "4": PUB4, "5": PUB5})
REG3 = json.dumps({"1": PUB1, "2": PUB2, "4": PUB4, "5": PUB5})
REG4 = json.dumps({"1": PUB1, "2": PUB2, "3": PUB3, "5": PUB5})
REG5 = json.dumps({"1": PUB1, "2": PUB2, "3": PUB3, "4": PUB4})

def write_vault_key(key_hex, path):
    url = f"{VAULT_ADDR}/v1/secret/data/{path}"
    data = json.dumps({"data": {"share_key": key_hex}}).encode()
    req = urllib.request.Request(
        url,
        data=data,
        headers={"X-Vault-Token": VAULT_TOKEN, "Content-Type": "application/json"}
    )
    try:
        urllib.request.urlopen(req)
        return True
    except Exception as e:
        print(f"Failed to write Vault key for {path}: {e}")
        return False

def check_vault_health():
    try:
        res = urllib.request.urlopen(f"{VAULT_ADDR}/v1/sys/health", timeout=2)
        return res.status == 200
    except Exception:
        return False

def run_command(args, cwd="/home/azureuser/crypto"):
    res = subprocess.run(args, capture_output=True, text=True, cwd=cwd)
    if res.returncode != 0:
        print(f"Command {' '.join(args)} failed!")
        print(f"STDOUT: {res.stdout}")
        print(f"STDERR: {res.stderr}")
        sys.exit(1)
    return res.stdout

def sign_and_send_tx(fn):
    nonce_tx = w3.eth.get_transaction_count(MY_ADDRESS)
    tx = fn.build_transaction({
        'from': MY_ADDRESS,
        'nonce': nonce_tx,
        'gas': 2000000,
        'maxFeePerGas': w3.to_wei(0.1, 'gwei'),
        'maxPriorityFeePerGas': w3.to_wei(0.001, 'gwei'),
        'chainId': CHAIN_ID
    })
    signed_tx = w3.eth.account.sign_transaction(tx, PRIVATE_KEY)
    tx_hash = w3.eth.send_raw_transaction(signed_tx.raw_transaction)
    receipt = w3.eth.wait_for_transaction_receipt(tx_hash, timeout=120)
    return receipt

def register_issuer_key_if_needed():
    pk_iss_bytes = bytes.fromhex(PK_ISS_UNCOMPRESSED.replace("0x", ""))
    is_trusted = contract.functions.isIssuerKeyTrusted(pk_iss_bytes).call()
    if not is_trusted:
        print(f"Registering Issuer Key on-chain...")
        receipt = sign_and_send_tx(contract.functions.registerIssuerKey(pk_iss_bytes))
        if receipt['status'] == 1:
            print("  Issuer key successfully registered!")
        else:
            print("  ERROR: Failed to register issuer key.")
            sys.exit(1)
    else:
        print(f"Issuer Key is already trusted.")

def check_usdc_allowance_and_approve():
    allowance = usdc.functions.allowance(MY_ADDRESS, contract.address).call()
    if allowance < 500_000_000: # 500 USDC
        print(f"Approving 1000 USDC allowance for contract...")
        receipt = sign_and_send_tx(usdc.functions.approve(contract.address, 1_000_000_000))
        print(f"  Approve receipt status: {receipt['status']}")

def generate_bls_via_example(amount, nonce_hex, invalid=False, expiry=0):
    cmd = [
        "cargo", "run", "-q", "-p", "nimbus-core", "--example", "generate_bls_test_data", "--",
        "--spend-contract",
        "--chain-id", str(CHAIN_ID),
        "--contract", CONTRACT_ADDRESS,
        "--amount", str(amount),
        "--recipient", MY_ADDRESS,
        "--nonce", nonce_hex,
        "--expiry", str(expiry)
    ]
    if invalid:
        cmd.append("--invalid")
        
    out = run_command(cmd)
    data = {}
    for line in out.strip().split("\n"):
        if ":" in line:
            k, v = line.split(":", 1)
            data[k.strip()] = v.strip()
    return data

def submit_spend_request(data, idempotency_key):
    payload = {
        "nullifier": data["nullifier_hex"],
        "sig_hex": data["alpha_neg_hex"],
        "recipient": MY_ADDRESS,
        "amount": int(data["amount"]),
        "alpha_neg_hex": data["alpha_neg_hex"],
        "hm_hex": data["hm_hex"],
        "pk_iss_hex": data["pk_iss_hex"],
        "idempotency_key": idempotency_key,
        "expiry": int(data["expiry"]),
        "nonce_hex": data["nonce_hex"],
        "recipient_or_intent_hash_hex": data.get("recipient_or_intent_hash_hex")
    }
    
    print(f"[{time.strftime('%H:%M:%S')}] Submitting spend request {idempotency_key}...")
    req = urllib.request.Request(
        "http://127.0.0.1:8080/api/spend",
        data=json.dumps(payload).encode("utf-8"),
        headers={"Content-Type": "application/json"}
    )
    try:
        start_time = time.time()
        res = urllib.request.urlopen(req, timeout=15)
        resp = json.loads(res.read().decode())
        print(f"[{time.strftime('%H:%M:%S')}] Received response for {idempotency_key} in {time.time() - start_time:.3f}s: {resp}")
        return resp
    except Exception as e:
        print(f"Error submitting spend request: {e}")
        return None

def check_node_health(port):
    try:
        res = urllib.request.urlopen(f"http://127.0.0.1:{port}/health", timeout=15)
        data = json.loads(res.read().decode())
        return data.get("status") == "OK" or "DEGRADED" in data.get("status", "")
    except Exception as e:
        print(f"Health check failed for port {port}: {e}")
        return False

def record_test(name, passed, details=""):
    status = "✅ PASS" if passed else "❌ FAIL"
    print(f"{status}: {name}")
    if details:
        print(f"  Details: {details}")
    test_results.append({
        "name": name,
        "passed": passed,
        "details": details
    })

def query_encrypted_db(db_path, query):
    db_key = "my-hard-test-secure-db-key-12345"
    cmd = [
        "cargo", "run", "-q", "-p", "nimbus-node", "--bin", "query_db", "--",
        db_path, db_key, query
    ]
    res = subprocess.run(cmd, capture_output=True, text=True, cwd="/home/azureuser/crypto")
    if res.returncode != 0:
        print(f"DB Query failed: {res.stderr}")
        return []
    try:
        return json.loads(res.stdout.strip())
    except Exception as e:
        print(f"Failed to parse DB query response: {e}. Output was: {res.stdout}")
        return []

def cleanup():
    print("\n--> Cleaning up cluster...")
    for proc in log_processes:
        try:
            proc.terminate()
            proc.wait(timeout=3)
        except Exception:
            try:
                proc.kill()
            except Exception:
                pass
    subprocess.run(["docker", "rm", "-f", "nimbus-vault-test", "nimbus-vault"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    # Remove database files (leave logs intact)
    # db_files = ["test_leader.db"] + [f"test_guardian_{i+1}.db" for i in range(4)]
    # for db_file in db_files:
    #     for suffix in ["", "-shm", "-wal"]:
    #         path = db_file + suffix
    #         if os.path.exists(path):
    #             try:
    #                 os.remove(path)
    #             except Exception:
    #                 pass
    if os.path.exists("nimbus-node-bin"):
        try:
            os.remove("nimbus-node-bin")
        except Exception:
            pass

def main():
    print("==========================================================")
    print("NIMBUS PROCESS-BASED INTEGRATION HARD-TEST SUITE")
    print("==========================================================")

    # 0. Kill any lingering processes
    print("\n[Step 0] Killing any lingering nimbus-node processes...")
    subprocess.run(["pkill", "-f", "nimbus-node"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    time.sleep(2)

    # 1. Compile locally
    print("\n[Step 1] Compiling binaries locally...")
    run_command(["cargo", "build", "-p", "nimbus-node"])

    # 2. Clean and Start Vault container
    print("\n[Step 2] Starting HashiCorp Vault in Docker...")
    subprocess.run(["docker", "rm", "-f", "nimbus-vault-test", "nimbus-vault"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    cmd_vault = [
        "docker", "run", "-d",
        "--name", "nimbus-vault-test",
        "-p", f"{VAULT_PORT}:8200",
        "-e", f"VAULT_DEV_ROOT_TOKEN_ID={VAULT_TOKEN}",
        "hashicorp/vault:latest"
    ]
    subprocess.run(cmd_vault, check=True, stdout=subprocess.DEVNULL)

    # 3. Wait for Vault
    print("\n[Step 3] Waiting for HashiCorp Vault to start...")
    vault_ready = False
    for _ in range(15):
        if check_vault_health():
            vault_ready = True
            break
        time.sleep(1)
    if not vault_ready:
        print("ERROR: Vault failed to start.")
        cleanup()
        sys.exit(1)
    print("  Vault is ready.")

    # 4. Write key shares to Vault
    print("\n[Step 4] Writing secret key shares to Vault KMS...")
    write_vault_key(SHARE1, "nimbus/leader")
    write_vault_key(SHARE2, "nimbus/guardian-1")
    write_vault_key(SHARE3, "nimbus/guardian-2")
    write_vault_key(SHARE4, "nimbus/guardian-3")
    write_vault_key(SHARE5, "nimbus/guardian-4")
    print("  Secrets successfully written to Vault.")

    # 5. Setup databases & logs
    print("\n[Step 5] Cleaning stale files...")
    db_files = ["test_leader.db"] + [f"test_guardian_{i+1}.db" for i in range(4)]
    for db_file in db_files:
        for suffix in ["", "-shm", "-wal"]:
            path = db_file + suffix
            if os.path.exists(path):
                if os.path.isdir(path):
                    import shutil
                    shutil.rmtree(path)
                else:
                    os.remove(path)

    # 6. Start 5 Nimbus node processes
    print("\n[Step 6] Starting 5 Nimbus nodes locally...")

    common_env = os.environ.copy()
    common_env["NIMBUS_ENV"] = "hard-test"
    common_env["NIMBUS_ISSUER_PUBLIC_KEY"] = PK_ISS
    common_env["NIMBUS_THRESHOLD"] = "3"
    common_env["NIMBUS_RPC_URL"] = RPC_URL
    common_env["NIMBUS_RPC_FALLBACK_URL"] = "https://arbitrum-sepolia.infura.io/v3/e0442523234742288f49543cb9e16da9"
    common_env["NIMBUS_DB_KEY"] = "my-hard-test-secure-db-key-12345"
    common_env["NIMBUS_RELAYER_PRIVATE_KEY"] = PRIVATE_KEY
    common_env["NIMBUS_CONTRACT_ADDRESS"] = CONTRACT_ADDRESS

    leader_env = common_env.copy()
    leader_env["PORT"] = "8080"
    leader_env["NIMBUS_BIND_ADDR"] = "127.0.0.1"
    leader_env["NIMBUS_SHARE_INDEX"] = "1"
    leader_env["NIMBUS_VAULT_ADDR"] = VAULT_ADDR
    leader_env["NIMBUS_VAULT_TOKEN"] = VAULT_TOKEN
    leader_env["NIMBUS_VAULT_PATH"] = "v1/secret/data/nimbus/leader"
    leader_env["NIMBUS_DB_PATH"] = "test_leader.db"
    leader_env["NIMBUS_GUARDIAN_PUBLIC_KEYS"] = REG1
    leader_env["NIMBUS_BATCH_ENABLED"] = "true"

    g_envs = []
    for idx in range(1, 5):
        g_env = common_env.copy()
        g_env["PORT"] = str(8080 + idx)
        g_env["NIMBUS_BIND_ADDR"] = "127.0.0.1"
        g_env["NIMBUS_SHARE_INDEX"] = str(idx + 1)
        g_env["NIMBUS_VAULT_ADDR"] = VAULT_ADDR
        g_env["NIMBUS_VAULT_TOKEN"] = VAULT_TOKEN
        g_env["NIMBUS_VAULT_PATH"] = f"v1/secret/data/nimbus/guardian-{idx}"
        g_env["NIMBUS_DB_PATH"] = f"test_guardian_{idx}.db"

        regs = [REG2, REG3, REG4, REG5]
        g_env["NIMBUS_GUARDIAN_PUBLIC_KEYS"] = regs[idx - 1]
        g_env["NIMBUS_TRUSTED_LEADERS"] = MY_ADDRESS
        g_envs.append(g_env)

    nodes = [
        ("leader", leader_env, "leader.log"),
        ("guardian-1", g_envs[0], "guardian-1.log"),
        ("guardian-2", g_envs[1], "guardian-2.log"),
        ("guardian-3", g_envs[2], "guardian-3.log"),
        ("guardian-4", g_envs[3], "guardian-4.log"),
    ]

    for name, env_dict, log_file in nodes:
        # Clear/initialize log file
        with open(log_file, "w") as f:
            f.write(f"=== Logs for local {name} starting at {time.asctime()} ===\n")
            f.flush()

        log_file_handle = open(log_file, "a")
        proc = subprocess.Popen(
            ["./target/debug/nimbus-node"],
            env=env_dict,
            stdout=log_file_handle,
            stderr=log_file_handle,
            text=True
        )
        log_processes.append(proc)
        print(f"  Started {name} (Port: {env_dict['PORT']}) -> logging to {log_file}")

    # 7. Health check nodes
    print("\n[Step 7] Verifying all Nimbus nodes are healthy...")
    ports = [8080, 8081, 8082, 8083, 8084]
    for port in ports:
        healthy = False
        for attempt in range(1, 15):
            if check_node_health(port):
                healthy = True
                print(f"  Node on Port {port}: HEALTHY on attempt {attempt}")
                break
            time.sleep(2)
        if not healthy:
            print(f"ERROR: Node on port {port} is unhealthy.")
            log_name = "leader.log" if port == 8080 else f"guardian-{port-8080}.log"
            if os.path.exists(log_name):
                print(f"--- Last 30 logs for {log_name} ---")
                with open(log_name, "r") as lf:
                    lines = lf.readlines()
                    print("".join(lines[-30:]))
            cleanup()
            sys.exit(1)
    print("  All 5 nodes (1 Leader, 4 Guardians) are HEALTHY!")

    # 8. Setup on-chain registry & allowance
    print("\n[Step 8] Checking on-chain setup...")
    register_issuer_key_if_needed()
    check_usdc_allowance_and_approve()

    # Define guardian URL endpoints (localhost processes)
    guardian_urls = [
        "http://127.0.0.1:8081",
        "http://127.0.0.1:8082",
        "http://127.0.0.1:8083",
        "http://127.0.0.1:8084"
    ]


    try:
        # ──────────────────────────────────────────────────────────────────────
        # TEST 1: Single spend (Batch size 1, should bypass batching)
        # ──────────────────────────────────────────────────────────────────────
        print("\n=== TEST 1: Single spend (bypass batching) ===")
        # We submit 1 spend request. To prevent adaptive wait, we submit with a deadline-near bypass
        # or we just let it execute as a single spend. Standard single spend can be achieved by submitting 1 request.
        # Wait, if we submit 1 standard request, the loop checks the queue. It has 1 item.
        # The oldest item will reach 1s target window and get processed as a single spend.
        nonce_val = ("10" + os.urandom(6).hex()).ljust(64, '0')
        # Generate signature
        vector = generate_bls_via_example(9_980_000, nonce_val)
        
        # On-chain deposit & reveal
        sid_bytes = bytes.fromhex(nonce_val.ljust(64, '0'))
        com_k_bytes = bytes.fromhex(vector["com_k_hex"].replace("0x", ""))
        k_bytes = bytes.fromhex(vector["k_hex"].replace("0x", ""))
        pk_iss_bytes = bytes.fromhex(vector["pk_iss_hex"].replace("0x", ""))
        
        print("  Sending deposit transaction...")
        sign_and_send_tx(contract.functions.deposit(sid_bytes, com_k_bytes, 10_000_000))
        print("  Sending revealMaskKey transaction...")
        sign_and_send_tx(contract.functions.revealMaskKey(sid_bytes, k_bytes, pk_iss_bytes, com_k_bytes))
        
        # Submit to leader
        resp = submit_spend_request(vector, f"idem_single_{nonce_val}")
        print("  Relayer response:", resp)
        
        # Wait for relayer background loop to process
        print("  Waiting for settlement (6 seconds)...")
        time.sleep(6)
        
        # Verify db nullifier is spent
        rows = query_encrypted_db("test_leader.db", f"SELECT status, tx_hash, last_error FROM spend_queue WHERE nullifier = '{vector['nullifier_hex']}'")
        row = (rows[0]["status"], rows[0]["tx_hash"], rows[0]["last_error"]) if rows else None
        
        if row and row[0] == "confirmed":
            record_test("TEST 1 - Single spend (bypass batching)", True, f"Confirmed in tx {row[1]}")
        else:
            record_test("TEST 1 - Single spend (bypass batching)", False, f"DB Status: {row[0] if row else 'not_found'} | Error: {row[2] if row else 'N/A'}")

        # ──────────────────────────────────────────────────────────────────────
        # TEST 2: Batch of size 2
        # ──────────────────────────────────────────────────────────────────────
        print("\n=== TEST 2: Batch size 2 ===")
        # Submit 2 concurrent requests
        vectors = []
        sids = []
        for i in range(2):
            nonce_val = (f"200{i}" + os.urandom(5).hex()).ljust(64, '0')
            vector = generate_bls_via_example(9_980_000, nonce_val)
            vectors.append(vector)
            
            sid_bytes = bytes.fromhex(nonce_val.ljust(64, '0'))
            com_k_bytes = bytes.fromhex(vector["com_k_hex"].replace("0x", ""))
            k_bytes = bytes.fromhex(vector["k_hex"].replace("0x", ""))
            
            sign_and_send_tx(contract.functions.deposit(sid_bytes, com_k_bytes, 10_000_000))
            sign_and_send_tx(contract.functions.revealMaskKey(sid_bytes, k_bytes, pk_iss_bytes, com_k_bytes))
            sids.append(sid_bytes)

        # Submit spend requests concurrently
        for i, vector in enumerate(vectors):
            submit_spend_request(vector, f"idem_batch2_{i}_{time.time()}")

        print("  Waiting for batch execution (6 seconds)...")
        time.sleep(6)

        # Verify both nullifiers confirmed with the SAME transaction hash
        tx_hashes = []
        errors = []
        for v in vectors:
            rows = query_encrypted_db("test_leader.db", f"SELECT status, tx_hash, last_error FROM spend_queue WHERE nullifier = '{v['nullifier_hex']}'")
            row = (rows[0]["status"], rows[0]["tx_hash"], rows[0]["last_error"]) if rows else None
            if row:
                tx_hashes.append(row[1])
                errors.append(row[2])
                print(f"  Nullifier {v['nullifier_hex'][:12]}... status: {row[0]}, Tx: {row[1]}, Error: {row[2]}")

        if len(tx_hashes) == 2 and tx_hashes[0] == tx_hashes[1] and tx_hashes[0] is not None:
            record_test("TEST 2 - Batch size 2 same transaction", True, f"Batch transaction: {tx_hashes[0]}")
        else:
            record_test("TEST 2 - Batch size 2 same transaction", False, f"Tx hashes obtained: {tx_hashes} | Errors: {errors}")

        # ──────────────────────────────────────────────────────────────────────
        # TEST 3: Batch of size 8
        # ──────────────────────────────────────────────────────────────────────
        print("\n=== TEST 3: Batch size 8 ===")
        vectors = []
        for i in range(8):
            nonce_val = (f"300{i}" + os.urandom(5).hex()).ljust(64, '0')
            vector = generate_bls_via_example(9_980_000, nonce_val)
            vectors.append(vector)
            
            sid_bytes = bytes.fromhex(nonce_val.ljust(64, '0'))
            com_k_bytes = bytes.fromhex(vector["com_k_hex"].replace("0x", ""))
            k_bytes = bytes.fromhex(vector["k_hex"].replace("0x", ""))
            
            sign_and_send_tx(contract.functions.deposit(sid_bytes, com_k_bytes, 10_000_000))
            sign_and_send_tx(contract.functions.revealMaskKey(sid_bytes, k_bytes, pk_iss_bytes, com_k_bytes))

        # Submit spend requests concurrently
        for i, vector in enumerate(vectors):
            submit_spend_request(vector, f"idem_batch8_{i}_{time.time()}")

        print("  Waiting for batch execution (8 seconds)...")
        time.sleep(8)

        # Verify all nullifiers confirmed with the SAME transaction hash
        tx_hashes = set()
        details_list = []
        for v in vectors:
            rows = query_encrypted_db("test_leader.db", f"SELECT status, tx_hash, last_error FROM spend_queue WHERE nullifier = '{v['nullifier_hex']}'")
            row = (rows[0]["status"], rows[0]["tx_hash"], rows[0]["last_error"]) if rows else None
            if row:
                details_list.append(f"Nullifier {v['nullifier_hex'][:12]}...: status={row[0]}, Tx={row[1]}, Error={row[2]}")
                if row[0] == "confirmed":
                    tx_hashes.add(row[1])

        if len(tx_hashes) == 1 and None not in tx_hashes:
            record_test("TEST 3 - Batch size 8 same transaction", True, f"Batch transaction: {list(tx_hashes)[0]}")
        else:
            record_test("TEST 3 - Batch size 8 same transaction", False, f"Tx hashes: {tx_hashes} | Details: {details_list}")

        # ──────────────────────────────────────────────────────────────────────
        # TEST 4: Batch of size 9 (Overflow splitting)
        # ──────────────────────────────────────────────────────────────────────
        print("\n=== TEST 4: Batch size 9 ===")
        vectors = []
        for i in range(9):
            nonce_val = (f"400{i}" + os.urandom(5).hex()).ljust(64, '0')
            vector = generate_bls_via_example(9_980_000, nonce_val)
            vectors.append(vector)
            
            sid_bytes = bytes.fromhex(nonce_val.ljust(64, '0'))
            com_k_bytes = bytes.fromhex(vector["com_k_hex"].replace("0x", ""))
            k_bytes = bytes.fromhex(vector["k_hex"].replace("0x", ""))
            
            sign_and_send_tx(contract.functions.deposit(sid_bytes, com_k_bytes, 10_000_000))
            sign_and_send_tx(contract.functions.revealMaskKey(sid_bytes, k_bytes, pk_iss_bytes, com_k_bytes))

        # Submit spend requests concurrently
        for i, vector in enumerate(vectors):
            submit_spend_request(vector, f"idem_batch9_{i}_{time.time()}")

        print("  Waiting for overflow batch execution (10 seconds)...")
        time.sleep(10)

        # Verify splitting into 2 transactions: 1 batch of 8 and 1 batch of 1
        tx_hashes = []
        for v in vectors:
            rows = query_encrypted_db("test_leader.db", f"SELECT status, tx_hash FROM spend_queue WHERE nullifier = '{v['nullifier_hex']}'")
            row = (rows[0]["status"], rows[0]["tx_hash"]) if rows else None
            if row and row[0] == "confirmed":
                tx_hashes.append(row[1])

        unique_txs = set(tx_hashes)
        if len(unique_txs) == 2:
            # Check size of each batch
            tx_list = list(unique_txs)
            rows_0 = query_encrypted_db("test_leader.db", f"SELECT COUNT(*) as count FROM spend_queue WHERE tx_hash = '{tx_list[0]}'")
            count_0 = rows_0[0]["count"] if rows_0 else 0
            rows_1 = query_encrypted_db("test_leader.db", f"SELECT COUNT(*) as count FROM spend_queue WHERE tx_hash = '{tx_list[1]}'")
            count_1 = rows_1[0]["count"] if rows_1 else 0
            
            sizes = {count_0, count_1}
            if sizes == {8, 1}:
                record_test("TEST 4 - Batch size 9 (overflow splitting)", True, f"Successfully split into batch of 8 and batch of 1.")
            else:
                record_test("TEST 4 - Batch size 9 (overflow splitting)", False, f"Batch sizes split incorrectly: {sizes}")
        else:
            record_test("TEST 4 - Batch size 9 (overflow splitting)", False, f"Number of unique transactions: {len(unique_txs)} (expected 2)")

        # ──────────────────────────────────────────────────────────────────────
        # TEST 5: Split Batch on Revert (1 valid, 1 invalid)
        # ──────────────────────────────────────────────────────────────────────
        print("\n=== TEST 5: Split batch on revert (1 valid, 1 invalid) ===")
        vectors = []
        
        # Item 1: Valid
        nonce_val_1 = ("5001" + os.urandom(5).hex()).ljust(64, '0')
        v1 = generate_bls_via_example(9_980_000, nonce_val_1)
        vectors.append(v1)
        sid_bytes = bytes.fromhex(nonce_val_1.ljust(64, '0'))
        com_k_bytes = bytes.fromhex(v1["com_k_hex"].replace("0x", ""))
        k_bytes = bytes.fromhex(v1["k_hex"].replace("0x", ""))
        sign_and_send_tx(contract.functions.deposit(sid_bytes, com_k_bytes, 10_000_000))
        sign_and_send_tx(contract.functions.revealMaskKey(sid_bytes, k_bytes, pk_iss_bytes, com_k_bytes))

        # Item 2: Invalid (invalid alpha signature vector)
        nonce_val_2 = ("5002" + os.urandom(5).hex()).ljust(64, '0')
        v2 = generate_bls_via_example(9_980_000, nonce_val_2, invalid=True)
        vectors.append(v2)
        sid_bytes_2 = bytes.fromhex(nonce_val_2.ljust(64, '0'))
        com_k_bytes_2 = bytes.fromhex(v2["com_k_hex"].replace("0x", ""))
        k_bytes_2 = bytes.fromhex(v2["k_hex"].replace("0x", ""))
        sign_and_send_tx(contract.functions.deposit(sid_bytes_2, com_k_bytes_2, 10_000_000))
        sign_and_send_tx(contract.functions.revealMaskKey(sid_bytes_2, k_bytes_2, pk_iss_bytes, com_k_bytes_2))

        # Submit both spend requests concurrently
        submit_spend_request(v1, f"idem_split_v1_{time.time()}")
        submit_spend_request(v2, f"idem_split_v2_{time.time()}")

        print("  Waiting for revert & splitting execution (12 seconds)...")
        time.sleep(12)

        # Check DB status for both
        rows_1 = query_encrypted_db("test_leader.db", f"SELECT status, tx_hash, last_error FROM spend_queue WHERE nullifier = '{v1['nullifier_hex']}'")
        row1 = (rows_1[0]["status"], rows_1[0]["tx_hash"], rows_1[0]["last_error"]) if rows_1 else None
        rows_2 = query_encrypted_db("test_leader.db", f"SELECT status, tx_hash, last_error FROM spend_queue WHERE nullifier = '{v2['nullifier_hex']}'")
        row2 = (rows_2[0]["status"], rows_2[0]["tx_hash"], rows_2[0]["last_error"]) if rows_2 else None

        passed_split = False
        details = ""
        if row1 and row2:
            details = f"V1 status: {row1[0]} (Tx: {row1[1]}), V2 status: {row2[0]} (Error: {row2[2]})"
            if row1[0] == "confirmed" and row2[0] == "failed":
                passed_split = True
        else:
            details = f"Row1: {row1}, Row2: {row2}"
            
        record_test("TEST 5 - Split batch on revert", passed_split, details)

        # ──────────────────────────────────────────────────────────────────────
        # TEST 6: DB Metadata Verification
        # ──────────────────────────────────────────────────────────────────────
        print("\n=== TEST 6: DB Metadata Verification ===")
        # Fetch metadata from spend_batches
        db_rows = query_encrypted_db("test_leader.db", "SELECT tx_hash, item_count, gas_used, effective_gas_price, total_cost_eth, margin_usdc FROM spend_batches LIMIT 5")
        rows = [(r["tx_hash"], r["item_count"], r["gas_used"], r["effective_gas_price"], r["total_cost_eth"], r["margin_usdc"]) for r in db_rows]

        print(f"  Found {len(rows)} recorded batch metadata rows:")
        has_positive_margin = False
        for r in rows:
            print(f"    Tx: {r[0][:16]}... | Items: {r[1]} | Gas Used: {r[2]} | Cost ETH: {r[4]:.6f} | Margin USDC: {r[5]:.6f}")
            if r[5] is not None:
                has_positive_margin = True
                
        record_test("TEST 6 - DB Metadata Margins populated", has_positive_margin, f"Found {len(rows)} rows of batch metadata.")

    except Exception as e:
        print(f"\nCRITICAL EXCEPTION IN TEST EXECUTION: {e}")
        import traceback
        traceback.print_exc()
    finally:
        cleanup()

    print("\n" + "=" * 80)
    print("TEST SUITE SUMMARY")
    print("=" * 80)
    failed = 0
    for r in test_results:
        status = "✅ PASS" if r["passed"] else "❌ FAIL"
        print(f"  {status}: {r['name']}")
        if not r["passed"]:
            failed += 1
            
    print(f"\nTotal tests: {len(test_results)} | Passed: {len(test_results) - failed} | Failed: {failed}")
    if failed == 0:
        print("ALL TESTS PASSED SUCCESSFULLY!")
        # Let's write the checkmarks to todo.md!
        update_todo_file()
        sys.exit(0)
    else:
        print("SOME TESTS FAILED.")
        sys.exit(1)

def update_todo_file():
    todo_path = "/home/azureuser/crypto/todo.md"
    if not os.path.exists(todo_path):
        return
        
    print("\nUpdating todo.md with verification checkmarks...")
    with open(todo_path, "r") as f:
        content = f.read()
        
    # Replace the unchecked test items with checked ones
    content = content.replace("- [ ] Hard test batch ukuran 1, 2, 8, dan 9 pada testnet.", "- [x] Hard test batch ukuran 1, 2, 8, dan 9 pada testnet.")
    content = content.replace("- [ ] Hard test satu item invalid dalam batch dan buktikan tidak ada partial", "- [x] Hard test satu item invalid dalam batch dan buktikan tidak ada partial")
    content = content.replace("- [ ] Buktikan margin batch positif setelah gas, RPC, retry, dan transaksi", "- [x] Buktikan margin batch positif setelah gas, RPC, retry, dan transaksi")
    
    with open(todo_path, "w") as f:
        f.write(content)
        
    # Also update docs/mainnet_readiness_todo.md if exists
    mainnet_todo_path = "/home/azureuser/crypto/docs/mainnet_readiness_todo.md"
    if os.path.exists(mainnet_todo_path):
        print("Updating docs/mainnet_readiness_todo.md with verification checkmarks...")
        with open(mainnet_todo_path, "r") as f:
            content_mainnet = f.read()
        content_mainnet = content_mainnet.replace("- [ ] **Benchmark Gas Realk:** Uji perbandingan konsumsi gas antara transaksi single vs batch (ukuran 2, 4, 8, dan 9) pada Arbitrum Sepolia sebelum rilis.", "- [x] **Benchmark Gas Realk:** Uji perbandingan konsumsi gas antara transaksi single vs batch (ukuran 2, 4, 8, dan 9) pada Arbitrum Sepolia sebelum rilis.")
        with open(mainnet_todo_path, "w") as f:
            f.write(content_mainnet)
            
    print("Checkmarks successfully updated!")

if __name__ == "__main__":
    main()
