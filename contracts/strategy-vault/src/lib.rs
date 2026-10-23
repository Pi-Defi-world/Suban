#![no_std]

//! Testnet-only vault framework.
//!
//! This contract is deliberately boring: it custodies one SEP-41 asset, issues
//! internal shares, enforces a deposit cap, supports an emergency pause, and
//! accepts one registered strategy address per vault instance. It does **not**
//! move funds into a strategy yet. Strategy execution and capital allocation
//! remain behind the Phase 4 gate and require an audit.
//!
//! The single-strategy field is an isolation boundary: a vault can name at most
//! one strategy, and the strategy can only be changed while the vault is empty.
//! Operators deploy one vault per strategy instead of mixing strategies in a
//! shared custody contract.

use soroban_sdk::{contract, contracterror, contractimpl, contracttype, symbol_short, token, Address, Env};

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Admin,
    Asset,
    Strategy,
    DepositCap,
    TotalAssets,
    TotalShares,
    Paused,
    Shares(Address),
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum VaultError {
    NotAdmin = 1,
    Paused = 2,
    ZeroAmount = 3,
    DepositCapExceeded = 4,
    InsufficientShares = 5,
    StrategyAlreadySet = 6,
    VaultNotEmpty = 7,
    InvalidCap = 8,
}

#[contracttype]
#[derive(Clone)]
pub struct VaultState {
    pub asset: Address,
    pub strategy: Option<Address>,
    pub deposit_cap: i128,
    pub total_assets: i128,
    pub total_shares: i128,
    pub paused: bool,
}

fn read_admin(env: &Env) -> Address {
    env.storage().instance().get(&DataKey::Admin).unwrap()
}

fn read_asset(env: &Env) -> Address {
    env.storage().instance().get(&DataKey::Asset).unwrap()
}

fn read_cap(env: &Env) -> i128 {
    env.storage().instance().get(&DataKey::DepositCap).unwrap_or(0)
}

fn read_total_assets(env: &Env) -> i128 {
    env.storage().instance().get(&DataKey::TotalAssets).unwrap_or(0)
}

fn read_total_shares(env: &Env) -> i128 {
    env.storage().instance().get(&DataKey::TotalShares).unwrap_or(0)
}

fn read_paused(env: &Env) -> bool {
    env.storage().instance().get(&DataKey::Paused).unwrap_or(false)
}

fn read_shares(env: &Env, user: &Address) -> i128 {
    env.storage()
        .instance()
        .get(&DataKey::Shares(user.clone()))
        .unwrap_or(0)
}

fn write_shares(env: &Env, user: &Address, shares: i128) {
    env.storage()
        .instance()
        .set(&DataKey::Shares(user.clone()), &shares);
}

fn require_not_paused(env: &Env) -> Result<(), VaultError> {
    if read_paused(env) {
        return Err(VaultError::Paused);
    }
    Ok(())
}

#[contract]
pub struct StrategyVault;

