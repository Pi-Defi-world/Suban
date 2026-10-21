/**
 * Webhook subscriptions, durable outbox and retry dispatcher.
 *
 * A delivery is created in the same SQLite database as the indexed event. The
 * unique (subscription_id, event_id) constraint makes enqueueing idempotent, and
 * the dispatcher retries with exponential backoff until INDEXER_WEBHOOK_MAX_ATTEMPTS.
 */
import crypto from 'crypto';
import Database from 'better-sqlite3';
import { IndexerConfig } from './config.js';
import {
  webhookDeliveries,
  webhookDuration,
  webhookPending,
} from './metrics.js';

export interface WebhookSubscriptionInput {
  url: string;
  eventTypes?: string[];
  contractIds?: string[];
  secret?: string;
  active?: boolean;
}

export interface WebhookSubscription {
  id: string;
  url: string;
  secret: string;
  event_types: string[];
  contract_ids: string[];
  active: boolean;
  created_at: string;
  updated_at: string;
}

export interface IndexedEventForWebhook {
  event_id: string;
  contract_id: string;
  contract_name: string;
  event_type: string;
  tx_hash: string;
  ledger: number;
  timestamp: number;
  data: unknown;
}

function parseArray(value: string): string[] {
  try {
    const parsed = JSON.parse(value);
    return Array.isArray(parsed) ? parsed.map(String) : [];
  } catch {
    return [];
  }
}

function normalizeUrl(raw: string): string {
  const url = new URL(raw);
  if (url.protocol !== 'http:' && url.protocol !== 'https:') {
    throw new Error('Webhook URL must use http or https');
  }
  return url.toString();
}

export class WebhookStore {
  constructor(private readonly db: Database.Database) {}

  createSubscription(input: WebhookSubscriptionInput): WebhookSubscription {
    const url = normalizeUrl(input.url);
    const id = `wh_${crypto.randomUUID()}`;
    const secret = input.secret || crypto.randomBytes(32).toString('hex');
    const eventTypes = input.eventTypes?.length ? input.eventTypes : ['*'];
    const contractIds = input.contractIds || [];

    this.db
      .prepare(
        `INSERT INTO webhook_subscriptions
         (id, url, secret, event_types, contract_ids, active)
         VALUES (?, ?, ?, ?, ?, ?)`
      )
      .run(
        id,
        url,
        secret,
        JSON.stringify(eventTypes),
        JSON.stringify(contractIds),
        input.active === false ? 0 : 1
      );

    return this.getSubscription(id)!;
  }

  getSubscription(id: string): WebhookSubscription | null {
    const row = this.db
      .prepare('SELECT * FROM webhook_subscriptions WHERE id = ?')
      .get(id) as any;
    if (!row) return null;
    return {
      ...row,
      event_types: parseArray(row.event_types),
      contract_ids: parseArray(row.contract_ids),
      active: Boolean(row.active),
    };
  }

  listSubscriptions(): Array<Omit<WebhookSubscription, 'secret'>> {
    const rows = this.db
      .prepare('SELECT * FROM webhook_subscriptions ORDER BY created_at DESC')
      .all() as any[];
    return rows.map((row) => {
      const { secret: _secret, ...rest } = row;
      return {
        ...rest,
        event_types: parseArray(row.event_types),
        contract_ids: parseArray(row.contract_ids),
        active: Boolean(row.active),
      };
    });
  }

  deleteSubscription(id: string): boolean {
    return this.db.prepare('DELETE FROM webhook_subscriptions WHERE id = ?').run(id).changes > 0;
  }

  enqueue(
    event: IndexedEventForWebhook,
    now = Math.floor(Date.now() / 1000)
  ): number {
    const payload = JSON.stringify({
      id: event.event_id,
      type: event.event_type,
      contract_id: event.contract_id,
      contract_name: event.contract_name,
      tx_hash: event.tx_hash,
      ledger: event.ledger,
      timestamp: event.timestamp,
      data: event.data,
    });

    const subscriptions = this.db
      .prepare('SELECT * FROM webhook_subscriptions WHERE active = 1')
      .all() as any[];
    if (subscriptions.length === 0) return 0;

    let inserted = 0;
    const insert = this.db.prepare(
      `INSERT OR IGNORE INTO webhook_deliveries
       (id, subscription_id, event_id, status, attempts, next_attempt_at, payload)
       VALUES (?, ?, ?, 'pending', 0, ?, ?)`
    );

    this.db.transaction(() => {
      for (const subscription of subscriptions) {
        const eventTypes = parseArray(subscription.event_types);
        const contractIds = parseArray(subscription.contract_ids);
        const typeMatches =
          eventTypes.length === 0 ||
          eventTypes.includes('*') ||
          eventTypes.includes(event.event_type);
        const contractMatches =
          contractIds.length === 0 || contractIds.includes(event.contract_id);
        if (!typeMatches || !contractMatches) continue;

        inserted += insert.run(
          `wd_${crypto.randomUUID()}`,
          subscription.id,
          event.event_id,
          now,
          payload
        ).changes;
      }
    })();

    if (inserted > 0) this.refreshPending();
    return inserted;
  }

