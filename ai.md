# ScoutChain — AI Assistant Integration Guide

This document is the authoritative reference for AI assistants and new team members
working across the ScoutChain repositories (`scoutchain-contracts`,
`scoutchain-backend`, `scoutchain-frontend`).

---

## Project Overview

ScoutChain is a decentralised football talent-scouting platform built on the
Stellar network using Soroban smart contracts (Rust). It solves the visibility
problem for undiscovered players in underserved regions by providing:

- **Tamper-proof player profiles** — on-chain identity backed by IPFS/Arweave
  highlight reels and verified stats.
- **Four-tier progress verification** — milestones approved by authorised
  validators (coaches, academy directors, certified trainers) advance a player
  from *Unverified* (Level 0) to *Elite Tier* (Level 3).
- **Scout discovery** — scouts filter the talent pool by region, position, and
  verified level, then pay micro-fees in XLM to unlock contact details.
- **On-chain trial offers** — scouts with an Elite subscription log trial offers
  that advance a player to Level 3 and form an immutable connection record.

Stellar underpins every payment: fees settle in 3–5 seconds at fractions of a
cent, making cross-border microtransactions practical for scouts and players
alike.

---

## Repository Layout

```
scoutchain-contracts/
├── contracts/
│   ├── registration/       # Player & scout on-chain identity
│   ├── verification/       # Validator registry & milestone approvals
│   ├── progress/           # Four-tier level state machine
│   └── scout_access/       # Subscriptions, pay-to-contact, trial offers
├── bindings/               # Auto-generated TypeScript clients (post-deploy)
├── migrations/
│   └── 001_initial_schema.sql
├── scripts/
│   ├── setup-testnet.sh    # One-command full testnet setup
│   ├── deploy.sh
│   ├── initialize.sh       # Initialise contracts + wire cross-contract link
│   └── generate-bindings.sh
├── testnet/seed.sh
├── config/
│   ├── testnet.json
│   └── mainnet.json
├── .env.example
└── Cargo.toml
```

---

## Cross-Contract Wiring

After deploying the four contracts you **must** run one additional wiring step
before the platform is fully operational. This wiring stores the address of one
contract inside another so that cross-contract calls can be made at runtime.

### Why wiring is required

`approve_milestone` in the **verification** contract needs to call
`advance_level` on the **progress** contract so that both state changes — the
milestone record and the progress-level update — happen atomically within the
same Stellar transaction. The progress contract address is not known at
compile time, so it must be registered on-chain after both contracts are
deployed.

### Wiring table

| # | Setter function | Called on contract | Registers address of | Direction |
|---|---|---|---|---|
| 1 | `set_progress_contract(progress_contract)` | `verification` | `progress` | verification → progress |

**That is the only cross-contract wiring link in this codebase.**

### How to apply the wiring

The easiest way is to run `./scripts/initialize.sh`, which initialises all four
contracts and applies the wiring step automatically:

```bash
./scripts/initialize.sh testnet
```

To apply it manually:

```bash
stellar contract invoke \
  --id $VERIFICATION_CONTRACT_ID \
  --source $DEPLOYER_SECRET \
  --network testnet \
  -- set_progress_contract \
  --progress_contract $PROGRESS_CONTRACT_ID
```

> **Warning:** Without this step, validators can still call `approve_milestone`
> and milestones will be recorded, but **player progress levels will not
> advance**. Always wire before going to production.

---

## TypeScript Bindings

After deploying the contracts, generate TypeScript clients with:

```bash
./scripts/generate-bindings.sh testnet
```

Clients are written to `bindings/{contract}/`. Import them in the backend or
frontend:

```typescript
import { Client as RegistrationClient } from "@scoutchain/bindings-registration";
import { Client as VerificationClient } from "@scoutchain/bindings-verification";
import { Client as ProgressClient }     from "@scoutchain/bindings-progress";
import { Client as ScoutAccessClient }  from "@scoutchain/bindings-scout-access";
```

See `bindings/README.md` for full usage details including auth helpers and
network configuration.

---

## Key Contract Functions Reference

### registration contract

| Function | Auth | Description |
|---|---|---|
| `initialize(admin)` | admin | One-time setup |
| `register_player(wallet, vitals, ipfs_hashes)` | player wallet | Create Level-0 player profile |
| `update_profile(player_id, ipfs_hashes)` | player wallet | Update IPFS content hashes |
| `register_scout(wallet, region)` | scout wallet | Create scout profile |
| `get_player(player_id)` | — | Fetch full player profile |
| `get_player_by_wallet(wallet)` | — | Look up player by wallet address |
| `get_scout(scout_id)` | — | Fetch scout profile |
| `pause_contract()` / `unpause_contract()` | admin | Circuit breaker |
| `health()` | — | Returns `true` when initialised |

### verification contract

