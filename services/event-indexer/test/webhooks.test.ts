import crypto from 'crypto';
import { describe, expect, it } from 'vitest';
import { WebhookDispatcher, WebhookStore } from '../src/webhooks.js';
import { memoryDb, testConfig } from './helpers.js';

const event = {
  event_id: 'C1:evt-1',
  contract_id: 'C1',
  contract_name: 'pool',
  event_type: 'swap',
  tx_hash: 'tx1',
  ledger: 101,
  timestamp: 1_700_000_000,
  data: { amount: '10' },
};

describe('webhook outbox', () => {
  it('enqueues matching events once and signs delivery', async () => {
    const db = memoryDb();
    const store = new WebhookStore(db);
    store.createSubscription({
      url: 'https://example.test/hook',
      eventTypes: ['swap'],
      secret: 's3cret',
    });
    store.createSubscription({
      url: 'https://example.test/other',
      eventTypes: ['transfer'],
      secret: 'other',
    });

    expect(store.enqueue(event, 1_700_000_000)).toBe(1);
    expect(store.enqueue(event, 1_700_000_000)).toBe(0);
    const due = store.due(1_700_000_000);
    expect(due).toHaveLength(1);

    const expected = crypto
      .createHmac('sha256', 's3cret')
      .update(due[0].payload)
      .digest('hex');
    const calls: any[] = [];
    const fakeFetch = async (_url: string, init: any) => {
      calls.push(init);
      return new Response(null, { status: 204 });
    };
    const dispatcher = new WebhookDispatcher(db, testConfig(), fakeFetch as any);
    expect(await dispatcher.dispatchOnce(1_700_000_000)).toBe(1);
    expect(calls).toHaveLength(1);
    expect(calls[0].headers['x-suban-signature']).toBe(`sha256=${expected}`);
    expect(calls[0].headers['idempotency-key']).toMatch(/^delivery:wd_/);
    expect(store.due(1_700_000_000)).toHaveLength(0);
    db.close();
  });

  it('retries with backoff and eventually marks failed', async () => {
    const db = memoryDb();
    const store = new WebhookStore(db);
    store.createSubscription({
      url: 'https://example.test/hook',
      eventTypes: ['swap'],
      secret: 's3cret',
    });
    store.enqueue(event, 1_700_000_000);
    const config = testConfig({
      webhook: {
        dispatchIntervalMs: 60_000,
        maxAttempts: 2,
        baseBackoffMs: 1_000,
        maxBackoffMs: 10_000,
        timeoutMs: 1_000,
      },
    });
    const fakeFetch = async () => new Response('nope', { status: 500 });
    const dispatcher = new WebhookDispatcher(db, config, fakeFetch as any);

    await dispatcher.dispatchOnce(1_700_000_000);
    let delivery = db
      .prepare('SELECT status, attempts, next_attempt_at FROM webhook_deliveries')
      .get() as any;
    expect(delivery.status).toBe('pending');
    expect(delivery.attempts).toBe(1);

    await dispatcher.dispatchOnce(1_700_000_010);
    delivery = db
      .prepare('SELECT status, attempts FROM webhook_deliveries')
      .get() as any;
    expect(delivery.status).toBe('failed');
    expect(delivery.attempts).toBe(2);
    db.close();
  });
});
