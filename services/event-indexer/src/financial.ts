/**
 * Financial-data views derived from indexed contract events.
 *
 * This is deliberately a read-only projection over the `events` table. It does
 * not call RPC/Horizon and it does not invent data that the contracts do not
 * emit. In particular:
 * - Swap events contain two raw amounts but not token direction or decimals.
 * - Pool events do not contain pool reserves.
 * - LP positions require current LP-token balances, which are not indexed.
 *
 * The API labels those gaps explicitly instead of publishing a fabricated USD
 * volume or an inferred position.
 */
import { Router, Request, Response } from 'express';
import Database from 'better-sqlite3';
import { scValToNative, xdr } from '@stellar/stellar-sdk';

type JsonObject = Record<string, unknown>;

const FINANCIAL_EVENT_TYPES = [
  'swap',
  'ss_swap',
  'add_liq',
  'ss_add',
  'rm_liq',
  'ss_rm',
] as const;

interface FinancialEvent {
  id: string;
  contract_id: string;
  contract_name: string;
  pool_kind: 'cpmm' | 'stableswap' | 'unknown';
  event_type: 'swap' | 'add_liquidity' | 'remove_liquidity';
  account: string | null;
  tx_hash: string;
  ledger: number;
  timestamp: number;
  amount_a_raw: string | null;
  amount_b_raw: string | null;
  shares_raw: string | null;
  token_a: null;
  token_b: null;
  usd_value: null;
  data: unknown;
}

function parseJson(value: unknown, fallback: unknown): unknown {
  if (typeof value !== 'string') return fallback;
  try {
    return JSON.parse(value);
  } catch {
    return fallback;
  }
}

function decodeTopic(value: unknown): unknown {
  if (typeof value !== 'string') return value;
  try {
    return scValToNative(xdr.ScVal.fromXDR(value, 'base64'));
  } catch {
    return value;
  }
}

function topicsOf(row: JsonObject): unknown[] {
  const raw = parseJson(row.raw, null);
  if (!raw || typeof raw !== 'object') return [];
  const event = raw as JsonObject;
  const topics = event.topic ?? event.topics;
  if (!Array.isArray(topics)) return [];
  return topics.map(decodeTopic);
}

function accountOf(row: JsonObject): string | null {
  for (const topic of topicsOf(row)) {
    if (typeof topic === 'string' && /^G[A-Z2-7]{55}$/.test(topic)) {
      return topic;
    }
  }
  return null;
}

function rawAmount(value: unknown): string | null {
  if (typeof value === 'bigint') return value.toString();
  if (typeof value === 'number' && Number.isFinite(value)) return String(value);
  if (typeof value === 'string' && value.length > 0) return value;
  return null;
}

function poolKind(contractName: string, eventType: string): FinancialEvent['pool_kind'] {
  if (contractName.toLowerCase().includes('stable') || eventType.startsWith('ss_')) {
    return 'stableswap';
  }
  if (contractName.toLowerCase().includes('cpmm')) return 'cpmm';
  return 'unknown';
}

export function classifyFinancialEvent(
  eventType: string
): 'swap' | 'add_liquidity' | 'remove_liquidity' | null {
  switch (eventType) {
    case 'swap':
    case 'ss_swap':
      return 'swap';
    case 'add_liq':
    case 'ss_add':
      return 'add_liquidity';
    case 'rm_liq':
    case 'ss_rm':
      return 'remove_liquidity';
    default:
      return null;
  }
}

export function parseFinancialEvent(row: JsonObject): FinancialEvent | null {
  const eventType = String(row.event_type || '');
  const normalized = classifyFinancialEvent(eventType);
  if (!normalized) return null;

  const data = parseJson(row.data, null);
  const values = Array.isArray(data) ? data : [];

  return {
    id: String(row.event_id || `event_${row.id}`),
    contract_id: String(row.contract_id || ''),
    contract_name: String(row.contract_name || ''),
    pool_kind: poolKind(String(row.contract_name || ''), eventType),
    event_type: normalized,
    account: accountOf(row),
    tx_hash: String(row.tx_hash || ''),
    ledger: Number(row.ledger || 0),
    timestamp: Number(row.timestamp || 0),
    amount_a_raw: rawAmount(values[0]),
    amount_b_raw: rawAmount(values[1]),
    shares_raw: normalized === 'swap' ? null : rawAmount(values[2]),
    token_a: null,
    token_b: null,
    usd_value: null,
    data,
  };
}

function sumRaw(values: Array<string | null>): string {
  let total = 0n;
  for (const value of values) {
    if (value === null || !/^-?\d+$/.test(value)) continue;
    try {
      total += BigInt(value);
    } catch {
      // Ignore a single malformed amount rather than failing the whole view.
    }
  }
  return total.toString();
}

