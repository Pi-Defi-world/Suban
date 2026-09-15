#![no_std]

use soroban_sdk::{
    contract, contractimpl, contracttype, contracterror, symbol_short,
    Address, Env, Map, Symbol,
};

const POOL_ADMIN: Symbol = symbol_short!("admin");
const POOL_ADDRESS: Symbol = symbol_short!("pool");
const BACKSTOP_ASSET: Symbol = symbol_short!("asset");
const CLAIM_REQUESTS: Symbol = symbol_short!("clmReq");
const BACKSTOP_BALANCE: Symbol = symbol_short!("balance");
const BACKSTOP_TARGET: Symbol = symbol_short!("target");
const DEPOSITORS: Symbol = symbol_short!("depos");
const DEPOSITOR_SHARES: Symbol = symbol_short!("shares");
const TOTAL_SHARES: Symbol = symbol_short!("totShr");
const TOTAL_DEPOSITS: Symbol = symbol_short!("totDep");
const CLAIMED: Symbol = symbol_short!("claim");
const PAUSED: Symbol = symbol_short!("paused");
const CLAIM_DELAY: Symbol = symbol_short!("clmDly");
const MAX_DEPOSIT: Symbol = symbol_short!("maxDep");
const MIN_DEPOSIT: Symbol = symbol_short!("minDep");

#[derive(Clone, PartialEq, Debug)]
#[contracterror]
pub enum BackstopError {
    NotAdmin = 1,
    Paused = 2,
    ZeroAmount = 3,
    InsufficientBalance = 4,
    DepositTooSmall = 5,
    DepositTooLarge = 6,
    NoShares = 7,
    ClaimPending = 8,
    ClaimExpired = 9,
    NoBadDebt = 10,
}

#[derive(Clone)]
#[contracttype]
pub struct BackstopState {
    pub total_shares: i128,
    pub total_deposits: i128,
    pub balance: i128,
    pub target: i128,
    pub claim_delay: u32,
    pub max_deposit: i128,
    pub min_deposit: i128,
}

#[derive(Clone)]
#[contracttype]
pub struct ClaimRequest {
    pub depositor: Address,
    pub shares: i128,
    pub requested_at: u32,
}

#[contract]
pub struct Backstop;

#[contractimpl]
impl Backstop {
    /// Initialize the backstop module for a lending pool
    pub fn initialize(
        env: Env,
        admin: Address,
        pool: Address,
        asset: Address,
        target: i128,
        claim_delay: u32,
        max_deposit: i128,
        min_deposit: i128,
    ) -> Result<(), BackstopError> {
        if target <= 0 {
            return Err(BackstopError::ZeroAmount);
        }

        env.storage().instance().set(&POOL_ADMIN, &admin);
        env.storage().instance().set(&POOL_ADDRESS, &pool);
        env.storage().instance().set(&BACKSTOP_ASSET, &asset);
        env.storage().instance().set(&BACKSTOP_TARGET, &target);
        env.storage().instance().set(&CLAIM_DELAY, &claim_delay);
        env.storage().instance().set(&MAX_DEPOSIT, &max_deposit);
        env.storage().instance().set(&MIN_DEPOSIT, &min_deposit);
        env.storage().instance().set(&BACKSTOP_BALANCE, &0i128);
        env.storage().instance().set(&TOTAL_SHARES, &0i128);
        env.storage().instance().set(&TOTAL_DEPOSITS, &0i128);
        env.storage().instance().set(&PAUSED, &false);

        Ok(())
    }

