/**
 * Prometheus metrics for the Suban data platform.
 */
import client from 'prom-client';
import { NextFunction, Request, Response } from 'express';

export const registry = new client.Registry();
client.collectDefaultMetrics({ register: registry, prefix: 'suban_indexer_' });

export const pollsTotal = new client.Counter({
  name: 'suban_indexer_polls_total',
  help: 'Indexer poll cycles',
  registers: [registry],
});

export const eventsIndexed = new client.Counter({
  name: 'suban_indexer_events_indexed_total',
  help: 'Events written by contract and event type',
  labelNames: ['contract', 'event_type'],
  registers: [registry],
});

export const ledgersIndexed = new client.Counter({
  name: 'suban_indexer_ledgers_indexed_total',
  help: 'Horizon ledgers written',
  registers: [registry],
});

export const transactionsIndexed = new client.Counter({
  name: 'suban_indexer_transactions_indexed_total',
  help: 'Horizon transactions written',
  registers: [registry],
});

export const operationsIndexed = new client.Counter({
  name: 'suban_indexer_operations_indexed_total',
  help: 'Horizon operations written',
  registers: [registry],
});

export const accountsObserved = new client.Counter({
  name: 'suban_indexer_accounts_observed_total',
  help: 'Account identifiers observed in indexed activity',
  registers: [registry],
});

export const replayResets = new client.Counter({
  name: 'suban_indexer_replay_resets_total',
  help: 'Checkpoint resets requested for replay',
  labelNames: ['stream'],
  registers: [registry],
});

export const reorgDetected = new client.Counter({
  name: 'suban_indexer_reorg_detected_total',
  help: 'Parent-hash mismatches detected while ingesting a stream',
  labelNames: ['stream'],
  registers: [registry],
});

export const operationCompleteness = new client.Gauge({
  name: 'suban_indexer_operation_completeness_ratio',
  help: 'Stored operations divided by the operation count reported by the latest ledger',
  registers: [registry],
});

export const fetchErrors = new client.Counter({
  name: 'suban_indexer_fetch_errors_total',
  help: 'Upstream fetch errors by source',
  labelNames: ['source'],
  registers: [registry],
});

export const indexErrors = new client.Counter({
  name: 'suban_indexer_errors_total',
  help: 'Indexer errors by stage',
  labelNames: ['stage'],
  registers: [registry],
});

export const lastLedger = new client.Gauge({
  name: 'suban_indexer_last_ledger',
  help: 'Last ledger stored per monitored contract',
  labelNames: ['contract'],
  registers: [registry],
});

export const latestObservedLedger = new client.Gauge({
  name: 'suban_indexer_latest_observed_ledger',
  help: 'Latest ledger observed from Horizon',
  registers: [registry],
});

export const ingestLagLedgers = new client.Gauge({
  name: 'suban_indexer_ingest_lag_ledgers',
  help: 'Ledger distance between Horizon and the indexed state',
  registers: [registry],
});

export const ingestLagSeconds = new client.Gauge({
  name: 'suban_indexer_ingest_lag_seconds',
  help: 'Seconds since the last successful upstream poll',
  registers: [registry],
});

export const storedEvents = new client.Gauge({
  name: 'suban_indexer_stored_events',
  help: 'Total events currently stored in SQLite',
  registers: [registry],
});

export const storedTransactions = new client.Gauge({
  name: 'suban_indexer_stored_transactions',
  help: 'Total transactions currently stored in SQLite',
  registers: [registry],
});

export const storedOperations = new client.Gauge({
  name: 'suban_indexer_stored_operations',
  help: 'Total operations currently stored in SQLite',
  registers: [registry],
});

export const storedAccounts = new client.Gauge({
  name: 'suban_indexer_stored_accounts',
  help: 'Total accounts currently stored in SQLite',
  registers: [registry],
});

export const running = new client.Gauge({
  name: 'suban_indexer_running',
  help: '1 when the indexer poll loop is running',
  registers: [registry],
});

export const apiKeyRequests = new client.Counter({
  name: 'suban_indexer_api_key_requests_total',
  help: 'Public API requests by API key and outcome',
  labelNames: ['key', 'result'],
  registers: [registry],
});

export const webhookDeliveries = new client.Counter({
  name: 'suban_indexer_webhook_deliveries_total',
  help: 'Webhook delivery outcomes',
  labelNames: ['result'],
  registers: [registry],
});

export const webhookDuration = new client.Histogram({
  name: 'suban_indexer_webhook_duration_seconds',
  help: 'Webhook delivery duration',
  labelNames: ['result'],
  buckets: [0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1, 2.5, 5],
  registers: [registry],
});

export const webhookPending = new client.Gauge({
  name: 'suban_indexer_webhook_pending',
  help: 'Webhook deliveries waiting for retry or first attempt',
  registers: [registry],
});

export const httpRequests = new client.Counter({
  name: 'suban_indexer_http_requests_total',
  help: 'HTTP requests to the indexer API',
  labelNames: ['method', 'route', 'status'],
  registers: [registry],
});

function normalizeRoute(path: string): string {
  const exact = [
    [/^\/events\/contract\/[^/]+$/, '/events/contract/:contractId'],
    [/^\/events\/type\/[^/]+$/, '/events/type/:eventType'],
    [/^\/events\/ledger\/[^/]+$/, '/events/ledger/:ledger'],
    [/^\/transactions\/[^/]+$/, '/transactions/:hash'],
    [/^\/accounts\/[^/]+\/transactions$/, '/accounts/:accountId/transactions'],
    [/^\/accounts\/[^/]+\/operations$/, '/accounts/:accountId/operations'],
    [/^\/accounts\/[^/]+$/, '/accounts/:accountId'],
    [/^\/ledgers\/[^/]+$/, '/ledgers/:sequence'],
    [/^\/contracts\/[^/]+\/events$/, '/contracts/:contractId/events'],
    [/^\/contracts\/[^/]+$/, '/contracts/:contractId'],
    [/^\/admin\/webhooks\/[^/]+$/, '/admin/webhooks/:id'],
    [/^\/admin\/contracts\/[^/]+$/, '/admin/contracts/:contractId'],
  ] as const;

  for (const [pattern, replacement] of exact) {
    if (pattern.test(path)) return replacement;
  }
  return path;
}

export function httpMetricsMiddleware(
  req: Request,
  res: Response,
  next: NextFunction
): void {
  if (req.path === '/metrics') {
    next();
    return;
  }
  const route = normalizeRoute(req.path);
  res.on('finish', () => {
    httpRequests.inc({
      method: req.method,
      route,
      status: String(res.statusCode),
    });
  });
  next();
}

export async function metricsHandler(
  _req: Request,
  res: Response
): Promise<void> {
  res.setHeader('Content-Type', registry.contentType);
  res.end(await registry.metrics());
}
