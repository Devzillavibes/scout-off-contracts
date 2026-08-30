#!/usr/bin/env bash
# health-check.sh — verify on-chain health() for all four ScoutChain contracts.
#
# Usage:
#   ./scripts/health-check.sh [NETWORK]
#
# NETWORK defaults to "testnet" if not supplied.
#
# Requires:
#   - stellar CLI on PATH
#   - .env.contracts in the repo root with the four CONTRACT_ID exports
#   - DEPLOYER_SECRET in environment or .env
#
# Exit codes:
#   0 — all contracts healthy (initialized=true, paused=false,
#                              pay_to_contact_paused=false where applicable)
#   1 — one or more contracts failed the health check
set -euo pipefail

NETWORK="${1:-testnet}"
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ENV_CONTRACTS="$REPO_ROOT/.env.contracts"
ENV_FILE="$REPO_ROOT/.env"

# Load .env if present (provides DEPLOYER_SECRET, etc.)
if [[ -f "$ENV_FILE" ]]; then
  # shellcheck source=/dev/null
  source "$ENV_FILE"
fi

if [[ ! -f "$ENV_CONTRACTS" ]]; then
  echo "ERROR: $ENV_CONTRACTS not found. Run scripts/deploy.sh and scripts/initialize.sh first."
  exit 1
fi

# shellcheck source=/dev/null
source "$ENV_CONTRACTS"

DEPLOYER="${DEPLOYER_SECRET:-}"
if [[ -z "$DEPLOYER" ]]; then
  echo "ERROR: DEPLOYER_SECRET is not set. Export it or add it to .env."
  exit 1
fi

FAIL=0

# ---------------------------------------------------------------------------
# Helper: invoke health() and parse fields
# ---------------------------------------------------------------------------
check_health() {
  local contract_name="$1"
  local contract_id="$2"
  local check_pay_to_contact="${3:-false}"

  echo ""
  echo "==> Checking $contract_name ($contract_id)..."

  if [[ -z "$contract_id" ]]; then
    echo "  ERROR: contract ID is empty for $contract_name"
    FAIL=1
    return
  fi

  local raw
  if ! raw=$(stellar contract invoke \
    --id "$contract_id" \
    --network "$NETWORK" \
    --source "$DEPLOYER" \
    -- health 2>&1); then
    echo "  ERROR: health() invocation failed for $contract_name"
    echo "  Output: $raw"
    FAIL=1
    return
  fi

  # Parse initialized field
  local initialized
  initialized=$(echo "$raw" | python3 -c "
import sys, json
data = json.loads(sys.stdin.read())
print(str(data.get('initialized', 'MISSING')).lower())
" 2>/dev/null || echo "parse_error")

  # Parse paused field
  local paused
  paused=$(echo "$raw" | python3 -c "
import sys, json
data = json.loads(sys.stdin.read())
print(str(data.get('paused', 'MISSING')).lower())
" 2>/dev/null || echo "parse_error")

  echo "  initialized: $initialized"
  echo "  paused:      $paused"

  if [[ "$initialized" != "true" ]]; then
    echo "  FAIL: $contract_name is not initialized (got: $initialized)"
    FAIL=1
  fi

  if [[ "$paused" != "false" ]]; then
    echo "  FAIL: $contract_name is unexpectedly paused (got: $paused) — run unpause_contract before going live"
    FAIL=1
  fi

  # Check pay_to_contact_paused if this contract exposes it
  if [[ "$check_pay_to_contact" == "true" ]]; then
    local ptc_paused
    ptc_paused=$(echo "$raw" | python3 -c "
import sys, json
data = json.loads(sys.stdin.read())
val = data.get('pay_to_contact_paused', 'MISSING')
print(str(val).lower())
" 2>/dev/null || echo "parse_error")

    echo "  pay_to_contact_paused: $ptc_paused"

    if [[ "$ptc_paused" == "missing" ]]; then
      echo "  WARN: pay_to_contact_paused field absent from health() response — contract may need updating (see #1056)"
    elif [[ "$ptc_paused" != "false" ]]; then
      echo "  FAIL: $contract_name has pay_to_contact inadvertently paused (got: $ptc_paused) — check function-scoped pause state"
      FAIL=1
    else
      echo "  OK: pay_to_contact_paused == false"
    fi
  fi

  if [[ "$initialized" == "true" ]] && [[ "$paused" == "false" ]]; then
    echo "  OK: $contract_name healthy"
  fi
}

# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

echo "========================================"
echo "  ScoutChain Health Check — $NETWORK"
echo "========================================"

check_health "registration" "${REGISTRATION_CONTRACT_ID:-}"
check_health "verification" "${VERIFICATION_CONTRACT_ID:-}"
check_health "progress"     "${PROGRESS_CONTRACT_ID:-}"
# scout_access exposes pay_to_contact_paused (function-scoped pause, #1056)
check_health "scout_access" "${SCOUT_ACCESS_CONTRACT_ID:-}" "true"

echo ""
echo "========================================"
if [[ $FAIL -ne 0 ]]; then
  echo "  RESULT: FAIL — one or more contracts are unhealthy"
  echo "========================================"
  exit 1
else
  echo "  RESULT: PASS — all contracts healthy"
  echo "========================================"
fi
