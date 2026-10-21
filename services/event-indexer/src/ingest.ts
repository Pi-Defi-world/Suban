/**
 * Ingestion pipeline.
 *
 * Two independent streams feed the same normalized schema:
 * - Soroban contract events through Stellar RPC.
 * - Ledgers, transactions, operations and account snapshots through Horizon.
 *
 * Both streams are checkpointed and idempotent, so a replay from a checkpoint
 * reproduces the same rows instead of duplicating them.
 */
import crypto from 'crypto';
import Database from 'better-sqlite3';
import { rpc, scValToNative, xdr } from '@stellar/stellar-sdk';
import { ContractConfig, IndexerConfig } from './config.js';
import {
  HorizonAccount,
  HorizonClient,
  HorizonLedger,
  HorizonOperation,
  HorizonTransaction,
} from './horizon.js';
import {
  accountsObserved,
  eventsIndexed,
  fetchErrors,
  indexErrors,
  ingestLagLedgers,
  ingestLagSeconds,
  lastLedger,
  latestObservedLedger,
  ledgersIndexed,
  operationCompleteness,
  operationsIndexed,
  pollsTotal,
  reorgDetected,
  replayResets,
  running as runningGauge,
  storedAccounts,
  storedEvents,
  storedOperations,
  storedTransactions,
  transactionsIndexed,
} from './metrics.js';
import {
  getCheckpoint,
  getNumberMeta,
  nowIso,
  resetCheckpoint,
  setCheckpoint,
  setMeta,
} from './meta.js';
import { IndexedEventForWebhook, WebhookStore } from './webhooks.js';

const ACCOUNT_RE = /\bG[A-Z2-7]{55}\b/g;

interface SorobanEvent {
  id?: string;
  ledger?: number | string;
  txHash?: string;
  transactionHash?: string;
  ledgerClosedAt?: string;
  topic?: unknown[];
  topics?: unknown[];
  value?: unknown;
}

function collectAccounts(value: unknown, into: Set<string>): void {
  if (typeof value === 'string') {
    const matches = value.match(ACCOUNT_RE);
    if (matches) matches.forEach((match) => into.add(match));
    return;
  }
  if (Array.isArray(value)) {
    value.forEach((entry) => collectAccounts(entry, into));
    return;
  }
  if (value && typeof value === 'object') {
    Object.values(value as Record<string, unknown>).forEach((entry) =>
      collectAccounts(entry, into)
    );
  }
}

function decodeScVal(value: unknown): unknown {
  if (typeof value !== 'string') return value;
  try {
    return scValToNative(xdr.ScVal.fromXDR(value, 'base64'));
  } catch {
    return value;
  }
}

function eventTypeFrom(e: SorobanEvent): string {
  const topics = e.topic || e.topics || [];
  if (!topics.length) return 'unknown';
  const decoded = decodeScVal(topics[0]);
  return typeof decoded === 'string' ? decoded : JSON.stringify(decoded);
}

function eventData(e: SorobanEvent): unknown {
  if (e.value === undefined) return null;
  const decoded = decodeScVal(e.value);
  if (typeof decoded === 'string' && decoded.length > 0) return decoded;
  return decoded;
}

function deterministicEventId(
  contractId: string,
  event: SorobanEvent,
  index: number
): string {
  if (event.id) return `${contractId}:${event.id}`;
  const hash = crypto
    .createHash('sha256')
    .update(
      [
        contractId,
        event.ledger || 0,
        event.txHash || event.transactionHash || '',
        JSON.stringify(event.topic || event.topics || []),
        JSON.stringify(event.value || null),
        index,
      ].join('|')
    )
    .digest('hex');
  return `${contractId}:${hash}`;
}

function eventTimestamp(event: SorobanEvent): number {
  if (event.ledgerClosedAt) {
    const parsed = Date.parse(event.ledgerClosedAt);
    if (Number.isFinite(parsed)) return Math.floor(parsed / 1000);
  }
  return Math.floor(Date.now() / 1000);
}

export function upsertContract(
  db: Database.Database,
  contract: ContractConfig,
  network: string,
  metadata: Record<string, unknown> = {}
): void {
  db.prepare(
    `INSERT INTO contracts
     (contract_id, network, name, interface_version, verified, metadata, updated_at)
     VALUES (?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP)
     ON CONFLICT(contract_id) DO UPDATE SET
       network = excluded.network,
       name = excluded.name,
       interface_version = excluded.interface_version,
       verified = excluded.verified,
       metadata = COALESCE(NULLIF(excluded.metadata, '{}'), contracts.metadata),
       updated_at = CURRENT_TIMESTAMP`
  ).run(
    contract.contractId,
    network,
    contract.name,
    contract.interfaceVersion,
    contract.verified ? 1 : 0,
    JSON.stringify(metadata)
  );
}

