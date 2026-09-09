#![no_std]

use soroban_sdk::{symbol_short, Address, Env, IntoVal, Symbol, Vec};
use hub_errors::HubError;

/// Standardized token interface trait.
/// Works for PUSD, WPi, LP shares, debt tokens, backstop shares.
/// All primitives interact with tokens through this trait.
pub trait HubToken {
    /// Get the token name.
    fn name(env: &Env) -> soroban_sdk::BytesN<32>;

    /// Get the token symbol.
    fn symbol(env: &Env) -> soroban_sdk::BytesN<32>;

    /// Get the number of decimals.
    fn decimals(env: &Env) -> u32;

    /// Get the total supply.
    fn total_supply(env: &Env) -> i128;

    /// Get the balance of an account.
    fn balance(env: &Env, account: &Address) -> i128;

    /// Get the allowance of a spender for an owner.
    fn allowance(env: &Env, owner: &Address, spender: &Address) -> i128;

    /// Transfer tokens from `from` to `to`.
    fn transfer(env: &Env, from: &Address, to: &Address, amount: i128) -> Result<(), HubError>;

    /// Transfer tokens using allowance.
    fn transfer_from(
        env: &Env,
        spender: &Address,
        from: &Address,
        to: &Address,
        amount: i128,
    ) -> Result<(), HubError>;

    /// Approve a spender to spend tokens.
    fn approve(env: &Env, owner: &Address, spender: &Address, amount: i128) -> Result<(), HubError>;

    /// Mint tokens (admin only).
    fn mint(env: &Env, admin: &Address, to: &Address, amount: i128) -> Result<(), HubError>;

    /// Burn tokens (admin only).
    fn burn(env: &Env, admin: &Address, from: &Address, amount: i128) -> Result<(), HubError>;
}

/// Client wrapper for calling HubToken contracts.
pub struct HubTokenClient<'a> {
    env: &'a Env,
    contract_id: Address,
}

impl<'a> HubTokenClient<'a> {
    pub fn new(env: &'a Env, contract_id: &Address) -> Self {
        Self {
            env,
            contract_id: contract_id.clone(),
        }
    }

    pub fn balance(&self, account: &Address) -> i128 {
        self.env.invoke_contract(
            &self.contract_id,
            &symbol_short!("balance"),
            Vec::from_array(self.env, [account.to_val()]),
        )
    }

    pub fn transfer(&self, from: &Address, to: &Address, amount: i128) -> Result<(), HubError> {
        self.env.invoke_contract(
            &self.contract_id,
            &symbol_short!("transfer"),
            Vec::from_array(self.env, [from.to_val(), to.to_val(), amount.into_val(self.env)]),
        )
    }

    pub fn total_supply(&self) -> i128 {
        self.env.invoke_contract(
            &self.contract_id,
            &Symbol::new(self.env, "total_supply"),
            Vec::new(self.env),
        )
    }

    pub fn approve(&self, owner: &Address, spender: &Address, amount: i128) -> Result<(), HubError> {
        self.env.invoke_contract(
            &self.contract_id,
            &symbol_short!("approve"),
            Vec::from_array(self.env, [owner.to_val(), spender.to_val(), amount.into_val(self.env)]),
        )
    }
}
