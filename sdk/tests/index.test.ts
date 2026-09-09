import { describe, it, expect } from 'vitest';
import {
  SubanClient,
  PoolFactoryClient,
  CpmmPoolClient,
  StableswapClient,
  LiquidityMiningClient,
  LendingClient,
  EscrowClient,
  OracleClient,
  BridgeClient,
} from '../src/index.js';

describe('Suban SDK', () => {
  it('exports SubanClient', () => {
    expect(SubanClient).toBeDefined();
    expect(typeof SubanClient).toBe('function');
  });

  it('creates SubanClient instance', () => {
    const client = new SubanClient({
      rpcUrl: 'https://rpc.testnet.minepi.com',
      networkPassphrase: 'Pi Testnet',
    });
    expect(client).toBeDefined();
    expect(client.server).toBeDefined();
  });

  it('exports all AMM clients', () => {
    expect(PoolFactoryClient).toBeDefined();
    expect(CpmmPoolClient).toBeDefined();
    expect(StableswapClient).toBeDefined();
    expect(LiquidityMiningClient).toBeDefined();
  });

  it('exports LendingClient', () => {
    expect(LendingClient).toBeDefined();
  });

  it('exports EscrowClient', () => {
    expect(EscrowClient).toBeDefined();
  });

  it('exports OracleClient', () => {
    expect(OracleClient).toBeDefined();
  });

  it('exports BridgeClient', () => {
    expect(BridgeClient).toBeDefined();
  });
});