export class SubanIngestor {
  private readonly horizon: HorizonClient | null;
  private readonly webhookStore: WebhookStore;
  private running = false;
  private timer: NodeJS.Timeout | null = null;

  constructor(
    private readonly db: Database.Database,
    private readonly config: IndexerConfig,
    private readonly server: rpc.Server,
    horizon: HorizonClient | null = null,
    webhookStore?: WebhookStore
  ) {
    this.horizon =
      horizon || (config.horizonUrl ? new HorizonClient(config.horizonUrl) : null);
    this.webhookStore = webhookStore || new WebhookStore(db);

    for (const contract of config.contracts) {
      upsertContract(db, contract, config.networkPassphrase);
      lastLedger.set(
        { contract: contract.name },
        getCheckpoint(db, `events:${contract.contractId}`)
      );
    }
    this.refreshStoredGauges();
  }

  async start(): Promise<void> {
    if (this.running) return;
    this.running = true;
    runningGauge.set(1);
    console.log(
      `Data platform started: ${this.config.contracts.length} contracts, ` +
        `Horizon ${this.horizon ? 'enabled' : 'disabled'}, poll ${this.config.pollIntervalMs}ms`
    );

    while (this.running) {
      await this.pollOnce();
      await new Promise((resolve) => {
        this.timer = setTimeout(resolve, this.config.pollIntervalMs);
        this.timer.unref?.();
      });
    }
  }

  stop(): void {
    this.running = false;
    if (this.timer) clearTimeout(this.timer);
    this.timer = null;
    runningGauge.set(0);
    console.log('Data platform stopped');
  }

  isRunning(): boolean {
    return this.running;
  }

  async pollOnce(): Promise<void> {
    pollsTotal.inc();
    let succeeded = false;

    try {
      await this.pollContractEvents();
      succeeded = true;
    } catch (error: any) {
      indexErrors.inc({ stage: 'events' });
      fetchErrors.inc({ source: 'soroban-rpc' });
      console.error('Contract event ingestion failed:', error?.message || error);
    }

    if (this.horizon) {
      try {
        await this.pollHorizon();
        succeeded = true;
      } catch (error: any) {
        indexErrors.inc({ stage: 'horizon' });
        fetchErrors.inc({ source: 'horizon' });
        console.error('Horizon ingestion failed:', error?.message || error);
      }
    }

    const latestIndexed = this.latestIndexedLedger();
    const observed = getNumberMeta(this.db, 'latest_observed_ledger') || latestIndexed;
    setMeta(this.db, 'latest_indexed_ledger', latestIndexed);
    setMeta(this.db, 'last_poll_at', nowIso());
    if (succeeded) {
      const successSeconds = Math.floor(Date.now() / 1000);
      setMeta(this.db, 'last_success_at', nowIso());
      setMeta(this.db, 'last_success_at_ms', successSeconds);
    }

    const lag = Math.max(0, observed - latestIndexed);
    ingestLagLedgers.set(lag);
    const lastSuccess = getNumberMeta(this.db, 'last_success_at_ms');
    if (lastSuccess) {
      ingestLagSeconds.set(Math.max(0, Math.floor(Date.now() / 1000) - lastSuccess));
    }
    this.refreshStoredGauges();
  }

