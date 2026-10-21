import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import type { Server } from 'http';
import type { AddressInfo } from 'net';
import { createApi } from '../src/api.js';
import { QuotaStore } from '../src/apiKeys.js';
import { parseApiKeys } from '../src/config.js';
import { setCheckpoint, setMeta } from '../src/meta.js';
import { SubanIngestor } from '../src/ingest.js';
import { WebhookStore } from '../src/webhooks.js';
import { memoryDb, fakeRpc, testConfig } from './helpers.js';

const ACCOUNT = 'GB6PCM5GBUZR3EYET4IWQ4TBI7IYPFXFJI4TX3BHDPC6H3PSGB5UYB7W';

describe('data API', () => {
  let db: any;
  let server: Server;
  let base: string;

  beforeEach(async () => {
    db = memoryDb();
    db.prepare(
      `INSERT INTO events
       (event_id, contract_id, contract_name, event_type, tx_hash, ledger, timestamp, data)
       VALUES ('C1:evt-1', 'C1', 'pool', 'swap', 'tx1', 101, 1700000000, '{"amount":10}')`
    ).run();
    db.prepare(
      `INSERT INTO transactions
       (hash, ledger, successful, source_account, fee_account, created_at)
       VALUES ('tx1', 101, 1, ?, ?, '2026-10-21T00:00:00Z')`
    ).run(ACCOUNT, ACCOUNT);
    db.prepare(
      `INSERT INTO operations
       (id, tx_hash, ledger, source_account, type, created_at, details)
       VALUES ('op1', 'tx1', 101, ?, 'payment', '2026-10-21T00:00:00Z', '{"amount":"10"}')`
    ).run(ACCOUNT);
    db.prepare(
      `INSERT INTO transaction_accounts (tx_hash, account_id) VALUES ('tx1', ?)`
    ).run(ACCOUNT);
    db.prepare(
      `INSERT INTO operation_accounts (operation_id, account_id) VALUES ('op1', ?)`
    ).run(ACCOUNT);
    db.prepare(
      `INSERT INTO accounts (account_id, last_seen_ledger, balances)
       VALUES (?, 101, '[{"asset_type":"native","balance":"10.0"}]')`
    ).run(ACCOUNT);
    db.prepare(
      `INSERT INTO contracts (contract_id, network, name, interface_version, verified)
       VALUES ('C1', 'Pi Testnet', 'pool', '1.0.0', 0)`
    ).run();
    setCheckpoint(db, 'horizon', 101);
    setMeta(db, 'latest_observed_ledger', 101);
    setMeta(db, 'last_success_at', new Date().toISOString());
    setMeta(db, 'last_success_at_ms', Math.floor(Date.now() / 1000));

    const config = testConfig({
      requireApiKey: true,
      apiKeys: parseApiKeys('testkey:test:100:1000'),
      adminApiKey: 'admin-key',
      contracts: [],
    });
    const ingestor = new SubanIngestor(db, config, fakeRpc([], 101));
    const app = createApi({
      db,
      config,
      ingestor,
      webhooks: new WebhookStore(db),
      quota: new QuotaStore(db),
      isRunning: () => true,
    });
    server = app.listen(0);
    await new Promise<void>((resolve) => server.once('listening', resolve));
    base = `http://127.0.0.1:${(server.address() as AddressInfo).port}`;
  });

  afterEach(async () => {
    await new Promise<void>((resolve) => server.close(() => resolve()));
    db.close();
  });

  it('requires an API key for data routes', async () => {
    const unauthorized = await fetch(`${base}/events`);
    expect(unauthorized.status).toBe(401);

    const response = await fetch(`${base}/events`, {
      headers: { 'x-api-key': 'testkey' },
    });
    expect(response.status).toBe(200);
    const body = await response.json();
    expect(body.schema_version).toBe('2.0');
    expect(body.freshness.latest_indexed_ledger).toBe(101);
    expect(body.freshness.stale).toBe(false);
    expect(body.data[0].type).toBe('swap');
    expect(body.data[0].data).toEqual({ amount: 10 });
    expect(response.headers.get('x-api-key-name')).toBe('test');
  });

  it('serves historical transactions, operations and accounts from SQLite', async () => {
    const headers = { 'x-api-key': 'testkey' };
    const tx = await (await fetch(`${base}/transactions?account=${ACCOUNT}`, { headers })).json();
    expect(tx.data).toHaveLength(1);
    expect(tx.data[0].hash).toBe('tx1');

    const ops = await (await fetch(`${base}/operations?account=${ACCOUNT}`, { headers })).json();
    expect(ops.data).toHaveLength(1);
    expect(ops.data[0].type).toBe('payment');

    const account = await (await fetch(`${base}/accounts/${ACCOUNT}`, { headers })).json();
    expect(account.data.account_id).toBe(ACCOUNT);
    expect(account.data.balances[0].balance).toBe('10.0');

    const contracts = await (await fetch(`${base}/contracts`, { headers })).json();
    expect(contracts.data[0].contract_id).toBe('C1');
  });

  it('protects admin routes and exposes per-key usage', async () => {
    const missing = await fetch(`${base}/admin/api-keys/usage`);
    expect(missing.status).toBe(401);

    const response = await fetch(`${base}/admin/api-keys/usage`, {
      headers: { 'x-api-key': 'admin-key' },
    });
    expect(response.status).toBe(200);
    const body = await response.json();
    expect(body.require_api_key).toBe(true);
    expect(body.keys[0].name).toBe('test');
  });

  it('reports health without requiring a public key', async () => {
    const response = await fetch(`${base}/health`);
    expect(response.status).toBe(200);
    const body = await response.json();
    expect(body.status).toBe('healthy');
    expect(body.freshness.lag_ledgers).toBe(0);
  });
});
