# Contract Reference

Quick reference for all four ScoutChain Soroban contracts.

---

## registration

Handles player and scout on-chain identity.

| Function | Auth | Description |
|----------|------|-------------|
| `initialize(admin)` | admin | One-time setup |
| `register_player(wallet, vitals, ipfs_hashes)` | wallet | Create player profile at Level 0 |
| `update_profile(player_id, ipfs_hashes)` | player wallet | Update IPFS content hashes |
| `register_scout(wallet, region)` | wallet | Create scout profile |
| `get_player(player_id)` | — | Read player profile |
| `get_player_by_wallet(wallet)` | — | Lookup player by wallet |
| `get_scout(scout_id)` | — | Read scout profile |
| `pause_contract()` / `unpause_contract()` | admin | Circuit breaker |
| `health()` | — | Returns true if initialized |

### ScoutChainError Codes

| Code | Error | Description |
|------|-------|-------------|
| 1 | `AlreadyInitialized` | Contract has already been initialized |
| 2 | `NotInitialized` | Contract has not been initialized yet |
| 3 | `PlayerNotFound` | Player ID does not exist |
| 4 | `ValidatorNotAuthorized` | Caller is not a registered and active validator |
| 5 | `InvalidProgressTransition` | Requested level transition is not allowed |
| 6 | `ScoutNotSubscribed` | Scout does not have an active subscription |
| 7 | `InsufficientFee` | Payment amount is below the required fee |
| 8 | `AlreadyRegistered` | Wallet already has a registered profile |
| 9 | `ContractPaused` | Contract is paused by the emergency circuit breaker |
| 10 | `Unauthorized` | Caller is not authorized for the requested operation |
| 11 | `Overflow` | Arithmetic overflow in fee calculation |
| 12 | `ScoutNotFound` | Scout ID does not exist |
| 13 | `InvalidInput` | One or more input parameters are invalid |
| 14 | `ValidatorCapReached` | Maximum number of registered validators has been reached |
| 15 | `PlayerCapReached` | Maximum number of registered players has been reached |
| 16 | `RegistrationCooldown` | Registration attempted before the cooldown period has elapsed |
| 17 | `PlayerRecordEvicted` | Player record was evicted from contract storage |
| 18 | `ScoutRecordEvicted` | Scout record was evicted from contract storage |

---

## verification

Manages the trusted validator registry and milestone approvals.

| Function | Auth | Description |
|----------|------|-------------|
| `initialize(admin)` | admin | One-time setup |
| `set_progress_contract(progress_contract)` | admin | Wire cross-contract link |
| `register_validator(wallet, credentials)` | admin | Add trusted validator |
| `revoke_validator(wallet)` | admin | Deactivate validator |
| `approve_milestone(validator_wallet, player_id, description, evidence_hash)` | validator | Record milestone (with ledger_sequence for audit) + cross-call progress.advance_level |
| `get_milestone(player_id, index)` | — | Read a specific milestone |
| `get_milestone_count(player_id)` | — | Total milestones for a player |
| `get_validator(wallet)` | — | Read validator record |
| `is_active_validator(wallet)` | — | Boolean check |
| `pause_contract()` / `unpause_contract()` | admin | Circuit breaker |
| `health()` | — | Returns true if initialized |

---

## progress

Maintains the tamper-proof four-tier level state machine.

| Function | Auth | Description |
|----------|------|-------------|
| `initialize(admin)` | admin | One-time setup |
| `advance_level(caller, player_id, milestone_ref)` | caller (validator or scout) | Move player up one level |
| `get_level(player_id)` | — | Current progress level |
| `get_history_count(player_id)` | — | Number of level changes |
| `get_history_entry(player_id, index)` | — | Specific history entry |
| `pause_contract()` / `unpause_contract()` | admin | Circuit breaker |
| `health()` | — | Returns true if initialized |

### ProgressError Codes

| Code | Error | Description |
|------|-------|-------------|
| 1 | `AlreadyInitialized` | Contract has already been initialized |
| 2 | `NotInitialized` | Contract has not been initialized yet |
| 3 | `ContractPaused` | Contract is paused by the emergency circuit breaker |
| 4 | `Unauthorized` | Caller is not authorized for the requested operation |
| 5 | `InvalidProgressTransition` | Requested level transition is not allowed |
| 6 | `AlreadyAtMaxLevel` | Player is already at the maximum progress level (EliteTier) |
| 7 | `PlayerNotFound` | Player ID does not exist in progress storage |
| 8 | `HistoryNotFound` | Progress history record does not exist for this player |
| 9 | `InvalidHistoryEntry` | History entry data is malformed or inconsistent |
| 10 | `ProgressRecordEvicted` | Progress record was evicted from contract storage |
| 11 | `MigrationNotActive` | Migration operation attempted when no migration is in progress |
| 12 | `HistoryAlreadyExists` | History entry for this level already exists for the player |
| 13 | `MerkleRootMismatch` | Provided Merkle root does not match the stored root |
| 14 | `InvalidHistoryIndex` | Requested history index is out of bounds |
| 15 | `PlayerLevelRecordEvicted` | Player level record was evicted from contract storage |

