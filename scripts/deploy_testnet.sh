#!/bin/bash
set -euo pipefail

# Stylus Deployment Helper Script for Nimbus Protocol
# Target: Arbitrum Sepolia Testnet

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
CONTRACT_DIR="${REPO_ROOT}/nimbus-contracts"
DEPLOYMENT_DIR="${NIMBUS_DEPLOYMENT_DIR:-${REPO_ROOT}/deployments}"
CHECK_URL=${RPC_URL:-"https://sepolia-rollup.arbitrum.io/rpc"}
CHAIN_ID_HINT="${CHAIN_ID:-421614}"
TIMESTAMP_UTC="$(date -u +"%Y-%m-%dT%H:%M:%SZ")"
TIMESTAMP_FILE="$(date -u +"%Y%m%dT%H%M%SZ")"

json_escape() {
    python3 -c 'import json,sys; print(json.dumps(sys.stdin.read()))'
}

write_manifest() {
    local manifest_path="$1"
    local phase="$2"
    local status="$3"
    local contract_address="${4:-}"
    local deploy_log_path="${5:-}"

    local git_commit="unknown"
    local git_dirty="unknown"
    local rustc_version="unknown"
    local cargo_stylus_version="unknown"

    git_commit="$(git -C "${REPO_ROOT}" rev-parse HEAD 2>/dev/null || echo unknown)"
    if [ -n "$(git -C "${REPO_ROOT}" status --porcelain 2>/dev/null || true)" ]; then
        git_dirty="true"
    else
        git_dirty="false"
    fi
    rustc_version="$(rustc -Vv 2>/dev/null || echo unknown)"
    cargo_stylus_version="$(cargo stylus --version 2>/dev/null || cargo-stylus --version 2>/dev/null || echo unknown)"

    mkdir -p "$(dirname "${manifest_path}")"
    cat > "${manifest_path}" <<EOF_MANIFEST
{
  "schema": "nimbus.deployment-manifest.v1",
  "created_at": "${TIMESTAMP_UTC}",
  "phase": "${phase}",
  "status": "${status}",
  "network": "arbitrum-sepolia",
  "chain_id_hint": ${CHAIN_ID_HINT},
  "rpc_url": $(printf '%s' "${CHECK_URL}" | json_escape),
  "contract_address": $(printf '%s' "${contract_address}" | json_escape),
  "deploy_log": $(printf '%s' "${deploy_log_path}" | json_escape),
  "git": {
    "commit": "${git_commit}",
    "dirty": ${git_dirty}
  },
  "toolchain": {
    "rustc": $(printf '%s' "${rustc_version}" | json_escape),
    "cargo_stylus": $(printf '%s' "${cargo_stylus_version}" | json_escape)
  },
  "checks": {
    "stylus_check": "${status}"
  },
  "notes": [
    "Manifest does not include private keys or secrets.",
    "Verify deployed bytecode, ABI selectors, init parameters, and receipt status before using this deployment for hard tests."
  ]
}
EOF_MANIFEST
}

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
cd "${CONTRACT_DIR}"
cargo stylus check --endpoint="$CHECK_URL"

mkdir -p "${DEPLOYMENT_DIR}"
CHECK_MANIFEST="${DEPLOYMENT_DIR}/arbitrum-sepolia-${TIMESTAMP_FILE}-check.json"
write_manifest "${CHECK_MANIFEST}" "check" "passed" "" ""

echo "=========================================================="
echo "STYLUS VALIDATION CHECK PASSED"
echo "Check manifest written to: ${CHECK_MANIFEST}"
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

if [ -n "${RPC_URL:-}" ] && [ -n "${PRIVATE_KEY:-}" ]; then
    echo "Executing deployment to $RPC_URL..."
    DEPLOY_LOG="${DEPLOYMENT_DIR}/arbitrum-sepolia-${TIMESTAMP_FILE}-deploy.log"
    set +e
    cargo stylus deploy --endpoint="$RPC_URL" --private-key="$PRIVATE_KEY" 2>&1 | tee "${DEPLOY_LOG}"
    DEPLOY_STATUS=${PIPESTATUS[0]}
    set -e

    CONTRACT_ADDRESS="$(
        grep -Eo '0x[a-fA-F0-9]{40}' "${DEPLOY_LOG}" | tail -n 1 || true
    )"
    DEPLOY_MANIFEST="${DEPLOYMENT_DIR}/arbitrum-sepolia-${TIMESTAMP_FILE}-deploy.json"

    if [ "${DEPLOY_STATUS}" -eq 0 ]; then
        write_manifest "${DEPLOY_MANIFEST}" "deploy" "passed" "${CONTRACT_ADDRESS}" "${DEPLOY_LOG}"
        echo "Deployment manifest written to: ${DEPLOY_MANIFEST}"
        if [ -z "${CONTRACT_ADDRESS}" ]; then
            echo "Warning: deployment succeeded but contract address was not extracted from cargo stylus output."
            echo "Add the deployed address to the manifest before hard-test evidence is accepted."
        fi
    else
        write_manifest "${DEPLOY_MANIFEST}" "deploy" "failed" "${CONTRACT_ADDRESS}" "${DEPLOY_LOG}"
        echo "Deployment failed. Failure manifest written to: ${DEPLOY_MANIFEST}"
        exit "${DEPLOY_STATUS}"
    fi
fi
