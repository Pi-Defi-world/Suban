#![no_std]

//! Constant-Product Market Maker (x * y = k) with real token transfers.
//!
//! Each pool holds two SEP-41 tokens and tracks LP shares internally.
//! Formula: (x + dx)(y - dy) = xy => dy = y * dx / (x + dx) after fee

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, token, Address, Env,
};
use hub_errors::HubError;

// ─── Integer square root (Babylonian method) ─────────────────────────

fn isqrt(n: u128) -> u128 {
    if n == 0 {
        return 0;
    }
    let mut x = n;
    let mut y = (x + 1) / 2;
    while y < x {
        x = y;
        y = (x + n / x) / 2;
    }
    x
}

// ─── Storage Keys ────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Admin,
    TokenA,
    TokenB,
    ReserveA,
    ReserveB,
    TotalShares,
    FeeBps,
    LpBalance(Address),
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum PoolError {
    NotAdmin = 1,
    InsufficientLiquidity = 2,
    SlippageExceeded = 3,
    ZeroAmount = 4,
    SameToken = 5,
    InvalidFee = 6,
    InsufficientLpShares = 7,
}

// ─── Helpers ─────────────────────────────────────────────────────────

fn read_admin(env: &Env) -> Address {
    env.storage()
        .instance()
        .get::<DataKey, Address>(&DataKey::Admin)
        .unwrap()
}

fn read_reserve_a(env: &Env) -> i128 {
    env.storage()
        .instance()
        .get::<DataKey, i128>(&DataKey::ReserveA)
        .unwrap_or(0)
}

fn write_reserve_a(env: &Env, amount: i128) {
    env.storage().instance().set(&DataKey::ReserveA, &amount);
}

fn read_reserve_b(env: &Env) -> i128 {
    env.storage()
        .instance()
        .get::<DataKey, i128>(&DataKey::ReserveB)
        .unwrap_or(0)
}

fn write_reserve_b(env: &Env, amount: i128) {
    env.storage().instance().set(&DataKey::ReserveB, &amount);
}

fn read_total_shares(env: &Env) -> i128 {
    env.storage()
        .instance()
        .get::<DataKey, i128>(&DataKey::TotalShares)
        .unwrap_or(0)
}

fn write_total_shares(env: &Env, amount: i128) {
    env.storage().instance().set(&DataKey::TotalShares, &amount);
}

fn read_fee_bps(env: &Env) -> u32 {
    env.storage()
        .instance()
        .get::<DataKey, u32>(&DataKey::FeeBps)
        .unwrap_or(30)
}

fn read_lp_balance(env: &Env, addr: &Address) -> i128 {
    env.storage()
        .instance()
        .get::<DataKey, i128>(&DataKey::LpBalance(addr.clone()))
        .unwrap_or(0)
}

fn write_lp_balance(env: &Env, addr: &Address, amount: i128) {
    env.storage()
        .instance()
        .set(&DataKey::LpBalance(addr.clone()), &amount);
}

// ─── Contract ────────────────────────────────────────────────────────

#[contract]
pub struct CpmmPool;

