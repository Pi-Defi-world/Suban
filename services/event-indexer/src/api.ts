/**
 * Public and administrative HTTP API for the data platform.
 *
 * All data routes read from SQLite. No query handler calls Horizon or Soroban
 * RPC, so historical responses are reproducible from the indexed checkpoint.
 */
import express, { NextFunction, Request, Response } from 'express';
import cors from 'cors';
import Database from 'better-sqlite3';
import { IndexerConfig, SCHEMA_VERSION } from './config.js';
import { createAdminMiddleware, createApiKeyMiddleware, QuotaStore } from './apiKeys.js';
import { getFreshness } from './freshness.js';
import { SubanIngestor, upsertContract } from './ingest.js';
import { httpMetricsMiddleware, metricsHandler } from './metrics.js';
import { WebhookStore } from './webhooks.js';

const MAX_LIMIT = 1_000;

function limitOf(req: Request): number {
  const parsed = parseInt(String(req.query.limit || '100'), 10);
  return Math.min(MAX_LIMIT, Math.max(1, Number.isFinite(parsed) ? parsed : 100));
}

function offsetOf(req: Request): number {
  const parsed = parseInt(String(req.query.offset || '0'), 10);
  return Math.max(0, Number.isFinite(parsed) ? parsed : 0);
}

function epochOf(value: unknown): number | null {
  if (value === undefined || value === null || value === '') return null;
  const textual = String(value);
  const numeric = Number(textual);
  if (Number.isFinite(numeric) && !textual.includes('-')) return Math.floor(numeric);
  const parsed = Date.parse(textual);
  return Number.isFinite(parsed) ? Math.floor(parsed / 1000) : null;
}

function isoOf(value: unknown): string | null {
  if (value === undefined || value === null || value === '') return null;
  const parsed = Date.parse(String(value));
  return Number.isFinite(parsed) ? new Date(parsed).toISOString() : null;
}

function parseJson(value: unknown, fallback: unknown): unknown {
  if (typeof value !== 'string') return fallback;
  try {
    return JSON.parse(value);
  } catch {
    return fallback;
  }
}

function eventRow(row: any) {
  return {
    id: row.event_id || `event_${row.id}`,
    contract_id: row.contract_id,
    contract_name: row.contract_name,
    type: row.event_type,
    tx_hash: row.tx_hash,
    ledger: row.ledger,
    timestamp: row.timestamp,
    ledger_closed_at: row.ledger_closed_at,
    data: parseJson(row.data, row.data),
  };
}

function transactionRow(row: any) {
  return {
    hash: row.hash,
    ledger: row.ledger,
    successful: Boolean(row.successful),
    source_account: row.source_account,
    fee_account: row.fee_account,
    fee_charged: row.fee_charged,
    max_fee: row.max_fee,
    operation_count: row.operation_count,
    created_at: row.created_at,
    memo_type: row.memo_type,
    memo: row.memo,
  };
}

function operationRow(row: any) {
  const details = parseJson(row.details, {});
  return {
    id: row.id,
    tx_hash: row.tx_hash,
    ledger: row.ledger,
    source_account: row.source_account,
    type: row.type,
    type_i: row.type_i,
    created_at: row.created_at,
    transaction_successful: Boolean(row.transaction_successful),
    details,
  };
}

function accountRow(row: any) {
  return {
    account_id: row.account_id,
    sequence: row.sequence,
    sequence_ledger: row.sequence_ledger,
    last_modified_ledger: row.last_modified_ledger,
    last_seen_ledger: row.last_seen_ledger,
    balances: parseJson(row.balances, []),
    signers: parseJson(row.signers, []),
    thresholds: parseJson(row.thresholds, {}),
    flags: parseJson(row.flags, {}),
    home_domain: row.home_domain,
    subentry_count: row.subentry_count,
    num_sponsoring: row.num_sponsoring,
    num_sponsored: row.num_sponsored,
    updated_at: row.updated_at,
  };
}

function contractRow(row: any) {
  return {
    contract_id: row.contract_id,
    network: row.network,
    name: row.name,
    interface_version: row.interface_version,
    verified: Boolean(row.verified),
    metadata: parseJson(row.metadata, {}),
    created_at: row.created_at,
    updated_at: row.updated_at,
  };
}

interface QueryPage {
  rows: any[];
  total: number;
  limit: number;
  offset: number;
}