  private async pollContractEvents(): Promise<void> {
    if (this.config.contracts.length === 0) return;
    const latest = await this.server.getLatestLedger();
    const latestSequence = Number(latest.sequence);
    this.observeLedger(latestSequence);

    for (const contract of this.config.contracts) {
      const stream = `events:${contract.contractId}`;
      let checkpoint = getCheckpoint(this.db, stream);
      if (checkpoint === 0) {
        checkpoint = Math.max(
          1,
          latestSequence - this.config.eventBootstrapLedgers
        );
        setCheckpoint(this.db, stream, checkpoint);
      }
      if (checkpoint >= latestSequence) {
        lastLedger.set({ contract: contract.name }, checkpoint);
        continue;
      }

      let from = checkpoint;
      for (let page = 0; page < 20 && from < latestSequence; page++) {
        const response: any = await this.server.getEvents({
          startLedger: from + 1,
          filters: [{ type: 'contract', contractIds: [contract.contractId] }],
          limit: 200,
        });
        const events: SorobanEvent[] = response.events || [];
        if (events.length === 0) break;

        let maxLedger = from;
        for (let index = 0; index < events.length; index++) {
          const event = events[index];
          const ledger = parseInt(String(event.ledger || '0'), 10);
          const eventId = deterministicEventId(contract.contractId, event, index);
          const type = eventTypeFrom(event);
          const data = eventData(event);
          const timestamp = eventTimestamp(event);
          const txHash = event.txHash || event.transactionHash || '';
          const raw = JSON.stringify(event);

          const inserted = this.db
            .prepare(
              `INSERT OR IGNORE INTO events
               (event_id, contract_id, contract_name, event_type, tx_hash, ledger,
                timestamp, data, ledger_closed_at, raw)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`
            )
            .run(
              eventId,
              contract.contractId,
              contract.name,
              type,
              txHash,
              ledger,
              timestamp,
              data === null ? null : JSON.stringify(data),
              event.ledgerClosedAt || null,
              raw
            ).changes;

          if (inserted > 0) {
            eventsIndexed.inc({ contract: contract.name, event_type: type });
            this.webhookStore.enqueue({
              event_id: eventId,
              contract_id: contract.contractId,
              contract_name: contract.name,
              event_type: type,
              tx_hash: txHash,
              ledger,
              timestamp,
              data,
            } as IndexedEventForWebhook);
          }
          if (ledger > maxLedger) maxLedger = ledger;
        }

        setCheckpoint(this.db, stream, maxLedger);
        lastLedger.set({ contract: contract.name }, maxLedger);
        from = maxLedger;
        if (events.length < 200) break;
        if (response.cursor && response.cursor === events[events.length - 1]?.id) break;
      }
    }
  }

  private async pollHorizon(): Promise<void> {
    if (!this.horizon) return;
    const latest = await this.horizon.latestLedger();
    this.observeLedger(latest.sequence);

    let checkpoint = getCheckpoint(this.db, 'horizon');
    if (checkpoint === 0) {
      checkpoint = Math.max(
        1,
        latest.sequence - this.config.horizonBootstrapLedgers
      );
      setCheckpoint(this.db, 'horizon', checkpoint);
    }
    if (checkpoint >= latest.sequence) {
      this.markSuccess();
      return;
    }

    const target = Math.min(
      latest.sequence,
      checkpoint + this.config.maxLedgersPerPoll
    );
    const ledgers = await this.horizon.ledgersFrom(checkpoint, target - checkpoint);
    const relevant = ledgers.filter(
      (ledger) => ledger.sequence > checkpoint && ledger.sequence <= target
    );

    const newlyObserved = new Set<string>();
    for (const ledger of relevant) {
      const transactions = await this.horizon.transactionsForLedger(ledger.sequence);
      const operations = new Map<string, HorizonOperation[]>();
      const ledgerOperations = await this.horizon.operationsForLedger(ledger.sequence);
      for (const operation of ledgerOperations) {
        const txHash = operation.transaction_hash;
        const list = operations.get(txHash) || [];
        list.push(operation);
        operations.set(txHash, list);
      }
      this.writeLedger(ledger, transactions, operations, newlyObserved);
      setCheckpoint(this.db, 'horizon', ledger.sequence);
      this.markSuccess();
    }

    if (relevant.length === 0) {
      // A quiet chain can still advance. Do not get stuck retrying an empty gap.
      setCheckpoint(this.db, 'horizon', target);
      this.markSuccess();
    }

    // Core ledger data is now durable. Hydration is bounded and non-fatal so
    // account snapshot latency never makes indexed chain data look stale.
    try {
      await this.hydrateAccounts(newlyObserved);
    } catch (error: any) {
      indexErrors.inc({ stage: 'accounts' });
      console.warn('Account snapshot hydration failed:', error?.message || error);
    }
  }

  private markSuccess(): void {
    setMeta(this.db, 'last_success_at', nowIso());
    setMeta(this.db, 'last_success_at_ms', Math.floor(Date.now() / 1000));
  }

