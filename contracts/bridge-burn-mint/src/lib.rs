#![no_std]

//! Bridge Burn-Mint — Cross-chain PUSD bridge between Stellar and Arc.
//!
//! Burns PUSD on Stellar side to bridge to Arc.
//! Mints PUSD on Stellar side when bridged from Arc.

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, BytesN, Env, IntoVal,
    Symbol,
};

// ─── Storage Keys ─────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Admin,
    PusdToken,
    Signer(Address),
    SignerCount,
    Threshold,
    NextNonce,
    ProcessedHash(BytesN<32>),
    ChainMinted(Symbol),
    ChainBurned(Symbol),
    ChainMintCap(Symbol),
    VolumeWindowStart,
    VolumeInWindow,
    VolumeCap,
    VolumeWindowLedgers,
    Paused,
}

// ─── Types ────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BridgeConfig {
    pub admin: Address,
    pub threshold: u32,
    pub signer_count: u32,
    pub pusd_token: Address,
    pub volume_cap: i128,
    pub volume_window_ledgers: u32,
    pub paused: bool,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChainState {
    pub total_minted: i128,
    pub total_burned: i128,
}

// ─── Events ───────────────────────────────────────────────────────────

const TOPIC_BURN: Symbol = symbol_short!("burn");
const TOPIC_MINT: Symbol = symbol_short!("mint");
const TOPIC_SIGNER: Symbol = symbol_short!("signer");
const TOPIC_CONFIG: Symbol = symbol_short!("config");
const TOPIC_VOLUME: Symbol = symbol_short!("volume");

// ─── Errors ───────────────────────────────────────────────────────────

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum BridgeError {
    NotAdmin = 1,
    NotSigner = 2,
    ThresholdNotMet = 3,
    InvalidAmount = 4,
    AlreadyProcessed = 5,
    DuplicateSigner = 6,
    SignerNotFound = 7,
    CannotRemoveBelowThreshold = 8,
    InvalidThreshold = 9,
    VolumeCapExceeded = 10,
    MintCapExceeded = 11,
    AlreadyPaused = 12,
    NotPaused = 13,
}

// ─── Contract ─────────────────────────────────────────────────────────

#[contract]
pub struct BridgeBurnMint;

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

fn read_threshold(env: &Env) -> u32 {
    env.storage()
        .instance()
        .get::<DataKey, u32>(&DataKey::Threshold)
        .unwrap_or(1)
}

fn next_nonce(env: &Env) -> u64 {
    env.storage()
        .instance()
        .get::<DataKey, u64>(&DataKey::NextNonce)
        .unwrap_or(0)
}

fn get_chain_minted(env: &Env, chain: &Symbol) -> i128 {
    env.storage()
        .instance()
        .get::<DataKey, i128>(&DataKey::ChainMinted(chain.clone()))
        .unwrap_or(0)
}

fn set_chain_minted(env: &Env, chain: &Symbol, amount: i128) {
    env.storage()
        .instance()
        .set(&DataKey::ChainMinted(chain.clone()), &amount);
}

fn get_chain_burned(env: &Env, chain: &Symbol) -> i128 {
    env.storage()
        .instance()
        .get::<DataKey, i128>(&DataKey::ChainBurned(chain.clone()))
        .unwrap_or(0)
}

fn set_chain_burned(env: &Env, chain: &Symbol, amount: i128) {
    env.storage()
        .instance()
        .set(&DataKey::ChainBurned(chain.clone()), &amount);
}

fn get_chain_mint_cap(env: &Env, chain: &Symbol) -> i128 {
    env.storage()
        .instance()
        .get::<DataKey, i128>(&DataKey::ChainMintCap(chain.clone()))
        .unwrap_or(0)
}

