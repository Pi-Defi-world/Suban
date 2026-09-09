#![no_std]

//! Node Operator Staking — economic incentives for honest node operation.
//! Node operators stake tokens to participate in the network.
//! Misbehavior can be slashed via admin action.

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, Env,
};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NodeInfo {
    pub operator: Address,
    pub staked: i128,
    pub rewards_accrued: i128,
    pub active: bool,
    pub joined_at: u64,
}

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Admin,
    Node(Address),
    TotalStaked,
    TotalNodes,
    MinStake,
    RewardRateBps,
    SlashedAmount,
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum StakingError {
    NotAdmin = 1,
    AlreadyInitialized = 2,
    NodeNotFound = 3,
    NodeAlreadyRegistered = 4,
    BelowMinStake = 5,
    InsufficientBalance = 6,
    NodeNotActive = 7,
    NothingToUnstake = 8,
}

#[contract]
pub struct NodeStaking;

fn read_admin(env: &Env) -> Address {
    env.storage()
        .instance()
        .get::<DataKey, Address>(&DataKey::Admin)
        .unwrap()
}

fn read_min_stake(env: &Env) -> i128 {
    env.storage()
        .instance()
        .get::<DataKey, i128>(&DataKey::MinStake)
        .unwrap_or(1_000_000_000)
}

fn read_reward_rate(env: &Env) -> u32 {
    env.storage()
        .instance()
        .get::<DataKey, u32>(&DataKey::RewardRateBps)
        .unwrap_or(50)
}

#[contractimpl]
impl NodeStaking {
    pub fn initialize(env: Env, admin: Address, min_stake: i128, reward_rate_bps: u32) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("already initialized");
        }
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::TotalStaked, &0i128);
        env.storage().instance().set(&DataKey::TotalNodes, &0u32);
        env.storage().instance().set(&DataKey::MinStake, &min_stake);
        env.storage().instance().set(&DataKey::RewardRateBps, &reward_rate_bps);
        env.storage().instance().set(&DataKey::SlashedAmount, &0i128);
    }

    pub fn register_node(
        env: Env,
        operator: Address,
        stake_amount: i128,
    ) -> Result<(), StakingError> {
        operator.require_auth();

        if env.storage().instance().has(&DataKey::Node(operator.clone())) {
            return Err(StakingError::NodeAlreadyRegistered);
        }

        if stake_amount < read_min_stake(&env) {
            return Err(StakingError::BelowMinStake);
        }

        let node = NodeInfo {
            operator: operator.clone(),
            staked: stake_amount,
            rewards_accrued: 0,
            active: true,
            joined_at: env.ledger().timestamp(),
        };

        env.storage().instance().set(&DataKey::Node(operator.clone()), &node);

        let total_staked: i128 = env
            .storage()
            .instance()
            .get(&DataKey::TotalStaked)
            .unwrap_or(0);
        env.storage()
            .instance()
            .set(&DataKey::TotalStaked, &(total_staked + stake_amount));

        let total_nodes: u32 = env
            .storage()
            .instance()
            .get(&DataKey::TotalNodes)
            .unwrap_or(0);
        env.storage()
            .instance()
            .set(&DataKey::TotalNodes, &(total_nodes + 1));

        env.events()
            .publish((symbol_short!("nd_reg"), &operator), &stake_amount);

        Ok(())
    }

    pub fn add_stake(
        env: Env,
        operator: Address,
        amount: i128,
    ) -> Result<(), StakingError> {
        operator.require_auth();

        let mut node: NodeInfo = env
            .storage()
            .instance()
            .get(&DataKey::Node(operator.clone()))
            .ok_or(StakingError::NodeNotFound)?;

        if !node.active {
            return Err(StakingError::NodeNotActive);
        }

        node.staked += amount;
        env.storage().instance().set(&DataKey::Node(operator), &node);

        let total_staked: i128 = env
            .storage()
            .instance()
            .get(&DataKey::TotalStaked)
            .unwrap_or(0);
        env.storage()
            .instance()
            .set(&DataKey::TotalStaked, &(total_staked + amount));

        Ok(())
    }

    pub fn unstake(
        env: Env,
        operator: Address,
        amount: i128,
    ) -> Result<i128, StakingError> {
        operator.require_auth();

        let mut node: NodeInfo = env
            .storage()
            .instance()
            .get(&DataKey::Node(operator.clone()))
            .ok_or(StakingError::NodeNotFound)?;

        if amount <= 0 {
            return Err(StakingError::NothingToUnstake);
        }

        if node.staked < amount {
            return Err(StakingError::InsufficientBalance);
        }

        node.staked -= amount;
        node.active = node.staked >= read_min_stake(&env);
        let payout = amount + node.rewards_accrued;
        node.rewards_accrued = 0;
        env.storage().instance().set(&DataKey::Node(operator.clone()), &node);

        let total_staked: i128 = env
            .storage()
            .instance()
            .get(&DataKey::TotalStaked)
            .unwrap_or(0);
        env.storage()
            .instance()
            .set(&DataKey::TotalStaked, &(total_staked - amount));

        env.events()
            .publish((symbol_short!("nd_unstke"), &operator), &payout);

        Ok(payout)
    }

    pub fn claim_rewards(
        env: Env,
        operator: Address,
    ) -> Result<i128, StakingError> {
        operator.require_auth();

        let mut node: NodeInfo = env
            .storage()
            .instance()
            .get(&DataKey::Node(operator.clone()))
            .ok_or(StakingError::NodeNotFound)?;

        let rewards = node.rewards_accrued;
        node.rewards_accrued = 0;
        env.storage().instance().set(&DataKey::Node(operator.clone()), &node);

        env.events()
            .publish((symbol_short!("nd_claim"), &operator), &rewards);

        Ok(rewards)
    }

    pub fn slash(
        env: Env,
        admin: Address,
        operator: Address,
        amount: i128,
    ) -> Result<(), StakingError> {
        if admin != read_admin(&env) {
            return Err(StakingError::NotAdmin);
        }
        admin.require_auth();

        let mut node: NodeInfo = env
            .storage()
            .instance()
            .get(&DataKey::Node(operator.clone()))
            .ok_or(StakingError::NodeNotFound)?;

        let slashed = core::cmp::min(amount, node.staked);
        node.staked -= slashed;
        node.active = node.staked >= read_min_stake(&env);
        env.storage().instance().set(&DataKey::Node(operator.clone()), &node);

        let total_slashed: i128 = env
            .storage()
            .instance()
            .get(&DataKey::SlashedAmount)
            .unwrap_or(0);
        env.storage()
            .instance()
            .set(&DataKey::SlashedAmount, &(total_slashed + slashed));

        let total_staked: i128 = env
            .storage()
            .instance()
            .get(&DataKey::TotalStaked)
            .unwrap_or(0);
        env.storage()
            .instance()
            .set(&DataKey::TotalStaked, &(total_staked - slashed));

        env.events()
            .publish((symbol_short!("nd_slash"), &operator), &slashed);

        Ok(())
    }

    pub fn get_node(env: Env, operator: Address) -> Result<NodeInfo, StakingError> {
        env.storage()
            .instance()
            .get::<DataKey, NodeInfo>(&DataKey::Node(operator))
            .ok_or(StakingError::NodeNotFound)
    }

    pub fn total_staked(env: Env) -> i128 {
        env.storage()
            .instance()
            .get(&DataKey::TotalStaked)
            .unwrap_or(0)
    }

    pub fn total_nodes(env: Env) -> u32 {
        env.storage()
            .instance()
            .get(&DataKey::TotalNodes)
            .unwrap_or(0)
    }

    pub fn min_stake(env: Env) -> i128 {
        read_min_stake(&env)
    }

    pub fn admin(env: Env) -> Address {
        read_admin(&env)
    }
}

