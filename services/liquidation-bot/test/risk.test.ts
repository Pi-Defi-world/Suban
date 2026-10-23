import { describe, expect, it } from 'vitest';
import { isLiquidatable, planLiquidation } from '../src/risk.js';

const account = 'GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF';

function plan(overrides: Partial<Parameters<typeof planLiquidation>[0]> = {}) {
  return planLiquidation({
    account,
    healthBps: 9_000n,
    borrowRaw: 1_000n,
    collateralRaw: 2_000n,
    maxRepayRaw: 500n,
    ...overrides,
  });
}

describe('liquidation risk', () => {
  it('treats exactly 10,000 bps as healthy', () => {
    expect(isLiquidatable(10_000n)).toBe(false);
    expect(plan({ healthBps: 10_000n }).liquidatable).toBe(false);
  });

  it('treats 9,999 bps as liquidatable', () => {
    expect(isLiquidatable(9_999n)).toBe(true);
  });

  it('caps repayment and applies the bonus', () => {
    const result = plan();
    expect(result.liquidatable).toBe(true);
    expect(result.repayRaw).toBe(500n);
    expect(result.estimatedCollateralRaw).toBe(525n); // 500 * 1.05
    expect(result.minCollateralRaw).toBe(519n); // 1% slippage buffer
  });

  it('repays the whole debt when below the cap', () => {
    const result = plan({ borrowRaw: 300n, maxRepayRaw: 500n });
    expect(result.repayRaw).toBe(300n);
    expect(result.estimatedCollateralRaw).toBe(315n);
  });

  it('bounds repayment so the bonus-inclusive seizure fits collateral', () => {
    const result = plan({ collateralRaw: 100n });
    // 100 * 10,000 / 10,500 = 95 (truncated), then 95 * 1.05 = 99.
    expect(result.repayRaw).toBe(95n);
    expect(result.estimatedCollateralRaw).toBe(99n);
    expect(result.minCollateralRaw).toBe(98n);
  });

  it('skips positions with no debt', () => {
    const result = plan({ borrowRaw: 0n });
    expect(result.liquidatable).toBe(false);
    expect(result.reason).toBe('no-debt');
  });

  it('skips when the repay cap is zero', () => {
    const result = plan({ maxRepayRaw: 0n });
    expect(result.liquidatable).toBe(false);
    expect(result.reason).toBe('repay-cap-zero');
  });

  it('honours a custom health threshold', () => {
    expect(plan({ healthBps: 10_500n, minHealthBps: 11_000n }).liquidatable).toBe(
      true
    );
    expect(plan({ healthBps: 10_500n, minHealthBps: 10_000n }).liquidatable).toBe(
      false
    );
  });
});
