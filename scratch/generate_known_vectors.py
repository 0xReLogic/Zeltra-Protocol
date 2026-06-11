import subprocess
import json
import re

def run_generator(args):
    cmd = ["cargo", "run", "-q", "-p", "nimbus-core", "--example", "generate_bls_test_data", "--"] + args
    res = subprocess.run(cmd, capture_output=True, text=True, check=True)
    
    # Parse the stdout lines into a dictionary
    data = {}
    for line in res.stdout.strip().split("\n"):
        if ":" in line:
            parts = line.split(":", 1)
            key = parts[0].strip()
            value = parts[1].strip()
            # If value starts with 0x, keep it as string. If it's a number, convert to int if possible.
            if value.startswith("0x"):
                data[key] = value
            else:
                try:
                    data[key] = int(value)
                except ValueError:
                    data[key] = value
    return data

def main():
    print("Generating known-answer test vectors...")
    
    vectors = []
    
    # Vector 1: Standard Valid (unstructured)
    print("Generating Vector 1: Standard Valid...")
    v1 = run_generator([])
    vectors.append(v1)
    
    # Vector 2: Standard Invalid (unstructured, corrupted signature)
    print("Generating Vector 2: Standard Invalid...")
    v2 = run_generator(["--invalid"])
    vectors.append(v2)
    
    # Vector 3: Structured Contract Spend Valid
    print("Generating Vector 3: Structured Contract Spend Valid...")
    v3 = run_generator([
        "--spend-contract",
        "--chain-id", "421614",
        "--contract", "0x62ca774e20b76431d189e1635400b91b03c2b031",
        "--recipient", "0x23e32D309c575A3D5E7CD2867BE12B00efa44Bb1",
        "--amount", "5000000",
        "--nonce", "0x0000000000000000000000000000000000000000000000000000000000000001",
        "--expiry", "0"
    ])
    vectors.append(v3)
    
    # Vector 4: Structured Contract Spend Invalid (corrupted signature)
    print("Generating Vector 4: Structured Contract Spend Invalid...")
    v4 = run_generator([
        "--spend-contract",
        "--invalid",
        "--chain-id", "421614",
        "--contract", "0x62ca774e20b76431d189e1635400b91b03c2b031",
        "--recipient", "0x23e32D309c575A3D5E7CD2867BE12B00efa44Bb1",
        "--amount", "5000000",
        "--nonce", "0x0000000000000000000000000000000000000000000000000000000000000001",
        "--expiry", "0"
    ])
    vectors.append(v4)

    # Vector 5: Structured Contract Spend Valid (alternate parameters)
    print("Generating Vector 5: Structured Contract Spend (alternate params)...")
    v5 = run_generator([
        "--spend-contract",
        "--chain-id", "421614",
        "--contract", "0x62ca774e20b76431d189e1635400b91b03c2b031",
        "--recipient", "0xa1b2c3d4e5f6a7b8c9d0a1b2c3d4e5f6a7b8c9d0",
        "--amount", "10000000",
        "--nonce", "0x0000000000000000000000000000000000000000000000000000000000000002",
        "--expiry", "1780720000"
    ])
    vectors.append(v5)

    # Save to JSON file
    output_path = "/home/azureuser/crypto/test-reports/known_answer_vectors.json"
    with open(output_path, "w") as f:
        json.dump(vectors, f, indent=2)
        
    print(f"Successfully generated and saved 5 test vectors to {output_path}")

if __name__ == "__main__":
    main()
