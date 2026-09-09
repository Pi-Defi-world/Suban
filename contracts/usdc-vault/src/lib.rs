#![no_std]

//! USDC Bridge Vault — holds real USDC from the Stellar Asset Contract.
//!
//! This contract provides:
//! - Deposit USDC (for bridging from Stellar -> Pi)
//! - Release USDC (for redeeming wPi -> Stellar USDC)
//! - Admin controls for pausing and withdrawals
//! - Accounting for reserves and deposits

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, token, Address, Env,
};

// ─── USDC Asset Configuration ────────────────────────────────────────

pub const USDC_DECIMALS: u32 = 7;

// ─── Storage Keys ────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Admin,
    Paused,
    UsdcToken,
    TotalDeposited,
    TotalReleased,
    Deposit(Address),
    ReserveBalance,
}

// ─── Errors ──────────────────────────────────────────────────────────

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum VaultError {
    NotAdmin = 1,
    Paused = 2,
    InsufficientBalance = 3,
    InsufficientVaultBalance = 4,
    BelowReserveMinimum = 5,
    InvalidAmount = 6,
    UsdcTokenNotSet = 7,
    AlreadyInitialized = 8,
}

// ─── Internal Helpers ────────────────────────────────────────────────

fn read_admin(env: &Env) -> Address {
    env.storage()
        .instance()
        .get::<DataKey, Address>(&DataKey::Admin)
        .unwrap()
}

fn is_paused(env: &Env) -> bool {
    env.storage()
        .instance()
        .get::<DataKey, bool>(&DataKey::Paused)
        .unwrap_or(false)
}

fn set_paused(env: &Env, paused: bool) {
    env.storage().instance().set(&DataKey::Paused, &paused);
}

fn read_usdc_token(env: &Env) -> Option<Address> {
    env.storage()
        .instance()
        .get::<DataKey, Address>(&DataKey::UsdcToken)
}

fn write_usdc_token(env: &Env, token: &Address) {
    env.storage().instance().set(&DataKey::UsdcToken, token);
}

fn read_total_deposited(env: &Env) -> i128 {
    env.storage()
        .instance()
        .get::<DataKey, i128>(&DataKey::TotalDeposited)
        .unwrap_or(0)
}

fn write_total_deposited(env: &Env, amount: i128) {
    env.storage().instance().set(&DataKey::TotalDeposited, &amount);
}

fn read_total_released(env: &Env) -> i128 {
    env.storage()
        .instance()
        .get::<DataKey, i128>(&DataKey::TotalReleased)
        .unwrap_or(0)
}

fn write_total_released(env: &Env, amount: i128) {
    env.storage().instance().set(&DataKey::TotalReleased, &amount);
}

fn read_user_deposit(env: &Env, user: &Address) -> i128 {
    env.storage()
        .instance()
        .get::<DataKey, i128>(&DataKey::Deposit(user.clone()))
        .unwrap_or(0)
}

fn write_user_deposit(env: &Env, user: &Address, amount: i128) {
    env.storage()
        .instance()
        .set(&DataKey::Deposit(user.clone()), &amount);
}

fn read_reserve_balance(env: &Env) -> i128 {
    env.storage()
        .instance()
        .get::<DataKey, i128>(&DataKey::ReserveBalance)
        .unwrap_or(0)
}

fn write_reserve_balance(env: &Env, amount: i128) {
    env.storage().instance().set(&DataKey::ReserveBalance, &amount);
}

// ─── Contract ────────────────────────────────────────────────────────

#[contract]
pub struct UsdcVault;

