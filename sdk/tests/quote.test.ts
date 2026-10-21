import { describe, expect, it } from 'vitest';
import type { SubanClient } from '../src/index.js';
import { CpmmPoolClient, quoteCpmm } from '../src/index.js';

describe('CPMM quote preview', () => {
  it('matches the contract integer formula for a balanced pool', () => {
    const quote = quoteCpmm({
      reserveIn: '1000000',
      reserveOut: '1000000',
      amountIn: '1000',
      feeBps: 30,
    });

    expect(quote.amountInAfterFee).toBe('997');
    expect(quote.feeAmount).toBe('3');
    expect(quote.amountOut).toBe('996');
    expect(quote.spotPriceOutPerIn).toBe(1);
    expect(quote.executionPriceOutPerIn).toBeCloseTo(0.996, 12);
    expect(quote.priceImpactBps).toBe(40);
  });

  it('uses the input-side reserve for price impact in an asymmetric pool', () => {
    const quote = quoteCpmm({
      reserveIn: '2000000',
      reserveOut: '1000000',
      amountIn: '1000',
      feeBps: 30,
    });

    expect(quote.amountOut).toBe('498');
    expect(quote.spotPriceOutPerIn).toBe(0.5);
    expect(quote.executionPriceOutPerIn).toBeCloseTo(0.498, 12);
    expect(quote.priceImpactBps).toBe(40);
  });

  it('returns zero output for a dust trade instead of rounding up', () => {
    const quote = quoteCpmm({
      reserveIn: '1000000',
      reserveOut: '1000000',
      amountIn: '1',
      feeBps: 30,
    });

    expect(quote.amountInAfterFee).toBe('0');
    expect(quote.amountOut).toBe('0');
    expect(quote.priceImpactBps).toBe(10_000);
  });

  it('charges no fee when feeBps is zero', () => {
    const quote = quoteCpmm({
      reserveIn: '1000',
      reserveOut: '2000',
      amountIn: '100',
      feeBps: 0,
    });

    expect(quote.feeAmount).toBe('0');
    expect(quote.amountOut).toBe('181');
    expect(quote.priceImpactBps).toBe(950);
  });

  it('rejects invalid reserves, amounts, fees and decimal units', () => {
    expect(() => quoteCpmm({ reserveIn: '0', reserveOut: '1', amountIn: '1', feeBps: 0 })).toThrow(
      /reserves/
    );
    expect(() => quoteCpmm({ reserveIn: '1', reserveOut: '1', amountIn: '0', feeBps: 0 })).toThrow(
      /amountIn/
    );
    expect(() => quoteCpmm({ reserveIn: '1', reserveOut: '1', amountIn: '1', feeBps: 1001 })).toThrow(
      /feeBps/
    );
    expect(() => quoteCpmm({ reserveIn: '1', reserveOut: '1', amountIn: '1.5', feeBps: 0 })).toThrow(
      /integer/
    );
  });

  it('delegates to the on-chain get_quote view without local arithmetic drift', async () => {
    const calls: { method: string; args: unknown[] }[] = [];
    const client = {
      async simulate(_contractId: string, method: string, args: unknown[] = []) {
        calls.push({ method, args });
        return '996';
      },
    } as unknown as SubanClient;
    const pool = new CpmmPoolClient(client, 'CABC');

    await expect(pool.getQuote('TOKEN', '1000')).resolves.toBe('996');
    expect(calls).toEqual([{ method: 'get_quote', args: ['TOKEN', '1000'] }]);
  });
});
