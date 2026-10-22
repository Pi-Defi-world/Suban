import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import express from 'express';
import type { Server } from 'http';
import type { AddressInfo } from 'net';
import { Address, nativeToScVal, StrKey } from '@stellar/stellar-sdk';
import { createApi } from '../src/api.js';
import { QuotaStore } from '../src/apiKeys.js';
import { createFinancialRouter } from '../src/financial.js';
import { SubanIngestor } from '../src/ingest.js';
import { WebhookStore } from '../src/webhooks.js';
import { setCheckpoint, setMeta } from '../src/meta.js';
import { fakeRpc, memoryDb, testConfig } from './helpers.js';

const ACCOUNT = 'GB6PCM5GBUZR3EYET4IWQ4TBI7IYPFXFJI4TX3BHDPC6H3PSGB5UYB7W';
const POOL = 'CCJZ5DGASBWQXR5MP74KZQ6ZJZ3M7N5ZL7QX2X7ZM7ZP5W5Q5X5Q5X5Q';
const TOKEN_A = StrKey.encodeContract(Buffer.alloc(32, 1));
const TOKEN_B = StrKey.encodeContract(Buffer.alloc(32, 2));

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
    expect(body.coverage.lp_positions).toBe(true);

    const detail = await (await fetch(`${base}/financial/pools/${POOL}`)).json();
    expect(detail.data.volume.amount_in_raw).toBe('100');
    expect(detail.data.volume.amount_out_raw).toBe('50');
    expect(detail.data.volume.usd_value).toBeNull();
    expect(detail.data.lp_positions.available).toBe(true);
    expect(detail.data.lp_positions.positions[0]).toMatchObject({
      account: ACCOUNT,
      shares_raw: '25',
    });

    const valuation = await fetch(`${base}/financial/pools/${POOL}/valuation`);
    expect(valuation.status).toBe(503);
    const valuationBody = await valuation.json();
    expect(valuationBody.message).toContain('INDEXER_PRICE_API_URL');
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
    expect(activity.coverage.lp_positions).toBe(true);
  });

  it('exposes enriched token direction, reserves, LP positions and lending snapshots', async () => {
    db.prepare(
      `INSERT INTO events
       (event_id, contract_id, contract_name, event_type, tx_hash, ledger, timestamp, data, raw)
       VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)`
    ).run(
      'E3',
      POOL,
      'cpmm-pool',
      'swap',
      'tx3',
      103,
      1700000200,
      JSON.stringify([1000, 900, TOKEN_A, TOKEN_B, 11000, 9100, 30, 5000, TOKEN_A, TOKEN_B]),
      rawEvent([topic('swap'), new Address(ACCOUNT).toScVal().toXDR('base64')])
    );
    db.prepare(
      `INSERT INTO events
       (event_id, contract_id, contract_name, event_type, tx_hash, ledger, timestamp, data, raw)
       VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)`
    ).run(
      'E4',
      POOL,
      'cpmm-pool',
      'add_liq',
      'tx4',
      104,
      1700000300,
      JSON.stringify([1000, 1000, 1000, TOKEN_A, TOKEN_B, 10000, 10000, 1000]),
      rawEvent([topic('add_liq'), new Address(ACCOUNT).toScVal().toXDR('base64')])
    );
    db.prepare(
      `INSERT INTO events
       (event_id, contract_id, contract_name, event_type, tx_hash, ledger, timestamp, data, raw)
       VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)`
    ).run(
      'E5',
      POOL,
      'lending-pool',
      'deposit',
      'tx5',
      105,
      1700000400,
      JSON.stringify([1000, 1000, 1000, 0, 500, 1000, 0, 0]),
      rawEvent([topic('deposit'), new Address(ACCOUNT).toScVal().toXDR('base64')])
    );
    db.prepare(
      `INSERT INTO events
       (event_id, contract_id, contract_name, event_type, tx_hash, ledger, timestamp, data, raw)
       VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)`
    ).run(
      'E6',
      POOL,
      'lending-pool',
      'borrow',
      'tx6',
      106,
      1700000500,
      JSON.stringify([100, 1000, 100, 500, 1000, 100, 0]),
      rawEvent([topic('borrow'), new Address(ACCOUNT).toScVal().toXDR('base64')])
    );

    const detail = await (await fetch(`${base}/financial/pools/${POOL}`)).json();
    expect(detail.data.state).toMatchObject({
      token_a: TOKEN_A,
      token_b: TOKEN_B,
      reserve_a_raw: '10000',
      reserve_b_raw: '10000',
      total_shares_raw: '1000',
      fee_bps: 30,
    });
    expect(detail.data.volume.by_token_in_raw[TOKEN_A]).toBe('1000');
    expect(detail.data.lp_positions.positions[0]).toMatchObject({
      account: ACCOUNT,
      shares_raw: '1025',
    });

    // Route discovery reads the same enriched state from the pool list, so a
    // caller does not have to issue one request per discovered pool.
    const list = await (await fetch(`${base}/financial/pools`)).json();
    expect(list.data[0].state).toMatchObject({
      token_a: TOKEN_A,
      token_b: TOKEN_B,
      reserve_a_raw: '10000',
      reserve_b_raw: '10000',
      fee_bps: 30,
      ledger: 104,
    });
    expect(list.coverage.route_discovery).toBe(true);

    const position = await (
      await fetch(`${base}/financial/lending/accounts/${ACCOUNT}`)
    ).json();
    expect(position.data.position).toEqual({
      deposit_raw: '1000',
      borrow_raw: '100',
      collateral_raw: '500',
    });
    expect(position.data.as_of.event_type).toBe('borrow');
    expect(position.data.source).toBe('indexed_contract_events');

    const lending = await (await fetch(`${base}/financial/lending/summary`)).json();
    expect(lending.coverage.lending_events).toBe(true);
    expect(lending.counts.deposit).toBe(1);
    expect(lending.counts.borrow).toBe(1);
  });

  it('publishes a valuation only for fresh, healthy, mapped prices', async () => {
    db.prepare('DELETE FROM events').run();
    db.prepare(
      `INSERT INTO events
       (event_id, contract_id, contract_name, event_type, tx_hash, ledger, timestamp, data, raw)
       VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)`
    ).run(
      'V1',
      POOL,
      'cpmm-pool',
      'swap',
      'txv1',
      201,
      1700001000,
      JSON.stringify([1000, 900, TOKEN_A, TOKEN_B, 11000, 9100, 30, 5000, TOKEN_A, TOKEN_B]),
      rawEvent([topic('swap'), new Address(ACCOUNT).toScVal().toXDR('base64')])
    );
    db.prepare(
      `INSERT INTO events
       (event_id, contract_id, contract_name, event_type, tx_hash, ledger, timestamp, data, raw)
       VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)`
    ).run(
      'V2',
      POOL,
      'cpmm-pool',
      'add_liq',
      'txv2',
      202,
      1700001100,
      JSON.stringify([1000, 1000, 1000, TOKEN_A, TOKEN_B, 10000, 10000, 1000]),
      rawEvent([topic('add_liq'), new Address(ACCOUNT).toScVal().toXDR('base64')])
    );

    let fresh = true;
    const fetchImpl = (async (input: unknown) => {
      const asset = new URL(String(input)).searchParams.get('asset');
      return new Response(
        JSON.stringify({
          price_usd: asset === 'PI' ? 2 : 1,
          timestamp: '2026-10-22T00:00:00.000Z',
          is_fresh: fresh,
          trust: { state: fresh ? 'HEALTHY' : 'DEGRADED' },
        }),
        { status: 200, headers: { 'content-type': 'application/json' } }
      );
    }) as typeof fetch;

    const app = express();
    app.use(
      '/financial',
      createFinancialRouter(
        db,
        (body) => ({ schema_version: '2.0', ...body }),
        {
          priceApiUrl: 'http://prices.test',
          tokenAssets: { [TOKEN_A]: 'PI', [TOKEN_B]: 'USDC' },
          tokenDecimals: { [TOKEN_A]: 7, [TOKEN_B]: 7 },
          fetchImpl,
        }
      )
    );
    const valuationServer = app.listen(0);
    await new Promise<void>((resolve) => valuationServer.once('listening', resolve));
    const valuationBase = `http://127.0.0.1:${
      (valuationServer.address() as AddressInfo).port
    }`;

    try {
      const response = await fetch(`${valuationBase}/financial/pools/${POOL}/valuation`);
      expect(response.status).toBe(200);
      const body = await response.json();
      expect(body.data.source).toBe('indexed_contract_events');
      expect(body.data.valuation_source).toBe('controller_price_api');
      expect(body.data.reserves_usd.token_a).toBeCloseTo(0.002);
      expect(body.data.reserves_usd.token_b).toBeCloseTo(0.001);
      expect(body.data.reserves_usd.total).toBeCloseTo(0.003);
      expect(body.data.volume_usd).toBeCloseTo(0.0002);
      expect(body.data.as_of.ledger).toBe(202);

      fresh = false;
      const stale = await fetch(`${valuationBase}/financial/pools/${POOL}/valuation`);
      expect(stale.status).toBe(503);
      expect((await stale.json()).message).toContain('stale');
    } finally {
      await new Promise<void>((resolve) => valuationServer.close(() => resolve()));
    }
  });
});
