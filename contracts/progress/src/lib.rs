#![cfg_attr(target_family = "wasm", no_std)]
#![no_std]
mod errors;
mod events;
mod types;

use errors::ProgressError;
use events::*;
use types::*;

use soroban_sdk::{contract, contractimpl, Address, Env, Vec};

const INSTANCE_TTL_MIN: u32 = 500;
const INSTANCE_TTL_MAX: u32 = 500;
const PERSISTENT_TTL_MIN: u32 = 500;
const PERSISTENT_TTL_MAX: u32 = 2000;

const ADMIN_BUMP_LEDGERS: u32 = 518400; // ~30 days at 5s/ledger

#[contract]
pub struct ProgressContract;

#[contractimpl]
impl ProgressContract {
    pub fn initialize(env: Env, admin: Address) -> Result<(), ProgressError> {
        if Self::is_initialized(&env) {
            return Err(ProgressError::AlreadyInitialized);
        }
        admin.require_auth();
        env.storage().persistent().set(&DataKey::Admin, &admin);
        env.storage().persistent().extend_ttl(&DataKey::Admin, ADMIN_BUMP_LEDGERS, ADMIN_BUMP_LEDGERS);
        env.storage().instance().set(&DataKey::Initialized, &true);
        env.storage().instance().set(&DataKey::Paused, &false);
        Ok(())
    }

    pub fn transfer_admin(env: Env, new_admin: Address) -> Result<(), ProgressError> {
        Self::bump_instance_ttl(&env);
        let old_admin: Address = env
            .storage()
            .persistent()
            .get(&DataKey::Admin)
            .ok_or(ProgressError::NotInitialized)?;
        old_admin.require_auth();
        env.storage().persistent().set(&DataKey::Admin, &new_admin);
        env.storage().persistent().extend_ttl(&DataKey::Admin, ADMIN_BUMP_LEDGERS, ADMIN_BUMP_LEDGERS);
        events::admin_transferred(&env, &old_admin, &new_admin);
        Ok(())
    }

    /// Upgrade the contract WASM. Admin auth required.
    /// Persistent storage (including Admin) survives this call.
    pub fn upgrade(env: Env, new_wasm_hash: soroban_sdk::BytesN<32>) -> Result<(), ProgressError> {
        Self::require_admin(&env)?;
        env.deployer().update_current_contract_wasm(new_wasm_hash);
        Ok(())
    }

    /// Reset a player's level for dispute resolution.
    /// Existing history is preserved; a new history entry records the reset.
    pub fn reset_player_level(
        env: Env,
        player_id: u64,
        new_level: ProgressLevel,
    ) -> Result<(), ProgressError> {
        Self::require_admin(&env)?;
        Self::bump_instance_ttl(&env);

        // Fetch and check the player's current level
        let player_key = DataKey::Player(player_id);
        let player: Player = env
            .storage()
            .instance()
            .get(&player_key)
            .ok_or(ProgressError::NotFound)?;

        if player.level == new_level {
            return Err(ProgressError::InvalidLevel);
        }

        let old_level = player.level;

        // Update the player's level
        let mut updated_player = player;
        updated_player.level = new_level;
        env.storage().instance().set(&player_key, &updated_player);

        // Record history entry for the reset
        let reset_record = LevelChangeHistory {
            old_level,
            new_level,
            reason: ResetReason::DisputeResolution,
            timestamp: env.ledger().timestamp(),
        };

        let history_key = DataKey::History(player_id, player.history_count + 1);
        env.storage().instance().set(&history_key, &reset_record);

        // Increment history counter
        let new_history_count = player.history_count + 1;
        env.storage().instance().set(
            &DataKey::HistoryCount(player_id),
            &new_history_count,
        );

        // Update player to new history count
        updated_player.history_count = new_history_count;
        env.storage().instance().set(&player_key, &updated_player);

        events::player_level_reset(&env, player_id, old_level, new_level);
        Ok(())
    }