    /// Deposit assets into the backstop to provide insurance
    pub fn deposit(env: Env, from: Address, amount: i128) -> Result<i128, BackstopError> {
        if amount <= 0 {
            return Err(BackstopError::ZeroAmount);
        }
        Self::require_not_paused(&env)?;
        from.require_auth();

        let min_deposit: i128 = env.storage().instance().get(&MIN_DEPOSIT).unwrap_or(0);
        let max_deposit: i128 = env.storage().instance().get(&MAX_DEPOSIT).unwrap_or(i128::MAX);

        if amount < min_deposit {
            return Err(BackstopError::DepositTooSmall);
        }
        if amount > max_deposit {
            return Err(BackstopError::DepositTooLarge);
        }

        let total_shares: i128 = env.storage().instance().get(&TOTAL_SHARES).unwrap_or(0);
        let total_deposits: i128 = env.storage().instance().get(&TOTAL_DEPOSITS).unwrap_or(0);

        let shares = if total_shares == 0 {
            amount
        } else {
            (amount * total_shares) / total_deposits
        };

        // Update depositor shares
        let mut depositor_shares: Map<Address, i128> = env
            .storage()
            .instance()
            .get(&DEPOSITOR_SHARES)
            .unwrap_or(Map::new(&env));
        let current_shares = depositor_shares.get(from.clone()).unwrap_or(0);
        depositor_shares.set(from.clone(), current_shares + shares);
        env.storage().instance().set(&DEPOSITOR_SHARES, &depositor_shares);

        // Update totals
        env.storage().instance().set(&TOTAL_SHARES, &(total_shares + shares));
        env.storage().instance().set(&TOTAL_DEPOSITS, &(total_deposits + amount));

        // Update balance (funds available to absorb bad debt)
        let balance: i128 = env.storage().instance().get(&BACKSTOP_BALANCE).unwrap_or(0);
        env.storage().instance().set(&BACKSTOP_BALANCE, &(balance + amount));

        // Track deposits for withdrawal
        let mut deposits: Map<Address, i128> = env
            .storage()
            .instance()
            .get(&DEPOSITORS)
            .unwrap_or(Map::new(&env));
        let current_deposit = deposits.get(from.clone()).unwrap_or(0);
        deposits.set(from.clone(), current_deposit + amount);
        env.storage().instance().set(&DEPOSITORS, &deposits);

        // Custody: pull the deposited underlying from the depositor into the backstop.
        let asset = soroban_sdk::token::Client::new(&env, &Self::backstop_asset(&env));
        let vault = env.current_contract_address();
        asset.transfer(&from, &vault, &amount);

        Ok(shares)
    }

    /// Withdraw assets from the backstop (subject to claim delay)
    pub fn withdraw(env: Env, from: Address, shares: i128) -> Result<i128, BackstopError> {
        if shares <= 0 {
            return Err(BackstopError::ZeroAmount);
        }
        Self::require_not_paused(&env)?;
        from.require_auth();

        let depositor_shares: Map<Address, i128> = env
            .storage()
            .instance()
            .get(&DEPOSITOR_SHARES)
            .unwrap_or(Map::new(&env));
        let current_shares = depositor_shares.get(from.clone()).unwrap_or(0);

        if current_shares < shares {
            return Err(BackstopError::InsufficientBalance);
        }

        let total_shares: i128 = env.storage().instance().get(&TOTAL_SHARES).unwrap_or(0);
        let total_deposits: i128 = env.storage().instance().get(&TOTAL_DEPOSITS).unwrap_or(0);

        let amount = (shares * total_deposits) / total_shares;

        // Update depositor shares
        let mut depositor_shares_mut = depositor_shares;
        depositor_shares_mut.set(from.clone(), current_shares - shares);
        env.storage().instance().set(&DEPOSITOR_SHARES, &depositor_shares_mut);

        // Update totals
        env.storage().instance().set(&TOTAL_SHARES, &(total_shares - shares));
        env.storage().instance().set(&TOTAL_DEPOSITS, &(total_deposits - amount));

        // Update deposits tracking
        let mut deposits: Map<Address, i128> = env
            .storage()
            .instance()
            .get(&DEPOSITORS)
            .unwrap_or(Map::new(&env));
        let current_deposit = deposits.get(from.clone()).unwrap_or(0);
        deposits.set(from.clone(), current_deposit - amount);
        env.storage().instance().set(&DEPOSITORS, &deposits);

        // Custody: return the underlying to the depositor.
        let asset = soroban_sdk::token::Client::new(&env, &Self::backstop_asset(&env));
        let vault = env.current_contract_address();
        asset.transfer(&vault, &from, &amount);

        Ok(amount)
    }

    /// Request to claim a share of bad debt absorbed by the backstop.
    /// The actual payout is released only after `claim_delay` ledgers have passed.
    pub fn request_claim(env: Env, from: Address, shares: i128) -> Result<(), BackstopError> {
        if shares <= 0 {
            return Err(BackstopError::ZeroAmount);
        }
        Self::require_not_paused(&env)?;
        from.require_auth();

        let mut requests: Map<Address, ClaimRequest> = env
            .storage()
            .instance()
            .get(&CLAIM_REQUESTS)
            .unwrap_or(Map::new(&env));
        requests.set(
            from.clone(),
            ClaimRequest {
                depositor: from.clone(),
                shares,
                requested_at: env.ledger().sequence(),
            },
        );
        env.storage().instance().set(&CLAIM_REQUESTS, &requests);

        Ok(())
    }

