#![no_std]

//! Swap Router — pool registry and multi-hop path finder.
//!
//! Records pools and their token pairs. Finds optimal routing paths.
//! Actual swap execution happens client-side via the pool contracts.

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, Env, Vec,
};

// ─── Types ───────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PoolRoute {
    pub pool_address: Address,
    pub token_a: Address,
    pub token_b: Address,
    pub fee_bps: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SwapStep {
    pub pool_address: Address,
    pub token_in: Address,
    pub token_out: Address,
}

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Admin,
    PoolCount,
    Pool(u32),
    // token_address → Vec<u32> of pool IDs that contain this token
    TokenPools(Address),
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum RouterError {
    NotAdmin = 1,
    AlreadyInitialized = 2,
    PoolNotFound = 3,
    NoPathFound = 4,
    InvalidToken = 5,
    InvalidFee = 6,
}

#[contract]
pub struct SwapRouter;

fn read_admin(env: &Env) -> Address {
    env.storage().instance().get::<DataKey, Address>(&DataKey::Admin).unwrap()
}

#[contractimpl]
impl SwapRouter {
    pub fn initialize(env: Env, admin: Address) {
        if env.storage().instance().has(&DataKey::Admin) { panic!("already initialized"); }
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::PoolCount, &0u32);
    }

    /// Register a pool.
    pub fn register_pool(
        env: Env,
        admin: Address,
        pool_address: Address,
        token_a: Address,
        token_b: Address,
        fee_bps: u32,
    ) -> Result<u32, RouterError> {
        if admin != read_admin(&env) { return Err(RouterError::NotAdmin); }
        admin.require_auth();
        if fee_bps > 1000 { return Err(RouterError::InvalidFee); }

        let count: u32 = env.storage().instance()
            .get::<DataKey, u32>(&DataKey::PoolCount).unwrap_or(0);

        let route = PoolRoute {
            pool_address,
            token_a: token_a.clone(),
            token_b: token_b.clone(),
            fee_bps,
        };

        env.storage().instance().set(&DataKey::Pool(count), &route);
        env.storage().instance().set(&DataKey::PoolCount, &(count + 1));

        env.events().publish(
            (symbol_short!("rtr_add"), count),
            (&token_a, &token_b),
        );

        Ok(count)
    }

    /// Find a pool that swaps token_in → token_out.
    pub fn find_pool(
        env: Env,
        token_in: Address,
        token_out: Address,
    ) -> Result<PoolRoute, RouterError> {
        let count: u32 = env.storage().instance()
            .get::<DataKey, u32>(&DataKey::PoolCount).unwrap_or(0);

        for i in 0..count {
            let route: PoolRoute = env.storage().instance()
                .get(&DataKey::Pool(i))
                .ok_or(RouterError::PoolNotFound)?;

            let matches = (route.token_a == token_in && route.token_b == token_out)
                || (route.token_b == token_in && route.token_a == token_out);

            if matches { return Ok(route); }
        }

        Err(RouterError::NoPathFound)
    }

    /// Find a multi-hop path: token_in → intermediate → token_out.
    /// Returns (step1, step2) or error if no path exists.
    pub fn find_path(
        env: Env,
        token_in: Address,
        token_out: Address,
    ) -> Result<(SwapStep, Option<SwapStep>), RouterError> {
        // Try direct first
        if let Ok(route) = Self::find_pool(env.clone(), token_in.clone(), token_out.clone()) {
            let step = SwapStep {
                pool_address: route.pool_address,
                token_in: token_in.clone(),
                token_out: token_out.clone(),
            };
            return Ok((step, None));
        }

        // Try one-hop through intermediate tokens
        let count: u32 = env.storage().instance()
            .get::<DataKey, u32>(&DataKey::PoolCount).unwrap_or(0);

        for i in 0..count {
            let route: PoolRoute = env.storage().instance()
                .get(&DataKey::Pool(i))
                .ok_or(RouterError::PoolNotFound)?;

            let intermediate = if route.token_a == token_in {
                Some(route.token_b.clone())
            } else if route.token_b == token_in {
                Some(route.token_a.clone())
            } else {
                None
            };

            if let Some(inter) = intermediate {
                if let Ok(route2) = Self::find_pool(env.clone(), inter.clone(), token_out.clone()) {
                    let step1 = SwapStep {
                        pool_address: route.pool_address,
                        token_in: token_in.clone(),
                        token_out: inter.clone(),
                    };
                    let step2 = SwapStep {
                        pool_address: route2.pool_address,
                        token_in: inter,
                        token_out: token_out.clone(),
                    };
                    return Ok((step1, Some(step2)));
                }
            }
        }

        Err(RouterError::NoPathFound)
    }

    /// Get all registered pools.
    pub fn list_pools(env: Env) -> Vec<PoolRoute> {
        let count: u32 = env.storage().instance()
            .get::<DataKey, u32>(&DataKey::PoolCount).unwrap_or(0);
        let mut pools = Vec::new(&env);
        for i in 0..count {
            if let Some(route) = env.storage().instance().get::<DataKey, PoolRoute>(&DataKey::Pool(i)) {
                pools.push_back(route);
            }
        }
        pools
    }

    pub fn get_pool_count(env: Env) -> u32 {
        env.storage().instance().get::<DataKey, u32>(&DataKey::PoolCount).unwrap_or(0)
    }

    pub fn admin(env: Env) -> Address {
        read_admin(&env)
    }
}