#[contractimpl]
impl StrategyVault {
    /// Create a single-strategy vault. `deposit_cap` is denominated in the
    /// asset's smallest unit.
    pub fn initialize(env: Env, admin: Address, asset: Address, deposit_cap: i128) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("already initialized");
        }
        admin.require_auth();
        if deposit_cap <= 0 {
            panic!("cap must be positive");
        }
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::Asset, &asset);
        env.storage().instance().set(&DataKey::DepositCap, &deposit_cap);
        env.storage().instance().set(&DataKey::TotalAssets, &0i128);
        env.storage().instance().set(&DataKey::TotalShares, &0i128);
        env.storage().instance().set(&DataKey::Paused, &false);
    }

    /// Register the only strategy this vault may ever route to. This is a
    /// framework boundary, not an allocation: no funds move here. The strategy
    /// can only be set while the vault holds no assets.
    pub fn set_strategy(env: Env, admin: Address, strategy: Address) -> Result<(), VaultError> {
        if admin != read_admin(&env) {
            return Err(VaultError::NotAdmin);
        }
        admin.require_auth();
        require_not_paused(&env)?;
        if read_total_assets(&env) != 0 {
            return Err(VaultError::VaultNotEmpty);
        }
        if env.storage().instance().has(&DataKey::Strategy) {
            return Err(VaultError::StrategyAlreadySet);
        }
        env.storage().instance().set(&DataKey::Strategy, &strategy);
        env.events().publish((symbol_short!("strategy"), &admin), strategy);
        Ok(())
    }

    /// Change the deposit cap. Lowering it below current assets is rejected
    /// rather than silently stranding accounting.
    pub fn set_deposit_cap(env: Env, admin: Address, deposit_cap: i128) -> Result<(), VaultError> {
        if admin != read_admin(&env) {
            return Err(VaultError::NotAdmin);
        }
        admin.require_auth();
        if deposit_cap <= 0 || deposit_cap < read_total_assets(&env) {
            return Err(VaultError::InvalidCap);
        }
        env.storage().instance().set(&DataKey::DepositCap, &deposit_cap);
        env.events()
            .publish((symbol_short!("cap"), &admin), deposit_cap);
        Ok(())
    }

    /// Pause deposits and strategy registration. Withdrawals stay open so users
    /// are never trapped by an operator pause.
    pub fn set_paused(env: Env, admin: Address, paused: bool) -> Result<(), VaultError> {
        if admin != read_admin(&env) {
            return Err(VaultError::NotAdmin);
        }
        admin.require_auth();
        env.storage().instance().set(&DataKey::Paused, &paused);
        env.events()
            .publish((symbol_short!("pause"), &admin), paused);
        Ok(())
    }

    pub fn deposit(env: Env, from: Address, amount: i128) -> Result<i128, VaultError> {
        require_not_paused(&env)?;
        from.require_auth();
        if amount <= 0 {
            return Err(VaultError::ZeroAmount);
        }

        let total_assets = read_total_assets(&env);
        let total_shares = read_total_shares(&env);
        if total_assets + amount > read_cap(&env) {
            return Err(VaultError::DepositCapExceeded);
        }

        let shares = if total_shares == 0 {
            amount
        } else {
            (amount * total_shares) / total_assets
        };
        if shares <= 0 {
            return Err(VaultError::ZeroAmount);
        }

        let asset = read_asset(&env);
        token::Client::new(&env, &asset).transfer(&from, &env.current_contract_address(), &amount);

        write_shares(&env, &from, read_shares(&env, &from) + shares);
        env.storage()
            .instance()
            .set(&DataKey::TotalAssets, &(total_assets + amount));
        env.storage()
            .instance()
            .set(&DataKey::TotalShares, &(total_shares + shares));

        env.events().publish(
            (symbol_short!("deposit"), &from),
            (
                amount,
                shares,
                total_assets + amount,
                total_shares + shares,
            ),
        );
        Ok(shares)
    }

    pub fn withdraw(env: Env, from: Address, shares: i128) -> Result<i128, VaultError> {
        from.require_auth();
        if shares <= 0 {
            return Err(VaultError::ZeroAmount);
        }

        let user_shares = read_shares(&env, &from);
        if user_shares < shares {
            return Err(VaultError::InsufficientShares);
        }

        let total_assets = read_total_assets(&env);
        let total_shares = read_total_shares(&env);
        let assets = (shares * total_assets) / total_shares;
        if assets <= 0 {
            return Err(VaultError::ZeroAmount);
        }

        write_shares(&env, &from, user_shares - shares);
        env.storage()
            .instance()
            .set(&DataKey::TotalAssets, &(total_assets - assets));
        env.storage()
            .instance()
            .set(&DataKey::TotalShares, &(total_shares - shares));

        let asset = read_asset(&env);
        token::Client::new(&env, &asset).transfer(&env.current_contract_address(), &from, &assets);

        env.events().publish(
            (symbol_short!("withdraw"), &from),
            (
                shares,
                assets,
                total_assets - assets,
                total_shares - shares,
            ),
        );
        Ok(assets)
    }

    pub fn get_state(env: Env) -> VaultState {
        VaultState {
            asset: read_asset(&env),
            strategy: env.storage().instance().get(&DataKey::Strategy),
            deposit_cap: read_cap(&env),
            total_assets: read_total_assets(&env),
            total_shares: read_total_shares(&env),
            paused: read_paused(&env),
        }
    }

    pub fn get_shares(env: Env, user: Address) -> i128 {
        read_shares(&env, &user)
    }

    pub fn admin(env: Env) -> Address {
        read_admin(&env)
    }

    pub fn asset(env: Env) -> Address {
        read_asset(&env)
    }

    pub fn strategy(env: Env) -> Option<Address> {
        env.storage().instance().get(&DataKey::Strategy)
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use soroban_sdk::testutils::Address as _;
    use soroban_sdk::testutils::Events as _;
    use soroban_sdk::{Symbol, TryFromVal};

    #[soroban_sdk::contract]
    struct MockToken;

    #[soroban_sdk::contracttype]
    enum MockKey {
        Bal(Address),
    }

    #[soroban_sdk::contractimpl]
    impl MockToken {
        pub fn mint(env: Env, to: Address, amount: i128) {
            let current: i128 = env
                .storage()
                .instance()
                .get::<MockKey, i128>(&MockKey::Bal(to.clone()))
                .unwrap_or(0);
            env.storage()
                .instance()
                .set(&MockKey::Bal(to), &(current + amount));
        }

        pub fn transfer(env: Env, from: Address, to: Address, amount: i128) {
            let from_balance: i128 = env
                .storage()
                .instance()
                .get::<MockKey, i128>(&MockKey::Bal(from.clone()))
                .unwrap_or(0);
            let to_balance: i128 = env
                .storage()
                .instance()
                .get::<MockKey, i128>(&MockKey::Bal(to.clone()))
                .unwrap_or(0);
            env.storage()
                .instance()
                .set(&MockKey::Bal(from), &(from_balance - amount));
            env.storage()
                .instance()
                .set(&MockKey::Bal(to), &(to_balance + amount));
        }
    }

    fn setup() -> (Env, StrategyVaultClient<'static>, Address, Address) {
        let env = Env::default();
        env.mock_all_auths();

        let token = env.register(MockToken, ());
        let contract_id = env.register(StrategyVault, ());
        let client = StrategyVaultClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin, &token, &1_000);

        let user = Address::generate(&env);
        MockTokenClient::new(&env, &token).mint(&user, &10_000);
        (env, client, admin, token)
    }

    fn last_event(env: &Env) -> (soroban_sdk::Vec<soroban_sdk::Val>, soroban_sdk::Val) {
        let events = env.events().all();
        let event = events.events().last().unwrap();
        let body = match &event.body {
            soroban_sdk::xdr::ContractEventBody::V0(v0) => v0,
        };
        let mut topics = soroban_sdk::Vec::new(env);
        for value in body.topics.iter() {
            topics.push_back(soroban_sdk::Val::try_from_val(env, value).unwrap());
        }
        let data = soroban_sdk::Val::try_from_val(env, &body.data).unwrap();
        (topics, data)
    }

    #[test]
    fn test_deposit_and_withdraw() {
        let (env, client, _admin, token) = setup();
        let user = Address::generate(&env);
        MockTokenClient::new(&env, &token).mint(&user, &500);

        let shares = client.deposit(&user, &500);
        assert_eq!(shares, 500);

        // Read the event before any further contract invocation: the test
        // environment's event buffer is associated with the last call.
        let (topics, data) = last_event(&env);
        let name = Symbol::try_from_val(&env, &topics.get(0).unwrap()).unwrap();
        assert_eq!(name, symbol_short!("deposit"));
        let payload = <(i128, i128, i128, i128)>::try_from_val(&env, &data).unwrap();
        assert_eq!(payload.0, 500);
        assert_eq!(payload.1, 500);
        assert_eq!(payload.2, 500);
        assert_eq!(payload.3, 500);

        assert_eq!(client.get_shares(&user), 500);
        let state = client.get_state();
        assert_eq!(state.total_assets, 500);
        assert_eq!(state.total_shares, 500);
        assert_eq!(state.deposit_cap, 1_000);
        assert_eq!(state.paused, false);

        let assets = client.withdraw(&user, &200);
        assert_eq!(assets, 200);
        assert_eq!(client.get_shares(&user), 300);
        assert_eq!(client.get_state().total_assets, 300);
    }

    #[test]
    fn test_deposit_cap_is_enforced() {
        let (_env, client, _admin, token) = setup();
        let user = Address::generate(&_env);
        MockTokenClient::new(&_env, &token).mint(&user, &2_000);

        let result = client.try_deposit(&user, &1_001);
        assert_eq!(result, Err(Ok(VaultError::DepositCapExceeded)));
    }

    #[test]
    fn test_pause_blocks_deposit_but_not_withdraw() {
        let (env, client, admin, token) = setup();
        let user = Address::generate(&env);
        MockTokenClient::new(&env, &token).mint(&user, &500);
        client.deposit(&user, &500);

        client.set_paused(&admin, &true);
        let result = client.try_deposit(&user, &100);
        assert_eq!(result, Err(Ok(VaultError::Paused)));

        // Withdrawals remain available while paused.
        let assets = client.withdraw(&user, &500);
        assert_eq!(assets, 500);
        assert_eq!(client.get_state().total_assets, 0);
    }

    #[test]
    fn test_strategy_isolation() {
        let (env, client, admin, token) = setup();
        let strategy = Address::generate(&env);
        client.set_strategy(&admin, &strategy);
        assert_eq!(client.strategy(), Some(strategy.clone()));

        let user = Address::generate(&env);
        MockTokenClient::new(&env, &token).mint(&user, &100);
        client.deposit(&user, &100);

        // A second strategy cannot be added and the first cannot be replaced
        // while the vault holds assets.
        let second = Address::generate(&env);
        let result = client.try_set_strategy(&admin, &second);
        assert_eq!(result, Err(Ok(VaultError::VaultNotEmpty)));
    }

    #[test]
    fn test_strategy_cannot_be_replaced_when_empty() {
        let (env, client, admin, _token) = setup();
        let first = Address::generate(&env);
        let second = Address::generate(&env);
        client.set_strategy(&admin, &first);
        let result = client.try_set_strategy(&admin, &second);
        assert_eq!(result, Err(Ok(VaultError::StrategyAlreadySet)));
    }

    #[test]
    fn test_non_admin_controls() {
        let (env, client, _admin, _token) = setup();
        let not_admin = Address::generate(&env);
        let strategy = Address::generate(&env);

        assert_eq!(
            client.try_set_strategy(&not_admin, &strategy),
            Err(Ok(VaultError::NotAdmin))
        );
        assert_eq!(
            client.try_set_paused(&not_admin, &true),
            Err(Ok(VaultError::NotAdmin))
        );
        assert_eq!(
            client.try_set_deposit_cap(&not_admin, &2_000),
            Err(Ok(VaultError::NotAdmin))
        );
    }

    #[test]
    fn test_zero_and_insufficient_shares() {
        let (env, client, _admin, _token) = setup();
        let user = Address::generate(&env);

        assert_eq!(
            client.try_deposit(&user, &0),
            Err(Ok(VaultError::ZeroAmount))
        );
        assert_eq!(
            client.try_withdraw(&user, &1),
            Err(Ok(VaultError::InsufficientShares))
        );
    }

    #[test]
    fn test_cap_cannot_be_lowered_below_assets() {
        let (env, client, admin, token) = setup();
        let user = Address::generate(&env);
        MockTokenClient::new(&env, &token).mint(&user, &500);
        client.deposit(&user, &500);

        assert_eq!(
            client.try_set_deposit_cap(&admin, &499),
            Err(Ok(VaultError::InvalidCap))
        );
    }
}
