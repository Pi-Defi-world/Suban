import { describe, expect, it } from 'vitest';
import { accountsFromEvents, discoverAccounts } from '../src/discovery.js';

const A = 'GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF';
const B = 'GBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB';

describe('candidate discovery', () => {
  it('keeps accounts with positive debt, de-duplicates and ignores invalid entries', () => {
    const accounts = accountsFromEvents(
      [
        { accounts: [A, B], position: { borrow: '10' } },
        { accounts: [A], position: { borrow: '20' } },
        { accounts: [B], position: { borrow: '0' } },
        { accounts: ['not-an-account'], position: { borrow: '5' } },
      ],
      10
    );
    expect(accounts).toEqual([A, B]);
  });

  it('caps the candidate list', () => {
    const accounts = accountsFromEvents(
      [{ accounts: [A, B], position: { borrow: '1' } }],
      1
    );
    expect(accounts).toEqual([A]);
  });

  it('queries the data platform and returns candidates', async () => {
    const fetchImpl = (async () =>
      new Response(
        JSON.stringify({
          data: [{ accounts: [A], position: { borrow: '100' } }],
        }),
        { status: 200, headers: { 'content-type': 'application/json' } }
      )) as typeof fetch;

    await expect(discoverAccounts('http://data.local/', 10, fetchImpl)).resolves.toEqual([
      A,
    ]);
  });

  it('fails closed when the data platform is unavailable', async () => {
    const fetchImpl = (async () =>
      new Response('nope', { status: 503 })) as typeof fetch;
    await expect(discoverAccounts('http://data.local', 10, fetchImpl)).rejects.toThrow(
      'Data platform returned 503'
    );
  });
});
