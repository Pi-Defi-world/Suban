#![no_std]

//! Bridge Multi-Sig — On-chain quorum approval for bridge operations.
//!
//! Replaces the single-admin model with threshold-based multi-signature.
//! Every bridge mint or release requires N-of-M signer approval before execution.
//!
//! Features:
//! - Configurable signer set with threshold
//! - Proposal lifecycle: propose → approve → execute
//! - Nonce tracking for replay protection per deposit/burn
//! - Volume circuit breaker (auto-halt if volume exceeds cap)
//! - Time-based proposal expiration
//! - Pause mechanism

use soroban_sdk::{contract, contracterror, contractimpl, contracttype, symbol_short, Address, BytesN, Env, Symbol};
use hub_errors::HubError;

// ─── Storage Keys ─────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Admin,
    Signer(Address),
    SignerCount,
    Threshold,
    Proposal(u32),
    ProposalCount,
    Nonce(u64),
    NextNonce,
    VolumeWindowStart,
    VolumeInWindow,
    VolumeCap,
    VolumeWindowLedgers,
    Paused,
    MaxProposalAge,
}

// ─── Types ────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Signer {
    pub address: Address,
    pub active: bool,
}

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProposalStatus {
    Pending = 0,
    Executed = 1,
    Rejected = 2,
    Expired = 3,
}

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProposalKind {
    MintWpi = 0,       // Mint wPi from a confirmed Pi deposit
    ReleaseUsdc = 1,   // Release USDC from vault for a redemption
    AddSigner = 2,     // Add a new signer
    RemoveSigner = 3,  // Remove a signer
    SetThreshold = 4,  // Change the threshold
    SetVolumeCap = 5,  // Change the volume circuit breaker cap
    Pause = 6,         // Pause bridge operations
    Unpause = 7,       // Unpause bridge operations
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Proposal {
    pub id: u32,
    pub kind: ProposalKind,
    pub proposer: Address,
    pub target: Address,          // recipient of mint/release, or new signer address
    pub amount: i128,             // amount to mint/release
    pub deposit_id: BytesN<32>,   // pi_deposit_id or burn_nonce for replay protection
    pub approvals: soroban_sdk::Vec<Address>,
    pub status: ProposalStatus,
    pub created_at: u64,          // ledger sequence
    pub executed_at: Option<u64>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BridgeConfig {
    pub admin: Address,
    pub threshold: u32,
    pub signer_count: u32,
    pub volume_cap: i128,
    pub volume_window_ledgers: u32,
    pub paused: bool,
    pub max_proposal_age: u32,
}

// ─── Events ───────────────────────────────────────────────────────────

const TOPIC_PROPOSAL: Symbol = symbol_short!("proposal");
const TOPIC_APPROVE: Symbol = symbol_short!("approve");
const TOPIC_EXECUTE: Symbol = symbol_short!("execute");
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
    ProposalNotFound = 4,
    ProposalAlreadyExecuted = 5,
    ProposalExpired = 6,
    DuplicateApproval = 7,
    DuplicateSigner = 8,
    SignerNotFound = 9,
    CannotRemoveBelowThreshold = 10,
    CannotRemoveLastSigner = 11,
    InvalidThreshold = 12,
    DepositAlreadyProcessed = 13,
    VolumeCapExceeded = 14,
    InvalidAmount = 15,
    AlreadyPaused = 16,
    NotPaused = 17,
    InvalidProposalKind = 18,
}

// ─── Contract ─────────────────────────────────────────────────────────

#[contract]
pub struct BridgeMultisig;

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

fn next_proposal_id(env: &Env) -> u32 {
    env.storage()
        .instance()
        .get::<DataKey, u32>(&DataKey::ProposalCount)
        .unwrap_or(0)
}

fn next_nonce(env: &Env) -> u64 {
    env.storage()
        .instance()
        .get::<DataKey, u64>(&DataKey::NextNonce)
        .unwrap_or(0)
}

#[contractimpl]
impl BridgeMultisig {
    // ─── Initialization ───────────────────────────────────────────────

    pub fn initialize(
        env: Env,
        admin: Address,
        signers: soroban_sdk::Vec<Address>,
        threshold: u32,
        volume_cap: i128,
        volume_window_ledgers: u32,
        max_proposal_age: u32,
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
        env.storage().instance().set(&DataKey::Threshold, &threshold);
        env.storage().instance().set(&DataKey::SignerCount, &signer_count);
        env.storage().instance().set(&DataKey::ProposalCount, &0u32);
        env.storage().instance().set(&DataKey::NextNonce, &0u64);
        env.storage().instance().set(&DataKey::Paused, &false);
        env.storage().instance().set(&DataKey::VolumeCap, &volume_cap);
        env.storage().instance().set(&DataKey::VolumeWindowLedgers, &volume_window_ledgers);
        env.storage().instance().set(&DataKey::VolumeWindowStart, &0u64);
        env.storage().instance().set(&DataKey::VolumeInWindow, &0i128);
        env.storage().instance().set(&DataKey::MaxProposalAge, &max_proposal_age);

        for i in 0..signers.len() {
            let signer = signers.get(i).unwrap();
            env.storage().instance().set(&DataKey::Signer(signer.clone()), &true);
        }

        env.events().publish(
            (TOPIC_CONFIG, symbol_short!("init")),
            (admin, threshold, signer_count),
        );
    }

    // ─── Propose ──────────────────────────────────────────────────────

    /// Propose a bridge operation (mint wPi or release USDC).
    pub fn propose(
        env: Env,
        proposer: Address,
        kind: ProposalKind,
        target: Address,
        amount: i128,
        deposit_id: BytesN<32>,
    ) -> Result<u32, BridgeError> {
        proposer.require_auth();

        if is_paused(&env) {
            return Err(BridgeError::AlreadyPaused);
        }

        if !Self::is_signer(&env, &proposer) {
            return Err(BridgeError::NotSigner);
        }

        if amount <= 0 {
            return Err(BridgeError::InvalidAmount);
        }

        // Only allow mint/release proposals through this function
        match kind {
            ProposalKind::MintWpi | ProposalKind::ReleaseUsdc => {}
            _ => return Err(BridgeError::InvalidProposalKind),
        }

        // Replay protection: check deposit_id not already used
        if env.storage().instance().has(&DataKey::Nonce(deposit_id_to_nonce(&deposit_id))) {
            return Err(BridgeError::DepositAlreadyProcessed);
        }

        // Volume circuit breaker check
        let cap = env.storage().instance().get::<DataKey, i128>(&DataKey::VolumeCap).unwrap_or(0);
        if cap > 0 {
            Self::check_volume(&env, amount, cap)?;
        }

        let proposal_id = next_proposal_id(&env);
        let ledger = env.ledger().sequence() as u64;

        let mut approvals = soroban_sdk::Vec::new(&env);
        approvals.push_back(proposer.clone());

        let proposal = Proposal {
            id: proposal_id,
            kind: kind.clone(),
            proposer: proposer.clone(),
            target: target.clone(),
            amount,
            deposit_id: deposit_id.clone(),
            approvals,
            status: ProposalStatus::Pending,
            created_at: ledger,
            executed_at: None,
        };

        env.storage().instance().set(&DataKey::Proposal(proposal_id), &proposal);
        env.storage().instance().set(&DataKey::ProposalCount, &(proposal_id + 1));

        env.events().publish(
            (TOPIC_PROPOSAL, symbol_short!("created")),
            (proposal_id, kind, proposer, target, amount, deposit_id),
        );

        Ok(proposal_id)
    }

    // ─── Approve ──────────────────────────────────────────────────────

    /// Approve a pending proposal. Executes automatically when threshold is met.
    pub fn approve(
        env: Env,
        signer: Address,
        proposal_id: u32,
    ) -> Result<ProposalStatus, BridgeError> {
        signer.require_auth();

        if !Self::is_signer(&env, &signer) {
            return Err(BridgeError::NotSigner);
        }

        let mut proposal = Self::get_proposal(&env, proposal_id)?;

        if proposal.status != ProposalStatus::Pending {
            return Err(BridgeError::ProposalAlreadyExecuted);
        }

        // Check expiration
        let current_ledger = env.ledger().sequence() as u64;
        let max_age = env.storage().instance().get::<DataKey, u32>(&DataKey::MaxProposalAge).unwrap_or(100);
        if current_ledger > proposal.created_at + max_age as u64 {
            proposal.status = ProposalStatus::Expired;
            env.storage().instance().set(&DataKey::Proposal(proposal_id), &proposal);
            return Err(BridgeError::ProposalExpired);
        }

        // Check for duplicate approval
        for i in 0..proposal.approvals.len() {
            if proposal.approvals.get(i).unwrap() == signer {
                return Err(BridgeError::DuplicateApproval);
            }
        }

        proposal.approvals.push_back(signer.clone());

        env.events().publish(
            (TOPIC_APPROVE, symbol_short!("vote")),
            (proposal_id, signer, proposal.approvals.len()),
        );

        // Check if threshold met
        let threshold = read_threshold(&env);
        if proposal.approvals.len() >= threshold {
            // Execute
            let status = Self::execute_proposal(&env, &mut proposal)?;
            proposal.status = status;
            proposal.executed_at = Some(current_ledger);
            env.storage().instance().set(&DataKey::Proposal(proposal_id), &proposal);
            return Ok(status);
        }

        env.storage().instance().set(&DataKey::Proposal(proposal_id), &proposal);
        Ok(ProposalStatus::Pending)
    }

    // ─── Execute (internal) ───────────────────────────────────────────

    fn execute_proposal(
        env: &Env,
        proposal: &mut Proposal,
    ) -> Result<ProposalStatus, BridgeError> {
        match proposal.kind {
            ProposalKind::MintWpi => {
                // Mark nonce as used (replay protection)
                let nonce = deposit_id_to_nonce(&proposal.deposit_id);
                env.storage().instance().set(&DataKey::Nonce(nonce), &true);

                // Update volume tracking
                Self::add_volume(env, proposal.amount);

                env.events().publish(
                    (TOPIC_EXECUTE, symbol_short!("mint")),
                    (proposal.id, proposal.target.clone(), proposal.amount, proposal.deposit_id.clone()),
                );
            }
            ProposalKind::ReleaseUsdc => {
                // Mark nonce as used (replay protection)
                let nonce = deposit_id_to_nonce(&proposal.deposit_id);
                env.storage().instance().set(&DataKey::Nonce(nonce), &true);

                // Update volume tracking
                Self::add_volume(env, proposal.amount);

                env.events().publish(
                    (TOPIC_EXECUTE, symbol_short!("release")),
                    (proposal.id, proposal.target.clone(), proposal.amount, proposal.deposit_id.clone()),
                );
            }
            _ => {}
        }

        Ok(ProposalStatus::Executed)
    }

    // ─── Volume Circuit Breaker ───────────────────────────────────────

    fn check_volume(env: &Env, amount: i128, cap: i128) -> Result<(), BridgeError> {
        let current_volume = env.storage().instance().get::<DataKey, i128>(&DataKey::VolumeInWindow).unwrap_or(0);

        if current_volume + amount > cap {
            env.events().publish(
                (TOPIC_VOLUME, symbol_short!("breach")),
                (current_volume, amount, cap),
            );
            return Err(BridgeError::VolumeCapExceeded);
        }

        Ok(())
    }

    fn add_volume(env: &Env, amount: i128) {
        let current_ledger = env.ledger().sequence() as u64;
        let window_start = env.storage().instance().get::<DataKey, u64>(&DataKey::VolumeWindowStart).unwrap_or(0);
        let window_ledgers = env.storage().instance().get::<DataKey, u32>(&DataKey::VolumeWindowLedgers).unwrap_or(100);
        let mut volume = env.storage().instance().get::<DataKey, i128>(&DataKey::VolumeInWindow).unwrap_or(0);

        // Reset window if expired
        if current_ledger >= window_start + window_ledgers as u64 {
            volume = 0;
            env.storage().instance().set(&DataKey::VolumeWindowStart, &current_ledger);
        }

        volume += amount;
        env.storage().instance().set(&DataKey::VolumeInWindow, &volume);
    }

    // ─── Admin: Signer Management ─────────────────────────────────────

    pub fn add_signer(env: Env, admin: Address, new_signer: Address) -> Result<(), BridgeError> {
        if admin != read_admin(&env) {
            return Err(BridgeError::NotAdmin);
        }
        admin.require_auth();

        if Self::is_signer(&env, &new_signer) {
            return Err(BridgeError::DuplicateSigner);
        }

        env.storage().instance().set(&DataKey::Signer(new_signer.clone()), &true);
        let count = env.storage().instance().get::<DataKey, u32>(&DataKey::SignerCount).unwrap_or(0);
        env.storage().instance().set(&DataKey::SignerCount, &(count + 1));

        env.events().publish(
            (TOPIC_SIGNER, symbol_short!("added")),
            new_signer,
        );

        Ok(())
    }

    pub fn remove_signer(env: Env, admin: Address, signer: Address) -> Result<(), BridgeError> {
        if admin != read_admin(&env) {
            return Err(BridgeError::NotAdmin);
        }
        admin.require_auth();

        if !Self::is_signer(&env, &signer) {
            return Err(BridgeError::SignerNotFound);
        }

        let count = env.storage().instance().get::<DataKey, u32>(&DataKey::SignerCount).unwrap_or(0);
        let threshold = read_threshold(&env);

        if count <= 1 {
            return Err(BridgeError::CannotRemoveLastSigner);
        }

        if count - 1 < threshold {
            return Err(BridgeError::CannotRemoveBelowThreshold);
        }

        env.storage().instance().set(&DataKey::Signer(signer.clone()), &false);
        env.storage().instance().set(&DataKey::SignerCount, &(count - 1));

        env.events().publish(
            (TOPIC_SIGNER, symbol_short!("removed")),
            signer,
        );

        Ok(())
    }

    // ─── Admin: Config ────────────────────────────────────────────────

    pub fn set_threshold(env: Env, admin: Address, new_threshold: u32) -> Result<(), BridgeError> {
        if admin != read_admin(&env) {
            return Err(BridgeError::NotAdmin);
        }
        admin.require_auth();

        let count = env.storage().instance().get::<DataKey, u32>(&DataKey::SignerCount).unwrap_or(0);
        if new_threshold == 0 || new_threshold > count {
            return Err(BridgeError::InvalidThreshold);
        }

        env.storage().instance().set(&DataKey::Threshold, &new_threshold);

        env.events().publish(
            (TOPIC_CONFIG, symbol_short!("threshold")),
            new_threshold,
        );

        Ok(())
    }

    pub fn set_volume_cap(
        env: Env,
        admin: Address,
        cap: i128,
        window_ledgers: u32,
    ) -> Result<(), BridgeError> {
        if admin != read_admin(&env) {
            return Err(BridgeError::NotAdmin);
        }
        admin.require_auth();

        env.storage().instance().set(&DataKey::VolumeCap, &cap);
        env.storage().instance().set(&DataKey::VolumeWindowLedgers, &window_ledgers);

        env.events().publish(
            (TOPIC_CONFIG, symbol_short!("vol_cap")),
            (cap, window_ledgers),
        );

        Ok(())
    }

    pub fn pause(env: Env, admin: Address) -> Result<(), BridgeError> {
        if admin != read_admin(&env) {
            return Err(BridgeError::NotAdmin);
        }
        admin.require_auth();

        env.storage().instance().set(&DataKey::Paused, &true);

        env.events().publish(
            (TOPIC_CONFIG, symbol_short!("paused")),
            (),
        );

        Ok(())
    }

    pub fn unpause(env: Env, admin: Address) -> Result<(), BridgeError> {
        if admin != read_admin(&env) {
            return Err(BridgeError::NotAdmin);
        }
        admin.require_auth();

        env.storage().instance().set(&DataKey::Paused, &false);

        env.events().publish(
            (TOPIC_CONFIG, symbol_short!("unpaused")),
            (),
        );

        Ok(())
    }

    pub fn set_admin(env: Env, admin: Address, new_admin: Address) -> Result<(), BridgeError> {
        if admin != read_admin(&env) {
            return Err(BridgeError::NotAdmin);
        }
        admin.require_auth();

        env.storage().instance().set(&DataKey::Admin, &new_admin);

        env.events().publish(
            (TOPIC_CONFIG, symbol_short!("admin")),
            new_admin,
        );

        Ok(())
    }

    pub fn set_max_proposal_age(
        env: Env,
        admin: Address,
        max_age: u32,
    ) -> Result<(), BridgeError> {
        if admin != read_admin(&env) {
            return Err(BridgeError::NotAdmin);
        }
        admin.require_auth();

        env.storage().instance().set(&DataKey::MaxProposalAge, &max_age);
        Ok(())
    }

    // ─── Read-only ────────────────────────────────────────────────────

    pub fn is_signer(env: &Env, address: &Address) -> bool {
        env.storage()
            .instance()
            .get::<DataKey, bool>(&DataKey::Signer(address.clone()))
            .unwrap_or(false)
    }

    pub fn get_proposal(env: &Env, proposal_id: u32) -> Result<Proposal, BridgeError> {
        env.storage()
            .instance()
            .get::<DataKey, Proposal>(&DataKey::Proposal(proposal_id))
            .ok_or(BridgeError::ProposalNotFound)
    }

    pub fn is_deposit_processed(env: &Env, deposit_id: BytesN<32>) -> bool {
        let nonce = deposit_id_to_nonce(&deposit_id);
        env.storage().instance().has(&DataKey::Nonce(nonce))
    }

    pub fn config(env: Env) -> BridgeConfig {
        BridgeConfig {
            admin: read_admin(&env),
            threshold: read_threshold(&env),
            signer_count: env.storage().instance().get::<DataKey, u32>(&DataKey::SignerCount).unwrap_or(0),
            volume_cap: env.storage().instance().get::<DataKey, i128>(&DataKey::VolumeCap).unwrap_or(0),
            volume_window_ledgers: env.storage().instance().get::<DataKey, u32>(&DataKey::VolumeWindowLedgers).unwrap_or(0),
            paused: is_paused(&env),
            max_proposal_age: env.storage().instance().get::<DataKey, u32>(&DataKey::MaxProposalAge).unwrap_or(100),
        }
    }

    pub fn volume_stats(env: Env) -> (i128, u64, u32, i128) {
        let volume = env.storage().instance().get::<DataKey, i128>(&DataKey::VolumeInWindow).unwrap_or(0);
        let window_start = env.storage().instance().get::<DataKey, u64>(&DataKey::VolumeWindowStart).unwrap_or(0);
        let window_ledgers = env.storage().instance().get::<DataKey, u32>(&DataKey::VolumeWindowLedgers).unwrap_or(0);
        let cap = env.storage().instance().get::<DataKey, i128>(&DataKey::VolumeCap).unwrap_or(0);
        (volume, window_start, window_ledgers, cap)
    }

    pub fn admin(env: Env) -> Address {
        read_admin(&env)
    }

    pub fn paused(env: Env) -> bool {
        is_paused(&env)
    }

    pub fn threshold(env: Env) -> u32 {
        read_threshold(&env)
    }

    pub fn signer_count(env: Env) -> u32 {
        env.storage().instance().get::<DataKey, u32>(&DataKey::SignerCount).unwrap_or(0)
    }

    pub fn proposal_count(env: Env) -> u32 {
        next_proposal_id(&env)
    }
}

// ─── Helpers ──────────────────────────────────────────────────────────

/// Convert a 32-byte deposit_id to a u64 nonce for storage.
fn deposit_id_to_nonce(deposit_id: &BytesN<32>) -> u64 {
    let bytes = deposit_id.to_array();
    u64::from_be_bytes([
        bytes[0], bytes[1], bytes[2], bytes[3],
        bytes[4], bytes[5], bytes[6], bytes[7],
    ])
}

// ─── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod test {
    extern crate std;
    use super::*;
    use soroban_sdk::{testutils::{Address as _, Ledger}, Address, Env, BytesN};

    fn setup() -> (Env, BridgeMultisigClient<'static>, Address, [Address; 3]) {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(BridgeMultisig, ());
        let client = BridgeMultisigClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let signer1 = Address::generate(&env);
        let signer2 = Address::generate(&env);
        let signer3 = Address::generate(&env);

        let mut signers = soroban_sdk::Vec::new(&env);
        signers.push_back(signer1.clone());
        signers.push_back(signer2.clone());
        signers.push_back(signer3.clone());

        client.initialize(&admin, &signers, &2, &1_000_000_000, &100, &100);

        (env, client, admin, [signer1, signer2, signer3])
    }

    fn make_deposit_id(env: &Env, seed: u8) -> BytesN<32> {
        let mut bytes = [0u8; 32];
        bytes[0] = seed;
        BytesN::from_array(env, &bytes)
    }

    #[test]
    fn test_initialize() {
        let (_env, client, admin, signers) = setup();
        assert_eq!(client.admin(), admin);
        assert_eq!(client.threshold(), 2);
        assert_eq!(client.signer_count(), 3);
        assert!(!client.paused());
    }

    #[test]
    fn test_propose_and_approve() {
        let (env, client, _admin, signers) = setup();

        let deposit_id = make_deposit_id(&env, 1);
        let target = Address::generate(&env);

        // Signer1 proposes
        let proposal_id = client.propose(
            &signers[0],
            &ProposalKind::MintWpi,
            &target,
            &100_000_000,
            &deposit_id,
        );
        assert_eq!(proposal_id, 0);

        // Proposal is pending
        let proposal = client.get_proposal(&proposal_id);
        assert_eq!(proposal.status, ProposalStatus::Pending);
        assert_eq!(proposal.approvals.len(), 1);

        // Signer2 approves — threshold met, should auto-execute
        let status = client.approve(&signers[1], &proposal_id);
        assert_eq!(status, ProposalStatus::Executed);

        // Nonce marked as used (replay protection)
        assert!(client.is_deposit_processed(&deposit_id));

        // Proposal is executed
        let proposal = client.get_proposal(&proposal_id);
        assert_eq!(proposal.status, ProposalStatus::Executed);
        assert!(proposal.executed_at.is_some());
    }

    #[test]
    fn test_replay_protection() {
        let (env, client, _admin, signers) = setup();

        let deposit_id = make_deposit_id(&env, 1);
        let target = Address::generate(&env);

        // First proposal succeeds
        let proposal_id = client.propose(
            &signers[0],
            &ProposalKind::MintWpi,
            &target,
            &100_000_000,
            &deposit_id,
        );
        let status = client.approve(&signers[1], &proposal_id);
        assert_eq!(status, ProposalStatus::Executed);

        // Second proposal with same deposit_id fails
        let result = client.try_propose(
            &signers[0],
            &ProposalKind::MintWpi,
            &target,
            &100_000_000,
            &deposit_id,
        );
        assert_eq!(result, Err(Ok(BridgeError::DepositAlreadyProcessed)));
    }

    #[test]
    fn test_volume_circuit_breaker() {
        let (env, client, _admin, signers) = setup();

        let target = Address::generate(&env);

        // First mint within cap
        let deposit_id1 = make_deposit_id(&env, 1);
        let proposal_id = client.propose(
            &signers[0],
            &ProposalKind::MintWpi,
            &target,
            &500_000_000,
            &deposit_id1,
        );
        client.approve(&signers[1], &proposal_id);

        // Second mint within cap
        let deposit_id2 = make_deposit_id(&env, 2);
        let proposal_id = client.propose(
            &signers[0],
            &ProposalKind::MintWpi,
            &target,
            &400_000_000,
            &deposit_id2,
        );
        client.approve(&signers[1], &proposal_id);

        // Third mint exceeds cap (500M + 400M + 200M = 1.1B > 1B cap)
        let deposit_id3 = make_deposit_id(&env, 3);
        let result = client.try_propose(
            &signers[0],
            &ProposalKind::MintWpi,
            &target,
            &200_000_000,
            &deposit_id3,
        );
        assert_eq!(result, Err(Ok(BridgeError::VolumeCapExceeded)));
    }

    #[test]
    fn test_threshold_not_met() {
        let (env, client, _admin, signers) = setup();

        let deposit_id = make_deposit_id(&env, 1);
        let target = Address::generate(&env);

        // Only one signer approves (need 2)
        let proposal_id = client.propose(
            &signers[0],
            &ProposalKind::MintWpi,
            &target,
            &100_000_000,
            &deposit_id,
        );

        let proposal = client.get_proposal(&proposal_id);
        assert_eq!(proposal.status, ProposalStatus::Pending);
        assert_eq!(proposal.approvals.len(), 1);
    }

    #[test]
    fn test_duplicate_approval_fails() {
        let (env, client, _admin, signers) = setup();

        let deposit_id = make_deposit_id(&env, 1);
        let target = Address::generate(&env);

        let proposal_id = client.propose(
            &signers[0],
            &ProposalKind::MintWpi,
            &target,
            &100_000_000,
            &deposit_id,
        );

        // Signer1 tries to approve again
        let result = client.try_approve(&signers[0], &proposal_id);
        assert_eq!(result, Err(Ok(BridgeError::DuplicateApproval)));
    }

    #[test]
    fn test_non_signer_cannot_propose() {
        let (env, client, _admin, _signers) = setup();

        let rando = Address::generate(&env);
        let deposit_id = make_deposit_id(&env, 1);
        let target = Address::generate(&env);

        let result = client.try_propose(
            &rando,
            &ProposalKind::MintWpi,
            &target,
            &100_000_000,
            &deposit_id,
        );
        assert_eq!(result, Err(Ok(BridgeError::NotSigner)));
    }

    #[test]
    fn test_admin_add_remove_signer() {
        let (env, client, admin, signers) = setup();

        let new_signer = Address::generate(&env);

        // Add signer
        client.add_signer(&admin, &new_signer);
        assert!(client.is_signer(&new_signer));
        assert_eq!(client.signer_count(), 4);

        // Remove signer
        client.remove_signer(&admin, &new_signer);
        assert!(!client.is_signer(&new_signer));
        assert_eq!(client.signer_count(), 3);
    }

    #[test]
    fn test_cannot_remove_below_threshold() {
        let (env, client, admin, signers) = setup();

        // Threshold is 2, we have 3 signers. Removing one leaves 2 >= threshold. OK.
        client.remove_signer(&admin, &signers[2]);
        assert_eq!(client.signer_count(), 2);

        // Now try to remove another — would leave 1 < threshold
        let result = client.try_remove_signer(&admin, &signers[1]);
        assert_eq!(result, Err(Ok(BridgeError::CannotRemoveBelowThreshold)));
    }

    #[test]
    fn test_admin_set_threshold() {
        let (env, client, admin, _signers) = setup();

        client.set_threshold(&admin, &3);
        assert_eq!(client.threshold(), 3);
    }

    #[test]
    fn test_pause_unpause() {
        let (env, client, admin, _signers) = setup();

        client.pause(&admin);
        assert!(client.paused());

        client.unpause(&admin);
        assert!(!client.paused());
    }

    #[test]
    fn test_cannot_propose_when_paused() {
        let (env, client, admin, signers) = setup();

        client.pause(&admin);

        let deposit_id = make_deposit_id(&env, 1);
        let target = Address::generate(&env);

        let result = client.try_propose(
            &signers[0],
            &ProposalKind::MintWpi,
            &target,
            &100_000_000,
            &deposit_id,
        );
        assert_eq!(result, Err(Ok(BridgeError::AlreadyPaused)));
    }

    #[test]
    fn test_proposal_expiration() {
        let (env, client, _admin, signers) = setup();

        let deposit_id = make_deposit_id(&env, 1);
        let target = Address::generate(&env);

        let proposal_id = client.propose(
            &signers[0],
            &ProposalKind::MintWpi,
            &target,
            &100_000_000,
            &deposit_id,
        );

        // Advance past max_proposal_age (100 ledgers)
        env.ledger().set_sequence_number(150);

        let result = client.try_approve(&signers[1], &proposal_id);
        assert_eq!(result, Err(Ok(BridgeError::ProposalExpired)));
    }

    #[test]
    fn test_volume_stats() {
        let (env, client, _admin, signers) = setup();

        let target = Address::generate(&env);
        let deposit_id = make_deposit_id(&env, 1);

        let proposal_id = client.propose(
            &signers[0],
            &ProposalKind::MintWpi,
            &target,
            &100_000_000,
            &deposit_id,
        );
        client.approve(&signers[1], &proposal_id);

        let (volume, _start, _window, cap) = client.volume_stats();
        assert_eq!(volume, 100_000_000);
        assert_eq!(cap, 1_000_000_000);
    }
}
