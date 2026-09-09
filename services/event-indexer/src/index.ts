/**
 * Event Indexer — off-chain service that listens to all Zyrachain contract events
 * and stores them in a queryable SQLite database.
 */

import { rpc, Contract, Address } from '@stellar/stellar-sdk';
import Database from 'better-sqlite3';
import path from 'path';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));

// ─── Config ──────────────────────────────────────────────────────────

interface IndexerConfig {
  rpcUrl: string;
  networkPassphrase: string;
  dbPath: string;
  pollIntervalMs: number;
  contracts: {
    name: string;
    contractId: string;
  }[];
}

const DEFAULT_CONFIG: IndexerConfig = {
  rpcUrl: process.env.PI_SOROBAN_RPC_URL || 'https://rpc.testnet.minepi.com',
  networkPassphrase: process.env.PI_NETWORK_PASSPHRASE || 'Pi Testnet',
  dbPath: process.env.INDEXER_DB_PATH || path.join(__dirname, '..', 'events.db'),
  pollIntervalMs: parseInt(process.env.INDEXER_POLL_MS || '10000'),
  contracts: [
    { name: 'pool-factory', contractId: process.env.POOL_FACTORY_CONTRACT_ID || '' },
    { name: 'cpmm-pool', contractId: process.env.CPMM_POOL_CONTRACT_ID || '' },
    { name: 'stableswap', contractId: process.env.STABLESWAP_CONTRACT_ID || '' },
    { name: 'lending-pool', contractId: process.env.LENDING_POOL_CONTRACT_ID || '' },
    { name: 'backstop', contractId: process.env.BACKSTOP_CONTRACT_ID || '' },
    { name: 'escrow', contractId: process.env.ESCROW_CONTRACT_ID || '' },
    { name: 'oracle', contractId: process.env.ORACLE_CONTRACT_ID || '' },
    { name: 'bridge-multisig', contractId: process.env.BRIDGE_MULTISIG_CONTRACT_ID || '' },
    { name: 'event-registry', contractId: process.env.EVENT_REGISTRY_CONTRACT_ID || '' },
    { name: 'node-staking', contractId: process.env.NODE_STAKING_CONTRACT_ID || '' },
    { name: 'identity', contractId: process.env.IDENTITY_CONTRACT_ID || '' },
  ].filter(c => c.contractId),
};

// ─── Database ────────────────────────────────────────────────────────

function initDatabase(dbPath: string): Database.Database {
  const db = new Database(dbPath);

  db.exec(`
    CREATE TABLE IF NOT EXISTS events (
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

    CREATE INDEX IF NOT EXISTS idx_events_contract ON events(contract_id);
    CREATE INDEX IF NOT EXISTS idx_events_type ON events(event_type);
    CREATE INDEX IF NOT EXISTS idx_events_ledger ON events(ledger);
    CREATE INDEX IF NOT EXISTS idx_events_tx ON events(tx_hash);

    CREATE TABLE IF NOT EXISTS index_state (
      contract_id TEXT PRIMARY KEY,
      last_ledger INTEGER NOT NULL DEFAULT 0,
      updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
    );
  `);

  return db;
}

// ─── Indexer ─────────────────────────────────────────────────────────

class EventIndexer {
  private server: rpc.Server;
  private db: Database.Database;
  private config: IndexerConfig;
  private running = false;

  constructor(config: IndexerConfig) {
    this.server = new rpc.Server(config.rpcUrl);
    this.db = initDatabase(config.dbPath);
    this.config = config;
  }

  private getLastIndexedLedger(contractId: string): number {
    const row = this.db.prepare(
      'SELECT last_ledger FROM index_state WHERE contract_id = ?'
    ).get(contractId) as { last_ledger: number } | undefined;
    return row?.last_ledger || 0;
  }

  private updateLastIndexedLedger(contractId: string, ledger: number) {
    this.db.prepare(`
      INSERT INTO index_state (contract_id, last_ledger, updated_at)
      VALUES (?, ?, CURRENT_TIMESTAMP)
      ON CONFLICT(contract_id) DO UPDATE SET last_ledger = ?, updated_at = CURRENT_TIMESTAMP
    `).run(contractId, ledger, ledger);
  }