---

## scout_access

Handles scout subscriptions, pay-to-contact, and trial offer logging.

| Function | Auth | Description |
|----------|------|-------------|
| `initialize(admin, xlm_token, fee_config)` | admin | One-time setup |
| `update_fee_config(fee_config)` | admin | Adjust fee rates |
| `withdraw_fees(to)` | admin | Collect platform revenue |
| `subscribe(scout, tier)` | scout | Purchase Basic/Pro/Elite subscription |
| `pay_to_contact(scout, player_id)` | scout | Pay micro-fee to unlock player contact |
| `log_trial_offer(scout, player_id, details_hash)` | scout (Elite only) | Record trial offer on-chain |
| `get_subscription(scout)` | — | Read subscription record |
| `get_fee_config()` | — | Current fee configuration |
| `get_accumulated_fees()` | — | Platform fees pending withdrawal |
| `has_contacted(scout, player_id)` | — | Boolean contact check |
| `get_trial_offer(player_id, index)` | — | Read a trial offer |
| `get_trial_count(player_id)` | — | Total trial offers for a player |
| `pause_contract()` / `unpause_contract()` | admin | Circuit breaker |
| `health()` | — | Returns true if initialized |

### ScoutAccessError Codes

| Code | Error | Description |
|------|-------|-------------|
| 1 | `AlreadyInitialized` | Contract has already been initialized |
| 2 | `NotInitialized` | Contract has not been initialized yet |
| 3 | `ContractPaused` | Contract is paused by admin (circuit breaker) |
| 4 | `Unauthorized` | Caller is not authorized for this operation |
| 5 | `InsufficientFee` | Payment amount is below the required fee |
| 6 | `ScoutNotSubscribed` | Scout does not have an active subscription |
| 7 | `SubscriptionExpired` | Scout's subscription has expired |
| 8 | `AlreadyContacted` | Scout has already contacted this player |
| 9 | `InvalidTier` | Subscription tier value is not valid |
| 10 | `Overflow` | Arithmetic overflow in fee calculation |
| 11 | `TrialOfferNotFound` | Trial offer record does not exist |
| 12 | `PlayerNotRegistered` | Player is not registered in the registration contract |
| 13 | `ScoutNotRegistered` | Scout is not registered in the registration contract |
| 14 | `PlayerCapReached` | Maximum number of players per scout has been reached |
| 15 | `SubscriptionNotFound` | Subscription record not found for this scout |
| 16 | `ContactRecordNotFound` | Contact record not found |
| 17 | `TrialOfferExpired` | Trial offer has passed its expiry ledger |
| 18 | `InvalidSubscriptionDuration` | Subscription duration value is not valid |
| 19 | `FeeConfigNotFound` | Fee configuration has not been set |
| 20 | `TokenTransferFailed` | XLM or platform token transfer failed |
| 21 | `InvalidContactFee` | Contact fee value is not valid |
| 22 | `InvalidSubFee` | Subscription fee value is not valid |
| 23 | `EliteOnlyFeature` | Operation requires an Elite-tier subscription |
| 24 | `MigrationAlreadyComplete` | Migration has already been completed |
| 25 | `MigrationNotFound` | Migration record does not exist |
| 26 | `InvalidMigrationVersion` | Migration version number is not valid |
| 27 | `MigrationDataCorrupted` | Migration data failed integrity check |
| 28 | `MigrationStateMismatch` | Migration state does not match expected state |
| 29 | `MigrationNotActive` | Migration is not currently active |
| 30 | `MigrationReplayDetected` | Migration replay attempt detected |
| 31 | `MigrationConflict` | Migration conflicts with existing state |
| 32 | `MigrationVersionMismatch` | Migration version does not match current contract version |
| 33 | `MigrationChecksumFailed` | Migration checksum verification failed |
| 34 | `MigrationRollbackFailed` | Migration rollback could not be completed |
| 35 | `SubscriptionRecordEvicted` | Subscription record was evicted from contract storage |
| 36 | `PayToContactPaused` | Pay-to-contact feature is currently paused |
| 37 | `TrialEscrowNotOutstanding` | No outstanding trial escrow exists for this player |

---

## Progress Levels

| Integer | Enum | Trigger |
|---------|------|---------|
| 0 | `Unverified` | Profile created |
| 1 | `VerifiedIdentity` | Validator approves identity milestone |
| 2 | `PerformanceMilestones` | Validator approves performance milestone |
| 3 | `EliteTier` | Scout logs trial offer |

---

## Events

| Event | Contract | Emitted When |
|-------|----------|-------------|
| `player_registered` | registration | New player profile created |
| `scout_registered` | registration | New scout profile created |
| `profile_updated` | registration | Player updates IPFS content hashes |
| `milestone_approved` | verification | Validator confirms a player achievement |
| `progress_updated` | progress | Player advances to a new level |
| `scout_subscribed` | scout_access | Scout purchases a subscription |
| `player_contacted` | scout_access | Scout pays to unlock player contact |
| `trial_offer_logged` | scout_access | Scout records a trial offer |
| `fees_withdrawn` | scout_access | Admin withdraws accumulated fees |
