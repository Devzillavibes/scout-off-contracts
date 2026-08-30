# TTL Policy

Soroban contracts store state in three buckets — **instance**, **persistent**, and **temporary** — each with its own TTL (time-to-live in ledgers). When a TTL expires the entry is *archived* and reads return nothing (or panic if `.expect()` / `.unwrap()` is used). Every contract in this workspace must keep its storage alive or return typed errors on expiry.

---

## Verification Contract

### Instance storage keys

| Key | Purpose |
|-----|---------|
| `Initialized` | One-time init flag |
| `Paused` | Circuit-breaker flag |
| `TotalMilestoneCount` | Global milestone counter |
| `ProgressContract` | Cross-contract address |
| `ProgressContractSet` | One-time set flag |

### Keep-alive strategy

Every public entrypoint (mutating *and* query) calls `bump_instance_ttl` as its **first operation**:

```rust
#[inline(always)]
fn bump_instance_ttl(env: &Env) {
    env.storage()
        .instance()
        .extend_ttl(INSTANCE_TTL_MIN, INSTANCE_TTL_MAX);
}
```

Constants (in `lib.rs`):

```rust
const INSTANCE_TTL_MIN: u32 = 100;   // ~8 min at 5 s/ledger
const INSTANCE_TTL_MAX: u32 = 500;   // ~40 min at 5 s/ledger
```

This means **any traffic** — whether milestone approvals, validator queries, or admin calls — keeps the instance entry alive. A deployment that receives no calls at all for `INSTANCE_TTL_MAX` ledgers may still archive; the recommended mitigation is a lightweight heartbeat cron job calling `health()`.

### Persistent storage keys

Milestone records (`DataKey::Milestone(player_id, index)`) and the milestone counter (`DataKey::MilestoneCounter(player_id)`) are bumped inside `get_milestone` reads and written with explicit TTL on every `approve_milestone` write.

Admin key (`DataKey::Admin`) is bumped in every `require_admin` call.

---

## Other Contracts

| Contract | Instance TTL strategy |
|----------|-----------------------|
| `scout_access` | `bump_instance_ttl` called in every entrypoint |
| `registration` | Instance counters bumped inline; no dedicated bump helper (low-traffic risk) |
| `progress` | `bump_instance_ttl` called in every entrypoint |

---

## References

- [Soroban Storage & TTL docs](https://developers.stellar.org/docs/learn/smart-contract-internals/state-archival)
- Issue #1158 — verification instance TTL
- Issue #797 — platform-wide guard-ordering
- Issue #704 — GlobalMilestoneIndex persistent migration
