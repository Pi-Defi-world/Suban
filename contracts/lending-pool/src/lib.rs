#![no_std]

use soroban_sdk::{
    contract, contractimpl, contracttype, contracterror, symbol_short,
    Address, Env, Map, Symbol, Vec, IntoVal, Val,
};
use hub_errors::HubError;
use hub_types::Price;

const TOTAL_SUPPLIES: Symbol = symbol_short!("totSup");
const TOTAL_BORROWS: Symbol = symbol_short!("totBor");
const COLLATERAL_FACTOR: Symbol = symbol_short!("colFac");
const LIQUIDATION_THRESHOLD: Symbol = symbol_short!("liqThr");
const LIQUIDATION_BONUS: Symbol = symbol_short!("liqBon");
const POOL_ADMIN: Symbol = symbol_short!("admin");
const USER_DEPOSITS: Symbol = symbol_short!("usrDep");
const USER_BORROWS: Symbol = symbol_short!("usrBrw");
const USER_COLLATERAL: Symbol = symbol_short!("usrCol");
const LAST_ACCRUAL: Symbol = symbol_short!("lstAcc");
const INTEREST_RATE: Symbol = symbol_short!("intRate");
const RESERVE_FACTOR: Symbol = symbol_short!("rsrvFc");
const TOTAL_RESERVES: Symbol = symbol_short!("totRsv");
const POOL_PAUSED: Symbol = symbol_short!("paused");
const COLLATERAL_ASSET: Symbol = symbol_short!("colAs");
const COLLATERAL_TOKEN: Symbol = symbol_short!("colTok");
const PRICE_ORACLE: Symbol = symbol_short!("oracle");
const ORACLE_MAX_AGE: Symbol = symbol_short!("orclAge");

#[derive(Clone, PartialEq, Debug)]
#[contracterror]
pub enum PoolError {
    NotAdmin = 1,
    PoolPaused = 2,
    InsufficientBalance = 3,
    InsufficientCollateral = 4,
    HealthFactorBelowOne = 5,
    LiquidationNotProfitable = 6,
    SlippageExceeded = 7,
    ZeroAmount = 8,
    ReserveFactorTooHigh = 9,
    CollateralFactorTooHigh = 10,
    OracleUnavailable = 11,
    InvalidOraclePrice = 12,
}

#[derive(Clone)]
#[contracttype]
pub struct UserPosition {
    pub deposit: i128,
    pub borrow: i128,
    pub collateral: i128,
}

#[derive(Clone)]
#[contracttype]
pub struct PoolConfig {
    pub collateral_factor: i128,
    pub liquidation_threshold: i128,
    pub liquidation_bonus: i128,
    pub interest_rate: i128,
    pub reserve_factor: i128,
    pub collateral_asset: Address,
}

#[derive(Clone)]
#[contracttype]
pub struct PoolState {
    pub total_supply: i128,
    pub total_borrows: i128,
    pub total_reserves: i128,
    pub last_accrual: u32,
}

#[contract]
pub struct LendingPool;

#[contractimpl]
impl LendingPool {
    pub fn initialize(
        env: Env,
        admin: Address,
        collateral_factor: i128,
        liquidation_threshold: i128,
        liquidation_bonus: i128,
        interest_rate: i128,
        reserve_factor: i128,
    ) -> Result<(), PoolError> {
        if collateral_factor > 8000 {
            return Err(PoolError::CollateralFactorTooHigh);
        }
        if reserve_factor > 5000 {
            return Err(PoolError::ReserveFactorTooHigh);
        }

        env.storage().instance().set(&POOL_ADMIN, &admin);
        env.storage().instance().set(&COLLATERAL_FACTOR, &collateral_factor);
        env.storage().instance().set(&LIQUIDATION_THRESHOLD, &liquidation_threshold);
        env.storage().instance().set(&LIQUIDATION_BONUS, &liquidation_bonus);
        env.storage().instance().set(&INTEREST_RATE, &interest_rate);
        env.storage().instance().set(&RESERVE_FACTOR, &reserve_factor);
        env.storage().instance().set(&POOL_PAUSED, &false);
        env.storage().instance().set(&TOTAL_SUPPLIES, &0i128);
        env.storage().instance().set(&TOTAL_BORROWS, &0i128);
        env.storage().instance().set(&TOTAL_RESERVES, &0i128);
        env.storage().instance().set(&LAST_ACCRUAL, &env.ledger().sequence());

        Ok(())
    }

    pub fn deposit(env: Env, from: Address, amount: i128) -> Result<i128, PoolError> {
        if amount <= 0 {
            return Err(PoolError::ZeroAmount);
        }
        Self::require_not_paused(&env)?;
        from.require_auth();

        let total_supply: i128 = env.storage().instance().get(&TOTAL_SUPPLIES).unwrap_or(0);
        let total_borrows: i128 = env.storage().instance().get(&TOTAL_BORROWS).unwrap_or(0);
        let total_reserves: i128 = env.storage().instance().get(&TOTAL_RESERVES).unwrap_or(0);

        let b_tokens = if total_supply == 0 {
            amount
        } else {
            let total_assets = total_supply + total_borrows + total_reserves;
            (amount * total_supply) / total_assets
        };

        let mut user_deposits: Map<Address, i128> = env
            .storage()
            .instance()
            .get(&USER_DEPOSITS)
            .unwrap_or(Map::new(&env));
        let current = user_deposits.get(from.clone()).unwrap_or(0);
        user_deposits.set(from.clone(), current + amount);
        env.storage().instance().set(&USER_DEPOSITS, &user_deposits);

        env.storage().instance().set(&TOTAL_SUPPLIES, &(total_supply + b_tokens));

        // Custody: pull the deposited underlying from the user into the pool.
        let asset = soroban_sdk::token::Client::new(&env, &Self::pool_asset(&env));
        let vault = env.current_contract_address();
        asset.transfer(&from, &vault, &amount);

        let (deposit_after, borrow_after, collateral_after) =
            Self::read_user_position(&env, &from);
        let (total_supply_after, total_borrows_after, total_reserves_after) =
            Self::read_pool_totals(&env);
        env.events().publish(
            (symbol_short!("deposit"), &from),
            (
                amount,
                b_tokens,
                deposit_after,
                borrow_after,
                collateral_after,
                total_supply_after,
                total_borrows_after,
                total_reserves_after,
            ),
        );

        Ok(b_tokens)
    }

