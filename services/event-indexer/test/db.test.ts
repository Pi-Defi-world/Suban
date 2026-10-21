import Database from 'better-sqlite3';
import { describe, expect, it } from 'vitest';
import { migrate, openDatabase, SCHEMA_VERSION } from '../src/db.js';

describe('schema migrations', () => {
  it('creates the Phase 2 schema and records the version', () => {
    const db = openDatabase(':memory:');
    const tables = db
      .prepare("SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name")
      .all() as Array<{ name: string }>;
    const names = tables.map((table) => table.name);
    expect(names).toEqual(
      expect.arrayContaining([
        'accounts',
        'api_key_usage',
        'checkpoints',
        'contracts',
        'events',
        'indexer_meta',
        'ledgers',
        'operations',
        'schema_migrations',
        'transactions',
        'webhook_deliveries',
        'webhook_subscriptions',
      ])
    );
    expect(
      (db.prepare("SELECT value FROM indexer_meta WHERE key = 'schema_version'").get() as any)
        .value
    ).toBe(SCHEMA_VERSION);
    db.close();
  });

  it('is idempotent', () => {
    const db = openDatabase(':memory:');
    migrate(db);
    migrate(db);
    const row = db.prepare('SELECT COUNT(*) AS count FROM schema_migrations').get() as any;
    expect(row.count).toBe(1);
    db.close();
  });

  it('upgrades an existing Phase 1 events database in place', () => {
    const db = new Database(':memory:');
    db.exec(`
      CREATE TABLE events (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        contract_id TEXT NOT NULL,
        contract_name TEXT NOT NULL,
        event_type TEXT NOT NULL,
        tx_hash TEXT NOT NULL,
        ledger INTEGER NOT NULL,
        timestamp INTEGER NOT NULL,
        data TEXT,
        created_at DATETIME DEFAULT CURRENT_TIMESTAMP
      );
      CREATE TABLE index_state (
        contract_id TEXT PRIMARY KEY,
        last_ledger INTEGER NOT NULL DEFAULT 0,
        updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
      );
      INSERT INTO events (contract_id, contract_name, event_type, tx_hash, ledger, timestamp, data)
      VALUES ('C1', 'pool', 'swap', 'tx1', 100, 1, '{}');
    `);
    migrate(db);
    const columns = db.prepare('PRAGMA table_info(events)').all() as Array<{ name: string }>;
    expect(columns.map((column) => column.name)).toEqual(
      expect.arrayContaining(['event_id', 'ledger_closed_at', 'raw'])
    );
    expect((db.prepare('SELECT COUNT(*) AS count FROM events').get() as any).count).toBe(1);
    db.close();
  });
});
