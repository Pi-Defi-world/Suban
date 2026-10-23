import { describe, expect, it } from 'vitest';
import { loadConfig } from '../src/config.js';

const POOL = `C${'A'.repeat(55)}`;
const ACCOUNT = `G${'A'.repeat(55)}`;

function base(overrides: NodeJS.ProcessEnv = {}): NodeJS.ProcessEnv {
  return {
    LENDING_POOL_CONTRACT_ID: POOL,
    LIQUIDATOR_PUBLIC_KEY: ACCOUNT,
    LIQUIDATION_ACCOUNTS: ACCOUNT,
    ...overrides,
  };
}

describe('liquidation bot config', () => {
  it('defaults to dry-run', () => {
    expect(loadConfig(base()).mode).toBe('dry-run');
  });

  it('requires a secret for execute mode', () => {
    expect(() => loadConfig(base({ LIQUIDATION_MODE: 'execute' }))).toThrow(
      'LIQUIDATOR_SECRET is required in execute mode'
    );
  });

  it('requires an account source', () => {
    expect(() =>
      loadConfig(base({ LIQUIDATOR_PUBLIC_KEY: '' }))
    ).toThrow('LIQUIDATOR_PUBLIC_KEY');
  });

  it('requires discovery configuration', () => {
    expect(() =>
      loadConfig(base({ LIQUIDATION_ACCOUNTS: '' }))
    ).toThrow('LIQUIDATION_ACCOUNTS or DATA_PLATFORM_URL');
  });

  it('rejects an invalid contract id', () => {
    expect(() =>
      loadConfig(base({ LENDING_POOL_CONTRACT_ID: 'not-a-contract' }))
    ).toThrow('LENDING_POOL_CONTRACT_ID');
  });

  it('parses numeric limits', () => {
    const config = loadConfig(
      base({
        LIQUIDATION_MAX_REPAY_RAW: '42',
        LIQUIDATION_POLL_MS: '1000',
        STELLAR_TX_FEE: '3000000',
      })
    );
    expect(config.maxRepayRaw).toBe(42n);
    expect(config.pollMs).toBe(1000);
    expect(config.txFee).toBe('3000000');
  });
});