    pub fn withdraw(env: Env, from: Address, b_token_amount: i128) -> Result<i128, PoolError> {
        if b_token_amount <= 0 {
            return Err(PoolError::ZeroAmount);
        }
        Self::require_not_paused(&env)?;
        from.require_auth();

        let total_supply: i128 = env.storage().instance().get(&TOTAL_SUPPLIES).unwrap_or(0);
        let total_borrows: i128 = env.storage().instance().get(&TOTAL_BORROWS).unwrap_or(0);
        let total_reserves: i128 = env.storage().instance().get(&TOTAL_RESERVES).unwrap_or(0);

        let total_assets = total_supply + total_borrows + total_reserves;
        let underlying = (b_token_amount * total_assets) / total_supply;

        let user_deposits: Map<Address, i128> = env
            .storage()
            .instance()
            .get(&USER_DEPOSITS)
            .unwrap_or(Map::new(&env));
        let user_balance = user_deposits.get(from.clone()).unwrap_or(0);
        if user_balance < b_token_amount {
            return Err(PoolError::InsufficientBalance);
        }

        let health = Self::calculate_health_factor(&env, &from)?;
        if health < 10000 {
            return Err(PoolError::HealthFactorBelowOne);
        }

        let mut user_deposits_mut: Map<Address, i128> = env
            .storage()
            .instance()
            .get(&USER_DEPOSITS)
            .unwrap_or(Map::new(&env));
        user_deposits_mut.set(from.clone(), user_balance - b_token_amount);
        env.storage().instance().set(&USER_DEPOSITS, &user_deposits_mut);

        env.storage().instance().set(&TOTAL_SUPPLIES, &(total_supply - b_token_amount));

        // Custody: return the underlying to the user.
        let asset = soroban_sdk::token::Client::new(&env, &Self::pool_asset(&env));
        let vault = env.current_contract_address();
        asset.transfer(&vault, &from, &underlying);

        let (deposit_after, borrow_after, collateral_after) =
            Self::read_user_position(&env, &from);
        let (total_supply_after, total_borrows_after, total_reserves_after) =
            Self::read_pool_totals(&env);
        env.events().publish(
            (symbol_short!("withdraw"), &from),
            (
                b_token_amount,
                underlying,
                deposit_after,
                borrow_after,
                collateral_after,
                total_supply_after,
                total_borrows_after,
                total_reserves_after,
            ),
        );

        Ok(underlying)
    }

    pub fn borrow(env: Env, from: Address, amount: i128) -> Result<(), PoolError> {
        if amount <= 0 {
            return Err(PoolError::ZeroAmount);
        }
        Self::require_not_paused(&env)?;
        from.require_auth();

        let total_supply: i128 = env.storage().instance().get(&TOTAL_SUPPLIES).unwrap_or(0);
        let total_borrows: i128 = env.storage().instance().get(&TOTAL_BORROWS).unwrap_or(0);

        if amount > total_supply - total_borrows {
            return Err(PoolError::ZeroAmount);
        }

        let user_borrows: Map<Address, i128> = env
            .storage()
            .instance()
            .get(&USER_BORROWS)
            .unwrap_or(Map::new(&env));
        let current_borrow = user_borrows.get(from.clone()).unwrap_or(0);

        let mut user_borrows_mut = user_borrows.clone();
        user_borrows_mut.set(from.clone(), current_borrow + amount);
        env.storage().instance().set(&USER_BORROWS, &user_borrows_mut);

        let health = Self::calculate_health_factor(&env, &from)?;
        if health < 10000 {
            user_borrows_mut.set(from, current_borrow);
            env.storage().instance().set(&USER_BORROWS, &user_borrows_mut);
            return Err(PoolError::HealthFactorBelowOne);
        }

        env.storage().instance().set(&TOTAL_BORROWS, &(total_borrows + amount));

        // Custody: send the borrowed underlying to the borrower.
        let asset = soroban_sdk::token::Client::new(&env, &Self::pool_asset(&env));
        let vault = env.current_contract_address();
        asset.transfer(&vault, &from, &amount);

        let (deposit_after, borrow_after, collateral_after) =
            Self::read_user_position(&env, &from);
        let (total_supply_after, total_borrows_after, total_reserves_after) =
            Self::read_pool_totals(&env);
        env.events().publish(
            (symbol_short!("borrow"), &from),
            (
                amount,
                deposit_after,
                borrow_after,
                collateral_after,
                total_supply_after,
                total_borrows_after,
                total_reserves_after,
            ),
        );

        Ok(())
    }