| Function | Auth | Description |
|---|---|---|
| `initialize(admin)` | admin | One-time setup |
| `set_progress_contract(progress_contract)` | admin | **Wiring step** — register progress contract address |
| `register_validator(wallet, credentials)` | admin | Add trusted validator |
| `revoke_validator(wallet)` | admin | Deactivate validator |
| `approve_milestone(validator_wallet, player_id, description, evidence_hash)` | validator | Record milestone; cross-calls `progress.advance_level` |
| `get_milestone(player_id, index)` | — | Fetch a specific milestone |
| `get_milestone_count(player_id)` | — | Number of milestones for a player |
| `is_active_validator(wallet)` | — | Check validator status |
| `pause_contract()` / `unpause_contract()` | admin | Circuit breaker |
| `health()` | — | Returns `true` when initialised |

### progress contract

| Function | Auth | Description |
|---|---|---|
| `initialize(admin)` | admin | One-time setup |
| `advance_level(caller, player_id, milestone_ref)` | validator / verification contract | Advance player one tier (called cross-contract by verification) |
| `get_level(player_id)` | — | Current progress level |
| `get_history_count(player_id)` | — | Number of history entries |
| `get_history_entry(player_id, index)` | — | A single history entry |
| `transfer_admin(new_admin)` | admin | Hand off admin rights |
| `pause_contract()` / `unpause_contract()` | admin | Circuit breaker |
| `health()` | — | Returns `true` when initialised |

### scout_access contract

| Function | Auth | Description |
|---|---|---|
| `initialize(admin, xlm_token, fee_config)` | admin | One-time setup |
| `subscribe(scout, tier)` | scout wallet | Purchase Basic / Pro / Elite subscription (XLM fee) |
| `pay_to_contact(scout, player_id)` | scout wallet | Pay micro-fee to unlock player contact (active subscription required) |
| `log_trial_offer(scout, player_id, details_hash)` | scout wallet (Elite tier) | Record trial offer; backend should then call `progress.advance_level` to reach Level 3 |
| `get_subscription(scout)` | — | Fetch subscription details |
| `get_fee_config()` | — | Current fee schedule |
| `get_accumulated_fees()` | — | Platform fees awaiting withdrawal |
| `has_contacted(scout, player_id)` | — | Check if scout already contacted a player |
| `get_trial_offer(player_id, index)` | — | Fetch a specific trial offer |
| `get_trial_count(player_id)` | — | Number of trial offers for a player |
| `update_fee_config(fee_config)` | admin | Adjust fee schedule |
| `withdraw_fees(to)` | admin | Withdraw accumulated fees |
| `pause_contract()` / `unpause_contract()` | admin | Circuit breaker |
| `health()` | — | Returns `true` when initialised |

---

## Progress Level Model

| Level | Name | How to reach |
|---|---|---|
| 0 | Unverified | Player calls `register_player` |
| 1 | VerifiedIdentity | Validator calls `approve_milestone` (identity confirmed) |
| 2 | PerformanceMilestones | Validator calls `approve_milestone` (performance stats verified) |
| 3 | EliteTier | Scout with Elite subscription calls `log_trial_offer`; backend calls `progress.advance_level` |

Levels are strictly sequential — no skipping, no going backwards.

---

## Error Codes

| Code | Name | Common Cause |
|---|---|---|
| 1 | AlreadyInitialized | `initialize` called twice |
| 2 | NotInitialized | Operation before `initialize` |
| 3 | PlayerNotFound | Invalid `player_id` |
| 4 | ValidatorNotAuthorized | Unregistered account approving milestone |
| 5 | InvalidProgressTransition | Skipping levels or going backwards |
| 6 | ScoutNotSubscribed | No active subscription |
| 7 | InsufficientFee | Payment below required amount |
| 8 | AlreadyRegistered | Duplicate registration |
| 9 | ContractPaused | Emergency circuit breaker active |
| 10 | Unauthorized | Wrong account for admin operation |
| 11 | Overflow | Arithmetic overflow in fee calculation |

---

## Events

| Event | Emitted by | Trigger |
|---|---|---|
| `player_registered` | registration | New player profile created |
| `profile_updated` | registration | Player updates IPFS hashes |
| `scout_registered` | registration | New scout profile created |
| `validator_registered` | verification | Admin adds a validator |
| `validator_revoked` | verification | Admin removes a validator |
| `milestone_approved` | verification | Validator confirms a player achievement |
| `progress_updated` | progress | Player advances to a new level |
| `admin_transferred` | progress | Admin rights transferred |
| `scout_subscribed` | scout_access | Scout purchases a subscription |
| `player_contacted` | scout_access | Scout pays to unlock player contact |
| `trial_offer_logged` | scout_access | Scout records a trial offer |
| `fees_withdrawn` | scout_access | Admin withdraws platform fees |

---

## Quick-Start Checklist for AI Assistants

1. **Read this file first** — it is the single source of truth for cross-repo
   integration.
2. **One wiring link exists**: `verification.set_progress_contract` →
   registers the `progress` contract address. Check `scripts/initialize.sh`
   for the exact invocation.
3. **Do not add phantom wiring links** — there is no `set_verification_contract`,
   `set_registration_contract`, or any other setter. If you see references to
   multiple wiring links in other documents they are incorrect.
4. **TypeScript clients live in `bindings/`** — always import from there rather
   than hand-rolling RPC calls.
5. **All four contracts have a circuit breaker** — check `health()` before
   assuming a contract is operational.
