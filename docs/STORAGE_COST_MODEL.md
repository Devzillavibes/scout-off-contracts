# Storage Cost Model

This document describes the on-chain storage growth model for the four
ScoutChain Soroban contracts, covering how many persistent keys each contract
writes per operation, the rent implications, and the projected growth at scale.

## Background

Soroban charges rent on persistent storage entries based on their size and
TTL. Every `set()` on a persistent key creates or updates a ledger entry;
`extend_ttl()` refreshes the rent clock. The platform uses a 30-day TTL
(`518,400` ledgers at ~5 s average close time) for all identity and status
keys — see [TTL_POLICY.md](TTL_POLICY.md) for the full policy.

## Per-Operation Key Growth

### Registration Contract

| Operation | Keys written | Notes |
|-----------|-------------|-------|
| `register_player` | 3 | `Player(id)`, `PlayersByLevel(level)`, `PlayersByLevelRegion(level, region)` |
| `register_scout` | 1 | `Scout(id)` |
| `update_profile` | 1 | `Player(id)` update in-place |

### Verification Contract

| Operation | Keys written | Notes |
|-----------|-------------|-------|
| `register_validator` | 2 | `Validator(wallet)`, `ValidatorVector` updated |
| `approve_milestone` | 4 | `Milestone(player, index)`, `MilestoneCounter(player)`, `EvidenceUsed(hash)`, `ValidatorMilestoneCount(wallet)` |
| `dispute_milestone` | 1 | appended to `OpenDisputeIndex` |
| `resolve_dispute` | 1 | removed from `OpenDisputeIndex` |

### Progress Contract

| Operation | Keys written | Notes |
|-----------|-------------|-------|
| `advance_level` | 3 | `PlayerLevel(player)`, `HistoryEntry(player, index)`, `HistoryCounter(player)` |

### Scout Access Contract

| Operation | Keys written | Notes |
|-----------|-------------|-------|
| `subscribe` | 2 | `Subscription(scout)`, `TierSubscribers(tier)` updated |
| `pay_to_contact` | 3 | `ContactRecord(player, scout)`, `ScoutContacts(scout)` updated, `PlayerContacts(player)` updated |
| `batch_contact_players(n)` | 1 + 2×n | `ProContactCount(scout)`, n × `ContactRecord` + index updates |
| `log_trial_offer` | 4 | `TrialOffer(player, index)`, `TrialCounter(player)`, `TrialOfferLastSent(scout, player)`, `ScoutTrialOffers(scout)` |

## Key-Growth Projections

The following table shows the number of persistent keys expected at various
platform-scale milestones. These are upper bounds — keys that are updated in
place (e.g. `PlayersByLevel`) are counted once regardless of update frequency.

| Milestone | Players | Scouts | Validators | Milestones | Contacts | Total keys (approx.) |
|-----------|---------|--------|------------|------------|----------|----------------------|
| Testnet MVP | 10 | 5 | 5 | 50 | 20 | ~200 |
| Early mainnet | 1,000 | 200 | 50 | 5,000 | 2,000 | ~20,000 |
| Growth | 10,000 | 2,000 | 100 | 50,000 | 50,000 | ~200,000 |

At 200,000 keys, each averaging 500 bytes, the total on-chain storage is
approximately 100 MB — well within Stellar's network capacity but worth
monitoring for rent cost projections.

## Dead / Removed Keys

The `ContactCount(scout, month_bucket)` key was previously written on every
contact operation as a legacy wall-clock quota tracker. It was never read for
enforcement (the renewal-aware `ProContactPeriod` / `ProContactCount` path is
the actual enforcer) and was never TTL-bumped, meaning it slowly archived.
This key was **removed** in the fix for
[issue #1159](https://github.com/scout-off/scout-off-contracts/issues/1159).
See [TTL_POLICY.md](TTL_POLICY.md) for the current set of live keys.

## Unbounded Key Families

The following key families are inherently unbounded and grow with usage.
Each entry is kept alive by the 30-day TTL policy; entries that have not been
touched in 30 days are archived (and may eventually be evicted).

- `ContactRecord(player_id, scout)` — one per unique (player, scout) pair.
  Elite scouts have no contact limit; Pro scouts are capped at
  `pro_contact_limit` per subscription period.
- `EvidenceUsed(hash)` — one per unique evidence CID. Grows monotonically with
  approved milestones. The 100-validator cap and the 5-milestones-per-player-
  per-validator cap bound growth per player, but the global set is unbounded.

## Related Documentation

- [TTL_POLICY.md](TTL_POLICY.md) — per-key TTL values and keep-alive mechanisms.
- [docs/INDEXER.md](INDEXER.md) — the reconciliation script that validates on-chain state against the backend database.
- [README.md — Database Schema](../README.md#database-schema) — the PostgreSQL schema.