    pub fn repay(env: Env, from: Address, amount: i128) -> Result<i128, PoolError> {
        if amount <= 0 {
            return Err(PoolError::ZeroAmount);
        }
        Self::require_not_paused(&env)?;
        from.require_auth();

        let user_borrows: Map<Address, i128> = env
            .storage()
            .instance()
            .get(&USER_BORROWS)
            .unwrap_or(Map::new(&env));
        let current_borrow = user_borrows.get(from.clone()).unwrap_or(0);

        let repay_amount = amount.min(current_borrow);

        let mut user_borrows_mut = user_borrows;
        user_borrows_mut.set(from.clone(), current_borrow - repay_amount);
        env.storage().instance().set(&USER_BORROWS, &user_borrows_mut);

        let total_borrows: i128 = env.storage().instance().get(&TOTAL_BORROWS).unwrap_or(0);
        env.storage().instance().set(&TOTAL_BORROWS, &(total_borrows - repay_amount));

        // Custody: pull the repaid underlying from the user into the pool.
        let asset = soroban_sdk::token::Client::new(&env, &Self::pool_asset(&env));
        let vault = env.current_contract_address();
        asset.transfer(&from, &vault, &repay_amount);

        let (deposit_after, borrow_after, collateral_after) =
            Self::read_user_position(&env, &from);
        let (total_supply_after, total_borrows_after, total_reserves_after) =
            Self::read_pool_totals(&env);
        env.events().publish(
            (symbol_short!("repay"), &from),
            (
                repay_amount,
                deposit_after,
                borrow_after,
                collateral_after,
                total_supply_after,
                total_borrows_after,
                total_reserves_after,
            ),
        );

        Ok(repay_amount)
    }

    pub fn set_collateral(env: Env, user: Address, amount: i128) -> Result<(), PoolError> {
        Self::require_not_paused(&env)?;
        user.require_auth();

        let user_deposits: Map<Address, i128> = env
            .storage()
            .instance()
            .get(&USER_DEPOSITS)
            .unwrap_or(Map::new(&env));
        let deposit = user_deposits.get(user.clone()).unwrap_or(0);

        if amount > deposit {
            return Err(PoolError::InsufficientBalance);
        }

        let mut user_collateral: Map<Address, i128> = env
            .storage()
            .instance()
            .get(&USER_COLLATERAL)
            .unwrap_or(Map::new(&env));
        user_collateral.set(user.clone(), amount);
        env.storage().instance().set(&USER_COLLATERAL, &user_collateral);

        let health = Self::calculate_health_factor(&env, &user)?;
        if health < 10000 {
            return Err(PoolError::HealthFactorBelowOne);
        }

        let (deposit_after, borrow_after, collateral_after) =
            Self::read_user_position(&env, &user);
        let (total_supply_after, total_borrows_after, total_reserves_after) =
            Self::read_pool_totals(&env);
        env.events().publish(
            (symbol_short!("set_col"), &user),
            (
                amount,
                deposit_after,
                borrow_after,
                collateral_after,
                total_supply_after,
                total_borrows_after,
                total_reserves_after,
            ),
        );

        Ok(())
    }

    pub fn liquidate(
        env: Env,
        liquidator: Address,
        borrower: Address,
        repay_amount: i128,
        min_collateral: i128,
    ) -> Result<(), PoolError> {
        if repay_amount <= 0 {
            return Err(PoolError::ZeroAmount);
        }
        Self::require_not_paused(&env)?;
        liquidator.require_auth();

        let health = Self::calculate_health_factor(&env, &borrower)?;
        if health >= 10000 {
            return Err(PoolError::LiquidationNotProfitable);
        }

        let user_borrows: Map<Address, i128> = env
            .storage()
            .instance()
            .get(&USER_BORROWS)
            .unwrap_or(Map::new(&env));
        let borrower_borrow = user_borrows.get(borrower.clone()).unwrap_or(0);

        let actual_repay = repay_amount.min(borrower_borrow);

        let liquidation_bonus: i128 = env.storage().instance().get(&LIQUIDATION_BONUS).unwrap_or(500);
        let collateral_seize = (actual_repay * (10000 + liquidation_bonus)) / 10000;

        if collateral_seize < min_collateral {
            return Err(PoolError::SlippageExceeded);
        }

        let user_collateral: Map<Address, i128> = env
            .storage()
            .instance()
            .get(&USER_COLLATERAL)
            .unwrap_or(Map::new(&env));
        let borrower_collateral = user_collateral.get(borrower.clone()).unwrap_or(0);

        if collateral_seize > borrower_collateral {
            return Err(PoolError::InsufficientCollateral);
        }

        let mut user_borrows_mut = user_borrows;
        user_borrows_mut.set(borrower.clone(), borrower_borrow - actual_repay);
        env.storage().instance().set(&USER_BORROWS, &user_borrows_mut);

        let mut user_collateral_mut = user_collateral;
        user_collateral_mut.set(borrower.clone(), borrower_collateral - collateral_seize);
        env.storage().instance().set(&USER_COLLATERAL, &user_collateral_mut);

        let total_borrows: i128 = env.storage().instance().get(&TOTAL_BORROWS).unwrap_or(0);
        env.storage().instance().set(&TOTAL_BORROWS, &(total_borrows - actual_repay));

        // Custody: liquidator repays the debt; seized collateral is sent to them.
        let debt_asset = Self::pool_asset(&env);
        let collateral_asset = Self::collateral_token(&env);
        let vault = env.current_contract_address();
        soroban_sdk::token::Client::new(&env, &debt_asset).transfer(
            &liquidator,
            &vault,
            &actual_repay,
        );
        soroban_sdk::token::Client::new(&env, &collateral_asset).transfer(
            &vault,
            &liquidator,
            &collateral_seize,
        );

        let (borrower_deposit_after, borrower_borrow_after, borrower_collateral_after) =
            Self::read_user_position(&env, &borrower);
        let (total_supply_after, total_borrows_after, total_reserves_after) =
            Self::read_pool_totals(&env);
        env.events().publish(
            (symbol_short!("liquidate"), &liquidator, &borrower),
            (
                actual_repay,
                collateral_seize,
                borrower_deposit_after,
                borrower_borrow_after,
                borrower_collateral_after,
                total_supply_after,
                total_borrows_after,
                total_reserves_after,
            ),
        );

        Ok(())
    }

