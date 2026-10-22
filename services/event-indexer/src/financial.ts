/**
 * Financial-data views derived from indexed contract events.
 *
 * This is a read-only projection over the `events` table. Core pool and
 * lending views do not call RPC/Horizon. They expose only values the contracts
 * actually emit; missing enrichment is reported as unavailable rather than
 * inferred. USD valuation is an explicit opt-in adapter that calls the
 * controller price API and refuses stale or degraded prices.
 *
 * Event payload versions:
 * - Legacy swap/liquidity payloads: (amount_a, amount_b, shares?)
 * - CPMM swap: (amount_in, amount_out, token_in, token_out,
 *   reserve_a_after, reserve_b_after, fee_bps, total_shares_after)
 * - Stableswap swap: the CPMM fields plus amplification.
 * - CPMM/stableswap liquidity: (amount_a, amount_b, shares, token_a, token_b,
 *   reserve_a_after, reserve_b_after, total_shares_after, amplification?)
 * - Lending: position snapshots and pool totals after every state change.
 */
import { Router, Request, Response } from 'express';
import Database from 'better-sqlite3';
import { scValToNative, xdr } from '@stellar/stellar-sdk';

type JsonObject = Record<string, unknown>;
type PoolKind = 'cpmm' | 'stableswap' | 'unknown';
type PoolEventType = 'swap' | 'add_liquidity' | 'remove_liquidity';
type LendingEventType =
  | 'deposit'
  | 'withdraw'
  | 'borrow'
  | 'repay'
  | 'set_collateral'
  | 'liquidate'
  | 'accrue';

const FINANCIAL_EVENT_TYPES = [
  'swap',
  'ss_swap',
  'add_liq',
  'ss_add',
  'rm_liq',
  'ss_rm',
] as const;

const LENDING_EVENT_TYPES = [
  'deposit',
  'withdraw',
  'borrow',
  'repay',
  'set_col',
  'liquidate',
  'accrue',
] as const;

interface PoolEvent {
  id: string;
  contract_id: string;
  contract_name: string;
  pool_kind: PoolKind;
  event_type: PoolEventType;
  account: string | null;
  tx_hash: string;
  ledger: number;
  timestamp: number;
  amount_a_raw: string | null;
  amount_b_raw: string | null;
  shares_raw: string | null;
  token_in: string | null;
  token_out: string | null;
  token_a: string | null;
  token_b: string | null;
  reserve_a_after: string | null;
  reserve_b_after: string | null;
  total_shares_after: string | null;
  fee_bps: number | null;
  amplification: string | null;
  usd_value: null;
  data: unknown;
}

interface PositionSnapshot {
  deposit_raw: string;
  borrow_raw: string;
  collateral_raw: string;
}

interface LendingEvent {
  id: string;
  contract_id: string;
  contract_name: string;
  event_type: LendingEventType;
  account: string | null;
  counterparty: string | null;
  accounts: string[];
  tx_hash: string;
  ledger: number;
  timestamp: number;
  amount_raw: string | null;
  secondary_amount_raw: string | null;
  position: PositionSnapshot | null;
  totals: {
    total_supply_raw: string | null;
    total_borrows_raw: string | null;
    total_reserves_raw: string | null;
  };
  data: unknown;
}

