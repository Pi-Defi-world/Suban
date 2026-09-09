#![no_std]

//! Liquidity Mining — emissions engine for LP reward distribution.
//!
//! LP providers stake their LP tokens into a farm to earn ZYR rewards.
//! Rewards accrue per block proportional to staked shares.
//! Claiming distributes accumulated rewards.

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, token, Address, Env,
};

// ─── Types ───────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone)]
pub struct FarmInfo {
    pub reward_token: Address,
    pub reward_per_ledger: i128,
    pub total_staked: i128,
    pub last_ledger: u32,
    pub accumulated_rewards_per_share: i128,
}

#[contracttype]
#[derive(Clone)]
pub struct StakeInfo {
    pub amount: i128,
    pub reward_debt: i128, // accumulated_rewards_per_share at time of stake
}

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Admin,
    Farm,
    Stake(Address),
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum MiningError {
    NotAdmin = 1,
    AlreadyInitialized = 2,
    ZeroAmount = 3,
    InsufficientStake = 4,
    NoRewards = 5,
}

// ─── Helpers ─────────────────────────────────────────────────────────

fn read_admin(env: &Env) -> Address {
    env.storage().instance().get::<DataKey, Address>(&DataKey::Admin).unwrap()
}

fn read_farm(env: &Env) -> FarmInfo {
    env.storage().instance().get::<DataKey, FarmInfo>(&DataKey::Farm).unwrap()
}

fn write_farm(env: &Env, farm: &FarmInfo) {
    env.storage().instance().set(&DataKey::Farm, farm);
}

fn read_stake(env: &Env, addr: &Address) -> StakeInfo {
    env.storage().instance().get::<DataKey, StakeInfo>(&DataKey::Stake(addr.clone()))
        .unwrap_or(StakeInfo { amount: 0, reward_debt: 0 })
}

fn write_stake(env: &Env, addr: &Address, stake: &StakeInfo) {
    env.storage().instance().set(&DataKey::Stake(addr.clone()), stake);
}

/// Accrue rewards for all stakers up to current ledger.
fn accrue_rewards(env: &Env) {
    let mut farm = read_farm(env);
    if farm.total_staked == 0 { return; }

    let current_ledger = env.ledger().sequence();
    let ledgers_passed = current_ledger.saturating_sub(farm.last_ledger);

    if ledgers_passed > 0 {
        let reward = farm.reward_per_ledger * (ledgers_passed as i128);
        farm.accumulated_rewards_per_share += reward * 1_000_000 / farm.total_staked;
        farm.last_ledger = current_ledger;
        write_farm(env, &farm);
    }
}

// ─── Contract ────────────────────────────────────────────────────────

#[contract]
pub struct LiquidityMining;

#[contractimpl]
impl LiquidityMining {
    pub fn initialize(
        env: Env,
        admin: Address,
        reward_token: Address,
        reward_per_ledger: i128,
    ) {
        if env.storage().instance().has(&DataKey::Admin) { panic!("already initialized"); }
        admin.require_auth();
        if reward_per_ledger < 0 { panic!("reward must be non-negative"); }

        let farm = FarmInfo {
            reward_token,
            reward_per_ledger,
            total_staked: 0,
            last_ledger: env.ledger().sequence(),
            accumulated_rewards_per_share: 0,
        };

        env.storage().instance().set(&DataKey::Admin, &admin);
        write_farm(&env, &farm);
    }

    /// Update emission rate (admin only).
    pub fn set_reward_per_ledger(
        env: Env,
        admin: Address,
        new_rate: i128,
    ) -> Result<(), MiningError> {
        if admin != read_admin(&env) { return Err(MiningError::NotAdmin); }
        admin.require_auth();
        if new_rate < 0 { return Err(MiningError::ZeroAmount); }

        accrue_rewards(&env);
        let mut farm = read_farm(&env);
        farm.reward_per_ledger = new_rate;
        write_farm(&env, &farm);

        env.events().publish(
            (symbol_short!("rate_set"),),
            (new_rate,),
        );

        Ok(())
    }