    pub fn accrue_interest(env: Env) -> Result<(), PoolError> {
        Self::require_not_paused(&env)?;

        let last_accrual: u32 = env.storage().instance().get(&LAST_ACCRUAL).unwrap_or(0);
        let current_ledger = env.ledger().sequence();
        let ledgers_passed = current_ledger.checked_sub(last_accrual).unwrap_or(0);

        if ledgers_passed == 0 {
            return Ok(());
        }

        let total_borrows: i128 = env.storage().instance().get(&TOTAL_BORROWS).unwrap_or(0);
        let interest_rate: i128 = env.storage().instance().get(&INTEREST_RATE).unwrap_or(500);
        let reserve_factor: i128 = env.storage().instance().get(&RESERVE_FACTOR).unwrap_or(1000);

        let interest = (total_borrows * interest_rate * ledgers_passed as i128) / (10000 * 100000);
        let reserve_share = (interest * reserve_factor) / 10000;

        let total_borrows_new = total_borrows + interest;
        let total_reserves: i128 = env.storage().instance().get(&TOTAL_RESERVES).unwrap_or(0);

        env.storage().instance().set(&TOTAL_BORROWS, &total_borrows_new);
        env.storage().instance().set(&TOTAL_RESERVES, &(total_reserves + reserve_share));
        env.storage().instance().set(&LAST_ACCRUAL, &current_ledger);

        env.events().publish(
            (symbol_short!("accrue"),),
            (
                interest,
                reserve_share,
                total_borrows_new,
                total_reserves + reserve_share,
                current_ledger,
            ),
        );

        Ok(())
    }

    pub fn get_position(env: Env, user: Address) -> UserPosition {
        let user_deposits: Map<Address, i128> = env
            .storage()
            .instance()
            .get(&USER_DEPOSITS)
            .unwrap_or(Map::new(&env));
        let user_borrows: Map<Address, i128> = env
            .storage()
            .instance()
            .get(&USER_BORROWS)
            .unwrap_or(Map::new(&env));
        let user_collateral: Map<Address, i128> = env
            .storage()
            .instance()
            .get(&USER_COLLATERAL)
            .unwrap_or(Map::new(&env));

        UserPosition {
            deposit: user_deposits.get(user.clone()).unwrap_or(0),
            borrow: user_borrows.get(user.clone()).unwrap_or(0),
            collateral: user_collateral.get(user).unwrap_or(0),
        }
    }

    pub fn get_health_factor(env: Env, user: Address) -> i128 {
        Self::calculate_health_factor(&env, &user).unwrap_or(0)
    }

    pub fn get_pool_state(env: Env) -> PoolState {
        PoolState {
            total_supply: env.storage().instance().get(&TOTAL_SUPPLIES).unwrap_or(0),
            total_borrows: env.storage().instance().get(&TOTAL_BORROWS).unwrap_or(0),
            total_reserves: env.storage().instance().get(&TOTAL_RESERVES).unwrap_or(0),
            last_accrual: env.storage().instance().get(&LAST_ACCRUAL).unwrap_or(0),
        }
    }

    pub fn get_pool_config(env: Env) -> PoolConfig {
        PoolConfig {
            collateral_factor: env.storage().instance().get(&COLLATERAL_FACTOR).unwrap_or(8000),
            liquidation_threshold: env.storage().instance().get(&LIQUIDATION_THRESHOLD).unwrap_or(8500),
            liquidation_bonus: env.storage().instance().get(&LIQUIDATION_BONUS).unwrap_or(500),
            interest_rate: env.storage().instance().get(&INTEREST_RATE).unwrap_or(500),
            reserve_factor: env.storage().instance().get(&RESERVE_FACTOR).unwrap_or(1000),
            collateral_asset: Self::collateral_token(&env),
        }
    }

    pub fn get_collateral_token(env: Env) -> Address {
        Self::collateral_token(&env)
    }

    pub fn get_price_oracle(env: Env) -> Option<Address> {
        Self::price_oracle(&env)
    }

    pub fn set_paused(env: Env, admin: Address, paused: bool) -> Result<(), PoolError> {
        let stored_admin: Address = env.storage().instance().get(&POOL_ADMIN).unwrap();
        if admin != stored_admin {
            return Err(PoolError::NotAdmin);
        }
        admin.require_auth();
        env.storage().instance().set(&POOL_PAUSED, &paused);
        Ok(())
    }

    pub fn update_config(
        env: Env,
        admin: Address,
        collateral_factor: Option<i128>,
        liquidation_threshold: Option<i128>,
        liquidation_bonus: Option<i128>,
        interest_rate: Option<i128>,
        reserve_factor: Option<i128>,
    ) -> Result<(), PoolError> {
        let stored_admin: Address = env.storage().instance().get(&POOL_ADMIN).unwrap();
        if admin != stored_admin {
            return Err(PoolError::NotAdmin);
        }
        admin.require_auth();

        if let Some(cf) = collateral_factor {
            if cf > 8000 {
                return Err(PoolError::CollateralFactorTooHigh);
            }
            env.storage().instance().set(&COLLATERAL_FACTOR, &cf);
        }
        if let Some(lt) = liquidation_threshold {
            env.storage().instance().set(&LIQUIDATION_THRESHOLD, &lt);
        }
        if let Some(lb) = liquidation_bonus {
            env.storage().instance().set(&LIQUIDATION_BONUS, &lb);
        }
        if let Some(ir) = interest_rate {
            env.storage().instance().set(&INTEREST_RATE, &ir);
        }
        if let Some(rf) = reserve_factor {
            if rf > 5000 {
                return Err(PoolError::ReserveFactorTooHigh);
            }
            env.storage().instance().set(&RESERVE_FACTOR, &rf);
        }

        Ok(())
    }

