#![no_std]

//! Escrow Manager — milestone-based escrow for ZyraPay, Pi Wave, and any Zyrachain app.
//!
//! Lifecycle: create → fund → submit milestone → approve → release → complete
//! Disputes: either party can dispute, arbitrator resolves

use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short, Address, Env, Symbol, Vec,
};
use hub_errors::HubError;
use hub_types::{EscrowConfig, EscrowState, EscrowStatus, Milestone, MilestoneStatus};

// ─── Storage Keys ────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Admin,
    Paused,
    EscrowState(u32),
    EscrowConfig(u32),
    EscrowCount,
}

// ─── Helpers ─────────────────────────────────────────────────────────

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

fn next_escrow_id(env: &Env) -> u32 {
    let count: u32 = env
        .storage()
        .instance()
        .get::<DataKey, u32>(&DataKey::EscrowCount)
        .unwrap_or(0);
    env.storage()
        .instance()
        .set(&DataKey::EscrowCount, &(count + 1));
    count
}

fn read_escrow_state(env: &Env, escrow_id: u32) -> Result<EscrowState, HubError> {
    env.storage()
        .instance()
        .get::<DataKey, EscrowState>(&DataKey::EscrowState(escrow_id))
        .ok_or(HubError::EscrowNotFound)
}

fn write_escrow_state(env: &Env, escrow_id: u32, state: &EscrowState) {
    env.storage()
        .instance()
        .set(&DataKey::EscrowState(escrow_id), state);
}

fn read_escrow_config(env: &Env, escrow_id: u32) -> Result<EscrowConfig, HubError> {
    env.storage()
        .instance()
        .get::<DataKey, EscrowConfig>(&DataKey::EscrowConfig(escrow_id))
        .ok_or(HubError::EscrowNotFound)
}

fn write_escrow_config(env: &Env, escrow_id: u32, config: &EscrowConfig) {
    env.storage()
        .instance()
        .set(&DataKey::EscrowConfig(escrow_id), config);
}

// ─── Contract ────────────────────────────────────────────────────────

#[contract]
pub struct EscrowManager;

