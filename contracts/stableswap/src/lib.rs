#![no_std]

//! Stableswap Pool — Curve-style invariant for low-slippage stable pairs.
//!
//! Uses the StableSwap invariant: A·n^n·Σx_i + D = A·D·n^n + D^(n+1)/(n^n·Πx_i)
//! For n=2 tokens: A·4·(x+y) + D = 4·A·D + D³/(4·x·y)
//!
//! Newton method per Curve's Vyper implementation for safe convergence.

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, token, Address, Env,
};

const N: u128 = 2;
const MAX_ITERATIONS: u32 = 256;
/// Hard cap on the StableSwap amplification coefficient to bound price-manipulation risk.
const MAX_AMPLIFICATION: u128 = 1_000_000;

/// Compute D using Curve's standard Newton iteration.
/// D_P = D^(n+1) / (n^n * prod(x_i))
/// D_next = (Ann * S + n * D_P) * D / ((Ann - 1) * D + (n+1) * D_P)
fn compute_d(reserve_a: u128, reserve_b: u128, amp: u128) -> u128 {
    let s = reserve_a + reserve_b;
    if s == 0 { return 0; }

    let ann = amp * N * N;
    let mut d = s;

    for _ in 0..MAX_ITERATIONS {
        let mut d_p = d;
        d_p = d_p * d / (reserve_a * N);
        d_p = d_p * d / (reserve_b * N);

        let d_prev = d;
        d = (ann * s + d_p * N) * d / ((ann - 1) * d + (N + 1) * d_p);

        if d > d_prev {
            if d - d_prev <= 1 { return d; }
        } else {
            if d_prev - d <= 1 { return d; }
        }
    }

    d
}

/// Compute y given x using Curve's standard Newton iteration.
/// Given old reserves (reserve_in, reserve_out) and amount_in,
/// find new_reserve_out such that D(reserve_in + amount_in, new_reserve_out) = D(reserve_in, reserve_out).
///
/// Curve formula for n=2:
/// D = compute_d(reserve_in, reserve_out) [old state = invariant]
/// c = D^3 / (4 * (reserve_in + amount_in) * Ann)
/// b = (reserve_in + amount_in) + D / Ann
/// Newton: y = (y^2 + c) / (2y + b - D)
fn compute_y(reserve_in: u128, amount_in: u128, reserve_out: u128, amp: u128) -> u128 {
    let ann = amp * N * N;
    let d = compute_d(reserve_in, reserve_out, amp);
    if d == 0 { return 0; }

    let new_reserve_in = reserve_in + amount_in;

    // c = D^(n+1) / (n^n * prod_of_other_reserves * Ann)
    // For n=2: c = D^3 / (4 * new_reserve_in * Ann)
    // Computed as: c = D; c = c * D / (new_reserve_in * 2); c = c * D / (Ann * 2)
    let mut c = d;
    c = c * d / (new_reserve_in * N);
    c = c * d / (ann * N);

    // b = sum_of_other_reserves + D / Ann
    let b = new_reserve_in + d / ann;

    // Newton iteration: y_new = (y^2 + c) / (2y + b - D)
    let mut y = reserve_out;

    for _ in 0..MAX_ITERATIONS {
        let y_prev = y;
        y = (y * y + c) / (2 * y + b - d);

        if y > y_prev {
            if y - y_prev <= 1 { return y; }
        } else {
            if y_prev - y <= 1 { return y; }
        }
    }

    y
}

// ─── Storage ─────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Admin,
    TokenA,
    TokenB,
    ReserveA,
    ReserveB,
    TotalShares,
    Amplification,
    FeeBps,
    LpBalance(Address),
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum StableswapError {
    NotAdmin = 1,
    ZeroAmount = 2,
    SlippageExceeded = 3,
    InsufficientLiquidity = 4,
    InsufficientLpShares = 5,
    SameToken = 6,
    InvalidFee = 7,
}

// ─── Helpers ─────────────────────────────────────────────────────────

fn read_admin(env: &Env) -> Address {
    env.storage().instance().get::<DataKey, Address>(&DataKey::Admin).unwrap()
}