    /// Set the collateral asset (e.g., PUSD) for this pool
    pub fn set_collateral_asset(env: Env, admin: Address, asset: Address) -> Result<(), PoolError> {
        let stored_admin: Address = env.storage().instance().get(&POOL_ADMIN).unwrap();
        if admin != stored_admin {
            return Err(PoolError::NotAdmin);
        }
        admin.require_auth();
        env.storage().instance().set(&COLLATERAL_ASSET, &asset);
        Ok(())
    }

    /// Optional: use a distinct token as collateral. When unset, the pool's
    /// debt/deposit asset is also the collateral asset (legacy behaviour).
    pub fn set_collateral_token(env: Env, admin: Address, asset: Address) -> Result<(), PoolError> {
        let stored_admin: Address = env.storage().instance().get(&POOL_ADMIN).unwrap();
        if admin != stored_admin {
            return Err(PoolError::NotAdmin);
        }
        admin.require_auth();
        env.storage().instance().set(&COLLATERAL_TOKEN, &asset);
        Ok(())
    }

    /// Register an optional price oracle and maximum age in ledgers. When a
    /// distinct collateral token is configured, the oracle is required.
    pub fn set_price_oracle(
        env: Env,
        admin: Address,
        oracle: Address,
        max_age_ledgers: u64,
    ) -> Result<(), PoolError> {
        let stored_admin: Address = env.storage().instance().get(&POOL_ADMIN).unwrap();
        if admin != stored_admin {
            return Err(PoolError::NotAdmin);
        }
        admin.require_auth();
        if max_age_ledgers == 0 {
            return Err(PoolError::InvalidOraclePrice);
        }
        env.storage().instance().set(&PRICE_ORACLE, &oracle);
        env.storage().instance().set(&ORACLE_MAX_AGE, &max_age_ledgers);
        Ok(())
    }

    pub fn clear_price_oracle(env: Env, admin: Address) -> Result<(), PoolError> {
        let stored_admin: Address = env.storage().instance().get(&POOL_ADMIN).unwrap();
        if admin != stored_admin {
            return Err(PoolError::NotAdmin);
        }
        admin.require_auth();
        env.storage().instance().remove(&PRICE_ORACLE);
        Ok(())
    }

    /// Deposit a distinct collateral token. For single-asset pools, callers can
    /// continue to use `set_collateral` after depositing the debt asset.
    pub fn deposit_collateral(env: Env, from: Address, amount: i128) -> Result<(), PoolError> {
        if amount <= 0 {
            return Err(PoolError::ZeroAmount);
        }
        Self::require_not_paused(&env)?;
        from.require_auth();

        let asset = Self::collateral_token(&env);
        soroban_sdk::token::Client::new(&env, &asset).transfer(
            &from,
            &env.current_contract_address(),
            &amount,
        );

        let mut user_collateral: Map<Address, i128> = env
            .storage()
            .instance()
            .get(&USER_COLLATERAL)
            .unwrap_or(Map::new(&env));
        let current = user_collateral.get(from.clone()).unwrap_or(0);
        user_collateral.set(from.clone(), current + amount);
        env.storage().instance().set(&USER_COLLATERAL, &user_collateral);

        let (deposit_after, borrow_after, collateral_after) =
            Self::read_user_position(&env, &from);
        let (total_supply_after, total_borrows_after, total_reserves_after) =
            Self::read_pool_totals(&env);
        env.events().publish(
            (symbol_short!("dep_col"), &from),
            (
                amount,
                deposit_after,
                borrow_after,
                collateral_after,
                total_supply_after,
                total_borrows_after,
                total_reserves_after,
            ),
        );
        Ok(())
    }

    /// Withdraw a distinct collateral token. The health factor is checked after
    /// the withdrawal; a position cannot withdraw into under-collateralisation.
    pub fn withdraw_collateral(env: Env, from: Address, amount: i128) -> Result<(), PoolError> {
        if amount <= 0 {
            return Err(PoolError::ZeroAmount);
        }
        Self::require_not_paused(&env)?;
        from.require_auth();

        let mut user_collateral: Map<Address, i128> = env
            .storage()
            .instance()
            .get(&USER_COLLATERAL)
            .unwrap_or(Map::new(&env));
        let current = user_collateral.get(from.clone()).unwrap_or(0);
        if amount > current {
            return Err(PoolError::InsufficientCollateral);
        }
        user_collateral.set(from.clone(), current - amount);
        env.storage().instance().set(&USER_COLLATERAL, &user_collateral);

        let health = Self::calculate_health_factor(&env, &from)?;
        if health < 10000 {
            user_collateral.set(from.clone(), current);
            env.storage().instance().set(&USER_COLLATERAL, &user_collateral);
            return Err(PoolError::HealthFactorBelowOne);
        }

        let asset = Self::collateral_token(&env);
        soroban_sdk::token::Client::new(&env, &asset).transfer(
            &env.current_contract_address(),
            &from,
            &amount,
        );

        let (deposit_after, borrow_after, collateral_after) =
            Self::read_user_position(&env, &from);
        let (total_supply_after, total_borrows_after, total_reserves_after) =
            Self::read_pool_totals(&env);
        env.events().publish(
            (symbol_short!("wd_col"), &from),
            (
                amount,
                deposit_after,
                borrow_after,
                collateral_after,
                total_supply_after,
                total_borrows_after,
                total_reserves_after,
            ),
        );
        Ok(())
    }