#[contractimpl]
impl CpmmPool {
    pub fn initialize(
        env: Env,
        admin: Address,
        token_a: Address,
        token_b: Address,
        fee_bps: u32,
    ) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("already initialized");
        }
        admin.require_auth();
        if token_a == token_b {
            panic!("same token");
        }
        if fee_bps > 1000 {
            panic!("fee too high");
        }

        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::TokenA, &token_a);
        env.storage().instance().set(&DataKey::TokenB, &token_b);
        write_reserve_a(&env, 0);
        write_reserve_b(&env, 0);
        write_total_shares(&env, 0);
        env.storage().instance().set(&DataKey::FeeBps, &fee_bps);
    }

    /// Add liquidity. Transfers tokens from provider, mints LP shares.
    pub fn add_liquidity(
        env: Env,
        provider: Address,
        amount_a: i128,
        amount_b: i128,
        min_shares_out: i128,
    ) -> Result<i128, PoolError> {
        provider.require_auth();
        if amount_a <= 0 || amount_b <= 0 {
            return Err(PoolError::ZeroAmount);
        }

        let reserve_a = read_reserve_a(&env);
        let reserve_b = read_reserve_b(&env);
        let total_shares = read_total_shares(&env);

        let shares: i128;
        if total_shares == 0 {
            shares = isqrt((amount_a as u128) * (amount_b as u128)) as i128;
        } else {
            let shares_a = (amount_a as i128) * total_shares / reserve_a;
            let shares_b = (amount_b as i128) * total_shares / reserve_b;
            shares = shares_a.min(shares_b);
        }

        if shares < min_shares_out {
            return Err(PoolError::SlippageExceeded);
        }

        let token_a_addr: Address = env.storage().instance().get(&DataKey::TokenA).unwrap();
        let token_b_addr: Address = env.storage().instance().get(&DataKey::TokenB).unwrap();
        let vault = env.current_contract_address();

        token::Client::new(&env, &token_a_addr).transfer(&provider, &vault, &amount_a);
        token::Client::new(&env, &token_b_addr).transfer(&provider, &vault, &amount_b);

        write_lp_balance(&env, &provider, read_lp_balance(&env, &provider) + shares);

        write_reserve_a(&env, reserve_a + amount_a);
        write_reserve_b(&env, reserve_b + amount_b);
        write_total_shares(&env, total_shares + shares);

        env.events().publish(
            (symbol_short!("add_liq"), &provider),
            (amount_a, amount_b, shares),
        );

        Ok(shares)
    }

    /// Remove liquidity. Burns LP shares, transfers tokens back.
    pub fn remove_liquidity(
        env: Env,
        provider: Address,
        shares_in: i128,
        min_a_out: i128,
        min_b_out: i128,
    ) -> Result<(i128, i128), PoolError> {
        provider.require_auth();
        if shares_in <= 0 {
            return Err(PoolError::ZeroAmount);
        }

        let provider_shares = read_lp_balance(&env, &provider);
        if provider_shares < shares_in {
            return Err(PoolError::InsufficientLpShares);
        }

        let reserve_a = read_reserve_a(&env);
        let reserve_b = read_reserve_b(&env);
        let total_shares = read_total_shares(&env);

        if total_shares == 0 {
            return Err(PoolError::InsufficientLiquidity);
        }

        let amount_a = (shares_in as i128) * reserve_a / total_shares;
        let amount_b = (shares_in as i128) * reserve_b / total_shares;

        if amount_a < min_a_out || amount_b < min_b_out {
            return Err(PoolError::SlippageExceeded);
        }

        let token_a_addr: Address = env.storage().instance().get(&DataKey::TokenA).unwrap();
        let token_b_addr: Address = env.storage().instance().get(&DataKey::TokenB).unwrap();
        let vault = env.current_contract_address();

        write_lp_balance(&env, &provider, provider_shares - shares_in);

        write_reserve_a(&env, reserve_a - amount_a);
        write_reserve_b(&env, reserve_b - amount_b);
        write_total_shares(&env, total_shares - shares_in);

        // CEI: update state before sending the outbound tokens.
        token::Client::new(&env, &token_a_addr).transfer(&vault, &provider, &amount_a);
        token::Client::new(&env, &token_b_addr).transfer(&vault, &provider, &amount_b);

        env.events().publish(
            (symbol_short!("rm_liq"), &provider),
            (amount_a, amount_b, shares_in),
        );

        Ok((amount_a, amount_b))
    }

    /// Swap tokens. Returns amount out.
    pub fn swap(
        env: Env,
        trader: Address,
        token_in: Address,
        amount_in: i128,
        min_amount_out: i128,
    ) -> Result<i128, PoolError> {
        trader.require_auth();
        if amount_in <= 0 {
            return Err(PoolError::ZeroAmount);
        }

        let token_a_addr: Address = env.storage().instance().get(&DataKey::TokenA).unwrap();
        let token_b_addr: Address = env.storage().instance().get(&DataKey::TokenB).unwrap();

        let (reserve_in, reserve_out, token_out_addr, swap_to_a) = if token_in == token_a_addr {
            (read_reserve_a(&env), read_reserve_b(&env), token_b_addr, false)
        } else if token_in == token_b_addr {
            (read_reserve_b(&env), read_reserve_a(&env), token_a_addr, true)
        } else {
            return Err(PoolError::InvalidFee);
        };

        if reserve_in == 0 || reserve_out == 0 {
            return Err(PoolError::InsufficientLiquidity);
        }

        let fee_bps = read_fee_bps(&env);
        let amount_in_after_fee = (amount_in as i128) * ((10000 - fee_bps) as i128) / 10000;
        let amount_out =
            (reserve_out as i128) * amount_in_after_fee / ((reserve_in as i128) + amount_in_after_fee);

        if amount_out < min_amount_out {
            return Err(PoolError::SlippageExceeded);
        }

        let vault = env.current_contract_address();
        token::Client::new(&env, &token_in).transfer(&trader, &vault, &amount_in);

        if swap_to_a {
            write_reserve_a(&env, reserve_out - amount_out);
            write_reserve_b(&env, reserve_in + amount_in);
        } else {
            write_reserve_a(&env, reserve_in + amount_in);
            write_reserve_b(&env, reserve_out - amount_out);
        }

        // CEI: update reserves before sending the outbound tokens.
        token::Client::new(&env, &token_out_addr).transfer(&vault, &trader, &amount_out);

        env.events().publish(
            (symbol_short!("swap"), &trader),
            (amount_in, amount_out),
        );

        Ok(amount_out)
    }

    // ─── Queries ────────────────────────────────────────────────────

    pub fn get_reserves(env: Env) -> (i128, i128) {
        (read_reserve_a(&env), read_reserve_b(&env))
    }

    pub fn get_total_shares(env: Env) -> i128 {
        read_total_shares(&env)
    }

    pub fn get_fee_bps(env: Env) -> u32 {
        read_fee_bps(&env)
    }

    pub fn get_token_a(env: Env) -> Address {
        env.storage().instance().get(&DataKey::TokenA).unwrap()
    }

    pub fn get_token_b(env: Env) -> Address {
        env.storage().instance().get(&DataKey::TokenB).unwrap()
    }

    pub fn get_lp_balance(env: Env, owner: Address) -> i128 {
        read_lp_balance(&env, &owner)
    }

    pub fn get_quote(env: Env, token_in: Address, amount_in: i128) -> Result<i128, PoolError> {
        let token_a_addr: Address = env.storage().instance().get(&DataKey::TokenA).unwrap();
        let token_b_addr: Address = env.storage().instance().get(&DataKey::TokenB).unwrap();

        let (reserve_in, reserve_out) = if token_in == token_a_addr {
            (read_reserve_a(&env), read_reserve_b(&env))
        } else if token_in == token_b_addr {
            (read_reserve_b(&env), read_reserve_a(&env))
        } else {
            return Err(PoolError::InvalidFee);
        };

        if reserve_in == 0 || reserve_out == 0 {
            return Err(PoolError::InsufficientLiquidity);
        }

        let fee_bps = read_fee_bps(&env);
        let amount_in_after_fee = (amount_in as i128) * ((10000 - fee_bps) as i128) / 10000;
        let amount_out =
            (reserve_out as i128) * amount_in_after_fee / ((reserve_in as i128) + amount_in_after_fee);

        Ok(amount_out)
    }

    pub fn admin(env: Env) -> Address {
        read_admin(&env)
    }
}