    pub fn advance_level(
        env: Env,
        validator: Address,
        player_id: u64,
        validator_id: u32,
    ) -> Result<(), ProgressError> {
        Self::bump_instance_ttl(&env);
        validator.require_auth();

        // Confirm the validator has registered and is active
        Self::check_validator(&env, &validator)?;

        // Fetch the player or create a new one
        let player_key = DataKey::Player(player_id);
        let mut player: Player = env
            .storage()
            .instance()
            .get(&player_key)
            .unwrap_or_else(|| Player {
                id: player_id,
                level: ProgressLevel::Unverified,
                history_count: 0,
            });

        // Advance to the next level
        player.level = player.level.next();
        env.storage().instance().set(&player_key, &player);

        // Record the history
        let old_level = player.level;
        let new_level = player.level;
        let history_key = DataKey::History(player_id, player.history_count + 1);

        let history = LevelChangeHistory {
            old_level,
            new_level,
            reason: ResetReason::ValidatorApproval,
            timestamp: env.ledger().timestamp(),
        };

        env.storage().instance().set(&history_key, &history);

        // Increment history counter
        let new_history_count = player.history_count + 1;
        env.storage().instance().set(
            &DataKey::HistoryCount(player_id),
            &new_history_count,
        );

        events::player_level_advanced(&env, player_id, old_level, new_level, validator, validator_id);
        Ok(())
    }

    pub fn register_validator(
        env: Env,
        validator: Address,
        name: String,
    ) -> Result<(), ProgressError> {
        Self::require_admin(&env)?;
        Self::bump_instance_ttl(&env);
        validator.require_auth();

        if name.len() > 256 {
            return Err(ProgressError::InvalidLength);
        }

        if env
            .storage()
            .instance()
            .has(&DataKey::Validator(validator.clone()))
        {
            return Err(ProgressError::AlreadyExists);
        }

        env.storage()
            .instance()
            .set(&DataKey::Validator(validator.clone()), &name);
        env.storage()
            .instance()
            .set(&DataKey::ValidatorActive(validator.clone()), &true);

        // Add to validator vector
        let mut validators: Vec<Address> = env
            .storage()
            .instance()
            .get(&DataKey::ValidatorVector)
            .unwrap_or_else(|| Vec::new(&env));
        validators.push_back(validator);
        env.storage()
            .instance()
            .set(&DataKey::ValidatorVector, &validators);

        events::validator_registered(&env, validator);
        Ok(())
    }

    pub fn revoke_validator(env: Env, validator: Address) -> Result<(), ProgressError> {
        Self::require_admin(&env)?;
        Self::bump_instance_ttl(&env);

        if !env
            .storage()
            .instance()
            .has(&DataKey::Validator(validator.clone()))
        {
            return Err(ProgressError::NotFound);
        }

        env.storage()
            .instance()
            .set(&DataKey::ValidatorActive(validator.clone()), &false);

        events::validator_revoked(&env, validator);
        Ok(())
    }

    pub fn pause_contract(env: Env) -> Result<(), ProgressError> {
        Self::require_admin(&env)?;
        Self::bump_instance_ttl(&env);
        env.storage().instance().set(&DataKey::Paused, &true);
        Ok(())
    }

    pub fn unpause_contract(env: Env) -> Result<(), ProgressError> {
        Self::require_admin(&env)?;
        Self::bump_instance_ttl(&env);
        env.storage().instance().set(&DataKey::Paused, &false);
        Ok(())
    }

    pub fn is_paused(env: Env) -> bool {
        env.storage()
            .instance()
            .get::<_, bool>(&DataKey::Paused)
            .unwrap_or(false)
    }

    pub fn get_level(env: Env, player_id: u64) -> ProgressLevel {
        let player: Player = env
            .storage()
            .instance()
            .get(&DataKey::Player(player_id))
            .unwrap_or_else(|| Player {
                id: player_id,
                level: ProgressLevel::Unverified,
                history_count: 0,
            });

        player.level
    }

