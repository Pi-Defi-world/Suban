import { describe, expect, it } from 'vitest';
import { nativeToScVal } from '@stellar/stellar-sdk';
import { decodeScVal } from '../src/scval.js';

describe('decodeScVal', () => {
  it('decodes base64 XDR values', () => {
    const encoded = nativeToScVal('swap', { type: 'symbol' }).toXDR('base64');
    expect(decodeScVal(encoded)).toBe('swap');
  });

  it('decodes xdr.ScVal instances returned by the RPC SDK', () => {
    expect(decodeScVal(nativeToScVal('swap', { type: 'symbol' }))).toBe('swap');
    expect(decodeScVal(nativeToScVal(10n))).toBe('10');
  });

  it('decodes already-decoded symbol and address objects', () => {
    expect(decodeScVal({ symbol: 'add_liq' })).toBe('add_liq');
    expect(decodeScVal({ address: 'GABC' })).toBe('GABC');
  });

  it('decodes nested vectors from the Pi RPC shape', () => {
    expect(
      decodeScVal({
        vec: [
          { i128: '10000000000' },
          { address: 'CABC' },
          { u32: 7 },
        ],
      })
    ).toEqual(['10000000000', 'CABC', 7]);
  });

  it('preserves unknown scalar values', () => {
    expect(decodeScVal(42)).toBe(42);
    expect(decodeScVal(null)).toBe(null);
  });
});
