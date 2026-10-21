import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import type { Server } from 'http';
import type { AddressInfo } from 'net';
import { Address, nativeToScVal } from '@stellar/stellar-sdk';
import { createApi } from '../src/api.js';
import { QuotaStore } from '../src/apiKeys.js';
import { SubanIngestor } from '../src/ingest.js';
import { WebhookStore } from '../src/webhooks.js';
import { setCheckpoint, setMeta } from '../src/meta.js';
import { fakeRpc, memoryDb, testConfig } from './helpers.js';

const ACCOUNT = 'GB6PCM5GBUZR3EYET4IWQ4TBI7IYPFXFJI4TX3BHDPC6H3PSGB5UYB7W';
const POOL = 'CCJZ5DGASBWQXR5MP74KZQ6ZJZ3M7N5ZL7QX2X7ZM7ZP5W5Q5X5Q5X5Q';

function topic(value: unknown) {
  return nativeToScVal(value).toXDR('base64');
}

function rawEvent(topics: string[]) {
  return JSON.stringify({ topic: topics, value: 'not-decoded-here' });
}

describe('financial data projections', () => {
  let db: any;
  let server: Server;
  let base: string;

  beforeEach(async () => {
    db = memoryDb();
    const accountTopic = new Address(ACCOUNT).toScVal().toXDR('base64');
    db.prepare(
      `INSERT INTO events
       (event_id, contract_id, contract_name, event_type, tx_hash, ledger, timestamp, data, raw)
       VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)`
    ).run(
      'E1',
      POOL,
      'cpmm-pool',
      'swap',
      'tx1',
      101,
      1700000000,
      '[100,50]',
      rawEvent([topic('swap'), accountTopic])
    );
    db.prepare(
      `INSERT INTO events
       (event_id, contract_id, contract_name, event_type, tx_hash, ledger, timestamp, data, raw)
       VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)`
    ).run(
      'E2',
      POOL,
      'cpmm-pool',
      'add_liq',
      'tx2',
      102,
      1700000100,
      '[10,20,25]',
      rawEvent([topic('add_liq'), accountTopic])
    );
    db.prepare(
      `INSERT INTO contracts (contract_id, network, name, interface_version, verified)
       VALUES (?, 'Pi Testnet', 'cpmm-pool', '1.0.0', 0)`
    ).run(POOL);
    setCheckpoint(db, 'horizon', 102);
    setMeta(db, 'latest_observed_ledger', 102);
    setMeta(db, 'last_success_at', new Date().toISOString());
    setMeta(db, 'last_success_at_ms', Math.floor(Date.now() / 1000));

    const config = testConfig({ contracts: [] });
    const app = createApi({
      db,
      config,
      ingestor: new SubanIngestor(db, config, fakeRpc([], 102)),
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

  it('lists pools and reports raw volume without inventing USD values', async () => {
    const response = await fetch(`${base}/financial/pools`);
    expect(response.status).toBe(200);
    const body = await response.json();

    expect(body.data).toHaveLength(1);
    expect(body.data[0]).toMatchObject({
      contract_id: POOL,
      pool_kind: 'cpmm',
      event_count: 2,
      trade_count: 1,
      add_liquidity_count: 1,
      remove_liquidity_count: 0,
    });
    expect(body.coverage.usd_volume).toBe(false);
    expect(body.coverage.lp_positions).toBe(false);

    const detail = await (await fetch(`${base}/financial/pools/${POOL}`)).json();
    expect(detail.data.volume.amount_in_raw).toBe('100');
    expect(detail.data.volume.amount_out_raw).toBe('50');
    expect(detail.data.volume.usd_value).toBeNull();
    expect(detail.data.lp_positions.available).toBe(false);
  });

  it('serves trades, liquidity events and bounded account activity', async () => {
    const trades = await (await fetch(`${base}/financial/pools/${POOL}/trades`)).json();
    expect(trades.data).toHaveLength(1);
    expect(trades.data[0]).toMatchObject({
      event_type: 'swap',
      account: ACCOUNT,
      amount_a_raw: '100',
      amount_b_raw: '50',
      usd_value: null,
    });

    const liquidity = await (
      await fetch(`${base}/financial/pools/${POOL}/liquidity`)
    ).json();
    expect(liquidity.data).toHaveLength(1);
    expect(liquidity.data[0]).toMatchObject({
      event_type: 'add_liquidity',
      amount_a_raw: '10',
      amount_b_raw: '20',
      shares_raw: '25',
    });

    const activity = await (
      await fetch(`${base}/financial/accounts/${ACCOUNT}/activity`)
    ).json();
    expect(activity.data).toHaveLength(2);
    expect(activity.coverage.lp_positions).toBe(false);
  });
});
