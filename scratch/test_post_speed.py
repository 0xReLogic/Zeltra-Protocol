import urllib.request
import json
import time

url = "http://127.0.0.1:8080/api/spend"
payload = {
    "nullifier": "0x1234567890123456789012345678901234567890123456789012345678901234",
    "sig_hex": "0x1234",
    "recipient": "0x23e32D309c575A3D5E7CD2867BE12B00efa44Bb1",
    "amount": 9980000,
    "alpha_neg_hex": "0x1234",
    "hm_hex": "0x1234",
    "pk_iss_hex": "0x1234",
    "idempotency_key": f"test_speed_{time.time()}",
    "expiry": 0,
    "nonce_hex": "0x1234"
}

req = urllib.request.Request(
    url,
    data=json.dumps(payload).encode("utf-8"),
    headers={"Content-Type": "application/json"}
)

start = time.time()
try:
    with urllib.request.urlopen(req, timeout=5) as res:
        print("Status:", res.status)
        print("Response:", res.read().decode())
except Exception as e:
    print("Error:", e)
print("Time taken:", time.time() - start)