#[cfg(test)]
mod test {
    extern crate std;
    use super::{RouterError, SwapRouter, SwapRouterClient};
    use soroban_sdk::{testutils::Address as _, Address, Env};

    fn setup() -> (Env, SwapRouterClient<'static>, Address) {
        let env = Env::default();
        env.mock_all_auths();
        let id = env.register(SwapRouter, ());
        let client = SwapRouterClient::new(&env, &id);
        let admin = Address::generate(&env);
        client.initialize(&admin);
        (env, client, admin)
    }

    #[test]
    fn test_register_pool() {
        let (env, client, admin) = setup();
        let pool = Address::generate(&env);
        let token_a = Address::generate(&env);
        let token_b = Address::generate(&env);

        let pool_id = client.register_pool(&admin, &pool, &token_a, &token_b, &30);
        assert_eq!(pool_id, 0);
        assert_eq!(client.get_pool_count(), 1);
    }

    #[test]
    fn test_find_pool() {
        let (env, client, admin) = setup();
        let pool = Address::generate(&env);
        let token_a = Address::generate(&env);
        let token_b = Address::generate(&env);

        client.register_pool(&admin, &pool, &token_a, &token_b, &30);

        let found = client.try_find_pool(&token_a, &token_b);
        assert_eq!(found, Ok(Ok(super::PoolRoute {
            pool_address: pool.clone(),
            token_a: token_a.clone(),
            token_b: token_b.clone(),
            fee_bps: 30,
        })));

        // Reverse direction also works
        let found2 = client.try_find_pool(&token_b, &token_a);
        assert!(found2.is_ok());
    }

    #[test]
    fn test_find_path_direct() {
        let (env, client, admin) = setup();
        let pool = Address::generate(&env);
        let token_a = Address::generate(&env);
        let token_b = Address::generate(&env);

        client.register_pool(&admin, &pool, &token_a, &token_b, &30);

        let (step1, step2) = client.find_path(&token_a, &token_b);
        assert_eq!(step1.pool_address, pool);
        assert!(step2.is_none());
    }

    #[test]
    fn test_find_path_multi_hop() {
        let (env, client, admin) = setup();
        let pool1 = Address::generate(&env);
        let pool2 = Address::generate(&env);
        let token_a = Address::generate(&env);
        let token_b = Address::generate(&env);
        let token_c = Address::generate(&env);

        // A-B pool and B-C pool, find path A→C
        client.register_pool(&admin, &pool1, &token_a, &token_b, &30);
        client.register_pool(&admin, &pool2, &token_b, &token_c, &30);

        let (step1, step2) = client.find_path(&token_a, &token_c);
        assert_eq!(step1.pool_address, pool1);
        assert!(step2.is_some());
        assert_eq!(step2.unwrap().pool_address, pool2);
    }

    #[test]
    fn test_no_path_found() {
        let (env, client, _) = setup();
        let token_a = Address::generate(&env);
        let token_b = Address::generate(&env);

        let result = client.try_find_path(&token_a, &token_b);
        assert_eq!(result, Err(Ok(RouterError::NoPathFound)));
    }

    #[test]
    fn test_non_admin_fails() {
        let (env, client, _) = setup();
        let rando = Address::generate(&env);
        let pool = Address::generate(&env);
        let token_a = Address::generate(&env);
        let token_b = Address::generate(&env);

        let result = client.try_register_pool(&rando, &pool, &token_a, &token_b, &30);
        assert_eq!(result, Err(Ok(RouterError::NotAdmin)));
    }

    #[test]
    fn test_invalid_fee_fails() {
        let (env, client, admin) = setup();
        let pool = Address::generate(&env);
        let token_a = Address::generate(&env);
        let token_b = Address::generate(&env);

        let result = client.try_register_pool(&admin, &pool, &token_a, &token_b, &5000);
        assert_eq!(result, Err(Ok(RouterError::InvalidFee)));
    }

    #[test]
    fn test_list_pools() {
        let (env, client, admin) = setup();
        let token_a = Address::generate(&env);
        let token_b = Address::generate(&env);
        let token_c = Address::generate(&env);

        client.register_pool(&admin, &Address::generate(&env), &token_a, &token_b, &30);
        client.register_pool(&admin, &Address::generate(&env), &token_b, &token_c, &10);

        let pools = client.list_pools();
        assert_eq!(pools.len(), 2);
    }
}