fn read_reserve(env: &Env, key: &DataKey) -> i128 {
    env.storage().instance().get::<DataKey, i128>(key).unwrap_or(0)
}

fn write_reserve(env: &Env, key: &DataKey, val: i128) {
    env.storage().instance().set(key, &val);
}

fn read_lp_balance(env: &Env, addr: &Address) -> i128 {
    env.storage().instance().get::<DataKey, i128>(&DataKey::LpBalance(addr.clone())).unwrap_or(0)
}

fn write_lp_balance(env: &Env, addr: &Address, val: i128) {
    env.storage().instance().set(&DataKey::LpBalance(addr.clone()), &val);
}

fn read_total_shares(env: &Env) -> i128 {
    env.storage().instance().get::<DataKey, i128>(&DataKey::TotalShares).unwrap_or(0)
}

fn write_total_shares(env: &Env, val: i128) {
    env.storage().instance().set(&DataKey::TotalShares, &val)
}

fn read_fee_bps(env: &Env) -> u32 {
    env.storage().instance().get::<DataKey, u32>(&DataKey::FeeBps).unwrap_or(30)
}

// ─── Contract ────────────────────────────────────────────────────────

#[contract]
pub struct Stableswap;

#[contractimpl]
impl Stableswap {
    pub fn initialize(
        env: Env,
        admin: Address,
        token_a: Address,
        token_b: Address,
        amplification: u128,
    ) {
        if env.storage().instance().has(&DataKey::Admin) { panic!("already initialized"); }
        admin.require_auth();
        if token_a == token_b { panic!("same token"); }
        if amplification == 0 { panic!("amp must be > 0"); }
        if amplification > MAX_AMPLIFICATION { panic!("amp too high"); }

        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::TokenA, &token_a);
        env.storage().instance().set(&DataKey::TokenB, &token_b);
        write_reserve(&env, &DataKey::ReserveA, 0);
        write_reserve(&env, &DataKey::ReserveB, 0);
        write_total_shares(&env, 0);
        env.storage().instance().set(&DataKey::Amplification, &amplification);
        env.storage().instance().set(&DataKey::FeeBps, &30u32);
    }

    /// Set the swap fee (basis points). Admin only.
    pub fn set_fee(env: Env, admin: Address, fee_bps: u32) -> Result<(), StableswapError> {
        if admin != read_admin(&env) { return Err(StableswapError::NotAdmin); }
        admin.require_auth();
        if fee_bps > 1000 { return Err(StableswapError::InvalidFee); }
        env.storage().instance().set(&DataKey::FeeBps, &fee_bps);
        Ok(())
    }

    pub fn add_liquidity(
        env: Env,
        provider: Address,
        amount_a: i128,
        amount_b: i128,
        min_shares_out: i128,
    ) -> Result<i128, StableswapError> {
        provider.require_auth();
        if amount_a <= 0 || amount_b <= 0 { return Err(StableswapError::ZeroAmount); }

        let reserve_a = read_reserve(&env, &DataKey::ReserveA);
        let reserve_b = read_reserve(&env, &DataKey::ReserveB);
        let total_shares = read_total_shares(&env);
        let amp = env.storage().instance().get::<DataKey, u128>(&DataKey::Amplification).unwrap();

        let shares: i128;
        if total_shares == 0 {
            let d = compute_d(amount_a as u128, amount_b as u128, amp);
            shares = d as i128;
        } else {
            let d0 = compute_d(reserve_a as u128, reserve_b as u128, amp);
            let d1 = compute_d(
                (reserve_a + amount_a) as u128,
                (reserve_b + amount_b) as u128,
                amp,
            );
            shares = ((d1 - d0) * total_shares as u128 / d0) as i128;
        }

        if shares < min_shares_out { return Err(StableswapError::SlippageExceeded); }

        let token_a_addr: Address = env.storage().instance().get(&DataKey::TokenA).unwrap();
        let token_b_addr: Address = env.storage().instance().get(&DataKey::TokenB).unwrap();
        let vault = env.current_contract_address();

        token::Client::new(&env, &token_a_addr).transfer(&provider, &vault, &amount_a);
        token::Client::new(&env, &token_b_addr).transfer(&provider, &vault, &amount_b);

        write_lp_balance(&env, &provider, read_lp_balance(&env, &provider) + shares);
        write_reserve(&env, &DataKey::ReserveA, reserve_a + amount_a);
        write_reserve(&env, &DataKey::ReserveB, reserve_b + amount_b);
        write_total_shares(&env, total_shares + shares);

        env.events().publish(
            (symbol_short!("ss_add"), &provider),
            (amount_a, amount_b, shares),
        );

        Ok(shares)
    }

    pub fn remove_liquidity(
        env: Env,
        provider: Address,
        shares_in: i128,
        min_a_out: i128,
        min_b_out: i128,
    ) -> Result<(i128, i128), StableswapError> {
        provider.require_auth();
        if shares_in <= 0 { return Err(StableswapError::ZeroAmount); }

        let provider_shares = read_lp_balance(&env, &provider);
        if provider_shares < shares_in { return Err(StableswapError::InsufficientLpShares); }

        let reserve_a = read_reserve(&env, &DataKey::ReserveA);
        let reserve_b = read_reserve(&env, &DataKey::ReserveB);
        let total_shares = read_total_shares(&env);

        if total_shares == 0 { return Err(StableswapError::InsufficientLiquidity); }

        let amount_a = (shares_in as u128) * (reserve_a as u128) / (total_shares as u128);
        let amount_b = (shares_in as u128) * (reserve_b as u128) / (total_shares as u128);

        if (amount_a as i128) < min_a_out || (amount_b as i128) < min_b_out {
            return Err(StableswapError::SlippageExceeded);
        }

        let token_a_addr: Address = env.storage().instance().get(&DataKey::TokenA).unwrap();
        let token_b_addr: Address = env.storage().instance().get(&DataKey::TokenB).unwrap();
        let vault = env.current_contract_address();

        write_lp_balance(&env, &provider, provider_shares - shares_in);
        write_reserve(&env, &DataKey::ReserveA, reserve_a - (amount_a as i128));
        write_reserve(&env, &DataKey::ReserveB, reserve_b - (amount_b as i128));
        write_total_shares(&env, total_shares - shares_in);

        // CEI: update state before sending the outbound tokens.
        token::Client::new(&env, &token_a_addr).transfer(&vault, &provider, &(amount_a as i128));
        token::Client::new(&env, &token_b_addr).transfer(&vault, &provider, &(amount_b as i128));

        env.events().publish(
            (symbol_short!("ss_rm"), &provider),
            (amount_a as i128, amount_b as i128, shares_in),
        );

        Ok((amount_a as i128, amount_b as i128))
    }

    pub fn swap(
        env: Env,
        trader: Address,
        token_in: Address,
        amount_in: i128,
        min_amount_out: i128,
    ) -> Result<i128, StableswapError> {
        trader.require_auth();
        if amount_in <= 0 { return Err(StableswapError::ZeroAmount); }

        let token_a_addr: Address = env.storage().instance().get(&DataKey::TokenA).unwrap();
        let token_b_addr: Address = env.storage().instance().get(&DataKey::TokenB).unwrap();

        let (reserve_in_key, reserve_out_key, token_out_addr) =
            if token_in == token_a_addr {
                (DataKey::ReserveA, DataKey::ReserveB, token_b_addr)
            } else if token_in == token_b_addr {
                (DataKey::ReserveB, DataKey::ReserveA, token_a_addr)
            } else {
                return Err(StableswapError::InvalidFee);
            };

        let reserve_in = read_reserve(&env, &reserve_in_key);
        let reserve_out = read_reserve(&env, &reserve_out_key);
        if reserve_in == 0 || reserve_out == 0 {
            return Err(StableswapError::InsufficientLiquidity);
        }

        let amp = env.storage().instance().get::<DataKey, u128>(&DataKey::Amplification).unwrap();

        // Compute new output reserve using Curve's formula
        let new_reserve_out = compute_y(
            reserve_in as u128,
            amount_in as u128,
            reserve_out as u128,
            amp,
        );
        let amount_out = (reserve_out as u128) - new_reserve_out;
        let fee_bps = read_fee_bps(&env);
        let amount_out_net = (amount_out as i128) * (10000 - fee_bps as i128) / 10000;

        if amount_out_net < min_amount_out {
            return Err(StableswapError::SlippageExceeded);
        }

        let vault = env.current_contract_address();
        token::Client::new(&env, &token_in).transfer(&trader, &vault, &amount_in);

        write_reserve(&env, &reserve_in_key, reserve_in + amount_in);
        write_reserve(&env, &reserve_out_key, reserve_out - amount_out_net);

        // CEI: update reserves before sending the outbound tokens.
        token::Client::new(&env, &token_out_addr).transfer(&vault, &trader, &amount_out_net);

        env.events().publish(
            (symbol_short!("ss_swap"), &trader),
            (amount_in, amount_out_net),
        );

        Ok(amount_out_net)
    }

    // ─── Queries ────────────────────────────────────────────────────

    pub fn get_reserves(env: Env) -> (i128, i128) {
        (read_reserve(&env, &DataKey::ReserveA), read_reserve(&env, &DataKey::ReserveB))
    }

    pub fn get_total_shares(env: Env) -> i128 {
        read_total_shares(&env)
    }

    pub fn get_amplification(env: Env) -> u128 {
        env.storage().instance().get::<DataKey, u128>(&DataKey::Amplification).unwrap()
    }

    pub fn get_fee_bps(env: Env) -> u32 {
        read_fee_bps(&env)
    }

    pub fn get_lp_balance(env: Env, owner: Address) -> i128 {
        read_lp_balance(&env, &owner)
    }

    pub fn get_token_a(env: Env) -> Address {
        env.storage().instance().get(&DataKey::TokenA).unwrap()
    }

    pub fn get_token_b(env: Env) -> Address {
        env.storage().instance().get(&DataKey::TokenB).unwrap()
    }

    pub fn get_quote(env: Env, token_in: Address, amount_in: i128) -> Result<i128, StableswapError> {
        let token_a_addr: Address = env.storage().instance().get(&DataKey::TokenA).unwrap();
        let token_b_addr: Address = env.storage().instance().get(&DataKey::TokenB).unwrap();

        let (ri, ro) = if token_in == token_a_addr {
            (read_reserve(&env, &DataKey::ReserveA), read_reserve(&env, &DataKey::ReserveB))
        } else if token_in == token_b_addr {
            (read_reserve(&env, &DataKey::ReserveB), read_reserve(&env, &DataKey::ReserveA))
        } else {
            return Err(StableswapError::InvalidFee);
        };

        if ri == 0 || ro == 0 { return Err(StableswapError::InsufficientLiquidity); }

        let amp = env.storage().instance().get::<DataKey, u128>(&DataKey::Amplification).unwrap();
        let new_ro = compute_y(ri as u128, amount_in as u128, ro as u128, amp);
        let gross = ((ro as u128) - new_ro) as i128;
        let fee_bps = read_fee_bps(&env);
        Ok(gross * (10000 - fee_bps as i128) / 10000)
    }

    pub fn admin(env: Env) -> Address {
        read_admin(&env)
    }
}

