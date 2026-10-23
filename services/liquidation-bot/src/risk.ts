/**
 * Pure liquidation decision logic. No network, no keys, no side effects.
 *
 * Health factors are basis points from the lending contract (10_000 = 1.0).
 * The contract's liquidation bonus is applied to the repaid debt; the bot
 * subtracts a configurable slippage buffer from the minimum collateral it will
 * accept. The contract enforces the final health check.
 */

export interface LiquidationPlanInput {
  account: string;
  healthBps: bigint;
  borrowRaw: bigint;
  collateralRaw: bigint;
  maxRepayRaw: bigint;
  minHealthBps?: bigint;
  bonusBps?: bigint;
  slippageBps?: bigint;
}

export interface LiquidationPlan {
  account: string;
  liquidatable: boolean;
  reason: string;
  healthBps: bigint;
  repayRaw: bigint;
  estimatedCollateralRaw: bigint;
  minCollateralRaw: bigint;
}

export function isLiquidatable(
  healthBps: bigint,
  minHealthBps: bigint = 10_000n
): boolean {
  return healthBps < minHealthBps;
}

export function planLiquidation(input: LiquidationPlanInput): LiquidationPlan {
  const minHealthBps = input.minHealthBps ?? 10_000n;
  const bonusBps = input.bonusBps ?? 500n;
  const slippageBps = input.slippageBps ?? 100n;

  const base = {
    account: input.account,
    healthBps: input.healthBps,
  };

  if (!isLiquidatable(input.healthBps, minHealthBps)) {
    return {
      ...base,
      liquidatable: false,
      reason: 'healthy',
      repayRaw: 0n,
      estimatedCollateralRaw: 0n,
      minCollateralRaw: 0n,
    };
  }
  if (input.borrowRaw <= 0n) {
    return {
      ...base,
      liquidatable: false,
      reason: 'no-debt',
      repayRaw: 0n,
      estimatedCollateralRaw: 0n,
      minCollateralRaw: 0n,
    };
  }
  if (input.maxRepayRaw <= 0n) {
    return {
      ...base,
      liquidatable: false,
      reason: 'repay-cap-zero',
      repayRaw: 0n,
      estimatedCollateralRaw: 0n,
      minCollateralRaw: 0n,
    };
  }

  // The contract refuses a liquidation whose bonus-inclusive seizure exceeds
  // the borrower's collateral. Bound repayment by the collateral first; do not
  // merely cap the estimate after computing an unaffordable repayment.
  const maxRepayFromCollateral =
    input.collateralRaw > 0n
      ? (input.collateralRaw * 10_000n) / (10_000n + bonusBps)
      : 0n;
  const repayCap =
    input.maxRepayRaw < maxRepayFromCollateral
      ? input.maxRepayRaw
      : maxRepayFromCollateral;
  if (repayCap <= 0n) {
    return {
      ...base,
      liquidatable: false,
      reason: 'collateral-cap-zero',
      repayRaw: 0n,
      estimatedCollateralRaw: 0n,
      minCollateralRaw: 0n,
    };
  }

  const repayRaw = input.borrowRaw < repayCap ? input.borrowRaw : repayCap;
  const estimatedCollateralRaw = (repayRaw * (10_000n + bonusBps)) / 10_000n;

  const bufferBps = slippageBps > 10_000n ? 10_000n : slippageBps;
  const minCollateralRaw =
    (estimatedCollateralRaw * (10_000n - bufferBps)) / 10_000n;

  return {
    ...base,
    liquidatable: true,
    reason: 'undercollateralised',
    repayRaw,
    estimatedCollateralRaw,
    minCollateralRaw,
  };
}