  private async fetchEvents(contractId: string, fromLedger: number) {
    try {
      const latestLedger = await this.server.getLatestLedger();
      const toLedger = latestLedger.sequence;

      if (fromLedger >= toLedger) return [];

      const events = await this.server.getEvents({
        startLedger: fromLedger + 1,
        endLedger: toLedger,
        filters: [{
          type: 'contract',
          contractIds: [contractId],
        }],
        limit: 100,
      });

      return events.events || [];
    } catch (err) {
      console.error(`Error fetching events for ${contractId}:`, err);
      return [];
    }
  }

  private storeEvent(
    contractId: string,
    contractName: string,
    eventType: string,
    txHash: string,
    ledger: number,
    timestamp: number,
    data: string
  ) {
    this.db.prepare(`
      INSERT INTO events (contract_id, contract_name, event_type, tx_hash, ledger, timestamp, data)
      VALUES (?, ?, ?, ?, ?, ?, ?)
    `).run(contractId, contractName, eventType, txHash, ledger, timestamp, data);
  }

  async indexContract(contractConfig: { name: string; contractId: string }) {
    const { name, contractId } = contractConfig;
    const lastLedger = this.getLastIndexedLedger(contractId);
    const events = await this.fetchEvents(contractId, lastLedger);

    if (events.length === 0) return;

    let maxLedger = lastLedger;

    for (const event of events) {
      const ledger = parseInt(String(event.ledger || '0'));
      const txHash = event.transactionHash || '';
      const timestamp = Math.floor(Date.now() / 1000);

      // Extract event type from topics
      let eventType = 'unknown';
      let data = '';
      if (event.topics && event.topics.length > 0) {
        eventType = String(event.topics[0]);
      }
      if (event.value) {
        data = JSON.stringify(event.value);
      }

      this.storeEvent(contractId, name, eventType, txHash, ledger, timestamp, data);

      if (ledger > maxLedger) {
        maxLedger = ledger;
      }
    }

    this.updateLastIndexedLedger(contractId, maxLedger);
    console.log(`Indexed ${events.length} events for ${name} (ledger ${maxLedger})`);
  }

  async start() {
    this.running = true;
    console.log(`Event indexer started, polling every ${this.config.pollIntervalMs}ms`);
    console.log(`Monitoring ${this.config.contracts.length} contracts`);

    while (this.running) {
      try {
        for (const contract of this.config.contracts) {
          await this.indexContract(contract);
        }
      } catch (err) {
        console.error('Indexing error:', err);
      }

      await new Promise(resolve => setTimeout(resolve, this.config.pollIntervalMs));
    }
  }

  stop() {
    this.running = false;
    this.db.close();
    console.log('Event indexer stopped');
  }

  // ─── Query Methods ──────────────────────────────────────────────

  getEventsByContract(contractId: string, limit = 100) {
    return this.db.prepare(
      'SELECT * FROM events WHERE contract_id = ? ORDER BY ledger DESC LIMIT ?'
    ).all(contractId, limit);
  }

  getEventsByType(eventType: string, limit = 100) {
    return this.db.prepare(
      'SELECT * FROM events WHERE event_type = ? ORDER BY ledger DESC LIMIT ?'
    ).all(eventType, limit);
  }

  getEventsByLedger(ledger: number) {
    return this.db.prepare(
      'SELECT * FROM events WHERE ledger = ?'
    ).all(ledger);
  }

  getRecentEvents(limit = 100) {
    return this.db.prepare(
      'SELECT * FROM events ORDER BY ledger DESC LIMIT ?'
    ).all(limit);
  }

  getEventStats() {
    return this.db.prepare(`
      SELECT contract_name, event_type, COUNT(*) as count
      FROM events
      GROUP BY contract_name, event_type
      ORDER BY count DESC
    `).all();
  }
}

// ─── Main ────────────────────────────────────────────────────────────

const config = DEFAULT_CONFIG;
const indexer = new EventIndexer(config);

process.on('SIGINT', () => {
  indexer.stop();
  process.exit(0);
});

process.on('SIGTERM', () => {
  indexer.stop();
  process.exit(0);
});

indexer.start().catch(console.error);

export { EventIndexer, type IndexerConfig };