#[cfg(test)]
mod test {
    extern crate std;
    use super::{Stableswap, StableswapClient, StableswapError};
    use soroban_sdk::{
        contract, contractimpl, testutils::Address as _, Address, Env,
    };

    #[contract]
    struct MockToken;

    #[soroban_sdk::contracttype]
    enum MockKey { Bal(Address) }

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

    fn setup() -> (Env, StableswapClient<'static>, Address, Address, Address) {
        let env = Env::default();
        env.mock_all_auths();
        let pool_id = env.register(Stableswap, ());
        let pool = StableswapClient::new(&env, &pool_id);
        let admin = Address::generate(&env);
        let token_a = env.register(MockToken, ());
        let token_b = env.register(MockToken, ());
        MockTokenClient::new(&env, &token_a).initialize(&admin);
        MockTokenClient::new(&env, &token_b).initialize(&admin);
        pool.initialize(&admin, &token_a, &token_b, &100);
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
        assert_eq!(pool.get_reserves(), (10_000, 10_000));
        assert_eq!(pool.get_total_shares(), shares);
    }

    #[test]
    fn test_add_liquidity_imbalanced() {
        let (env, pool, token_a, token_b, _) = setup();
        let provider = Address::generate(&env);
        MockTokenClient::new(&env, &token_a).mint(&provider, &100_000);
        MockTokenClient::new(&env, &token_b).mint(&provider, &100_000);

        pool.add_liquidity(&provider, &10_000, &10_000, &0);

        let provider2 = Address::generate(&env);
        MockTokenClient::new(&env, &token_a).mint(&provider2, &100_000);
        MockTokenClient::new(&env, &token_b).mint(&provider2, &100_000);
        let shares = pool.add_liquidity(&provider2, &1_000, &1_000, &0);
        assert!(shares > 0);
    }

