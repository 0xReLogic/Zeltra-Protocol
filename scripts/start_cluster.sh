#!/bin/bash
# Nimbus Guardian Cluster — 1 Leader + 4 Guardians + Vault KMS
# Usage: bash scripts/start_cluster.sh [start|stop|status]
set -e

VAULT_CONTAINER="nimbus-vault"
VAULT_PORT=8200
VAULT_TOKEN="test-root-token"
THRESHOLD=3
BIN="./target/debug/nimbus-node"
DB_KEY="${NIMBUS_DB_KEY:-my-hard-test-secure-db-key-12345}"

# BLS ceremony keys (seed [0x42; 32])
PK_ISS="885940298f97a7fbd92d404b898dcc5747d24cfa048ef3bbf9f4515b5605e4d058107f76c788ec2535476f3c241e6a410bfd5c8883b1d864bdcbdd76bdc2102d833237e19a7bc9d38e6ee5d98729c0e7b90052e61c6df8ca377a241ace187c02"
SHARE1="0100000000000000ee580a1ebe9cc95f7a48a0a38d882d54f8d2a01a1b29fb035b08910b49e36760"
SHARE2="02000000000000002591737b212a5bb850368a813c6c6a8dc8efd0b56e6999f8744b3eccc5e52c64"
SHARE3="03000000000000003e00e675ef3022856780276d6447cb7677cf1a257a50b04cb5081cbb5a2a173f"
SHARE4="04000000000000003aa6610d27b11ec6bd82766608be0d640a4a207245b6793364bdc7015b581465"
SHARE5="05000000000000001883e641c9aa507b54e1786d252c74017c873f93c8c2bb7939eca37673c83662"
PUB1="b5e3ce24c5c3852699737786086f0f6a81a1d2deb25fce296cbb9a19361cd0cc5b661cc760501f1d00f04d7c4c83a12b0da040d013c14064581c0f400ee042925777fb15eb15c597ceea72fc6d4fa11a278d1a6d38a0e3c3da61f00ff2141607"
PUB2="a16f2160800821708345c04909441f5f4edaa399828f8547431786ec294898a147da29700ff61c9ba31577b11a2d6f56117f925869fdce9dcb9e95d07b560c872d76df5af63cd6692200e9d451bc204918bee98961fd2160b7c22c5cc6d8b753"
PUB3="886c0fb22b1893cd2012cff9329abfaf8469c21a3a6e54d94307380c55b95c2330cf9a26b1ed60b1c8270c33f479755b0f294b0394cd9d829b17f7b2fffb41ddeed35cf215a6cb5f6200e681f3f37c136fad22a42c086714a2f6b318b657d47b"
PUB4="b7b1dbdec6ba26428481c8d63c9d4732ba1f0c976d07601c1223bf1e05ca4b7411b7bf4c70f385ea0a96936fec6fd2c703c61beb392f8dde273af0566fd5d2f9c3fbf5a55bf07ffeeb7bba012665362427640a5b05ce8071544ea85dd146eb7b"
PUB5="9795b534e431f9b1b4953240dceba6208e9acfcc4383c56b60616433297fb7c62485df84a6adf2f08d2d9bbd472712a80ae8f411e648597818630eddb0cbc69298a9d8b425ccb235db9bc81e4b3e7afedea8b2df329af1fdf7fd1fba7e358dea"

# Per-node guardian registries (each node sees the OTHER nodes)
REG1="{\"2\":\"$PUB2\",\"3\":\"$PUB3\",\"4\":\"$PUB4\",\"5\":\"$PUB5\"}"
REG2="{\"1\":\"$PUB1\",\"3\":\"$PUB3\",\"4\":\"$PUB4\",\"5\":\"$PUB5\"}"
REG3="{\"1\":\"$PUB1\",\"2\":\"$PUB2\",\"4\":\"$PUB4\",\"5\":\"$PUB5\"}"
REG4="{\"1\":\"$PUB1\",\"2\":\"$PUB2\",\"3\":\"$PUB3\",\"5\":\"$PUB5\"}"
REG5="{\"1\":\"$PUB1\",\"2\":\"$PUB2\",\"3\":\"$PUB3\",\"4\":\"$PUB4\"}"

# Load .env.test if it exists (for RPC, contract, private key)
ENV_FILE="nimbus-node/.env.test"
if [ -f "$ENV_FILE" ]; then
  source "$ENV_FILE"
  echo "[+] Loaded $ENV_FILE"
else
  echo "[!] $ENV_FILE not found — using environment defaults"
fi

RPC="${NIMBUS_RPC_URL:-https://arbitrum-sepolia.core.chainstack.com/d18e11a2327c1a17c030975e3e0c8e24}"
RPC_FB="${NIMBUS_RPC_FALLBACK_URL:-https://arbitrum-sepolia.infura.io/v3/e0442523234742288f49543cb9e16da9}"
PK="${NIMBUS_RELAYER_PRIVATE_KEY:-}"
CONTRACT="${NIMBUS_CONTRACT_ADDRESS:-}"

