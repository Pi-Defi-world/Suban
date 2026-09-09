#![no_std]

//! LP Token — SEP-41-like token for liquidity pool shares.
//! Each pool deploys one instance. The pool contract is the admin (can mint/burn).

use soroban_sdk::{
    contract, contracterror, contractevent, contractimpl, contracttype, Address, BytesN, Env,
};

const DECIMALS: u32 = 7;

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Admin,
    Paused,
    TotalSupply,
    Balance(Address),
    Allowance(Address, Address),
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum LpError {
    NotAdmin = 1,
    Paused = 2,
    InsufficientBalance = 3,
    InsufficientAllowance = 4,
    InvalidAmount = 5,
}

#[contractevent(data_format = "single-value")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Transfer {
    #[topic]
    pub from: Address,
    #[topic]
    pub to: Address,
    pub amount: i128,
}

#[contractevent(data_format = "single-value")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Mint {
    #[topic]
    pub admin: Address,
    #[topic]
    pub to: Address,
    pub amount: i128,
}

#[contractevent(data_format = "single-value")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Burn {
    #[topic]
    pub admin: Address,
    #[topic]
    pub from: Address,
    pub amount: i128,
}

#[contract]
pub struct LpToken;

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

fn read_balance(env: &Env, addr: &Address) -> i128 {
    env.storage()
        .instance()
        .get::<DataKey, i128>(&DataKey::Balance(addr.clone()))
        .unwrap_or(0)
}

fn write_balance(env: &Env, addr: &Address, amount: i128) {
    env.storage()
        .instance()
        .set(&DataKey::Balance(addr.clone()), &amount);
}

fn read_allowance(env: &Env, from: &Address, spender: &Address) -> i128 {
    env.storage()
        .instance()
        .get::<DataKey, i128>(&DataKey::Allowance(from.clone(), spender.clone()))
        .unwrap_or(0)
}

fn write_allowance(env: &Env, from: &Address, spender: &Address, amount: i128) {
    env.storage()
        .instance()
        .set(&DataKey::Allowance(from.clone(), spender.clone()), &amount);
}

fn read_total_supply(env: &Env) -> i128 {
    env.storage()
        .instance()
        .get::<DataKey, i128>(&DataKey::TotalSupply)
        .unwrap_or(0)
}

fn write_total_supply(env: &Env, amount: i128) {
    env.storage()
        .instance()
        .set(&DataKey::TotalSupply, &amount);
}

#[contractimpl]
impl LpToken {
    pub fn initialize(env: Env, admin: Address) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("already initialized");
        }
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::Paused, &false);
        write_total_supply(&env, 0);
    }

    pub fn name(_env: Env) -> BytesN<32> {
        // "LP Token" padded
        let mut out = [0u8; 32];
        let b = b"LP Token";
        out[..b.len()].copy_from_slice(b);
        BytesN::from_array(&_env, &out)
    }

    pub fn symbol(_env: Env) -> BytesN<32> {
        let mut out = [0u8; 32];
        let b = b"LP";
        out[..b.len()].copy_from_slice(b);
        BytesN::from_array(&_env, &out)
    }

    pub fn decimals(_env: Env) -> u32 {
        DECIMALS
    }

    pub fn total_supply(env: Env) -> i128 {
        read_total_supply(&env)
    }

    pub fn balance(env: Env, owner: Address) -> i128 {
        read_balance(&env, &owner)
    }

    pub fn allowance(env: Env, owner: Address, spender: Address) -> i128 {
        read_allowance(&env, &owner, &spender)
    }

    pub fn transfer(env: Env, from: Address, to: Address, amount: i128) -> Result<(), LpError> {
        if is_paused(&env) {
            return Err(LpError::Paused);
        }
        if amount <= 0 {
            return Err(LpError::InvalidAmount);
        }
        from.require_auth();
        let from_bal = read_balance(&env, &from);
        if from_bal < amount {
            return Err(LpError::InsufficientBalance);
        }
        write_balance(&env, &from, from_bal - amount);
        write_balance(&env, &to, read_balance(&env, &to) + amount);
        Transfer { from, to, amount }.publish(&env);
        Ok(())
    }

    pub fn transfer_from(
        env: Env,
        spender: Address,
        from: Address,
        to: Address,
        amount: i128,
    ) -> Result<(), LpError> {
        if is_paused(&env) {
            return Err(LpError::Paused);
        }
        if amount <= 0 {
            return Err(LpError::InvalidAmount);
        }
        spender.require_auth();
        let allowance = read_allowance(&env, &from, &spender);
        if allowance < amount {
            return Err(LpError::InsufficientAllowance);
        }
        let from_bal = read_balance(&env, &from);
        if from_bal < amount {
            return Err(LpError::InsufficientBalance);
        }
        write_allowance(&env, &from, &spender, allowance - amount);
        write_balance(&env, &from, from_bal - amount);
        write_balance(&env, &to, read_balance(&env, &to) + amount);
        Transfer { from, to, amount }.publish(&env);
        Ok(())
    }

    pub fn approve(
        env: Env,
        owner: Address,
        spender: Address,
        amount: i128,
    ) -> Result<(), LpError> {
        if is_paused(&env) {
            return Err(LpError::Paused);
        }
        owner.require_auth();
        write_allowance(&env, &owner, &spender, amount);
        Ok(())
    }

    pub fn mint(env: Env, admin: Address, to: Address, amount: i128) -> Result<(), LpError> {
        let current_admin = read_admin(&env);
        if admin != current_admin {
            return Err(LpError::NotAdmin);
        }
        admin.require_auth();
        if amount <= 0 {
            return Err(LpError::InvalidAmount);
        }
        write_balance(&env, &to, read_balance(&env, &to) + amount);
        write_total_supply(&env, read_total_supply(&env) + amount);
        Mint { admin, to, amount }.publish(&env);
        Ok(())
    }

    pub fn burn(env: Env, admin: Address, from: Address, amount: i128) -> Result<(), LpError> {
        let current_admin = read_admin(&env);
        if admin != current_admin {
            return Err(LpError::NotAdmin);
        }
        admin.require_auth();
        if amount <= 0 {
            return Err(LpError::InvalidAmount);
        }
        let bal = read_balance(&env, &from);
        if bal < amount {
            return Err(LpError::InsufficientBalance);
        }
        write_balance(&env, &from, bal - amount);
        write_total_supply(&env, read_total_supply(&env) - amount);
        Burn { admin, from, amount }.publish(&env);
        Ok(())
    }

    pub fn set_paused(env: Env, admin: Address, paused: bool) -> Result<(), LpError> {
        let current_admin = read_admin(&env);
        if admin != current_admin {
            return Err(LpError::NotAdmin);
        }
        admin.require_auth();
        env.storage().instance().set(&DataKey::Paused, &paused);
        Ok(())
    }

    pub fn admin(env: Env) -> Address {
        read_admin(&env)
    }
}