interface ValuationOptions {
  priceApiUrl?: string;
  tokenAssets?: Record<string, string>;
  tokenDecimals?: Record<string, number>;
  fetchImpl?: typeof fetch;
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

function isAccount(value: unknown): value is string {
  return typeof value === 'string' && /^G[A-Z2-7]{55}$/.test(value);
}

function isContract(value: unknown): value is string {
  return typeof value === 'string' && /^C[A-Z2-7]{55}$/.test(value);
}

function addressOf(value: unknown): string | null {
  return isAccount(value) || isContract(value) ? value : null;
}

function accountOf(row: JsonObject): string | null {
  for (const topic of topicsOf(row)) {
    if (isAccount(topic)) return topic;
  }
  return null;
}

function rawAmount(value: unknown): string | null {
  if (typeof value === 'bigint') return value.toString();
  if (typeof value === 'number' && Number.isFinite(value)) return String(value);
  if (typeof value === 'string' && /^-?\d+$/.test(value)) return value;
  return null;
}

function numeric(value: unknown): number | null {
  if (typeof value === 'number' && Number.isFinite(value)) return value;
  if (typeof value === 'bigint') return Number(value);
  if (typeof value === 'string' && value.length > 0 && Number.isFinite(Number(value))) {
    return Number(value);
  }
  return null;
}

function poolKind(contractName: string, eventType: string): PoolKind {
  if (contractName.toLowerCase().includes('stable') || eventType.startsWith('ss_')) {
    return 'stableswap';
  }
  if (
    contractName.toLowerCase().includes('cpmm') ||
    eventType === 'swap' ||
    eventType === 'add_liq' ||
    eventType === 'rm_liq'
  ) {
    return 'cpmm';
  }
  return 'unknown';
}

export function classifyFinancialEvent(eventType: string): PoolEventType | null {
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

export function classifyLendingEvent(eventType: string): LendingEventType | null {
  switch (eventType) {
    case 'deposit':
    case 'withdraw':
    case 'borrow':
    case 'repay':
    case 'liquidate':
    case 'accrue':
      return eventType;
    case 'set_col':
      return 'set_collateral';
    default:
      return null;
  }
}

export function parseFinancialEvent(row: JsonObject): PoolEvent | null {
  const eventType = String(row.event_type || '');
  const normalized = classifyFinancialEvent(eventType);
  if (!normalized) return null;

  const data = parseJson(row.data, null);
  const values = Array.isArray(data) ? data : [];
  const isSwap = normalized === 'swap';

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
    shares_raw: isSwap ? null : rawAmount(values[2]),
    token_in: isSwap ? addressOf(values[2]) : null,
    token_out: isSwap ? addressOf(values[3]) : null,
    token_a: isSwap ? addressOf(values[8]) : addressOf(values[3]),
    token_b: isSwap ? addressOf(values[9]) : addressOf(values[4]),
    reserve_a_after: rawAmount(values[isSwap ? 4 : 5]),
    reserve_b_after: rawAmount(values[isSwap ? 5 : 6]),
    total_shares_after: rawAmount(values[7]),
    fee_bps: isSwap ? numeric(values[6]) : null,
    amplification: isSwap ? rawAmount(values[10]) : rawAmount(values[8]),
    usd_value: null,
    data,
  };
}

export function parseLendingEvent(row: JsonObject): LendingEvent | null {
  const eventType = String(row.event_type || '');
  const normalized = classifyLendingEvent(eventType);
  if (!normalized) return null;

  const data = parseJson(row.data, null);
  const values = Array.isArray(data) ? data : [];
  const topics = topicsOf(row);
  const accounts = topics.filter(isAccount);

  let account: string | null = accounts[0] || null;
  let counterparty: string | null = null;
  if (normalized === 'liquidate') {
    // The contract publishes (liquidate, liquidator, borrower). The position
    // snapshot belongs to the borrower.
    counterparty = accounts[0] || null;
    account = accounts[1] || null;
  }

  const position = (depositIndex: number): PositionSnapshot | null => {
    const deposit = rawAmount(values[depositIndex]);
    const borrow = rawAmount(values[depositIndex + 1]);
    const collateral = rawAmount(values[depositIndex + 2]);
    if (deposit === null || borrow === null || collateral === null) return null;
    return { deposit_raw: deposit, borrow_raw: borrow, collateral_raw: collateral };
  };

  let snapshot: PositionSnapshot | null = null;
  let amount: string | null = rawAmount(values[0]);
  let secondary: string | null = null;
  let totalSupply: string | null = null;
  let totalBorrows: string | null = null;
  let totalReserves: string | null = null;

  switch (normalized) {
    case 'deposit':
      secondary = rawAmount(values[1]);
      snapshot = position(2);
      totalSupply = rawAmount(values[5]);
      totalBorrows = rawAmount(values[6]);
      totalReserves = rawAmount(values[7]);
      break;
    case 'withdraw':
      secondary = rawAmount(values[1]);
      snapshot = position(2);
      totalSupply = rawAmount(values[5]);
      totalBorrows = rawAmount(values[6]);
      totalReserves = rawAmount(values[7]);
      break;
    case 'borrow':
    case 'repay':
    case 'set_collateral':
      snapshot = position(1);
      totalSupply = rawAmount(values[4]);
      totalBorrows = rawAmount(values[5]);
      totalReserves = rawAmount(values[6]);
      break;
    case 'liquidate':
      secondary = rawAmount(values[1]);
      snapshot = position(2);
      totalSupply = rawAmount(values[5]);
      totalBorrows = rawAmount(values[6]);
      totalReserves = rawAmount(values[7]);
      break;
    case 'accrue':
      amount = rawAmount(values[0]);
      secondary = rawAmount(values[1]);
      totalBorrows = rawAmount(values[2]);
      totalReserves = rawAmount(values[3]);
      break;
  }

  return {
    id: String(row.event_id || `event_${row.id}`),
    contract_id: String(row.contract_id || ''),
    contract_name: String(row.contract_name || ''),
    event_type: normalized,
    account,
    counterparty,
    accounts,
    tx_hash: String(row.tx_hash || ''),
    ledger: Number(row.ledger || 0),
    timestamp: Number(row.timestamp || 0),
    amount_raw: amount,
    secondary_amount_raw: secondary,
    position: snapshot,
    totals: {
      total_supply_raw: totalSupply,
      total_borrows_raw: totalBorrows,
      total_reserves_raw: totalReserves,
    },
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

function latestPoolState(events: PoolEvent[]) {
  let latest: {
    reserve_a_raw: string;
    reserve_b_raw: string;
    total_shares_raw: string;
    token_a: string | null;
    token_b: string | null;
    observed_at: string;
    ledger: number;
  } | null = null;
  let feeBps: number | null = null;
  let amplification: string | null = null;

  for (const event of events) {
    if (
      latest === null &&
      event.reserve_a_after !== null &&
      event.reserve_b_after !== null &&
      event.total_shares_after !== null
    ) {
      latest = {
        reserve_a_raw: event.reserve_a_after,
        reserve_b_raw: event.reserve_b_after,
        total_shares_raw: event.total_shares_after,
        token_a: event.token_a ?? event.token_in,
        token_b: event.token_b ?? event.token_out,
        observed_at: new Date(event.timestamp * 1_000).toISOString(),
        ledger: event.ledger,
      };
    }
    // Liquidity events do not repeat the fee/amplification. Preserve the most
    // recent known value instead of dropping it when a later event omits it.
    if (feeBps === null && event.fee_bps !== null) feeBps = event.fee_bps;
    if (amplification === null && event.amplification !== null) {
      amplification = event.amplification;
    }
  }

  return latest ? { ...latest, fee_bps: feeBps, amplification } : null;
}

function lpPositions(events: PoolEvent[], limit = 100) {
  const shares = new Map<string, bigint>();
  let latestTotal: bigint | null = null;

  for (const event of events) {
    if (!event.account || event.shares_raw === null) continue;
    const amount = BigInt(event.shares_raw);
    const current = shares.get(event.account) || 0n;
    shares.set(
      event.account,
      event.event_type === 'remove_liquidity' ? current - amount : current + amount
    );
    if (latestTotal === null && event.total_shares_after !== null) {
      latestTotal = BigInt(event.total_shares_after);
    }
  }

  const positions = [...shares.entries()]
    .filter(([, value]) => value > 0n)
    .map(([account, value]) => ({
      account,
      shares_raw: value.toString(),
      share_pct:
        latestTotal && latestTotal > 0n
          ? Number((value * 1_000_000n) / latestTotal) / 10_000
          : null,
    }))
    .sort((a, b) => (BigInt(b.shares_raw) > BigInt(a.shares_raw) ? 1 : -1));

  return {
    available: positions.length > 0,
    derived_from: 'liquidity_events',
    total_shares_raw: latestTotal?.toString() ?? null,
    positions: positions.slice(0, limit),
    note:
      'LP shares are internal contract balances and are derived by netting add/remove events. Transfers are not supported by the pool contracts.',
  };
}

function tokenVolume(events: PoolEvent[]) {
  const byToken = new Map<string, bigint>();
  let unattributed = 0n;

  for (const event of events) {
    if (event.event_type !== 'swap' || event.amount_a_raw === null) continue;
    if (!event.token_in) {
      unattributed += BigInt(event.amount_a_raw);
      continue;
    }
    byToken.set(
      event.token_in,
      (byToken.get(event.token_in) || 0n) + BigInt(event.amount_a_raw)
    );
  }

  return {
    amount_in_raw: sumRaw(events.filter((e) => e.event_type === 'swap').map((e) => e.amount_a_raw)),
    amount_out_raw: sumRaw(events.filter((e) => e.event_type === 'swap').map((e) => e.amount_b_raw)),
    by_token_in_raw: Object.fromEntries(
      [...byToken.entries()].map(([token, amount]) => [token, amount.toString()])
    ),
    unattributed_raw: unattributed.toString(),
    unit: 'raw_token_units',
    usd_value: null as number | null,
    note:
      'Raw amounts are never converted to USD without a fresh, configured price and token decimals.',
  };
}

function parseTokenMap(value: string | undefined): Record<string, string> {
  if (!value) return {};
  try {
    const parsed = JSON.parse(value);
    if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed)) return {};
    return Object.fromEntries(
      Object.entries(parsed).filter(
        ([token, asset]) =>
          token.length > 0 && typeof asset === 'string' && asset.length > 0
      )
    ) as Record<string, string>;
  } catch {
    return {};
  }
}

function parseDecimalsMap(value: string | undefined): Record<string, number> {
  if (!value) return {};
  try {
    const parsed = JSON.parse(value);
    if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed)) return {};
    return Object.fromEntries(
      Object.entries(parsed).filter(
        ([, decimals]) =>
          typeof decimals === 'number' &&
          Number.isInteger(decimals) &&
          decimals >= 0 &&
          decimals <= 24
      )
    ) as Record<string, number>;
  } catch {
    return {};
  }
}

