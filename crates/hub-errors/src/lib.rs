#![no_std]

use soroban_sdk::contracterror;

/// Shared errors across all Hub primitives.
/// Extends the zyra_common ZyraError with Hub-specific variants.
#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HubError {
    // ─── General ───────────────────────────────────────────────
    Unauthorized = 1,
    InvalidArgument = 2,
    NotFound = 3,
    AlreadyInitialized = 4,
    AlreadyExists = 5,
    Paused = 6,
    ContractError = 7,

    // ─── AMM ───────────────────────────────────────────────────
    InsufficientLiquidity = 10,
    SlippageExceeded = 11,
    InvalidPool = 12,
    ZeroAmount = 13,
    SameToken = 14,
    InvalidPath = 15,

    // ─── Lending ───────────────────────────────────────────────
    InsufficientCollateral = 20,
    BorrowLimitExceeded = 21,
    PositionHealthTooLow = 22,
    LiquidationFailed = 23,
    BackstopDepleted = 24,
    InvalidCollateralFactor = 25,

    // ─── Escrow ────────────────────────────────────────────────
    EscrowNotFound = 30,
    EscrowAlreadyCompleted = 31,
    MilestoneNotApprovable = 32,
    DeadlineNotReached = 33,
    DeadlineAlreadyPassed = 34,
    InsufficientEscrowBalance = 35,
    MilestoneAlreadySubmitted = 36,
    MilestoneAlreadyApproved = 37,
    EscrowNotFunded = 38,
    InvalidEscrowStatus = 39,

    // ─── Oracle ────────────────────────────────────────────────
    OracleStale = 40,
    OracleDeviationTooHigh = 41,
    OracleUnavailable = 42,
    CircuitBreakerTriggered = 43,

    // ─── Fee / Treasury ────────────────────────────────────────
    FeeBpsExceedsMaximum = 50,
    FeeRouteInvalid = 51,
    TreasuryWithdrawalFailed = 52,

    // ─── Pause ─────────────────────────────────────────────────
    PrimitiveIsPaused = 60,
    CannotPauseRegistry = 61,
}