function page(
  db: Database.Database,
  table: string,
  where: string[],
  params: unknown[],
  limit: number,
  offset: number,
  order: string
): QueryPage {
  const clause = where.length ? `WHERE ${where.join(' AND ')}` : '';
  const total = (
    db.prepare(`SELECT COUNT(*) AS count FROM ${table} ${clause}`).get(...params) as {
      count: number;
    }
  ).count;
  const rows = db
    .prepare(`SELECT * FROM ${table} ${clause} ORDER BY ${order} LIMIT ? OFFSET ?`)
    .all(...params, limit, offset);
  return { rows, total, limit, offset };
}

function pagination(result: QueryPage) {
  return {
    limit: result.limit,
    offset: result.offset,
    total: result.total,
    has_more: result.offset + result.rows.length < result.total,
  };
}

export interface ApiDependencies {
  db: Database.Database;
  config: IndexerConfig;
  ingestor: SubanIngestor;
  webhooks: WebhookStore;
  quota: QuotaStore;
  isRunning?: () => boolean;
}

export function createApi(deps: ApiDependencies): express.Express {
  const { db, config, ingestor, webhooks, quota } = deps;
  const app = express();
  app.disable('x-powered-by');
  app.use(cors());
  app.use(express.json({ limit: '1mb' }));
  app.use(httpMetricsMiddleware);

  const freshness = () => getFreshness(db, config);
  const payload = (body: Record<string, unknown>) => ({
    schema_version: SCHEMA_VERSION,
    freshness: freshness(),
    ...body,
  });

  app.get('/', (_req, res) => {
    res.json({
      name: 'Suban Data Platform',
      version: '2.0.0',
      schema_version: SCHEMA_VERSION,
      description: 'Indexed Pi ledgers, transactions, operations, accounts and contract events',
      endpoints: {
        status: '/status',
        contracts: '/contracts',
        events: '/events',
        transactions: '/transactions',
        operations: '/operations',
        accounts: '/accounts/:accountId',
        ledgers: '/ledgers',
        health: '/health',
        metrics: '/metrics',
        admin: '/admin/*',
      },
    });
  });

  app.get('/health', (_req, res) => {
    const fresh = freshness();
    const running = deps.isRunning ? deps.isRunning() : true;
    res.status(fresh.stale || !running ? 503 : 200).json({
      status: fresh.stale || !running ? 'degraded' : 'healthy',
      service: 'event-indexer',
      version: '2.0.0',
      running,
      freshness: fresh,
      timestamp: new Date().toISOString(),
    });
  });

  app.get('/metrics', metricsHandler);

  const publicApi = express.Router();
  publicApi.use(createApiKeyMiddleware(config, quota));

  publicApi.get('/status', (_req, res) => {
    const counts = {
      events: (db.prepare('SELECT COUNT(*) AS count FROM events').get() as any).count,
      transactions: (
        db.prepare('SELECT COUNT(*) AS count FROM transactions').get() as any
      ).count,
      operations: (db.prepare('SELECT COUNT(*) AS count FROM operations').get() as any)
        .count,
      accounts: (db.prepare('SELECT COUNT(*) AS count FROM accounts').get() as any).count,
      ledgers: (db.prepare('SELECT COUNT(*) AS count FROM ledgers').get() as any).count,
      contracts: (db.prepare('SELECT COUNT(*) AS count FROM contracts').get() as any)
        .count,
      webhook_subscriptions: (
        db.prepare('SELECT COUNT(*) AS count FROM webhook_subscriptions').get() as any
      ).count,
      webhook_pending: (
        db
          .prepare("SELECT COUNT(*) AS count FROM webhook_deliveries WHERE status = 'pending'")
          .get() as any
      ).count,
    };
    res.json(
      payload({
        running: deps.isRunning ? deps.isRunning() : true,
        horizon_enabled: Boolean(config.horizonUrl),
        counts,
        checkpoints: ingestor.getCheckpoints(),
      })
    );
  });

  publicApi.get('/stats', (_req, res) => {
    const count = (table: string) =>
      (db.prepare(`SELECT COUNT(*) AS count FROM ${table}`).get() as any).count;
    const byContract = db
      .prepare(
        `SELECT contract_id, contract_name, COUNT(*) AS count
         FROM events GROUP BY contract_id, contract_name
         ORDER BY count DESC`
      )
      .all();
    res.json(
      payload({
        events: count('events'),
        transactions: count('transactions'),
        operations: count('operations'),
        accounts: count('accounts'),
        ledgers: count('ledgers'),
        by_contract: byContract,
      })
    );
  });

  publicApi.get('/contracts', (_req, res) => {
    const rows = db
      .prepare('SELECT * FROM contracts ORDER BY name ASC')
      .all() as any[];
    res.json(payload({ data: rows.map(contractRow) }));
  });

  publicApi.get('/contracts/:contractId', (req, res) => {
    const row = db
      .prepare('SELECT * FROM contracts WHERE contract_id = ?')
      .get(req.params.contractId) as any;
    if (!row) {
      res.status(404).json({ error: 'Contract not found' });
      return;
    }
    res.json(payload({ data: contractRow(row) }));
  });

  publicApi.get('/contracts/:contractId/events', (req, res) => {
    const result = queryEvents(db, req, { contractId: req.params.contractId });
    res.json(
      payload({
        data: result.rows.map(eventRow),
        pagination: pagination(result),
      })
    );
  });

  publicApi.get('/events', (req, res) => {
    const result = queryEvents(db, req);
    res.json(
      payload({
        data: result.rows.map(eventRow),
        pagination: pagination(result),
      })
    );
  });

  publicApi.get('/events/contract/:contractId', (req, res) => {
    const result = queryEvents(db, req, { contractId: req.params.contractId });
    res.json(
      payload({
        events: result.rows.map(eventRow),
        pagination: pagination(result),
      })
    );
  });

  publicApi.get('/events/type/:eventType', (req, res) => {
    const result = queryEvents(db, req, { eventType: req.params.eventType });
    res.json(
      payload({
        events: result.rows.map(eventRow),
        pagination: pagination(result),
      })
    );
  });

  publicApi.get('/events/ledger/:ledger', (req, res) => {
    const ledger = parseInt(req.params.ledger, 10);
    if (!Number.isFinite(ledger)) {
      res.status(400).json({ error: 'Invalid ledger number' });
      return;
    }
    const result = queryEvents(db, req, { ledger });
    res.json(
      payload({
        events: result.rows.map(eventRow),
        pagination: pagination(result),
      })
    );
  });

  publicApi.get('/transactions', (req, res) => {
    const result = queryTransactions(db, req);
    res.json(
      payload({
        data: result.rows.map(transactionRow),
        pagination: pagination(result),
      })
    );
  });

  publicApi.get('/transactions/:hash', (req, res) => {
    const row = db
      .prepare('SELECT * FROM transactions WHERE hash = ?')
      .get(req.params.hash) as any;
    if (!row) {
      res.status(404).json({ error: 'Transaction not found in indexed data' });
      return;
    }
    res.json(payload({ data: transactionRow(row) }));
  });

  publicApi.get('/operations', (req, res) => {
    const result = queryOperations(db, req);
    res.json(
      payload({
        data: result.rows.map(operationRow),
        pagination: pagination(result),
      })
    );
  });

  publicApi.get('/accounts/:accountId', (req, res) => {
    const row = db
      .prepare('SELECT * FROM accounts WHERE account_id = ?')
      .get(req.params.accountId) as any;
    if (!row) {
      res.status(404).json({ error: 'Account not found in indexed data' });
      return;
    }
    res.json(payload({ data: accountRow(row) }));
  });

  publicApi.get('/accounts/:accountId/transactions', (req, res) => {
    const result = queryTransactions(db, req, { accountId: req.params.accountId });
    res.json(
      payload({
        data: result.rows.map(transactionRow),
        pagination: pagination(result),
      })
    );
  });

  publicApi.get('/accounts/:accountId/operations', (req, res) => {
    const result = queryOperations(db, req, { accountId: req.params.accountId });
    res.json(
      payload({
        data: result.rows.map(operationRow),
        pagination: pagination(result),
      })
    );
  });

  publicApi.get('/ledgers', (req, res) => {
    const limit = limitOf(req);
    const offset = offsetOf(req);
    const total = (
      db.prepare('SELECT COUNT(*) AS count FROM ledgers').get() as any
    ).count;
    const rows = db
      .prepare('SELECT * FROM ledgers ORDER BY sequence DESC LIMIT ? OFFSET ?')
      .all(limit, offset) as any[];
    res.json(
      payload({
        data: rows.map((row) => ({
          sequence: row.sequence,
          hash: row.hash,
          prev_hash: row.prev_hash,
          closed_at: row.closed_at,
          successful_transaction_count: row.successful_transaction_count,
          failed_transaction_count: row.failed_transaction_count,
          operation_count: row.operation_count,
          protocol_version: row.protocol_version,
        })),
        pagination: {
          limit,
          offset,
          total,
          has_more: offset + rows.length < total,
        },
      })
    );
  });

  publicApi.get('/ledgers/:sequence', (req, res) => {
    const sequence = parseInt(req.params.sequence, 10);
    if (!Number.isFinite(sequence)) {
      res.status(400).json({ error: 'Invalid ledger sequence' });
      return;
    }
    const row = db
      .prepare('SELECT * FROM ledgers WHERE sequence = ?')
      .get(sequence) as any;
    if (!row) {
      res.status(404).json({ error: 'Ledger not found in indexed data' });
      return;
    }
    res.json(
      payload({
        data: {
          ...row,
          raw: undefined,
        },
      })
    );
  });

  const adminApi = express.Router();
  adminApi.use(createAdminMiddleware(config));

  adminApi.get('/checkpoints', (_req, res) => {
    res.json({ data: ingestor.getCheckpoints() });
  });

  adminApi.post('/replay', (req, res) => {
    const stream = String(req.body?.stream || '');
    const fromLedger = parseInt(String(req.body?.from_ledger ?? ''), 10);
    if (!stream || !Number.isFinite(fromLedger) || fromLedger < 0) {
      res.status(400).json({ error: 'stream and from_ledger are required' });
      return;
    }
    ingestor.resetStream(stream, fromLedger);
    res.json({ status: 'scheduled', stream, from_ledger: fromLedger });
  });

  adminApi.get('/contracts', (_req, res) => {
    const rows = db.prepare('SELECT * FROM contracts ORDER BY name ASC').all() as any[];
    res.json({ data: rows.map(contractRow) });
  });

  adminApi.post('/contracts', (req, res) => {
    const body = req.body || {};
    if (!body.contract_id || !body.name) {
      res.status(400).json({ error: 'contract_id and name are required' });
      return;
    }
    upsertContract(
      db,
      {
        contractId: String(body.contract_id),
        name: String(body.name),
        interfaceVersion: String(body.interface_version || '1.0.0'),
        verified: body.verified === true,
      },
      String(body.network || config.networkPassphrase),
      body.metadata || {}
    );
    res.status(201).json({ data: contractRow(
      db.prepare('SELECT * FROM contracts WHERE contract_id = ?').get(body.contract_id)
    ) });
  });

  adminApi.delete('/contracts/:contractId', (req, res) => {
    const changed = db
      .prepare('DELETE FROM contracts WHERE contract_id = ?')
      .run(req.params.contractId).changes;
    res.status(changed ? 200 : 404).json({ deleted: changed > 0 });
  });

  adminApi.get('/webhooks', (_req, res) => {
    res.json({ data: webhooks.listSubscriptions() });
  });

  adminApi.post('/webhooks', (req, res) => {
    try {
      const subscription = webhooks.createSubscription({
        url: String(req.body?.url || ''),
        eventTypes: Array.isArray(req.body?.event_types) ? req.body.event_types : undefined,
        contractIds: Array.isArray(req.body?.contract_ids)
          ? req.body.contract_ids
          : undefined,
        secret: req.body?.secret ? String(req.body.secret) : undefined,
        active: req.body?.active !== false,
      });
      res.status(201).json({ data: subscription });
    } catch (error: any) {
      res.status(400).json({ error: error?.message || 'Invalid subscription' });
    }
  });

  adminApi.delete('/webhooks/:id', (req, res) => {
    const deleted = webhooks.deleteSubscription(req.params.id);
    res.status(deleted ? 200 : 404).json({ deleted });
  });

  adminApi.get('/webhooks/deliveries', (req, res) => {
    const status = String(req.query.status || '');
    const limit = limitOf(req);
    const where = status ? 'WHERE status = ?' : '';
    const params = status ? [status] : [];
    const rows = db
      .prepare(
        `SELECT id, subscription_id, event_id, status, attempts, next_attempt_at,
                last_error, created_at, delivered_at
         FROM webhook_deliveries ${where}
         ORDER BY created_at DESC LIMIT ?`
      )
      .all(...params, limit);
    res.json({ data: rows });
  });

  adminApi.get('/api-keys/usage', (_req, res) => {
    res.json({
      require_api_key: config.requireApiKey,
      keys: quota.usage(config.apiKeys),
    });
  });

  app.use('/admin', adminApi);

  app.use(publicApi);

  app.use((_req, res) => {
    res.status(404).json({ error: 'Not Found' });
  });

  app.use((err: any, _req: Request, res: Response, _next: NextFunction) => {
    console.error('API error:', err?.message || err);
    res.status(500).json({ error: 'Internal Server Error' });
  });

  return app;
}

