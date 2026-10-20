/**
 * Prometheus metrics for the bridge relayer.
 *
 * The relayer is a long-running worker rather than an HTTP API, so metrics are
 * exposed on a dedicated port (default 9464) with a small health endpoint.
 */
import client from 'prom-client';
import http from 'http';
import { logger } from './logger';

export const registry = new client.Registry();
client.collectDefaultMetrics({ register: registry, prefix: 'suban_relayer_' });

export const eventsSeen = new client.Counter({
  name: 'suban_relayer_events_seen_total',
  help: 'Burn events observed by source chain',
  labelNames: ['chain'],
  registers: [registry],
});

export const transfers = new client.Counter({
  name: 'suban_relayer_transfers_total',
  help: 'Cross-chain transfer outcomes by direction and status',
  labelNames: ['direction', 'status'],
  registers: [registry],
});

export const lastEventTimestamp = new client.Gauge({
  name: 'suban_relayer_last_event_timestamp_seconds',
  help: 'Unix timestamp of the last burn event observed per chain',
  labelNames: ['chain'],
  registers: [registry],
});

export const relayerRunning = new client.Gauge({
  name: 'suban_relayer_running',
  help: '1 while the relayer is running',
  registers: [registry],
});

export const processedTotal = new client.Gauge({
  name: 'suban_relayer_processed_total',
  help: 'Events tracked in the processor (current process)',
  registers: [registry],
});

export const processedCompleted = new client.Gauge({
  name: 'suban_relayer_processed_completed',
  help: 'Events completed successfully',
  registers: [registry],
});

export const processedFailed = new client.Gauge({
  name: 'suban_relayer_processed_failed',
  help: 'Events that failed processing',
  registers: [registry],
});

export const processedPending = new client.Gauge({
  name: 'suban_relayer_processed_pending',
  help: 'Events currently processing',
  registers: [registry],
});

export function updateProcessorStats(stats: {
  total: number;
  completed: number;
  failed: number;
  pending: number;
}): void {
  processedTotal.set(stats.total);
  processedCompleted.set(stats.completed);
  processedFailed.set(stats.failed);
  processedPending.set(stats.pending);
}

export function startMetricsServer(port: number): http.Server {
  const server = http.createServer(async (req, res) => {
    if (req.url === '/metrics') {
      res.setHeader('Content-Type', registry.contentType);
      res.end(await registry.metrics());
      return;
    }

    if (req.url === '/health') {
      res.setHeader('Content-Type', 'application/json');
      res.end(
        JSON.stringify({
          status: 'healthy',
          service: 'bridge-relayer',
          timestamp: new Date().toISOString(),
        })
      );
      return;
    }

    res.statusCode = 404;
    res.end('Not Found');
  });

  server.listen(port, () => {
    logger.info(`Bridge relayer metrics listening on port ${port}`);
  });

  return server;
}
