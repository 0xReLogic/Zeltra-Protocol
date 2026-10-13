#!/bin/bash
# Zeltra Protocol — Single Sequencer Relayer (DEC-038A / DEC-038C)
# Usage: bash scripts/start_relayer.sh [start|stop|status|restart]
set -e

PORT="${PORT:-8080}"
PID_FILE="relayer.pid"
LOG_FILE="relayer.log"
BIN="./target/debug/zeltra-node"

# Load .env.test if available
ENV_FILE="zeltra-node/.env.test"
if [ -f "$ENV_FILE" ]; then
  # shellcheck disable=SC1090
  source "$ENV_FILE"
  echo "[+] Loaded $ENV_FILE"
else
  echo "[!] $ENV_FILE not found — using current environment defaults"
fi

export PORT="$PORT"
export ZELTRA_ENV="${ZELTRA_ENV:-test}"
export ZELTRA_DB_KEY="${ZELTRA_DB_KEY:-my-hard-test-secure-db-key-12345}"
export ZELTRA_DATABASE_PATH="${ZELTRA_DATABASE_PATH:-relayer.db}"
export ZELTRA_RPC_URL="${ZELTRA_RPC_URL:-https://arbitrum-sepolia.core.chainstack.com/d18e11a2327c1a17c030975e3e0c8e24}"
export ZELTRA_RPC_FALLBACK_URL="${ZELTRA_RPC_FALLBACK_URL:-https://arbitrum-sepolia.infura.io/v3/e0442523234742288f49543cb9e16da9}"

start() {
  echo "=========================================="
  echo " Zeltra ZK-UTXO Relayer / Sequencer"
  echo "=========================================="

  if [ -f "$PID_FILE" ] && kill -0 "$(cat "$PID_FILE")" 2>/dev/null; then
    echo "[!] Relayer is already running with PID $(cat "$PID_FILE")"
    exit 0
  fi

  if [ ! -f "$BIN" ]; then
    echo "[1/2] Building zeltra-node binary..."
    cargo build -p zeltra-node
  else
    echo "[1/2] Binary found: $BIN"
  fi

  echo "[2/2] Starting zeltra-node on port $PORT..."
  nohup "$BIN" > "$LOG_FILE" 2>&1 &
  RELAYER_PID=$!
  echo "$RELAYER_PID" > "$PID_FILE"

  echo "  Spawned PID $RELAYER_PID. Waiting for readiness..."
  READY=0
  for i in $(seq 1 20); do
    if curl -sf "http://127.0.0.1:$PORT/live" > /dev/null 2>&1 || curl -sf "http://127.0.0.1:$PORT/health" > /dev/null 2>&1; then
      READY=1
      break
    fi
    sleep 1
  done

  if [ "$READY" -eq 1 ]; then
    echo "=========================================="
    echo " Relayer is ACTIVE and listening on :$PORT"
    echo " Log output: $LOG_FILE"
    echo " PID file  : $PID_FILE"
    echo "=========================================="
  else
    echo "[ERROR] Relayer failed to respond within 20s. Check $LOG_FILE"
    exit 1
  fi
}

stop() {
  echo "Stopping Relayer..."
  if [ -f "$PID_FILE" ]; then
    PID=$(cat "$PID_FILE")
    if kill -0 "$PID" 2>/dev/null; then
      kill "$PID" || true
      sleep 1
      if kill -0 "$PID" 2>/dev/null; then
        kill -9 "$PID" || true
      fi
      echo "Relayer (PID $PID) stopped."
    else
      echo "PID $PID not found in running processes."
    fi
    rm -f "$PID_FILE"
  else
    pkill -f zeltra-node 2>/dev/null || true
    echo "Killed any lingering zeltra-node processes."
  fi
}

status() {
  echo "=== Relayer Status ==="
  if [ -f "$PID_FILE" ] && kill -0 "$(cat "$PID_FILE")" 2>/dev/null; then
    echo "Status: RUNNING (PID $(cat "$PID_FILE"))"
    echo "Checking health endpoint..."
    curl -s "http://127.0.0.1:$PORT/health" || echo "Health check returned no output"
    echo ""
  else
    echo "Status: STOPPED"
  fi
}

case "$1" in
  start)
    start
    ;;
  stop)
    stop
    ;;
  restart)
    stop
    sleep 1
    start
    ;;
  status)
    status
    ;;
  *)
    echo "Usage: $0 {start|stop|restart|status}"
    exit 1
    ;;
esac