function queryEvents(
  db: Database.Database,
  req: Request,
  overrides: { contractId?: string; eventType?: string; ledger?: number } = {}
): QueryPage {
  const where: string[] = [];
  const params: unknown[] = [];
  const contractId = overrides.contractId || String(req.query.contract || req.query.contract_id || '');
  if (contractId) {
    where.push('contract_id = ?');
    params.push(contractId);
  }
  const eventType = overrides.eventType || String(req.query.type || req.query.event_type || '');
  if (eventType) {
    where.push('event_type = ?');
    params.push(eventType);
  }
  if (overrides.ledger !== undefined) {
    where.push('ledger = ?');
    params.push(overrides.ledger);
  }
  const ledgerFrom = parseInt(String(req.query.ledger_from || ''), 10);
  if (Number.isFinite(ledgerFrom)) {
    where.push('ledger >= ?');
    params.push(ledgerFrom);
  }
  const ledgerTo = parseInt(String(req.query.ledger_to || ''), 10);
  if (Number.isFinite(ledgerTo)) {
    where.push('ledger <= ?');
    params.push(ledgerTo);
  }
  const from = epochOf(req.query.from);
  if (from !== null) {
    where.push('timestamp >= ?');
    params.push(from);
  }
  const to = epochOf(req.query.to);
  if (to !== null) {
    where.push('timestamp <= ?');
    params.push(to);
  }
  return page(db, 'events', where, params, limitOf(req), offsetOf(req), 'ledger DESC, id DESC');
}