    #[test]
    fn test_swap_low_slippage() {
        let (env, pool, token_a, token_b, _) = setup();
        let provider = Address::generate(&env);
        MockTokenClient::new(&env, &token_a).mint(&provider, &100_000);
        MockTokenClient::new(&env, &token_b).mint(&provider, &100_000);
        pool.add_liquidity(&provider, &100_000, &100_000, &0);

        // 10% trade — large enough to see slippage
        let trader = Address::generate(&env);
        MockTokenClient::new(&env, &token_a).mint(&trader, &50_000);
        let out = pool.swap(&trader, &token_a, &10_000, &0);

        // Stableswap A=100: output > CPMM for same trade (less slippage)
        assert!(out > 9_000);
        assert!(out < 10_000);
    }

    #[test]
    fn test_remove_liquidity() {
        let (env, pool, token_a, token_b, _) = setup();
        let provider = Address::generate(&env);
        MockTokenClient::new(&env, &token_a).mint(&provider, &100_000);
        MockTokenClient::new(&env, &token_b).mint(&provider, &100_000);
        let shares = pool.add_liquidity(&provider, &10_000, &10_000, &0);

        let (a, b) = pool.remove_liquidity(&provider, &shares, &0, &0);
        assert_eq!(a, 10_000);
        assert_eq!(b, 10_000);
        assert_eq!(pool.get_total_shares(), 0);
    }