    fn require_not_paused(env: &Env) -> Result<(), PoolError> {
        let paused: bool = env.storage().instance().get(&POOL_PAUSED).unwrap_or(false);
        if paused {
            return Err(PoolError::PoolPaused);
        }
        Ok(())
    }

    /// The pool's underlying asset (deposits, borrows, and legacy same-asset
    /// collateral are denominated in this asset).
    fn pool_asset(env: &Env) -> Address {
        env.storage()
            .instance()
            .get(&COLLATERAL_ASSET)
            .unwrap_or_else(|| env.current_contract_address())
    }

    /// Collateral token. Defaults to the pool asset for compatibility with the
    /// original single-asset deployment.
    fn collateral_token(env: &Env) -> Address {
        env.storage()
            .instance()
            .get(&COLLATERAL_TOKEN)
            .unwrap_or_else(|| Self::pool_asset(env))
    }

    fn price_oracle(env: &Env) -> Option<Address> {
        env.storage().instance().get(&PRICE_ORACLE)
    }

    fn read_oracle_price(env: &Env, oracle: &Address, asset: &Address) -> Result<Price, PoolError> {
        let args: Vec<Val> = Vec::from_array(env, [asset.clone().into_val(env)]);
        let result: Result<Price, HubError> =
            env.invoke_contract(oracle, &Symbol::new(env, "get_price"), args);
        let price = result.map_err(|_| PoolError::OracleUnavailable)?;
        if price.price <= 0 {
            return Err(PoolError::InvalidOraclePrice);
        }
        let now = env.ledger().sequence() as u64;
        if price.timestamp > now {
            return Err(PoolError::InvalidOraclePrice);
        }
        let max_age: u64 = env.storage().instance().get(&ORACLE_MAX_AGE).unwrap_or(10);
        if now.saturating_sub(price.timestamp) > max_age {
            return Err(PoolError::OracleUnavailable);
        }
        Ok(price)
    }

    fn read_user_position(env: &Env, user: &Address) -> (i128, i128, i128) {
        let user_deposits: Map<Address, i128> = env
            .storage()
            .instance()
            .get(&USER_DEPOSITS)
            .unwrap_or(Map::new(env));
        let user_borrows: Map<Address, i128> = env
            .storage()
            .instance()
            .get(&USER_BORROWS)
            .unwrap_or(Map::new(env));
        let user_collateral: Map<Address, i128> = env
            .storage()
            .instance()
            .get(&USER_COLLATERAL)
            .unwrap_or(Map::new(env));

        (
            user_deposits.get(user.clone()).unwrap_or(0),
            user_borrows.get(user.clone()).unwrap_or(0),
            user_collateral.get(user.clone()).unwrap_or(0),
        )
    }

    fn read_pool_totals(env: &Env) -> (i128, i128, i128) {
        (
            env.storage().instance().get(&TOTAL_SUPPLIES).unwrap_or(0),
            env.storage().instance().get(&TOTAL_BORROWS).unwrap_or(0),
            env.storage().instance().get(&TOTAL_RESERVES).unwrap_or(0),
        )
    }

    fn calculate_health_factor(env: &Env, user: &Address) -> Result<i128, PoolError> {
        let user_collateral: Map<Address, i128> = env
            .storage()
            .instance()
            .get(&USER_COLLATERAL)
            .unwrap_or(Map::new(&env));
        let user_borrows: Map<Address, i128> = env
            .storage()
            .instance()
            .get(&USER_BORROWS)
            .unwrap_or(Map::new(&env));

        let collateral = user_collateral.get(user.clone()).unwrap_or(0);
        let borrow = user_borrows.get(user.clone()).unwrap_or(0);

        if borrow == 0 {
            return Ok(i128::MAX);
        }

        let liquidation_threshold: i128 = env
            .storage()
            .instance()
            .get(&LIQUIDATION_THRESHOLD)
            .unwrap_or(8500);

        let collateral_asset = Self::collateral_token(env);
        let debt_asset = Self::pool_asset(env);

        let collateral_value;
        let borrow_value;
        if let Some(oracle) = Self::price_oracle(env) {
            let collateral_price = Self::read_oracle_price(env, &oracle, &collateral_asset)?;
            let debt_price = if collateral_asset == debt_asset {
                collateral_price.clone()
            } else {
                Self::read_oracle_price(env, &oracle, &debt_asset)?
            };
            if collateral_price.decimals != debt_price.decimals {
                return Err(PoolError::InvalidOraclePrice);
            }
            collateral_value = collateral
                .checked_mul(collateral_price.price)
                .and_then(|value| value.checked_mul(liquidation_threshold))
                .ok_or(PoolError::InvalidOraclePrice)?;
            borrow_value = borrow
                .checked_mul(debt_price.price)
                .and_then(|value| value.checked_mul(10000))
                .ok_or(PoolError::InvalidOraclePrice)?;
        } else {
            if collateral_asset != debt_asset {
                return Err(PoolError::OracleUnavailable);
            }
            collateral_value = collateral * liquidation_threshold;
            borrow_value = borrow * 10000;
        }

        // Return the health factor in basis points (10_000 = 1.0), matching
        // the callers that compare against 10_000.
        Ok((collateral_value * 10000) / borrow_value)
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use soroban_sdk::testutils::Address as _;
    use soroban_sdk::testutils::Events as _;
    use soroban_sdk::testutils::Ledger as _;
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

        pub fn balance(env: Env, owner: Address) -> i128 {
            env.storage()
                .instance()
                .get::<MockKey, i128>(&MockKey::Bal(owner))
                .unwrap_or(0)
        }
    }

