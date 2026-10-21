/**
 * Runtime configuration for the Suban data platform.
 *
 * The indexer is intentionally configuration-driven: contracts, Horizon and
 * public API keys are deployment inputs, not code changes.
 */
import crypto from 'crypto';
import path from 'path';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));

export interface ContractConfig {
  name: string;
  contractId: string;
  interfaceVersion: string;
  verified: boolean;
}

export interface ApiKeyConfig {
  id: string;
  name: string;
  hash: string;
  perMinute: number;
  perDay: number;
}

export interface IndexerConfig {
  rpcUrl: string;
  networkPassphrase: string;
  horizonUrl: string;
  dbPath: string;
  pollIntervalMs: number;
  apiPort: number;
  contracts: ContractConfig[];
  eventBootstrapLedgers: number;
  horizonBootstrapLedgers: number;
  maxLedgersPerPoll: number;
  accountFetchLimit: number;
  fetchAccountSnapshots: boolean;
  staleAfterSeconds: number;
  maxLagLedgers: number;
  requireApiKey: boolean;
  apiKeys: ApiKeyConfig[];
  adminApiKey: string;
  webhook: {
    dispatchIntervalMs: number;
    maxAttempts: number;
    baseBackoffMs: number;
    maxBackoffMs: number;
    timeoutMs: number;
  };
}

export const SCHEMA_VERSION = '2.0';

function bool(value: string | undefined, fallback = false): boolean {
  if (value === undefined) return fallback;
  return value.toLowerCase() === 'true' || value === '1';
}

function int(value: string | undefined, fallback: number): number {
  const parsed = parseInt(value || '', 10);
  return Number.isFinite(parsed) ? parsed : fallback;
}

function knownContracts(env: NodeJS.ProcessEnv): ContractConfig[] {
  const raw = env.INDEXER_CONTRACTS_JSON;
  if (raw) {
    const parsed = JSON.parse(raw) as Array<Partial<ContractConfig>>;
    return parsed
      .filter((entry) => entry.contractId)
      .map((entry) => ({
        name: entry.name || entry.contractId!.slice(0, 12),
        contractId: entry.contractId!,
        interfaceVersion: entry.interfaceVersion || '1.0.0',
        verified: entry.verified === true,
      }));
  }

  const verified = new Set(
    (env.INDEXER_VERIFIED_CONTRACTS || '')
      .split(',')
      .map((entry) => entry.trim())
      .filter(Boolean)
  );

  return [
    { name: 'pool-factory', env: 'POOL_FACTORY_CONTRACT_ID' },
    { name: 'cpmm-pool', env: 'CPMM_POOL_CONTRACT_ID' },
    { name: 'stableswap', env: 'STABLESWAP_CONTRACT_ID' },
    { name: 'lending-pool', env: 'LENDING_POOL_CONTRACT_ID' },
    { name: 'backstop', env: 'BACKSTOP_CONTRACT_ID' },
    { name: 'escrow', env: 'ESCROW_CONTRACT_ID' },
    { name: 'oracle', env: 'ORACLE_CONTRACT_ID' },
    { name: 'bridge-multisig', env: 'BRIDGE_MULTISIG_CONTRACT_ID' },
    { name: 'event-registry', env: 'EVENT_REGISTRY_CONTRACT_ID' },
    { name: 'node-staking', env: 'NODE_STAKING_CONTRACT_ID' },
    { name: 'identity', env: 'IDENTITY_CONTRACT_ID' },
  ]
    .filter((entry) => env[entry.env])
    .map((entry) => ({
      name: entry.name,
      contractId: env[entry.env]!,
      interfaceVersion: '1.0.0',
      verified: verified.has(entry.name),
    }));
}

/**
 * Parse public API keys.
 *
 * Accepted forms:
 * - JSON: [{"key":"...","name":"demo","perMinute":60,"perDay":10000}]
 * - Compact: key:name:perMinute:perDay,key2:name2:...
 *
 * Only a SHA-256 hash is retained. The raw key is never logged or stored.
 */
