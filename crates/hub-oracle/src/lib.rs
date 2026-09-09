#![no_std]

use soroban_sdk::{Address, Env};
use hub_types::Price;
use hub_errors::HubError;

/// Oracle interface trait.
/// All primitives call the same oracle through this trait —
/// no primitive maintains its own private price feed.
pub trait HubOracle {
    /// Get the current price for an asset.
    fn get_price(env: &Env, asset: &Address) -> Result<Price, HubError>;

    /// Get price with staleness check.
    /// Returns error if price is older than max_age_ledgers.
    fn get_price_fresh(env: &Env, asset: &Address, max_age_ledgers: u64) -> Result<Price, HubError>;

    /// Check if the oracle is stale for a given asset.
    fn is_stale(env: &Env, asset: &Address, max_age_ledgers: u64) -> bool;

    /// Circuit breaker check.
    /// Returns true if price deviation from last known price exceeds threshold.
    fn circuit_breaker_check(
        env: &Env,
        asset: &Address,
        new_price: i128,
        max_deviation_bps: u32,
    ) -> bool;

    /// Get the last known price for an asset (cached).
    fn get_last_price(env: &Env, asset: &Address) -> Option<Price>;
}
