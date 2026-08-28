mod errors;
mod events;
mod types;

use errors::ScoutChainError;
use types::{DataKey, PlayerProfile, PlayerVitals, ProgressLevel, ScoutProfile};

use soroban_sdk::{contract, contractimpl, Address, Env, String, Vec};

const MAX_STRING_LEN: u32 = 64;
const MAX_REGION_LEN: u32 = 128;
const MAX_IPFS_HASHES: u32 = 10;

// Instance storage TTL constants (in ledger closures, ~10 seconds per closure)
const INSTANCE_TTL_MIN: u32 = 500;   // ~1.4 hours
const INSTANCE_TTL_MAX: u32 = 500;   // ~1.4 hours

// Persistent storage TTL constants
const PERSISTENT_TTL_MIN: u32 = 500;    // ~1.4 hours
const PERSISTENT_TTL_MAX: u32 = 2000;   // ~5.5 hours

// Admin persistent key bump interval (~30 days)
const ADMIN_BUMP_LEDGERS: u32 = 518400;

#[contract]
pub struct RegistrationContract;

#[contractimpl]
impl RegistrationContract {
    // -------------------------------------------------------------------------
    // Admin
    // -------------------------------------------------------------------------

    /// One-time contract initialisation. Must be called before any other function.
    pub fn initialize(env: Env, admin: Address) -> Result<(), ScoutChainError> {
        if env.storage().instance().has(&DataKey::Initialized) {
            return Err(ScoutChainError::AlreadyInitialized);
        }
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::Initialized, &true);
        env.storage().instance().set(&DataKey::Paused, &false);
        env.storage().instance().set(&DataKey::PlayerCounter, &0u64);
        env.storage().instance().set(&DataKey::ScoutCounter, &0u64);
        Self::bump_instance_ttl(&env);
        Ok(())
    }

    pub fn pause_contract(env: Env) -> Result<(), ScoutChainError> {
        Self::require_admin(&env)?;
        env.storage().instance().set(&DataKey::Paused, &true);
        Self::bump_instance_ttl(&env);
        Ok(())
    }

    pub fn unpause_contract(env: Env) -> Result<(), ScoutChainError> {
        Self::require_admin(&env)?;
        env.storage().instance().set(&DataKey::Paused, &false);
        Self::bump_instance_ttl(&env);
        Ok(())
    }

    // -------------------------------------------------------------------------
    // Player registration
    // -------------------------------------------------------------------------

    /// Register a new player profile at Level 0 (Unverified).
    /// `ipfs_hashes` — list of IPFS/Arweave CIDs for highlight reels and photos.
    pub fn register_player(
        env: Env,
        wallet: Address,
        vitals: PlayerVitals,
        ipfs_hashes: Vec<String>,
    ) -> Result<u64, ScoutChainError> {
        Self::require_not_paused(&env)?;
        Self::require_initialized(&env)?;
        wallet.require_auth();

        // Prevent duplicate registrations
        if env
            .storage()
            .persistent()
            .has(&DataKey::PlayerByWallet(wallet.clone()))
        {
            return Err(ScoutChainError::AlreadyRegistered);
        }

        // Validate vitals string lengths
        if vitals.position.len() > MAX_STRING_LEN
            || vitals.region.len() > MAX_STRING_LEN
            || vitals.nationality.len() > MAX_STRING_LEN
        {
            return Err(ScoutChainError::InvalidInput);
        }

        // Validate ipfs_hashes: non-empty and at most MAX_IPFS_HASHES
        if ipfs_hashes.is_empty() || ipfs_hashes.len() > MAX_IPFS_HASHES {
            return Err(ScoutChainError::InvalidInput);
        }

        let player_id = Self::next_player_id(&env);
        let now = env.ledger().timestamp();

        let profile = PlayerProfile {
            player_id,
            wallet: wallet.clone(),
            vitals,
            ipfs_hashes,
            level: ProgressLevel::Unverified,
            registered_at: now,
            updated_at: now,
        };

        env.storage()
            .persistent()
            .set(&DataKey::Player(player_id), &profile);
        env.storage()
            .persistent()
            .set(&DataKey::PlayerByWallet(wallet.clone()), &player_id);

        events::player_registered(&env, player_id, &wallet);
        Self::bump_instance_ttl(&env);
        Ok(player_id)
    }

    /// Update a player's IPFS content hashes (player auth required).
    pub fn update_profile(
        env: Env,
        player_id: u64,
        ipfs_hashes: Vec<String>,
    ) -> Result<(), ScoutChainError> {
        Self::require_not_paused(&env)?;
        let mut profile = Self::load_player(&env, player_id)?;
        profile.wallet.require_auth();
        if ipfs_hashes.is_empty() || ipfs_hashes.len() > MAX_IPFS_HASHES {
            return Err(ScoutChainError::InvalidInput);
        }
        profile.ipfs_hashes = ipfs_hashes;
        profile.updated_at = env.ledger().timestamp();
        env.storage()
            .persistent()
            .set(&DataKey::Player(player_id), &profile);
        events::profile_updated(&env, player_id);
        Self::bump_instance_ttl(&env);
        Ok(())
    }

    // -------------------------------------------------------------------------
    // Scout registration
    // -------------------------------------------------------------------------

    /// Register a new scout profile.
    pub fn register_scout(
        env: Env,
        wallet: Address,
        region: String,
    ) -> Result<u64, ScoutChainError> {
        Self::require_not_paused(&env)?;
        Self::require_initialized(&env)?;
        wallet.require_auth();

        if env
            .storage()
            .persistent()
            .has(&DataKey::ScoutByWallet(wallet.clone()))
        {
            return Err(ScoutChainError::AlreadyRegistered);
        }

        if region.len() > MAX_REGION_LEN {
            return Err(ScoutChainError::InvalidInput);
        }

        let scout_id = Self::next_scout_id(&env);
        let profile = ScoutProfile {
            scout_id,
            wallet: wallet.clone(),
            region,
            registered_at: env.ledger().timestamp(),
        };

        env.storage()
            .persistent()
            .set(&DataKey::Scout(scout_id), &profile);
        env.storage()
            .persistent()
            .set(&DataKey::ScoutByWallet(wallet.clone()), &scout_id);

        events::scout_registered(&env, scout_id, &wallet);
        Self::bump_instance_ttl(&env);
        Ok(scout_id)
    }

    /// Deregister a player from the system (player auth required).
    pub fn deregister_player(
        env: Env,
        player_id: u64,
    ) -> Result<(), ScoutChainError> {
        Self::require_not_paused(&env)?;
        
        // Load player to verify ownership and get wallet
        let profile = Self::load_player(&env, player_id)?;
        profile.wallet.require_auth();

        // Remove from persistent storage
        env.storage()
            .persistent()
            .remove(&DataKey::Player(player_id));
        env.storage()
            .persistent()
            .remove(&DataKey::PlayerByWallet(profile.wallet.clone()));

        // Decrement the player counter to reflect removal
        let current_count: u64 = env
            .storage()
            .instance()
            .get(&DataKey::PlayerCounter)
            .unwrap_or(0u64);
        if current_count > 0 {
            env.storage()
                .instance()
                .set(&DataKey::PlayerCounter, &(current_count - 1));
        }

        events::player_deregistered(&env, player_id, &profile.wallet);
        Self::bump_instance_ttl(&env);
        Ok(())
    }

    // -------------------------------------------------------------------------
    // Migration (for relayer-driven account recovery/bulk seeding)
    // -------------------------------------------------------------------------

    /// Public relayer-driven migration for players. Accepts pre-signed player data.
    /// Does NOT require admin auth; the signature is the authorization.
    pub fn redeem_migration_player(
        env: Env,
        wallet: Address,
        vitals: PlayerVitals,
        ipfs_hashes: Vec<String>,
    ) -> Result<u64, ScoutChainError> {
        Self::require_not_paused(&env)?;
        Self::require_initialized(&env)?;
        wallet.require_auth();

        // Validate inputs
        if vitals.position.len() > MAX_STRING_LEN
            || vitals.region.len() > MAX_STRING_LEN
            || vitals.nationality.len() > MAX_STRING_LEN
        {
            return Err(ScoutChainError::InvalidInput);
        }

        if ipfs_hashes.is_empty() || ipfs_hashes.len() > MAX_IPFS_HASHES {
            return Err(ScoutChainError::InvalidInput);
        }

        // Prevent duplicate registrations
        if env
            .storage()
            .persistent()
            .has(&DataKey::PlayerByWallet(wallet.clone()))
        {
            return Err(ScoutChainError::AlreadyRegistered);
        }

        // Use private helper to seed the player
        Self::_seed_player(&env, wallet, vitals, ipfs_hashes)
    }

    /// Public relayer-driven migration for scouts. Accepts pre-signed scout data.
    /// Does NOT require admin auth; the signature is the authorization.
    pub fn redeem_migration_scout(
        env: Env,
        wallet: Address,
        region: String,
    ) -> Result<u64, ScoutChainError> {
        Self::require_not_paused(&env)?;
        Self::require_initialized(&env)?;
        wallet.require_auth();

        if region.len() > MAX_REGION_LEN {
            return Err(ScoutChainError::InvalidInput);
        }

        if env
            .storage()
            .persistent()
            .has(&DataKey::ScoutByWallet(wallet.clone()))
        {
            return Err(ScoutChainError::AlreadyRegistered);
        }

        // Use private helper to seed the scout
        Self::_seed_scout(&env, wallet, region)
    }

    /// Private helper to seed a player (called by both redeem_migration_player and admin functions).
    /// No authorization check — the public caller is responsible for auth.
    fn _seed_player(
        env: &Env,
        wallet: Address,
        vitals: PlayerVitals,
        ipfs_hashes: Vec<String>,
    ) -> Result<u64, ScoutChainError> {
        let player_id = Self::next_player_id(&env);
        let now = env.ledger().timestamp();

        let profile = PlayerProfile {
            player_id,
            wallet: wallet.clone(),
            vitals,
            ipfs_hashes,
            level: ProgressLevel::Unverified,
            registered_at: now,
            updated_at: now,
        };

        env.storage()
            .persistent()
            .set(&DataKey::Player(player_id), &profile);
        env.storage()
            .persistent()
            .set(&DataKey::PlayerByWallet(wallet.clone()), &player_id);

        events::player_registered(&env, player_id, &wallet);
        Self::bump_instance_ttl(&env);
        Ok(player_id)
    }

    /// Private helper to seed a scout (called by both redeem_migration_scout and admin functions).
    /// No authorization check — the public caller is responsible for auth.
    fn _seed_scout(
        env: &Env,
        wallet: Address,
        region: String,
    ) -> Result<u64, ScoutChainError> {
        let scout_id = Self::next_scout_id(&env);
        let profile = ScoutProfile {
            scout_id,
            wallet: wallet.clone(),
            region,
            registered_at: env.ledger().timestamp(),
        };

        env.storage()
            .persistent()
            .set(&DataKey::Scout(scout_id), &profile);
        env.storage()
            .persistent()
            .set(&DataKey::ScoutByWallet(wallet.clone()), &scout_id);

        events::scout_registered(&env, scout_id, &wallet);
        Self::bump_instance_ttl(&env);
        Ok(scout_id)
    }

    // -------------------------------------------------------------------------
    // Queries
    // -------------------------------------------------------------------------

    pub fn get_player(env: Env, player_id: u64) -> Result<PlayerProfile, ScoutChainError> {
        Self::load_player(&env, player_id)
    }

    pub fn get_player_by_wallet(
        env: Env,
        wallet: Address,
    ) -> Result<PlayerProfile, ScoutChainError> {
        let player_id: u64 = env
            .storage()
            .persistent()
            .get(&DataKey::PlayerByWallet(wallet))
            .ok_or(ScoutChainError::PlayerNotFound)?;
        Self::load_player(&env, player_id)
    }

    pub fn get_scout(env: Env, scout_id: u64) -> Result<ScoutProfile, ScoutChainError> {
        env.storage()
            .persistent()
            .get(&DataKey::Scout(scout_id))
            .ok_or(ScoutChainError::ScoutNotFound)
    }

    pub fn health(env: Env) -> bool {
        env.storage()
            .instance()
            .get::<DataKey, bool>(&DataKey::Initialized)
            .unwrap_or(false)
    }

    pub fn get_player_count(env: Env) -> u64 {
        env.storage()
            .instance()
            .get::<DataKey, u64>(&DataKey::PlayerCounter)
            .unwrap_or(0u64)
    }

    pub fn get_scout_count(env: Env) -> u64 {
        env.storage()
            .instance()
            .get::<DataKey, u64>(&DataKey::ScoutCounter)
            .unwrap_or(0u64)
    }

    // -------------------------------------------------------------------------
    // Internal helpers
    // -------------------------------------------------------------------------

    fn require_initialized(env: &Env) -> Result<(), ScoutChainError> {
        if !env
            .storage()
            .instance()
            .get::<DataKey, bool>(&DataKey::Initialized)
            .unwrap_or(false)
        {
            return Err(ScoutChainError::NotInitialized);
        }
        Ok(())
    }

    fn require_not_paused(env: &Env) -> Result<(), ScoutChainError> {
        if env
            .storage()
            .instance()
            .get::<DataKey, bool>(&DataKey::Paused)
            .unwrap_or(false)
        {
            return Err(ScoutChainError::ContractPaused);
        }
        Ok(())
    }

    fn require_admin(env: &Env) -> Result<(), ScoutChainError> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(ScoutChainError::NotInitialized)?;
        admin.require_auth();
        Ok(())
    }

    fn load_player(env: &Env, player_id: u64) -> Result<PlayerProfile, ScoutChainError> {
        env.storage()
            .persistent()
            .get(&DataKey::Player(player_id))
            .ok_or(ScoutChainError::PlayerNotFound)
    }

    fn next_player_id(env: &Env) -> u64 {
        let id: u64 = env
            .storage()
            .instance()
            .get(&DataKey::PlayerCounter)
            .unwrap_or(0u64);
        let next = id.checked_add(1).expect("overflow");
        env.storage()
            .instance()
            .set(&DataKey::PlayerCounter, &next);
        next
    }

    fn next_scout_id(env: &Env) -> u64 {
        let id: u64 = env
            .storage()
            .instance()
            .get(&DataKey::ScoutCounter)
            .unwrap_or(0u64);
        let next = id.checked_add(1).expect("overflow");
        env.storage()
            .instance()
            .set(&DataKey::ScoutCounter, &next);
        next
    }

    fn bump_instance_ttl(env: &Env) {
        env.storage()
            .instance()
            .extend_ttl(INSTANCE_TTL_MIN, INSTANCE_TTL_MAX);
    }
}

