/**
 * Prometheus metrics for the Suban event indexer.
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

export const fetchErrors = new client.Counter({
  name: 'suban_indexer_fetch_errors_total',
  help: 'RPC fetch errors by contract',
  labelNames: ['contract'],
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

export const storedEvents = new client.Gauge({
  name: 'suban_indexer_stored_events',
  help: 'Total events currently stored in SQLite',
  registers: [registry],
});

export const running = new client.Gauge({
  name: 'suban_indexer_running',
  help: '1 when the indexer poll loop is running',
  registers: [registry],
});

export const httpRequests = new client.Counter({
  name: 'suban_indexer_http_requests_total',
  help: 'HTTP requests to the indexer API',
  labelNames: ['method', 'route', 'status'],
  registers: [registry],
});

function normalizeRoute(path: string): string {
  return path
    .replace(/^\/events\/contract\/[^/]+$/, '/events/contract/:contractId')
    .replace(/^\/events\/type\/[^/]+$/, '/events/type/:eventType')
    .replace(/^\/events\/ledger\/[^/]+$/, '/events/ledger/:ledger');
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