    #[test]
    fn test_zero_amount_fails() {
        let (env, pool, _, _, _) = setup();
        let provider = Address::generate(&env);
        let result = pool.try_add_liquidity(&provider, &0, &100, &0);
        assert_eq!(result, Err(Ok(StableswapError::ZeroAmount)));
    }

    #[test]
    fn test_slippage_protection() {
        let (env, pool, token_a, token_b, _) = setup();
        let provider = Address::generate(&env);
        MockTokenClient::new(&env, &token_a).mint(&provider, &100_000);
        MockTokenClient::new(&env, &token_b).mint(&provider, &100_000);
        pool.add_liquidity(&provider, &100_000, &100_000, &0);

        // 10% trade with min_amount_out too high
        let trader = Address::generate(&env);
        MockTokenClient::new(&env, &token_a).mint(&trader, &50_000);
        let result = pool.try_swap(&trader, &token_a, &10_000, &10_000);
        assert_eq!(result, Err(Ok(StableswapError::SlippageExceeded)));
    }

    #[test]
    fn test_get_quote() {
        let (env, pool, token_a, token_b, _) = setup();
        let provider = Address::generate(&env);
        MockTokenClient::new(&env, &token_a).mint(&provider, &100_000);
        MockTokenClient::new(&env, &token_b).mint(&provider, &100_000);
        pool.add_liquidity(&provider, &100_000, &100_000, &0);

        let q = pool.get_quote(&token_a, &10_000);
        assert!(q > 9_000);
        assert!(q < 10_000);
    }

    #[test]
    fn test_remove_insufficient_shares() {
        let (env, pool, token_a, token_b, _) = setup();
        let provider = Address::generate(&env);
        MockTokenClient::new(&env, &token_a).mint(&provider, &100_000);
        MockTokenClient::new(&env, &token_b).mint(&provider, &100_000);
        pool.add_liquidity(&provider, &10_000, &10_000, &0);

        let result = pool.try_remove_liquidity(&provider, &999_999_999, &0, &0);
        assert_eq!(result, Err(Ok(StableswapError::InsufficientLpShares)));
    }
}