#[contractimpl]
impl EscrowManager {
    pub fn initialize(env: Env, admin: Address) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("already initialized");
        }
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::Paused, &false);
        env.storage().instance().set(&DataKey::EscrowCount, &0u32);
    }

    /// Create a new escrow. Returns the escrow_id.
    pub fn create_escrow(
        env: Env,
        funder: Address,
        receiver: Address,
        arbitrator: Address,
        asset: Address,
        milestones: Vec<Milestone>,
        fee_bps: u32,
        deadline_ledger: u32,
    ) -> Result<u32, HubError> {
        if is_paused(&env) {
            return Err(HubError::Paused);
        }
        if fee_bps > 10000 {
            return Err(HubError::FeeBpsExceedsMaximum);
        }
        if milestones.is_empty() {
            return Err(HubError::InvalidArgument);
        }
        if deadline_ledger <= env.ledger().sequence() {
            return Err(HubError::DeadlineAlreadyPassed);
        }

        funder.require_auth();

        let escrow_id = next_escrow_id(&env);

        let config = EscrowConfig {
            funder: funder.clone(),
            receiver: receiver.clone(),
            arbitrator,
            asset,
            milestones: milestones.clone(),
            fee_bps,
            deadline_ledger,
        };

        // Initialize milestone statuses
        let mut statuses = Vec::new(&env);
        for _ in 0..milestones.len() {
            statuses.push_back(MilestoneStatus::Pending);
        }

        let state = EscrowState {
            escrow_id,
            config: config.clone(),
            total_deposited: 0,
            total_released: 0,
            milestones_completed: 0,
            milestone_statuses: statuses,
            status: EscrowStatus::Active,
        };

        write_escrow_config(&env, escrow_id, &config);
        write_escrow_state(&env, escrow_id, &state);

        env.events().publish(
            (symbol_short!("esc_crt"), escrow_id),
            (&funder, &receiver),
        );

        Ok(escrow_id)
    }

    /// Fund an escrow by depositing tokens.
    pub fn fund_escrow(env: Env, escrow_id: u32, amount: i128) -> Result<(), HubError> {
        if is_paused(&env) {
            return Err(HubError::Paused);
        }
        if amount <= 0 {
            return Err(HubError::InvalidArgument);
        }

        let mut state = read_escrow_state(&env, escrow_id)?;
        if state.status != EscrowStatus::Active {
            return Err(HubError::InvalidEscrowStatus);
        }

        state.config.funder.require_auth();

        let asset = soroban_sdk::token::Client::new(&env, &state.config.asset);
        let vault = env.current_contract_address();
        asset.transfer(&state.config.funder, &vault, &amount);

        state.total_deposited += amount;
        write_escrow_state(&env, escrow_id, &state);

        env.events()
            .publish((symbol_short!("esc_fnd"), escrow_id), amount);

        Ok(())
    }

    /// Submit a milestone for approval (receiver action).
    pub fn submit_milestone(env: Env, escrow_id: u32, milestone_idx: u32) -> Result<(), HubError> {
        if is_paused(&env) {
            return Err(HubError::Paused);
        }

        let mut state = read_escrow_state(&env, escrow_id)?;
        if state.status != EscrowStatus::Active {
            return Err(HubError::InvalidEscrowStatus);
        }
        if milestone_idx >= state.config.milestones.len() {
            return Err(HubError::InvalidArgument);
        }

        state.config.receiver.require_auth();

        let cur = state
            .milestone_statuses
            .get(milestone_idx)
            .ok_or(HubError::InvalidArgument)?;
        if cur != MilestoneStatus::Pending {
            return Err(HubError::MilestoneAlreadySubmitted);
        }

        state
            .milestone_statuses
            .set(milestone_idx, MilestoneStatus::Submitted);
        write_escrow_state(&env, escrow_id, &state);

        env.events().publish(
            (symbol_short!("esc_sub"), escrow_id, milestone_idx),
            &state.config.receiver,
        );

        Ok(())
    }

    /// Approve a milestone (designated approver action).
    pub fn approve_milestone(env: Env, escrow_id: u32, milestone_idx: u32) -> Result<(), HubError> {
        if is_paused(&env) {
            return Err(HubError::Paused);
        }

        let mut state = read_escrow_state(&env, escrow_id)?;
        if state.status != EscrowStatus::Active {
            return Err(HubError::InvalidEscrowStatus);
        }
        if milestone_idx >= state.config.milestones.len() {
            return Err(HubError::InvalidArgument);
        }

        let milestone = state
            .config
            .milestones
            .get(milestone_idx)
            .ok_or(HubError::InvalidArgument)?;
        milestone.approver.require_auth();

        let cur = state
            .milestone_statuses
            .get(milestone_idx)
            .ok_or(HubError::InvalidArgument)?;
        if cur != MilestoneStatus::Submitted {
            return Err(HubError::MilestoneNotApprovable);
        }

        state
            .milestone_statuses
            .set(milestone_idx, MilestoneStatus::Approved);
        state.milestones_completed += 1;
        write_escrow_state(&env, escrow_id, &state);

        env.events().publish(
            (symbol_short!("esc_apr"), escrow_id, milestone_idx),
            &milestone.approver,
        );

        Ok(())
    }

    /// Reject a milestone (designated approver action).
    pub fn reject_milestone(env: Env, escrow_id: u32, milestone_idx: u32) -> Result<(), HubError> {
        if is_paused(&env) {
            return Err(HubError::Paused);
        }

        let mut state = read_escrow_state(&env, escrow_id)?;
        if state.status != EscrowStatus::Active {
            return Err(HubError::InvalidEscrowStatus);
        }
        if milestone_idx >= state.config.milestones.len() {
            return Err(HubError::InvalidArgument);
        }

        let milestone = state
            .config
            .milestones
            .get(milestone_idx)
            .ok_or(HubError::InvalidArgument)?;
        milestone.approver.require_auth();

        let cur = state
            .milestone_statuses
            .get(milestone_idx)
            .ok_or(HubError::InvalidArgument)?;
        if cur != MilestoneStatus::Submitted {
            return Err(HubError::MilestoneNotApprovable);
        }

        state
            .milestone_statuses
            .set(milestone_idx, MilestoneStatus::Rejected);
        write_escrow_state(&env, escrow_id, &state);

        env.events().publish(
            (symbol_short!("esc_rjt"), escrow_id, milestone_idx),
            &milestone.approver,
        );

        Ok(())
    }

    /// Release funds to receiver for all approved milestones.
    pub fn release_funds(env: Env, escrow_id: u32) -> Result<(), HubError> {
        if is_paused(&env) {
            return Err(HubError::Paused);
        }

        let mut state = read_escrow_state(&env, escrow_id)?;
        if state.status != EscrowStatus::Active {
            return Err(HubError::InvalidEscrowStatus);
        }

        // Calculate release amount from approved milestones
        let mut release_amount: i128 = 0;
        for i in 0..state.config.milestones.len() {
            let status = state
                .milestone_statuses
                .get(i)
                .ok_or(HubError::InvalidArgument)?;
            if status == MilestoneStatus::Approved {
                let m = state
                    .config
                    .milestones
                    .get(i)
                    .ok_or(HubError::InvalidArgument)?;
                release_amount += m.amount;
            }
        }

        release_amount -= state.total_released;
        if release_amount <= 0 {
            return Err(HubError::InsufficientEscrowBalance);
        }

        let available = state.total_deposited - state.total_released;
        if available < release_amount {
            return Err(HubError::InsufficientEscrowBalance);
        }

        let asset = soroban_sdk::token::Client::new(&env, &state.config.asset);
        let vault = env.current_contract_address();
        asset.transfer(&vault, &state.config.receiver, &release_amount);

        state.total_released += release_amount;

        if state.milestones_completed == state.config.milestones.len() {
            state.status = EscrowStatus::Completed;
        }

        write_escrow_state(&env, escrow_id, &state);

        env.events().publish(
            (symbol_short!("esc_rel"), escrow_id),
            (&release_amount, &state.config.receiver),
        );

        Ok(())
    }

    /// Refund remaining funds to funder (after deadline or all milestones rejected).
    pub fn refund(env: Env, escrow_id: u32) -> Result<(), HubError> {
        if is_paused(&env) {
            return Err(HubError::Paused);
        }

        let mut state = read_escrow_state(&env, escrow_id)?;
        if state.status != EscrowStatus::Active {
            return Err(HubError::InvalidEscrowStatus);
        }

        state.config.funder.require_auth();

        let remaining = state.total_deposited - state.total_released;
        if remaining <= 0 {
            return Err(HubError::InsufficientEscrowBalance);
        }

        let all_rejected = state
            .milestone_statuses
            .iter()
            .all(|s| s == MilestoneStatus::Rejected);
        let deadline_passed = env.ledger().sequence() >= state.config.deadline_ledger;

        if !all_rejected && !deadline_passed {
            return Err(HubError::DeadlineNotReached);
        }

        let asset = soroban_sdk::token::Client::new(&env, &state.config.asset);
        let vault = env.current_contract_address();
        asset.transfer(&vault, &state.config.funder, &remaining);

        state.total_released += remaining;
        state.status = EscrowStatus::Refunded;
        write_escrow_state(&env, escrow_id, &state);

        env.events().publish(
            (symbol_short!("esc_ref"), escrow_id),
            (&remaining, &state.config.funder),
        );

        Ok(())
    }

    /// Dispute the escrow (funder or receiver can dispute).
    pub fn dispute(env: Env, escrow_id: u32, disputer: Address) -> Result<(), HubError> {
        if is_paused(&env) {
            return Err(HubError::Paused);
        }

        let mut state = read_escrow_state(&env, escrow_id)?;
        if state.status != EscrowStatus::Active {
            return Err(HubError::InvalidEscrowStatus);
        }

        if disputer != state.config.funder && disputer != state.config.receiver {
            return Err(HubError::Unauthorized);
        }
        disputer.require_auth();

        state.status = EscrowStatus::Disputed;
        write_escrow_state(&env, escrow_id, &state);

        env.events()
            .publish((symbol_short!("esc_dsp"), escrow_id), &disputer);

        Ok(())
    }

    /// Resolve a dispute (arbitrator only).
    /// outcome: "release" or "refund"
    pub fn resolve(env: Env, escrow_id: u32, outcome: Symbol) -> Result<(), HubError> {
        if is_paused(&env) {
            return Err(HubError::Paused);
        }

        let mut state = read_escrow_state(&env, escrow_id)?;
        if state.status != EscrowStatus::Disputed {
            return Err(HubError::InvalidEscrowStatus);
        }

        state.config.arbitrator.require_auth();

        let remaining = state.total_deposited - state.total_released;
        if remaining <= 0 {
            return Err(HubError::InsufficientEscrowBalance);
        }

        let asset = soroban_sdk::token::Client::new(&env, &state.config.asset);
        let vault = env.current_contract_address();

        let recipient = if outcome == symbol_short!("release") {
            &state.config.receiver
        } else if outcome == symbol_short!("refund") {
            &state.config.funder
        } else {
            return Err(HubError::InvalidArgument);
        };

        asset.transfer(&vault, recipient, &remaining);

        state.total_released += remaining;
        state.status = if outcome == symbol_short!("release") {
            EscrowStatus::Completed
        } else {
            EscrowStatus::Refunded
        };
        write_escrow_state(&env, escrow_id, &state);

        env.events().publish(
            (symbol_short!("esc_res"), escrow_id),
            (&state.config.arbitrator, outcome),
        );

        Ok(())
    }

    // ─── Admin ──────────────────────────────────────────────────────

    pub fn set_paused(env: Env, admin: Address, paused: bool) -> Result<(), HubError> {
        let current_admin = read_admin(&env);
        if admin != current_admin {
            return Err(HubError::Unauthorized);
        }
        admin.require_auth();
        env.storage().instance().set(&DataKey::Paused, &paused);
        Ok(())
    }

    pub fn set_admin(env: Env, admin: Address, new_admin: Address) -> Result<(), HubError> {
        let current_admin = read_admin(&env);
        if admin != current_admin {
            return Err(HubError::Unauthorized);
        }
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &new_admin);
        Ok(())
    }

    // ─── Queries ────────────────────────────────────────────────────

    pub fn get_escrow(env: Env, escrow_id: u32) -> Result<EscrowState, HubError> {
        read_escrow_state(&env, escrow_id)
    }

    pub fn get_config(env: Env, escrow_id: u32) -> Result<EscrowConfig, HubError> {
        read_escrow_config(&env, escrow_id)
    }

    pub fn get_milestone_status(
        env: Env,
        escrow_id: u32,
        milestone_idx: u32,
    ) -> Result<MilestoneStatus, HubError> {
        let state = read_escrow_state(&env, escrow_id)?;
        state
            .milestone_statuses
            .get(milestone_idx)
            .ok_or(HubError::InvalidArgument)
    }

    pub fn remaining_balance(env: Env, escrow_id: u32) -> Result<i128, HubError> {
        let state = read_escrow_state(&env, escrow_id)?;
        Ok(state.total_deposited - state.total_released)
    }

    pub fn admin(env: Env) -> Address {
        read_admin(&env)
    }

    pub fn is_paused(env: Env) -> bool {
        is_paused(&env)
    }

    pub fn escrow_count(env: Env) -> u32 {
        env.storage()
            .instance()
            .get::<DataKey, u32>(&DataKey::EscrowCount)
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod test {
    extern crate std;
    use super::{EscrowManager, EscrowManagerClient};
    use hub_errors::HubError;
    use hub_types::{EscrowStatus, Milestone, MilestoneStatus};
    use soroban_sdk::{
        contract, contractimpl, contracttype, testutils::Address as _, Address, BytesN, Env, Vec,
    };

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
            let b: i128 = env.storage().instance().get::<MockKey, i128>(&MockKey::Bal(to.clone())).unwrap_or(0);
            env.storage().instance().set(&MockKey::Bal(to), &(b + amount));
        }
        pub fn transfer(env: Env, from: Address, to: Address, amount: i128) {
            let fb: i128 = env.storage().instance().get::<MockKey, i128>(&MockKey::Bal(from.clone())).unwrap_or(0);
            let tb: i128 = env.storage().instance().get::<MockKey, i128>(&MockKey::Bal(to.clone())).unwrap_or(0);
            env.storage().instance().set(&MockKey::Bal(from), &(fb - amount));
            env.storage().instance().set(&MockKey::Bal(to), &(tb + amount));
        }
    }

    fn setup() -> (Env, EscrowManagerClient<'static>, Address, Address, Address) {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(EscrowManager, ());
        let client = EscrowManagerClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin);
        let funder = Address::generate(&env);
        let receiver = Address::generate(&env);
        (env, client, admin, funder, receiver)
    }

    fn setup_funded() -> (Env, EscrowManagerClient<'static>, Address, Address, u32) {
        let env = Env::default();
        env.mock_all_auths();
        let mgr_id = env.register(EscrowManager, ());
        let mgr = EscrowManagerClient::new(&env, &mgr_id);
        let admin = Address::generate(&env);
        mgr.initialize(&admin);
        let funder = Address::generate(&env);
        let receiver = Address::generate(&env);
        let arbitrator = Address::generate(&env);
        let token_id = env.register(MockToken, ());
        let token = MockTokenClient::new(&env, &token_id);
        token.initialize(&admin);
        token.mint(&funder, &100_000);

        let approver0 = Address::generate(&env);
        let approver1 = Address::generate(&env);
        let mut milestones = Vec::new(&env);
        milestones.push_back(Milestone {
            description: BytesN::from_array(&env, &[1u8; 32]),
            amount: 1000,
            approver: approver0,
        });
        milestones.push_back(Milestone {
            description: BytesN::from_array(&env, &[2u8; 32]),
            amount: 2000,
            approver: approver1,
        });

        let escrow_id = mgr.create_escrow(
            &funder, &receiver, &arbitrator, &token_id, &milestones, &100,
            &(env.ledger().sequence() + 100),
        );
        mgr.fund_escrow(&escrow_id, &3000);
        (env, mgr, funder, receiver, escrow_id)
    }

    fn make_milestones(env: &Env) -> Vec<Milestone> {
        let mut m = Vec::new(env);
        m.push_back(Milestone {
            description: BytesN::from_array(env, &[1u8; 32]),
            amount: 1000,
            approver: Address::generate(env),
        });
        m.push_back(Milestone {
            description: BytesN::from_array(env, &[2u8; 32]),
            amount: 2000,
            approver: Address::generate(env),
        });
        m
    }

    #[test]
    fn test_create_escrow() {
        let (env, client, _, funder, receiver) = setup();
        let arbitrator = Address::generate(&env);
        let asset = Address::generate(&env);
        let milestones = make_milestones(&env);
        let escrow_id = client.create_escrow(
            &funder, &receiver, &arbitrator, &asset, &milestones, &100,
            &(env.ledger().sequence() + 100),
        );
        assert_eq!(escrow_id, 0);
        let state = client.get_escrow(&escrow_id);
        assert_eq!(state.status, EscrowStatus::Active);
        assert_eq!(state.total_deposited, 0);
        assert_eq!(state.milestones_completed, 0);
    }

    #[test]
    fn test_create_escrow_empty_milestones_fails() {
        let (env, client, _, funder, receiver) = setup();
        let arbitrator = Address::generate(&env);
        let asset = Address::generate(&env);
        let result = client.try_create_escrow(
            &funder, &receiver, &arbitrator, &asset, &Vec::new(&env), &100,
            &(env.ledger().sequence() + 100),
        );
        assert_eq!(result, Err(Ok(HubError::InvalidArgument)));
    }

    #[test]
    fn test_create_escrow_fee_too_high() {
        let (env, client, _, funder, receiver) = setup();
        let arbitrator = Address::generate(&env);
        let asset = Address::generate(&env);
        let milestones = make_milestones(&env);
        let result = client.try_create_escrow(
            &funder, &receiver, &arbitrator, &asset, &milestones, &10001,
            &(env.ledger().sequence() + 100),
        );
        assert_eq!(result, Err(Ok(HubError::FeeBpsExceedsMaximum)));
    }

    #[test]
    fn test_create_escrow_past_deadline_fails() {
        let (env, client, _, funder, receiver) = setup();
        let arbitrator = Address::generate(&env);
        let asset = Address::generate(&env);
        let milestones = make_milestones(&env);
        let result = client.try_create_escrow(
            &funder, &receiver, &arbitrator, &asset, &milestones, &100,
            &env.ledger().sequence(),
        );
        assert_eq!(result, Err(Ok(HubError::DeadlineAlreadyPassed)));
    }

    #[test]
    fn test_escrow_count_increments() {
        let (env, client, _, funder, receiver) = setup();
        let arbitrator = Address::generate(&env);
        let asset = Address::generate(&env);
        let milestones = make_milestones(&env);
        assert_eq!(client.escrow_count(), 0);
        client.create_escrow(
            &funder, &receiver, &arbitrator, &asset, &milestones, &100,
            &(env.ledger().sequence() + 100),
        );
        assert_eq!(client.escrow_count(), 1);
    }

    #[test]
    fn test_reject_milestone() {
        let (env, client, _, funder, receiver) = setup();
        let arbitrator = Address::generate(&env);
        let asset = Address::generate(&env);
        let mut milestones = make_milestones(&env);
        let approver = Address::generate(&env);
        milestones.set(0, Milestone {
            description: BytesN::from_array(&env, &[1u8; 32]),
            amount: 1000, approver: approver.clone(),
        });
        let escrow_id = client.create_escrow(
            &funder, &receiver, &arbitrator, &asset, &milestones, &100,
            &(env.ledger().sequence() + 100),
        );
        client.submit_milestone(&escrow_id, &0);
        client.reject_milestone(&escrow_id, &0);
        assert_eq!(client.get_milestone_status(&escrow_id, &0), MilestoneStatus::Rejected);
    }

    #[test]
    fn test_dispute_and_resolve() {
        let (env, mgr, _, _, escrow_id) = setup_funded();
        let state = mgr.get_escrow(&escrow_id);
        let funder = state.config.funder.clone();

        mgr.dispute(&escrow_id, &funder);
        let state = mgr.get_escrow(&escrow_id);
        assert_eq!(state.status, EscrowStatus::Disputed);

        mgr.resolve(&escrow_id, &soroban_sdk::symbol_short!("refund"));
        let state = mgr.get_escrow(&escrow_id);
        assert_eq!(state.status, EscrowStatus::Refunded);
    }

    #[test]
    fn test_non_participant_cannot_dispute() {
        let (env, client, _, funder, receiver) = setup();
        let arbitrator = Address::generate(&env);
        let asset = Address::generate(&env);
        let milestones = make_milestones(&env);
        let rando = Address::generate(&env);
        let escrow_id = client.create_escrow(
            &funder, &receiver, &arbitrator, &asset, &milestones, &100,
            &(env.ledger().sequence() + 100),
        );
        let result = client.try_dispute(&escrow_id, &rando);
        assert_eq!(result, Err(Ok(HubError::Unauthorized)));
    }

    #[test]
    fn test_pause_blocks_operations() {
        let (env, client, admin, funder, receiver) = setup();
        let arbitrator = Address::generate(&env);
        let asset = Address::generate(&env);
        let milestones = make_milestones(&env);
        client.set_paused(&admin, &true);
        let result = client.try_create_escrow(
            &funder, &receiver, &arbitrator, &asset, &milestones, &100,
            &(env.ledger().sequence() + 100),
        );
        assert_eq!(result, Err(Ok(HubError::Paused)));
    }

    #[test]
    fn test_resolve_wrong_status_fails() {
        let (env, client, _, funder, receiver) = setup();
        let arbitrator = Address::generate(&env);
        let asset = Address::generate(&env);
        let milestones = make_milestones(&env);
        let escrow_id = client.create_escrow(
            &funder, &receiver, &arbitrator, &asset, &milestones, &100,
            &(env.ledger().sequence() + 100),
        );
        let result = client.try_resolve(&escrow_id, &soroban_sdk::symbol_short!("refund"));
        assert_eq!(result, Err(Ok(HubError::InvalidEscrowStatus)));
    }

    #[test]
    fn test_cannot_fund_non_active_escrow() {
        let (env, mgr, funder, _, escrow_id) = setup_funded();
        mgr.dispute(&escrow_id, &funder);
        mgr.resolve(&escrow_id, &soroban_sdk::symbol_short!("refund"));
        let result = mgr.try_fund_escrow(&escrow_id, &1000);
        assert_eq!(result, Err(Ok(HubError::InvalidEscrowStatus)));
    }

    #[test]
    fn test_full_happy_path() {
        let (env, mgr, _, _, escrow_id) = setup_funded();

        mgr.submit_milestone(&escrow_id, &0);
        mgr.approve_milestone(&escrow_id, &0);
        assert_eq!(mgr.get_milestone_status(&escrow_id, &0), MilestoneStatus::Approved);

        mgr.release_funds(&escrow_id);
        assert_eq!(mgr.remaining_balance(&escrow_id), 2000);

        mgr.submit_milestone(&escrow_id, &1);
        mgr.approve_milestone(&escrow_id, &1);

        mgr.release_funds(&escrow_id);
        assert_eq!(mgr.remaining_balance(&escrow_id), 0);
        let state = mgr.get_escrow(&escrow_id);
        assert_eq!(state.status, EscrowStatus::Completed);
        assert_eq!(state.milestones_completed, 2);
    }
}
