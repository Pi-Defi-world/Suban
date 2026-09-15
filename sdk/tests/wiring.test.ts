import { describe, it, expect } from 'vitest';
import type { Keypair } from '@stellar/stellar-sdk';
import {
  PoolFactoryClient,
  CpmmPoolClient,
  StableswapClient,
  LendingClient,
  EscrowClient,
  BridgeClient,
} from '../src/index.js';
import type { SubanClient } from '../src/index.js';

// Lightweight stand-in for SubanClient that records every invoke/simulate call
// so we can assert the SDK builds the correct contract method name + argument
// order/count (covers remediation plan 3.2 / 3.3 / 3.4 without a live network).
class RecordingClient {
  calls: { method: string; args: unknown[] }[] = [];
  async invoke(_cid: string, method: string, args: unknown[]): Promise<{ txHash: string; result: unknown }> {
    this.calls.push({ method, args });
    return { txHash: 'hash', result: {} };
  }
  async simulate(_cid: string, method: string, args: unknown[] = []): Promise<unknown> {
    this.calls.push({ method, args });
    // get_* methods return simple native shapes the SDK maps onto interfaces.
    if (method === 'get_reserves') return ['0', '0'];
    if (method === 'get_total_shares') return '0';
    if (method === 'get_fee_bps') return 30;
    if (method === 'get_amplification') return '1';
    return null;
  }
}

const kp = { publicKey: () => 'GTEST' } as unknown as Keypair;
const CID = 'CABC';

function recording(): RecordingClient {
  return new RecordingClient() as unknown as SubanClient;
}

describe('SDK contract wiring', () => {
  it('pool-factory create_pool is caller-first, 8 args', async () => {
    const c = new PoolFactoryClient(recording(), CID);
    await c.createPool(new Uint8Array(32), new Uint8Array(32), 'TA', 'TB', 0, 30, 'n', kp);
    const last = (c as unknown as { client: RecordingClient }).client.calls.at(-1)!;
    expect(last.method).toBe('create_pool');
    expect(last.args).toHaveLength(8);
    expect(last.args[0]).toBe('GTEST');
  });

  it('cpmm getState reads reserves/shares/fee (not get_pool_state)', async () => {
    const c = new CpmmPoolClient(recording(), CID);
    const state = await c.getState();
    const methods = (c as unknown as { client: RecordingClient }).client.calls.map((x) => x.method);
    expect(methods).toContain('get_reserves');
    expect(methods).toContain('get_total_shares');
    expect(methods).toContain('get_fee_bps');
    expect(methods).not.toContain('get_pool_state');
    expect(state.feeBps).toBe(30);
  });

  it('stableswap getState includes amplification', async () => {
    const c = new StableswapClient(recording(), CID);
    const state = await c.getState();
    const methods = (c as unknown as { client: RecordingClient }).client.calls.map((x) => x.method);
    expect(methods).toContain('get_amplification');
    expect(state.amplification).toBe('1');
  });

  it('escrow fund/release/refund take only escrow id', async () => {
    const c = new EscrowClient(recording(), CID);
    await c.fundEscrow(7, '100', kp);
    await c.releaseFunds(7, kp);
    await c.refund(7, kp);
    const calls = (c as unknown as { client: RecordingClient }).client.calls;
    expect(calls.at(-3)).toEqual({ method: 'fund_escrow', args: [7, '100'] });
    expect(calls.at(-2)).toEqual({ method: 'release_funds', args: [7] });
    expect(calls.at(-1)).toEqual({ method: 'refund', args: [7] });
  });

  it('escrow dispute/resolve map to contract arg order', async () => {
    const c = new EscrowClient(recording(), CID);
    await c.dispute(7, kp);
    await c.resolve(7, true, kp);
    await c.resolve(7, false, kp);
    const calls = (c as unknown as { client: RecordingClient }).client.calls;
    expect(calls.at(-3)).toEqual({ method: 'dispute', args: [7, 'GTEST'] });
    expect(calls.at(-2)).toEqual({ method: 'resolve', args: [7, 'release'] });
    expect(calls.at(-1)).toEqual({ method: 'resolve', args: [7, 'refund'] });
  });

  it('lending uses deposit/withdraw and liquidate has min_collateral', async () => {
    const c = new LendingClient(recording(), CID);
    await c.depositCollateral('100', kp);
    await c.withdrawCollateral('50', kp);
    await c.liquidate('GBORROW', '10', '9', kp);
    const calls = (c as unknown as { client: RecordingClient }).client.calls;
    expect(calls.at(-3)).toEqual({ method: 'deposit', args: ['GTEST', '100'] });
    expect(calls.at(-2)).toEqual({ method: 'withdraw', args: ['GTEST', '50'] });
    expect(calls.at(-1)).toEqual({ method: 'liquidate', args: ['GTEST', 'GBORROW', '10', '9'] });
  });

  it('bridge propose maps to propose(kind,target,amount,deposit_id)', async () => {
    const c = new BridgeClient(recording(), CID);
    await c.proposeMint('GTO', '100', new Uint8Array(32), kp);
    await c.approveMint(3, kp);
    const calls = (c as unknown as { client: RecordingClient }).client.calls;
    expect(calls.at(-2)).toEqual({
      method: 'propose',
      args: ['GTEST', 0, 'GTO', '100', new Uint8Array(32)],
    });
    expect(calls.at(-1)).toEqual({ method: 'approve', args: ['GTEST', 3] });
  });
});