function usd(raw: string | null, decimals: number, price: number): number | null {
  if (raw === null || !/^-?\d+$/.test(raw)) return null;
  return (Number(BigInt(raw)) / 10 ** decimals) * price;
}

async function fetchPrice(
  fetchImpl: typeof fetch,
  priceApiUrl: string,
  asset: string
): Promise<
  | {
      asset: string;
      price_usd: number;
      timestamp: string | null;
      trust_state: string | null;
      is_fresh: boolean;
      source: string;
    }
  | { error: string }
> {
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), 4_000);
  try {
    const response = await fetchImpl(
      `${priceApiUrl.replace(/\/$/, '')}/price?asset=${encodeURIComponent(asset)}`,
      { signal: controller.signal }
    );
    if (!response.ok) {
      return { error: `price API returned ${response.status} for ${asset}` };
    }
    const body = (await response.json()) as Record<string, unknown>;
    const price = numeric(body.price_usd);
    if (price === null || price <= 0) {
      return { error: `price API returned no positive price for ${asset}` };
    }
    const trust = body.trust as Record<string, unknown> | undefined;
    return {
      asset,
      price_usd: price,
      timestamp: typeof body.timestamp === 'string' ? body.timestamp : null,
      trust_state: typeof trust?.state === 'string' ? trust.state : null,
      is_fresh: body.is_fresh === true,
      source: 'controller_price_api',
    };
  } catch (error: any) {
    return { error: `price API request failed for ${asset}: ${error?.message || error}` };
  } finally {
    clearTimeout(timer);
  }
}