#[contractimpl]
impl UsdcVault {
    /// Initialize the vault with admin and USDC token address.
    pub fn initialize(env: Env, admin: Address, usdc_token: Address) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("already initialized");
        }
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
        write_usdc_token(&env, &usdc_token);
        set_paused(&env, false);
        write_total_deposited(&env, 0);
        write_total_released(&env, 0);
        write_reserve_balance(&env, 0);
    }

    /// Deposit USDC into the vault.
    /// Transfers USDC from depositor to this contract and tracks the deposit.
    pub fn deposit(env: Env, depositor: Address, amount: i128) -> Result<(), VaultError> {
        if is_paused(&env) {
            return Err(VaultError::Paused);
        }
        if amount <= 0 {
            return Err(VaultError::InvalidAmount);
        }

        depositor.require_auth();

        let usdc_addr = read_usdc_token(&env).ok_or(VaultError::UsdcTokenNotSet)?;
        let usdc = token::Client::new(&env, &usdc_addr);
        let vault_addr = env.current_contract_address();

        // Transfer USDC from depositor to vault
        usdc.transfer(&depositor, &vault_addr, &amount);

        // Update accounting
        let current_total = read_total_deposited(&env);
        write_total_deposited(&env, current_total + amount);

        let user_total = read_user_deposit(&env, &depositor);
        write_user_deposit(&env, &depositor, user_total + amount);

        // Emit event
        env.events().publish(
            (symbol_short!("usdc_dep"), depositor),
            amount,
        );

        Ok(())
    }

    /// Release USDC from the vault to a recipient.
    /// Used when redeeming wPi back to USDC on Stellar.
    pub fn release(
        env: Env,
        admin: Address,
        recipient: Address,
        amount: i128,
    ) -> Result<(), VaultError> {
        let current_admin = read_admin(&env);
        if admin != current_admin {
            return Err(VaultError::NotAdmin);
        }
        admin.require_auth();

        if is_paused(&env) {
            return Err(VaultError::Paused);
        }
        if amount <= 0 {
            return Err(VaultError::InvalidAmount);
        }

        let usdc_addr = read_usdc_token(&env).ok_or(VaultError::UsdcTokenNotSet)?;
        let usdc = token::Client::new(&env, &usdc_addr);
        let vault_addr = env.current_contract_address();

        // Check vault has enough USDC
        let vault_balance = usdc.balance(&vault_addr);
        if vault_balance < amount {
            return Err(VaultError::InsufficientVaultBalance);
        }

        // Check we don't dip below reserve minimum
        let reserve_min = read_reserve_balance(&env);
        if vault_balance - amount < reserve_min {
            return Err(VaultError::BelowReserveMinimum);
        }

        // Transfer USDC from vault to recipient
        usdc.transfer(&vault_addr, &recipient, &amount);

        // Update accounting
        let current_released = read_total_released(&env);
        write_total_released(&env, current_released + amount);

        // Emit event
        env.events().publish(
            (symbol_short!("usdc_rel"), recipient),
            amount,
        );

        Ok(())
    }

    /// Admin withdraw USDC (for operational needs).
    /// Respects the reserve minimum.
    pub fn admin_withdraw(
        env: Env,
        admin: Address,
        destination: Address,
        amount: i128,
    ) -> Result<(), VaultError> {
        let current_admin = read_admin(&env);
        if admin != current_admin {
            return Err(VaultError::NotAdmin);
        }
        admin.require_auth();

        if amount <= 0 {
            return Err(VaultError::InvalidAmount);
        }

        let usdc_addr = read_usdc_token(&env).ok_or(VaultError::UsdcTokenNotSet)?;
        let usdc = token::Client::new(&env, &usdc_addr);
        let vault_addr = env.current_contract_address();

        let vault_balance = usdc.balance(&vault_addr);
        if vault_balance < amount {
            return Err(VaultError::InsufficientVaultBalance);
        }

        let reserve_min = read_reserve_balance(&env);
        if vault_balance - amount < reserve_min {
            return Err(VaultError::BelowReserveMinimum);
        }

        usdc.transfer(&vault_addr, &destination, &amount);

        // Emit event
        env.events().publish(
            (symbol_short!("adm_wd"), admin, destination),
            amount,
        );

        Ok(())
    }

    /// Set the reserve minimum (USDC that must stay in vault).
    pub fn set_reserve_minimum(
        env: Env,
        admin: Address,
        reserve_minimum: i128,
    ) -> Result<(), VaultError> {
        let current_admin = read_admin(&env);
        if admin != current_admin {
            return Err(VaultError::NotAdmin);
        }
        admin.require_auth();
        write_reserve_balance(&env, reserve_minimum);
        Ok(())
    }

    /// Pause/unpause the vault.
    pub fn set_paused(env: Env, admin: Address, paused: bool) -> Result<(), VaultError> {
        let current_admin = read_admin(&env);
        if admin != current_admin {
            return Err(VaultError::NotAdmin);
        }
        admin.require_auth();
        set_paused(&env, paused);
        Ok(())
    }

    // ─── Read-only queries ─────────────────────────────────────────

    pub fn usdc_token(env: Env) -> Option<Address> {
        read_usdc_token(&env)
    }

    pub fn total_deposited(env: Env) -> i128 {
        read_total_deposited(&env)
    }

    pub fn total_released(env: Env) -> i128 {
        read_total_released(&env)
    }

    pub fn user_deposit(env: Env, user: Address) -> i128 {
        read_user_deposit(&env, &user)
    }

    pub fn reserve_minimum(env: Env) -> i128 {
        read_reserve_balance(&env)
    }

    pub fn vault_balance(env: Env) -> i128 {
        let usdc_addr = match read_usdc_token(&env) {
            Some(addr) => addr,
            None => return 0,
        };
        let usdc = token::Client::new(&env, &usdc_addr);
        usdc.balance(&env.current_contract_address())
    }

    pub fn available_balance(env: Env) -> i128 {
        let balance = Self::vault_balance(env.clone());
        let reserve = read_reserve_balance(&env);
        if balance < reserve {
            0
        } else {
            balance - reserve
        }
    }

    pub fn admin(env: Env) -> Address {
        read_admin(&env)
    }

    pub fn is_paused(env: Env) -> bool {
        is_paused(&env)
    }

    pub fn set_admin(env: Env, admin: Address, new_admin: Address) -> Result<(), VaultError> {
        let current_admin = read_admin(&env);
        if admin != current_admin {
            return Err(VaultError::NotAdmin);
        }
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &new_admin);
        Ok(())
    }
}