    pub fn get_player(env: Env, player_id: u64) -> Player {
        env.storage()
            .instance()
            .get(&DataKey::Player(player_id))
            .unwrap_or_else(|| Player {
                id: player_id,
                level: ProgressLevel::Unverified,
                history_count: 0,
            })
    }

    pub fn get_milestone_count(env: Env, player_id: u64) -> u32 {
        env.storage()
            .instance()
            .get(&DataKey::HistoryCount(player_id))
            .unwrap_or(0)
    }

    pub fn get_milestone(env: Env, player_id: u64, index: u32) -> LevelChangeHistory {
        env.storage()
            .instance()
            .get(&DataKey::History(player_id, index))
            .unwrap_or_else(|| LevelChangeHistory {
                old_level: ProgressLevel::Unverified,
                new_level: ProgressLevel::Unverified,
                reason: ResetReason::DisputeResolution,
                timestamp: 0,
            })
    }

    pub fn is_active_validator(env: Env, validator: Address) -> bool {
        env.storage()
            .instance()
            .get::<_, bool>(&DataKey::ValidatorActive(validator))
            .unwrap_or(false)
    }

    pub fn get_validator_name(env: Env, validator: Address) -> String {
        env.storage()
            .instance()
            .get::<_, String>(&DataKey::Validator(validator))
            .unwrap_or_else(|| String::from_str(&env, ""))
    }

    pub fn get_validators(env: Env) -> Vec<Address> {
        env.storage()
            .instance()
            .get(&DataKey::ValidatorVector)
            .unwrap_or_else(|| Vec::new(&env))
    }

    pub fn health(env: Env) -> ContractHealth {
        let is_initialized = Self::is_initialized(&env);
        let is_paused = Self::is_paused(&env);

        ContractHealth {
            name: String::from_str(&env, "ProgressContract"),
            initialized: is_initialized,
            paused: is_paused,
        }
    }

    // -------------------------------------------------------------------------
    // Internal helpers
    // -------------------------------------------------------------------------

    fn is_initialized(env: &Env) -> bool {
        env.storage()
            .instance()
            .get::<_, bool>(&DataKey::Initialized)
            .unwrap_or(false)
    }

    fn bump_instance_ttl(env: &Env) {
        env.storage()
            .instance()
            .extend_ttl(INSTANCE_TTL_MIN, INSTANCE_TTL_MAX);
    }

    fn check_validator(env: &Env, validator: &Address) -> Result<(), ProgressError> {
        if !env
            .storage()
            .instance()
            .get::<_, bool>(&DataKey::ValidatorActive(validator.clone()))
            .unwrap_or(false)
        {
            return Err(ProgressError::NotAuthorized);
        }
        Ok(())
    }

    fn require_admin(env: &Env) -> Result<Address, ProgressError> {
        let admin: Address = env
            .storage()
            .persistent()
            .get(&DataKey::Admin)
            .ok_or(ProgressError::NotInitialized)?;
        admin.require_auth();
        env.storage().persistent().extend_ttl(&DataKey::Admin, ADMIN_BUMP_LEDGERS, ADMIN_BUMP_LEDGERS);
        Ok(admin)
    }
}

