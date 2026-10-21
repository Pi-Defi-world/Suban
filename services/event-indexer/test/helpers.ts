import Database from 'better-sqlite3';
import { IndexerConfig, loadConfig } from '../src/config.js';
import { openDatabase } from '../src/db.js';

export function testConfig(overrides: Partial<IndexerConfig> = {}): IndexerConfig {
  const base = loadConfig({} as NodeJS.ProcessEnv);
  return {
    ...base,
    dbPath: ':memory:',
    contracts: [],
    horizonUrl: '',
    apiKeys: [],
    requireApiKey: false,
    adminApiKey: '',
    ...overrides,
    webhook: {
      ...base.webhook,
      dispatchIntervalMs: 60_000,
      ...(overrides.webhook || {}),
    },
  };
}

export function memoryDb(): Database.Database {
  return openDatabase(':memory:');
}

export function fakeRpc(events: any[] = [], latest = 101): any {
  return {
    getLatestLedger: async () => ({ sequence: latest }),
    getEvents: async () => ({ events }),
  };
}

export function fakeHorizon(input: {
  ledger?: any;
  transactions?: any[];
  operations?: any[];
  account?: any;
} = {}): any {
  const ledger = input.ledger || {
    sequence: 101,
    hash: 'ledger-hash-101',
    prev_hash: 'ledger-hash-100',
    closed_at: '2026-10-21T00:00:00Z',
    successful_transaction_count: 1,
    failed_transaction_count: 0,
    operation_count: 1,
    protocol_version: 28,
  };
  return {
    latestLedger: async () => ledger,
    ledgersFrom: async (sequence: number) => (sequence < ledger.sequence ? [ledger] : []),
    transactionsForLedger: async () => input.transactions || [],
    operationsForTransaction: async () => input.operations || [],
    operationsForLedger: async () => input.operations || [],
    account: async (id: string) =>
      input.account || {
        account_id: id,
        sequence: '100',
        sequence_ledger: 101,
        last_modified_ledger: 101,
        balances: [{ asset_type: 'native', balance: '10.0' }],
        signers: [],
        thresholds: {},
        flags: {},
      },
  };
}
