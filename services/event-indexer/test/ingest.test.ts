import { describe, expect, it } from 'vitest';
import { SubanIngestor } from '../src/ingest.js';
import { setCheckpoint } from '../src/meta.js';
import { memoryDb, fakeHorizon, fakeRpc, testConfig } from './helpers.js';

const ACCOUNT = 'GB6PCM5GBUZR3EYET4IWQ4TBI7IYPFXFJI4TX3BHDPC6H3PSGB5UYB7W';

describe('ingestion', () => {
  it('stores contract events idempotently from a checkpoint', async () => {
    const db = memoryDb();
    const config = testConfig({
      contracts: [
        {
          name: 'pool',
          contractId: 'C1',
          interfaceVersion: '1.0.0',
          verified: false,
        },
      ],
    });
    const server = fakeRpc([
      {
        id: 'evt-1',
        ledger: 101,
        txHash: 'tx1',
        ledgerClosedAt: '2026-10-21T00:00:00Z',
        topic: ['swap'],
        value: { amount: 10 },
      },
    ]);
    const ingestor = new SubanIngestor(db, config, server);

    await ingestor.pollOnce();
    await ingestor.pollOnce();

    const rows = db.prepare('SELECT * FROM events').all() as any[];
    expect(rows).toHaveLength(1);
    expect(rows[0].event_id).toBe('C1:evt-1');
    expect(rows[0].event_type).toBe('swap');
    expect(JSON.parse(rows[0].data)).toEqual({ amount: 10 });
    expect(
      (db.prepare("SELECT last_ledger FROM checkpoints WHERE stream = 'events:C1'").get() as any)
        .last_ledger
    ).toBe(101);
    db.close();
  });

  it('indexes ledgers, transactions, operations and accounts, and replays safely', async () => {
    const db = memoryDb();
    const config = testConfig({ horizonUrl: 'http://horizon.test' });
    const tx = {
      hash: 'tx1',
      ledger: 101,
      successful: true,
      source_account: ACCOUNT,
      fee_account: ACCOUNT,
      fee_charged: '100',
      max_fee: '100',
      operation_count: 1,
      created_at: '2026-10-21T00:00:00Z',
      memo_type: 'none',
    };
    const op = {
      id: 'op1',
      paging_token: 'op1',
      transaction_hash: 'tx1',
      source_account: ACCOUNT,
      type: 'payment',
      type_i: 1,
      created_at: '2026-10-21T00:00:00Z',
      transaction_successful: true,
      from: ACCOUNT,
      to: ACCOUNT,
      amount: '10',
    };
    const horizon = fakeHorizon({
      transactions: [tx],
      operations: [op],
      account: {
        account_id: ACCOUNT,
        sequence: '100',
        sequence_ledger: 101,
        last_modified_ledger: 101,
        balances: [{ asset_type: 'native', balance: '10.0' }],
        signers: [],
        thresholds: {},
        flags: {},
      },
    });
    setCheckpoint(db, 'horizon', 100);
    const ingestor = new SubanIngestor(db, config, fakeRpc([], 101), horizon);

    await ingestor.pollOnce();

    expect((db.prepare('SELECT COUNT(*) AS c FROM ledgers').get() as any).c).toBe(1);
    expect((db.prepare('SELECT COUNT(*) AS c FROM transactions').get() as any).c).toBe(1);
    expect((db.prepare('SELECT COUNT(*) AS c FROM operations').get() as any).c).toBe(1);
    expect((db.prepare('SELECT COUNT(*) AS c FROM transaction_accounts').get() as any).c).toBe(1);
    expect((db.prepare('SELECT COUNT(*) AS c FROM operation_accounts').get() as any).c).toBe(1);
    const account = db.prepare('SELECT * FROM accounts WHERE account_id = ?').get(ACCOUNT) as any;
    expect(JSON.parse(account.balances)[0].balance).toBe('10.0');

    ingestor.resetStream('horizon', 100);
    await ingestor.pollOnce();
    expect((db.prepare('SELECT COUNT(*) AS c FROM ledgers').get() as any).c).toBe(1);
    expect((db.prepare('SELECT COUNT(*) AS c FROM transactions').get() as any).c).toBe(1);
    expect((db.prepare('SELECT COUNT(*) AS c FROM operations').get() as any).c).toBe(1);
    db.close();
  });
});
