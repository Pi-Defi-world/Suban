/**
 * SQLite schema and migrations for the data platform.
 *
 * The schema is additive: an existing Phase 1 events database is upgraded in
 * place. Queries never depend on live RPC or Horizon.
 */
import Database from 'better-sqlite3';

export const SCHEMA_VERSION = '2.0';
export const MIGRATION_VERSION = '2.0.0';

function hasTable(db: Database.Database, table: string): boolean {
  return Boolean(
    db.prepare("SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?").get(table)
  );
}

function hasColumn(db: Database.Database, table: string, column: string): boolean {
  if (!hasTable(db, table)) return false;
  const rows = db.prepare(`PRAGMA table_info(${table})`).all() as Array<{ name: string }>;
  return rows.some((row) => row.name === column);
}

function addColumnIfMissing(
  db: Database.Database,
  table: string,
  column: string,
  definition: string
): void {
  if (!hasColumn(db, table, column)) {
    db.exec(`ALTER TABLE ${table} ADD COLUMN ${column} ${definition}`);
  }
}

const migration = (db: Database.Database): void => {
  db.exec(`
    CREATE TABLE IF NOT EXISTS events (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      event_id TEXT,
      contract_id TEXT NOT NULL,
      contract_name TEXT NOT NULL,
      event_type TEXT NOT NULL,
      tx_hash TEXT NOT NULL,
      ledger INTEGER NOT NULL,
      timestamp INTEGER NOT NULL,
      data TEXT,
      created_at DATETIME DEFAULT CURRENT_TIMESTAMP
    );
  `);

  addColumnIfMissing(db, 'events', 'event_id', 'TEXT');
  addColumnIfMissing(db, 'events', 'ledger_closed_at', 'TEXT');
  addColumnIfMissing(db, 'events', 'raw', 'TEXT');

  db.exec(`
    CREATE UNIQUE INDEX IF NOT EXISTS idx_events_event_id ON events(event_id);
    CREATE INDEX IF NOT EXISTS idx_events_contract ON events(contract_id);
    CREATE INDEX IF NOT EXISTS idx_events_type ON events(event_type);
    CREATE INDEX IF NOT EXISTS idx_events_ledger ON events(ledger);
    CREATE INDEX IF NOT EXISTS idx_events_tx ON events(tx_hash);
    CREATE INDEX IF NOT EXISTS idx_events_time ON events(timestamp);

    CREATE TABLE IF NOT EXISTS index_state (
      contract_id TEXT PRIMARY KEY,
      last_ledger INTEGER NOT NULL DEFAULT 0,
      updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
    );

    CREATE TABLE IF NOT EXISTS ledgers (
      sequence INTEGER PRIMARY KEY,
      hash TEXT NOT NULL,
      prev_hash TEXT,
      closed_at TEXT NOT NULL,
      successful_transaction_count INTEGER NOT NULL DEFAULT 0,
      failed_transaction_count INTEGER NOT NULL DEFAULT 0,
      operation_count INTEGER NOT NULL DEFAULT 0,
      tx_set_operation_count INTEGER NOT NULL DEFAULT 0,
      total_coins TEXT,
      fee_pool TEXT,
      base_fee_in_stroops INTEGER,
      base_reserve_in_stroops INTEGER,
      max_tx_set_size INTEGER,
      protocol_version INTEGER,
      header_xdr TEXT,
      raw TEXT,
      indexed_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
    );

    CREATE TABLE IF NOT EXISTS transactions (
      hash TEXT PRIMARY KEY,
      ledger INTEGER NOT NULL,
      successful INTEGER NOT NULL DEFAULT 0,
      source_account TEXT,
      source_account_sequence TEXT,
      fee_account TEXT,
      fee_charged TEXT,
      max_fee TEXT,
      operation_count INTEGER NOT NULL DEFAULT 0,
      created_at TEXT NOT NULL,
      memo_type TEXT,
      memo TEXT,
      envelope_xdr TEXT,
      result_xdr TEXT,
      result_meta_xdr TEXT,
      raw TEXT,
      indexed_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
    );
    CREATE INDEX IF NOT EXISTS idx_transactions_ledger ON transactions(ledger);
    CREATE INDEX IF NOT EXISTS idx_transactions_source ON transactions(source_account);
    CREATE INDEX IF NOT EXISTS idx_transactions_time ON transactions(created_at);

    CREATE TABLE IF NOT EXISTS operations (
      id TEXT PRIMARY KEY,
      paging_token TEXT,
      tx_hash TEXT NOT NULL,
      ledger INTEGER NOT NULL,
      source_account TEXT,
      type TEXT NOT NULL,
      type_i INTEGER,
      created_at TEXT NOT NULL,
      transaction_successful INTEGER NOT NULL DEFAULT 0,
      details TEXT,
      raw TEXT,
      indexed_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
    );
    CREATE INDEX IF NOT EXISTS idx_operations_tx ON operations(tx_hash);
    CREATE INDEX IF NOT EXISTS idx_operations_ledger ON operations(ledger);
    CREATE INDEX IF NOT EXISTS idx_operations_source ON operations(source_account);
    CREATE INDEX IF NOT EXISTS idx_operations_type ON operations(type);
    CREATE INDEX IF NOT EXISTS idx_operations_time ON operations(created_at);

    CREATE TABLE IF NOT EXISTS transaction_accounts (
      tx_hash TEXT NOT NULL,
      account_id TEXT NOT NULL,
      PRIMARY KEY (tx_hash, account_id)
    );
    CREATE INDEX IF NOT EXISTS idx_transaction_accounts_account
      ON transaction_accounts(account_id);

    CREATE TABLE IF NOT EXISTS operation_accounts (
      operation_id TEXT NOT NULL,
      account_id TEXT NOT NULL,
      PRIMARY KEY (operation_id, account_id)
    );
    CREATE INDEX IF NOT EXISTS idx_operation_accounts_account
      ON operation_accounts(account_id);

    CREATE TABLE IF NOT EXISTS accounts (
      account_id TEXT PRIMARY KEY,
      sequence TEXT,
      sequence_ledger INTEGER,
      last_modified_ledger INTEGER,
      last_seen_ledger INTEGER,
      balances TEXT,
      signers TEXT,
      thresholds TEXT,
      flags TEXT,
      home_domain TEXT,
      subentry_count INTEGER,
      num_sponsoring INTEGER,
      num_sponsored INTEGER,
      raw TEXT,
      created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
      updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
    );
    CREATE INDEX IF NOT EXISTS idx_accounts_last_seen ON accounts(last_seen_ledger);
    CREATE INDEX IF NOT EXISTS idx_accounts_modified ON accounts(last_modified_ledger);

    CREATE TABLE IF NOT EXISTS contracts (
      contract_id TEXT PRIMARY KEY,
      network TEXT NOT NULL,
      name TEXT NOT NULL,
      interface_version TEXT NOT NULL,
      verified INTEGER NOT NULL DEFAULT 0,
      metadata TEXT,
      created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
      updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
    );
    CREATE INDEX IF NOT EXISTS idx_contracts_name ON contracts(name);

    CREATE TABLE IF NOT EXISTS checkpoints (
      stream TEXT PRIMARY KEY,
      cursor TEXT,
      last_ledger INTEGER NOT NULL DEFAULT 0,
      updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
    );

    CREATE TABLE IF NOT EXISTS indexer_meta (
      key TEXT PRIMARY KEY,
      value TEXT NOT NULL,
      updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
    );

    CREATE TABLE IF NOT EXISTS api_key_usage (
      key_id TEXT NOT NULL,
      window_kind TEXT NOT NULL,
      window_start INTEGER NOT NULL,
      count INTEGER NOT NULL DEFAULT 0,
      PRIMARY KEY (key_id, window_kind, window_start)
    );
    CREATE INDEX IF NOT EXISTS idx_api_key_usage_window
      ON api_key_usage(window_kind, window_start);

    CREATE TABLE IF NOT EXISTS webhook_subscriptions (
      id TEXT PRIMARY KEY,
      url TEXT NOT NULL,
      secret TEXT NOT NULL,
      event_types TEXT NOT NULL DEFAULT '[]',
      contract_ids TEXT NOT NULL DEFAULT '[]',
      active INTEGER NOT NULL DEFAULT 1,
      created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
      updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
    );

    CREATE TABLE IF NOT EXISTS webhook_deliveries (
      id TEXT PRIMARY KEY,
      subscription_id TEXT NOT NULL,
      event_id TEXT NOT NULL,
      status TEXT NOT NULL DEFAULT 'pending',
      attempts INTEGER NOT NULL DEFAULT 0,
      next_attempt_at INTEGER NOT NULL,
      last_error TEXT,
      payload TEXT NOT NULL,
      created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
      delivered_at TEXT,
      UNIQUE (subscription_id, event_id),
      FOREIGN KEY (subscription_id) REFERENCES webhook_subscriptions(id)
        ON DELETE CASCADE
    );
    CREATE INDEX IF NOT EXISTS idx_webhook_deliveries_due
      ON webhook_deliveries(status, next_attempt_at);
    CREATE INDEX IF NOT EXISTS idx_webhook_deliveries_event
      ON webhook_deliveries(event_id);
  `);

  db.prepare(
    `INSERT INTO indexer_meta (key, value, updated_at)
     VALUES ('schema_version', ?, CURRENT_TIMESTAMP)
     ON CONFLICT(key) DO UPDATE SET value = excluded.value,
       updated_at = CURRENT_TIMESTAMP`
  ).run(SCHEMA_VERSION);

  db.pragma(`user_version = 2`);
};

export function migrate(db: Database.Database): void {
  db.pragma('journal_mode = WAL');
  db.pragma('foreign_keys = ON');

  db.exec(`
    CREATE TABLE IF NOT EXISTS schema_migrations (
      version TEXT PRIMARY KEY,
      applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
    );
  `);

  const applied = db
    .prepare('SELECT 1 FROM schema_migrations WHERE version = ?')
    .get(MIGRATION_VERSION);
  if (applied) return;

  db.transaction(() => {
    migration(db);
    db.prepare('INSERT INTO schema_migrations (version) VALUES (?)').run(MIGRATION_VERSION);
  })();
}

export function openDatabase(dbPath: string): Database.Database {
  const db = new Database(dbPath);
  migrate(db);
  return db;
}
