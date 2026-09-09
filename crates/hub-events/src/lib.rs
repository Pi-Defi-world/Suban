#![no_std]

use soroban_sdk::{symbol_short, Address, Symbol};

/// Standardized event topics for indexer compatibility.
pub mod topics {
    use soroban_sdk::{symbol_short, Symbol};

    pub const AMM: Symbol = symbol_short!("amm");
    pub const SWAP: Symbol = symbol_short!("swap");
    pub const LEND: Symbol = symbol_short!("lend");
    pub const ESCROW: Symbol = symbol_short!("escrow");
    pub const FEE: Symbol = symbol_short!("fee");
    pub const PAUSE: Symbol = symbol_short!("pause");
    pub const ORACLE: Symbol = symbol_short!("oracle");
    pub const BRIDGE: Symbol = symbol_short!("bridge");
}

// ─── AMM Events ──────────────────────────────────────────────────────

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PoolCreated {
    pub pool_id: Address,
    pub token_a: Address,
    pub token_b: Address,
    pub fee_bps: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiquidityAdded {
    pub pool_id: Address,
    pub provider: Address,
    pub amount_a: i128,
    pub amount_b: i128,
    pub shares: i128,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiquidityRemoved {
    pub pool_id: Address,
    pub provider: Address,
    pub shares: i128,
    pub amount_a: i128,
    pub amount_b: i128,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SwapExecuted {
    pub pool_id: Address,
    pub trader: Address,
    pub token_in: Address,
    pub amount_in: i128,
    pub amount_out: i128,
    pub fee: i128,
}

// ─── Lending Events ──────────────────────────────────────────────────

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SupplyMade {
    pub pool_id: Address,
    pub supplier: Address,
    pub amount: i128,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BorrowMade {
    pub pool_id: Address,
    pub borrower: Address,
    pub collateral: i128,
    pub borrowed: i128,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RepayMade {
    pub pool_id: Address,
    pub borrower: Address,
    pub amount: i128,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiquidationExecuted {
    pub pool_id: Address,
    pub liquidator: Address,
    pub victim: Address,
    pub repay_amount: i128,
    pub seized_amount: i128,
}

// ─── Escrow Events ───────────────────────────────────────────────────

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EscrowCreated {
    pub escrow_id: Address,
    pub funder: Address,
    pub receiver: Address,
    pub total_amount: i128,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EscrowFunded {
    pub escrow_id: Address,
    pub funder: Address,
    pub amount: i128,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MilestoneSubmitted {
    pub escrow_id: Address,
    pub milestone_idx: u32,
    pub submitter: Address,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MilestoneApproved {
    pub escrow_id: Address,
    pub milestone_idx: u32,
    pub approver: Address,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MilestoneRejected {
    pub escrow_id: Address,
    pub milestone_idx: u32,
    pub approver: Address,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EscrowReleased {
    pub escrow_id: Address,
    pub amount: i128,
    pub to: Address,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EscrowRefunded {
    pub escrow_id: Address,
    pub amount: i128,
    pub to: Address,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EscrowDisputed {
    pub escrow_id: Address,
    pub disputer: Address,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EscrowResolved {
    pub escrow_id: Address,
    pub arbitrator: Address,
    pub outcome: Symbol,
}

// ─── Fee Events ──────────────────────────────────────────────────────

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FeeRouted {
    pub source: Address,
    pub destination: Address,
    pub amount: i128,
    pub fee_type: Symbol,
}

// ─── Pause Events ────────────────────────────────────────────────────

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrimitivePaused {
    pub primitive: Symbol,
    pub admin: Address,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrimitiveUnpaused {
    pub primitive: Symbol,
    pub admin: Address,
}