    /// Claim a share of bad debt absorbed by the backstop.
    /// Requires a prior `request_claim` and that `claim_delay` ledgers have elapsed.
    pub fn claim(env: Env, from: Address, shares: i128) -> Result<i128, BackstopError> {
        if shares <= 0 {
            return Err(BackstopError::ZeroAmount);
        }
        Self::require_not_paused(&env)?;
        from.require_auth();

        let claim_delay: u32 = env.storage().instance().get(&CLAIM_DELAY).unwrap_or(0);
        let mut requests: Map<Address, ClaimRequest> = env
            .storage()
            .instance()
            .get(&CLAIM_REQUESTS)
            .unwrap_or(Map::new(&env));
        let request = requests.get(from.clone()).ok_or(BackstopError::ClaimPending)?;
        let now = env.ledger().sequence();
        if now < request.requested_at + claim_delay {
            return Err(BackstopError::ClaimPending);
        }
        requests.remove(from.clone());
        env.storage().instance().set(&CLAIM_REQUESTS, &requests);

        let depositor_shares: Map<Address, i128> = env
            .storage()
            .instance()
            .get(&DEPOSITOR_SHARES)
            .unwrap_or(Map::new(&env));
        let current_shares = depositor_shares.get(from.clone()).unwrap_or(0);

        if current_shares < shares {
            return Err(BackstopError::InsufficientBalance);
        }

        let total_shares: i128 = env.storage().instance().get(&TOTAL_SHARES).unwrap_or(0);
        let total_deposits: i128 = env.storage().instance().get(&TOTAL_DEPOSITS).unwrap_or(0);
        let balance: i128 = env.storage().instance().get(&BACKSTOP_BALANCE).unwrap_or(0);

        // Calculate payout from bad debt absorbed
        let payout = if total_shares > 0 {
            (shares * balance) / total_shares
        } else {
            0
        };

        if payout <= 0 {
            return Err(BackstopError::NoBadDebt);
        }

        // Update depositor shares
        let mut depositor_shares_mut = depositor_shares;
        depositor_shares_mut.set(from.clone(), current_shares - shares);
        env.storage().instance().set(&DEPOSITOR_SHARES, &depositor_shares_mut);

        // Update totals
        env.storage().instance().set(&TOTAL_SHARES, &(total_shares - shares));
        env.storage().instance().set(&TOTAL_DEPOSITS, &(total_deposits - payout));

        // Update balance
        env.storage().instance().set(&BACKSTOP_BALANCE, &(balance - payout));

        // Custody: release the payout to the depositor.
        let asset = soroban_sdk::token::Client::new(&env, &Self::backstop_asset(&env));
        let vault = env.current_contract_address();
        asset.transfer(&vault, &from, &payout);

        Ok(payout)
    }

    /// Absorb bad debt from the lending pool (called by pool during liquidation).
    /// Only the configured pool may call this; the covered tokens are sent to the pool.
    pub fn absorb_bad_debt(env: Env, amount: i128) -> Result<(), BackstopError> {
        Self::require_not_paused(&env)?;

        let pool: Address = env.storage().instance().get(&POOL_ADDRESS).unwrap();
        pool.require_auth();

        let balance: i128 = env.storage().instance().get(&BACKSTOP_BALANCE).unwrap_or(0);

        let covered = if amount > balance { balance } else { amount };

        env.storage().instance().set(&BACKSTOP_BALANCE, &(balance - covered));

        // Custody: send the covered balance to the pool to settle the bad debt.
        let asset = soroban_sdk::token::Client::new(&env, &Self::backstop_asset(&env));
        let vault = env.current_contract_address();
        asset.transfer(&vault, &pool, &covered);

        Ok(())
    }

    /// Get backstop state
    pub fn get_state(env: Env) -> BackstopState {
        BackstopState {
            total_shares: env.storage().instance().get(&TOTAL_SHARES).unwrap_or(0),
            total_deposits: env.storage().instance().get(&TOTAL_DEPOSITS).unwrap_or(0),
            balance: env.storage().instance().get(&BACKSTOP_BALANCE).unwrap_or(0),
            target: env.storage().instance().get(&BACKSTOP_TARGET).unwrap_or(0),
            claim_delay: env.storage().instance().get(&CLAIM_DELAY).unwrap_or(0),
            max_deposit: env.storage().instance().get(&MAX_DEPOSIT).unwrap_or(i128::MAX),
            min_deposit: env.storage().instance().get(&MIN_DEPOSIT).unwrap_or(0),
        }
    }

