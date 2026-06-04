#!/usr/bin/env python3
import subprocess
import time
import sys
import os
import urllib.request
import urllib.parse
import json

# Integration Testing & Simulation script for Item 7
# Spawns 1 Leader Node and 4 Guardian Nodes (t=3, n=5)
# Executes complete end-to-end threshold blind signature signing and verification.

print("==========================================================")
print("NIMBUS 5-NODE DISTRIBUTED RELAYER CLUSTER SIMULATION")
print("==========================================================")

# Step 1: Compile binaries
print("Compiling nimbus-cli and nimbus-node...")
try:
    subprocess.run(["cargo", "build", "-p", "nimbus-cli", "-p", "nimbus-node"], check=True)
except subprocess.CalledProcessError:
    print("Compilation failed!")
    sys.exit(1)

# Step 2: Generate keypair
print("\n[Step 2] Generating Issuer keys...")
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

print(f"  Issuer Secret Key (sk_iss) : {sk_iss[:12]}...{sk_iss[-12:]}")
print(f"  Issuer Public Key (pk_iss) : {pk_iss[:12]}...{pk_iss[-12:]}")

# Step 3: Split secret key into 5 shares
print("\n[Step 3] Splitting key using Shamir Secret Sharing (t=3, n=5)...")
proc_split = subprocess.run([cli_path, "split-key", "-s", sk_iss, "-t", "3", "-n", "5"], capture_output=True, text=True, check=True)
output_split = proc_split.stdout

shares = []
for line in output_split.split("\n"):
    if line.strip().startswith("Share"):
        pass
    elif len(line.strip()) == 80:  # Hex representation of index + scalar (8 + 32 bytes)
        shares.append(line.strip())

if len(shares) != 5:
    print("Error: Expected to split into 5 shares, but got:", len(shares))
    sys.exit(1)

for i, share in enumerate(shares):
    print(f"  Share {i+1}: {share[:12]}...{share[-12:]}")

# Step 4: Startup the 5 nodes
print("\n[Step 4] Launching relayer nodes (1 Leader, 4 Guardians)...")
nodes = []
# Node 1 is Leader on 8080. Nodes 2-5 are Guardians on 8081-8084.
for i in range(5):
    port = str(8080 + i)
    env = os.environ.copy()
    env["PORT"] = port
    env["NIMBUS_SHARE_INDEX"] = str(i + 1)
    env["NIMBUS_SHARE_KEY"] = shares[i]
    
    # Run the relayer node process
    proc = subprocess.Popen(["./target/debug/nimbus-node"], env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    nodes.append((port, proc))
    print(f"  Node {i+1} (Port {port}): Started Index {i+1}")

# Step 5: Wait for all nodes to start up and check health
time.sleep(3)
print("\n[Step 5] Checking nodes health...")
for port, _ in nodes:
    try:
        response = urllib.request.urlopen(f"http://127.0.0.1:{port}/health", timeout=2)
        data = json.loads(response.read().decode())
        print(f"  Node on Port {port}: HEALTHY (status: {data.get('status')})")
    except Exception as e:
        print(f"  Node on Port {port}: UNHEALTHY or unreachable: {e}")
        # Clean up and exit
        for _, p in nodes:
            p.terminate()
        sys.exit(1)

# Step 6: Perform blind signature ceremony
print("\n[Step 6] Performing client-side blinding of message...")
message = "private_intent_swap_shares_polymarket"
proc_blind = subprocess.run([cli_path, "blind", "-m", message], capture_output=True, text=True, check=True)
output_blind = proc_blind.stdout

blinded_msg = ""
r_factor = ""
for line in output_blind.split("\n"):
    if len(line.strip()) == 96:  # G1 point is 48 bytes (96 hex chars)
        blinded_msg = line.strip()
    elif len(line.strip()) == 64:  # scalar is 32 bytes (64 hex chars)
        r_factor = line.strip()

print(f"  Blinded Message (X) : {blinded_msg[:12]}...")
print(f"  Blinding Factor (r) : {r_factor[:12]}...")

# Step 7: Call Leader Node to collect signatures from Guardians
print("\n[Step 7] Initiating leader signing request to port 8080...")
payload = {
    "blinded_hex": blinded_msg,
    "guardian_urls": [
        "http://127.0.0.1:8081",
        "http://127.0.0.1:8082",
        "http://127.0.0.1:8083",
        "http://127.0.0.1:8084"
    ],
    "pk_iss_hex": pk_iss
}

req = urllib.request.Request(
    "http://127.0.0.1:8080/api/leader/sign",
    data=json.dumps(payload).encode("utf-8"),
    headers={"Content-Type": "application/json"}
)

try:
    response = urllib.request.urlopen(req, timeout=5)
    result = json.loads(response.read().decode())
    print("  Leader sign call: SUCCESS")
    com_k = result["com_k_hex"]
    k_val = result["k_hex"]
    sigs = result["partial_signatures"]
    print(f"    Commitment (com_k) : {com_k[:12]}...")
    print(f"    Masking Key (k)    : {k_val[:12]}...")
    print(f"    Received {len(sigs)} partial signatures:")
    for sig in sigs:
        print(f"      - Index {sig['index']}: {sig['signature_hex'][:12]}...")
except Exception as e:
    print(f"  Leader signing failed: {e}")
    for _, p in nodes:
        p.terminate()
    sys.exit(1)

# Step 8: Aggregate threshold signatures (need t=3 signatures)
# We take the first 3 partial signatures
indices_str = ",".join(str(s["index"]) for s in sigs[:3])
sigs_str = ",".join(s["signature_hex"] for s in sigs[:3])

print(f"\n[Step 8] Aggregating partial signatures from indices: {indices_str}...")
proc_agg = subprocess.run([
    cli_path, "aggregate",
    "-i", indices_str,
    "-s", sigs_str
], capture_output=True, text=True, check=True)
output_agg = proc_agg.stdout

masked_sig = ""
for line in output_agg.split("\n"):
    if len(line.strip()) == 96:  # G1 point (96 hex chars)
        masked_sig = line.strip()

print(f"  Aggregated Masked Signature (sigma_tilde): {masked_sig[:12]}...")

# Step 9: Unmask signature
print("\n[Step 9] Unmasking signature once masking key is revealed...")
proc_unmask = subprocess.run([
    cli_path, "unmask",
    "-m", masked_sig,
    "-r", r_factor,
    "-k", k_val
], capture_output=True, text=True, check=True)
output_unmask = proc_unmask.stdout

unmasked_sig = ""
for line in output_unmask.split("\n"):
    if len(line.strip()) == 96:  # G1 point (96 hex chars)
        unmasked_sig = line.strip()

print(f"  Unmasked Signature (alpha): {unmasked_sig[:12]}...")

# Step 10: Verify signature
print("\n[Step 10] Verifying final unblinded signature against Issuer Public Key...")
proc_verify = subprocess.run([
    cli_path, "verify",
    "-m", message,
    "-s", unmasked_sig,
    "-p", pk_iss
], capture_output=True, text=True, check=True)
output_verify = proc_verify.stdout

print(output_verify.strip())

# Clean up relayer processes
print("\nTerminating relayer nodes...")
for port, p in nodes:
    p.terminate()
print("Cluster shut down successfully.")
print("==========================================================")
print("INTEGRATION TEST PASSED SUCCESSFULLY!")
print("==========================================================")