  due(now = Math.floor(Date.now() / 1000), limit = 50): any[] {
    return this.db
      .prepare(
        `SELECT * FROM webhook_deliveries
         WHERE status = 'pending' AND next_attempt_at <= ?
         ORDER BY next_attempt_at ASC LIMIT ?`
      )
      .all(now, limit);
  }

  markDelivered(id: string): void {
    this.db
      .prepare(
        `UPDATE webhook_deliveries
         SET status = 'delivered', delivered_at = CURRENT_TIMESTAMP, last_error = NULL
         WHERE id = ?`
      )
      .run(id);
    this.refreshPending();
  }

  markRetry(id: string, attempts: number, nextAttemptAt: number, error: string): void {
    this.db
      .prepare(
        `UPDATE webhook_deliveries
         SET attempts = ?, next_attempt_at = ?, last_error = ?
         WHERE id = ?`
      )
      .run(attempts, nextAttemptAt, error.slice(0, 1_000), id);
    this.refreshPending();
  }

  markFailed(id: string, attempts: number, error: string): void {
    this.db
      .prepare(
        `UPDATE webhook_deliveries
         SET status = 'failed', attempts = ?, last_error = ?
         WHERE id = ?`
      )
      .run(attempts, error.slice(0, 1_000), id);
    this.refreshPending();
  }

  refreshPending(): void {
    const row = this.db
      .prepare("SELECT COUNT(*) AS count FROM webhook_deliveries WHERE status = 'pending'")
      .get() as { count: number };
    webhookPending.set(row?.count || 0);
  }
}

export type FetchLike = typeof fetch;

export class WebhookDispatcher {
  private timer: NodeJS.Timeout | null = null;
  private running = false;
  private store: WebhookStore;

  constructor(
    db: Database.Database,
    private readonly config: IndexerConfig,
    private readonly fetchImpl: FetchLike = fetch
  ) {
    this.store = new WebhookStore(db);
  }

  start(): void {
    if (this.running) return;
    this.running = true;
    this.store.refreshPending();
    this.timer = setInterval(() => {
      void this.dispatchOnce();
    }, this.config.webhook.dispatchIntervalMs);
    this.timer.unref?.();
  }

  stop(): void {
    this.running = false;
    if (this.timer) clearInterval(this.timer);
    this.timer = null;
  }

  async dispatchOnce(now = Math.floor(Date.now() / 1000)): Promise<number> {
    const deliveries = this.store.due(now);
    let delivered = 0;
    for (const delivery of deliveries) {
      const subscription = this.store.getSubscription(delivery.subscription_id);
      if (!subscription || !subscription.active) {
        this.store.markFailed(delivery.id, delivery.attempts, 'subscription inactive or missing');
        continue;
      }
      const body = delivery.payload;
      const signature = crypto
        .createHmac('sha256', subscription.secret)
        .update(body)
        .digest('hex');
      const started = process.hrtime.bigint();
      let outcome = 'failed';
      const controller = new AbortController();
      const timer = setTimeout(
        () => controller.abort(),
        this.config.webhook.timeoutMs
      );

      try {
        const response = await this.fetchImpl(subscription.url, {
          method: 'POST',
          headers: {
            'content-type': 'application/json',
            'user-agent': 'suban-indexer/2.0',
            'x-suban-event': delivery.event_id,
            'x-suban-delivery': delivery.id,
            'x-suban-signature': `sha256=${signature}`,
            'idempotency-key': `delivery:${delivery.id}`,
          },
          body,
          signal: controller.signal,
        });

        if (response.ok) {
          this.store.markDelivered(delivery.id);
          delivered++;
          outcome = 'delivered';
          webhookDeliveries.inc({ result: 'delivered' });
        } else {
          const text = await response.text().catch(() => '');
          const attempts = delivery.attempts + 1;
          this.handleFailure(
            delivery,
            attempts,
            `HTTP ${response.status}${text ? `: ${text.slice(0, 200)}` : ''}`,
            now
          );
        }
      } catch (error: any) {
        const attempts = delivery.attempts + 1;
        const message = error?.name === 'AbortError' ? 'timeout' : error?.message || String(error);
        this.handleFailure(delivery, attempts, message, now);
      } finally {
        clearTimeout(timer);
        const seconds = Number(process.hrtime.bigint() - started) / 1e9;
        webhookDuration.observe({ result: outcome }, seconds);
      }
    }
    this.store.refreshPending();
    return delivered;
  }

  private handleFailure(
    delivery: any,
    attempts: number,
    message: string,
    now = Math.floor(Date.now() / 1000)
  ): void {
    if (attempts >= this.config.webhook.maxAttempts) {
      this.store.markFailed(delivery.id, attempts, message);
      webhookDeliveries.inc({ result: 'failed' });
      return;
    }
    const delay = Math.min(
      this.config.webhook.maxBackoffMs,
      this.config.webhook.baseBackoffMs * 2 ** Math.max(0, attempts - 1)
    );
    this.store.markRetry(
      delivery.id,
      attempts,
      now + Math.ceil(delay / 1000),
      message
    );
    webhookDeliveries.inc({ result: 'retry' });
  }
}