    /// Stake LP tokens into the farm.
    pub fn stake(
        env: Env,
        user: Address,
        amount: i128,
    ) -> Result<(), MiningError> {
        user.require_auth();
        if amount <= 0 { return Err(MiningError::ZeroAmount); }

        accrue_rewards(&env);

        let farm = read_farm(&env);
        let mut user_stake = read_stake(&env, &user);

        // Transfer LP tokens from user to this contract
        let vault = env.current_contract_address();
        token::Client::new(&env, &farm.reward_token).transfer(&user, &vault, &amount);

        // Update accumulated rewards for this user before changing their stake
        let pending = user_stake.amount * farm.accumulated_rewards_per_share / 1_000_000 - user_stake.reward_debt;
        if pending > 0 {
            // Transfer pending rewards to user
            token::Client::new(&env, &farm.reward_token).transfer(&vault, &user, &pending);
        }

        user_stake.amount += amount;
        user_stake.reward_debt = user_stake.amount * farm.accumulated_rewards_per_share / 1_000_000;
        write_stake(&env, &user, &user_stake);

        let mut farm = read_farm(&env);
        farm.total_staked += amount;
        write_farm(&env, &farm);

        env.events().publish(
            (symbol_short!("staked"), &user),
            (amount,),
        );

        Ok(())
    }

    /// Unstake LP tokens and claim all pending rewards.
    pub fn unstake(
        env: Env,
        user: Address,
        amount: i128,
    ) -> Result<i128, MiningError> {
        user.require_auth();
        if amount <= 0 { return Err(MiningError::ZeroAmount); }

        accrue_rewards(&env);

        let farm = read_farm(&env);
        let mut user_stake = read_stake(&env, &user);

        if user_stake.amount < amount { return Err(MiningError::InsufficientStake); }

        // Calculate pending rewards
        let pending = user_stake.amount * farm.accumulated_rewards_per_share / 1_000_000 - user_stake.reward_debt;

        user_stake.amount -= amount;
        user_stake.reward_debt = user_stake.amount * farm.accumulated_rewards_per_share / 1_000_000;
        write_stake(&env, &user, &user_stake);

        let mut farm = read_farm(&env);
        farm.total_staked -= amount;
        write_farm(&env, &farm);

        // Transfer LP tokens back to user
        token::Client::new(&env, &farm.reward_token).transfer(&env.current_contract_address(), &user, &amount);

        // Transfer pending rewards
        if pending > 0 {
            token::Client::new(&env, &farm.reward_token).transfer(&env.current_contract_address(), &user, &pending);
        }

        env.events().publish(
            (symbol_short!("unstake"), &user),
            (amount, pending),
        );

        Ok(pending)
    }

    /// Claim pending rewards without unstaking.
    pub fn claim(
        env: Env,
        user: Address,
    ) -> Result<i128, MiningError> {
        user.require_auth();

        accrue_rewards(&env);

        let farm = read_farm(&env);
        let mut user_stake = read_stake(&env, &user);

        let pending = user_stake.amount * farm.accumulated_rewards_per_share / 1_000_000 - user_stake.reward_debt;
        if pending <= 0 { return Err(MiningError::NoRewards); }

        user_stake.reward_debt = user_stake.amount * farm.accumulated_rewards_per_share / 1_000_000;
        write_stake(&env, &user, &user_stake);

        token::Client::new(&env, &farm.reward_token).transfer(&env.current_contract_address(), &user, &pending);

        env.events().publish(
            (symbol_short!("claimed"), &user),
            (pending,),
        );

        Ok(pending)
    }

    // ─── Queries ────────────────────────────────────────────────────

    pub fn pending_rewards(env: Env, user: Address) -> i128 {
        let farm = read_farm(&env);
        let user_stake = read_stake(&env, &user);
        if user_stake.amount == 0 { return 0; }

        // Simulate accrual
        let current_ledger = env.ledger().sequence();
        let ledgers_passed = current_ledger.saturating_sub(farm.last_ledger);
        let mut arp = farm.accumulated_rewards_per_share;
        if ledgers_passed > 0 && farm.total_staked > 0 {
            let reward = farm.reward_per_ledger * (ledgers_passed as i128);
            arp += reward * 1_000_000 / farm.total_staked;
        }

        user_stake.amount * arp / 1_000_000 - user_stake.reward_debt
    }