#[cfg(test)]
mod test {
    extern crate std;
    use super::{NodeStaking, NodeStakingClient, StakingError};
    use soroban_sdk::{testutils::Address as _, Address, Env};

    fn setup() -> (Env, NodeStakingClient<'static>, Address) {
        let env = Env::default();
        env.mock_all_auths();
        let id = env.register(NodeStaking, ());
        let client = NodeStakingClient::new(&env, &id);
        let admin = Address::generate(&env);
        client.initialize(&admin, &1_000_000_000, &50);
        (env, client, admin)
    }

    #[test]
    fn test_register_node() {
        let (env, client, _admin) = setup();
        let operator = Address::generate(&env);

        client.register_node(&operator, &2_000_000_000);

        let node = client.get_node(&operator);
        assert_eq!(node.staked, 2_000_000_000);
        assert!(node.active);
        assert_eq!(client.total_nodes(), 1);
    }

    #[test]
    fn test_below_min_stake_fails() {
        let (env, client, _admin) = setup();
        let operator = Address::generate(&env);

        let result = client.try_register_node(&operator, &500_000_000);
        assert_eq!(result, Err(Ok(StakingError::BelowMinStake)));
    }

    #[test]
    fn test_add_stake() {
        let (env, client, _admin) = setup();
        let operator = Address::generate(&env);

        client.register_node(&operator, &2_000_000_000);
        client.add_stake(&operator, &1_000_000_000);

        let node = client.get_node(&operator);
        assert_eq!(node.staked, 3_000_000_000);
        assert_eq!(client.total_staked(), 3_000_000_000);
    }

    #[test]
    fn test_unstake() {
        let (env, client, _admin) = setup();
        let operator = Address::generate(&env);

        client.register_node(&operator, &3_000_000_000);
        let payout = client.unstake(&operator, &1_000_000_000);

        assert_eq!(payout, 1_000_000_000);
        let node = client.get_node(&operator);
        assert_eq!(node.staked, 2_000_000_000);
        assert!(node.active);
    }

    #[test]
    fn test_unstake_deactivates_below_min() {
        let (env, client, _admin) = setup();
        let operator = Address::generate(&env);

        client.register_node(&operator, &2_000_000_000);
        client.unstake(&operator, &1_500_000_000);

        let node = client.get_node(&operator);
        assert!(!node.active);
    }

    #[test]
    fn test_slash() {
        let (env, client, admin) = setup();
        let operator = Address::generate(&env);

        client.register_node(&operator, &3_000_000_000);
        client.slash(&admin, &operator, &1_000_000_000);

        let node = client.get_node(&operator);
        assert_eq!(node.staked, 2_000_000_000);
    }

    #[test]
    fn test_non_admin_cannot_slash() {
        let (env, client, _admin) = setup();
        let operator = Address::generate(&env);
        let rando = Address::generate(&env);

        client.register_node(&operator, &3_000_000_000);
        let result = client.try_slash(&rando, &operator, &1_000_000_000);
        assert_eq!(result, Err(Ok(StakingError::NotAdmin)));
    }
}
