#![no_std]

//! Oracle — On-chain price feed with staleness checks, circuit breakers, and deviation detection.
//!
//! The off-chain oracle service (suban-controller) pushes prices to this contract.
//! DeFi primitives (AMM, lending, escrow) read prices from this contract.

use soroban_sdk::{contract, contracterror, contractimpl, contracttype, symbol_short, Address, Env};
use hub_types::Price;
use hub_errors::HubError;

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Admin,
    Price(Address),
    LastPrice(Address),
    MaxAgeLedgers,
    CircuitBreakerThresholdBps,
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum OracleError {
    NotAdmin = 1,
    PriceNotFound = 2,
    PriceStale = 3,
    DeviationTooHigh = 4,
    AlreadyInitialized = 5,
    InvalidThreshold = 6,
}

#[contract]
pub struct Oracle;

fn read_admin(env: &Env) -> Address {
    env.storage()
        .instance()
        .get::<DataKey, Address>(&DataKey::Admin)
        .unwrap()
}

fn read_max_age(env: &Env) -> u64 {
    env.storage()
        .instance()
        .get::<DataKey, u64>(&DataKey::MaxAgeLedgers)
        .unwrap_or(10) // default: 10 ledgers (~1 minute)
}

fn read_breaker_threshold(env: &Env) -> u32 {
    env.storage()
        .instance()
        .get::<DataKey, u32>(&DataKey::CircuitBreakerThresholdBps)
        .unwrap_or(500) // default: 5% = 500 bps
}

#[contractimpl]
impl Oracle {
    /// Initialize the oracle with admin and configuration.
    pub fn initialize(
        env: Env,
        admin: Address,
        max_age_ledgers: u64,
        circuit_breaker_threshold_bps: u32,
    ) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("already initialized");
        }
        admin.require_auth();
        if circuit_breaker_threshold_bps == 0 || circuit_breaker_threshold_bps > 10000 {
            panic!("invalid threshold");
        }
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::MaxAgeLedgers, &max_age_ledgers);
        env.storage().instance().set(&DataKey::CircuitBreakerThresholdBps, &circuit_breaker_threshold_bps);
    }

    // ─── Admin: Push Price ────────────────────────────────────────────

    /// Push a new price for an asset. Called by the off-chain oracle service.
    /// Admin-only.
    pub fn set_price(
        env: Env,
        admin: Address,
        asset: Address,
        price: i128,
        decimals: u32,
        confidence: u32,
    ) -> Result<(), HubError> {
        if admin != read_admin(&env) {
            return Err(HubError::Unauthorized);
        }
        admin.require_auth();

        if price <= 0 {
            return Err(HubError::InvalidArgument);
        }

        let ledger = env.ledger().sequence() as u64;

        let price_data = Price {
            asset: asset.clone(),
            price,
            decimals,
            timestamp: ledger,
            confidence,
        };

        // Store current price
        env.storage().instance().set(&DataKey::Price(asset.clone()), &price_data);

        // Store as last known price (for circuit breaker)
        // Only update if there's no previous last price
        if !env.storage().instance().has(&DataKey::LastPrice(asset.clone())) {
            env.storage().instance().set(&DataKey::LastPrice(asset.clone()), &price_data);
        }

        // Emit event
        env.events().publish(
            (symbol_short!("price"), asset),
            (price, decimals, confidence, ledger),
        );

        Ok(())
    }

    /// Commit the current price as the "last known" price for circuit breaker.
    /// Called after a successful price update cycle. Admin-only.
    pub fn commit_price(env: Env, admin: Address, asset: Address) -> Result<(), HubError> {
        if admin != read_admin(&env) {
            return Err(HubError::Unauthorized);
        }
        admin.require_auth();

        let current = env.storage()
            .instance()
            .get::<DataKey, Price>(&DataKey::Price(asset.clone()))
            .ok_or(HubError::NotFound)?;

        env.storage().instance().set(&DataKey::LastPrice(asset.clone()), &current);
        Ok(())
    }

    // ─── Read: Get Price ─────────────────────────────────────────────

    /// Get the current price for an asset (no staleness check).
    pub fn get_price(env: Env, asset: Address) -> Result<Price, HubError> {
        env.storage()
            .instance()
            .get::<DataKey, Price>(&DataKey::Price(asset))
            .ok_or(HubError::NotFound)
    }

    /// Get price with staleness check.
    /// Returns error if price is older than max_age_ledgers.
    pub fn get_price_fresh(
        env: Env,
        asset: Address,
        max_age_ledgers: u64,
    ) -> Result<Price, HubError> {
        let price = env.storage()
            .instance()
            .get::<DataKey, Price>(&DataKey::Price(asset))
            .ok_or(HubError::NotFound)?;

        let current_ledger = env.ledger().sequence() as u64;
        let age = current_ledger.saturating_sub(price.timestamp);

        if age > max_age_ledgers {
            return Err(HubError::OracleStale);
        }

        Ok(price)
    }

    /// Check if the oracle is stale for a given asset.
    pub fn is_stale(env: Env, asset: Address, max_age_ledgers: u64) -> bool {
        let price = match env.storage()
            .instance()
            .get::<DataKey, Price>(&DataKey::Price(asset))
        {
            Some(p) => p,
            None => return true, // no price = stale
        };

        let current_ledger = env.ledger().sequence() as u64;
        let age = current_ledger.saturating_sub(price.timestamp);
        age > max_age_ledgers
    }

    /// Circuit breaker check.
    /// Returns true if price deviation from last known price exceeds threshold.
    pub fn circuit_breaker_check(
        env: Env,
        asset: Address,
        new_price: i128,
        max_deviation_bps: u32,
    ) -> bool {
        let last = match env.storage()
            .instance()
            .get::<DataKey, Price>(&DataKey::LastPrice(asset))
        {
            Some(p) => p,
            None => return false, // no previous price = no deviation
        };

        if last.price == 0 {
            return false;
        }

        let diff = (new_price - last.price).abs();
        let deviation_bps = (diff as u128 * 10000 / last.price as u128) as u32;
        deviation_bps > max_deviation_bps
    }

    /// Get the last known price for an asset (cached).
    pub fn get_last_price(env: Env, asset: Address) -> Option<Price> {
        env.storage()
            .instance()
            .get::<DataKey, Price>(&DataKey::LastPrice(asset))
    }

    // ─── Admin: Configuration ────────────────────────────────────────

    /// Update the max age for staleness checks.
    pub fn set_max_age(
        env: Env,
        admin: Address,
        max_age_ledgers: u64,
    ) -> Result<(), HubError> {
        if admin != read_admin(&env) {
            return Err(HubError::Unauthorized);
        }
        admin.require_auth();
        env.storage().instance().set(&DataKey::MaxAgeLedgers, &max_age_ledgers);
        Ok(())
    }

    /// Update the circuit breaker threshold.
    pub fn set_breaker_threshold(
        env: Env,
        admin: Address,
        threshold_bps: u32,
    ) -> Result<(), HubError> {
        if admin != read_admin(&env) {
            return Err(HubError::Unauthorized);
        }
        admin.require_auth();
        if threshold_bps == 0 || threshold_bps > 10000 {
            return Err(HubError::InvalidArgument);
        }
        env.storage().instance().set(&DataKey::CircuitBreakerThresholdBps, &threshold_bps);
        Ok(())
    }

    /// Transfer admin role.
    pub fn set_admin(
        env: Env,
        admin: Address,
        new_admin: Address,
    ) -> Result<(), HubError> {
        if admin != read_admin(&env) {
            return Err(HubError::Unauthorized);
        }
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &new_admin);
        Ok(())
    }

    /// Get admin address.
    pub fn admin(env: Env) -> Address {
        read_admin(&env)
    }

    /// Get current configuration.
    pub fn config(env: Env) -> (u64, u32) {
        (read_max_age(&env), read_breaker_threshold(&env))
    }
}