fn check_volume_circuit_breaker(env: &Env, amount: i128) -> Result<(), BridgeError> {
    let volume_cap: i128 = env
        .storage()
        .instance()
        .get::<DataKey, i128>(&DataKey::VolumeCap)
        .unwrap_or(0);

    if volume_cap == 0 {
        return Ok(());
    }

    let window_start: u64 = env
        .storage()
        .instance()
        .get::<DataKey, u64>(&DataKey::VolumeWindowStart)
        .unwrap_or(0);

    let window_ledgers: u32 = env
        .storage()
        .instance()
        .get::<DataKey, u32>(&DataKey::VolumeWindowLedgers)
        .unwrap_or(100);

    let current_ledger: u64 = env.ledger().sequence() as u64;

    let mut window_volume: i128 = env
        .storage()
        .instance()
        .get::<DataKey, i128>(&DataKey::VolumeInWindow)
        .unwrap_or(0);

    if current_ledger >= window_start + window_ledgers as u64 {
        env.storage()
            .instance()
            .set(&DataKey::VolumeWindowStart, &current_ledger);
        window_volume = 0;
    }

    if window_volume + amount > volume_cap {
        return Err(BridgeError::VolumeCapExceeded);
    }

    env.storage()
        .instance()
        .set(&DataKey::VolumeInWindow, &(window_volume + amount));
    Ok(())
}

#[contractimpl]
impl BridgeBurnMint {
    // ─── Initialization ───────────────────────────────────────────────