#[cfg(test)]
mod test {
    extern crate std;
    use super::{CpmmPool, CpmmPoolClient, PoolError};
    use soroban_sdk::{
        contract, contractimpl, testutils::Address as _, Address, Env,
    };

    #[contract]
    struct MockToken;

    #[soroban_sdk::contracttype]
    enum MockKey {
        Bal(Address),
    }

    #[contractimpl]
    impl MockToken {
        pub fn initialize(_env: Env, _admin: Address) {}
        pub fn mint(env: Env, to: Address, amount: i128) {
            let b: i128 = env.storage().instance().get::<MockKey, i128>(&MockKey::Bal(to.clone())).unwrap_or(0);
            env.storage().instance().set(&MockKey::Bal(to), &(b + amount));
        }
        pub fn transfer(env: Env, from: Address, to: Address, amount: i128) {
            let fb: i128 = env.storage().instance().get::<MockKey, i128>(&MockKey::Bal(from.clone())).unwrap_or(0);
            let tb: i128 = env.storage().instance().get::<MockKey, i128>(&MockKey::Bal(to.clone())).unwrap_or(0);
            env.storage().instance().set(&MockKey::Bal(from), &(fb - amount));
            env.storage().instance().set(&MockKey::Bal(to), &(tb + amount));
        }
        pub fn balance(env: Env, account: Address) -> i128 {
            env.storage().instance().get::<MockKey, i128>(&MockKey::Bal(account)).unwrap_or(0)
        }
    }

    use super::token::Client as TokenClient;

    fn setup() -> (Env, CpmmPoolClient<'static>, Address, Address, Address) {
        let env = Env::default();
        env.mock_all_auths();

        let pool_id = env.register(CpmmPool, ());
        let pool = CpmmPoolClient::new(&env, &pool_id);

        let admin = Address::generate(&env);
        let token_a = env.register(MockToken, ());
        let token_b = env.register(MockToken, ());

        // Initialize mock tokens
        let tc_a = MockTokenClient::new(&env, &token_a);
        let tc_b = MockTokenClient::new(&env, &token_b);
        tc_a.initialize(&admin);
        tc_b.initialize(&admin);

        pool.initialize(&admin, &token_a, &token_b, &30);

        (env, pool, token_a, token_b, admin)
    }

