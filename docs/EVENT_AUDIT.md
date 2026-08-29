# Event Audit Tool — `scripts/audit-event-history.js`

The `audit-event-history.js` script replays on-chain event history for one or
more players and validates it against the current contract state. It is the
primary operator tool for detecting off-chain indexer drift and event-chain
integrity issues.

## Usage

```bash
# Audit a single player
node scripts/audit-event-history.js player <player_id> [options]

# Audit all players (enumerates via registration.get_player_count)
node scripts/audit-event-history.js all [--sample <n>] [options]

# Common options
--network <name>     Soroban network alias (default: testnet)
--rpc-url <url>      Soroban RPC URL
--source <identity>  Stellar CLI identity (if required by CLI config)
--json               Emit report as JSON
```

### `all` mode

`all` mode calls `registration.get_player_count` and iterates player IDs
`1..=count`, respecting `--sample N` to cap the number audited. Prior to
this fix (#1176), the mode used a hardcoded placeholder list `[1, 2, 3]`
and ignored `--sample` entirely.

```bash
# Audit up to 10 players from the live contract
node scripts/audit-event-history.js all --sample 10 --network testnet

# CI reconciliation pipeline
node scripts/audit-event-history.js all --sample 100 --network testnet
```

## Event Types Audited

| Event | Source | What is validated |
|---|---|---|
| `player_registered` | registration | Player exists in contract state |
| `milestone_approved` | verification | Milestone index within bounds |
| `progress_updated` | progress | Level transition is valid (0→1→2→3) |
| `scout_subscribed` | scout_access | Subscription record matches tier |
| `player_contacted` | scout_access | Contact record exists |
| `trial_offer_logged` | scout_access | Trial offer record exists |
| `fees_withdrawn` | scout_access | Balance decreased by withdrawal amount |
| `admin_transferred` | all | Admin address matches post-event state |
| `attestation_recorded` | verification | Vote counted for correct round; revoked validator votes stripped |
| `attestation_window_expired` | verification | Post-expiry votes not counted toward old round |
| `validator_pending_votes_invalidated` | verification | Tally reset after invalidation event |

## k-of-n Attestation Replay (#1177)

The tool reconstructs pending-claim tallies from the three attestation event
types and validates the following invariants:

1. **No post-expiry vote in old round** — a vote emitted after
   `attestation_window_expired` for the same `(player_id, round)` must not
   be counted toward that round's threshold.
2. **Commit requires exactly threshold distinct active validators** — a
   milestone is only committed when `k` distinct active (non-revoked)
   validators have voted in the same round.
3. **Revoked validator vote stripped** — if a validator is later found to
   have been revoked at vote time, their vote must not count toward the
   threshold.

### Flags reported

| Flag | Meaning |
|---|---|
| `POST_EXPIRY_VOTE` | A vote was counted for a round after its window expired |
| `COMMIT_WITHOUT_THRESHOLD` | A milestone committed with fewer than `k` valid votes |
| `REVOKED_VALIDATOR_VOTE_COUNTED` | A revoked validator's vote contributed to a commit |

### Test fixture

`scripts/fixtures/attestation-sequence.json` contains a multi-round
attestation sequence (two full rounds with one revocation mid-round) that the
tool validates correctly. Run:

```bash
node scripts/audit-event-history.js player 1 \
  --fixture scripts/fixtures/attestation-sequence.json
```

## Known Limitations

1. Evidence-access events (`evidence_accessed`) are replayed but not
   chain-validated against on-chain evidence hashes (see #1177 companion item).
2. Fee config history is event-log-only; the tool reports it but does not
   diff against live state (no per-row on-chain analog).
3. The tool requires `stellar` CLI on `PATH` and a reachable Soroban RPC
   endpoint; it does not work fully offline.
4. Ledger sequence gaps (e.g. archival) may cause false-positive drift reports.
5. Very large player counts may exceed a single invocation's time budget; use
   `--sample` and run in batches.

> **Known Limitation 6 — resolved.** k-of-n attestation events
> (`attestation_recorded`, `attestation_window_expired`,
> `validator_pending_votes_invalidated`) were previously not replayed or
> chain-validated. This is now implemented as of #1177.