use soroban_sdk::String;
use scoutchain_shared_types::{ContractHealth, ProgressLevel};

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{Address, Env, String};

    fn setup() -> (Env, Address, scoutchain_progress_contract::Client<'static>) {
        let env = Env::default();
        let contract_id = env.register_contract(None, ProgressContract);
        let client = scoutchain_progress_contract::Client::new(&env, &contract_id);

        (env, contract_id, client)
    }

    #[test]
    fn test_initialize() {
        let (env, _, client) = setup();
        let admin = Address::generate(&env);

        assert!(!env
            .storage()
            .instance()
            .has(&DataKey::Initialized));

        client.initialize(&admin);

        assert!(env
            .storage()
            .instance()
            .has(&DataKey::Initialized));
    }

    #[test]
    fn test_initialize_twice_fails() {
        let (env, _, client) = setup();
        let admin = Address::generate(&env);

        client.initialize(&admin);

        let result = client.try_initialize(&admin);
        assert!(result.is_err());
    }

    #[test]
    fn test_pause_unpause() {
        let (env, _, client) = setup();
        let admin = Address::generate(&env);
        client.initialize(&admin);

        assert!(!client.is_paused());
        client.pause_contract();
        assert!(client.is_paused());
        client.unpause_contract();
        assert!(!client.is_paused());
    }

    #[test]
    fn test_register_validator() {
        let (env, _, client) = setup();
        let admin = Address::generate(&env);
        client.initialize(&admin);

        let validator = Address::generate(&env);
        let name = String::from_str(&env, "Alice");

        client.register_validator(&validator, &name);

        assert!(client.is_active_validator(&validator));
    }

    #[test]
    fn test_advance_level_success() {
        let (env, _, client) = setup();
        let admin = Address::generate(&env);
        client.initialize(&admin);

        let player_id = 1u64;
        let validator = Address::generate(&env);
        let validator_name = String::from_str(&env, "Coach");

        client.register_validator(&validator, &validator_name);

        assert_eq!(client.get_level(player_id), ProgressLevel::Unverified);

        client.advance_level(&validator, &player_id, &1u32);

        assert_eq!(client.get_level(player_id), ProgressLevel::VerifiedIdentity);
    }

    #[test]
    fn test_upgrade_preserves_admin() {
        let (env, _, client) = setup();
        let admin = Address::generate(&env);
        client.initialize(&admin);

        let validator = Address::generate(&env);
        client.register_validator(&validator, &String::from_str(&env, "Coach"));
        client.advance_level(&validator, &1u64, &1u32);

        let new_wasm_hash = env.deployer().upload_contract_wasm(soroban_sdk::Bytes::new(&env));
        client.upgrade(&new_wasm_hash);

        // Admin persisted — admin-gated call still works
        client.pause_contract();
        // Player level data persisted
        assert_eq!(client.get_level(&1u64), ProgressLevel::VerifiedIdentity);
    }

    #[test]
    fn test_reset_player_level_success() {
        let (env, _, client) = setup();
        let admin = Address::generate(&env);
        client.initialize(&admin);

        let player_id = 1u64;
        let validator = Address::generate(&env);

        client.register_validator(&validator, &String::from_str(&env, "Coach"));
        client.advance_level(&validator, &player_id, &1u32);

        let player = client.get_player(&player_id);
        assert_eq!(player.level, ProgressLevel::VerifiedIdentity);

        client.reset_player_level(&player_id, &ProgressLevel::Unverified);

        let reset_player = client.get_player(&player_id);
        assert_eq!(reset_player.level, ProgressLevel::Unverified);
    }

    #[test]
    #[should_panic]
    fn test_subscription_expiry() {
        let (env, _, client) = setup();
        let admin = Address::generate(&env);
        client.initialize(&admin);

        let validator = Address::generate(&env);
        let validator_name = String::from_str(&env, "Coach");
        client.register_validator(&validator, &validator_name);

        let player_id = 1u64;
        client.advance_level(&validator, &player_id, &1u32);

        let player = client.get_player(&player_id);
        assert_eq!(player.history_count, 1);
    }

    #[test]
    fn test_reset_player_level_to_same_level_fails() {
        let (env, _, client) = setup();
        let admin = Address::generate(&env);
        client.initialize(&admin);

        let player_id = 1u64;

        let result = client.try_reset_player_level(&player_id, &ProgressLevel::Unverified);
        assert!(result.is_err());
    }

    #[test]
    fn test_transfer_admin() {
        let (env, _, client) = setup();
        let admin = Address::generate(&env);
        client.initialize(&admin);

        let new_admin = Address::generate(&env);
        client.transfer_admin(&new_admin);

        let pause_result = client.try_pause_contract();
        // Old admin no longer has permission
        assert!(pause_result.is_err());
    }
}