start() {
  echo "=========================================="
  echo " Nimbus Cluster: 1 Leader + 4 Guardians"
  echo "=========================================="

  # Kill old
  pkill -f nimbus-node 2>/dev/null || true
  docker rm -f "$VAULT_CONTAINER" 2>/dev/null || true
  sleep 1

  # Build
  echo "[1/5] Building nimbus-node..."
  cargo build -p nimbus-node -q

  # Vault
  echo "[2/5] Starting Vault..."
  docker run -d --name "$VAULT_CONTAINER" -p "$VAULT_PORT:8200" \
    -e "VAULT_DEV_ROOT_TOKEN_ID=$VAULT_TOKEN" \
    hashicorp/vault:latest > /dev/null 2>&1
  for i in $(seq 1 15); do
    curl -sf "http://127.0.0.1:$VAULT_PORT/v1/sys/health" > /dev/null 2>&1 && break
    sleep 1
  done
  echo "  Vault ready."

  # Write shares
  echo "[3/5] Writing shares to Vault..."
  for idx in 1 2 3 4 5; do
    share_var="SHARE${idx}"
    share_val="${!share_var}"
    name="leader"
    [ "$idx" -gt 1 ] && name="guardian-$((idx-1))"
    curl -sf -X POST "http://127.0.0.1:$VAULT_PORT/v1/secret/data/nimbus/$name" \
      -H "X-Vault-Token: $VAULT_TOKEN" \
      -H "Content-Type: application/json" \
      -d "{\"data\":{\"share_key\":\"$share_val\"}}" > /dev/null
  done
  echo "  5 shares written."

  # Clean DBs
  echo "[4/5] Cleaning old databases..."
  rm -f test_leader.db test_leader.db-shm test_leader.db-wal
  for i in 1 2 3 4; do
    rm -f "test_guardian_${i}.db" "test_guardian_${i}.db-shm" "test_guardian_${i}.db-wal"
  done

  # Common env
  export NIMBUS_ENV="hard-test"
  export NIMBUS_ISSUER_PUBLIC_KEY="$PK_ISS"
  export NIMBUS_THRESHOLD="$THRESHOLD"
  export NIMBUS_RPC_URL="$RPC"
  export NIMBUS_RPC_FALLBACK_URL="$RPC_FB"
  export NIMBUS_DB_KEY="$DB_KEY"
  export NIMBUS_RELAYER_PRIVATE_KEY="$PK"
  export NIMBUS_CONTRACT_ADDRESS="$CONTRACT"
  export NIMBUS_VAULT_ADDR="http://127.0.0.1:$VAULT_PORT"
  export NIMBUS_VAULT_TOKEN="$VAULT_TOKEN"

  # Start nodes
  echo "[5/5] Starting nodes..."

  # Leader
  PORT=8080 NIMBUS_BIND_ADDR=127.0.0.1 NIMBUS_SHARE_INDEX=1 \
    NIMBUS_VAULT_PATH=v1/secret/data/nimbus/leader \
    NIMBUS_DB_PATH=test_leader.db NIMBUS_GUARDIAN_PUBLIC_KEYS="$REG1" \
    NIMBUS_BATCH_ENABLED=true \
    "$BIN" > leader.log 2>&1 &
  echo "  Leader    :8080 (pid $!)"

  # Guardians
  REGS=("$REG2" "$REG3" "$REG4" "$REG5")
  for i in 1 2 3 4; do
    port=$((8080 + i))
    share_idx=$((i + 1))
    PORT=$port NIMBUS_BIND_ADDR=127.0.0.1 NIMBUS_SHARE_INDEX=$share_idx \
      NIMBUS_VAULT_PATH="v1/secret/data/nimbus/guardian-$i" \
      NIMBUS_DB_PATH="test_guardian_${i}.db" \
      NIMBUS_GUARDIAN_PUBLIC_KEYS="${REGS[$((i-1))]}" \
      NIMBUS_TRUSTED_LEADERS="${TRUSTED_LEADERS:-}" \
      "$BIN" > "guardian-${i}.log" 2>&1 &
    echo "  Guardian-$i :$port (pid $!)"
  done

  # Health check
  echo ""
  echo "Waiting for nodes (timeout 30s)..."
  for port in 8080 8081 8082 8083 8084; do
    for attempt in $(seq 1 15); do
      curl -sf "http://127.0.0.1:$port/health" > /dev/null 2>&1 && break
      sleep 2
    done
    if curl -sf "http://127.0.0.1:$port/health" > /dev/null 2>&1; then
      echo "  :$port ✓"
    else
      echo "  :$port ✗ UNHEALTHY — check $( [ $port -eq 8080 ] && echo leader || echo guardian-$((port-8080)) ).log"
    fi
  done

  echo ""
  echo "Cluster ready. Logs: leader.log, guardian-{1..4}.log"
}

stop() {
  echo "Stopping cluster..."
  pkill -f nimbus-node 2>/dev/null || true
  docker rm -f "$VAULT_CONTAINER" 2>/dev/null || true
  sleep 1
  echo "Stopped."
}

status() {
  echo "=== Node Status ==="
  for port in 8080 8081 8082 8083 8084; do
    name=$( [ $port -eq 8080 ] && echo "Leader    " || echo "Guardian-$((port-8080))" )
    if curl -sf "http://127.0.0.1:$port/health" > /dev/null 2>&1; then
      echo "  $name :$port ✓"
    else
      echo "  $name :$port ✗"
    fi
  done
  echo ""
  if docker ps --format '{{.Names}}' | grep -q "$VAULT_CONTAINER"; then
    echo "  Vault: running"
  else
    echo "  Vault: stopped"
  fi
}

case "${1:-start}" in
  start)  start ;;
  stop)   stop ;;
  status) status ;;
  *)      echo "Usage: $0 [start|stop|status]" ;;
esac