#[cfg(test)]
mod test {
    extern crate std;
    use super::{LpError, LpToken, LpTokenClient};
    use soroban_sdk::{testutils::Address as _, Address, Env};

    fn setup() -> (Env, LpTokenClient<'static>, Address) {
        let env = Env::default();
        env.mock_all_auths();
        let id = env.register(LpToken, ());
        let client = LpTokenClient::new(&env, &id);
        let admin = Address::generate(&env);
        client.initialize(&admin);
        (env, client, admin)
    }

    #[test]
    fn test_initialize() {
        let (_, client, admin) = setup();
        assert_eq!(client.admin(), admin);
        assert_eq!(client.total_supply(), 0);
        assert_eq!(client.decimals(), 7);
    }

    #[test]
    fn test_mint() {
        let (env, client, admin) = setup();
        let user = Address::generate(&env);
        client.mint(&admin, &user, &5000);
        assert_eq!(client.balance(&user), 5000);
        assert_eq!(client.total_supply(), 5000);
    }

    #[test]
    fn test_mint_zero_fails() {
        let (env, client, admin) = setup();
        let user = Address::generate(&env);
        let result = client.try_mint(&admin, &user, &0);
        assert_eq!(result, Err(Ok(LpError::InvalidAmount)));
    }

    #[test]
    fn test_non_admin_cannot_mint() {
        let (env, client, _) = setup();
        let rando = Address::generate(&env);
        let user = Address::generate(&env);
        let result = client.try_mint(&rando, &user, &100);
        assert_eq!(result, Err(Ok(LpError::NotAdmin)));
    }

    #[test]
    fn test_burn() {
        let (env, client, admin) = setup();
        let user = Address::generate(&env);
        client.mint(&admin, &user, &5000);
        client.burn(&admin, &user, &2000);
        assert_eq!(client.balance(&user), 3000);
        assert_eq!(client.total_supply(), 3000);
    }

    #[test]
    fn test_burn_insufficient() {
        let (env, client, admin) = setup();
        let user = Address::generate(&env);
        client.mint(&admin, &user, &100);
        let result = client.try_burn(&admin, &user, &200);
        assert_eq!(result, Err(Ok(LpError::InsufficientBalance)));
    }

    #[test]
    fn test_transfer() {
        let (env, client, admin) = setup();
        let alice = Address::generate(&env);
        let bob = Address::generate(&env);
        client.mint(&admin, &alice, &5000);
        client.transfer(&alice, &bob, &1500);
        assert_eq!(client.balance(&alice), 3500);
        assert_eq!(client.balance(&bob), 1500);
    }

    #[test]
    fn test_transfer_insufficient() {
        let (env, client, admin) = setup();
        let alice = Address::generate(&env);
        let bob = Address::generate(&env);
        client.mint(&admin, &alice, &100);
        let result = client.try_transfer(&alice, &bob, &200);
        assert_eq!(result, Err(Ok(LpError::InsufficientBalance)));
    }

    #[test]
    fn test_approve_and_transfer_from() {
        let (env, client, admin) = setup();
        let alice = Address::generate(&env);
        let bob = Address::generate(&env);
        client.mint(&admin, &alice, &5000);
        client.approve(&alice, &bob, &1000);
        assert_eq!(client.allowance(&alice, &bob), 1000);
        client.transfer_from(&bob, &alice, &bob, &800);
        assert_eq!(client.balance(&alice), 4200);
        assert_eq!(client.balance(&bob), 800);
        assert_eq!(client.allowance(&alice, &bob), 200);
    }

    #[test]
    fn test_transfer_from_exceeds_allowance() {
        let (env, client, admin) = setup();
        let alice = Address::generate(&env);
        let bob = Address::generate(&env);
        client.mint(&admin, &alice, &5000);
        client.approve(&alice, &bob, &100);
        let result = client.try_transfer_from(&bob, &alice, &bob, &200);
        assert_eq!(result, Err(Ok(LpError::InsufficientAllowance)));
    }

    #[test]
    fn test_double_initialize_fails() {
        let (env, client, admin) = setup();
        let result = client.try_initialize(&admin);
        assert!(result.is_err());
    }
}