  private writeLedger(
    ledger: HorizonLedger,
    transactions: HorizonTransaction[],
    operations: Map<string, HorizonOperation[]>,
    observedAccounts: Set<string>
  ): void {
    this.db.transaction(() => {
      const previous = this.db
        .prepare('SELECT hash FROM ledgers WHERE sequence = ?')
        .get(ledger.sequence - 1) as { hash: string } | undefined;
      if (
        previous?.hash &&
        ledger.prev_hash &&
        previous.hash !== ledger.prev_hash
      ) {
        reorgDetected.inc({ stream: 'horizon' });
        console.warn(
          `Reorg signal at ledger ${ledger.sequence}: expected parent ${previous.hash}, got ${ledger.prev_hash}`
        );
      }

      this.db
        .prepare(
          `INSERT OR IGNORE INTO ledgers
           (sequence, hash, prev_hash, closed_at, successful_transaction_count,
            failed_transaction_count, operation_count, tx_set_operation_count,
            total_coins, fee_pool, base_fee_in_stroops, base_reserve_in_stroops,
            max_tx_set_size, protocol_version, header_xdr, raw)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`
        )
        .run(
          ledger.sequence,
          ledger.hash,
          ledger.prev_hash || null,
          ledger.closed_at,
          ledger.successful_transaction_count || 0,
          ledger.failed_transaction_count || 0,
          ledger.operation_count || 0,
          ledger.tx_set_operation_count || 0,
          ledger.total_coins || null,
          ledger.fee_pool || null,
          ledger.base_fee_in_stroops || null,
          ledger.base_reserve_in_stroops || null,
          ledger.max_tx_set_size || null,
          ledger.protocol_version || null,
          ledger.header_xdr || null,
          JSON.stringify(ledger)
        );

      const ledgerAccounts = new Set<string>();
      collectAccounts(ledger, ledgerAccounts);
      ledgerAccounts.forEach((account) => {
        observedAccounts.add(account);
      });
      const seenAccounts = new Set<string>(ledgerAccounts);

      for (const transaction of transactions) {
        const txInserted = this.db
          .prepare(
            `INSERT OR IGNORE INTO transactions
             (hash, ledger, successful, source_account, source_account_sequence,
              fee_account, fee_charged, max_fee, operation_count, created_at,
              memo_type, memo, envelope_xdr, result_xdr, result_meta_xdr, raw)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`
          )
          .run(
            transaction.hash,
            transaction.ledger,
            transaction.successful ? 1 : 0,
            transaction.source_account || null,
            transaction.source_account_sequence || null,
            transaction.fee_account || null,
            transaction.fee_charged || null,
            transaction.max_fee || null,
            transaction.operation_count || 0,
            transaction.created_at,
            transaction.memo_type || null,
            transaction.memo || null,
            transaction.envelope_xdr || null,
            transaction.result_xdr || null,
            transaction.result_meta_xdr || null,
            JSON.stringify(transaction)
          ).changes;
        if (txInserted > 0) transactionsIndexed.inc();

        const txAccounts = new Set<string>();
        collectAccounts(transaction, txAccounts);
        txAccounts.forEach((account) => {
          observedAccounts.add(account);
          seenAccounts.add(account);
          this.db
            .prepare(
              'INSERT OR IGNORE INTO transaction_accounts (tx_hash, account_id) VALUES (?, ?)'
            )
            .run(transaction.hash, account);
        });

        for (const operation of operations.get(transaction.hash) || []) {
          const opInserted = this.db
            .prepare(
              `INSERT OR IGNORE INTO operations
               (id, paging_token, tx_hash, ledger, source_account, type, type_i,
                created_at, transaction_successful, details, raw)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`
            )
            .run(
              operation.id,
              operation.paging_token || null,
              operation.transaction_hash || transaction.hash,
              transaction.ledger,
              operation.source_account || null,
              operation.type,
              operation.type_i || null,
              operation.created_at,
              operation.transaction_successful === false ? 0 : 1,
              JSON.stringify(operation),
              JSON.stringify(operation)
            ).changes;
          if (opInserted > 0) operationsIndexed.inc();

          const opAccounts = new Set<string>();
          collectAccounts(operation, opAccounts);
          opAccounts.forEach((account) => {
            observedAccounts.add(account);
            seenAccounts.add(account);
            this.db
              .prepare(
                'INSERT OR IGNORE INTO operation_accounts (operation_id, account_id) VALUES (?, ?)'
              )
              .run(operation.id, account);
          });
        }
      }

      seenAccounts.forEach((account) => this.upsertAccountStub(account, ledger.sequence));
      ledgersIndexed.inc();
      latestObservedLedger.set(Math.max(ledger.sequence, this.latestIndexedLedger()));
      const storedOperations = (
        this.db
          .prepare('SELECT COUNT(*) AS count FROM operations WHERE ledger = ?')
          .get(ledger.sequence) as { count: number }
      ).count;
      operationCompleteness.set(
        ledger.operation_count && ledger.operation_count > 0
          ? Math.min(1, storedOperations / ledger.operation_count)
          : 1
      );
    })();
  }

