/**
 * Suban Data Platform process entrypoint.
 */
import { rpc } from '@stellar/stellar-sdk';
import { createApi } from './api.js';
import { QuotaStore } from './apiKeys.js';
import { loadConfig, IndexerConfig, SCHEMA_VERSION } from './config.js';
import { openDatabase } from './db.js';
import { SubanIngestor } from './ingest.js';
import { WebhookDispatcher, WebhookStore } from './webhooks.js';

export interface RunningPlatform {
  stop: () => Promise<void>;
}

export function startDataPlatform(config: IndexerConfig = loadConfig()): RunningPlatform {
  const db = openDatabase(config.dbPath);
  const server = new rpc.Server(config.rpcUrl, { allowHttp: true });
  const webhooks = new WebhookStore(db);
  const ingestor = new SubanIngestor(db, config, server, undefined, webhooks);
  const quota = new QuotaStore(db);
  quota.prune();

  const dispatcher = new WebhookDispatcher(db, config);
  dispatcher.start();

  const app = createApi({
    db,
    config,
    ingestor,
    webhooks,
    quota,
    isRunning: () => ingestor.isRunning(),
  });

  const httpServer = app.listen(config.apiPort, () => {
    console.log(`Data platform API listening on port ${config.apiPort}`);
    console.log(`Schema version ${SCHEMA_VERSION}`);
    console.log(`RPC: ${config.rpcUrl}`);
    console.log(`Horizon: ${config.horizonUrl || 'disabled'}`);
    console.log(
      `API keys: ${config.requireApiKey ? 'required' : 'optional'}; ` +
        `${config.apiKeys.length} configured`
    );
  });

  const loop = ingestor.start().catch((error) => {
    console.error('Data platform loop crashed:', error);
  });

  return {
    stop: async () => {
      ingestor.stop();
      dispatcher.stop();
      await loop.catch(() => undefined);
      await new Promise<void>((resolve) => httpServer.close(() => resolve()));
      db.close();
    },
  };
}

const isMain = process.argv[1] && /index\.(js|ts)$/.test(process.argv[1]);
if (isMain) {
  const platform = startDataPlatform();
  const shutdown = async () => {
    await platform.stop();
    process.exit(0);
  };
  process.on('SIGINT', () => void shutdown());
  process.on('SIGTERM', () => void shutdown());
}