#[cfg(test)]
mod test {
    extern crate std;
    use super::*;
    use soroban_sdk::{testutils::{Address as _, Ledger}, Address, Env};

    fn setup() -> (Env, OracleClient<'static>, Address, Address) {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(Oracle, ());
        let client = OracleClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        let asset = Address::generate(&env);

        client.initialize(&admin, &10, &500); // 10 ledgers max age, 5% breaker

        (env, client, admin, asset)
    }

    #[test]
    fn test_initialize() {
        let (_env, client, admin, _asset) = setup();
        assert_eq!(client.admin(), admin);
        let (max_age, threshold) = client.config();
        assert_eq!(max_age, 10);
        assert_eq!(threshold, 500);
    }

    #[test]
    fn test_set_and_get_price() {
        let (_env, client, admin, asset) = setup();

        client.set_price(&admin, &asset, &315000000, &7, &95);

        let price = client.get_price(&asset);
        assert_eq!(price.price, 315000000);
        assert_eq!(price.decimals, 7);
        assert_eq!(price.confidence, 95);
    }

    #[test]
    fn test_get_price_fresh() {
        let (env, client, admin, asset) = setup();

        env.ledger().set_sequence_number(5);
        client.set_price(&admin, &asset, &315000000, &7, &95);

        let price = client.get_price_fresh(&asset, &10);
        assert_eq!(price.price, 315000000);

        // Advance ledger past max age
        env.ledger().set_sequence_number(20);
        let result = client.try_get_price_fresh(&asset, &10);
        assert_eq!(result, Err(Ok(HubError::OracleStale)));
    }

    #[test]
    fn test_is_stale() {
        let (env, client, admin, asset) = setup();

        assert!(client.is_stale(&asset, &10));

        env.ledger().set_sequence_number(5);
        client.set_price(&admin, &asset, &315000000, &7, &95);

        assert!(!client.is_stale(&asset, &10));

        // Advance past max age
        env.ledger().set_sequence_number(20);
        assert!(client.is_stale(&asset, &10));
    }

    #[test]
    fn test_circuit_breaker() {
        let (_env, client, admin, asset) = setup();

        client.set_price(&admin, &asset, &315000000, &7, &95);
        client.commit_price(&admin, &asset);

        assert!(client.circuit_breaker_check(&asset, &331000000, &500));

        assert!(!client.circuit_breaker_check(&asset, &325000000, &500));
    }

    #[test]
    fn test_unauthorized() {
        let (env, client, _admin, asset) = setup();
        let bad_actor = Address::generate(&env);

        let result = client.try_set_price(&bad_actor, &asset, &315000000, &7, &95);
        assert_eq!(result, Err(Ok(HubError::Unauthorized)));
    }

    #[test]
    fn test_update_config() {
        let (_env, client, admin, _asset) = setup();

        client.set_max_age(&admin, &20);
        assert_eq!(client.config().0, 20);

        client.set_breaker_threshold(&admin, &1000);
        assert_eq!(client.config().1, 1000);
    }

    #[test]
    fn test_commit_price() {
        let (_env, client, admin, asset) = setup();

        client.set_price(&admin, &asset, &315000000, &7, &95);
        client.commit_price(&admin, &asset);

        let last = client.get_last_price(&asset).unwrap();
        assert_eq!(last.price, 315000000);
    }
}