    fn setup() -> (Env, LendingPoolClient<'static>, Address) {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(LendingPool, ());
        let client = LendingPoolClient::new(&env, &contract_id);
        let admin = Address::generate(&env);

        client.initialize(&admin, &8000, &8500, &500, &500, &1000);

        (env, client, admin)
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

    fn setup_with_token() -> (Env, LendingPoolClient<'static>, Address, Address) {
        let (env, client, admin) = setup();
        let token = env.register(MockToken, ());
        client.set_collateral_asset(&admin, &token);
        let user = Address::generate(&env);
        MockTokenClient::new(&env, &token).mint(&user, &10_000);
        (env, client, user, token)
    }

    #[test]
    fn test_deposit_and_borrow_events_carry_position_snapshots() {
        let (env, client, user, _token) = setup_with_token();

        let b_tokens = client.deposit(&user, &1000);
        assert_eq!(b_tokens, 1000);

        let (topics, data) = last_event(&env);
        let name = Symbol::try_from_val(&env, &topics.get(0).unwrap()).unwrap();
        assert_eq!(name, symbol_short!("deposit"));
        let payload =
            <(i128, i128, i128, i128, i128, i128, i128, i128)>::try_from_val(&env, &data)
                .unwrap();
        assert_eq!(payload.0, 1000);
        assert_eq!(payload.1, 1000);
        assert_eq!(payload.2, 1000);
        assert_eq!(payload.3, 0);
        assert_eq!(payload.4, 0);
        assert_eq!(payload.5, 1000);
        assert_eq!(payload.6, 0);
        assert_eq!(payload.7, 0);

        client.set_collateral(&user, &500);
        client.borrow(&user, &100);

        let (topics, data) = last_event(&env);
        let name = Symbol::try_from_val(&env, &topics.get(0).unwrap()).unwrap();
        assert_eq!(name, symbol_short!("borrow"));
        let payload =
            <(i128, i128, i128, i128, i128, i128, i128)>::try_from_val(&env, &data).unwrap();
        assert_eq!(payload.0, 100);
        assert_eq!(payload.1, 1000);
        assert_eq!(payload.2, 100);
        assert_eq!(payload.3, 500);
        assert_eq!(payload.4, 1000);
        assert_eq!(payload.5, 100);
        assert_eq!(payload.6, 0);
        assert_eq!(client.get_health_factor(&user), 42_500);

        let position = client.get_position(&user);
        assert_eq!(position.deposit, 1000);
        assert_eq!(position.borrow, 100);
        assert_eq!(position.collateral, 500);
    }

    #[test]
    fn test_initialize() {
        let (_env, client, admin) = setup();

        let config = client.get_pool_config();
        assert_eq!(config.collateral_factor, 8000);
        assert_eq!(config.liquidation_threshold, 8500);
        assert_eq!(config.liquidation_bonus, 500);
        assert_eq!(config.interest_rate, 500);
        assert_eq!(config.reserve_factor, 1000);

        let state = client.get_pool_state();
        assert_eq!(state.total_supply, 0);
        assert_eq!(state.total_borrows, 0);
        assert_eq!(state.total_reserves, 0);
    }

    #[test]
    fn test_collateral_factor_too_high() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(LendingPool, ());
        let client = LendingPoolClient::new(&env, &contract_id);
        let admin = Address::generate(&env);

        let result = client.try_initialize(&admin, &9000, &8500, &500, &500, &1000);
        assert_eq!(result, Err(Ok(PoolError::CollateralFactorTooHigh)));
    }

    #[test]
    fn test_pool_paused() {
        let (env, client, admin) = setup();

        client.set_paused(&admin, &true);

        let user = Address::generate(&env);
        let result = client.try_deposit(&user, &1000);
        assert_eq!(result, Err(Ok(PoolError::PoolPaused)));
    }

    #[test]
    fn test_zero_amount_deposit() {
        let (env, client, _admin) = setup();

        let user = Address::generate(&env);
        let result = client.try_deposit(&user, &0);
        assert_eq!(result, Err(Ok(PoolError::ZeroAmount)));
    }

    #[test]
    fn test_set_paused_not_admin() {
        let (env, client, _admin) = setup();

        let not_admin = Address::generate(&env);
        let result = client.try_set_paused(&not_admin, &true);
        assert_eq!(result, Err(Ok(PoolError::NotAdmin)));
    }

    #[test]
    fn test_update_config_collateral_factor_too_high() {
        let (env, client, admin) = setup();

        let result = client.try_update_config(&admin, &Some(9000), &None, &None, &None, &None);
        assert_eq!(result, Err(Ok(PoolError::CollateralFactorTooHigh)));
    }

    #[test]
    fn test_accrue_interest_no_change() {
        let (env, client, _admin) = setup();

        client.accrue_interest();

        let state = client.get_pool_state();
        assert_eq!(state.total_borrows, 0);
        assert_eq!(state.total_reserves, 0);
    }

    #[test]
    fn test_get_health_factor_no_borrows() {
        let (env, client, _admin) = setup();

        let user = Address::generate(&env);
        let health = client.get_health_factor(&user);
        assert_eq!(health, i128::MAX);
    }