    #[test]
    fn test_add_liquidity_initial() {
        let (env, pool, token_a, token_b, _) = setup();
        let provider = Address::generate(&env);

        MockTokenClient::new(&env, &token_a).mint(&provider, &100_000);
        MockTokenClient::new(&env, &token_b).mint(&provider, &100_000);

        let shares = pool.add_liquidity(&provider, &10_000, &10_000, &0);
        assert!(shares > 0);

        let (ra, rb) = pool.get_reserves();
        assert_eq!(ra, 10_000);
        assert_eq!(rb, 10_000);
        assert_eq!(pool.get_total_shares(), shares);
        assert_eq!(pool.get_lp_balance(&provider), shares);
    }

    #[test]
    fn test_swap() {
        let (env, pool, token_a, token_b, _) = setup();
        let provider = Address::generate(&env);
        let trader = Address::generate(&env);

        MockTokenClient::new(&env, &token_a).mint(&provider, &100_000);
        MockTokenClient::new(&env, &token_b).mint(&provider, &100_000);
        pool.add_liquidity(&provider, &10_000, &10_000, &0);

        MockTokenClient::new(&env, &token_a).mint(&trader, &10_000);
        let amount_out = pool.swap(&trader, &token_a, &1000, &0);

        // 0.3% fee: out = 10000 * 997 / (10000 + 997) = 906
        assert!(amount_out > 0);
        assert!(amount_out < 1000);
        assert!(amount_out > 900);

        let (ra, rb) = pool.get_reserves();
        assert_eq!(ra, 11_000);
        assert_eq!(rb, 10_000 - amount_out);
    }

    #[test]
    fn test_remove_liquidity() {
        let (env, pool, token_a, token_b, _) = setup();
        let provider = Address::generate(&env);

        MockTokenClient::new(&env, &token_a).mint(&provider, &100_000);
        MockTokenClient::new(&env, &token_b).mint(&provider, &100_000);
        let shares = pool.add_liquidity(&provider, &10_000, &10_000, &0);

        let (a_out, b_out) = pool.remove_liquidity(&provider, &shares, &0, &0);
        assert_eq!(a_out, 10_000);
        assert_eq!(b_out, 10_000);
        assert_eq!(pool.get_total_shares(), 0);
        assert_eq!(pool.get_lp_balance(&provider), 0);
    }

    #[test]
    fn test_slippage_protection() {
        let (env, pool, token_a, token_b, _) = setup();
        let provider = Address::generate(&env);

        MockTokenClient::new(&env, &token_a).mint(&provider, &100_000);
        MockTokenClient::new(&env, &token_b).mint(&provider, &100_000);
        pool.add_liquidity(&provider, &10_000, &10_000, &0);

        let trader = Address::generate(&env);
        MockTokenClient::new(&env, &token_a).mint(&trader, &10_000);

        let result = pool.try_swap(&trader, &token_a, &1000, &1000);
        assert_eq!(result, Err(Ok(PoolError::SlippageExceeded)));
    }

    #[test]
    fn test_zero_amount_fails() {
        let (env, pool, _, _, _) = setup();
        let provider = Address::generate(&env);
        let result = pool.try_add_liquidity(&provider, &0, &100, &0);
        assert_eq!(result, Err(Ok(PoolError::ZeroAmount)));
    }

    #[test]
    fn test_get_quote() {
        let (env, pool, token_a, token_b, _) = setup();
        let provider = Address::generate(&env);

        MockTokenClient::new(&env, &token_a).mint(&provider, &100_000);
        MockTokenClient::new(&env, &token_b).mint(&provider, &100_000);
        pool.add_liquidity(&provider, &10_000, &10_000, &0);

        let quote = pool.get_quote(&token_a, &1000);
        assert!(quote > 0);
        assert!(quote < 1000);
    }

    #[test]
    fn test_remove_insufficient_shares() {
        let (env, pool, token_a, token_b, _) = setup();
        let provider = Address::generate(&env);

        MockTokenClient::new(&env, &token_a).mint(&provider, &100_000);
        MockTokenClient::new(&env, &token_b).mint(&provider, &100_000);
        pool.add_liquidity(&provider, &10_000, &10_000, &0);

        let result = pool.try_remove_liquidity(&provider, &999_999_999, &0, &0);
        assert_eq!(result, Err(Ok(PoolError::InsufficientLpShares)));
    }
}
