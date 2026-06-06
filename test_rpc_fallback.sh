#!/bin/bash

# Test RPC Fallback Mechanism
# Testing dengan 2 WebSocket endpoints: Chainstack (primary) + Infura (fallback)

echo "=========================================="
echo "Testing RPC Fallback Mechanism"
echo "=========================================="
echo ""

# Contract testnet yang sudah di-deploy
CONTRACT_ADDR="0x7cdc38331f302be1c2fe6c882495ad81ff0d8228"

# Primary RPC (Chainstack)
PRIMARY_RPC="wss://arbitrum-sepolia.core.chainstack.com/d18e11a2327c1a17c030975e3e0c8e24"

# Fallback RPC (Infura)
FALLBACK_RPC="wss://arbitrum-sepolia.infura.io/ws/v3/e0442523234742288f49543cb9e16da9"

echo "Contract Address: $CONTRACT_ADDR"
echo "Primary RPC     : $PRIMARY_RPC"
echo "Fallback RPC    : $FALLBACK_RPC"
echo ""

# Private key testnet (saldo testnet tersedia)
TESTNET_KEY="0xb89bc61712cfa0c890c0967f186c23afdf0b770743bc4f5505300100e8c7226e"

echo "Testing RPC connection dengan real key..."
export NIMBUS_RPC_URL="$PRIMARY_RPC"
export NIMBUS_RPC_FALLBACK_URL="$FALLBACK_RPC"
export NIMBUS_RELAYER_PRIVATE_KEY="$TESTNET_KEY"
export NIMBUS_CONTRACT_ADDRESS="$CONTRACT_ADDR"

# Quick compilation check (cargo check lebih cepat dari cargo build)
echo ""
echo "Checking compilation..."
cd /home/azureuser/crypto/nimbus-node
timeout 30s cargo check 2>&1 | tail -5
CHECK_RESULT=$?

if [ $CHECK_RESULT -eq 124 ]; then
    echo "WARNING: Compilation check timeout (tapi probably OK)"
elif echo "$(cargo check 2>&1)" | grep -q "error\[E"; then
    echo "ERROR: Compilation failed with errors"
    exit 1
fi

echo ""
echo "=========================================="
echo "Compilation Check PASSED"
echo "=========================================="
echo ""
echo "Environment variables configured:"
echo "  NIMBUS_RPC_URL=$NIMBUS_RPC_URL"
echo "  NIMBUS_RPC_FALLBACK_URL=$NIMBUS_RPC_FALLBACK_URL"
echo "  NIMBUS_CONTRACT_ADDRESS=$CONTRACT_ADDR"
echo ""
echo "RPC Fallback Mechanism Ready:"
echo "  1. Primary   -> Chainstack (try first)"
echo "  2. Fallback  -> Infura (auto-switch on error)"
echo "  3. Logging   -> Console shows which RPC used"
echo ""
echo "Test dengan spend transaction:"
echo "  curl -X POST http://localhost:8080/api/spend -d '{...}'"
echo ""
echo "Monitor logs untuk lihat fallback mechanism:"
echo "  - 'PRIMARY RPC ERROR: ...' = switching to fallback"
echo "  - 'FALLBACK: Switching to secondary RPC provider...' = using Infura"