    #[test]
    fn test_get_position_empty() {
        let (env, client, _admin) = setup();

        let user = Address::generate(&env);
        let position = client.get_position(&user);
        assert_eq!(position.deposit, 0);
        assert_eq!(position.borrow, 0);
        assert_eq!(position.collateral, 0);
    }

    #[test]
    fn test_set_collateral_asset() {
        let (env, client, admin) = setup();

        let pusd_asset = Address::generate(&env);
        client.set_collateral_asset(&admin, &pusd_asset);

        let config = client.get_pool_config();
        assert_eq!(config.collateral_asset, pusd_asset);
    }

    #[test]
    fn test_set_collateral_asset_not_admin() {
        let (env, client, _admin) = setup();

        let not_admin = Address::generate(&env);
        let pusd_asset = Address::generate(&env);
        let result = client.try_set_collateral_asset(&not_admin, &pusd_asset);
        assert_eq!(result, Err(Ok(PoolError::NotAdmin)));
    }

    #[soroban_sdk::contract]
    struct MockOracle;

    #[soroban_sdk::contracttype]
    enum OracleKey {
        Price(Address),
    }

    #[soroban_sdk::contractimpl]
    impl MockOracle {
        pub fn set_price(env: Env, asset: Address, price: i128, decimals: u32) {
            let data = (price, decimals, env.ledger().sequence() as u64);
            env.storage()
                .instance()
                .set(&OracleKey::Price(asset), &data);
        }

        pub fn get_price(env: Env, asset: Address) -> Result<Price, HubError> {
            let (price, decimals, timestamp): (i128, u32, u64) = env
                .storage()
                .instance()
                .get(&OracleKey::Price(asset.clone()))
                .unwrap();
            Ok(Price {
                asset,
                price,
                decimals,
                timestamp,
                confidence: 100,
            })
        }
    }

    #[test]
    fn test_oracle_prices_distinct_collateral_and_liquidation() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(LendingPool, ());
        let client = LendingPoolClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin, &8000, &8500, &500, &500, &1000);

        let debt = env.register(MockToken, ());
        let collateral = env.register(MockToken, ());
        let oracle = env.register(MockOracle, ());
        client.set_collateral_asset(&admin, &debt);
        client.set_collateral_token(&admin, &collateral);
        client.set_price_oracle(&admin, &oracle, &100);

        let oracle_client = MockOracleClient::new(&env, &oracle);
        oracle_client.set_price(&debt, &1_0000000, &7);
        oracle_client.set_price(&collateral, &2_0000000, &7);

        let debt_token = MockTokenClient::new(&env, &debt);
        let collateral_token = MockTokenClient::new(&env, &collateral);

        let lender = Address::generate(&env);
        debt_token.mint(&lender, &10_000);
        client.deposit(&lender, &10_000);

        let borrower = Address::generate(&env);
        collateral_token.mint(&borrower, &1_000);
        client.deposit_collateral(&borrower, &1_000);
        client.borrow(&borrower, &500);

        // 1,000 collateral * $2 * 8,500 threshold / (500 debt * $1) = 34,000 bps.
        assert_eq!(client.get_health_factor(&borrower), 34_000);

        // Collateral stays at $2; debt rises to $4. Health falls below 10,000.
        oracle_client.set_price(&debt, &4_0000000, &7);
        assert_eq!(client.get_health_factor(&borrower), 8_500);

        let liquidator = Address::generate(&env);
        debt_token.mint(&liquidator, &1_000);
        client.liquidate(&liquidator, &borrower, &500, &0);

        // 5% liquidation bonus: 500 debt seizes 525 collateral.
        assert_eq!(collateral_token.balance(&liquidator), 525);
        let position = client.get_position(&borrower);
        assert_eq!(position.borrow, 0);
        assert_eq!(position.collateral, 475);
        assert_eq!(debt_token.balance(&liquidator), 500);
    }

    #[test]
    fn test_distinct_collateral_requires_oracle() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(LendingPool, ());
        let client = LendingPoolClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin, &8000, &8500, &500, &500, &1000);

        let debt = env.register(MockToken, ());
        let collateral = env.register(MockToken, ());
        client.set_collateral_asset(&admin, &debt);
        client.set_collateral_token(&admin, &collateral);

        let debt_token = MockTokenClient::new(&env, &debt);
        let collateral_token = MockTokenClient::new(&env, &collateral);
        let lender = Address::generate(&env);
        debt_token.mint(&lender, &1_000);
        client.deposit(&lender, &1_000);
        let borrower = Address::generate(&env);
        collateral_token.mint(&borrower, &1_000);
        client.deposit_collateral(&borrower, &1_000);

        assert_eq!(
            client.try_borrow(&borrower, &100),
            Err(Ok(PoolError::OracleUnavailable))
        );
    }

    #[test]
    fn test_oracle_stale_price_rejected() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(LendingPool, ());
        let client = LendingPoolClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin, &8000, &8500, &500, &500, &1000);
        let oracle = env.register(MockOracle, ());
        client.set_price_oracle(&admin, &oracle, &1);

        let asset = env.register(MockToken, ());
        client.set_collateral_asset(&admin, &asset);
        let oracle_client = MockOracleClient::new(&env, &oracle);
        oracle_client.set_price(&asset, &1_0000000, &7);

        let token = MockTokenClient::new(&env, &asset);
        let user = Address::generate(&env);
        token.mint(&user, &1_000);
        client.deposit(&user, &1_000);
        client.set_collateral(&user, &1_000);

        // Advance the ledger beyond the configured one-ledger age.
        env.ledger().set_sequence_number(env.ledger().sequence() + 2);

        assert_eq!(
            client.try_borrow(&user, &100),
            Err(Ok(PoolError::OracleUnavailable))
        );
    }
}