// =============================================================================
// Tests
// =============================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{testutils::Address as _, vec, Env, String};

    fn setup() -> (Env, RegistrationContractClient<'static>) {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, RegistrationContract);
        let client = RegistrationContractClient::new(&env, &contract_id);
        (env, client)
    }

    fn dummy_vitals(env: &Env) -> PlayerVitals {
        PlayerVitals {
            age: 18,
            position: String::from_str(env, "Forward"),
            region: String::from_str(env, "West Africa"),
            nationality: String::from_str(env, "Ghana"),
        }
    }

    #[test]
    fn test_initialize_and_health() {
        let (env, client) = setup();
        let admin = Address::generate(&env);
        client.initialize(&admin);
        assert!(client.health());
    }

    #[test]
    fn test_register_player() {
        let (env, client) = setup();
        let admin = Address::generate(&env);
        client.initialize(&admin);

        let wallet = Address::generate(&env);
        let vitals = dummy_vitals(&env);
        let hashes: soroban_sdk::Vec<String> = vec![&env, String::from_str(&env, "QmTest123")];

        let player_id = client.register_player(&wallet, &vitals, &hashes);
        assert_eq!(player_id, 1);

        let profile = client.get_player(&player_id);
        assert_eq!(profile.wallet, wallet);
        assert_eq!(profile.level, ProgressLevel::Unverified);
    }

    #[test]
    #[should_panic]
    fn test_duplicate_registration_fails() {
        let (env, client) = setup();
        let admin = Address::generate(&env);
        client.initialize(&admin);

        let wallet = Address::generate(&env);
        let vitals = dummy_vitals(&env);
        let hashes: soroban_sdk::Vec<String> = vec![&env];

        client.register_player(&wallet, &vitals, &hashes);
        // second call should panic with AlreadyRegistered
        client.register_player(&wallet, &vitals, &hashes);
    }

    // -------------------------------------------------------------------------
    // Issue #6: position / region / nationality length validation
    // -------------------------------------------------------------------------

    #[test]
    #[should_panic]
    fn test_register_player_position_too_long() {
        let (env, client) = setup();
        let admin = Address::generate(&env);
        client.initialize(&admin);

        let wallet = Address::generate(&env);
        let long = String::from_str(&env, &"A".repeat(65));
        let vitals = PlayerVitals {
            age: 20,
            position: long,
            region: String::from_str(&env, "West Africa"),
            nationality: String::from_str(&env, "Ghana"),
        };
        let hashes = vec![&env, String::from_str(&env, "QmTest")];
        client.register_player(&wallet, &vitals, &hashes);
    }

    #[test]
    fn test_register_player_position_max_len_ok() {
        let (env, client) = setup();
        let admin = Address::generate(&env);
        client.initialize(&admin);

        let wallet = Address::generate(&env);
        let exactly_64 = String::from_str(&env, &"A".repeat(64));
        let vitals = PlayerVitals {
            age: 20,
            position: exactly_64,
            region: String::from_str(&env, "West Africa"),
            nationality: String::from_str(&env, "Ghana"),
        };
        let hashes = vec![&env, String::from_str(&env, "QmTest")];
        let id = client.register_player(&wallet, &vitals, &hashes);
        assert_eq!(id, 1);
    }

    #[test]
    #[should_panic]
    fn test_register_player_region_too_long() {
        let (env, client) = setup();
        let admin = Address::generate(&env);
        client.initialize(&admin);

        let wallet = Address::generate(&env);
        let long = String::from_str(&env, &"A".repeat(65));
        let vitals = PlayerVitals {
            age: 20,
            position: String::from_str(&env, "Forward"),
            region: long,
            nationality: String::from_str(&env, "Ghana"),
        };
        let hashes = vec![&env, String::from_str(&env, "QmTest")];
        client.register_player(&wallet, &vitals, &hashes);
    }

    #[test]
    #[should_panic]
    fn test_register_player_nationality_too_long() {
        let (env, client) = setup();
        let admin = Address::generate(&env);
        client.initialize(&admin);

        let wallet = Address::generate(&env);
        let long = String::from_str(&env, &"A".repeat(65));
        let vitals = PlayerVitals {
            age: 20,
            position: String::from_str(&env, "Forward"),
            region: String::from_str(&env, "West Africa"),
            nationality: long,
        };
        let hashes = vec![&env, String::from_str(&env, "QmTest")];
        client.register_player(&wallet, &vitals, &hashes);
    }

    // -------------------------------------------------------------------------
    // Issue #6 + #7: ipfs_hashes validation in register_player and update_profile
    // -------------------------------------------------------------------------

    #[test]
    #[should_panic]
    fn test_register_player_empty_hashes_fails() {
        let (env, client) = setup();
        let admin = Address::generate(&env);
        client.initialize(&admin);

        let wallet = Address::generate(&env);
        let vitals = dummy_vitals(&env);
        let hashes: soroban_sdk::Vec<String> = vec![&env];
        client.register_player(&wallet, &vitals, &hashes);
    }

    #[test]
    #[should_panic]
    fn test_register_player_too_many_hashes_fails() {
        let (env, client) = setup();
        let admin = Address::generate(&env);
        client.initialize(&admin);

        let wallet = Address::generate(&env);
        let vitals = dummy_vitals(&env);
        let h = String::from_str(&env, "QmHash");
        let hashes = vec![&env, h.clone(), h.clone(), h.clone(), h.clone(), h.clone(),
                          h.clone(), h.clone(), h.clone(), h.clone(), h.clone(), h.clone()];
        client.register_player(&wallet, &vitals, &hashes);
    }

    #[test]
    #[should_panic]
    fn test_update_profile_empty_hashes_fails() {
        let (env, client) = setup();
        let admin = Address::generate(&env);
        client.initialize(&admin);

        let wallet = Address::generate(&env);
        let vitals = dummy_vitals(&env);
        let hashes = vec![&env, String::from_str(&env, "QmTest")];
        let player_id = client.register_player(&wallet, &vitals, &hashes);

        let empty: soroban_sdk::Vec<String> = vec![&env];
        client.update_profile(&player_id, &empty);
    }

    #[test]
    #[should_panic]
    fn test_update_profile_too_many_hashes_fails() {
        let (env, client) = setup();
        let admin = Address::generate(&env);
        client.initialize(&admin);

        let wallet = Address::generate(&env);
        let vitals = dummy_vitals(&env);
        let hashes = vec![&env, String::from_str(&env, "QmTest")];
        let player_id = client.register_player(&wallet, &vitals, &hashes);

        let h = String::from_str(&env, "QmHash");
        let too_many = vec![&env, h.clone(), h.clone(), h.clone(), h.clone(), h.clone(),
                            h.clone(), h.clone(), h.clone(), h.clone(), h.clone(), h.clone()];
        client.update_profile(&player_id, &too_many);
    }

    #[test]
    fn test_update_profile_valid_hashes_persisted() {
        let (env, client) = setup();
        let admin = Address::generate(&env);
        client.initialize(&admin);

        let wallet = Address::generate(&env);
        let vitals = dummy_vitals(&env);
        let hashes = vec![&env, String::from_str(&env, "QmOld")];
        let player_id = client.register_player(&wallet, &vitals, &hashes);

        let new_hashes = vec![&env, String::from_str(&env, "QmNew1"), String::from_str(&env, "QmNew2")];
        client.update_profile(&player_id, &new_hashes);

        let profile = client.get_player(&player_id);
        assert_eq!(profile.ipfs_hashes.len(), 2);
    }

    // -------------------------------------------------------------------------
    // Issue #9: register_scout region length validation
    // -------------------------------------------------------------------------

    #[test]
    #[should_panic]
    fn test_register_scout_region_too_long() {
        let (env, client) = setup();
        let admin = Address::generate(&env);
        client.initialize(&admin);

        let wallet = Address::generate(&env);
        let long_region = String::from_str(&env, &"A".repeat(129));
        client.register_scout(&wallet, &long_region);
    }

    #[test]
    fn test_register_scout_region_max_len_ok() {
        let (env, client) = setup();
        let admin = Address::generate(&env);
        client.initialize(&admin);

        let wallet = Address::generate(&env);
        let exactly_128 = String::from_str(&env, &"A".repeat(128));
        let scout_id = client.register_scout(&wallet, &exactly_128);
        assert_eq!(scout_id, 1);
    }

    // -------------------------------------------------------------------------
    // Issue #1157: TTL Management
    // -------------------------------------------------------------------------

    #[test]
    fn test_instance_ttl_bumped_on_initialize() {
        let (env, client) = setup();
        let admin = Address::generate(&env);
        client.initialize(&admin);
        
        // Contract should be initialized and healthy (TTL was bumped)
        assert!(client.health());
    }

    #[test]
    fn test_instance_ttl_bumped_on_register_player() {
        let (env, client) = setup();
        let admin = Address::generate(&env);
        client.initialize(&admin);

        let wallet = Address::generate(&env);
        let vitals = dummy_vitals(&env);
        let hashes = vec![&env, String::from_str(&env, "QmTest")];
        
        // Register player should bump TTL
        let player_id = client.register_player(&wallet, &vitals, &hashes);
        assert_eq!(player_id, 1);
        assert!(client.health());
    }

    #[test]
    fn test_instance_ttl_bumped_on_register_scout() {
        let (env, client) = setup();
        let admin = Address::generate(&env);
        client.initialize(&admin);

        let wallet = Address::generate(&env);
        let region = String::from_str(&env, "West Africa");
        
        // Register scout should bump TTL
        let scout_id = client.register_scout(&wallet, &region);
        assert_eq!(scout_id, 1);
        assert!(client.health());
    }

    #[test]
    fn test_instance_ttl_bumped_on_pause() {
        let (env, client) = setup();
        let admin = Address::generate(&env);
        client.initialize(&admin);

        // Pause should bump TTL
        client.pause_contract();
        assert!(client.health());
    }

    #[test]
    fn test_instance_ttl_bumped_on_unpause() {
        let (env, client) = setup();
        let admin = Address::generate(&env);
        client.initialize(&admin);
        client.pause_contract();

        // Unpause should bump TTL
        client.unpause_contract();
        assert!(client.health());
    }

    // -------------------------------------------------------------------------
    // Issue #1153: Deregister Player
    // -------------------------------------------------------------------------

    #[test]
    fn test_deregister_player_removes_from_storage() {
        let (env, client) = setup();
        let admin = Address::generate(&env);
        client.initialize(&admin);

        let wallet = Address::generate(&env);
        let vitals = dummy_vitals(&env);
        let hashes = vec![&env, String::from_str(&env, "QmTest")];

        let player_id = client.register_player(&wallet, &vitals, &hashes);
        assert_eq!(player_id, 1);

        // Deregister should succeed
        client.deregister_player(&player_id);

        // Should not be able to get the player anymore
        let result = client.try_get_player(&player_id);
        assert!(result.is_err());
    }

    #[test]
    fn test_deregister_player_removes_wallet_index() {
        let (env, client) = setup();
        let admin = Address::generate(&env);
        client.initialize(&admin);

        let wallet = Address::generate(&env);
        let vitals = dummy_vitals(&env);
        let hashes = vec![&env, String::from_str(&env, "QmTest")];

        let player_id = client.register_player(&wallet, &vitals, &hashes);
        client.deregister_player(&player_id);

        // Should not be able to get player by wallet anymore
        let result = client.try_get_player_by_wallet(&wallet);
        assert!(result.is_err());
    }

    // -------------------------------------------------------------------------
    // Issue #1154: Player/Scout Count Getters
    // -------------------------------------------------------------------------

    #[test]
    fn test_get_player_count() {
        let (env, client) = setup();
        let admin = Address::generate(&env);
        client.initialize(&admin);

        assert_eq!(client.get_player_count(), 0);

        let wallet1 = Address::generate(&env);
        let vitals = dummy_vitals(&env);
        let hashes = vec![&env, String::from_str(&env, "QmTest")];
        client.register_player(&wallet1, &vitals, &hashes);
        assert_eq!(client.get_player_count(), 1);

        let wallet2 = Address::generate(&env);
        client.register_player(&wallet2, &vitals, &hashes);
        assert_eq!(client.get_player_count(), 2);
    }

    #[test]
    fn test_get_scout_count() {
        let (env, client) = setup();
        let admin = Address::generate(&env);
        client.initialize(&admin);

        assert_eq!(client.get_scout_count(), 0);

        let wallet1 = Address::generate(&env);
        let region = String::from_str(&env, "West Africa");
        client.register_scout(&wallet1, &region);
        assert_eq!(client.get_scout_count(), 1);

        let wallet2 = Address::generate(&env);
        client.register_scout(&wallet2, &region);
        assert_eq!(client.get_scout_count(), 2);
    }

    #[test]
    fn test_player_count_decrements_on_deregister() {
        let (env, client) = setup();
        let admin = Address::generate(&env);
        client.initialize(&admin);

        let wallet1 = Address::generate(&env);
        let vitals = dummy_vitals(&env);
        let hashes = vec![&env, String::from_str(&env, "QmTest")];

        let player_id1 = client.register_player(&wallet1, &vitals, &hashes);
        assert_eq!(client.get_player_count(), 1);

        let wallet2 = Address::generate(&env);
        let player_id2 = client.register_player(&wallet2, &vitals, &hashes);
        assert_eq!(client.get_player_count(), 2);

        // Deregister first player
        client.deregister_player(&player_id1);
        assert_eq!(client.get_player_count(), 1);

        // Deregister second player
        client.deregister_player(&player_id2);
        assert_eq!(client.get_player_count(), 0);
    }

    // -------------------------------------------------------------------------
    // Issue #1155: Migration Functions (Relayer Pattern)
    // -------------------------------------------------------------------------

    #[test]
    fn test_redeem_migration_player_succeeds_with_relayer() {
        let (env, client) = setup();
        let admin = Address::generate(&env);
        client.initialize(&admin);

        let wallet = Address::generate(&env);
        let vitals = dummy_vitals(&env);
        let hashes = vec![&env, String::from_str(&env, "QmTest")];

        // Relayer (wallet without admin role) can redeem a migration
        let player_id = client.redeem_migration_player(&wallet, &vitals, &hashes);
        assert_eq!(player_id, 1);

        let profile = client.get_player(&player_id);
        assert_eq!(profile.wallet, wallet);
    }

    #[test]
    fn test_redeem_migration_scout_succeeds_with_relayer() {
        let (env, client) = setup();
        let admin = Address::generate(&env);
        client.initialize(&admin);

        let wallet = Address::generate(&env);
        let region = String::from_str(&env, "West Africa");

        // Relayer (wallet without admin role) can redeem a migration
        let scout_id = client.redeem_migration_scout(&wallet, &region);
        assert_eq!(scout_id, 1);

        let profile = client.get_scout(&scout_id);
        assert_eq!(profile.wallet, wallet);
    }

    #[test]
    #[should_panic]
    fn test_redeem_migration_player_duplicate_fails() {
        let (env, client) = setup();
        let admin = Address::generate(&env);
        client.initialize(&admin);

        let wallet = Address::generate(&env);
        let vitals = dummy_vitals(&env);
        let hashes = vec![&env, String::from_str(&env, "QmTest")];

        client.redeem_migration_player(&wallet, &vitals, &hashes);
        // second call should panic with AlreadyRegistered
        client.redeem_migration_player(&wallet, &vitals, &hashes);
    }

    #[test]
    #[should_panic]
    fn test_redeem_migration_scout_duplicate_fails() {
        let (env, client) = setup();
        let admin = Address::generate(&env);
        client.initialize(&admin);

        let wallet = Address::generate(&env);
        let region = String::from_str(&env, "West Africa");

        client.redeem_migration_scout(&wallet, &region);
        // second call should panic with AlreadyRegistered
        client.redeem_migration_scout(&wallet, &region);
    }
}