#[cfg(test)]
mod test {
    extern crate std;
    use super::{UsdcVault, UsdcVaultClient, VaultError};
    use soroban_sdk::{testutils::Address as _, Address, Env};

    fn setup() -> (Env, UsdcVaultClient<'static>, Address, Address) {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(UsdcVault, ());
        let client = UsdcVaultClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        let usdc_token = Address::generate(&env);
        client.initialize(&admin, &usdc_token);
        (env, client, admin, usdc_token)
    }

    #[test]
    fn test_initialize() {
        let (env, client, admin, usdc_token) = setup();

        assert_eq!(client.admin(), admin);
        assert_eq!(client.usdc_token(), Some(usdc_token));
        assert_eq!(client.total_deposited(), 0);
        assert_eq!(client.total_released(), 0);
        assert_eq!(client.reserve_minimum(), 0);
        assert!(!client.is_paused());
    }

    #[test]
    fn test_set_reserve_minimum() {
        let (env, client, admin, _) = setup();

        client.set_reserve_minimum(&admin, &1000);
        assert_eq!(client.reserve_minimum(), 1000);
    }

    #[test]
    fn test_pause_unpause() {
        let (env, client, admin, _) = setup();

        assert!(!client.is_paused());
        client.set_paused(&admin, &true);
        assert!(client.is_paused());
        client.set_paused(&admin, &false);
        assert!(!client.is_paused());
    }

    #[test]
    fn test_non_admin_cannot_withdraw() {
        let (env, client, _, _) = setup();
        let rando = Address::generate(&env);

        let result = client.try_admin_withdraw(&rando, &rando, &100);
        assert_eq!(result, Err(Ok(VaultError::NotAdmin)));
    }

    #[test]
    fn test_double_initialize_fails() {
        let (env, client, admin, usdc_token) = setup();

        let result = client.try_initialize(&admin, &usdc_token);
        assert!(result.is_err());
    }
}