export function createFinancialRouter(
  db: Database.Database,
  payload: (body: Record<string, unknown>) => Record<string, unknown>,
  options: ValuationOptions = {}
): Router {
  const router = Router();
  const placeholders = FINANCIAL_EVENT_TYPES.map(() => '?').join(',');
  const lendingPlaceholders = LENDING_EVENT_TYPES.map(() => '?').join(',');
  const priceApiUrl = options.priceApiUrl ?? process.env.INDEXER_PRICE_API_URL;
  const tokenAssets =
    options.tokenAssets ?? parseTokenMap(process.env.INDEXER_TOKEN_ASSETS);
  const tokenDecimals =
    options.tokenDecimals ?? parseDecimalsMap(process.env.INDEXER_TOKEN_DECIMALS);
  const fetchImpl = options.fetchImpl ?? fetch;
  const valuationConfigured =
    Boolean(priceApiUrl) &&
    Object.keys(tokenAssets).length > 0 &&
    Object.keys(tokenDecimals).length > 0;

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
          token_direction: true,
          pool_reserves: true,
          lp_positions: true,
          usd_volume: valuationConfigured,
          reason: valuationConfigured
            ? 'USD valuation is available per pool through /financial/pools/:contractId/valuation.'
            : 'Set INDEXER_PRICE_API_URL, INDEXER_TOKEN_ASSETS and INDEXER_TOKEN_DECIMALS to enable USD valuation.',
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
      .filter((event): event is PoolEvent => event !== null);
    const trades = events.filter((event) => event.event_type === 'swap');
    const adds = events.filter((event) => event.event_type === 'add_liquidity');
    const removes = events.filter((event) => event.event_type === 'remove_liquidity');
    const state = latestPoolState(events);
    const positions = lpPositions(events);

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
          state,
          volume: tokenVolume(events),
          lp_positions: positions,
          recent_events: events.slice(0, 20),
        },
      })
    );
  });

  router.get('/pools/:contractId/lp-positions', (req: Request, res: Response) => {
    const rows = db
      .prepare(
        `SELECT * FROM events
          WHERE contract_id = ? AND event_type IN (${placeholders})
          ORDER BY timestamp DESC, id DESC
          LIMIT 1000`
      )
      .all(req.params.contractId, ...FINANCIAL_EVENT_TYPES) as JsonObject[];
    const events = rows
      .map(parseFinancialEvent)
      .filter((event): event is PoolEvent => event !== null);
    const positions = lpPositions(events);
    if (rows.length === 0) {
      const contract = db
        .prepare('SELECT * FROM contracts WHERE contract_id = ?')
        .get(req.params.contractId) as JsonObject | undefined;
      if (!contract) {
        res.status(404).json({ error: 'Pool not found' });
        return;
      }
    }
    res.json(payload({ data: positions }));
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
          .filter((event): event is PoolEvent => event !== null),
        pagination: pagination(limit, offset, total),
        coverage: {
          token_in: true,
          token_out: true,
          reserves_after: true,
          usd_value: false,
          reason:
            'Token direction and post-trade reserves come from enriched events. USD values require /valuation with fresh prices.',
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
          .filter((event): event is PoolEvent => event !== null),
        pagination: pagination(limit, offset, total),
        coverage: {
          lp_positions: true,
          reason:
            'LP positions are derived by netting add/remove share events; see /pools/:contractId/lp-positions.',
        },
      })
    );
  });

  router.get('/pools/:contractId/valuation', async (req: Request, res: Response) => {
    if (!priceApiUrl) {
      res.status(503).json({
        error: 'Valuation unavailable',
        message: 'INDEXER_PRICE_API_URL is not configured.',
      });
      return;
    }
    const rows = db
      .prepare(
        `SELECT * FROM events
          WHERE contract_id = ? AND event_type IN (${placeholders})
          ORDER BY timestamp DESC, id DESC
          LIMIT 1000`
      )
      .all(req.params.contractId, ...FINANCIAL_EVENT_TYPES) as JsonObject[];
    const events = rows
      .map(parseFinancialEvent)
      .filter((event): event is PoolEvent => event !== null);
    const state = latestPoolState(events);
    if (!state || !state.token_a || !state.token_b) {
      res.status(503).json({
        error: 'Valuation unavailable',
        message: 'The pool has no enriched reserve/token event yet.',
      });
      return;
    }

    const assetA = tokenAssets[state.token_a];
    const assetB = tokenAssets[state.token_b];
    const decimalsA = tokenDecimals[state.token_a];
    const decimalsB = tokenDecimals[state.token_b];
    if (!assetA || !assetB || decimalsA === undefined || decimalsB === undefined) {
      res.status(503).json({
        error: 'Valuation unavailable',
        message:
          'Token asset/decimals mapping is incomplete. Configure INDEXER_TOKEN_ASSETS and INDEXER_TOKEN_DECIMALS.',
        missing: {
          token_a: !assetA || decimalsA === undefined ? state.token_a : null,
          token_b: !assetB || decimalsB === undefined ? state.token_b : null,
        },
      });
      return;
    }

    const [priceA, priceB] = await Promise.all([
      fetchPrice(fetchImpl, priceApiUrl, assetA),
      fetchPrice(fetchImpl, priceApiUrl, assetB),
    ]);
    const failed = [priceA, priceB].find((price) => 'error' in price);
    if (failed && 'error' in failed) {
      res.status(503).json({ error: 'Valuation unavailable', message: failed.error });
      return;
    }
    const a = priceA as Exclude<typeof priceA, { error: string }>;
    const b = priceB as Exclude<typeof priceB, { error: string }>;
    if (!a.is_fresh || !b.is_fresh) {
      res.status(503).json({
        error: 'Valuation unavailable',
        message: 'A price is stale; refusing to publish a USD valuation.',
        prices: [a, b],
      });
      return;
    }
    if (a.trust_state !== 'HEALTHY' || b.trust_state !== 'HEALTHY') {
      res.status(503).json({
        error: 'Valuation unavailable',
        message: 'A price is not in HEALTHY trust state; refusing to publish a USD valuation.',
        prices: [a, b],
      });
      return;
    }

    const volume = tokenVolume(events);
    const byTokenUsd = Object.fromEntries(
      Object.entries(volume.by_token_in_raw).map(([token, raw]) => {
        if (token === state.token_a) return [token, usd(raw, decimalsA, a.price_usd)];
        if (token === state.token_b) return [token, usd(raw, decimalsB, b.price_usd)];
        return [token, null];
      })
    );
    const hasUnmappedVolume = Object.values(byTokenUsd).some(
      (value) => typeof value !== 'number' || !Number.isFinite(value)
    );
    const usdVolume =
      Object.values(byTokenUsd).reduce<number>(
        (total, value) => total + (typeof value === 'number' ? value : 0),
        0
      ) +
      (volume.unattributed_raw !== '0' ? Number.NaN : 0);
    if (hasUnmappedVolume || !Number.isFinite(usdVolume)) {
      res.status(503).json({
        error: 'Valuation unavailable',
        message: hasUnmappedVolume
          ? 'Some swap volume uses a token without a configured asset/decimals mapping.'
          : 'Some swap volume has no token direction; a complete USD total is unavailable.',
      });
      return;
    }

    res.json(
      payload({
        data: {
          contract_id: req.params.contractId,
          source: 'indexed_contract_events',
          as_of: {
            ledger: state.ledger,
            observed_at: state.observed_at,
          },
          state,
          prices: [a, b],
          reserves_usd: {
            token_a: usd(state.reserve_a_raw, decimalsA, a.price_usd),
            token_b: usd(state.reserve_b_raw, decimalsB, b.price_usd),
            total:
              (usd(state.reserve_a_raw, decimalsA, a.price_usd) ?? 0) +
              (usd(state.reserve_b_raw, decimalsB, b.price_usd) ?? 0),
          },
          volume_usd: usdVolume,
          volume_by_token_in_usd: byTokenUsd,
          valuation_source: 'controller_price_api',
          note:
            'This is a live valuation, not a historical price series. Stale or degraded prices return 503.',
        },
      })
    );
  });

  router.get('/lending/events', (req: Request, res: Response) => {
    const { limit, offset } = page(req);
    const clause = `event_type IN (${lendingPlaceholders})`;
    const rows = eventRows(db, clause, [...LENDING_EVENT_TYPES], limit, offset);
    const total = countRows(db, clause, [...LENDING_EVENT_TYPES]);
    res.json(
      payload({
        data: rows
          .map(parseLendingEvent)
          .filter((event): event is LendingEvent => event !== null),
        pagination: pagination(limit, offset, total),
        coverage: {
          positions: true,
          totals: true,
          note: 'Position snapshots are emitted after each lending state change.',
        },
      })
    );
  });

  router.get('/lending/accounts/:accountId', (req: Request, res: Response) => {
    const account = req.params.accountId;
    if (!isAccount(account)) {
      res.status(400).json({ error: 'Invalid account', message: 'Expected a Stellar account address.' });
      return;
    }
    // Account topics are XDR strings in `raw`; decode a bounded recent window
    // and select the newest event that contains this account.
    const scanLimit = 10_000;
    const rows = eventRows(
      db,
      `event_type IN (${lendingPlaceholders})`,
      [...LENDING_EVENT_TYPES],
      scanLimit,
      0
    );
    const events = rows
      .map(parseLendingEvent)
      .filter((event): event is LendingEvent => event !== null)
      .filter((event) => event.accounts.includes(account));
    const latest = events.find((event) => event.position !== null) || events[0] || null;
    if (!latest) {
      res.status(404).json({
        error: 'Lending position not found',
        account,
        scanned_events: rows.length,
        scan_limit: scanLimit,
      });
      return;
    }
    res.json(
      payload({
        data: {
          account,
          contract_id: latest.contract_id,
          contract_name: latest.contract_name,
          position: latest.position,
          totals: latest.totals,
          as_of: {
            ledger: latest.ledger,
            timestamp: new Date(latest.timestamp * 1_000).toISOString(),
            event_id: latest.id,
            event_type: latest.event_type,
          },
          recent_events: events.slice(0, 20),
          source: 'indexed_contract_events',
        },
        coverage: {
          scanned_events: rows.length,
          scan_limit: scanLimit,
          note:
            'Derived from a bounded recent-event scan; it can lag the chain and does not read live contract storage.',
        },
      })
    );
  });

  router.get('/lending/summary', (_req: Request, res: Response) => {
    const rows = db
      .prepare(
        `SELECT event_type, COUNT(*) AS count, MAX(timestamp) AS last_timestamp
           FROM events
          WHERE event_type IN (${lendingPlaceholders})
          GROUP BY event_type`
      )
      .all(...LENDING_EVENT_TYPES) as Array<{
      event_type: string;
      count: number;
      last_timestamp: number;
    }>;
    const counts: Record<string, number> = {};
    let lastTimestamp = 0;
    for (const row of rows) {
      const normalized = classifyLendingEvent(row.event_type);
      if (normalized) counts[normalized] = (counts[normalized] || 0) + row.count;
      lastTimestamp = Math.max(lastTimestamp, row.last_timestamp || 0);
    }
    res.json(
      payload({
        counts,
        last_event_at:
          lastTimestamp > 0 ? new Date(lastTimestamp * 1_000).toISOString() : null,
        coverage: {
          lending_events: true,
          position_snapshots: true,
          live_contract_state: false,
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
        (event): event is PoolEvent =>
          event !== null && event.account === req.params.accountId
      );
    res.json(
      payload({
        data: events.slice(offset, offset + limit),
        pagination: pagination(limit, offset, events.length),
        coverage: {
          scanned_events: rows.length,
          scan_limit: scanLimit,
          lp_positions: true,
          reason:
            'Activity is derived from a bounded recent-event scan. LP positions are available from /pools/:contractId/lp-positions.',
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
          token_direction: true,
          pool_reserves: true,
          lp_positions: true,
          usd_volume: valuationConfigured,
          lending_events: true,
          lending_positions: true,
          note: valuationConfigured
            ? 'USD valuation is opt-in per pool and refuses stale/degraded prices.'
            : 'USD valuation requires INDEXER_PRICE_API_URL, INDEXER_TOKEN_ASSETS and INDEXER_TOKEN_DECIMALS.',
        },
      })
    );
  });

  return router;
}
