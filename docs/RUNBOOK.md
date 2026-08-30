# ScoutChain Operations Runbook

This document covers day-to-day operational procedures for the four ScoutChain
Soroban contracts: **registration**, **verification**, **progress**, and
**scout_access**. Use it for health monitoring, incident response, fee
management, validator administration, and common troubleshooting.

---

## Table of Contents

1. [Health Checks](#health-checks)
2. [Contract Pause / Unpause (Circuit Breaker)](#contract-pause--unpause-circuit-breaker)
3. [Validator Administration](#validator-administration)
4. [Fee Management](#fee-management)
5. [Scout Subscriptions](#scout-subscriptions)
6. [Cross-Contract Wiring Verification](#cross-contract-wiring-verification)
7. [Incident Response](#incident-response)
8. [Common Error Codes](#common-error-codes)
9. [Runbook Checklist — Deployment Day](#runbook-checklist--deployment-day)

---

## Health Checks

Call `health()` on each contract to confirm it is initialized and report its
operational state.  The function returns a `ContractHealth` object with **three
fields**:

| Field | Type | Meaning |
|-------|------|---------|
| `initialized` | `bool` | Contract has been initialized via `initialize()` |
| `paused` | `bool` | Global circuit breaker is active — all state-changing calls will fail |
| `pay_to_contact_paused` | `bool` | Pay-to-contact function is individually paused (`scout_access` only; always `false` for the other three contracts) |

### registration

```bash
stellar contract invoke \
  --id $REGISTRATION_CONTRACT_ID \
  --network testnet \
  -- health
```

**Expected output (healthy, running normally)**

```json
{
  "initialized": true,
  "paused": false,
  "pay_to_contact_paused": false
}
```

**Expected output (contract globally paused)**

```json
{
  "initialized": true,
  "paused": true,
  "pay_to_contact_paused": false
}
```

> `pay_to_contact_paused` is always `false` for the registration contract —
> it has no pay-to-contact function.

---

### verification

```bash
stellar contract invoke \
  --id $VERIFICATION_CONTRACT_ID \
  --network testnet \
  -- health
```

**Expected output (healthy, running normally)**

```json
{
  "initialized": true,
  "paused": false,
  "pay_to_contact_paused": false
}
```

**Expected output (contract globally paused)**

```json
{
  "initialized": true,
  "paused": true,
  "pay_to_contact_paused": false
}
```

> `pay_to_contact_paused` is always `false` for the verification contract —
> it has no pay-to-contact function.

---

### progress

```bash
stellar contract invoke \
  --id $PROGRESS_CONTRACT_ID \
  --network testnet \
  -- health
```

**Expected output (healthy, running normally)**

```json
{
  "initialized": true,
  "paused": false,
  "pay_to_contact_paused": false
}
```

**Expected output (contract globally paused)**

```json
{
  "initialized": true,
  "paused": true,
  "pay_to_contact_paused": false
}
```

> `pay_to_contact_paused` is always `false` for the progress contract —
> it has no pay-to-contact function.

---

### scout_access

```bash
stellar contract invoke \
  --id $SCOUT_ACCESS_CONTRACT_ID \
  --network testnet \
  -- health
```

**Expected output (healthy, running normally)**

```json
{
  "initialized": true,
  "paused": false,
  "pay_to_contact_paused": false
}
```

**Expected output (contract globally paused)**

```json
{
  "initialized": true,
  "paused": true,
  "pay_to_contact_paused": false
}
```

**Expected output (pay-to-contact individually paused, global circuit breaker off)**

```json
{
  "initialized": true,
  "paused": false,
  "pay_to_contact_paused": true
}
```

**Expected output (both paused)**

```json
{
  "initialized": true,
  "paused": true,
  "pay_to_contact_paused": true
}
```

> `pay_to_contact_paused` reflects the function-scoped pause for
> `pay_to_contact` in the `scout_access` contract. It is independent of the
> global `paused` flag — one can be true while the other is false.

---

### Automated health sweep (all four contracts)

```bash
#!/usr/bin/env bash
set -euo pipefail
source .env.contracts   # loads REGISTRATION_CONTRACT_ID, etc.

NETWORK=${STELLAR_NETWORK:-testnet}
CONTRACTS=(
  "registration:$REGISTRATION_CONTRACT_ID"
  "verification:$VERIFICATION_CONTRACT_ID"
  "progress:$PROGRESS_CONTRACT_ID"
  "scout_access:$SCOUT_ACCESS_CONTRACT_ID"
)

for entry in "${CONTRACTS[@]}"; do
  name="${entry%%:*}"
  id="${entry##*:}"
  echo "=== $name ==="
  stellar contract invoke --id "$id" --network "$NETWORK" -- health
  echo ""
done
```

Any contract that returns `"initialized": false` has not been set up. Run
`./scripts/initialize.sh` to fix it. Any contract with `"paused": true` is in
circuit-breaker mode — see [Contract Pause / Unpause](#contract-pause--unpause-circuit-breaker).

---

## Contract Pause / Unpause (Circuit Breaker)

All four contracts support an emergency circuit breaker. When paused, every
state-changing call (registration, milestone approval, subscription purchase,
etc.) fails with error `9 ContractPaused`. Query calls (`get_player`,
`health`, etc.) continue to work.

### Pause a contract

```bash
stellar contract invoke \
  --id $REGISTRATION_CONTRACT_ID \
  --source-account $ADMIN_ADDRESS \
  --network testnet \
  -- pause_contract
```

Repeat with `VERIFICATION_CONTRACT_ID`, `PROGRESS_CONTRACT_ID`, and
`SCOUT_ACCESS_CONTRACT_ID` as needed.

### Unpause a contract

```bash
stellar contract invoke \
  --id $REGISTRATION_CONTRACT_ID \
  --source-account $ADMIN_ADDRESS \
  --network testnet \
  -- unpause_contract
```

### Verify pause state

Use `health()` — `"paused": true` confirms the contract is paused:

```json
{
  "initialized": true,
  "paused": true,
  "pay_to_contact_paused": false
}
```

---

## Validator Administration

### Register a new validator

```bash
stellar contract invoke \
  --id $VERIFICATION_CONTRACT_ID \
  --source-account $ADMIN_ADDRESS \
  --network testnet \
  -- register_validator \
  --wallet GVALIDATOR_ADDRESS \
  --credentials "Academy Director, FC Example"
```

### Revoke a validator

```bash
stellar contract invoke \
  --id $VERIFICATION_CONTRACT_ID \
  --source-account $ADMIN_ADDRESS \
  --network testnet \
  -- revoke_validator \
  --wallet GVALIDATOR_ADDRESS
```

### Check validator status

```bash
stellar contract invoke \
  --id $VERIFICATION_CONTRACT_ID \
  --network testnet \
  -- is_active_validator \
  --wallet GVALIDATOR_ADDRESS
```

Returns `true` if active, `false` if revoked or not registered.

---

## Fee Management

### Check accumulated platform fees

```bash
stellar contract invoke \
  --id $SCOUT_ACCESS_CONTRACT_ID \
  --network testnet \
  -- get_accumulated_fees
```

Returns the total stroops (1 XLM = 10,000,000 stroops) collected and pending
withdrawal.

### Withdraw fees to treasury

```bash
stellar contract invoke \
  --id $SCOUT_ACCESS_CONTRACT_ID \
  --source-account $ADMIN_ADDRESS \
  --network testnet \
  -- withdraw_fees \
  --to GTREASURY_ADDRESS
```

### Update fee configuration

```bash
stellar contract invoke \
  --id $SCOUT_ACCESS_CONTRACT_ID \
  --source-account $ADMIN_ADDRESS \
  --network testnet \
  -- update_fee_config \
  --fee_config '{
    "contact_fee_stroops": 100000,
    "basic_sub_stroops": 1000000,
    "pro_sub_stroops": 3000000,
    "elite_sub_stroops": 7000000,
    "sub_duration_secs": 2592000
  }'
```

---

## Scout Subscriptions

### Check a scout's subscription

```bash
stellar contract invoke \
  --id $SCOUT_ACCESS_CONTRACT_ID \
  --network testnet \
  -- get_subscription \
  --scout GSCOUT_ADDRESS
```

Returns the subscription record including tier, `subscribed_at`, and
`expires_at` timestamps.

### Check if a scout has contacted a player

```bash
stellar contract invoke \
  --id $SCOUT_ACCESS_CONTRACT_ID \
  --network testnet \
  -- has_contacted \
  --scout GSCOUT_ADDRESS \
  --player_id 42
```

---

## Cross-Contract Wiring Verification

The verification contract cross-calls the progress contract inside
`approve_milestone`. This link must be set once after deployment.

### Check if the link is set

```bash
stellar contract invoke \
  --id $VERIFICATION_CONTRACT_ID \
  --network testnet \
  -- get_progress_contract
```

If this returns an address, the link is active. If it errors or returns empty,
run the wiring step.

### Re-wire the cross-contract link

```bash
stellar contract invoke \
  --id $VERIFICATION_CONTRACT_ID \
  --source-account $ADMIN_ADDRESS \
  --network testnet \
  -- set_progress_contract \
  --progress_contract $PROGRESS_CONTRACT_ID
```

**Symptom when missing**: Validators can call `approve_milestone` successfully
(the milestone is recorded) but the player's progress level never advances.
Check `get_milestone_count(player_id)` vs `get_level(player_id)` to diagnose.

---

## Incident Response

### Scenario: Suspicious milestone approvals

1. Pause the verification contract immediately:
   ```bash
   stellar contract invoke --id $VERIFICATION_CONTRACT_ID \
     --source-account $ADMIN_ADDRESS --network testnet -- pause_contract
   ```
2. Identify the validator wallet involved via on-chain `milestone_approved` events.
3. Revoke the validator:
   ```bash
   stellar contract invoke --id $VERIFICATION_CONTRACT_ID \
     --source-account $ADMIN_ADDRESS --network testnet \
     -- revoke_validator --wallet GSUSPECT_VALIDATOR
   ```
4. Investigate milestones using `get_milestone(player_id, index)`.
5. Unpause when the situation is resolved.

### Scenario: Pay-to-contact exploit suspected

1. Pause only the pay-to-contact function on `scout_access` without disrupting
   subscriptions or trial offers by setting `pay_to_contact_paused`:
   ```bash
   stellar contract invoke --id $SCOUT_ACCESS_CONTRACT_ID \
     --source-account $ADMIN_ADDRESS --network testnet \
     -- pause_pay_to_contact
   ```
   After this, `health()` returns:
   ```json
   {
     "initialized": true,
     "paused": false,
     "pay_to_contact_paused": true
   }
   ```
2. Review `player_contacted` events from Horizon for anomalous patterns.
3. Resume pay-to-contact when the investigation is complete:
   ```bash
   stellar contract invoke --id $SCOUT_ACCESS_CONTRACT_ID \
     --source-account $ADMIN_ADDRESS --network testnet \
     -- unpause_pay_to_contact
   ```

### Scenario: Contract not initialized

`health()` returns `"initialized": false`. Run the full initialization:

```bash
./scripts/initialize.sh testnet
```

Or initialize the specific contract manually:

```bash
stellar contract invoke \
  --id $REGISTRATION_CONTRACT_ID \
  --source-account $ADMIN_ADDRESS \
  --network testnet \
  -- initialize \
  --admin $ADMIN_ADDRESS
```

---

## Common Error Codes

| Code | Error | Description | Resolution |
|------|-------|-------------|------------|
| 1 | `AlreadyInitialized` | `initialize()` called twice | No action; contract is already ready |
| 2 | `NotInitialized` | Operations before setup | Call `initialize()` first |
| 3 | `PlayerNotFound` | Invalid `player_id` | Verify player ID from registration tx |
| 4 | `ValidatorNotAuthorized` | Caller not a registered validator | Admin must call `register_validator` |
| 5 | `InvalidProgressTransition` | Level skip or backward transition | Follow the valid transition table |
| 6 | `ScoutNotSubscribed` | Scout accessing talent pool without subscription | Call `subscribe()` first |
| 7 | `InsufficientFee` | Payment below required amount | Check fees via `get_fee_config()` |
| 8 | `AlreadyRegistered` | Duplicate wallet registration | Use existing player or scout ID |
| 9 | `ContractPaused` | Circuit breaker active | Wait for admin to unpause; see `health()` |
| 10 | `Unauthorized` | Wrong account for admin operation | Use the correct admin Stellar account |
| 11 | `Overflow` | Arithmetic overflow in fee calculation | Use amounts within safe `i128` range |

---

## Runbook Checklist — Deployment Day

Use this list after every fresh deployment to testnet or mainnet.

- [ ] All four contracts deployed — IDs written to `.env.contracts`
- [ ] All four `health()` calls return `{"initialized": true, "paused": false, "pay_to_contact_paused": false}`
- [ ] Cross-contract link set: `verification → progress` via `set_progress_contract`
- [ ] Validators registered for demo data
- [ ] Fee configuration verified via `get_fee_config()`
- [ ] Testnet seed run (if applicable): `./testnet/seed.sh`
- [ ] TypeScript bindings regenerated: `./scripts/generate-bindings.sh testnet`
- [ ] Database migration applied: `psql $DATABASE_URL -f migrations/001_initial_schema.sql`
- [ ] End-to-end smoke test: register player → approve milestone → check progress level

---

*Last updated: 2026-08-30*