function queryTransactions(
  db: Database.Database,
  req: Request,
  overrides: { accountId?: string } = {}
): QueryPage {
  const where: string[] = [];
  const params: unknown[] = [];
  const accountId = overrides.accountId || String(req.query.account || '');
  const join = accountId
    ? 'transactions t JOIN transaction_accounts ta ON ta.tx_hash = t.hash'
    : 'transactions t';
  if (accountId) {
    where.push('ta.account_id = ?');
    params.push(accountId);
  }
  const ledger = parseInt(String(req.query.ledger || ''), 10);
  if (Number.isFinite(ledger)) {
    where.push('t.ledger = ?');
    params.push(ledger);
  }
  if (req.query.successful !== undefined) {
    where.push('t.successful = ?');
    params.push(String(req.query.successful) === 'true' ? 1 : 0);
  }
  const from = isoOf(req.query.from);
  if (from) {
    where.push('t.created_at >= ?');
    params.push(from);
  }
  const to = isoOf(req.query.to);
  if (to) {
    where.push('t.created_at <= ?');
    params.push(to);
  }
  const clause = where.length ? `WHERE ${where.join(' AND ')}` : '';
  const limit = limitOf(req);
  const offset = offsetOf(req);
  const total = (
    db
      .prepare(`SELECT COUNT(*) AS count FROM ${join} ${clause}`)
      .get(...params) as { count: number }
  ).count;
  const rows = db
    .prepare(
      `SELECT t.* FROM ${join} ${clause}
       ORDER BY t.ledger DESC, t.hash DESC LIMIT ? OFFSET ?`
    )
    .all(...params, limit, offset);
  return { rows, total, limit, offset };
}