  private upsertAccountStub(accountId: string, ledger: number): void {
    const inserted = this.db
      .prepare(
        `INSERT OR IGNORE INTO accounts
         (account_id, last_seen_ledger, created_at, updated_at)
         VALUES (?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)`
      )
      .run(accountId, ledger).changes;
    if (inserted > 0) {
      accountsObserved.inc();
      return;
    }

    this.db
      .prepare(
        `UPDATE accounts
         SET last_seen_ledger = CASE
               WHEN last_seen_ledger IS NULL OR last_seen_ledger < ?
               THEN ? ELSE last_seen_ledger END,
             updated_at = CURRENT_TIMESTAMP
         WHERE account_id = ?`
      )
      .run(ledger, ledger, accountId);
  }

  private async hydrateAccounts(accountIds: Set<string>): Promise<void> {
    if (!this.horizon || !this.config.fetchAccountSnapshots) return;
    const candidates = [...accountIds].slice(0, this.config.accountFetchLimit);
    for (const accountId of candidates) {
      const row = this.db
        .prepare('SELECT last_modified_ledger FROM accounts WHERE account_id = ?')
        .get(accountId) as { last_modified_ledger: number | null } | undefined;
      if (row?.last_modified_ledger) continue;

      try {
        const account = await this.horizon.account(accountId);
        this.writeAccountSnapshot(account);
      } catch (error: any) {
        if (error?.status !== 404) {
          fetchErrors.inc({ source: 'horizon-account' });
          console.warn(`Account snapshot failed for ${accountId}:`, error?.message || error);
        }
      }
    }
  }

  private writeAccountSnapshot(account: HorizonAccount): void {
    this.db
      .prepare(
        `INSERT INTO accounts
         (account_id, sequence, sequence_ledger, last_modified_ledger, last_seen_ledger,
          balances, signers, thresholds, flags, home_domain, subentry_count,
          num_sponsoring, num_sponsored, raw, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP)
         ON CONFLICT(account_id) DO UPDATE SET
           sequence = excluded.sequence,
           sequence_ledger = excluded.sequence_ledger,
           last_modified_ledger = excluded.last_modified_ledger,
           last_seen_ledger = CASE
             WHEN accounts.last_seen_ledger IS NULL OR accounts.last_seen_ledger < excluded.last_seen_ledger
             THEN excluded.last_seen_ledger ELSE accounts.last_seen_ledger END,
           balances = excluded.balances,
           signers = excluded.signers,
           thresholds = excluded.thresholds,
           flags = excluded.flags,
           home_domain = excluded.home_domain,
           subentry_count = excluded.subentry_count,
           num_sponsoring = excluded.num_sponsoring,
           num_sponsored = excluded.num_sponsored,
           raw = excluded.raw,
           updated_at = CURRENT_TIMESTAMP`
      )
      .run(
        account.account_id,
        account.sequence || null,
        account.sequence_ledger || null,
        account.last_modified_ledger || null,
        account.last_modified_ledger || null,
        JSON.stringify(account.balances || []),
        JSON.stringify(account.signers || []),
        JSON.stringify(account.thresholds || {}),
        JSON.stringify(account.flags || {}),
        account.home_domain || null,
        account.subentry_count || 0,
        account.num_sponsoring || 0,
        account.num_sponsored || 0,
        JSON.stringify(account)
      );
  }

  private observeLedger(sequence: number): void {
    const current = getNumberMeta(this.db, 'latest_observed_ledger') || 0;
    if (sequence > current) {
      setMeta(this.db, 'latest_observed_ledger', sequence);
      latestObservedLedger.set(sequence);
    }
  }

  private latestIndexedLedger(): number {
    const row = this.db
      .prepare('SELECT MAX(last_ledger) AS ledger FROM checkpoints')
      .get() as { ledger: number | null };
    return row?.ledger || 0;
  }

  private refreshStoredGauges(): void {
    const events = this.db.prepare('SELECT COUNT(*) AS count FROM events').get() as {
      count: number;
    };
    const transactions = this.db
      .prepare('SELECT COUNT(*) AS count FROM transactions')
      .get() as { count: number };
    const operations = this.db
      .prepare('SELECT COUNT(*) AS count FROM operations')
      .get() as { count: number };
    const accounts = this.db.prepare('SELECT COUNT(*) AS count FROM accounts').get() as {
      count: number;
    };
    storedEvents.set(events.count);
    storedTransactions.set(transactions.count);
    storedOperations.set(operations.count);
    storedAccounts.set(accounts.count);
  }

  resetStream(stream: string, fromLedger: number): void {
    resetCheckpoint(this.db, stream, fromLedger);
    replayResets.inc({ stream });
  }

  getCheckpoints() {
    return this.db
      .prepare('SELECT stream, cursor, last_ledger, updated_at FROM checkpoints ORDER BY stream')
      .all();
  }
}