function eventRows(
  db: Database.Database,
  clause: string,
  params: unknown[],
  limit: number,
  offset: number
): JsonObject[] {
  return db
    .prepare(
      `SELECT * FROM events
        WHERE ${clause}
        ORDER BY timestamp DESC, id DESC
        LIMIT ? OFFSET ?`
    )
    .all(...params, limit, offset) as JsonObject[];
}

function countRows(
  db: Database.Database,
  clause: string,
  params: unknown[]
): number {
  return (
    db.prepare(`SELECT COUNT(*) AS count FROM events WHERE ${clause}`).get(...params) as {
      count: number;
    }
  ).count;
}

function page(req: Request): { limit: number; offset: number } {
  const requestedLimit = parseInt(String(req.query.limit || '100'), 10);
  const requestedOffset = parseInt(String(req.query.offset || '0'), 10);
  return {
    limit: Math.min(1_000, Math.max(1, Number.isFinite(requestedLimit) ? requestedLimit : 100)),
    offset: Math.max(0, Number.isFinite(requestedOffset) ? requestedOffset : 0),
  };
}

function pagination(limit: number, offset: number, total: number) {
  return {
    limit,
    offset,
    total,
    has_more: offset + limit < total,
  };
}

export function createFinancialRouter(
  db: Database.Database,
  payload: (body: Record<string, unknown>) => Record<string, unknown>
): Router {
  const router = Router();
  const placeholders = FINANCIAL_EVENT_TYPES.map(() => '?').join(',');

  router.get('/pools', (req: Request, res: Response) => {
    const { limit, offset } = page(req);
    const rows = db
      .prepare(
        `SELECT contract_id, contract_name, event_type, COUNT(*) AS count,
                MAX(timestamp) AS last_timestamp
           FROM events
          WHERE event_type IN (${placeholders})
          GROUP BY contract_id, contract_name, event_type
          ORDER BY contract_id ASC`
      )
      .all(...FINANCIAL_EVENT_TYPES) as Array<{
      contract_id: string;
      contract_name: string;
      event_type: string;
      count: number;
      last_timestamp: number;
    }>;

    const pools = new Map<
      string,
      {
        contract_id: string;
        contract_name: string;
        pool_kind: string;
        event_count: number;
        trade_count: number;
        add_liquidity_count: number;
        remove_liquidity_count: number;
        last_event_at: string | null;
      }
    >();

    for (const row of rows) {
      const current =
        pools.get(row.contract_id) ||
        {
          contract_id: row.contract_id,
          contract_name: row.contract_name,
          pool_kind: poolKind(row.contract_name, row.event_type),
          event_count: 0,
          trade_count: 0,
          add_liquidity_count: 0,
          remove_liquidity_count: 0,
          last_event_at: null,
        };
      current.event_count += row.count;
      if (row.event_type === 'swap' || row.event_type === 'ss_swap') {
        current.trade_count += row.count;
      } else if (row.event_type === 'add_liq' || row.event_type === 'ss_add') {
        current.add_liquidity_count += row.count;
      } else {
        current.remove_liquidity_count += row.count;
      }
      current.last_event_at = new Date(
        Math.max(
          current.last_event_at ? Date.parse(current.last_event_at) : 0,
          row.last_timestamp * 1_000
        )
      ).toISOString();
      pools.set(row.contract_id, current);
    }

    const all = [...pools.values()];
    res.json(
      payload({
        data: all.slice(offset, offset + limit),
        pagination: pagination(limit, offset, all.length),
        coverage: {
          trades: true,
          liquidity_events: true,
          raw_volume: true,
          usd_volume: false,
          lp_positions: false,
          reason:
            'Swap and liquidity events expose raw amounts, not token direction/decimals or LP balances. USD volume and LP positions are intentionally unavailable.',
        },
      })
    );
  });

  router.get('/pools/:contractId', (req: Request, res: Response) => {
    const contractId = req.params.contractId;
    const rows = db
      .prepare(
        `SELECT * FROM events
          WHERE contract_id = ? AND event_type IN (${placeholders})
          ORDER BY timestamp DESC, id DESC
          LIMIT 1000`
      )
      .all(contractId, ...FINANCIAL_EVENT_TYPES) as JsonObject[];

    if (rows.length === 0) {
      const contract = db
        .prepare('SELECT * FROM contracts WHERE contract_id = ?')
        .get(contractId) as JsonObject | undefined;
      if (!contract) {
        res.status(404).json({ error: 'Pool not found' });
        return;
      }
    }

    const events = rows
      .map(parseFinancialEvent)
      .filter((event): event is FinancialEvent => event !== null);
    const trades = events.filter((event) => event.event_type === 'swap');
    const adds = events.filter((event) => event.event_type === 'add_liquidity');
    const removes = events.filter((event) => event.event_type === 'remove_liquidity');

    res.json(
      payload({
        data: {
          contract_id: contractId,
          contract_name: rows[0]?.contract_name || null,
          pool_kind: events[0]?.pool_kind || 'unknown',
          event_count: events.length,
          trade_count: trades.length,
          add_liquidity_count: adds.length,
          remove_liquidity_count: removes.length,
          volume: {
            amount_in_raw: sumRaw(trades.map((event) => event.amount_a_raw)),
            amount_out_raw: sumRaw(trades.map((event) => event.amount_b_raw)),
            unit: 'raw_token_units',
            usd_value: null,
            note:
              'Token direction and decimals are not in the emitted event; raw amounts are not converted to USD.',
          },
          lp_positions: {
            available: false,
            reason: 'LP-token balances are not indexed; only liquidity events are available.',
          },
          recent_events: events.slice(0, 20),
        },
      })
    );
  });

  router.get('/pools/:contractId/trades', (req: Request, res: Response) => {
    const { limit, offset } = page(req);
    const clause = `contract_id = ? AND event_type IN ('swap','ss_swap')`;
    const rows = eventRows(db, clause, [req.params.contractId], limit, offset);
    const total = countRows(db, clause, [req.params.contractId]);
    res.json(
      payload({
        data: rows
          .map(parseFinancialEvent)
          .filter((event): event is FinancialEvent => event !== null),
        pagination: pagination(limit, offset, total),
        coverage: {
          token_in: false,
          token_out: false,
          usd_value: false,
          reason:
            'The contracts emit amount_in and amount_out only; token direction and decimals are not available from the event.',
        },
      })
    );
  });

  router.get('/pools/:contractId/liquidity', (req: Request, res: Response) => {
    const { limit, offset } = page(req);
    const clause = `contract_id = ? AND event_type IN ('add_liq','ss_add','rm_liq','ss_rm')`;
    const rows = eventRows(db, clause, [req.params.contractId], limit, offset);
    const total = countRows(db, clause, [req.params.contractId]);
    res.json(
      payload({
        data: rows
          .map(parseFinancialEvent)
          .filter((event): event is FinancialEvent => event !== null),
        pagination: pagination(limit, offset, total),
        coverage: {
          lp_positions: false,
          reason:
            'Liquidity events show amounts and shares moved, not the current LP balance of an account.',
        },
      })
    );
  });

  router.get('/accounts/:accountId/activity', (req: Request, res: Response) => {
    const { limit, offset } = page(req);
    // The account is encoded inside the event topic XDR, so it cannot be
    // filtered in SQL. Bound the scan and filter the decoded topics.
    const scanLimit = 5_000;
    const rows = eventRows(
      db,
      `event_type IN (${placeholders})`,
      [...FINANCIAL_EVENT_TYPES],
      scanLimit,
      0
    );
    const events = rows
      .map(parseFinancialEvent)
      .filter(
        (event): event is FinancialEvent =>
          event !== null && event.account === req.params.accountId
      );
    res.json(
      payload({
        data: events.slice(offset, offset + limit),
        pagination: pagination(limit, offset, events.length),
        coverage: {
          scanned_events: rows.length,
          scan_limit: scanLimit,
          lp_positions: false,
          reason:
            'Activity is derived from a bounded recent-event scan; it is not a full LP position history.',
        },
      })
    );
  });

  router.get('/summary', (_req: Request, res: Response) => {
    const rows = db
      .prepare(
        `SELECT event_type, COUNT(*) AS count, MAX(timestamp) AS last_timestamp
           FROM events
          WHERE event_type IN (${placeholders})
          GROUP BY event_type`
      )
      .all(...FINANCIAL_EVENT_TYPES) as Array<{
      event_type: string;
      count: number;
      last_timestamp: number;
    }>;

    const counts = {
      swap: 0,
      add_liquidity: 0,
      remove_liquidity: 0,
    };
    let lastTimestamp = 0;
    for (const row of rows) {
      const normalized = classifyFinancialEvent(row.event_type);
      if (normalized) counts[normalized] += row.count;
      lastTimestamp = Math.max(lastTimestamp, row.last_timestamp || 0);
    }

    res.json(
      payload({
        counts,
        last_event_at:
          lastTimestamp > 0 ? new Date(lastTimestamp * 1_000).toISOString() : null,
        coverage: {
          pools: true,
          trades: true,
          liquidity_events: true,
          lp_positions: false,
          usd_volume: false,
        },
      })
    );
  });

  return router;
}