    pub fn initialize(
        env: Env,
        admin: Address,
        signers: soroban_sdk::Vec<Address>,
        threshold: u32,
        pusd_token: Address,
        volume_cap: i128,
        volume_window_ledgers: u32,
    ) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("already initialized");
        }
        admin.require_auth();

        let signer_count = signers.len();
        if threshold == 0 || threshold > signer_count {
            panic!("invalid threshold");
        }

        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage()
            .instance()
            .set(&DataKey::PusdToken, &pusd_token);
        env.storage()
            .instance()
            .set(&DataKey::Threshold, &threshold);
        env.storage()
            .instance()
            .set(&DataKey::SignerCount, &signer_count);
        env.storage()
            .instance()
            .set(&DataKey::VolumeCap, &volume_cap);
        env.storage()
            .instance()
            .set(&DataKey::VolumeWindowLedgers, &volume_window_ledgers);
        env.storage()
            .instance()
            .set(&DataKey::NextNonce, &0u64);
        env.storage().instance().set(&DataKey::Paused, &false);

        for i in 0..signer_count {
            let signer = signers.get(i).unwrap();
            env.storage()
                .instance()
                .set(&DataKey::Signer(signer.clone()), &true);
        }

        env.events().publish(
            (TOPIC_CONFIG, symbol_short!("init")),
            (admin, threshold, signer_count),
        );
    }

    // ─── Core: Burn PUSD (bridge to Arc) ──────────────────────────────

    pub fn burn_pusd(
        env: Env,
        sender: Address,
        amount: i128,
        destination: Symbol,
    ) -> Result<u64, BridgeError> {
        if is_paused(&env) {
            return Err(BridgeError::AlreadyPaused);
        }
        if amount <= 0 {
            return Err(BridgeError::InvalidAmount);
        }

        sender.require_auth();

        let nonce = next_nonce(&env);
        env.storage()
            .instance()
            .set(&DataKey::NextNonce, &(nonce + 1));

        // Update chain state
        let burned = get_chain_burned(&env, &destination);
        set_chain_burned(&env, &destination, burned + amount);

        // Burn via token contract — invoke the token's burn function
        let token: Address = env
            .storage()
            .instance()
            .get::<DataKey, Address>(&DataKey::PusdToken)
            .unwrap();

        // Use soroban_sdk invoke to call burn on the token contract
        env.invoke_contract::<()>(
            &token,
            &symbol_short!("burn"),
            soroban_sdk::vec![&env, sender.to_val(), amount.into_val(&env)],
        );

        // Emit event
        env.events()
            .publish((TOPIC_BURN, destination, sender, nonce), amount);

        Ok(nonce)
    }

    // ─── Core: Mint PUSD (bridged from Arc) ───────────────────────────

    pub fn mint_pusd(
        env: Env,
        _relayer: Address,
        recipient: Address,
        amount: i128,
        source_chain: Symbol,
        source_nonce: u64,
        source_tx_hash: BytesN<32>,
        signatures: soroban_sdk::Vec<Address>,
    ) -> Result<(), BridgeError> {
        if is_paused(&env) {
            return Err(BridgeError::AlreadyPaused);
        }
        if amount <= 0 {
            return Err(BridgeError::InvalidAmount);
        }

        // Check volume circuit breaker
        check_volume_circuit_breaker(&env, amount)?;

        // Check replay
        let hash_key = DataKey::ProcessedHash(source_tx_hash.clone());
        if env.storage().instance().has(&hash_key) {
            return Err(BridgeError::AlreadyProcessed);
        }

        // Verify threshold
        let threshold = read_threshold(&env);
        if signatures.len() < threshold {
            return Err(BridgeError::ThresholdNotMet);
        }

        // Verify each signer
        for i in 0..signatures.len() {
            let signer = signatures.get(i).unwrap();
            let is_valid: bool = env
                .storage()
                .instance()
                .get::<DataKey, bool>(&DataKey::Signer(signer.clone()))
                .unwrap_or(false);
            if !is_valid {
                return Err(BridgeError::NotSigner);
            }
        }

        // Check mint cap (0 = no cap)
        let cap = get_chain_mint_cap(&env, &source_chain);
        if cap > 0 {
            let minted = get_chain_minted(&env, &source_chain);
            if minted + amount > cap {
                return Err(BridgeError::MintCapExceeded);
            }
        }

        // Mark as processed
        env.storage().instance().set(&hash_key, &true);

        // Update chain state
        let minted = get_chain_minted(&env, &source_chain);
        set_chain_minted(&env, &source_chain, minted + amount);

        // Mint via token contract
        let token: Address = env
            .storage()
            .instance()
            .get::<DataKey, Address>(&DataKey::PusdToken)
            .unwrap();

        env.invoke_contract::<()>(
            &token,
            &symbol_short!("mint"),
            soroban_sdk::vec![&env, recipient.to_val(), amount.into_val(&env)],
        );

        // Emit event
        env.events().publish(
            (TOPIC_MINT, source_chain, source_nonce, recipient),
            amount,
        );

        Ok(())
    }

    // ─── Admin: Signer Management ─────────────────────────────────────

    pub fn add_signer(env: Env, signer: Address) -> Result<(), BridgeError> {
        let admin = read_admin(&env);
        admin.require_auth();

        let exists: bool = env
            .storage()
            .instance()
            .get::<DataKey, bool>(&DataKey::Signer(signer.clone()))
            .unwrap_or(false);
        if exists {
            return Err(BridgeError::DuplicateSigner);
        }

        env.storage()
            .instance()
            .set(&DataKey::Signer(signer.clone()), &true);

        let count: u32 = env
            .storage()
            .instance()
            .get::<DataKey, u32>(&DataKey::SignerCount)
            .unwrap();
        env.storage()
            .instance()
            .set(&DataKey::SignerCount, &(count + 1));

        env.events()
            .publish((TOPIC_SIGNER, symbol_short!("add")), signer);
        Ok(())
    }

    pub fn remove_signer(env: Env, signer: Address) -> Result<(), BridgeError> {
        let admin = read_admin(&env);
        admin.require_auth();

        let exists: bool = env
            .storage()
            .instance()
            .get::<DataKey, bool>(&DataKey::Signer(signer.clone()))
            .unwrap_or(false);
        if !exists {
            return Err(BridgeError::SignerNotFound);
        }

        let count: u32 = env
            .storage()
            .instance()
            .get::<DataKey, u32>(&DataKey::SignerCount)
            .unwrap();
        let threshold = read_threshold(&env);
        if count - 1 < threshold {
            return Err(BridgeError::CannotRemoveBelowThreshold);
        }

        env.storage()
            .instance()
            .set(&DataKey::Signer(signer.clone()), &false);
        env.storage()
            .instance()
            .set(&DataKey::SignerCount, &(count - 1));

        env.events()
            .publish((TOPIC_SIGNER, symbol_short!("remove")), signer);
        Ok(())
    }

    pub fn set_threshold(env: Env, new_threshold: u32) -> Result<(), BridgeError> {
        let admin = read_admin(&env);
        admin.require_auth();

        let count: u32 = env
            .storage()
            .instance()
            .get::<DataKey, u32>(&DataKey::SignerCount)
            .unwrap();
        if new_threshold == 0 || new_threshold > count {
            return Err(BridgeError::InvalidThreshold);
        }

        env.storage()
            .instance()
            .set(&DataKey::Threshold, &new_threshold);

        env.events().publish(
            (TOPIC_CONFIG, symbol_short!("threshold")),
            new_threshold,
        );
        Ok(())
    }

    // ─── Admin: Volume & Cap Management ───────────────────────────────

    pub fn set_volume_cap(env: Env, cap: i128, window_ledgers: u32) -> Result<(), BridgeError> {
        let admin = read_admin(&env);
        admin.require_auth();

        env.storage().instance().set(&DataKey::VolumeCap, &cap);
        env.storage()
            .instance()
            .set(&DataKey::VolumeWindowLedgers, &window_ledgers);

        env.events()
            .publish((TOPIC_VOLUME, symbol_short!("set_cap")), cap);
        Ok(())
    }

    pub fn set_chain_mint_cap(
        env: Env,
        chain: Symbol,
        cap: i128,
    ) -> Result<(), BridgeError> {
        let admin = read_admin(&env);
        admin.require_auth();

        env.storage()
            .instance()
            .set(&DataKey::ChainMintCap(chain.clone()), &cap);

        env.events()
            .publish((TOPIC_CONFIG, symbol_short!("mint_cap")), cap);
        Ok(())
    }

    // ─── Admin: Pause ─────────────────────────────────────────────────

    pub fn pause(env: Env) -> Result<(), BridgeError> {
        let admin = read_admin(&env);
        admin.require_auth();
        if is_paused(&env) {
            return Err(BridgeError::AlreadyPaused);
        }
        env.storage().instance().set(&DataKey::Paused, &true);
        Ok(())
    }

    pub fn unpause(env: Env) -> Result<(), BridgeError> {
        let admin = read_admin(&env);
        admin.require_auth();
        if !is_paused(&env) {
            return Err(BridgeError::NotPaused);
        }
        env.storage().instance().set(&DataKey::Paused, &false);
        Ok(())
    }

    // ─── View Functions ───────────────────────────────────────────────

    pub fn get_config(env: Env) -> BridgeConfig {
        BridgeConfig {
            admin: read_admin(&env),
            threshold: read_threshold(&env),
            signer_count: env
                .storage()
                .instance()
                .get::<DataKey, u32>(&DataKey::SignerCount)
                .unwrap(),
            pusd_token: env
                .storage()
                .instance()
                .get::<DataKey, Address>(&DataKey::PusdToken)
                .unwrap(),
            volume_cap: env
                .storage()
                .instance()
                .get::<DataKey, i128>(&DataKey::VolumeCap)
                .unwrap_or(0),
            volume_window_ledgers: env
                .storage()
                .instance()
                .get::<DataKey, u32>(&DataKey::VolumeWindowLedgers)
                .unwrap_or(100),
            paused: is_paused(&env),
        }
    }

    pub fn get_chain_minted(env: Env, chain: Symbol) -> i128 {
        get_chain_minted(&env, &chain)
    }

    pub fn get_chain_burned(env: Env, chain: Symbol) -> i128 {
        get_chain_burned(&env, &chain)
    }

    pub fn is_processed(env: Env, tx_hash: BytesN<32>) -> bool {
        env.storage()
            .instance()
            .has(&DataKey::ProcessedHash(tx_hash))
    }

    pub fn get_nonce(env: Env) -> u64 {
        next_nonce(&env)
    }

    pub fn is_signer(env: Env, signer: Address) -> bool {
        env.storage()
            .instance()
            .get::<DataKey, bool>(&DataKey::Signer(signer))
            .unwrap_or(false)
    }
}
