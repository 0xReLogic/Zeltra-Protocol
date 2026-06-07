#!/usr/bin/env python3
import subprocess
import time
import sys
import os
import urllib.request
import urllib.parse
import json
import sqlite3

def env_required(name):
    value = os.environ.get(name)
    if not value:
        print(f"Error: missing required environment variable: {name}")
        sys.exit(2)
    return value


print("==========================================================")
print("NIMBUS END-TO-END CCIP INTEGRATION TESTER")
print("==========================================================")

# 1. Compile relayer and CLI binaries
print("\n[Step 1] Compiling nimbus-node and nimbus-cli...")
try:
    subprocess.run(["cargo", "build", "-p", "nimbus-cli", "-p", "nimbus-node"], check=True)
except subprocess.CalledProcessError:
    print("Compilation failed!")
    sys.exit(1)

# 2. Generate key and split into shares using CLI
print("\n[Step 2] Generating keys and shares using nimbus-cli...")
cli_path = "./target/debug/nimbus-cli"
proc_keys = subprocess.run([cli_path, "generate-keys"], capture_output=True, text=True, check=True)
output_keys = proc_keys.stdout

sk_iss = ""
pk_iss = ""
for line in output_keys.split("\n"):
    if line.strip() and not line.startswith("NEW") and not line.startswith("---") and not line.startswith("Secret Key") and not line.startswith("Public Key"):
        if not sk_iss:
            sk_iss = line.strip()
        else:
            pk_iss = line.strip()

proc_split = subprocess.run([cli_path, "split-key", "-s", sk_iss, "-t", "3", "-n", "5"], capture_output=True, text=True, check=True)
output_split = proc_split.stdout

shares = []
for line in output_split.split("\n"):
    if len(line.strip()) == 80:
        shares.append(line.strip())

if not shares:
    print("Error: Failed to split keys into shares")
    sys.exit(1)

share_key = shares[0]
print(f"  Generated Share Key: {share_key[:12]}...")

# 3. Setup Relayer Node Environment Variables
print("\n[Step 3] Setting up relayer node configuration...")
env = os.environ.copy()
env["PORT"] = "8099"
env["NIMBUS_ENV"] = "test"
env["NIMBUS_SHARE_KEY"] = share_key
env["NIMBUS_RPC_URL"] = env_required("NIMBUS_RPC_URL")
if os.environ.get("NIMBUS_RPC_FALLBACK_URL"):
    env["NIMBUS_RPC_FALLBACK_URL"] = os.environ["NIMBUS_RPC_FALLBACK_URL"]
env["NIMBUS_RELAYER_PRIVATE_KEY"] = env_required("NIMBUS_RELAYER_PRIVATE_KEY")
env["NIMBUS_CONTRACT_ADDRESS"] = env_required("NIMBUS_CONTRACT_ADDRESS")
env["NIMBUS_DB_PATH"] = "nimbus_e2e_test.db"

# Remove old test DB if exists
if os.path.exists("nimbus_e2e_test.db"):
    os.remove("nimbus_e2e_test.db")

# 4. Start Relayer Node Process
print("\n[Step 4] Launching relayer node process...")
node_proc = subprocess.Popen(
    ["./target/debug/nimbus-node"],
    env=env,
    stdout=subprocess.PIPE,
    stderr=subprocess.STDOUT,
    text=True,
    bufsize=1
)

# Wait for node startup by polling health check
healthy = False
for _ in range(15):
    time.sleep(1)
    try:
        response = urllib.request.urlopen("http://127.0.0.1:8099/health", timeout=1)
        data = json.loads(response.read().decode())
        if data.get("status") == "OK" or "DEGRADED" in data.get("status", ""):
            healthy = True
            print("  Relayer Node is HEALTHY and listening on port 8099.")
            break
    except Exception:
        pass

if not healthy:
    print("Error: Relayer node failed to start or is unhealthy.")
    # Print node output for debugging
    node_proc.terminate()
    stdout, _ = node_proc.communicate()
    print("Node logs:")
    print(stdout)
    sys.exit(1)

# 5. Send CCIP Cross-Chain Spend Request to Relayer Node
print("\n[Step 5] Submitting real CCIP Cross-Chain Spend request...")
dummy_nullifier = "0x" + os.urandom(32).hex()
dummy_alpha_neg = "0x" + os.urandom(128).hex()
dummy_hm = "0x" + os.urandom(128).hex()
dummy_pk_iss = "0x" + os.urandom(256).hex()
recipient_address = "0x23e32d309c575a3d5e7cd2867be12b00efa44bb1"

payload = {
    "nullifier": dummy_nullifier,
    "sig_hex": "0x" + os.urandom(64).hex(),
    "recipient": recipient_address,
    "amount": 5000000, # 5 USDC (minimum limit)
    "alpha_neg_hex": dummy_alpha_neg,
    "hm_hex": dummy_hm,
    "pk_iss_hex": dummy_pk_iss,
    "cross_chain": {
        "destination_chain_selector": 10344971235874465080, # Base Sepolia
        "destination_contract": "0x208f0e4390f59e3052c557bf23a47b2ab4697a10"
    }
}

req = urllib.request.Request(
    "http://127.0.0.1:8099/api/spend",
    data=json.dumps(payload).encode("utf-8"),
    headers={"Content-Type": "application/json"}
)

try:
    response = urllib.request.urlopen(req, timeout=5)
    result = json.loads(response.read().decode())
    print("  Spend request status:", result.get("status"))
    print("  Message:", result.get("message"))
    print("  Queue position:", result.get("queue_position"))
except Exception as e:
    print(f"Error submitting spend request: {e}")
    node_proc.terminate()
    sys.exit(1)

# 6. Wait for Batch Processing and Verification
print("\n[Step 6] Waiting for batch execution (Relayer loops every 2 seconds)...")
time.sleep(6) # Let the relayer process the queue

# 7. Check SQLite Database to verify the nullifier is stored permanently
print("\n[Step 7] Checking SQLite database persistence...")
try:
    conn = sqlite3.connect("nimbus_e2e_test.db")
    cursor = conn.cursor()
    cursor.execute("SELECT nullifier FROM nullifiers WHERE nullifier = ?", (dummy_nullifier,))
    row = cursor.fetchone()
    conn.close()
    if row:
        print(f"  SUCCESS: Nullifier {dummy_nullifier[:12]}... recorded in SQLite database.")
    else:
        print("  WARNING: Nullifier not found in SQLite database yet.")
except Exception as e:
    print(f"Error querying SQLite database: {e}")

# 8. Shutdown Relayer Process and print stdout logs
print("\n[Step 8] Terminating relayer node and retrieving stdout logs...")
node_proc.terminate()
time.sleep(1)

stdout, _ = node_proc.communicate()
print("==========================================================")
print("RELAYER NODE OUTPUT LOGS:")
print("==========================================================")
print(stdout)
print("==========================================================")

# Verify if broadcast logs exist
if "Real CCIP transaction broadcasted successfully" in stdout:
    print("\nE2E CCIP INTEGRATION TEST PASSED SUCCESSFULLY!")
    print("==========================================================")
    sys.exit(0)
else:
    print("\nE2E CCIP INTEGRATION TEST FAILED!")
    print("==========================================================")
    sys.exit(1)