export function parseApiKeys(raw: string | undefined): ApiKeyConfig[] {
  if (!raw?.trim()) return [];

  const keys: ApiKeyConfig[] = [];
  const seen = new Set<string>();

  const add = (key: string, name: string, perMinute: number, perDay: number) => {
    if (!key) return;
    const hash = crypto.createHash('sha256').update(key).digest('hex');
    if (seen.has(hash)) return;
    seen.add(hash);
    keys.push({
      id: `key_${hash.slice(0, 12)}`,
      name: name || 'unnamed',
      hash,
      perMinute: perMinute > 0 ? perMinute : 60,
      perDay: perDay > 0 ? perDay : 10_000,
    });
  };

  if (raw.trim().startsWith('[')) {
    const parsed = JSON.parse(raw) as Array<{
      key?: string;
      name?: string;
      perMinute?: number;
      perDay?: number;
      per_minute?: number;
      per_day?: number;
    }>;
    for (const entry of parsed) {
      add(
        entry.key || '',
        entry.name || 'unnamed',
        entry.perMinute ?? entry.per_minute ?? 60,
        entry.perDay ?? entry.per_day ?? 10_000
      );
    }
    return keys;
  }

  for (const token of raw.split(',')) {
    const [key, name, perMinute, perDay] = token.trim().split(':');
    add(key, name || 'unnamed', int(perMinute, 60), int(perDay, 10_000));
  }
  return keys;
}

export function loadConfig(env: NodeJS.ProcessEnv = process.env): IndexerConfig {
  return {
    rpcUrl: env.PI_SOROBAN_RPC_URL || 'https://rpc.testnet.minepi.com',
    networkPassphrase: env.PI_NETWORK_PASSPHRASE || 'Pi Testnet',
    horizonUrl: env.HORIZON_URL || '',
    dbPath: env.INDEXER_DB_PATH || path.join(__dirname, '..', 'events.db'),
    pollIntervalMs: int(env.INDEXER_POLL_MS, 10_000),
    apiPort: int(env.INDEXER_API_PORT, 3002),
    contracts: knownContracts(env),
    eventBootstrapLedgers: int(env.INDEXER_EVENT_BOOTSTRAP_LEDGERS, 1_000),
    horizonBootstrapLedgers: int(env.INDEXER_HORIZON_BOOTSTRAP_LEDGERS, 0),
    maxLedgersPerPoll: int(env.INDEXER_MAX_LEDGERS_PER_POLL, 25),
    accountFetchLimit: int(env.INDEXER_ACCOUNT_FETCH_LIMIT, 10),
    fetchAccountSnapshots: bool(env.INDEXER_FETCH_ACCOUNT_SNAPSHOTS, true),
    staleAfterSeconds: int(env.INDEXER_STALE_AFTER_SECONDS, 120),
    maxLagLedgers: int(env.INDEXER_MAX_LAG_LEDGERS, 10),
    requireApiKey: bool(env.INDEXER_REQUIRE_API_KEY, false),
    apiKeys: parseApiKeys(env.INDEXER_API_KEYS),
    adminApiKey: env.INDEXER_ADMIN_API_KEY || '',
    webhook: {
      dispatchIntervalMs: int(env.INDEXER_WEBHOOK_INTERVAL_MS, 5_000),
      maxAttempts: int(env.INDEXER_WEBHOOK_MAX_ATTEMPTS, 8),
      baseBackoffMs: int(env.INDEXER_WEBHOOK_BASE_BACKOFF_MS, 1_000),
      maxBackoffMs: int(env.INDEXER_WEBHOOK_MAX_BACKOFF_MS, 300_000),
      timeoutMs: int(env.INDEXER_WEBHOOK_TIMEOUT_MS, 5_000),
    },
  };
}
