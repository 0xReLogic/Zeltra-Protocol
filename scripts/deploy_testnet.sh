#!/bin/bash
set -e

# Stylus Deployment Helper Script for Nimbus Protocol
# Target: Arbitrum Sepolia Testnet

echo "=========================================================="
# No emoji as per system constraints
echo "NIMBUS SMART CONTRACT STYLUS DEPLOYMENT HELPERS"
echo "=========================================================="

# 1. Check if rustc is installed
if ! command -v rustc &> /dev/null; then
    echo "Error: Rust compiler (rustc) is not installed."
    echo "Please install it by running: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    exit 1
fi

# 2. Check if wasm32 target is installed
echo "Checking wasm32 target..."
if ! rustup target list --installed | grep -q "wasm32-unknown-unknown"; then
    echo "Installing wasm32 target..."
    rustup target add wasm32-unknown-unknown
else
    echo "wasm32 target is already installed."
fi

# 3. Check if cargo stylus is installed
echo "Checking cargo-stylus..."
if ! command -v cargo-stylus &> /dev/null && ! cargo --list | grep -q "stylus"; then
    echo "cargo-stylus not found. Installing cargo-stylus CLI..."
    cargo install --force cargo-stylus
else
    echo "cargo-stylus is already installed."
fi

# 4. Perform compilation and optimization check
echo "Performing Stylus validation check..."
cd nimbus-contracts
cargo stylus check

echo "=========================================================="
echo "STYLUS VALIDATION CHECK PASSED"
echo "=========================================================="
echo "To deploy the contract to Arbitrum Sepolia, run the following command:"
echo ""
echo "cargo stylus deploy \\"
echo "  --endpoint='https://sepolia-rollup.arbitrum.io/rpc' \\"
echo "  --private-key='YOUR_PRIVATE_KEY'"
echo ""
echo "Or configure the following environment variables and run this script as:"
echo "RPC_URL=https://sepolia-rollup.arbitrum.io/rpc PRIVATE_KEY=0x... ./deploy_testnet.sh"
echo "=========================================================="

if [ ! -z "$RPC_URL" ] && [ ! -z "$PRIVATE_KEY" ]; then
    echo "Executing deployment to $RPC_URL..."
    cargo stylus deploy --endpoint="$RPC_URL" --private-key="$PRIVATE_KEY"
fi
