#!/usr/bin/env bash
# smoke-test.sh — post-deploy smoke tests for the ScoutChain contracts.
#
# Runs a sequence of lightweight assertions against a live deployment to
# confirm the contracts came up healthy and key invariants hold.
#
# Usage:
#   ./scripts/smoke-test.sh [NETWORK]
#
# NETWORK defaults to "testnet" if not supplied.
#
# Requires:
#   - stellar CLI on PATH
#   - .env.contracts in the repo root (written by deploy.sh)
#   - DEPLOYER_SECRET exported or in .env
#
# Exit codes:
#   0 — all smoke tests passed
#   1 — one or more smoke tests failed
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
  echo "ERROR: $ENV_CONTRACTS not found. Run scripts/deploy.sh first."
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
TESTS_RUN=0
TESTS_PASSED=0

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------
pass() {
  echo "  PASS: $1"
  TESTS_PASSED=$((TESTS_PASSED + 1))
}

fail() {
  echo "  FAIL: $1"
  FAIL=1
}

invoke_health() {
  local contract_id="$1"
  stellar contract invoke \
    --id "$contract_id" \
    --network "$NETWORK" \
    --source "$DEPLOYER" \
    -- health 2>&1
}

parse_bool() {
  local json="$1"
  local field="$2"
  echo "$json" | python3 -c "
import sys, json
data = json.loads(sys.stdin.read())
val = data.get('$field', 'MISSING')
print(str(val).lower())
" 2>/dev/null || echo "parse_error"
}

# ---------------------------------------------------------------------------
# Test: contract health assertions
#
# For every contract:
#   - initialized must be true
#   - paused must be false (fresh deployment should never come up paused)
#
# For scout_access specifically:
#   - pay_to_contact_paused must also be false (function-scoped pause, #1056)
# ---------------------------------------------------------------------------
assert_health() {
  local name="$1"
  local contract_id="$2"
  local check_ptc="${3:-false}"

  TESTS_RUN=$((TESTS_RUN + 1))
  echo ""
  echo "-- smoke: $name health() --"

  if [[ -z "$contract_id" ]]; then
    fail "$name: contract ID is empty — was deploy.sh run successfully?"
    return
  fi

  local raw
  if ! raw=$(invoke_health "$contract_id"); then
    fail "$name: health() invocation failed — $raw"
    return
  fi

  local initialized paused
  initialized=$(parse_bool "$raw" "initialized")
  paused=$(parse_bool "$raw" "paused")

  if [[ "$initialized" == "true" ]]; then
    pass "$name: initialized == true"
  else
    fail "$name: initialized expected true, got '$initialized'"
  fi

  if [[ "$paused" == "false" ]]; then
    pass "$name: paused == false"
  else
    fail "$name: paused expected false, got '$paused' — call unpause_contract if this is intentional"
  fi

  if [[ "$check_ptc" == "true" ]]; then
    TESTS_RUN=$((TESTS_RUN + 1))
    local ptc_paused
    ptc_paused=$(parse_bool "$raw" "pay_to_contact_paused")

    if [[ "$ptc_paused" == "missing" ]]; then
      # Field absent means the contract predates #1056; treat as a warning
      echo "  WARN: pay_to_contact_paused field absent — contract may not include #1056 yet"
    elif [[ "$ptc_paused" == "false" ]]; then
      pass "$name: pay_to_contact_paused == false"
    else
      fail "$name: pay_to_contact_paused expected false, got '$ptc_paused' — pay_to_contact is inadvertently paused"
    fi
  fi
}

# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

echo "========================================"
echo "  ScoutChain Smoke Test — $NETWORK"
echo "========================================"

assert_health "registration"  "${REGISTRATION_CONTRACT_ID:-}"
assert_health "verification"  "${VERIFICATION_CONTRACT_ID:-}"
assert_health "progress"      "${PROGRESS_CONTRACT_ID:-}"
# scout_access has a function-scoped pay_to_contact_paused flag (#1056)
assert_health "scout_access"  "${SCOUT_ACCESS_CONTRACT_ID:-}" "true"

# ---------------------------------------------------------------------------
# Summary
# ---------------------------------------------------------------------------
echo ""
echo "========================================"
echo "  Smoke test summary — $NETWORK"
echo "  Tests run:    $TESTS_RUN"
echo "  Tests passed: $TESTS_PASSED"
if [[ $FAIL -ne 0 ]]; then
  echo "  RESULT: FAIL"
  echo "========================================"
  exit 1
else
  echo "  RESULT: PASS"
  echo "========================================"
fi