    pub fn get_stake(env: Env, user: Address) -> StakeInfo {
        read_stake(&env, &user)
    }

    pub fn get_farm(env: Env) -> FarmInfo {
        read_farm(&env)
    }

    pub fn admin(env: Env) -> Address {
        read_admin(&env)
    }
}

#[cfg(test)]
mod test {
    extern crate std;
    use super::{LiquidityMining, LiquidityMiningClient, MiningError};
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

    fn setup() -> (Env, LiquidityMiningClient<'static>, Address, Address) {
        let env = Env::default();
        env.mock_all_auths();
        let id = env.register(LiquidityMining, ());
        let client = LiquidityMiningClient::new(&env, &id);
        let admin = Address::generate(&env);
        let reward_token = env.register(MockToken, ());
        MockTokenClient::new(&env, &reward_token).initialize(&admin);
        client.initialize(&admin, &reward_token, &100);
        (env, client, admin, reward_token)
    }

    #[test]
    fn test_initialize() {
        let (env, client, _, _) = setup();
        let farm = client.get_farm();
        assert_eq!(farm.reward_per_ledger, 100);
        assert_eq!(farm.total_staked, 0);
    }

    #[test]
    fn test_stake() {
        let (env, client, _, reward_token) = setup();
        let user = Address::generate(&env);
        MockTokenClient::new(&env, &reward_token).mint(&user, &10_000);

        client.stake(&user, &1_000);

        let stake = client.get_stake(&user);
        assert_eq!(stake.amount, 1_000);
        assert_eq!(client.get_farm().total_staked, 1_000);
    }

    #[test]
    fn test_unstake() {
        let (env, client, _, reward_token) = setup();
        let user = Address::generate(&env);
        MockTokenClient::new(&env, &reward_token).mint(&user, &10_000);

        client.stake(&user, &1_000);
        client.unstake(&user, &500);

        let stake = client.get_stake(&user);
        assert_eq!(stake.amount, 500);
        assert_eq!(client.get_farm().total_staked, 500);
    }

    #[test]
    fn test_insufficient_stake_fails() {
        let (env, client, _, reward_token) = setup();
        let user = Address::generate(&env);
        MockTokenClient::new(&env, &reward_token).mint(&user, &10_000);

        client.stake(&user, &1_000);
        let result = client.try_unstake(&user, &2_000);
        assert_eq!(result, Err(Ok(MiningError::InsufficientStake)));
    }

    #[test]
    fn test_set_reward_per_ledger() {
        let (env, client, admin, _) = setup();
        client.set_reward_per_ledger(&admin, &200);
        assert_eq!(client.get_farm().reward_per_ledger, 200);
    }

    #[test]
    fn test_non_admin_fails() {
        let (env, client, _, _) = setup();
        let rando = Address::generate(&env);
        let result = client.try_set_reward_per_ledger(&rando, &50);
        assert_eq!(result, Err(Ok(MiningError::NotAdmin)));
    }

    #[test]
    fn test_zero_amount_fails() {
        let (env, client, _, _) = setup();
        let user = Address::generate(&env);
        let result = client.try_stake(&user, &0);
        assert_eq!(result, Err(Ok(MiningError::ZeroAmount)));
    }

    #[test]
    fn test_claim_no_rewards_fails() {
        let (env, client, _, reward_token) = setup();
        let user = Address::generate(&env);
        MockTokenClient::new(&env, &reward_token).mint(&user, &10_000);

        client.stake(&user, &1_000);
        // No ledgers passed, no rewards
        let result = client.try_claim(&user);
        assert_eq!(result, Err(Ok(MiningError::NoRewards)));
    }

    #[test]
    fn test_pending_rewards_zero_stake() {
        let (env, client, _, _) = setup();
        let user = Address::generate(&env);
        assert_eq!(client.pending_rewards(&user), 0);
    }
}