    /// Get depositor shares
    pub fn get_shares(env: Env, depositor: Address) -> i128 {
        let depositor_shares: Map<Address, i128> = env
            .storage()
            .instance()
            .get(&DEPOSITOR_SHARES)
            .unwrap_or(Map::new(&env));
        depositor_shares.get(depositor).unwrap_or(0)
    }

    /// Pause/unpause the backstop
    pub fn set_paused(env: Env, admin: Address, paused: bool) -> Result<(), BackstopError> {
        let stored_admin: Address = env.storage().instance().get(&POOL_ADMIN).unwrap();
        if admin != stored_admin {
            return Err(BackstopError::NotAdmin);
        }
        admin.require_auth();
        env.storage().instance().set(&PAUSED, &paused);
        Ok(())
    }

    fn backstop_asset(env: &Env) -> Address {
        env.storage()
            .instance()
            .get(&BACKSTOP_ASSET)
            .unwrap_or_else(|| env.current_contract_address())
    }

    fn require_not_paused(env: &Env) -> Result<(), BackstopError> {
        let paused: bool = env.storage().instance().get(&PAUSED).unwrap_or(false);
        if paused {
            return Err(BackstopError::Paused);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use soroban_sdk::{contract, contractimpl, contracttype, testutils::Address as _, testutils::Ledger as _};

    #[contract]
    struct MockToken;

    #[contracttype]
    enum MockKey {
        Bal(Address),
    }

    #[contractimpl]
    impl MockToken {
        pub fn initialize(_env: Env, _admin: Address) {}
        pub fn mint(env: Env, to: Address, amount: i128) {
            let b: i128 = env
                .storage()
                .instance()
                .get::<MockKey, i128>(&MockKey::Bal(to.clone()))
                .unwrap_or(0);
            env.storage().instance().set(&MockKey::Bal(to), &(b + amount));
        }
        pub fn transfer(env: Env, from: Address, to: Address, amount: i128) {
            let fb: i128 = env
                .storage()
                .instance()
                .get::<MockKey, i128>(&MockKey::Bal(from.clone()))
                .unwrap_or(0);
            let tb: i128 = env
                .storage()
                .instance()
                .get::<MockKey, i128>(&MockKey::Bal(to.clone()))
                .unwrap_or(0);
            env.storage().instance().set(&MockKey::Bal(from), &(fb - amount));
            env.storage().instance().set(&MockKey::Bal(to), &(tb + amount));
        }
    }

    fn setup() -> (Env, BackstopClient<'static>, Address, Address, Address) {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(Backstop, ());
        let client = BackstopClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        let pool = Address::generate(&env);
        let asset = env.register(MockToken, ());
        MockTokenClient::new(&env, &asset).initialize(&admin);

        client.initialize(&admin, &pool, &asset, &1000000, &100, &100000, &100);

        (env, client, admin, pool, asset)
    }

    fn fund(env: &Env, asset: &Address, to: &Address, amount: i128) {
        MockTokenClient::new(env, asset).mint(to, &amount);
    }

    #[test]
    fn test_initialize() {
        let (env, client, _admin, _pool, _asset) = setup();

        let state = client.get_state();
        assert_eq!(state.total_shares, 0);
        assert_eq!(state.total_deposits, 0);
        assert_eq!(state.balance, 0);
        assert_eq!(state.target, 1000000);
        assert_eq!(state.claim_delay, 100);
        assert_eq!(state.max_deposit, 100000);
        assert_eq!(state.min_deposit, 100);
    }

    #[test]
    fn test_deposit() {
        let (env, client, _admin, _pool, asset) = setup();

        let depositor = Address::generate(&env);
        fund(&env, &asset, &depositor, 1000);
        let shares = client.deposit(&depositor, &1000);

        assert!(shares > 0);

        let state = client.get_state();
        assert_eq!(state.total_shares, shares);
        assert_eq!(state.total_deposits, 1000);
    }

    #[test]
    fn test_deposit_too_small() {
        let (env, client, _admin, _pool, _asset) = setup();

        let depositor = Address::generate(&env);
        let result = client.try_deposit(&depositor, &50);

        assert_eq!(result, Err(Ok(BackstopError::DepositTooSmall)));
    }

    #[test]
    fn test_deposit_too_large() {
        let (env, client, _admin, _pool, _asset) = setup();

        let depositor = Address::generate(&env);
        let result = client.try_deposit(&depositor, &200000);

        assert_eq!(result, Err(Ok(BackstopError::DepositTooLarge)));
    }

    #[test]
    fn test_zero_amount_deposit() {
        let (env, client, _admin, _pool, _asset) = setup();

        let depositor = Address::generate(&env);
        let result = client.try_deposit(&depositor, &0);

        assert_eq!(result, Err(Ok(BackstopError::ZeroAmount)));
    }

    #[test]
    fn test_withdraw() {
        let (env, client, _admin, _pool, asset) = setup();

        let depositor = Address::generate(&env);
        fund(&env, &asset, &depositor, 1000);
        let shares = client.deposit(&depositor, &1000);

        let withdrawn = client.withdraw(&depositor, &shares);
        assert_eq!(withdrawn, 1000);

        let state = client.get_state();
        assert_eq!(state.total_shares, 0);
        assert_eq!(state.total_deposits, 0);
    }

    #[test]
    fn test_withdraw_insufficient_shares() {
        let (env, client, _admin, _pool, asset) = setup();

        let depositor = Address::generate(&env);
        fund(&env, &asset, &depositor, 1000);
        let _shares = client.deposit(&depositor, &1000);

        let result = client.try_withdraw(&depositor, &9999);
        assert_eq!(result, Err(Ok(BackstopError::InsufficientBalance)));
    }

    #[test]
    fn test_absorb_bad_debt() {
        let (env, client, _admin, _pool, asset) = setup();

        let depositor = Address::generate(&env);
        fund(&env, &asset, &depositor, 10000);
        client.deposit(&depositor, &10000);

        // Simulate bad debt absorption (covered tokens are sent to the pool).
        client.absorb_bad_debt(&5000);

        let state = client.get_state();
        assert_eq!(state.balance, 5000);
    }

    #[test]
    fn test_claim_requires_request_and_delay() {
        let (env, client, _admin, _pool, asset) = setup();

        let depositor = Address::generate(&env);
        fund(&env, &asset, &depositor, 10000);
        let shares = client.deposit(&depositor, &10000);

        // Claim without a prior request fails.
        let no_request = client.try_claim(&depositor, &shares);
        assert_eq!(no_request, Err(Ok(BackstopError::ClaimPending)));

        // Request, but claim too early (delay not elapsed) fails.
        client.request_claim(&depositor, &shares);
        let too_early = client.try_claim(&depositor, &shares);
        assert_eq!(too_early, Err(Ok(BackstopError::ClaimPending)));

        // Advance past the claim delay and claim successfully.
        env.ledger()
            .set_sequence_number(env.ledger().sequence() + client.get_state().claim_delay + 1);
        let payout = client.claim(&depositor, &shares);
        assert!(payout > 0);

        let state = client.get_state();
        assert_eq!(state.total_shares, 0);
    }

    #[test]
    fn test_claim_no_bad_debt() {
        let (env, client, _admin, _pool, asset) = setup();

        let depositor = Address::generate(&env);
        fund(&env, &asset, &depositor, 10000);
        let shares = client.deposit(&depositor, &10000);

        // With only deposits and no absorbed bad debt, claim returns the
        // proportional share of the balance (which equals the deposit).
        client.request_claim(&depositor, &shares);
        env.ledger()
            .set_sequence_number(env.ledger().sequence() + client.get_state().claim_delay + 1);
        let payout = client.claim(&depositor, &shares);
        assert_eq!(payout, 10000);

        let state = client.get_state();
        assert_eq!(state.balance, 0);
    }

    #[test]
    fn test_set_paused() {
        let (env, client, admin, _pool, _asset) = setup();

        client.set_paused(&admin, &true);

        let depositor = Address::generate(&env);
        let result = client.try_deposit(&depositor, &1000);

        assert_eq!(result, Err(Ok(BackstopError::Paused)));
    }

    #[test]
    fn test_set_paused_not_admin() {
        let (env, client, _admin, _pool, _asset) = setup();

        let not_admin = Address::generate(&env);
        let result = client.try_set_paused(&not_admin, &true);

        assert_eq!(result, Err(Ok(BackstopError::NotAdmin)));
    }

    #[test]
    fn test_get_shares() {
        let (env, client, _admin, _pool, asset) = setup();

        let depositor = Address::generate(&env);
        fund(&env, &asset, &depositor, 1000);
        let shares = client.deposit(&depositor, &1000);

        let stored_shares = client.get_shares(&depositor);
        assert_eq!(stored_shares, shares);
    }
}