function queryOperations(
  db: Database.Database,
  req: Request,
  overrides: { accountId?: string } = {}
): QueryPage {
  const where: string[] = [];
  const params: unknown[] = [];
  const accountId = overrides.accountId || String(req.query.account || '');
  const join = accountId
    ? 'operations o JOIN operation_accounts oa ON oa.operation_id = o.id'
    : 'operations o';
  if (accountId) {
    where.push('oa.account_id = ?');
    params.push(accountId);
  }
  const txHash = String(req.query.tx_hash || '');
  if (txHash) {
    where.push('o.tx_hash = ?');
    params.push(txHash);
  }
  const ledger = parseInt(String(req.query.ledger || ''), 10);
  if (Number.isFinite(ledger)) {
    where.push('o.ledger = ?');
    params.push(ledger);
  }
  const type = String(req.query.type || '');
  if (type) {
    where.push('o.type = ?');
    params.push(type);
  }
  const from = isoOf(req.query.from);
  if (from) {
    where.push('o.created_at >= ?');
    params.push(from);
  }
  const to = isoOf(req.query.to);
  if (to) {
    where.push('o.created_at <= ?');
    params.push(to);
  }
  const clause = where.length ? `WHERE ${where.join(' AND ')}` : '';
  const limit = limitOf(req);
  const offset = offsetOf(req);
  const total = (
    db
      .prepare(`SELECT COUNT(*) AS count FROM ${join} ${clause}`)
      .get(...params) as { count: number }
  ).count;
  const rows = db
    .prepare(
      `SELECT o.* FROM ${join} ${clause}
       ORDER BY o.ledger DESC, o.id DESC LIMIT ? OFFSET ?`
    )
    .all(...params, limit, offset);
  return { rows, total, limit, offset };
}
