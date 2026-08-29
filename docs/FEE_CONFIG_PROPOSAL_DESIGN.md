# Fee Config Proposal Design

This document describes the two-phase fee configuration update mechanism
(`propose_fee_config` → `activate_fee_config`) and the cancellation path
(`cancel_fee_config_proposal`) that resolves the open item previously listed
in the Rollback and Edge Cases section.

## Overview

Updating live fee parameters is a high-impact admin action. To reduce the
risk of typos or accidental changes taking effect immediately, fee updates
follow a timelock pattern:

1. **Propose** — Admin calls `propose_fee_config(fee_config)`. The proposed
   config is stored under `DataKey::PendingFeeConfig`. The current active
   config is unchanged.
2. **Activate** — After the required delay has elapsed, any caller (or an
   automated keeper) calls `activate_fee_config()`. The pending config
   replaces the active one and `DataKey::PendingFeeConfig` is cleared.
3. **Cancel** *(new — this issue)* — If the admin reconsiders before the
   delay elapses, they call `cancel_fee_config_proposal()`. The pending
   config is removed and the active config remains untouched.

## Rollback and Edge Cases

| Scenario | Behaviour |
|---|---|
| `activate_fee_config` called with no pending proposal | Returns `NoPendingFeeConfig` |
| `cancel_fee_config_proposal` called with no pending proposal | Returns `NoPendingFeeConfig` |
| Second `propose_fee_config` before activation | Overwrites the first proposal; delay clock restarts |
| `cancel_fee_config_proposal` called, then new proposal | Allowed — clean slate after cancellation |

### Cancellation (resolved — was future work)

`cancel_fee_config_proposal(env)` is an admin-only function that:

- Removes `DataKey::PendingFeeConfig` from persistent storage.
- Emits `fee_config_proposal_cancelled` with the cancelled `FeeConfig` in
  the event data so off-chain indexers can record the abandoned change.
- Returns `NoPendingFeeConfig` if there is nothing to cancel.

This closes the open item that previously read:
> *"Emit a cancellation event (not implemented; future work) or manually
> clear the pending state (not exposed; requires contract upgrade)."*

The cancellation is now fully implemented and does not require a contract
upgrade.

## Events

| Event | Topics | Data |
|---|---|---|
| `fee_config_proposal_proposed` | `(event_name, admin)` | `proposed_config: FeeConfig` |
| `fee_config_proposal_cancelled` | `(event_name, admin)` | `cancelled_config: FeeConfig` |
| `fee_config_activated` | `(event_name, admin)` | `(old_config: FeeConfig, new_config: FeeConfig)` |

## Implementation Notes

- `DataKey::PendingFeeConfig` is a new variant added to the `DataKey` enum
  in `contracts/scout_access/src/types.rs`.
- `NoPendingFeeConfig` (error code 24) is a new variant added to
  `ScoutAccessError` in `contracts/scout_access/src/errors.rs`.
- All three functions (`propose_fee_config`, `activate_fee_config`,
  `cancel_fee_config_proposal`) are admin-gated via `require_admin`.
- TTL for `PendingFeeConfig` follows the same `PERSISTENT_TTL_*` policy as
  other admin-facing persistent keys.

## References

- Issue [#1178](https://github.com/scout-off/scout-off-contracts/issues/1178)
- `contracts/scout_access/src/lib.rs` — implementation
- `contracts/scout_access/src/events.rs` — event helpers
- `contracts/scout_access/src/errors.rs` — `NoPendingFeeConfig`
- `docs/CONTRACT_REFERENCE.md` — public API docs
