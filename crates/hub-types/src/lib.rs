#![no_std]

use soroban_sdk::{contracttype, Address, BytesN, Vec};

// ─── Price ───────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Price {
    pub asset: Address,
    pub price: i128,
    pub decimals: u32,
    pub timestamp: u64,
    pub confidence: u32,
}

// ─── AMM / Liquidity ─────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PoolConfig {
    pub token_a: Address,
    pub token_b: Address,
    pub fee_bps: u32,
    pub pool_type: PoolType,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PoolType {
    ConstantProduct = 0,
    Stableswap = 1,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Pool {
    pub pool_id: Address,
    pub token_a: Address,
    pub token_b: Address,
    pub reserve_a: i128,
    pub reserve_b: i128,
    pub total_shares: i128,
    pub fee_bps: u32,
    pub pool_type: PoolType,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SwapResult {
    pub amount_out: i128,
    pub fee: i128,
    pub pool_id: Address,
}

// ─── Lending ─────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LendingPoolConfig {
    pub lend_token: Address,
    pub collateral_token: Address,
    pub collateral_factor: u32,
    pub supply_rate: u32,
    pub borrow_rate: u32,
    pub backstop_take_rate: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Position {
    pub owner: Address,
    pub collateral: i128,
    pub debt: i128,
    pub health_factor: i128,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SupplyPosition {
    pub owner: Address,
    pub amount: i128,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BorrowPosition {
    pub owner: Address,
    pub collateral_amount: i128,
    pub borrow_amount: i128,
    pub health_factor: i128,
}

// ─── Escrow ──────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EscrowConfig {
    pub funder: Address,
    pub receiver: Address,
    pub arbitrator: Address,
    pub asset: Address,
    pub milestones: Vec<Milestone>,
    pub fee_bps: u32,
    pub deadline_ledger: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Milestone {
    pub description: BytesN<32>,
    pub amount: i128,
    pub approver: Address,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MilestoneStatus {
    Pending = 0,
    Submitted = 1,
    Approved = 2,
    Rejected = 3,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EscrowState {
    pub escrow_id: u32,
    pub config: EscrowConfig,
    pub total_deposited: i128,
    pub total_released: i128,
    pub milestones_completed: u32,
    pub milestone_statuses: Vec<MilestoneStatus>,
    pub status: EscrowStatus,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EscrowStatus {
    Active = 0,
    Completed = 1,
    Refunded = 2,
    Disputed = 3,
}

// ─── Fee / Treasury ──────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FeeConfig {
    pub lp_share: u32,
    pub protocol_share: u32,
    pub backstop_share: u32,
    pub platform_share: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FeeRoute {
    pub destination: Address,
    pub share_bps: u32,
}

// ─── Pause ───────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Primitive {
    Amm = 0,
    Lending = 1,
    Escrow = 2,
    Bridge = 3,
    All = 4,
}
