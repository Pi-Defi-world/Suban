#![no_std]

//! Bridge Burn-Mint — Cross-chain PUSD bridge between Stellar and Arc.
//!
//! Burns PUSD on Stellar side to bridge to Arc.
//! Mints PUSD on Stellar side when bridged from Arc.
//!
//! Features:
//! - M-of-N signer set (separate from bridge-multisig)
//! - Per-chain mint caps
//! - Volume circuit breaker
//! - Replay protection via nonces
//! - Pause mechanism

use soroban_sdk::{contract, contracterror, contractimpl, contracttype, symbol_short, Address, BytesN, Env, Symbol};

// ─── Storage Keys ─────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Admin,
    PusdToken,
    Signer(Address),
    SignerCount,
    Threshold,
    Nonce(u64),
    NextNonce,
    ProcessedHash(BytesN<32>),
    ChainMintCap(String),
    ChainTotalMinted(String),
    ChainTotalBurned(String),
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
    pub mint_cap: i128,
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
    EmptyDestination = 14,
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

fn check_volume_circuit_breaker(env: &Env, amount: i128) {
    let volume_cap: i128 = env
        .storage()
        .instance()
        .get::<DataKey, i128>(&DataKey::VolumeCap)
        .unwrap_or(0);

    if volume_cap == 0 {
        return; // no cap
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
        // reset window
        env.storage()
            .instance()
            .set(&DataKey::VolumeWindowStart, &current_ledger);
        window_volume = 0;
    }

    if window_volume + amount > volume_cap {
        panic!("volume cap exceeded");
    }

    env.storage()
        .instance()
        .set(&DataKey::VolumeInWindow, &(window_volume + amount));
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
    }

    // ─── Core: Burn PUSD (bridge to Arc) ──────────────────────────────

    pub fn burn_pusd(env: Env, sender: Address, amount: i128, destination: String) {
        if is_paused(&env) {
            panic!("bridge is paused");
        }
        if amount <= 0 {
            panic!("invalid amount");
        }
        if destination.is_empty() {
            panic!("empty destination");
        }

        sender.require_auth();

        let nonce = next_nonce(&env);
        env.storage()
            .instance()
            .set(&DataKey::NextNonce, &(nonce + 1));

        // Update chain state
        let mut state: ChainState = env
            .storage()
            .instance()
            .get::<DataKey, ChainState>(&DataKey::ChainTotalBurned(destination.clone()))
            .unwrap_or(ChainState {
                total_minted: 0,
                total_burned: 0,
                mint_cap: 0,
            });
        state.total_burned += amount;
        env.storage()
            .instance()
            .set(&DataKey::ChainTotalBurned(destination.clone()), &state);

        // Burn via token contract
        let token: Address = env
            .storage()
            .instance()
            .get::<DataKey, Address>(&DataKey::PusdToken)
            .unwrap();
        env.invoke_contract::<()>(
            &token,
            &symbol_short!("burn"),
            soroban_sdk::vec![&env, &sender, &amount],
        );

        // Emit event
        env.events()
            .publish((TOPIC_BURN, destination, sender, nonce), amount);
    }

    // ─── Core: Mint PUSD (bridged from Arc) ───────────────────────────

    pub fn mint_pusd(
        env: Env,
        relayer: Address,
        recipient: Address,
        amount: i128,
        source_chain: String,
        source_nonce: u64,
        source_tx_hash: BytesN<32>,
        signatures: soroban_sdk::Vec<Address>,
    ) {
        if is_paused(&env) {
            panic!("bridge is paused");
        }
        if amount <= 0 {
            panic!("invalid amount");
        }

        // Check volume circuit breaker
        check_volume_circuit_breaker(&env, amount);

        // Check replay
        let hash_key = DataKey::ProcessedHash(source_tx_hash.clone());
        if env.storage().instance().has(&hash_key) {
            panic!("already processed");
        }

        // Verify threshold
        let threshold = read_threshold(&env);
        if signatures.len() < threshold {
            panic!("insufficient signatures");
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
                panic!("invalid signer");
            }
        }

        // Check mint cap
        let mut state: ChainState = env
            .storage()
            .instance()
            .get::<DataKey, ChainState>(&DataKey::ChainTotalMinted(source_chain.clone()))
            .unwrap_or(ChainState {
                total_minted: 0,
                total_burned: 0,
                mint_cap: 0,
            });

        if state.mint_cap > 0 && state.total_minted + amount > state.mint_cap {
            panic!("mint cap exceeded");
        }

        // Mark as processed
        env.storage().instance().set(&hash_key, &true);

        // Update chain state
        state.total_minted += amount;
        env.storage()
            .instance()
            .set(&DataKey::ChainTotalMinted(source_chain.clone()), &state);

        // Mint via token contract
        let token: Address = env
            .storage()
            .instance()
            .get::<DataKey, Address>(&DataKey::PusdToken)
            .unwrap();
        env.invoke_contract::<()>(
            &token,
            &symbol_short!("mint"),
            soroban_sdk::vec![&env, &recipient, &amount],
        );

        // Emit event
        env.events().publish(
            (TOPIC_MINT, source_chain, source_nonce, recipient),
            amount,
        );
    }

    // ─── Admin: Signer Management ─────────────────────────────────────

    pub fn add_signer(env: Env, signer: Address) {
        let admin = read_admin(&env);
        if admin != env.invoker() {
            panic!("not admin");
        }

        let exists: bool = env
            .storage()
            .instance()
            .get::<DataKey, bool>(&DataKey::Signer(signer.clone()))
            .unwrap_or(false);
        if exists {
            panic!("duplicate signer");
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
    }

    pub fn remove_signer(env: Env, signer: Address) {
        let admin = read_admin(&env);
        if admin != env.invoker() {
            panic!("not admin");
        }

        let exists: bool = env
            .storage()
            .instance()
            .get::<DataKey, bool>(&DataKey::Signer(signer.clone()))
            .unwrap_or(false);
        if !exists {
            panic!("signer not found");
        }

        let count: u32 = env
            .storage()
            .instance()
            .get::<DataKey, u32>(&DataKey::SignerCount)
            .unwrap();
        let threshold = read_threshold(&env);
        if count - 1 < threshold {
            panic!("cannot remove below threshold");
        }

        env.storage()
            .instance()
            .set(&DataKey::Signer(signer.clone()), &false);
        env.storage()
            .instance()
            .set(&DataKey::SignerCount, &(count - 1));

        env.events()
            .publish((TOPIC_SIGNER, symbol_short!("remove")), signer);
    }

    pub fn set_threshold(env: Env, new_threshold: u32) {
        let admin = read_admin(&env);
        if admin != env.invoker() {
            panic!("not admin");
        }

        let count: u32 = env
            .storage()
            .instance()
            .get::<DataKey, u32>(&DataKey::SignerCount)
            .unwrap();
        if new_threshold == 0 || new_threshold > count {
            panic!("invalid threshold");
        }

        env.storage()
            .instance()
            .set(&DataKey::Threshold, &new_threshold);

        env.events().publish(
            (TOPIC_CONFIG, symbol_short!("threshold")),
            new_threshold,
        );
    }

    // ─── Admin: Volume & Cap Management ───────────────────────────────

    pub fn set_volume_cap(env: Env, cap: i128, window_ledgers: u32) {
        let admin = read_admin(&env);
        if admin != env.invoker() {
            panic!("not admin");
        }

        env.storage().instance().set(&DataKey::VolumeCap, &cap);
        env.storage()
            .instance()
            .set(&DataKey::VolumeWindowLedgers, &window_ledgers);

        env.events()
            .publish((TOPIC_VOLUME, symbol_short!("set_cap")), cap);
    }

    pub fn set_chain_mint_cap(env: Env, chain: String, cap: i128) {
        let admin = read_admin(&env);
        if admin != env.invoker() {
            panic!("not admin");
        }

        let mut state: ChainState = env
            .storage()
            .instance()
            .get::<DataKey, ChainState>(&DataKey::ChainTotalMinted(chain.clone()))
            .unwrap_or(ChainState {
                total_minted: 0,
                total_burned: 0,
                mint_cap: 0,
            });
        state.mint_cap = cap;
        env.storage()
            .instance()
            .set(&DataKey::ChainTotalMinted(chain), &state);
    }

    // ─── Admin: Pause ─────────────────────────────────────────────────

    pub fn pause(env: Env) {
        let admin = read_admin(&env);
        if admin != env.invoker() {
            panic!("not admin");
        }
        let paused = is_paused(&env);
        if paused {
            panic!("already paused");
        }
        env.storage().instance().set(&DataKey::Paused, &true);
    }

    pub fn unpause(env: Env) {
        let admin = read_admin(&env);
        if admin != env.invoker() {
            panic!("not admin");
        }
        let paused = is_paused(&env);
        if !paused {
            panic!("not paused");
        }
        env.storage().instance().set(&DataKey::Paused, &false);
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

    pub fn get_chain_state(env: Env, chain: String) -> ChainState {
        env.storage()
            .instance()
            .get::<DataKey, ChainState>(&DataKey::ChainTotalMinted(chain))
            .unwrap_or(ChainState {
                total_minted: 0,
                total_burned: 0,
                mint_cap: 0,
            })
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
